//! A run on a world bundle (shot G1): the run directory it writes, and the CLI that asks for it.
//! The bundle is built here, so CI needs no scene and no Blender.

use ecosim::bundle::{Bundle, Medium};
use ecosim::check::check_run;
use ecosim::output::{run_with, RunOptions, BUNDLE_FORMAT_VERSION};
use ecosim::Params;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&d);
    d
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

/// Ground cells per column edge and the crop's edge in metres: a 16 m square at 0.5 m cells.
const RATIO: usize = 2;
const SIZE_M: usize = 16;
const GW: usize = SIZE_M * RATIO;

/// Write a synthetic bundle: a gentle east-facing slope of lawn, a 4 × 3 roof block 7 m tall, an
/// asphalt path and a pond. Returns its directory.
fn write_bundle(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    let code = |m: Medium| Medium::ALL.iter().position(|&k| k == m).unwrap() as u8;
    let mut ground_h = vec![0f32; GW * GW];
    let mut medium = vec![code(Medium::Lawn); GW * GW];
    let mut building_h = vec![0f32; GW * GW];
    for gy in 0..GW {
        for gx in 0..GW {
            ground_h[gx + GW * gy] = 0.25 * (gx as f32 * 0.5);
        }
    }
    let mut paint = |x0: usize, x1: usize, y0: usize, y1: usize, m: Medium, bh: f32| {
        for y in y0..y1 {
            for x in x0..x1 {
                for gy in y * RATIO..(y + 1) * RATIO {
                    for gx in x * RATIO..(x + 1) * RATIO {
                        medium[gx + GW * gy] = code(m);
                        building_h[gx + GW * gy] = bh;
                    }
                }
            }
        }
    };
    paint(2, 6, 2, 5, Medium::Roof, 7.0);
    paint(0, 16, 6, 7, Medium::Asphalt, 0.0);
    paint(11, 15, 11, 14, Medium::Water, 0.0);
    let media: Vec<String> = Medium::ALL.iter().map(|m| format!("{:?}", m.name())).collect();
    let json = format!(
        "{{\"format\":\"ecosim-world-bundle\",\"version\":2,\"name\":\"synthetic\",\"size_m\":{SIZE_M},\
         \"ground_cell_m\":0.5,\"ground_width\":{GW},\"ground_depth\":{GW},\"media\":[{}],\
         \"source\":\"built by tests/bundle.rs\",\"counts\":{{\"trees\":1,\"shrubs\":1,\"pipes\":1}}}}",
        media.join(",")
    );
    fs::write(dir.join("bundle.json"), json).unwrap();
    let f32s = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    fs::write(dir.join("ground_h.f32"), f32s(&ground_h)).unwrap();
    fs::write(dir.join("building_h.f32"), f32s(&building_h)).unwrap();
    fs::write(dir.join("medium.u8"), &medium).unwrap();
    fs::write(
        dir.join("trees.json"),
        "[{\"x\":8.5,\"y\":9.5,\"height\":11.0,\"crown_radius\":3.5,\"crown_base\":4.0}]",
    )
    .unwrap();
    fs::write(dir.join("shrubs.json"), "[{\"x\":3.0,\"y\":12.0,\"height\":1.4,\"rx\":0.9,\"ry\":0.6,\"angle\":0.0}]")
        .unwrap();
    fs::write(
        dir.join("pipes.json"),
        "[{\"id\":\"drain-1\",\"inlet\":[12.0,12.0],\"outlet\":[16.0,12.0],\"capacity_m3h\":18.0,\"illustrative\":true}]",
    )
    .unwrap();
}

/// The crate's params with `set` applied. (`tests/common` is not used here: this file needs one
/// helper from it and would warn about the rest.)
fn params(set: &[&str]) -> (Params, Vec<String>) {
    let set: Vec<String> = set.iter().map(|s| s.to_string()).collect();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml");
    (Params::load_with(&path, &set).unwrap(), set)
}

/// The params a bundle run uses here: the crate's own with the two overrides every garden-series
/// run on a bundle world carries — animals off (shot G0) and flat rainfall (shot G3a: the
/// west-to-east ramp is the noise strip's point and means nothing on a photographed site) — plus
/// the bundle's dimensions.
fn bundle_params(b: &Bundle) -> (Params, Vec<String>) {
    let (mut p, set) = params(&["animals.enabled=false", "climate.rain_gradient=0"]);
    b.apply_to(&mut p).unwrap();
    (p, set)
}

fn meta(run_dir: &Path) -> Value {
    serde_json::from_slice(&fs::read(run_dir.join("meta.json")).unwrap()).unwrap()
}

/// A full-length run on a bundle writes a format-4 run directory: the ground grid once in `world/`,
/// the bundle's description in `meta.json`, and the snapshots every other run writes. `ecosim
/// check` reads it (its verdicts are about the ecology, not the format, so they are not asserted).
#[test]
#[cfg_attr(coverage, ignore = "full-length run; runs in `cargo test` and CI step 8, not under llvm-cov")]
fn a_bundle_run_writes_a_format_4_run_dir_that_check_reads() {
    let dir = tmp("bundle_src");
    write_bundle(&dir);
    let b = Bundle::load(&dir).unwrap();
    assert_eq!((b.trees.len(), b.shrubs.len(), b.pipes.len()), (1, 1, 1), "the scene files are parsed");
    let (p, set) = bundle_params(&b);
    let out = tmp("bundle_run");
    let opts = RunOptions { format_version: BUNDLE_FORMAT_VERSION, bundle: Some(&b), ..Default::default() };
    run_with(p, 7, 20_000, 5_000, &set, &out, opts).unwrap();

    let m = meta(&out);
    assert_eq!(m["format_version"], BUNDLE_FORMAT_VERSION);
    assert_eq!(m["dims"], serde_json::json!({"x": SIZE_M, "y": SIZE_M, "z": 32, "patch": 8}));
    assert_eq!(m["world"]["name"], "synthetic");
    assert_eq!(m["world"]["ground_cell_m"], 0.5);
    assert_eq!((m["world"]["ground_width"].as_u64(), m["world"]["ground_depth"].as_u64()), (Some(32), Some(32)));
    assert_eq!(m["world"]["media"][0], "soil");
    assert_eq!(m["world"]["media"].as_array().unwrap().len(), 9);
    // Shot S9. This synthetic bundle has no `latitude_deg`, the way every bundle written before
    // S9 has none, and the key is still written -- as null, so a reader can tell "this world has
    // no latitude" from "this run predates the key".
    assert!(m["world"].as_object().unwrap().contains_key("latitude_deg"));
    assert_eq!(m["world"]["latitude_deg"], Value::Null);
    assert_eq!(m["animals"], false, "the garden series runs without animals");

    // The static ground grid is written once, at the run root.
    let w = out.join("world");
    for (name, len) in [("ground_h.bin", GW * GW * 4), ("medium.bin", GW * GW), ("building_h.bin", GW * GW * 4)] {
        assert_eq!(fs::read(w.join(name)).unwrap().len(), len, "world/{name}");
    }
    assert_eq!(fs::read(w.join("ground_h.bin")).unwrap(), fs::read(dir.join("ground_h.f32")).unwrap());
    assert_eq!(fs::read(w.join("medium.bin")).unwrap(), fs::read(dir.join("medium.u8")).unwrap());
    assert_eq!(fs::read(w.join("pipes.json")).unwrap(), fs::read(dir.join("pipes.json")).unwrap());
    assert!(!out.join("snap_000000/medium.bin").exists(), "the ground grid is not repeated per snapshot");

    // The snapshot files are the ones every run writes, at the bundle's dimensions.
    let snap = out.join("snap_010000");
    for (name, len) in
        [("material.bin", SIZE_M * SIZE_M * 32), ("light.bin", SIZE_M * SIZE_M * 32), ("height.bin", SIZE_M * SIZE_M)]
    {
        assert_eq!(fs::read(snap.join(name)).unwrap().len(), len, "{name}");
    }
    // The roof block is Rock, so its columns hold nothing, and the pond is water.
    let height: Vec<u8> = fs::read(snap.join("height.bin")).unwrap();
    let material: Vec<u8> = fs::read(snap.join("material.bin")).unwrap();
    let at = |x: usize, y: usize, z: usize| material[x + SIZE_M * (y + SIZE_M * z)];
    assert_eq!(at(3, 3, height[3 + SIZE_M * 3] as usize), 2, "rock under the roof block");
    assert_eq!(at(12, 12, height[12 + SIZE_M * 12] as usize), 3, "water in the pond");
    assert_eq!(at(9, 9, height[9 + SIZE_M * 9] as usize), 1, "soil on the lawn");

    let report = check_run(&out).expect("check reads a format-4 run directory");
    assert!(report.lines.iter().any(|l| l.key == "grass_band"), "check evaluated the plant invariants");
    assert!(report.lines.iter().find(|l| l.key == "grazer_cycle").is_some_and(|l| l.na), "no animals: n/a");
    // Shot G11: the footing invariant applies to a bundle run and holds. The roof block is Rock, so
    // no tree can root in it, and the 1 m asphalt path covers at most two cells of a column beside
    // it. The check reads the run's own `world/medium.bin`, so it sees the same sealed cells the
    // bundle declared.
    let foot = report.lines.iter().find(|l| l.key == "tree_footing").expect("a bundle run has a footing line");
    assert!(!foot.na && foot.pass, "{foot:?}");
    assert!(foot.observed.contains("0 over half sealed, 0 roof cells"), "{foot:?}");
    assert!(foot.observed.contains("over 5 snapshots"), "{foot:?}");
    fs::remove_dir_all(&dir).unwrap();
}

/// Rewrite `dir/bundle.json` with `latitude_deg` set to `lat`, leaving every other key as it was.
fn set_latitude(dir: &Path, lat: f64) {
    let text = fs::read_to_string(dir.join("bundle.json")).unwrap();
    let mut j: serde_json::Map<String, Value> = serde_json::from_str(&text).unwrap();
    j.insert("latitude_deg".into(), serde_json::json!(lat));
    fs::write(dir.join("bundle.json"), serde_json::to_vec(&j).unwrap()).unwrap();
}

/// Shot S9: the scene's latitude reaches the run, and nothing else makes one up. A bundle that
/// carries a latitude puts it in `meta.json`'s `world`; a noise world has no site at all and writes
/// null there. Nothing in the simulator reads the number -- the run is the carrier, and the
/// renderer that draws the sun is the reader -- so what is tested is that it arrives unaltered.
///
/// The latitude used here is southern (-33.8688, Sydney) on purpose: a sign dropped anywhere
/// between the scene and `meta.json` would put this site's sun in the northern sky, and a
/// northern test number could not tell.
#[test]
#[cfg_attr(coverage, ignore = "two short runs on a 16 m world; runs in `cargo test`, not under llvm-cov")]
fn the_scenes_latitude_reaches_the_runs_meta_json() {
    let dir = tmp("bundle_lat_src");
    write_bundle(&dir);
    set_latitude(&dir, -33.8688);
    let b = Bundle::load(&dir).unwrap();
    assert_eq!(b.latitude_deg, Some(-33.8688), "the loader reads it");
    let (p, set) = bundle_params(&b);
    let out = tmp("bundle_lat_run");
    let opts = RunOptions { format_version: BUNDLE_FORMAT_VERSION, bundle: Some(&b), ..Default::default() };
    run_with(p, 7, 100, 100, &set, &out, opts).unwrap();
    assert_eq!(meta(&out)["world"]["latitude_deg"], serde_json::json!(-33.8688), "and the run carries it");

    // A noise world is format 4 through the water tier and has no bundle, so it has no latitude.
    let noise = tmp("noise_lat_run");
    let (p2, set2) = params(&["world.width=16", "world.depth=16", "world.patch=8"]);
    run_with(p2, 7, 100, 100, &set2, &noise, RunOptions::default()).unwrap();
    let m = meta(&noise);
    assert_eq!(m["format_version"], BUNDLE_FORMAT_VERSION, "the water tier writes format 4 on noise too");
    assert_eq!(m["world"]["bundle"], false);
    assert_eq!(m["world"]["latitude_deg"], Value::Null, "a noise world is nowhere on Earth");

    // A latitude that is not one is the scene's mistake and stops the load, naming the file.
    set_latitude(&dir, 120.0);
    let e = Bundle::load(&dir).unwrap_err();
    assert!(e.contains("bundle.json") && e.contains("latitude_deg = 120"), "{e}");

    fs::remove_dir_all(&dir).unwrap();
    fs::remove_dir_all(&out).unwrap();
    fs::remove_dir_all(&noise).unwrap();
}

/// `ecosim run --world` builds the bundle world and writes format version 4. Asking for an older
/// version with `--world` is an error; asking for version 4 without one is not, since the water
/// tier writes it on a noise world too (shot G4).
#[test]
fn the_cli_runs_a_bundle_and_pairs_format_4_with_world() {
    let exe = Path::new(env!("CARGO_BIN_EXE_ecosim"));
    let dir = tmp("bundle_cli_src");
    write_bundle(&dir);
    let out = tmp("bundle_cli_run");
    let params = Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml");
    let run = |args: &[&str], out: &Path| {
        Command::new(exe)
            .args(["run", "--seed", "3", "--ticks", "200", "--snapshot-every", "100", "--out"])
            .arg(out)
            .arg("--params")
            .arg(&params)
            .args(["--set", "animals.enabled=false", "--set", "climate.rain_gradient=0"])
            .args(args)
            .output()
            .expect("spawn ecosim run")
    };
    let ok = run(&["--world", dir.to_str().unwrap()], &out);
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    let m = meta(&out);
    assert_eq!(m["format_version"], BUNDLE_FORMAT_VERSION);
    assert_eq!(m["world"]["name"], "synthetic");
    assert_eq!(m["params"]["world"]["width"], SIZE_M, "the bundle sets the ecology dimensions");
    assert_eq!(m["params"]["bundle"]["base_z"], 8, "[bundle] is in meta.json even at its defaults (shot S2)");

    let mismatch = run(&["--world", dir.to_str().unwrap(), "--format-version", "3"], &tmp("bundle_cli_bad"));
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("--world writes format_version 4"));
    let lonely_out = tmp("bundle_cli_noise4");
    let lonely = run(&["--format-version", "4"], &lonely_out);
    assert!(lonely.status.success(), "{}", String::from_utf8_lossy(&lonely.stderr));
    assert_eq!(meta(&lonely_out)["world"]["bundle"], Value::Bool(false), "a noise world, not a bundle");
    let missing = run(&["--world", "no-such-bundle"], &tmp("bundle_cli_bad3"));
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("bundle.json"));
    fs::remove_dir_all(&dir).unwrap();
}

/// A noise world writes format version 4 too since the water tier (shot G4): the same `world/`
/// directory, with its ground grid all soil, and `world.bundle` false. With the tier off it is the
/// version-3 directory it was before, with no `world/` directory and no `world` key in `meta.json`.
#[test]
fn a_noise_run_writes_format_4_with_a_synthetic_world_directory() {
    let out = tmp("bundle_noise");
    let square = ["world.width=64", "climate.rain_gradient=0", "world.slope_bias=0"];
    let (p, set) = params(&square);
    run_with(p, 1, 100, 50, &set, &out, RunOptions::default()).unwrap();
    let m = meta(&out);
    assert_eq!(m["format_version"], BUNDLE_FORMAT_VERSION);
    assert_eq!((&m["world"]["name"], &m["world"]["bundle"]), (&Value::from("noise"), &Value::Bool(false)));
    for f in ["ground_h.bin", "medium.bin", "building_h.bin", "pipes.json"] {
        assert!(out.join("world").join(f).exists(), "{f}");
    }

    let dry = tmp("bundle_noise_dry");
    let (p, set) = params(&[square.as_slice(), &["hydro.enabled=false"]].concat());
    run_with(p, 1, 100, 50, &set, &dry, RunOptions::default()).unwrap();
    let m = meta(&dry);
    assert_eq!(m["format_version"], 3);
    assert_eq!(m["world"], Value::Null);
    assert!(!dry.join("world").exists());

    // Shot G11: both are noise worlds, so the footing invariant is n/a either way — the synthesised
    // grid is all soil, in which nothing is sealed, and the version-3 run has no grid at all.
    for d in [&out, &dry] {
        let l = check_run(d).unwrap().get("tree_footing").cloned().expect("a footing line");
        assert!(l.na && l.pass && l.observed.contains(ecosim::check::NA_REASON_NO_GROUND), "{l:?}");
    }
    fs::remove_dir_all(&out).unwrap();
    fs::remove_dir_all(&dry).unwrap();
}

/// The committed Capitol bundle (shot G2), the first real world in the repo. The exporter runs in
/// Blender, which CI does not have, so what CI checks is the bundle as committed: that G1's loader
/// reads it, and that the world it builds still has the shape shot G2 recorded in DECISIONS.md. A
/// re-export that moved any of these numbers would show up here rather than in a later shot's run.
#[test]
fn the_committed_capitol_bundle_loads_with_its_documented_shape() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/capitol");
    let b = Bundle::load(&dir).unwrap();
    assert_eq!(b.name, "capitol");
    assert_eq!(b.size_m, 256, "a 256 m crop is 256 x 256 ecology columns");
    assert_eq!((b.ground.width, b.ground.depth, b.ground.ratio), (512, 512, 2), "0.5 m ground cells");
    assert_eq!((b.trees.len(), b.shrubs.len(), b.pipes.len()), (81, 64, 4));
    assert!(b.source.contains("OpenStreetMap"), "the ODbL credit travels with the bundle");
    assert_eq!(
        b.latitude_deg,
        Some(42.73365),
        "the dome's latitude (shot S9), the same number README.md gives the crop centre in prose"
    );

    // Media: the LiDAR-and-OSM raster, in ground cells (256 m² each 0.5 m cell covers 0.25 m²).
    let mut cells = std::collections::BTreeMap::new();
    for i in 0..b.ground.cells() {
        *cells.entry(b.ground.medium_at(i).name()).or_insert(0usize) += 1;
    }
    assert_eq!(cells["lawn"], 169_876);
    assert_eq!(cells["concrete"], 22_684);
    assert_eq!(cells["asphalt"], 44_879);
    assert_eq!(cells["roof"], 24_705);
    assert_eq!(cells.len(), 4, "the scene classified nothing as soil, bed, mulch, gravel or water");
    let h = |v: &[f32]| (v.iter().cloned().fold(f32::MAX, f32::min), v.iter().cloned().fold(f32::MIN, f32::max));
    assert_eq!(h(&b.ground_h), (0.0, 8.589), "ground heights are metres above the crop minimum");
    assert_eq!(h(&b.building_h).1, 76.011, "the dome, the tallest thing in the crop");

    let (p, _) = bundle_params(&b);
    let w = ecosim::world::World::from_bundle(&b, &p).unwrap();
    assert_eq!((w.dims.wx, w.dims.wy, w.dims.wz), (256, 256, 32));
    let rock = w.class.iter().filter(|&&k| k == ecosim::world::ColClass::Rock).count();
    assert_eq!(rock, 21_705, "sealed columns, 33.1% of the grid: building, street or walk");
    assert_eq!(w.class.iter().filter(|&&k| k == ecosim::world::ColClass::Water).count(), 0);
    let (lo, hi) = (*w.height.iter().min().unwrap(), *w.height.iter().max().unwrap());
    assert_eq!((lo, hi), (8, 16), "base_z = 8 plus 0..8 m of real ground, well under wz = 32");
}

/// The Capitol's scene planted (shot G3). Like the shape above, these numbers are the committed
/// bundle's, so a re-export that moved a tree shows up here and not in a later shot's run.
#[test]
fn the_capitol_scene_plants_its_trees_and_shrubs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/capitol");
    let b = Bundle::load(&dir).unwrap();
    let (p, _) = bundle_params(&b);
    let (sim, imp) = ecosim::Sim::from_bundle(p, 42, &b).unwrap();

    assert_eq!(imp.trees_in_scene, 81);
    assert_eq!(imp.trees_planted, 79);
    assert_eq!(imp.trees_moved, 0, "no tree on a sealed column had a plantable one within 2 m");
    assert_eq!(imp.trees_dropped, 2, "two street trees stand in asphalt, more than 2 m from soil");
    assert_eq!(imp.trees_merged, 0, "no two scene trees share a 1 m column");
    assert_eq!((imp.shrubs_in_scene, imp.shrub_columns, imp.shrub_patches), (64, 750, 87));

    // Every trunk is on a plantable column, one per column, and the scene's trees are all at least
    // 4.6 m tall, so every one of them starts mature.
    assert_eq!(sim.trees.len(), 79);
    let mature = sim.trees.iter().filter(|t| sim.tree_stage(t) == ecosim::trees::Stage::Mature).count();
    assert_eq!(mature, 79, "the shortest scene tree is 4.675 m, well over the 3 m mature height");
    let mut cols: Vec<usize> = sim.trees.iter().map(|t| t.col(sim.world.dims)).collect();
    assert!(cols.iter().all(|&c| sim.world.is_plantable(c)));
    cols.sort_unstable();
    let n = cols.len();
    cols.dedup();
    assert_eq!(cols.len(), n, "one trunk per column");

    // The shrub beds raise 87 of the 1024 patches above `shrub.initial` and leave the rest alone.
    let raised = sim.patches.iter().filter(|q| q.shrub > p_initial(&sim)).count();
    assert_eq!(raised, 87);
    assert!(sim.patches.iter().all(|q| q.shrub <= 1.0));
    // The drains are in the world for shot G6.
    assert_eq!(sim.world.pipes.len(), 4);
    assert_eq!(sim.world.pipes[0].id, "pipe_1");
}

/// The starting shrub density of a patch the scene did not touch.
fn p_initial(sim: &ecosim::Sim) -> f32 {
    sim.params.shrub.initial
}

/// `fixtures/capitol-mini` is the committed 2-snapshot Capitol run the renderer reads (shot G3).
/// A fresh run of the command that made it matches it byte for byte, `timing.json` apart.
#[test]
#[cfg_attr(coverage, ignore = "a 256x256 world; runs in `cargo test` and CI step 3, not under llvm-cov")]
fn the_committed_capitol_mini_fixture_matches_a_fresh_run() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let b = Bundle::load(&root.join("worlds/capitol")).unwrap();
    let (p, set) = bundle_params(&b);
    let out = tmp("capitol_mini");
    let opts = RunOptions { format_version: BUNDLE_FORMAT_VERSION, bundle: Some(&b), ..Default::default() };
    run_with(p, 42, 100, 100, &set, &out, opts).unwrap();
    let fixture = root.join("fixtures/capitol-mini");
    if !regen_fixture(&fixture, &out) {
        assert_eq!(ecosim::check::diff_runs(&fixture, &out).unwrap(), Vec::<String>::new());
    }
    fs::remove_dir_all(&out).unwrap();
}

/// The params for the one committed bundle-world run that keeps its animals (shot S1): flat
/// rainfall like every other bundle run (shot G3a), and `animals.enabled` left at the file's own
/// `true`, which is the entire point of the fixture and so is not overridden here.
fn bundle_params_with_animals(b: &Bundle) -> (Params, Vec<String>) {
    let (mut p, set) = params(&["climate.rain_gradient=0"]);
    b.apply_to(&mut p).unwrap();
    (p, set)
}

/// `fixtures/capitol-animals-mini` is the Capitol at seed 42 with the animal tier left on, 2000
/// ticks, snapshots at 0 and 2000 (shot S1). A fresh run of the command `README.md` documents
/// matches it byte for byte, `timing.json` apart — the same pin `capitol-mini` carries.
#[test]
#[cfg_attr(coverage, ignore = "a 256x256 world for 2000 ticks; runs in `cargo test` and CI step 3, not under llvm-cov")]
fn the_committed_capitol_animals_mini_fixture_matches_a_fresh_run() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let b = Bundle::load(&root.join("worlds/capitol")).unwrap();
    let (p, set) = bundle_params_with_animals(&b);
    let out = tmp("capitol_animals_mini");
    let opts = RunOptions { format_version: BUNDLE_FORMAT_VERSION, bundle: Some(&b), ..Default::default() };
    run_with(p, 42, 2_000, 2_000, &set, &out, opts).unwrap();
    let fixture = root.join("fixtures/capitol-animals-mini");
    if !regen_fixture(&fixture, &out) {
        assert_eq!(ecosim::check::diff_runs(&fixture, &out).unwrap(), Vec::<String>::new());
    }
    fs::remove_dir_all(&out).unwrap();
}

/// Read `entities.json` and return, for each animal, whether its position is written as a JSON
/// float, plus the grazer count of every 8 × 8 patch of a 256-column world.
fn animals_of(snap: &Path) -> (usize, usize, usize, Vec<u32>) {
    let v: Vec<Value> = serde_json::from_slice(&fs::read(snap.join("entities.json")).unwrap()).unwrap();
    let (mut grazers, mut hunters, mut floats) = (0, 0, 0);
    let mut per_patch = vec![0u32; 32 * 32];
    for e in &v {
        match e["kind"].as_str().unwrap() {
            "grazer" => grazers += 1,
            "hunter" => hunters += 1,
            _ => continue,
        }
        // `as_i64` refuses `81.0`: serde_json keeps the literal's kind, which is what a reader
        // that declared these fields `i32` tripped over.
        if e["x"].as_i64().is_none() && e["y"].as_i64().is_none() {
            floats += 1;
        }
        if e["kind"] == "grazer" {
            let (x, y) = (e["x"].as_f64().unwrap() as usize, e["y"].as_f64().unwrap() as usize);
            per_patch[x / 8 + 32 * (y / 8)] += 1;
        }
    }
    (grazers, hunters, floats, per_patch)
}

/// What the animals fixture is *for*, asserted on the committed bytes. Each of these is a hole the
/// animals-off reference runs left open, and each has already been fallen into once: a regeneration
/// that quietly lost the animals has to fail here and not in a viewer six shots later.
#[test]
fn the_animals_fixture_carries_what_an_animals_off_run_cannot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("fixtures/capitol-animals-mini");
    let m = meta(&fixture);

    // (1) The tier is on, and since shot S2 `meta.json` says so in words: `params.animals.enabled`
    // is written whether or not it is at its default. The top-level `animals` key is still the
    // shorthand shot G0 defined — present and false when the tier is off, absent when it is on —
    // and both are asserted, because a reader may be using either.
    assert!(m["animals"].is_null(), "the G0 shorthand: an absent `animals` key means the tier is on");
    assert_eq!(m["params"]["animals"]["enabled"], true, "and the params section says so positively");
    assert_eq!(m["overrides"], serde_json::json!(["climate.rain_gradient=0"]));
    assert_eq!(m["snapshots"], serde_json::json!([0, 2000]));

    // (2) Animals are in `entities.json` at both snapshots, and their positions are JSON *floats*.
    // `Animal::x` is documented as integer-valued and on this run every one of them is a whole
    // number, so the value is not what breaks an integer reader — the `.0` in the file is.
    let (g0, h0, f0, _) = animals_of(&fixture.join("snap_000000"));
    assert_eq!((g0, h0), (300, 20), "the tier is populated at the first snapshot, not only later");
    assert_eq!(f0, g0 + h0, "every animal position at tick 0 is a JSON float");
    let (g, h, floats, per_patch) = animals_of(&fixture.join("snap_002000"));
    // Shot S11 re-cut this fixture: (9204, 28) was S1's count, before tree crowding read crowns.
    assert_eq!((g, h), (9169, 28), "the committed run's animals at tick 2000");
    assert_eq!(floats, g + h, "every animal position at tick 2000 is a JSON float");

    // (3) The crowding an overlay has to survive. A reader that takes the top of its crowding
    // scale from `disease.grazer_threshold` — twice it, as one has — saturates far below what this
    // run reaches, and the saturated patches are exactly the ones such an overlay exists to show.
    let scale_top = 2 * m["params"]["disease"]["grazer_threshold"].as_u64().unwrap() as u32;
    let busiest = *per_patch.iter().max().unwrap();
    // 95 before the S11 re-cut; the point of the assertion is the ratio to `scale_top`, which is
    // still more than double it, and not the digits.
    assert_eq!((scale_top, busiest), (32, 70));
    assert!(busiest > 2 * scale_top, "the busiest patch is far over a scale built from {scale_top}");
    assert_eq!(per_patch.iter().filter(|&&n| n >= scale_top).count(), 7, "patches at or over the scale top");

    // The older fixture is untouched: `capitol-mini` still has no animal in it. This one is an
    // addition and not a flip, because flipping it would move every pixel test that reads it.
    let old = root.join("fixtures/capitol-mini");
    assert_eq!(meta(&old)["animals"], false);
    assert_eq!(animals_of(&old.join("snap_000100")), (0, 0, 0, vec![0; 32 * 32]));
}

/// The two crown fractions are the surveyed ones (shot S3), and this is where that claim is
/// checked against the survey rather than asserted in a comment: the medians of `crown_radius /
/// height` and `crown_base / height` over the 81 trees in the committed Capitol bundle are 0.3040
/// and 0.3684, and `params.toml` carries them to two decimals. The spread is wide (radius sd 0.126)
/// and the median is what a skewed sample of 81 supports, so the tolerance is the rounding, not the
/// scatter. `ecoview-native` holds the same two constants for its own geometry and can read them
/// from `meta.json` now that `params.tree` is published in full (shot S2).
#[test]
fn the_crown_fractions_are_the_surveyed_medians() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let trees: Vec<Value> = serde_json::from_slice(&fs::read(root.join("worlds/capitol/trees.json")).unwrap()).unwrap();
    assert_eq!(trees.len(), 81, "the committed survey");
    let median = |key: &str| {
        let mut v: Vec<f64> = trees.iter().map(|t| t[key].as_f64().unwrap() / t["height"].as_f64().unwrap()).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let p = Params::load(&root.join("params.toml")).unwrap();
    for (key, got, name) in [
        ("crown_radius", p.tree.crown_radius_frac, "crown_radius_frac"),
        ("crown_base", p.tree.crown_base_frac, "crown_base_frac"),
    ] {
        let want = median(key);
        assert!((f64::from(got) - want).abs() <= 0.005, "{name} is {got}, the survey median is {want:.4}");
    }
}

/// What shot S3 set out to publish, on the world it was measured on. Every tree of the committed
/// Capitol fixture carries a height, a crown radius and a crown light; the light is a fraction of
/// full sun; and it takes **many** values across the 79 planted survey trees, where the simulator's
/// own `light.bin` takes exactly one per stage. The fixture is the real surveyed arrangement, so
/// most of its trees stand in the open: the median is full sun and only the clustered ones are
/// shaded, which is the shape a photograph of the site has.
#[test]
fn the_capitol_fixture_publishes_a_crown_light_that_varies() {
    // Under `ECOSIM_REGEN_MANIFEST=1` the fixture this reads is being rewritten by
    // `the_committed_capitol_mini_fixture_matches_a_fresh_run` in the same binary, and the two race.
    // A re-cut is one commit's worth of work; every ordinary run, CI included, checks the file.
    if std::env::var_os("ECOSIM_REGEN_MANIFEST").is_some() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snap = root.join("fixtures/capitol-mini/snap_000000");
    let v: Vec<Value> = serde_json::from_slice(&fs::read(snap.join("entities.json")).unwrap()).unwrap();
    let trees: Vec<&Value> = v.iter().filter(|e| e["kind"] == "tree").collect();
    assert!(trees.len() > 50, "{} trees", trees.len());
    let mut lights: Vec<f64> = Vec::new();
    for t in &trees {
        let (h, r, l) = (
            t["height_m"].as_f64().unwrap(),
            t["crown_radius_m"].as_f64().unwrap(),
            t["crown_light"].as_f64().unwrap(),
        );
        assert!(h > 0.0 && h <= 20.0, "height {h}");
        assert!((r - 0.30 * h).abs() < 0.01, "crown radius {r} of a {h} m tree");
        assert!((0.0..=1.0).contains(&l), "crown light {l}");
        lights.push(l);
    }
    lights.sort_by(f64::total_cmp);
    lights.dedup();
    assert!(lights.len() >= 10, "only {} distinct crown lights: {lights:?}", lights.len());
    assert_eq!(*lights.last().unwrap(), 1.0, "a tree in the open is in full sun");
    assert!(lights[0] < 0.9, "and the most crowded one is not: {}", lights[0]);
}
