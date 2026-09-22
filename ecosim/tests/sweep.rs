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

/// Every line except runtime (wall time differs between runs and is excluded from sweeps). An
/// invariant that does not apply to the run has no margin — it is `NaN`, which is not equal to
/// itself — so its margin is dropped and its key, verdict and observed text compared (shot G11).
fn comparable(r: &CheckReport) -> Vec<(&'static str, bool, String, Option<f64>)> {
    r.lines
        .iter()
        .filter(|l| l.key != "runtime")
        .map(|l| (l.key, l.pass, l.observed.clone(), (!l.na).then_some(l.margin)))
        .collect()
}

/// One fresh seed-42 run (20000 ticks, a snapshot every 100, as `runs/s42`: the reference strip
/// since shot 15), shared by the golden check test and the manifest test.
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
    if regenerating() {
        let text: String = report
            .lines
            .iter()
            .map(|l| {
                format!(
                    "{} {}: {}
",
                    if l.pass { "PASS" } else { "FAIL" },
                    l.name,
                    l.observed
                )
            })
            .collect();
        fs::write(data_path("s42-check.txt"), text).unwrap();
        return;
    }
    let golden = fs::read_to_string(data_path("s42-check.txt")).unwrap();
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

/// True with `ECOSIM_REGEN_MANIFEST=1` in the environment: the committed manifests and the golden
/// `check` output are rewritten from the fresh runs instead of asserted. It is for the one commit a
/// shot that changes behaviour on purpose re-cuts them in, and nothing sets it in CI (shot G4b, which
/// converted every rate to per hour or per year, was the first shot to use it; DECISIONS.md, "Units
/// calibration").
fn regenerating() -> bool {
    std::env::var_os("ECOSIM_REGEN_MANIFEST").is_some()
}

fn data_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name)
}

/// `got` equals the committed manifest `name`, or, when [`regenerating`], replaces it.
fn assert_manifest(name: &str, got: &BTreeMap<String, String>) {
    if regenerating() {
        let text: String = got
            .iter()
            .map(|(path, hash)| {
                format!(
                    "{hash}  {path}
"
                )
            })
            .collect();
        fs::write(data_path(name), text).unwrap();
        return;
    }
    assert_same_manifest(&read_manifest(name), got);
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
/// regenerates the manifest (`ECOSIM_REGEN_MANIFEST=1`) and says so in DECISIONS.md. Shot G4b did,
/// for every manifest at once: converting a rate from per tick to per hour changes the numbers a run
/// produces, so none of the pre-conversion manifests can be reproduced.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn fresh_s42_matches_committed_manifest() {
    let got = hash_run(fresh_s42(), |_, b| b);
    // 11 files per snapshot: the water tier added water.bin and soil_water.bin (shot G4, which also
    // made every run format_version 4 and so gave it the four `world/` files) and the nutrient tier
    // added npk.bin (shot G5, which did not move the version because nothing has to read it);
    // events.csv since version 3.
    assert_eq!(got.len(), 2 + 201 * 11 + 4);
    assert!(got.contains_key(EVENTS_FILE));
    assert_eq!(got.keys().filter(|k| k.ends_with("/state.bin")).count(), 201);
    assert_manifest("s42-manifest.sha256", &got);
}

/// Shot 14b regenerated the manifest for events.csv only: every other line of the shot-14 manifest
/// (kept as `s42-64-manifest.sha256` when shot 15 moved seed 42 to the strip) is the one it held
/// before (`s42-manifest-preshot14a.sha256`, which 14a left equal to it).
#[test]
fn manifest_regeneration_for_the_event_log_changed_no_existing_line() {
    let want = read_manifest("s42-64-manifest.sha256");
    assert_eq!(want.len(), 2 + 201 * 8);
    assert_same_manifest(&read_manifest("s42-manifest-preshot14a.sha256"), &without_events(want));
}

/// The pre-conversion identity chain, kept as history (shot G4b). Each of these manifests holds the
/// bytes of a run made before the units conversion, and the four tests below used to reproduce one of
/// them with the feature it predates switched off, which made each an identity with code that no
/// longer exists. None of them can be reproduced now: a rate that was charged per tick and is now
/// charged per hour lands a different amount in the same update, and the moisture a plant draws is
/// millimetres of soil water rather than steps of an index. So the chain is retired rather than
/// regenerated in place, and this test is what the retirement leaves: each `-g4b` cut covers the same
/// files as the manifest it replaces, the world the seed makes at tick 0 is unchanged, and the run has
/// moved by tick 20000. Since shot G4c the tick-0 claim is about the **terrain** only: `material.bin`
/// and `height.bin` are still the world a seed makes, and `light.bin` has moved with the canopy's
/// optics (see the comment in the body). The `-g4b` file names are historical and keep the name of
/// the shot that cut them, not of every shot that has regenerated them since.
#[test]
fn the_pre_conversion_manifests_are_kept_as_history() {
    for (before, g4b) in [
        ("s42-manifest-preG4.sha256", "s42-manifest-g4b-water-off.sha256"),
        ("s42-manifest-preshot11.sha256", "s42-manifest-g4b-heredity-off.sha256"),
        ("s42-manifest-preshot10.sha256", "s42-manifest-g4b-crowding-off.sha256"),
        ("s42-manifest-prefire.sha256", "s42-manifest-g4b-fire-off.sha256"),
        ("s42-64-manifest.sha256", "s42-manifest-g4b-64.sha256"),
    ] {
        let (a, b) = (read_manifest(before), read_manifest(g4b));
        assert!(a.keys().all(|k| b.contains_key(k)), "{g4b} is missing files of {before}");
        // The three manifests from before the event log have no `events.csv` line; the cuts do.
        let extra: Vec<&String> = b.keys().filter(|k| !a.contains_key(*k)).collect();
        assert!(extra.iter().all(|k| *k == EVENTS_FILE), "{g4b} adds {extra:?} to {before}");
        for f in ["material.bin", "height.bin"] {
            let k = format!("snap_000000/{f}");
            assert_eq!(a[&k], b[&k], "{k}: the world at tick 0 moved between {before} and {g4b}");
        }
        // `snap_000000/light.bin` used to be in that list. Shot G4c took it out, because the
        // quantity changed rule: the historical manifests hash `255 - 100 x canopy layers` and the
        // cuts now hash `255 x exp(-1.0 x layers)`, so the twelve young trees a seed plants at tick
        // 0 shade their columns to 94 where they used to shade them to 155. The terrain a seed makes
        // is still unchanged, which is what this assertion was for; that light moved is asserted
        // rather than dropped, and `integration.rs`'s `light_is_the_v1_file_re_extincted` is where
        // the shape of the move is pinned byte for byte.
        let k = "snap_000000/light.bin".to_string();
        assert_ne!(a[&k], b[&k], "{k}: shot G4c re-extincted the canopy, so this must differ");
        assert_ne!(a["series.csv"], b["series.csv"], "{before} and {g4b} are the same series");
        let k = "snap_020000/patches.json".to_string();
        assert_ne!(a[&k], b[&k], "{k}: {before} and {g4b} end the same");
    }
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
    assert_eq!(extinctions(&from_events, true), extinctions(&rows, true));
    assert_eq!(unlit_burnout(&events), None);
    // Every tree cause but `waterlog`: the strip drains, so no column on it stays above field
    // capacity for the `hydro.waterlog_ticks` a drowning needs, and no seed-42 tree ever drowns
    // (`sweeps/shotG5/FINDINGS.md` measures `waterlogged_frac` at 0 on every reference run). The
    // cause is exercised in `integration.rs`'s `forced_tree_extinction_by_waterlogging`.
    for c in TREE_CAUSES.iter().filter(|c| **c != "waterlog") {
        assert!(events.iter().any(|e| e.kind == EventKind::TreeDeath && e.cause == *c), "tree {c}");
    }
}

/// The square world’s manifest: the default params on the 64×64×32 world with patch 8, rain gradient
/// 0 and slope bias 0 (`common::SQUARE`), the world every fixture was made on. Until shot G4b this
/// hashed to the shot-14 manifest, which was the claim that the strip’s dimensions, gradient and slope
/// changed nothing on the 64 world; the units conversion ended that
/// (`the_pre_conversion_manifests_are_kept_as_history`), and what is left is the byte pin for this
/// world, shared with `old_hunting_economics_via_set_reproduce_the_square_manifest`.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn square_world_reproduces_its_manifest() {
    let dir = tmp("s42_square");
    let set = common::square_set(&["world.depth=64", "world.height=32", "world.patch=8", "hydro.enabled=false"]);
    // The pre-G5 soil is set on the loaded params rather than through `--set`, so the `overrides`
    // this run records stay the ones the manifest was cut with.
    let mut p = Params::load_with(&params_path(), &set).unwrap();
    common::pre_g5(&mut p);
    run(p, 42, 20_000, 100, &set, &dir).unwrap();
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).unwrap()).unwrap();
    assert_eq!(meta["dims"], serde_json::json!({"x": 64, "y": 64, "z": 32, "patch": 8}));
    assert_manifest("s42-manifest-g4b-64.sha256", &hash_run(&dir, common::without_npk_and_water));
}

/// Identity case for food-limited hunters (shots 14a and 14a-rev): the pre-shot hunting economics set
/// explicitly through `--set` (hunter crowding 0.001, kill_energy 40, hunt_cost 0, handling_ticks 0)
/// reproduce the square world’s manifest byte for byte, which is the standing claim that they are what
/// the defaults still do. `hunt_cost` 0 charges nothing on a kill and leaves a miss at `fail_cost`
/// alone; handling 0 never enters Handling and writes the version-3 `state.bin`.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn old_hunting_economics_via_set_reproduce_the_square_manifest() {
    let dir = tmp("s42_old_hunting");
    let set = common::square_set(&[
        "disease.hunter_rate=0.001",
        "hunter.kill_energy=40",
        "hunter.hunt_cost=0",
        "hunter.handling_ticks=0",
        "hydro.enabled=false",
    ]);
    let mut p = Params::load_with(&params_path(), &set).unwrap();
    common::pre_g5(&mut p);
    run(p, 42, 20_000, 100, &set, &dir).unwrap();
    let got = hash_run(&dir, common::without_npk_and_water);
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_manifest("s42-manifest-g4b-64.sha256", &got);
}

/// Default params on the square world with shot 11 switched off: mutation 0 (the immigration floors
/// already default to 0).
fn pre_shot_11() -> Params {
    let mut p = pre_g4();
    p.heredity.mutation = 0.0;
    p
}

/// Default params on the square world with the water tier off: the pre-G4 moisture update. Since
/// shot G5 it also puts the soil back to the pre-G5 one (`common::pre_g5`), because every manifest
/// it feeds was cut from a binary that had no nutrient pools and a litter decay rate ten times too
/// fast, and a run that keeps either cannot reproduce those bytes.
fn pre_g4() -> Params {
    let mut p = common::square();
    p.hydro.enabled = false;
    common::pre_g5(&mut p);
    p
}

/// Heredity at rate 0: at mutation 0 and floors 0, seed 42 hashes to its own manifest once the trait
/// columns, the `entities.json` trait fields and the `state.bin` traits section are cut
/// (`common::without_traits`, which asserts every cut value is the species default). The cut is the
/// standing part of the claim — at mutation 0 heredity writes nothing but defaults — and until shot
/// G4b the manifest was the one from before shot 11, which made it an identity with the code that
/// predated the feature (`the_pre_conversion_manifests_are_kept_as_history`).
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn heredity_off_cuts_to_its_manifest() {
    let dir = tmp("s42_heredity_off");
    run(pre_shot_11(), 42, 20_000, 100, &[], &dir).unwrap();
    let got = hash_run(&dir, |f, b| common::without_traits(f, common::without_npk_and_water(f, b)));
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_manifest("s42-manifest-g4b-heredity-off.sha256", &got);
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

/// Crowding mortality at rate 0: with both crowding rates at 0 and the refractory at the old cooldown,
/// seed 42 hashes to its own manifest once shot 11's trait additions are cut. Shot 10 added no columns
/// (`crowded` already existed), so at rate 0 crowding draws nothing and writes nothing; the manifest
/// was the one from before shot 10 until shot G4b re-cut it
/// (`the_pre_conversion_manifests_are_kept_as_history`).
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn crowding_off_cuts_to_its_manifest() {
    let dir = tmp("s42_crowding_off");
    run(pre_shot_10(), 42, 20_000, 100, &[], &dir).unwrap();
    let got = hash_run(&dir, |f, b| common::without_traits(f, common::without_npk_and_water(f, b)));
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_manifest("s42-manifest-g4b-crowding-off.sha256", &got);
}

/// **Tree** crowding mortality at rate 0 (shot S11), which the test above is not about: its
/// `pre_shot_10` turns off the two *animal* `disease` rates and leaves `tree.crowding_mortality` at
/// the file's 0.02. This one turns the tree term off and nothing else.
///
/// The manifest was cut from the binary of the commit **before** S11, and S11 replaced the trunk
/// count that decided tree crowding with a crown-overlap term. So this is the shot's rate-0
/// identity: the new term is read outside the RNG draw, never inside it, and a run with the rate at
/// 0 makes the same draws in the same order and writes the same bytes as the code that predates it.
///
/// It is deliberately **not** regenerable. Every other manifest in this file is rewritten by
/// `ECOSIM_REGEN_MANIFEST=1`, and this one holds bytes that a later run of this code cannot produce
/// again if it is wrong — a regeneration would quietly replace the evidence with the claim. Which is why shot G5 put this
/// run back on the pre-G5 soil (`common::pre_g5`) rather than re-cutting the file: the S11 binary
/// had no nutrient pools, so the identity only survives with the tier off.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn tree_crowding_off_cuts_to_its_manifest() {
    let dir = tmp("s42_tree_crowding_off");
    let mut p = Params::load_default();
    p.tree.crowding_mortality = 0.0;
    common::pre_g5(&mut p);
    run(p, 42, 20_000, 100, &[], &dir).unwrap();
    let got = hash_run(&dir, common::without_npk);
    assert_eq!(got.len(), 2 + 201 * 10 + 4);
    assert_same_manifest(&read_manifest("s42-manifest-S11-tree-crowding-off.sha256"), &got);
}

/// Fire at rate 0: with `fire.base_rate=0` (and shots 10 and 11 switched off), seed 42 hashes to its
/// own manifest once the trait additions and the fire columns, the `burning_ticks_left` patch field
/// and the `state.bin` fire section are cut. So fire at rate 0 draws nothing and writes nothing but
/// zeros; the manifest was the one from before fire (shot 8) until shot G4b re-cut it
/// (`the_pre_conversion_manifests_are_kept_as_history`).
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn fire_off_cuts_to_its_manifest() {
    let dir = tmp("s42_fire_off");
    let mut p = pre_shot_10();
    p.fire.base_rate = 0.0;
    run(p, 42, 20_000, 100, &[], &dir).unwrap();
    let got =
        hash_run(&dir, |f, b| common::without_fire(f, common::without_traits(f, common::without_npk_and_water(f, b))));
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_manifest("s42-manifest-g4b-fire-off.sha256", &got);
}

/// The water tier off: with `hydro.enabled=false`, seed 42 on the reference strip hashes to its own
/// manifest once the six water columns are cut (`common::without_water`, which asserts every cut value
/// is 0). So with the tier off the storm draws nothing, writes no field and adds no file, and the run
/// is still `format_version` 3. Until shot G4b the manifest was the pre-G4 one, which made this an
/// identity with the code that predated the tier (`the_pre_conversion_manifests_are_kept_as_history`);
/// the converted plant draws speak in millimetres, which the pre-G4 moisture index cannot reproduce.
/// Since shot G5 the run is also put back on the pre-G5 soil and the nutrient columns come off with
/// the water ones (`common::without_npk_and_water`), so the manifest is still the one G4b cut.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn water_off_cuts_to_its_manifest() {
    let dir = tmp("s42_water_off");
    let mut p = Params::load_default();
    p.hydro.enabled = false;
    common::pre_g5(&mut p);
    run(p, 42, 20_000, 100, &[], &dir).unwrap();
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).unwrap()).unwrap();
    assert_eq!(meta["format_version"], 3);
    assert!(!dir.join("world").exists(), "no world directory without the water tier");
    let got = hash_run(&dir, common::without_npk_and_water);
    assert_eq!(got.len(), 2 + 201 * 8);
    assert_manifest("s42-manifest-g4b-water-off.sha256", &got);
}

/// The nutrient tier off (shot G5): with `npk.enabled=false` and `climate.decay_k` put back to the
/// 6.0 the shot corrected, seed 42 on the reference strip reproduces the run the pre-G5 ecosim
/// wrote, byte for byte, once the six nutrient columns are cut (`common::without_npk`, which asserts
/// every cut value is 0). That is the claim that the whole of G5 sits behind one switch and one
/// number: no other draw, order or rounding in the tick moved, and a run with the tier off still
/// writes no `npk.bin` and the version-5 `state.bin`.
///
/// `s42-manifest-preG5.sha256` was cut from a run of the commit before this shot, not regenerated
/// from this one, so `assert_same_manifest` is called here rather than `assert_manifest`:
/// `ECOSIM_REGEN_MANIFEST=1` must never overwrite the thing this test compares against.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn npk_off_cuts_to_the_pre_g5_manifest() {
    let dir = tmp("s42_npk_off");
    let mut p = Params::load_default();
    p.npk.enabled = false;
    p.climate.decay_k = 6.0;
    run(p, 42, 20_000, 100, &[], &dir).unwrap();
    assert!(!dir.join("snap_000000").join("npk.bin").exists(), "no nutrient field with the tier off");
    let got = hash_run(&dir, common::without_npk);
    assert_same_manifest(&read_manifest("s42-manifest-preG5.sha256"), &got);
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

/// `--baseline` margins equal `ecosim check` margins on the same seed (runtime excluded), on the
/// small world (`common::SMALL`) to keep the coverage run short. It reads a margin table, so it
/// needs a world whose invariants pass: shot G4b moved it off the flat `common::SQUARE`, where the
/// corrected rainfall leaves too little water for a tree once its own column's grass has drunk
/// (`sweeps/shotG4b/FINDINGS.md`), and a failing row carries a `!` marker the format assertion
/// below would trip over.
#[test]
fn baseline_margins_equal_check_margins() {
    let cfg = SweepConfig {
        params_path: params_path(),
        fixed: common::small_set(&[]),
        specs: vec![],
        seeds: vec![2],
        ticks: 10_500,
        jobs: 1,
    };
    let reports = baseline(&cfg).unwrap();
    let dir = tmp("baseline_s2");
    let small = ecosim::Params::load_with(&params_path(), &common::small_set(&[])).unwrap();
    run(small, 2, 10_500, 2_500, &common::small_set(&[]), &dir).unwrap();
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
    for (set, needle) in
        [("grazer.no_such_key=1", "grazer.no_such_key"), ("tree.mature_age_years=soon", "tree.mature_age_years")]
    {
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

/// Shot S3's identity claim, kept the way shot 14b kept the event log's: the manifest it
/// regenerated differs from the one before it (`s42-manifest-preS3.sha256`, a copy of
/// `s42-manifest.sha256` taken before the re-cut) on the 201 `entities.json` lines and on nothing
/// else. The crown a tree has is published, not consumed — no field moves, no RNG draw is made and
/// the series does not shift — and this is that claim as bytes rather than as prose.
///
/// The second half of the pair is `s42-manifest-preS11.sha256` since shot S11, not the live
/// manifest. S11 made the mortality rule read those crowns, so the live manifest has moved on every
/// line and the S3 claim is no longer a statement about it. `preS11` is a byte copy of the manifest
/// S3 cut, so the claim is the same two files it always compared — the historical pair is pinned
/// rather than retired, which is what `the_pre_conversion_manifests_are_kept_as_history` does for
/// the older chain.
#[test]
fn the_crown_fields_changed_only_entities_json() {
    let (before, after) = (read_manifest("s42-manifest-preS3.sha256"), read_manifest("s42-manifest-preS11.sha256"));
    let keys: Vec<&String> = before.keys().collect();
    assert_eq!(keys, after.keys().collect::<Vec<&String>>(), "the file set moved");
    let differing: Vec<&String> = keys.iter().copied().filter(|k| before[*k] != after[*k]).collect();
    assert!(differing.iter().all(|k| k.ends_with("/entities.json")), "{differing:?}");
    assert_eq!(differing.len(), 201, "one per snapshot");
}
