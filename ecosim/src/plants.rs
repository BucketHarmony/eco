//! Planting a world bundle's scene (shot G3): the scene's trees become tree entities and its
//! shrub ellipses raise the starting shrub density of the patches they cover.
//!
//! Only a bundle world goes through here. A noise world still places `tree.initial_count` trees on
//! random soil columns, and draws exactly what it always drew.

use crate::bundle::{Bundle, BundleShrub, BundleTree};
use crate::params::Params;
use crate::sim::{offsets_within, Sim};
use crate::world::World;

/// What planting a bundle's scene did, reported by `ecosim run --world` and by the tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlantImport {
    /// Trees in the scene.
    pub trees_in_scene: usize,
    /// Trees that became tree entities.
    pub trees_planted: usize,
    /// Planted trees whose trunk had to move off an unplantable column.
    pub trees_moved: usize,
    /// Trees dropped for want of a plantable column within `bundle.tree_move_radius`.
    pub trees_dropped: usize,
    /// Trees dropped because a taller tree took the same column.
    pub trees_merged: usize,
    /// Shrubs in the scene.
    pub shrubs_in_scene: usize,
    /// Plantable columns whose centre falls inside a shrub ellipse.
    pub shrub_columns: usize,
    /// Patches whose starting shrub density the scene raised.
    pub shrub_patches: usize,
}

impl PlantImport {
    /// One line for the run's stdout, as `ecosim run --world` prints it.
    pub fn summary(&self) -> String {
        format!(
            "scene: {} trees -> {} planted ({} moved, {} dropped, {} merged); {} shrubs over {} columns in {} patches",
            self.trees_in_scene,
            self.trees_planted,
            self.trees_moved,
            self.trees_dropped,
            self.trees_merged,
            self.shrubs_in_scene,
            self.shrub_columns,
            self.shrub_patches
        )
    }
}

/// Age in ticks a scene tree of `height` metres starts at.
///
/// Piecewise linear and monotone through (0 m, age 0), (`tree_mature_height`,
/// `tree.mature_age_years`) and (`tree_tall_height`, `tree_tall_age_years`), flat above the last: a
/// tree at least as tall as the sim's mature canopy always starts at `tree.mature_age_years` or
/// more, and the tallest tree in a scene still starts well short of `tree.max_age_years`, so an
/// imported wood does not die all at once. The tall age is read as at least the mature age, so no
/// parameter setting can make the map fall with height. Both come from
/// [`Params::tree_ages`](crate::params::Params::tree_ages), in ticks (shot G4c).
pub fn import_age(height: f32, p: &Params) -> u32 {
    let bp = &p.bundle;
    let (hm, ht) = (bp.tree_mature_height, bp.tree_tall_height);
    let ages = p.tree_ages();
    let (mature, tall) = (ages.mature as f32, ages.tall_import.max(ages.mature) as f32);
    let age = if height.is_nan() || height <= 0.0 {
        0.0
    } else if hm > 0.0 && height < hm {
        mature * height / hm
    } else if ht > hm && height < ht {
        mature + (tall - mature) * (height - hm) / (ht - hm)
    } else {
        tall
    };
    age.round().clamp(0.0, u32::MAX as f32) as u32
}

/// Whether the point (px, py), in metres from the world's south-west corner, lies inside the
/// shrub's ellipse. The ellipse has half-axes `rx` (east–west) and `ry` (north–south) before it is
/// turned by `angle` radians counter-clockwise.
pub fn in_shrub(s: &BundleShrub, px: f32, py: f32) -> bool {
    if !(s.rx > 0.0 && s.ry > 0.0) {
        return false;
    }
    let (dx, dy) = (px - s.x, py - s.y);
    let (c, sn) = (libm::cosf(s.angle), libm::sinf(s.angle));
    let u = (dx * c + dy * sn) / s.rx;
    let v = (dy * c - dx * sn) / s.ry;
    u * u + v * v <= 1.0
}

/// Where a scene tree's trunk goes: its own column when that is plantable, else the nearest
/// plantable column within `radius` (ties broken by the offset order, nearest first), else nothing.
/// The bool says the trunk moved.
fn trunk_column(world: &World, t: &BundleTree, offsets: &[(i32, i32, i32)]) -> Option<(usize, usize, bool)> {
    let d = world.dims;
    // A tree may stand exactly on the north or east edge (x = size_m), which floors out of the grid.
    let x0 = (t.x.floor() as i32).clamp(0, d.wx as i32 - 1);
    let y0 = (t.y.floor() as i32).clamp(0, d.wy as i32 - 1);
    offsets.iter().find_map(|&(dx, dy, _)| {
        let (x, y) = (x0 + dx, y0 + dy);
        (d.in_bounds(x, y) && world.is_plantable(d.cidx(x as usize, y as usize)))
            .then(|| (x as usize, y as usize, (dx, dy) != (0, 0)))
    })
}

impl Sim {
    /// Plant the bundle's scene into a tick-0 sim, in place of the noise world's random trees.
    ///
    /// Trees are resolved column by column first — moved, dropped or beaten by a taller tree — and
    /// then planted in ascending column order, so the entity ids follow the world and not the order
    /// the scene happened to list them in. Shrub ellipses then raise each patch's starting density
    /// by the fraction of its plantable columns they cover.
    pub(crate) fn import_scene(&mut self, b: &Bundle) -> PlantImport {
        let mut imp =
            PlantImport { trees_in_scene: b.trees.len(), shrubs_in_scene: b.shrubs.len(), ..Default::default() };
        let d = self.world.dims;
        let offsets = offsets_within(self.params.bundle.tree_move_radius);
        // Per column, the tallest scene tree that reached it: (height, scene index, moved).
        let mut winner: Vec<Option<(f32, usize, bool)>> = vec![None; d.cols()];
        for (i, t) in b.trees.iter().enumerate() {
            let Some((x, y, moved)) = trunk_column(&self.world, t, &offsets) else {
                imp.trees_dropped += 1;
                continue;
            };
            let c = d.cidx(x, y);
            match winner[c] {
                Some((h, _, _)) if h >= t.height => imp.trees_merged += 1,
                Some(_) => {
                    imp.trees_merged += 1;
                    winner[c] = Some((t.height, i, moved));
                }
                None => winner[c] = Some((t.height, i, moved)),
            }
        }
        for (c, w) in winner.iter().enumerate() {
            let Some((height, _, moved)) = *w else { continue };
            let (x, y) = d.xy(c);
            let age = import_age(height, &self.params);
            self.plant_tree(x, y, age);
            imp.trees_planted += 1;
            imp.trees_moved += moved as usize;
        }

        let mut covered = vec![false; d.cols()];
        for s in &b.shrubs {
            let r = s.rx.max(s.ry);
            let lo = |v: f32| (v - r).floor().max(0.0) as usize;
            let hi = |v: f32, n: usize| ((v + r).ceil().max(0.0) as usize).min(n);
            for y in lo(s.y)..hi(s.y, d.wy) {
                for x in lo(s.x)..hi(s.x, d.wx) {
                    let c = d.cidx(x, y);
                    if !covered[c] && self.world.is_plantable(c) && in_shrub(s, x as f32 + 0.5, y as f32 + 0.5) {
                        covered[c] = true;
                    }
                }
            }
        }
        for p in 0..d.patches() {
            let soil = &self.world.patch_soil[p];
            if soil.is_empty() {
                continue;
            }
            let n = soil.iter().filter(|&&c| covered[c]).count();
            if n > 0 {
                let share = n as f32 / soil.len() as f32;
                self.patches[p].shrub = (self.patches[p].shrub + share).clamp(0.0, 1.0);
                imp.shrub_columns += n;
                imp.shrub_patches += 1;
            }
        }
        imp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests::{build, bundle_params, flat_bundle, paint};
    use crate::bundle::Medium;
    use crate::trees::Stage;
    use crate::world::World;
    use proptest::prelude::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn tree(x: f32, y: f32, height: f32) -> BundleTree {
        BundleTree { x, y, height, crown_radius: height / 4.0, crown_base: height / 3.0 }
    }

    fn shrub(x: f32, y: f32, rx: f32, ry: f32, angle: f32) -> BundleShrub {
        BundleShrub { x, y, height: 1.5, rx, ry, angle }
    }

    /// A tick-0 sim on the bundle, with the crate's params and animals left out.
    fn sim_of(b: &Bundle) -> (Sim, PlantImport) {
        let mut p = bundle_params(b);
        p.animals.enabled = false;
        let world = World::from_bundle(b, &p).unwrap();
        Sim::with_bundle(p, ChaCha8Rng::seed_from_u64(42), world, b)
    }

    /// The planted trees as (x, y, stage), in column order.
    fn planted(s: &Sim) -> Vec<(u8, u8, Stage)> {
        s.trees.iter().filter(|t| t.alive).map(|t| (t.x, t.y, s.tree_stage(t))).collect()
    }

    #[test]
    fn one_fifteen_metre_tree_becomes_one_mature_tree_on_its_own_column() {
        let mut b = flat_bundle(16, 2);
        b.trees.push(tree(6.4, 9.8, 15.0));
        let (s, imp) = sim_of(&b);
        assert_eq!(planted(&s), vec![(6, 9, Stage::Mature)]);
        assert_eq!(imp, PlantImport { trees_in_scene: 1, trees_planted: 1, ..PlantImport::default() });
        // 15 m is between the mature height (3 m) and a tall tree (20 m), so the age is between too.
        let p = &s.params;
        let ages = p.tree_ages();
        assert_eq!(s.trees[0].age, import_age(15.0, p));
        assert!((ages.mature..ages.tall_import).contains(&s.trees[0].age));
    }

    #[test]
    fn a_tree_on_a_roof_moves_to_the_nearest_plantable_column() {
        let mut b = flat_bundle(16, 2);
        build(&mut b, 5, 5, 6.0);
        b.trees.push(tree(5.5, 5.5, 12.0));
        let (s, imp) = sim_of(&b);
        // The 3x3 roof neighbours are all lawn; the nearest-first offsets take (5, 4) (dy = -1).
        assert_eq!(planted(&s), vec![(5, 4, Stage::Mature)]);
        assert_eq!((imp.trees_planted, imp.trees_moved, imp.trees_dropped), (1, 1, 0));
    }

    #[test]
    fn a_tree_deep_in_a_roof_is_dropped() {
        let mut b = flat_bundle(16, 2);
        for y in 3..10 {
            for x in 3..10 {
                build(&mut b, x, y, 6.0);
            }
        }
        b.trees.push(tree(6.5, 6.5, 12.0));
        let (s, imp) = sim_of(&b);
        assert_eq!(planted(&s), vec![]);
        assert_eq!((imp.trees_planted, imp.trees_moved, imp.trees_dropped), (0, 0, 1));
    }

    #[test]
    fn two_trees_in_one_column_keep_the_taller() {
        for (a, c) in [(6.0f32, 18.0f32), (18.0, 6.0)] {
            let mut b = flat_bundle(16, 2);
            b.trees.push(tree(7.2, 7.2, a));
            b.trees.push(tree(7.9, 7.6, c));
            let (s, imp) = sim_of(&b);
            assert_eq!(planted(&s), vec![(7, 7, Stage::Mature)], "one trunk on the column");
            assert_eq!((imp.trees_planted, imp.trees_merged), (1, 1));
            assert_eq!(s.trees[0].age, import_age(a.max(c), &s.params), "the taller tree's age");
        }
    }

    #[test]
    fn a_shrub_over_half_a_patch_raises_its_density_by_half() {
        let mut b = flat_bundle(16, 2);
        // Patch (0, 0) is columns 0..8 x 0..8; this ellipse holds exactly 32 of their centres,
        // and the acceptance is measured within one column's share (1/64) of half.
        b.shrubs.push(shrub(4.0, 4.0, 2.6, 3.8, 0.0));
        let (s, imp) = sim_of(&b);
        let p0 = s.world.dims.patch_of(0, 0);
        let base = s.params.shrub.initial;
        let share = imp.shrub_columns as f32 / 64.0;
        assert!((share - 0.5).abs() <= 1.0 / 64.0, "the ellipse covers {} of 64 columns", imp.shrub_columns);
        assert!((s.patches[p0].shrub - (base + 0.5)).abs() <= 1.0 / 64.0, "shrub {}", s.patches[p0].shrub);
        assert_eq!(imp.shrub_patches, 1, "no other patch is touched");
        for p in 0..s.world.dims.patches() {
            if p != p0 {
                assert_eq!(s.patches[p].shrub, base);
            }
        }
    }

    /// A rotated ellipse covers the same columns as the same ellipse turned by a right angle with
    /// its axes swapped, and sealed columns never count.
    #[test]
    fn shrub_cover_turns_with_the_ellipse_and_skips_sealed_columns() {
        let cover = |angle: f32, rx: f32, ry: f32, seal: bool| {
            let mut b = flat_bundle(16, 2);
            if seal {
                paint(&mut b, 8, 8, Medium::Asphalt);
            }
            b.shrubs.push(shrub(8.0, 8.0, rx, ry, angle));
            sim_of(&b).1.shrub_columns
        };
        let n = cover(0.0, 3.5, 1.2, false);
        assert!(n > 4);
        assert_eq!(cover(std::f32::consts::FRAC_PI_2, 1.2, 3.5, false), n, "a right angle swaps the axes");
        assert_eq!(cover(0.0, 3.5, 1.2, true), n - 1, "the asphalt column is not plantable");
    }

    /// Pipes are carried into the world for the drain network in shot G6; a noise world has none.
    #[test]
    fn pipes_reach_the_world() {
        let mut b = flat_bundle(16, 2);
        b.pipes.push(crate::bundle::Pipe {
            id: "drain-1".into(),
            inlet: [4.0, 4.0],
            outlet: [16.0, 4.0],
            capacity_m3h: 12.0,
            illustrative: true,
        });
        let (s, _) = sim_of(&b);
        assert_eq!(s.world.pipes.len(), 1);
        assert_eq!(s.world.pipes[0].id, "drain-1");
        assert!(Sim::bare(&vec![12u8; crate::world::sq::COLS]).world.pipes.is_empty());
    }

    /// Regression sibling of `prop_import_age_is_monotone`: the mapping's corners at the defaults.
    #[test]
    fn import_age_hits_the_growth_curve_at_its_corners() {
        let p = crate::Params::load_default();
        let a = p.tree_ages();
        assert_eq!(import_age(0.0, &p), 0);
        assert_eq!(import_age(-1.0, &p), 0, "a tree of no height is a seed, not a negative age");
        assert_eq!(import_age(1.5, &p), a.young, "half the mature height is exactly young");
        assert_eq!(import_age(3.0, &p), a.mature);
        assert_eq!(import_age(20.0, &p), a.tall_import);
        assert_eq!(import_age(80.0, &p), a.tall_import, "taller than tall is still tall");
        assert!(a.tall_import < a.max, "even the tallest scene tree starts short of its lifespan");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(64)))]

        /// The height-to-age map never falls with height, and never starts a tree below
        /// `tree.mature_age_years` once it is as tall as a mature canopy — whatever the parameters
        /// are.
        #[test]
        fn prop_import_age_is_monotone(
            heights in prop::collection::vec(-1.0f32..60.0, 2),
            mature_h in -1.0f32..30.0,
            tall_h in -1.0f32..60.0,
            tall_age in 0.0f32..2.25,
            mature_age in 0.0f32..1.0,
        ) {
            let mut p = crate::Params::load_default();
            p.tree.mature_age_years = mature_age;
            p.bundle.tree_mature_height = mature_h;
            p.bundle.tree_tall_height = tall_h;
            p.bundle.tree_tall_age_years = tall_age;
            let mature_ticks = p.tree_ages().mature;
            let (lo, hi) = (heights[0].min(heights[1]), heights[0].max(heights[1]));
            let (a, b) = (import_age(lo, &p), import_age(hi, &p));
            prop_assert!(a <= b, "age({lo}) = {a} > age({hi}) = {b}");
            if hi >= mature_h && hi > 0.0 {
                prop_assert!(b >= mature_ticks, "a mature-height tree starts at {b}, under {mature_ticks}");
            }
        }
    }
}
