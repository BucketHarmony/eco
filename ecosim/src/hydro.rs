//! Storms, surface runoff, ponding and soil water (shot G4).
//!
//! Three grids meet here. Rain falls on the **ground grid** (0.5 m cells in the Capitol bundle,
//! 1 m cells in a noise world), runs downhill over it and ponds in its depressions. What soaks in
//! fills a soil-water store on the **ecology grid** (1 m columns), and that store is what the
//! `moisture` field now reports: `255 × min(1, soil_water / field_capacity)`.
//!
//! The flow graph is built once, at load: the ground doesn't move, so neither do the depressions,
//! the receivers or the order they are visited in. Every storm is one pass over that order.
//!
//! Water is counted in a millimetre-cubic-metre ledger (`Ledger`), in f64, so that after every tick
//! `rain = Δponded + Δsoil + evaporation + ET + drainage + outflow` holds to rounding. The two
//! stores are f64 in memory for the same reason — the acceptance asks for 1e-9 relative, and f32
//! accumulation over 20000 ticks cannot hold it; the snapshot files stay f32 and u16
//! (DECISIONS.md, shot G4).

use crate::bundle::Medium;
use crate::events::{Detail, EventKind};
use crate::params::Params;
use crate::sim::Sim;
use crate::world::{ColClass, World};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Hours in a year, the one place a tick's length in physical time comes from (shot G4b). A tick is
/// `HOURS_PER_YEAR / climate.year_len` hours, so at the default 4000 ticks a year it is 2.1915 h and
/// a 20000-tick run is five years. Every rate in `params.toml` is per hour or per year and is turned
/// into a per-update amount through [`tick_hours`] or [`years`]; nothing is per tick.
pub const HOURS_PER_YEAR: f64 = 8766.0;

/// How many years `ticks` ticks are.
pub fn years(p: &Params, ticks: u32) -> f64 {
    ticks as f64 / p.climate.year_len.max(1) as f64
}

/// Hours in one tick.
pub fn tick_hours(p: &Params) -> f64 {
    HOURS_PER_YEAR * years(p, 1)
}

/// A receiver that carries water out of the world (the edge, or a cell of open water).
pub const OUT: u32 = u32::MAX;

/// A monotone map from f32 to u32: `key(a) <= key(b)` exactly when `a <= b`, for every finite
/// value. It lets the priority flood order elevations with integer keys, so ties break on the cell
/// index and nothing depends on a float comparison.
fn key(v: f32) -> u32 {
    let b = v.to_bits();
    if b & 0x8000_0000 != 0 {
        !b
    } else {
        b | 0x8000_0000
    }
}

/// The static routing of one ground grid: where each cell's surplus goes, how much it ponds first,
/// and the order a storm visits the cells in.
#[derive(Debug, Clone)]
pub struct Flow {
    /// Cells along x and y.
    pub dims: (usize, usize),
    /// Downhill neighbour of each cell, or [`OUT`].
    pub recv: Vec<u32>,
    /// Every cell exactly once, each before its receiver.
    pub order: Vec<u32>,
    /// Depression storage of each cell: its spill level above its own ground, in mm.
    pub pond_cap: Vec<f32>,
    /// Ecology column under each cell.
    pub col: Vec<u32>,
    /// Ground cells per ecology column.
    pub per_col: f64,
    /// Area of one ground cell, m².
    pub cell_area: f64,
}

/// The four-neighbourhood of a cell, in a fixed order.
fn neighbours(i: usize, w: usize, h: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (i % w, i / w);
    [(0i32, -1i32), (-1, 0), (1, 0), (0, 1)].into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        ((0..w as i32).contains(&nx) && (0..h as i32).contains(&ny)).then(|| nx as usize + w * ny as usize)
    })
}

impl Flow {
    /// Build the routing for a world: a priority flood of the ground surface (Barnes et al. 2014,
    /// ties broken by cell index) gives every cell a spill level and a receiver on the filled
    /// surface; roofs are lifted out of that surface and routed to their downspout instead; and a
    /// topological sort turns the receiver tree into the order a storm walks.
    pub fn build(world: &World) -> Flow {
        let g = &world.ground_grid;
        let (w, h) = (g.width, g.depth);
        let n = w * h;
        let roof = |i: usize| g.medium_at(i) == Medium::Roof;
        // 1. The surface the flood runs on: ground height in mm, with roofs above everything, so
        //    the flood reaches them last and no ground cell ever drains into one.
        let peak = world.ground_h.iter().copied().fold(0.0f32, f32::max);
        let elev: Vec<f32> =
            (0..n).map(|i| 1000.0 * (world.ground_h[i] + if roof(i) { peak + 1.0 } else { 0.0 })).collect();
        // 2. Priority flood from every outlet: the world's edge and every cell of open water.
        let outlet = |i: usize| {
            let (x, y) = (i % w, i / w);
            x == 0 || y == 0 || x + 1 == w || y + 1 == h || g.medium_at(i) == Medium::Water
        };
        let mut filled = vec![f32::NAN; n];
        let mut recv = vec![OUT; n];
        let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::with_capacity(n);
        for i in (0..n).filter(|&i| outlet(i)) {
            filled[i] = elev[i];
            heap.push(Reverse((key(elev[i]), i as u32)));
        }
        while let Some(Reverse((_, c))) = heap.pop() {
            let c = c as usize;
            for d in neighbours(c, w, h) {
                if filled[d].is_nan() {
                    filled[d] = elev[d].max(filled[c]);
                    recv[d] = c as u32;
                    heap.push(Reverse((key(filled[d]), d as u32)));
                }
            }
        }
        // 3. Roofs: every cell of a building sends its water to one downspout, the lowest-indexed
        //    non-roof cell touching the building.
        let mut seen = vec![false; n];
        let mut queue: Vec<usize> = Vec::new();
        for start in 0..n {
            if !roof(start) || seen[start] {
                continue;
            }
            let (mut spout, mut at) = (OUT, 0);
            queue.clear();
            queue.push(start);
            seen[start] = true;
            while at < queue.len() {
                let c = queue[at];
                at += 1;
                for d in neighbours(c, w, h) {
                    if roof(d) {
                        if !seen[d] {
                            seen[d] = true;
                            queue.push(d);
                        }
                    } else {
                        spout = spout.min(d as u32);
                    }
                }
            }
            for &c in &queue {
                recv[c] = spout;
            }
        }
        // 4. Depression storage, in mm of water depth. A roof holds none.
        let pond_cap: Vec<f32> =
            (0..n).map(|i| if roof(i) || outlet(i) { 0.0 } else { (filled[i] - elev[i]).max(0.0) }).collect();
        let order = topological(&mut recv, n);
        let col: Vec<u32> = (0..n).map(|i| ((i % w) / g.ratio + world.dims.wx * ((i / w) / g.ratio)) as u32).collect();
        let cell = g.cell_m() as f64;
        Flow { dims: (w, h), recv, order, pond_cap, col, per_col: g.cells_per_column() as f64, cell_area: cell * cell }
    }

    /// Number of cells.
    pub fn cells(&self) -> usize {
        self.recv.len()
    }

    /// The two ends of the ponded-depth ramp, in millimetres: the scale `meta.json` publishes for
    /// the `water` overlay (shot S10). Logarithmic, and the top is the site's own.
    ///
    /// A renderer colouring `water.bin` has to decide what depth is the top of its ramp, and until
    /// this shot no run said. The number is not a matter of taste: ponded depth on a real site spans
    /// decades, because most wet ground is a film a few millimetres deep and a few cells are the
    /// bottom of a bowl metres deep. On the Capitol at tick 10000 the median wet cell holds 10 mm,
    /// the ninetieth percentile 48 mm and the deepest 5,074 mm (`ecoview-native/MEASUREMENTS.md`,
    /// S5), so a linear ramp to the deepest draws 99% of the standing water in its bottom band and
    /// the site looks dry.
    ///
    /// **The top is the deepest water this site can hold, rounded up to a decade.** Every cell's
    /// [`Flow::pond_cap`] is its spill level: fill it past that and the surplus moves downhill
    /// within the same storm pass, so no snapshot can show a cell deeper than its own cap, and the
    /// largest cap is the deepest standing water the terrain admits. That is a fact about the
    /// ground rather than about one run's weather -- it is computed once, at load, from the same
    /// priority flood the storms use -- which is what the scale of an overlay has to be if two
    /// snapshots of the same site are to be comparable at a glance
    /// (`ecoview-native/DECISIONS.md`, "V2 the scale is the simulator's range, never the data's").
    ///
    /// Rounding up to a decade keeps the ramp a whole number of decades, so the bands sit on
    /// millimetre, centimetre, decimetre and metre; it also stops the scale twitching between two
    /// sites that differ only in where their deepest pit happens to be. An exact decade is left
    /// alone. The floor is [`POND_RAMP_MIN_HI_MM`]: a site with no depression at all still needs a
    /// scale, because a storm can stand water on a slope for a tick before it runs off.
    ///
    /// `deepest` is compared against a running product of ten rather than fed to a logarithm: the
    /// decades are exact in `f32` and the answer needs no `libm` (DECISIONS.md, shot 4).
    pub fn pond_ramp_mm(&self) -> (f32, f32) {
        let deepest = self.pond_cap.iter().copied().fold(0.0f32, f32::max);
        let mut hi = POND_RAMP_MIN_HI_MM;
        while hi < deepest {
            hi *= 10.0;
        }
        (POND_RAMP_LO_MM, hi)
    }
}

/// The bottom of the ponded-depth ramp, in millimetres.
///
/// `water.bin` is quantised to 0.1 mm, so this is ten quanta, and it is the depth at which wet
/// ground becomes water standing on it. Below it the field still says what it says; it is the ramp
/// that stops resolving, and "no water at all" is a category off the bottom of the ramp rather than
/// its first band (shot S2 left fire's quiet ground to the renderer for the same reason).
pub const POND_RAMP_LO_MM: f32 = 1.0;

/// The shallowest top the ponded-depth ramp is given, in millimetres: two decades above
/// [`POND_RAMP_LO_MM`].
///
/// A world whose ground has no depression -- a roof, a plane, a bare slope -- would otherwise be
/// handed a ramp with no room in it. A tenth of a metre is a deep puddle and a shallow pond, so it
/// is where a site that holds nothing is still drawn honestly.
pub const POND_RAMP_MIN_HI_MM: f32 = 100.0;

/// Kahn's topological sort of the receiver graph, upstream first. A cell caught in a cycle (a
/// courtyard whose only way out is over a roof) has its receiver cut to [`OUT`], lowest index
/// first, until the order covers every cell; that is deterministic and loses no water.
fn topological(recv: &mut [u32], n: usize) -> Vec<u32> {
    let mut indeg = vec![0u32; n];
    for &r in recv.iter() {
        if r != OUT {
            indeg[r as usize] += 1;
        }
    }
    let mut order: Vec<u32> = Vec::with_capacity(n);
    let mut queue: Vec<u32> = (0..n as u32).filter(|&i| indeg[i as usize] == 0).collect();
    let mut cut = 0usize;
    loop {
        while let Some(c) = queue.pop() {
            order.push(c);
            let r = recv[c as usize];
            if r != OUT {
                indeg[r as usize] -= 1;
                if indeg[r as usize] == 0 {
                    queue.push(r);
                }
            }
        }
        if order.len() == n {
            return order;
        }
        // Everything left is in a cycle: break the lowest-indexed one open.
        let mut done = vec![false; n];
        for &c in &order {
            done[c as usize] = true;
        }
        let c = (cut..n).find(|&i| !done[i]).expect("a cycle has a cell");
        cut = c + 1;
        indeg[recv[c] as usize] -= 1;
        recv[c] = OUT;
        queue.push(c as u32);
    }
}

/// The running water balance of a run, in mm·m² (a litre), all of it f64.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ledger {
    /// Rain that has fallen.
    pub rain: f64,
    /// Ponded water that has evaporated.
    pub evap: f64,
    /// Soil water lost to evaporation, transpiration and plant draw.
    pub et: f64,
    /// Soil water that has drained out of the bottom of a column.
    pub drain: f64,
    /// Water that has left the world, over its edge or into open water.
    pub outflow: f64,
    /// Ponded plus soil water at tick 0.
    pub start: f64,
}

/// The six water columns of one `series.csv` row, as world means in millimetres.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Water {
    /// Rain during this tick.
    pub rain_mm: f32,
    /// Rain that ran off the cell it fell on instead of soaking in or ponding there. Water passing
    /// through a cell on its way downhill is not counted again: what leaves a cell is split between
    /// the rain that fell on it and the run-on it received, in proportion. So this is at most
    /// `rain_mm`, and the ratio of the two is the storm's runoff coefficient.
    pub runoff_mm: f32,
    /// Ponded water now standing on the ground.
    pub ponded_mm: f32,
    /// Soil water now held in the columns.
    pub soil_water_mm: f32,
    /// Water that drained out of the bottom of the soil during this tick.
    pub drainage_mm: f32,
    /// Water that left the world during this tick.
    pub outflow_mm: f32,
}

impl Water {
    /// The six values in `series.csv` column order.
    pub fn as_array(self) -> [f32; 6] {
        [self.rain_mm, self.runoff_mm, self.ponded_mm, self.soil_water_mm, self.drainage_mm, self.outflow_mm]
    }

    /// The inverse of [`Water::as_array`].
    pub fn from_array(v: [f32; 6]) -> Water {
        Water {
            rain_mm: v[0],
            runoff_mm: v[1],
            ponded_mm: v[2],
            soil_water_mm: v[3],
            drainage_mm: v[4],
            outflow_mm: v[5],
        }
    }
}

/// Surface and soil water: the two stores, the medium rates behind them and the ledger.
#[derive(Debug, Clone)]
pub struct Hydro {
    /// The static routing.
    pub flow: Flow,
    /// Ponded water per ground cell, mm.
    pub ponded: Vec<f64>,
    /// Soil water per ecology column, mm.
    pub soil: Vec<f64>,
    /// Field capacity per ecology column, mm; 0 where nothing roots. Water above it percolates.
    pub capacity: Vec<f64>,
    /// The most water a column's soil can hold, `capacity × hydro.saturation`, mm.
    pub store_max: Vec<f64>,
    /// Infiltration rate per ground cell, mm per hour.
    pub infiltration: Vec<f32>,
    /// Percolation rate per ecology column, mm per hour.
    pub percolation: Vec<f32>,
    /// Run-on waiting at each cell during a storm pass; zero between storms.
    runon: Vec<f64>,
    /// The run's water balance.
    pub ledger: Ledger,
    /// This tick's row values.
    pub water: Water,
    /// Total area of the world, m².
    pub area: f64,
}

impl Hydro {
    /// The stores and rates a world starts with: every column filled to `hydro.initial_fill` of
    /// its capacity, nothing ponded.
    pub fn new(world: &World, params: &Params) -> Hydro {
        let flow = Flow::build(world);
        let g = &world.ground_grid;
        let (cols, cells) = (world.dims.cols(), flow.cells());
        let media = &params.medium;
        let infiltration: Vec<f32> = (0..cells).map(|i| media.get(g.medium_at(i)).infiltration_mm_h).collect();
        let mut capacity = vec![0.0f64; cols];
        let mut percolation = vec![0.0f32; cols];
        for i in 0..cells {
            let m = media.get(g.medium_at(i));
            let c = flow.col[i] as usize;
            capacity[c] += m.field_capacity_mm as f64 / flow.per_col;
            percolation[c] += m.percolation_mm_h / flow.per_col as f32;
        }
        for c in 0..cols {
            if world.class[c] != ColClass::Soil {
                capacity[c] = 0.0;
                percolation[c] = 0.0;
            }
        }
        let soil: Vec<f64> = capacity.iter().map(|&f| f * params.hydro.initial_fill.clamp(0.0, 1.0) as f64).collect();
        let saturation = params.hydro.saturation.max(1.0) as f64;
        let store_max: Vec<f64> = capacity.iter().map(|&f| f * saturation).collect();
        let start = soil.iter().sum::<f64>();
        Hydro {
            ponded: vec![0.0; cells],
            water: Water { soil_water_mm: (start / cols as f64) as f32, ..Water::default() },
            ledger: Ledger { start, ..Ledger::default() },
            area: cols as f64,
            soil,
            capacity,
            store_max,
            infiltration,
            percolation,
            runon: vec![0.0; cells],
            flow,
        }
    }

    /// Ponded plus soil water, in mm·m².
    pub fn storage(&self) -> f64 {
        self.ponded.iter().sum::<f64>() * self.flow.cell_area + self.soil.iter().sum::<f64>()
    }

    /// How far the ledger is from closing, relative to the rain that has fallen:
    /// `rain − (Δstorage + evaporation + ET + drainage + outflow)`.
    pub fn balance_error(&self) -> f64 {
        let l = &self.ledger;
        let out = self.storage() - l.start + l.evap + l.et + l.drain + l.outflow;
        let scale = l.rain.abs().max(l.start.abs()).max(1.0);
        (l.rain - out) / scale
    }

    /// Refresh the storage columns of the row values.
    fn observe(&mut self) {
        self.water.ponded_mm = (self.ponded.iter().sum::<f64>() * self.flow.cell_area / self.area) as f32;
        self.water.soil_water_mm = (self.soil.iter().sum::<f64>() / self.area) as f32;
    }
}

impl Sim {
    /// The moisture a column's soil water shows: `255 × min(1, soil / capacity)`, and 0 where
    /// nothing roots.
    pub fn derive_moisture(&mut self, c: usize) {
        if let Some(h) = &self.hydro {
            let cap = h.capacity[c];
            self.moisture[c] = if cap > 0.0 { (255.0 * (h.soil[c] / cap).min(1.0)) as f32 } else { 0.0 };
        }
    }

    /// How wet a column is, as a fraction of its available water capacity: 0 at the wilting point
    /// and 1 at field capacity, and above 1 while it is draining. Every plant's moisture curve is
    /// read on this scale (shot G4b). With the water tier off there is no store, so the pre-G4
    /// index is reported on the same scale.
    pub fn water_fraction(&self, c: usize) -> f32 {
        match &self.hydro {
            Some(h) if h.capacity[c] > 0.0 => (h.soil[c] / h.capacity[c]) as f32,
            Some(_) => 0.0,
            None => self.moisture[c] / 255.0,
        }
    }

    /// Take `mm` millimetres of water out of a column, as a plant does when it transpires. With the
    /// water tier on this is soil water leaving, charged to the ledger. With it off there is no
    /// store to take from, so the demand is converted to the pre-G4 index at the soil medium's
    /// capacity, which is the bed the index was calibrated on (shot G4b; the pre-G4 moisture model
    /// itself is left in its old units, UNITS.md "Deferred to shot G4c").
    pub fn draw_water_mm(&mut self, c: usize, mm: f32) {
        match &mut self.hydro {
            None => {
                let cap = self.params.medium.soil.field_capacity_mm.max(f32::MIN_POSITIVE);
                self.moisture[c] = (self.moisture[c] - 255.0 * mm / cap).max(0.0)
            }
            Some(h) => {
                let taken = h.soil[c].min(mm as f64).max(0.0);
                h.soil[c] -= taken;
                h.ledger.et += taken;
                self.derive_moisture(c);
            }
        }
    }

    /// The temperature factor of a patch: the old evaporation rate at its temperature over the
    /// same rate at the mean temperature, so it is 1 in an average patch and never negative.
    fn temp_factor(&self, patch: usize) -> f64 {
        let c = &self.params.climate;
        let at = |t: f32| (c.evap_base + t / c.evap_div).max(0.0) as f64;
        let norm = at(c.temp_base);
        if norm <= 0.0 {
            0.0
        } else {
            at(self.patches[patch].temperature) / norm
        }
    }

    /// One tick of rain: `rain.annual_mm` millimetres fall a year in storms of mean depth
    /// `rain.storm_mean_mm`, so a storm starts with probability
    /// `annual_mm / (year_len × storm_mean_mm) × the season's rain factor` (that factor averages 1
    /// over a year), its depth is drawn exponentially, and the whole depth falls and routes in this
    /// tick. Exactly one draw on a dry tick and two on a wet one.
    pub fn storm(&mut self, tick: u32) {
        self.hydro.as_mut().expect("storm needs the water tier").water = Water::default();
        let rp = self.params.rain.clone();
        let season = if self.params.climate.rain_base > 0.0 {
            (self.rain(tick) / self.params.climate.rain_base).max(0.0)
        } else {
            0.0
        };
        let per_year = (self.params.climate.year_len.max(1) as f64 * rp.storm_mean_mm as f64).max(f64::MIN_POSITIVE);
        let p = (rp.annual_mm as f64 / per_year * season as f64).clamp(0.0, 1.0);
        let u: f64 = rand::Rng::gen(&mut self.rng);
        if u >= p {
            let h = self.hydro.as_mut().expect("water tier");
            h.observe();
            return;
        }
        let v: f64 = rand::Rng::gen(&mut self.rng);
        let depth = -(rp.storm_mean_mm as f64) * libm::log((1.0 - v).max(f64::MIN_POSITIVE));
        self.route_storm(depth);
    }

    /// Send one storm of `depth` mm over the ground grid: rain, run-on, infiltration, ponding and
    /// what is left going downhill, in one pass over the flow order.
    pub(crate) fn route_storm(&mut self, depth: f64) {
        let tick_h = tick_hours(&self.params);
        let gradient = self.params.climate.rain_gradient;
        let width = self.world.dims.wx;
        let h = self.hydro.as_mut().expect("water tier");
        let (area, per_col) = (h.flow.cell_area, h.flow.per_col);
        let (mut rain, mut runoff, mut outflow) = (0.0f64, 0.0f64, 0.0f64);
        for i in 0..h.runon.len() {
            h.runon[i] = 0.0;
        }
        for k in 0..h.flow.order.len() {
            let i = h.flow.order[k] as usize;
            let c = h.flow.col[i] as usize;
            let fall = if gradient == 0.0 {
                depth
            } else {
                crate::abiotic::rain_at(depth as f32, gradient, c % width, width) as f64
            };
            rain += fall;
            let mut w = fall + h.runon[i];
            // Infiltration, limited by the medium's rate and by what the column can still hold.
            let room = ((h.store_max[c] - h.soil[c]).max(0.0) * per_col).min(h.infiltration[i] as f64 * tick_h);
            let take = w.min(room);
            h.soil[c] += take / per_col;
            w -= take;
            // Depression storage, then everything above the spill level moves on.
            let free = (h.flow.pond_cap[i] as f64 - h.ponded[i]).max(0.0);
            let store = w.min(free);
            h.ponded[i] += store;
            w -= store;
            if w > 0.0 {
                // The share of what leaves that the cell's own rain paid for (see `Water::runoff_mm`).
                runoff += w * fall / (fall + h.runon[i]);
                match h.flow.recv[i] {
                    OUT => outflow += w,
                    r => h.runon[r as usize] += w,
                }
            }
        }
        h.ledger.rain += rain * area;
        h.ledger.outflow += outflow * area;
        h.water = Water {
            rain_mm: (rain * area / h.area) as f32,
            runoff_mm: (runoff * area / h.area) as f32,
            outflow_mm: (outflow * area / h.area) as f32,
            ..Water::default()
        };
        h.observe();
        let w = h.water;
        for c in 0..self.world.dims.cols() {
            self.derive_moisture(c);
        }
        self.log_with(EventKind::Storm, "", 0, None, "", Detail::Storm(w.rain_mm, w.runoff_mm, w.outflow_mm));
    }

    /// The between-storm half of the soil update, run with the every-10-ticks moisture step:
    /// ponded water soaks in and evaporates, soil water above field capacity drains away, and what
    /// is left evaporates and transpires. Returns the drainage of each column, in mm, for leaching.
    pub fn settle_water(&mut self, hours: f64) -> Vec<f32> {
        let d = self.world.dims;
        let cols = d.cols();
        let (mut tf, mut cover) = (vec![0.0f64; d.patches()], vec![0.0f64; d.patches()]);
        for p in 0..d.patches() {
            tf[p] = self.temp_factor(p);
            cover[p] = (self.patches[p].grass + self.patches[p].shrub).clamp(0.0, 1.0) as f64;
        }
        let (evap_mm, et_mm) = (self.params.hydro.evap_mm_h as f64 * hours, self.params.hydro.et_mm_h as f64 * hours);
        let mut drained = vec![0.0f32; cols];
        let h = self.hydro.as_mut().expect("water tier");
        let per_col = h.flow.per_col;
        let (mut evap, mut drain, mut et) = (0.0f64, 0.0f64, 0.0f64);
        // 1. Ponded water: soak in first, then evaporate what is still standing.
        for i in 0..h.ponded.len() {
            if h.ponded[i] <= 0.0 {
                continue;
            }
            let c = h.flow.col[i] as usize;
            let room = ((h.store_max[c] - h.soil[c]).max(0.0) * per_col).min(h.infiltration[i] as f64 * hours);
            let take = h.ponded[i].min(room);
            h.soil[c] += take / per_col;
            h.ponded[i] -= take;
            let patch = d.patch_of(c % d.wx, c / d.wx);
            let gone = h.ponded[i].min(evap_mm * tf[patch]);
            h.ponded[i] -= gone;
            evap += gone * h.flow.cell_area;
        }
        // 2. Percolation out of the bottom, then evaporation and transpiration from what is left.
        for (c, into_soil) in drained.iter_mut().enumerate() {
            if h.capacity[c] <= 0.0 {
                continue;
            }
            let excess = (h.soil[c] - h.capacity[c]).max(0.0);
            let out = excess.min(h.percolation[c] as f64 * hours);
            h.soil[c] -= out;
            *into_soil = out as f32;
            drain += out;
            let patch = d.patch_of(c % d.wx, c / d.wx);
            let want = et_mm * tf[patch] * (0.2 + 0.8 * cover[patch]);
            let gone = h.soil[c].min(want);
            h.soil[c] -= gone;
            et += gone;
        }
        h.ledger.evap += evap;
        h.ledger.drain += drain;
        h.ledger.et += et;
        h.water.drainage_mm = (drain / h.area) as f32;
        h.observe();
        for c in 0..cols {
            self.derive_moisture(c);
        }
        drained
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests::{build, bundle_params, flat_bundle, paint};
    use crate::bundle::Bundle;
    use proptest::prelude::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// How far the ledger may be from closing. The acceptance asks for 1e-9 relative.
    const EPS: f64 = 1e-9;

    /// A tick-0 sim on a bundle, animals off, with the water tier on.
    fn sim_of(b: &Bundle) -> Sim {
        let mut p = bundle_params(b);
        p.animals.enabled = false;
        let world = World::from_bundle(b, &p).unwrap();
        Sim::with_bundle(p, ChaCha8Rng::seed_from_u64(1), world, b).0
    }

    /// Paint every ground cell of the bundle with one medium.
    fn cover(b: &mut Bundle, m: Medium) {
        for y in 0..b.size_m {
            for x in 0..b.size_m {
                paint(b, x, y, m);
            }
        }
    }

    fn hydro(s: &Sim) -> &Hydro {
        s.hydro.as_ref().expect("the water tier is on")
    }

    fn ponded(s: &Sim) -> f64 {
        hydro(s).ponded.iter().sum()
    }

    fn soil(s: &Sim) -> f64 {
        hydro(s).soil.iter().sum()
    }

    /// Acceptance: a tilted plane of asphalt keeps nothing. Every cell sends its water to its
    /// downhill neighbour, and all of it leaves at the low edge.
    #[test]
    fn tilted_asphalt_sends_every_drop_to_the_low_edge() {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Asphalt);
        // 0.1 m per cell down to the east.
        for i in 0..b.ground_h.len() {
            b.ground_h[i] = 0.1 * (b.ground.width - 1 - i % b.ground.width) as f32;
        }
        let mut s = sim_of(&b);
        let (w, h) = hydro(&s).flow.dims;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                assert_eq!(hydro(&s).flow.recv[x + w * y], (x + 1 + w * y) as u32, "cell ({x}, {y}) drains east");
            }
        }
        s.route_storm(10.0);
        let l = hydro(&s).ledger;
        assert!((l.outflow - l.rain).abs() < EPS * l.rain, "all {} mm left the world, not {}", l.rain, l.outflow);
        assert_eq!((ponded(&s), soil(&s)), (0.0, 0.0), "asphalt neither ponds nor holds water");
        assert!(hydro(&s).balance_error().abs() < EPS);
    }

    /// Acceptance: a bowl fills before it spills. A storm smaller than the depression stays in it;
    /// a larger one leaves the depression full and sends the rest out of the world.
    #[test]
    fn a_bowl_fills_then_spills() {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Asphalt);
        let (w, d) = (b.ground.width, b.ground.depth);
        // A cone draining inward: every ring is lower than the ring outside it.
        for i in 0..b.ground_h.len() {
            let (x, y) = ((i % w) as f32, (i / w) as f32);
            let r = (x - (w - 1) as f32 / 2.0).abs().max((y - (d - 1) as f32 / 2.0).abs());
            b.ground_h[i] = 0.01 * r;
        }
        let mut s = sim_of(&b);
        let capacity: f64 = hydro(&s).flow.pond_cap.iter().map(|&v| v as f64).sum();
        assert!(capacity > 0.0, "the cone is a depression");
        let small = capacity / (w * d) as f64 / 2.0;
        // The rim is the world's edge, so the rain that falls on it leaves; the rest runs inward.
        let (rim, inside) = ((2 * w + 2 * d - 4) as f64, ((w - 2) * (d - 2)) as f64);
        s.route_storm(small);
        let l = hydro(&s).ledger;
        let area = hydro(&s).flow.cell_area;
        assert!((l.outflow - small * rim * area).abs() < 1e-9 * l.rain, "only the rim sheds: {}", l.outflow);
        assert!((ponded(&s) - small * inside).abs() < 1e-9 * l.rain, "the rest stays in the bowl");
        s.route_storm(10.0 * small);
        let l = hydro(&s).ledger;
        assert!((ponded(&s) - capacity).abs() < 1e-6 * capacity, "the bowl is full: {} of {capacity}", ponded(&s));
        assert!(l.outflow > 0.0, "the rest spills out");
        assert!(hydro(&s).balance_error().abs() < EPS);
    }

    /// Acceptance: a roof sends its rain down at its downspout. A block of roof with one lawn cell
    /// beside it puts the whole world's rain into that cell's column.
    #[test]
    fn a_roof_sends_its_rain_to_its_downspout() {
        let mut b = flat_bundle(8, 1);
        for y in 0..b.size_m {
            for x in 0..b.size_m {
                if (x, y) != (0, 0) {
                    build(&mut b, x, y, 4.0);
                }
            }
        }
        let mut s = sim_of(&b);
        assert!((1..64).all(|i| hydro(&s).flow.recv[i] == 0), "every roof cell drains to the lawn cell");
        let depth = 0.5;
        let before = soil(&s);
        s.route_storm(depth);
        let h = hydro(&s);
        assert!((h.soil[0] - before - 64.0 * depth).abs() < EPS * 64.0, "the corner column took {} mm", h.soil[0]);
        assert_eq!((ponded(&s), h.ledger.outflow), (0.0, 0.0), "nothing ponds and nothing leaves");
        assert!(h.balance_error().abs() < EPS);
    }

    /// Acceptance: lawn takes a small storm whole and sheds a large one. The split is the medium's
    /// infiltration rate times the hours in a tick.
    #[test]
    fn lawn_absorbs_a_small_storm_and_sheds_a_large_one() {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Lawn);
        let mut s = sim_of(&b);
        let rate = s.params.medium.lawn.infiltration_mm_h as f64 * tick_hours(&s.params);
        assert!((5.0..60.0).contains(&rate), "5 mm is under one tick of infiltration and 60 mm is over it");
        let before = soil(&s);
        s.route_storm(5.0);
        assert_eq!(hydro(&s).water.runoff_mm, 0.0, "5 mm all soaks in");
        assert!((soil(&s) - before - 5.0 * 64.0).abs() < EPS * 64.0);
        let mut s = sim_of(&b);
        s.route_storm(60.0);
        assert!(hydro(&s).water.runoff_mm > 0.0, "60 mm runs off");
        assert!(hydro(&s).ledger.outflow > 0.0, "and leaves the world");
        assert!((soil(&s) - before - rate * 64.0).abs() < 1e-6 * 64.0, "every cell took one tick of infiltration");
        assert!(hydro(&s).balance_error().abs() < EPS);
    }

    /// A world of random media and heights, with random storms and settling between them. The
    /// ledger closes after every step: rain = change in ponded and soil water + evaporation + ET +
    /// drainage + outflow.
    fn balance_holds(media: &[u8], heights: &[u8], storms: &[u8]) -> Result<(), TestCaseError> {
        let size = 8;
        let mut b = flat_bundle(size, 1);
        for (i, &m) in media.iter().enumerate() {
            let medium = Medium::ALL[m as usize % Medium::ALL.len()];
            let (x, y) = (i % size, i / size);
            paint(&mut b, x, y, medium);
            if medium == Medium::Roof {
                build(&mut b, x, y, 3.0);
            }
        }
        for (i, &h) in heights.iter().enumerate() {
            b.ground_h[i] = h as f32 / 64.0;
        }
        let mut s = sim_of(&b);
        for (t, &d) in storms.iter().enumerate() {
            s.route_storm(d as f64 / 4.0);
            prop_assert!(hydro(&s).balance_error().abs() < EPS, "storm {}: {}", t, hydro(&s).balance_error());
            s.settle_water(10.0 * tick_hours(&s.params));
            prop_assert!(hydro(&s).balance_error().abs() < EPS, "settle {}: {}", t, hydro(&s).balance_error());
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(8)))]

        #[test]
        fn prop_water_balance_closes(
            media in prop::collection::vec(any::<u8>(), 64),
            heights in prop::collection::vec(any::<u8>(), 64),
            storms in prop::collection::vec(any::<u8>(), 1..12),
        ) {
            balance_holds(&media, &heights, &storms)?;
        }
    }

    /// The regression sibling of `prop_water_balance_closes`: one world with every medium in it, a
    /// roof block, a slope and storms big enough to fill and spill its depressions.
    #[test]
    fn water_balance_regression_every_medium() {
        let media: Vec<u8> = (0..64).collect();
        let heights: Vec<u8> = (0..64).map(|i| ((i % 8) * (i / 8)) as u8).collect();
        balance_holds(&media, &heights, &[0, 1, 40, 255, 3, 0, 200]).unwrap();
    }

    /// Is `hi` one of the decades [`Flow::pond_ramp_mm`] is allowed to return?
    fn is_decade(hi: f32) -> bool {
        let mut d = POND_RAMP_MIN_HI_MM;
        while d < hi {
            d *= 10.0;
        }
        d == hi
    }

    /// A flat bundle whose interior cell (4, 4) sits `dip` metres below the rest.
    fn bundle_with_a_pit(dip: f32) -> Bundle {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Asphalt);
        let w = b.ground.width;
        b.ground_h[4 + w * 4] = -dip;
        b
    }

    /// Acceptance (shot S10): the published ponded-depth ramp starts at a millimetre and ends at the
    /// deepest water the ground can hold, rounded up to a decade. It is the scale `meta.json` gives
    /// a renderer for `water.bin`, so what it is measured from is the point.
    #[test]
    fn the_pond_ramp_ends_at_the_deepest_bowl_rounded_up() {
        // A plane with nowhere to hold water still gets a ramp: the floor, two decades wide.
        let mut flat = flat_bundle(8, 1);
        cover(&mut flat, Medium::Asphalt);
        let s = sim_of(&flat);
        assert_eq!(hydro(&s).flow.pond_cap.iter().copied().fold(0.0f32, f32::max), 0.0, "a plane holds nothing");
        assert_eq!(hydro(&s).flow.pond_ramp_mm(), (1.0, POND_RAMP_MIN_HI_MM));
        // 0.4 m of storage is over the floor and under a metre, so the ramp runs to a metre.
        let s = sim_of(&bundle_with_a_pit(0.4));
        assert_eq!(hydro(&s).flow.pond_cap[4 + 8 * 4], 400.0, "the pit's spill level is 400 mm above it");
        assert_eq!(hydro(&s).flow.pond_ramp_mm(), (1.0, 1000.0));
        // An exact decade is left alone rather than rounded to the next one.
        let s = sim_of(&bundle_with_a_pit(1.0));
        assert_eq!(hydro(&s).flow.pond_ramp_mm(), (1.0, 1000.0));
        // A metre and a bit needs the decade above it.
        let s = sim_of(&bundle_with_a_pit(1.2));
        assert_eq!(hydro(&s).flow.pond_ramp_mm(), (1.0, 10_000.0));
    }

    /// The invariant behind the ramp: no cell can hold more than its own depression storage, so the
    /// top of the ramp bounds every depth any snapshot of this world can show.
    fn ramp_bounds_every_pond(media: &[u8], heights: &[u8], storms: &[u8]) -> Result<(), TestCaseError> {
        let size = 8;
        let mut b = flat_bundle(size, 1);
        for (i, &m) in media.iter().enumerate() {
            let medium = Medium::ALL[m as usize % Medium::ALL.len()];
            let (x, y) = (i % size, i / size);
            paint(&mut b, x, y, medium);
            if medium == Medium::Roof {
                build(&mut b, x, y, 3.0);
            }
        }
        for (i, &h) in heights.iter().enumerate() {
            b.ground_h[i] = h as f32 / 64.0;
        }
        let mut s = sim_of(&b);
        let (lo, hi) = hydro(&s).flow.pond_ramp_mm();
        prop_assert_eq!(lo, POND_RAMP_LO_MM);
        prop_assert!(is_decade(hi), "{} is not a decade", hi);
        for (t, &d) in storms.iter().enumerate() {
            s.route_storm(d as f64 / 4.0);
            let deepest = hydro(&s).ponded.iter().copied().fold(0.0, f64::max);
            prop_assert!(deepest <= hi as f64, "storm {}: {} mm stands, over a ramp to {}", t, deepest, hi);
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(8)))]

        #[test]
        fn prop_pond_ramp_bounds_every_pond(
            media in prop::collection::vec(any::<u8>(), 64),
            heights in prop::collection::vec(any::<u8>(), 64),
            storms in prop::collection::vec(any::<u8>(), 1..12),
        ) {
            ramp_bounds_every_pond(&media, &heights, &storms)?;
        }
    }

    /// The regression sibling of `prop_pond_ramp_bounds_every_pond`: the world of
    /// `water_balance_regression_every_medium`, whose storms fill and spill its depressions.
    #[test]
    fn pond_ramp_regression_every_medium() {
        let media: Vec<u8> = (0..64).collect();
        let heights: Vec<u8> = (0..64).map(|i| ((i % 8) * (i / 8)) as u8).collect();
        ramp_bounds_every_pond(&media, &heights, &[0, 1, 40, 255, 3, 0, 200]).unwrap();
    }

    /// With `hydro.enabled` off there is no water tier at all, and the pre-G4 moisture update runs.
    #[test]
    fn the_rate_switch_leaves_the_tier_out() {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Lawn);
        let mut p = bundle_params(&b);
        p.animals.enabled = false;
        p.hydro.enabled = false;
        let world = World::from_bundle(&b, &p).unwrap();
        let (mut s, _) = Sim::with_bundle(p, ChaCha8Rng::seed_from_u64(1), world, &b);
        assert!(s.hydro.is_none());
        let start = s.moisture.clone();
        for _ in 0..10 {
            s.step();
        }
        assert!(s.hydro.is_none(), "no water tier is ever allocated");
        assert!(s.moisture != start, "the pre-G4 moisture update ran instead");
    }
}
