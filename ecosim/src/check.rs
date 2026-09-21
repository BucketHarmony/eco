//! `check`, `stats` and `diff`: everything that reads a finished run directory. The invariants
//! themselves live in [`evaluate`], which `check` and `sweep` share.

use crate::animals::{Cause, CAUSES};
use crate::bundle::{Ground, Medium, ECO_CELL_M};
use crate::events::{deaths_per_tick, parse_events, EVENTS_FILE};
use crate::hydro::Water;
use crate::output::{snapshot_dir_name, SERIES_FIELDS, SERIES_HEADER, TRAIT_FIELDS, WATER_FIELDS};
use crate::sim::StatsRow;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

// The windows below are in years, not ticks (shot G4b). A tick is one `climate.year_len`th of a
// year, so a tick count means a fixed amount of simulated time only once it is divided by the
// `year_len` of the run being checked, which `check` reads from that run's `meta.json`. At the
// default year_len of 4000 every one of them is the tick count it replaced, named in its doc comment.

/// Length of the burn-in ignored before the invariant window: half a year (was 2000 ticks).
pub const WINDOW_YEARS: f64 = 0.5;
/// Anchor age of the trees' `max_10x` limit: 1.25 years (was tick 5000). Trees grow slowly from a
/// dozen, so a half-year anchor measured establishment speed rather than a runaway; animals keep
/// [`WINDOW_YEARS`].
pub const TREE_ANCHOR_YEARS: f64 = 1.25;
/// Length a short run must reach: 5 years (was 20000 ticks).
pub const RUN_YEARS: f64 = 5.0;
/// Age at which the addendum's extra checks are sampled: 2.5 years (was tick 10000). The renderer's
/// pixel tests read the snapshot at this tick, so at year_len 4000 it is the same snapshot as before.
pub const SAMPLE_YEARS: f64 = 2.5;
/// Minimum length of a run for `check --long`: 15 years (was 60000 ticks).
pub const LONG_YEARS: f64 = 15.0;
/// Start of the `check --long` population band window, and the age its bounds are relative to:
/// 5 years (was tick 20000).
pub const LONG_BAND_FROM_YEARS: f64 = 5.0;
/// Year length assumed for a series read without a `meta.json` to say otherwise (a sweep cell, a
/// bare `series.csv`): the shipped default.
pub const DEFAULT_YEAR_LEN: u32 = 4000;

/// How many ticks `years` years are, in a run of `year_len` ticks a year.
pub fn ticks_in(years: f64, year_len: u32) -> u32 {
    (years * year_len.max(1) as f64).round() as u32
}
/// Runtime invariant limit for a 20000-tick run on the 64×64 world, in milliseconds.
pub const RUNTIME_LIMIT_MS: u64 = 30_000;
/// Ceiling of the area-scaled runtime limit (shot 15: 20k ticks on the strip in under 90 s).
pub const RUNTIME_CAP_MS: u64 = 90_000;

/// Runtime limit for a world of `cols` columns: [`RUNTIME_LIMIT_MS`] per 64×64 of area, never below
/// it and never above [`RUNTIME_CAP_MS`].
pub fn runtime_limit_ms(cols: u64) -> u64 {
    (RUNTIME_LIMIT_MS * cols / 4096).clamp(RUNTIME_LIMIT_MS, RUNTIME_CAP_MS)
}

/// Share of the window's ticks on which mean soil moisture must be below field capacity (shot
/// G4b). The derived moisture index is 0 at the wilting point and 255 at field capacity
/// (`Sim::derive_moisture`), so both bounds are the soil's own rather than chosen numbers: a field
/// mean pinned at 0 is a site with no plant-available water left, one pinned at 255 is a site
/// holding all it can, and in neither does a plant's moisture curve respond to anything. Storms do
/// briefly fill every column, so the upper bound is a share of ticks while the wilting point is an
/// every-tick bound.
pub const MOISTURE_BELOW_CAPACITY: f64 = 0.95;

/// Floor on the share of one snapshot's trees standing on no sealed ground cell (shot G11): the
/// scale-free form of "the site is mostly lawn and the trees mostly stand on it". The Capitol's
/// worst snapshot reads 89.26% (DECISIONS.md, shot G11).
pub const TREE_OPEN_SHARE: f64 = 0.80;

/// Number of fire columns `series.csv` gained in shot 9.
const FIRE_FIELDS: usize = 2;

/// `SERIES_HEADER` without its last `cut` columns: the header an older ecosim wrote.
fn header_without(cut: usize) -> &'static str {
    let at = SERIES_HEADER.match_indices(',').nth(SERIES_FIELDS - cut - 1).map_or(SERIES_HEADER.len(), |(i, _)| i);
    &SERIES_HEADER[..at]
}

/// Read and parse `series.csv` from a run directory.
pub fn read_series(run_dir: &Path) -> Result<Vec<StatsRow>, String> {
    let path = run_dir.join("series.csv");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_series(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// `series.csv` with its death columns replaced by the counts of `death` rows in `events.csv`, when
/// the run directory has one (format version 3): what `ecosim stats` reports extinctions from.
/// Directories without it (versions 1 and 2) keep the series' own columns.
pub fn read_series_for_stats(run_dir: &Path) -> Result<Vec<StatsRow>, String> {
    let mut rows = read_series(run_dir)?;
    let path = run_dir.join(EVENTS_FILE);
    if path.exists() {
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let events = parse_events(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let deaths = deaths_per_tick(&events, rows.len()).map_err(|e| format!("{}: {e}", path.display()))?;
        rows.iter_mut().zip(deaths).for_each(|(r, d)| r.deaths = d);
    }
    Ok(rows)
}

/// Parse `series.csv` text. Sweeps parse their own in-memory CSV through this too, so a sweep cell
/// is evaluated on exactly the values a run directory would hold.
///
/// Runs written before the trait columns (shot 11) or before fire (shot 9) as well still parse, with
/// the missing columns read as 0.
pub fn parse_series(text: &str) -> Result<Vec<StatsRow>, String> {
    let mut lines = text.lines();
    let header = lines.next().ok_or("unexpected header")?;
    let dry = SERIES_FIELDS - WATER_FIELDS;
    let fields = [SERIES_FIELDS, dry, dry - TRAIT_FIELDS, dry - TRAIT_FIELDS - FIRE_FIELDS]
        .into_iter()
        .find(|&n| header_without(SERIES_FIELDS - n) == header)
        .ok_or("unexpected header")?;
    lines
        .enumerate()
        .map(|(i, line)| {
            let f: Vec<&str> = line.split(',').collect();
            if f.len() != fields {
                return Err(format!("series.csv line {}: expected {fields} fields", i + 2));
            }
            let u = |k: usize| f[k].parse::<u32>().map_err(|e| format!("line {}: {e}", i + 2));
            let x = |k: usize| f[k].parse::<f32>().map_err(|e| format!("line {}: {e}", i + 2));
            Ok(StatsRow {
                tick: u(0)?,
                grazers: u(1)?,
                hunters: u(2)?,
                trees: u(3)?,
                grass_mean: x(4)?,
                shrub_mean: x(5)?,
                moisture_mean: x(6)?,
                fertility_mean: x(7)?,
                detritus_total: x(8)?,
                temperature: x(9)?,
                hunter_immigrants: u(10)?,
                deaths: [[u(11)?, u(12)?, u(13)?, u(14)?, u(15)?], [u(16)?, u(17)?, u(18)?, u(19)?, u(20)?]],
                patches_burning: if fields > 21 { u(21)? } else { 0 },
                total_burnt: if fields > 21 { u(22)? } else { 0 },
                traits: if fields >= dry {
                    let mut t = [[0.0; TRAIT_FIELDS / 2]; 2];
                    for (k, v) in t.iter_mut().flatten().enumerate() {
                        *v = x(23 + k)?;
                    }
                    t
                } else {
                    Default::default()
                },
                water: if fields == SERIES_FIELDS {
                    Water {
                        rain_mm: x(dry)?,
                        runoff_mm: x(dry + 1)?,
                        ponded_mm: x(dry + 2)?,
                        soil_water_mm: x(dry + 3)?,
                        drainage_mm: x(dry + 4)?,
                        outflow_mm: x(dry + 5)?,
                    }
                } else {
                    Water::default()
                },
            })
        })
        .collect()
}

/// Centered moving average of width `w` (window [t − w/2, t + w/2 − 1]); None where incomplete.
pub fn moving_average(v: &[f64], w: usize) -> Vec<Option<f64>> {
    let mut prefix = vec![0.0; v.len() + 1];
    for (i, x) in v.iter().enumerate() {
        prefix[i + 1] = prefix[i] + x;
    }
    let half = w / 2;
    (0..v.len())
        .map(|t| {
            if t < half || t + (w - half) > v.len() {
                None
            } else {
                Some((prefix[t + w - half] - prefix[t - half]) / w as f64)
            }
        })
        .collect()
}

/// Local maxima of the 200-tick smoothed grazer count within [from, to] (row index = tick):
/// The 200-tick smoothing window, the plus-or-minus 500-tick neighbourhood and the 1500-tick
/// separation in `evaluate` stay in ticks (shot G4b): animal energy, reproduction cooldowns and ages
/// are all still per tick, so the grazer cycle has no length in years to be measured against until
/// the animal tier is converted (UNITS.md, "Deferred to shot G4c").
/// ≥ every smoothed value within ±500 and strictly > at least one. Plateaus collapse to their first tick.
pub fn grazer_maxima(rows: &[StatsRow], from: usize, to: usize) -> Vec<usize> {
    let g: Vec<f64> = rows.iter().map(|r| r.grazers as f64).collect();
    let ma = moving_average(&g, 200);
    let mut out: Vec<usize> = Vec::new();
    for t in from..=to.min(rows.len().saturating_sub(1)) {
        let Some(v) = ma[t] else { continue };
        let lo = t.saturating_sub(500);
        let hi = (t + 500).min(ma.len() - 1);
        let mut ok = true;
        let mut strictly = false;
        for (s, u) in ma.iter().enumerate().take(hi + 1).skip(lo) {
            if s == t {
                continue;
            }
            if let Some(u) = *u {
                if u > v {
                    ok = false;
                    break;
                }
                if v > u {
                    strictly = true;
                }
            }
        }
        if ok && strictly {
            let plateau_continues =
                out.last().is_some_and(|&p| ma[p] == Some(v) && (p + 1..t).all(|s| ma[s] == Some(v)));
            if !plateau_continues {
                out.push(t);
            }
        }
    }
    out
}

/// Everything the invariants look at: the series plus the two facts that live outside it.
pub struct Series {
    /// One row per tick, starting at tick 0.
    pub rows: Vec<StatsRow>,
    /// Mature trees in the [`SAMPLE_YEARS`] snapshot; `None` when the run is shorter than that.
    pub mature_at_10000: Option<Result<usize, String>>,
    /// Ticks in a year (`climate.year_len` from `meta.json`), which every window in years is
    /// measured with.
    pub year_len: u32,
    /// Wall time of the run, if known and evaluated.
    pub timing: Timing,
    /// Whether the run placed grazers and hunters; false makes the animal invariants n/a.
    pub animals: bool,
    /// Where the run's trees have their feet, over every snapshot; `None` when the run has no
    /// bundle ground grid, which makes the footing invariant n/a.
    pub footing: Option<Result<Footing, String>>,
}

/// How much sealed ground the run's trees stand on (shot G11). A tree occupies one 1 m ecology
/// column spanning `ratio²` ground cells — four at the Capitol's 0.5 m grid — and one or two sealed
/// cells is a trunk beside a walk, not a tree on the walk. Summed over every snapshot, so a tree
/// that germinates on a roof and dies before the end is still seen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Footing {
    /// Snapshots examined.
    pub snapshots: usize,
    /// Tree sightings: one per living tree per snapshot.
    pub trees: usize,
    /// Sightings by how many of the column's cells are sealed ([`Medium::is_sealed`]); index 0 is a
    /// tree on none, and the length is `ratio² + 1`.
    pub per_sealed: Vec<usize>,
    /// Sealed cells under a tree that are [`Medium::Roof`], over every sighting.
    pub roof_cells: usize,
    /// The first sighting that broke the rule: tick, position, sealed cells, roof cells.
    pub first_bad: Option<(u32, f32, f32, usize, usize)>,
    /// The snapshot with the smallest share of trees on no sealed cell: tick, share, trees. Per
    /// snapshot rather than over every sighting, because a run ends with most of its trees and an
    /// aggregate hides a tighter snapshot in the middle (DECISIONS.md, shot G11).
    pub worst_open: Option<(u32, f64, usize)>,
}

impl Footing {
    /// Ground cells under one column, from the histogram's length.
    fn cells_per_column(&self) -> usize {
        self.per_sealed.len().saturating_sub(1)
    }

    /// Sightings with more than half the column's cells sealed: three or four of four.
    pub fn mostly_sealed(&self) -> usize {
        self.per_sealed.iter().skip(self.cells_per_column() / 2 + 1).sum()
    }

    /// Smallest share of one snapshot's trees on no sealed cell; 1.0 when no snapshot held a tree.
    pub fn open_share(&self) -> f64 {
        self.worst_open.map_or(1.0, |(_, share, _)| share)
    }
}

/// Where the runtime invariant gets its wall time.
pub enum Timing {
    /// Wall time from `timing.json` and the limit for the run's world size (from `meta.json`).
    Ms(u64, u64),
    /// `timing.json` absent or unreadable: the runtime invariant fails.
    Missing,
    /// Not evaluated (sweeps: wall time isn't comparable across parallel jobs).
    Excluded,
}

impl Series {
    /// Load the series, the [`SAMPLE_YEARS`] mature-tree count and `timing.json` from a run dir.
    pub fn from_run_dir(run_dir: &Path) -> Result<Series, String> {
        let rows = read_series(run_dir)?;
        let year_len = meta_year_len(run_dir);
        let sample = ticks_in(SAMPLE_YEARS, year_len) as usize;
        let mature_at_10000 = (rows.len() > sample).then(|| mature_trees_at(run_dir, sample as u32));
        let text = fs::read_to_string(run_dir.join("timing.json")).unwrap_or_default();
        let timing = match serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| v["wall_ms"].as_u64()) {
            Some(ms) => Timing::Ms(ms, runtime_limit_ms(meta_cols(run_dir))),
            None => Timing::Missing,
        };
        let footing = tree_footing(run_dir);
        Ok(Series { rows, mature_at_10000, timing, animals: run_has_animals(run_dir), year_len, footing })
    }
}

/// A run directory's `meta.json`, or `Null` when it is missing or unreadable.
fn meta_json(run_dir: &Path) -> serde_json::Value {
    let text = fs::read_to_string(run_dir.join("meta.json")).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

/// Ticks in a year from `meta.json`'s `year_len`; [`DEFAULT_YEAR_LEN`] when absent or unreadable.
pub fn meta_year_len(run_dir: &Path) -> u32 {
    meta_json(run_dir)["year_len"].as_u64().map_or(DEFAULT_YEAR_LEN, |v| v.max(1) as u32)
}

/// Columns of the run's world from `meta.json`'s `dims`; 4096 (64×64) when absent or unreadable.
fn meta_cols(run_dir: &Path) -> u64 {
    let v = meta_json(run_dir);
    v["dims"]["x"].as_u64().unwrap_or(64) * v["dims"]["y"].as_u64().unwrap_or(64)
}

/// Whether a run placed grazers and hunters, from `meta.json`'s `animals`. The key is written only
/// by an animals-off run, so every other run directory — including every one written before shot
/// G0 — reads as having animals.
pub fn run_has_animals(run_dir: &Path) -> bool {
    meta_json(run_dir)["animals"].as_bool().unwrap_or(true)
}

/// The series columns the invariants and reports cover, in report order: the three species with
/// animals, trees alone without them.
fn species_columns(animals: bool) -> Vec<(&'static str, Column<u32>)> {
    let all: [(&'static str, Column<u32>); 3] =
        [("grazers", |r| r.grazers), ("hunters", |r| r.hunters), ("trees", |r| r.trees)];
    if animals {
        all.to_vec()
    } else {
        vec![all[2]]
    }
}

/// Reads one series column from a row.
type Column<T> = fn(&StatsRow) -> T;

/// One invariant's outcome. `margin` is the signed distance from `value` to `threshold` as a fraction
/// of the threshold, positive when passing. Invariants with several parts (species, band sides)
/// report the part with the smallest margin in `value`/`threshold`/`margin`.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckLine {
    /// Short column-safe id used in sweep output.
    pub key: &'static str,
    /// Human-readable invariant, as printed by `ecosim check`.
    pub name: &'static str,
    /// Whether the invariant holds. Always true for an invariant that does not apply (`na`).
    pub pass: bool,
    /// The invariant does not apply to this run (an animal invariant on an animals-off run), so it
    /// counts towards neither pass nor fail.
    pub na: bool,
    /// What was measured, as printed by `ecosim check`.
    pub observed: String,
    /// The measured value of the binding part.
    pub value: f64,
    /// The limit that value is compared with.
    pub threshold: f64,
    /// Signed distance from `value` to `threshold` as a fraction of the threshold; ≥ 0 passes.
    pub margin: f64,
}

/// Every invariant evaluated on one run, in report order.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckReport {
    /// One line per invariant.
    pub lines: Vec<CheckLine>,
}

impl CheckReport {
    /// True when every invariant that applies passes.
    pub fn pass(&self) -> bool {
        self.lines.iter().all(|l| l.na || l.pass)
    }

    /// The keys of the invariants that do not apply to this run, in report order.
    pub fn not_applicable(&self) -> Vec<&'static str> {
        self.lines.iter().filter(|l| l.na).map(|l| l.key).collect()
    }

    /// The line for an invariant key, if it was evaluated.
    pub fn get(&self, key: &str) -> Option<&CheckLine> {
        self.lines.iter().find(|l| l.key == key)
    }
}

/// Every invariant key in report order. `run_length` and `tick_10000` only appear for short runs,
/// and `runtime` only when timing is evaluated. New keys are appended, so a sweep CSV's invariant
/// columns keep their order (shot G11).
pub const INVARIANT_KEYS: [&str; 13] = [
    "run_length",
    "no_extinction",
    "max_10x",
    "grazer_cycle",
    "moisture_band",
    "fertility_band",
    "grass_band",
    "tree_growth",
    "runtime",
    "tick_10000",
    "mature_trees_10k",
    "animals_10k",
    "tree_footing",
];

/// Margin for "value ≥ threshold".
fn margin_at_least(value: f64, threshold: f64) -> f64 {
    if threshold == 0.0 {
        return if value >= 0.0 { 0.0 } else { -1.0 };
    }
    (value - threshold) / threshold.abs()
}

/// Margin for "value ≤ threshold".
fn margin_at_most(value: f64, threshold: f64) -> f64 {
    if threshold == 0.0 {
        return if value <= 0.0 { 0.0 } else { -1.0 };
    }
    (threshold - value) / threshold.abs()
}

/// (value, threshold, margin) of the part with the smallest margin.
fn tightest(parts: impl IntoIterator<Item = (f64, f64, f64)>) -> (f64, f64, f64) {
    parts.into_iter().fold((f64::NAN, f64::NAN, f64::INFINITY), |acc, p| if p.2 < acc.2 { p } else { acc })
}

/// Two-sided band: the tighter side.
fn band(min: f64, max: f64, lo: f64, hi: f64) -> (f64, f64, f64) {
    tightest([(min, lo, margin_at_least(min, lo)), (max, hi, margin_at_most(max, hi))])
}

struct Builder(Vec<CheckLine>);

impl Builder {
    fn push(&mut self, key: &'static str, name: &'static str, pass: bool, observed: String, vtm: (f64, f64, f64)) {
        let (value, threshold, margin) = vtm;
        self.0.push(CheckLine { key, name, pass, observed, na: false, value, threshold, margin });
    }

    /// An invariant that does not apply because the run has no animals.
    fn push_na(&mut self, key: &'static str, name: &'static str) {
        self.push_na_because(key, name, NA_REASON);
    }

    /// An invariant that does not apply to this run, for the reason given.
    fn push_na_because(&mut self, key: &'static str, name: &'static str, reason: &str) {
        let observed = format!("n/a ({reason})");
        self.0.push(CheckLine {
            key,
            name,
            pass: true,
            observed,
            na: true,
            value: f64::NAN,
            threshold: f64::NAN,
            margin: f64::NAN,
        });
    }
}

/// Why an invariant is reported as not applicable.
pub const NA_REASON: &str = "animals off";

/// The invariants that do not apply at all to an animals-off run. `no_extinction` and `max_10x`
/// still apply: they drop their grazer and hunter parts and keep their tree part.
pub const ANIMAL_ONLY_KEYS: [&str; 2] = ["grazer_cycle", "animals_10k"];

/// The `check --long` invariants that do not apply to an animals-off run.
pub const ANIMAL_ONLY_LONG_KEYS: [&str; 1] = ["long_band"];

/// Why the footing invariant is reported as not applicable.
pub const NA_REASON_NO_GROUND: &str = "no bundle ground grid";

/// The invariants that only a run on a world bundle has: a noise world's ground grid is all soil,
/// so no tree can stand on sealed ground and the question is empty (shot G11).
pub const BUNDLE_ONLY_KEYS: [&str; 1] = ["tree_footing"];

/// A tree in a snapshot's `entities.json`, reduced to the three fields the footing needs.
#[derive(serde::Deserialize)]
struct FootingEntity {
    kind: String,
    x: f32,
    y: f32,
}

/// The run's bundle ground grid, or `None` when it has none the invariant can use: a format-3 run
/// has no `world/` at all, and a noise world's grid is the all-soil one its ecology columns mirror,
/// in which nothing is sealed and the question is empty (`meta.json`'s `world.bundle`).
fn bundle_ground(run_dir: &Path, meta: &serde_json::Value) -> Option<Result<Ground, String>> {
    let w = &meta["world"];
    if w["bundle"].as_bool() != Some(true) {
        return None;
    }
    Some((|| {
        let path = run_dir.join("world").join("medium.bin");
        let medium = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let size = |k: &str| w[k].as_u64().ok_or(format!("meta.json: world.{k} missing")).map(|v| v as usize);
        let (width, depth) = (size("ground_width")?, size("ground_depth")?);
        let cell_m = w["ground_cell_m"].as_f64().ok_or("meta.json: world.ground_cell_m missing")?;
        let ratio = (f64::from(ECO_CELL_M) / cell_m).round() as usize;
        if cell_m <= 0.0 || ratio == 0 {
            return Err(format!("meta.json: world.ground_cell_m {cell_m} is not a fraction of a 1 m column"));
        }
        let names = w["media"].as_array().ok_or("meta.json: world.media missing")?;
        let media = names
            .iter()
            .map(|n| {
                let name = n.as_str().unwrap_or_default();
                Medium::from_name(name).ok_or(format!("meta.json: world.media has unknown medium {name:?}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if medium.len() != width * depth {
            return Err(format!("{}: {} bytes for a {width}x{depth} grid", path.display(), medium.len()));
        }
        if medium.iter().any(|&c| c as usize >= media.len()) {
            return Err(format!("{}: a medium code is past the {} world.media lists", path.display(), media.len()));
        }
        Ok(Ground { width, depth, ratio, medium, media })
    })())
}

/// Add every tree in one snapshot to `f`. Positions are in ecology-grid metres, so a tree stands in
/// column `(floor x, floor y)` and its ground cells are that column's ([`Ground::cells_of`], the
/// simulator's own mapping rather than a second copy of it).
fn add_footing(f: &mut Footing, ground: &Ground, tick: u32, trees: &[(f32, f32)]) -> Result<(), String> {
    let per = ground.cells_per_column();
    let (cols_x, cols_y) = (ground.width / ground.ratio, ground.depth / ground.ratio);
    if f.per_sealed.is_empty() {
        f.per_sealed = vec![0; per + 1];
    }
    f.snapshots += 1;
    let mut open = 0usize;
    for &(x, y) in trees {
        let (cx, cy) = (x.floor(), y.floor());
        if cx < 0.0 || cy < 0.0 || cx as usize >= cols_x || cy as usize >= cols_y {
            return Err(format!("snapshot {tick}: tree at ({x}, {y}) is outside the ground grid"));
        }
        let (sealed, roofs) = ground.cells_of(cx as usize, cy as usize).fold((0, 0), |(s, r), i| {
            let m = ground.medium_at(i);
            (s + usize::from(m.is_sealed()), r + usize::from(m == Medium::Roof))
        });
        f.trees += 1;
        f.per_sealed[sealed] += 1;
        f.roof_cells += roofs;
        open += usize::from(sealed == 0);
        if (roofs > 0 || sealed > per / 2) && f.first_bad.is_none() {
            f.first_bad = Some((tick, x, y, sealed, roofs));
        }
    }
    let share = open as f64 / trees.len().max(1) as f64;
    if !trees.is_empty() && f.worst_open.is_none_or(|(_, worst, _)| share < worst) {
        f.worst_open = Some((tick, share, trees.len()));
    }
    Ok(())
}

/// Every tree of every snapshot against the run's bundle ground grid; `None` when it has none, which
/// makes the footing invariant not applicable.
fn tree_footing(run_dir: &Path) -> Option<Result<Footing, String>> {
    let meta = meta_json(run_dir);
    let ground = match bundle_ground(run_dir, &meta)? {
        Ok(g) => g,
        Err(e) => return Some(Err(e)),
    };
    Some((|| {
        let ticks = meta["snapshots"].as_array().ok_or("meta.json: snapshots missing")?;
        let mut f = Footing::default();
        for tick in ticks.iter().filter_map(|t| t.as_u64()).map(|t| t as u32) {
            let path = run_dir.join(snapshot_dir_name(tick)).join("entities.json");
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let ents: Vec<FootingEntity> =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            let trees: Vec<(f32, f32)> = ents.iter().filter(|e| e.kind == "tree").map(|e| (e.x, e.y)).collect();
            add_footing(&mut f, &ground, tick, &trees)?;
        }
        Ok(f)
    })())
}

/// Mature-tree count in a snapshot's entities.json.
fn mature_trees_at(run_dir: &Path, tick: u32) -> Result<usize, String> {
    let p: PathBuf = run_dir.join(snapshot_dir_name(tick)).join("entities.json");
    let text = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(v.as_array().map(|a| a.iter().filter(|e| e["kind"] == "tree" && e["stage"] == "mature").count()).unwrap_or(0))
}

/// `ecosim check` on a run directory.
pub fn check_run(run_dir: &Path) -> Result<CheckReport, String> {
    evaluate(&Series::from_run_dir(run_dir)?)
}

/// Every invariant from the SAD plus the addendum's extra acceptance checks.
pub fn evaluate(series: &Series) -> Result<CheckReport, String> {
    let rows = &series.rows;
    if rows.is_empty() {
        return Err("series.csv has no rows".into());
    }
    for (i, r) in rows.iter().enumerate() {
        if r.tick as usize != i {
            return Err(format!("series.csv: row {i} has tick {}", r.tick));
        }
    }
    let last = rows.len() - 1;
    let mut out = Builder(Vec::new());
    let full = ticks_in(RUN_YEARS, series.year_len) as f64;
    if (last as f64) < full {
        let vtm = (last as f64, full, margin_at_least(last as f64, full));
        out.push("run_length", "run length >= 5 years", false, format!("last tick {last}"), vtm);
    }
    let from = (ticks_in(WINDOW_YEARS, series.year_len) as usize).min(last);
    let win = &rows[from..];
    // An animals-off run never placed a grazer or a hunter, so only the trees are counted here.
    let species = species_columns(series.animals);

    // 1. No species count reaches 0 (every minimum ≥ 1).
    let mins: Vec<(&str, u32)> = species.iter().map(|(n, f)| (*n, win.iter().map(f).min().unwrap_or(0))).collect();
    out.push(
        "no_extinction",
        "no species reaches 0",
        mins.iter().all(|m| m.1 > 0),
        mins.iter().map(|(n, m)| format!("min {n}={m}")).collect::<Vec<_>>().join(" "),
        tightest(mins.iter().map(|&(_, m)| (m as f64, 1.0, margin_at_least(m as f64, 1.0)))),
    );

    // 2. No species count exceeds 10× its anchor value (value = max count, threshold = limit).
    // Grazers and hunters are anchored at tick 2000, trees at tick 5000.
    let mut ok = true;
    let mut obs = Vec::new();
    let mut parts = Vec::new();
    for (n, f) in species.iter() {
        let anchor =
            if *n == "trees" { (ticks_in(TREE_ANCHOR_YEARS, series.year_len) as usize).min(last) } else { from };
        let base = f(&rows[anchor]);
        let max = win.iter().map(f).max().unwrap_or(0);
        ok &= max <= 10 * base;
        obs.push(format!("{n} max={max} limit={}", 10 * base));
        parts.push((max as f64, (10 * base) as f64, margin_at_most(max as f64, (10 * base) as f64)));
    }
    out.push(
        "max_10x",
        "no species exceeds 10x its anchor count (animals: 0.5 years, trees: 1.25 years)",
        ok,
        obs.join(" "),
        tightest(parts),
    );

    // 3. Grazer cycle: ≥ 2 local maxima ≥ 1500 ticks apart (value = first-to-last span, 0 if < 2).
    let cycle_name = "grazer cycle (2 maxima >= 1500 ticks apart)";
    if series.animals {
        let maxima = grazer_maxima(rows, from, last);
        let span = match (maxima.first(), maxima.last()) {
            (Some(a), Some(b)) => b - a,
            _ => 0,
        };
        out.push(
            "grazer_cycle",
            cycle_name,
            maxima.len() >= 2 && span >= 1500,
            format!("{} maxima at {:?}, span {span}", maxima.len(), maxima),
            (span as f64, 1500.0, margin_at_least(span as f64, 1500.0)),
        );
    } else {
        out.push_na("grazer_cycle", cycle_name);
    }

    // 4. Soil moisture between the wilting point and field capacity (shot G4b).
    let (mmin, _) = range(win.iter().map(|r| r.moisture_mean));
    let below = win.iter().filter(|r| r.moisture_mean < 255.0).count() as f64 / win.len().max(1) as f64;
    out.push(
        "moisture_band",
        "moisture_mean above the wilting point every tick, below field capacity on >= 95% of them",
        mmin > 0.0 && below >= MOISTURE_BELOW_CAPACITY,
        format!("min {mmin:.2} of 255, below capacity on {:.2}% of ticks", 100.0 * below),
        tightest([
            (mmin as f64, 0.0, margin_at_least(mmin as f64, 0.0)),
            (below, MOISTURE_BELOW_CAPACITY, margin_at_least(below, MOISTURE_BELOW_CAPACITY)),
        ]),
    );

    // 5. fertility_mean in [40, 220].
    let (fmin, fmax) = range(win.iter().map(|r| r.fertility_mean));
    out.push(
        "fertility_band",
        "fertility_mean in [40, 220]",
        fmin >= 40.0 && fmax <= 220.0,
        format!("[{fmin:.2}, {fmax:.2}]"),
        band(fmin as f64, fmax as f64, 40.0, 220.0),
    );

    // 6. grass_mean in [0.05, 0.95].
    let (gmin, gmax) = range(win.iter().map(|r| r.grass_mean));
    out.push(
        "grass_band",
        "grass_mean in [0.05, 0.95]",
        gmin >= 0.05 && gmax <= 0.95,
        format!("[{gmin:.4}, {gmax:.4}]"),
        band(gmin as f64, gmax as f64, 0.05, 0.95),
    );

    // 7. Succession: trees at the end ≥ 1.5× trees at tick 0.
    let (t0, tn) = (rows[0].trees, rows[last].trees);
    let need = 1.5 * t0 as f64;
    out.push(
        "tree_growth",
        "trees at end >= 1.5x trees at tick 0",
        tn as f64 >= need,
        format!("{tn} vs {t0} (need {need:.1})"),
        (tn as f64, need, margin_at_least(tn as f64, need)),
    );

    // 8. Runtime under 30 s (per 64×64 of area, at most 90 s).
    match series.timing {
        Timing::Ms(ms, limit_ms) => out.push(
            "runtime",
            if limit_ms == RUNTIME_LIMIT_MS { "run time < 30 s" } else { "run time < 30 s per 64x64, at most 90 s" },
            ms < limit_ms,
            format!("{ms} ms"),
            (ms as f64, limit_ms as f64, margin_at_most(ms as f64, limit_ms as f64)),
        ),
        Timing::Missing => {
            let limit = RUNTIME_LIMIT_MS as f64;
            out.push("runtime", "run time < 30 s", false, "timing.json missing".into(), (f64::NAN, limit, -1.0))
        }
        Timing::Excluded => {}
    }

    // Addendum extras at 2.5 years, tick 10000 at the default year length (the renderer's pixel
    // tests depend on them).
    let sample = ticks_in(SAMPLE_YEARS, series.year_len) as usize;
    match &series.mature_at_10000 {
        Some(mature) if last >= sample => {
            let name = "mature trees at 2.5 years >= 35";
            match mature {
                Ok(m) => {
                    let m = *m as f64;
                    out.push("mature_trees_10k", name, m >= 35.0, format!("{m}"), (m, 35.0, margin_at_least(m, 35.0)))
                }
                Err(e) => out.push("mature_trees_10k", name, false, e.clone(), (f64::NAN, 35.0, -1.0)),
            }
            let animals_name = "at 2.5 years grazers >= 10 and hunters >= 2";
            if series.animals {
                let r = &rows[sample];
                let (g, h) = (r.grazers as f64, r.hunters as f64);
                out.push(
                    "animals_10k",
                    animals_name,
                    r.grazers >= 10 && r.hunters >= 2,
                    format!("grazers={} hunters={}", r.grazers, r.hunters),
                    tightest([(g, 10.0, margin_at_least(g, 10.0)), (h, 2.0, margin_at_least(h, 2.0))]),
                );
            } else {
                out.push_na("animals_10k", animals_name);
            }
        }
        _ => out.push(
            "tick_10000",
            "2.5-year checks",
            false,
            format!("run shorter than {sample} ticks"),
            (last as f64, sample as f64, margin_at_least(last as f64, sample as f64)),
        ),
    }

    // Shot G11: no tree stands on sealed ground. Moved here from a Playwright test in `ecoview`
    // that drove a browser to read `entities.json` and `world/medium.bin` and examined no pixel; it
    // is an assertion about this component's output, so it is checked by this component's own
    // command. A run with no bundle ground grid has no sealed cell to stand on and reports n/a.
    let footing_name = "no tree on a roof or on more than half its ground cells sealed, >= 80% on none";
    match &series.footing {
        None => out.push_na_because("tree_footing", footing_name, NA_REASON_NO_GROUND),
        Some(Err(e)) => out.push("tree_footing", footing_name, false, e.clone(), (f64::NAN, 0.0, -1.0)),
        Some(Ok(f)) => {
            let (roofs, mostly, open) = (f.roof_cells as f64, f.mostly_sealed() as f64, f.open_share());
            let per = f.cells_per_column();
            let bad = f.first_bad.map_or(String::new(), |(t, x, y, sealed, roofs)| {
                format!("; first bad at tick {t}, tree ({x:.1}, {y:.1}) on {sealed} of {per} sealed, {roofs} roof")
            });
            let worst = f.worst_open.map_or(String::new(), |(t, _, n)| format!(" at tick {t} of {n} trees"));
            out.push(
                "tree_footing",
                footing_name,
                f.roof_cells == 0 && f.mostly_sealed() == 0 && open >= TREE_OPEN_SHARE,
                format!(
                    "{} sightings over {} snapshots: {} over half sealed, {} roof cells, per-tree sealed {:?}; worst snapshot {:.2}% on none{worst}{bad}",
                    f.trees,
                    f.snapshots,
                    f.mostly_sealed(),
                    f.roof_cells,
                    f.per_sealed,
                    100.0 * open,
                ),
                tightest([
                    (roofs, 0.0, margin_at_most(roofs, 0.0)),
                    (mostly, 0.0, margin_at_most(mostly, 0.0)),
                    (open, TREE_OPEN_SHARE, margin_at_least(open, TREE_OPEN_SHARE)),
                ]),
            );
        }
    }
    Ok(CheckReport { lines: out.0 })
}

/// Every `check --long` key in report order; `long_run_length` only appears for short runs.
pub const LONG_KEYS: [&str; 4] = ["long_run_length", "long_no_extinction", "long_fertility", "long_band"];

/// `ecosim check --long` on a run directory.
pub fn check_run_long(run_dir: &Path) -> Result<CheckReport, String> {
    evaluate_long(&read_series(run_dir)?, run_has_animals(run_dir), meta_year_len(run_dir))
}

/// The long-run invariants, for runs of at least [`LONG_YEARS`]: no species reaches 0 at any tick,
/// fertility_mean stays inside the same [40, 220] band the short check uses, and grazers and
/// hunters stay within [0.2×, 5×] of their 5-year count over years 5–15. These replace,
/// rather than extend, the 20000-tick invariants. Fertility is here because a 20000-tick run ends
/// a few hundred ticks after fertility crosses 220 in the noise world, so the short check never
/// saw the saturation the long run has always had (shot G4, DECISIONS.md "Fertility has a sink").
pub fn evaluate_long(rows: &[StatsRow], animals: bool, year_len: u32) -> Result<CheckReport, String> {
    if rows.is_empty() {
        return Err("series.csv has no rows".into());
    }
    for (i, r) in rows.iter().enumerate() {
        if r.tick as usize != i {
            return Err(format!("series.csv: row {i} has tick {}", r.tick));
        }
    }
    let last = rows.len() - 1;
    let mut out = Builder(Vec::new());
    let long_ticks = ticks_in(LONG_YEARS, year_len) as usize;
    let long = long_ticks as f64;
    if last < long_ticks {
        let vtm = (last as f64, long, margin_at_least(last as f64, long));
        out.push("long_run_length", "run length >= 15 years", false, format!("last tick {last}"), vtm);
    }
    let species = species_columns(animals);
    let mins: Vec<(&str, u32)> = species.iter().map(|(n, f)| (*n, rows.iter().map(f).min().unwrap_or(0))).collect();
    out.push(
        "long_no_extinction",
        "no species reaches 0 over the whole run",
        mins.iter().all(|m| m.1 > 0),
        mins.iter().map(|(n, m)| format!("min {n}={m}")).collect::<Vec<_>>().join(" "),
        tightest(mins.iter().map(|&(_, m)| (m as f64, 1.0, margin_at_least(m as f64, 1.0)))),
    );
    let (fmin, fmax) = range(rows.iter().map(|r| r.fertility_mean));
    out.push(
        "long_fertility",
        "fertility_mean in [40, 220] over the whole run",
        fmin >= 40.0 && fmax <= 220.0,
        format!("[{fmin:.2}, {fmax:.2}]"),
        band(fmin as f64, fmax as f64, 40.0, 220.0),
    );
    let band_name = "grazers and hunters over years 5-15 within [0.2x, 5x] of their 5-year count";
    if !animals {
        out.push_na("long_band", band_name);
        return Ok(CheckReport { lines: out.0 });
    }
    let from = (ticks_in(LONG_BAND_FROM_YEARS, year_len) as usize).min(last);
    let win = &rows[from..=long_ticks.min(last)];
    let (mut ok, mut obs, mut parts) = (true, Vec::new(), Vec::new());
    for (n, f) in &species[..2] {
        let base = f(&rows[from]) as f64;
        let (lo, hi) = (0.2 * base, 5.0 * base);
        let min = win.iter().map(f).min().unwrap_or(0) as f64;
        let max = win.iter().map(f).max().unwrap_or(0) as f64;
        ok &= min >= lo && max <= hi;
        obs.push(format!("{n} [{min}, {max}] vs [{lo:.1}, {hi:.1}]"));
        parts.push(band(min, max, lo, hi));
    }
    out.push("long_band", band_name, ok, obs.join(" "), tightest(parts));
    Ok(CheckReport { lines: out.0 })
}

fn range(it: impl Iterator<Item = f32>) -> (f32, f32) {
    it.fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)))
}

/// First row at which any species count is 0, over the whole run. Without `animals` only the trees
/// are looked at: a run that never placed a grazer or a hunter has not lost one.
pub fn first_extinction(rows: &[StatsRow], animals: bool) -> Option<&StatsRow> {
    rows.iter().find(|r| (animals && (r.grazers == 0 || r.hunters == 0)) || r.trees == 0)
}

/// Ticks of death counts attributed to an extinction: the extinction tick and the 499 before it.
/// In ticks rather than years (shot G4b): it is a window over the death log of the per-tick animal
/// tier, and it sets no threshold, so no amount of simulated time depends on it.
pub const CAUSE_WINDOW: u32 = 500;

/// One species reaching 0, with what killed it off.
#[derive(Debug, Clone, PartialEq)]
pub struct Extinction {
    /// `grazers`, `hunters` or `trees`.
    pub species: &'static str,
    /// First tick at which its count is 0.
    pub tick: u32,
    /// Deaths by cause over the `CAUSE_WINDOW` ticks ending at `tick`; `None` for trees, whose
    /// death causes are not recorded.
    pub causes: Option<[u32; CAUSES]>,
    /// The food supply over the same window: (label, mean). Mean grazer count for hunters, mean
    /// `grass_mean` for grazers; `None` for trees.
    pub food: Option<(&'static str, f64)>,
}

impl Extinction {
    /// The cause with the most deaths in the window, ties to the first in `Cause` order; `None`
    /// when no death was recorded there (or for trees).
    pub fn dominant(&self) -> Option<Cause> {
        let c = self.causes?;
        let best = Cause::ALL.iter().copied().max_by_key(|&k| (c[k as usize], std::cmp::Reverse(k as usize)))?;
        (c[best as usize] > 0).then_some(best)
    }

    /// The dominant cause's name, `none` without recorded deaths, `unrecorded` for trees.
    pub fn dominant_name(&self) -> &'static str {
        match (self.causes, self.dominant()) {
            (None, _) => "unrecorded",
            (Some(_), None) => "none",
            (Some(_), Some(c)) => c.name(),
        }
    }
}

/// Every species that reaches 0 at some tick (whole run, burn-in included), in tick order; species
/// reaching 0 on the same tick keep the order grazers, hunters, trees. Without `animals` the
/// grazers and hunters are skipped: they were never placed, so they never died out.
pub fn extinctions(rows: &[StatsRow], animals: bool) -> Vec<Extinction> {
    let species: [(&'static str, Column<u32>); 3] =
        [("grazers", |r| r.grazers), ("hunters", |r| r.hunters), ("trees", |r| r.trees)];
    let mut out: Vec<Extinction> = species
        .iter()
        .enumerate()
        .filter(|(k, _)| animals || *k == 2)
        .filter_map(|(k, (name, count))| {
            let at = rows.iter().position(|r| count(r) == 0)?;
            let win = &rows[at.saturating_sub(CAUSE_WINDOW as usize - 1)..=at];
            let mean = |f: Column<f64>| win.iter().map(f).sum::<f64>() / win.len() as f64;
            let (causes, food) = match k {
                0 | 1 => {
                    let mut c = [0; CAUSES];
                    for r in win {
                        c.iter_mut().zip(r.deaths[k]).for_each(|(a, b)| *a += b);
                    }
                    let food = if k == 0 {
                        ("grass_mean", mean(|r| r.grass_mean as f64))
                    } else {
                        ("grazers", mean(|r| r.grazers as f64))
                    };
                    (Some(c), Some(food))
                }
                _ => (None, None),
            };
            Some(Extinction { species: name, tick: rows[at].tick, causes, food })
        })
        .collect();
    out.sort_by_key(|e| e.tick);
    out
}

/// One printable line per extinction, for `ecosim stats`.
pub fn extinction_line(e: &Extinction) -> String {
    let Some(c) = e.causes else {
        return format!("extinction: {} at tick {} (tree death causes are not recorded)", e.species, e.tick);
    };
    let from = e.tick.saturating_sub(CAUSE_WINDOW - 1);
    let counts: Vec<String> = Cause::ALL.iter().map(|&k| format!("{}={}", k.name(), c[k as usize])).collect();
    let (label, mean) = e.food.unwrap_or(("", f64::NAN));
    format!(
        "extinction: {} at tick {}; deaths in ticks {from}-{}: {}; dominant cause: {}; mean {label} {mean:.4}",
        e.species,
        e.tick,
        e.tick,
        counts.join(" "),
        e.dominant_name()
    )
}

// The six `SIG_*` constants stay in ticks (shot G4b), for the reason `grazer_maxima` gives: they
// describe the predator-prey cycle, whose clock is the per-tick animal tier. Converting them to
// years before that tier is converted would measure a per-tick cycle on a per-year ruler.

/// Lag range of the predator-prey signature: -`SIG_MAX_LAG`..=`SIG_MAX_LAG` ticks in `SIG_LAG_STEP`s.
pub const SIG_MAX_LAG: i32 = 8000;
/// Step between the signature's lags, and between the hunter autocorrelation's lags.
pub const SIG_LAG_STEP: i32 = 50;
/// First tick of the signature window, which runs to `SIG_END` (or the run's end).
pub const SIG_START: u32 = 5000;
/// Last tick of the signature window.
pub const SIG_END: u32 = 60_000;
/// Width of the centred moving average subtracted from both series before correlating.
pub const SIG_DETREND: u32 = 12_000;
/// Largest lag searched for pp_period in the hunter autocorrelation.
pub const SIG_MAX_PERIOD: i32 = 20_000;

/// The predator–prey signature of a run: the lagged cross-correlation of detrended, deseasonalised
/// hunters against grazers over ticks `SIG_START`..=`SIG_END` (or the run's end).
#[derive(Debug, Clone, PartialEq)]
pub enum Signature {
    /// The lag with the largest Pearson correlation of grazers(t) with hunters(t + lag), and that
    /// correlation. A positive lag means hunter numbers follow grazer numbers.
    Cycle {
        /// Ticks by which hunters trail grazers at the best lag (`pp_lag`).
        lag: i32,
        /// The correlation at that lag (`pp_corr`).
        corr: f64,
        /// The hunter cycle's period from its own autocorrelation (`pp_period`, `hunter_period`);
        /// `None` when it has no positive maximum after its first negative value.
        period: Option<i32>,
    },
    /// Grazers or hunters reach 0 inside the window, so the correlation is undefined there.
    Extinct(Extinction),
    /// No lag has two samples with non-zero variance on both sides (a flat or too-short series).
    Flat,
    /// The run left animals out (`animals.enabled = false`), so it has no predator or prey.
    NotApplicable,
}

impl Signature {
    /// `pp_pass`: hunters trail grazers by less than half a hunter period, with pp_corr > 0.3.
    pub fn pass(&self) -> bool {
        match self {
            Signature::Cycle { lag, corr, period: Some(p) } => *lag > 0 && 2 * lag < *p && *corr > 0.3,
            _ => false,
        }
    }
}

/// `x` minus its centred moving average over `SIG_DETREND` ticks (t − 6000..=t + 6000, cut short
/// at the ends of the series).
fn detrend(x: &[f64]) -> Vec<f64> {
    let half = SIG_DETREND as usize / 2;
    let mut sum = vec![0.0; x.len() + 1];
    for (i, v) in x.iter().enumerate() {
        sum[i + 1] = sum[i] + v;
    }
    (0..x.len())
        .map(|t| {
            let (lo, hi) = (t.saturating_sub(half), (t + half + 1).min(x.len()));
            x[t] - (sum[hi] - sum[lo]) / (hi - lo) as f64
        })
        .collect()
}

/// `x` (sampled at tick = index) minus its mean by season phase (tick mod `year_len`), each phase's
/// mean taken over the whole series.
fn deseasonalise(x: &[f64], year_len: u32) -> Vec<f64> {
    let year = year_len.max(1) as usize;
    let (mut sum, mut n) = (vec![0.0; year], vec![0u32; year]);
    for (t, v) in x.iter().enumerate() {
        sum[t % year] += v;
        n[t % year] += 1;
    }
    x.iter().enumerate().map(|(t, v)| v - sum[t % year] / n[t % year] as f64).collect()
}

/// pp_period: the lag of the first positive local maximum of `h`'s autocorrelation after the
/// autocorrelation has first fallen below 0, searching lags `SIG_LAG_STEP`..=`SIG_MAX_PERIOD` in
/// `SIG_LAG_STEP`s. A local maximum is strictly above the lag before it and at least the lag after
/// it. `None` when there is none (or the series is too short to reach one). Lags are computed one
/// at a time and the search stops at the first match, since each costs a pass over the window.
fn hunter_period(h: &[f64]) -> Option<i32> {
    let ac = |lag: i32| {
        let k = lag as usize;
        (k + 2 <= h.len()).then(|| pearson(&h[..h.len() - k], &h[k..])).flatten()
    };
    let (mut fell, mut prev, mut cur) = (false, None, None);
    for lag in (SIG_LAG_STEP..=SIG_MAX_PERIOD + SIG_LAG_STEP).step_by(SIG_LAG_STEP as usize) {
        let next = ac(lag);
        if let (true, Some(p), Some(c), Some(n)) = (fell, prev, cur, next) {
            if c > 0.0 && c > p && c >= n {
                return Some(lag - SIG_LAG_STEP);
            }
        }
        fell |= next.is_some_and(|n| n < 0.0);
        (prev, cur) = (cur, next);
    }
    None
}

/// The predator–prey signature (`ecosim stats --signature`, sweep columns `pp_lag`, `pp_corr`,
/// `pp_period`, `pp_pass`). Both series are detrended over the whole run first (`detrend`), then
/// lose their mean by season phase (`deseasonalise`, `year_len` ticks a year). Each lag L then
/// correlates grazers(t) with hunters(t + L) over the ticks t where both t and t + L lie in the
/// window. Lags run from −8000 upward, and only a strictly larger correlation replaces the best, so
/// ties go to the most negative lag. pp_period comes from the hunters alone (`hunter_period`).
pub fn signature(rows: &[StatsRow], year_len: u32, animals: bool) -> Signature {
    if !animals {
        return Signature::NotApplicable;
    }
    let end = rows.len().min(SIG_END as usize + 1);
    let from = (SIG_START as usize).min(end);
    let gone = extinctions(rows, true).into_iter().filter(|e| e.species != "trees");
    for e in gone {
        let count = |r: &StatsRow| if e.species == "grazers" { r.grazers } else { r.hunters };
        if rows[from..end].iter().any(|r| count(r) == 0) {
            return Signature::Extinct(e);
        }
    }
    let clean = |f: fn(&StatsRow) -> u32| {
        deseasonalise(&detrend(&rows.iter().map(|r| f(r) as f64).collect::<Vec<_>>()), year_len)
    };
    let (g, h) = (clean(|r| r.grazers), clean(|r| r.hunters));
    let (g, h) = (&g[from..end], &h[from..end]);
    let mut best: Option<(i32, f64)> = None;
    for lag in (-SIG_MAX_LAG..=SIG_MAX_LAG).step_by(SIG_LAG_STEP as usize) {
        let shift = lag.unsigned_abs() as usize;
        if shift + 2 > g.len() {
            continue;
        }
        let n = g.len() - shift;
        let (a, b) = if lag >= 0 { (&g[..n], &h[shift..]) } else { (&g[shift..], &h[..n]) };
        if let Some(c) = pearson(a, b) {
            if best.is_none_or(|(_, bc)| c > bc) {
                best = Some((lag, c));
            }
        }
    }
    best.map_or(Signature::Flat, |(lag, corr)| Signature::Cycle { lag, corr, period: hunter_period(h) })
}

/// Pearson correlation of two equal-length samples; `None` when either has zero variance.
fn pearson(a: &[f64], b: &[f64]) -> Option<f64> {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        let (dx, dy) = (x - ma, y - mb);
        sab += dx * dy;
        saa += dx * dx;
        sbb += dy * dy;
    }
    (saa > 0.0 && sbb > 0.0).then(|| sab / (saa * sbb).sqrt())
}

/// `ecosim stats --signature` on a path: a run directory (its `series_for_stats` rows, with the
/// year length from `meta.json` unless `year_len` is given) or a bare `series.csv`-format file, such
/// as a sweep's `cells/*.csv`, which needs `year_len`.
pub fn signature_of(path: &Path, year_len: Option<u32>) -> Result<Signature, String> {
    if path.is_file() {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rows = parse_series(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let year_len = year_len.ok_or_else(|| format!("{}: a series file needs --year-len", path.display()))?;
        return Ok(signature(&rows, year_len, true));
    }
    let rows = read_series_for_stats(path)?;
    let year_len = match year_len {
        Some(y) => y,
        None => {
            let p = path.join("meta.json");
            let text = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
            let y = v["year_len"].as_u64().ok_or_else(|| format!("{}: no year_len", p.display()))?;
            u32::try_from(y).map_err(|e| format!("{}: {e}", p.display()))?
        }
    };
    Ok(signature(&rows, year_len, run_has_animals(path)))
}

/// The printable `ecosim stats --signature` line.
pub fn signature_line(s: &Signature) -> String {
    match s {
        Signature::Cycle { lag, corr, period } => format!(
            "signature: pp_lag {lag} pp_corr {corr:.4} pp_period {} pp_pass {} (grazers(t) vs hunters(t+lag), detrended by a {SIG_DETREND}-tick moving average and deseasonalised, ticks {SIG_START}-{SIG_END}, lags -{SIG_MAX_LAG}..{SIG_MAX_LAG} step {SIG_LAG_STEP}; period from the hunter autocorrelation)",
            period.map_or("undefined".into(), |p| p.to_string()),
            s.pass()
        ),
        Signature::Extinct(e) => format!(
            "signature: undefined, {} extinct at tick {} (dominant cause: {}) pp_pass false",
            e.species,
            e.tick,
            e.dominant_name()
        ),
        Signature::Flat => "signature: undefined, no lag with variance in both series pp_pass false".into(),
        Signature::NotApplicable => {
            format!("signature: not applicable, the run has no animals ({NA_REASON}) pp_pass false")
        }
    }
}

/// min/max/mean per series column plus first extinction tick, as printable lines. Extinction causes
/// come from `events.csv` when the run has one (`read_series_for_stats`).
pub fn stats_report(run_dir: &Path) -> Result<Vec<String>, String> {
    let rows = read_series_for_stats(run_dir)?;
    let animals = run_has_animals(run_dir);
    if rows.is_empty() {
        return Err("series.csv has no rows".into());
    }
    let cols: [(&str, Column<f64>); 10] = [
        ("grazers", |r| r.grazers as f64),
        ("hunters", |r| r.hunters as f64),
        ("trees", |r| r.trees as f64),
        ("grass_mean", |r| r.grass_mean as f64),
        ("shrub_mean", |r| r.shrub_mean as f64),
        ("moisture_mean", |r| r.moisture_mean as f64),
        ("fertility_mean", |r| r.fertility_mean as f64),
        ("detritus_total", |r| r.detritus_total as f64),
        ("temperature", |r| r.temperature as f64),
        ("hunter_immigrants", |r| r.hunter_immigrants as f64),
    ];
    let mut out = vec![format!("{:<18}{:>12}{:>12}{:>12}", "column", "min", "max", "mean")];
    for (name, f) in cols.iter() {
        let v: Vec<f64> = rows.iter().map(f).collect();
        let min = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        out.push(format!("{name:<18}{min:>12.4}{max:>12.4}{mean:>12.4}"));
    }
    out.push(match first_extinction(&rows, animals) {
        Some(r) => {
            format!("first extinction: tick {} (grazers={} hunters={} trees={})", r.tick, r.grazers, r.hunters, r.trees)
        }
        None => "first extinction: none".into(),
    });
    out.extend(extinctions(&rows, animals).iter().map(extinction_line));
    Ok(out)
}

fn list_files(root: &Path, dir: &Path, out: &mut BTreeSet<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let p = entry?.path();
        if p.is_dir() {
            list_files(root, &p, out)?;
        } else if let Ok(rel) = p.strip_prefix(root) {
            if rel != Path::new("timing.json") {
                out.insert(rel.to_path_buf());
            }
        }
    }
    Ok(())
}

/// Byte-compare two run directories (ignoring timing.json). Returns the differing paths.
pub fn diff_runs(a: &Path, b: &Path) -> Result<Vec<String>, String> {
    let (mut fa, mut fb) = (BTreeSet::new(), BTreeSet::new());
    list_files(a, a, &mut fa).map_err(|e| format!("{}: {e}", a.display()))?;
    list_files(b, b, &mut fb).map_err(|e| format!("{}: {e}", b.display()))?;
    let mut out = Vec::new();
    for rel in fa.union(&fb) {
        let shown = rel.to_string_lossy().replace('\\', "/");
        match (fa.contains(rel), fb.contains(rel)) {
            (true, false) => out.push(format!("only in {}: {shown}", a.display())),
            (false, true) => out.push(format!("only in {}: {shown}", b.display())),
            _ => {
                let x = fs::read(a.join(rel)).map_err(|e| e.to_string())?;
                let y = fs::read(b.join(rel)).map_err(|e| e.to_string())?;
                if x != y {
                    out.push(format!("differs: {shown}"));
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    fn rows_from(grazers: impl Fn(usize) -> u32, n: usize) -> Vec<StatsRow> {
        (0..n)
            .map(|t| StatsRow {
                tick: t as u32,
                grazers: grazers(t),
                hunters: 5,
                trees: 20,
                grass_mean: 0.5,
                shrub_mean: 0.2,
                moisture_mean: 100.0,
                fertility_mean: 100.0,
                detritus_total: 10.0,
                temperature: 12.0,
                hunter_immigrants: 0,
                deaths: Default::default(),
                patches_burning: 0,
                total_burnt: 0,
                traits: Default::default(),
                water: Default::default(),
            })
            .collect()
    }

    /// Year length of every hand-built series in this file: the shipped default, so a window in
    /// years is the tick count the file used before shot G4b (`WINDOW_YEARS` 0.5 is tick 2000).
    const YEAR: u32 = DEFAULT_YEAR_LEN;
    const CYCLE_TICKS: usize = 66_001;

    /// A grazer series and hunters that copy it `delay` ticks later. Four incommensurate cycles,
    /// none a whole number of years, so no lag other than `delay` lines the two up across ±8000.
    fn delayed_copy(delay: i32, phase: f64) -> Vec<StatsRow> {
        let g = |t: i32| {
            let w = |p: f64| t as f64 / p * std::f64::consts::TAU;
            let s = 150.0 * libm::sin(w(9000.0) + phase) + 80.0 * libm::sin(w(5700.0) + 1.0);
            (400.0 + s + 60.0 * libm::sin(w(3100.0) + 2.0) + 40.0 * libm::sin(w(1300.0))) as u32
        };
        let mut rows = rows_from(|t| g(t as i32), 60_001);
        rows.iter_mut().enumerate().for_each(|(t, r)| r.hunters = g(t as i32 - delay) / 10 + 1);
        rows
    }

    /// The signature finds a known delay: hunters that are the grazer series `delay` ticks later
    /// (delay a multiple of 50 within ±8000) give pp_lag = delay and pp_corr near 1.
    fn signature_finds_the_delay(delay: i32, phase: f64) -> Result<(), TestCaseError> {
        match signature(&delayed_copy(delay, phase), YEAR, true) {
            Signature::Cycle { lag, corr, .. } => {
                prop_assert_eq!(lag, delay);
                // Not 1: the moving average is cut short over the run's last 6000 ticks, which
                // detrends the two series differently there; the larger the delay, the more it shows.
                prop_assert!(corr > 0.95, "corr {}", corr);
            }
            other => prop_assert!(false, "{:?}", other),
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(24)))]

        #[test]
        fn prop_signature_finds_the_delay(k in -160i32..=160, phase in 0.0f64..std::f64::consts::TAU) {
            signature_finds_the_delay(k * SIG_LAG_STEP, phase)?;
        }
    }

    #[test]
    fn signature_regression_delay_edges_extinct_and_flat() {
        for d in [0, 350, -350, SIG_MAX_LAG, -SIG_MAX_LAG] {
            signature_finds_the_delay(d, 0.0).unwrap();
        }
        let line = signature_line(&signature(&delayed_copy(350, 0.0), YEAR, true));
        assert!(
            line.starts_with("signature: pp_lag 350 pp_corr 0.9") && line.ends_with("hunter autocorrelation)"),
            "{line}"
        );
        // Hunters at 0 inside the window: undefined, reported with the extinction's cause.
        let mut rows = delayed_copy(350, 0.0);
        for r in &mut rows[12_000..] {
            r.hunters = 0;
        }
        rows[12_000].deaths[1][Cause::Starved as usize] = 3;
        let s = signature(&rows, YEAR, true);
        assert!(matches!(&s, Signature::Extinct(e) if e.species == "hunters" && e.tick == 12_000), "{s:?}");
        assert!(!s.pass());
        assert_eq!(
            signature_line(&s),
            "signature: undefined, hunters extinct at tick 12000 (dominant cause: starved) pp_pass false"
        );
        // An extinction before tick 5000 leaves 0s in the window too; trees reaching 0 do not count.
        let mut rows = delayed_copy(0, 0.0);
        rows.iter_mut().for_each(|r| r.trees = 0);
        assert!(matches!(signature(&rows, YEAR, true), Signature::Cycle { lag: 0, .. }));
        rows[4500..].iter_mut().for_each(|r| r.grazers = 0);
        assert!(
            matches!(signature(&rows, YEAR, true), Signature::Extinct(e) if e.species == "grazers" && e.tick == 4500)
        );
        // An extinction that ends before the window does not count: the window never sees a 0.
        let mut rows = delayed_copy(0, 0.0);
        rows[3000].hunters = 0;
        assert!(matches!(signature(&rows, YEAR, true), Signature::Cycle { lag: 0, .. }));
        // Constant hunters (the default of `rows_from`) have no variance at any lag.
        let flat = rows_from(|t| 100 + (t % 7) as u32, 20_001);
        assert_eq!(signature(&flat, YEAR, true), Signature::Flat);
        assert!(signature_line(&Signature::Flat).starts_with("signature: undefined"));
    }

    /// Grazers on a `period`-tick cycle and hunters on the same cycle `delay` ticks later (negative:
    /// hunters lead), each with a one-year seasonal term and a linear trend on top. The series runs
    /// 6000 ticks past the window, so the moving average is not cut short inside it: at the run's
    /// end the cut average detrends the two shifted series differently, which can move a flat
    /// 11000-tick peak by two lag steps. The
    /// cycle's amplitude swells and fades over 25000 ticks, as a real population cycle's does: a
    /// constant-amplitude sinusoid correlates as well at delay ± period as at delay, which would
    /// make pp_lag a tie between lobes. The seed sets the cycle's and the envelope's phases.
    fn cycle_with_seasons(period: f64, delay: i32, seed: u64) -> Vec<StatsRow> {
        cycle_under(period, delay, seed, 25_000.0)
    }

    /// [`cycle_with_seasons`] with the amplitude envelope on an `envelope`-tick cycle.
    fn cycle_under(period: f64, delay: i32, seed: u64, envelope: f64) -> Vec<StatsRow> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let (a, b) = (rng.gen_range(0.0..std::f64::consts::TAU), rng.gen_range(0.0..std::f64::consts::TAU));
        let w = |t: i32, p: f64| t as f64 / p * std::f64::consts::TAU;
        let cycle = |t: i32| (1.0 + 0.6 * libm::sin(w(t, envelope) + a)) * libm::sin(w(t, period) + b);
        let season = |t: i32| libm::sin(w(t, YEAR as f64));
        let g = |t: i32| 600.0 + 150.0 * cycle(t) + 120.0 * season(t) + t as f64 / 600.0;
        let h = |t: i32| 100.0 + 40.0 * cycle(t - delay) - 15.0 * season(t + 700) - t as f64 / 3000.0;
        let mut rows = rows_from(|t| g(t as i32) as u32, CYCLE_TICKS);
        rows.iter_mut().enumerate().for_each(|(t, r)| r.hunters = h(t as i32) as u32);
        rows
    }

    /// Hunters trailing a 6000–12000-tick grazer cycle by less than half a period, under a seasonal
    /// term and a trend: pp_lag within 100 of the delay, pp_period within 5% of the period, pass.
    fn trailing_cycle_passes(period: i32, delay: i32, seed: u64) -> Result<(), TestCaseError> {
        let s = signature(&cycle_with_seasons(period as f64, delay, seed), YEAR, true);
        let Signature::Cycle { lag, period: Some(got), .. } = s else {
            return Err(TestCaseError::fail(format!("{s:?}")));
        };
        prop_assert!((lag - delay).abs() <= 100, "lag {} for delay {}", lag, delay);
        prop_assert!((got - period).abs() * 20 <= period, "period {} for {}", got, period);
        prop_assert!(s.pass(), "{:?}", s);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(12)))]

        #[test]
        fn prop_signature_trailing_cycle_passes(p in 6000i32..=12000, f in 0.05f64..0.45, seed in 0u64..1_000_000) {
            trailing_cycle_passes(p, (p as f64 * f) as i32, seed)?;
        }
    }

    #[test]
    fn signature_regression_trailing_cycle_seasons_and_leading() {
        for (p, d) in [(6000, 300), (8000, 3500), (12000, 5000), (9000, 850)] {
            trailing_cycle_passes(p, d, d as u64).unwrap();
        }
        // Seasons plus independent noise on both species: no hunter cycle to find, no pass.
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let mut noisy = cycle_with_seasons(8000.0, 0, 3);
        for (t, r) in noisy.iter_mut().enumerate() {
            let season = libm::sin(t as f64 / YEAR as f64 * std::f64::consts::TAU);
            r.grazers = (600.0 + 120.0 * season + rng.gen_range(-40.0..40.0)) as u32;
            r.hunters = (60.0 - 15.0 * season + rng.gen_range(-8.0..8.0)) as u32;
        }
        let s = signature(&noisy, YEAR, true);
        assert!(!s.pass(), "{s:?}");
        // Hunters leading grazers by less than a quarter period: a negative lag, no pass.
        for (p, d) in [(8000, 1500), (6000, 1000), (12000, 2900)] {
            let s = signature(&cycle_with_seasons(p as f64, -d, 5), YEAR, true);
            assert!(matches!(s, Signature::Cycle { lag, .. } if lag < 0), "{s:?}");
            assert!(!s.pass(), "{s:?}");
        }
        assert_eq!(
            signature_line(&Signature::Cycle { lag: 50, corr: 0.5, period: None }).split(" (").next(),
            Some("signature: pp_lag 50 pp_corr 0.5000 pp_period undefined pp_pass false")
        );
        assert!(Signature::Cycle { lag: 50, corr: 0.5, period: Some(200) }.pass());
        assert!(!Signature::Cycle { lag: 100, corr: 0.5, period: Some(200) }.pass());
        assert!(!Signature::Cycle { lag: 50, corr: 0.3, period: Some(200) }.pass());
    }

    /// Shot 15's case: hunters 6000 ticks behind a grazer cycle longer than 12000 ticks, under the
    /// 4000-tick season and a trend. The check recovers the lag within 200 and the period within 500.
    /// Envelope of the long-cycle case. Over the 55000-tick window a 25000-tick envelope moves the
    /// hunter autocorrelation's peak by up to 600 ticks on a 13000–16000-tick cycle, and periods of
    /// 15000 and more read up to 550 short even under a slow envelope (DECISIONS.md, shot 15); at
    /// 60000 and periods 12500–14000 the estimate stays within 500.
    const ENVELOPE: f64 = 60_000.0;
    fn long_lag_recovered(period: i32, seed: u64) -> Result<(), TestCaseError> {
        let s = signature(&cycle_under(period as f64, 6000, seed, ENVELOPE), YEAR, true);
        let Signature::Cycle { lag, period: Some(got), .. } = s else {
            return Err(TestCaseError::fail(format!("{s:?}")));
        };
        prop_assert!((lag - 6000).abs() <= 200, "lag {} for 6000 on a {}-tick cycle", lag, period);
        prop_assert!((got - period).abs() <= 500, "period {} for {}", got, period);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(8)))]

        #[test]
        fn prop_signature_recovers_a_6000_tick_lag_on_a_long_cycle(p in 12_500i32..=14_000, seed in 0u64..1_000_000) {
            long_lag_recovered(p, seed)?;
        }
    }

    #[test]
    fn signature_regression_6000_tick_lag_on_a_14000_tick_cycle() {
        for (p, seed) in [(14_000, 1), (12_500, 2), (13_500, 3)] {
            long_lag_recovered(p, seed).unwrap();
        }
    }

    #[test]
    fn maxima_detects_a_cycle_and_ignores_flat_lines() {
        let cyc =
            rows_from(|t| (100.0 + 40.0 * libm::sin(t as f64 * 2.0 * std::f64::consts::PI / 4000.0)) as u32, 20001);
        let m = grazer_maxima(&cyc, 2000, 20000);
        assert!(m.len() >= 4, "{m:?}");
        assert!(m.last().unwrap() - m.first().unwrap() >= 1500);
        let flat = rows_from(|_| 100, 20001);
        assert!(grazer_maxima(&flat, 2000, 20000).is_empty());
    }

    #[test]
    fn moving_average_is_centered() {
        let v: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let ma = moving_average(&v, 200);
        assert!(ma[99].is_none());
        assert_eq!(ma[100], Some(99.5));
        assert!(ma[900].is_some() && ma[901].is_none());
    }

    #[test]
    fn margins_are_signed_fractions_of_the_threshold() {
        let mut rows = rows_from(|t| 100 + (t % 3000 < 1500) as u32 * 50, 20001);
        rows[5000].fertility_mean = 210.0; // upper side: (220 − 210)/220 ≈ 0.045, tighter than (100 − 40)/40
        rows[6000].grass_mean = 0.04; // below the band: (0.04 − 0.05)/0.05 = −0.2
        let s = Series {
            rows,
            mature_at_10000: Some(Ok(42)),
            timing: Timing::Excluded,
            animals: true,
            year_len: YEAR,
            footing: Some(Ok(healthy_footing())),
        };
        let r = evaluate(&s).unwrap();
        assert!(r.get("runtime").is_none());
        let f = r.get("fertility_band").unwrap();
        assert!(f.pass && f.threshold == 220.0 && (f.margin - 10.0 / 220.0).abs() < 1e-9);
        let g = r.get("grass_band").unwrap();
        assert!(!g.pass && g.threshold == 0.05 && (g.margin + 0.2).abs() < 1e-6, "{g:?}");
        let m = r.get("mature_trees_10k").unwrap();
        assert!(m.pass && (m.margin - 7.0 / 35.0).abs() < 1e-12);
        let a = r.get("animals_10k").unwrap(); // hunters 5 vs 2 → 1.5; grazers ≥ 100 vs 10 → ≥ 9
        assert_eq!((a.value, a.threshold, a.margin), (5.0, 2.0, 1.5));
        for l in r.lines.iter().filter(|l| !l.na) {
            assert_eq!(l.pass, l.margin >= 0.0, "{l:?}");
        }
    }

    /// A synthetic 20000-tick series that meets every invariant: sine grazers (period P, base B,
    /// amplitude A), constant hunters H, trees rising linearly from T0 to 2·T0, grass and fertility
    /// constant inside their bands.
    #[derive(Debug, Clone)]
    struct Healthy {
        period: f64,
        base: f64,
        amp: f64,
        hunters: u32,
        t0: u32,
        grass: f32,
        fertility: f32,
        mature: usize,
        ms: u64,
    }

    fn healthy() -> impl Strategy<Value = Healthy> {
        (
            2000.0..5000.0,
            100.0..300.0,
            0.2f64..0.5,
            5u32..50,
            10u32..50,
            0.2f32..0.8,
            60.0f32..200.0,
            35usize..200,
            0u64..30_000,
        )
            .prop_map(|(period, base, a, hunters, t0, grass, fertility, mature, ms)| Healthy {
                period,
                base,
                amp: (base * a).max(20.0),
                hunters,
                t0,
                grass,
                fertility,
                mature,
                ms,
            })
    }

    fn build(h: &Healthy) -> Series {
        let rows = (0..=20_000u32)
            .map(|t| StatsRow {
                tick: t,
                grazers: (h.base + h.amp * libm::sin(t as f64 * std::f64::consts::TAU / h.period)).round() as u32,
                hunters: h.hunters,
                trees: h.t0 + h.t0 * t / 20_000,
                grass_mean: h.grass,
                shrub_mean: 0.2,
                moisture_mean: 100.0,
                fertility_mean: h.fertility,
                detritus_total: 10.0,
                temperature: 12.0,
                hunter_immigrants: 0,
                deaths: Default::default(),
                patches_burning: 0,
                total_burnt: 0,
                traits: Default::default(),
                water: Default::default(),
            })
            .collect();
        Series {
            rows,
            mature_at_10000: Some(Ok(h.mature)),
            timing: Timing::Ms(h.ms, RUNTIME_LIMIT_MS),
            animals: true,
            year_len: YEAR,
            footing: Some(Ok(healthy_footing())),
        }
    }

    /// A footing that passes: 1000 trees on four ground cells each, 900 of them on no sealed cell,
    /// 60 on one and 40 on two, which is the shape the Capitol reference run has.
    fn healthy_footing() -> Footing {
        Footing {
            snapshots: 1,
            trees: 1000,
            per_sealed: vec![900, 60, 40, 0, 0],
            roof_cells: 0,
            first_bad: None,
            worst_open: Some((10_000, 0.9, 1000)),
        }
    }

    /// The invariants a violation may break: all but `tick_10000`, which needs a run under 10000 ticks.
    const VIOLABLE: [&str; 12] = [
        "run_length",
        "no_extinction",
        "max_10x",
        "grazer_cycle",
        "moisture_band",
        "fertility_band",
        "grass_band",
        "tree_growth",
        "runtime",
        "mature_trees_10k",
        "animals_10k",
        "tree_footing",
    ];

    /// Break exactly one invariant at tick `t` (in [2001, 19999], never 5000 or 10000). `side` picks the band
    /// side; `keep` is the row count for `run_length` (15001..=20000).
    fn violate(s: &mut Series, key: &str, t: usize, side: bool, keep: usize) {
        let rows = &mut s.rows;
        match key {
            "run_length" => rows.truncate(keep),
            "no_extinction" => rows[t].hunters = 0,
            "max_10x" => rows[t].trees = 10 * rows[ticks_in(TREE_ANCHOR_YEARS, YEAR) as usize].trees + 1,
            "grazer_cycle" => rows.iter_mut().for_each(|r| r.grazers = 150),
            // The dry side is a single tick, which is all that clause allows; the wet side is a
            // fraction of the run, because field capacity on one tick is within the 5% the check
            // allows a saturated soil (shot G4b).
            "moisture_band" => {
                if side {
                    rows[t].moisture_mean = 0.0;
                } else {
                    rows.iter_mut().step_by(8).for_each(|r| r.moisture_mean = 255.0);
                }
            }
            "fertility_band" => rows[t].fertility_mean = if side { 39.9 } else { 220.1 },
            "grass_band" => rows[t].grass_mean = if side { 0.049 } else { 0.951 },
            "tree_growth" => rows[20_000].trees = rows[0].trees,
            "runtime" => s.timing = Timing::Ms(30_000 + t as u64, RUNTIME_LIMIT_MS),
            "mature_trees_10k" => s.mature_at_10000 = Some(Ok(t % 35)),
            "animals_10k" => rows[10_000].hunters = 1,
            // `side` picks which half of the footing rule breaks: a tree rooted on a roof cell, or
            // one standing on three of its four cells sealed.
            "tree_footing" => {
                let f = s.footing.as_mut().unwrap().as_mut().unwrap();
                if side {
                    f.roof_cells = 1;
                    f.first_bad = Some((t as u32, 1.5, 2.5, 4, 1));
                } else {
                    f.per_sealed[0] -= 1;
                    f.per_sealed[3] += 1;
                    f.first_bad = Some((t as u32, 1.5, 2.5, 3, 0));
                }
            }
            _ => unreachable!("{key}"),
        }
    }

    fn failing(s: &Series) -> Vec<&'static str> {
        evaluate(s).unwrap().lines.iter().filter(|l| !l.pass).map(|l| l.key).collect()
    }

    fn violation_fails_only_itself(
        h: &Healthy,
        key: &str,
        t: usize,
        side: bool,
        keep: usize,
    ) -> Result<(), TestCaseError> {
        let mut s = build(h);
        prop_assert_eq!(failing(&s), Vec::<&str>::new(), "healthy series fails: {:?}", h);
        violate(&mut s, key, t, side, keep);
        prop_assert_eq!(failing(&s), vec![key]);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(24)))]

        #[test]
        fn prop_one_violation_fails_exactly_that_invariant(
            h in healthy(),
            key in prop::sample::select(&VIOLABLE[..]),
            t in (2001usize..=19_999).prop_filter("not an anchor tick", |&t| t != 5_000 && t != 10_000),
            side in any::<bool>(),
            keep in 15_001usize..=20_000,
        ) {
            violation_fails_only_itself(&h, key, t, side, keep)?;
        }
    }

    #[test]
    fn evaluate_regression_each_violation_on_one_series() {
        let h = Healthy {
            period: 3000.0,
            base: 150.0,
            amp: 50.0,
            hunters: 10,
            t0: 20,
            grass: 0.5,
            fertility: 100.0,
            mature: 40,
            ms: 9000,
        };
        for key in VIOLABLE {
            violation_fails_only_itself(&h, key, 12_345, true, 17_000).unwrap();
        }
    }

    /// Keys failing in `evaluate_long` on these rows.
    fn failing_long(rows: &[StatsRow]) -> Vec<&'static str> {
        evaluate_long(rows, true, YEAR).unwrap().lines.iter().filter(|l| !l.pass).map(|l| l.key).collect()
    }

    /// A 4x4 ground grid at 0.5 m cells, so 2x2 ecology columns of four cells each: column (0, 0)
    /// all lawn, (1, 0) all roof, (0, 1) half lawn and half asphalt, (1, 1) three concrete cells of
    /// four.
    fn test_ground() -> Ground {
        let code = |m: Medium| Medium::ALL.iter().position(|&k| k == m).unwrap() as u8;
        let (l, r, a, c) = (code(Medium::Lawn), code(Medium::Roof), code(Medium::Asphalt), code(Medium::Concrete));
        Ground {
            width: 4,
            depth: 4,
            ratio: 2,
            // Row by row from the south-west corner.
            medium: vec![l, l, r, r, l, l, r, r, l, a, c, c, l, a, c, l],
            media: Medium::ALL.to_vec(),
        }
    }

    /// The footing counts a tree by how many of its column's four ground cells are sealed, and a
    /// tree on the roof column breaks the rule outright (shot G11).
    #[test]
    fn footing_counts_sealed_cells_under_each_tree() {
        let g = test_ground();
        let mut f = Footing::default();
        // One tree per column, in column order: lawn, roof, half asphalt, three of four concrete.
        add_footing(&mut f, &g, 100, &[(0.5, 0.5), (1.5, 0.2), (0.1, 1.9), (1.5, 1.5)]).unwrap();
        assert_eq!(f.per_sealed, vec![1, 0, 1, 1, 1]);
        assert_eq!((f.trees, f.snapshots, f.roof_cells), (4, 1, 4));
        assert_eq!(f.mostly_sealed(), 2, "the roof column and the three-of-four concrete one");
        assert_eq!(f.open_share(), 0.25);
        assert_eq!(f.first_bad.map(|(t, _, _, sealed, roofs)| (t, sealed, roofs)), Some((100, 4, 4)));

        // Two cells of four is a trunk beside a walk: legitimate, and the rule holds.
        let mut ok = Footing::default();
        add_footing(&mut ok, &g, 100, &[(0.5, 0.5), (0.3, 1.2), (0.9, 1.7), (0.0, 1.0)]).unwrap();
        assert_eq!(ok.per_sealed, vec![1, 0, 3, 0, 0]);
        assert_eq!((ok.mostly_sealed(), ok.roof_cells), (0, 0));
        assert_eq!(ok.open_share(), 0.25);

        // A tree outside the grid is an error, not a silent pass.
        let mut off = Footing::default();
        let e = add_footing(&mut off, &g, 300, &[(2.5, 0.5)]).unwrap_err();
        assert!(e.contains("outside the ground grid"), "{e}");
    }

    /// The share floor is the worst snapshot's, not the whole run's: a run that ends with most of
    /// its trees would otherwise dilute a tighter snapshot in the middle away.
    #[test]
    fn footing_share_floor_is_the_worst_snapshot() {
        let g = test_ground();
        let mut f = Footing::default();
        // Tick 100: one tree of two on lawn. Tick 200: nineteen on lawn, one on asphalt.
        add_footing(&mut f, &g, 100, &[(0.5, 0.5), (0.3, 1.2)]).unwrap();
        let many: Vec<(f32, f32)> = (0..19).map(|_| (0.5, 0.5)).chain([(0.3, 1.2)]).collect();
        add_footing(&mut f, &g, 200, &many).unwrap();
        assert_eq!(f.worst_open.map(|(t, _, n)| (t, n)), Some((100, 2)));
        assert_eq!(f.open_share(), 0.5);
        assert!((f.per_sealed[0] as f64 / f.trees as f64 - 20.0 / 22.0).abs() < 1e-9, "the aggregate is higher");
    }

    /// Every side of the footing invariant, and the n/a a run without a bundle ground grid gets.
    #[test]
    fn footing_invariant_fails_each_way_and_is_na_without_a_ground_grid() {
        let h = Healthy {
            period: 3000.0,
            base: 150.0,
            amp: 50.0,
            hunters: 10,
            t0: 20,
            grass: 0.5,
            fertility: 100.0,
            mature: 40,
            ms: 9000,
        };
        let with = |f: Option<Result<Footing, String>>| {
            let mut s = build(&h);
            s.footing = f;
            evaluate(&s).unwrap()
        };
        // A tree on all four cells sealed fails; the same tree on two passes.
        let mut four = healthy_footing();
        four.per_sealed = vec![900, 60, 39, 0, 1];
        assert_eq!(failing(&with_footing(&h, four)), ["tree_footing"]);
        let mut two = healthy_footing();
        two.per_sealed = vec![900, 60, 40, 0, 0];
        assert!(failing(&with_footing(&h, two)).is_empty());
        // One roof cell under one tree fails on its own, sealed count or not.
        let mut roof = healthy_footing();
        roof.roof_cells = 1;
        assert_eq!(failing(&with_footing(&h, roof)), ["tree_footing"]);
        // The share floor: 80% passes, a hair under does not.
        let mut floor = healthy_footing();
        floor.worst_open = Some((4200, TREE_OPEN_SHARE, 500));
        assert!(failing(&with_footing(&h, floor.clone())).is_empty());
        floor.worst_open = Some((4200, TREE_OPEN_SHARE - 1e-6, 500));
        assert_eq!(failing(&with_footing(&h, floor)), ["tree_footing"]);

        // No bundle ground grid: n/a, counting towards neither pass nor fail.
        let r = with(None);
        let l = r.get("tree_footing").unwrap();
        assert!(l.na && l.pass && l.observed.contains(NA_REASON_NO_GROUND), "{l:?}");
        assert_eq!(r.not_applicable(), BUNDLE_ONLY_KEYS.to_vec());
        assert!(r.pass(), "{:?}", r.lines);
        // An unreadable ground grid or snapshot is a failure, not an n/a.
        let r = with(Some(Err("world/medium.bin: missing".into())));
        let l = r.get("tree_footing").unwrap();
        assert!(!l.na && !l.pass && l.observed.contains("world/medium.bin"), "{l:?}");
    }

    fn with_footing(h: &Healthy, f: Footing) -> Series {
        let mut s = build(h);
        s.footing = Some(Ok(f));
        s
    }

    #[test]
    fn long_check_passes_a_healthy_run_and_flags_each_violation() {
        let n = ticks_in(LONG_YEARS, YEAR) as usize + 1;
        let base = |t: usize| if t < 20_000 { 200 } else { 100 + (t % 7) as u32 };
        let healthy = rows_from(base, n);
        let report = evaluate_long(&healthy, true, YEAR).unwrap();
        assert!(report.pass(), "{:?}", report.lines);
        assert_eq!(report.lines.iter().map(|l| l.key).collect::<Vec<_>>(), LONG_KEYS[1..]);

        // A zero anywhere, even before tick 20000, is an extinction; the band only watches 20000+.
        let mut rows = healthy.clone();
        rows[500].hunters = 0;
        assert_eq!(failing_long(&rows), ["long_no_extinction"]);
        // 5.1x the anchor fails the band; 5x and just above 0.2x pass.
        let mut rows = healthy.clone();
        let anchor = rows[ticks_in(LONG_BAND_FROM_YEARS, YEAR) as usize].grazers;
        rows[40_000].grazers = anchor * 5;
        rows[40_001].grazers = anchor.div_ceil(5);
        assert!(failing_long(&rows).is_empty());
        rows[40_000].grazers = anchor * 51 / 10;
        assert_eq!(failing_long(&rows), ["long_band"]);
        let mut rows = healthy.clone();
        rows[59_999].hunters = 26;
        assert_eq!(failing_long(&rows), ["long_band"], "hunters 26 > 5 x 5");
        // Ticks after 60000 are outside the band window (but still watched for extinction).
        let mut rows = rows_from(base, n + 10);
        rows[n + 5].grazers = 10_000;
        assert!(failing_long(&rows).is_empty());
        // Fertility is watched over the whole long run, which is where it saturates.
        let mut rows = healthy.clone();
        rows[45_000].fertility_mean = 220.1;
        assert_eq!(failing_long(&rows), ["long_fertility"]);
        rows[45_000].fertility_mean = 39.9;
        assert_eq!(failing_long(&rows), ["long_fertility"]);
        rows[45_000].fertility_mean = 220.0;
        assert!(failing_long(&rows).is_empty());
    }

    /// Without animals, the long check drops its grazer and hunter parts: `long_band` is n/a and
    /// `long_no_extinction` watches the trees alone, so the all-zero animal columns pass.
    #[test]
    fn long_check_without_animals_marks_the_band_na() {
        let n = ticks_in(LONG_YEARS, YEAR) as usize + 1;
        let mut rows = rows_from(|t| if t < 20_000 { 200 } else { 100 + (t % 7) as u32 }, n);
        rows.iter_mut().for_each(|r| {
            r.grazers = 0;
            r.hunters = 0;
        });
        assert_eq!(failing_long(&rows), ["long_no_extinction"], "with animals, the empty columns are an extinction");
        let report = evaluate_long(&rows, false, YEAR).unwrap();
        assert!(report.pass(), "{:?}", report.lines);
        assert_eq!(report.not_applicable(), ANIMAL_ONLY_LONG_KEYS.to_vec());
        assert_eq!(report.get("long_no_extinction").unwrap().observed, "min trees=20");
        // The trees still count: a tree reaching 0 fails, animals off or not.
        rows[30_000].trees = 0;
        assert_eq!(
            evaluate_long(&rows, false, YEAR)
                .unwrap()
                .lines
                .iter()
                .filter(|l| !l.pass)
                .map(|l| l.key)
                .collect::<Vec<_>>(),
            ["long_no_extinction"]
        );
    }

    #[test]
    fn long_check_on_a_short_run_reports_its_length() {
        let rows = rows_from(|_| 50, 20_001);
        let report = evaluate_long(&rows, true, YEAR).unwrap();
        assert_eq!(report.lines.iter().map(|l| l.key).collect::<Vec<_>>(), LONG_KEYS);
        assert_eq!(failing_long(&rows), ["long_run_length"]);
        assert!(evaluate_long(&rows_from(|_| 50, 10), true, YEAR).unwrap().get("long_band").unwrap().pass);
        assert!(evaluate_long(&[], true, YEAR).is_err());
        let mut gap = rows_from(|_| 50, 10);
        gap.remove(4);
        assert!(evaluate_long(&gap, true, YEAR).unwrap_err().contains("row 4 has tick 5"));
    }

    #[test]
    fn extinctions_attribute_the_window_before_each_species_reaches_zero() {
        let mut rows = rows_from(|t| if t >= 1200 { 0 } else { 100 }, 2001);
        // Grazer deaths: one outside the window (tick 700), then a starved/eaten tie inside it.
        rows[700].deaths[0] = [0, 0, 9, 0, 0];
        rows[701].deaths[0] = [4, 0, 0, 0, 0];
        rows[1200].deaths[0] = [0, 4, 0, 0, 0];
        // Hunters reach 0 at 1500 with only old-age deaths; trees at 1500 too.
        rows[1500].hunters = 0;
        rows[1500].trees = 0;
        rows[1499].deaths[1] = [0, 0, 1, 0, 0];
        let e = extinctions(&rows, true);
        assert_eq!(
            e.iter().map(|x| (x.species, x.tick)).collect::<Vec<_>>(),
            [("grazers", 1200), ("hunters", 1500), ("trees", 1500)]
        );
        assert_eq!(e[0].causes, Some([4, 4, 0, 0, 0]), "window is ticks 701..=1200");
        assert_eq!(e[0].dominant(), Some(Cause::Starved), "ties go to the first cause");
        assert_eq!(e[0].food, Some(("grass_mean", 0.5)));
        assert_eq!(e[1].dominant_name(), "old_age");
        let (label, mean) = e[1].food.unwrap();
        assert!(label == "grazers" && (mean - 39.8).abs() < 1e-9, "100 grazers on 199 of the 500 ticks: {mean}");
        assert_eq!((e[2].causes, e[2].dominant_name()), (None, "unrecorded"));
        assert_eq!(extinction_line(&e[2]), "extinction: trees at tick 1500 (tree death causes are not recorded)");
        // A species at 0 from the start has no deaths to attribute.
        let e = extinctions(&rows_from(|_| 0, 10), true);
        assert_eq!((e[0].tick, e[0].dominant_name()), (0, "none"));
        assert!(extinctions(&rows_from(|_| 5, 10), true).is_empty());
    }

    #[test]
    fn stats_and_diff_on_small_run_dirs() {
        let root = std::env::temp_dir().join(format!("ecosim-check-{}", std::process::id()));
        let (a, b) = (root.join("a"), root.join("b"));
        let mut rows = rows_from(|t| if t == 3 { 0 } else { 10 + t as u32 }, 5);
        rows[2].deaths[0] = [2, 1, 0, 0, 0];
        rows[3].deaths[0] = [1, 0, 0, 0, 0];
        for d in [&a, &b] {
            fs::create_dir_all(d.join("snap_000000")).unwrap();
            fs::write(d.join("series.csv"), crate::output::series_csv(&rows)).unwrap();
            fs::write(d.join("snap_000000").join("height.bin"), [1u8, 2]).unwrap();
        }
        let lines = stats_report(&a).unwrap();
        assert_eq!(lines.len(), 13, "{lines:?}");
        assert_eq!(
            lines[12],
            "extinction: grazers at tick 3; deaths in ticks 0-3: starved=3 eaten=1 old_age=0 crowded=0 burnt=0; \
             dominant cause: starved; mean grass_mean 0.5000"
        );
        assert!(lines[1].starts_with("grazers") && lines[1].contains("14.0000"), "{}", lines[1]);
        assert_eq!(lines[11], "first extinction: tick 3 (grazers=0 hunters=5 trees=20)");
        assert_eq!(diff_runs(&a, &b).unwrap(), Vec::<String>::new());

        fs::write(b.join("timing.json"), "{}").unwrap();
        fs::write(b.join("snap_000000").join("height.bin"), [1u8, 3]).unwrap();
        fs::write(a.join("only_a.txt"), "").unwrap();
        fs::write(b.join("only_b.txt"), "").unwrap();
        let d = diff_runs(&a, &b).unwrap();
        assert_eq!(d.len(), 3, "{d:?}");
        assert!(d.iter().any(|l| l == "differs: snap_000000/height.bin"));
        assert!(d.iter().any(|l| l.starts_with("only in") && l.ends_with("only_a.txt")));
        assert!(d.iter().any(|l| l.starts_with("only in") && l.ends_with("only_b.txt")));

        fs::write(a.join("series.csv"), crate::output::series_csv(&[])).unwrap();
        assert!(stats_report(&a).is_err());
        assert!(stats_report(&root.join("missing")).is_err());
        assert!(diff_runs(&a, &root.join("missing")).is_err());
        fs::remove_dir_all(&root).unwrap();
    }
}
