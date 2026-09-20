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

/// The params a bundle run uses here: the crate's own with animals off (the garden series runs
/// without them) and the bundle's dimensions.
fn bundle_params(b: &Bundle) -> (Params, Vec<String>) {
    let (mut p, set) = params(&["animals.enabled=false"]);
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
    fs::remove_dir_all(&dir).unwrap();
}

/// `ecosim run --world` builds the bundle world, and the format version and `--world` go together.
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
            .args(["--set", "animals.enabled=false"])
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
    assert_eq!(m["params"]["bundle"], Value::Null, "[bundle] is left out of meta.json at its defaults");

    let mismatch = run(&["--world", dir.to_str().unwrap(), "--format-version", "3"], &tmp("bundle_cli_bad"));
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("use it with --world"));
    let lonely = run(&["--format-version", "4"], &tmp("bundle_cli_bad2"));
    assert!(!lonely.status.success());
    assert!(String::from_utf8_lossy(&lonely.stderr).contains("use it with --world"));
    let missing = run(&["--world", "no-such-bundle"], &tmp("bundle_cli_bad3"));
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("bundle.json"));
    fs::remove_dir_all(&dir).unwrap();
}

/// A noise world is untouched by the bundle work: it still writes format version 3, with no
/// `world/` directory and no `world` key in `meta.json`.
#[test]
fn a_noise_run_still_writes_format_3_without_a_world_directory() {
    let out = tmp("bundle_noise");
    let (p, set) = params(&["world.width=64", "climate.rain_gradient=0", "world.slope_bias=0"]);
    run_with(p, 1, 100, 50, &set, &out, RunOptions::default()).unwrap();
    let m = meta(&out);
    assert_eq!(m["format_version"], 3);
    assert_eq!(m["world"], Value::Null);
    assert!(!out.join("world").exists());
    fs::remove_dir_all(&out).unwrap();
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
    let diff = ecosim::check::diff_runs(&root.join("fixtures/capitol-mini"), &out).unwrap();
    assert_eq!(diff, Vec::<String>::new());
    fs::remove_dir_all(&out).unwrap();
}
