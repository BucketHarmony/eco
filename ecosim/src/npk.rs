//! Soil nitrogen, phosphorus and potassium (shot G5): the three pools that replace the 0-255
//! fertility index, and the waterlogging that came with them.
//!
//! **One pool per element per ecology column, in grams per square metre**, and a column is one
//! square metre, so a pool value is also a mass. Nitrogen and potassium are held as the
//! plant-available (mineral) fraction alone; phosphorus is held whole, with `npk.p_avail_frac` of
//! it reachable, because nearly all soil phosphorus is bound to particles -- and the bound part is
//! exactly what runoff carries away, so a model that kept only the available fraction would have
//! nothing for the storm to move.
//!
//! **Nothing is tracked that can be derived.** A ground cover's nutrient content is `need ×
//! density × columns` exactly, because a species takes up `need` per unit of density it gains and
//! its litter returns `need` per unit it loses; so the cover's pool is read off the densities
//! rather than accumulated beside them. That is what makes [`NpkLedger`] a test rather than
//! bookkeeping: a flux this module forgets to charge shows up at the next snapshot as a balance
//! that no longer closes, where an accumulator would have drifted along with the mistake and said
//! nothing. Trees and animals do carry a stored `npk`, because neither has a biomass the rest of
//! the sim already stores -- a tree's uptake is bought against its height and an animal's is
//! whatever it has grazed.
//!
//! The ledger is in f64 and closes to [`crate::output::NPK_BALANCE_EPS`] relative at every
//! snapshot, the same arrangement the water tier uses (DECISIONS.md, shot G4, "Water in f64"). The
//! published files stay small: `npk.bin` is three f32 planes and `fertility.bin` is still one u8
//! plane, now holding `255 ×` the growth factor rather than an index of its own.

use crate::animals::Animal;
use crate::params::{NpkParams, Params, SpeciesNpk};
use crate::sim::Sim;
use crate::world::{ColClass, World};

/// Index of nitrogen in every three-element array here.
pub const N: usize = 0;
/// Index of phosphorus.
pub const P: usize = 1;
/// Index of potassium.
pub const K: usize = 2;

/// How many `series.csv` columns the nutrient tier adds.
pub const NPK_FIELDS: usize = 6;

/// What a run has put into the three pools and taken out of them, in grams, all f64.
///
/// Animals enter with nothing: a newborn, an immigrant and a tick-0 grazer all start empty and
/// fill up by eating, so there is no immigration term to keep. That is not a simplification of the
/// balance, it *is* the balance -- an animal is a place nutrients pass through.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NpkLedger {
    /// Soil, detritus and plants at tick 0.
    pub start: [f64; 3],
    /// Nitrogen deposited from the atmosphere.
    pub deposition: [f64; 3],
    /// Lost with water draining out of the bottom of a column.
    pub leached: [f64; 3],
    /// Lost with water leaving the world over its edge or into open water.
    pub outflow: [f64; 3],
    /// Lost to the air when plant matter burns.
    pub volatilised: [f64; 3],
}

/// How many f64 values [`NpkLedger::as_array`] holds.
pub const NPK_LEDGER_FIELDS: usize = 15;

impl NpkLedger {
    /// Everything that has entered the world, the tick-0 stock included.
    pub fn inflow(&self) -> [f64; 3] {
        [0, 1, 2].map(|i| self.start[i] + self.deposition[i])
    }

    /// Everything that has left it.
    pub fn lost(&self) -> [f64; 3] {
        [0, 1, 2].map(|i| self.leached[i] + self.outflow[i] + self.volatilised[i])
    }

    /// The fifteen values in the order `state.bin` stores them.
    pub fn as_array(&self) -> [f64; NPK_LEDGER_FIELDS] {
        let mut v = [0.0; NPK_LEDGER_FIELDS];
        let groups = [self.start, self.deposition, self.leached, self.outflow, self.volatilised];
        for (k, g) in groups.into_iter().enumerate() {
            v[3 * k..3 * k + 3].copy_from_slice(&g);
        }
        v
    }

    /// The inverse of [`NpkLedger::as_array`].
    pub fn from_array(v: [f64; NPK_LEDGER_FIELDS]) -> NpkLedger {
        let g = |k: usize| [v[3 * k], v[3 * k + 1], v[3 * k + 2]];
        NpkLedger { start: g(0), deposition: g(1), leached: g(2), outflow: g(3), volatilised: g(4) }
    }
}

/// The world's mean pools and its latest sinks, as `series.csv` writes them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NpkRow {
    /// Mean soil nitrogen over plantable columns, g/m².
    pub soil_n: f32,
    /// Mean soil phosphorus, g/m².
    pub soil_p: f32,
    /// Mean soil potassium, g/m².
    pub soil_k: f32,
    /// Nitrogen leached during the last soil update, per plantable column, g/m².
    pub leached_n: f32,
    /// Phosphorus carried out of the world by the last storm, per plantable column, g/m².
    pub outflow_p: f32,
    /// Share of plantable columns that are waterlogged.
    pub waterlogged_frac: f32,
}

impl NpkRow {
    /// The six values in `series.csv` column order.
    pub fn as_array(self) -> [f32; NPK_FIELDS] {
        [self.soil_n, self.soil_p, self.soil_k, self.leached_n, self.outflow_p, self.waterlogged_frac]
    }

    /// The inverse of [`NpkRow::as_array`].
    pub fn from_array(v: [f32; NPK_FIELDS]) -> NpkRow {
        NpkRow { soil_n: v[0], soil_p: v[1], soil_k: v[2], leached_n: v[3], outflow_p: v[4], waterlogged_frac: v[5] }
    }
}

/// The three pools, the waterlogging clock and the ledger.
#[derive(Debug, Clone)]
pub struct Npk {
    /// Soil nitrogen, phosphorus and potassium per ecology column, g/m², as `[element][column]`.
    pub soil: [Vec<f64>; 3],
    /// Nutrients held in each patch's detritus, in grams.
    pub detritus: Vec<[f64; 3]>,
    /// Ticks each column has been continuously above `hydro.waterlog_frac` of field capacity.
    pub wet_ticks: Vec<u32>,
    /// Whether each column is waterlogged now.
    pub waterlogged: Vec<bool>,
    /// Whether anything roots in each column: the columns the pools are kept on.
    pub plantable: Vec<bool>,
    /// The run's nutrient balance.
    pub ledger: NpkLedger,
    /// The latest row values.
    pub row: NpkRow,
    /// How many columns are plantable: the divisor of every mean the row publishes.
    pub columns: usize,
    /// Scratch for [`Sim::route_storm`](crate::sim::Sim): grams of phosphorus and of nitrogen on
    /// their way downhill out of each ground cell, the nutrient twin of `Hydro::runon`. Sized on
    /// first use and cleared at the start of every storm, so it is state between cells of one
    /// storm and nothing at all between storms.
    pub carry: Vec<[f64; 2]>,
}

impl Npk {
    /// The pools a world starts with: every plantable column at `npk.init_*` per unit of
    /// `climate.initial_fertility`, and nothing anywhere else -- which is the old fertility
    /// initialisation read through three conversion factors, so a run's starting soil is the same
    /// soil it was before the index was split up.
    ///
    /// The litter is seeded too, at `npk.init_detritus` g/m² of plantable ground. A site that
    /// started bare of litter would spend its first years mining the mineral pool to build one,
    /// and five years is not long enough to finish: litter here turns over in about a decade once
    /// the temperature and moisture factors are in `climate.decay_k`, so an empty pile is a sink
    /// that never fills and the soil never gets its nitrogen back (TUNING.md, shot G5).
    pub fn new(world: &World, params: &Params) -> Npk {
        let cols = world.dims.cols();
        let np = &params.npk;
        let f = params.climate.initial_fertility as f64;
        let plantable: Vec<bool> = (0..cols).map(|c| world.class[c] == ColClass::Soil).collect();
        let init = [np.init_n as f64 * f, np.init_p as f64 * f, np.init_k as f64 * f];
        let soil = [0, 1, 2].map(|i| (0..cols).map(|c| if plantable[c] { init[i] } else { 0.0 }).collect());
        let columns = plantable.iter().filter(|&&b| b).count();
        let litter = np.init_detritus.map(|v| v as f64);
        let detritus = (0..world.dims.patches())
            .map(|p| {
                let m2 = world.patch_soil[p].len() as f64;
                [0, 1, 2].map(|i| litter[i] * m2)
            })
            .collect();
        Npk {
            soil,
            detritus,
            wet_ticks: vec![0; cols],
            waterlogged: vec![false; cols],
            plantable,
            ledger: NpkLedger::default(),
            row: NpkRow::default(),
            columns,
            carry: Vec::new(),
        }
    }

    /// Available pool of element `i` in column `c`: all of the nitrogen and potassium, and
    /// `npk.p_avail_frac` of the phosphorus.
    pub fn available(&self, np: &NpkParams, i: usize, c: usize) -> f64 {
        let v = self.soil[i][c].max(0.0);
        if i == P {
            v * np.p_avail_frac as f64
        } else {
            v
        }
    }

    /// Nutrients in the soil, in grams.
    pub fn soil_total(&self) -> [f64; 3] {
        [0, 1, 2].map(|i| self.soil[i].iter().sum())
    }

    /// Nutrients in every patch's detritus, in grams.
    pub fn detritus_total(&self) -> [f64; 3] {
        let mut t = [0.0; 3];
        for d in &self.detritus {
            for (i, v) in d.iter().enumerate() {
                t[i] += v;
            }
        }
        t
    }
}

/// The nutrient factor of growth: Liebig's law of the minimum with a saturating term per element,
/// `min_i(a_i / (a_i + half_sat_i × need_i))` over the available pools `a_i`.
///
/// It is in [0, 1], never falls when a pool rises, and is 0 when a pool the species actually needs
/// is empty -- which is the law of the minimum doing its job, and is what separates it from the
/// product of three factors: a soil with plenty of two elements and none of the third grows
/// nothing, rather than a third as much. A species with `need_i = 0` is not limited by element `i`
/// at all, so an empty pool it does not need leaves the factor alone.
pub fn liebig(avail: [f64; 3], needs: [f32; 3], half_sat: [f32; 3]) -> f64 {
    let mut f = 1.0f64;
    for i in 0..3 {
        let half = half_sat[i] as f64 * needs[i] as f64;
        if half <= 0.0 {
            continue;
        }
        let a = avail[i].max(0.0);
        f = f.min(a / (a + half));
    }
    f.clamp(0.0, 1.0)
}

/// Which element limits growth here, or `None` when none does because the species needs none of
/// them. Ties go to the lower index, so the answer is a function of the pools alone.
pub fn limiting(avail: [f64; 3], needs: [f32; 3], half_sat: [f32; 3]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for i in 0..3 {
        let half = half_sat[i] as f64 * needs[i] as f64;
        if half <= 0.0 {
            continue;
        }
        let a = avail[i].max(0.0);
        let f = a / (a + half);
        if best.is_none_or(|(_, b)| f < b) {
            best = Some((i, f));
        }
    }
    best.map(|(i, _)| i)
}

/// Move an animal's nutrients above what its body can hold into `dung`, in grams. A body holds
/// `animals.npk_content × energy`, so an animal that has eaten more than it is worth passes the
/// rest straight through -- which is how grazing returns most of what it removes to the patch it
/// removed it from, rather than locking it up in a herd.
pub fn excrete(a: &mut Animal, content: [f32; 3], dung: &mut [f64; 3]) {
    for i in 0..3 {
        let cap = (content[i] as f64 * a.energy as f64).max(0.0);
        let over = a.npk[i] - cap;
        if over > 0.0 {
            a.npk[i] -= over;
            dung[i] += over;
        }
    }
}

impl Sim {
    /// Nutrients held in living plants, in grams: `need × density × columns` for the two ground
    /// covers, and what each tree has actually taken up.
    pub fn plant_npk(&self) -> [f64; 3] {
        let mut t = [0.0f64; 3];
        let (g, s) = (self.params.grass.npk.needs(), self.params.shrub.npk.needs());
        for p in 0..self.world.dims.patches() {
            let n = self.world.patch_soil[p].len() as f64;
            if n == 0.0 {
                continue;
            }
            let (gd, sd) = (self.patches[p].grass as f64, self.patches[p].shrub as f64);
            for (i, v) in t.iter_mut().enumerate() {
                *v += n * (gd * g[i] as f64 + sd * s[i] as f64);
            }
        }
        for tree in self.trees.iter().filter(|t| t.alive) {
            for (i, v) in t.iter_mut().enumerate() {
                *v += tree.npk[i];
            }
        }
        t
    }

    /// Nutrients held in living animals, in grams. A dead one has already emptied into its patch.
    pub fn animal_npk(&self) -> [f64; 3] {
        let mut t = [0.0f64; 3];
        for a in self.grazers.iter().chain(&self.hunters).filter(|a| a.alive) {
            for (i, v) in t.iter_mut().enumerate() {
                *v += a.npk[i];
            }
        }
        t
    }

    /// Everything the world holds now: soil, detritus, plants and animals, in grams.
    pub fn npk_inventory(&self) -> [f64; 3] {
        let n = self.npk.as_ref().expect("nutrient tier");
        let (soil, det, plants, animals) = (n.soil_total(), n.detritus_total(), self.plant_npk(), self.animal_npk());
        [0, 1, 2].map(|i| soil[i] + det[i] + plants[i] + animals[i])
    }

    /// How far each pool is from closing, relative to everything that has entered the world:
    /// `(in − lost − inventory) / in`.
    pub fn npk_balance_error(&self) -> [f64; 3] {
        let n = self.npk.as_ref().expect("nutrient tier");
        let (inflow, lost, have) = (n.ledger.inflow(), n.ledger.lost(), self.npk_inventory());
        [0, 1, 2].map(|i| (inflow[i] - lost[i] - have[i]) / inflow[i].abs().max(1.0))
    }

    /// Record the tick-0 stock, once everything that stands at tick 0 has been placed.
    pub(crate) fn open_npk_ledger(&mut self) {
        if self.npk.is_none() {
            return;
        }
        let have = self.npk_inventory();
        self.npk.as_mut().expect("nutrient tier").ledger.start = have;
        self.observe_npk();
        self.refresh_fertility();
    }

    /// The share of `want` grams per element that patch `p`'s columns can actually cover, in
    /// [0, 1]: the scarcest element decides, and nothing is removed.
    ///
    /// Splitting the question from the spending is what lets a caller obey "growth is cut to what
    /// the pool can cover" exactly. It asks for the share first, applies it to the growth, and
    /// only then spends against the growth it kept -- so the plant that results is the plant the
    /// soil paid for, with no second guess about which of the two was rounded.
    pub fn npk_share(&self, p: usize, want: [f64; 3]) -> f64 {
        let Some(n) = &self.npk else { return 1.0 };
        let cols = &self.world.patch_soil[p];
        if cols.is_empty() {
            return 0.0;
        }
        let np = &self.params.npk;
        let mut share = 1.0f64;
        for (i, &w) in want.iter().enumerate() {
            if w <= 0.0 {
                continue;
            }
            let avail: f64 = cols.iter().map(|&c| n.available(np, i, c)).sum();
            if avail < w {
                share = share.min((avail / w).max(0.0));
            }
        }
        share
    }

    /// Remove `take` grams of each element from patch `p` and return what was actually removed.
    ///
    /// Two passes: an even split over the patch's columns, then a sweep that collects whatever the
    /// poorer columns could not cover from the ones that still have it. The second pass is what
    /// makes `npk_share` an honest promise -- the share is measured on the patch's total, so a
    /// spend that stopped at the even split would take less than the share said and leak the
    /// difference out of the ledger.
    pub(crate) fn npk_spend(&mut self, p: usize, take: [f64; 3]) -> [f64; 3] {
        let ncols = self.world.patch_soil[p].len();
        if ncols == 0 || self.npk.is_none() {
            return [0.0; 3];
        }
        let cols = &self.world.patch_soil[p];
        let n = self.npk.as_mut().expect("nutrient tier");
        let mut got = [0.0; 3];
        for (i, &want) in take.iter().enumerate() {
            if want <= 0.0 {
                continue;
            }
            let each = want / ncols as f64;
            for &c in cols {
                let v = each.min(n.soil[i][c].max(0.0));
                n.soil[i][c] -= v;
                got[i] += v;
            }
            let mut short = want - got[i];
            for &c in cols {
                if short <= 0.0 {
                    break;
                }
                let v = short.min(n.soil[i][c].max(0.0));
                n.soil[i][c] -= v;
                got[i] += v;
                short -= v;
            }
        }
        got
    }

    /// Add `add` grams of each element to patch `p`'s soil, split evenly over its columns.
    pub(crate) fn npk_return(&mut self, p: usize, add: [f64; 3]) {
        let ncols = self.world.patch_soil[p].len();
        let Some(n) = &mut self.npk else { return };
        if ncols == 0 {
            return;
        }
        for (i, &v) in add.iter().enumerate() {
            for &c in &self.world.patch_soil[p] {
                n.soil[i][c] += v / ncols as f64;
            }
        }
    }

    /// Move `add` grams of each element into patch `p`'s detritus: what a dead plant, a corpse or
    /// a pile of dung leaves behind.
    pub(crate) fn npk_to_detritus(&mut self, p: usize, add: [f64; 3]) {
        if let Some(n) = &mut self.npk {
            for (i, &v) in add.iter().enumerate() {
                n.detritus[p][i] += v;
            }
        }
    }

    /// Take up to `want` grams of each element out of one column, for a tree drawing on the soil
    /// under its own trunk. Unlike a ground cover, a tree is not cut back when the soil is short:
    /// it takes what is there and stays poorer for it, because its height comes from its age and
    /// nothing here can un-grow it.
    pub(crate) fn npk_take_column(&mut self, c: usize, want: [f64; 3]) -> [f64; 3] {
        let avail_p = self.params.npk.p_avail_frac as f64;
        let Some(n) = &mut self.npk else { return [0.0; 3] };
        let mut got = [0.0; 3];
        for (i, &w) in want.iter().enumerate() {
            if w <= 0.0 {
                continue;
            }
            let reachable = if i == P { n.soil[P][c].max(0.0) * avail_p } else { n.soil[i][c].max(0.0) };
            got[i] = w.min(reachable);
            n.soil[i][c] -= got[i];
        }
        got
    }

    /// The mean available pools over patch `p`'s columns, g/m².
    pub fn patch_available(&self, p: usize) -> [f64; 3] {
        let Some(n) = &self.npk else { return [0.0; 3] };
        let cols = &self.world.patch_soil[p];
        if cols.is_empty() {
            return [0.0; 3];
        }
        let (np, m) = (&self.params.npk, cols.len() as f64);
        [0, 1, 2].map(|i| cols.iter().map(|&c| n.available(np, i, c)).sum::<f64>() / m)
    }

    /// The nutrient factor of a species' growth on patch `p`, read on the patch's mean pools.
    /// 1 with the nutrient tier off, which is what leaves the pre-G5 growth equation alone.
    pub fn npk_growth_factor(&self, p: usize, sp: &SpeciesNpk) -> f32 {
        if self.npk.is_none() {
            return 1.0;
        }
        liebig(self.patch_available(p), sp.needs(), self.params.npk.half_sat) as f32
    }

    /// The waterlogging multiplier a species' growth gets on patch `p`: its tolerance where the
    /// patch's columns are waterlogged and 1 where they are not, in proportion.
    pub fn waterlog_factor(&self, p: usize, sp: &SpeciesNpk) -> f32 {
        let Some(n) = &self.npk else { return 1.0 };
        let cols = &self.world.patch_soil[p];
        if cols.is_empty() {
            return 1.0;
        }
        let wet = cols.iter().filter(|&&c| n.waterlogged[c]).count() as f32;
        1.0 - (wet / cols.len() as f32) * (1.0 - sp.waterlog_tolerance.clamp(0.0, 1.0))
    }

    /// Whether column `c` is waterlogged now.
    pub fn is_waterlogged(&self, c: usize) -> bool {
        self.npk.as_ref().is_some_and(|n| n.waterlogged[c])
    }

    /// `fertility.bin`'s field with the nutrient tier on: `255 ×` the growth factor grass would
    /// get in each column, so the renderer's fertility overlay still answers "how well can plants
    /// grow here" -- now as a limitation rather than as a stock.
    ///
    /// Refreshed on the soil update rather than every tick. The pools move on that tier's own
    /// cadence, which is the cadence the field it replaced was written on too, so a reader loses
    /// nothing and the sim does not pay for a pass over every column every tick.
    pub(crate) fn refresh_fertility(&mut self) {
        let Some(n) = &self.npk else { return };
        let (np, needs) = (&self.params.npk, self.params.grass.npk.needs());
        for c in 0..self.world.dims.cols() {
            self.fertility[c] = if n.plantable[c] {
                255.0 * liebig([0, 1, 2].map(|i| n.available(np, i, c)), needs, np.half_sat) as f32
            } else {
                0.0
            };
        }
    }

    /// The nutrient half of the soil update, every `schedule.soil_every` ticks: deposition, the
    /// waterlogging clock and leaching with the drainage.
    ///
    /// Detritus decay moves nutrients in [`Sim::decay_detritus`], which is where the share that
    /// decayed is decided. `drained` is each column's drainage in millimetres, as
    /// [`Sim::settle_water`] returns it, and is empty when the water tier is off -- in which case
    /// nothing leaches and nothing waterlogs, because there is no water flux to do either.
    pub(crate) fn update_npk(&mut self, drained: &[f32]) {
        if self.npk.is_none() {
            return;
        }
        let ticks = self.params.schedule.soil_every.max(1);
        let years = crate::hydro::years(&self.params, ticks);
        let (np, hp) = (self.params.npk, self.params.hydro.clone());
        let cols = self.world.dims.cols();
        // Read off the same soil water the plants read, before anything below moves it.
        let wet: Vec<bool> = (0..cols).map(|c| self.water_fraction(c) >= hp.waterlog_frac).collect();
        let store: Vec<f64> = match &self.hydro {
            Some(h) => h.soil.clone(),
            None => vec![0.0; cols],
        };
        let n = self.npk.as_mut().expect("nutrient tier");
        let dep = np.n_deposition as f64 * years;
        let (mut deposited, mut logged, mut leached) = (0.0f64, 0usize, [0.0f64; 3]);
        for c in 0..cols {
            if !n.plantable[c] {
                continue;
            }
            // 1. Deposition: nitrogen out of the air, on every plantable column.
            n.soil[N][c] += dep;
            deposited += dep;
            // 2. The waterlogging clock. A column has to stay wet for `hydro.waterlog_ticks`
            //    before roots suffer, so one storm does not drown a lawn.
            n.wet_ticks[c] = if wet[c] { n.wet_ticks[c] + ticks } else { 0 };
            n.waterlogged[c] = n.wet_ticks[c] >= hp.waterlog_ticks;
            logged += n.waterlogged[c] as usize;
            // 3. Leaching: nitrogen leaves with the water that drained out of the bottom of the
            //    column, at the concentration of a fully mixed store, and potassium at a fraction
            //    of that rate because most of it is held on the exchange complex. Phosphorus does
            //    not leach at all: it moves over the ground rather than through it, which is the
            //    storm's business (`Sim::route_storm`).
            let d = drained.get(c).copied().unwrap_or(0.0) as f64;
            if d <= 0.0 {
                continue;
            }
            let frac = (d / (store[c] + d)).clamp(0.0, 1.0);
            for (i, rate) in [(N, 1.0), (K, np.k_leach_ratio as f64)] {
                let gone = n.soil[i][c].max(0.0) * (frac * rate).clamp(0.0, 1.0);
                n.soil[i][c] -= gone;
                leached[i] += gone;
            }
        }
        n.ledger.deposition[N] += deposited;
        for (i, v) in leached.iter().enumerate() {
            n.ledger.leached[i] += v;
        }
        let per_col = n.columns.max(1) as f64;
        n.row.leached_n = (leached[N] / per_col) as f32;
        n.row.waterlogged_frac = logged as f32 / per_col as f32;
        self.observe_npk();
        self.refresh_fertility();
    }

    /// Refresh the three mean-pool values of the row.
    pub(crate) fn observe_npk(&mut self) {
        if let Some(n) = &mut self.npk {
            let m = n.columns.max(1) as f64;
            let t = n.soil_total();
            (n.row.soil_n, n.row.soil_p, n.row.soil_k) = ((t[N] / m) as f32, (t[P] / m) as f32, (t[K] / m) as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests::{bundle_params, flat_bundle, paint};
    use crate::bundle::{Bundle, Medium};
    use crate::params::SpeciesNpk;
    use crate::world::World;
    use proptest::prelude::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    const NEEDS: [f32; 3] = [7.5, 0.6, 6.0];
    const HALF: [f32; 3] = [0.133, 1.67, 0.5];

    #[test]
    fn liebig_is_the_minimum_and_bounded() {
        assert_eq!(liebig([0.0, 5.0, 30.0], NEEDS, HALF), 0.0, "no nitrogen, no growth");
        assert_eq!(liebig([5.0, 0.0, 30.0], NEEDS, HALF), 0.0, "no phosphorus, no growth");
        assert_eq!(liebig([5.0, 30.0, 0.0], NEEDS, HALF), 0.0, "no potassium, no growth");
        assert!(liebig([1e9, 1e9, 1e9], NEEDS, HALF) > 0.999);
        // A species that needs nothing is never limited, whatever the soil holds.
        assert_eq!(liebig([0.0; 3], [0.0; 3], HALF), 1.0);
    }

    #[test]
    fn liebig_regression_half_saturation_is_a_half() {
        // One element scarce and the others plentiful: the factor is exactly its own term.
        let half = HALF[N] as f64 * NEEDS[N] as f64;
        assert!((liebig([half, 1e9, 1e9], NEEDS, HALF) - 0.5).abs() < 1e-12);
        assert_eq!(limiting([half, 1e9, 1e9], NEEDS, HALF), Some(N));
        assert_eq!(limiting([1e9, 0.0, 1e9], NEEDS, HALF), Some(P));
        assert_eq!(limiting([1e9, 1e9, 1e9], [0.0; 3], HALF), None);
    }

    proptest! {
        /// The acceptance's Liebig property: non-decreasing in each nutrient, zero when any the
        /// species needs is zero, and never outside [0, 1].
        #[test]
        fn prop_liebig_monotone_and_bounded(
            a in prop::array::uniform3(0.0f64..1e4),
            i in 0usize..3,
            add in 0.0f64..1e4,
        ) {
            let f = liebig(a, NEEDS, HALF);
            prop_assert!((0.0..=1.0).contains(&f));
            let mut b = a;
            b[i] += add;
            prop_assert!(liebig(b, NEEDS, HALF) >= f - 1e-12, "raising a pool lowered the factor");
            let mut z = a;
            z[i] = 0.0;
            prop_assert_eq!(liebig(z, NEEDS, HALF), 0.0);
        }

        /// The limiting element is the one whose term the factor equals.
        #[test]
        fn prop_limiting_names_the_binding_term(a in prop::array::uniform3(0.0f64..1e4)) {
            let i = limiting(a, NEEDS, HALF).expect("every need is above 0");
            let half = HALF[i] as f64 * NEEDS[i] as f64;
            prop_assert!((liebig(a, NEEDS, HALF) - a[i] / (a[i] + half)).abs() < 1e-12);
        }
    }

    // ----- the mechanism on synthetic worlds -----

    /// How far the nutrient ledger may be from closing, relative. The acceptance asks for 1e-6.
    const EPS: f64 = 1e-6;

    /// A tick-0 sim on a bundle, animals off, with both the water and the nutrient tiers on.
    fn sim_of(b: &Bundle) -> Sim {
        let mut p = bundle_params(b);
        p.animals.enabled = false;
        let world = World::from_bundle(b, &p).unwrap();
        let mut s = Sim::with_bundle(p, ChaCha8Rng::seed_from_u64(1), world, b).0;
        s.open_npk_ledger();
        s
    }

    /// Paint every ground cell of the bundle with one medium.
    fn cover(b: &mut Bundle, m: Medium) {
        for y in 0..b.size_m {
            for x in 0..b.size_m {
                paint(b, x, y, m);
            }
        }
    }

    fn pools(s: &Sim) -> &Npk {
        s.npk.as_ref().expect("the nutrient tier is on")
    }

    /// Acceptance: water that drains out of the bottom of a column takes nitrogen with it, takes
    /// `npk.k_leach_ratio` as much of its potassium, and leaves the phosphorus where it is --
    /// phosphorus travels over the ground, not through it. A column that drains nothing loses
    /// nothing, and what the pools lost is what the ledger says left.
    #[test]
    fn drainage_takes_nitrogen_and_leaves_phosphorus() {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Lawn);
        let mut s = sim_of(&b);
        let cols = s.world.dims.cols();
        let before = [0, 1, 2].map(|i| pools(&s).soil[i].clone());
        // Half the columns drain 20 mm; the rest drain nothing.
        let drained: Vec<f32> = (0..cols).map(|c| if c % 2 == 0 { 20.0 } else { 0.0 }).collect();
        s.update_npk(&drained);
        let after = [0, 1, 2].map(|i| pools(&s).soil[i].clone());
        let np = s.params.npk;
        let dep = np.n_deposition as f64 * crate::hydro::years(&s.params, s.params.schedule.soil_every);
        let (mut wet, mut dry) = (0, 0);
        for c in 0..cols {
            if !pools(&s).plantable[c] {
                continue;
            }
            assert_eq!(after[P][c], before[P][c], "column {c} lost phosphorus down the profile");
            if c % 2 == 0 {
                let (fed, lost_k) = (before[N][c] + dep, before[K][c] - after[K][c]);
                assert!(after[N][c] < fed, "column {c} drained and kept its nitrogen");
                let ratio = (lost_k / before[K][c]) / ((fed - after[N][c]) / fed);
                assert!((ratio - np.k_leach_ratio as f64).abs() < 1e-9, "potassium left at {ratio} of nitrogen");
                wet += 1;
            } else {
                assert_eq!(after[N][c], before[N][c] + dep, "column {c} drained nothing and still lost nitrogen");
                assert_eq!(after[K][c], before[K][c], "column {c} drained nothing and still lost potassium");
                dry += 1;
            }
        }
        assert!(wet > 0 && dry > 0, "the test needs both a draining and a dry column, not {wet} and {dry}");
        assert_eq!(pools(&s).ledger.leached[P], 0.0, "phosphorus does not leach");
        assert!(pools(&s).ledger.leached[N] > 0.0, "nitrogen does");
        assert!(s.npk_balance_error().iter().all(|e| e.abs() < EPS), "{:?}", s.npk_balance_error());
    }

    /// Acceptance: runoff carries phosphorus downhill, and a strip of ground that drinks what
    /// reaches it keeps what the water was carrying. The slope is lawn tilted to the east over a
    /// crusted surface that takes water at a millimetre an hour, so most of a storm runs off it;
    /// the last two columns are stripped of phosphorus and given an unlimited store and rate, so
    /// they take every drop that reaches them: a buffer strip at the foot of a field. What the
    /// slope gave up is in the strip or over the world's edge, to the gram.
    #[test]
    fn runoff_carries_phosphorus_downhill_into_the_strip_that_drinks_it() {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Lawn);
        // 0.1 m per cell down to the east, so every cell drains into its eastern neighbour.
        for i in 0..b.ground_h.len() {
            b.ground_h[i] = 0.1 * (b.ground.width - 1 - i % b.ground.width) as f32;
        }
        let mut s = sim_of(&b);
        let w = s.world.dims.wx;
        const STRIP: usize = 6;
        let h = s.hydro.as_mut().expect("the water tier is on");
        for i in 0..h.infiltration.len() {
            h.infiltration[i] = 1.0;
        }
        for c in 0..h.soil.len() {
            if c % w >= STRIP {
                h.soil[c] = 0.0;
                h.store_max[c] = 1e6;
            }
        }
        let cells: Vec<usize> = (0..h.flow.col.len()).filter(|&i| h.flow.col[i] as usize % w >= STRIP).collect();
        for i in cells {
            h.infiltration[i] = 1e6;
        }
        let n = s.npk.as_mut().expect("the nutrient tier is on");
        for c in 0..n.soil[P].len() {
            if c % w >= STRIP {
                n.soil[P][c] = 0.0;
            }
        }
        s.open_npk_ledger();
        let before = pools(&s).soil[P].clone();
        let total_before = pools(&s).soil_total()[P];
        s.route_storm(10.0);
        let after = pools(&s).soil[P].clone();
        let (mut gave, mut caught) = (0.0f64, 0.0f64);
        for c in 0..after.len() {
            if !pools(&s).plantable[c] {
                continue;
            }
            if c % w >= STRIP {
                assert!(after[c] >= before[c], "column {c} is in the strip and lost phosphorus");
                caught += after[c] - before[c];
            } else {
                assert!(after[c] <= before[c], "column {c} is on the slope and gained phosphorus");
                gave += before[c] - after[c];
            }
        }
        let out = pools(&s).ledger.outflow[P];
        assert!(caught > out, "the strip caught {caught} g of the {gave} g the slope gave up, {out} g left");
        assert!((gave - caught - out).abs() < 1e-9, "{gave} g left the slope, {caught} g landed, {out} g went");
        let total_after = pools(&s).soil_total()[P];
        assert!((total_before - total_after - out).abs() < 1e-9, "{total_before} g became {total_after} g");
        assert!(s.npk_balance_error().iter().all(|e| e.abs() < EPS), "{:?}", s.npk_balance_error());
    }

    /// Acceptance: a column held at or above `hydro.waterlog_frac` for `hydro.waterlog_ticks`
    /// stops a species that cannot stand wet feet and leaves one that can alone. One wet update is
    /// not enough -- the clock has to run -- and drying out resets it.
    #[test]
    fn a_waterlogged_patch_slows_only_the_intolerant() {
        let mut b = flat_bundle(8, 1);
        cover(&mut b, Medium::Lawn);
        let mut s = sim_of(&b);
        let drowns = SpeciesNpk { waterlog_tolerance: 0.0, ..Default::default() };
        let swims = SpeciesNpk { waterlog_tolerance: 1.0, ..Default::default() };
        let p = (0..s.world.dims.patches()).find(|&p| !s.world.patch_soil[p].is_empty()).expect("a soil patch");
        // Soak the whole world past field capacity and hold it there.
        let capacity = s.hydro.as_ref().expect("the water tier is on").capacity.clone();
        let soak = |s: &mut Sim, f: f64| {
            let h = s.hydro.as_mut().expect("the water tier is on");
            for (c, cap) in capacity.iter().enumerate() {
                h.soil[c] = cap * f;
            }
        };
        soak(&mut s, 1.5);
        let ticks = s.params.schedule.soil_every.max(1);
        s.update_npk(&[]);
        assert_eq!(s.waterlog_factor(p, &drowns), 1.0, "one wet update is not waterlogging");
        for _ in 0..s.params.hydro.waterlog_ticks.div_ceil(ticks) {
            s.update_npk(&[]);
        }
        assert!(s.is_waterlogged(s.world.patch_soil[p][0]), "the column stayed wet for the whole clock");
        assert_eq!(s.waterlog_factor(p, &drowns), 0.0, "a species with no tolerance stops growing");
        assert_eq!(s.waterlog_factor(p, &swims), 1.0, "a tolerant species is untouched by the same water");
        soak(&mut s, 0.0);
        s.update_npk(&[]);
        assert_eq!(s.waterlog_factor(p, &drowns), 1.0, "the ground dried out and the clock restarted");
    }

    /// Run a world of `media` over `heights` through storms and soil updates, and require the
    /// nutrient ledger to close after every one of them.
    fn balance_holds(media: &[u8], heights: &[u8], storms: &[u8]) -> Result<(), TestCaseError> {
        let mut b = flat_bundle(8, 1);
        let size = b.size_m;
        for y in 0..size {
            for x in 0..size {
                paint(&mut b, x, y, Medium::ALL[media[x + y * size] as usize % Medium::ALL.len()]);
            }
        }
        for (i, h) in heights.iter().enumerate() {
            b.ground_h[i] = *h as f32 * 0.02;
        }
        let mut s = sim_of(&b);
        for (t, storm) in storms.iter().enumerate() {
            s.route_storm(*storm as f64 * 0.5);
            let e = s.npk_balance_error();
            prop_assert!(e.iter().all(|v| v.abs() < EPS), "storm {t}: {e:?}");
            let drained = s.settle_water(10.0 * crate::hydro::tick_hours(&s.params));
            s.update_npk(&drained);
            let e = s.npk_balance_error();
            prop_assert!(e.iter().all(|v| v.abs() < EPS), "soil {t}: {e:?}");
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(8)))]

        /// Every gram the world was given is in the soil, the litter, a plant, an animal or the
        /// ledger's losses, on any ground and after any weather.
        #[test]
        fn prop_nutrient_balance_closes(
            media in prop::collection::vec(any::<u8>(), 64),
            heights in prop::collection::vec(any::<u8>(), 64),
            storms in prop::collection::vec(any::<u8>(), 1..12),
        ) {
            balance_holds(&media, &heights, &storms)?;
        }
    }

    /// The regression sibling of `prop_nutrient_balance_closes`: one world with every medium in it,
    /// a slope, and storms big enough to run off the sealed cells and drain the soil ones.
    #[test]
    fn nutrient_balance_regression_every_medium() {
        let media: Vec<u8> = (0..64).collect();
        let heights: Vec<u8> = (0..64).map(|i| ((i % 8) * (i / 8)) as u8).collect();
        balance_holds(&media, &heights, &[0, 1, 40, 255, 3, 0, 200]).unwrap();
    }
}
