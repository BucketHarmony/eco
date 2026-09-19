//! Tree entities: stages, canopy geometry and light, seeding and death.

use crate::producers::suitability;
use crate::sim::{Sim, NO_TREE};
use crate::world::{cidx, in_bounds, patch_of};
use rand::Rng;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Sapling,
    Young,
    Mature,
}

#[derive(Debug, Clone)]
pub struct Tree {
    pub id: u32,
    pub x: u8,
    pub y: u8,
    pub age: u32,
    pub dry_ticks: u32,
    pub alive: bool,
}

impl Tree {
    #[inline]
    pub fn col(&self) -> usize {
        cidx(self.x as usize, self.y as usize)
    }
}

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
    pub fn tree_stage(&self, t: &Tree) -> Stage {
        stage_of(t.age, self.params.tree.young_age, self.params.tree.mature_age)
    }

    /// True if no live trunk is closer than `min_spacing` (Chebyshev) to (x, y).
    pub fn spacing_ok(&self, x: i32, y: i32) -> bool {
        let r = self.params.tree.min_spacing - 1;
        for dy in -r..=r {
            for dx in -r..=r {
                let (nx, ny) = (x + dx, y + dy);
                if in_bounds(nx, ny) && self.trunk_at[cidx(nx as usize, ny as usize)] != NO_TREE {
                    return false;
                }
            }
        }
        true
    }

    /// Distinct canopy voxel z values over column (x, y), from live young and mature trees.
    pub fn canopy_z(&self, x: i32, y: i32) -> Vec<u8> {
        let mut zs: Vec<u8> = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (tx, ty) = (x + dx, y + dy);
                if !in_bounds(tx, ty) {
                    continue;
                }
                let ti = self.trunk_at[cidx(tx as usize, ty as usize)];
                if ti == NO_TREE {
                    continue;
                }
                let t = &self.trees[ti as usize];
                let h = self.world.height[t.col()];
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
        let absorb = self.params.world.canopy_absorb;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (x as i32 + dx, y as i32 + dy);
                if !in_bounds(cx, cy) {
                    continue;
                }
                let zs = self.canopy_z(cx, cy);
                self.canopy_cover[cidx(cx as usize, cy as usize)] = !zs.is_empty();
                self.world.set_column_light(cx as usize, cy as usize, &zs, absorb);
            }
        }
    }

    pub fn plant_tree(&mut self, x: usize, y: usize, age: u32) {
        let id = self.alloc_id();
        self.trunk_at[cidx(x, y)] = self.trees.len() as u32;
        self.trees.push(Tree { id, x: x as u8, y: y as u8, age, dry_ticks: 0, alive: true });
        self.refresh_canopy_columns(x as u8, y as u8);
    }

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

    fn kill_tree(&mut self, i: usize) {
        let (x, y, col) = {
            let t = &mut self.trees[i];
            t.alive = false;
            (t.x, t.y, t.col())
        };
        self.trunk_at[col] = NO_TREE;
        self.patches[patch_of(x as usize, y as usize)].detritus += self.params.tree.death_detritus;
        self.refresh_canopy_columns(x, y);
    }

    /// Germination probability on a soil column: f_L(surface light)·f_M(moisture)·f_T(patch temp).
    pub fn germination_prob(&self, x: usize, y: usize) -> f32 {
        let c = cidx(x, y);
        let tp = &self.params.tree;
        suitability(&self.params.tree_light_curve(), self.world.surface_light(c) as f32)
            * suitability(&tp.moisture, self.moisture[c])
            * suitability(&tp.temp, self.patches[patch_of(x, y)].temperature)
    }

    fn try_seed(&mut self, i: usize) {
        let (x, y) = (self.trees[i].x as f32, self.trees[i].y as f32);
        let r = self.params.tree.seed_radius * self.rng.gen::<f32>().sqrt();
        let theta = 2.0 * std::f32::consts::PI * self.rng.gen::<f32>();
        let tx = (x + r * theta.cos()).round() as i32;
        let ty = (y + r * theta.sin()).round() as i32;
        if !self.world.is_soil(tx, ty) || !self.spacing_ok(tx, ty) {
            return;
        }
        let p = self.germination_prob(tx as usize, ty as usize);
        if p > 0.0 && self.rng.gen::<f32>() < p {
            self.plant_tree(tx as usize, ty as usize, 0);
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
            let c = self.trees[i].col();
            self.trees[i].age += tp.update_every;
            self.moisture[c] = (self.moisture[c] - tp.moisture_draw).max(0.0);
            if self.moisture[c] < tp.dry_moisture {
                self.trees[i].dry_ticks += tp.update_every;
            } else {
                self.trees[i].dry_ticks = 0;
            }
            if self.trees[i].age >= tp.max_age || self.trees[i].dry_ticks >= tp.dry_death_ticks {
                self.kill_tree(i);
                continue;
            }
            let after = self.tree_stage(&self.trees[i]);
            if after != before {
                let (x, y) = (self.trees[i].x, self.trees[i].y);
                self.refresh_canopy_columns(x, y);
            }
            if after == Stage::Mature && self.trees[i].age % tp.seed_every == 0 {
                self.try_seed(i);
            }
        }
    }

    pub fn count_mature_trees(&self) -> usize {
        self.trees.iter().filter(|t| t.alive && self.tree_stage(t) == Stage::Mature).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;
    use crate::world::{World, COLS};
    use rand::SeedableRng;

    fn bare_sim() -> Sim {
        let mut p = Params::load_default();
        p.tree.initial_count = 0;
        p.grazer.start_count = 0;
        p.hunter.start_count = 0;
        let world = World::from_heights(&vec![14u8; COLS], &p);
        Sim::with_world(p, rand_chacha::ChaCha8Rng::seed_from_u64(3), world)
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
        sim.kill_tree(0);
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
