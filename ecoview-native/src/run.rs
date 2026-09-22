//! The **run directory**: the second thing the viewer reads, beside the world bundle.
//!
//! A world bundle is a photograph of a site at one instant. A run directory is what `ecosim` did to
//! that site over 20,000 ticks, written as a snapshot every N ticks. The two are independent files on
//! disk and that is the whole interface -- this module parses the run's own `meta.json` and
//! `entities.json` and shares no code with the simulator (CLAUDE.md, "share no code and have no IPC").
//! The format is SAD 1's "Run directory format"; CLAUDE.md's summary is the short version.
//!
//! Like the rest of the library half this module never mentions Bevy, so the CI gate exercises it
//! with `--no-default-features` and no engine.

use crate::bundle::Bundle;
use crate::tree::{mix, Life, TreeForm};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// The only run format this viewer reads. Version 4 is the first that records the **ground grid** in
/// `meta.json`, and without it there is no way to check that a run and a bundle describe the same
/// site -- which is the one thing that must not be guessed at. A version 1-3 run is rejected by name
/// rather than half-read.
pub const FORMAT_VERSION: u32 = 4;

/// The ecology grid, from `meta.json`. Not the ground grid: the ecology grid is 1 m columns and the
/// bundle's ground grid is finer (CLAUDE.md, "The run directory contract").
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Dims {
    pub x: usize,
    pub y: usize,
    pub z: usize,
    pub patch: usize,
}

/// `meta.json`'s `world` object: the ground grid the run was computed on.
#[derive(Debug, Clone, Deserialize)]
pub struct RunWorld {
    pub name: String,
    #[serde(default)]
    pub bundle: bool,
    pub ground_cell_m: f32,
    pub ground_width: usize,
    pub ground_depth: usize,
}

/// `Default` exists so [`crate::tree::Life`] has something to read when no run is loaded; it is not
/// a serde default. Every field below is still required in the file unless it says otherwise, so a
/// `meta.json` with no `dims` is a parse error rather than a 0x0 world drawn silently.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RunMeta {
    pub format_version: u32,
    pub dims: Dims,
    pub seed: u64,
    pub ticks: u64,
    pub snapshot_every: u64,
    pub snapshots: Vec<u64>,
    /// Ticks in one simulated year. The tree model needs it to read an `age` in ticks against the
    /// species' age thresholds, which shot G4c converted to years (`tree.rs`, `Life`). 0 means the
    /// run did not state one.
    #[serde(default)]
    pub year_len: u64,
    pub world: Option<RunWorld>,
    /// The species table. The simulator owns the species colours (CLAUDE.md) and this is where it
    /// says so; `palette` reads the tree's trunk and canopy colours out of it.
    #[serde(default)]
    pub species: Vec<Species>,
    /// The overlay ramps: the other half of the palette, added to the run format by `ecosim` shot S2
    /// so that the two hues an overlay runs between come from the run rather than from this viewer's
    /// copy of the `ecoview` legend. Empty on a run written before that shot, and then
    /// [`crate::palette::Overlay::ramp`] falls back and says on screen that it did.
    #[serde(default)]
    pub overlays: Vec<OverlayColors>,
    /// The subset of `params` an overlay scale is built from. Everything else in the object is
    /// ignored, and every field here is optional: a run written before a parameter existed still
    /// loads, and the viewer says on screen that the number is its own fallback rather than the
    /// simulator's (`Scale::source`).
    #[serde(default)]
    pub params: Params,
}

/// One row of `meta.json`'s `species`.
#[derive(Debug, Clone, Deserialize)]
pub struct Species {
    pub id: u32,
    pub name: String,
    pub kind: String,
    pub color: String,
    #[serde(default)]
    pub canopy_color: Option<String>,
}

/// One row of `meta.json`'s `overlays`: an overlay, and the colours a renderer draws it in.
///
/// `lo` and `hi` are the two ends of the ramp; the *numbers* those ends stand for are still this
/// viewer's reading of the run ([`crate::overlay::Scale`]), which is the division the format draws:
/// the simulator says what wet ground looks like, the viewer says how wet the wettest column is.
/// `mid` (only on `traits`, which this viewer has no overlay for) and `burnt` (only on `fire`) are
/// off the ramp, and both are optional.
#[derive(Debug, Clone, Deserialize)]
pub struct OverlayColors {
    pub name: String,
    pub lo: String,
    pub hi: String,
    #[serde(default)]
    pub mid: Option<String>,
    #[serde(default)]
    pub burnt: Option<String>,
}

/// A species' temperature tolerance curve: the four breakpoints in °C.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Curves {
    pub temp: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DiseaseParams {
    pub grazer_threshold: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FireParams {
    pub duration: Option<u32>,
}

/// `params.tree`: the tolerance curve the temperature overlay reads, plus the three age thresholds
/// the tree model reads (shot V3). All in **years** since shot G4c converted them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TreeParams {
    pub temp: Option<Vec<f32>>,
    pub young_age_years: Option<f32>,
    pub mature_age_years: Option<f32>,
    pub max_age_years: Option<f32>,
}

/// `params.bundle`: the two breakpoints of the simulator's own age-to-height map (shot V3).
///
/// `ecosim` writes this object to `meta.json` **only when it is not at its defaults**, so a run at
/// the defaults carries none of it and [`crate::tree::Life`] falls back with the fallback named on
/// screen. Every field is therefore optional twice over.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BundleParams {
    pub tree_mature_height: Option<f32>,
    pub tree_tall_height: Option<f32>,
    pub tree_tall_age_years: Option<f32>,
}

/// One row of `params.medium`: what the simulator says a surface does.
///
/// Only `plantable` is read. The three hydrology numbers beside it in the file are the simulator's
/// business -- this viewer draws no infiltration -- and leaving them out of the struct is what keeps
/// that true rather than merely unimplemented.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct MediumRow {
    /// Whether a plant roots in this medium. `None` when the run predates the field.
    pub plantable: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Params {
    pub grass: Curves,
    pub shrub: Curves,
    pub tree: TreeParams,
    pub bundle: BundleParams,
    pub disease: DiseaseParams,
    pub fire: FireParams,
    /// `params.medium`, keyed by the medium's name -- the same names the bundle publishes in its own
    /// `media` list, which is what lets a code in `medium.u8` be looked up here (shot S4).
    ///
    /// A `BTreeMap` rather than a struct of nine fields on purpose: the scene contract's media are
    /// the simulator's to name, and a tenth one added there should arrive here as data rather than
    /// as a parse error or a silently dropped row.
    pub medium: BTreeMap<String, MediumRow>,
}

/// The three tree stages `ecosim` writes.
///
/// V1 also kept a `shape()` here, mapping each stage onto the trunk-and-blob solid `ecoview` draws
/// on its 1 m voxels, and said in its own doc comment that shot V3 would replace it. It did:
/// **size now comes from `age`, continuously**, through the simulator's own age-to-height curve
/// (`tree.rs`, `Life::height_of`), so three discrete sizes no longer exist and the function is
/// gone rather than left behind unused. The stage is still read, because it is still what the
/// simulator calls the tree and an unknown one has to be visible rather than guessed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Sapling,
    Young,
    Mature,
}

impl Stage {
    pub fn parse(s: &str) -> Option<Stage> {
        match s {
            "sapling" => Some(Stage::Sapling),
            "young" => Some(Stage::Young),
            "mature" => Some(Stage::Mature),
            _ => None,
        }
    }

    /// Index into [`SnapshotTrees::stages`], oldest last.
    pub fn index(self) -> usize {
        match self {
            Stage::Sapling => 0,
            Stage::Young => 1,
            Stage::Mature => 2,
        }
    }
}

/// One row of `entities.json`.
///
/// `x` and `y` are read as floats because the simulator writes animals at continuous positions --
/// `"x":82.0` -- and trees at whole columns. V1 read both as `i32`, which parsed every Capitol
/// snapshot (that run has animals off) and failed on the first run with a grazer in it, taking the
/// whole snapshot with it. One field of one kind must not decide whether the trees draw.
#[derive(Debug, Deserialize)]
struct EntityJson {
    kind: String,
    x: f32,
    y: f32,
    #[serde(default)]
    stage: String,
    /// The simulator's own tree id, which is what makes one tree's wood its own and keeps it that
    /// way as the tree ages: the seed is this mixed with the run's seed, so scrubbing the timeline
    /// grows the same tree older rather than a different tree (shot V3).
    #[serde(default)]
    id: u64,
    /// Age in **ticks**. The only dimensional thing the simulator knows about a tree, through its
    /// own age-to-height curve (`tree.rs`, `Life::height_of`).
    #[serde(default)]
    age: u32,
}

/// One snapshot's vegetation, already in the bundle's metre frame.
#[derive(Debug, Default, Clone)]
pub struct SnapshotTrees {
    pub trees: Vec<TreeForm>,
    /// Entities whose `kind` is a tree but whose `stage` this viewer does not know. Counted rather
    /// than guessed at, and reported: a new stage in the simulator should be visible, not silent.
    pub unknown_stage: usize,
    /// Entities of every other kind (animals). V1 draws none of them.
    pub other_kinds: usize,
    /// How many trees the simulator calls sapling, young and mature, in [`Stage::index`] order.
    pub stages: [usize; 3],
    /// Tree height in metres over this snapshot: `(min, median, max)`. Zeroes when there are none.
    pub height: (f32, f32, f32),
    /// Crown light as a fraction of full sun: `(min, mean, max)`.
    pub light: (f32, f32, f32),
    /// Where that light came from, in words, for the HUD to print. When `light.bin` could not be
    /// read this names the fallback instead of hiding it.
    pub light_source: String,
}

/// A run directory on disk, plus the snapshot list from its `meta.json`.
#[derive(Debug, Clone)]
pub struct Run {
    pub dir: PathBuf,
    pub meta: RunMeta,
    /// Every `burnout` row of `events.csv`, as `(tick, patch_x, patch_y)`. The fire overlay draws a
    /// patch burnt when one of these falls between the previous snapshot and this one, which is what
    /// `ecoview` does (`loader.ts`, `burntPatches`). Kept in full because there are few of them --
    /// 600 in the 1.6 MB `events.csv` of the 20,000-tick Capitol run -- and filtering once at load
    /// is cheaper than re-reading the file at every scrub.
    pub burnouts: Vec<(u64, u32, u32)>,
    /// The species' age-to-height curve, read from `meta.json`'s `params` at load with every
    /// missing number named in [`Life::source`]. Held on the run because it is the run that says
    /// it: two runs of the same site with different parameters grow different trees.
    pub life: Life,
}

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

impl Run {
    /// Reads `<dir>/meta.json`. Rejects any format but 4, by name.
    pub fn load(dir: &Path) -> io::Result<Run> {
        let raw = std::fs::read_to_string(dir.join("meta.json"))?;
        let meta: RunMeta = serde_json::from_str(&raw)
            .map_err(|e| bad(format!("{}/meta.json: {e}", dir.display())))?;
        if meta.format_version != FORMAT_VERSION {
            return Err(bad(format!(
                "{}: run format_version {}, expected {FORMAT_VERSION}; only version {FORMAT_VERSION} \
                 records the ground grid this viewer needs to line a run up with a bundle",
                dir.display(),
                meta.format_version
            )));
        }
        if meta.snapshots.is_empty() {
            return Err(bad(format!("{}: the run has no snapshots", dir.display())));
        }
        let burnouts = crate::overlay::read_burnouts(&dir.join("events.csv"));
        let life = Life::of(&meta);
        Ok(Run {
            dir: dir.to_path_buf(),
            meta,
            burnouts,
            life,
        })
    }

    /// Is this run about the site this bundle draws? Compared rather than assumed: a run laid over
    /// the wrong ground would put trees in the air with nothing to say so.
    pub fn check_against(&self, b: &Bundle) -> Result<(), String> {
        let Some(w) = &self.meta.world else {
            return Err(format!(
                "run {} has no `world` in meta.json, so it was not computed on a ground grid and \
                 cannot be drawn over the bundle {}",
                self.dir.display(),
                b.name
            ));
        };
        if w.ground_width != b.width || w.ground_depth != b.depth {
            return Err(format!(
                "run {} is {}x{} ground cells and bundle {} is {}x{}",
                self.dir.display(),
                w.ground_width,
                w.ground_depth,
                b.name,
                b.width,
                b.depth
            ));
        }
        if (w.ground_cell_m - b.ground_cell_m).abs() > 1e-6 {
            return Err(format!(
                "run {} is at {} m ground cells and bundle {} is at {} m",
                self.dir.display(),
                w.ground_cell_m,
                b.name,
                b.ground_cell_m
            ));
        }
        // The ecology grid is always 1 m columns, so its width in columns is the site's width in
        // metres. If that disagrees with the bundle, the two cover different ground.
        if (self.meta.dims.x as f32 - b.size_m).abs() > 1e-3
            || (self.meta.dims.y as f32 - b.size_m).abs() > 1e-3
        {
            return Err(format!(
                "run {} has a {}x{} m ecology grid and bundle {} is {} m across",
                self.dir.display(),
                self.meta.dims.x,
                self.meta.dims.y,
                b.name,
                b.size_m
            ));
        }
        if w.name != b.name {
            return Err(format!(
                "run {} was computed on world '{}' and this bundle is '{}'",
                self.dir.display(),
                w.name,
                b.name
            ));
        }
        Ok(())
    }

    pub fn snapshot_count(&self) -> usize {
        self.meta.snapshots.len()
    }

    /// The tick of snapshot `i`, clamped to the run.
    pub fn tick_at(&self, i: usize) -> u64 {
        let i = i.min(self.meta.snapshots.len().saturating_sub(1));
        self.meta.snapshots.get(i).copied().unwrap_or(0)
    }

    /// `<dir>/snap_NNNNNN`, the tick zero-padded to six digits.
    pub fn snapshot_dir(&self, i: usize) -> PathBuf {
        self.dir.join(format!("snap_{:06}", self.tick_at(i)))
    }

    /// Reads snapshot `i`'s `entities.json` and turns its trees into metre-frame [`TreeForm`]s.
    ///
    /// The simulator's `x` and `y` are ecology columns, so a tree goes at the **centre** of its
    /// column. Its `z` is deliberately not used: that is the ecology grid's surface level at 1 m,
    /// and the viewer draws terrain on the bundle's finer lattice, so a tree placed at the ecology
    /// level would float or sink by up to a metre against the ground the user can see. The base
    /// comes from the viewer's own column instead, which is what [`crate::VoxelWorld`] does for
    /// bundle trees already.
    ///
    /// Every other number in the form comes from the run too: the height from `age` through the
    /// run's own curve, the light from `light.bin` at the crown's level, the seed from the run's
    /// seed mixed with the simulator's tree id. Nothing here is drawn from a size the simulator
    /// never computed -- see `tree.rs` for the one input (biomass) the run does not carry.
    pub fn trees_at(&self, i: usize) -> io::Result<SnapshotTrees> {
        let path = self.snapshot_dir(i).join("entities.json");
        let raw = std::fs::read_to_string(&path)?;
        let list: Vec<EntityJson> =
            serde_json::from_str(&raw).map_err(|e| bad(format!("{}: {e}", path.display())))?;
        let sun = CrownLight::of(self, i);
        // One ecology column is 1 m wide whatever the ground cell is (CLAUDE.md: "the ecology grid
        // stays at 1 m columns"), so the half-metre offset puts the trunk at the column's centre
        // rather than at its south-west corner.
        let mut out = SnapshotTrees {
            light_source: sun.source.clone(),
            ..SnapshotTrees::default()
        };
        for e in list {
            if e.kind != "tree" {
                out.other_kinds += 1;
                continue;
            }
            let Some(stage) = Stage::parse(&e.stage) else {
                out.unknown_stage += 1;
                continue;
            };
            out.stages[stage.index()] += 1;
            let (col_x, col_y) = (e.x.floor(), e.y.floor());
            let mut t = TreeForm::grown(
                col_x + 0.5,
                col_y + 0.5,
                self.life.height_of(e.age),
                1.0,
                mix(self.meta.seed, e.id),
            );
            // The simulator's own reading of this tree's light. See `CrownLight` for the sample
            // this shot measured first and rejected.
            t.light = sun.at(col_x, col_y);
            out.trees.push(t);
        }
        out.summarise();
        Ok(out)
    }

    /// The snapshot whose tick is nearest `tick`, for `--tick` and for a BRP caller who thinks in
    /// ticks rather than in indices.
    pub fn index_of_tick(&self, tick: u64) -> usize {
        let mut best = 0usize;
        let mut best_d = u64::MAX;
        for (i, &t) in self.meta.snapshots.iter().enumerate() {
            let d = t.abs_diff(tick);
            if d < best_d {
                best_d = d;
                best = i;
            }
        }
        best
    }
}

impl SnapshotTrees {
    /// Fills in the height and light spans. Kept out of the read loop so the numbers the HUD prints
    /// are measured over the trees that were actually built, not accumulated as they were guessed.
    fn summarise(&mut self) {
        if self.trees.is_empty() {
            return;
        }
        let mut h: Vec<f32> = self.trees.iter().map(|t| t.height).collect();
        h.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        self.height = (h[0], h[h.len() / 2], h[h.len() - 1]);
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        let mut sum = 0.0f32;
        for t in &self.trees {
            lo = lo.min(t.light);
            hi = hi.max(t.light);
            sum += t.light;
        }
        self.light = (lo, sum / self.trees.len() as f32, hi);
    }
}

/// `light.bin` and `height.bin` of one snapshot, read so a tree can be asked how much sun it gets.
///
/// **Where the sample is taken, and why it is not the crown's centre.** This shot first sampled
/// `light.bin` at the middle of the procedural crown, 13.7 m up on a 20 m tree, which is where the
/// leaves are. Measured over the reference run that returns 1.00 of full sun for every tree at every
/// tick (MEASUREMENTS.md, V3): the simulator's light field is computed for a canopy one to three
/// voxels tall, so everything above about 3 m is sky. Sampling there makes the light input dead.
///
/// So the sample is the one the **simulator itself** calls this tree's light: `light.bin` at the
/// first voxel above the column's surface, which is `ecosim`'s `World::surface_light` -- the number
/// its own germination and growth curves read (`ecosim/src/world.rs`). The viewer expresses a number
/// the simulator computed rather than inventing a better place to measure it
/// (`overnight/DIRECTION-native-viewer.md`).
///
/// The light overlay reads the same two files the same way (`overlay.rs`, `fields_at`), so a tree
/// with a sparse crown stands on ground the overlay draws dark. Read separately because the tree
/// model is wanted with or without an overlay loaded.
pub struct CrownLight {
    dims: Dims,
    height: Vec<u8>,
    light: Vec<u8>,
    /// What the light is, in words, or the named fallback when the files could not be read.
    pub source: String,
    /// False when the numbers below are this viewer's fallback rather than the run's.
    pub from_file: bool,
}

impl CrownLight {
    pub fn of(run: &Run, i: usize) -> CrownLight {
        let d = run.meta.dims;
        let cols = d.columns();
        let dir = run.snapshot_dir(i);
        let h = std::fs::read(dir.join("height.bin"));
        let l = std::fs::read(dir.join("light.bin"));
        match (h, l) {
            (Ok(h), Ok(l)) if cols > 0 && d.z > 0 && h.len() == cols && l.len() == cols * d.z => {
                CrownLight {
                    dims: d,
                    height: h,
                    light: l,
                    source: "light.bin at the tree's own column, the simulator's surface sample"
                        .into(),
                    from_file: true,
                }
            }
            // A snapshot with no readable light is drawn in full sun rather than in the dark, and
            // says so: a viewer that silently drew every crown shaded would look like an ecology
            // result. The file is checked against `dims` for the same reason `fields_at` does it.
            _ => CrownLight {
                dims: d,
                height: Vec::new(),
                light: Vec::new(),
                source: format!(
                    "this viewer's fallback: {} has no light.bin and height.bin pair matching \
                     dims, so every crown is drawn in full sun",
                    dir.display()
                ),
                from_file: false,
            },
        }
    }

    /// The light at column `(x, y)`, as a fraction of full sun. Out of bounds, or with no file,
    /// this is full sun.
    pub fn at(&self, x: f32, y: f32) -> f32 {
        if !self.from_file || x < 0.0 || y < 0.0 {
            return 1.0;
        }
        let d = self.dims;
        let (xi, yi) = (x as usize, y as usize);
        if xi >= d.x || yi >= d.y {
            return 1.0;
        }
        let c = xi + d.x * yi;
        // `height` is the surface level and the light of a solid voxel is zero, so the sample is the
        // first voxel of air above it -- the same index `ecosim`'s `surface_light` uses.
        let z = (self.height[c] as usize + 1).min(d.z - 1);
        // x-fastest, z slowest, as everywhere in the contract.
        self.light[c + d.columns() * z] as f32 / crate::overlay::FULL_SUN
    }
}
