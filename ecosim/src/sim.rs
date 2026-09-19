//! Simulation state and the fixed tick order.

use crate::animals::{Animal, Kind};
use crate::params::Params;
use crate::trees::Tree;
use crate::world::{patch_of, ColClass, World, COLS, PATCHES, WX};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Patch {
    pub grass: f32,
    pub shrub: f32,
    pub detritus: f32,
    pub temperature: f32,
}

/// One `series.csv` row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatsRow {
    pub tick: u32,
    pub grazers: u32,
    pub hunters: u32,
    pub trees: u32,
    pub grass_mean: f32,
    pub shrub_mean: f32,
    pub moisture_mean: f32,
    pub fertility_mean: f32,
    pub detritus_total: f32,
    pub temperature: f32,
}

pub const NO_TREE: u32 = u32::MAX;

fn rebuild_grid(grid: &mut [Vec<u32>], animals: &[Animal]) {
    grid.iter_mut().for_each(Vec::clear);
    for (i, a) in animals.iter().enumerate().filter(|(_, a)| a.alive) {
        grid[Sim::animal_col(a)].push(i as u32);
    }
}

/// All (dx, dy, dx² + dy²) with Euclidean length ≤ r, sorted nearest first (then dy, dx).
fn offsets_within(r: f32) -> Vec<(i32, i32, i32)> {
    let k = r.max(0.0) as i32;
    let mut v: Vec<(i32, i32, i32)> = (-k..=k)
        .flat_map(|dy| (-k..=k).map(move |dx| (dx, dy, dx * dx + dy * dy)))
        .filter(|&(_, _, d2)| (d2 as f32).sqrt() <= r)
        .collect();
    v.sort_by_key(|&(dx, dy, d2)| (d2, dy, dx));
    v
}

pub struct Sim {
    pub params: Params,
    pub rng: ChaCha8Rng,
    pub world: World,
    /// Surface moisture per column (soil columns only; others stay 0).
    pub moisture: Vec<f32>,
    /// Surface fertility per column (soil columns only; others stay 0).
    pub fertility: Vec<f32>,
    pub patches: Vec<Patch>,
    pub trees: Vec<Tree>,
    pub grazers: Vec<Animal>,
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
    /// Column offsets within the hunter seek radius, nearest first.
    pub seek_offsets: Vec<(i32, i32, i32)>,
    /// Column offsets within the grazer flee radius, nearest first.
    pub flee_offsets: Vec<(i32, i32, i32)>,
    pub tick: u32,
    pub next_id: u32,
}

impl Sim {
    /// Generate the world and initial populations. Tick 0 state; no update has run.
    pub fn new(params: Params, seed: u64) -> Sim {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let world = World::generate(&params, &mut rng);
        Sim::with_world(params, rng, world)
    }

    pub fn with_world(params: Params, rng: ChaCha8Rng, world: World) -> Sim {
        let c = &params.climate;
        let mut moisture = vec![0.0; COLS];
        let mut fertility = vec![0.0; COLS];
        for i in 0..COLS {
            if world.class[i] == ColClass::Soil {
                moisture[i] = c.initial_moisture;
                fertility[i] = c.initial_fertility;
            }
        }
        let patches = (0..PATCHES)
            .map(|p| {
                let has_soil = !world.patch_soil[p].is_empty();
                Patch {
                    grass: if has_soil { params.grass.initial } else { 0.0 },
                    shrub: if has_soil { params.shrub.initial } else { 0.0 },
                    detritus: 0.0,
                    temperature: 0.0,
                }
            })
            .collect();
        let mut sim = Sim {
            params,
            rng,
            world,
            moisture,
            fertility,
            patches,
            trees: Vec::new(),
            grazers: Vec::new(),
            hunters: Vec::new(),
            trunk_at: vec![NO_TREE; COLS],
            canopy_cover: vec![false; COLS],
            grazers_in_patch: vec![0; PATCHES],
            grazer_grid: vec![Vec::new(); COLS],
            hunter_grid: vec![Vec::new(); COLS],
            seek_offsets: Vec::new(),
            flee_offsets: Vec::new(),
            tick: 0,
            next_id: 0,
        };
        sim.seek_offsets = offsets_within(sim.params.hunter.seek_radius);
        sim.flee_offsets = offsets_within(sim.params.grazer.flee_radius);
        sim.place_initial_trees();
        sim.place_initial_animals();
        sim.rebuild_grazer_grid();
        sim.update_temperature(0);
        sim
    }

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
                return Some((c % WX, c / WX));
            }
            k -= v.len();
        }
        None
    }

    fn place_initial_animals(&mut self) {
        for kind in [Kind::Grazer, Kind::Hunter] {
            let (n, energy, age_max, cd) = match kind {
                Kind::Grazer => {
                    let g = &self.params.grazer;
                    (g.start_count, g.start_energy, g.start_age_max, g.cooldown)
                }
                Kind::Hunter => {
                    let h = &self.params.hunter;
                    (h.start_count, h.start_energy, h.start_age_max, h.cooldown)
                }
            };
            for _ in 0..n {
                let Some((x, y)) = self.random_soil_column() else { return };
                let age = self.rng.gen_range(0..age_max.max(1));
                let cooldown = self.rng.gen_range(0..=cd);
                let id = self.alloc_id();
                let a = Animal::new(id, kind, x, y, energy, age, cooldown);
                match kind {
                    Kind::Grazer => {
                        self.grazers_in_patch[patch_of(x, y)] += 1;
                        self.grazers.push(a);
                    }
                    Kind::Hunter => self.hunters.push(a),
                }
            }
        }
    }

    /// Advance one tick, in the fixed order:
    /// animals → producers → moisture/fertility (every 10) → temperature (every 100).
    /// Snapshots and stats rows are taken by the caller after this returns.
    pub fn step(&mut self) {
        self.tick += 1;
        let t = self.tick;
        self.update_animals();
        self.update_producers(t);
        if t % self.params.tree.update_every == 0 {
            self.update_trees();
        }
        if t % 10 == 0 {
            self.update_soil(t);
        }
        if t % 100 == 0 {
            self.update_temperature(t);
        }
        if t % self.params.world.compact_every == 0 {
            self.compact();
        }
    }

    /// Remove dead entities and rebuild the trunk index.
    pub fn compact(&mut self) {
        self.grazers.retain(|a| a.alive);
        self.hunters.retain(|a| a.alive);
        self.trees.retain(|t| t.alive);
        self.rebuild_grazer_grid();
        self.trunk_at.fill(NO_TREE);
        for (i, t) in self.trees.iter().enumerate() {
            self.trunk_at[t.col()] = i as u32;
        }
    }

    pub fn rebuild_grazer_grid(&mut self) {
        rebuild_grid(&mut self.grazer_grid, &self.grazers);
    }

    pub fn rebuild_hunter_grid(&mut self) {
        rebuild_grid(&mut self.hunter_grid, &self.hunters);
    }

    pub fn count_grazers(&self) -> u32 {
        self.grazers.iter().filter(|a| a.alive).count() as u32
    }

    pub fn count_hunters(&self) -> u32 {
        self.hunters.iter().filter(|a| a.alive).count() as u32
    }

    pub fn count_trees(&self) -> u32 {
        self.trees.iter().filter(|t| t.alive).count() as u32
    }

    pub fn stats(&self) -> StatsRow {
        let soil_patches: Vec<&Patch> = (0..PATCHES)
            .filter(|&p| !self.world.patch_soil[p].is_empty())
            .map(|p| &self.patches[p])
            .collect();
        let np = soil_patches.len().max(1) as f64;
        let soil_cols: Vec<usize> =
            (0..COLS).filter(|&c| self.world.class[c] == ColClass::Soil).collect();
        let nc = soil_cols.len().max(1) as f64;
        StatsRow {
            tick: self.tick,
            grazers: self.count_grazers(),
            hunters: self.count_hunters(),
            trees: self.count_trees(),
            grass_mean: (soil_patches.iter().map(|p| p.grass as f64).sum::<f64>() / np) as f32,
            shrub_mean: (soil_patches.iter().map(|p| p.shrub as f64).sum::<f64>() / np) as f32,
            moisture_mean: (soil_cols.iter().map(|&c| self.moisture[c] as f64).sum::<f64>() / nc)
                as f32,
            fertility_mean: (soil_cols.iter().map(|&c| self.fertility[c] as f64).sum::<f64>()
                / nc) as f32,
            detritus_total: self.patches.iter().map(|p| p.detritus as f64).sum::<f64>() as f32,
            temperature: (self.patches.iter().map(|p| p.temperature as f64).sum::<f64>()
                / PATCHES as f64) as f32,
        }
    }
}
