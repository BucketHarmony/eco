//! The ecological overlays: what each one reads out of a snapshot, and what its ramp runs between.
//!
//! Shot V2. The six overlays -- light, moisture, fertility, temperature, crowding and fire -- are
//! the fields `ecosim` already writes every snapshot, so nothing here computes ecology. It reads,
//! scales and bands. **Every number in a scale comes from the run's own `meta.json`**, and where the
//! file does not carry one the [`Scale`] says so in words rather than substituting a constant
//! silently (DECISIONS.md, "V2: what meta.json owns and what it does not").
//!
//! Like the rest of the library half this module never mentions Bevy.

use crate::palette::{Overlay, BANDS, FIRE_BURNT, FIRE_QUIET};
use crate::run::{Dims, Params, Run, RunMeta};
use serde::Deserialize;
use std::io;
use std::path::Path;

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Every `burnout` row of `events.csv`, found by header name rather than by column number.
///
/// A missing or unreadable `events.csv` is not fatal: the fire overlay then shows what is alight now
/// and nothing that has already burnt, which is a smaller picture and not a wrong one.
pub(crate) fn read_burnouts(path: &Path) -> Vec<(u64, u32, u32)> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = raw.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let cols: Vec<&str> = header.split(',').map(str::trim).collect();
    let at = |name: &str| cols.iter().position(|c| *c == name);
    let (Some(i_tick), Some(i_kind), Some(i_px), Some(i_py)) =
        (at("tick"), at("kind"), at("patch_x"), at("patch_y"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() <= i_py || f.get(i_kind) != Some(&"burnout") {
            continue;
        }
        if let (Ok(t), Ok(px), Ok(py)) = (
            f[i_tick].parse::<u64>(),
            f[i_px].parse::<u32>(),
            f[i_py].parse::<u32>(),
        ) {
            out.push((t, px, py));
        }
    }
    out
}

impl Dims {
    pub fn columns(&self) -> usize {
        self.x * self.y
    }

    /// The patch grid, in patches.
    pub fn patch_grid(&self) -> (usize, usize) {
        let p = self.patch.max(1);
        (self.x.div_ceil(p), self.y.div_ceil(p))
    }

    pub fn patch_count(&self) -> usize {
        let (px, py) = self.patch_grid();
        px * py
    }

    /// The patch an ecology column belongs to.
    pub fn patch_of(&self, x: usize, y: usize) -> usize {
        let p = self.patch.max(1);
        let (px, _) = self.patch_grid();
        (x / p) + px * (y / p)
    }
}

/// What one overlay's ramp runs between, and where each of those two numbers came from.
///
/// `source` is shown on screen beside the range, and that is the point of the shot: a viewer that
/// draws a blue-to-red temperature map without saying what blue is has not shown you the simulation,
/// it has shown you a picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Scale {
    pub lo: f32,
    pub hi: f32,
    pub unit: &'static str,
    pub source: String,
}

/// The fallbacks, used only when the run's `meta.json` does not carry the number. They are
/// `ecoview/src/world.ts`'s constants, so a run too old to state its own scale is drawn the way the
/// browser viewer draws it rather than in some third way.
const FALLBACK_TEMP: (f32, f32) = (0.0, 30.0);
const FALLBACK_CROWDING_FULL: f32 = 32.0;
const FALLBACK_FIRE_DURATION: f32 = 3.0;

fn viewer_fallback(what: &str) -> String {
    format!("this viewer's fallback: meta.json has no {what}")
}

impl Scale {
    /// The scale for an overlay, read from the run's `meta.json`.
    pub fn of(o: Overlay, meta: &RunMeta) -> Scale {
        let p = &meta.params;
        match o {
            // The byte in `light.bin` is `FULL_SUN x` the fraction of full sun with `FULL_SUN = 255`
            // (ecosim/UNITS.md, 3.6), so the range is the format's and not a choice of this viewer's.
            Overlay::Light => Scale {
                lo: 0.0,
                hi: 1.0,
                unit: "of full sun",
                source: "light.bin: 255 x the fraction of full sun".into(),
            },
            // 255 x (soil water / available water capacity) (ecosim/UNITS.md, section 1).
            Overlay::Moisture => Scale {
                lo: 0.0,
                hi: 1.0,
                unit: "of available water capacity",
                source: "moisture.bin: 255 x soil water / AWC".into(),
            },
            // Still a bare index: UNITS.md marks fertility `deferred` and row G5 replaces the field
            // with N, P and K. Drawing it 0-255 is drawing exactly what the file holds.
            Overlay::Fertility => Scale {
                lo: 0.0,
                hi: 255.0,
                unit: "index (not yet physical)",
                source: "fertility.bin: the 0-255 index, deferred in UNITS.md".into(),
            },
            Overlay::Temperature => match temp_span(p) {
                Some((lo, hi)) => Scale {
                    lo,
                    hi,
                    unit: "C",
                    source: "params.{grass,shrub,tree}.temp, the species tolerance curves".into(),
                },
                None => Scale {
                    lo: FALLBACK_TEMP.0,
                    hi: FALLBACK_TEMP.1,
                    unit: "C",
                    source: viewer_fallback("species temp curves"),
                },
            },
            // Twice the threshold at which crowding disease starts, which is `ecoview`'s rule for
            // the same overlay -- but read from the run instead of written into the viewer.
            Overlay::Crowding => match p.disease.grazer_threshold {
                Some(t) => Scale {
                    lo: 0.0,
                    hi: 2.0 * t,
                    unit: "grazers per patch",
                    source: "2 x params.disease.grazer_threshold".into(),
                },
                None => Scale {
                    lo: 0.0,
                    hi: FALLBACK_CROWDING_FULL,
                    unit: "grazers per patch",
                    source: viewer_fallback("disease.grazer_threshold"),
                },
            },
            Overlay::Fire => match p.fire.duration {
                Some(d) => Scale {
                    lo: 0.0,
                    hi: d as f32,
                    unit: "ticks left",
                    source: "params.fire.duration".into(),
                },
                None => Scale {
                    lo: 0.0,
                    hi: FALLBACK_FIRE_DURATION,
                    unit: "ticks left",
                    source: viewer_fallback("fire.duration"),
                },
            },
            Overlay::Surface => Scale {
                lo: 0.0,
                hi: 0.0,
                unit: "",
                source: "the scene contract's media, not the run".into(),
            },
        }
    }

    /// Is this scale the simulator's, or this viewer's guess at it?
    pub fn from_meta(&self) -> bool {
        !self.source.starts_with("this viewer's fallback")
    }
}

/// The widest temperature any species in `meta.json` has an opinion about: the lowest first
/// breakpoint and the highest last one over grass, shrub and tree. Outside it every tolerance curve
/// is zero, so it is exactly the interval where a colour means something.
fn temp_span(p: &Params) -> Option<(f32, f32)> {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for c in [&p.grass, &p.shrub, &p.tree] {
        if let Some(t) = &c.temp {
            if let (Some(a), Some(b)) = (t.first(), t.last()) {
                lo = lo.min(*a);
                hi = hi.max(*b);
            }
        }
    }
    (lo.is_finite() && hi > lo).then_some((lo, hi))
}

/// One snapshot's ecological fields, read from the files the run directory contract names.
#[derive(Debug, Clone, Default)]
pub struct Fields {
    /// Per ecology column.
    pub moisture: Vec<u8>,
    pub fertility: Vec<u8>,
    /// Light one voxel above each column's surface, the voxel a seedling would sit in. This is the
    /// sample `ecoview` takes (`world.ts`, `columnColor`): the surface voxel itself is ground and is
    /// always dark.
    pub light: Vec<u8>,
    /// Per patch.
    pub temperature: Vec<f32>,
    pub burning: Vec<u32>,
    pub grazers: Vec<u32>,
    pub burnt: Vec<bool>,
}

#[derive(Debug, Deserialize)]
struct PatchJson {
    temperature: f32,
    #[serde(default)]
    burning_ticks_left: u32,
}

/// Animals move continuously, so their `x` and `y` are floats in the file; the column one stands in
/// is the floor of the position (`src/run.rs`, `EntityJson`).
#[derive(Debug, Deserialize)]
struct GrazerJson {
    kind: String,
    x: f32,
    y: f32,
}

/// What a field actually held at this snapshot, in the scale's own unit. Reported on screen so the
/// ramp is never the only evidence: a flat picture and a range of 0.02 are the same fact, and only
/// one of them is visible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
}

fn read_exact_len(path: &Path, want: usize) -> io::Result<Vec<u8>> {
    let raw = std::fs::read(path)?;
    if raw.len() != want {
        return Err(bad(format!(
            "{}: {} bytes, expected {want}",
            path.display(),
            raw.len()
        )));
    }
    Ok(raw)
}

impl Run {
    /// Reads snapshot `i`'s fields. Every file is checked against the `dims` in `meta.json` rather
    /// than trusted: a short file read as a field would silently draw the wrong half of the site.
    pub fn fields_at(&self, i: usize) -> io::Result<Fields> {
        let d = self.meta.dims;
        let dir = self.snapshot_dir(i);
        let cols = d.columns();
        let moisture = read_exact_len(&dir.join("moisture.bin"), cols)?;
        let fertility = read_exact_len(&dir.join("fertility.bin"), cols)?;
        let height = read_exact_len(&dir.join("height.bin"), cols)?;
        let light_vox = read_exact_len(&dir.join("light.bin"), cols * d.z)?;
        // x-fastest, z slowest: `x + width * (y + depth * z)` (CLAUDE.md, the run directory contract).
        let light = (0..cols)
            .map(|c| light_vox[c + cols * (height[c] as usize + 1).min(d.z - 1)])
            .collect();

        let raw = std::fs::read_to_string(dir.join("patches.json"))?;
        let patches: Vec<PatchJson> = serde_json::from_str(&raw)
            .map_err(|e| bad(format!("{}/patches.json: {e}", dir.display())))?;
        if patches.len() != d.patch_count() {
            return Err(bad(format!(
                "{}/patches.json: {} patches, expected {}",
                dir.display(),
                patches.len(),
                d.patch_count()
            )));
        }
        let temperature = patches.iter().map(|p| p.temperature).collect();
        let burning = patches.iter().map(|p| p.burning_ticks_left).collect();

        let mut grazers = vec![0u32; d.patch_count()];
        let raw = std::fs::read_to_string(dir.join("entities.json"))?;
        let list: Vec<GrazerJson> = serde_json::from_str(&raw)
            .map_err(|e| bad(format!("{}/entities.json: {e}", dir.display())))?;
        for e in list.iter().filter(|e| e.kind == "grazer") {
            let (x, y) = (e.x.floor(), e.y.floor());
            if x >= 0.0 && y >= 0.0 && (x as usize) < d.x && (y as usize) < d.y {
                grazers[d.patch_of(x as usize, y as usize)] += 1;
            }
        }

        // A patch is drawn burnt if it burnt out since the previous snapshot. Before the first
        // snapshot there is no previous tick, so nothing is.
        let (px, py) = d.patch_grid();
        let mut burnt = vec![false; d.patch_count()];
        if i > 0 {
            let (from, to) = (self.tick_at(i - 1), self.tick_at(i));
            for &(tick, bx, by) in &self.burnouts {
                if tick > from && tick <= to && (bx as usize) < px && (by as usize) < py {
                    burnt[bx as usize + px * by as usize] = true;
                }
            }
        }

        Ok(Fields {
            moisture,
            fertility,
            light,
            temperature,
            burning,
            grazers,
            burnt,
        })
    }
}

/// The band a value falls in, clamped at both ends. The top of the range is the last band, not one
/// past it.
pub fn band_of(v: f32, s: &Scale) -> u8 {
    let t = if s.hi > s.lo {
        (v - s.lo) / (s.hi - s.lo)
    } else {
        0.0
    };
    ((t.clamp(0.0, 1.0) * BANDS as f32) as usize).min(BANDS - 1) as u8
}

impl Fields {
    /// The value this overlay reads at one ecology column, in the scale's unit.
    pub fn value(&self, o: Overlay, d: &Dims, x: usize, y: usize) -> f32 {
        let c = x + d.x * y;
        let p = d.patch_of(x, y);
        match o {
            Overlay::Light => self.light.get(c).map_or(0.0, |v| *v as f32 / 255.0),
            Overlay::Moisture => self.moisture.get(c).map_or(0.0, |v| *v as f32 / 255.0),
            Overlay::Fertility => self.fertility.get(c).map_or(0.0, |v| *v as f32),
            Overlay::Temperature => self.temperature.get(p).copied().unwrap_or(0.0),
            Overlay::Crowding => self.grazers.get(p).map_or(0.0, |v| *v as f32),
            Overlay::Fire => self.burning.get(p).map_or(0.0, |v| *v as f32),
            Overlay::Surface => 0.0,
        }
    }

    /// One band index per ecology column, plus what the field held.
    ///
    /// Fire does not ramp from its own bottom: band 0 is ground that is neither alight nor freshly
    /// burnt and band 1 is ground that burnt out since the previous snapshot, so the ramp above them
    /// is burning only. The other five overlays are a plain linear ramp over [`Scale`].
    pub fn bands(&self, o: Overlay, d: &Dims, s: &Scale) -> (Vec<u8>, FieldStats) {
        let mut out = vec![0u8; d.columns()];
        let (mut min, mut max, mut sum) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f64);
        for y in 0..d.y {
            for x in 0..d.x {
                let v = self.value(o, d, x, y);
                min = min.min(v);
                max = max.max(v);
                sum += v as f64;
                out[x + d.x * y] = if o == Overlay::Fire {
                    self.fire_band(d, x, y, s)
                } else {
                    band_of(v, s)
                };
            }
        }
        let n = d.columns().max(1);
        (
            out,
            FieldStats {
                min: if min.is_finite() { min } else { 0.0 },
                max: if max.is_finite() { max } else { 0.0 },
                mean: (sum / n as f64) as f32,
            },
        )
    }

    fn fire_band(&self, d: &Dims, x: usize, y: usize, s: &Scale) -> u8 {
        let p = d.patch_of(x, y);
        let alight = self.burning.get(p).copied().unwrap_or(0);
        if alight == 0 {
            return if self.burnt.get(p).copied().unwrap_or(false) {
                FIRE_BURNT
            } else {
                FIRE_QUIET
            };
        }
        let first = FIRE_BURNT as usize + 1;
        let span = (BANDS - first - 1) as f32;
        let t = if s.hi > 1.0 {
            (alight as f32 - 1.0) / (s.hi - 1.0)
        } else {
            1.0
        };
        (first + (t.clamp(0.0, 1.0) * span).round() as usize).min(BANDS - 1) as u8
    }

    /// How many patches are alight, and how many burnt out since the previous snapshot. Fire is the
    /// overlay whose picture is usually empty, so the counts go on screen beside it.
    pub fn fire_counts(&self) -> (usize, usize) {
        (
            self.burning.iter().filter(|b| **b > 0).count(),
            self.burnt.iter().filter(|b| **b).count(),
        )
    }
}
