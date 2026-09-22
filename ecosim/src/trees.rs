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
    /// Consecutive ticks spent below `tree.dry_fraction` of available water capacity.
    pub dry_ticks: u32,
    /// Age at which it dies, drawn at planting.
    pub lifespan: u32,
    /// False once dead; removed at the next compaction.
    pub alive: bool,
    /// Nitrogen, phosphorus and potassium the tree has taken up, in grams (shot G5).
    ///
    /// A tree is the one plant whose nutrient content is stored rather than derived: the ground
    /// covers have a density the patch already holds, but a tree's size is its age, and what it
    /// managed to take up out of the soil under it is not a function of its age -- a tree on poor
    /// ground is the same height as one on good ground and holds less. All of it goes to the
    /// patch's detritus when it dies.
    pub npk: [f64; 3],
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
        let a = self.params.tree_ages();
        stage_of(t.age, a.young, a.mature)
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
        let (extinction, d) = (self.params.canopy_extinction(), self.world.dims);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (x as i32 + dx, y as i32 + dy);
                if !d.in_bounds(cx, cy) {
                    continue;
                }
                let zs = self.canopy_z(cx, cy);
                self.canopy_cover[d.cidx(cx as usize, cy as usize)] = !zs.is_empty();
                self.world.set_column_light(cx as usize, cy as usize, &zs, extinction);
            }
        }
    }

    /// Plant a tree on (x, y) and refresh the light of the columns its canopy covers. Its lifespan is
    /// `max_age_years · (1 + lifespan_jitter · u)` with u uniform in [−1, 1], one draw from the sim RNG.
    /// Returns the new tree's id.
    pub fn plant_tree(&mut self, x: usize, y: usize, age: u32) -> u32 {
        let id = self.alloc_id();
        let tp = &self.params.tree;
        let (mean, jitter) = (self.params.tree_ages().max as f32, tp.lifespan_jitter);
        let u: f32 = self.rng.gen_range(-1.0..=1.0);
        let lifespan = (mean * (1.0 + jitter * u)).round().max(0.0) as u32;
        self.trunk_at[self.world.dims.cidx(x, y)] = self.trees.len() as u32;
        self.trees.push(Tree { id, x: x as u8, y: y as u8, age, dry_ticks: 0, lifespan, alive: true, npk: [0.0; 3] });
        self.refresh_canopy_columns(x as u8, y as u8);
        id
    }

    /// Plant `initial_count` trees on random soil columns, respecting `min_spacing`.
    pub fn place_initial_trees(&mut self) {
        let (n, age) = (self.params.tree.initial_count, self.params.tree_ages().initial);
        let mut placed = 0;
        let mut attempts = 0;
        while placed < n && attempts < 10_000 {
            attempts += 1;
            let Some((x, y)) = self.random_soil_column() else { return };
            if self.world.can_root_a_trunk(self.world.dims.cidx(x, y)) && self.spacing_ok(x as i32, y as i32) {
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
        let p = self.world.dims.patch_of(x as usize, y as usize);
        self.patches[p].detritus += self.params.tree.death_detritus;
        // Everything the tree took up goes back to the patch it stood in, as dead wood.
        let held = std::mem::take(&mut self.trees[i].npk);
        self.npk_to_detritus(p, held);
        self.refresh_canopy_columns(x, y);
    }

    /// How much of tree `i`'s crown footprint other live crowns cover, in [0, 1] (shot S11).
    ///
    /// **This is the quantity that decides tree-on-tree competition, and it replaced a count.**
    /// Until S11 the rule counted other *trunks* in a 5 × 5 m window — mature ones within Chebyshev
    /// 2, young ones within 1 — which is the footprint the canopy *voxels* shade. Shot S3 measured
    /// what the trees actually carry: a median mature crown radius of 3.19 m, a maximum of 6.00 m,
    /// and a reference run whose crowns summed to 266% of the site while that window saw almost
    /// nothing. A rule at 5 m cannot see a canopy three deep, so the trees competed at one scale
    /// and shaded each other at another.
    ///
    /// Each neighbour covers [`overlap_fraction`] of this crown's disc, and the open share is the
    /// product of what each one leaves: `1 − Π(1 − f_j)`. That is the same mean-field independence
    /// [`Sim::crown_light`](crate::sim::Sim::crown_light) assumes, for the same reason and with the
    /// same error — exact for one
    /// neighbour and for neighbours that do not overlap each other, and too crowded when several
    /// cover the same side. Stage does not appear: a neighbour's weight is the crown its age gives
    /// it, so a sapling (radius 0, from a height of 0) contributes exactly nothing without being
    /// named as a special case, and the dead `Stage::Young => 1` arm of the old rule — unreachable
    /// in any run, because `min_spacing = 2` forbids a trunk at Chebyshev 1 — is gone rather than
    /// repaired.
    ///
    /// It draws nothing from the RNG and writes no field.
    ///
    /// `crowns` is the [`Sim::crowns`] of the stand as it stood when the update pass began, shared
    /// by every tree the pass judges, so all of them are measured against one instant. A tree the
    /// pass has planted since is not in it and is skipped — which is exact rather than approximate,
    /// because `Sim::try_seed` plants at age 0, a tree of age 0 is 0 m tall and a crown of radius
    /// 0 contributes nothing to any overlap. A tree the pass has **killed** is not skipped by
    /// `crowns` but by `trunk_at`, which `Sim::kill_tree` clears: the gap opens immediately, in
    /// the same pass, exactly as it does for light.
    pub fn crown_crowding(&self, i: usize, crowns: &Crowns) -> f32 {
        let me = &crowns.of[i];
        let d = self.world.dims;
        let (tx, ty) = (me.x.floor() as i32, me.y.floor() as i32);
        // Nothing whose trunk is outside this box can reach the footprint.
        let reach = (me.radius + crowns.max_radius).ceil().max(0.0) as i32;
        let mut open = 1.0f32;
        for ny in ty - reach..=ty + reach {
            for nx in tx - reach..=tx + reach {
                if !d.in_bounds(nx, ny) {
                    continue;
                }
                let j = self.trunk_at[d.cidx(nx as usize, ny as usize)] as usize;
                if j == NO_TREE as usize || j == i || j >= crowns.of.len() {
                    continue;
                }
                let f = overlap_fraction(me, &crowns.of[j]);
                if f > 0.0 {
                    open *= 1.0 - f;
                }
            }
        }
        (1.0 - open).clamp(0.0, 1.0)
    }

    /// The share of a full canopy a tree in patch `p` carries (shot G10): 0 at and below
    /// `tree.leaf_off_temp`, 1 at and above `tree.leaf_on_temp`, linear between, on the patch
    /// temperature. A step at `leaf_on_temp` if the ramp has no width.
    pub fn leaf_on(&self, p: usize) -> f32 {
        let tp = &self.params.tree;
        leaf_fraction(self.patches[p].temperature, tp.leaf_off_temp, tp.leaf_on_temp)
    }

    /// The mean over one model year of [`leaf_draw`] at the season's temperature (shot G10),
    /// sampled at every temperature update, as the patches see it. `transpiration_mm_h` is an
    /// annual figure (UNITS.md R8, 300 mm a year), so a tree's draw is divided by this and a
    /// deciduous tree transpires the same year's water as an evergreen, all of it while in leaf.
    /// It reads the open-ground season, not a patch's: a tree whose own canopy cools its patch
    /// spends a little longer bare and so draws a little less than the year's figure. 0 when the
    /// tree never carries a leaf at all.
    pub fn leaf_draw_mean(&self) -> f64 {
        let (cl, tp) = (&self.params.climate, &self.params.tree);
        let (year, step) = (cl.year_len.max(1), self.params.schedule.temperature_every.max(1));
        let n = year.div_ceil(step);
        let sum: f64 = (0..n)
            .map(|k| {
                let t = cl.temp_base
                    + self.params.season.amplitude
                        * libm::sinf(2.0 * std::f32::consts::PI * (k * step) as f32 / year as f32);
                leaf_draw(tp.deciduous, leaf_fraction(t, tp.leaf_off_temp, tp.leaf_on_temp)) as f64
            })
            .sum();
        sum / n as f64
    }

    /// Germination probability on a soil column: f_L(surface light)·f_M(moisture)·f_T(patch temp).
    pub fn germination_prob(&self, x: usize, y: usize) -> f32 {
        let c = self.world.dims.cidx(x, y);
        let tp = &self.params.tree;
        suitability(&self.params.tree_light_curve(), self.world.surface_light_fraction(c))
            * suitability(&tp.moisture, self.water_fraction(c))
            * suitability(&tp.temp, self.patches[self.world.dims.patch_of(x, y)].temperature)
    }

    fn try_seed(&mut self, i: usize) {
        let (x, y) = (self.trees[i].x as f32, self.trees[i].y as f32);
        let r = self.params.tree.seed_radius * self.rng.gen::<f32>().sqrt();
        let theta = 2.0 * std::f32::consts::PI * self.rng.gen::<f32>();
        let tx = (x + r * libm::cosf(theta)).round() as i32;
        let ty = (y + r * libm::sinf(theta)).round() as i32;
        if !self.world.trunk_site_ok(tx, ty) || !self.spacing_ok(tx, ty) {
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
        let ages = self.params.tree_ages();
        // The stand as the pass found it: one crown per tree, shared by every judgement below.
        // `crowding_mortality = 0` is the one setting that reads no crown at all, and it must not
        // pay for one either — nor may it touch the RNG (the rate-0 identity, tests/sweep.rs).
        let crowns = (tp.crowding_mortality > 0.0).then(|| self.crowns());
        // Leaf-off moves a year's transpiration into the leafy part of it rather than removing
        // any (shot G10), so the draw is divided by its own mean over the model's year.
        let norm = (tp.deciduous > 0.0).then(|| self.leaf_draw_mean());
        let n = self.trees.len();
        for i in 0..n {
            if !self.trees[i].alive {
                continue;
            }
            let before = self.tree_stage(&self.trees[i]);
            let c = self.trees[i].col(self.world.dims);
            let h0 = crate::plants::height_of_age(self.trees[i].age, &self.params);
            self.trees[i].age += tp.update_every;
            if self.npk.is_some() {
                // A tree buys its height from the soil under its own trunk, `npk.need_*` grams a
                // metre. It is not cut back when the soil is short -- its height is its age, and
                // nothing here can un-grow it -- so a starved tree is simply a poorer one, and
                // returns less when it dies.
                let grew = (crate::plants::height_of_age(self.trees[i].age, &self.params) - h0).max(0.0) as f64;
                let needs = tp.npk.needs();
                let got = self.npk_take_column(c, [0, 1, 2].map(|i| grew * needs[i] as f64));
                for (v, g) in self.trees[i].npk.iter_mut().zip(got) {
                    *v += g;
                }
            }
            // Transpiration over the update's length, in mm (shot G4b), less what a bare canopy
            // does not draw (shot G10). The rate check sits outside, so an evergreen reads no
            // temperature and draws exactly what it drew before the leaves could fall.
            let hours = tp.update_every as f64 * crate::hydro::tick_hours(&self.params);
            let mut mm = tp.transpiration_mm_h as f64 * hours;
            if let Some(norm) = norm {
                let leaf = self.leaf_on(self.world.dims.patch_of(self.trees[i].x as usize, self.trees[i].y as usize));
                mm = if norm > 0.0 { mm * leaf_draw(tp.deciduous, leaf) as f64 / norm } else { 0.0 };
            }
            self.draw_water_mm(c, mm as f32);
            if self.water_fraction(c) < tp.dry_fraction {
                self.trees[i].dry_ticks += tp.update_every;
            } else {
                self.trees[i].dry_ticks = 0;
            }
            if self.trees[i].age >= self.trees[i].lifespan {
                self.kill_tree(i, "old_age");
                continue;
            }
            if self.trees[i].dry_ticks >= ages.dry_death {
                self.kill_tree(i, "drought");
                continue;
            }
            // Drowned roots (shot G5). The rate check and the waterlogging check both sit outside
            // the draw, so with the nutrient tier off -- where no column is ever waterlogged --
            // this reads nothing and leaves the random stream exactly where it was.
            if tp.waterlog_mortality > 0.0
                && self.is_waterlogged(c)
                && self.rng.gen::<f32>() < tp.waterlog_mortality * (1.0 - tp.npk.waterlog_tolerance.clamp(0.0, 1.0))
            {
                self.kill_tree(i, "waterlog");
                continue;
            }
            let after = self.tree_stage(&self.trees[i]);
            // Canopy self-thinning: one draw, only for a mature tree whose crown is at least
            // `crowding_overlap` covered by its neighbours' crowns (shot S11). The rate check stays
            // outside the measurement and outside the draw, so at `crowding_mortality = 0` this
            // makes no draw, reads no crown and leaves the random stream exactly where it was.
            if after == Stage::Mature
                && tp.crowding_mortality > 0.0
                && self.crown_crowding(i, crowns.as_ref().expect("built when the rate is above 0"))
                    >= tp.crowding_overlap
                && self.rng.gen::<f32>() < tp.crowding_mortality
            {
                self.kill_tree(i, "crowded");
                continue;
            }
            if after != before {
                let (x, y) = (self.trees[i].x, self.trees[i].y);
                self.refresh_canopy_columns(x, y);
            }
            // Seeds once per `seed_every` ticks: the update whose age crosses a multiple of it. The
            // old test was `age.is_multiple_of(seed_every)`, which is the same thing whenever
            // `update_every` divides `seed_every` — it does at the shipped 50 and 200 — and silently
            // seeds never when it does not. `seeds_per_year` can now be set to a value that makes it
            // not (shot G4c; FINDINGS.md). A rate faster than one update seeds once per update.
            if after == Stage::Mature && self.trees[i].age % ages.seed_every < tp.update_every {
                self.try_seed(i);
            }
        }
    }
}

/// The crown the allometry gives a tree, in metres and in world coordinates (shot S3).
///
/// The simulator's canopy **voxels** are a 3x3 m stamp two voxels deep, which is the shade a tree
/// casts on the 1 m ecology grid. They are not the crown a tree *has*: run the age through
/// [`height_of_age`](crate::plants::height_of_age) and the surveyed fractions
/// (`tree.crown_radius_frac`, `tree.crown_base_frac`) and a mature tree's crown is 6-12 m across.
/// That gap is why `light.bin` carries no per-tree information -- every mature tree reads the same
/// `exp(-2k)` at its own trunk, because the only canopy over that column is its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crown {
    /// Trunk centre, in ecology-grid metres (the column's middle).
    pub x: f32,
    /// Trunk centre, in ecology-grid metres.
    pub y: f32,
    /// Absolute z of the ground the trunk stands on: the first air voxel of its column, so that a
    /// height in metres above the tree's own ground and a `shade_top` are the same quantity.
    pub ground: f32,
    /// Height of the tree in metres.
    pub height: f32,
    /// Height of the lowest branch in metres.
    pub base: f32,
    /// Crown radius in metres.
    pub radius: f32,
}

impl Crown {
    /// Absolute z of the crown's middle, where [`Sim::crown_light`] measures its light.
    #[inline]
    pub fn mid(&self) -> f32 {
        self.ground + 0.5 * (self.base + self.height)
    }

    /// Crown depth in metres, never negative.
    #[inline]
    pub fn depth(&self) -> f32 {
        (self.height - self.base).max(0.0)
    }

    /// Metres of this crown a vertical ray passes through on its way down to absolute height `z`:
    /// 0 for a ray that stops above the crown, and the whole depth for one that stops below it.
    #[inline]
    pub fn depth_above(&self, z: f32) -> f32 {
        let (top, bottom) = (self.ground + self.height, self.ground + self.base);
        (top - z.max(bottom)).clamp(0.0, self.depth())
    }
}

/// Every tree's crown, by `Sim::trees` index, with the reach a neighbour search needs.
pub struct Crowns {
    /// One per entry of `Sim::trees`; a dead tree's is never read, because `trunk_at` has dropped it.
    pub of: Vec<Crown>,
    /// The largest live crown radius, which bounds how far a neighbour can reach.
    pub max_radius: f32,
}

/// Area of the intersection of two discs over the area of the first: how much of crown `a`'s
/// footprint crown `b` covers. An `a` of zero radius is a point, covered or not.
pub fn overlap_fraction(a: &Crown, b: &Crown) -> f32 {
    let (ra, rb) = (a.radius.max(0.0), b.radius.max(0.0));
    let d = libm::sqrtf((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y));
    if ra <= 0.0 {
        return f32::from(rb > 0.0 && d <= rb);
    }
    if d >= ra + rb {
        return 0.0;
    }
    if d <= (ra - rb).abs() {
        // One disc contains the other: either all of a, or all of b sitting inside it.
        return if rb >= ra { 1.0 } else { (rb * rb) / (ra * ra) };
    }
    let cos = |v: f32| libm::acosf(v.clamp(-1.0, 1.0));
    let lens = ra * ra * cos((d * d + ra * ra - rb * rb) / (2.0 * d * ra))
        + rb * rb * cos((d * d + rb * rb - ra * ra) / (2.0 * d * rb))
        - 0.5 * libm::sqrtf((((ra + rb) * (ra + rb) - d * d) * (d * d - (ra - rb) * (ra - rb))).max(0.0));
    (lens / (std::f32::consts::PI * ra * ra)).clamp(0.0, 1.0)
}

impl Sim {
    /// The crown of one tree: its age through the simulator's own height curve, and the surveyed
    /// fractions around that height.
    pub fn crown_of(&self, t: &Tree) -> Crown {
        let height = crate::plants::height_of_age(t.age, &self.params).max(0.0);
        let tp = &self.params.tree;
        Crown {
            x: f32::from(t.x) + 0.5,
            y: f32::from(t.y) + 0.5,
            ground: f32::from(self.world.height[t.col(self.world.dims)]) + 1.0,
            height,
            base: tp.crown_base_frac.clamp(0.0, 1.0) * height,
            radius: tp.crown_radius_frac.max(0.0) * height,
        }
    }

    /// Every tree's crown, built once so a whole snapshot's worth of [`Sim::crown_light`] shares it.
    pub fn crowns(&self) -> Crowns {
        let of: Vec<Crown> = self.trees.iter().map(|t| self.crown_of(t)).collect();
        let max_radius = self.trees.iter().zip(&of).filter(|(t, _)| t.alive).fold(0.0f32, |m, (_, c)| m.max(c.radius));
        Crowns { of, max_radius }
    }

    /// The light tree `i`'s crown receives, as a fraction of full sun (shot S3).
    ///
    /// **Why this is not `light.bin`.** The light field is computed for canopy voxels, and a tree's
    /// canopy covers its own 3x3 columns and no more, so the field at a trunk is `exp(-2k)` for
    /// every mature tree in the world and `exp(-k)` for every young one -- measured on
    /// `runs/capitol-s42` at tick 20000, exactly one distinct value per stage across 4,082 trees.
    /// The number below is read at the **middle of the crown the allometry gives the tree**, which
    /// is 6-12 m across on a mature tree, so who stands near it changes it.
    ///
    /// Two things darken it, and they multiply:
    ///
    /// 1. **Neighbouring crowns.** For every other live tree whose crown overlaps this one's
    ///    footprint, the ray down to this crown's middle passes through [`Crown::depth_above`]
    ///    metres of it. A whole crown has the optical depth of the voxel model's mature canopy,
    ///    `2 x canopy_k x canopy_lai`, spread evenly down its depth, so a whole crown still passes
    ///    the 13.5% of full sun `params.toml` calibrates: this is the same optics at the crown's own
    ///    scale, not a second set of constants. Each neighbour covers [`overlap_fraction`] of the
    ///    footprint and the factors multiply, which assumes the overlaps fall independently over the
    ///    crown. That is a mean-field approximation and the only modelling liberty here -- it is
    ///    exact for one neighbour and for neighbours that do not overlap each other, and it errs
    ///    towards too dark when several cover the same side.
    /// 2. **Buildings.** A crown standing in a roof's shadow gets nothing where the shadow reaches
    ///    its middle, which is what `set_column_light` does to a voxel below `shade_top`. This term
    ///    is the share of the footprint's columns lit at that height, and it is exactly 1 in a noise
    ///    world, which has no buildings.
    ///
    /// It draws nothing from the RNG and writes no field: it is read at snapshot time and published,
    /// and the ecology is untouched by it (DECISIONS.md, shot S3).
    pub fn crown_light(&self, i: usize, crowns: &Crowns) -> f32 {
        let me = &crowns.of[i];
        let d = self.world.dims;
        let mid = me.mid();
        let tau = 2.0 * self.params.canopy_extinction();
        let (tx, ty) = (me.x.floor() as i32, me.y.floor() as i32);
        let mut light = 1.0f32;
        // 1. Neighbouring crowns. Nothing whose trunk is outside this box can reach the footprint.
        let reach = (me.radius + crowns.max_radius).ceil().max(0.0) as i32;
        for ny in ty - reach..=ty + reach {
            for nx in tx - reach..=tx + reach {
                if !d.in_bounds(nx, ny) {
                    continue;
                }
                let j = self.trunk_at[d.cidx(nx as usize, ny as usize)];
                if j == NO_TREE || j as usize == i {
                    continue;
                }
                let other = &crowns.of[j as usize];
                let depth = other.depth();
                let f = if depth > 0.0 { overlap_fraction(me, other) } else { 0.0 };
                if f <= 0.0 {
                    continue;
                }
                light *= 1.0 - f * (1.0 - libm::expf(-tau * other.depth_above(mid) / depth));
            }
        }
        // 2. Buildings, which only a bundle world has. The trunk's own column always counts, so a
        // crown of no radius is still asked whether it stands in a shadow.
        let (mut cols, mut lit) = (0u32, 0u32);
        let r = me.radius.max(0.0);
        let span = r.ceil() as i32;
        for cy in ty - span..=ty + span {
            for cx in tx - span..=tx + span {
                let (dx, dy) = (cx as f32 + 0.5 - me.x, cy as f32 + 0.5 - me.y);
                if !d.in_bounds(cx, cy) || !((cx, cy) == (tx, ty) || dx * dx + dy * dy <= r * r) {
                    continue;
                }
                cols += 1;
                lit += u32::from(f32::from(self.world.shade_top[d.cidx(cx as usize, cy as usize)]) <= mid);
            }
        }
        if cols > 0 {
            light *= lit as f32 / cols as f32;
        }
        light.clamp(0.0, 1.0)
    }
}

/// Leaf-on fraction at temperature `t` for a canopy bare at `off` and full at `on` (shot G10).
pub fn leaf_fraction(t: f32, off: f32, on: f32) -> f32 {
    if on <= off {
        return if t >= on { 1.0 } else { 0.0 };
    }
    ((t - off) / (on - off)).clamp(0.0, 1.0)
}

/// The share of its full-leaf draw a tree takes at leaf-on fraction `leaf`, when leaf-off gives up
/// `deciduous` of it (shot G10). Both are clamped to [0, 1], so the draw is never negative and
/// never more than a tree in full leaf takes.
pub fn leaf_draw(deciduous: f32, leaf: f32) -> f32 {
    1.0 - deciduous.clamp(0.0, 1.0) * (1.0 - leaf.clamp(0.0, 1.0))
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
        let extinction = sim.params.canopy_extinction();
        for y in 0..WY {
            for x in 0..WX {
                let zs = sim.canopy_z(x as i32, y as i32);
                sim.canopy_cover[cidx(x, y)] = !zs.is_empty();
                sim.world.set_column_light(x, y, &zs, extinction);
            }
        }
        prop_assert!(light == sim.world.light, "incremental light differs from a full recompute");
        prop_assert_eq!(cover, sim.canopy_cover);
        Ok(())
    }

    /// Columns a tree's canopy lies over: the 3x3 around a mature trunk, the trunk column of a young one.
    /// `crown_crowding` equals a brute force over **every** live tree in the world, with no search
    /// box at all: `1 − Π(1 − overlap_fraction)`. The box is the only thing the shipped version does
    /// differently, so this is the test that the box never cuts off a neighbour that reaches.
    /// Planted trees keep `lifespan` within `max_age · (1 ± lifespan_jitter)`.
    ///
    /// The comparison has a tolerance because the two multiply the same factors in different orders
    /// — column-scan order against tree-index order — and IEEE multiplication is not associative.
    fn crowding_matches_brute_force(plants: &[(usize, usize, u32)], jitter: f32) -> Result<(), TestCaseError> {
        let mut sim = bare_sim();
        sim.params.tree.lifespan_jitter = jitter;
        for &(x, y, age) in plants {
            if sim.world.is_soil(x as i32, y as i32) && sim.spacing_ok(x as i32, y as i32) {
                sim.plant_tree(x, y, age);
            }
        }
        let mean = sim.params.tree_ages().max as f32;
        let crowns = sim.crowns();
        for (i, t) in sim.trees.iter().enumerate() {
            let (lo, hi) = ((mean * (1.0 - jitter)).floor() as u32, (mean * (1.0 + jitter)).ceil() as u32);
            prop_assert!((lo..=hi).contains(&t.lifespan), "lifespan {} outside [{}, {}]", t.lifespan, lo, hi);
            let me = &crowns.of[i];
            let mut open = 1.0f32;
            for (j, o) in sim.trees.iter().enumerate() {
                if j != i && o.alive {
                    open *= 1.0 - overlap_fraction(me, &crowns.of[j]);
                }
            }
            let (got, want) = (sim.crown_crowding(i, &crowns), (1.0 - open).clamp(0.0, 1.0));
            prop_assert!(
                (got - want).abs() <= 1e-5,
                "tree {} at ({}, {}): {} against a brute force over the whole world of {}",
                i,
                t.x,
                t.y,
                got,
                want
            );
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(32)))]

        #[test]
        fn prop_crown_crowding_matches_brute_force(
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
        // Mature at x = 10, 12, 14 (spacing 2) and a young tree at 13: under the old trunk count
        // the middle mature tree had three neighbours and the young one none, because a young
        // neighbour only ever counted at Chebyshev 1 and `min_spacing = 2` forbids that. Under the
        // crown rule the young tree is a 3.1 m crown among 11.5 m ones and is counted by its size,
        // which is the arm this shot deleted rather than repaired.
        let plants = [(10, 10, 2000), (12, 10, 2000), (14, 10, 2000), (13, 12, 600), (5, 5, 0)];
        crowding_matches_brute_force(&plants, 0.2).unwrap();
    }

    /// The scale change, as one arrangement: two mature trees 4 m apart crowd each other now and
    /// did not before.
    ///
    /// The old rule counted trunks within Chebyshev 2 and needed **two** of them, so a pair at 4 m
    /// was invisible to it twice over — too far, and too few. Their crowns are 3.45 m in radius at
    /// this age, so each covers a quarter of the other, and a third tree at 4 m on the other side
    /// takes the middle one past half. That is the whole of the shot in one picture: the trees
    /// compete at the size of what they carry, not at the size of the voxel stamp they shade.
    #[test]
    fn crowns_meet_at_four_metres_where_the_trunk_count_saw_nothing() {
        let mut sim = bare_sim();
        let crowns = |sim: &Sim| {
            let c = sim.crowns();
            (0..sim.trees.len()).map(|i| sim.crown_crowding(i, &c)).collect::<Vec<f32>>()
        };
        sim.plant_tree(10, 10, 2000);
        assert_eq!(crowns(&sim), [0.0], "a lone tree is not crowded by anybody");
        sim.plant_tree(14, 10, 2000);
        let pair = crowns(&sim);
        assert_eq!(pair[0], pair[1], "the pair is symmetric");
        assert!((0.2..0.35).contains(&pair[0]), "a neighbour 4 m away covers {} of the crown", pair[0]);
        sim.plant_tree(6, 10, 2000);
        let row = crowns(&sim);
        assert!(row[0] > 0.5, "the middle tree of three is over half covered: {}", row[0]);
        assert!(row[1] < 0.5 && row[2] < 0.5, "the outer two are not: {row:?}");
        // The old rule's answer to the same arrangement, for the record: no trunk is within
        // Chebyshev 2 of another, so it counted 0 for every one of the three.
        for (i, t) in sim.trees.iter().enumerate() {
            let near = sim
                .trees
                .iter()
                .enumerate()
                .filter(|&(j, o)| {
                    j != i
                        && o.alive
                        && (i32::from(o.x) - i32::from(t.x)).abs().max((i32::from(o.y) - i32::from(t.y)).abs()) <= 2
                })
                .count();
            assert_eq!(near, 0, "tree {i} had a trunk within Chebyshev 2");
        }
    }

    /// Self-thinning takes the crowded trees and leaves the one with room, the gap it opens is the
    /// gap a death opens, and at rate 0 nothing happens at all.
    ///
    /// The arrangement is the old test's — three mature trees in a row at x = 10, 12, 14, the
    /// closest `min_spacing` allows — and the verdict has moved, which is the shot. Their crowns
    /// are 3.45 m in radius, so at 2 m apart each pair overlaps by about 0.63 and every one of the
    /// three is over `crowding_overlap` where the old trunk count let the outer two through. The
    /// pass thins in index order and reads the live world as it goes, exactly as the old rule did:
    /// tree 0 goes, then tree 1 (still next to tree 2), and tree 2 is left standing alone. A row
    /// this tight is not a stand, it is one tree's worth of room.
    #[test]
    fn crowded_mature_tree_thins_and_lone_ones_survive() {
        let mut sim = bare_sim();
        sim.params.tree.crowding_mortality = 1.0;
        sim.moisture.fill(200.0);
        for x in [10, 12, 14] {
            sim.plant_tree(x, 10, 2000);
        }
        sim.update_trees();
        let alive: Vec<bool> = sim.trees.iter().map(|t| t.alive).collect();
        assert_eq!(alive, [false, false, true]);
        // Both dead trunks stand in the same patch, so it gets two trees' worth of detritus.
        assert_eq!(sim.patches[patch_of(12, 10)].detritus, 2.0 * sim.params.tree.death_detritus);
        assert_eq!(sim.world.surface_light(cidx(12, 10)), 255, "the gap over the dead trunk opens");
        // And the survivor is stable: a second update, with nobody left to crowd it, kills nothing.
        sim.update_trees();
        assert_eq!(sim.trees.iter().map(|t| t.alive).collect::<Vec<bool>>(), [false, false, true]);
        // With mortality 0 nothing thins — no draw is made and no crown is even measured.
        let mut sim = bare_sim();
        sim.params.tree.crowding_mortality = 0.0;
        sim.moisture.fill(200.0);
        for x in [10, 12, 14] {
            sim.plant_tree(x, 10, 2000);
        }
        sim.update_trees();
        assert!(sim.trees.iter().all(|t| t.alive));
    }

    /// `crowding_overlap` is the whole of the rule's severity, read on one pair 4 m apart whose
    /// crowns cover about 0.27 of each other.
    ///
    /// Above that the pair is left alone; below it the first tree goes and the second, now with
    /// nothing over it, stays. At 0 even that survivor dies, because a crown covered by nothing at
    /// all still satisfies `0 >= 0` — the degenerate end of the dial, recorded rather than
    /// special-cased, since a rule that kills isolated trees is not a setting anyone wants. The
    /// shipped default sits above the pair and is a tuning choice (TUNING.md, shot S11).
    #[test]
    fn the_overlap_threshold_is_the_severity_dial() {
        let pair = |overlap: f32| {
            let mut sim = bare_sim();
            sim.params.tree.crowding_mortality = 1.0;
            sim.params.tree.crowding_overlap = overlap;
            sim.moisture.fill(200.0);
            for x in [10, 14] {
                sim.plant_tree(x, 10, 2000);
            }
            sim.update_trees();
            sim.trees.iter().filter(|t| t.alive).count()
        };
        assert_eq!(pair(0.5), 2, "0.27 of a crown is under the default, so neither is at risk");
        assert_eq!(pair(0.2), 1, "below it the first goes and the second is then uncrowded");
        assert_eq!(pair(0.0), 0, "at 0 an uncrowded tree is at risk too");
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
            assert_eq!(sim.world.surface_light(cidx(x, y)), 35, "column ({x},{y})");
        }
        assert_eq!(sim.world.surface_light(cidx(12, 10)), 255);
        assert!(!sim.spacing_ok(11, 11));
        assert!(sim.spacing_ok(12, 12));
        sim.kill_tree(0, "old_age");
        assert_eq!(sim.world.surface_light(cidx(10, 10)), 255);
        assert_eq!(sim.patches[patch_of(10, 10)].detritus, 40.0);
    }

    /// Seeding fires once per `seed_every` ticks for any `seeds_per_year`, not only for the ones
    /// `tree.update_every` happens to divide (shot G4c). At the shipped 20 a year the new window
    /// test picks exactly the ages the old `is_multiple_of` test picked.
    #[test]
    fn a_mature_tree_seeds_at_its_rate_whatever_the_rate_is() {
        let fires = |seeds_per_year: f32| {
            let mut p = crate::Params::load_square();
            p.tree.seeds_per_year = seeds_per_year;
            let (every, step) = (p.tree_ages().seed_every, p.tree.update_every);
            let ages: Vec<u32> = (1..=p.climate.year_len / step).map(|k| k * step).collect();
            (every, ages.iter().filter(|a| *a % every < step).count())
        };
        let (every, n) = fires(20.0);
        assert_eq!((every, n), (200, 20), "the shipped rate, unchanged");
        for rate in [1.0, 3.0, 7.0, 20.0, 33.0, 50.0] {
            let (_, n) = fires(rate);
            let want = rate as usize;
            assert!(n.abs_diff(want) <= 1, "{rate} a year seeded {n} times");
        }
        assert_eq!(fires(200.0).1, 80, "a rate faster than the cadence seeds once per update");
    }

    #[test]
    fn young_canopy_is_one_voxel() {
        let mut sim = bare_sim();
        sim.plant_tree(20, 20, 500);
        assert_eq!(sim.world.surface_light(cidx(20, 20)), 94);
        assert_eq!(sim.world.surface_light(cidx(21, 20)), 255);
        sim.plant_tree(30, 30, 0);
        assert_eq!(sim.world.surface_light(cidx(30, 30)), 255, "saplings cast no shade");
    }

    // ---- Shot S3: the crown the allometry gives a tree, and the light it receives ----

    /// A sim with no trees in which every tree planted keeps the age it is given: `bare_sim` on
    /// flat ground, with the crown fractions at their shipped values.
    fn crown_sim() -> Sim {
        let mut sim = bare_sim();
        sim.params.tree.min_spacing = 1;
        sim
    }

    /// Plant a tree of `age` at (x, y) without the spacing rule, so a test can put two crowns as
    /// close as it likes; `plant_tree` draws one lifespan from the RNG, as it does in a run.
    fn plant(sim: &mut Sim, x: usize, y: usize, age: u32) -> usize {
        sim.plant_tree(x, y, age);
        sim.trees.len() - 1
    }

    /// The age at which the height curve is flat: `bundle.tree_tall_age_years`, where a tree is
    /// `tree_tall_height` metres tall and stays there.
    fn tall_age(sim: &Sim) -> u32 {
        sim.params.tree_ages().tall_import
    }

    /// `height_of_age` inverts `import_age` wherever the map is invertible: a height under
    /// `tree_tall_height` survives the round trip to within the metre-per-tick the age grid rounds
    /// to, and the map is monotone in both directions.
    fn height_round_trips(height: f32, p: &crate::params::Params) -> Result<(), TestCaseError> {
        let age = crate::plants::import_age(height, p);
        let back = crate::plants::height_of_age(age, p);
        let ages = p.tree_ages();
        // One tick of age is at most this many metres anywhere on the curve, so one tick of
        // rounding in `import_age` is at most this much height.
        let per_tick = (p.bundle.tree_tall_height / ages.tall_import.max(1) as f32)
            .max(p.bundle.tree_mature_height / ages.mature.max(1) as f32);
        prop_assert!(
            (back - height).abs() <= per_tick + 1e-3,
            "{height} m -> age {age} -> {back} m, over one tick of {per_tick} m"
        );
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(128)))]

        #[test]
        fn prop_height_of_age_inverts_import_age(height in 0.0f32..=20.0) {
            height_round_trips(height, &crate::params::Params::load_square())?;
        }

        /// The other direction, and monotonicity: an older tree is never shorter.
        #[test]
        fn prop_height_of_age_is_monotone(a in 0u32..12_000, b in 0u32..12_000) {
            let p = crate::params::Params::load_square();
            let (lo, hi) = (a.min(b), a.max(b));
            prop_assert!(crate::plants::height_of_age(lo, &p) <= crate::plants::height_of_age(hi, &p));
        }
    }

    /// Regression sibling of the two properties above: the corners of the shipped curve. A seed is
    /// 0 m, a tree of `mature_age` is exactly `tree_mature_height`, one of `tree_tall_age` is
    /// exactly `tree_tall_height`, and every age past it is still that height.
    #[test]
    fn height_of_age_regression_curve_corners() {
        let p = crate::params::Params::load_square();
        let (a, b) = (p.tree_ages(), &p.bundle);
        let h = |age| crate::plants::height_of_age(age, &p);
        assert_eq!(h(0), 0.0);
        assert!((h(a.mature) - b.tree_mature_height).abs() < 1e-4, "{}", h(a.mature));
        assert!((h(a.tall_import) - b.tree_tall_height).abs() < 1e-4, "{}", h(a.tall_import));
        assert_eq!(h(a.tall_import * 4), b.tree_tall_height, "the curve is flat above the tall age");
        assert!((h(a.mature / 2) - b.tree_mature_height / 2.0).abs() < 1e-3, "half of mature is half the height");
        for height in [0.0, 1.5, 3.0, 12.0, 20.0] {
            height_round_trips(height, &p).unwrap();
        }
    }

    /// `overlap_fraction` is an area ratio: `f(a, b) x area(a)` is the same lens as
    /// `f(b, a) x area(b)`, it is 1 when b swallows a, and 0 once the discs are apart.
    fn overlap_is_an_area_ratio(ra: f32, rb: f32, d: f32) -> Result<(), TestCaseError> {
        let at = |x: f32, r: f32| Crown { x, y: 0.0, ground: 0.0, height: 10.0, base: 0.0, radius: r };
        let (a, b) = (at(0.0, ra), at(d, rb));
        let (fa, fb) = (overlap_fraction(&a, &b), overlap_fraction(&b, &a));
        prop_assert!((0.0..=1.0).contains(&fa) && (0.0..=1.0).contains(&fb), "{fa}, {fb}");
        if ra > 0.0 && rb > 0.0 {
            let (la, lb) = (fa * ra * ra, fb * rb * rb);
            prop_assert!((la - lb).abs() <= 1e-3 * (la.max(lb).max(1.0)), "lens {la} vs {lb}");
        }
        if d >= ra + rb {
            prop_assert_eq!(fa, 0.0);
        }
        if rb >= ra + d && rb > 0.0 {
            prop_assert_eq!(fa, 1.0, "b swallows a");
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(128)))]

        #[test]
        fn prop_overlap_fraction_is_an_area_ratio(
            ra in 0.0f32..8.0,
            rb in 0.0f32..8.0,
            d in 0.0f32..20.0,
        ) {
            overlap_is_an_area_ratio(ra, rb, d)?;
        }
    }

    /// Regression sibling: the three cases with a closed form. Equal discs centre on centre are
    /// one disc; equal discs touching share nothing; a disc over the rim of an equal one shares
    /// the standard lens, `(2 acos(1/2) - sin(2 acos(1/2))) / pi` of it at d = r.
    #[test]
    fn overlap_fraction_regression_closed_forms() {
        let at = |x: f32, r: f32| Crown { x, y: 0.0, ground: 0.0, height: 10.0, base: 0.0, radius: r };
        assert_eq!(overlap_fraction(&at(0.0, 4.0), &at(0.0, 4.0)), 1.0);
        assert_eq!(overlap_fraction(&at(0.0, 4.0), &at(8.0, 4.0)), 0.0);
        assert_eq!(overlap_fraction(&at(0.0, 4.0), &at(2.0, 2.0)), 0.25, "a quarter of the area, wholly inside");
        let got = overlap_fraction(&at(0.0, 4.0), &at(4.0, 4.0));
        let theta = 2.0 * libm::acosf(0.5);
        let want = (theta - libm::sinf(theta)) / std::f32::consts::PI;
        assert!((got - want).abs() < 1e-4, "{got} vs {want}");
        // A point crown is covered or it is not.
        assert_eq!(overlap_fraction(&at(0.0, 0.0), &at(3.0, 4.0)), 1.0);
        assert_eq!(overlap_fraction(&at(0.0, 0.0), &at(5.0, 4.0)), 0.0);
    }

    /// Crown light is a fraction of full sun whatever the trees do: finite, in [0, 1], and 1 for a
    /// tree standing alone.
    fn crown_light_is_a_fraction(ops: &[(usize, usize, u32)]) -> Result<(), TestCaseError> {
        let mut sim = crown_sim();
        for &(x, y, age) in ops {
            if sim.world.is_soil(x as i32, y as i32) && sim.trunk_at[cidx(x, y)] == NO_TREE {
                plant(&mut sim, x, y, age);
            }
        }
        let crowns = sim.crowns();
        for i in 0..sim.trees.len() {
            let l = sim.crown_light(i, &crowns);
            prop_assert!(l.is_finite() && (0.0..=1.0).contains(&l), "tree {i}: {l}");
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(32)))]

        #[test]
        fn prop_crown_light_is_a_fraction_of_full_sun(
            ops in prop::collection::vec((2usize..60, 2usize..60, 0u32..8000), 0..24),
        ) {
            crown_light_is_a_fraction(&ops)?;
        }

        /// A neighbour never brightens a crown, and moving it away never darkens one: light rises
        /// with distance, from the two trees on top of each other to the two out of reach.
        #[test]
        fn prop_crown_light_rises_as_a_neighbour_retreats(
            age in 1000u32..8000,
            near in 0usize..7,
            step in 1usize..8,
        ) {
            let light_at = |gap: usize| {
                let mut sim = crown_sim();
                let a = plant(&mut sim, 20, 20, age);
                plant(&mut sim, 20 + gap, 20, age);
                let crowns = sim.crowns();
                sim.crown_light(a, &crowns)
            };
            let (close, far) = (light_at(near), light_at(near + step));
            prop_assert!(close <= far + 1e-6, "gap {near}: {close}, gap {}: {far}", near + step);
            prop_assert!(far <= 1.0 + 1e-6);
        }
    }

    /// Regression sibling, and the numbers the model is pinned to. A tree alone is in full sun. A
    /// crown wholly under an equally tall one loses the optical depth of that crown's upper half,
    /// `exp(-tau/2)` at `tau = 2 x canopy_k x canopy_lai`. One wholly under a crown that is
    /// entirely above it loses the whole `exp(-tau)`, which is the 13.5% of full sun `params.toml`
    /// calibrates for a mature canopy. Two neighbours multiply.
    #[test]
    fn crown_light_regression_alone_under_one_and_under_two() {
        let mut sim = crown_sim();
        let tau = 2.0 * sim.params.canopy_extinction();
        let age = tall_age(&sim);
        let a = plant(&mut sim, 20, 20, age);
        let crowns = sim.crowns();
        assert_eq!(sim.crown_light(a, &crowns), 1.0, "a tree alone is in full sun");

        // A second tree of the same age on the same column: same crown, so the ray to a's middle
        // passes through the upper half of b's.
        let mut sim = crown_sim();
        let a = plant(&mut sim, 20, 20, age);
        sim.trees.push(Tree {
            id: 999,
            x: 20,
            y: 20,
            age,
            dry_ticks: 0,
            lifespan: u32::MAX,
            alive: true,
            npk: [0.0; 3],
        });
        sim.trunk_at[cidx(20, 20)] = 1;
        let crowns = sim.crowns();
        let want = libm::expf(-tau * 0.5);
        let got = sim.crown_light(a, &crowns);
        assert!((got - want).abs() < 1e-5, "{got} vs {want}");

        // The same, twice over: two identical crowns overhead multiply to exp(-tau).
        sim.trees.push(Tree {
            id: 1000,
            x: 21,
            y: 20,
            age,
            dry_ticks: 0,
            lifespan: u32::MAX,
            alive: true,
            npk: [0.0; 3],
        });
        sim.trunk_at[cidx(21, 20)] = 2;
        let crowns = sim.crowns();
        let got = sim.crown_light(a, &crowns);
        assert!(got < want, "a second neighbour darkens it further: {got} vs {want}");
        assert!(got > libm::expf(-tau), "and not below two whole crowns: {got}");
    }

    /// The measurement this shot exists for, in miniature. On the simulator's own light field every
    /// mature tree reads the same `exp(-2k)` at its trunk, however it stands; the published crown
    /// light of the same trees takes three different values, because a crown 6 m across notices its
    /// neighbours and a 3x3 m canopy stamp cannot.
    #[test]
    fn crown_light_varies_where_the_light_field_cannot() {
        let mut sim = crown_sim();
        let age = tall_age(&sim);
        let alone = plant(&mut sim, 6, 6, age);
        let pair = plant(&mut sim, 30, 30, age);
        plant(&mut sim, 33, 30, age);
        let crowded = plant(&mut sim, 50, 50, age);
        for (dx, dy) in [(3, 0), (-3, 0), (0, 3), (0, -3)] {
            plant(&mut sim, (50i32 + dx) as usize, (50i32 + dy) as usize, age);
        }
        let field: Vec<u8> =
            [alone, pair, crowded].iter().map(|&i| sim.world.surface_light(sim.trees[i].col(D))).collect();
        assert_eq!(field[0], field[1], "the light field cannot tell them apart");
        assert_eq!(field[1], field[2]);
        let crowns = sim.crowns();
        let light: Vec<f32> = [alone, pair, crowded].iter().map(|&i| sim.crown_light(i, &crowns)).collect();
        assert_eq!(light[0], 1.0);
        assert!(light[1] < light[0] && light[2] < light[1], "{light:?}");
    }

    /// A building's shadow reaches a crown: `shade_top` above the crown's middle takes that share
    /// of the footprint's columns to nothing, and a crown clear of it keeps full sun. In a noise
    /// world `shade_top` is all zeroes, so the term is exactly 1 and nothing below changes there.
    #[test]
    fn crown_light_regression_a_roof_shadow_over_a_crown() {
        let mut sim = crown_sim();
        let age = tall_age(&sim);
        let i = plant(&mut sim, 20, 20, age);
        let crowns = sim.crowns();
        assert_eq!(sim.crown_light(i, &crowns), 1.0, "no buildings in a noise world");
        let mid = crowns.of[i].mid();

        // Shade every column of the footprint to above the crown's middle.
        for y in 12..=28 {
            for x in 12..=28 {
                sim.world.shade_top[cidx(x, y)] = (mid + 1.0) as u8;
            }
        }
        assert_eq!(sim.crown_light(i, &sim.crowns()), 0.0, "a crown wholly in shadow gets nothing");

        // Half of them, and the light halves with the lit share of the footprint.
        for y in 12..=28 {
            for x in 21..=28 {
                sim.world.shade_top[cidx(x, y)] = 0;
            }
        }
        let got = sim.crown_light(i, &sim.crowns());
        assert!((0.3..0.7).contains(&got), "half a footprint in shadow: {got}");

        // A shadow that stops below the crown's middle does not reach it.
        for y in 12..=28 {
            for x in 12..=28 {
                sim.world.shade_top[cidx(x, y)] = (mid - 1.0) as u8;
            }
        }
        assert_eq!(sim.crown_light(i, &sim.crowns()), 1.0, "a shadow under the crown is not on it");
    }

    /// A dead tree shades nothing: `kill_tree` clears `trunk_at`, which is what the neighbour
    /// search reads, so the survivor is back in full sun at the same tick.
    #[test]
    fn crown_light_regression_a_dead_neighbour_stops_shading() {
        let mut sim = crown_sim();
        let age = tall_age(&sim);
        let a = plant(&mut sim, 20, 20, age);
        let b = plant(&mut sim, 22, 20, age);
        assert!(sim.crown_light(a, &sim.crowns()) < 1.0);
        sim.kill_tree(b, "old_age");
        assert_eq!(sim.crown_light(a, &sim.crowns()), 1.0);
    }

    /// The crown of a tree the simulator grew: `crown_of` is the height curve and the two surveyed
    /// fractions, and nothing else. A sapling of age 0 has no crown at all, which is why its light
    /// is read at ground level, where a neighbour's whole crown stands over it.
    #[test]
    fn crown_of_regression_shape_by_age() {
        let mut sim = crown_sim();
        let age = tall_age(&sim);
        let tp = sim.params.tree.clone();
        let i = plant(&mut sim, 20, 20, age);
        let c = sim.crown_of(&sim.trees[i]);
        assert_eq!((c.x, c.y), (20.5, 20.5));
        assert_eq!(c.height, sim.params.bundle.tree_tall_height);
        assert!((c.radius - tp.crown_radius_frac * c.height).abs() < 1e-5);
        assert!((c.base - tp.crown_base_frac * c.height).abs() < 1e-5);
        assert!((c.mid() - (c.ground + 0.5 * (c.base + c.height))).abs() < 1e-5);
        assert_eq!(c.depth_above(c.ground + c.height + 1.0), 0.0, "nothing above the crown");
        assert!((c.depth_above(c.ground) - c.depth()).abs() < 1e-5, "everything below it");

        let j = plant(&mut sim, 40, 40, 0);
        let seed = sim.crown_of(&sim.trees[j]);
        assert_eq!((seed.height, seed.radius, seed.base, seed.depth()), (0.0, 0.0, 0.0, 0.0));
        assert_eq!(seed.mid(), seed.ground, "a seed's light is read at the ground");
    }

    /// The leaf-on share stays in [0, 1] and never falls as the patch warms (shot G10).
    fn leaf_fraction_monotone(t: (f32, f32), off: f32, width: f32) -> Result<(), TestCaseError> {
        let (lo, hi) = if t.0 <= t.1 { t } else { (t.1, t.0) };
        let on = off + width;
        let (a, b) = (leaf_fraction(lo, off, on), leaf_fraction(hi, off, on));
        prop_assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b), "{a} {b}");
        prop_assert!(a <= b, "leaf fell from {a} at {lo} to {b} at {hi} (off {off}, on {on})");
        Ok(())
    }

    /// A tree's draw is never negative, never more than in full leaf, never falls as the canopy
    /// fills, and is exactly the full-leaf draw for an evergreen (shot G10).
    fn leaf_draw_bounded(deciduous: f32, leaf: (f32, f32)) -> Result<(), TestCaseError> {
        let (lo, hi) = if leaf.0 <= leaf.1 { leaf } else { (leaf.1, leaf.0) };
        let (a, b) = (leaf_draw(deciduous, lo), leaf_draw(deciduous, hi));
        prop_assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b), "{a} {b}");
        prop_assert!(a <= b, "the draw fell from {a} to {b} as the canopy filled");
        prop_assert_eq!(leaf_draw(0.0, lo), 1.0);
        prop_assert_eq!(leaf_draw(deciduous, 1.0), 1.0);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(256)))]

        #[test]
        fn prop_leaf_fraction_monotone(
            t in (-40.0f32..50.0, -40.0f32..50.0),
            off in -10.0f32..20.0,
            width in -5.0f32..15.0,
        ) {
            leaf_fraction_monotone(t, off, width)?;
        }

        #[test]
        fn prop_leaf_draw_bounded(deciduous in -0.5f32..1.5, leaf in (-0.5f32..1.5, -0.5f32..1.5)) {
            leaf_draw_bounded(deciduous, leaf)?;
        }
    }

    #[test]
    fn leaf_fraction_regression_ramp_ends_and_step() {
        assert_eq!(leaf_fraction(5.0, 5.0, 10.0), 0.0);
        assert_eq!(leaf_fraction(7.5, 5.0, 10.0), 0.5);
        assert_eq!(leaf_fraction(10.0, 5.0, 10.0), 1.0);
        // No width: a step at the on temperature, from either side.
        assert_eq!(leaf_fraction(9.99, 10.0, 10.0), 0.0);
        assert_eq!(leaf_fraction(10.0, 10.0, 10.0), 1.0);
        assert_eq!(leaf_fraction(9.0, 12.0, 8.0), 1.0);
        leaf_fraction_monotone((4.0, 6.0), 5.0, 0.0).unwrap();
        leaf_fraction_monotone((6.0, 4.0), 5.0, -3.0).unwrap();
    }

    #[test]
    fn leaf_draw_regression_corners() {
        assert_eq!(leaf_draw(1.0, 0.0), 0.0, "a bare fully deciduous tree draws nothing");
        assert_eq!(leaf_draw(0.5, 0.0), 0.5);
        assert_eq!(leaf_draw(2.0, -1.0), 0.0, "out-of-range inputs clamp, never go negative");
        leaf_draw_bounded(1.0, (0.0, 1.0)).unwrap();
        leaf_draw_bounded(-0.5, (1.5, -0.5)).unwrap();
    }

    /// One tree over one update: in a cold patch a deciduous tree draws nothing, in a warm one it
    /// draws what it always did, and an evergreen draws the same in both (shot G10).
    #[test]
    fn a_bare_tree_does_not_drink() {
        let drop = |deciduous: f32, temperature: f32| {
            let mut sim = bare_sim();
            sim.params.tree.deciduous = deciduous;
            sim.moisture.fill(200.0);
            for p in sim.patches.iter_mut() {
                p.temperature = temperature;
            }
            sim.plant_tree(20, 20, 2000);
            let c = cidx(20, 20);
            let before = sim.moisture[c];
            sim.update_trees();
            before - sim.moisture[c]
        };
        let full = drop(0.0, 20.0);
        assert!(full > 0.0, "a tree in leaf draws water");
        assert_eq!(drop(0.0, -5.0), full, "an evergreen draws the same in winter");
        assert_eq!(drop(1.0, -5.0), 0.0, "a bare deciduous tree draws nothing");
        // In leaf it draws the year's water in less of the year, so more than an evergreen does.
        let norm = {
            let mut sim = bare_sim();
            sim.params.tree.deciduous = 1.0;
            sim.leaf_draw_mean() as f32
        };
        assert!((0.5..0.7).contains(&norm), "the model year is {norm} in leaf");
        let leafy = drop(1.0, 20.0);
        assert!((leafy - full / norm).abs() < 1e-3 * leafy, "in full leaf it draws {leafy}, not {full} / {norm}");
        let half = drop(1.0, 7.5);
        assert!((half - leafy / 2.0).abs() < 1e-3 * leafy, "half a canopy draws half: {half} of {leafy}");
    }

    /// Over a whole model year a deciduous tree draws what an evergreen draws, to within the
    /// stepping of the temperature update, at any `deciduous` (shot G10): leaf-off moves the water,
    /// it does not save it. A tree that never leafs draws nothing, and the mean is then 0.
    #[test]
    fn a_year_of_leaf_off_moves_the_water_and_saves_none() {
        let year = |deciduous: f32, on: f32| {
            let mut sim = bare_sim();
            (sim.params.tree.deciduous, sim.params.tree.leaf_on_temp) = (deciduous, on);
            sim.params.tree.leaf_off_temp = on - 5.0;
            (sim.params.tree.max_age_years, sim.params.tree.dry_fraction) = (1000.0, 0.0);
            sim.params.climate.canopy_cool = 0.0;
            sim.moisture.fill(255.0);
            sim.plant_tree(20, 20, 2000);
            let c = cidx(20, 20);
            let mut drawn = 0.0f64;
            for t in 0..sim.params.climate.year_len {
                if t % sim.params.schedule.temperature_every == 0 {
                    sim.update_temperature(t);
                }
                if t % sim.params.tree.update_every == 0 {
                    sim.moisture[c] = 255.0;
                    sim.update_trees();
                    drawn += (255.0 - sim.moisture[c]) as f64;
                }
            }
            drawn
        };
        let evergreen = year(0.0, 10.0);
        for d in [0.25, 0.5, 1.0] {
            let got = year(d, 10.0);
            assert!((got - evergreen).abs() < 0.01 * evergreen, "deciduous {d}: {got} against {evergreen}");
        }
        assert_eq!(year(1.0, 60.0), 0.0);
        let mut sim = bare_sim();
        sim.params.tree.deciduous = 0.0;
        assert_eq!(sim.leaf_draw_mean(), 1.0, "at deciduous 0 there is nothing to move");
    }
}
