//! Sweep harness and shared-check tests: a sweep cell must equal `ecosim run --set …` + `ecosim check`.

use ecosim::check::diff_runs;
use ecosim::check::{check_run, evaluate, extinctions, parse_series, read_series, read_series_for_stats};
use ecosim::check::{CheckReport, Series};
use ecosim::events::{deaths_per_tick, parse_events, unlit_burnout, EventKind, EVENTS_FILE, TREE_CAUSES};
use ecosim::output::run;
use ecosim::sweep::{baseline, margin_table, sweep, ParamSpec, SweepConfig};
use ecosim::Params;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

mod common;

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

/// A committed manifest (`tests/data/<name>`): path → sha256.
fn read_manifest(name: &str) -> BTreeMap<String, String> {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name)).unwrap();
    manifest
        .lines()
        .map(|l| {
            let (hash, path) = l.split_once("  ").expect("manifest line is `sha256  path`");
            (path.to_string(), hash.to_string())
        })
        .collect()
}

/// The manifest of a run directory (series.csv, events.csv and every snapshot file), each file
/// passed through `f` before hashing.
fn hash_run(dir: &Path, f: impl Fn(&Path, Vec<u8>) -> Vec<u8>) -> BTreeMap<String, String> {
    let mut got = BTreeMap::new();
    for name in ["series.csv", EVENTS_FILE] {
        let p = dir.join(name);
        got.insert(name.to_string(), sha256_hex(&f(&p, fs::read(&p).unwrap())));
    }
    for snap in fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()) {
        for p in fs::read_dir(&snap).unwrap().map(|e| e.unwrap().path()) {
            let rel =
                format!("{}/{}", snap.file_name().unwrap().to_str().unwrap(), p.file_name().unwrap().to_str().unwrap());
            got.insert(rel, sha256_hex(&f(&p, fs::read(&p).unwrap())));
        }
    }
    got
}

/// A manifest from before the event log (shot 14b) has no `events.csv` line: drop the fresh one.
fn without_events(mut got: BTreeMap<String, String>) -> BTreeMap<String, String> {
    assert!(got.remove(EVENTS_FILE).is_some());
    got
}

fn assert_same_manifest(want: &BTreeMap<String, String>, got: &BTreeMap<String, String>) {
    let differing: Vec<&String> = want.keys().chain(got.keys()).filter(|k| want.get(*k) != got.get(*k)).collect();
    assert!(
        differing.is_empty(),
        "{} manifest entries differ, first: {:?}",
        differing.len(),
        &differing[..differing.len().min(5)]
    );
}

/// A fresh seed-42 run hashes to `tests/data/s42-manifest.sha256` (series.csv plus every snapshot
/// file): the guard against unintended behaviour changes. A shot that changes behaviour on purpose
/// regenerates the manifest and says so in DECISIONS.md.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn fresh_s42_matches_committed_manifest() {
    let want = read_manifest("s42-manifest.sha256");
    let got = hash_run(fresh_s42(), |_, b| b);
    // 8 files per snapshot since format_version 2 added state.bin, and events.csv since version 3.
    assert_eq!(got.len(), 2 + 201 * 8);
    assert!(want.contains_key(EVENTS_FILE));
    assert_eq!(want.keys().filter(|k| k.ends_with("/state.bin")).count(), 201);
    assert_same_manifest(&want, &got);
}

/// Shot 14b regenerated the manifest for events.csv only: every other line is the one the manifest
/// held before (`s42-manifest-preshot14a.sha256`, which 14a left equal to it).
#[test]
fn manifest_regeneration_for_the_event_log_changed_no_existing_line() {
    let want = read_manifest("s42-manifest.sha256");
    assert_eq!(want.len(), 2 + 201 * 8);
    assert_same_manifest(&read_manifest("s42-manifest-preshot14a.sha256"), &without_events(want));
}

/// The seed-42 event log: its death rows equal the series death columns on every tick, so `stats`
/// reports the same extinctions from either (the old method); every burnout was lit first; every tree
/// death cause occurs; and the file is well under the 20 MB that would call for compression.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn s42_event_log_matches_the_series_and_stays_small() {
    let dir = fresh_s42();
    let text = fs::read_to_string(dir.join(EVENTS_FILE)).unwrap();
    assert!(text.len() < 20_000_000, "events.csv is {} bytes", text.len());
    let events = parse_events(&text).unwrap();
    let rows = read_series(dir).unwrap();
    let counted = deaths_per_tick(&events, rows.len()).unwrap();
    for (r, d) in rows.iter().zip(&counted) {
        assert_eq!(r.deaths, *d, "tick {}", r.tick);
    }
    let from_events = read_series_for_stats(dir).unwrap();
    assert_eq!(from_events, rows);
    assert_eq!(extinctions(&from_events), extinctions(&rows));
    assert_eq!(unlit_burnout(&events), None);
    for c in TREE_CAUSES {
        assert!(events.iter().any(|e| e.kind == EventKind::TreeDeath && e.cause == c), "tree {c}");
    }
}

/// Identity case for food-limited hunters (shots 14a and 14a-rev): the pre-shot hunting economics
/// set explicitly through `--set` (hunter crowding 0.001, kill_energy 40, hunt_cost 0, handling_ticks
/// 0) reproduce the shot-14 manifest byte for byte, and the manifest as shot 14b left it (events.csv
/// included). `hunt_cost` 0 charges nothing on a kill and leaves a miss at `fail_cost` alone;
/// handling 0 never enters Handling and writes the version-3 `state.bin`.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn old_hunting_economics_via_set_reproduce_the_shot_14_manifest() {
    let dir = tmp("s42_old_hunting");
    let set: Vec<String> =
        ["disease.hunter_rate=0.001", "hunter.kill_energy=40", "hunter.hunt_cost=0", "hunter.handling_ticks=0"]
            .map(String::from)
            .to_vec();
    run(Params::load_with(&params_path(), &set).unwrap(), 42, 20_000, 100, &set, &dir).unwrap();
    let got = hash_run(&dir, |_, b| b);
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_same_manifest(&read_manifest("s42-manifest.sha256"), &got);
    assert_same_manifest(&read_manifest("s42-manifest-preshot14a.sha256"), &without_events(got));
}

/// Default params with shot 11 switched off: mutation 0 (the immigration floors already default to 0).
fn pre_shot_11() -> Params {
    let mut p = Params::load_default();
    p.heredity.mutation = 0.0;
    p
}

/// Rate-0 identity for heredity and open boundaries: at mutation 0 and floors 0, seed 42 hashes to
/// the manifest as it stood before shot 11 (`s42-manifest-preshot11.sha256`) once the trait columns,
/// the `entities.json` trait fields and the `state.bin` traits section are cut
/// (`common::without_traits`, which asserts every cut value is the species default). So at mutation
/// 0 heredity draws nothing and changes nothing else in any of the 201 snapshots.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn heredity_off_reproduces_the_pre_shot_11_manifest() {
    let dir = tmp("s42_heredity_off");
    run(pre_shot_11(), 42, 20_000, 100, &[], &dir).unwrap();
    let got = hash_run(&dir, common::without_traits);
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_same_manifest(&read_manifest("s42-manifest-preshot11.sha256"), &without_events(got));
}

/// Default params with shots 10 and 11 switched off: crowding rates 0, the hunter refractory at the
/// old fixed cooldown (5000), which it replaced with the same meaning, and mutation 0.
fn pre_shot_10() -> Params {
    let mut p = pre_shot_11();
    p.disease.grazer_rate = 0.0;
    p.disease.hunter_rate = 0.0;
    p.hunter.refractory = 5000;
    p
}

/// Rate-0 identity for crowding mortality: with both crowding rates at 0 and the refractory at the
/// old cooldown, seed 42 hashes to the manifest as it stood before shot 10
/// (`s42-manifest-preshot10.sha256`) byte for byte once shot 11's trait additions are cut. Shot 10
/// added no columns (`crowded` already existed): at rate 0 crowding draws nothing and writes nothing.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn crowding_off_reproduces_the_pre_shot_10_manifest() {
    let dir = tmp("s42_crowding_off");
    run(pre_shot_10(), 42, 20_000, 100, &[], &dir).unwrap();
    let got = hash_run(&dir, common::without_traits);
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_same_manifest(&read_manifest("s42-manifest-preshot10.sha256"), &without_events(got));
}

/// Rate-0 identity for fire: with `fire.base_rate=0` (and shots 10 and 11 switched off), seed 42
/// hashes to the manifest as it stood before fire (`s42-manifest-prefire.sha256`, shot 8) once the
/// trait additions and the fire columns, the `burning_ticks_left` patch field and the `state.bin` fire section are cut. So fire at
/// rate 0 draws nothing and writes nothing: every other byte of all 201 snapshots is unchanged.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn fire_off_reproduces_the_pre_fire_manifest() {
    let dir = tmp("s42_fire_off");
    let mut p = pre_shot_10();
    p.fire.base_rate = 0.0;
    run(p, 42, 20_000, 100, &[], &dir).unwrap();
    let got = hash_run(&dir, |f, b| common::without_fire(f, common::without_traits(f, b)));
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_same_manifest(&read_manifest("s42-manifest-prefire.sha256"), &without_events(got));
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
         hunter_extinction_tick,hunter_immigrants,pp_lag,pp_corr,pp_undefined,pp_period,pp_pass"
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

/// Fork at T with no overrides, run to 20000, is byte-identical from T on to the uninterrupted run,
/// for T in {100, 5000, 12300, 19900} on seeds 1 and 42. The fork copies the parent's rows and
/// snapshots before T, so the whole run directory matches except `meta.json`, which gains
/// `forked_from`. The forks go through the CLI.
#[test]
#[cfg_attr(coverage, ignore = "full-length runs; run in `cargo test`, not under llvm-cov")]
fn fork_matches_the_uninterrupted_run_from_the_fork_tick_on() {
    let exe = env!("CARGO_BIN_EXE_ecosim");
    let fork_all = |seed: u64, parent: &Path| {
        std::thread::scope(|s| {
            for at in [100u32, 5000, 12300, 19900] {
                s.spawn(move || {
                    let out = tmp(&format!("fork_s{seed}_at{at}"));
                    let st = Command::new(exe)
                        .arg("fork")
                        .arg(parent)
                        .args(["--at", &at.to_string(), "--ticks", &(20_000 - at).to_string(), "--out"])
                        .arg(&out)
                        .status()
                        .unwrap();
                    assert!(st.success(), "fork of seed {seed} at {at} failed");
                    let diff = diff_runs(parent, &out).unwrap();
                    assert_eq!(diff, vec!["differs: meta.json".to_string()], "seed {seed}, fork at {at}");
                    let meta: serde_json::Value =
                        serde_json::from_slice(&fs::read(out.join("meta.json")).unwrap()).unwrap();
                    assert_eq!(meta["forked_from"]["tick"], at);
                    assert_eq!(meta["overrides"], serde_json::json!([]));
                    fs::remove_dir_all(&out).unwrap();
                });
            }
        });
    };
    std::thread::scope(|s| {
        s.spawn(|| {
            let s1 = tmp("s1_fork_parent");
            run(Params::load_default(), 1, 20_000, 100, &[], &s1).unwrap();
            fork_all(1, &s1);
        });
        s.spawn(|| fork_all(42, fresh_s42()));
    });
}
