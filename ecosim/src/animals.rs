//! Grazers and hunters: fixed-priority behaviour, energy, reproduction and death.

use crate::sim::Sim;
use crate::world::{cidx, patch_of, PATCHES_X, UNREACHABLE};
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
    /// Grazer stepping away from the nearest hunter.
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
}

/// The 8 neighbour offsets, in a fixed order so random picks are deterministic.
const NEIGHBOURS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

impl Animal {
    /// A live animal on column (x, y), in state Wander.
    pub fn new(id: u32, kind: Kind, x: usize, y: usize, energy: f32, age: u32, cooldown: u32) -> Animal {
        Animal { id, kind, x: x as f32, y: y as f32, energy, age, cooldown, state: State::Wander, alive: true }
    }

    /// The column it stands on, as signed coordinates.
    #[inline]
    pub fn col(&self) -> (i32, i32) {
        (self.x as i32, self.y as i32)
    }

    /// The patch it stands in.
    #[inline]
    pub fn patch(&self) -> usize {
        patch_of(self.x as usize, self.y as usize)
    }
}

/// Stability rule 1 (type II response): intake = min(intake_max, intake_k · grass).
pub fn grazing_intake(grass: f32, intake_max: f32, intake_k: f32) -> f32 {
    (intake_k * grass).min(intake_max).max(0.0)
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
        if !crate::world::in_bounds(nx, ny) {
            continue;
        }
        for &j in &grid[cidx(nx as usize, ny as usize)] {
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
            .min_by_key(|&(nx, ny)| self.world.dist_to_patch(q, cidx(nx as usize, ny as usize)))
    }

    /// Move grazer `i` to (x, y), keeping the per-patch counts and the column grid current.
    fn move_grazer(&mut self, i: usize, x: i32, y: i32) {
        let (from, from_col) = (self.grazers[i].patch(), Sim::animal_col(&self.grazers[i]));
        self.grazers[i].x = x as f32;
        self.grazers[i].y = y as f32;
        let (to, to_col) = (self.grazers[i].patch(), Sim::animal_col(&self.grazers[i]));
        if from != to {
            self.grazers_in_patch[from] -= 1;
            self.grazers_in_patch[to] += 1;
        }
        if from_col != to_col {
            grid_remove(&mut self.grazer_grid[from_col], i);
            self.grazer_grid[to_col].push(i as u32);
        }
    }

    fn kill_grazer(&mut self, i: usize) {
        let g = &mut self.grazers[i];
        g.alive = false;
        let (p, c) = (g.patch(), Sim::animal_col(g));
        self.grazers_in_patch[p] -= 1;
        grid_remove(&mut self.grazer_grid[c], i);
        self.patches[p].detritus += self.params.grazer.corpse_detritus;
    }

    /// A grazer can be attacked only outside the shrub refugium (stability rule 3).
    pub fn attackable(&self, g: &crate::animals::Animal) -> bool {
        g.alive && self.patches[g.patch()].shrub <= self.params.hunter.refugium_shrub
    }

    /// Animals phase: grazers in Vec order, then hunters. Newborns act from the next tick.
    pub fn update_animals(&mut self) {
        self.rebuild_hunter_grid();
        let n = self.grazers.len();
        for i in 0..n {
            if self.grazers[i].alive {
                self.update_grazer(i);
            }
        }
        let n = self.hunters.len();
        for i in 0..n {
            if self.hunters[i].alive {
                self.update_hunter(i);
            }
        }
    }

    /// Nearest live hunter within the flee radius of column (x, y); lowest index wins ties.
    fn nearest_hunter(&self, x: i32, y: i32) -> Option<(f32, f32)> {
        nearest_in_grid(&self.hunter_grid, &self.flee_offsets, x, y, |_| true)
            .map(|(j, _)| (self.hunters[j].x, self.hunters[j].y))
    }

    /// Target patch within Chebyshev patch-distance `search_patches`, scored grass − grazers/crowding.
    /// Every reachable patch scoring within `choice_tolerance` of the best is acceptable. The grazer
    /// stays if its own patch is acceptable; otherwise it takes the acceptable patch it personally
    /// prefers (a fixed hash of its id and the patch index), so neighbours don't all stampede to the
    /// same patch. Patches it cannot walk to (no soil, or cut off by water/rock) are never chosen.
    fn target_patch(&self, id: u32, p: usize, c: usize) -> usize {
        let gp = &self.params.grazer;
        let (px, py) = ((p % PATCHES_X) as i32, (p / PATCHES_X) as i32);
        let r = gp.search_patches;
        let rows = (crate::world::PATCHES / PATCHES_X) as i32;
        let mut scored = Vec::with_capacity(25);
        for qy in (py - r).max(0)..=(py + r).min(rows - 1) {
            for qx in (px - r).max(0)..=(px + r).min(PATCHES_X as i32 - 1) {
                let q = qx as usize + PATCHES_X * qy as usize;
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

    fn update_grazer(&mut self, i: usize) {
        let gp = self.params.grazer.clone();
        {
            let g = &mut self.grazers[i];
            g.age += 1;
            g.cooldown = g.cooldown.saturating_sub(1);
        }
        let (x, y) = self.grazers[i].col();
        let p = self.grazers[i].patch();
        let mut step = None;
        let state;
        if let Some((hx, hy)) = self.nearest_hunter(x, y) {
            state = State::Flee;
            step = self.greedy_step(x, y, hx, hy, -1.0);
        } else if self.patches[p].grass > gp.eat_min_grass && self.grazers[i].energy < gp.eat_below {
            state = State::Eat;
            let intake = grazing_intake(self.patches[p].grass, gp.intake_max, gp.intake_k);
            let g = &mut self.grazers[i];
            g.energy = (g.energy + intake).min(100.0);
            let grass = &mut self.patches[p].grass;
            *grass = (*grass - intake * gp.grass_per_energy).max(0.0);
        } else {
            let best = self.target_patch(self.grazers[i].id, p, cidx(x as usize, y as usize));
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
        let cost = gp.energy_cost * if step.is_some() { 2.0 } else { 1.0 };
        {
            let g = &mut self.grazers[i];
            g.state = state;
            g.energy -= cost;
        }
        let g = &self.grazers[i];
        if g.energy <= 0.0 || g.age >= gp.max_age {
            self.kill_grazer(i);
            return;
        }
        let p = g.patch();
        if g.energy > gp.repro_energy && g.cooldown == 0 && self.grazers_in_patch[p] < gp.max_grazers_per_patch {
            let (cx, cy) = (g.x as usize, g.y as usize);
            let g = &mut self.grazers[i];
            g.energy -= gp.repro_cost;
            g.cooldown = gp.cooldown;
            self.spawn_grazer(cx, cy);
        }
    }

    /// Add a newborn grazer on column (x, y), keeping the per-patch counts and the column grid current.
    fn spawn_grazer(&mut self, x: usize, y: usize) {
        let gp = &self.params.grazer;
        let (energy, cooldown) = (gp.newborn_energy, gp.cooldown);
        let id = self.alloc_id();
        self.grazer_grid[cidx(x, y)].push(self.grazers.len() as u32);
        self.grazers.push(Animal::new(id, Kind::Grazer, x, y, energy, 0, cooldown));
        self.grazers_in_patch[patch_of(x, y)] += 1;
    }

    /// Nearest attackable grazer within the seek radius of column (x, y): (index, distance).
    /// Lowest index wins ties.
    fn nearest_prey(&self, x: i32, y: i32) -> Option<(usize, f32)> {
        nearest_in_grid(&self.grazer_grid, &self.seek_offsets, x, y, |j| self.attackable(&self.grazers[j]))
    }

    /// One attack by hunter `h` on grazer `j` (stability rule 2).
    fn attack(&mut self, h: usize, j: usize) {
        let hp = self.params.hunter.clone();
        if self.rng.gen_bool(hp.kill_prob.clamp(0.0, 1.0)) {
            self.kill_grazer(j);
            let e = &mut self.hunters[h].energy;
            *e = (*e + hp.kill_energy).min(100.0);
        } else {
            self.hunters[h].energy -= hp.fail_cost;
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

    /// One hunter update: rest when satiated, else attack, approach or wander; then energy, death and birth.
    pub fn update_hunter(&mut self, i: usize) {
        let hp = self.params.hunter.clone();
        {
            let h = &mut self.hunters[i];
            h.age += 1;
            h.cooldown = h.cooldown.saturating_sub(1);
        }
        let (x, y) = self.hunters[i].col();
        let mut step = None;
        let state;
        if self.hunters[i].energy > hp.satiation {
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
        let cost = hp.energy_cost * if step.is_some() { 2.0 } else { 1.0 };
        let h = &mut self.hunters[i];
        if let Some((nx, ny)) = step {
            h.x = nx as f32;
            h.y = ny as f32;
        }
        h.state = state;
        h.energy -= cost;
        if h.energy <= 0.0 || h.age >= hp.max_age {
            h.alive = false;
            let p = h.patch();
            self.patches[p].detritus += hp.corpse_detritus;
            return;
        }
        if h.energy > hp.repro_energy && h.cooldown == 0 {
            h.energy -= hp.repro_cost;
            h.cooldown = hp.cooldown;
            let (cx, cy) = (h.x as usize, h.y as usize);
            let id = self.alloc_id();
            self.hunters.push(Animal::new(id, Kind::Hunter, cx, cy, hp.newborn_energy, 0, hp.cooldown));
        }
    }

    /// Column index an animal stands on.
    pub fn animal_col(a: &Animal) -> usize {
        cidx(a.x as usize, a.y as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;
    use crate::sim::offsets_within;
    use crate::world::{World, COLS, WX, WY};
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

    /// One hunter next to one grazer, kill_prob 1: the attack succeeds unless the refugium applies.
    fn attack_outcome(shrub: f32) -> bool {
        let mut p = Params::load_default();
        p.tree.initial_count = 0;
        p.grazer.start_count = 0;
        p.hunter.start_count = 0;
        p.hunter.kill_prob = 1.0;
        let world = World::from_heights(&vec![14u8; COLS], &p);
        let mut sim = Sim::with_world(p, rand_chacha::ChaCha8Rng::seed_from_u64(9), world);
        sim.grazers.push(Animal::new(0, Kind::Grazer, 10, 10, 50.0, 0, 100));
        sim.grazers_in_patch[patch_of(10, 10)] = 1;
        sim.hunters.push(Animal::new(1, Kind::Hunter, 11, 10, 50.0, 0, 100));
        sim.patches[patch_of(10, 10)].shrub = shrub;
        sim.rebuild_grazer_grid();
        sim.update_hunter(0);
        !sim.grazers[0].alive
    }

    #[test]
    fn refugium_blocks_attack() {
        assert!(attack_outcome(0.4), "grazer outside refugium is killed");
        assert!(!attack_outcome(0.6), "grazer in shrub > 0.5 cannot be attacked");
    }

    fn intake_bounded_and_monotone(a: f32, b: f32, k: f32) -> Result<(), TestCaseError> {
        let max = Params::load_default().grazer.intake_max;
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
        sim.hunters.push(Animal::new(id, Kind::Hunter, x, y, energy, 0, 100));
        sim.rebuild_hunter_grid();
    }

    fn positions(sim: &Sim) -> Vec<(bool, f32, f32)> {
        sim.grazers.iter().map(|g| (g.alive, g.x, g.y)).collect()
    }

    /// Stability rule 3: a grazer in a patch whose shrub exceeds the threshold is never chosen as
    /// prey, and one hunter update leaves it alive and where it was.
    fn refuge_is_never_attacked(
        grazers: &[(usize, usize)],
        shrub: &[f32],
        hunter: (usize, usize),
        energy: f32,
        kill_prob: f64,
    ) -> Result<(), TestCaseError> {
        let mut sim = meadow(grazers, shrub);
        sim.params.hunter.kill_prob = kill_prob;
        add_hunter(&mut sim, hunter, energy);
        let threshold = sim.params.hunter.refugium_shrub;
        let in_refuge = |sim: &Sim, j: usize| sim.patches[sim.grazers[j].patch()].shrub > threshold;
        if let Some((j, _)) = sim.nearest_prey(hunter.0 as i32, hunter.1 as i32) {
            prop_assert!(!in_refuge(&sim, j), "grazer {j} in a refuge was selected as prey");
        }
        // Refuge membership is taken before the update: a failed attack can push a grazer into one.
        let sheltered: Vec<bool> = (0..sim.grazers.len()).map(|j| in_refuge(&sim, j)).collect();
        let before = positions(&sim);
        sim.update_hunter(0);
        for (j, (b, a)) in before.iter().zip(positions(&sim)).enumerate() {
            if sheltered[j] {
                prop_assert_eq!(*b, a, "refuge grazer {} was touched", j);
            }
        }
        Ok(())
    }

    /// Stability rule 2: a hunter above satiation makes no attack, whatever is in reach.
    fn satiated_hunter_rests(
        grazers: &[(usize, usize)],
        hunter: (usize, usize),
        energy: f32,
    ) -> Result<(), TestCaseError> {
        let mut sim = meadow(grazers, &[0.0; 64]);
        sim.params.hunter.kill_prob = 1.0;
        add_hunter(&mut sim, hunter, energy);
        let before = positions(&sim);
        sim.update_hunter(0);
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
                Op::Death(i) => sim.kill_grazer(live[i % live.len()]),
            }
            let kept = (sorted_cells(&sim.grazer_grid), sim.grazers_in_patch.clone());
            let mut counts = vec![0u32; crate::world::PATCHES];
            sim.grazers.iter().filter(|g| g.alive).for_each(|g| counts[g.patch()] += 1);
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
        let got = nearest_in_grid(&grid, &offsets_within(r), x, y, |j| keep[j]);
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
        fn prop_refuge_is_never_attacked(
            grazers in prop::collection::vec(column(), 1..40),
            shrub in prop::collection::vec(0.0f32..1.0, 64),
            hunter in column(),
            energy in 1.0f32..=85.0,
            kill_prob in 0.0f64..=1.0,
        ) {
            refuge_is_never_attacked(&grazers, &shrub, hunter, energy, kill_prob)?;
        }

        #[test]
        fn prop_satiated_hunter_rests(
            grazers in prop::collection::vec(column(), 1..40),
            hunter in column(),
            energy in 85.001f32..=100.0,
        ) {
            satiated_hunter_rests(&grazers, hunter, energy)?;
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
    fn refuge_regression_nearer_grazer_in_refuge() {
        // Patch 0 is a refuge, patch 1 is not; the refuge grazer is nearer but must be skipped.
        let mut shrub = [0.0; 64];
        shrub[0] = 0.9;
        refuge_is_never_attacked(&[(7, 3), (9, 3)], &shrub, (7, 4), 50.0, 1.0).unwrap();
    }

    #[test]
    fn refuge_regression_failed_attack_pushes_grazer_into_refuge() {
        // Found by proptest: an exposed grazer displaced by a failed attack lands in a refuge patch.
        let mut shrub = [0.0; 64];
        shrub[63] = 0.86;
        refuge_is_never_attacked(&[(61, 53)], &shrub, (61, 51), 1.0, 0.0).unwrap();
    }

    #[test]
    fn satiation_regression_grazer_in_reach() {
        satiated_hunter_rests(&[(10, 10)], (11, 10), 90.0).unwrap();
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
}
