//! Voxel world: terrain generation, materials, column classes and the light field.

use crate::bundle::{Bundle, Ground, Medium, Pipe};
use crate::params::Params;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// Material code: air.
pub const AIR: u8 = 0;
/// Material code: soil.
pub const SOIL: u8 = 1;
/// Material code: rock.
pub const ROCK: u8 = 2;
/// Material code: water.
pub const WATER: u8 = 3;

/// World dimensions, from `[world]` `width`, `depth`, `height` and `patch` (validated when the
/// params load: width and depth are multiples of the patch size and at most 256).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dims {
    /// Columns along x.
    pub wx: usize,
    /// Columns along y.
    pub wy: usize,
    /// Voxels along z (up).
    pub wz: usize,
    /// Patch edge length in columns.
    pub patch: usize,
}

impl Dims {
    /// The dimensions the params describe.
    pub fn of(params: &Params) -> Dims {
        let w = &params.world;
        Dims { wx: w.width as usize, wy: w.depth as usize, wz: w.height as usize, patch: w.patch as usize }
    }

    /// Number of columns.
    #[inline]
    pub fn cols(&self) -> usize {
        self.wx * self.wy
    }

    /// Number of voxels.
    #[inline]
    pub fn voxels(&self) -> usize {
        self.wx * self.wy * self.wz
    }

    /// Patches along x.
    #[inline]
    pub fn patches_x(&self) -> usize {
        self.wx / self.patch
    }

    /// Patches along y.
    #[inline]
    pub fn patches_y(&self) -> usize {
        self.wy / self.patch
    }

    /// Number of patches.
    #[inline]
    pub fn patches(&self) -> usize {
        self.patches_x() * self.patches_y()
    }

    /// Voxel index, x fastest: x + wx·(y + wy·z).
    #[inline]
    pub fn vidx(&self, x: usize, y: usize, z: usize) -> usize {
        x + self.wx * (y + self.wy * z)
    }

    /// Column index: x + wx·y.
    #[inline]
    pub fn cidx(&self, x: usize, y: usize) -> usize {
        x + self.wx * y
    }

    /// Column coordinates of a column index.
    #[inline]
    pub fn xy(&self, c: usize) -> (usize, usize) {
        (c % self.wx, c / self.wx)
    }

    /// Patch index of a column: px + patches_x·py.
    #[inline]
    pub fn patch_of(&self, x: usize, y: usize) -> usize {
        x / self.patch + self.patches_x() * (y / self.patch)
    }

    /// Patch coordinates of a patch index.
    #[inline]
    pub fn patch_xy(&self, p: usize) -> (usize, usize) {
        (p % self.patches_x(), p / self.patches_x())
    }

    /// Whether (x, y) is inside the world.
    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.wx && (y as usize) < self.wy
    }

    /// Whether patch coordinates (px, py) are inside the world.
    #[inline]
    pub fn patch_in_bounds(&self, px: i32, py: i32) -> bool {
        px >= 0 && py >= 0 && (px as usize) < self.patches_x() && (py as usize) < self.patches_y()
    }
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
    /// The world's dimensions.
    pub dims: Dims,
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
    /// column of each patch: `patch_dist[p * cols + c]`, `UNREACHABLE` if there is no path.
    pub patch_dist: Vec<u16>,
    /// Lowest z per column that a building does **not** shade: voxels below it are dark whatever
    /// the canopy does. 0 (nothing shaded) in a noise world, which has no buildings.
    pub shade_top: Vec<u8>,
    /// The ground grid at full resolution: the bundle's, or a 1 m mirror of the ecology grid in a
    /// noise world (all soil, except its water columns).
    pub ground_grid: Ground,
    /// Ground height per ground cell, metres; the noise world's is its terrain height.
    pub ground_h: Vec<f32>,
    /// Building height above the ground per ground cell; all zero in a noise world.
    pub building_h: Vec<f32>,
    /// Whether any ground cell under the column is a `roof`: all false in a noise world, which has
    /// no buildings, and true in a bundle world for every column a building outline crosses without
    /// covering (shot G12). A roofed column may still be `Soil` — a lawn under the edge of an eave
    /// is still a lawn — but no trunk may root in it.
    pub roofed: Vec<bool>,
    /// Whether the world was loaded from a bundle (`ecosim run --world`).
    pub bundle_world: bool,
    /// The bundle's storm drains; empty in a noise world. Read by the drain network in shot G6.
    pub pipes: Vec<Pipe>,
}

/// What tops a column, beyond the soil-over-rock fill every column gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Top {
    /// Whatever the fill left: soil, unless `soil_depth` is 0.
    Terrain,
    /// A rock cap: the noise world's high ground, or a bundle column that is mostly sealed.
    Rock,
    /// A water surface: the noise world's flooding, or a bundle column that is mostly water.
    Water,
}

/// Marks a patch that cannot be walked to from a column.
pub const UNREACHABLE: u16 = u16::MAX;

/// Multi-source BFS over soil columns (8-neighbour moves) from every patch's soil columns.
fn patch_distances(d: Dims, class: &[ColClass], patch_soil: &[Vec<usize>]) -> Vec<u16> {
    let cols = d.cols();
    let mut dist = vec![UNREACHABLE; d.patches() * cols];
    let mut queue = std::collections::VecDeque::new();
    for (p, soil) in patch_soil.iter().enumerate() {
        let pd = &mut dist[p * cols..(p + 1) * cols];
        queue.clear();
        for &c in soil {
            pd[c] = 0;
            queue.push_back(c);
        }
        while let Some(c) = queue.pop_front() {
            let (x, y) = ((c % d.wx) as i32, (c / d.wx) as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx, dy) == (0, 0) || !d.in_bounds(nx, ny) {
                        continue;
                    }
                    let n = d.cidx(nx as usize, ny as usize);
                    if class[n] == ColClass::Soil && pd[n] == UNREACHABLE {
                        pd[n] = pd[c] + 1;
                        queue.push_back(n);
                    }
                }
            }
        }
    }
    dist
}

/// One octave of seeded 2D value noise over the world, lattice spacing `period` columns. The
/// lattice is drawn row by row, x fastest, so the 64×64 world draws what it always drew.
fn value_noise_octave(d: Dims, rng: &mut ChaCha8Rng, period: usize) -> Vec<f32> {
    let (nx, ny) = (d.wx.div_ceil(period) + 1, d.wy.div_ceil(period) + 1);
    let lattice: Vec<f32> = (0..nx * ny).map(|_| rng.gen::<f32>()).collect();
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let mut out = vec![0.0; d.cols()];
    for y in 0..d.wy {
        for x in 0..d.wx {
            let (gx, gy) = (x / period, y / period);
            let tx = smooth((x % period) as f32 / period as f32);
            let ty = smooth((y % period) as f32 / period as f32);
            let v = |i: usize, j: usize| lattice[i + nx * j];
            let a = v(gx, gy) + (v(gx + 1, gy) - v(gx, gy)) * tx;
            let b = v(gx, gy + 1) + (v(gx + 1, gy + 1) - v(gx, gy + 1)) * tx;
            out[d.cidx(x, y)] = a + (b - a) * ty;
        }
    }
    out
}

/// The west–east ramp at column x: 2x/(width − 1) − 1, so −1 on the west edge (x = 0) and +1 on
/// the east edge (x = width − 1), linear between. The rain gradient scales it (wetter east) and
/// the slope bias scales its negation (higher west).
#[inline]
pub fn west_east(x: usize, width: usize) -> f32 {
    2.0 * x as f32 / (width.max(2) - 1) as f32 - 1.0
}

/// Heightmap from 2 octaves of value noise (periods 32 and 8, amplitudes 1.0 and 0.35).
/// Normalization is piecewise-linear so that the lowest `water_fraction` of columns lands below
/// the water line: [min, q] → [height_min, water_level − 0.5], [q, max] → [water_level − 0.5, height_max],
/// where q is the `water_fraction` quantile of the noise. A nonzero `world.slope_bias` then adds
/// `slope_bias × (1 − 2x/(width − 1))` (high ground on the dry west edge) before rounding and clamping.
pub fn generate_heights(params: &Params, rng: &mut ChaCha8Rng) -> Vec<u8> {
    let d = Dims::of(params);
    let cols = d.cols();
    let o1 = value_noise_octave(d, rng, 32);
    let o2 = value_noise_octave(d, rng, 8);
    let v: Vec<f32> = o1.iter().zip(&o2).map(|(a, b)| a + 0.35 * b).collect();
    let mut sorted = v.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let lo = sorted[0];
    let hi = sorted[cols - 1];
    let qi = ((params.world.water_fraction * cols as f32) as usize).clamp(1, cols - 2);
    let q = sorted[qi].clamp(lo + 1e-6, hi - 1e-6);
    let wp = &params.world;
    let (hmin, hmax, hw) = (wp.height_min as f32, wp.height_max as f32, wp.water_level as f32 - 0.5);
    let slope = wp.slope_bias;
    v.iter()
        .enumerate()
        .map(|(c, &x)| {
            let mut h =
                if x < q { hmin + (x - lo) / (q - lo) * (hw - hmin) } else { hw + (x - q) / (hi - q) * (hmax - hw) };
            if slope != 0.0 {
                h -= slope * west_east(c % d.wx, d.wx);
            }
            h.round().clamp(hmin, hmax) as u8
        })
        .collect()
}

/// Light of a voxel in full sun. The field holds `FULL_SUN x` the fraction of full sun that
/// reaches a voxel (shot G4c), so it is still the 0-255 byte `light.bin` has always held.
pub const FULL_SUN: u8 = 255;

/// Shade cast by roofs, as `World::shade_top`.
///
/// The sun sits due south at a fixed altitude, so a roof whose top is at absolute z `t` darkens the
/// columns north of it: `dy` columns away, the grazing ray is at `t − dy / shade_slope`, and every
/// voxel at or below it is dark, so the shadow ends after `shade_slope × height` columns.
/// `shade_slope = 1` is a 45° sun. The roof's own column is dark to its full height, so a building
/// is opaque where it stands. At 0 nothing is shaded and the field stays all zeroes, as it is in a
/// noise world.
fn building_shade(d: Dims, heights: &[u8], building: &[f32], shade_slope: f32) -> Vec<u8> {
    let mut shade = vec![0u8; d.cols()];
    if shade_slope.is_nan() || shade_slope <= 0.0 {
        return shade;
    }
    let top_z = (d.wz - 1) as f32;
    for y in 0..d.wy {
        for x in 0..d.wx {
            let bh = building[d.cidx(x, y)];
            if bh <= 0.0 {
                continue;
            }
            let top = heights[d.cidx(x, y)] as f32 + bh;
            let reach = (bh * shade_slope).floor().min(d.wy as f32) as usize;
            for dy in 0..=reach {
                if y + dy >= d.wy {
                    break;
                }
                let blocked = ((top - dy as f32 / shade_slope).floor() + 1.0).clamp(0.0, top_z) as u8;
                let t = d.cidx(x, y + dy);
                shade[t] = shade[t].max(blocked);
            }
        }
    }
    shade
}

impl World {
    /// Generate terrain from the seeded RNG and build the world.
    pub fn generate(params: &Params, rng: &mut ChaCha8Rng) -> World {
        let heights = generate_heights(params, rng);
        World::from_heights(&heights, params)
    }

    /// Build materials, column classes and initial (canopy-free) light from terrain heights, in the
    /// noise world's way: a rock cap at `rock_top_height` and flooding up to `water_level`.
    pub fn from_heights(heights: &[u8], params: &Params) -> World {
        World::build(heights, params, None, vec![0; Dims::of(params).cols()])
    }

    /// The shared builder. `tops` replaces the noise world's height rules with an explicit top per
    /// column (a bundle world: media decide, not heights), and `shade_top` is the building shade.
    fn build(heights: &[u8], params: &Params, tops: Option<&[Top]>, shade_top: Vec<u8>) -> World {
        let d = Dims::of(params);
        assert_eq!(heights.len(), d.cols());
        assert_eq!(shade_top.len(), d.cols());
        let wp = &params.world;
        let mut material = vec![AIR; d.voxels()];
        let mut height = vec![0u8; d.cols()];
        let mut class = vec![ColClass::Rock; d.cols()];
        let top_z = (d.wz - 1) as u8;
        for y in 0..d.wy {
            for x in 0..d.wx {
                let c = d.cidx(x, y);
                let h = heights[c].min(top_z) as usize;
                for z in 0..=h {
                    let soil_layer = z + wp.soil_depth as usize > h;
                    material[d.vidx(x, y, z)] = if soil_layer { SOIL } else { ROCK };
                }
                match tops {
                    None => {
                        if h >= wp.rock_top_height as usize {
                            material[d.vidx(x, y, h)] = ROCK;
                        }
                        let wl = wp.water_level as usize;
                        for z in (h + 1)..=wl.min(d.wz - 1) {
                            material[d.vidx(x, y, z)] = WATER;
                        }
                    }
                    Some(t) => match t[c] {
                        Top::Terrain => {}
                        Top::Rock => material[d.vidx(x, y, h)] = ROCK,
                        Top::Water => material[d.vidx(x, y, h)] = WATER,
                    },
                }
                let top = (0..d.wz).rev().find(|&z| material[d.vidx(x, y, z)] != AIR).unwrap_or(0);
                height[c] = top as u8;
                class[c] = match material[d.vidx(x, y, top)] {
                    SOIL => ColClass::Soil,
                    WATER => ColClass::Water,
                    _ => ColClass::Rock,
                };
            }
        }
        let mut patch_soil = vec![Vec::new(); d.patches()];
        for y in 0..d.wy {
            for x in 0..d.wx {
                if class[d.cidx(x, y)] == ColClass::Soil {
                    patch_soil[d.patch_of(x, y)].push(d.cidx(x, y));
                }
            }
        }
        // Sort each patch's list so iteration is ascending by column index.
        for p in patch_soil.iter_mut() {
            p.sort_unstable();
        }
        let patch_dist = patch_distances(d, &class, &patch_soil);
        let mut w = World {
            dims: d,
            material,
            light: vec![0; d.voxels()],
            height,
            ground: heights.iter().map(|&h| h.min(top_z)).collect(),
            class,
            patch_soil,
            patch_dist,
            shade_top,
            ground_grid: Ground { width: d.wx, depth: d.wy, ratio: 1, medium: Vec::new(), media: Medium::ALL.to_vec() },
            ground_h: Vec::new(),
            building_h: vec![0.0; d.cols()],
            roofed: vec![false; d.cols()],
            bundle_world: false,
            pipes: Vec::new(),
        };
        // The noise world's ground grid mirrors the ecology grid: one 1 m cell per column, soil
        // everywhere but the water columns, and the terrain height as the ground height.
        w.ground_grid.medium =
            w.class.iter().map(|&k| if k == ColClass::Water { Medium::Water } else { Medium::Soil } as u8).collect();
        w.ground_h = w.ground.iter().map(|&h| h as f32).collect();
        for y in 0..d.wy {
            for x in 0..d.wx {
                w.set_column_light(x, y, &[], params.canopy_extinction());
            }
        }
        w
    }

    /// Build the world a bundle describes (`ecosim run --world`).
    ///
    /// A column's surface layer is `[bundle] base_z + round(the mean ground height of the ground
    /// cells under it)`, filled below exactly as the noise world fills terrain. The media decide
    /// what tops it: Rock when more than half its ground cells are sealed (`roof`, `asphalt`,
    /// `concrete`), otherwise Water when more than half are `water`, otherwise soil. A tie is
    /// neither. Roofs then cast shade (`building_shade`), and every column a roof cell touches at
    /// all is marked `roofed`, which keeps trunks out of it without changing what it is made of
    /// (shot G12). The bundle's dimensions must already be in `params` (`Bundle::apply_to`).
    pub fn from_bundle(b: &Bundle, params: &Params) -> Result<World, String> {
        let d = Dims::of(params);
        if (d.wx, d.wy) != (b.size_m, b.size_m) {
            return Err(format!(
                "{}: the bundle is {} m across but [world] is {}×{} columns",
                b.dir.display(),
                b.size_m,
                d.wx,
                d.wy
            ));
        }
        let (base, top_z, n) = (params.bundle.base_z as i32, (d.wz - 1) as i32, b.cells_per_column());
        let mut heights = vec![0u8; d.cols()];
        let mut tops = vec![Top::Terrain; d.cols()];
        let mut building = vec![0f32; d.cols()];
        let mut roofed = vec![false; d.cols()];
        for y in 0..d.wy {
            for x in 0..d.wx {
                let (mut sum, mut sealed, mut water, mut roof) = (0.0f32, 0usize, 0usize, 0.0f32);
                let mut roofs = 0usize;
                for i in b.ground.cells_of(x, y) {
                    sum += b.ground_h[i];
                    let m = b.ground.medium_at(i);
                    sealed += (m != Medium::Water && !params.medium.get(m).plantable) as usize;
                    roofs += (m == Medium::Roof) as usize;
                    water += (m == Medium::Water) as usize;
                    roof = roof.max(b.building_h[i]);
                }
                let h = base + (sum / n as f32).round() as i32;
                if !(0..=top_z).contains(&h) {
                    return Err(format!(
                        "{}: column ({x}, {y}) has surface layer {h}, which does not fit under [world] height = {}",
                        b.dir.display(),
                        d.wz
                    ));
                }
                let c = d.cidx(x, y);
                heights[c] = h as u8;
                tops[c] = if 2 * sealed > n {
                    Top::Rock
                } else if 2 * water > n {
                    Top::Water
                } else {
                    Top::Terrain
                };
                building[c] = roof;
                roofed[c] = roofs > 0;
            }
        }
        let shade = building_shade(d, &heights, &building, params.shade_slope());
        let mut w = World::build(&heights, params, Some(&tops), shade);
        w.ground_grid = b.ground.clone();
        w.ground_h = b.ground_h.clone();
        w.building_h = b.building_h.clone();
        w.roofed = roofed;
        w.bundle_world = true;
        w.pipes = b.pipes.clone();
        Ok(w)
    }

    /// Whether column `c` can hold a plant: its top is soil, so it is neither sealed (Rock) nor
    /// under water. Trees, shrub cover and grass all start on plantable columns only (shot G3).
    #[inline]
    pub fn is_plantable(&self, c: usize) -> bool {
        self.class[c] == ColClass::Soil
    }

    /// Whether a tree trunk may root in column `c`: the column is plantable **and** no roof covers
    /// any part of it. Grass and shrub cover ask only [`World::is_plantable`], because a lawn under
    /// the edge of an eave is still a lawn; a *trunk* under one is a tree growing through a
    /// building, which `ecosim check`'s `tree_footing` forbids outright (shot G12).
    #[inline]
    pub fn can_root_a_trunk(&self, c: usize) -> bool {
        self.is_plantable(c) && !self.roofed[c]
    }

    /// [`World::can_root_a_trunk`] at (x, y), false outside the world.
    #[inline]
    pub fn trunk_site_ok(&self, x: i32, y: i32) -> bool {
        self.dims.in_bounds(x, y) && self.can_root_a_trunk(self.dims.cidx(x as usize, y as usize))
    }

    /// Whether (x, y) is inside the world and a soil column.
    #[inline]
    pub fn is_soil(&self, x: i32, y: i32) -> bool {
        self.dims.in_bounds(x, y) && self.class[self.dims.cidx(x as usize, y as usize)] == ColClass::Soil
    }

    /// Light of the voxel directly above the column's surface.
    #[inline]
    pub fn surface_light(&self, c: usize) -> u8 {
        let (x, y) = self.dims.xy(c);
        let z = (self.height[c] as usize + 1).min(self.dims.wz - 1);
        self.light[self.dims.vidx(x, y, z)]
    }

    /// Surface light as a fraction of full sun, which is what every light curve is in since shot
    /// G4c.
    #[inline]
    pub fn surface_light_fraction(&self, c: usize) -> f32 {
        self.surface_light(c) as f32 / FULL_SUN as f32
    }

    /// Recompute one column's light: solids are 0, and so is anything a building shades
    /// (`shade_top`, always 0 in a noise world); the rest of the air and water gets
    /// [`FULL_SUN`] × exp(−`extinction` × canopy voxels strictly above), the Beer–Lambert
    /// transmittance of that many layers of canopy (shot G4c; `extinction` is
    /// [`Params::canopy_extinction`](crate::params::Params::canopy_extinction), the optical depth
    /// of one layer). `canopy_z` lists the distinct canopy voxel z values in this column.
    pub fn set_column_light(&mut self, x: usize, y: usize, canopy_z: &[u8], extinction: f32) {
        let shade = self.shade_top[self.dims.cidx(x, y)] as usize;
        for z in 0..self.dims.wz {
            let i = self.dims.vidx(x, y, z);
            let m = self.material[i];
            self.light[i] = if m == SOIL || m == ROCK || z < shade {
                0
            } else {
                let above = canopy_z.iter().filter(|&&cz| cz as usize > z).count() as f32;
                (FULL_SUN as f32 * libm::expf(-extinction * above)).round() as u8
            };
        }
    }

    /// Walking distance from column `c` to patch `p` (see `patch_dist`).
    #[inline]
    pub fn dist_to_patch(&self, p: usize, c: usize) -> u16 {
        self.patch_dist[p * self.dims.cols() + c]
    }
}

/// The 64×64×32 world with 8×8 patches that the unit tests use (`Params::load_square`), and index
/// helpers for it.
#[cfg(test)]
pub(crate) mod sq {
    use super::Dims;

    /// The square test world.
    pub(crate) const D: Dims = Dims { wx: 64, wy: 64, wz: 32, patch: 8 };
    pub(crate) const WX: usize = 64;
    pub(crate) const WY: usize = 64;
    pub(crate) const WZ: usize = 32;
    pub(crate) const COLS: usize = WX * WY;
    pub(crate) const PATCHES_X: usize = 8;
    pub(crate) const PATCHES: usize = 64;

    pub(crate) fn vidx(x: usize, y: usize, z: usize) -> usize {
        D.vidx(x, y, z)
    }
    pub(crate) fn cidx(x: usize, y: usize) -> usize {
        D.cidx(x, y)
    }
    pub(crate) fn patch_of(x: usize, y: usize) -> usize {
        D.patch_of(x, y)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::sq::{cidx, vidx, COLS, WX, WY, WZ};
    use super::*;
    use proptest::prelude::*;
    use rand::SeedableRng;

    fn flat(h: u8) -> World {
        World::from_heights(&vec![h; COLS], &Params::load_square())
    }

    /// One canopy voxel at the shipped optical depth (k 0.5 x LAI 2 = 1.0 per layer) passes
    /// exp(-1) = 36.8% of full sun, two pass 13.5% and three 5.0%: Beer-Lambert, so light thins
    /// towards zero instead of reaching it (shot G4c).
    #[test]
    fn light_column_with_one_canopy_voxel() {
        let mut w = flat(12);
        let k = Params::load_square().canopy_extinction();
        assert_eq!(k, 1.0, "the shipped canopy_k x canopy_lai");
        w.set_column_light(5, 5, &[15], k);
        assert_eq!(w.light[vidx(5, 5, 20)], 255);
        assert_eq!(w.light[vidx(5, 5, 15)], 255, "canopy voxel itself is not below itself");
        assert_eq!(w.light[vidx(5, 5, 14)], 94);
        assert_eq!(w.light[vidx(5, 5, 13)], 94);
        assert_eq!(w.light[vidx(5, 5, 12)], 0, "solid ground is 0");
        assert_eq!(w.surface_light(cidx(5, 5)), 94);
        assert!((w.surface_light_fraction(cidx(5, 5)) - 0.3686).abs() < 1e-3);
        // A neighbouring column is untouched.
        assert_eq!(w.surface_light(cidx(6, 5)), 255);
        // Three canopy voxels are dark but not black.
        w.set_column_light(5, 5, &[15, 16, 17], k);
        assert_eq!(w.surface_light(cidx(5, 5)), 13);
    }

    /// The acceptance line of shot G4c: a column under a closed canopy is inside the published
    /// transmittance band for its leaf area index (10-25% at LAI 3-5, UNITS.md R10). A mature sim
    /// crown is two canopy voxels, so its LAI is 2 x `canopy_lai` and its transmittance
    /// exp(-canopy_k x LAI).
    #[test]
    fn closed_canopy_transmittance_is_inside_the_published_band() {
        let p = Params::load_square();
        let lai = 2.0 * p.world.canopy_lai;
        assert!((3.0..=5.0).contains(&lai), "a mature crown's LAI is {lai}");
        assert!((0.4..=0.7).contains(&p.world.canopy_k), "k is {}", p.world.canopy_k);
        let mut w = flat(12);
        w.set_column_light(5, 5, &[15, 16], p.canopy_extinction());
        let t = w.surface_light_fraction(cidx(5, 5));
        assert!((0.10..=0.25).contains(&t), "a closed canopy passes {t} of full sun");
    }

    #[test]
    fn materials_and_classes() {
        let mut h = vec![12u8; COLS];
        h[cidx(0, 0)] = 8; // under water level 10
        h[cidx(1, 0)] = 22; // rock-topped
        let w = World::from_heights(&h, &Params::load_square());
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
        let w = World::from_heights(&h, &Params::load_square());
        assert_eq!(w.dist_to_patch(0, cidx(3, 3)), 0);
        assert_eq!(w.dist_to_patch(0, cidx(10, 3)), 3);
        // From x = 13 the way west is blocked: 18 steps to get round the wall's end at y = 21,
        // then 14 back up to patch 0 (x < 8, y < 8).
        assert_eq!(w.dist_to_patch(0, cidx(13, 3)), 18 + 14);
        assert_eq!(w.dist_to_patch(0, cidx(12, 3)), UNREACHABLE, "rock column itself");
    }

    /// Square test params resized to `wx` × `wy` columns (patch 8).
    pub(crate) fn sized(wx: u32, wy: u32) -> Params {
        let mut p = Params::load_square();
        (p.world.width, p.world.depth) = (wx, wy);
        p
    }

    /// Mostly soil, with water (below level 10) and rock-topped (≥ 21) columns mixed in.
    pub(crate) fn terrain_of(cols: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(prop_oneof![6 => 11u8..=20, 1 => 6u8..=9, 1 => 21u8..=24], cols)
    }

    /// `terrain_of` the square test world.
    pub(crate) fn terrain() -> impl Strategy<Value = Vec<u8>> {
        terrain_of(COLS)
    }

    /// Single-source BFS over soil columns (8-neighbour steps) from column `s`.
    fn bfs_from(w: &World, s: usize) -> Vec<u16> {
        let d = w.dims;
        let mut dist = vec![UNREACHABLE; d.cols()];
        let mut q = std::collections::VecDeque::from([s]);
        dist[s] = 0;
        while let Some(c) = q.pop_front() {
            let (x, y) = ((c % d.wx) as i32, (c / d.wx) as i32);
            for (nx, ny) in (y - 1..=y + 1).flat_map(|ny| (x - 1..=x + 1).map(move |nx| (nx, ny))) {
                if w.is_soil(nx, ny) && dist[d.cidx(nx as usize, ny as usize)] == UNREACHABLE {
                    dist[d.cidx(nx as usize, ny as usize)] = dist[c] + 1;
                    q.push_back(d.cidx(nx as usize, ny as usize));
                }
            }
        }
        dist
    }

    /// `patch_dist` equals, for every `step`-th patch, the minimum over its soil columns of a
    /// single-source BFS.
    fn patch_dist_matches_brute_force_in(w: &World, step: usize) -> Result<(), TestCaseError> {
        let cols = w.dims.cols();
        for p in (0..w.dims.patches()).step_by(step) {
            let mut want = vec![UNREACHABLE; cols];
            for &s in &w.patch_soil[p] {
                for (m, d) in want.iter_mut().zip(bfs_from(w, s)) {
                    *m = (*m).min(d);
                }
            }
            prop_assert_eq!(&w.patch_dist[p * cols..(p + 1) * cols], &want[..], "patch {}", p);
        }
        Ok(())
    }

    fn patch_dist_matches_brute_force(heights: &[u8]) -> Result<(), TestCaseError> {
        patch_dist_matches_brute_force_in(&World::from_heights(heights, &Params::load_square()), 1)
    }

    /// Solids are 0; air and water are `FULL_SUN × exp(−extinction × canopy voxels strictly
    /// above)`, rounded (shot G4c).
    fn column_light_formula(
        heights: &[u8],
        x: usize,
        y: usize,
        canopy: &[u8],
        extinction: f32,
    ) -> Result<(), TestCaseError> {
        let mut w = World::from_heights(heights, &Params::load_square());
        let mut zs = canopy.to_vec();
        zs.sort_unstable();
        zs.dedup();
        w.set_column_light(x, y, &zs, extinction);
        for z in 0..WZ {
            let m = w.material[vidx(x, y, z)];
            let above = zs.iter().filter(|&&cz| cz as usize > z).count() as f32;
            let want = if m == SOIL || m == ROCK {
                0
            } else {
                (FULL_SUN as f32 * libm::expf(-extinction * above)).round() as u8
            };
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

        /// The same on a 4:1 strip (128 × 32, 16 × 4 patches): row-major patch and column indices
        /// with a width that is not the depth.
        #[test]
        fn prop_patch_dist_matches_brute_force_on_a_strip(heights in terrain_of(128 * 32)) {
            patch_dist_matches_brute_force_in(&World::from_heights(&heights, &sized(128, 32)), 1)?;
        }
    }

    proptest! {
        #[test]
        fn prop_column_light_formula(
            heights in terrain(),
            (x, y) in (0..WX, 0..WY),
            canopy in prop::collection::vec(0u8..WZ as u8, 0..6),
            extinction in prop_oneof![Just(Params::load_square().canopy_extinction()), 0.0f32..8.0],
        ) {
            column_light_formula(&heights, x, y, &canopy, extinction)?;
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
        let w = World::from_heights(&h, &Params::load_square());
        assert_eq!(w.dist_to_patch(0, cidx(40, 0)), UNREACHABLE);
    }

    /// Regression sibling on the reference strip itself (the default params, seed 42's terrain with
    /// its slope bias): every 7th of its patches, brute force.
    #[test]
    fn patch_dist_regression_reference_strip() {
        let p = Params::load_default();
        let w = World::generate(&p, &mut ChaCha8Rng::seed_from_u64(42));
        assert_eq!((w.dims.wx, w.dims.wy, w.dims.patches()), (256, 64, 256));
        patch_dist_matches_brute_force_in(&w, 7).unwrap();
    }

    #[test]
    fn column_light_regression_default_absorb_over_water() {
        let mut h = vec![14u8; COLS];
        h[cidx(3, 4)] = 7;
        let k = Params::load_square().canopy_extinction();
        column_light_formula(&h, 3, 4, &[12, 13, 13], k).unwrap();
        column_light_formula(&h, 3, 4, &[20, 21, 22], 0.35).unwrap();
    }

    /// The fixed sun is a parameter in degrees since shot G4c, and 45° is the old `shade_slope`
    /// of 1.0 to the bit, so no building's shadow moved when the unit changed.
    #[test]
    fn sun_altitude_45_is_the_old_shade_slope_of_one() {
        let mut p = Params::load_square();
        assert_eq!(p.bundle.sun_altitude_deg, 45.0);
        assert_eq!(p.shade_slope(), 1.0);
        for (deg, want) in [(0.0, 0.0), (90.0, 0.0), (180.0, 0.0), (-10.0, 0.0), (60.0, 0.57735026)] {
            p.bundle.sun_altitude_deg = deg;
            assert_eq!(p.shade_slope(), want, "{deg} degrees");
        }
    }

    /// Row-major index helpers on a strip: every column and voxel index is hit once, and each
    /// column's patch holds it.
    #[test]
    fn strip_indices_are_row_major_and_cover_the_world() {
        let d = Dims { wx: 256, wy: 64, wz: 32, patch: 8 };
        assert_eq!((d.cols(), d.voxels(), d.patches_x(), d.patches_y(), d.patches()), (16384, 524288, 32, 8, 256));
        let mut seen = vec![false; d.cols()];
        for y in 0..d.wy {
            for x in 0..d.wx {
                let c = d.cidx(x, y);
                assert!(!seen[c]);
                seen[c] = true;
                assert_eq!(d.xy(c), (x, y));
                let (px, py) = d.patch_xy(d.patch_of(x, y));
                assert_eq!((px, py), (x / 8, y / 8));
            }
        }
        assert_eq!(d.vidx(255, 63, 31), d.voxels() - 1);
        assert!(d.in_bounds(255, 63) && !d.in_bounds(256, 0) && !d.in_bounds(0, 64));
    }

    /// The slope bias raises the west edge and lowers the east edge by `slope_bias` before
    /// rounding, and leaves the terrain alone at 0.
    #[test]
    fn slope_bias_tilts_the_terrain_west_up() {
        let mut p = sized(128, 32);
        let flat = generate_heights(&p, &mut ChaCha8Rng::seed_from_u64(5));
        p.world.slope_bias = 4.0;
        let tilted = generate_heights(&p, &mut ChaCha8Rng::seed_from_u64(5));
        let wp = &p.world;
        for (c, (&a, &b)) in flat.iter().zip(&tilted).enumerate() {
            let want =
                (a as f32 - 4.0 * west_east(c % 128, 128)).round().clamp(wp.height_min as f32, wp.height_max as f32);
            assert!((b as f32 - want).abs() <= 1.0, "column {c}: {a} → {b}, want about {want}");
        }
        let mean = |h: &[u8], x0: usize| (0..32).map(|y| h[x0 + 128 * y] as f32).sum::<f32>() / 32.0;
        assert!(mean(&tilted, 0) > mean(&flat, 0) && mean(&tilted, 127) < mean(&flat, 127));
    }
}
