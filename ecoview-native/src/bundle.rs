//! The world bundle on disk, read exactly as `ecoview` reads it: from the committed files, with no
//! code shared with `ecosim` (CLAUDE.md, "share no code and have no IPC"). Format: docs/SCENE-CONTRACT.md.

use serde::Deserialize;
use std::io;
use std::path::Path;

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
}

/// A tree as the scene contract writes it: metres from the south-west corner.
#[derive(Debug, Clone, Deserialize)]
pub struct Tree {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub crown_radius: f32,
    pub crown_base: f32,
}

/// A shrub: an ellipse in plan, `angle` radians from east.
#[derive(Debug, Clone, Deserialize)]
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
        })
    }

    /// The synthetic stress world: `n`x`n` cells, value noise terrain, `buildings` random blocks and
    /// scattered trees. Its cell size is deliberately **not** 0.5 m, so a hard-coded edge fails loudly
    /// rather than hiding (V0-spike.md, correction 4). Deterministic from `seed`.
    pub fn stress(n: usize, cell_m: f32, buildings: usize, seed: u64) -> Bundle {
        let mut rng = Lcg(seed);
        let media: Vec<String> = ["soil", "lawn", "bed", "mulch", "gravel", "concrete", "asphalt", "roof", "water"]
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
        }
    }
}

/// A 64-bit LCG. The stress world only has to be reproducible, so it does not need the simulator's
/// ChaCha8 and must not depend on it.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 11
    }
}
