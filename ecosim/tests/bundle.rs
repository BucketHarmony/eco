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
