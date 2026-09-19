//! `ecosim sweep`: run a grid of parameter values across seeds in memory and report, per invariant,
//! where the sim passes and where it breaks.
//!
//! Grid order: the first `--param` varies slowest and the last fastest (odometer order), and seeds
//! vary fastest of all, in the order given. Cell results are written in this order whatever order
//! the worker threads finish in.

use crate::check::{
    evaluate, extinctions, grazer_maxima, parse_series, signature, CheckReport, Series, Signature, Timing,
    INVARIANT_KEYS, WINDOW_START,
};
use crate::output::{series_csv, simulate};
use crate::params::Params;
use crate::sim::Sim;
use crate::trees::Stage;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// One swept parameter: a dotted params key and its values, as the exact strings passed to `--set`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    /// Dotted params key, as for `--set`.
    pub key: String,
    /// Values in grid order, as the strings passed to `--set`.
    pub values: Vec<String>,
}

/// Number of digits after the decimal point in a plain decimal literal.
fn decimals(s: &str) -> usize {
    s.split_once('.').map_or(0, |(_, f)| f.len())
}

/// `start:stop:step`, inclusive, as formatted strings. Values are computed as `start + i·step` and
/// printed with as many decimals as the most precise of the three inputs, so there is no float drift.
pub fn parse_range(spec: &str) -> Result<Vec<String>, String> {
    let parts: Vec<&str> = spec.split(':').map(str::trim).collect();
    let &[a, b, c] = &parts[..] else { return Err(format!("--range {spec}: expected start:stop:step")) };
    let num = |s: &str| -> Result<f64, String> {
        if s.contains(['e', 'E']) {
            return Err(format!("--range {spec}: use plain decimals, not exponents"));
        }
        s.parse::<f64>().map_err(|_| format!("--range {spec}: '{s}' is not a number"))
    };
    let (start, stop, step) = (num(a)?, num(b)?, num(c)?);
    if step <= 0.0 || stop < start {
        return Err(format!("--range {spec}: need step > 0 and stop >= start"));
    }
    let places = decimals(a).max(decimals(b)).max(decimals(c));
    let n = ((stop - start) / step + 1e-9).floor() as usize + 1;
    Ok((0..n)
        .map(|i| {
            let s = format!("{:.*}", places, start + i as f64 * step);
            if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
                s[1..].to_string()
            } else {
                s
            }
        })
        .collect())
}

/// `a,b,c` as trimmed strings.
pub fn parse_values(spec: &str) -> Result<Vec<String>, String> {
    let v: Vec<String> = spec.split(',').map(|s| s.trim().to_string()).collect();
    if v.iter().any(String::is_empty) {
        return Err(format!("--values {spec}: empty value"));
    }
    Ok(v)
}

/// One sweep cell: a value index per swept param plus a seed.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Index into each swept param’s values, in `--param` order.
    pub idx: Vec<usize>,
    /// Seed of the run.
    pub seed: u64,
}

/// Full grid in the documented order (first param slowest, seeds fastest).
pub fn grid(specs: &[ParamSpec], seeds: &[u64]) -> Vec<Cell> {
    let mut out = Vec::new();
    let mut idx = vec![0; specs.len()];
    loop {
        for &seed in seeds {
            out.push(Cell { idx: idx.clone(), seed });
        }
        // Odometer increment, last param fastest.
        let mut k = specs.len();
        loop {
            if k == 0 {
                return out;
            }
            k -= 1;
            idx[k] += 1;
            if idx[k] < specs[k].values.len() {
                break;
            }
            idx[k] = 0;
        }
    }
}

/// `p1=v1_p2=v2_s=N`.
pub fn cell_id(specs: &[ParamSpec], cell: &Cell) -> String {
    let mut s = String::new();
    for (p, &i) in specs.iter().zip(&cell.idx) {
        let _ = write!(s, "{}={}_", p.key, p.values[i]);
    }
    let _ = write!(s, "s={}", cell.seed);
    s
}

/// The `--set` strings equivalent to a cell: fixed overrides first, then the swept values.
pub fn cell_overrides(specs: &[ParamSpec], fixed: &[String], cell: &Cell) -> Vec<String> {
    let mut v = fixed.to_vec();
    v.extend(specs.iter().zip(&cell.idx).map(|(p, &i)| format!("{}={}", p.key, p.values[i])));
    v
}

/// One evaluated sweep cell.
pub struct CellResult {
    /// The series exactly as `ecosim run` would write it to `series.csv`.
    pub csv: String,
    /// Invariant results (runtime excluded).
    pub report: CheckReport,
    /// First tick at which any species count is 0, over the whole run.
    pub first_extinction: Option<u32>,
    /// The species at 0 on that tick, joined with `+` when several reach 0 together.
    pub first_extinction_species: Option<String>,
    /// Dominant death cause of the first of them over the preceding `CAUSE_WINDOW` ticks
    /// (`Extinction::dominant_name`).
    pub first_extinction_cause: Option<&'static str>,
    /// Grazer maxima from tick 2000, as the cycle invariant counts them.
    pub grazer_peaks: usize,
    /// First tick at which hunters are 0, over the whole run (immigration may bring them back).
    pub hunter_extinction: Option<u32>,
    /// Hunters that immigrated over the run.
    pub hunter_immigrants: u32,
    /// The predator–prey signature (`pp_lag`, `pp_corr`, `pp_period`, `pp_pass`), as `ecosim stats --signature` reports it.
    pub signature: Signature,
}

/// Run one cell in memory (no snapshots) and evaluate it exactly as `ecosim check` would.
pub fn run_cell(params_text: &str, overrides: &[String], seed: u64, ticks: u32) -> Result<CellResult, String> {
    let params = Params::from_toml_str_with(params_text, overrides)?;
    let year_len = params.climate.year_len;
    let mut sim = Sim::new(params, seed);
    let mut mature = None;
    let rows = simulate(&mut sim, ticks, |s| {
        if s.tick == 10_000 {
            mature = Some(s.trees.iter().filter(|t| t.alive && s.tree_stage(t) == Stage::Mature).count());
        }
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    let csv = series_csv(&rows);
    // Evaluate the parsed CSV, not the in-memory floats, so results match `check` on a run directory.
    let rows = parse_series(&csv)?;
    let last = rows.len() - 1;
    let grazer_peaks = grazer_maxima(&rows, (WINDOW_START as usize).min(last), last).len();
    let ext = extinctions(&rows);
    let first_extinction = ext.first().map(|e| e.tick);
    let first_extinction_species =
        first_extinction.map(|t| ext.iter().filter(|e| e.tick == t).map(|e| e.species).collect::<Vec<_>>().join("+"));
    let first_extinction_cause = ext.first().map(|e| e.dominant_name());
    let hunter_extinction = rows.iter().find(|r| r.hunters == 0).map(|r| r.tick);
    let hunter_immigrants = rows[last].hunter_immigrants;
    let signature = signature(&rows, year_len);
    let report = evaluate(&Series { rows, mature_at_10000: mature.map(Ok), timing: Timing::Excluded })?;
    Ok(CellResult {
        csv,
        report,
        first_extinction,
        first_extinction_species,
        first_extinction_cause,
        grazer_peaks,
        hunter_extinction,
        hunter_immigrants,
        signature,
    })
}

/// Run labelled cells on `jobs` threads; results come back in input order.
fn run_all(
    params_text: &str,
    cells: &[(String, Vec<String>, u64)],
    ticks: u32,
    jobs: usize,
) -> Result<Vec<CellResult>, String> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Result<CellResult, String>>>> = Mutex::new((0..cells.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1).min(cells.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= cells.len() {
                    break;
                }
                let (label, overrides, seed) = &cells[i];
                let t = Instant::now();
                let r = run_cell(params_text, overrides, *seed, ticks);
                let k = done.fetch_add(1, Ordering::SeqCst) + 1;
                let verdict = match &r {
                    Ok(c) if c.report.pass() => "pass".to_string(),
                    Ok(_) => "FAIL".to_string(),
                    Err(e) => format!("error: {e}"),
                };
                eprintln!("[{k}/{}] {label}: {verdict} ({:.1} s)", cells.len(), t.elapsed().as_secs_f64());
                results.lock().unwrap()[i] = Some(r);
            });
        }
    });
    results.into_inner().unwrap().into_iter().map(|r| r.expect("every cell ran")).collect()
}

/// Everything a sweep or baseline needs.
pub struct SweepConfig {
    /// Params file the overrides apply to.
    pub params_path: PathBuf,
    /// `--set` overrides applied to every cell before the swept values.
    pub fixed: Vec<String>,
    /// Swept params, first varying slowest.
    pub specs: Vec<ParamSpec>,
    /// Seeds per grid point, varying fastest.
    pub seeds: Vec<u64>,
    /// Ticks per cell.
    pub ticks: u32,
    /// Worker threads.
    pub jobs: usize,
}

/// Invariant keys that appear in at least one report, in report order (never `runtime`).
fn present_keys(results: &[CellResult]) -> Vec<&'static str> {
    INVARIANT_KEYS.iter().copied().filter(|k| results.iter().any(|r| r.report.get(k).is_some())).collect()
}

fn params_text(cfg: &SweepConfig) -> Result<String, String> {
    fs::read_to_string(&cfg.params_path).map_err(|e| format!("{}: {e}", cfg.params_path.display()))
}

/// `--baseline`: the params file (plus fixed overrides) on each seed.
pub fn baseline(cfg: &SweepConfig) -> Result<Vec<(u64, CheckReport)>, String> {
    let text = params_text(cfg)?;
    Params::from_toml_str_with(&text, &cfg.fixed)?;
    let cells: Vec<_> = cfg.seeds.iter().map(|&s| (format!("baseline s={s}"), cfg.fixed.clone(), s)).collect();
    let res = run_all(&text, &cells, cfg.ticks, cfg.jobs)?;
    Ok(cfg.seeds.iter().copied().zip(res.into_iter().map(|r| r.report)).collect())
}

/// Margin table for `--baseline`: one row per invariant, one column per seed, plus the minimum.
pub fn margin_table(reports: &[(u64, CheckReport)]) -> String {
    let mut s = format!("{:<18}", "invariant");
    for (seed, _) in reports {
        let _ = write!(s, "{:>12}", format!("s{seed}"));
    }
    let _ = writeln!(s, "{:>12}", "min");
    for key in INVARIANT_KEYS {
        let ms: Vec<Option<f64>> = reports.iter().map(|(_, r)| r.get(key).map(|l| l.margin)).collect();
        if ms.iter().all(Option::is_none) {
            continue;
        }
        let _ = write!(s, "{key:<18}");
        for m in &ms {
            let _ = write!(s, "{:>12}", m.map_or("-".into(), |m| format!("{m:+.4}")));
        }
        let min = ms.iter().flatten().copied().fold(f64::INFINITY, f64::min);
        let _ = writeln!(s, "{:>12}", format!("{min:+.4}{}", if min < 0.0 { " !" } else { "" }));
    }
    s
}

/// Create `out`, or clear it if it holds a previous sweep. Refuses anything else.
fn prepare_out(out: &Path) -> Result<(), String> {
    if out.exists() {
        let empty = fs::read_dir(out).map_err(|e| e.to_string())?.next().is_none();
        if !empty && !out.join("sweep.csv").exists() {
            return Err(format!("{} exists and is not a sweep directory; refusing to overwrite", out.display()));
        }
        fs::remove_dir_all(out).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(out.join("cells")).map_err(|e| e.to_string())
}

fn num(v: f64) -> String {
    if v.is_nan() {
        String::new()
    } else {
        format!("{v}")
    }
}

/// Run the grid and write `sweep.csv`, `sweep.md` and `cells/<cell-id>.csv` into `out`.
pub fn sweep(cfg: &SweepConfig, out: &Path) -> Result<Vec<CellResult>, String> {
    if cfg.specs.is_empty() {
        return Err("sweep needs at least one --param (or --baseline)".into());
    }
    let text = params_text(cfg)?;
    let cells = grid(&cfg.specs, &cfg.seeds);
    let jobs: Vec<_> =
        cells.iter().map(|c| (cell_id(&cfg.specs, c), cell_overrides(&cfg.specs, &cfg.fixed, c), c.seed)).collect();
    // Validate every cell's overrides before running anything.
    for (_, o, _) in &jobs {
        Params::from_toml_str_with(&text, o)?;
    }
    prepare_out(out)?;
    let start = Instant::now();
    let results = run_all(&text, &jobs, cfg.ticks, cfg.jobs)?;
    let wall = start.elapsed().as_secs_f64();

    let keys = present_keys(&results);
    let mut csv = String::new();
    for p in &cfg.specs {
        let _ = write!(csv, "{},", p.key);
    }
    csv.push_str("seed");
    for k in &keys {
        let _ = write!(csv, ",{k}_pass,{k}_value,{k}_margin");
    }
    csv.push_str(",first_extinction_tick,first_extinction_species,first_extinction_dominant_cause");
    csv.push_str(
        ",grazer_peaks,hunter_extinction_tick,hunter_immigrants,pp_lag,pp_corr,pp_undefined,pp_period,pp_pass\n",
    );
    for ((cell, (id, _, _)), r) in cells.iter().zip(&jobs).zip(&results) {
        for (p, &i) in cfg.specs.iter().zip(&cell.idx) {
            let _ = write!(csv, "{},", p.values[i]);
        }
        let _ = write!(csv, "{}", cell.seed);
        for k in &keys {
            match r.report.get(k) {
                Some(l) => {
                    let _ = write!(csv, ",{},{},{:.6}", l.pass, num(l.value), l.margin);
                }
                None => csv.push_str(",,,"),
            }
        }
        let ext = r.first_extinction.map_or(String::new(), |t| t.to_string());
        let species = r.first_extinction_species.as_deref().unwrap_or("");
        let cause = r.first_extinction_cause.unwrap_or("");
        let hext = r.hunter_extinction.map_or(String::new(), |t| t.to_string());
        let pp = match &r.signature {
            Signature::Cycle { lag, corr, period } => {
                format!("{lag},{corr:.4},,{}", period.map_or(String::new(), |p| p.to_string()))
            }
            Signature::Extinct(e) => format!(",,{} {},", e.species, e.dominant_name()),
            Signature::Flat => ",,flat,".into(),
        };
        let pass = r.signature.pass();
        let _ = writeln!(csv, ",{ext},{species},{cause},{},{hext},{},{pp},{pass}", r.grazer_peaks, r.hunter_immigrants);
        fs::write(out.join("cells").join(format!("{id}.csv")), &r.csv).map_err(|e| e.to_string())?;
    }
    fs::write(out.join("sweep.csv"), csv).map_err(|e| e.to_string())?;
    let md = report_md(cfg, &text, &cells, &results, &keys, wall, out);
    fs::write(out.join("sweep.md"), md).map_err(|e| e.to_string())?;
    Ok(results)
}

/// Current value of a dotted key in the params text after fixed overrides, for marking the default.
fn default_value(text: &str, fixed: &[String], key: &str) -> Option<String> {
    let mut root = toml::Value::Table(text.parse::<toml::Table>().ok()?);
    for f in fixed {
        crate::params::apply_override(&mut root, f).ok()?;
    }
    let mut cur = &root;
    for part in key.split('.') {
        cur = cur.as_table()?.get(part)?;
    }
    Some(cur.to_string())
}

fn same_value(a: &str, b: &str) -> bool {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => (x - y).abs() <= 1e-9 * x.abs().max(1.0),
        _ => a == b,
    }
}

/// Failing invariants over a set of results: (key, failing seeds, worst margin), most-failing first,
/// ties broken by the most negative margin.
fn failures(set: &[(&Cell, &CellResult)], keys: &[&'static str]) -> Vec<(&'static str, Vec<u64>, f64)> {
    let mut out: Vec<(&'static str, Vec<u64>, f64)> = keys
        .iter()
        .filter_map(|&k| {
            let fails: Vec<(u64, f64)> = set
                .iter()
                .filter_map(|(c, r)| r.report.get(k).filter(|l| !l.pass).map(|l| (c.seed, l.margin)))
                .collect();
            if fails.is_empty() {
                return None;
            }
            let mut seeds: Vec<u64> = fails.iter().map(|f| f.0).collect();
            seeds.sort_unstable();
            seeds.dedup();
            let worst = fails.iter().map(|f| f.1).fold(f64::INFINITY, f64::min);
            Some((k, seeds, worst))
        })
        .collect();
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.2.total_cmp(&b.2)));
    out
}

fn seeds_str(seeds: &[u64]) -> String {
    seeds.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")
}

fn fail_summary(f: &[(&'static str, Vec<u64>, f64)]) -> String {
    f.iter()
        .map(|(k, s, m)| {
            format!("`{k}` (seed{} {}; worst margin {m:+.3})", if s.len() > 1 { "s" } else { "" }, seeds_str(s))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn report_md(
    cfg: &SweepConfig,
    text: &str,
    cells: &[Cell],
    results: &[CellResult],
    keys: &[&'static str],
    wall: f64,
    out: &Path,
) -> String {
    let n_seeds = cfg.seeds.len();
    let mut md = String::new();
    let name = out.file_name().map_or("sweep".into(), |n| n.to_string_lossy().to_string());
    let _ = writeln!(md, "# Sweep `{name}`\n");
    let _ = writeln!(md, "- params: `{}`", cfg.params_path.display());
    if !cfg.fixed.is_empty() {
        let _ = writeln!(
            md,
            "- fixed overrides: {}",
            cfg.fixed.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", ")
        );
    }
    for p in &cfg.specs {
        let _ = writeln!(md, "- `{}`: {}", p.key, p.values.join(", "));
    }
    let _ = writeln!(
        md,
        "- seeds: {}; ticks: {}; cells: {}; jobs: {}",
        seeds_str(&cfg.seeds),
        cfg.ticks,
        cells.len(),
        cfg.jobs
    );
    let _ = writeln!(md, "- wall time: {wall:.1} s");
    let _ = writeln!(
        md,
        "- invariants: {} (runtime excluded)\n",
        keys.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(
        md,
        "A value is **safe** when every invariant passes on every seed in every cell with that value \
         (across all values of the other swept params). The safe band is the contiguous run of safe values \
         containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, \
         i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in \
         the most cells at the neighbouring value, ties to the most negative margin.\n"
    );
    let pairs: Vec<(&Cell, &CellResult)> = cells.iter().zip(results).collect();
    for (pi, p) in cfg.specs.iter().enumerate() {
        let default = default_value(text, &cfg.fixed, &p.key);
        let at = |vi: usize| -> Vec<(&Cell, &CellResult)> {
            pairs.iter().copied().filter(|(c, _)| c.idx[pi] == vi).collect()
        };
        let safe: Vec<bool> = (0..p.values.len()).map(|vi| at(vi).iter().all(|(_, r)| r.report.pass())).collect();
        let mut bands: Vec<(usize, usize)> = Vec::new();
        for (i, &ok) in safe.iter().enumerate() {
            if ok {
                match bands.last_mut() {
                    Some(b) if b.1 + 1 == i => b.1 = i,
                    _ => bands.push((i, i)),
                }
            }
        }
        let def_idx = default.as_ref().and_then(|d| p.values.iter().position(|v| same_value(v, d)));
        let band = def_idx
            .and_then(|d| bands.iter().copied().find(|b| b.0 <= d && d <= b.1))
            .or_else(|| bands.iter().copied().max_by_key(|b| (b.1 - b.0, std::cmp::Reverse(b.0))));
        let _ = writeln!(md, "## `{}`\n", p.key);
        let _ = writeln!(md, "- default: {}", default.as_deref().unwrap_or("?"));
        match band {
            None => {
                let _ = writeln!(md, "- safe band: **none** — **fragile**");
            }
            Some((lo, hi)) => {
                let count = hi - lo + 1;
                let _ = writeln!(
                    md,
                    "- safe band: **[{}, {}]** ({count} of {} values){}",
                    p.values[lo],
                    p.values[hi],
                    p.values.len(),
                    if count < 3 { " — **fragile**" } else { "" }
                );
                if bands.len() > 1 {
                    let others: Vec<String> = bands
                        .iter()
                        .filter(|b| **b != (lo, hi))
                        .map(|b| format!("[{}, {}]", p.values[b.0], p.values[b.1]))
                        .collect();
                    let _ = writeln!(md, "- other safe runs: {}", others.join(", "));
                }
                for (label, edge) in
                    [("lower", lo.checked_sub(1)), ("upper", Some(hi + 1).filter(|&i| i < p.values.len()))]
                {
                    match edge {
                        None => {
                            let _ = writeln!(md, "- {label} edge: passes to the end of the grid");
                        }
                        Some(e) => {
                            let f = failures(&at(e), keys);
                            let _ = writeln!(
                                md,
                                "- {label} edge: at {} first fails {}",
                                p.values[e],
                                fail_summary(&f[..1])
                            );
                            if f.len() > 1 {
                                let _ = writeln!(md, "  - also failing there: {}", fail_summary(&f[1..]));
                            }
                        }
                    }
                }
            }
        }
        let _ = writeln!(md, "\n| value | cells passing | failing invariants (seeds; worst margin) |\n|---|---|---|");
        for (vi, v) in p.values.iter().enumerate() {
            let set = at(vi);
            let passing = set.iter().filter(|(_, r)| r.report.pass()).count();
            let mark = if Some(vi) == def_idx { " (default)" } else { "" };
            let f = failures(&set, keys);
            let _ = writeln!(
                md,
                "| {v}{mark} | {passing}/{} | {} |",
                set.len(),
                if f.is_empty() { "—".into() } else { fail_summary(&f) }
            );
        }
        md.push('\n');
    }
    if cfg.specs.len() == 2 {
        let (a, b) = (&cfg.specs[0], &cfg.specs[1]);
        let _ = writeln!(md, "## Matrix: seeds passing out of {n_seeds}\n");
        let _ = writeln!(md, "| `{}` \\ `{}` | {} |", a.key, b.key, b.values.join(" | "));
        let _ = writeln!(md, "|---|{}", "---|".repeat(b.values.len()));
        for (ai, av) in a.values.iter().enumerate() {
            let row: Vec<String> = (0..b.values.len())
                .map(|bi| {
                    let k = pairs.iter().filter(|(c, r)| c.idx == [ai, bi] && r.report.pass()).count();
                    format!("{k}/{n_seeds}")
                })
                .collect();
            let _ = writeln!(md, "| {av} | {} |", row.join(" | "));
        }
        md.push('\n');
    }
    md.push_str(&extinctions_md(cfg, &pairs));
    md
}

/// The `sweep.md` extinctions section: extinction cells counted apart from invariant failures,
/// then grouped by the first species to reach 0 and its dominant death cause.
fn extinctions_md(cfg: &SweepConfig, pairs: &[(&Cell, &CellResult)]) -> String {
    let n = pairs.len();
    let failing = pairs.iter().filter(|(_, r)| !r.report.pass()).count();
    let extinct: Vec<_> = pairs.iter().filter(|(_, r)| r.first_extinction.is_some()).collect();
    let failing_alive = pairs.iter().filter(|(_, r)| !r.report.pass() && r.first_extinction.is_none()).count();
    let passing_extinct = extinct.iter().filter(|(_, r)| r.report.pass()).count();
    let mut md = String::from("## Extinctions by cause\n\n");
    let _ = writeln!(md, "- cells failing an invariant: {failing} of {n}");
    let _ = writeln!(md, "- cells in which a species reaches 0 at any tick: {} of {n}", extinct.len());
    let _ = writeln!(
        md,
        "- failing cells with no extinction: {failing_alive}; extinction cells passing every invariant: \
         {passing_extinct}\n"
    );
    if extinct.is_empty() {
        return md;
    }
    let _ = writeln!(
        md,
        "Grouped by the first species to reach 0 and its dominant death cause over the {} ticks ending there.\n",
        crate::check::CAUSE_WINDOW
    );
    let mut groups: Vec<((&str, &str), Vec<String>)> = Vec::new();
    for (c, r) in &extinct {
        let key = (r.first_extinction_species.as_deref().unwrap_or(""), r.first_extinction_cause.unwrap_or(""));
        let label = format!("{} @{}", cell_id(&cfg.specs, c), r.first_extinction.unwrap_or(0));
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.1.push(label),
            None => groups.push((key, vec![label])),
        }
    }
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    let _ = writeln!(md, "| first extinct | dominant cause | cells | cell @ tick |\n|---|---|---|---|");
    for ((species, cause), cells) in &groups {
        let _ = writeln!(md, "| {species} | `{cause}` | {} | {} |", cells.len(), cells.join(", "));
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn range_is_inclusive_and_drift_free() {
        let v = parse_range("0.04:0.16:0.02").unwrap();
        assert_eq!(v, ["0.04", "0.06", "0.08", "0.10", "0.12", "0.14", "0.16"]);
        for (i, s) in v.iter().enumerate() {
            assert!((s.parse::<f64>().unwrap() - (0.04 + 0.02 * i as f64)).abs() < 1e-9);
        }
        assert_eq!(parse_range("500:2500:500").unwrap(), ["500", "1000", "1500", "2000", "2500"]);
        assert_eq!(parse_range("0:12:3").unwrap(), ["0", "3", "6", "9", "12"]);
        assert_eq!(parse_range("0.1:0.5:0.1").unwrap(), ["0.1", "0.2", "0.3", "0.4", "0.5"]);
        assert_eq!(parse_range("0.2:0.8:0.1").unwrap().len(), 7);
        assert_eq!(parse_range("1:2:0.4").unwrap(), ["1.0", "1.4", "1.8"]);
        for bad in ["1:2", "a:2:1", "2:1:1", "1:2:0", "1:2:1e-1"] {
            assert!(parse_range(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn values_parse() {
        assert_eq!(parse_values("0.2, 0.3,0.4").unwrap(), ["0.2", "0.3", "0.4"]);
        assert!(parse_values("0.2,,0.4").is_err());
    }

    #[test]
    fn grid_order_is_first_param_slowest_seed_fastest() {
        let specs = vec![
            ParamSpec { key: "a.x".into(), values: vec!["1".into(), "2".into()] },
            ParamSpec { key: "b.y".into(), values: vec!["p".into(), "q".into(), "r".into()] },
        ];
        let g = grid(&specs, &[7, 3]);
        assert_eq!(g.len(), 12);
        let ids: Vec<String> = g.iter().map(|c| cell_id(&specs, c)).collect();
        assert_eq!(&ids[..4], ["a.x=1_b.y=p_s=7", "a.x=1_b.y=p_s=3", "a.x=1_b.y=q_s=7", "a.x=1_b.y=q_s=3"]);
        assert_eq!(ids[11], "a.x=2_b.y=r_s=3");
        assert_eq!(cell_overrides(&specs, &["c.z=0".into()], &g[5]), ["c.z=0", "a.x=1", "b.y=r"]);
        assert_eq!(grid(&specs, &[1]), grid(&specs, &[1]));
    }

    /// `units / 10^places` as a plain decimal string.
    fn decimal(units: i64, places: u32) -> String {
        let scale = 10i64.pow(places);
        let (sign, a) = (if units < 0 { "-" } else { "" }, units.abs());
        if places == 0 {
            format!("{sign}{a}")
        } else {
            format!("{sign}{}.{:0w$}", a / scale, a % scale, w = places as usize)
        }
    }

    /// A range written in whole units of 10^-places: start `s`, span `span`, step `step` (units).
    /// The count is span / step + 1 (exact integer division), the values are start + i·step, and the
    /// last never passes stop by more than 1e-9.
    fn range_count_and_bounds(s: i64, span: i64, step: i64, places: u32) -> Result<(), TestCaseError> {
        let spec = format!("{}:{}:{}", decimal(s, places), decimal(s + span, places), decimal(step, places));
        let v = parse_range(&spec).map_err(TestCaseError::fail)?;
        prop_assert_eq!(v.len() as i64, span / step + 1, "{}", spec);
        let (scale, stop) = (10f64.powi(places as i32), (s + span) as f64 / 10f64.powi(places as i32));
        for (i, x) in v.iter().enumerate() {
            let want = (s + i as i64 * step) as f64 / scale;
            prop_assert!((x.parse::<f64>().unwrap() - want).abs() < 1e-9, "{}: value {} is {}", spec, i, x);
        }
        prop_assert!(v.last().unwrap().parse::<f64>().unwrap() <= stop + 1e-9, "{}", spec);
        Ok(())
    }

    proptest! {
        #[test]
        fn prop_range_count_and_bounds(s in -5000i64..5000, span in 0i64..5000, step in 1i64..500, places in 0u32..=3) {
            range_count_and_bounds(s, span, step, places)?;
        }
    }

    #[test]
    fn range_regression_float_quotient_just_below_integer() {
        // (0.16 − 0.04) / 0.02 is 5.999999999999999 in f64; the count must still be 7.
        range_count_and_bounds(4, 12, 2, 2).unwrap();
        range_count_and_bounds(-7, 0, 3, 1).unwrap();
    }
}
