//! `check`, `stats` and `diff`: everything that reads a finished run directory. The invariants
//! themselves live in [`evaluate`], which `check` and `sweep` share.

use crate::animals::{Cause, CAUSES};
use crate::output::{snapshot_dir_name, SERIES_FIELDS, SERIES_HEADER, TRAIT_FIELDS};
use crate::sim::StatsRow;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// First tick of the invariant window; the burn-in before it is ignored.
pub const WINDOW_START: u32 = 2000;
/// Anchor tick of the trees' `max_10x` limit. Trees grow slowly from a dozen, so a tick-2000 anchor
/// measured establishment speed rather than a runaway; animals keep `WINDOW_START`.
pub const TREE_ANCHOR: u32 = 5000;
/// Minimum length of a run for `check --long`.
pub const LONG_TICKS: u32 = 60_000;
/// Start of the `check --long` population band window and the tick its bounds are relative to.
pub const LONG_BAND_FROM: u32 = 20_000;
/// Runtime invariant limit for a 20000-tick run, in milliseconds.
pub const RUNTIME_LIMIT_MS: u64 = 30_000;

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

/// Parse `series.csv` text. Sweeps parse their own in-memory CSV through this too, so a sweep cell
/// is evaluated on exactly the values a run directory would hold.
///
/// Runs written before the trait columns (shot 11) or before fire (shot 9) as well still parse, with
/// the missing columns read as 0.
pub fn parse_series(text: &str) -> Result<Vec<StatsRow>, String> {
    let mut lines = text.lines();
    let header = lines.next().ok_or("unexpected header")?;
    let fields = [SERIES_FIELDS, SERIES_FIELDS - TRAIT_FIELDS, SERIES_FIELDS - TRAIT_FIELDS - FIRE_FIELDS]
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
                traits: if fields == SERIES_FIELDS {
                    let mut t = [[0.0; TRAIT_FIELDS / 2]; 2];
                    for (k, v) in t.iter_mut().flatten().enumerate() {
                        *v = x(23 + k)?;
                    }
                    t
                } else {
                    Default::default()
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
    /// Mature trees in the tick-10000 snapshot; `None` when the run is shorter than 10000 ticks.
    pub mature_at_10000: Option<Result<usize, String>>,
    /// Wall time of the run, if known and evaluated.
    pub timing: Timing,
}

/// Where the runtime invariant gets its wall time.
pub enum Timing {
    /// Wall time from `timing.json`.
    Ms(u64),
    /// `timing.json` absent or unreadable: the runtime invariant fails.
    Missing,
    /// Not evaluated (sweeps: wall time isn't comparable across parallel jobs).
    Excluded,
}

impl Series {
    /// Load the series, the tick-10000 mature-tree count and `timing.json` from a run directory.
    pub fn from_run_dir(run_dir: &Path) -> Result<Series, String> {
        let rows = read_series(run_dir)?;
        let mature_at_10000 = (rows.len() > 10_000).then(|| mature_trees_at(run_dir, 10_000));
        let text = fs::read_to_string(run_dir.join("timing.json")).unwrap_or_default();
        let timing = match serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| v["wall_ms"].as_u64()) {
            Some(ms) => Timing::Ms(ms),
            None => Timing::Missing,
        };
        Ok(Series { rows, mature_at_10000, timing })
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
    /// Whether the invariant holds.
    pub pass: bool,
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
    /// True when every invariant passes.
    pub fn pass(&self) -> bool {
        self.lines.iter().all(|l| l.pass)
    }

    /// The line for an invariant key, if it was evaluated.
    pub fn get(&self, key: &str) -> Option<&CheckLine> {
        self.lines.iter().find(|l| l.key == key)
    }
}

/// Every invariant key in report order. `run_length` and `tick_10000` only appear for short runs,
/// and `runtime` only when timing is evaluated.
pub const INVARIANT_KEYS: [&str; 11] = [
    "run_length",
    "no_extinction",
    "max_10x",
    "grazer_cycle",
    "fertility_band",
    "grass_band",
    "tree_growth",
    "runtime",
    "tick_10000",
    "mature_trees_10k",
    "animals_10k",
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
        self.0.push(CheckLine { key, name, pass, observed, value, threshold, margin });
    }
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
    if last < 20_000 {
        let vtm = (last as f64, 20_000.0, margin_at_least(last as f64, 20_000.0));
        out.push("run_length", "run length ≥ 20000 ticks", false, format!("last tick {last}"), vtm);
    }
    let from = (WINDOW_START as usize).min(last);
    let win = &rows[from..];
    let species: [(&str, Column<u32>); 3] =
        [("grazers", |r| r.grazers), ("hunters", |r| r.hunters), ("trees", |r| r.trees)];

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
        let anchor = if *n == "trees" { (TREE_ANCHOR as usize).min(last) } else { from };
        let base = f(&rows[anchor]);
        let max = win.iter().map(f).max().unwrap_or(0);
        ok &= max <= 10 * base;
        obs.push(format!("{n} max={max} limit={}", 10 * base));
        parts.push((max as f64, (10 * base) as f64, margin_at_most(max as f64, (10 * base) as f64)));
    }
    out.push(
        "max_10x",
        "no species exceeds 10x its anchor count (animals: tick 2000, trees: tick 5000)",
        ok,
        obs.join(" "),
        tightest(parts),
    );

    // 3. Grazer cycle: ≥ 2 local maxima ≥ 1500 ticks apart (value = first-to-last span, 0 if < 2).
    let maxima = grazer_maxima(rows, from, last);
    let span = match (maxima.first(), maxima.last()) {
        (Some(a), Some(b)) => b - a,
        _ => 0,
    };
    out.push(
        "grazer_cycle",
        "grazer cycle (2 maxima >= 1500 ticks apart)",
        maxima.len() >= 2 && span >= 1500,
        format!("{} maxima at {:?}, span {span}", maxima.len(), maxima),
        (span as f64, 1500.0, margin_at_least(span as f64, 1500.0)),
    );

    // 4. fertility_mean in [40, 220].
    let (fmin, fmax) = range(win.iter().map(|r| r.fertility_mean));
    out.push(
        "fertility_band",
        "fertility_mean in [40, 220]",
        fmin >= 40.0 && fmax <= 220.0,
        format!("[{fmin:.2}, {fmax:.2}]"),
        band(fmin as f64, fmax as f64, 40.0, 220.0),
    );

    // 5. grass_mean in [0.05, 0.95].
    let (gmin, gmax) = range(win.iter().map(|r| r.grass_mean));
    out.push(
        "grass_band",
        "grass_mean in [0.05, 0.95]",
        gmin >= 0.05 && gmax <= 0.95,
        format!("[{gmin:.4}, {gmax:.4}]"),
        band(gmin as f64, gmax as f64, 0.05, 0.95),
    );

    // 6. Succession: trees at the end ≥ 1.5× trees at tick 0.
    let (t0, tn) = (rows[0].trees, rows[last].trees);
    let need = 1.5 * t0 as f64;
    out.push(
        "tree_growth",
        "trees at end >= 1.5x trees at tick 0",
        tn as f64 >= need,
        format!("{tn} vs {t0} (need {need:.1})"),
        (tn as f64, need, margin_at_least(tn as f64, need)),
    );

    // 7. Runtime under 30 s.
    let limit = RUNTIME_LIMIT_MS as f64;
    match series.timing {
        Timing::Ms(ms) => out.push(
            "runtime",
            "run time < 30 s",
            ms < RUNTIME_LIMIT_MS,
            format!("{ms} ms"),
            (ms as f64, limit, margin_at_most(ms as f64, limit)),
        ),
        Timing::Missing => {
            out.push("runtime", "run time < 30 s", false, "timing.json missing".into(), (f64::NAN, limit, -1.0))
        }
        Timing::Excluded => {}
    }

    // Addendum extras at tick 10000 (renderer pixel tests depend on them).
    match &series.mature_at_10000 {
        Some(mature) if last >= 10_000 => {
            let name = "mature trees at tick 10000 >= 35";
            match mature {
                Ok(m) => {
                    let m = *m as f64;
                    out.push("mature_trees_10k", name, m >= 35.0, format!("{m}"), (m, 35.0, margin_at_least(m, 35.0)))
                }
                Err(e) => out.push("mature_trees_10k", name, false, e.clone(), (f64::NAN, 35.0, -1.0)),
            }
            let r = &rows[10_000];
            let (g, h) = (r.grazers as f64, r.hunters as f64);
            out.push(
                "animals_10k",
                "at tick 10000 grazers >= 10 and hunters >= 2",
                r.grazers >= 10 && r.hunters >= 2,
                format!("grazers={} hunters={}", r.grazers, r.hunters),
                tightest([(g, 10.0, margin_at_least(g, 10.0)), (h, 2.0, margin_at_least(h, 2.0))]),
            );
        }
        _ => out.push(
            "tick_10000",
            "tick-10000 checks",
            false,
            "run shorter than 10000 ticks".into(),
            (last as f64, 10_000.0, margin_at_least(last as f64, 10_000.0)),
        ),
    }
    Ok(CheckReport { lines: out.0 })
}

/// Every `check --long` key in report order; `long_run_length` only appears for short runs.
pub const LONG_KEYS: [&str; 3] = ["long_run_length", "long_no_extinction", "long_band"];

/// `ecosim check --long` on a run directory.
pub fn check_run_long(run_dir: &Path) -> Result<CheckReport, String> {
    evaluate_long(&read_series(run_dir)?)
}

/// The long-run invariants, for runs of at least `LONG_TICKS`: no species reaches 0 at any tick,
/// and grazers and hunters stay within [0.2×, 5×] of their tick-20000 count over ticks 20000–60000.
/// These replace, rather than extend, the 20000-tick invariants.
pub fn evaluate_long(rows: &[StatsRow]) -> Result<CheckReport, String> {
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
    let long = LONG_TICKS as f64;
    if last < LONG_TICKS as usize {
        let vtm = (last as f64, long, margin_at_least(last as f64, long));
        out.push("long_run_length", "run length >= 60000 ticks", false, format!("last tick {last}"), vtm);
    }
    let species: [(&str, Column<u32>); 3] =
        [("grazers", |r| r.grazers), ("hunters", |r| r.hunters), ("trees", |r| r.trees)];
    let mins: Vec<(&str, u32)> = species.iter().map(|(n, f)| (*n, rows.iter().map(f).min().unwrap_or(0))).collect();
    out.push(
        "long_no_extinction",
        "no species reaches 0 over the whole run",
        mins.iter().all(|m| m.1 > 0),
        mins.iter().map(|(n, m)| format!("min {n}={m}")).collect::<Vec<_>>().join(" "),
        tightest(mins.iter().map(|&(_, m)| (m as f64, 1.0, margin_at_least(m as f64, 1.0)))),
    );
    let from = (LONG_BAND_FROM as usize).min(last);
    let win = &rows[from..=(LONG_TICKS as usize).min(last)];
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
    out.push(
        "long_band",
        "grazers and hunters over ticks 20000-60000 within [0.2x, 5x] of their tick-20000 count",
        ok,
        obs.join(" "),
        tightest(parts),
    );
    Ok(CheckReport { lines: out.0 })
}

fn range(it: impl Iterator<Item = f32>) -> (f32, f32) {
    it.fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)))
}

/// First row at which any species count is 0, over the whole run.
pub fn first_extinction(rows: &[StatsRow]) -> Option<&StatsRow> {
    rows.iter().find(|r| r.grazers == 0 || r.hunters == 0 || r.trees == 0)
}

/// Ticks of death counts attributed to an extinction: the extinction tick and the 499 before it.
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
/// reaching 0 on the same tick keep the order grazers, hunters, trees.
pub fn extinctions(rows: &[StatsRow]) -> Vec<Extinction> {
    let species: [(&'static str, Column<u32>); 3] =
        [("grazers", |r| r.grazers), ("hunters", |r| r.hunters), ("trees", |r| r.trees)];
    let mut out: Vec<Extinction> = species
        .iter()
        .enumerate()
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

/// min/max/mean per series column plus first extinction tick, as printable lines.
pub fn stats_report(run_dir: &Path) -> Result<Vec<String>, String> {
    let rows = read_series(run_dir)?;
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
    out.push(match first_extinction(&rows) {
        Some(r) => {
            format!("first extinction: tick {} (grazers={} hunters={} trees={})", r.tick, r.grazers, r.hunters, r.trees)
        }
        None => "first extinction: none".into(),
    });
    out.extend(extinctions(&rows).iter().map(extinction_line));
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
            })
            .collect()
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
        let s = Series { rows, mature_at_10000: Some(Ok(42)), timing: Timing::Excluded };
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
        for l in &r.lines {
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
            })
            .collect();
        Series { rows, mature_at_10000: Some(Ok(h.mature)), timing: Timing::Ms(h.ms) }
    }

    /// The invariants a violation may break: all but `tick_10000`, which needs a run under 10000 ticks.
    const VIOLABLE: [&str; 10] = [
        "run_length",
        "no_extinction",
        "max_10x",
        "grazer_cycle",
        "fertility_band",
        "grass_band",
        "tree_growth",
        "runtime",
        "mature_trees_10k",
        "animals_10k",
    ];

    /// Break exactly one invariant at tick `t` (in [2001, 19999], never 5000 or 10000). `side` picks the band
    /// side; `keep` is the row count for `run_length` (15001..=20000).
    fn violate(s: &mut Series, key: &str, t: usize, side: bool, keep: usize) {
        let rows = &mut s.rows;
        match key {
            "run_length" => rows.truncate(keep),
            "no_extinction" => rows[t].hunters = 0,
            "max_10x" => rows[t].trees = 10 * rows[TREE_ANCHOR as usize].trees + 1,
            "grazer_cycle" => rows.iter_mut().for_each(|r| r.grazers = 150),
            "fertility_band" => rows[t].fertility_mean = if side { 39.9 } else { 220.1 },
            "grass_band" => rows[t].grass_mean = if side { 0.049 } else { 0.951 },
            "tree_growth" => rows[20_000].trees = rows[0].trees,
            "runtime" => s.timing = Timing::Ms(30_000 + t as u64),
            "mature_trees_10k" => s.mature_at_10000 = Some(Ok(t % 35)),
            "animals_10k" => rows[10_000].hunters = 1,
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
        evaluate_long(rows).unwrap().lines.iter().filter(|l| !l.pass).map(|l| l.key).collect()
    }

    #[test]
    fn long_check_passes_a_healthy_run_and_flags_each_violation() {
        let n = LONG_TICKS as usize + 1;
        let base = |t: usize| if t < 20_000 { 200 } else { 100 + (t % 7) as u32 };
        let healthy = rows_from(base, n);
        let report = evaluate_long(&healthy).unwrap();
        assert!(report.pass(), "{:?}", report.lines);
        assert_eq!(report.lines.iter().map(|l| l.key).collect::<Vec<_>>(), LONG_KEYS[1..]);

        // A zero anywhere, even before tick 20000, is an extinction; the band only watches 20000+.
        let mut rows = healthy.clone();
        rows[500].hunters = 0;
        assert_eq!(failing_long(&rows), ["long_no_extinction"]);
        // 5.1x the anchor fails the band; 5x and just above 0.2x pass.
        let mut rows = healthy.clone();
        let anchor = rows[LONG_BAND_FROM as usize].grazers;
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
    }

    #[test]
    fn long_check_on_a_short_run_reports_its_length() {
        let rows = rows_from(|_| 50, 20_001);
        let report = evaluate_long(&rows).unwrap();
        assert_eq!(report.lines.iter().map(|l| l.key).collect::<Vec<_>>(), LONG_KEYS);
        assert_eq!(failing_long(&rows), ["long_run_length"]);
        assert!(evaluate_long(&rows_from(|_| 50, 10)).unwrap().get("long_band").unwrap().pass);
        assert!(evaluate_long(&[]).is_err());
        let mut gap = rows_from(|_| 50, 10);
        gap.remove(4);
        assert!(evaluate_long(&gap).unwrap_err().contains("row 4 has tick 5"));
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
        let e = extinctions(&rows);
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
        let e = extinctions(&rows_from(|_| 0, 10));
        assert_eq!((e[0].tick, e[0].dominant_name()), (0, "none"));
        assert!(extinctions(&rows_from(|_| 5, 10)).is_empty());
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
