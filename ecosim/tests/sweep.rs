//! Sweep harness and shared-check tests: a sweep cell must equal `ecosim run --set …` + `ecosim check`.

use ecosim::check::{check_run, evaluate, parse_series, CheckReport, Series};
use ecosim::output::run;
use ecosim::sweep::{baseline, sweep, ParamSpec, SweepConfig};
use ecosim::Params;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&d);
    d
}

fn params_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml")
}

/// Every line except runtime (wall time differs between runs and is excluded from sweeps).
fn comparable(r: &CheckReport) -> Vec<(&'static str, bool, String, f64)> {
    r.lines.iter().filter(|l| l.key != "runtime").map(|l| (l.key, l.pass, l.observed.clone(), l.margin)).collect()
}

/// `check::evaluate` on a fresh seed-42 run reproduces the committed `ecosim check runs/s42` output
/// (captured before check moved into the library), line for line apart from wall time.
#[test]
fn evaluate_matches_committed_s42_check_output() {
    let dir = tmp("s42_eval");
    // Snapshot interval doesn't affect the series; 10000 keeps the tick-10000 snapshot the check reads.
    run(Params::load_default(), 42, 20_000, 10_000, &[], &dir).unwrap();
    let series = Series::from_run_dir(&dir).unwrap();
    let report = evaluate(&series).unwrap();
    let golden = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/s42-check.txt")).unwrap();
    let golden: Vec<&str> = golden.lines().collect();
    assert_eq!(golden.len(), report.lines.len());
    for (g, l) in golden.iter().zip(&report.lines) {
        let line = format!("{} {}: {}", if l.pass { "PASS" } else { "FAIL" }, l.name, l.observed);
        if l.key == "runtime" {
            assert_eq!(g.split(':').next(), line.split(':').next());
        } else {
            assert_eq!(*g, line);
        }
    }
    assert!(report.pass());
}

/// A 2-value × 1-seed × 500-tick sweep writes 2 rows and 2 cell CSVs, and each cell equals a
/// standalone `ecosim run --set …` (series bytes) and `ecosim check` (report) with the same seed.
#[test]
fn sweep_cells_equal_standalone_runs() {
    let out = tmp("sweep_2x1");
    let cfg = SweepConfig {
        params_path: params_path(),
        fixed: vec!["season.amplitude=6".into()],
        specs: vec![ParamSpec { key: "hunter.kill_prob".into(), values: vec!["0.2".into(), "0.4".into()] }],
        seeds: vec![3],
        ticks: 500,
        jobs: 2,
    };
    let results = sweep(&cfg, &out).unwrap();
    let csv = fs::read_to_string(out.join("sweep.csv")).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 3, "{csv}");
    assert!(lines[0].starts_with("hunter.kill_prob,seed,run_length_pass,run_length_value,run_length_margin,"));
    assert!(lines[0].ends_with(",first_extinction_tick,grazer_peaks"));
    assert!(!lines[0].contains("runtime"));
    assert!(lines[1].starts_with("0.2,3,false,500,"));
    assert_eq!(fs::read_dir(out.join("cells")).unwrap().count(), 2);
    assert!(out.join("sweep.md").exists());

    let exe = env!("CARGO_BIN_EXE_ecosim");
    for (v, r) in ["0.2", "0.4"].iter().zip(&results) {
        let cell = fs::read_to_string(out.join("cells").join(format!("hunter.kill_prob={v}_s=3.csv"))).unwrap();
        assert_eq!(cell, r.csv);
        let run_dir = tmp(&format!("sweep_2x1_run_{v}"));
        let st = Command::new(exe)
            .args(["run", "--seed", "3", "--ticks", "500", "--snapshot-every", "500", "--set", "season.amplitude=6"])
            .args(["--set", &format!("hunter.kill_prob={v}"), "--out"])
            .arg(&run_dir)
            .arg("--params")
            .arg(params_path())
            .status()
            .unwrap();
        assert!(st.success());
        assert_eq!(fs::read_to_string(run_dir.join("series.csv")).unwrap(), cell, "series differs for {v}");
        assert_eq!(comparable(&check_run(&run_dir).unwrap()), comparable(&r.report));
        let meta: serde_json::Value = serde_json::from_slice(&fs::read(run_dir.join("meta.json")).unwrap()).unwrap();
        assert_eq!(meta["overrides"], serde_json::json!(["season.amplitude=6", format!("hunter.kill_prob={v}")]));
        assert_eq!(meta["params"]["hunter"]["kill_prob"].as_f64().unwrap(), v.parse::<f64>().unwrap());
    }
    // The two values really differ in effect or at least ran independently: both parse back.
    for r in &results {
        assert_eq!(parse_series(&r.csv).unwrap().len(), 501);
    }
}

/// `--baseline` margins equal `ecosim check` margins on the same seed (runtime excluded).
#[test]
fn baseline_margins_equal_check_margins() {
    let cfg = SweepConfig {
        params_path: params_path(),
        fixed: vec![],
        specs: vec![],
        seeds: vec![2],
        ticks: 10_500,
        jobs: 1,
    };
    let reports = baseline(&cfg).unwrap();
    let dir = tmp("baseline_s2");
    run(Params::load_default(), 2, 10_500, 2_500, &[], &dir).unwrap();
    let check = check_run(&dir).unwrap();
    assert!(check.get("mature_trees_10k").is_some());
    assert_eq!(comparable(&reports[0].1), comparable(&check));
}

#[test]
fn run_rejects_bad_overrides_before_running() {
    let exe = env!("CARGO_BIN_EXE_ecosim");
    for (set, needle) in [("grazer.no_such_key=1", "grazer.no_such_key"), ("tree.mature_age=soon", "tree.mature_age")] {
        let out = tmp("bad_set");
        let o = Command::new(exe)
            .args(["run", "--seed", "1", "--ticks", "10", "--set", set, "--out"])
            .arg(&out)
            .arg("--params")
            .arg(params_path())
            .output()
            .unwrap();
        assert!(!o.status.success());
        assert!(String::from_utf8_lossy(&o.stderr).contains(needle));
        assert!(!out.exists(), "run directory created despite a bad override");
    }
}
