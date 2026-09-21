//! Ground cover and vines: **expression, not simulation**.
//!
//! Shot V4. The simulator keeps grass and shrub as two fractions per 8 m patch and moisture and
//! light as two bytes per 1 m ecology column. This module turns those four numbers into the drivers
//! the viewer needs to scatter blades over the ground and run climbers up a wall, at whatever cube
//! edge the bundle uses. Nothing here is ecology.
//!
//! **What a vine is and is not** (`overnight/DIRECTION-native-viewer.md`): a vine is a deterministic
//! response to the simulator's moisture, shade and cover at the wall's own base column. It has no
//! biomass, does not compete for light, is not an entity in any run, and feeds nothing back into
//! the simulation. Any report or screenshot that shows one says so. The same is true of *where* a
//! blade of grass stands: the simulator says a patch is 64% grass, and which of that patch's 256
//! ground cells carry a blade is this viewer's draw from the run's seed.
//!
//! | Driver | Source | Grid |
//! |---|---|---|
//! | grass, shrub | `patches.json`, the fraction of the patch the simulator clamps to [0, 1] | 8 m patch |
//! | moisture | `moisture.bin`, 255 x soil water / available water capacity | 1 m column |
//! | shade | `1 -` `light.bin` at the column's surface voxel, the simulator's own sample | 1 m column |
//!
//! Like the rest of the library half this module never mentions Bevy.

use crate::overlay::{Fields, FULL_SUN};
use crate::run::Dims;
use crate::tree::mix;

/// Metres of wall a vine at full vigour covers. **The viewer's number, not the simulator's**: the
/// run says nothing about climbers. A vine is drawn as a height rather than as a fraction of its
/// wall so that a garden fence disappears under one and the Capitol's dome does not.
pub const VINE_REACH_M: f32 = 12.0;
/// The fraction of available water capacity at which a vine is no longer short of water. Also the
/// viewer's: the Capitol's mean soil water is 0.22 of capacity, so a saturation at 1.0 would make
/// every wall bare and one at 0.1 would make the driver constant.
pub const VINE_WATER_SAT: f32 = 0.35;
/// What a wall in **full sun** keeps of its vigour. A shaded damp wall is the picture the direction
/// asks for, and a sunny one still grows something, so shade is a factor and not a gate.
pub const VINE_SUN_FLOOR: f32 = 0.35;
/// Below this a wall grows nothing at all, rather than a one-voxel fringe along every building.
pub const VINE_MIN_VIGOUR: f32 = 0.05;
/// How far one cell's own draw moves its climb, either way, so the top edge of a vine is ragged.
pub const VINE_JITTER: f32 = 0.25;
/// How tall a shrub is drawn. The simulator has no shrub height -- shrub is a fraction of a patch --
/// so this is the viewer's, and it is the one number here a user would call a plant's size.
pub const SHRUB_HEIGHT_M: f32 = 1.2;

/// Independent streams off the run's seed, so moving a blade never moves a vine.
const SALT_COVER: u64 = 0xc0de_0000_0000_0001;
const SALT_VINE: u64 = 0x_11fe_0000_0000_0002;

/// A number in `[0, 1)` for one ground cell, fixed by the run's seed and the cell's position. Exact
/// in binary -- 24 bits over 2^24 -- for the reason `tree.rs` gives: the goldens are hashed on
/// Windows and asserted on Linux CI.
pub fn draw(seed: u64, x: usize, y: usize, salt: u64) -> f32 {
    let k = mix(seed ^ salt, (x as u64) | ((y as u64) << 32));
    (k >> 40) as f32 / 16_777_216.0
}

/// What the simulator said about one ecology column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnCover {
    /// Fraction of the column's patch under grass, and under shrub. The simulator clamps both to
    /// `[0, 1]` itself (`ecosim/src/producers.rs`), so the scale is its own and not a choice here.
    pub grass: f32,
    pub shrub: f32,
    /// Soil water as a fraction of available water capacity.
    pub water: f32,
    /// `1 -` the fraction of full sun reaching the column's surface.
    pub shade: f32,
}

/// The means of the four drivers over the site, and of the vigour they produce. Printed beside any
/// picture with a vine in it, because a viewer that draws climbers without saying what drove them
/// has shown you a texture rather than a simulation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CoverStats {
    pub grass: f32,
    pub shrub: f32,
    pub water: f32,
    pub shade: f32,
    pub vigour: f32,
}

/// One snapshot's four drivers, per ecology column, plus the seed that scatters them.
#[derive(Debug, Clone)]
pub struct Cover {
    pub dims: Dims,
    pub seed: u64,
    /// Draw blades on the ground? A **field overlay turns this off**: the overlay is a map of the
    /// ground and 64% grass cover would be a map of the grass (DECISIONS.md, V4).
    pub ground: bool,
    /// Draw vines on the walls? Left on under an overlay, because no overlay colours a wall and the
    /// moisture and light maps are exactly what explains where the vines are.
    pub vines: bool,
    grass: Vec<f32>,
    shrub: Vec<f32>,
    water: Vec<f32>,
    shade: Vec<f32>,
}

impl Cover {
    /// Resamples one snapshot's fields onto the ecology columns. Patch fields are spread over the
    /// patch's columns, which claims no resolution the simulator does not have: a patch is one
    /// number and every column in it gets that number.
    pub fn of(f: &Fields, dims: Dims, seed: u64) -> Cover {
        let n = dims.columns();
        let mut c = Cover {
            dims,
            seed,
            ground: true,
            vines: true,
            grass: vec![0.0; n],
            shrub: vec![0.0; n],
            water: vec![0.0; n],
            shade: vec![0.0; n],
        };
        for y in 0..dims.y {
            for x in 0..dims.x {
                let i = x + dims.x * y;
                let p = dims.patch_of(x, y);
                c.grass[i] = f.grass.get(p).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                c.shrub[i] = f.shrub.get(p).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                c.water[i] = f.moisture.get(i).map_or(0.0, |v| *v as f32 / 255.0);
                c.shade[i] = 1.0 - f.light.get(i).map_or(1.0, |v| *v as f32 / FULL_SUN);
            }
        }
        c
    }

    /// The drivers at one ecology column; all zero off the grid.
    pub fn at(&self, x: usize, y: usize) -> ColumnCover {
        if x >= self.dims.x || y >= self.dims.y {
            return ColumnCover {
                grass: 0.0,
                shrub: 0.0,
                water: 0.0,
                shade: 0.0,
            };
        }
        let i = x + self.dims.x * y;
        ColumnCover {
            grass: self.grass[i],
            shrub: self.shrub[i],
            water: self.water[i],
            shade: self.shade[i],
        }
    }

    /// How hard a wall rooted at this column is climbed, in `[0, 1]`.
    ///
    /// Cover **gates** it and water and shade drive it: a wall standing in asphalt grows nothing
    /// however wet it is, which is the response the user asked to be able to see by re-paving the
    /// ground. The three constants above are this viewer's; the three inputs are the run's.
    pub fn vine_vigour(&self, x: usize, y: usize) -> f32 {
        let c = self.at(x, y);
        let cover = (c.grass + c.shrub).clamp(0.0, 1.0);
        let water = (c.water / VINE_WATER_SAT).clamp(0.0, 1.0);
        let sun = VINE_SUN_FLOOR + (1.0 - VINE_SUN_FLOOR) * c.shade.clamp(0.0, 1.0);
        cover * water * sun
    }

    /// Site means of the four drivers and of the vigour, for the HUD and the write-up.
    pub fn means(&self) -> CoverStats {
        let n = self.dims.columns().max(1);
        let mean = |v: &[f32]| v.iter().map(|x| *x as f64).sum::<f64>() / n as f64;
        let mut vig = 0.0f64;
        for y in 0..self.dims.y {
            for x in 0..self.dims.x {
                vig += self.vine_vigour(x, y) as f64;
            }
        }
        CoverStats {
            grass: mean(&self.grass) as f32,
            shrub: mean(&self.shrub) as f32,
            water: mean(&self.water) as f32,
            shade: mean(&self.shade) as f32,
            vigour: (vig / n as f64) as f32,
        }
    }

    /// The draw that decides what one ground cell carries.
    ///
    /// **One uniform, partitioned**, rather than a draw per species: the shrub fraction takes the
    /// bottom of the interval and grass takes what is left under it, so the realised fractions are
    /// the simulator's exactly, and where the two sum past 1 it is grass that loses the cell. That
    /// is the same direction as the simulator's own `cover.grass_suppression`, arrived at here
    /// because a cell cannot hold two plants.
    pub fn cell(&self, gx: usize, gy: usize, ex: usize, ey: usize) -> Option<CellPlant> {
        let c = self.at(ex, ey);
        let u = draw(self.seed, gx, gy, SALT_COVER);
        if u < c.shrub {
            Some(CellPlant::Shrub)
        } else if u < c.shrub + c.grass {
            Some(CellPlant::Grass)
        } else {
            None
        }
    }

    /// How many levels of wall one cell of vine climbs, given the cell it is rooted in.
    pub fn vine_levels(&self, gx: usize, gy: usize, ex: usize, ey: usize, cell_m: f32) -> i32 {
        let vigour = self.vine_vigour(ex, ey);
        if vigour < VINE_MIN_VIGOUR {
            return 0;
        }
        let j = 1.0 + VINE_JITTER * (2.0 * draw(self.seed, gx, gy, SALT_VINE) - 1.0);
        ((vigour * j * VINE_REACH_M) / cell_m.max(1e-3)).round() as i32
    }
}

/// What one ground cell carries, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellPlant {
    Grass,
    Shrub,
}
