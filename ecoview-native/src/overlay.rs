//! The ecological overlays: what each one reads out of a snapshot, and what its ramp runs between.
//!
//! Shot V2. The six overlays -- light, moisture, fertility, temperature, crowding and fire -- are
//! the fields `ecosim` already writes every snapshot, so nothing here computes ecology. It reads,
//! scales and bands. **Every number in a scale that the run publishes comes from the run's own
//! `meta.json`**, and where the file does not carry one the [`Scale`] says so in words rather than
//! substituting a constant silently (DECISIONS.md, "V2: what meta.json owns and what it does not").
//! One scale is the viewer's because there is no number in the run to read: crowding's, since shot
//! S7 -- see [`CROWDING_RAMP`]. Standing water's was the viewer's too until `ecosim` shot S10
//! published it; since shot V12 it is read from `meta.json`, and so are the three nutrient scales
//! G13 published for `npk.bin` ([`published`]).
//!
//! Like the rest of the library half this module never mentions Bevy.

use crate::palette::{Overlay, BANDS, CROWDING_EMPTY, FIRE_BURNT, FIRE_QUIET, WATER_DRY};
use crate::run::{Dims, Params, PublishedScale, Run, RunMeta};
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
const FALLBACK_FIRE_DURATION: f32 = 3.0;

fn viewer_fallback(what: &str) -> String {
    format!("this viewer's fallback: meta.json has no {what}")
}

/// Crowding's scale line: whose the ramp is, and where on it the simulator starts killing grazers.
///
/// The second clause is the only surviving use of `disease.grazer_threshold` in this viewer. It is a
/// landmark and not a bound, so it is written where a reader can see both at once rather than
/// silently deciding the ramp.
fn crowding_source(p: &Params) -> String {
    let mut s = viewer_fallback("scale for grazers per patch (log2 1-256, measured)");
    if let Some(t) = p.disease.grazer_threshold {
        s.push_str(&format!("; disease starts at {t:.0}"));
    }
    s
}

/// `water.bin` is ponded depth in tenths of a millimetre (CLAUDE.md, the run directory contract).
pub const WATER_TENTHS_MM: f32 = 10.0;

/// The two ends of the water overlay's ramp, in mm, and they are **logarithmic**.
///
/// Measured on `runs/capitol-s42` at tick 10000, over the 13,206 ground cells holding any water at
/// all: the median is 10 mm, the ninetieth percentile 48 mm and the maximum 5,074 mm, which is one
/// corner of the site that never drains. A linear ramp to that maximum puts 99% of the standing
/// water in the bottom band and draws the site as though it were dry; a linear ramp to the ninetieth
/// percentile throws away the two decades above it. Four decades of log10, 1 mm to 10 m, carries the
/// whole span at about 1.35x a band and is the same scale in every snapshot of every run, which a
/// scale taken from each snapshot's own maximum would not be (MEASUREMENTS.md, S5).
pub const WATER_RAMP_MM: (f32, f32) = (1.0, 10_000.0);

/// The three nutrient ramps, in g/m2 and log10, for a run that carries `npk.bin` but was written
/// before `ecosim` shot G13 published a scale for it. They are G13's published numbers -- the 2nd to
/// 98th percentile of G5's runs, rounded outward to decades (ecosim/DECISIONS.md, G13) -- copied, and
/// the source line says so. In N, P, K order, the file's.
pub const NUTRIENT_RAMP_G_M2: [(f32, f32); 3] = [(0.01, 10.0), (0.001, 100.0), (0.01, 100.0)];

/// The top of the soil water ramp when the run's `params` do not give one, in mm: the deepest
/// field capacity in `ecosim`'s shipped media table (bed and mulch, 200 mm) at its shipped
/// saturation of 1.2. Named as the viewer's in the source line whenever it is used.
pub const SOIL_WATER_FALLBACK_MM: f32 = 240.0;

/// The soil water ramp's top: the wettest any root zone in this run can be, which is the largest
/// `params.medium.*.field_capacity_mm` times `params.hydro.saturation` (shot G8). Linear from 0,
/// because the quantity is bounded at both ends and a millimetre means the same at either.
fn soil_water_scale(p: &Params) -> Scale {
    let fc = p
        .medium
        .values()
        .filter_map(|m| m.field_capacity_mm)
        .fold(0.0f32, f32::max);
    match (fc > 0.0, p.hydro.saturation) {
        (true, Some(sat)) if sat > 0.0 => Scale {
            lo: 0.0,
            hi: fc * sat,
            unit: "mm in the root zone",
            source: format!(
                "params: deepest medium field capacity {fc:.0} mm x hydro.saturation {sat}"
            ),
        },
        _ => Scale {
            lo: 0.0,
            hi: SOIL_WATER_FALLBACK_MM,
            unit: "mm in the root zone",
            source: viewer_fallback("medium field capacities and hydro.saturation"),
        },
    }
}

/// `npk.bin` is three whole f32 planes per ecology column (SAD 1, "Nutrients").
pub const NPK_PLANES: usize = 3;

/// The `scale` a run publishes for an overlay, if it publishes one this viewer can draw.
///
/// Only `log10` is drawn: every overlay that carries a scale today is a log ramp, and a published
/// `linear` one would be drawn wrong on this viewer's log bands, so it is refused by name instead
/// ([`Scale::of`] falls back and says why) rather than drawn on the wrong curve in silence.
pub fn published(o: Overlay, meta: &RunMeta) -> Result<&PublishedScale, String> {
    let row = meta
        .overlays
        .iter()
        .find(|r| r.name == o.name())
        .ok_or_else(|| format!("meta.json has no overlays.{}", o.name()))?;
    let s = row
        .scale
        .as_ref()
        .ok_or_else(|| format!("meta.json overlays.{} has no scale", o.name()))?;
    if s.curve != "log10" {
        return Err(format!(
            "meta.json overlays.{}.scale is {}, and this viewer draws log10 only",
            o.name(),
            s.curve
        ));
    }
    if !(s.lo > 0.0 && s.hi > s.lo) {
        return Err(format!(
            "meta.json overlays.{}.scale runs {}..{}, not a log range",
            o.name(),
            s.lo,
            s.hi
        ));
    }
    Ok(s)
}

/// A number for the HUD and the stdout line: two decimals, as every overlay has always printed, and
/// three significant figures below a tenth. Phosphorus's scale starts at 0.001 g/m2, which two
/// decimals print as `0.00` -- a ramp from nothing, which it is not (shot V12).
pub fn sig(v: f32) -> String {
    if v == 0.0 || v.abs() >= 0.1 || !v.is_finite() {
        format!("{v:.2}")
    } else {
        let places = (2 - v.abs().log10().floor() as i32).max(0) as usize;
        format!("{v:.places$}")
    }
}

/// A log10 scale from the run if it publishes one, else the viewer's constant with the reason.
fn log_scale(o: Overlay, meta: &RunMeta, fallback: (f32, f32), unit: &'static str) -> Scale {
    match published(o, meta) {
        Ok(p) => Scale {
            lo: p.lo,
            hi: p.hi,
            unit,
            source: format!("meta.json overlays.{}.scale", o.name()),
        },
        Err(why) => Scale {
            lo: fallback.0,
            hi: fallback.1,
            unit,
            source: format!("this viewer's fallback: {why}"),
        },
    }
}

/// The shallowest standing water drawn as a **voxel**, in mm.
///
/// The overlay maps every wet cell, including a film a tenth of a millimetre deep. Geometry cannot:
/// the thinnest voxel this viewer can draw is one ground cell, 0.5 m on the Capitol, so anything it
/// draws at all it draws far too deep. 5 mm is where a wet pavement becomes a puddle, and it is the
/// line this viewer draws between the two -- the viewer's, not the run's, and the HUD prints both
/// counts so the difference is never hidden (DECISIONS.md, S5).
pub const POND_MIN_MM: f32 = 5.0;

/// The two ends of the crowding overlay's ramp, in grazers standing on one patch, and they are
/// **log2**. Shot S7 -- the number used to be `2 x params.disease.grazer_threshold`, which is 32.
///
/// A disease threshold is a statement about one animal's health, not about how many animals a
/// viewer will be asked to draw, and the two turned out to differ by nearly a decade. Measured over
/// every snapshot of every committed or reference run that has the animal tier on:
/// `ecosim/fixtures/capitol-animals-mini` at tick 2000 holds 9,204 grazers over 869 occupied
/// patches, median **10**, ninetieth percentile 15, ninety-ninth **28** and busiest **95**;
/// `runs/s42`, the 20,000-tick strip, runs 201 snapshots with a median of 9 and reaches **229** on
/// one patch at tick 12000. So the field spans an empty patch to better than two hundred, and its
/// middle sits at ten.
///
/// A linear ramp fits neither end. To 32 the busiest patches clamp -- 0.6% of the occupied patches
/// on the fixture and 3.6% on the strip, which sounds small and is exactly the population the
/// overlay exists to find. To 229 the median lands in band 1 of 32 and the site is drawn empty.
/// Eight doublings of log2 carry the whole span at 3.75 bands a doubling, and they are the same
/// eight doublings in every snapshot of every run, which a ramp taken from each snapshot's own
/// maximum would not be (DECISIONS.md, "V2 the scale is the simulator's range, never the data's").
///
/// 256 is the first power of two above the largest patch count ever measured, which is the strip's
/// 229. 1 is the smallest patch that holds anybody; an empty patch is [`CROWDING_EMPTY`], off the
/// ramp, the way dry ground is off the water ramp.
pub const CROWDING_RAMP: (f32, f32) = (1.0, 256.0);

/// The byte `light.bin` uses for full sun (ecosim/UNITS.md, 3.6). Named because shot V3 reads the
/// same file at a crown's level (`run.rs`, `CrownLight`) and the two must divide by the same number.
pub const FULL_SUN: f32 = 255.0;

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
            // Measured, not parameterised: see [`CROWDING_RAMP`] for the distribution and shot S7
            // for why the disease threshold stopped being the top of this ramp. `meta.json` carries
            // no scale for how many grazers a patch holds, so this says whose number it is, in the
            // same words the water ramp uses -- and it still names the threshold, because "where
            // disease starts" is a true and useful mark on a scale even when it is not its end.
            Overlay::Crowding => Scale {
                lo: CROWDING_RAMP.0,
                hi: CROWDING_RAMP.1,
                unit: "grazers per patch, log2",
                source: crowding_source(p),
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
            // Ponded depth, on a **logarithmic** ramp: see `WATER_RAMP_MM` for the measurement
            // that rules a linear one out. `ecosim` shot S10 publishes the ends in
            // `overlays.water.scale`; until shot V12 this viewer never read them and said on screen
            // that the run had none, which was false (DECISIONS.md, V13). A run older than S10 gets
            // `WATER_RAMP_MM`, named as the fallback it is.
            Overlay::Water => log_scale(o, meta, WATER_RAMP_MM, "mm standing, log10"),
            Overlay::SoilWater => soil_water_scale(p),
            // The three soil pools, in g/m2 on the log10 ramps G13 measured (shot V12).
            Overlay::Nitrogen | Overlay::Phosphorus | Overlay::Potassium => {
                let k = o.nutrient().unwrap_or(0);
                log_scale(o, meta, NUTRIENT_RAMP_G_M2[k], "g/m2, log10")
            }
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
    for t in [&p.grass.temp, &p.shrub.temp, &p.tree.temp]
        .into_iter()
        .flatten()
    {
        if let (Some(a), Some(b)) = (t.first(), t.last()) {
            lo = lo.min(*a);
            hi = hi.max(*b);
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
    /// The fraction of each patch under grass and under shrub, as the simulator clamps them. No
    /// overlay ramps these -- shot V4 reads them to scatter ground cover ([`crate::cover`]) -- but
    /// they come out of the same file on the same grid, so they are read on the same pass.
    pub grass: Vec<f32>,
    pub shrub: Vec<f32>,
    /// `npk.bin`: nitrogen, phosphorus and potassium in g/m2, three whole planes of one value per
    /// ecology column, in that order (shot V12). `None` when the snapshot has no such file, which is
    /// a run with the nutrient tier off or one older than `ecosim` shot G5 -- not an error until a
    /// nutrient overlay asks for it.
    pub npk: Option<Vec<f32>>,
    /// `soil_water.bin`: water in each ecology column's root zone, in mm (shot G8, from `ecosim`
    /// G4). `None` when the snapshot has none -- the water tier off -- on the terms `npk` is.
    pub soil_water: Option<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct PatchJson {
    temperature: f32,
    #[serde(default)]
    burning_ticks_left: u32,
    /// Absent in a run whose patches predate these fields; an absent cover is no cover, which draws
    /// bare ground rather than refusing to open the run.
    #[serde(default)]
    grass: f32,
    #[serde(default)]
    shrub: f32,
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

/// An optional file of `n` little-endian f32s: `None` when absent, an error when the wrong length.
fn read_f32s(path: &Path, n: usize) -> io::Result<Option<Vec<f32>>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = read_exact_len(path, n * 4)?;
    Ok(Some(
        raw.as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect(),
    ))
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
        let grass = patches.iter().map(|p| p.grass).collect();
        let shrub = patches.iter().map(|p| p.shrub).collect();

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

        // Absent is allowed and short is not: a missing `npk.bin` is a run without the tier, while a
        // file of the wrong length is a corrupt one and would draw one pool over another's columns.
        let npk = read_f32s(&dir.join("npk.bin"), cols * NPK_PLANES)?;
        let soil_water = read_f32s(&dir.join("soil_water.bin"), cols)?;

        Ok(Fields {
            moisture,
            fertility,
            light,
            temperature,
            burning,
            grazers,
            burnt,
            grass,
            shrub,
            npk,
            soil_water,
        })
    }
}

/// The band one patch's grazer count falls in: 0 for an empty patch, and a log2 ramp above it.
///
/// Empty is exactly zero, the way dry ground is in [`water_band`]: nobody standing on a patch is not
/// a small amount of crowding, it is a different fact, and on a white-to-magenta ramp the palest
/// band and an empty patch would otherwise be the same colour. Above it the ramp is the eight
/// doublings of [`CROWDING_RAMP`].
pub fn crowding_band(n: f32, s: &Scale) -> u8 {
    if n <= 0.0 {
        return CROWDING_EMPTY;
    }
    let first = CROWDING_EMPTY as usize + 1;
    let span = (BANDS - first - 1) as f32;
    let (lo, hi) = (s.lo.max(1.0), s.hi);
    let t = if hi > lo {
        (n.max(lo).log2() - lo.log2()) / (hi.log2() - lo.log2())
    } else {
        1.0
    };
    (first + (t.clamp(0.0, 1.0) * span).round() as usize).min(BANDS - 1) as u8
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
            Overlay::Light => self.light.get(c).map_or(0.0, |v| *v as f32 / FULL_SUN),
            Overlay::Moisture => self.moisture.get(c).map_or(0.0, |v| *v as f32 / 255.0),
            Overlay::Fertility => self.fertility.get(c).map_or(0.0, |v| *v as f32),
            Overlay::Temperature => self.temperature.get(p).copied().unwrap_or(0.0),
            Overlay::Crowding => self.grazers.get(p).map_or(0.0, |v| *v as f32),
            Overlay::Fire => self.burning.get(p).map_or(0.0, |v| *v as f32),
            Overlay::Nitrogen | Overlay::Phosphorus | Overlay::Potassium => {
                let k = o.nutrient().unwrap_or(0);
                self.npk
                    .as_ref()
                    .and_then(|v| v.get(k * d.columns() + c))
                    .copied()
                    .unwrap_or(0.0)
            }
            Overlay::SoilWater => self
                .soil_water
                .as_ref()
                .and_then(|v| v.get(c))
                .copied()
                .unwrap_or(0.0),
            // Standing water is not in `Fields` and never can be: it is per **ground cell**, on the
            // bundle's finer grid, and every field here is per ecology column. [`Ponds`] reads it,
            // and `apply_world_state` routes the water overlay there instead of here.
            Overlay::Water | Overlay::Surface => 0.0,
        }
    }

    /// One band index per ecology column, plus what the field held.
    ///
    /// Fire does not ramp from its own bottom: band 0 is ground that is neither alight nor freshly
    /// burnt and band 1 is ground that burnt out since the previous snapshot, so the ramp above them
    /// is burning only. The nutrients (shot V12) are a log10 ramp above a categorical "no soil" band,
    /// and their stats are over the columns that have soil: a roof holds no nitrogen, and counting
    /// it would put every minimum at zero and pull every mean down by the paved share of the site.
    /// The other five overlays are a plain linear ramp over [`Scale`].
    pub fn bands(&self, o: Overlay, d: &Dims, s: &Scale) -> (Vec<u8>, FieldStats) {
        let mut out = vec![0u8; d.columns()];
        let (mut min, mut max, mut sum) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f64);
        let mut n = 0usize;
        // Soil water leaves the no-soil columns out of its stats too, for the same reason: a roof
        // holds no soil water, and it is not the driest lawn (shot G8).
        let nutrient = o.nutrient().is_some() || o == Overlay::SoilWater;
        for y in 0..d.y {
            for x in 0..d.x {
                let v = self.value(o, d, x, y);
                if !nutrient || v > 0.0 {
                    min = min.min(v);
                    max = max.max(v);
                    sum += v as f64;
                    n += 1;
                }
                out[x + d.x * y] = match o {
                    Overlay::Fire => self.fire_band(d, x, y, s),
                    // Crowding is the third overlay with a categorical bottom band and a ramp above
                    // it, and it is log (shot S7, [`crowding_band`]).
                    Overlay::Crowding => crowding_band(v, s),
                    // Soil water is linear above a "none here" band 0 (shot G8).
                    Overlay::SoilWater => linear_band_above_none(v, s),
                    // The nutrients are water's shape: none is off the ramp, the rest log10.
                    _ if nutrient => log10_band(v, s),
                    _ => band_of(v, s),
                };
            }
        }
        let n = n.max(1);
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

// -------------------------------------------------------------------------------------------
// Shot S5: standing water.
//
// `water.bin` is ponded depth on the **ground grid**, in tenths of a millimetre, written every
// snapshot since shot G4. Until this shot nothing in this viewer read it, so a column holding
// 215 mm of standing water was drawn as the driest ground on the site: the moisture overlay reads
// `moisture.bin`, which is soil water in the ecology columns, and a paved column has none. Both
// readings were honest and the picture was still wrong.
//
// Nothing here models water. The simulator decides where it stands and how deep; this reads the
// file, bands it for the overlay and quantises it to the lattice for the geometry, and says on
// screen which of those two numbers is the file's and which is the lattice's.
// -------------------------------------------------------------------------------------------

/// One snapshot's ponded depth, per ground cell, in mm.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ponds {
    pub width: usize,
    pub depth: usize,
    /// Ponded depth in mm, one per ground cell, x-fastest.
    pub mm: Vec<f32>,
}

/// What a snapshot's standing water came to, for the HUD, the stdout line and `ecoview.stats`.
///
/// `wet` counts every cell with any water at all and `drawn` only the ones deep enough to become a
/// voxel ([`POND_MIN_MM`]), because those two numbers differ by a factor of four on the Capitol and
/// a picture that showed the second while reporting the first would be overstating itself.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PondStats {
    pub wet: usize,
    pub drawn: usize,
    pub max_mm: f32,
    /// Mean depth over the wet cells only. The mean over the whole site is in [`FieldStats`], which
    /// is what the overlay's own legend reports.
    pub mean_mm: f32,
    /// Everything standing on the site, in cubic metres.
    pub volume_m3: f64,
}

/// One pond depth per ground cell, quantised to the drawing lattice: how many voxels of water stand
/// on each cell. Zero everywhere the run left the ground dry.
#[derive(Debug, Clone, PartialEq)]
pub struct PondLevels {
    pub width: usize,
    pub depth: usize,
    pub levels: Vec<u8>,
}

impl Run {
    /// Reads snapshot `i`'s `water.bin`.
    ///
    /// The length is checked against `meta.json`'s **ground** grid, not its ecology grid, for the
    /// reason [`Run::fields_at`] checks every file it reads: a short file read as a field draws the
    /// wrong half of the site in silence. A run with no `world` object is a version 1-3 run and
    /// [`Run::load`] has already refused it, so the absence here is a corrupt file, not an old one.
    pub fn ponds_at(&self, i: usize) -> io::Result<Ponds> {
        let Some(w) = &self.meta.world else {
            return Err(bad(format!(
                "{}: meta.json has no `world`, so there is no ground grid for water.bin",
                self.dir.display()
            )));
        };
        let (width, depth) = (w.ground_width, w.ground_depth);
        let path = self.snapshot_dir(i).join("water.bin");
        let raw = read_exact_len(&path, width * depth * 2)?;
        let mm = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b) as f32 / WATER_TENTHS_MM)
            .collect();
        Ok(Ponds { width, depth, mm })
    }
}

impl Ponds {
    pub fn cells(&self) -> usize {
        self.width * self.depth
    }

    /// What this snapshot's standing water came to. `cell_m` is the ground cell's side in metres,
    /// which is what turns a heap of depths into a volume; it comes from the bundle, because the
    /// run's own `meta.json` states it only for the grid it computed on.
    pub fn stats(&self, cell_m: f32) -> PondStats {
        let (mut wet, mut drawn, mut max, mut sum) = (0usize, 0usize, 0.0f32, 0.0f64);
        for &d in &self.mm {
            if d > 0.0 {
                wet += 1;
                sum += d as f64;
                max = max.max(d);
            }
            if d >= POND_MIN_MM {
                drawn += 1;
            }
        }
        PondStats {
            wet,
            drawn,
            max_mm: max,
            mean_mm: if wet > 0 {
                (sum / wet as f64) as f32
            } else {
                0.0
            },
            // mm of depth over one cell is a millimetre-metre-metre; the thousand takes it to m3.
            volume_m3: sum * (cell_m * cell_m) as f64 / 1000.0,
        }
    }

    /// The overlay's bands, one per ground cell, and what the field held over the whole site.
    ///
    /// Band 0 is dry ground -- categorical, off the ramp -- and every wet cell is at least band 1,
    /// however thin the film. Above that the ramp is log10 over [`WATER_RAMP_MM`].
    pub fn bands(&self, s: &Scale) -> (Vec<u8>, FieldStats) {
        let mut out = vec![0u8; self.cells()];
        let (mut min, mut max, mut sum) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f64);
        for (i, &d) in self.mm.iter().enumerate() {
            min = min.min(d);
            max = max.max(d);
            sum += d as f64;
            out[i] = water_band(d, s);
        }
        let n = self.cells().max(1);
        (
            out,
            FieldStats {
                min: if min.is_finite() { min } else { 0.0 },
                max: if max.is_finite() { max } else { 0.0 },
                mean: (sum / n as f64) as f32,
            },
        )
    }

    /// How many voxels of water stand on each ground cell, on a lattice of `cell_m` metre cubes.
    ///
    /// **A cell shallower than [`POND_MIN_MM`] gets none**, and everything above it gets at least
    /// one, which is where this stops being a measurement and starts being a drawing: 50 mm of
    /// water on a 0.5 m lattice is a tenth of a voxel, and the two honest choices are one voxel or
    /// nothing. One voxel it is, and every report that shows it says so in millimetres.
    pub fn levels(&self, cell_m: f32) -> PondLevels {
        let cell_m = if cell_m > 0.0 { cell_m } else { 1.0 };
        let levels = self
            .mm
            .iter()
            .map(|&d| {
                if d < POND_MIN_MM {
                    return 0;
                }
                ((d / 1000.0 / cell_m).round() as i64).clamp(1, u8::MAX as i64) as u8
            })
            .collect();
        PondLevels {
            width: self.width,
            depth: self.depth,
            levels,
        }
    }
}

/// The band one ponded depth falls in: 0 for dry ground, and a log10 ramp above it.
///
/// Dry is exactly zero, not "less than the bottom of the ramp": the simulator either put water here
/// or it did not, and that distinction is the whole point of the map.
pub fn water_band(mm: f32, s: &Scale) -> u8 {
    log10_band(mm, s)
}

/// Band 0 for nothing at all, and a log10 ramp over `s` from band 1 up. Water's shape (S5), which the
/// three nutrients share (V12): their band 0 is [`crate::palette::NUTRIENT_NONE`], a column with no
/// soil, and it is the same band index as [`WATER_DRY`].
pub fn log10_band(v: f32, s: &Scale) -> u8 {
    if v <= 0.0 {
        return WATER_DRY;
    }
    let first = WATER_DRY as usize + 1;
    let span = (BANDS - first - 1) as f32;
    let (lo, hi) = (s.lo.max(f32::MIN_POSITIVE), s.hi);
    let t = if hi > lo {
        (v.max(lo).log10() - lo.log10()) / (hi.log10() - lo.log10())
    } else {
        1.0
    };
    (first + (t.clamp(0.0, 1.0) * span).round() as usize).min(BANDS - 1) as u8
}

/// Band 0 for nothing at all, and a linear ramp over `s` from band 1 up: soil water's shape (shot G8).
/// A column holding no water is one with no soil -- a roof, paving -- or one wrung perfectly dry,
/// and either way it is not the bottom of a millimetre ramp.
pub fn linear_band_above_none(v: f32, s: &Scale) -> u8 {
    if v <= 0.0 {
        return WATER_DRY;
    }
    let first = WATER_DRY as usize + 1;
    let span = (BANDS - first - 1) as f32;
    let t = if s.hi > s.lo {
        (v - s.lo) / (s.hi - s.lo)
    } else {
        1.0
    };
    (first + (t.clamp(0.0, 1.0) * span).round() as usize).min(BANDS - 1) as u8
}
