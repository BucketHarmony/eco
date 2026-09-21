//! The world bundle on disk, read exactly as `ecoview` reads it: from the committed files, with no
//! code shared with `ecosim` (CLAUDE.md, "share no code and have no IPC"). Format: docs/SCENE-CONTRACT.md.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// The two strings `bundle.json` must carry, and which a save writes back unchanged.
pub const BUNDLE_FORMAT: &str = "ecosim-world-bundle";
pub const BUNDLE_VERSION: u32 = 2;

#[derive(Debug, Deserialize)]
struct BundleJson {
    format: String,
    version: u32,
    name: String,
    size_m: f32,
    ground_cell_m: f32,
    ground_width: usize,
    ground_depth: usize,
    media: Vec<String>,
    /// Provenance: the data source, its licence and the crop centre. Nothing in the viewer reads
    /// it, and a save writes it back **verbatim**, because an edited copy of the Capitol is still
    /// made of USGS LiDAR and the credit travels with the file (`ecosim/worlds/capitol/README.md`).
    #[serde(default)]
    source: String,
}

/// A tree as the scene contract writes it: metres from the south-west corner.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tree {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub crown_radius: f32,
    pub crown_base: f32,
}

/// A shrub: an ellipse in plan, `angle` radians from east.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Shrub {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub rx: f32,
    pub ry: f32,
    pub angle: f32,
}

/// One world bundle in memory. The three grids are indexed `x + width * y`, x east, y north.
#[derive(Debug, Clone)]
pub struct Bundle {
    pub name: String,
    pub size_m: f32,
    /// The cube edge, in metres. Never assume 0.5: the Capitol is 0.5 and other bundles are not
    /// (ecoview/DECISIONS.md, "E3 block world").
    pub ground_cell_m: f32,
    pub width: usize,
    pub depth: usize,
    pub media: Vec<String>,
    pub ground_h: Vec<f32>,
    pub medium: Vec<u8>,
    pub building_h: Vec<f32>,
    pub trees: Vec<Tree>,
    pub shrubs: Vec<Shrub>,
    /// `bundle.json`'s `source`, carried through a save untouched. Empty for a synthetic world.
    pub source: String,
    /// Where this bundle was read from, or `None` for a synthetic one. A save copies `pipes.json`
    /// from here rather than writing an empty list: the viewer has no pipe model and shot G6 will,
    /// so dropping the drains on the way through an edit would silently change the site.
    pub dir: Option<std::path::PathBuf>,
}

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn read_f32(path: &Path, cells: usize) -> io::Result<Vec<f32>> {
    let raw = std::fs::read(path)?;
    if raw.len() != cells * 4 {
        return Err(bad(format!(
            "{}: {} bytes, expected {}",
            path.display(),
            raw.len(),
            cells * 4
        )));
    }
    Ok(raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    let raw = std::fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(|e| bad(format!("{}: {e}", path.display())))
}

impl Bundle {
    /// Reads `<dir>/bundle.json` and the five files beside it. Rejects anything but bundle v2.
    pub fn load(dir: &Path) -> io::Result<Bundle> {
        let meta: BundleJson = read_json(&dir.join("bundle.json"))?;
        if meta.format != "ecosim-world-bundle" || meta.version != 2 {
            return Err(bad(format!(
                "{}: format {} version {}, expected ecosim-world-bundle v2",
                dir.display(),
                meta.format,
                meta.version
            )));
        }
        let cells = meta.ground_width * meta.ground_depth;
        let medium = std::fs::read(dir.join("medium.u8"))?;
        if medium.len() != cells {
            return Err(bad(format!(
                "medium.u8: {} bytes, expected {cells}",
                medium.len()
            )));
        }
        Ok(Bundle {
            name: meta.name,
            size_m: meta.size_m,
            ground_cell_m: meta.ground_cell_m,
            width: meta.ground_width,
            depth: meta.ground_depth,
            media: meta.media,
            ground_h: read_f32(&dir.join("ground_h.f32"), cells)?,
            medium,
            building_h: read_f32(&dir.join("building_h.f32"), cells)?,
            trees: read_json(&dir.join("trees.json"))?,
            shrubs: read_json(&dir.join("shrubs.json"))?,
            source: meta.source,
            dir: Some(dir.to_path_buf()),
        })
    }

    /// The synthetic stress world: `n`x`n` cells, value noise terrain, `buildings` random blocks and
    /// scattered trees. Its cell size is deliberately **not** 0.5 m, so a hard-coded edge fails loudly
    /// rather than hiding (V0-spike.md, correction 4). Deterministic from `seed`.
    pub fn stress(n: usize, cell_m: f32, buildings: usize, seed: u64) -> Bundle {
        let mut rng = Lcg(seed);
        let media: Vec<String> = [
            "soil", "lawn", "bed", "mulch", "gravel", "concrete", "asphalt", "roof", "water",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let mut ground_h = vec![0.0f32; n * n];
        for y in 0..n {
            for x in 0..n {
                let fx = x as f32 / n as f32;
                let fy = y as f32 / n as f32;
                // Three octaves of a smooth analytic field: terrain with slopes and steps, no data file.
                let h = 6.0 * (fx * 6.28).sin() * (fy * 6.28).cos()
                    + 2.5 * (fx * 18.85 + 1.3).sin() * (fy * 12.57).sin()
                    + 0.8 * (fx * 50.3).cos() * (fy * 44.0 + 0.7).cos();
                ground_h[x + n * y] = h + 10.0;
            }
        }
        let mut medium = vec![1u8; n * n]; // lawn
        let mut building_h = vec![0.0f32; n * n];
        for _ in 0..buildings {
            let w = 6 + (rng.next() as usize % 20);
            let d = 6 + (rng.next() as usize % 20);
            let x0 = rng.next() as usize % n.saturating_sub(w).max(1);
            let y0 = rng.next() as usize % n.saturating_sub(d).max(1);
            let h = 3.0 + (rng.next() % 40) as f32;
            for y in y0..(y0 + d).min(n) {
                for x in x0..(x0 + w).min(n) {
                    medium[x + n * y] = 7; // roof
                    building_h[x + n * y] = h;
                }
            }
        }
        let trees = (0..200)
            .map(|_| Tree {
                x: (rng.next() % (n as u64)) as f32 * cell_m,
                y: (rng.next() % (n as u64)) as f32 * cell_m,
                height: 6.0 + (rng.next() % 12) as f32,
                crown_radius: 2.0 + (rng.next() % 4) as f32,
                crown_base: 2.0,
            })
            .collect();
        Bundle {
            name: "stress".into(),
            size_m: n as f32 * cell_m,
            ground_cell_m: cell_m,
            width: n,
            depth: n,
            media,
            ground_h,
            medium,
            building_h,
            trees,
            shrubs: Vec::new(),
            source: String::new(),
            dir: None,
        }
    }

    /// Writes this bundle, with `grids` in place of its own three, into a **fresh** directory.
    ///
    /// This is the first half of the round trip: the viewer edits columns in [`crate::VoxelWorld`],
    /// and `ecosim` reads a directory, so the edited grids have to become files before anything can
    /// be run on them. `grids` is `(ground_h, medium, building_h)` straight off the voxel world;
    /// passing them in rather than mutating the bundle keeps the loaded bundle the site as it was
    /// read, which is what `--world` still means after twenty edits.
    ///
    /// **It never writes over the bundle it came from.** `dir` is a scratch directory the caller
    /// made; the committed Capitol is 22 MB of public LiDAR and an editor that could overwrite it
    /// by mis-clicking is an editor nobody should run. The one guard is here rather than in the
    /// caller so every caller has it.
    ///
    /// `pipes.json` is copied byte for byte from [`Bundle::dir`] when there is one, and written as
    /// `[]` when there is not. Everything else is this bundle's own fields.
    pub fn save(&self, dir: &Path, grids: (&[f32], &[u8], &[f32]), note: &str) -> io::Result<()> {
        if let Some(src) = &self.dir {
            if same_dir(src, dir) {
                return Err(bad(format!(
                    "refusing to write the edited bundle over its own source at {}",
                    src.display()
                )));
            }
        }
        let (ground_h, medium, building_h) = grids;
        let cells = self.width * self.depth;
        for (name, len) in [
            ("ground_h", ground_h.len()),
            ("medium", medium.len()),
            ("building_h", building_h.len()),
        ] {
            if len != cells {
                return Err(bad(format!(
                    "{name} has {len} cells and the bundle is {}x{} = {cells}",
                    self.width, self.depth
                )));
            }
        }
        std::fs::create_dir_all(dir)?;
        // The site's drains, copied byte for byte. Read before `bundle.json` is written because the
        // counts below are of what actually goes in the directory, not of what was hoped for.
        let pipes = match &self.dir {
            Some(src) => std::fs::read(src.join("pipes.json")).unwrap_or_else(|_| b"[]".to_vec()),
            None => b"[]".to_vec(),
        };
        let pipe_count = serde_json::from_slice::<serde_json::Value>(&pipes)
            .ok()
            .and_then(|v| v.as_array().map(Vec::len))
            .unwrap_or(0);
        // `edited_by` is not in the scene contract. `ecosim`'s reader ignores unknown keys by
        // design ("a later exporter may add fields"), and a bundle that has been through an editor
        // should say so on its face rather than only in the directory it happens to sit in.
        let meta = serde_json::json!({
            "format": BUNDLE_FORMAT,
            "version": BUNDLE_VERSION,
            "name": self.name,
            "size_m": self.size_m,
            "ground_cell_m": self.ground_cell_m,
            "ground_width": self.width,
            "ground_depth": self.depth,
            "media": self.media,
            "source": self.source,
            // The exporter writes these and nothing reads them; they are a human's check that the
            // directory holds what its name says, so they are recounted rather than carried over.
            "counts": {
                "trees": self.trees.len(),
                "shrubs": self.shrubs.len(),
                "pipes": pipe_count,
            },
            "edited_by": note,
        });
        std::fs::write(dir.join("bundle.json"), serde_json::to_vec_pretty(&meta)?)?;
        std::fs::write(dir.join("ground_h.f32"), f32_bytes(ground_h))?;
        std::fs::write(dir.join("medium.u8"), medium)?;
        std::fs::write(dir.join("building_h.f32"), f32_bytes(building_h))?;
        std::fs::write(dir.join("trees.json"), serde_json::to_vec(&self.trees)?)?;
        std::fs::write(dir.join("shrubs.json"), serde_json::to_vec(&self.shrubs)?)?;
        std::fs::write(dir.join("pipes.json"), pipes)?;
        Ok(())
    }
}

/// Little-endian f32s, the way both projects read them.
fn f32_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// Are these the same directory? Compared by canonical path when both exist, and textually when
/// they do not -- the destination is usually about to be created, so `canonicalize` fails on it.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// A 64-bit LCG. The stress world only has to be reproducible, so it does not need the simulator's
/// ChaCha8 and must not depend on it.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }
}
