//! Sweep harness and shared-check tests: a sweep cell must equal `ecosim run --set …` + `ecosim check`.

use ecosim::check::{check_run, evaluate, parse_series, CheckReport, Series};
use ecosim::output::run;
use ecosim::sweep::{baseline, margin_table, sweep, ParamSpec, SweepConfig};
use ecosim::Params;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

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

/// One fresh seed-42 run (20000 ticks, a snapshot every 100, as `runs/s42`), shared by the golden
/// check test and the manifest test.
fn fresh_s42() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tmp("s42_fresh");
        run(Params::load_default(), 42, 20_000, 100, &[], &dir).unwrap();
        dir
    })
}

/// `check::evaluate` on a fresh seed-42 run reproduces the committed `ecosim check runs/s42` output
/// (regenerated with the dynamics fixes, DECISIONS.md), line for line apart from wall time. The runtime
/// line is compared by name only: its verdict depends on the build profile and the machine.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn evaluate_matches_committed_s42_check_output() {
    let series = Series::from_run_dir(fresh_s42()).unwrap();
    let report = evaluate(&series).unwrap();
    let golden = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/s42-check.txt")).unwrap();
    let golden: Vec<&str> = golden.lines().collect();
    assert_eq!(golden.len(), report.lines.len());
    for (g, l) in golden.iter().zip(&report.lines) {
        if l.key == "runtime" {
            let name = g.split_once(' ').and_then(|(_, rest)| rest.split(':').next());
            assert_eq!(name, Some(l.name));
        } else {
            assert_eq!(*g, format!("{} {}: {}", if l.pass { "PASS" } else { "FAIL" }, l.name, l.observed));
        }
    }
    assert!(report.lines.iter().filter(|l| l.key != "runtime").all(|l| l.pass));
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A fresh seed-42 run hashes to `tests/data/s42-manifest.sha256` (series.csv plus every snapshot
/// file): the guard against unintended behaviour changes. A shot that changes behaviour on purpose
/// regenerates the manifest and says so in DECISIONS.md.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn fresh_s42_matches_committed_manifest() {
    let dir = fresh_s42();
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/s42-manifest.sha256")).unwrap();
    let want: BTreeMap<String, String> = manifest
        .lines()
        .map(|l| {
            let (hash, path) = l.split_once("  ").expect("manifest line is `sha256  path`");
            (path.to_string(), hash.to_string())
        })
        .collect();
    let mut got = BTreeMap::new();
    got.insert("series.csv".to_string(), sha256_hex(&fs::read(dir.join("series.csv")).unwrap()));
    for snap in fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()) {
        for f in fs::read_dir(&snap).unwrap().map(|e| e.unwrap().path()) {
            let rel =
                format!("{}/{}", snap.file_name().unwrap().to_str().unwrap(), f.file_name().unwrap().to_str().unwrap());
            got.insert(rel, sha256_hex(&fs::read(&f).unwrap()));
        }
    }
    assert_eq!(got.len(), 1 + 201 * 7);
    let differing: Vec<&String> = want.keys().chain(got.keys()).filter(|k| want.get(*k) != got.get(*k)).collect();
    assert!(
        differing.is_empty(),
        "{} manifest entries differ, first: {:?}",
        differing.len(),
        &differing[..differing.len().min(5)]
    );
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
    assert!(lines[0].ends_with(
        ",first_extinction_tick,first_extinction_species,first_extinction_dominant_cause,grazer_peaks,\
         hunter_extinction_tick,hunter_immigrants"
    ));
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
    let table = margin_table(&reports);
    assert!(table.starts_with("invariant") && table.lines().next().unwrap().contains("s2"), "{table}");
    let row = table.lines().find(|l| l.starts_with("mature_trees_10k")).expect("mature_trees_10k row");
    assert_eq!(row.split_whitespace().count(), 3, "{row}");
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

/// A cell whose grazers starve out carries the species and cause in `sweep.csv`, and `sweep.md`
/// lists it under "Extinctions by cause", counted apart from the invariant failures.
#[test]
fn sweep_reports_extinctions_by_cause() {
    let out = tmp("sweep_extinction");
    let cfg = SweepConfig {
        params_path: params_path(),
        fixed: vec![],
        specs: vec![ParamSpec { key: "grazer.energy_cost".into(), values: vec!["0.1".into(), "5.0".into()] }],
        seeds: vec![4],
        ticks: 200,
        jobs: 2,
    };
    let results = sweep(&cfg, &out).unwrap();
    assert_eq!(results[0].first_extinction, None);
    let starved = &results[1];
    assert_eq!(starved.first_extinction_species.as_deref(), Some("grazers"));
    assert_eq!(starved.first_extinction_cause, Some("starved"));
    let csv = fs::read_to_string(out.join("sweep.csv")).unwrap();
    let ext = starved.first_extinction.unwrap();
    assert!(csv.lines().nth(2).unwrap().contains(&format!(",{ext},grazers,starved,")), "{csv}");
    assert!(csv.lines().nth(1).unwrap().contains(",,,,"), "no extinction leaves the three columns empty: {csv}");
    let md = fs::read_to_string(out.join("sweep.md")).unwrap();
    let section = md.split("## Extinctions by cause").nth(1).expect("extinctions section");
    assert!(section.contains("- cells failing an invariant: 2 of 2"), "{section}");
    assert!(section.contains("- cells in which a species reaches 0 at any tick: 1 of 2"), "{section}");
    assert!(section.contains("- failing cells with no extinction: 1; extinction cells passing every invariant: 0"));
    assert!(section.contains(&format!("| grazers | `starved` | 1 | grazer.energy_cost=5.0_s=4 @{ext} |")), "{section}");
}
