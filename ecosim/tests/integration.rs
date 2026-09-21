//! Integration tests from SAD 1 and the addendum. They load the crate's own `params.toml`
//! (so tuning applies) and drive the library directly; only the determinism test shells out. Tests
//! whose facts were measured on the 64×64 world run there (`common::SQUARE`); the rest run on the
//! default reference strip.

use ecosim::output::{
    run, run_with as run_opts, write_meta, write_snapshot, RunOptions, BUNDLE_FORMAT_VERSION, FORMAT_VERSION,
    SERIES_HEADER,
};
use ecosim::{Params, Sim};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

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
    let mut p = common::square();
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
    let mut p = common::square();
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

/// On the reference strip: `meta.json` carries the four dimensions and every `.bin` file has the
/// size they imply.
#[test]
fn snapshot_round_trips_through_reader() {
    let mut sim = Sim::new(Params::load_default(), 3);
    while sim.tick < 300 {
        sim.step();
    }
    let dir = tmp("snap_rt");
    fs::create_dir_all(&dir).unwrap();
    write_meta(&sim, 3, 300, 100, &["hunter.kill_prob=0.2".to_string()], &dir).unwrap();
    write_snapshot(&sim, &dir, true, true).unwrap();

    let meta = read_json(&dir.join("meta.json"));
    assert_eq!(meta["format_version"], FORMAT_VERSION);
    let d = sim.world.dims;
    assert_eq!(meta["dims"], serde_json::json!({"x": 256, "y": 64, "z": 32, "patch": 8}));
    let (wx, wy, cols) = (d.wx, d.wy, d.cols());
    assert_eq!(meta["snapshots"].as_array().unwrap().len(), 4);
    assert_eq!(meta["overrides"], serde_json::json!(["hunter.kill_prob=0.2"]));
    assert_eq!(meta["params"]["season"]["amplitude"], sim.params.season.amplitude as f64);
    // A run with animals writes neither the `animals` key nor the `[animals]` params section.
    assert_eq!(meta.get("animals"), None, "{meta}");
    assert_eq!(meta["params"].get("animals"), None, "{meta}");

    let snap = dir.join("snap_000300");
    assert_eq!(fs::read(snap.join("material.bin")).unwrap(), sim.world.material);
    assert_eq!(fs::read(snap.join("light.bin")).unwrap(), sim.world.light);
    assert_eq!(fs::read(snap.join("height.bin")).unwrap(), sim.world.height);
    assert_eq!(sim.world.material.len(), d.voxels());
    for f in ["moisture.bin", "fertility.bin"] {
        assert_eq!(fs::read(snap.join(f)).unwrap().len(), cols);
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
        assert!((0.0..wx as f64).contains(&x) && (0.0..wy as f64).contains(&y));
        let col = x as usize + wx * y as usize;
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

/// The crate's `params.toml` on the small world (`common::SMALL`) with `set` applied.
fn small_with(set: &[String]) -> Params {
    let set: Vec<&str> = set.iter().map(String::as_str).collect();
    Params::load_with(&Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml"), &common::small_set(&set)).unwrap()
}

/// Seed 1 for 20000 ticks on the small world with `set` applied, a snapshot every 1000, into `dir`.
/// Returns the last row.
fn run_with(dir: &Path, set: &[String]) -> ecosim::StatsRow {
    let set: Vec<&str> = set.iter().map(String::as_str).collect();
    run_on(dir, &common::small_set(&set))
}

/// Seed 1 for 20000 ticks with `set` applied, a snapshot every 1000, into `dir`. Returns the last row.
fn run_on(dir: &Path, set: &[String]) -> ecosim::StatsRow {
    let params = Params::load_with(&Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml"), set).unwrap();
    let s = run(params, 1, 20_000, 1000, set, dir).unwrap();
    assert_eq!(s.rows.len(), 20_001);
    *s.rows.last().unwrap()
}

/// The run in `dir` completed: the series has no NaN, and every snapshot `meta.json` lists exists
/// with full-size fields and finite patch and entity values. Returns `ecosim stats` output.
fn assert_valid_run(dir: &Path) -> String {
    let csv = fs::read_to_string(dir.join("series.csv")).unwrap();
    assert!(!csv.to_lowercase().contains("nan") && !csv.contains("inf"), "non-finite value in series.csv");

    let meta = read_json(&dir.join("meta.json"));
    let snaps = meta["snapshots"].as_array().unwrap();
    assert_eq!(snaps.len(), 21);
    let dim = |k: &str| meta["dims"][k].as_u64().unwrap() as usize;
    let (cols, voxels) = (dim("x") * dim("y"), dim("x") * dim("y") * dim("z"));
    for t in snaps {
        let snap = dir.join(format!("snap_{:06}", t.as_u64().unwrap()));
        for (f, len) in [("material.bin", voxels), ("light.bin", voxels), ("height.bin", cols)] {
            assert_eq!(fs::read(snap.join(f)).unwrap().len(), len, "{}/{f}", snap.display());
        }
        for f in ["moisture.bin", "fertility.bin"] {
            assert_eq!(fs::read(snap.join(f)).unwrap().len(), cols);
        }
        let finite = |v: &Value| v.as_f64().is_some_and(f64::is_finite);
        let patches = read_json(&snap.join("patches.json"));
        for p in patches.as_array().unwrap() {
            for k in ["grass", "shrub", "detritus", "temperature"] {
                assert!(finite(&p[k]), "{}: patch {k} = {}", snap.display(), p[k]);
            }
            assert!(p["burning_ticks_left"].is_u64(), "{}: {p}", snap.display());
        }
        for e in read_json(&snap.join("entities.json")).as_array().unwrap() {
            assert!(finite(&e["x"]) && finite(&e["y"]), "{}: {e}", snap.display());
            assert!(e["kind"] == "tree" || finite(&e["energy"]), "{}: {e}", snap.display());
        }
    }
    // `stats` reads death causes from events.csv; they equal the series columns on every tick.
    let rows = ecosim::check::read_series(dir).unwrap();
    assert_eq!(ecosim::check::read_series_for_stats(dir).unwrap(), rows, "events.csv deaths = series deaths");
    let events = ecosim::events::parse_events(&fs::read_to_string(dir.join("events.csv")).unwrap()).unwrap();
    assert_eq!(ecosim::events::unlit_burnout(&events), None);
    let out = Command::new(env!("CARGO_BIN_EXE_ecosim")).arg("stats").arg(dir).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let animals = ecosim::check::run_has_animals(dir);
    let old: Vec<String> =
        ecosim::check::extinctions(&rows, animals).iter().map(ecosim::check::extinction_line).collect();
    let new: Vec<&str> = text.lines().filter(|l| l.starts_with("extinction:")).collect();
    assert_eq!(new, old, "stats from events.csv = the series method");
    text
}

/// Forced extinction: `grazer.energy_cost=1.0` starves the grazers out on seed 1 (tick 4056, hunters
/// at 5300), on the small world (`common::SMALL`). Fire and grazer crowding are off
/// (`fire.base_rate=0`, `disease.grazer_rate=0`) so starvation is the only thing forced: crowding
/// thins grazers enough that the survivors can feed. The run still completes 20000 ticks with valid
/// snapshots, and `ecosim stats` names `starved` as the dominant cause of the grazer extinction.
#[test]
fn forced_grazer_extinction_runs_to_the_end_and_is_attributed_to_starvation() {
    let dir = tmp("forced_extinction");
    let last = run_with(
        &dir,
        &["grazer.energy_cost=1.0".to_string(), "fire.base_rate=0".to_string(), "disease.grazer_rate=0".to_string()],
    );
    assert_eq!((last.grazers, last.hunters), (0, 0), "both animal species extinct by the end");
    assert!(last.trees > 0);
    let text = assert_valid_run(&dir);
    let grazer =
        text.lines().find(|l| l.starts_with("extinction: grazers at tick")).unwrap_or_else(|| panic!("{text}"));
    assert!(grazer.contains("dominant cause: starved"), "{grazer}");
    assert!(text.contains("extinction: hunters at tick"), "{text}");
}

/// Forced extinction on the reference strip (shot 15): starvation forcing at the default dimensions,
/// rain gradient and slope. The small world's `energy_cost=1.0` leaves grazers alive on the strip,
/// which is four times the area and better watered, so this uses 3.5: grazers starve out on seed 1 at
/// tick 116, hunters at 1251. 2.0 was enough before the units conversion, and 3.0 now kills the
/// hunters first and leaves the last grazers to die of old age instead (shot G4b). The run completes
/// 20000 ticks with valid snapshots of the strip's size, `ecosim stats` names `starved` for the
/// grazers, and the last snapshot restores onto the strip.
#[test]
#[cfg_attr(
    coverage,
    ignore = "full-length run on the strip that reaches no line the unit tests miss; runs in `cargo test`"
)]
fn forced_grazer_extinction_on_the_strip_runs_to_the_end() {
    let dir = tmp("forced_extinction_strip");
    let set: Vec<String> =
        ["grazer.energy_cost=3.5", "fire.base_rate=0", "disease.grazer_rate=0"].map(String::from).to_vec();
    let last = run_on(&dir, &set);
    assert_eq!((last.grazers, last.hunters), (0, 0), "both animal species extinct by the end");
    assert!(last.trees > 0);
    let text = assert_valid_run(&dir);
    assert_eq!(read_json(&dir.join("meta.json"))["dims"], serde_json::json!({"x": 256, "y": 64, "z": 32, "patch": 8}));
    let grazer =
        text.lines().find(|l| l.starts_with("extinction: grazers at tick")).unwrap_or_else(|| panic!("{text}"));
    assert!(grazer.contains("dominant cause: starved"), "{grazer}");
    let params = Params::load_with(&Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml"), &set).unwrap();
    let sim = Sim::restore(params, &dir.join("snap_020000")).unwrap();
    assert_eq!(
        (sim.tick, sim.world.dims.cols(), sim.count_grazers()),
        (20_000, 256 * 64, 0),
        "the last snapshot restores"
    );
    assert!(
        Sim::restore(small_with(&set), &dir.join("snap_020000")).is_err(),
        "a strip snapshot is not a 64-world one"
    );
}

/// Forced extinction by fire: every patch with fuel can ignite at any temperature and fire kills any
/// animal in one tick, so both animal species burn out on seed 1 in the first few hundred ticks.
/// Ignition is scaled by the square of dryness, and the water tier keeps a good part of the soil's
/// capacity filled (shot G4), so `base_rate` is far above 1: the product is clamped to a
/// probability, and this leaves it at 1 on a fuelled patch. 8000 ignitions per patch per year is the
/// 20 of shot G4 over the 400 fire updates a year the cadence now divides it into (shot G4b).
/// Mutation is off (`heredity.mutation=0`) so fire is the only thing forced.
/// The run continues to 20000 ticks with valid snapshots, fires keep burning, and `ecosim stats`
/// names `burnt` as the dominant cause of both extinctions.
#[test]
fn forced_fire_extinction_runs_to_the_end_and_is_attributed_to_fire() {
    let dir = tmp("forced_fire_extinction");
    let set: Vec<String> = [
        "fire.base_rate=8000",
        "fire.temp_min=-50",
        "fire.temp_full=-40",
        "fire.animal_damage=100",
        "heredity.mutation=0",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let last = run_with(&dir, &set);
    assert_eq!((last.grazers, last.hunters), (0, 0), "both animal species burnt out");
    assert!(last.total_burnt > 1000, "total_burnt {}", last.total_burnt);
    let text = assert_valid_run(&dir);
    for species in ["grazers", "hunters"] {
        let line = text
            .lines()
            .find(|l| l.starts_with(&format!("extinction: {species} at tick")))
            .unwrap_or_else(|| panic!("{text}"));
        assert!(line.contains("dominant cause: burnt"), "{line}");
    }
    let snap = dir.join("snap_020000");
    let sim = Sim::restore(small_with(&set), &snap).unwrap();
    assert_eq!((sim.tick, sim.total_burnt), (20_000, last.total_burnt), "the last snapshot restores");
}

/// Forced extinction by the refractory: at `hunter.refractory=1000000` hunters (almost) never breed,
/// so they die out of old age (seed 1: tick 7986). Crowding stays at its defaults. The run continues
/// to 20000 ticks with valid snapshots, `ecosim stats` names `old_age` as the dominant cause of the
/// hunter extinction, the hunter-free grazers keep dying of crowding to the end, and the last
/// snapshot restores.
#[test]
fn forced_hunter_extinction_by_refractory_runs_to_the_end() {
    let dir = tmp("forced_refractory_extinction");
    let set = ["hunter.refractory=1000000".to_string()];
    let last = run_with(&dir, &set);
    assert_eq!(last.hunters, 0, "hunters extinct by the end");
    assert!(last.grazers > 0 && last.trees > 0);
    let text = assert_valid_run(&dir);
    let line = text.lines().find(|l| l.starts_with("extinction: hunters at tick")).unwrap_or_else(|| panic!("{text}"));
    assert!(line.contains("dominant cause: old_age"), "{line}");
    assert!(!text.contains("extinction: grazers"), "{text}");
    let series = ecosim::check::parse_series(&fs::read_to_string(dir.join("series.csv")).unwrap()).unwrap();
    let crowded = |rows: &[ecosim::StatsRow]| rows.iter().map(|r| r.deaths[0][3]).sum::<u32>();
    let (all, late) = (crowded(&series), crowded(&series[15_000..]));
    assert!(all > 1000 && late > 0, "grazer crowded deaths {all}, after tick 15000 {late}");
    let params = small_with(&set);
    let sim = Sim::restore(params, &dir.join("snap_020000")).unwrap();
    assert_eq!((sim.tick, sim.count_hunters()), (20_000, 0), "the last snapshot restores");
}

/// Forced extinction by hunting cost: at `hunter.hunt_cost=8` an attack costs more than a hunter
/// can win back at the typical success rate, so the hunters starve out on seed 1 (tick 6622). The run continues
/// to 20000 ticks with valid snapshots, `ecosim stats` names `starved`, the predator–prey signature
/// is reported as undefined with that cause, and the last snapshot restores.
#[test]
#[cfg_attr(coverage, ignore = "full-length run that reaches no line the unit tests miss; runs in `cargo test`")]
fn forced_hunter_starvation_by_hunt_cost_runs_to_the_end() {
    let dir = tmp("forced_hunt_cost_extinction");
    let set = ["hunter.hunt_cost=8".to_string()];
    let last = run_with(&dir, &set);
    assert_eq!(last.hunters, 0, "hunters extinct by the end");
    let text = assert_valid_run(&dir);
    let line = text.lines().find(|l| l.starts_with("extinction: hunters at tick")).unwrap_or_else(|| panic!("{text}"));
    assert!(line.contains("dominant cause: starved"), "{line}");
    let out = Command::new(env!("CARGO_BIN_EXE_ecosim")).args(["stats", "--signature"]).arg(&dir).output().unwrap();
    assert!(out.status.success());
    let sig = String::from_utf8(out.stdout).unwrap();
    assert!(sig.starts_with("signature: undefined, hunters extinct at tick ") && sig.contains("starved"), "{sig}");
    let params = small_with(&set);
    let sim = Sim::restore(params, &dir.join("snap_020000")).unwrap();
    assert_eq!((sim.tick, sim.count_hunters()), (20_000, 0), "the last snapshot restores");
}

/// Forced extinction by handling time: at `hunter.handling_ticks=1000000` a hunter's first kill is
/// its last, so the hunters starve out on seed 1 (tick 2561) while grazers boom. The run continues to
/// 20000 ticks with valid snapshots, the tick-1000 snapshot shows hunters in state
/// `handling`, `ecosim stats` and the signature name `starved`, and the last snapshot restores.
#[test]
#[cfg_attr(coverage, ignore = "full-length run that reaches no line the unit tests miss; runs in `cargo test`")]
fn forced_hunter_starvation_by_handling_time_runs_to_the_end() {
    let dir = tmp("forced_handling_extinction");
    let set = ["hunter.handling_ticks=1000000".to_string()];
    let last = run_with(&dir, &set);
    assert_eq!(last.hunters, 0, "hunters extinct by the end");
    assert!(last.grazers > 0);
    let text = assert_valid_run(&dir);
    let line = text.lines().find(|l| l.starts_with("extinction: hunters at tick")).unwrap_or_else(|| panic!("{text}"));
    assert!(line.contains("dominant cause: starved"), "{line}");
    let entities = fs::read_to_string(dir.join("snap_001000").join("entities.json")).unwrap();
    assert!(entities.contains("\"state\":\"handling\""), "no hunter handling at tick 1000");
    let out = Command::new(env!("CARGO_BIN_EXE_ecosim")).args(["stats", "--signature"]).arg(&dir).output().unwrap();
    let sig = String::from_utf8(out.stdout).unwrap();
    assert!(sig.starts_with("signature: undefined, hunters extinct at tick ") && sig.contains("starved"), "{sig}");
    let params = small_with(&set);
    let sim = Sim::restore(params, &dir.join("snap_020000")).unwrap();
    assert_eq!((sim.tick, sim.count_hunters()), (20_000, 0), "the last snapshot restores");
}

/// Forced extinction under heredity: at `grazer.repro_energy=400` no grazer can ever breed (energy
/// tops out at 100, and the clamp keeps `repro_threshold` at 100 or more), so with mutation 0.2 the
/// grazers are eaten out on seed 1 (tick 2096) and the hunters starve after them. The run continues
/// to 20000 ticks with valid snapshots and no NaN. Grazer trait deviations stay 0 (no grazer is ever
/// born), hunter ones rise above 0 (hunters breed and mutate), and both species' trait columns read
/// 0 once they are extinct. `entities.json` animals carry the three traits, and the last snapshot
/// restores.
#[test]
fn forced_grazer_extinction_under_heredity_runs_to_the_end() {
    let dir = tmp("forced_heredity_extinction");
    let set = ["grazer.repro_energy=400".to_string(), "heredity.mutation=0.2".to_string()];
    let last = run_with(&dir, &set);
    assert_eq!((last.grazers, last.hunters), (0, 0), "both animal species extinct by the end");
    assert_eq!(last.traits, [[0.0; 6]; 2], "trait columns of extinct species");
    let text = assert_valid_run(&dir);
    let line = text.lines().find(|l| l.starts_with("extinction: grazers at tick")).unwrap_or_else(|| panic!("{text}"));
    assert!(line.contains("dominant cause: eaten"), "{line}");
    let series = ecosim::check::parse_series(&fs::read_to_string(dir.join("series.csv")).unwrap()).unwrap();
    assert!(series.iter().all(|r| r.traits[0][1] == 0.0 && r.traits[0][3] == 0.0 && r.traits[0][5] == 0.0));
    assert!(series.iter().any(|r| r.traits[1][1] > 0.0), "hunter energy_cost_mult never varied");
    let ents = read_json(&dir.join("snap_001000").join("entities.json"));
    let animals: Vec<&Value> = ents.as_array().unwrap().iter().filter(|e| e["kind"] != "tree").collect();
    assert!(!animals.is_empty());
    for a in animals {
        for k in ["energy_cost_mult", "flee_distance", "repro_threshold"] {
            assert!(a[k].as_f64().is_some_and(f64::is_finite), "{a}");
        }
    }
    let params = small_with(&set);
    let sim = Sim::restore(params, &dir.join("snap_020000")).unwrap();
    assert_eq!((sim.tick, sim.count_grazers()), (20_000, 0), "the last snapshot restores");
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

/// Copy `fresh` over the committed fixture at `fixture` when `ECOSIM_REGEN_MANIFEST=1` is set, and
/// report whether it did. The same switch `tests/sweep.rs` uses for the manifests and the golden
/// `check` output, so that one environment variable re-cuts every committed artefact a shot that
/// changes behaviour on purpose has to re-cut (shot G4c; before it these two fixtures were the only
/// ones left needing a throwaway script).
fn regen_fixture(fixture: &Path, fresh: &Path) -> bool {
    if std::env::var_os("ECOSIM_REGEN_MANIFEST").is_none() {
        return false;
    }
    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for e in fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if e.file_type().unwrap().is_dir() {
                copy_dir(&src, &dst);
            } else {
                fs::copy(&src, &dst).unwrap();
            }
        }
    }
    fs::remove_dir_all(fixture).unwrap();
    copy_dir(fresh, fixture);
    true
}

/// Every byte of a `light.bin` written today is the Beer-Lambert re-expression of the byte version 1
/// wrote at the same index (shot G4c): version 1 held `255 - 100 x layers` saturating at 0, and today
/// the same column holds `255 x exp(-canopy_k x canopy_lai x layers)` with `canopy_k x canopy_lai`
/// = 1.0. The layer count is recoverable from the old byte for every value the old rule could write
/// except 0, which is a solid voxel or three-and-more layers of canopy; a solid voxel is still 0
/// today, and both fixture snapshots predate any mature crown, so 0 means solid here and the mapping
/// is exhaustive. That last part is asserted, not assumed: an unexpected 0 fails.
fn light_is_the_v1_file_re_extincted(got: &[u8], want: &[u8], rel: &str) {
    assert_eq!(got.len(), want.len(), "{rel} length");
    for (i, (&g, &w)) in got.iter().zip(want).enumerate() {
        let expect = if w == 0 {
            0
        } else {
            let layers = f32::from((255 - w) / 100);
            (255.0 * libm::expf(-layers)).round() as u8
        };
        assert_eq!(g, expect, "{rel} byte {i}: version 1 wrote {w}");
    }
}

/// Format version 2, fire and traits only add files, fields and columns. A fresh seed-42 mini run
/// with fire, crowding and mutation off, the hunter refractory at the old cooldown and the water tier
/// off, on the square world (`common::SQUARE`) the fixtures were made on, is compared with the v1
/// fixture after the trait and fire additions are cut (`common::without_traits`,
/// `common::without_fire`, `common::without_water`).
///
/// Shot G4b ended the byte-for-byte half of this comparison, and did not replace it with a rewritten
/// fixture: converting a per-tick rate to a rate per hour changes the numbers a run produces, and the
/// version-1 writer is gone, so `fixtures/s42-mini` cannot be re-cut (DECISIONS.md, "Units
/// calibration"). Four series columns (`grass_mean`, `moisture_mean`, `fertility_mean` and
/// `detritus_total`) now differ in their fourth decimal, because the conversion rounded the derived
/// constants to three or four significant figures. What version 1 still pins is the format: the
/// tick-0 snapshot byte for byte (the world a seed makes has not moved, and the fire and trait fields
/// are all that version 2 and the shots after it add to it), the file set of both snapshots plus
/// `state.bin`, the terrain files at tick 100, the series header and length, and the shape of
/// `meta.json`, whose every version-1 params key is still written under its own name or the name the
/// conversion gave it.
///
/// Shot G4c took `light.bin` out of that byte comparison, because the quantity it holds no longer
/// exists: version 1 wrote `255 - 100 x canopy layers` and every run since G4c writes
/// `255 x exp(-1.0 x layers)`. It is not widened in its place — the file is still checked, and more
/// tightly than equality would: [`light_is_the_v1_file_re_extincted`] asserts that every byte of the
/// fresh file is the Beer-Lambert re-expression of the byte version 1 wrote at the same index, so the
/// claim "only the light rule changed, column for column" is the thing being tested.
#[test]
fn format_2_and_fire_only_add_to_version_1_files() {
    let v1 = fixture("s42-mini");
    let fresh = tmp("mini_fire_off");
    let mut p = common::square();
    p.fire.base_rate = 0.0;
    p.disease.grazer_rate = 0.0;
    p.disease.hunter_rate = 0.0;
    p.hunter.refractory = 5000;
    p.heredity.mutation = 0.0;
    p.hydro.enabled = false;
    run(p, 42, 100, 100, &[], &fresh).unwrap();
    let cut = |rel: &str| {
        let f = Path::new(rel);
        let b = common::without_water(f, fs::read(fresh.join(rel)).unwrap());
        common::without_fire(f, common::without_traits(f, b))
    };
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
    let (got, want) = (text(cut("series.csv")), text(fs::read(v1.join("series.csv")).unwrap()));
    assert_eq!(got.lines().next(), want.lines().next(), "series header");
    assert_eq!(got.lines().count(), want.lines().count(), "series rows");
    for snap in ["snap_000000", "snap_000100"] {
        let files: Vec<_> = fs::read_dir(v1.join(snap)).unwrap().map(|e| e.unwrap().file_name()).collect();
        for f in &files {
            let name = f.to_str().unwrap();
            let rel = format!("{snap}/{name}");
            let terrain = matches!(name, "material.bin" | "light.bin" | "height.bin");
            if name == "light.bin" {
                light_is_the_v1_file_re_extincted(&cut(&rel), &fs::read(v1.join(&rel)).unwrap(), &rel);
            } else if snap == "snap_000000" || terrain {
                assert!(cut(&rel) == fs::read(v1.join(&rel)).unwrap(), "{rel}");
            }
        }
        assert_eq!(fs::read_dir(fresh.join(snap)).unwrap().count(), files.len() + 1);
        assert!(fresh.join(snap).join("state.bin").exists());
    }
    let (a, b) = (read_json(&v1.join("meta.json")), read_json(&fresh.join("meta.json")));
    assert_eq!((a["format_version"].as_u64(), b["format_version"].as_u64()), (Some(1), Some(FORMAT_VERSION as u64)));
    assert!(b["forked_from"].is_null() && b["params"]["fire"].is_object());
    assert_eq!(
        (&a["params"]["hunter"]["cooldown"], &b["params"]["hunter"]["refractory"]),
        (&5000.into(), &5000.into())
    );
    for k in ["seed", "ticks", "snapshot_every", "snapshots", "species", "water_level", "year_len", "overrides"] {
        assert_eq!(a[k], b[k], "{k}");
    }
    for k in ["x", "y", "z"] {
        assert_eq!(a["dims"][k], b["dims"][k], "dims.{k}");
    }
    // The params keys the units conversion renamed (shots G4b and G4c), as (section, version 1,
    // now). A rename changes the key, not the modelled quantity: the value may be in a new unit.
    const RENAMED: [(&str, &str, &str); 11] = [
        ("hunter", "cooldown", "refractory"),
        ("cover", "moisture_draw", "water_per_growth_mm"),
        ("tree", "moisture_draw", "transpiration_mm_h"),
        ("tree", "dry_moisture", "dry_fraction"),
        ("world", "canopy_absorb", "canopy_k"),
        ("tree", "initial_age", "initial_age_years"),
        ("tree", "young_age", "young_age_years"),
        ("tree", "mature_age", "mature_age_years"),
        ("tree", "max_age", "max_age_years"),
        ("tree", "dry_death_ticks", "dry_death_days"),
        ("tree", "seed_every", "seeds_per_year"),
    ];
    for (section, keys) in a["params"].as_object().unwrap() {
        let now = b["params"][section].as_object().unwrap_or_else(|| panic!("params.{section} is gone"));
        for key in keys.as_object().unwrap().keys() {
            let rename = RENAMED.iter().find(|(s, old, _)| s == section && old == key);
            let want = rename.map_or(key.as_str(), |(_, _, now)| now);
            assert!(now.contains_key(want), "params.{section}.{key} is gone (looked for {want})");
        }
    }

    // `--format-version 2` still writes the committed v2 fixture byte for byte; format 3 adds only
    // events.csv and the version number.
    let v2 = tmp("mini_v2");
    let set = common::square_set(&[]);
    run_opts(common::square(), 42, 100, 100, &set, &v2, RunOptions { format_version: 2, ..Default::default() })
        .unwrap();
    if !regen_fixture(&fixture("s42-mini-v2"), &v2) {
        assert_eq!(ecosim::check::diff_runs(&fixture("s42-mini-v2"), &v2).unwrap(), Vec::<String>::new());
    }
    let defaults = tmp("mini_defaults");
    run(common::square(), 42, 100, 100, &set, &defaults).unwrap();
    let diff = ecosim::check::diff_runs(&v2, &defaults).unwrap();
    let (only, differs): (Vec<&String>, Vec<&String>) = diff.iter().partition(|s| s.starts_with("only in"));
    assert_eq!(differs, ["differs: meta.json"]);
    // Version 3 added events.csv, version 4 the four `world/` files and the two water files per
    // snapshot (shot G4). Everything else is the same run: the water tier is on in both.
    assert_eq!(only.len(), 1 + 4 + 2 * 2, "{only:?}");
    assert!(only.iter().all(|s| ["events.csv", "world/", "water.bin"].iter().any(|k| s.contains(k))), "{only:?}");
    let (mut a, mut b) = (read_json(&v2.join("meta.json")), read_json(&defaults.join("meta.json")));
    assert_eq!((a["format_version"].as_u64(), b["format_version"].as_u64()), (Some(2), Some(4)));
    a["format_version"] = 4.into();
    b.as_object_mut().unwrap().remove("world");
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

/// `rng.stream` (shot 14). Stream 0 is the stream every earlier run drew from: params with it set
/// explicitly give a byte-identical run directory, and `meta.json` carries no `rng` section at 0.
/// Another stream keeps the seed's terrain but is a different run on it, recorded in `meta.json`.
#[test]
fn rng_stream_0_is_the_default_stream_and_others_differ() {
    let (a, b, c) = (tmp("stream_default"), tmp("stream_0"), tmp("stream_5"));
    let with = |s: &str| {
        Params::from_toml_str_with(
            &fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml")).unwrap(),
            &[s.to_string()],
        )
        .unwrap()
    };
    run(Params::load_default(), 42, 300, 100, &[], &a).unwrap();
    run(with("rng.stream=0"), 42, 300, 100, &[], &b).unwrap();
    run(with("rng.stream=5"), 42, 300, 100, &[], &c).unwrap();
    assert_eq!(ecosim::check::diff_runs(&a, &b).unwrap(), Vec::<String>::new());
    let (ma, mb, mc) =
        (read_json(&a.join("meta.json")), read_json(&b.join("meta.json")), read_json(&c.join("meta.json")));
    assert!(ma["params"].get("rng").is_none() && mb["params"].get("rng").is_none());
    assert_eq!(mc["params"]["rng"]["stream"], 5);
    let differs = ecosim::check::diff_runs(&a, &c).unwrap();
    assert!(
        differs.contains(&"differs: series.csv".to_string())
            && differs.contains(&"differs: snap_000000/entities.json".to_string()),
        "{differs:?}"
    );
    assert!(!differs.iter().any(|d| d.ends_with("height.bin")), "{differs:?}");
}

/// `ecosim run --profile` (shot 15a): the same seed with and without the profiler writes
/// byte-identical run directories, the profile lies outside them, and its phase times sum to within
/// 5% of its total. A profile path inside `--out` is refused before anything is written.
#[test]
fn profile_leaves_the_run_directory_unchanged_and_accounts_for_the_run() {
    let exe = Path::new(env!("CARGO_BIN_EXE_ecosim"));
    let (a, b, file) = (tmp("profile_off"), tmp("profile_on"), tmp("profile.json"));
    let params = Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml");
    let run = |out: &Path, extra: &[&Path]| {
        let mut cmd = Command::new(exe);
        cmd.args(["run", "--seed", "42", "--ticks", "600", "--snapshot-every", "200", "--params"]).arg(&params);
        cmd.arg("--out").arg(out);
        if let Some(f) = extra.first() {
            cmd.arg("--profile").arg(f);
        }
        cmd.status().expect("spawn ecosim run").success()
    };
    assert!(run(&a, &[]));
    assert!(run(&b, &[&file]));
    assert_same_run(exe, &a, &b);
    assert!(!fs::read_dir(&b).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("profile")));

    let p = read_json(&file);
    let (total, sum) = (p["total_ms"].as_f64().unwrap(), p["phase_sum_ms"].as_f64().unwrap());
    let phases = p["phases"].as_array().unwrap();
    let listed: f64 = phases.iter().map(|ph| ph["ms"].as_f64().unwrap()).sum();
    assert!((listed - sum).abs() < 1e-6 * total.max(1.0), "{listed} vs {sum}");
    assert!(total > 0.0 && (total - sum).abs() <= 0.05 * total, "phases sum to {sum} ms of {total} ms");
    let names: Vec<&str> = phases.iter().map(|ph| ph["name"].as_str().unwrap()).collect();
    let want = ["animals", "producers", "moisture_fertility", "temperature_season", "fire", "events", "snapshot_write"];
    for name in want.iter().chain(&["stats_row"]) {
        assert!(names.contains(name), "no phase {name} in {names:?}");
    }
    assert_eq!((p["ticks"].as_u64(), p["seed"].as_u64()), (Some(600), Some(42)));
    assert_eq!(p["dims"], serde_json::json!([256, 64, 32, 8]));
    assert!(p["ticks_per_second"].as_f64().unwrap() > 0.0);

    let c = tmp("profile_inside");
    assert!(!run(&c, &[&c.join("profile.json")]));
    assert!(!c.exists());
}

/// Shot G0: `animals.enabled = false` leaves the animal tier out of a whole 20000-tick run on the
/// reference strip. No grazer or hunter exists at any tick, `events.csv` has no animal row,
/// `meta.json` records `"animals": false`, two runs of the same seed are byte-identical, and
/// `ecosim check` marks exactly the animal-only invariants n/a while every other one still counts
/// (`no_extinction` and `max_10x` keep their tree part). `ecosim stats` reports no extinction of a
/// species that was never placed, and the signature is not applicable.
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn animals_off_run_has_no_animals_and_marks_the_animal_invariants_na() {
    let set = vec!["animals.enabled=false".to_string()];
    let dir = tmp("animals_off");
    let last = run_on(&dir, &set);
    assert_eq!((last.grazers, last.hunters), (0, 0));
    assert!(last.trees > 0, "trees died out without animals");

    let rows = ecosim::check::read_series(&dir).unwrap();
    assert!(rows.iter().all(|r| r.grazers == 0 && r.hunters == 0 && r.hunter_immigrants == 0), "an animal exists");
    assert!(rows.iter().all(|r| r.deaths.iter().flatten().all(|&n| n == 0)), "an animal died");
    let events = ecosim::events::parse_events(&fs::read_to_string(dir.join("events.csv")).unwrap()).unwrap();
    assert!(!events.is_empty(), "no events at all");
    assert!(events.iter().all(|e| e.species != "grazer" && e.species != "hunter"), "an animal event was logged");

    let meta = read_json(&dir.join("meta.json"));
    assert_eq!(meta["animals"], serde_json::json!(false));
    assert_eq!(meta["params"]["animals"], serde_json::json!({ "enabled": false }));
    // The water tier makes every run format version 4 (shot G4); the animals switch is not itself a
    // format change.
    assert_eq!(meta["format_version"], BUNDLE_FORMAT_VERSION);

    let again = tmp("animals_off_again");
    run_on(&again, &set);
    assert_eq!(ecosim::check::diff_runs(&dir, &again).unwrap(), Vec::<String>::new());

    let report = ecosim::check::check_run(&dir).unwrap();
    assert_eq!(report.not_applicable(), ecosim::check::ANIMAL_ONLY_KEYS.to_vec());
    for l in report.lines.iter().filter(|l| l.na) {
        assert!(l.observed.contains(ecosim::check::NA_REASON), "{l:?}");
    }
    let counted = report.lines.iter().filter(|l| !l.na).count();
    assert_eq!(counted, report.lines.len() - 2);
    let no_ext = report.get("no_extinction").unwrap();
    assert!(no_ext.observed.starts_with("min trees=") && !no_ext.observed.contains("grazers"), "{no_ext:?}");
    assert!(report.get("max_10x").unwrap().observed.starts_with("trees "));
    assert!(report.pass(), "{:?}", report.lines);

    let text = assert_valid_run(&dir);
    assert!(text.contains("first extinction: none"), "{text}");
    assert!(!text.contains("extinction: grazers") && !text.contains("extinction: hunters"), "{text}");
    let sig = ecosim::check::signature_of(&dir, None).unwrap();
    assert_eq!(sig, ecosim::check::Signature::NotApplicable);
    assert!(ecosim::check::signature_line(&sig).contains("not applicable"));
}
