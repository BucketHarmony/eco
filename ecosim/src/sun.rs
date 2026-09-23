//! The sun that moves (shot G9): a light budget per column over the day and the year.
//!
//! Before G9 a building's shadow came from a fixed sun due south at 45°, so it was permanent and
//! absolute: every voxel under it was black all year. This replaces it with what a column actually
//! gets. The year is cut into `sun.season_samples` slices, each centred on one day; on that day the
//! sun is stepped through `sun.day_samples` positions between its own sunrise and sunset at the
//! site's latitude, and a ray towards each position is walked over the ground grid's building
//! heights. A column's light for the slice is
//!
//! `255 × (diffuse_fraction × sky_view + (1 − diffuse_fraction) × (1 − cloud_cover) × beam_fraction)`
//!
//! times the share of the column no roof covers, where `beam_fraction` is the share of the sampled
//! positions that reach it weighted by the sine of the sun's altitude (the light a level surface
//! takes from a beam at that altitude), and `sky_view` is the share of an evenly bright sky the
//! buildings leave open. Nothing is random: the budget is computed once, at load, from the world.
//!
//! The simulator's light field stays a fraction of the open sky's light, which is what every light
//! curve was calibrated against (shot G4c): a column's factor is its byte over the open-sky byte,
//! exactly 1 wherever nothing is in the way. A noise world has no buildings, carries no budget and
//! lights every column as it always has (DECISIONS.md, shot G9).

use std::f64::consts::PI;

use crate::bundle::Ground;
use crate::params::SunParams;

/// The Earth's axial tilt in degrees: the sun's declination at the solstices.
pub const OBLIQUITY_DEG: f64 = 23.44;

/// Edges in degrees of the altitude bands `sky_view` samples. Each band but the last is one ring,
/// walked at [`SKY_AZIMUTHS`] azimuths at the band's middle altitude; the last band is the cap
/// around the zenith, which no building beside a column can hide, so any column no roof covers sees
/// at least that much sky.
const SKY_BANDS_DEG: [f64; 5] = [0.0, 30.0, 60.0, 82.5, 90.0];

/// Rings per azimuth: every band but the zenith cap.
const SKY_RINGS: usize = SKY_BANDS_DEG.len() - 2;

/// Azimuths per sky ring.
const SKY_AZIMUTHS: usize = 16;

/// One direction towards the sky: the unit horizontal step (east, north) and the tangent of its
/// altitude, with the weight it carries in its sum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    /// East component of the unit horizontal direction.
    pub east: f64,
    /// North component of the unit horizontal direction.
    pub north: f64,
    /// Tangent of the altitude above the horizon.
    pub tan_alt: f64,
    /// Weight in the ray's sum: `sin(altitude)` for a sun position, `sin · cos` for a sky ring.
    pub weight: f64,
}

/// The sun's declination in degrees at `phase` of the year, 0 being the spring equinox — the tick
/// the simulator's temperature crosses its mean going up, which is also the viewer's tick 0
/// (`ecoview-native/src/sky.rs`). A quarter of a year on is the summer solstice.
pub fn declination_deg(phase: f64) -> f64 {
    OBLIQUITY_DEG * libm::sin(2.0 * PI * phase)
}

/// The sun's positions on one day: `n` hour angles evenly spaced between sunrise and sunset (the
/// midpoints of `n` equal steps, so none sits on the horizon), at latitude `lat_deg` with
/// declination `decl_deg`. Empty on a day the sun does not rise.
pub fn sun_rays(lat_deg: f64, decl_deg: f64, n: usize) -> Vec<Ray> {
    let (phi, dec) = (lat_deg.to_radians(), decl_deg.to_radians());
    let cos_h0 = (-libm::tan(phi) * libm::tan(dec)).clamp(-1.0, 1.0);
    let h0 = libm::acos(cos_h0);
    let mut rays = Vec::with_capacity(n);
    for i in 0..n {
        let h = -h0 + (i as f64 + 0.5) * 2.0 * h0 / n as f64;
        // East, north and up components of the unit vector towards the sun.
        let east = -libm::cos(dec) * libm::sin(h);
        let north = libm::cos(phi) * libm::sin(dec) - libm::sin(phi) * libm::cos(dec) * libm::cos(h);
        let up = libm::sin(phi) * libm::sin(dec) + libm::cos(phi) * libm::cos(dec) * libm::cos(h);
        let flat = (east * east + north * north).sqrt();
        if up <= 0.0 || flat <= 0.0 {
            continue;
        }
        rays.push(Ray { east: east / flat, north: north / flat, tan_alt: up / flat, weight: up });
    }
    rays
}

/// The share of an evenly bright sky's light a level surface takes from the band between
/// altitudes `lo` and `hi` (degrees), as a fraction of the whole sky's: `sin²hi − sin²lo`.
fn band_weight(lo: f64, hi: f64) -> f64 {
    libm::sin(hi.to_radians()).powi(2) - libm::sin(lo.to_radians()).powi(2)
}

/// The directions `sky_view` samples: `SKY_AZIMUTHS` azimuths on each ring of `SKY_BANDS_DEG`,
/// azimuth by azimuth with the rings in rising order, each carrying its band's `band_weight`
/// shared among the azimuths. Their weights and [`zenith_weight`] sum to 1.
pub fn sky_rays() -> Vec<Ray> {
    let mut rays = Vec::with_capacity(SKY_RINGS * SKY_AZIMUTHS);
    for k in 0..SKY_AZIMUTHS {
        let az = 2.0 * PI * k as f64 / SKY_AZIMUTHS as f64;
        for b in 0..SKY_RINGS {
            let (lo, hi) = (SKY_BANDS_DEG[b], SKY_BANDS_DEG[b + 1]);
            let alt = (0.5 * (lo + hi)).to_radians();
            let weight = band_weight(lo, hi) / SKY_AZIMUTHS as f64;
            rays.push(Ray { east: libm::sin(az), north: libm::cos(az), tan_alt: libm::tan(alt), weight });
        }
    }
    rays
}

/// The zenith cap's share of the sky's light: always open over a column no roof covers.
pub fn zenith_weight() -> f64 {
    band_weight(SKY_BANDS_DEG[SKY_RINGS], 90.0)
}

/// The annual light budget: one byte per column per season slice.
#[derive(Debug, Clone, PartialEq)]
pub struct SunBudget {
    /// Season slices, `sun.season_samples`.
    pub slices: usize,
    /// Columns per slice.
    pub cols: usize,
    /// The budget, slice-major: `bytes[slice * cols + column]`. This is `world/sun.bin`.
    pub bytes: Vec<u8>,
    /// The byte of a column nothing shades: every column's light is its byte over this one.
    pub open: u8,
    /// Latitude the budget was computed at, degrees north.
    pub latitude_deg: f64,
}

impl SunBudget {
    /// Slice `k`'s centre as a phase of the year, 0 being the spring equinox: `k / slices`, so
    /// four slices are the two equinoxes and the two solstices.
    pub fn phase(&self, k: usize) -> f64 {
        k as f64 / self.slices.max(1) as f64
    }

    /// The slice tick `tick` falls in, in a year of `year_len` ticks: the one whose centre is
    /// nearest.
    pub fn slice_of(&self, tick: u32, year_len: u32) -> usize {
        let n = self.slices.max(1);
        let phase = (tick % year_len.max(1)) as f64 / year_len.max(1) as f64;
        (phase * n as f64).round() as usize % n
    }

    /// Column `c`'s light as a fraction of the open sky's in slice `k`.
    #[inline]
    pub fn factor(&self, k: usize, c: usize) -> f32 {
        self.bytes[k * self.cols + c] as f32 / self.open as f32
    }

    /// The budget of every column of an ecology grid `wx` × `wy` over `ground`, whose cells hold
    /// `ground_h` (metres) and `building_h` (metres above the ground; 0 where there is no
    /// building), at latitude `lat_deg`.
    pub fn compute(
        p: &SunParams,
        lat_deg: f64,
        wx: usize,
        wy: usize,
        g: &Ground,
        ground_h: &[f32],
        building_h: &[f32],
    ) -> SunBudget {
        let lat_deg = lat_deg.clamp(-90.0, 90.0);
        let slices = p.season_samples.max(1) as usize;
        let cols = wx * wy;
        let d = f64::from(p.diffuse_fraction.clamp(0.0, 1.0));
        let beam_share = (1.0 - d) * (1.0 - f64::from(p.cloud_cover.clamp(0.0, 1.0)));
        let byte = |v: f64| (255.0 * v).round().clamp(0.0, 255.0) as u8;
        let open = byte(d + beam_share).max(1);
        let mut budget = SunBudget { slices, cols, bytes: vec![open; slices * cols], open, latitude_deg: lat_deg };
        let tops = Tops::new(g, ground_h, building_h);
        let sky = sky_rays();
        let suns: Vec<Vec<Ray>> = (0..slices)
            .map(|k| sun_rays(lat_deg, declination_deg(budget.phase(k)), p.day_samples.max(1) as usize))
            .collect();
        let r = g.ratio;
        for y in 0..wy {
            for x in 0..wx {
                let cells: Vec<usize> = g.cells_of(x, y).collect();
                let z0 = cells.iter().map(|&i| f64::from(ground_h[i])).sum::<f64>() / cells.len() as f64;
                let roofed = cells.iter().filter(|&&i| building_h[i] > 0.0).count() as f64 / cells.len() as f64;
                let origin =
                    Origin { gx: (x as f64 + 0.5) * r as f64, gy: (y as f64 + 0.5) * r as f64, col: (x, y), z0 };
                let sky_view = if tops.none {
                    1.0
                } else {
                    // One walk per azimuth answers every ring on it.
                    let mut open = zenith_weight();
                    for az in sky.chunks(SKY_RINGS) {
                        let hz =
                            tops.horizon(&origin, az[0].east, az[0].north, az[0].tan_alt, az[az.len() - 1].tan_alt);
                        open += az.iter().filter(|ray| ray.tan_alt >= hz).map(|ray| ray.weight).sum::<f64>();
                    }
                    open
                };
                for (k, rays) in suns.iter().enumerate() {
                    let total: f64 = rays.iter().map(|ray| ray.weight).sum();
                    let beam = if total <= 0.0 || tops.none {
                        1.0
                    } else {
                        rays.iter().filter(|ray| !tops.blocked(&origin, ray)).map(|ray| ray.weight).sum::<f64>() / total
                    };
                    let v = (d * sky_view + beam_share * beam) * (1.0 - roofed);
                    // The diffuse share is the whole point: a column the sky can see is never black.
                    let mut b = byte(v);
                    if b == 0 && v > 0.0 {
                        b = 1;
                    }
                    budget.bytes[k * cols + x + wx * y] = b;
                }
            }
        }
        budget
    }

    /// The share of `cols` (column indices) below `threshold` of the open sky in slice `k`.
    pub fn share_below(&self, k: usize, cols: &[usize], threshold: f32) -> f64 {
        if cols.is_empty() {
            return 0.0;
        }
        cols.iter().filter(|&&c| self.factor(k, c) < threshold).count() as f64 / cols.len() as f64
    }
}

/// Where a ray starts: the middle of ecology column `col`, in ground-cell units, at height `z0`.
struct Origin {
    gx: f64,
    gy: f64,
    col: (usize, usize),
    z0: f64,
}

/// Edge in ground cells of the blocks [`Tops`] keeps a maximum for.
const BLOCK: usize = 16;

/// The obstacle field: absolute building top per ground cell (`None` where there is no building),
/// the highest in each [`BLOCK`]-cell square, and the highest of all, which bounds every walk.
struct Tops<'a> {
    g: &'a Ground,
    top: Vec<Option<f32>>,
    block_max: Vec<f64>,
    blocks_x: usize,
    max_top: f64,
    none: bool,
}

impl<'a> Tops<'a> {
    fn new(g: &'a Ground, ground_h: &[f32], building_h: &[f32]) -> Tops<'a> {
        let top: Vec<Option<f32>> =
            ground_h.iter().zip(building_h).map(|(&gh, &bh)| (bh > 0.0).then_some(gh + bh)).collect();
        let max_top = top.iter().flatten().fold(f64::NEG_INFINITY, |m, &t| m.max(f64::from(t)));
        let (blocks_x, blocks_y) = (g.width.div_ceil(BLOCK), g.depth.div_ceil(BLOCK));
        let mut block_max = vec![f64::NEG_INFINITY; blocks_x * blocks_y];
        for (i, t) in top.iter().enumerate() {
            if let Some(t) = t {
                let b = (i % g.width) / BLOCK + blocks_x * ((i / g.width) / BLOCK);
                block_max[b] = block_max[b].max(f64::from(*t));
            }
        }
        Tops { g, none: !max_top.is_finite(), top, block_max, blocks_x, max_top }
    }

    /// Whether a building stands between `o` and the sky along `ray`.
    fn blocked(&self, o: &Origin, ray: &Ray) -> bool {
        self.horizon(o, ray.east, ray.north, ray.tan_alt, ray.tan_alt) > ray.tan_alt
    }

    /// The horizon from `o` towards the horizontal direction (`east`, `north`): the largest
    /// tangent of the elevation of a building top above the origin, or −∞ where none stands. Only
    /// answers above `floor` matter to the caller, and none above `ceil`, so the walk stops once the
    /// ray at `floor` (or at the horizon so far, if higher) clears the tallest roof, or once the
    /// horizon passes `ceil`.
    ///
    /// The walk steps one ground cell at a time and stops at the world's edge. The origin's own
    /// column is skipped: the share of it a roof covers is charged separately. Where the ray is
    /// already above everything in the current block it jumps to the block's edge, which the ray,
    /// rising, leaves still above it -- the jump skips no cell that could raise the horizon.
    fn horizon(&self, o: &Origin, east: f64, north: f64, floor: f64, ceil: f64) -> f64 {
        let cell_m = f64::from(self.g.cell_m());
        let r = self.g.ratio;
        let (w, h) = (self.g.width as f64, self.g.depth as f64);
        let mut best = f64::NEG_INFINITY;
        let mut k = 1.0;
        loop {
            let dist = k * cell_m;
            let z = o.z0 + dist * best.max(floor);
            if z > self.max_top {
                return best;
            }
            let (px, py) = (o.gx + k * east, o.gy + k * north);
            if px < 0.0 || py < 0.0 || px >= w || py >= h {
                return best;
            }
            let (cx, cy) = (px as usize, py as usize);
            let (bx, by) = (cx / BLOCK, cy / BLOCK);
            if z > self.block_max[bx + self.blocks_x * by] {
                // Steps to the block's far edge along each axis; the nearer one is where it leaves.
                let edge = |p: f64, dir: f64, b: usize| {
                    if dir > 1e-12 {
                        (((b + 1) * BLOCK) as f64 - p) / dir
                    } else if dir < -1e-12 {
                        (p - (b * BLOCK) as f64) / -dir
                    } else {
                        f64::INFINITY
                    }
                };
                k += edge(px, east, bx).min(edge(py, north, by)).floor().max(1.0);
                continue;
            }
            if (cx / r, cy / r) != o.col {
                if let Some(t) = self.top[cx + self.g.width * cy] {
                    best = best.max((f64::from(t) - o.z0) / dist);
                    if best > ceil {
                        return best;
                    }
                }
            }
            k += 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::Medium;

    /// A flat `n` × `n` ecology grid of 2×2 ground cells with one building `bh` metres tall on
    /// the ecology columns in `block` (x and y ranges, inclusive).
    fn world(n: usize, block: ((usize, usize), (usize, usize)), bh: f32) -> (Ground, Vec<f32>, Vec<f32>) {
        let r = 2;
        let g = Ground {
            width: n * r,
            depth: n * r,
            ratio: r,
            medium: vec![0; n * n * r * r],
            media: Medium::ALL.to_vec(),
        };
        let mut b = vec![0.0; g.cells()];
        let ((x0, x1), (y0, y1)) = block;
        for y in y0..=y1 {
            for x in x0..=x1 {
                for i in g.cells_of(x, y) {
                    b[i] = bh;
                }
            }
        }
        let gh = vec![0.0; g.cells()];
        (g, gh, b)
    }

    fn params() -> SunParams {
        SunParams::default()
    }

    #[test]
    fn the_sun_is_due_south_at_noon_at_its_textbook_altitude() {
        // One sample is noon; at the equinox it stands at 90° − latitude.
        let rays = sun_rays(42.7, 0.0, 1);
        assert_eq!(rays.len(), 1);
        let alt = libm::atan(rays[0].tan_alt).to_degrees();
        assert!((alt - 47.3).abs() < 1e-6, "{alt}");
        assert!((rays[0].north + 1.0).abs() < 1e-9 && rays[0].east.abs() < 1e-9);
        // Morning is in the east, evening in the west.
        let day = sun_rays(42.7, 0.0, 9);
        assert!(day[0].east > 0.0 && day[8].east < 0.0);
        // The summer solstice is higher at noon than the winter one.
        let (s, w) = (sun_rays(42.7, OBLIQUITY_DEG, 1)[0], sun_rays(42.7, -OBLIQUITY_DEG, 1)[0]);
        assert!(s.tan_alt > w.tan_alt);
    }

    #[test]
    fn a_lone_block_shades_north_and_the_loss_falls_off_with_distance() {
        let n = 64;
        let (g, gh, bh) = world(n, ((30, 33), (20, 23)), 10.0);
        let b = SunBudget::compute(&params(), 42.7, n, n, &g, &gh, &bh);
        let at = |k: usize, x: usize, y: usize| b.bytes[k * n * n + x + n * y];
        let winter = 3;
        // North of the block loses light, south of it almost none.
        // Four columns off each face: north has lost most of the beam, south only the slice of
        // diffuse sky the block hides.
        let (north4, south4) = (at(winter, 31, 27), at(winter, 31, 16));
        assert!(
            3 * (b.open - south4) < b.open - north4,
            "south loses little: {south4} vs north {north4} of {}",
            b.open
        );
        // The beam alone: the winter sun is always in the southern sky, so south of the block keeps
        // all of it, right up against the wall.
        let beam = SunBudget::compute(&SunParams { diffuse_fraction: 0.0, ..params() }, 42.7, n, n, &g, &gh, &bh);
        let beam_at = |x: usize, y: usize| beam.bytes[winter * n * n + x + n * y];
        assert_eq!(beam_at(31, 19), beam.open, "the beam south of the block is untouched");
        assert!(beam_at(31, 24) < beam.open / 4, "and north of it mostly gone: {}", beam_at(31, 24));
        // The loss falls off with distance north rather than stopping dead.
        let north: Vec<u8> = (24..60).map(|y| at(winter, 31, y)).collect();
        assert!(north.windows(2).all(|w| w[0] <= w[1]), "monotone recovery: {north:?}");
        let distinct = north.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert!(distinct >= 4, "several steps of recovery, not a cliff: {north:?}");
        // 36 m on it is back within 2% of open sky: only a sliver of the low sky is still hidden.
        assert!(f32::from(*north.last().unwrap()) >= 0.98 * f32::from(b.open), "far away is open again: {north:?}");
        // The block's own columns are dark: a roof covers them.
        assert_eq!(at(winter, 31, 21), 0);
    }

    #[test]
    fn no_open_column_is_ever_black() {
        // A courtyard: a ring of 20 m walls around a 4×4 yard. The yard's floor sees only a patch of
        // sky straight up, and the diffuse share alone must keep it above zero.
        let n = 32;
        let r = 2;
        let g = Ground {
            width: n * r,
            depth: n * r,
            ratio: r,
            medium: vec![0; n * n * r * r],
            media: Medium::ALL.to_vec(),
        };
        let mut bh = vec![0.0; g.cells()];
        for y in 10..20 {
            for x in 10..20 {
                if !(13..17).contains(&x) || !(13..17).contains(&y) {
                    for i in g.cells_of(x, y) {
                        bh[i] = 20.0;
                    }
                }
            }
        }
        let gh = vec![0.0; g.cells()];
        let b = SunBudget::compute(&params(), 42.7, n, n, &g, &gh, &bh);
        for k in 0..b.slices {
            for y in 0..n {
                for x in 0..n {
                    let roofed = g.cells_of(x, y).any(|i| bh[i] > 0.0);
                    if !roofed {
                        assert!(b.bytes[k * n * n + x + n * y] > 0, "slice {k} column ({x}, {y}) is black");
                    }
                }
            }
        }
    }

    #[test]
    fn winter_shades_more_ground_than_summer() {
        let n = 64;
        let (g, gh, bh) = world(n, ((30, 33), (20, 23)), 10.0);
        let b = SunBudget::compute(&params(), 42.7, n, n, &g, &gh, &bh);
        let (summer, winter) = (1, 3);
        assert!((b.phase(summer) - 0.25).abs() < 1e-12 && (b.phase(winter) - 0.75).abs() < 1e-12);
        let shaded = |k: usize| (0..n * n).filter(|&c| b.bytes[k * n * n + c] < b.open).count();
        assert!(shaded(winter) > shaded(summer), "winter {} vs summer {}", shaded(winter), shaded(summer));
        // Both equinoxes cast the same shadow.
        assert_eq!(b.bytes[..n * n], b.bytes[2 * n * n..3 * n * n]);
    }

    #[test]
    fn the_sky_s_weights_add_up_to_the_whole_sky() {
        let total: f64 = sky_rays().iter().map(|r| r.weight).sum::<f64>() + zenith_weight();
        assert!((total - 1.0).abs() < 1e-12, "{total}");
        assert!(zenith_weight() > 0.01, "the cap is a real share of the sky: {}", zenith_weight());
    }

    #[test]
    fn a_world_without_buildings_is_open_sky_everywhere() {
        let n = 16;
        let (g, gh, _) = world(n, ((0, 0), (0, 0)), 0.0);
        let bh = vec![0.0; g.cells()];
        let b = SunBudget::compute(&params(), 42.7, n, n, &g, &gh, &bh);
        assert!(b.bytes.iter().all(|&v| v == b.open));
        assert!((0..n * n).all(|c| b.factor(2, c) == 1.0));
    }

    #[test]
    fn the_budget_is_computed_not_sampled() {
        let n = 32;
        let (g, gh, bh) = world(n, ((10, 12), (10, 14)), 7.5);
        let a = SunBudget::compute(&params(), 42.7, n, n, &g, &gh, &bh);
        let b = SunBudget::compute(&params(), 42.7, n, n, &g, &gh, &bh);
        assert_eq!(a, b);
    }

    #[test]
    fn a_tick_falls_in_the_slice_whose_centre_is_nearest() {
        let b = SunBudget { slices: 4, cols: 0, bytes: vec![], open: 178, latitude_deg: 42.7 };
        let year = 4000;
        assert_eq!(b.slice_of(0, year), 0);
        assert_eq!(b.slice_of(499, year), 0);
        assert_eq!(b.slice_of(500, year), 1);
        assert_eq!(b.slice_of(1000, year), 1);
        assert_eq!(b.slice_of(3000, year), 3);
        assert_eq!(b.slice_of(3600, year), 0, "late winter rounds up to the spring equinox");
        assert_eq!(b.slice_of(4000 + 2000, year), 2);
    }

    #[test]
    fn the_southern_hemisphere_shades_south() {
        let n = 64;
        let (g, gh, bh) = world(n, ((30, 33), (30, 33)), 10.0);
        // Slice 1 is the June solstice: the southern winter, the sun low in the north all day.
        let beam = SunBudget::compute(&SunParams { diffuse_fraction: 0.0, ..params() }, -33.9, n, n, &g, &gh, &bh);
        let at = |x: usize, y: usize| beam.bytes[n * n + x + n * y];
        assert_eq!(at(31, 34), beam.open, "north of the block keeps the beam");
        assert!(at(31, 29) < beam.open / 4, "south of it is in the shadow: {}", at(31, 29));
    }
}
