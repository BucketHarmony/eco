//! `check`, `stats` and `diff`: everything that reads a finished run directory.

use crate::output::{snapshot_dir_name, SERIES_HEADER};
use crate::sim::StatsRow;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const WINDOW_START: u32 = 2000;
pub const RUNTIME_LIMIT_MS: u64 = 30_000;

pub fn read_series(run_dir: &Path) -> Result<Vec<StatsRow>, String> {
    let path = run_dir.join("series.csv");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines();
    if lines.next() != Some(SERIES_HEADER) {
        return Err(format!("{}: unexpected header", path.display()));
    }
    lines
        .enumerate()
        .map(|(i, line)| {
            let f: Vec<&str> = line.split(',').collect();
            if f.len() != 10 {
                return Err(format!("series.csv line {}: expected 10 fields", i + 2));
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
        for s in lo..=hi {
            if s == t {
                continue;
            }
            if let Some(u) = ma[s] {
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
            let plateau_continues = out.last().is_some_and(|&p| ma[p] == Some(v) && (p + 1..t).all(|s| ma[s] == Some(v)));
            if !plateau_continues {
                out.push(t);
            }
        }
    }
    out
}

pub struct CheckLine {
    pub pass: bool,
    pub name: &'static str,
    pub observed: String,
}

fn line(pass: bool, name: &'static str, observed: String) -> CheckLine {
    CheckLine { pass, name, observed }
}

/// Mature-tree count in a snapshot's entities.json.
fn mature_trees_at(run_dir: &Path, tick: u32) -> Result<usize, String> {
    let p: PathBuf = run_dir.join(snapshot_dir_name(tick)).join("entities.json");
    let text = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(v.as_array()
        .map(|a| a.iter().filter(|e| e["kind"] == "tree" && e["stage"] == "mature").count())
        .unwrap_or(0))
}

/// Every invariant from the SAD plus the addendum's extra acceptance checks.
pub fn check_run(run_dir: &Path) -> Result<Vec<CheckLine>, String> {
    let rows = read_series(run_dir)?;
    for (i, r) in rows.iter().enumerate() {
        if r.tick as usize != i {
            return Err(format!("series.csv: row {i} has tick {}", r.tick));
        }
    }
    let last = rows.len().saturating_sub(1);
    let mut out = Vec::new();
    if last < 20_000 {
        out.push(line(false, "run length ≥ 20000 ticks", format!("last tick {last}")));
    }
    let from = (WINDOW_START as usize).min(last);
    let win = &rows[from..];
    let species: [(&str, fn(&StatsRow) -> u32); 3] =
        [("grazers", |r| r.grazers), ("hunters", |r| r.hunters), ("trees", |r| r.trees)];

    // 1. No species count reaches 0.
    let mins: Vec<(&str, u32)> = species.iter().map(|(n, f)| (*n, win.iter().map(f).min().unwrap_or(0))).collect();
    out.push(line(
        mins.iter().all(|m| m.1 > 0),
        "no species reaches 0",
        mins.iter().map(|(n, m)| format!("min {n}={m}")).collect::<Vec<_>>().join(" "),
    ));

    // 2. No species count exceeds 10× its tick-2000 value.
    let mut ok = true;
    let mut obs = Vec::new();
    for (n, f) in species.iter() {
        let base = f(&rows[from]);
        let max = win.iter().map(f).max().unwrap_or(0);
        ok &= max <= 10 * base;
        obs.push(format!("{n} max={max} limit={}", 10 * base));
    }
    out.push(line(ok, "no species exceeds 10x its tick-2000 count", obs.join(" ")));

    // 3. Grazer cycle: ≥ 2 local maxima ≥ 1500 ticks apart.
    let maxima = grazer_maxima(&rows, from, last);
    let span = match (maxima.first(), maxima.last()) {
        (Some(a), Some(b)) => b - a,
        _ => 0,
    };
    out.push(line(
        maxima.len() >= 2 && span >= 1500,
        "grazer cycle (2 maxima >= 1500 ticks apart)",
        format!("{} maxima at {:?}, span {span}", maxima.len(), maxima),
    ));

    // 4. fertility_mean in [40, 220].
    let (fmin, fmax) = range(win.iter().map(|r| r.fertility_mean));
    out.push(line(fmin >= 40.0 && fmax <= 220.0, "fertility_mean in [40, 220]", format!("[{fmin:.2}, {fmax:.2}]")));

    // 5. grass_mean in [0.05, 0.95].
    let (gmin, gmax) = range(win.iter().map(|r| r.grass_mean));
    out.push(line(gmin >= 0.05 && gmax <= 0.95, "grass_mean in [0.05, 0.95]", format!("[{gmin:.4}, {gmax:.4}]")));

    // 6. Succession: trees at the end ≥ 1.5× trees at tick 0.
    let (t0, tn) = (rows[0].trees, rows[last].trees);
    out.push(line(
        tn as f64 >= 1.5 * t0 as f64,
        "trees at end >= 1.5x trees at tick 0",
        format!("{tn} vs {t0} (need {:.1})", 1.5 * t0 as f64),
    ));

    // 7. Runtime under 30 s.
    let timing = fs::read_to_string(run_dir.join("timing.json")).unwrap_or_default();
    let wall = serde_json::from_str::<serde_json::Value>(&timing).ok().and_then(|v| v["wall_ms"].as_u64());
    out.push(match wall {
        Some(ms) => line(ms < RUNTIME_LIMIT_MS, "run time < 30 s", format!("{ms} ms")),
        None => line(false, "run time < 30 s", "timing.json missing".into()),
    });

    // Addendum extras at tick 10000 (renderer pixel tests depend on them).
    if last >= 10_000 {
        let r = &rows[10_000];
        let mature = mature_trees_at(run_dir, 10_000);
        out.push(match mature {
            Ok(m) => line(m >= 35, "mature trees at tick 10000 >= 35", format!("{m}")),
            Err(e) => line(false, "mature trees at tick 10000 >= 35", e),
        });
        out.push(line(
            r.grazers >= 10 && r.hunters >= 2,
            "at tick 10000 grazers >= 10 and hunters >= 2",
            format!("grazers={} hunters={}", r.grazers, r.hunters),
        ));
    } else {
        out.push(line(false, "tick-10000 checks", "run shorter than 10000 ticks".into()));
    }
    Ok(out)
}

fn range(it: impl Iterator<Item = f32>) -> (f32, f32) {
    it.fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)))
}

/// min/max/mean per series column plus first extinction tick, as printable lines.
pub fn stats_report(run_dir: &Path) -> Result<Vec<String>, String> {
    let rows = read_series(run_dir)?;
    if rows.is_empty() {
        return Err("series.csv has no rows".into());
    }
    let cols: [(&str, fn(&StatsRow) -> f64); 9] = [
        ("grazers", |r| r.grazers as f64),
        ("hunters", |r| r.hunters as f64),
        ("trees", |r| r.trees as f64),
        ("grass_mean", |r| r.grass_mean as f64),
        ("shrub_mean", |r| r.shrub_mean as f64),
        ("moisture_mean", |r| r.moisture_mean as f64),
        ("fertility_mean", |r| r.fertility_mean as f64),
        ("detritus_total", |r| r.detritus_total as f64),
        ("temperature", |r| r.temperature as f64),
    ];
    let mut out = vec![format!("{:<16}{:>12}{:>12}{:>12}", "column", "min", "max", "mean")];
    for (name, f) in cols.iter() {
        let v: Vec<f64> = rows.iter().map(f).collect();
        let min = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        out.push(format!("{name:<16}{min:>12.4}{max:>12.4}{mean:>12.4}"));
    }
    let ext = rows.iter().find(|r| r.grazers == 0 || r.hunters == 0 || r.trees == 0);
    out.push(match ext {
        Some(r) => format!(
            "first extinction: tick {} (grazers={} hunters={} trees={})",
            r.tick, r.grazers, r.hunters, r.trees
        ),
        None => "first extinction: none".into(),
    });
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
            })
            .collect()
    }

    #[test]
    fn maxima_detects_a_cycle_and_ignores_flat_lines() {
        let cyc = rows_from(|t| (100.0 + 40.0 * (t as f64 * 2.0 * std::f64::consts::PI / 4000.0).sin()) as u32, 20001);
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
}
