//! Grazers and hunters: fixed-priority behaviour, energy, reproduction and death.

use crate::events::EventKind;
use crate::heredity::Traits;
use crate::sim::Sim;
use crate::world::{ColClass, Dims, UNREACHABLE};
use rand::Rng;
use serde::Serialize;

/// Animal species.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Eats grass, flees hunters.
    Grazer,
    /// Hunts grazers.
    Hunter,
}

/// The behaviour an animal chose on its last update (written to snapshots).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Stepping out of a burning patch (either species), or a grazer stepping away from the nearest hunter.
    Flee,
    /// Grazer eating grass in its patch.
    Eat,
    /// Walking toward a target patch (grazer) or prey (hunter).
    Move,
    /// Random step: nothing better to do.
    Wander,
    /// Satiated hunter; makes no attack.
    Rest,
    /// Hunter attacking prey within the attack radius.
    Hunt,
    /// Hunter handling a kill: stays put, makes no attack, pays the resting cost.
    Handling,
}

/// Why an animal died. The discriminant indexes the per-tick death counts (`Sim::deaths`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// Energy reached 0. Takes precedence when old age falls on the same tick.
    Starved = 0,
    /// Killed by a hunter's attack.
    Eaten = 1,
    /// Reached the species' `max_age`.
    OldAge = 2,
    /// Density-dependent mortality: too many of its species in its patch (`[disease]`).
    Crowded = 3,
    /// Energy reached 0 in a burning patch (fire damage took part). Takes precedence over `Starved`.
    Burnt = 4,
}

/// Number of death causes.
pub const CAUSES: usize = 5;

impl Cause {
    /// Every cause, in discriminant order.
    pub const ALL: [Cause; CAUSES] = [Cause::Starved, Cause::Eaten, Cause::OldAge, Cause::Crowded, Cause::Burnt];

    /// The name used in `series.csv` columns and reports.
    pub fn name(self) -> &'static str {
        ["starved", "eaten", "old_age", "crowded", "burnt"][self as usize]
    }
}

/// One grazer or hunter.
#[derive(Debug, Clone)]
pub struct Animal {
    /// Unique id, shared with trees (allocated from `Sim::next_id`).
    pub id: u32,
    /// Species.
    pub kind: Kind,
    /// Column coordinates; always integer-valued.
    pub x: f32,
    /// Row coordinate; always integer-valued.
    pub y: f32,
    /// Energy in [0, 100]; the animal dies at 0.
    pub energy: f32,
    /// Age in ticks.
    pub age: u32,
    /// Ticks until it may reproduce again.
    pub cooldown: u32,
    /// Behaviour chosen on the last update.
    pub state: State,
    /// False once dead; removed at the next compaction.
    pub alive: bool,
    /// Heritable traits, used in place of the species parameters they name.
    pub traits: Traits,
    /// Hunter only: updates left in state Handling after a kill (0 when not handling).
    pub handling: u32,
    /// Nitrogen, phosphorus and potassium in the animal's body, in grams (shot G5).
    ///
    /// Every animal starts empty -- placed, born or immigrated -- and a grazer fills up on what it
    /// crops, passing whatever it cannot hold (`animals.npk_content × energy`) into the dung of
    /// the patch it is standing on. A hunter never fills up at all: it takes energy from a kill,
    /// not matter, so a carcass's nutrients go to the ground where it fell rather than into the
    /// animal that made it (DECISIONS.md, shot G5).
    pub npk: [f64; 3],
}

/// The 8 neighbour offsets, in a fixed order so random picks are deterministic.
const NEIGHBOURS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

impl Animal {
    /// A live animal on column (x, y), in state Wander.
    pub fn new(
        id: u32,
        kind: Kind,
        (x, y): (usize, usize),
        energy: f32,
        age: u32,
        cooldown: u32,
        traits: Traits,
    ) -> Animal {
        Animal {
            id,
            kind,
            x: x as f32,
            y: y as f32,
            energy,
            age,
            cooldown,
            state: State::Wander,
            alive: true,
            traits,
            handling: 0,
            npk: [0.0; 3],
        }
    }

    /// The column it stands on, as signed coordinates.
    #[inline]
    pub fn col(&self) -> (i32, i32) {
        (self.x as i32, self.y as i32)
    }

    /// The patch it stands in.
    #[inline]
    pub fn patch(&self, d: Dims) -> usize {
        d.patch_of(self.x as usize, self.y as usize)
    }

    /// Index of the column it stands on.
    #[inline]
    pub fn col_index(&self, d: Dims) -> usize {
        d.cidx(self.x as usize, self.y as usize)
    }
}

/// Stability rule 1 (type II response): intake = min(intake_max, intake_k · grass).
pub fn grazing_intake(grass: f32, intake_max: f32, intake_k: f32) -> f32 {
    (intake_k * grass).min(intake_max).max(0.0)
}

/// Stability rule 3 (continuous shrub refugium): chance an attack kills a grazer standing in a patch
/// with this shrub density, `kill_prob · (1 − shrub)^refugium_k`. Shrub only lowers the chance; it
/// never makes a grazer an illegal target.
pub fn attack_success(kill_prob: f64, shrub: f32, refugium_k: f32) -> f64 {
    let cover = (1.0 - shrub as f64).clamp(0.0, 1.0);
    (kill_prob * libm::pow(cover, refugium_k as f64)).clamp(0.0, 1.0)
}

/// Density-dependent mortality: the extra death chance, per update, of an animal in a patch holding
/// `n` of its species (itself included): `rate · max(0, n − threshold) / threshold`, clamped to
/// [0, 1]. It is 0 at or below the threshold. A threshold of 0 divides by 1.
pub fn crowding_death_p(rate: f32, n: u32, threshold: u32) -> f64 {
    if n <= threshold {
        return 0.0;
    }
    (rate as f64 * (n - threshold) as f64 / threshold.max(1) as f64).clamp(0.0, 1.0)
}

/// Why an animal that died this update died: `Burnt` if its energy ran out in a burning patch,
/// `Starved` if it ran out elsewhere, else `OldAge`.
fn death_cause(energy: f32, burning: bool) -> Cause {
    match (energy <= 0.0, burning) {
        (true, true) => Cause::Burnt,
        (true, false) => Cause::Starved,
        _ => Cause::OldAge,
    }
}

/// A grazer's fixed, arbitrary ranking of patches (lower is preferred): an integer hash mix.
fn preference(id: u32, patch: usize) -> u32 {
    let mut h = id.wrapping_mul(0x9E37_79B1) ^ (patch as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

fn grid_remove(cell: &mut Vec<u32>, i: usize) {
    if let Some(k) = cell.iter().position(|&j| j as usize == i) {
        cell.swap_remove(k);
    }
}

/// Scan `offsets` (nearest first) around (x, y) and return the lowest index accepted by `keep`
/// among the animals at the nearest distance that has any, with that distance.
fn nearest_in_grid(
    d: Dims,
    grid: &[Vec<u32>],
    offsets: &[(i32, i32, i32)],
    x: i32,
    y: i32,
    keep: impl Fn(usize) -> bool,
) -> Option<(usize, f32)> {
    let mut found: Option<(usize, i32)> = None;
    for &(dx, dy, d2) in offsets {
        if let Some((_, fd2)) = found {
            if d2 > fd2 {
                break;
            }
        }
        let (nx, ny) = (x + dx, y + dy);
        if !d.in_bounds(nx, ny) {
            continue;
        }
        for &j in &grid[d.cidx(nx as usize, ny as usize)] {
            let j = j as usize;
            if found.is_none_or(|(fj, _)| j < fj) && keep(j) {
                found = Some((j, d2));
            }
        }
    }
    found.map(|(j, d2)| (j, (d2 as f32).sqrt()))
}

fn dist(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

impl Sim {
    /// Valid (soil, in-world) neighbour columns of (x, y), in NEIGHBOURS order.
    fn valid_neighbours(&self, x: i32, y: i32) -> Vec<(i32, i32)> {
        NEIGHBOURS.iter().map(|&(dx, dy)| (x + dx, y + dy)).filter(|&(nx, ny)| self.world.is_soil(nx, ny)).collect()
    }

    fn random_step(&mut self, x: i32, y: i32) -> Option<(i32, i32)> {
        let n = self.valid_neighbours(x, y);
        if n.is_empty() {
            None
        } else {
            Some(n[self.rng.gen_range(0..n.len())])
        }
    }

    /// The neighbour that minimises (sign = 1) or maximises (sign = −1) distance to a target,
    /// only if it strictly improves on the current column. First in NEIGHBOURS order wins ties.
    fn greedy_step(&self, x: i32, y: i32, tx: f32, ty: f32, sign: f32) -> Option<(i32, i32)> {
        let mut best = sign * dist(x as f32, y as f32, tx, ty);
        let mut pick = None;
        for (nx, ny) in self.valid_neighbours(x, y) {
            let d = sign * dist(nx as f32, ny as f32, tx, ty);
            if d < best {
                best = d;
                pick = Some((nx, ny));
            }
        }
        pick
    }

    /// One step along the shortest soil path into patch `q`: the first neighbour (NEIGHBOURS order)
    /// with the lowest walking distance, so terrain never pins a grazer against a wall.
    fn step_toward_patch(&self, x: i32, y: i32, q: usize) -> Option<(i32, i32)> {
        self.valid_neighbours(x, y)
            .into_iter()
            .min_by_key(|&(nx, ny)| self.world.dist_to_patch(q, self.world.dims.cidx(nx as usize, ny as usize)))
    }

    /// Move grazer `i` to (x, y), keeping the per-patch counts and the column grid current.
    fn move_grazer(&mut self, i: usize, x: i32, y: i32) {
        let d = self.world.dims;
        let (from, from_col) = (self.grazers[i].patch(d), self.grazers[i].col_index(d));
        self.grazers[i].x = x as f32;
        self.grazers[i].y = y as f32;
        let (to, to_col) = (self.grazers[i].patch(d), self.grazers[i].col_index(d));
        if from != to {
            self.grazers_in_patch[from] -= 1;
            self.grazers_in_patch[to] += 1;
        }
        if from_col != to_col {
            grid_remove(&mut self.grazer_grid[from_col], i);
            self.grazer_grid[to_col].push(i as u32);
        }
    }

    fn kill_grazer(&mut self, i: usize, cause: Cause) {
        self.deaths[Kind::Grazer as usize][cause as usize] += 1;
        let (id, at) = (self.grazers[i].id, (self.grazers[i].x as usize, self.grazers[i].y as usize));
        self.log_at(EventKind::Death, "grazer", at, cause.name(), id);
        let d = self.world.dims;
        let g = &mut self.grazers[i];
        g.alive = false;
        let (p, c) = (g.patch(d), g.col_index(d));
        self.grazers_in_patch[p] -= 1;
        grid_remove(&mut self.grazer_grid[c], i);
        self.patches[p].detritus += self.params.grazer.corpse_detritus;
        let held = std::mem::take(&mut self.grazers[i].npk);
        self.npk_to_detritus(p, held);
    }

    /// Move what a grazer just cropped off patch `p` into the grazer, and whatever it cannot hold
    /// on into the patch's dung. `before` is the patch's grass density as the bite found it.
    fn graze_npk(&mut self, i: usize, p: usize, before: f32) {
        let eaten = (before - self.patches[p].grass) as f64 * self.world.patch_soil[p].len() as f64;
        let needs = self.params.grass.npk.needs();
        let content = self.params.animals.npk_content;
        let mut dung = [0.0f64; 3];
        let g = &mut self.grazers[i];
        for (k, v) in g.npk.iter_mut().enumerate() {
            *v += eaten * needs[k] as f64;
        }
        crate::npk::excrete(g, content, &mut dung);
        self.npk_to_detritus(p, dung);
    }

    /// Animals phase: grazers in Vec order, then hunters. Newborns act from the next tick.
    /// Crowding mortality is checked per species here, once, so a rate of 0 never reaches a draw.
    /// So is mutation: at `heredity.mutation` 0 newborns copy their parent's traits with no draw.
    pub fn update_animals(&mut self) {
        self.rebuild_hunter_grid();
        let d = &self.params.disease;
        let (grazer_crowding, hunter_crowding) = (d.grazer_rate > 0.0, d.hunter_rate > 0.0);
        let mutate = self.params.heredity.mutation > 0.0;
        let n = self.grazers.len();
        for i in 0..n {
            if self.grazers[i].alive {
                self.update_grazer(i, grazer_crowding, mutate);
            }
        }
        let n = self.hunters.len();
        for i in 0..n {
            if self.hunters[i].alive {
                self.update_hunter(i, hunter_crowding, mutate);
            }
        }
    }

    /// One crowding draw for an animal in a patch holding `n` of its species. No draw when the
    /// chance is 0 (at or below the threshold).
    fn crowded_out(&mut self, rate: f32, n: u32, threshold: u32) -> bool {
        let p = crowding_death_p(rate, n, threshold);
        p > 0.0 && self.rng.gen_bool(p)
    }

    /// Nearest live hunter within `flee_distance` of column (x, y); lowest index wins ties. The
    /// offsets are the prefix of `flee_offsets` (built for the largest distance the clamp allows)
    /// that lies within `flee_distance`: the same list `offsets_within(flee_distance)` builds.
    pub(crate) fn nearest_hunter(&self, x: i32, y: i32, flee_distance: f32) -> Option<(f32, f32)> {
        let n = self.flee_offsets.partition_point(|&(_, _, d2)| (d2 as f32).sqrt() <= flee_distance);
        nearest_in_grid(self.world.dims, &self.hunter_grid, &self.flee_offsets[..n], x, y, |_| true)
            .map(|(j, _)| (self.hunters[j].x, self.hunters[j].y))
    }

    /// Target patch within Chebyshev patch-distance `search_patches`, scored grass − grazers/crowding.
    /// Every reachable patch scoring within `choice_tolerance` of the best is acceptable. The grazer
    /// stays if its own patch is acceptable; otherwise it takes the acceptable patch it personally
    /// prefers (a fixed hash of its id and the patch index), so neighbours don't all stampede to the
    /// same patch. Patches it cannot walk to (no soil, or cut off by water/rock) are never chosen.
    fn target_patch(&self, id: u32, p: usize, c: usize) -> usize {
        let gp = &self.params.grazer;
        let d = self.world.dims;
        let (px, py) = (d.patch_xy(p).0 as i32, d.patch_xy(p).1 as i32);
        let r = gp.search_patches;
        let (cols, rows) = (d.patches_x() as i32, d.patches_y() as i32);
        let mut scored = Vec::with_capacity(25);
        for qy in (py - r).max(0)..=(py + r).min(rows - 1) {
            for qx in (px - r).max(0)..=(px + r).min(cols - 1) {
                let q = qx as usize + d.patches_x() * qy as usize;
                if self.world.dist_to_patch(q, c) == UNREACHABLE {
                    continue;
                }
                scored.push((q, self.patches[q].grass - self.grazers_in_patch[q] as f32 / gp.crowding));
            }
        }
        let best = scored.iter().map(|s| s.1).fold(f32::NEG_INFINITY, f32::max);
        let floor = best - gp.choice_tolerance;
        if scored.iter().any(|&(q, s)| q == p && s >= floor) {
            return p;
        }
        scored.iter().filter(|s| s.1 >= floor).min_by_key(|s| preference(id, s.0)).map_or(p, |s| s.0)
    }

    /// Fire damage for an animal standing in patch `p`: its new energy, and whether the patch burns.
    fn scorch(&self, p: usize, energy: f32) -> (f32, bool) {
        if self.is_burning(p) {
            (energy - self.params.fire.animal_damage, true)
        } else {
            (energy, false)
        }
    }

    /// The step out of burning patch `p`: away from its centre, as from a hunter.
    fn flee_fire(&self, x: i32, y: i32, p: usize) -> Option<(i32, i32)> {
        let (cx, cy) = crate::fire::patch_centre(self.world.dims, p);
        self.greedy_step(x, y, cx, cy, -1.0)
    }

    pub(crate) fn update_grazer(&mut self, i: usize, crowding: bool, mutate: bool) {
        let gp = self.params.grazer.clone();
        let d = self.world.dims;
        let p = self.grazers[i].patch(d);
        let (energy, burning) = self.scorch(p, self.grazers[i].energy);
        {
            let g = &mut self.grazers[i];
            g.age += 1;
            g.cooldown = g.cooldown.saturating_sub(1);
            g.energy = energy;
        }
        let (x, y) = self.grazers[i].col();
        let mut step = None;
        let state;
        if burning {
            state = State::Flee;
            step = self.flee_fire(x, y, p);
        } else if let Some((hx, hy)) = self.nearest_hunter(x, y, self.grazers[i].traits.flee_distance) {
            state = State::Flee;
            step = self.greedy_step(x, y, hx, hy, -1.0);
        } else if self.patches[p].grass > gp.eat_min_grass && self.grazers[i].energy < gp.eat_below {
            state = State::Eat;
            let intake = grazing_intake(self.patches[p].grass, gp.intake_max, gp.intake_k);
            let before = self.patches[p].grass;
            let g = &mut self.grazers[i];
            g.energy = (g.energy + intake).min(100.0);
            let grass = &mut self.patches[p].grass;
            *grass = (*grass - intake * gp.grass_per_energy).max(0.0);
            if self.npk.is_some() {
                self.graze_npk(i, p, before);
            }
        } else {
            let best = self.target_patch(self.grazers[i].id, p, d.cidx(x as usize, y as usize));
            if best != p {
                state = State::Move;
                step = self.step_toward_patch(x, y, best);
            } else {
                state = State::Wander;
                step = self.random_step(x, y);
            }
        }
        if let Some((nx, ny)) = step {
            self.move_grazer(i, nx, ny);
        }
        let cost = gp.energy_cost * self.grazers[i].traits.energy_cost_mult * if step.is_some() { 2.0 } else { 1.0 };
        {
            let g = &mut self.grazers[i];
            g.state = state;
            g.energy -= cost;
        }
        let g = &self.grazers[i];
        if g.energy <= 0.0 || g.age >= gp.max_age {
            self.kill_grazer(i, death_cause(g.energy, burning));
            return;
        }
        let p = g.patch(d);
        let dp = &self.params.disease;
        if crowding && self.crowded_out(dp.grazer_rate, self.grazers_in_patch[p], dp.grazer_threshold) {
            self.kill_grazer(i, Cause::Crowded);
            return;
        }
        let g = &self.grazers[i];
        if g.energy > g.traits.repro_threshold && g.cooldown == 0 && self.grazers_in_patch[p] < gp.max_grazers_per_patch
        {
            let (cx, cy, parent) = (g.x as usize, g.y as usize, g.traits);
            let g = &mut self.grazers[i];
            g.energy -= gp.repro_cost;
            g.cooldown = gp.cooldown;
            let traits = self.offspring_traits(Kind::Grazer, parent, mutate);
            let id = self.add_grazer(cx, cy, gp.newborn_energy, gp.cooldown, traits);
            self.log_at(EventKind::Birth, "grazer", (cx, cy), "", id);
        }
    }

    /// Test helper: add a newborn grazer with the default traits on column (x, y), keeping the per-patch counts
    /// and the column grid current.
    #[cfg(test)]
    pub(crate) fn spawn_grazer(&mut self, x: usize, y: usize) {
        let gp = &self.params.grazer;
        self.add_grazer(x, y, gp.newborn_energy, gp.cooldown, self.params.default_traits(Kind::Grazer));
    }

    /// Add a live grazer on column (x, y) and return its id.
    fn add_grazer(&mut self, x: usize, y: usize, energy: f32, cooldown: u32, traits: Traits) -> u32 {
        let id = self.alloc_id();
        let d = self.world.dims;
        self.grazer_grid[d.cidx(x, y)].push(self.grazers.len() as u32);
        self.grazers.push(Animal::new(id, Kind::Grazer, (x, y), energy, 0, cooldown, traits));
        self.grazers_in_patch[d.patch_of(x, y)] += 1;
        id
    }

    /// A uniformly random soil column on the world's edge (x or y at 0 or its last value), or None
    /// (and no draw) if the edge has none.
    fn random_edge_soil_column(&mut self) -> Option<(usize, usize)> {
        let d = self.world.dims;
        let edge: Vec<usize> = (0..d.cols())
            .filter(|&c| {
                let (x, y) = d.xy(c);
                (x == 0 || y == 0 || x == d.wx - 1 || y == d.wy - 1) && self.world.class[c] == ColClass::Soil
            })
            .collect();
        if edge.is_empty() {
            return None;
        }
        let c = edge[self.rng.gen_range(0..edge.len())];
        Some(d.xy(c))
    }

    /// Open boundaries: on ticks that are a multiple of a species' immigration cadence — the animals'
    /// `immigration_interval`, and for the tree the cadence derived from `tree.immigrants_per_year`
    /// (shot G4e) — one immigrant of that species arrives at a random edge soil column if fewer than
    /// `immigration_floor` are alive. Animal immigrants have the default traits, `start_energy`, age
    /// 0 and cooldown 0, and act from the next tick. A tree immigrant is a sapling (age 0), planted
    /// only if its column keeps `min_spacing`. A floor of 0 never draws, and neither animal
    /// immigrates when `animals.enabled` is false.
    pub fn immigrate(&mut self, t: u32) {
        let animals = self.params.animals.enabled;
        let gp = self.params.grazer.clone();
        if animals && t.is_multiple_of(gp.immigration_interval) && self.count_grazers() < gp.immigration_floor {
            if let Some((x, y)) = self.random_edge_soil_column() {
                let id = self.add_grazer(x, y, gp.start_energy, 0, self.params.default_traits(Kind::Grazer));
                self.log_at(EventKind::Immigration, "grazer", (x, y), "", id);
            }
        }
        let hp = self.params.hunter.clone();
        if animals && t.is_multiple_of(hp.immigration_interval) && self.count_hunters() < hp.immigration_floor {
            if let Some((x, y)) = self.random_edge_soil_column() {
                let id = self.alloc_id();
                let traits = self.params.default_traits(Kind::Hunter);
                self.hunters.push(Animal::new(id, Kind::Hunter, (x, y), hp.start_energy, 0, 0, traits));
                self.hunters_in_patch[self.world.dims.patch_of(x, y)] += 1;
                self.hunter_immigrants += 1;
                self.log_at(EventKind::Immigration, "hunter", (x, y), "", id);
            }
        }
        let tree_every = self.params.tree_immigration_every();
        let tp = &self.params.tree;
        if t.is_multiple_of(tree_every) && self.count_trees() < tp.immigration_floor {
            if let Some((x, y)) = self.random_edge_soil_column() {
                // An edge column a roof crosses takes no trunk, the same as one too close to a
                // neighbour: the immigrant does not arrive this cadence (shot G12).
                if self.world.can_root_a_trunk(self.world.dims.cidx(x, y)) && self.spacing_ok(x as i32, y as i32) {
                    let id = self.plant_tree(x, y, 0);
                    self.log_at(EventKind::Immigration, "tree", (x, y), "", id);
                }
            }
        }
    }

    /// Nearest live grazer within the seek radius of column (x, y): (index, distance). Every live
    /// grazer is a legal target, whatever the shrub. Lowest index wins ties.
    fn nearest_prey(&self, x: i32, y: i32) -> Option<(usize, f32)> {
        nearest_in_grid(self.world.dims, &self.grazer_grid, &self.seek_offsets, x, y, |j| self.grazers[j].alive)
    }

    /// One attack by hunter `h` on grazer `j` (stability rules 2 and 3). Every attempt costs
    /// `hunt_cost`: a kill leaves `min(energy + kill_energy − hunt_cost, 100)` and starts
    /// `handling_ticks` of handling, a miss `energy − hunt_cost − fail_cost` and displaces the grazer.
    pub(crate) fn attack(&mut self, h: usize, j: usize) {
        let hp = self.params.hunter.clone();
        let shrub = self.patches[self.grazers[j].patch(self.world.dims)].shrub;
        let p = attack_success(hp.kill_prob, shrub, hp.refugium_k);
        if self.rng.gen_bool(p) {
            self.kill_grazer(j, Cause::Eaten);
            let h = &mut self.hunters[h];
            h.energy = (h.energy + hp.kill_energy - hp.hunt_cost).min(100.0);
            h.handling = hp.handling_ticks;
        } else {
            self.hunters[h].energy = self.hunters[h].energy - hp.hunt_cost - hp.fail_cost;
            let (hx, hy) = self.hunters[h].col();
            let (gx, gy) = self.grazers[j].col();
            let (mut dx, mut dy) = ((gx - hx).signum(), (gy - hy).signum());
            if dx == 0 && dy == 0 {
                (dx, dy) = NEIGHBOURS[self.rng.gen_range(0..NEIGHBOURS.len())];
            }
            let (mut x, mut y) = (gx, gy);
            for _ in 0..hp.displace_steps {
                if !self.world.is_soil(x + dx, y + dy) {
                    break;
                }
                x += dx;
                y += dy;
            }
            self.move_grazer(j, x, y);
        }
    }

    /// One hunter update: handle a kill while `handling` is left (no move, even out of a fire), else
    /// flee fire, rest when satiated, or attack, approach or wander; then energy, death (starved,
    /// burnt, old age, then crowded when `crowding` is on) and birth. Handling reads and writes no
    /// RNG, so at `handling_ticks` 0 the update is the pre-handling one.
    pub fn update_hunter(&mut self, i: usize, crowding: bool, mutate: bool) {
        let hp = self.params.hunter.clone();
        let d = self.world.dims;
        let p = self.hunters[i].patch(d);
        let (energy, burning) = self.scorch(p, self.hunters[i].energy);
        {
            let h = &mut self.hunters[i];
            h.age += 1;
            h.cooldown = h.cooldown.saturating_sub(1);
            h.energy = energy;
        }
        let (x, y) = self.hunters[i].col();
        let mut step = None;
        let state;
        if self.hunters[i].handling > 0 {
            state = State::Handling;
            self.hunters[i].handling -= 1;
        } else if burning {
            state = State::Flee;
            step = self.flee_fire(x, y, p);
        } else if self.hunters[i].energy > hp.satiation {
            state = State::Rest;
            step = self.random_step(x, y);
        } else {
            match self.nearest_prey(x, y) {
                Some((j, d)) if d <= hp.attack_radius => {
                    state = State::Hunt;
                    self.attack(i, j);
                }
                Some((j, _)) => {
                    state = State::Move;
                    let (gx, gy) = (self.grazers[j].x, self.grazers[j].y);
                    step = self.greedy_step(x, y, gx, gy, 1.0).or_else(|| self.random_step(x, y));
                }
                None => {
                    state = State::Wander;
                    step = self.random_step(x, y);
                }
            }
        }
        let cost = hp.energy_cost * self.hunters[i].traits.energy_cost_mult * if step.is_some() { 2.0 } else { 1.0 };
        let h = &mut self.hunters[i];
        if let Some((nx, ny)) = step {
            h.x = nx as f32;
            h.y = ny as f32;
            self.hunters_in_patch[p] -= 1;
            self.hunters_in_patch[h.patch(d)] += 1;
        }
        h.state = state;
        h.energy -= cost;
        let p = h.patch(d);
        if h.energy <= 0.0 || h.age >= hp.max_age {
            let cause = death_cause(h.energy, burning);
            self.kill_hunter(i, cause);
            return;
        }
        let dp = &self.params.disease;
        if crowding && self.crowded_out(dp.hunter_rate, self.hunters_in_patch[p], dp.hunter_threshold) {
            self.kill_hunter(i, Cause::Crowded);
            return;
        }
        let h = &mut self.hunters[i];
        if h.energy > h.traits.repro_threshold && h.cooldown == 0 {
            h.energy -= hp.repro_cost;
            h.cooldown = hp.refractory;
            let (cx, cy, parent) = (h.x as usize, h.y as usize, h.traits);
            let traits = self.offspring_traits(Kind::Hunter, parent, mutate);
            let id = self.alloc_id();
            self.hunters.push(Animal::new(id, Kind::Hunter, (cx, cy), hp.newborn_energy, 0, hp.refractory, traits));
            self.hunters_in_patch[p] += 1;
            self.log_at(EventKind::Birth, "hunter", (cx, cy), "", id);
        }
    }

    fn kill_hunter(&mut self, i: usize, cause: Cause) {
        self.deaths[Kind::Hunter as usize][cause as usize] += 1;
        let (id, at) = (self.hunters[i].id, (self.hunters[i].x as usize, self.hunters[i].y as usize));
        self.log_at(EventKind::Death, "hunter", at, cause.name(), id);
        let d = self.world.dims;
        let h = &mut self.hunters[i];
        h.alive = false;
        let p = h.patch(d);
        self.hunters_in_patch[p] -= 1;
        self.patches[p].detritus += self.params.hunter.corpse_detritus;
        let held = std::mem::take(&mut self.hunters[i].npk);
        self.npk_to_detritus(p, held);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;
    use crate::sim::offsets_within;
    use crate::world::sq::*;
    use crate::world::World;
    use proptest::prelude::*;
    use rand::SeedableRng;

    #[test]
    fn type_two_intake_caps_at_three() {
        assert_eq!(grazing_intake(1.0, 3.0, 20.0), 3.0);
        assert_eq!(grazing_intake(0.15, 3.0, 20.0), 3.0);
        assert!((grazing_intake(0.1, 3.0, 20.0) - 2.0).abs() < 1e-6);
        assert!((grazing_intake(0.01, 3.0, 20.0) - 0.2).abs() < 1e-6);
        assert_eq!(grazing_intake(0.0, 3.0, 20.0), 0.0);
    }

    /// One hunter next to one grazer, kill_prob 1: (grazer killed, hunter state, hunter energy).
    fn attack_outcome(shrub: f32) -> (bool, State, f32) {
        let mut p = Params::load_square();
        p.tree.initial_count = 0;
        p.grazer.start_count = 0;
        p.hunter.start_count = 0;
        p.hunter.kill_prob = 1.0;
        let world = World::from_heights(&vec![14u8; COLS], &p);
        let mut sim = Sim::with_world(p, rand_chacha::ChaCha8Rng::seed_from_u64(9), world);
        sim.grazers.push(Animal::new(0, Kind::Grazer, (10, 10), 50.0, 0, 100, sim.params.default_traits(Kind::Grazer)));
        sim.grazers_in_patch[patch_of(10, 10)] = 1;
        sim.hunters.push(Animal::new(1, Kind::Hunter, (11, 10), 50.0, 0, 100, sim.params.default_traits(Kind::Hunter)));
        sim.patches[patch_of(10, 10)].shrub = shrub;
        sim.rebuild_grazer_grid();
        sim.update_hunter(0, false, false);
        (!sim.grazers[0].alive, sim.hunters[0].state, sim.hunters[0].energy)
    }

    #[test]
    fn shrub_lowers_attack_success_but_never_forbids_the_attempt() {
        let hp = Params::load_square().hunter;
        let (killed, state, energy) = attack_outcome(0.0);
        assert!(
            killed && state == State::Hunt && energy == 50.0 + hp.kill_energy - hp.energy_cost,
            "bare ground: {energy}"
        );
        // Full shrub: success is 0, but the hunter still attacks and pays the failed-attack cost.
        let (killed, state, energy) = attack_outcome(1.0);
        assert!(
            !killed && state == State::Hunt && energy == 50.0 - hp.fail_cost - hp.energy_cost,
            "full shrub: {energy}"
        );
    }

    /// One direct attack by a hunter with `before` energy on an adjacent grazer on bare ground, at
    /// kill_prob 1 (`hit`) or 0: the hunter's energy afterwards.
    fn energy_after_attack(hit: bool, before: f32, kill_energy: f32, hunt_cost: f32, fail_cost: f32) -> f32 {
        let mut p = Params::load_square();
        (p.tree.initial_count, p.grazer.start_count, p.hunter.start_count) = (0, 0, 0);
        p.hunter.kill_prob = if hit { 1.0 } else { 0.0 };
        (p.hunter.kill_energy, p.hunter.hunt_cost, p.hunter.fail_cost) = (kill_energy, hunt_cost, fail_cost);
        let world = World::from_heights(&vec![14u8; COLS], &p);
        let mut sim = Sim::with_world(p, rand_chacha::ChaCha8Rng::seed_from_u64(3), world);
        sim.grazers.push(Animal::new(0, Kind::Grazer, (10, 10), 50.0, 0, 100, sim.params.default_traits(Kind::Grazer)));
        sim.grazers_in_patch[patch_of(10, 10)] = 1;
        sim.hunters.push(Animal::new(
            1,
            Kind::Hunter,
            (11, 10),
            before,
            0,
            100,
            sim.params.default_traits(Kind::Hunter),
        ));
        sim.rebuild_grazer_grid();
        sim.attack(0, 0);
        assert_eq!(sim.grazers[0].alive, !hit);
        sim.hunters[0].energy
    }

    /// Food-limited hunters: every attempt costs `hunt_cost`. After a kill the hunter has
    /// `before + kill_energy − hunt_cost`, clamped to 100; after a miss `before − hunt_cost`, less
    /// the extra `fail_cost` a miss has always cost.
    fn attack_energy_accounting(before: f32, kill: f32, hunt: f32, fail: f32) -> Result<(), TestCaseError> {
        prop_assert_eq!(energy_after_attack(true, before, kill, hunt, fail), (before + kill - hunt).min(100.0));
        prop_assert_eq!(energy_after_attack(false, before, kill, hunt, fail), before - hunt - fail);
        Ok(())
    }

    #[test]
    fn attack_energy_regression_clamp_and_zero_costs() {
        // A kill that would overshoot is clamped after the cost is taken: 90 + 40 − 1 → 100.
        assert_eq!(energy_after_attack(true, 90.0, 40.0, 1.0, 0.25), 100.0);
        assert_eq!(energy_after_attack(true, 50.0, 40.0, 1.0, 0.25), 89.0);
        // hunt_cost 0 is the pre-shot rule: a kill adds kill_energy, only a miss costs fail_cost.
        assert_eq!(energy_after_attack(true, 30.0, 40.0, 0.0, 0.25), 70.0);
        assert_eq!(energy_after_attack(false, 30.0, 40.0, 0.0, 0.25), 29.75);
        // fail_cost 0 leaves the property's bare form: a miss costs exactly hunt_cost.
        assert_eq!(energy_after_attack(false, 30.0, 40.0, 2.0, 0.0), 28.0);
        // Energy is not clamped below: a miss can take a hunter under 0, and it starves in its update.
        assert_eq!(energy_after_attack(false, 1.0, 40.0, 2.0, 0.25), -1.25);
    }

    #[test]
    fn attack_success_formula() {
        assert_eq!(attack_success(0.2, 0.0, 2.0), 0.2);
        assert!((attack_success(0.2, 0.5, 2.0) - 0.05).abs() < 1e-7);
        assert!((attack_success(1.0, 0.75, 1.0) - 0.25).abs() < 1e-7);
        assert_eq!(attack_success(0.2, 1.0, 2.0), 0.0);
        assert_eq!(attack_success(0.2, 0.6, 0.0), 0.2, "k = 0 switches the refugium off");
    }

    /// Stability rule 3: success equals `kill_prob` on bare ground, never exceeds it, and does not
    /// rise as shrub rises.
    fn success_monotone_in_shrub(kill_prob: f64, a: f32, b: f32, k: f32) -> Result<(), TestCaseError> {
        let (lo, hi) = (a.min(b), a.max(b));
        prop_assert_eq!(attack_success(kill_prob, 0.0, k), kill_prob);
        let (at_lo, at_hi) = (attack_success(kill_prob, lo, k), attack_success(kill_prob, hi, k));
        prop_assert!(at_hi <= at_lo, "success rose from {} to {} as shrub rose {} -> {}", at_lo, at_hi, lo, hi);
        prop_assert!((0.0..=kill_prob).contains(&at_hi) && at_lo <= kill_prob);
        Ok(())
    }

    fn intake_bounded_and_monotone(a: f32, b: f32, k: f32) -> Result<(), TestCaseError> {
        let max = Params::load_square().grazer.intake_max;
        let (lo, hi) = (grazing_intake(a.min(b), max, k), grazing_intake(a.max(b), max, k));
        prop_assert!((0.0..=3.0).contains(&lo) && (0.0..=3.0).contains(&hi), "{lo} {hi}");
        prop_assert!(lo <= hi, "intake fell from {lo} to {hi} as grass rose");
        Ok(())
    }

    /// An all-soil sim with grazers at `grazers` and per-patch shrub cover `shrub`.
    fn meadow(grazers: &[(usize, usize)], shrub: &[f32]) -> Sim {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        for (p, &s) in shrub.iter().enumerate() {
            sim.patches[p].shrub = s;
        }
        for &(x, y) in grazers {
            sim.spawn_grazer(x, y);
        }
        sim
    }

    fn add_hunter(sim: &mut Sim, (x, y): (usize, usize), energy: f32) {
        let id = sim.alloc_id();
        sim.hunters.push(Animal::new(
            id,
            Kind::Hunter,
            (x, y),
            energy,
            0,
            100,
            sim.params.default_traits(Kind::Hunter),
        ));
        sim.rebuild_hunter_grid();
    }

    fn positions(sim: &Sim) -> Vec<(bool, f32, f32)> {
        sim.grazers.iter().map(|g| (g.alive, g.x, g.y)).collect()
    }

    /// Stability rule 3 has no threshold: a hungry hunter's prey is the nearest live grazer within
    /// the seek radius whatever the shrub (lowest index on ties), it attacks that grazer when in reach,
    /// and no other grazer is touched. A grazer whose success chance is 0 always survives.
    fn every_grazer_is_a_legal_target(
        grazers: &[(usize, usize)],
        shrub: &[f32],
        hunter: (usize, usize),
        energy: f32,
        kill_prob: f64,
    ) -> Result<(), TestCaseError> {
        let mut sim = meadow(grazers, shrub);
        sim.params.hunter.kill_prob = kill_prob;
        add_hunter(&mut sim, hunter, energy);
        let (hx, hy) = (hunter.0 as i32, hunter.1 as i32);
        let hp = sim.params.hunter.clone();
        let want = grazers
            .iter()
            .enumerate()
            .map(|(j, &(x, y))| ((x as i32 - hx).pow(2) + (y as i32 - hy).pow(2), j))
            .filter(|&(d2, _)| (d2 as f32).sqrt() <= hp.seek_radius)
            .min();
        let got = sim.nearest_prey(hx, hy);
        prop_assert_eq!(got.map(|g| g.0), want.map(|w| w.1), "prey is not the nearest grazer");
        let before = positions(&sim);
        sim.update_hunter(0, false, false);
        let after = positions(&sim);
        for (j, (b, a)) in before.iter().zip(&after).enumerate() {
            if Some(j) != got.map(|g| g.0) {
                prop_assert_eq!(b, a, "grazer {} is not the prey but was touched", j);
            }
        }
        if let Some((j, d)) = got.filter(|g| g.1 <= hp.attack_radius) {
            prop_assert_eq!(sim.hunters[0].state, State::Hunt);
            let p = attack_success(kill_prob, sim.patches[patch_of(grazers[j].0, grazers[j].1)].shrub, hp.refugium_k);
            prop_assert!(p > 0.0 || after[j].0, "grazer {} at distance {} died with success chance 0", j, d);
        }
        Ok(())
    }

    /// Stability rule 2: a hunter above satiation makes no attack, whatever is in reach and however
    /// bare the ground (kill_prob 1, any shrub).
    fn satiated_hunter_rests(
        grazers: &[(usize, usize)],
        shrub: &[f32],
        hunter: (usize, usize),
        energy: f32,
    ) -> Result<(), TestCaseError> {
        let mut sim = meadow(grazers, shrub);
        sim.params.hunter.kill_prob = 1.0;
        add_hunter(&mut sim, hunter, energy);
        let before = positions(&sim);
        sim.update_hunter(0, false, false);
        prop_assert_eq!(before, positions(&sim), "a satiated hunter touched a grazer");
        prop_assert_eq!(sim.hunters[0].state, State::Rest);
        Ok(())
    }

    #[derive(Debug, Clone)]
    enum Op {
        Move(usize, usize, usize),
        Birth(usize),
        Death(usize),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            3 => (any::<usize>(), 0..WX, 0..WY).prop_map(|(i, x, y)| Op::Move(i, x, y)),
            1 => any::<usize>().prop_map(Op::Birth),
            1 => any::<usize>().prop_map(Op::Death),
        ]
    }

    fn sorted_cells(grid: &[Vec<u32>]) -> Vec<Vec<u32>> {
        grid.iter()
            .map(|c| {
                let mut c = c.clone();
                c.sort_unstable();
                c
            })
            .collect()
    }

    /// The incrementally kept grazer grid and patch counts equal a rebuild after every operation.
    /// Order within a grid cell is irrelevant (queries take the lowest index), so cells compare sorted.
    fn grids_match_rebuild(start: &[(usize, usize)], ops: &[Op]) -> Result<(), TestCaseError> {
        let mut sim = meadow(start, &[0.0; 64]);
        for (n, op) in ops.iter().enumerate() {
            let live: Vec<usize> = (0..sim.grazers.len()).filter(|&i| sim.grazers[i].alive).collect();
            match *op {
                _ if live.is_empty() => sim.spawn_grazer(0, 0),
                Op::Move(i, x, y) => sim.move_grazer(live[i % live.len()], x as i32, y as i32),
                Op::Birth(i) => {
                    let (x, y) = sim.grazers[live[i % live.len()]].col();
                    sim.spawn_grazer(x as usize, y as usize);
                }
                Op::Death(i) => sim.kill_grazer(live[i % live.len()], Cause::Eaten),
            }
            let kept = (sorted_cells(&sim.grazer_grid), sim.grazers_in_patch.clone());
            let mut counts = vec![0u32; crate::world::sq::PATCHES];
            sim.grazers.iter().filter(|g| g.alive).for_each(|g| counts[g.patch(crate::world::sq::D)] += 1);
            sim.rebuild_grazer_grid();
            prop_assert_eq!(kept, (sorted_cells(&sim.grazer_grid), counts), "after op {} ({:?})", n, op);
        }
        Ok(())
    }

    /// `nearest_in_grid` equals a brute-force scan: minimum (d², index) over kept animals within r.
    fn nearest_matches_brute_force(
        animals: &[(i32, i32)],
        order: &[usize],
        keep: &[bool],
        (x, y): (i32, i32),
        r: f32,
    ) -> Result<(), TestCaseError> {
        let mut grid = vec![Vec::new(); COLS];
        for &j in order {
            let (ax, ay) = animals[j];
            grid[cidx(ax as usize, ay as usize)].push(j as u32);
        }
        let got = nearest_in_grid(D, &grid, &offsets_within(r), x, y, |j| keep[j]);
        let want = animals
            .iter()
            .enumerate()
            .filter(|&(j, _)| keep[j])
            .map(|(j, &(ax, ay))| ((ax - x).pow(2) + (ay - y).pow(2), j))
            .filter(|&(d2, _)| (d2 as f32).sqrt() <= r)
            .min()
            .map(|(d2, j)| (j, (d2 as f32).sqrt()));
        prop_assert_eq!(got, want);
        Ok(())
    }

    fn column() -> impl Strategy<Value = (usize, usize)> {
        (0..WX, 0..WY)
    }

    type NearestCase = (Vec<(i32, i32)>, Vec<usize>, Vec<bool>, (i32, i32), f32);

    fn nearest_case() -> impl Strategy<Value = NearestCase> {
        // A 12×12 corner window keeps animals dense enough for ties and exercises the world edge.
        (1usize..=200).prop_flat_map(|n| {
            (
                prop::collection::vec((0i32..12, 0i32..12), n),
                Just((0..n).collect::<Vec<_>>()).prop_shuffle(),
                prop::collection::vec(any::<bool>(), n),
                (0i32..12, 0i32..12),
                0.0f32..20.0,
            )
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(64)))]

        #[test]
        fn prop_intake_bounded_and_monotone(a in 0.0f32..=1.0, b in 0.0f32..=1.0, k in 0.0f32..100.0) {
            intake_bounded_and_monotone(a, b, k)?;
        }

        #[test]
        fn prop_attack_success_monotone_in_shrub(
            kill_prob in 0.0f64..=1.0,
            a in 0.0f32..=1.0,
            b in 0.0f32..=1.0,
            k in 0.0f32..8.0,
        ) {
            success_monotone_in_shrub(kill_prob, a, b, k)?;
        }

        #[test]
        fn prop_attack_energy_accounting(
            before in 0.0f32..=100.0,
            kill in 0.0f32..=100.0,
            hunt in 0.0f32..=5.0,
            fail in prop::sample::select(vec![0.0f32, 0.25, 1.0]),
        ) {
            attack_energy_accounting(before, kill, hunt, fail)?;
        }

        #[test]
        fn prop_every_grazer_is_a_legal_target(
            grazers in prop::collection::vec(column(), 1..40),
            shrub in prop::collection::vec(0.0f32..=1.0, 64),
            hunter in column(),
            energy in 1.0f32..=85.0,
            kill_prob in 0.0f64..=1.0,
        ) {
            every_grazer_is_a_legal_target(&grazers, &shrub, hunter, energy, kill_prob)?;
        }

        #[test]
        fn prop_satiated_hunter_rests(
            grazers in prop::collection::vec(column(), 1..40),
            shrub in prop::collection::vec(0.0f32..=1.0, 64),
            hunter in column(),
            energy in 85.001f32..=100.0,
        ) {
            satiated_hunter_rests(&grazers, &shrub, hunter, energy)?;
        }

        #[test]
        fn prop_immigration_follows_the_floor(
            heights in crate::world::tests::terrain(),
            hunters in 0usize..12,
            grazer_floor in 0u32..4,
            tree_floor in 0u32..3,
            t in 1u32..3000,
        ) {
            immigration_follows_the_floor(&heights, hunters, grazer_floor, tree_floor, t)?;
        }

        #[test]
        fn prop_grids_match_rebuild(
            start in prop::collection::vec(column(), 0..30),
            ops in prop::collection::vec(op(), 1..=500),
        ) {
            grids_match_rebuild(&start, &ops)?;
        }

        #[test]
        fn prop_nearest_matches_brute_force((animals, order, keep, at, r) in nearest_case()) {
            nearest_matches_brute_force(&animals, &order, &keep, at, r)?;
        }
    }

    #[test]
    fn intake_regression_saturated_vs_linear() {
        intake_bounded_and_monotone(0.1, 0.2, 20.0).unwrap();
    }

    #[test]
    fn success_regression_zero_exponent_at_full_shrub() {
        // (1 - 1)^0 is 1 in libm::pow, so k = 0 disables the refugium even at shrub 1.
        success_monotone_in_shrub(0.3, 1.0, 0.0, 0.0).unwrap();
    }

    #[test]
    fn target_regression_nearer_grazer_in_dense_shrub() {
        // Under the old threshold rule the nearer grazer (patch 0, shrub 0.9) was skipped for the
        // farther one; now it is the prey.
        let mut shrub = [0.0; 64];
        shrub[0] = 0.9;
        every_grazer_is_a_legal_target(&[(7, 3), (9, 3)], &shrub, (7, 4), 50.0, 1.0).unwrap();
    }

    #[test]
    fn target_regression_full_shrub_always_survives() {
        let mut shrub = [0.0; 64];
        shrub[patch_of(61, 53)] = 1.0;
        every_grazer_is_a_legal_target(&[(61, 53), (61, 60)], &shrub, (61, 52), 10.0, 1.0).unwrap();
    }

    #[test]
    fn satiation_regression_grazer_in_reach_on_bare_ground() {
        satiated_hunter_rests(&[(10, 10)], &[0.0; 64], (11, 10), 90.0).unwrap();
    }

    /// Open boundaries: at a multiple of the interval, exactly one hunter arrives on an edge soil
    /// column with the default traits, start energy, age 0 and cooldown 0, and only when fewer than
    /// the floor are alive; the cumulative counter tracks it. Grazers follow the same rule with their
    /// own floor, and trees too (a sapling, age 0). With no edge soil nothing arrives. When nothing is
    /// due there is no draw.
    fn immigration_follows_the_floor(
        heights: &[u8],
        hunters: usize,
        grazer_floor: u32,
        tree_floor: u32,
        t: u32,
    ) -> Result<(), TestCaseError> {
        let mut sim = Sim::bare(heights);
        let soil: Vec<usize> = (0..COLS).filter(|&c| sim.world.class[c] == ColClass::Soil).collect();
        prop_assume!(!soil.is_empty());
        sim.params.hunter.immigration_floor = 8;
        sim.params.grazer.immigration_floor = grazer_floor;
        sim.params.tree.immigration_floor = tree_floor;
        for k in 0..hunters {
            let c = soil[k % soil.len()];
            add_hunter(&mut sim, (c % WX, c / WX), 50.0);
        }
        let (h0, g0, n0, w0) =
            (sim.count_hunters(), sim.count_grazers(), sim.hunter_immigrants, sim.rng.get_word_pos());
        sim.immigrate(t);
        let is_edge = |c: usize| c.is_multiple_of(WX) || c / WX == 0 || c % WX == WX - 1 || c / WX == WY - 1;
        let any_edge = soil.iter().any(|&c| is_edge(c)) as u32;
        let hunter_due = (t.is_multiple_of(sim.params.hunter.immigration_interval) && h0 < 8) as u32;
        prop_assert_eq!(sim.count_hunters(), h0 + hunter_due * any_edge);
        prop_assert_eq!(sim.hunter_immigrants, n0 + hunter_due * any_edge);
        let grazer_due = (t.is_multiple_of(sim.params.grazer.immigration_interval) && g0 < grazer_floor) as u32;
        prop_assert_eq!(sim.count_grazers(), g0 + grazer_due * any_edge);
        let tree_due = (t.is_multiple_of(sim.params.tree_immigration_every()) && tree_floor > 0) as u32;
        prop_assert_eq!(sim.count_trees(), tree_due * any_edge, "the bare sim has no trees to crowd a sapling");
        if hunter_due + grazer_due + tree_due == 0 {
            prop_assert_eq!(sim.rng.get_word_pos(), w0, "a draw with nothing due");
        }
        let class = sim.world.class.clone();
        let placed_ok = |c: usize| class[c] == ColClass::Soil && is_edge(c);
        if hunter_due * any_edge == 1 {
            let h = sim.hunters.last().unwrap();
            prop_assert!(placed_ok(h.col_index(crate::world::sq::D)), "immigrant hunter at ({}, {})", h.x, h.y);
            prop_assert_eq!((h.energy, h.age, h.cooldown), (sim.params.hunter.start_energy, 0, 0));
            prop_assert_eq!(h.traits, sim.params.default_traits(Kind::Hunter));
        }
        if grazer_due * any_edge == 1 {
            let g = sim.grazers.last().unwrap();
            prop_assert!(placed_ok(g.col_index(crate::world::sq::D)), "immigrant grazer at ({}, {})", g.x, g.y);
            prop_assert_eq!((g.energy, g.age, g.cooldown), (sim.params.grazer.start_energy, 0, 0));
            prop_assert_eq!(g.traits, sim.params.default_traits(Kind::Grazer));
            let kept = sorted_cells(&sim.grazer_grid);
            sim.rebuild_grazer_grid();
            prop_assert_eq!(kept, sorted_cells(&sim.grazer_grid));
        }
        if tree_due * any_edge == 1 {
            let tr = sim.trees.last().unwrap();
            prop_assert!(placed_ok(tr.col(crate::world::sq::D)), "immigrant tree at ({}, {})", tr.x, tr.y);
            prop_assert_eq!((tr.age, sim.trunk_at[tr.col(crate::world::sq::D)]), (0, 0));
        }
        Ok(())
    }

    #[test]
    fn immigration_regression_flat_world() {
        // Tick 500 brings one hunter to the edge, tick 499 none; 8 live hunters stop it.
        let flat = vec![14u8; COLS];
        immigration_follows_the_floor(&flat, 0, 0, 0, 500).unwrap();
        immigration_follows_the_floor(&flat, 0, 3, 1, 499).unwrap();
        immigration_follows_the_floor(&flat, 8, 3, 1, 1000).unwrap();
        // The default floors (0) never bring a grazer or a tree.
        let mut sim = Sim::bare(&flat);
        let d = Params::load_square();
        sim.params.hunter.immigration_floor = 8;
        sim.params.grazer.immigration_floor = d.grazer.immigration_floor;
        sim.params.tree.immigration_floor = d.tree.immigration_floor;
        sim.immigrate(500);
        assert_eq!((sim.count_hunters(), sim.count_grazers(), sim.count_trees(), sim.hunter_immigrants), (1, 0, 0, 1));
    }

    #[test]
    fn immigration_regression_no_edge_soil_brings_nothing() {
        // Water all round the rim (height 8, below the water level), soil inside.
        let mut heights = vec![14u8; COLS];
        for (c, h) in heights.iter_mut().enumerate() {
            let (x, y) = (c % WX, c / WX);
            if x == 0 || y == 0 || x == WX - 1 || y == WY - 1 {
                *h = 8;
            }
        }
        immigration_follows_the_floor(&heights, 0, 1, 1, 1500).unwrap();
    }

    /// A tree immigrant keeps `min_spacing`: an edge column next to a live trunk gets no sapling.
    #[test]
    fn tree_immigrant_respects_min_spacing() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        for x in (0..WX).step_by(2) {
            for y in [0, WY - 1] {
                sim.plant_tree(x, y, 0);
            }
        }
        for y in (2..WY - 2).step_by(2) {
            for x in [0, WX - 1] {
                sim.plant_tree(x, y, 0);
            }
        }
        let n = sim.count_trees();
        sim.params.tree.immigration_floor = n + 1;
        sim.immigrate(sim.params.tree_immigration_every());
        assert_eq!(sim.count_trees(), n, "every edge column is within min_spacing of a trunk");
    }

    /// Tree immigration fires on the **world** clock at the cadence `tree.immigrants_per_year`
    /// derives, for any rate (shot G4e), and the shipped 8 a year is the 500-tick
    /// `immigration_interval` the key replaced, to the tick. A rate of 0 leaves immigration out.
    #[test]
    fn a_tree_immigrates_at_its_rate_whatever_the_rate_is() {
        let shipped = crate::Params::load_square();
        assert_eq!(shipped.climate.year_len, 4000);
        assert_eq!(shipped.tree.immigrants_per_year, 8.0);
        assert_eq!(shipped.tree_immigration_every(), 500, "the shipped cadence, unmoved by the rename");
        // The tick of the first arrival in a fresh world whose floor is above the population.
        let first_arrival = |per_year: f32, until: u32| {
            let mut sim = Sim::bare(&vec![14u8; COLS]);
            sim.params.tree.immigrants_per_year = per_year;
            sim.params.tree.immigration_floor = 1;
            (1..=until).find(|&t| {
                sim.immigrate(t);
                sim.count_trees() > 0
            })
        };
        for (per_year, want) in [(8.0, 500), (4.0, 1000), (20.0, 200), (1.0, 4000), (4000.0, 1)] {
            let p = {
                let mut p = crate::Params::load_square();
                p.tree.immigrants_per_year = per_year;
                p
            };
            assert_eq!(p.tree_immigration_every(), want, "{per_year} a year");
            assert_eq!(first_arrival(per_year, 2 * want), Some(want), "{per_year} a year arrives at its cadence");
        }
        assert_eq!(first_arrival(0.0, 20_000), None, "a rate of 0 leaves immigration out");
    }

    #[test]
    fn grids_regression_move_across_patch_then_die() {
        let ops = [Op::Move(0, 8, 0), Op::Birth(0), Op::Move(1, 63, 63), Op::Death(0), Op::Death(0)];
        grids_match_rebuild(&[(7, 0), (7, 0)], &ops).unwrap();
    }

    #[test]
    fn nearest_regression_tie_goes_to_lowest_index() {
        // Indices 2 and 1 are both at distance 1 (inserted in that order); 0 is filtered out.
        let animals = [(5, 5), (6, 5), (4, 5)];
        nearest_matches_brute_force(&animals, &[2, 0, 1], &[false, true, true], (5, 5), 3.0).unwrap();
    }

    /// Crowding mortality: the extra death chance is 0 at or below the threshold, positive above it
    /// (for a positive rate), within [0, 1], and never falls as the patch count rises.
    fn crowding_zero_below_threshold_and_monotone(
        rate: f32,
        a: u32,
        b: u32,
        threshold: u32,
    ) -> Result<(), TestCaseError> {
        let (lo, hi) = (a.min(b), a.max(b));
        for n in [lo, hi] {
            let p = crowding_death_p(rate, n, threshold);
            prop_assert!((0.0..=1.0).contains(&p), "p({}) = {}", n, p);
            prop_assert_eq!(p == 0.0, n <= threshold || rate == 0.0, "p({}) = {} at threshold {}", n, p, threshold);
        }
        let (at_lo, at_hi) = (crowding_death_p(rate, lo, threshold), crowding_death_p(rate, hi, threshold));
        prop_assert!(at_lo <= at_hi, "p fell from {} to {} as n rose {} -> {}", at_lo, at_hi, lo, hi);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(64)))]

        #[test]
        fn prop_crowding_zero_below_threshold_and_monotone(
            rate in 0.0f32..=2.0,
            a in 0u32..200,
            b in 0u32..200,
            threshold in 0u32..40,
        ) {
            crowding_zero_below_threshold_and_monotone(rate, a, b, threshold)?;
        }
    }

    #[test]
    fn crowding_regression_threshold_edge_and_clamp() {
        // n = threshold is still 0; one above is rate / threshold; far above clamps to 1.
        crowding_zero_below_threshold_and_monotone(0.01, 16, 17, 16).unwrap();
        assert_eq!(crowding_death_p(0.01, 16, 16), 0.0);
        assert!((crowding_death_p(0.01, 24, 16) - 0.005).abs() < 1e-9);
        assert_eq!(crowding_death_p(5.0, 100, 4), 1.0);
        // Threshold 0 divides by 1 instead of by 0.
        crowding_zero_below_threshold_and_monotone(0.5, 0, 3, 0).unwrap();
        assert_eq!(crowding_death_p(0.5, 1, 0), 0.5);
    }

    /// 20 grazers eating on one column and 6 hunters wandering in the middle of one patch, at a rate
    /// high enough that any count above the threshold is certain death: each species is thinned to
    /// exactly its threshold, every death is `crowded`, and the kept counts match.
    #[test]
    fn crowding_thins_a_patch_to_its_threshold() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        sim.params.disease.grazer_rate = 1000.0;
        sim.params.disease.grazer_threshold = 10;
        sim.params.disease.hunter_rate = 1000.0;
        sim.params.disease.hunter_threshold = 2;
        for _ in 0..20 {
            sim.spawn_grazer(40, 40);
        }
        sim.grazers.iter_mut().for_each(|g| g.energy = 50.0);
        for _ in 0..6 {
            let id = sim.alloc_id();
            sim.hunters.push(Animal::new(
                id,
                Kind::Hunter,
                (4, 4),
                50.0,
                0,
                100,
                sim.params.default_traits(Kind::Hunter),
            ));
        }
        sim.update_animals();
        assert_eq!(sim.deaths, [[0, 0, 0, 10, 0], [0, 0, 0, 4, 0]]);
        assert_eq!((sim.grazers_in_patch[patch_of(40, 40)], sim.hunters_in_patch[0]), (10, 2));
        assert!(sim.grazers.iter().filter(|g| g.alive).all(|g| g.state == State::Eat));
    }

    /// At rate 0 the same crowded patch loses nothing, and the RNG ends where it does with crowding
    /// impossible (threshold above the count): the switched-off rule makes no draws.
    #[test]
    fn crowding_at_rate_zero_kills_nothing_and_draws_nothing() {
        let run = |rate: f32, threshold: u32| {
            let mut sim = Sim::bare(&vec![14u8; COLS]);
            sim.params.disease.grazer_rate = rate;
            sim.params.disease.grazer_threshold = threshold;
            sim.params.disease.hunter_rate = rate;
            sim.params.disease.hunter_threshold = threshold;
            for _ in 0..20 {
                sim.spawn_grazer(40, 40);
            }
            for _ in 0..6 {
                let id = sim.alloc_id();
                sim.hunters.push(Animal::new(
                    id,
                    Kind::Hunter,
                    (4, 4),
                    50.0,
                    0,
                    100,
                    sim.params.default_traits(Kind::Hunter),
                ));
            }
            for _ in 0..50 {
                sim.update_animals();
            }
            (sim.deaths, sim.count_grazers(), sim.count_hunters(), sim.rng.get_word_pos())
        };
        let off = run(0.0, 1);
        assert_eq!(off, run(1.0, 1000));
        assert_eq!(off.0[0][Cause::Crowded as usize] + off.0[1][Cause::Crowded as usize], 0);
    }

    /// Hunter births are gated by energy and a short refractory: a hunter above `repro_energy` with
    /// its refractory run out gives birth, and parent and newborn both wait `refractory` ticks;
    /// below `repro_energy` there is no birth whatever the refractory.
    #[test]
    fn hunter_births_are_energy_gated_with_a_refractory() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        let hp = sim.params.hunter.clone();
        let id = sim.alloc_id();
        sim.hunters.push(Animal::new(
            id,
            Kind::Hunter,
            (20, 20),
            hp.repro_energy + 5.0,
            0,
            1,
            sim.params.default_traits(Kind::Hunter),
        ));
        sim.rebuild_hunter_grid();
        sim.update_hunter(0, false, false);
        assert_eq!(sim.hunters.len(), 2, "cooldown 1 runs out on this update");
        assert_eq!((sim.hunters[0].cooldown, sim.hunters[1].cooldown), (hp.refractory, hp.refractory));
        assert_eq!(sim.hunters[1].energy, hp.newborn_energy);
        sim.hunters[0].cooldown = 0;
        sim.hunters[0].energy = hp.repro_energy - 1.0;
        sim.update_hunter(0, false, false);
        assert_eq!(sim.hunters.len(), 2, "no birth below repro_energy");
        sim.hunters[0].energy = hp.repro_energy + 5.0;
        sim.update_hunter(0, false, false);
        assert_eq!((sim.hunters.len(), sim.hunters_in_patch[patch_of(20, 20)]), (3, 3));
    }

    /// Knobs that make every recorded cause happen within a few hundred ticks.
    #[derive(Debug, Clone)]
    struct Mortality {
        grazer_cost: f32,
        grazer_max_age: u32,
        hunter_cost: f32,
        hunter_max_age: u32,
        kill_prob: f64,
    }

    /// With compaction off, dead animals stay in their Vec, so the deaths of a tick are the growth
    /// of each species' dead count, newborns eaten on their first tick included. The recorded causes
    /// must sum to exactly that, per species and tick, and the stats row must carry them. The kept
    /// per-patch hunter counts equal a recount after every tick.
    fn death_causes_sum_to_deaths(seed: u64, ticks: u32, m: &Mortality) -> Result<crate::sim::Deaths, TestCaseError> {
        let mut p = Params::load_square();
        p.world.compact_every = u32::MAX;
        p.grazer.energy_cost = m.grazer_cost;
        p.grazer.max_age = m.grazer_max_age;
        p.hunter.energy_cost = m.hunter_cost;
        p.hunter.max_age = m.hunter_max_age;
        p.hunter.kill_prob = m.kill_prob;
        let mut sim = Sim::new(p, seed);
        let dead = |s: &Sim| [&s.grazers, &s.hunters].map(|v| v.iter().filter(|a| !a.alive).count() as u32);
        prop_assert_eq!(sim.stats().deaths, crate::sim::Deaths::default());
        let mut total = crate::sim::Deaths::default();
        for _ in 0..ticks {
            let before = dead(&sim);
            sim.step();
            let after = dead(&sim);
            for k in 0..2 {
                let recorded: u32 = sim.deaths[k].iter().sum();
                prop_assert_eq!(recorded, after[k] - before[k], "species {} at tick {}", k, sim.tick);
            }
            let mut counts = vec![0u32; crate::world::sq::PATCHES];
            sim.hunters.iter().filter(|h| h.alive).for_each(|h| counts[h.patch(crate::world::sq::D)] += 1);
            prop_assert_eq!(&sim.hunters_in_patch, &counts, "hunters_in_patch at tick {}", sim.tick);
            prop_assert_eq!(sim.deaths[Kind::Hunter as usize][Cause::Eaten as usize], 0, "hunters have no predator");
            prop_assert_eq!(sim.stats().deaths, sim.deaths);
            total.iter_mut().flatten().zip(sim.deaths.iter().flatten()).for_each(|(t, d)| *t += d);
        }
        Ok(total)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(32)))]

        #[test]
        fn prop_death_causes_sum_to_deaths(
            seed in any::<u64>(),
            ticks in 1u32..250,
            grazer_cost in 0.05f32..1.5,
            grazer_max_age in 20u32..3000,
            hunter_cost in 0.02f32..1.5,
            hunter_max_age in 20u32..3000,
            kill_prob in 0.0f64..=1.0,
        ) {
            let m = Mortality { grazer_cost, grazer_max_age, hunter_cost, hunter_max_age, kill_prob };
            death_causes_sum_to_deaths(seed, ticks, &m)?;
        }
    }

    #[test]
    fn death_causes_regression_every_cause_in_one_run() {
        // grazer_cost rose from 0.6 to 1.2 in shot G5: the nutrient tier holds the sward below what
        // 0.6 could starve a grazer out of in 200 ticks, and this test needs a starvation to count.
        let m =
            Mortality { grazer_cost: 1.2, grazer_max_age: 1100, hunter_cost: 1.2, hunter_max_age: 900, kill_prob: 0.3 };
        let [g, h] = death_causes_sum_to_deaths(1, 200, &m).unwrap();
        assert!(g[..3].iter().all(|&n| n > 0), "grazer starved/eaten/old_age {g:?}");
        assert!(h[0] > 0 && h[2] > 0, "hunter starved/old_age {h:?}");
    }

    /// An animal out of energy on the tick it reaches `max_age` is counted once, as starved.
    #[test]
    fn death_cause_regression_starvation_beats_old_age() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        let (gmax, hmax) = (sim.params.grazer.max_age, sim.params.hunter.max_age);
        sim.grazers.push(Animal::new(
            0,
            Kind::Grazer,
            (10, 10),
            0.01,
            gmax - 1,
            100,
            sim.params.default_traits(Kind::Grazer),
        ));
        sim.grazers.push(Animal::new(
            1,
            Kind::Grazer,
            (40, 40),
            50.0,
            gmax - 1,
            100,
            sim.params.default_traits(Kind::Grazer),
        ));
        sim.grazers_in_patch[patch_of(10, 10)] += 1;
        sim.grazers_in_patch[patch_of(40, 40)] += 1;
        sim.patches[patch_of(10, 10)].grass = 0.0;
        sim.hunters.push(Animal::new(
            2,
            Kind::Hunter,
            (20, 50),
            0.01,
            hmax - 1,
            100,
            sim.params.default_traits(Kind::Hunter),
        ));
        sim.hunters.push(Animal::new(
            3,
            Kind::Hunter,
            (50, 20),
            50.0,
            hmax - 1,
            100,
            sim.params.default_traits(Kind::Hunter),
        ));
        sim.rebuild_grazer_grid();
        sim.step();
        assert_eq!(sim.deaths, [[1, 0, 1, 0, 0], [1, 0, 1, 0, 0]]);
        assert_eq!(sim.stats().deaths, sim.deaths);
        sim.step();
        assert_eq!(sim.deaths, [[0; CAUSES]; 2], "counts are per tick");
    }

    /// Kill ticks per hunter from `ticks` hunter updates (grazers stand still) of three always-hungry
    /// hunters in a herd of 300, at `handling_ticks` = `h`. Checks every update on the way: a hunter
    /// with handling left is in state Handling, kills nothing, stays put, pays exactly the resting
    /// cost (so it makes no attack, which would cost `hunt_cost`) and counts down by one; a kill
    /// happens only in state Hunt and sets handling to `h`; and every kill is followed by exactly `h`
    /// Handling updates (unless the run ends first).
    fn handling_run(h: u32, kill_prob: f64, seed: u64, ticks: u32) -> Result<Vec<Vec<u32>>, TestCaseError> {
        let herd: Vec<(usize, usize)> = (0..300).map(|i| (20 + i % 20, 20 + i / 20)).collect();
        let mut sim = meadow(&herd, &[]);
        sim.rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let hp = &mut sim.params.hunter;
        (hp.handling_ticks, hp.kill_prob, hp.satiation, hp.repro_energy, hp.hunt_cost) =
            (h, kill_prob, 1000.0, 1000.0, 0.5);
        for at in [(25, 25), (30, 28), (35, 31)] {
            add_hunter(&mut sim, at, 50.0);
        }
        let mut kills = vec![Vec::new(); 3];
        let mut handled: [Option<u32>; 3] = [None; 3];
        for t in 0..ticks {
            sim.rebuild_hunter_grid();
            for (i, k) in kills.iter_mut().enumerate() {
                let b = sim.hunters[i].clone();
                let eaten = sim.deaths[0][Cause::Eaten as usize];
                sim.update_hunter(i, false, false);
                let a = &sim.hunters[i];
                let killed = sim.deaths[0][Cause::Eaten as usize] > eaten;
                if a.state == State::Handling {
                    handled[i] = handled[i].map(|n| n + 1);
                } else if let Some(n) = handled[i].take() {
                    prop_assert_eq!(n, h, "handling lasted {} updates", n);
                }
                if b.handling > 0 {
                    prop_assert_eq!(a.state, State::Handling);
                    prop_assert!(!killed && (a.x, a.y) == (b.x, b.y), "a handling hunter attacked or moved");
                    prop_assert_eq!(a.energy, b.energy - sim.params.hunter.energy_cost * b.traits.energy_cost_mult);
                    prop_assert_eq!(a.handling, b.handling - 1);
                } else if killed {
                    prop_assert_eq!((a.state, a.handling), (State::Hunt, h));
                    k.push(t);
                    handled[i] = Some(0);
                } else {
                    prop_assert!(a.state != State::Handling && a.handling == 0);
                }
            }
        }
        prop_assert!(sim.hunters.iter().all(|a| a.alive), "the test needs all three alive");
        Ok(kills)
    }

    /// Handling lasts exactly `h` updates (checked in `handling_run`), so consecutive kills are at
    /// least h + 1 apart, and over any window of W ticks a hunter makes at most W/h + 1 kills.
    fn handling_bounds_kills(h: u32, kill_prob: f64, seed: u64) -> Result<(), TestCaseError> {
        for k in handling_run(h, kill_prob, seed, 400)? {
            for (i, &a) in k.iter().enumerate() {
                for (j, &b) in k.iter().enumerate().skip(i + 1) {
                    let w = b - a + 1;
                    prop_assert!((j - i + 1) as f64 <= w as f64 / h as f64 + 1.0, "{} kills in {} ticks", j - i + 1, w);
                }
                if let Some(&next) = k.get(i + 1) {
                    prop_assert!(next - a > h, "kills at {} and {}", a, next);
                }
            }
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(16)))]

        #[test]
        fn prop_handling_bounds_kills(h in 1u32..=120, kill_prob in prop_oneof![Just(1.0), 0.05f64..1.0], seed in any::<u64>()) {
            handling_bounds_kills(h, kill_prob, seed)?;
        }
    }

    /// Handling 1 at kill_prob 1: a kill every other update while prey stays in reach. A handling time longer than the
    /// run: one kill each. Handling 0 is the pre-handling rule: a kill on every update prey is in reach, never Handling.
    #[test]
    fn handling_regression_one_tick_longer_than_the_run_and_off() {
        handling_bounds_kills(1, 1.0, 5).unwrap();
        let every_other = handling_run(1, 1.0, 5, 40).unwrap();
        let want: Vec<u32> = (0..20).step_by(2).collect();
        assert!(every_other.iter().any(|k| k[..10] == want[..]), "{every_other:?}");
        let once = handling_run(1000, 1.0, 5, 400).unwrap();
        assert!(once.iter().all(|k| k.len() == 1), "{once:?}");
        let off = handling_run(0, 1.0, 5, 40).unwrap();
        let want: Vec<u32> = (0..10).collect();
        assert!(off.iter().any(|k| k[..10] == want[..]), "{off:?}");
    }

    /// `animals.enabled = false` against the same run with both animal species' `start_count` and
    /// `immigration_floor` at 0: the animal tier is empty either way, so the switch must make no
    /// draw and no write of its own. The two sims stay bit-identical (`state.bin` carries the RNG
    /// word position), no grazer or hunter ever exists, and no animal event is logged.
    fn animals_off_is_an_empty_animal_tier(seed: u64, ticks: u32) -> Result<(), TestCaseError> {
        let mut off = Params::load_square();
        off.animals.enabled = false;
        let mut empty = off.clone();
        empty.animals.enabled = true;
        empty.grazer.start_count = 0;
        empty.hunter.start_count = 0;
        empty.grazer.immigration_floor = 0;
        empty.hunter.immigration_floor = 0;
        // Off must also ignore floors that would otherwise let animals in.
        off.grazer.immigration_floor = 50;
        off.hunter.immigration_floor = 10;
        let (mut a, mut b) = (Sim::new(off, seed), Sim::new(empty, seed));
        a.log_events = true;
        for _ in 0..ticks {
            a.step();
            b.step();
            prop_assert_eq!(a.stats(), b.stats(), "tick {}", a.tick);
            prop_assert_eq!((a.count_grazers(), a.count_hunters()), (0, 0), "tick {}", a.tick);
        }
        prop_assert!(a.events.iter().all(|e| e.species != "grazer" && e.species != "hunter"), "animal event logged");
        prop_assert_eq!(crate::state::encode(&a), crate::state::encode(&b));
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(4)))]

        #[test]
        fn prop_animals_off_is_an_empty_animal_tier(seed in any::<u64>()) {
            animals_off_is_an_empty_animal_tier(seed, 400)?;
        }
    }

    /// The reference seeds, run past the first immigration interval and the first compaction.
    #[test]
    fn animals_off_regression_seeds_1_to_3() {
        for seed in 1..=3 {
            animals_off_is_an_empty_animal_tier(seed, 1200).unwrap();
        }
    }
}
