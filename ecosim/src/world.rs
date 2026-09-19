//! Voxel world: terrain generation, materials, column classes and the light field.

use crate::params::Params;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// World size in x (columns).
pub const WX: usize = 64;
/// World size in y (columns).
pub const WY: usize = 64;
/// World height in voxels; z is up.
pub const WZ: usize = 32;
/// Number of columns.
pub const COLS: usize = WX * WY;
/// Number of voxels.
pub const VOXELS: usize = WX * WY * WZ;
/// Patch edge length in columns.
pub const PATCH_SIZE: usize = 8;
/// Patches along x.
pub const PATCHES_X: usize = WX / PATCH_SIZE;
/// Number of patches.
pub const PATCHES: usize = PATCHES_X * (WY / PATCH_SIZE);

/// Material code: air.
pub const AIR: u8 = 0;
/// Material code: soil.
pub const SOIL: u8 = 1;
/// Material code: rock.
pub const ROCK: u8 = 2;
/// Material code: water.
pub const WATER: u8 = 3;

/// Voxel index, x fastest: x + 64·(y + 64·z).
#[inline]
pub fn vidx(x: usize, y: usize, z: usize) -> usize {
    x + WX * (y + WY * z)
}

/// Column index: x + 64·y.
#[inline]
pub fn cidx(x: usize, y: usize) -> usize {
    x + WX * y
}

/// Patch index of a column.
#[inline]
pub fn patch_of(x: usize, y: usize) -> usize {
    x / PATCH_SIZE + PATCHES_X * (y / PATCH_SIZE)
}

/// Whether (x, y) is inside the world.
#[inline]
pub fn in_bounds(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && (x as usize) < WX && (y as usize) < WY
}

/// What a column is topped with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColClass {
    /// Soil on top: plants grow, animals walk.
    Soil,
    /// Water on top (terrain below the water level).
    Water,
    /// Rock on top (at or above `rock_top_height`).
    Rock,
}

/// Terrain, materials, light and walking distances.
pub struct World {
    /// Material code per voxel (`vidx` order).
    pub material: Vec<u8>,
    /// Light per voxel, 0–255 (`vidx` order).
    pub light: Vec<u8>,
    /// z of the topmost non-Air voxel per column (water columns report the water surface).
    pub height: Vec<u8>,
    /// Terrain height (topmost solid voxel) per column.
    pub ground: Vec<u8>,
    /// Column class per column.
    pub class: Vec<ColClass>,
    /// Soil column indices per patch, ascending.
    pub patch_soil: Vec<Vec<usize>>,
    /// Walking distance (8-connected steps over soil) from each column to the nearest soil
    /// column of each patch: `patch_dist[p * COLS + c]`, `UNREACHABLE` if there is no path.
    pub patch_dist: Vec<u16>,
}

/// Marks a patch that cannot be walked to from a column.
pub const UNREACHABLE: u16 = u16::MAX;

/// Multi-source BFS over soil columns (8-neighbour moves) from every patch's soil columns.
fn patch_distances(class: &[ColClass], patch_soil: &[Vec<usize>]) -> Vec<u16> {
    let mut dist = vec![UNREACHABLE; PATCHES * COLS];
    let mut queue = std::collections::VecDeque::new();
    for (p, cols) in patch_soil.iter().enumerate() {
        let d = &mut dist[p * COLS..(p + 1) * COLS];
        queue.clear();
        for &c in cols {
            d[c] = 0;
            queue.push_back(c);
        }
        while let Some(c) = queue.pop_front() {
            let (x, y) = ((c % WX) as i32, (c / WX) as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx, dy) == (0, 0) || !in_bounds(nx, ny) {
                        continue;
                    }
                    let n = cidx(nx as usize, ny as usize);
                    if class[n] == ColClass::Soil && d[n] == UNREACHABLE {
                        d[n] = d[c] + 1;
                        queue.push_back(n);
                    }
                }
            }
        }
    }
    dist
}

/// One octave of seeded 2D value noise over the world, lattice spacing `period` columns.
fn value_noise_octave(rng: &mut ChaCha8Rng, period: usize) -> Vec<f32> {
    let n = WX / period + 1;
    let lattice: Vec<f32> = (0..n * n).map(|_| rng.gen::<f32>()).collect();
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let mut out = vec![0.0; COLS];
    for y in 0..WY {
        for x in 0..WX {
            let (gx, gy) = (x / period, y / period);
            let tx = smooth((x % period) as f32 / period as f32);
            let ty = smooth((y % period) as f32 / period as f32);
            let v = |i: usize, j: usize| lattice[i + n * j];
            let a = v(gx, gy) + (v(gx + 1, gy) - v(gx, gy)) * tx;
            let b = v(gx, gy + 1) + (v(gx + 1, gy + 1) - v(gx, gy + 1)) * tx;
            out[cidx(x, y)] = a + (b - a) * ty;
        }
    }
    out
}

/// Heightmap from 2 octaves of value noise (periods 32 and 8, amplitudes 1.0 and 0.35).
/// Normalization is piecewise-linear so that the lowest `water_fraction` of columns lands below
/// the water line: [min, q] → [height_min, water_level − 0.5], [q, max] → [water_level − 0.5, height_max],
/// where q is the `water_fraction` quantile of the noise.
pub fn generate_heights(params: &Params, rng: &mut ChaCha8Rng) -> Vec<u8> {
    let o1 = value_noise_octave(rng, 32);
    let o2 = value_noise_octave(rng, 8);
    let v: Vec<f32> = o1.iter().zip(&o2).map(|(a, b)| a + 0.35 * b).collect();
    let mut sorted = v.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let lo = sorted[0];
    let hi = sorted[COLS - 1];
    let qi = ((params.world.water_fraction * COLS as f32) as usize).clamp(1, COLS - 2);
    let q = sorted[qi].clamp(lo + 1e-6, hi - 1e-6);
    let wp = &params.world;
    let (hmin, hmax, hw) = (wp.height_min as f32, wp.height_max as f32, wp.water_level as f32 - 0.5);
    v.iter()
        .map(|&x| {
            let h =
                if x < q { hmin + (x - lo) / (q - lo) * (hw - hmin) } else { hw + (x - q) / (hi - q) * (hmax - hw) };
            h.round().clamp(hmin, hmax) as u8
        })
        .collect()
}

impl World {
    /// Generate terrain from the seeded RNG and build the world.
    pub fn generate(params: &Params, rng: &mut ChaCha8Rng) -> World {
        let heights = generate_heights(params, rng);
        World::from_heights(&heights, params)
    }

    /// Build materials, column classes and initial (canopy-free) light from terrain heights.
    pub fn from_heights(heights: &[u8], params: &Params) -> World {
        assert_eq!(heights.len(), COLS);
        let wp = &params.world;
        let mut material = vec![AIR; VOXELS];
        let mut height = vec![0u8; COLS];
        let mut class = vec![ColClass::Rock; COLS];
        for y in 0..WY {
            for x in 0..WX {
                let c = cidx(x, y);
                let h = heights[c].min(WZ as u8 - 1) as usize;
                for z in 0..=h {
                    let soil_layer = z + wp.soil_depth as usize > h;
                    material[vidx(x, y, z)] = if soil_layer { SOIL } else { ROCK };
                }
                if h >= wp.rock_top_height as usize {
                    material[vidx(x, y, h)] = ROCK;
                }
                let wl = wp.water_level as usize;
                for z in (h + 1)..=wl.min(WZ - 1) {
                    material[vidx(x, y, z)] = WATER;
                }
                let top = (0..WZ).rev().find(|&z| material[vidx(x, y, z)] != AIR).unwrap_or(0);
                height[c] = top as u8;
                class[c] = match material[vidx(x, y, top)] {
                    SOIL => ColClass::Soil,
                    WATER => ColClass::Water,
                    _ => ColClass::Rock,
                };
            }
        }
        let mut patch_soil = vec![Vec::new(); PATCHES];
        for y in 0..WY {
            for x in 0..WX {
                if class[cidx(x, y)] == ColClass::Soil {
                    patch_soil[patch_of(x, y)].push(cidx(x, y));
                }
            }
        }
        // Sort each patch's list so iteration is ascending by column index.
        for p in patch_soil.iter_mut() {
            p.sort_unstable();
        }
        let patch_dist = patch_distances(&class, &patch_soil);
        let mut w = World {
            material,
            light: vec![0; VOXELS],
            height,
            ground: heights.iter().map(|&h| h.min(WZ as u8 - 1)).collect(),
            class,
            patch_soil,
            patch_dist,
        };
        for y in 0..WY {
            for x in 0..WX {
                w.set_column_light(x, y, &[], params.world.canopy_absorb);
            }
        }
        w
    }

    /// Whether (x, y) is inside the world and a soil column.
    #[inline]
    pub fn is_soil(&self, x: i32, y: i32) -> bool {
        in_bounds(x, y) && self.class[cidx(x as usize, y as usize)] == ColClass::Soil
    }

    /// Light of the voxel directly above the column's surface.
    #[inline]
    pub fn surface_light(&self, c: usize) -> u8 {
        let (x, y) = (c % WX, c / WX);
        let z = (self.height[c] as usize + 1).min(WZ - 1);
        self.light[vidx(x, y, z)]
    }

    /// Recompute one column's light: solids are 0; air and water get
    /// 255 − absorb × (canopy voxels strictly above), saturating at 0.
    /// `canopy_z` lists the distinct canopy voxel z values in this column.
    pub fn set_column_light(&mut self, x: usize, y: usize, canopy_z: &[u8], absorb: u8) {
        for z in 0..WZ {
            let i = vidx(x, y, z);
            let m = self.material[i];
            self.light[i] = if m == SOIL || m == ROCK {
                0
            } else {
                let above = canopy_z.iter().filter(|&&cz| cz as usize > z).count() as u32;
                255 - (above * absorb as u32).min(255) as u8
            };
        }
    }

    /// Walking distance from column `c` to patch `p` (see `patch_dist`).
    #[inline]
    pub fn dist_to_patch(&self, p: usize, c: usize) -> u16 {
        self.patch_dist[p * COLS + c]
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use proptest::prelude::*;

    fn flat(h: u8) -> World {
        World::from_heights(&vec![h; COLS], &Params::load_default())
    }

    #[test]
    fn light_column_with_one_canopy_voxel() {
        let mut w = flat(12);
        w.set_column_light(5, 5, &[15], 100);
        assert_eq!(w.light[vidx(5, 5, 20)], 255);
        assert_eq!(w.light[vidx(5, 5, 15)], 255, "canopy voxel itself is not below itself");
        assert_eq!(w.light[vidx(5, 5, 14)], 155);
        assert_eq!(w.light[vidx(5, 5, 13)], 155);
        assert_eq!(w.light[vidx(5, 5, 12)], 0, "solid ground is 0");
        assert_eq!(w.surface_light(cidx(5, 5)), 155);
        // A neighbouring column is untouched.
        assert_eq!(w.surface_light(cidx(6, 5)), 255);
        // Three canopy voxels saturate at 0.
        w.set_column_light(5, 5, &[15, 16, 17], 100);
        assert_eq!(w.surface_light(cidx(5, 5)), 0);
    }

    #[test]
    fn materials_and_classes() {
        let mut h = vec![12u8; COLS];
        h[cidx(0, 0)] = 8; // under water level 10
        h[cidx(1, 0)] = 22; // rock-topped
        let w = World::from_heights(&h, &Params::load_default());
        assert_eq!(w.class[cidx(0, 0)], ColClass::Water);
        assert_eq!(w.height[cidx(0, 0)], 10);
        assert_eq!(w.material[vidx(0, 0, 9)], WATER);
        assert_eq!(w.material[vidx(0, 0, 8)], SOIL);
        assert_eq!(w.material[vidx(0, 0, 5)], ROCK);
        assert_eq!(w.class[cidx(1, 0)], ColClass::Rock);
        assert_eq!(w.material[vidx(1, 0, 21)], SOIL);
        assert_eq!(w.class[cidx(2, 0)], ColClass::Soil);
        assert_eq!(w.material[vidx(2, 0, 12)], SOIL);
        assert_eq!(w.material[vidx(2, 0, 10)], SOIL);
        assert_eq!(w.material[vidx(2, 0, 9)], ROCK);
        assert_eq!(w.material[vidx(2, 0, 13)], AIR);
        assert_eq!(w.surface_light(cidx(2, 0)), 255);
    }

    #[test]
    fn patch_distance_walks_around_a_wall() {
        // A rock wall at x = 12 from y = 0 to y = 20 splits patch 1 (x 8..16) from patch 0's side.
        let mut h = vec![12u8; COLS];
        for y in 0..=20 {
            h[cidx(12, y)] = 22;
        }
        let w = World::from_heights(&h, &Params::load_default());
        assert_eq!(w.dist_to_patch(0, cidx(3, 3)), 0);
        assert_eq!(w.dist_to_patch(0, cidx(10, 3)), 3);
        // From x = 13 the way west is blocked: 18 steps to get round the wall's end at y = 21,
        // then 14 back up to patch 0 (x < 8, y < 8).
        assert_eq!(w.dist_to_patch(0, cidx(13, 3)), 18 + 14);
        assert_eq!(w.dist_to_patch(0, cidx(12, 3)), UNREACHABLE, "rock column itself");
    }

    /// Mostly soil, with water (below level 10) and rock-topped (≥ 21) columns mixed in.
    pub(crate) fn terrain() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(prop_oneof![6 => 11u8..=20, 1 => 6u8..=9, 1 => 21u8..=24], COLS)
    }

    /// Single-source BFS over soil columns (8-neighbour steps) from column `s`.
    fn bfs_from(w: &World, s: usize) -> Vec<u16> {
        let mut d = vec![UNREACHABLE; COLS];
        let mut q = std::collections::VecDeque::from([s]);
        d[s] = 0;
        while let Some(c) = q.pop_front() {
            let (x, y) = ((c % WX) as i32, (c / WX) as i32);
            for (nx, ny) in (y - 1..=y + 1).flat_map(|ny| (x - 1..=x + 1).map(move |nx| (nx, ny))) {
                if w.is_soil(nx, ny) && d[cidx(nx as usize, ny as usize)] == UNREACHABLE {
                    d[cidx(nx as usize, ny as usize)] = d[c] + 1;
                    q.push_back(cidx(nx as usize, ny as usize));
                }
            }
        }
        d
    }

    /// `patch_dist` equals, per patch, the minimum over its soil columns of a single-source BFS.
    fn patch_dist_matches_brute_force(heights: &[u8]) -> Result<(), TestCaseError> {
        let w = World::from_heights(heights, &Params::load_default());
        let mut want = vec![UNREACHABLE; PATCHES * COLS];
        let from: Vec<Vec<u16>> =
            (0..COLS).map(|s| if w.class[s] == ColClass::Soil { bfs_from(&w, s) } else { Vec::new() }).collect();
        for (p, cols) in w.patch_soil.iter().enumerate() {
            for &s in cols {
                for c in 0..COLS {
                    want[p * COLS + c] = want[p * COLS + c].min(from[s][c]);
                }
            }
        }
        for p in 0..PATCHES {
            prop_assert_eq!(&w.patch_dist[p * COLS..(p + 1) * COLS], &want[p * COLS..(p + 1) * COLS], "patch {}", p);
        }
        Ok(())
    }

    /// Solids are 0; air and water are 255 − absorb × (canopy voxels strictly above), clamped at 0.
    fn column_light_formula(
        heights: &[u8],
        x: usize,
        y: usize,
        canopy: &[u8],
        absorb: u8,
    ) -> Result<(), TestCaseError> {
        let mut w = World::from_heights(heights, &Params::load_default());
        let mut zs = canopy.to_vec();
        zs.sort_unstable();
        zs.dedup();
        w.set_column_light(x, y, &zs, absorb);
        for z in 0..WZ {
            let m = w.material[vidx(x, y, z)];
            let above = zs.iter().filter(|&&cz| cz as usize > z).count() as i32;
            let want = if m == SOIL || m == ROCK { 0 } else { (255 - absorb as i32 * above).max(0) as u8 };
            prop_assert_eq!(w.light[vidx(x, y, z)], want, "z {}", z);
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(8)))]

        #[test]
        fn prop_patch_dist_matches_brute_force(heights in terrain()) {
            patch_dist_matches_brute_force(&heights)?;
        }
    }

    proptest! {
        #[test]
        fn prop_column_light_formula(
            heights in terrain(),
            (x, y) in (0..WX, 0..WY),
            canopy in prop::collection::vec(0u8..WZ as u8, 0..6),
            absorb in prop_oneof![Just(Params::load_default().world.canopy_absorb), any::<u8>()],
        ) {
            column_light_formula(&heights, x, y, &canopy, absorb)?;
        }
    }

    #[test]
    fn patch_dist_regression_island_and_wall() {
        let mut h = vec![14u8; COLS];
        for y in 0..WY {
            h[cidx(30, y)] = 23; // a full-height rock wall cuts the world in two
        }
        h[cidx(5, 5)] = 7; // a pond
        patch_dist_matches_brute_force(&h).unwrap();
        let w = World::from_heights(&h, &Params::load_default());
        assert_eq!(w.dist_to_patch(0, cidx(40, 0)), UNREACHABLE);
    }

    #[test]
    fn column_light_regression_default_absorb_over_water() {
        let mut h = vec![14u8; COLS];
        h[cidx(3, 4)] = 7;
        column_light_formula(&h, 3, 4, &[12, 13, 13], 100).unwrap();
        column_light_formula(&h, 3, 4, &[20, 21, 22], 96).unwrap();
    }
}
