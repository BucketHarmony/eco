//! World bundles: a world exported from a tagged Blender scene, with a ground grid finer than the
//! ecology grid. The format is `../docs/SCENE-CONTRACT.md`, which is authoritative; this module
//! reads version 2 of it and nothing else.
//!
//! The ecology grid keeps its 1 m columns, so every species parameter keeps its units; the ground
//! grid (0.5 m in the Capitol bundle) carries ground height, medium and building height at full
//! resolution for the water and nutrient work that follows.

use crate::params::Params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The `format` string `bundle.json` must carry.
pub const BUNDLE_FORMAT: &str = "ecosim-world-bundle";

/// The only bundle version this ecosim reads.
pub const BUNDLE_VERSION: u32 = 2;

/// Edge of one ecology column, in metres. Species parameters are written in these units, so it is
/// fixed; only the ground grid is finer (DECISIONS.md, shot G1).
pub const ECO_CELL_M: f32 = 1.0;

/// The surface a ground cell is covered with (`eco_medium` in the scene contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medium {
    /// Bare soil.
    Soil,
    /// Mown grass.
    Lawn,
    /// A planting bed.
    Bed,
    /// Mulch over soil.
    Mulch,
    /// Loose gravel.
    Gravel,
    /// Concrete paving.
    Concrete,
    /// Asphalt paving.
    Asphalt,
    /// A building roof.
    Roof,
    /// Open water.
    Water,
}

impl Medium {
    /// Every medium in the scene contract's order, so the index is the bundle's medium code.
    pub const ALL: [Medium; 9] = [
        Medium::Soil,
        Medium::Lawn,
        Medium::Bed,
        Medium::Mulch,
        Medium::Gravel,
        Medium::Concrete,
        Medium::Asphalt,
        Medium::Roof,
        Medium::Water,
    ];

    /// The name the scene contract gives this medium.
    pub fn name(self) -> &'static str {
        match self {
            Medium::Soil => "soil",
            Medium::Lawn => "lawn",
            Medium::Bed => "bed",
            Medium::Mulch => "mulch",
            Medium::Gravel => "gravel",
            Medium::Concrete => "concrete",
            Medium::Asphalt => "asphalt",
            Medium::Roof => "roof",
            Medium::Water => "water",
        }
    }

    /// The medium a contract name means, or `None` for an unknown name.
    pub fn from_name(name: &str) -> Option<Medium> {
        Medium::ALL.into_iter().find(|m| m.name() == name)
    }

    /// Whether the surface is sealed (`roof`, `asphalt`, `concrete`): nothing roots through it, so
    /// a column they mostly cover is Rock.
    pub fn is_sealed(self) -> bool {
        matches!(self, Medium::Roof | Medium::Asphalt | Medium::Concrete)
    }
}

/// The ground grid: the bundle's medium map at full resolution, kept in the world state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ground {
    /// Cells along x (east), `size_m / cell_m`.
    pub width: usize,
    /// Cells along y (north).
    pub depth: usize,
    /// Ground cells per ecology column edge, `1 / cell_m`: a column covers `ratio²` of them.
    pub ratio: usize,
    /// Medium code per cell, indexed `x + width·y` with cell (0, 0) at the south-west corner.
    pub medium: Vec<u8>,
    /// Medium code → medium, as `bundle.json` lists it.
    pub media: Vec<Medium>,
}

impl Ground {
    /// Cell edge in metres.
    pub fn cell_m(&self) -> f32 {
        ECO_CELL_M / self.ratio as f32
    }

    /// Number of ground cells.
    pub fn cells(&self) -> usize {
        self.width * self.depth
    }

    /// Ground cells under one ecology column.
    pub fn cells_per_column(&self) -> usize {
        self.ratio * self.ratio
    }

    /// The ground cell indices under ecology column (x, y), row by row.
    pub fn cells_of(&self, x: usize, y: usize) -> impl Iterator<Item = usize> + '_ {
        let (x0, y0) = (x * self.ratio, y * self.ratio);
        (y0..y0 + self.ratio).flat_map(move |gy| (x0..x0 + self.ratio).map(move |gx| gx + self.width * gy))
    }

    /// The medium of ground cell `i`.
    pub fn medium_at(&self, i: usize) -> Medium {
        self.media[self.medium[i] as usize]
    }
}

/// A tree from the scene (`trees.json`). Positions are metres from the south-west corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleTree {
    /// Trunk base, metres east of the corner.
    pub x: f32,
    /// Trunk base, metres north of the corner.
    pub y: f32,
    /// Height in metres.
    pub height: f32,
    /// Crown radius in metres.
    pub crown_radius: f32,
    /// Height of the lowest crown foliage, in metres.
    pub crown_base: f32,
}

/// A shrub from the scene (`shrubs.json`), an ellipse centred on (x, y).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleShrub {
    /// Centre, metres east of the corner.
    pub x: f32,
    /// Centre, metres north of the corner.
    pub y: f32,
    /// Height in metres.
    pub height: f32,
    /// Half the east–west extent in metres, before rotation.
    pub rx: f32,
    /// Half the north–south extent in metres, before rotation.
    pub ry: f32,
    /// Rotation of the ellipse, in radians.
    pub angle: f32,
}

/// A storm drain from the scene (`pipes.json`), used from shot G6 on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pipe {
    /// The scene's name for the pipe.
    pub id: String,
    /// Inlet, metres from the corner.
    pub inlet: [f32; 2],
    /// Outlet, metres from the corner; on or past the crop edge it drains out of the world.
    pub outlet: [f32; 2],
    /// Capacity in cubic metres per hour.
    pub capacity_m3h: f32,
    /// Whether the route is illustrative rather than surveyed.
    #[serde(default)]
    pub illustrative: bool,
}

/// A loaded world bundle: the ground grid, its height and building fields, and the scene's plants
/// and pipes.
#[derive(Debug, Clone)]
pub struct Bundle {
    /// The directory it was read from.
    pub dir: PathBuf,
    /// The world's name (`eco_name`).
    pub name: String,
    /// Crop edge in metres, which is also the ecology grid's edge in columns.
    pub size_m: usize,
    /// Provenance: data source, licence, crop centre.
    pub source: String,
    /// The ground grid.
    pub ground: Ground,
    /// Ground height per ground cell, metres above the crop minimum.
    pub ground_h: Vec<f32>,
    /// Building height above ground per ground cell, 0 where there is no roof.
    pub building_h: Vec<f32>,
    /// Trees in the scene (used from shot G3 on).
    pub trees: Vec<BundleTree>,
    /// Shrubs in the scene (used from shot G3 on).
    pub shrubs: Vec<BundleShrub>,
    /// Storm drains in the scene (used from shot G6 on).
    pub pipes: Vec<Pipe>,
}

/// `bundle.json` as the exporter writes it. Unknown keys are ignored, so a later exporter may add
/// fields (the counts, for one) without breaking this reader.
#[derive(Deserialize)]
struct BundleJson {
    format: String,
    version: u32,
    name: String,
    size_m: f64,
    ground_cell_m: f64,
    ground_width: usize,
    ground_depth: usize,
    media: Vec<String>,
    #[serde(default)]
    source: String,
}

/// `v` as a whole number, or `None` when it is not finite or not whole.
fn whole(v: f64) -> Option<usize> {
    (v.is_finite() && v >= 0.0 && v.fract() == 0.0 && v <= usize::MAX as f64).then_some(v as usize)
}

impl Bundle {
    /// Read and validate a bundle directory. Every error names the file it is about.
    pub fn load(dir: &Path) -> Result<Bundle, String> {
        let at = |f: &str| dir.join(f).display().to_string();
        let text = fs::read_to_string(dir.join("bundle.json")).map_err(|e| format!("{}: {e}", at("bundle.json")))?;
        let j: BundleJson = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", at("bundle.json")))?;
        let bad = |m: String| Err(format!("{}: {m}", at("bundle.json")));
        if j.format != BUNDLE_FORMAT {
            return bad(format!("format is {:?}, expected {BUNDLE_FORMAT:?}", j.format));
        }
        if j.version != BUNDLE_VERSION {
            return bad(format!(
                "version {} is not supported; this ecosim reads world bundle version {BUNDLE_VERSION}",
                j.version
            ));
        }
        let Some(size_m) = whole(j.size_m) else {
            return bad(format!("size_m = {} must be a whole number of metres", j.size_m));
        };
        if !(1..=256).contains(&size_m) {
            return bad(format!("size_m = {size_m} must be in 1..=256: one ecology column is 1 m and x, y are u8"));
        }
        if !(j.ground_cell_m.is_finite() && j.ground_cell_m > 0.0) {
            return bad(format!("ground_cell_m = {} must be positive", j.ground_cell_m));
        }
        let per_m = 1.0 / j.ground_cell_m;
        let Some(ratio) = whole(per_m.round()).filter(|&r| r >= 1 && (per_m - per_m.round()).abs() < 1e-6) else {
            return bad(format!(
                "ground_cell_m = {} divides the 1 m ecology column into {per_m} ground cells; it must be a whole number",
                j.ground_cell_m
            ));
        };
        if (j.ground_width, j.ground_depth) != (size_m * ratio, size_m * ratio) {
            return bad(format!(
                "ground grid is {}×{}, but size_m = {size_m} at {} m cells is {}×{}",
                j.ground_width,
                j.ground_depth,
                j.ground_cell_m,
                size_m * ratio,
                size_m * ratio
            ));
        }
        let mut media = Vec::with_capacity(j.media.len());
        for (code, name) in j.media.iter().enumerate() {
            let m = Medium::from_name(name)
                .ok_or_else(|| format!("{}: media[{code}] = {name:?} is not a medium", at("bundle.json")))?;
            if media.contains(&m) {
                return bad(format!("media[{code}] = {name:?} is listed twice"));
            }
            media.push(m);
        }
        if media.first() != Some(&Medium::Soil) {
            return bad("media[0] must be \"soil\"".into());
        }
        let ground = Ground { width: j.ground_width, depth: j.ground_depth, ratio, medium: Vec::new(), media };
        let n = ground.cells();
        let ground_h = read_f32(dir, "ground_h.f32", n)?;
        let building_h = read_f32(dir, "building_h.f32", n)?;
        let medium = read_bytes(dir, "medium.u8", n)?;
        if let Some((i, c)) = medium.iter().enumerate().find(|(_, &c)| c as usize >= ground.media.len()) {
            return Err(format!(
                "{}: cell {i} has medium code {c}, but bundle.json lists {} media",
                at("medium.u8"),
                ground.media.len()
            ));
        }
        if let Some((i, h)) = building_h.iter().enumerate().find(|(_, &h)| h < 0.0) {
            return Err(format!("{}: cell {i} has building height {h}", at("building_h.f32")));
        }
        let ground = Ground { medium, ..ground };
        let b = Bundle {
            dir: dir.to_path_buf(),
            name: j.name,
            size_m,
            source: j.source,
            ground,
            ground_h,
            building_h,
            trees: read_json(dir, "trees.json")?,
            shrubs: read_json(dir, "shrubs.json")?,
            pipes: read_json(dir, "pipes.json")?,
        };
        b.check_scene()?;
        Ok(b)
    }

    /// Trees, shrubs and pipes are validated on load even though nothing reads them before shot G3:
    /// a bad export should fail at the bundle, not halfway through a run.
    fn check_scene(&self) -> Result<(), String> {
        let edge = self.size_m as f32;
        let at = |f: &str| self.dir.join(f).display().to_string();
        let inside =
            |x: f32, y: f32| x.is_finite() && y.is_finite() && (0.0..=edge).contains(&x) && (0.0..=edge).contains(&y);
        let size = |v: f32| v.is_finite() && v > 0.0;
        for (i, t) in self.trees.iter().enumerate() {
            if !inside(t.x, t.y) {
                return Err(format!(
                    "{}: tree {i} stands at ({}, {}), outside the 0..{edge} m crop",
                    at("trees.json"),
                    t.x,
                    t.y
                ));
            }
            if !(size(t.height) && t.crown_radius.is_finite() && t.crown_radius >= 0.0) {
                return Err(format!(
                    "{}: tree {i} has height {} and crown radius {}",
                    at("trees.json"),
                    t.height,
                    t.crown_radius
                ));
            }
            if !(t.crown_base.is_finite() && (0.0..=t.height).contains(&t.crown_base)) {
                return Err(format!(
                    "{}: tree {i} has crown base {} outside 0..{}",
                    at("trees.json"),
                    t.crown_base,
                    t.height
                ));
            }
        }
        for (i, s) in self.shrubs.iter().enumerate() {
            if !inside(s.x, s.y) {
                return Err(format!(
                    "{}: shrub {i} sits at ({}, {}), outside the 0..{edge} m crop",
                    at("shrubs.json"),
                    s.x,
                    s.y
                ));
            }
            if !(size(s.height) && size(s.rx) && size(s.ry) && s.angle.is_finite()) {
                return Err(format!(
                    "{}: shrub {i} has height {}, rx {}, ry {}, angle {}",
                    at("shrubs.json"),
                    s.height,
                    s.rx,
                    s.ry,
                    s.angle
                ));
            }
        }
        for p in &self.pipes {
            // An outlet may lie on or past the crop edge: that is how a pipe drains out of the world.
            if !inside(p.inlet[0], p.inlet[1]) {
                return Err(format!(
                    "{}: pipe {:?} has its inlet at ({}, {}), outside the crop",
                    at("pipes.json"),
                    p.id,
                    p.inlet[0],
                    p.inlet[1]
                ));
            }
            if !(p.outlet[0].is_finite() && p.outlet[1].is_finite()) {
                return Err(format!(
                    "{}: pipe {:?} has outlet ({}, {})",
                    at("pipes.json"),
                    p.id,
                    p.outlet[0],
                    p.outlet[1]
                ));
            }
            if !(p.capacity_m3h.is_finite() && p.capacity_m3h >= 0.0) {
                return Err(format!("{}: pipe {:?} has capacity {}", at("pipes.json"), p.id, p.capacity_m3h));
            }
        }
        Ok(())
    }

    /// Ground cells under one ecology column.
    pub fn cells_per_column(&self) -> usize {
        self.ground.cells_per_column()
    }

    /// Put the bundle's dimensions into `params`: the ecology grid is the crop, one column per
    /// metre, so `[world] width` and `depth` become `size_m`. Every other key, `patch` included,
    /// stays as the params file set it.
    pub fn apply_to(&self, params: &mut Params) -> Result<(), String> {
        params.world.width = self.size_m as u32;
        params.world.depth = self.size_m as u32;
        params.check_dims().map_err(|e| format!("{}: {e}", self.dir.display()))
    }

    /// Copy the static ground-grid files into a run directory's `world/` (format version 4). The
    /// bytes are the bundle's own, so a run directory carries the ground it was built on.
    pub fn write_world_dir(&self, run_dir: &Path) -> io::Result<()> {
        let dir = run_dir.join("world");
        fs::create_dir_all(&dir)?;
        for (from, to) in [
            ("ground_h.f32", "ground_h.bin"),
            ("medium.u8", "medium.bin"),
            ("building_h.f32", "building_h.bin"),
            ("pipes.json", "pipes.json"),
        ] {
            fs::copy(self.dir.join(from), dir.join(to))?;
        }
        Ok(())
    }
}

fn read_bytes(dir: &Path, name: &str, len: usize) -> Result<Vec<u8>, String> {
    let p = dir.join(name);
    let b = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    if b.len() != len {
        return Err(format!("{}: {} bytes, expected {len}", p.display(), b.len()));
    }
    Ok(b)
}

fn read_f32(dir: &Path, name: &str, len: usize) -> Result<Vec<f32>, String> {
    let b = read_bytes(dir, name, len * 4)?;
    let v: Vec<f32> = b.as_chunks::<4>().0.iter().map(|&c| f32::from_le_bytes(c)).collect();
    match v.iter().position(|x| !x.is_finite()) {
        Some(i) => Err(format!("{}: cell {i} is {}", dir.join(name).display(), v[i])),
        None => Ok(v),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> Result<Vec<T>, String> {
    let p = dir.join(name);
    let text = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::world::{ColClass, World};
    use crate::Sim;
    use proptest::prelude::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A bundle of `size_m` metres at `1/ratio` m ground cells: flat at height 0, all lawn, no
    /// buildings, plants or pipes. The tests edit its fields.
    pub(crate) fn flat_bundle(size_m: usize, ratio: usize) -> Bundle {
        let (w, d) = (size_m * ratio, size_m * ratio);
        Bundle {
            dir: PathBuf::from("synthetic"),
            name: "synthetic".into(),
            size_m,
            source: "test".into(),
            ground: Ground { width: w, depth: d, ratio, medium: vec![1; w * d], media: Medium::ALL.to_vec() },
            ground_h: vec![0.0; w * d],
            building_h: vec![0.0; w * d],
            trees: Vec::new(),
            shrubs: Vec::new(),
            pipes: Vec::new(),
        }
    }

    /// The params a bundle test runs with: the crate's own, on the bundle's dimensions.
    pub(crate) fn bundle_params(b: &Bundle) -> Params {
        let mut p = Params::load_default();
        p.world.patch = 8;
        p.climate.rain_gradient = 0.0;
        p.world.slope_bias = 0.0;
        b.apply_to(&mut p).unwrap();
        p
    }

    /// Set every ground cell of ecology column (x, y) to `m`.
    pub(crate) fn paint(b: &mut Bundle, x: usize, y: usize, m: Medium) {
        let code = b.ground.media.iter().position(|&k| k == m).unwrap() as u8;
        for i in b.ground.cells_of(x, y).collect::<Vec<_>>() {
            b.ground.medium[i] = code;
        }
    }

    /// Put a building of `h` metres on ecology column (x, y): roof over all its ground cells.
    pub(crate) fn build(b: &mut Bundle, x: usize, y: usize, h: f32) {
        paint(b, x, y, Medium::Roof);
        for i in b.ground.cells_of(x, y).collect::<Vec<_>>() {
            b.building_h[i] = h;
        }
    }

    fn world_of(b: &Bundle) -> World {
        World::from_bundle(b, &bundle_params(b)).unwrap()
    }

    #[test]
    fn flat_terrain_gives_flat_surface_layers() {
        let mut b = flat_bundle(16, 2);
        b.ground_h.iter_mut().for_each(|h| *h = 3.2);
        let w = world_of(&b);
        assert_eq!(w.dims.wx, 16);
        assert!(w.height.iter().all(|&h| h == 8 + 3), "every column is base_z + round(3.2)");
        assert!(w.class.iter().all(|&k| k == ColClass::Soil));
        // The medium grid is kept at full resolution.
        let g = w.ground_grid.as_ref().unwrap();
        assert_eq!((g.width, g.depth, g.ratio), (32, 32, 2));
        assert_eq!(g.medium_at(0), Medium::Lawn);
    }

    #[test]
    fn a_slope_gives_rounded_monotone_surface_layers() {
        let mut b = flat_bundle(16, 2);
        // 0.4 m per metre east, sampled at the ground cell's centre.
        for gy in 0..b.ground.depth {
            for gx in 0..b.ground.width {
                b.ground_h[gx + b.ground.width * gy] = 0.4 * (gx as f32 * 0.5);
            }
        }
        let w = world_of(&b);
        for x in 0..16 {
            let mean: f32 = b.ground.cells_of(x, 0).map(|i| b.ground_h[i]).sum::<f32>() / b.cells_per_column() as f32;
            assert_eq!(w.height[w.dims.cidx(x, 3)], 8 + mean.round() as u8, "column {x}");
        }
        assert!(w.height.windows(2).take(15).all(|p| p[0] <= p[1]), "monotone along the slope");
        // The east edge's cells are 6.0 and 6.2 m up, so its column sits 6 layers above base_z.
        assert_eq!(w.height[w.dims.cidx(15, 0)], 8 + 6);
    }

    #[test]
    fn a_roof_block_is_exactly_that_rock_footprint() {
        let mut b = flat_bundle(16, 2);
        for y in 4..7 {
            for x in 2..5 {
                build(&mut b, x, y, 3.0);
            }
        }
        let w = world_of(&b);
        for y in 0..16 {
            for x in 0..16 {
                let want = (2..5).contains(&x) && (4..7).contains(&y);
                assert_eq!(w.class[w.dims.cidx(x, y)] == ColClass::Rock, want, "column ({x}, {y})");
            }
        }
        assert!(w.patch_soil.iter().flatten().all(|&c| w.class[c] == ColClass::Soil));
    }

    #[test]
    fn a_mixed_column_follows_the_majority_rule_and_a_tie_is_not_rock() {
        let mut b = flat_bundle(16, 2);
        let cell = |b: &Bundle, x: usize, y: usize, k: usize| b.ground.cells_of(x, y).nth(k).unwrap();
        let code = |m: Medium| Medium::ALL.iter().position(|&k| k == m).unwrap() as u8;
        // (1, 1): two of four sealed — a tie, so not Rock.
        for k in 0..2 {
            let i = cell(&b, 1, 1, k);
            b.ground.medium[i] = code(Medium::Asphalt);
        }
        // (2, 1): three of four sealed, mixing all three sealed media.
        for (k, m) in [Medium::Asphalt, Medium::Concrete, Medium::Roof].into_iter().enumerate() {
            let i = cell(&b, 2, 1, k);
            b.ground.medium[i] = code(m);
        }
        // (3, 1): three of four water; (4, 1): two of four water, another tie.
        for k in 0..3 {
            let i = cell(&b, 3, 1, k);
            b.ground.medium[i] = code(Medium::Water);
        }
        for k in 0..2 {
            let i = cell(&b, 4, 1, k);
            b.ground.medium[i] = code(Medium::Water);
        }
        let w = world_of(&b);
        let class = |x: usize| w.class[w.dims.cidx(x, 1)];
        assert_eq!(class(1), ColClass::Soil, "a sealed tie is not Rock");
        assert_eq!(class(2), ColClass::Rock);
        assert_eq!(class(3), ColClass::Water);
        assert_eq!(class(4), ColClass::Soil, "a water tie is not Water");
    }

    #[test]
    fn a_tall_block_shades_the_columns_north_of_it() {
        let mut b = flat_bundle(16, 2);
        build(&mut b, 5, 5, 6.0);
        let w = world_of(&b);
        let light = |x: usize, y: usize| w.surface_light(w.dims.cidx(x, y));
        for dy in 1..=5 {
            assert_eq!(light(5, 5 + dy), 0, "column 5 m north by {dy} is in shadow");
        }
        assert_eq!(light(5, 11), 255, "the shadow ends after 6 columns");
        assert_eq!(light(5, 4), 255, "nothing south of the block is shaded");
        assert_eq!(light(4, 6), 255, "nor west of it");
        assert_eq!(light(6, 6), 255, "nor east of it");
    }

    #[test]
    fn shade_slope_0_turns_building_shade_off() {
        let mut b = flat_bundle(16, 2);
        build(&mut b, 5, 5, 6.0);
        let mut p = bundle_params(&b);
        p.bundle.shade_slope = 0.0;
        let w = World::from_bundle(&b, &p).unwrap();
        assert!(w.shade_top.iter().all(|&s| s == 0));
        assert_eq!(w.surface_light(w.dims.cidx(5, 6)), 255);
    }

    #[test]
    fn a_column_whose_top_does_not_fit_under_world_height_is_an_error() {
        let mut b = flat_bundle(16, 2);
        b.ground_h[0] = 100.0;
        let e = World::from_bundle(&b, &bundle_params(&b)).err().expect("a column above world.height");
        assert!(e.contains("column (0, 0)") && e.contains("[world] height"), "{e}");
    }

    /// Rock columns are exactly the sealed-majority ones, and they hold no cover: they are in no
    /// patch's soil list (so no grass or shrub), carry no trunk and no canopy, at every tick.
    fn rock_columns_hold_no_cover(b: &Bundle, ticks: u32) -> Result<(), TestCaseError> {
        let p = bundle_params(b);
        let mut sim = Sim::from_bundle(p, 9, b).unwrap();
        let d = sim.world.dims;
        let rock: Vec<usize> = (0..d.cols()).filter(|&c| sim.world.class[c] == ColClass::Rock).collect();
        for c in 0..d.cols() {
            let (x, y) = d.xy(c);
            let sealed = b.ground.cells_of(x, y).filter(|&i| b.ground.medium_at(i).is_sealed()).count();
            prop_assert_eq!(rock.contains(&c), 2 * sealed > b.cells_per_column(), "column {:?}", (x, y));
        }
        for t in 0..=ticks {
            for &c in &rock {
                prop_assert!(
                    !sim.world.patch_soil[d.patch_of(d.xy(c).0, d.xy(c).1)].contains(&c),
                    "tick {}: grass on rock",
                    t
                );
                prop_assert_eq!(sim.trunk_at[c], crate::sim::NO_TREE, "tick {}: a trunk on rock", t);
                prop_assert!(!sim.canopy_cover[c], "tick {}: canopy over rock", t);
            }
            if t < ticks {
                sim.step();
            }
        }
        Ok(())
    }

    /// A bundle with a random scatter of sealed, water and plantable ground cells.
    fn scattered_bundle() -> impl Strategy<Value = Bundle> {
        prop::collection::vec(0u8..9, 32 * 32).prop_map(|codes| {
            let mut b = flat_bundle(16, 2);
            b.ground.medium = codes;
            b
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(4)))]

        #[test]
        fn prop_rock_columns_never_hold_cover(b in scattered_bundle()) {
            rock_columns_hold_no_cover(&b, 60)?;
        }
    }

    /// Regression sibling: a roof block, a car park and a pond, run past the first producer update.
    #[test]
    fn rock_columns_hold_no_cover_regression_block_and_pond() {
        let mut b = flat_bundle(16, 2);
        for y in 2..6 {
            for x in 2..6 {
                build(&mut b, x, y, 9.0);
            }
        }
        for x in 6..12 {
            paint(&mut b, x, 3, Medium::Asphalt);
        }
        for y in 10..13 {
            for x in 10..14 {
                paint(&mut b, x, y, Medium::Water);
            }
        }
        rock_columns_hold_no_cover(&b, 120).unwrap();
        let w = world_of(&b);
        assert_eq!(w.class[w.dims.cidx(11, 11)], ColClass::Water);
        assert_eq!(w.class[w.dims.cidx(8, 3)], ColClass::Rock);
    }

    /// A bundle world is deterministic: the same bundle and seed give the same tick-0 state.
    #[test]
    fn two_sims_on_one_bundle_agree() {
        let mut b = flat_bundle(16, 2);
        build(&mut b, 4, 4, 5.0);
        let sim = |seed| Sim::from_bundle(bundle_params(&b), seed, &b).unwrap();
        let (a, c) = (sim(4), sim(4));
        assert_eq!(crate::state::encode(&a), crate::state::encode(&c));
        assert_eq!(a.world.material, c.world.material);
        assert_eq!(a.world.light, c.world.light);
        // The terrain comes from the bundle, not the seed, so another seed keeps it.
        assert_eq!(sim(5).world.material, a.world.material);
        assert_eq!(a.world.dims.cols(), 256);
    }

    fn scratch_dir() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "ecosim-bundle-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// Write `b` to a fresh directory as a bundle, with `bundle.json`'s version and media list
    /// taken from `version` and the bundle itself.
    pub(crate) fn write_bundle(b: &Bundle, version: u32) -> PathBuf {
        let dir = scratch_dir();
        fs::create_dir_all(&dir).unwrap();
        let media: Vec<String> = b.ground.media.iter().map(|m| format!("{:?}", m.name())).collect();
        let json = format!(
            "{{\"format\":\"{BUNDLE_FORMAT}\",\"version\":{version},\"name\":\"{}\",\"size_m\":{},\
             \"ground_cell_m\":{},\"ground_width\":{},\"ground_depth\":{},\"media\":[{}],\"source\":\"{}\"}}",
            b.name,
            b.size_m,
            b.ground.cell_m(),
            b.ground.width,
            b.ground.depth,
            media.join(","),
            b.source
        );
        fs::write(dir.join("bundle.json"), json).unwrap();
        let f32s = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        fs::write(dir.join("ground_h.f32"), f32s(&b.ground_h)).unwrap();
        fs::write(dir.join("building_h.f32"), f32s(&b.building_h)).unwrap();
        fs::write(dir.join("medium.u8"), &b.ground.medium).unwrap();
        fs::write(dir.join("trees.json"), serde_json::to_vec(&b.trees).unwrap()).unwrap();
        fs::write(dir.join("shrubs.json"), serde_json::to_vec(&b.shrubs).unwrap()).unwrap();
        fs::write(dir.join("pipes.json"), serde_json::to_vec(&b.pipes).unwrap()).unwrap();
        dir
    }

    /// A bundle written to disk and read back is the bundle that was written.
    #[test]
    fn a_written_bundle_reads_back() {
        let mut b = flat_bundle(16, 2);
        build(&mut b, 3, 3, 7.5);
        b.ground_h[5] = 2.25;
        b.trees.push(BundleTree { x: 1.5, y: 2.5, height: 9.0, crown_radius: 3.0, crown_base: 2.0 });
        b.shrubs.push(BundleShrub { x: 4.0, y: 4.0, height: 1.2, rx: 0.8, ry: 0.5, angle: 0.3 });
        b.pipes.push(Pipe {
            id: "p1".into(),
            inlet: [2.0, 2.0],
            outlet: [16.0, 8.0],
            capacity_m3h: 20.0,
            illustrative: true,
        });
        let dir = write_bundle(&b, BUNDLE_VERSION);
        let got = Bundle::load(&dir).unwrap();
        assert_eq!((got.size_m, got.ground.ratio), (16, 2));
        assert_eq!(got.ground_h, b.ground_h);
        assert_eq!(got.building_h, b.building_h);
        assert_eq!(got.ground.medium, b.ground.medium);
        assert_eq!((got.trees, got.shrubs, got.pipes), (b.trees, b.shrubs, b.pipes));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A rejection case: its name, what it breaks in a good bundle, and the text the error carries.
    type BadCase = (&'static str, Box<dyn Fn(&Path)>, &'static str);

    /// Every rejection names the file and says what is wrong.
    #[test]
    fn bad_bundles_are_rejected_with_a_clear_error() {
        let b = flat_bundle(16, 2);
        let cases: Vec<BadCase> = vec![
            ("version 1", Box::new(|_: &Path| {}), "version 1 is not supported"),
            (
                "short medium.u8",
                Box::new(|d: &Path| fs::write(d.join("medium.u8"), vec![0u8; 3]).unwrap()),
                "3 bytes, expected 1024",
            ),
            (
                "short ground_h.f32",
                Box::new(|d: &Path| fs::write(d.join("ground_h.f32"), vec![0u8; 8]).unwrap()),
                "8 bytes, expected 4096",
            ),
            (
                "unknown medium",
                Box::new(|d: &Path| {
                    let t = fs::read_to_string(d.join("bundle.json")).unwrap().replace("\"lawn\"", "\"astroturf\"");
                    fs::write(d.join("bundle.json"), t).unwrap();
                }),
                "is not a medium",
            ),
            (
                "ground grid that does not match size_m",
                Box::new(|d: &Path| {
                    let t = fs::read_to_string(d.join("bundle.json"))
                        .unwrap()
                        .replace("\"ground_width\":32", "\"ground_width\":30");
                    fs::write(d.join("bundle.json"), t).unwrap();
                }),
                "ground grid is 30×32",
            ),
            (
                "a ground cell that is not a whole fraction of a metre",
                Box::new(|d: &Path| {
                    let t = fs::read_to_string(d.join("bundle.json"))
                        .unwrap()
                        .replace("\"ground_cell_m\":0.5", "\"ground_cell_m\":0.3");
                    fs::write(d.join("bundle.json"), t).unwrap();
                }),
                "must be a whole number",
            ),
            (
                "a medium code with no medium",
                Box::new(|d: &Path| {
                    let mut m = fs::read(d.join("medium.u8")).unwrap();
                    m[7] = 9;
                    fs::write(d.join("medium.u8"), m).unwrap();
                }),
                "cell 7 has medium code 9",
            ),
            (
                "a tree outside the crop",
                Box::new(|d: &Path| {
                    fs::write(
                        d.join("trees.json"),
                        "[{\"x\":40.0,\"y\":1.0,\"height\":5.0,\"crown_radius\":2.0,\"crown_base\":1.0}]",
                    )
                    .unwrap()
                }),
                "outside the 0..16 m crop",
            ),
            (
                "a pipe with no capacity number",
                Box::new(|d: &Path| {
                    fs::write(
                        d.join("pipes.json"),
                        "[{\"id\":\"p\",\"inlet\":[1.0,1.0],\"outlet\":[0.0,0.0],\"capacity_m3h\":-1.0}]",
                    )
                    .unwrap()
                }),
                "has capacity -1",
            ),
        ];
        for (name, damage, want) in cases {
            let dir = write_bundle(&b, if name == "version 1" { 1 } else { BUNDLE_VERSION });
            damage(&dir);
            let e = Bundle::load(&dir).unwrap_err();
            assert!(e.contains(want), "{name}: error {e:?} does not mention {want:?}");
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn a_missing_bundle_directory_is_an_error() {
        let e = Bundle::load(Path::new("no-such-bundle")).unwrap_err();
        assert!(e.contains("bundle.json"), "{e}");
    }

    /// Dimensions that don't tile into whole patches are rejected, naming the patch size.
    #[test]
    fn dims_that_are_not_a_multiple_of_the_patch_are_rejected() {
        let b = flat_bundle(20, 2);
        let mut p = Params::load_default();
        p.world.patch = 8;
        let e = b.apply_to(&mut p).unwrap_err();
        assert!(e.contains("width = 20 must be a multiple of patch = 8"), "{e}");
    }

    #[test]
    fn media_names_round_trip() {
        for m in Medium::ALL {
            assert_eq!(Medium::from_name(m.name()), Some(m));
        }
        assert_eq!(Medium::from_name("astroturf"), None);
        assert!(Medium::ALL.iter().filter(|m| m.is_sealed()).count() == 3);
    }
}
