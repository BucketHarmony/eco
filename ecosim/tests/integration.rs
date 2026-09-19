//! Integration tests from SAD 1 and the addendum. They load the crate's own `params.toml`
//! (so tuning applies) and drive the library directly; only the determinism test shells out.

use ecosim::output::{run, write_meta, write_snapshot, FORMAT_VERSION, SERIES_HEADER};
use ecosim::world::{COLS, WX, WY, WZ};
use ecosim::{Params, Sim};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&d);
    d
}

/// `ecosim run --seed 7 --ticks 3000 --snapshot-every 500` with `exe` into `out`.
fn run_binary(exe: &Path, out: &Path) {
    let params = Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml");
    let st = Command::new(exe)
        .args(["run", "--seed", "7", "--ticks", "3000", "--snapshot-every", "500", "--out"])
        .arg(out)
        .arg("--params")
        .arg(&params)
        .status()
        .expect("spawn ecosim run");
    assert!(st.success(), "ecosim run failed ({})", exe.display());
}

fn assert_same_run(exe: &Path, a: &Path, b: &Path) {
    let st = Command::new(exe).arg("diff").arg(a).arg(b).status().expect("spawn ecosim diff");
    assert!(st.success(), "ecosim diff reported differences between {} and {}", a.display(), b.display());
}

#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn determinism_two_runs_are_byte_identical() {
    let exe = Path::new(env!("CARGO_BIN_EXE_ecosim"));
    let (a, b) = (tmp("det_a"), tmp("det_b"));
    run_binary(exe, &a);
    run_binary(exe, &b);
    assert_same_run(exe, &a, &b);
}

/// The debug and release binaries produce identical run directories. The other profile's binary
/// (`target/release/` from a debug test run, and vice versa) must already be built and current;
/// when it is missing the test passes with a note, unless `ECOSIM_REQUIRE_CROSS=1` (set in CI).
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn determinism_debug_and_release_binaries_agree() {
    let exe = Path::new(env!("CARGO_BIN_EXE_ecosim"));
    let other = if cfg!(debug_assertions) { "release" } else { "debug" };
    let sibling = exe.parent().unwrap().parent().unwrap().join(other).join(exe.file_name().unwrap());
    if !sibling.exists() {
        assert!(std::env::var("ECOSIM_REQUIRE_CROSS").as_deref() != Ok("1"), "{} not built", sibling.display());
        eprintln!("skipping: {} not built", sibling.display());
        return;
    }
    let (a, b) = (tmp("cross_self"), tmp("cross_other"));
    run_binary(exe, &a);
    run_binary(&sibling, &b);
    assert_same_run(exe, &a, &b);
}

#[test]
fn drought_kills_grass_by_tick_5000() {
    let mut p = Params::load_default();
    p.climate.rain_base = 0.0;
    p.climate.rain_amp = 0.0;
    p.climate.pond_moisture = 0.0;
    let mut sim = Sim::new(p, 1);
    while sim.tick < 5000 {
        sim.step();
    }
    let g = sim.stats().grass_mean;
    assert!(g < 0.05, "grass_mean at tick 5000 = {g}");
}

#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn hunters_disabled_grazers_stay_within_30pct_of_capacity() {
    let mut p = Params::load_default();
    p.hunter.start_count = 0;
    p.hunter.immigration_floor = 0;
    let mut sim = Sim::new(p, 42);
    let mut g = vec![sim.count_grazers() as f64];
    while sim.tick < 20000 {
        sim.step();
        g.push(sim.count_grazers() as f64);
    }
    let window = &g[8000..=20000];
    let k = window.iter().sum::<f64>() / window.len() as f64;
    assert!(k > 0.0, "grazers extinct");
    for t in 8000..=(20000 - 200) {
        let ma = g[t..t + 200].iter().sum::<f64>() / 200.0;
        assert!(ma >= 0.7 * k && ma <= 1.3 * k, "200-tick MA at {t} = {ma:.1}, K = {k:.1}");
    }
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
}

#[test]
fn snapshot_round_trips_through_reader() {
    let mut sim = Sim::new(Params::load_default(), 3);
    while sim.tick < 300 {
        sim.step();
    }
    let dir = tmp("snap_rt");
    fs::create_dir_all(&dir).unwrap();
    write_meta(&sim, 3, 300, 100, &["hunter.kill_prob=0.2".to_string()], &dir).unwrap();
    write_snapshot(&sim, &dir, true).unwrap();

    let meta = read_json(&dir.join("meta.json"));
    assert_eq!(meta["format_version"], FORMAT_VERSION);
    assert_eq!(meta["dims"]["x"], WX);
    assert_eq!(meta["snapshots"].as_array().unwrap().len(), 4);
    assert_eq!(meta["overrides"], serde_json::json!(["hunter.kill_prob=0.2"]));
    assert_eq!(meta["params"]["season"]["amplitude"], sim.params.season.amplitude as f64);

    let snap = dir.join("snap_000300");
    assert_eq!(fs::read(snap.join("material.bin")).unwrap(), sim.world.material);
    assert_eq!(fs::read(snap.join("light.bin")).unwrap(), sim.world.light);
    assert_eq!(fs::read(snap.join("height.bin")).unwrap(), sim.world.height);
    assert_eq!(sim.world.material.len(), WX * WY * WZ);
    for f in ["moisture.bin", "fertility.bin"] {
        assert_eq!(fs::read(snap.join(f)).unwrap().len(), COLS);
    }
    let moist = fs::read(snap.join("moisture.bin")).unwrap();
    for (c, &m) in moist.iter().enumerate() {
        if m != 0 {
            assert!((m as f32 - sim.moisture[c]).abs() <= 0.5, "moisture col {c}");
        }
    }

    let patches = read_json(&snap.join("patches.json"));
    let patches = patches.as_array().unwrap();
    assert_eq!(patches.len(), sim.patches.len());
    for (v, p) in patches.iter().zip(&sim.patches) {
        assert_eq!(v["grass"].as_f64().unwrap() as f32, p.grass);
        assert_eq!(v["shrub"].as_f64().unwrap() as f32, p.shrub);
    }

    let ents = read_json(&snap.join("entities.json"));
    let ents = ents.as_array().unwrap();
    let count = |k: &str| ents.iter().filter(|e| e["kind"] == k).count() as u32;
    assert_eq!(count("tree"), sim.count_trees());
    assert_eq!(count("grazer"), sim.count_grazers());
    assert_eq!(count("hunter"), sim.count_hunters());
    for e in ents {
        let (x, y) = (e["x"].as_f64().unwrap(), e["y"].as_f64().unwrap());
        assert!((0.0..WX as f64).contains(&x) && (0.0..WY as f64).contains(&y));
        let col = x as usize + WX * y as usize;
        assert_eq!(e["z"].as_u64().unwrap(), sim.world.height[col] as u64 + 1);
    }
}

#[test]
fn full_run_writes_series_header_and_one_row_per_tick() {
    let dir = tmp("series");
    let s = run(Params::load_default(), 5, 250, 100, &[], &dir).unwrap();
    let csv = fs::read_to_string(dir.join("series.csv")).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], SERIES_HEADER);
    assert_eq!(lines.len(), 252);
    assert_eq!(s.rows.len(), 251);
    for t in [0, 100, 200] {
        assert!(dir.join(format!("snap_{t:06}")).join("entities.json").exists());
    }
    assert!(!dir.join("snap_000250").exists());
}

/// Forced extinction: `grazer.energy_cost=0.5` starves the grazers out on seed 1 (tick 8461), and
/// the hunters follow. The run still completes 20000 ticks: every snapshot `meta.json` lists exists
/// with full-size fields and finite patch and entity values, the series has no NaN, and
/// `ecosim stats` names `starved` as the dominant cause of the grazer extinction.
#[test]
fn forced_grazer_extinction_runs_to_the_end_and_is_attributed_to_starvation() {
    let dir = tmp("forced_extinction");
    let set = ["grazer.energy_cost=0.5".to_string()];
    let params = Params::load_with(&Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml"), &set).unwrap();
    let s = run(params, 1, 20_000, 1000, &set, &dir).unwrap();
    assert_eq!(s.rows.len(), 20_001);
    let last = s.rows.last().unwrap();
    assert_eq!((last.grazers, last.hunters), (0, 0), "both animal species extinct by the end");
    assert!(last.trees > 0);
    let csv = fs::read_to_string(dir.join("series.csv")).unwrap();
    assert!(!csv.to_lowercase().contains("nan") && !csv.contains("inf"), "non-finite value in series.csv");

    let meta = read_json(&dir.join("meta.json"));
    let snaps = meta["snapshots"].as_array().unwrap();
    assert_eq!(snaps.len(), 21);
    for t in snaps {
        let snap = dir.join(format!("snap_{:06}", t.as_u64().unwrap()));
        for (f, len) in [("material.bin", WX * WY * WZ), ("light.bin", WX * WY * WZ), ("height.bin", COLS)] {
            assert_eq!(fs::read(snap.join(f)).unwrap().len(), len, "{}/{f}", snap.display());
        }
        for f in ["moisture.bin", "fertility.bin"] {
            assert_eq!(fs::read(snap.join(f)).unwrap().len(), COLS);
        }
        let finite = |v: &Value| v.as_f64().is_some_and(f64::is_finite);
        let patches = read_json(&snap.join("patches.json"));
        for p in patches.as_array().unwrap() {
            for k in ["grass", "shrub", "detritus", "temperature"] {
                assert!(finite(&p[k]), "{}: patch {k} = {}", snap.display(), p[k]);
            }
        }
        for e in read_json(&snap.join("entities.json")).as_array().unwrap() {
            assert!(finite(&e["x"]) && finite(&e["y"]), "{}: {e}", snap.display());
            assert!(e["kind"] == "tree" || finite(&e["energy"]), "{}: {e}", snap.display());
        }
    }

    let out = Command::new(env!("CARGO_BIN_EXE_ecosim")).arg("stats").arg(&dir).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let grazer =
        text.lines().find(|l| l.starts_with("extinction: grazers at tick")).unwrap_or_else(|| panic!("{text}"));
    assert!(grazer.contains("dominant cause: starved"), "{grazer}");
    assert!(text.contains("extinction: hunters at tick"), "{text}");
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

/// Format version 2 only adds files: the v2 mini fixture (same command as the v1 one) differs from it
/// in `meta.json` and the two new `state.bin` files, and its `meta.json` only in the version and the
/// `forked_from` key.
#[test]
fn format_2_keeps_every_version_1_file_byte_for_byte() {
    let (v1, v2) = (fixture("s42-mini"), fixture("s42-mini-v2"));
    let diff = ecosim::check::diff_runs(&v1, &v2).unwrap();
    let only_v2 = |f: &str| format!("only in {}: {f}", v2.display());
    assert_eq!(
        diff,
        vec!["differs: meta.json".to_string(), only_v2("snap_000000/state.bin"), only_v2("snap_000100/state.bin")]
    );
    let (mut a, mut b) = (read_json(&v1.join("meta.json")), read_json(&v2.join("meta.json")));
    assert_eq!((a["format_version"].as_u64(), b["format_version"].as_u64()), (Some(1), Some(FORMAT_VERSION as u64)));
    assert!(b["forked_from"].is_null());
    for m in [&mut a, &mut b] {
        let o = m.as_object_mut().unwrap();
        o.remove("format_version");
        o.remove("forked_from");
    }
    assert_eq!(a, b);
}

/// Version-1 run directories still work with `check`, `stats` and `diff`; `fork` refuses them with
/// an error that names the version.
#[test]
fn version_1_runs_still_check_stat_and_diff_but_do_not_fork() {
    let exe = env!("CARGO_BIN_EXE_ecosim");
    let v1 = fixture("s42-mini");
    let out = Command::new(exe).arg("check").arg(&v1).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("FAIL run length") && text.contains("PASS"), "{text}");
    let out = Command::new(exe).arg("stats").arg(&v1).output().unwrap();
    assert!(out.status.success() && String::from_utf8(out.stdout).unwrap().contains("first extinction: none"));
    assert!(Command::new(exe).arg("diff").arg(&v1).arg(&v1).status().unwrap().success());
    let dir = tmp("fork_v1");
    let out = Command::new(exe)
        .arg("fork")
        .arg(&v1)
        .args(["--at", "100", "--ticks", "10", "--out"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("format_version 1") && err.contains("can't be forked"), "{err}");
    assert!(!dir.exists());
}
