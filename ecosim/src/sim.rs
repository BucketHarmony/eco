//! Simulation state and the fixed tick order.

use crate::animals::{Animal, Kind, CAUSES};
use crate::bundle::Bundle;
use crate::events::Event;
use crate::heredity::{trait_stats, TraitStats, Traits, TRAIT_CLAMP};
use crate::hydro::{Hydro, Water};
use crate::npk::{Npk, NpkRow};
use crate::params::Params;
use crate::plants::PlantImport;
use crate::profile::{lap, Phase, Profiler};
use crate::trees::Tree;
use crate::world::{ColClass, Dims, World};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;

/// Per-patch state (`world.patch` × `world.patch` columns).
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Patch {
    /// Grass density in [0, 1].
    pub grass: f32,
    /// Shrub density in [0, 1].
    pub shrub: f32,
    /// Detritus pool; decays into fertility.
    pub detritus: f32,
    /// Temperature, °C.
    pub temperature: f32,
    /// Ticks until the patch burns out; 0 when it is not burning.
    pub burning_ticks_left: u32,
}

/// Deaths during one tick, indexed `[Kind as usize][Cause as usize]` (grazers, then hunters).
pub type Deaths = [[u32; CAUSES]; 2];

/// One `series.csv` row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatsRow {
    /// Tick the row describes.
    pub tick: u32,
    /// Live grazers.
    pub grazers: u32,
    /// Live hunters.
    pub hunters: u32,
    /// Live trees.
    pub trees: u32,
    /// Mean grass density over patches with soil.
    pub grass_mean: f32,
    /// Mean shrub density over patches with soil.
    pub shrub_mean: f32,
    /// Mean surface moisture over soil columns.
    pub moisture_mean: f32,
    /// Mean surface fertility over soil columns.
    pub fertility_mean: f32,
    /// Total detritus over all patches.
    pub detritus_total: f32,
    /// Mean patch temperature.
    pub temperature: f32,
    /// Hunters that have immigrated so far (cumulative).
    pub hunter_immigrants: u32,
    /// Deaths during this tick by species and cause.
    pub deaths: Deaths,
    /// Patches burning at the end of this tick.
    pub patches_burning: u32,
    /// Patches that have burnt out so far (cumulative).
    pub total_burnt: u32,
    /// Heritable trait means and standard deviations per species.
    pub traits: TraitStats,
    /// The six water columns (shot G4); all zero when the water tier is off.
    pub water: Water,
    /// The six nutrient columns (shot G5); all zero when the nutrient tier is off.
    pub npk: NpkRow,
}

/// Marks a column with no trunk in `Sim::trunk_at`.
pub const NO_TREE: u32 = u32::MAX;

pub(crate) fn rebuild_grid(grid: &mut [Vec<u32>], animals: &[Animal], d: Dims) {
    grid.iter_mut().for_each(Vec::clear);
    for (i, a) in animals.iter().enumerate().filter(|(_, a)| a.alive) {
        grid[a.col_index(d)].push(i as u32);
    }
}

/// All (dx, dy, dx² + dy²) with Euclidean length ≤ r, sorted nearest first (then dy, dx).
pub(crate) fn offsets_within(r: f32) -> Vec<(i32, i32, i32)> {
    let k = r.max(0.0) as i32;
    let mut v: Vec<(i32, i32, i32)> = (-k..=k)
        .flat_map(|dy| (-k..=k).map(move |dx| (dx, dy, dx * dx + dy * dy)))
        .filter(|&(_, _, d2)| (d2 as f32).sqrt() <= r)
        .collect();
    v.sort_by_key(|&(dx, dy, d2)| (d2, dy, dx));
    v
}

/// Column offsets out to the largest grazer `flee_distance` the trait clamp allows, nearest first.
/// Each grazer uses the prefix within its own distance.
pub(crate) fn flee_offsets(params: &Params) -> Vec<(i32, i32, i32)> {
    offsets_within(params.grazer.flee_radius * TRAIT_CLAMP.1)
}

/// The whole simulation state.
pub struct Sim {
    /// Parameters the run was started with.
    pub params: Params,
    /// The single random source for the run.
    pub rng: ChaCha8Rng,
    /// Terrain, materials and light.
    pub world: World,
    /// Surface moisture per column (soil columns only; others stay 0).
    pub moisture: Vec<f32>,
    /// Surface fertility per column (soil columns only; others stay 0).
    pub fertility: Vec<f32>,
    /// Surface and soil water, or `None` when `hydro.enabled` is false (shot G4).
    pub hydro: Option<Hydro>,
    /// The three soil nutrient pools, or `None` when `npk.enabled` is false (shot G5). With it
    /// `None` the `fertility` field above is the 0-255 index it always was; with it `Some` that
    /// field is derived from these pools and nothing writes to it but `Sim::refresh_fertility`,
    /// which lives with the rest of the tier in [`crate::npk`].
    pub npk: Option<Npk>,
    /// Per-patch state, indexed by patch.
    pub patches: Vec<Patch>,
    /// Trees, live and dead since the last compaction.
    pub trees: Vec<Tree>,
    /// Grazers, live and dead since the last compaction.
    pub grazers: Vec<Animal>,
    /// Hunters, live and dead since the last compaction.
    pub hunters: Vec<Animal>,
    /// Index into `trees` of the live tree whose trunk stands on each column, or NO_TREE.
    pub trunk_at: Vec<u32>,
    /// Whether any canopy voxel lies over each column.
    pub canopy_cover: Vec<bool>,
    /// Live grazer count per patch, kept current through moves, births and deaths.
    pub grazers_in_patch: Vec<u32>,
    /// Indices into `grazers` of the live grazers on each column, kept current like the counts.
    pub grazer_grid: Vec<Vec<u32>>,
    /// Indices into `hunters` of the live hunters on each column, rebuilt before the grazers act.
    pub hunter_grid: Vec<Vec<u32>>,
    /// Live hunter count per patch, rebuilt with the hunter grid and kept current while hunters act.
    pub hunters_in_patch: Vec<u32>,
    /// Column offsets within the hunter seek radius, nearest first.
    pub seek_offsets: Vec<(i32, i32, i32)>,
    /// Column offsets within the largest grazer flee distance, nearest first.
    pub flee_offsets: Vec<(i32, i32, i32)>,
    /// Ticks completed.
    pub tick: u32,
    /// Next entity id to hand out.
    pub next_id: u32,
    /// Hunters that have immigrated so far.
    pub hunter_immigrants: u32,
    /// Deaths during the current tick by species and cause; cleared at the start of each step.
    pub deaths: Deaths,
    /// Patches that have burnt out so far.
    pub total_burnt: u32,
    /// Whether events are recorded into `events` (`events.csv`); off by default.
    pub log_events: bool,
    /// Events recorded since the writer last drained them.
    pub events: Vec<Event>,
}

impl Sim {
    /// Generate the world and initial populations. Tick 0 state; no update has run.
    pub fn new(params: Params, seed: u64) -> Sim {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let world = World::generate(&params, &mut rng);
        // The terrain comes from the seed alone; the stream varies everything drawn after it.
        if params.rng.stream != 0 {
            rng.set_stream(params.rng.stream);
        }
        Sim::with_world(params, rng, world)
    }

    /// Generate the world from a bundle (`ecosim run --world`) and plant its scene. The terrain is
    /// the bundle's, so unlike [`Sim::new`] the seed draws nothing for it; the bundle's dimensions
    /// must already be in `params` (`Bundle::apply_to`). Returns what the scene planted.
    pub fn from_bundle(params: Params, seed: u64, bundle: &Bundle) -> Result<(Sim, PlantImport), String> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let world = World::from_bundle(bundle, &params)?;
        if params.rng.stream != 0 {
            rng.set_stream(params.rng.stream);
        }
        Ok(Sim::with_bundle(params, rng, world, bundle))
    }

    /// A tick-0 sim on a given world, with `tree.initial_count` random trees and the initial
    /// animals placed using `rng`.
    pub fn with_world(params: Params, rng: ChaCha8Rng, world: World) -> Sim {
        Sim::assemble(params, rng, world, None).0
    }

    /// [`Sim::with_world`] on a bundle world: the scene's trees and shrubs are the starting
    /// vegetation, in place of the random trees (shot G3).
    pub fn with_bundle(params: Params, rng: ChaCha8Rng, world: World, bundle: &Bundle) -> (Sim, PlantImport) {
        Sim::assemble(params, rng, world, Some(bundle))
    }

    fn assemble(params: Params, rng: ChaCha8Rng, world: World, bundle: Option<&Bundle>) -> (Sim, PlantImport) {
        let c = &params.climate;
        let (cols, npatches) = (world.dims.cols(), world.dims.patches());
        let mut moisture = vec![0.0; cols];
        let mut fertility = vec![0.0; cols];
        for i in 0..cols {
            if world.class[i] == ColClass::Soil {
                moisture[i] = c.initial_moisture;
                fertility[i] = c.initial_fertility;
            }
        }
        let patches = (0..npatches)
            .map(|p| {
                let has_soil = !world.patch_soil[p].is_empty();
                Patch {
                    grass: if has_soil { params.grass.initial } else { 0.0 },
                    shrub: if has_soil { params.shrub.initial } else { 0.0 },
                    detritus: 0.0,
                    temperature: 0.0,
                    burning_ticks_left: 0,
                }
            })
            .collect();
        let mut sim = Sim {
            params,
            rng,
            world,
            moisture,
            fertility,
            hydro: None,
            npk: None,
            patches,
            trees: Vec::new(),
            grazers: Vec::new(),
            hunters: Vec::new(),
            trunk_at: vec![NO_TREE; cols],
            canopy_cover: vec![false; cols],
            grazers_in_patch: vec![0; npatches],
            grazer_grid: vec![Vec::new(); cols],
            hunter_grid: vec![Vec::new(); cols],
            hunters_in_patch: vec![0; npatches],
            seek_offsets: Vec::new(),
            flee_offsets: Vec::new(),
            tick: 0,
            next_id: 0,
            hunter_immigrants: 0,
            deaths: Deaths::default(),
            total_burnt: 0,
            log_events: false,
            events: Vec::new(),
        };
        sim.seek_offsets = offsets_within(sim.params.hunter.seek_radius);
        sim.flee_offsets = flee_offsets(&sim.params);
        if sim.params.hydro.enabled {
            sim.hydro = Some(Hydro::new(&sim.world, &sim.params));
            for c in 0..cols {
                sim.derive_moisture(c);
            }
        }
        if sim.params.npk.enabled {
            sim.npk = Some(Npk::new(&sim.world, &sim.params));
        }
        let import = match bundle {
            None => {
                sim.place_initial_trees();
                PlantImport::default()
            }
            Some(b) => sim.import_scene(b),
        };
        sim.place_initial_animals();
        sim.rebuild_grazer_grid();
        sim.update_temperature(0);
        // Last, so the opening stock counts everything that stands at tick 0 -- the scene's trees
        // and shrubs included.
        sim.open_npk_ledger();
        (sim, import)
    }

    /// A sim on the given terrain of the square test world (`Params::load_square`) with no trees
    /// or animals and a fixed RNG: the unit tests' fixture.
    #[cfg(test)]
    pub(crate) fn bare(heights: &[u8]) -> Sim {
        let mut p = Params::load_square();
        p.tree.initial_count = 0;
        p.grazer.start_count = 0;
        p.hunter.start_count = 0;
        p.hunter.immigration_floor = 0;
        p.grazer.immigration_floor = 0;
        p.tree.immigration_floor = 0;
        p.hydro.enabled = false;
        p.npk.enabled = false;
        p.heredity.mutation = 0.0;
        p.fire.base_rate = 0.0;
        p.disease.grazer_rate = 0.0;
        p.disease.hunter_rate = 0.0;
        let world = World::from_heights(heights, &p);
        Sim::with_world(p, ChaCha8Rng::seed_from_u64(3), world)
    }

    /// Replace the params from the next step on (`ecosim fork --set`), recomputing what is derived from them.
    pub fn set_params(&mut self, params: Params) {
        self.seek_offsets = offsets_within(params.hunter.seek_radius);
        self.flee_offsets = flee_offsets(&params);
        self.params = params;
    }

    /// Hand out the next entity id.
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// A uniformly random soil column, or None if the world has none.
    pub fn random_soil_column(&mut self) -> Option<(usize, usize)> {
        let soil: usize = self.world.patch_soil.iter().map(|v| v.len()).sum();
        if soil == 0 {
            return None;
        }
        let mut k = self.rng.gen_range(0..soil);
        for v in &self.world.patch_soil {
            if k < v.len() {
                let c = v[k];
                return Some(self.world.dims.xy(c));
            }
            k -= v.len();
        }
        None
    }

    fn place_initial_animals(&mut self) {
        if !self.params.animals.enabled {
            return;
        }
        for kind in [Kind::Grazer, Kind::Hunter] {
            let (n, energy, age_max, cd) = match kind {
                Kind::Grazer => {
                    let g = &self.params.grazer;
                    (g.start_count, g.start_energy, g.start_age_max, g.cooldown)
                }
                Kind::Hunter => {
                    let h = &self.params.hunter;
                    (h.start_count, h.start_energy, h.start_age_max, h.refractory)
                }
            };
            for _ in 0..n {
                let Some((x, y)) = self.random_soil_column() else { return };
                let age = self.rng.gen_range(0..age_max.max(1));
                let cooldown = self.rng.gen_range(0..=cd);
                let id = self.alloc_id();
                let traits: Traits = self.params.default_traits(kind);
                let a = Animal::new(id, kind, (x, y), energy, age, cooldown, traits);
                match kind {
                    Kind::Grazer => {
                        self.grazers_in_patch[self.world.dims.patch_of(x, y)] += 1;
                        self.grazers.push(a);
                    }
                    Kind::Hunter => {
                        self.hunters_in_patch[self.world.dims.patch_of(x, y)] += 1;
                        self.hunters.push(a);
                    }
                }
            }
        }
    }

    /// Advance one tick, in the fixed order:
    /// animals → immigration → producers → trees (every `update_every`) → fire → moisture/fertility
    /// (every 10) → temperature (every 100). The animal phase is skipped outright when
    /// `animals.enabled` is false, so it draws nothing from the RNG.
    /// Snapshots and stats rows are taken by the caller after this returns.
    pub fn step(&mut self) {
        self.step_profiled(None);
    }

    /// [`Sim::step`], charging each phase's wall time to `prof` when given. The timer never
    /// touches sim state, so the step is the same with and without it.
    pub fn step_profiled(&mut self, mut prof: Option<&mut Profiler>) {
        self.tick += 1;
        self.deaths = Deaths::default();
        let t = self.tick;
        if self.params.animals.enabled {
            self.update_animals();
        }
        lap(&mut prof, Phase::Animals);
        self.immigrate(t);
        lap(&mut prof, Phase::Immigration);
        self.update_producers(t);
        lap(&mut prof, Phase::Producers);
        if t.is_multiple_of(self.params.tree.update_every) {
            self.update_trees();
        }
        lap(&mut prof, Phase::Trees);
        self.update_fire(t);
        lap(&mut prof, Phase::Fire);
        if self.params.hydro.enabled {
            self.storm(t);
        }
        if t.is_multiple_of(self.params.schedule.soil_every.max(1)) {
            self.update_soil(t);
        }
        lap(&mut prof, Phase::MoistureFertility);
        if t.is_multiple_of(self.params.schedule.temperature_every.max(1)) {
            self.update_temperature(t);
        }
        lap(&mut prof, Phase::TemperatureSeason);
        if t.is_multiple_of(self.params.world.compact_every) {
            self.compact();
        }
        lap(&mut prof, Phase::Compaction);
    }

    /// Remove dead entities and rebuild the trunk index.
    pub fn compact(&mut self) {
        self.grazers.retain(|a| a.alive);
        self.hunters.retain(|a| a.alive);
        self.trees.retain(|t| t.alive);
        self.rebuild_grazer_grid();
        self.trunk_at.fill(NO_TREE);
        for (i, t) in self.trees.iter().enumerate() {
            self.trunk_at[t.col(self.world.dims)] = i as u32;
        }
    }

    /// Recompute the grazer grid from scratch.
    pub fn rebuild_grazer_grid(&mut self) {
        rebuild_grid(&mut self.grazer_grid, &self.grazers, self.world.dims);
    }

    /// Recompute the hunter grid and the per-patch hunter counts from scratch.
    pub fn rebuild_hunter_grid(&mut self) {
        rebuild_grid(&mut self.hunter_grid, &self.hunters, self.world.dims);
        self.hunters_in_patch.fill(0);
        for h in self.hunters.iter().filter(|h| h.alive) {
            self.hunters_in_patch[h.patch(self.world.dims)] += 1;
        }
    }

    /// Live grazers.
    pub fn count_grazers(&self) -> u32 {
        self.grazers.iter().filter(|a| a.alive).count() as u32
    }

    /// Live hunters.
    pub fn count_hunters(&self) -> u32 {
        self.hunters.iter().filter(|a| a.alive).count() as u32
    }

    /// Live trees.
    pub fn count_trees(&self) -> u32 {
        self.trees.iter().filter(|t| t.alive).count() as u32
    }

    /// The `series.csv` row for the current state.
    pub fn stats(&self) -> StatsRow {
        let soil_patches: Vec<&Patch> =
            self.patches.iter().zip(&self.world.patch_soil).filter(|(_, s)| !s.is_empty()).map(|(p, _)| p).collect();
        let np = soil_patches.len().max(1) as f64;
        let soil_cols: Vec<usize> =
            (0..self.world.dims.cols()).filter(|&c| self.world.class[c] == ColClass::Soil).collect();
        let nc = soil_cols.len().max(1) as f64;
        StatsRow {
            tick: self.tick,
            grazers: self.count_grazers(),
            hunters: self.count_hunters(),
            trees: self.count_trees(),
            grass_mean: (soil_patches.iter().map(|p| p.grass as f64).sum::<f64>() / np) as f32,
            shrub_mean: (soil_patches.iter().map(|p| p.shrub as f64).sum::<f64>() / np) as f32,
            moisture_mean: (soil_cols.iter().map(|&c| self.moisture[c] as f64).sum::<f64>() / nc) as f32,
            fertility_mean: (soil_cols.iter().map(|&c| self.fertility[c] as f64).sum::<f64>() / nc) as f32,
            detritus_total: self.patches.iter().map(|p| p.detritus as f64).sum::<f64>() as f32,
            temperature: (self.patches.iter().map(|p| p.temperature as f64).sum::<f64>() / self.patches.len() as f64)
                as f32,
            hunter_immigrants: self.hunter_immigrants,
            deaths: self.deaths,
            patches_burning: self.patches.iter().filter(|p| p.burning_ticks_left > 0).count() as u32,
            total_burnt: self.total_burnt,
            traits: [trait_stats(&self.grazers), trait_stats(&self.hunters)],
            water: self.hydro.as_ref().map(|h| h.water).unwrap_or_default(),
            npk: self.npk.as_ref().map(|n| n.row).unwrap_or_default(),
        }
    }
}
