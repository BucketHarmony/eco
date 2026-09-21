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

use crate::bundle::{Bundle, Tree};
use serde::Deserialize;
use std::io;
use std::path::{Path, PathBuf};

/// The only run format this viewer reads. Version 4 is the first that records the **ground grid** in
/// `meta.json`, and without it there is no way to check that a run and a bundle describe the same
/// site -- which is the one thing that must not be guessed at. A version 1-3 run is rejected by name
/// rather than half-read.
pub const FORMAT_VERSION: u32 = 4;

/// The ecology grid, from `meta.json`. Not the ground grid: the ecology grid is 1 m columns and the
/// bundle's ground grid is finer (CLAUDE.md, "The run directory contract").
#[derive(Debug, Clone, Copy, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct RunMeta {
    pub format_version: u32,
    pub dims: Dims,
    pub seed: u64,
    pub ticks: u64,
    pub snapshot_every: u64,
    pub snapshots: Vec<u64>,
    pub world: Option<RunWorld>,
    /// The species table. The simulator owns the species colours (CLAUDE.md) and this is where it
    /// says so; `palette` reads the tree's trunk and canopy colours out of it.
    #[serde(default)]
    pub species: Vec<Species>,
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Params {
    pub grass: Curves,
    pub shrub: Curves,
    pub tree: Curves,
    pub disease: DiseaseParams,
    pub fire: FireParams,
}

/// The three tree stages `ecosim` writes, and the shape each one draws as.
///
/// The simulator gives a tree no height: `entities.json` carries `stage` and `age` and nothing
/// dimensional, because ecology happens on a 1 m grid where a tree is one to three voxels tall. The
/// sizes below are that same shape expressed in **metres**, so the viewer's own lattice -- whatever
/// the bundle's cell size is -- draws it at the scale the simulator means. They match what `ecoview`
/// draws (`ecoview/src/entities.ts`, `canopyVoxels`): a sapling is a bare trunk voxel, a young tree
/// is a trunk with one canopy voxel over it, a mature tree is a trunk under a 3x3x2 crown.
///
/// Shot V3 replaces all of this with procedural branching geometry. Until then the viewer draws the
/// simulator's own blocks, and does not invent a size the simulator never computed.
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

    /// `(height, crown_base, crown_radius)` in metres. A zero crown radius means no crown at all.
    pub fn shape(self) -> (f32, f32, f32) {
        match self {
            Stage::Sapling => (1.0, 0.0, 0.0),
            Stage::Young => (2.0, 1.0, 0.5),
            Stage::Mature => (3.0, 1.0, 1.5),
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
}

/// One snapshot's vegetation, already in the bundle's metre frame.
#[derive(Debug, Default, Clone)]
pub struct SnapshotTrees {
    pub trees: Vec<Tree>,
    /// Entities whose `kind` is a tree but whose `stage` this viewer does not know. Counted rather
    /// than guessed at, and reported: a new stage in the simulator should be visible, not silent.
    pub unknown_stage: usize,
    /// Entities of every other kind (animals). V1 draws none of them.
    pub other_kinds: usize,
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
        Ok(Run {
            dir: dir.to_path_buf(),
            meta,
            burnouts,
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

    /// Reads snapshot `i`'s `entities.json` and turns its trees into metre-frame [`Tree`]s.
    ///
    /// The simulator's `x` and `y` are ecology columns, so a tree goes at the **centre** of its
    /// column. Its `z` is deliberately not used: that is the ecology grid's surface level at 1 m,
    /// and the viewer draws terrain on the bundle's finer lattice, so a tree placed at the ecology
    /// level would float or sink by up to a metre against the ground the user can see. The base
    /// comes from the viewer's own column instead, which is what [`crate::VoxelWorld`] does for
    /// bundle trees already.
    pub fn trees_at(&self, i: usize) -> io::Result<SnapshotTrees> {
        let path = self.snapshot_dir(i).join("entities.json");
        let raw = std::fs::read_to_string(&path)?;
        let list: Vec<EntityJson> =
            serde_json::from_str(&raw).map_err(|e| bad(format!("{}: {e}", path.display())))?;
        // One ecology column is 1 m wide whatever the ground cell is (CLAUDE.md: "the ecology grid
        // stays at 1 m columns"), so the half-metre offset puts the trunk at the column's centre
        // rather than at its south-west corner.
        let mut out = SnapshotTrees::default();
        for e in list {
            if e.kind != "tree" {
                out.other_kinds += 1;
                continue;
            }
            let Some(stage) = Stage::parse(&e.stage) else {
                out.unknown_stage += 1;
                continue;
            };
            let (height, crown_base, crown_radius) = stage.shape();
            out.trees.push(Tree {
                x: e.x.floor() + 0.5,
                y: e.y.floor() + 0.5,
                height,
                crown_radius,
                crown_base,
            });
        }
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
