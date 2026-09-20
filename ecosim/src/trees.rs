//! Tree entities: stages, canopy geometry and light, seeding and death.

use crate::events::EventKind;
use crate::producers::suitability;
use crate::sim::{Sim, NO_TREE};
use crate::world::Dims;
use rand::Rng;
use serde::Serialize;

/// Growth stage, from age.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// No canopy.
    Sapling,
    /// One canopy voxel over its own column.
    Young,
    /// Canopy over the 3×3 columns around the trunk, two voxels deep; seeds.
    Mature,
}

/// One tree.
#[derive(Debug, Clone)]
pub struct Tree {
    /// Unique id, shared with animals.
    pub id: u32,
    /// Trunk column x.
    pub x: u8,
    /// Trunk column y.
    pub y: u8,
    /// Age in ticks.
    pub age: u32,
    /// Consecutive ticks spent below `dry_moisture`.
    pub dry_ticks: u32,
    /// Age at which it dies, drawn at planting.
    pub lifespan: u32,
    /// False once dead; removed at the next compaction.
    pub alive: bool,
}

impl Tree {
    /// Index of the trunk column.
    #[inline]
    pub fn col(&self, d: Dims) -> usize {
        d.cidx(self.x as usize, self.y as usize)
    }
}

/// Stage for an age, given the young and mature thresholds.
pub fn stage_of(age: u32, young_age: u32, mature_age: u32) -> Stage {
    if age < young_age {
        Stage::Sapling
    } else if age < mature_age {
        Stage::Young
    } else {
        Stage::Mature
    }
}

impl Sim {
    /// Stage of a tree under the current params.
    pub fn tree_stage(&self, t: &Tree) -> Stage {
        stage_of(t.age, self.params.tree.young_age, self.params.tree.mature_age)
    }

    /// True if no live trunk is closer than `min_spacing` (Chebyshev) to (x, y).
    pub fn spacing_ok(&self, x: i32, y: i32) -> bool {
        let (r, d) = (self.params.tree.min_spacing - 1, self.world.dims);
        for dy in -r..=r {
            for dx in -r..=r {
                let (nx, ny) = (x + dx, y + dy);
                if d.in_bounds(nx, ny) && self.trunk_at[d.cidx(nx as usize, ny as usize)] != NO_TREE {
                    return false;
                }
            }
        }
        true
    }

    /// Distinct canopy voxel z values over column (x, y), from live young and mature trees.
    pub fn canopy_z(&self, x: i32, y: i32) -> Vec<u8> {
        let d = self.world.dims;
        let mut zs: Vec<u8> = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (tx, ty) = (x + dx, y + dy);
                if !d.in_bounds(tx, ty) {
                    continue;
                }
                let ti = self.trunk_at[d.cidx(tx as usize, ty as usize)];
                if ti == NO_TREE {
                    continue;
                }
                let t = &self.trees[ti as usize];
                let h = self.world.height[t.col(d)];
                match self.tree_stage(t) {
                    Stage::Sapling => {}
                    Stage::Young => {
                        if dx == 0 && dy == 0 {
                            zs.push(h + 2);
                        }
                    }
                    Stage::Mature => {
                        zs.push(h + 2);
                        zs.push(h + 3);
                    }
                }
            }
        }
        zs.sort_unstable();
        zs.dedup();
        zs
    }

    /// Recompute light and canopy cover for the 3×3 columns around a trunk.
    pub fn refresh_canopy_columns(&mut self, x: u8, y: u8) {
        let (absorb, d) = (self.params.world.canopy_absorb, self.world.dims);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (x as i32 + dx, y as i32 + dy);
                if !d.in_bounds(cx, cy) {
                    continue;
                }
                let zs = self.canopy_z(cx, cy);
                self.canopy_cover[d.cidx(cx as usize, cy as usize)] = !zs.is_empty();
                self.world.set_column_light(cx as usize, cy as usize, &zs, absorb);
            }
        }
    }

    /// Plant a tree on (x, y) and refresh the light of the columns its canopy covers. Its lifespan is
    /// `max_age · (1 + lifespan_jitter · u)` with u uniform in [−1, 1], one draw from the sim RNG.
    /// Returns the new tree's id.
    pub fn plant_tree(&mut self, x: usize, y: usize, age: u32) -> u32 {
        let id = self.alloc_id();
        let tp = &self.params.tree;
        let (mean, jitter) = (tp.max_age as f32, tp.lifespan_jitter);
        let u: f32 = self.rng.gen_range(-1.0..=1.0);
        let lifespan = (mean * (1.0 + jitter * u)).round().max(0.0) as u32;
        self.trunk_at[self.world.dims.cidx(x, y)] = self.trees.len() as u32;
        self.trees.push(Tree { id, x: x as u8, y: y as u8, age, dry_ticks: 0, lifespan, alive: true });
        self.refresh_canopy_columns(x as u8, y as u8);
        id
    }

    /// Plant `initial_count` trees on random soil columns, respecting `min_spacing`.
    pub fn place_initial_trees(&mut self) {
        let (n, age) = (self.params.tree.initial_count, self.params.tree.initial_age);
        let mut placed = 0;
        let mut attempts = 0;
        while placed < n && attempts < 10_000 {
            attempts += 1;
            let Some((x, y)) = self.random_soil_column() else { return };
            if self.spacing_ok(x as i32, y as i32) {
                self.plant_tree(x, y, age);
                placed += 1;
            }
        }
    }

    /// Kill tree `i` (`cause` is one of `events::TREE_CAUSES`): clear its trunk, add `death_detritus`
    /// to its patch and reopen the light under its crown.
    pub(crate) fn kill_tree(&mut self, i: usize, cause: &'static str) {
        let (x, y, col, id) = {
            let t = &mut self.trees[i];
            t.alive = false;
            (t.x, t.y, t.col(self.world.dims), t.id)
        };
        self.log_at(EventKind::TreeDeath, "tree", (x as usize, y as usize), cause, id);
        self.trunk_at[col] = NO_TREE;
        self.patches[self.world.dims.patch_of(x as usize, y as usize)].detritus += self.params.tree.death_detritus;
        self.refresh_canopy_columns(x, y);
    }

    /// Other live trees whose canopy lies over any column of tree `i`'s mature crown (the 3×3
    /// around its trunk): mature trees with trunks within Chebyshev 2, young trees within 1.
    pub fn crowding(&self, i: usize) -> usize {
        let (x, y) = (self.trees[i].x as i32, self.trees[i].y as i32);
        let d = self.world.dims;
        let mut n = 0;
        for dy in -2..=2i32 {
            for dx in -2..=2i32 {
                let (tx, ty) = (x + dx, y + dy);
                if (dx == 0 && dy == 0) || !d.in_bounds(tx, ty) {
                    continue;
                }
                let ti = self.trunk_at[d.cidx(tx as usize, ty as usize)];
                if ti == NO_TREE {
                    continue;
                }
                let reach = match self.tree_stage(&self.trees[ti as usize]) {
                    Stage::Sapling => continue,
                    Stage::Young => 1,
                    Stage::Mature => 2,
                };
                if dx.abs().max(dy.abs()) <= reach {
                    n += 1;
                }
            }
        }
        n
    }

    /// Germination probability on a soil column: f_L(surface light)·f_M(moisture)·f_T(patch temp).
    pub fn germination_prob(&self, x: usize, y: usize) -> f32 {
        let c = self.world.dims.cidx(x, y);
        let tp = &self.params.tree;
        suitability(&self.params.tree_light_curve(), self.world.surface_light(c) as f32)
            * suitability(&tp.moisture, self.moisture[c])
            * suitability(&tp.temp, self.patches[self.world.dims.patch_of(x, y)].temperature)
    }

    fn try_seed(&mut self, i: usize) {
        let (x, y) = (self.trees[i].x as f32, self.trees[i].y as f32);
        let r = self.params.tree.seed_radius * self.rng.gen::<f32>().sqrt();
        let theta = 2.0 * std::f32::consts::PI * self.rng.gen::<f32>();
        let tx = (x + r * libm::cosf(theta)).round() as i32;
        let ty = (y + r * libm::sinf(theta)).round() as i32;
        if !self.world.is_soil(tx, ty) || !self.spacing_ok(tx, ty) {
            return;
        }
        let p = self.germination_prob(tx as usize, ty as usize);
        if p > 0.0 && self.rng.gen::<f32>() < p {
            let id = self.plant_tree(tx as usize, ty as usize, 0);
            self.log_at(EventKind::Germination, "tree", (tx as usize, ty as usize), "", id);
        }
    }

    /// Tree update (on ticks where tick % update_every == 0). Trees planted this tick wait.
    pub fn update_trees(&mut self) {
        let tp = self.params.tree.clone();
        let n = self.trees.len();
        for i in 0..n {
            if !self.trees[i].alive {
                continue;
            }
            let before = self.tree_stage(&self.trees[i]);
            let c = self.trees[i].col(self.world.dims);
            self.trees[i].age += tp.update_every;
            self.draw_moisture(c, tp.moisture_draw);
            if self.moisture[c] < tp.dry_moisture {
                self.trees[i].dry_ticks += tp.update_every;
            } else {
                self.trees[i].dry_ticks = 0;
            }
            if self.trees[i].age >= self.trees[i].lifespan {
                self.kill_tree(i, "old_age");
                continue;
            }
            if self.trees[i].dry_ticks >= tp.dry_death_ticks {
                self.kill_tree(i, "drought");
                continue;
            }
            let after = self.tree_stage(&self.trees[i]);
            // Canopy self-thinning: one draw, only for a mature tree under ≥ 2 other canopies.
            if after == Stage::Mature
                && tp.crowding_mortality > 0.0
                && self.crowding(i) >= 2
                && self.rng.gen::<f32>() < tp.crowding_mortality
            {
                self.kill_tree(i, "crowded");
                continue;
            }
            if after != before {
                let (x, y) = (self.trees[i].x, self.trees[i].y);
                self.refresh_canopy_columns(x, y);
            }
            if after == Stage::Mature && self.trees[i].age.is_multiple_of(tp.seed_every) {
                self.try_seed(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::sq::*;
    use crate::world::tests::terrain;
    use crate::world::ColClass;
    use proptest::prelude::*;

    fn bare_sim() -> Sim {
        Sim::bare(&vec![14u8; COLS])
    }

    #[derive(Debug, Clone)]
    enum Op {
        Plant(usize, usize, u32),
        Kill(usize),
        Update,
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            3 => (0..WX, 0..WY, 0u32..1200).prop_map(|(x, y, age)| Op::Plant(x, y, age)),
            1 => any::<usize>().prop_map(Op::Kill),
            2 => Just(Op::Update),
        ]
    }

    /// Light and canopy cover kept incrementally (3×3 refreshes on plant, death and stage change,
    /// including seedlings from `update_trees`) equal a full recompute of every column.
    fn incremental_light_matches_full(heights: &[u8], soil_moisture: f32, ops: &[Op]) -> Result<(), TestCaseError> {
        let mut sim = Sim::bare(heights);
        // Heavy self-thinning so updates also kill crowded trees.
        sim.params.tree.crowding_mortality = 0.5;
        sim.moisture.iter_mut().zip(&sim.world.class).for_each(|(m, &k)| {
            if k == ColClass::Soil {
                *m = soil_moisture;
            }
        });
        for op in ops {
            match *op {
                Op::Plant(x, y, age) => {
                    if sim.world.is_soil(x as i32, y as i32) && sim.spacing_ok(x as i32, y as i32) {
                        sim.plant_tree(x, y, age);
                    }
                }
                Op::Kill(i) => {
                    let live: Vec<usize> = (0..sim.trees.len()).filter(|&i| sim.trees[i].alive).collect();
                    if !live.is_empty() {
                        sim.kill_tree(live[i % live.len()], "old_age");
                    }
                }
                Op::Update => sim.update_trees(),
            }
        }
        let (light, cover) = (sim.world.light.clone(), sim.canopy_cover.clone());
        let absorb = sim.params.world.canopy_absorb;
        for y in 0..WY {
            for x in 0..WX {
                let zs = sim.canopy_z(x as i32, y as i32);
                sim.canopy_cover[cidx(x, y)] = !zs.is_empty();
                sim.world.set_column_light(x, y, &zs, absorb);
            }
        }
        prop_assert!(light == sim.world.light, "incremental light differs from a full recompute");
        prop_assert_eq!(cover, sim.canopy_cover);
        Ok(())
    }

    /// Columns a tree's canopy lies over: the 3x3 around a mature trunk, the trunk column of a young one.
    fn canopy_columns(sim: &Sim, t: &Tree) -> Vec<(i32, i32)> {
        let (x, y) = (t.x as i32, t.y as i32);
        match sim.tree_stage(t) {
            Stage::Sapling => vec![],
            Stage::Young => vec![(x, y)],
            Stage::Mature => (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (x + dx, y + dy))).collect(),
        }
    }

    /// `crowding` equals a brute-force count: the other live trees whose canopy columns meet the
    /// 3x3 crown of tree `i`. Planted trees keep `lifespan` within `max_age · (1 ± lifespan_jitter)`.
    fn crowding_matches_brute_force(plants: &[(usize, usize, u32)], jitter: f32) -> Result<(), TestCaseError> {
        let mut sim = bare_sim();
        sim.params.tree.lifespan_jitter = jitter;
        for &(x, y, age) in plants {
            if sim.spacing_ok(x as i32, y as i32) {
                sim.plant_tree(x, y, age);
            }
        }
        let mean = sim.params.tree.max_age as f32;
        for (i, t) in sim.trees.iter().enumerate() {
            let (lo, hi) = ((mean * (1.0 - jitter)).floor() as u32, (mean * (1.0 + jitter)).ceil() as u32);
            prop_assert!((lo..=hi).contains(&t.lifespan), "lifespan {} outside [{}, {}]", t.lifespan, lo, hi);
            let crown: Vec<(i32, i32)> =
                (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (t.x as i32 + dx, t.y as i32 + dy))).collect();
            let want = sim
                .trees
                .iter()
                .enumerate()
                .filter(|&(j, o)| j != i && o.alive && canopy_columns(&sim, o).iter().any(|c| crown.contains(c)))
                .count();
            prop_assert_eq!(sim.crowding(i), want, "tree {} at ({}, {})", i, t.x, t.y);
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(32)))]

        #[test]
        fn prop_crowding_matches_brute_force(
            plants in prop::collection::vec((0..16usize, 0..16usize, 0u32..1500), 1..60),
            jitter in 0.0f32..=0.9,
        ) {
            crowding_matches_brute_force(&plants, jitter)?;
        }

        #[test]
        fn prop_incremental_light_matches_full(
            heights in terrain(),
            soil_moisture in 0.0f32..200.0,
            ops in prop::collection::vec(op(), 1..80),
        ) {
            incremental_light_matches_full(&heights, soil_moisture, &ops)?;
        }
    }

    #[test]
    fn light_regression_overlapping_canopies_then_one_dies() {
        let ops = [Op::Plant(10, 10, 950), Op::Plant(12, 10, 1000), Op::Update, Op::Kill(1), Op::Update];
        incremental_light_matches_full(&vec![14u8; COLS], 100.0, &ops).unwrap();
    }

    #[test]
    fn crowding_regression_row_of_mature_and_young_trees() {
        // Mature at x = 10, 12, 14 (spacing 2) and a young tree at 13: the middle mature tree has
        // three neighbours, the young one is under two canopies.
        let plants = [(10, 10, 2000), (12, 10, 2000), (14, 10, 2000), (13, 12, 600), (5, 5, 0)];
        crowding_matches_brute_force(&plants, 0.2).unwrap();
    }

    #[test]
    fn crowded_mature_tree_thins_and_lone_ones_survive() {
        let mut sim = bare_sim();
        sim.params.tree.crowding_mortality = 1.0;
        sim.moisture.fill(200.0);
        for x in [10, 12, 14] {
            sim.plant_tree(x, 10, 2000);
        }
        // Tree 0 has one neighbour; tree 1 two (dies); after that, tree 2 has none.
        sim.update_trees();
        let alive: Vec<bool> = sim.trees.iter().map(|t| t.alive).collect();
        assert_eq!(alive, [true, false, true]);
        assert_eq!(sim.patches[patch_of(12, 10)].detritus, sim.params.tree.death_detritus);
        assert_eq!(sim.world.surface_light(cidx(12, 10)), 255, "the gap over the dead trunk opens");
        // With mortality 0 nothing thins.
        let mut sim = bare_sim();
        sim.params.tree.crowding_mortality = 0.0;
        sim.moisture.fill(200.0);
        for x in [10, 12, 14] {
            sim.plant_tree(x, 10, 2000);
        }
        sim.update_trees();
        assert!(sim.trees.iter().all(|t| t.alive));
    }

    #[test]
    fn tree_dies_at_its_own_lifespan() {
        let mut sim = bare_sim();
        sim.moisture.fill(200.0);
        sim.plant_tree(20, 20, 0);
        sim.plant_tree(40, 40, 0);
        sim.trees[0].lifespan = 100;
        sim.trees[1].lifespan = 101;
        sim.update_trees(); // age 50
        sim.update_trees(); // age 100: tree 0 reaches its lifespan
        assert!(!sim.trees[0].alive && sim.trees[1].alive);
        sim.update_trees();
        assert!(!sim.trees[1].alive);
    }

    #[test]
    fn mature_canopy_shades_three_by_three() {
        let mut sim = bare_sim();
        sim.plant_tree(10, 10, 2000);
        for (x, y) in [(9, 9), (10, 10), (11, 11), (9, 11)] {
            assert_eq!(sim.world.surface_light(cidx(x, y)), 55, "column ({x},{y})");
        }
        assert_eq!(sim.world.surface_light(cidx(12, 10)), 255);
        assert!(!sim.spacing_ok(11, 11));
        assert!(sim.spacing_ok(12, 12));
        sim.kill_tree(0, "old_age");
        assert_eq!(sim.world.surface_light(cidx(10, 10)), 255);
        assert_eq!(sim.patches[patch_of(10, 10)].detritus, 40.0);
    }

    #[test]
    fn young_canopy_is_one_voxel() {
        let mut sim = bare_sim();
        sim.plant_tree(20, 20, 500);
        assert_eq!(sim.world.surface_light(cidx(20, 20)), 155);
        assert_eq!(sim.world.surface_light(cidx(21, 20)), 255);
        sim.plant_tree(30, 30, 0);
        assert_eq!(sim.world.surface_light(cidx(30, 30)), 255, "saplings cast no shade");
    }
}
