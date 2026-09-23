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
use crate::npk::{N, P};
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

/// One storm drain as the flow graph sees it (shot G6): a pipe of the scene placed on ground cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Drain {
    /// Index into `World::pipes`, which is also the pipe's index in `meta.json`'s `world.pipes`
    /// and the first number of its `pipe` events.
    pub pipe: u32,
    /// Ground cell of the inlet.
    pub inlet: u32,
    /// Ground cell the pipe empties onto, or [`OUT`] when its outlet is on or past the crop edge.
    pub outlet: u32,
    /// Capacity in cubic metres an hour, before `pipes.capacity_scale`.
    pub capacity_m3h: f64,
}

impl Drain {
    /// What the drain can take in one tick, in mm over its inlet cell: `capacity_m3h` × the hours
    /// in a tick × `pipes.capacity_scale`, spread over the cell's `area` m².
    pub fn capacity_mm(&self, tick_h: f64, scale: f64, area: f64) -> f64 {
        self.capacity_m3h * tick_h * scale * 1000.0 / area
    }
}

/// No drain has its inlet on this cell.
const NO_DRAIN: u32 = u32::MAX;

/// Place the scene's pipes on the ground grid and check that the network can drain.
///
/// A pipe's inlet is the ground cell under its first point. Its outlet is the cell under its last
/// point, or [`OUT`] when that point is on or past the crop edge: an edge cell is already a flood
/// outlet, so water put there would leave the world anyway, and the pipe is simply charged with it.
///
/// **The network must be acyclic.** Water delivered at an interior outlet runs downhill along the
/// receivers from there, so a pipe *feeds* every pipe whose inlet lies on that path. If following
/// "feeds" from some pipe comes back to it, the water in it would go round for ever, and the load
/// is refused with the cycle named. The returned list is sorted by inlet cell, then pipe index.
pub fn place_drains(flow: &Flow, world: &World) -> Result<Vec<Drain>, String> {
    let (w, h) = flow.dims;
    let cell = world.ground_grid.cell_m();
    let at = |p: [f32; 2]| -> Option<(usize, usize)> {
        let (x, y) = ((p[0] / cell).floor(), (p[1] / cell).floor());
        (x >= 0.0 && y >= 0.0 && x < w as f32 && y < h as f32).then_some((x as usize, y as usize))
    };
    let mut drains: Vec<Drain> = Vec::with_capacity(world.pipes.len());
    for (k, p) in world.pipes.iter().enumerate() {
        // The bundle loader has already refused an inlet outside the crop; one exactly on the far
        // edge belongs to the last cell.
        let ix = (p.inlet[0] / cell).floor().clamp(0.0, (w - 1) as f32) as usize;
        let iy = (p.inlet[1] / cell).floor().clamp(0.0, (h - 1) as f32) as usize;
        let outlet = match at(p.outlet) {
            Some((x, y)) if x > 0 && y > 0 && x + 1 < w && y + 1 < h => (x + w * y) as u32,
            _ => OUT,
        };
        drains.push(Drain { pipe: k as u32, inlet: (ix + w * iy) as u32, outlet, capacity_m3h: p.capacity_m3h as f64 });
    }
    drains.sort_by_key(|d| (d.inlet, d.pipe));
    // Which pipes each pipe feeds: every inlet on the downhill path from its outlet.
    let feeds: Vec<Vec<usize>> = drains
        .iter()
        .map(|d| {
            let mut fed = Vec::new();
            let mut c = d.outlet;
            while c != OUT {
                fed.extend((0..drains.len()).filter(|&j| drains[j].inlet == c));
                c = flow.recv[c as usize];
            }
            fed
        })
        .collect();
    // Depth-first search for a cycle: 0 unvisited, 1 on the stack, 2 finished.
    fn visit(d: usize, feeds: &[Vec<usize>], state: &mut [u8], stack: &mut Vec<usize>) -> Option<Vec<usize>> {
        state[d] = 1;
        stack.push(d);
        for &e in &feeds[d] {
            if state[e] == 1 {
                let from = stack.iter().position(|&s| s == e).expect("on the stack");
                let mut cycle = stack[from..].to_vec();
                cycle.push(e);
                return Some(cycle);
            }
            if state[e] == 0 {
                if let Some(c) = visit(e, feeds, state, stack) {
                    return Some(c);
                }
            }
        }
        stack.pop();
        state[d] = 2;
        None
    }
    let mut state = vec![0u8; drains.len()];
    let mut stack: Vec<usize> = Vec::new();
    for d in 0..drains.len() {
        if state[d] != 0 {
            continue;
        }
        if let Some(cycle) = visit(d, &feeds, &mut state, &mut stack) {
            let names: Vec<&str> = cycle.iter().map(|&k| world.pipes[drains[k].pipe as usize].id.as_str()).collect();
            return Err(format!(
                "pipes.json: the drain network has a cycle, {}: each pipe's outlet runs downhill to the next one's inlet",
                names.join(" -> ")
            ));
        }
    }
    Ok(drains)
}

/// One storm's account of one drain: what reached its inlet and what it took, in mm over the inlet
/// cell, and the phosphorus and nitrogen the captured water carried, in grams.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DrainTally {
    /// Water that reached the inlet, over every pass of the storm.
    pub arrived: f64,
    /// The part of it the drain took; `arrived − captured` is the overflow.
    pub captured: f64,
    /// Phosphorus and nitrogen, in that order, that went down with it.
    pub load: [f64; 2],
}

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

/// The two pipe columns of one `series.csv` row (shot G6), world means in millimetres over this
/// tick: the water the drains captured, and the part of it they emptied over the crop edge. The
/// rest of what they captured was put back on the ground inside the world in the same storm.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PipeRow {
    /// Water captured by every inlet this tick.
    pub in_mm: f32,
    /// Captured water that left the world through a pipe this tick.
    pub out_edge_mm: f32,
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
    /// The storm drains, sorted by inlet cell (shot G6). Empty on a world with no pipes, and when
    /// the network was refused (`pipe_error`).
    pub drains: Vec<Drain>,
    /// For each ground cell, the first entry of `drains` with its inlet there, or `NO_DRAIN`.
    /// Empty when there are no drains.
    drain_at: Vec<u32>,
    /// Why the scene's pipes could not be made a network, if they could not. `Sim::from_bundle`
    /// returns it as the load error.
    pub pipe_error: Option<String>,
    /// Infiltration each cell has taken during this storm, mm, so a later routing pass cannot
    /// give a cell a second tick's worth. Only kept when a drain empties inside the world.
    infiltrated: Vec<f64>,
    /// This tick's two pipe columns of `series.csv`.
    pub pipe_row: PipeRow,
    /// Each drain's account of the latest storm, in `drains` order. Only written while the
    /// network runs (`pipes.capacity_scale` above 0).
    pub drain_tally: Vec<DrainTally>,
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
        let (drains, pipe_error) = match place_drains(&flow, world) {
            Ok(d) => (d, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let mut drain_at = if drains.is_empty() { Vec::new() } else { vec![NO_DRAIN; cells] };
        for (k, d) in drains.iter().enumerate().rev() {
            drain_at[d.inlet as usize] = k as u32;
        }
        let infiltrated = if drains.iter().any(|d| d.outlet != OUT) { vec![0.0; cells] } else { Vec::new() };
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
            drains,
            drain_at,
            pipe_error,
            infiltrated,
            pipe_row: PipeRow::default(),
            drain_tally: Vec::new(),
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
        let h = self.hydro.as_mut().expect("storm needs the water tier");
        h.water = Water::default();
        h.pipe_row = PipeRow::default();
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
    ///
    /// With storm drains (shot G6) a cell with an inlet on it first gives each of its drains what
    /// arrives there -- its rain and its run-on, roof water included -- up to what that drain can
    /// still take this tick; the rest is the overflow and goes on as before. What a drain takes is
    /// charged to the ledger as outflow when it empties over the crop edge, and is put down as
    /// run-on at its outlet cell otherwise. Water put down inside the world is routed onward in a
    /// further pass over the same order, rain-free, until no drain has anything left to deliver;
    /// the network is acyclic ([`place_drains`]), so that is at most one pass per drain.
    pub(crate) fn route_storm(&mut self, depth: f64) {
        let tick_h = tick_hours(&self.params);
        let gradient = self.params.climate.rain_gradient;
        let width = self.world.dims.wx;
        let np = self.params.npk;
        let scale = self.params.pipes.capacity_scale as f64;
        let h = self.hydro.as_mut().expect("water tier");
        let (area, per_col) = (h.flow.cell_area, h.flow.per_col);
        let (mut rain, mut runoff, mut outflow) = (0.0f64, 0.0f64, 0.0f64);
        for i in 0..h.runon.len() {
            h.runon[i] = 0.0;
        }
        // The rate switch sits here, outside the loop: at 0 no drain is consulted, no pass is
        // added and nothing below reads a pipe.
        let piped = scale > 0.0 && !h.drains.is_empty();
        // What each drain can still take this tick, in mm over its inlet cell.
        let mut left: Vec<f64> = h.drains.iter().map(|d| d.capacity_mm(tick_h, scale, area)).collect();
        let mut tally = vec![DrainTally::default(); h.drains.len()];
        // What each drain took during the pass under way, and the load that came with it.
        let mut taken = vec![(0.0f64, [0.0f64; 2]); h.drains.len()];
        // The drains that took water at the cell being visited, and how much.
        let mut here: Vec<(usize, f64)> = Vec::new();
        let mut pipe_edge = 0.0f64;
        let track = piped && !h.infiltrated.is_empty();
        if track {
            h.infiltrated.iter_mut().for_each(|v| *v = 0.0);
        }
        // The nutrient load the same water carries (shot G5): phosphorus lifted off the ground it
        // runs over, and the share of a column's nitrogen that sits in the layer it mixes with.
        // Both ride the flow graph the water already built, which is why this is here and not in
        // `npk.rs` -- the order cells are visited in is what makes a load land downhill of where
        // it started.
        let mut npk = self.npk.as_mut();
        let mut out_npk = [0.0f64; 2];
        if let Some(n) = npk.as_deref_mut() {
            n.carry.clear();
            n.carry.resize(h.runon.len(), [0.0; 2]);
        }
        for pass in 0..=h.drains.len() {
            for k in 0..h.flow.order.len() {
                let i = h.flow.order[k] as usize;
                let c = h.flow.col[i] as usize;
                let fall = if pass > 0 {
                    // A later pass carries only what the drains put down: skip the dry cells.
                    let loaded = npk.as_deref().is_some_and(|n| n.carry[i] != [0.0; 2]);
                    if h.runon[i] == 0.0 && !loaded {
                        continue;
                    }
                    0.0
                } else if gradient == 0.0 {
                    depth
                } else {
                    crate::abiotic::rain_at(depth as f32, gradient, c % width, width) as f64
                };
                rain += fall;
                let mut w = fall + h.runon[i];
                // The drains on this cell take what arrives before anything soaks in.
                let mut captured = 0.0f64;
                if piped && h.drain_at[i] != NO_DRAIN {
                    here.clear();
                    let mut d = h.drain_at[i] as usize;
                    while d < h.drains.len() && h.drains[d].inlet as usize == i {
                        let got = w.min(left[d]);
                        tally[d].arrived += w;
                        tally[d].captured += got;
                        taken[d].0 += got;
                        left[d] -= got;
                        w -= got;
                        captured += got;
                        here.push((d, got));
                        d += 1;
                    }
                }
                // Infiltration, limited by the medium's rate and by what the column can still hold.
                let mut rate = h.infiltration[i] as f64 * tick_h;
                if track {
                    rate = (rate - h.infiltrated[i]).max(0.0);
                }
                let room = ((h.store_max[c] - h.soil[c]).max(0.0) * per_col).min(rate);
                let take = w.min(room);
                h.soil[c] += take / per_col;
                w -= take;
                if track {
                    h.infiltrated[i] += take;
                }
                // Depression storage, then everything above the spill level moves on.
                let free = (h.flow.pond_cap[i] as f64 - h.ponded[i]).max(0.0);
                let store = w.min(free);
                h.ponded[i] += store;
                w -= store;
                if let Some(n) = npk.as_deref_mut() {
                    let arrived = std::mem::take(&mut n.carry[i]);
                    let present = fall + h.runon[i];
                    // What the cell keeps of the load that reached it is what it kept of the water.
                    let kept = if present > 0.0 { ((take + store) / present).clamp(0.0, 1.0) } else { 1.0 };
                    // `carry` and `moving` hold phosphorus at 0 and nitrogen at 1, in that order.
                    let mut moving = arrived;
                    // A drain takes the load in the share it took of the water.
                    let mut drained = 0.0;
                    if captured > 0.0 {
                        for &(d, got) in &here {
                            let share = got / present;
                            taken[d].1[0] += arrived[0] * share;
                            taken[d].1[1] += arrived[1] * share;
                            drained += share;
                        }
                        let rest = (1.0 - drained).max(0.0);
                        moving = [arrived[0] * rest, arrived[1] * rest];
                    }
                    if n.plantable[c] {
                        for (slot, element) in [P, N].into_iter().enumerate() {
                            n.soil[element][c] += arrived[slot] * kept;
                            moving[slot] = if captured > 0.0 {
                                arrived[slot] * (1.0 - kept - drained).max(0.0)
                            } else {
                                arrived[slot] * (1.0 - kept)
                            };
                        }
                    }
                    // On a sealed surface nothing soaks in, so the whole load stays in the water.
                    if w > 0.0 && n.plantable[c] {
                        // Phosphorus goes with the soil the water lifts, so many grams a square metre
                        // per millimetre, capped at this cell's share of the column's pool.
                        let lift = (np.p_runoff_g_per_mm as f64 * w * area).min(n.soil[P][c].max(0.0) / per_col);
                        n.soil[P][c] -= lift;
                        moving[0] += lift;
                        // Dissolved nitrogen mixes with the column's own store, and only the share in
                        // the top of the profile is in reach of water that never gets in.
                        let mix = (np.n_runoff_frac as f64 * w / (w + h.soil[c])).clamp(0.0, 1.0);
                        let gone = n.soil[N][c].max(0.0) / per_col * mix;
                        n.soil[N][c] -= gone;
                        moving[1] += gone;
                    }
                    match h.flow.recv[i] {
                        OUT => {
                            out_npk[0] += moving[0];
                            out_npk[1] += moving[1];
                        }
                        r => {
                            n.carry[r as usize][0] += moving[0];
                            n.carry[r as usize][1] += moving[1];
                        }
                    }
                }
                if captured > 0.0 {
                    // Water that went down a drain ran off the cell too.
                    runoff += (w + captured) * fall / (fall + h.runon[i]);
                } else if w > 0.0 {
                    // The share of what leaves that the cell's own rain paid for (see `Water::runoff_mm`).
                    runoff += w * fall / (fall + h.runon[i]);
                }
                if w > 0.0 {
                    match h.flow.recv[i] {
                        OUT => outflow += w,
                        r => h.runon[r as usize] += w,
                    }
                }
            }
            if !piped {
                break;
            }
            // Empty the drains: over the edge, or onto the ground for the next pass.
            h.runon.iter_mut().for_each(|v| *v = 0.0);
            let mut inside = false;
            for ((d, t), tl) in h.drains.iter().zip(taken.iter_mut()).zip(tally.iter_mut()) {
                let (water, load) = std::mem::take(t);
                tl.load[0] += load[0];
                tl.load[1] += load[1];
                if d.outlet == OUT {
                    pipe_edge += water;
                    out_npk[0] += load[0];
                    out_npk[1] += load[1];
                } else if water > 0.0 || load != [0.0; 2] {
                    h.runon[d.outlet as usize] += water;
                    if let Some(n) = npk.as_deref_mut() {
                        n.carry[d.outlet as usize][0] += load[0];
                        n.carry[d.outlet as usize][1] += load[1];
                    }
                    inside = true;
                }
            }
            if !inside {
                break;
            }
            debug_assert!(
                pass < h.drains.len(),
                "an acyclic network of {} drains needs no more passes",
                h.drains.len()
            );
        }
        if let Some(n) = npk {
            n.ledger.outflow[P] += out_npk[0];
            n.ledger.outflow[N] += out_npk[1];
            n.row.outflow_p = (out_npk[0] / n.columns.max(1) as f64) as f32;
        }
        h.ledger.rain += rain * area;
        h.ledger.outflow += (outflow + pipe_edge) * area;
        h.water = Water {
            rain_mm: (rain * area / h.area) as f32,
            runoff_mm: (runoff * area / h.area) as f32,
            outflow_mm: (outflow * area / h.area) as f32,
            ..Water::default()
        };
        if piped {
            let captured: f64 = tally.iter().map(|t| t.captured).sum();
            h.pipe_row =
                PipeRow { in_mm: (captured * area / h.area) as f32, out_edge_mm: (pipe_edge * area / h.area) as f32 };
            h.drain_tally.clone_from(&tally);
        }
        h.observe();
        let w = h.water;
        let pipes: Vec<(usize, u32, DrainTally)> = if piped {
            h.drains.iter().zip(&tally).map(|(d, t)| (h.flow.col[d.inlet as usize] as usize, d.pipe, *t)).collect()
        } else {
            Vec::new()
        };
        for c in 0..self.world.dims.cols() {
            self.derive_moisture(c);
        }
        self.log_with(EventKind::Storm, "", 0, None, "", Detail::Storm(w.rain_mm, w.runoff_mm, w.outflow_mm));
        let wx = self.world.dims.wx;
        for (c, pipe, t) in pipes {
            let (x, y) = (c % wx, c / wx);
            let m3 = |mm: f64| (mm * area / 1000.0) as f32;
            let detail =
                Detail::Pipe(pipe, m3(t.captured), m3(t.arrived - t.captured), t.load[1] as f32, t.load[0] as f32);
            self.log_with(EventKind::Pipe, "", self.world.dims.patch_of(x, y), Some((x, y)), "", detail);
        }
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

    // ---- Storm drains (shot G6) ----

    use crate::bundle::Pipe;

    /// A pipe from the centre of cell `inlet` to the point `outlet` (metres), taking `mm` millimetres
    /// a tick over a 1 m cell at `pipes.capacity_scale` 1.
    fn pipe(id: &str, inlet: (usize, usize), outlet: [f32; 2], mm: f64, p: &Params) -> Pipe {
        Pipe {
            id: id.into(),
            inlet: [inlet.0 as f32 + 0.5, inlet.1 as f32 + 0.5],
            outlet,
            capacity_m3h: (mm / 1000.0 / tick_hours(p)) as f32,
            illustrative: true,
        }
    }

    /// The centre of cell (x, y), as a pipe's outlet.
    fn centre(x: usize, y: usize) -> [f32; 2] {
        [x as f32 + 0.5, y as f32 + 0.5]
    }

    /// Asphalt tilted down to the east, 0.1 m a cell: every interior cell drains to its east
    /// neighbour and out of the world at the east edge.
    fn tilted_east() -> Bundle {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Asphalt);
        for i in 0..b.ground_h.len() {
            b.ground_h[i] = 0.1 * (b.ground.width - 1 - i % b.ground.width) as f32;
        }
        b
    }

    /// The capacity of drain `d` this tick, in mm over its inlet cell.
    fn capacity(s: &Sim, d: usize) -> f64 {
        let h = hydro(s);
        h.drains[d].capacity_mm(tick_hours(&s.params), s.params.pipes.capacity_scale as f64, h.flow.cell_area)
    }

    /// Acceptance: a V-shaped valley with an inlet at the bottom drains into the pipe until the pipe
    /// is full, and then ponds. The valley floor falls south, and one floor cell is a pit 0.3 m deep,
    /// the only depression on the site; the inlet sits in it and the pipe empties over the south edge.
    #[test]
    fn a_valley_drains_into_its_inlet_until_the_pipe_is_full_then_ponds() {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Asphalt);
        let (w, cx) = (b.ground.width, 4usize);
        for i in 0..b.ground_h.len() {
            let (x, y) = (i % w, i / w);
            b.ground_h[i] = 0.1 * (x as f32 - cx as f32).abs() + 0.05 * y as f32;
        }
        let pit = cx + w * 2;
        b.ground_h[pit] = -0.2;
        let p = bundle_params(&b);
        // How much reaches the pit from a 2 mm storm: every cell whose water passes through it.
        let depth = 2.0;
        let arrival = |s: &Sim| {
            let f = &hydro(s).flow;
            let passes = |mut c: u32| loop {
                if c as usize == pit {
                    return true;
                }
                if c == OUT {
                    return false;
                }
                c = f.recv[c as usize];
            };
            depth * (0..f.cells() as u32).filter(|&c| passes(c)).count() as f64
        };
        for (full, mm) in [(false, 1e6), (true, 0.0)] {
            let mut b = b.clone();
            // With `full` the pipe takes half of what reaches it; otherwise far more than all of it.
            let s0 = sim_of(&b);
            let reach = arrival(&s0);
            assert!(reach > 10.0 * depth, "the pit collects the valley, {reach} mm");
            assert!(hydro(&s0).flow.pond_cap[pit] > 0.0, "the pit is a depression");
            let mm = if full { reach / 2.0 } else { mm };
            b.pipes = vec![pipe("sump", (cx, 2), [cx as f32 + 0.5, -1.0], mm, &p)];
            let mut s = sim_of(&b);
            assert_eq!(hydro(&s).drains.len(), 1);
            assert_eq!(hydro(&s).drains[0].outlet, OUT, "an outlet past the edge drains out of the world");
            s.route_storm(depth);
            let h = hydro(&s);
            let t = h.drain_tally[0];
            assert!((t.arrived - reach).abs() < 1e-9 * reach, "{} mm reached the inlet, not {reach}", t.arrived);
            if full {
                let cap = capacity(&s, 0);
                assert!((t.captured - cap).abs() < 1e-9 * cap, "the pipe took its capacity {cap}, not {}", t.captured);
                let over = reach - cap;
                let held = h.ponded[pit];
                assert!(held > 0.0, "the pit ponds once the pipe is full");
                assert!(
                    (held - over.min(h.flow.pond_cap[pit] as f64)).abs() < 1e-9 * reach,
                    "the overflow {over} ponds: {held}"
                );
            } else {
                assert!((t.captured - reach).abs() < 1e-9 * reach, "the pipe took everything: {}", t.captured);
                assert_eq!(h.ponded[pit], 0.0, "nothing ponds while the pipe has room");
            }
            let cells = (w * w) as f64;
            assert!((h.pipe_row.in_mm as f64 - t.captured / cells).abs() < 1e-6, "pipe_in_mm is the world mean");
            assert_eq!(h.pipe_row.in_mm, h.pipe_row.out_edge_mm, "all of it left over the edge");
            assert!(h.balance_error().abs() < EPS);
            assert!(s.npk_balance_error().iter().all(|e| e.abs() < 1e-9), "{:?}", s.npk_balance_error());
        }
    }

    /// Acceptance: water a pipe puts down inside the world runs on downhill from its outlet, in the
    /// same storm, and reaches the next depression. On a plane tilted east, a drain in row 2 empties
    /// into row 4, which runs into a pit: the pit ends the storm holding exactly what the drain took
    /// on top of what it holds without the network.
    #[test]
    fn an_interior_outlet_reaches_the_next_depression_downhill() {
        let mut b = tilted_east();
        let w = b.ground.width;
        let pit = 6 + w * 4;
        b.ground_h[pit] = -0.4;
        let p = bundle_params(&b);
        b.pipes = vec![pipe("culvert", (2, 2), centre(3, 4), 1e6, &p)];
        let run = |scale: f32| {
            let mut s = sim_of(&b);
            s.params.pipes.capacity_scale = scale;
            s.route_storm(1.0);
            s
        };
        let (off, on) = (run(0.0), run(1.0));
        assert!(hydro(&off).drain_tally.is_empty(), "at scale 0 no drain is consulted");
        assert_eq!(hydro(&off).pipe_row, PipeRow::default());
        let taken = hydro(&on).drain_tally[0].captured;
        assert!((taken - 2.0).abs() < 1e-12, "the inlet takes the rain of its row upstream: {taken}");
        let gained = hydro(&on).ponded[pit] - hydro(&off).ponded[pit];
        assert!((gained - taken).abs() < 1e-12, "the pit gained {gained} mm of the {taken} the pipe carried");
        assert_eq!(hydro(&on).pipe_row.out_edge_mm, 0.0, "nothing went over the edge through the pipe");
        let (lo, lh) = (hydro(&off).ledger, hydro(&on).ledger);
        assert!((lo.outflow - lh.outflow - taken).abs() < 1e-12, "row 2 no longer sheds what the drain took");
        assert!(hydro(&on).balance_error().abs() < EPS);
        assert!(on.npk_balance_error().iter().all(|e| e.abs() < 1e-9), "{:?}", on.npk_balance_error());
    }

    /// A tick-0 sim through `Sim::from_bundle`, the path a run loads a world by.
    fn load(b: &Bundle) -> Result<Sim, String> {
        let mut p = bundle_params(b);
        p.animals.enabled = false;
        Sim::from_bundle(p, 1, b).map(|(s, _)| s)
    }

    /// Acceptance: a pipe network whose water would go round for ever is refused at load, with the
    /// cycle named. Two drains that each empty upstream of the other's inlet form one; so does a
    /// single drain that empties upstream of itself. A chain that ends over the edge is accepted.
    #[test]
    fn a_cyclic_pipe_graph_is_rejected_with_a_clear_error() {
        let mut b = tilted_east();
        let p = bundle_params(&b);
        b.pipes = vec![pipe("a", (5, 2), centre(2, 4), 10.0, &p), pipe("b", (5, 4), centre(2, 2), 10.0, &p)];
        let e = load(&b).err().expect("a two-pipe cycle is refused");
        assert!(e.contains("cycle") && e.contains("a -> b -> a"), "{e}");
        b.pipes = vec![pipe("loop", (5, 2), centre(2, 2), 10.0, &p)];
        let e = load(&b).err().expect("a pipe that feeds itself is refused");
        assert!(e.contains("loop -> loop"), "{e}");
        b.pipes = vec![pipe("a", (5, 2), centre(2, 4), 10.0, &p), pipe("b", (5, 4), [8.0, 4.5], 10.0, &p)];
        let s = load(&b).expect("a chain that ends at the edge drains");
        assert_eq!(hydro(&s).drains.len(), 2);
        assert!(hydro(&s).pipe_error.is_none());
    }

    /// A world of random media and heights with up to four random drains of random capacity, some
    /// emptying inside it and some over its edge, through random storms and soil updates. After
    /// every storm each drain took no more than its capacity and no more than reached it, and it let
    /// water go by only when it was full; and after every step both ledgers close. A network that
    /// happens to be cyclic is refused at load and has nothing to test.
    fn drains_hold(media: &[u8], heights: &[u8], pipes: &[(u8, u8, u8)], storms: &[u8]) -> Result<bool, TestCaseError> {
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
        let p = bundle_params(&b);
        for (k, &(inlet, outlet, cap)) in pipes.iter().enumerate() {
            let at = |v: u8| (v as usize % 64 % size, v as usize % 64 / size);
            // Outlets from 64 up leave the world across its east edge.
            let out = if outlet < 64 { centre(at(outlet).0, at(outlet).1) } else { [size as f32, 0.5] };
            b.pipes.push(pipe(&format!("p{k}"), at(inlet), out, cap as f64 / 4.0, &p));
        }
        let mut s = sim_of(&b);
        if hydro(&s).pipe_error.is_some() {
            return Ok(false);
        }
        for (t, &d) in storms.iter().enumerate() {
            s.route_storm(d as f64 / 4.0);
            for (k, tl) in hydro(&s).drain_tally.iter().enumerate() {
                let cap = capacity(&s, k);
                let tol = 1e-9 * cap.max(tl.arrived).max(1.0);
                prop_assert!(tl.captured <= cap + tol, "storm {t} drain {k}: took {} of {cap}", tl.captured);
                prop_assert!(
                    tl.captured <= tl.arrived + tol,
                    "storm {t} drain {k}: took {} of {}",
                    tl.captured,
                    tl.arrived
                );
                if tl.arrived - tl.captured > tol {
                    prop_assert!(
                        cap - tl.captured <= tol,
                        "storm {t} drain {k} overflowed at {} of {cap}",
                        tl.captured
                    );
                }
            }
            prop_assert!(hydro(&s).balance_error().abs() < EPS, "storm {}: {}", t, hydro(&s).balance_error());
            let e = s.npk_balance_error();
            prop_assert!(e.iter().all(|v| v.abs() < 1e-9), "storm {t}: {e:?}");
            let drained = s.settle_water(10.0 * tick_hours(&s.params));
            s.update_npk(&drained);
            prop_assert!(hydro(&s).balance_error().abs() < EPS, "settle {}: {}", t, hydro(&s).balance_error());
            let e = s.npk_balance_error();
            prop_assert!(e.iter().all(|v| v.abs() < 1e-9), "settle {t}: {e:?}");
        }
        Ok(true)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(8)))]

        #[test]
        fn prop_drains_respect_capacity_and_the_balances(
            media in prop::collection::vec(any::<u8>(), 64),
            heights in prop::collection::vec(any::<u8>(), 64),
            pipes in prop::collection::vec(any::<(u8, u8, u8)>(), 0..5),
            storms in prop::collection::vec(any::<u8>(), 1..12),
        ) {
            drains_hold(&media, &heights, &pipes, &storms)?;
        }
    }

    /// The regression sibling of `prop_drains_respect_capacity_and_the_balances`: the every-medium
    /// world of `water_balance_regression_every_medium` with a small drain that overflows, a large
    /// one, two on one inlet cell, and one emptying inside the world onto the slope above another.
    #[test]
    fn drains_regression_every_medium() {
        let media: Vec<u8> = (0..64).collect();
        let heights: Vec<u8> = (0..64).map(|i| ((i % 8) * (i / 8)) as u8).collect();
        let pipes = [(27, 200, 2), (27, 100, 255), (45, 9, 40), (9, 64, 8)];
        assert!(drains_hold(&media, &heights, &pipes, &[0, 1, 40, 255, 3, 0, 200]).unwrap(), "the network is acyclic");
    }
}
