//! Run directory writer: meta.json, series.csv, snap_NNNNNN/, timing.json.

use crate::animals::{Animal, State};
use crate::params::Params;
use crate::sim::{Sim, StatsRow};
use crate::trees::Stage;
use crate::world::{ColClass, COLS, WX, WY, WZ};
use serde::Serialize;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

pub const FORMAT_VERSION: u32 = 1;

pub const SERIES_HEADER: &str =
    "tick,grazers,hunters,trees,grass_mean,shrub_mean,moisture_mean,fertility_mean,detritus_total,temperature";

#[derive(Serialize)]
struct Dims {
    x: usize,
    y: usize,
    z: usize,
}

#[derive(Serialize)]
struct Species {
    id: u32,
    name: &'static str,
    kind: &'static str,
    color: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    canopy_color: Option<&'static str>,
}

#[derive(Serialize)]
struct Meta<'a> {
    format_version: u32,
    dims: Dims,
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    year_len: u32,
    water_level: u8,
    snapshots: Vec<u32>,
    species: Vec<Species>,
    params: &'a Params,
}

fn species_list() -> Vec<Species> {
    let s = |id, name, kind, color, canopy_color| Species { id, name, kind, color, canopy_color };
    vec![
        s(0, "grass", "cover", "#7cc242", None),
        s(1, "shrub", "cover", "#2f6b2a", None),
        s(2, "tree", "tree", "#6b4a2b", Some("#2e8b3d")),
        s(3, "grazer", "animal", "#f2d024", None),
        s(4, "hunter", "animal", "#d6332a", None),
    ]
}

#[derive(Serialize)]
struct TreeOut {
    id: u32,
    kind: &'static str,
    x: u8,
    y: u8,
    z: u8,
    age: u32,
    stage: Stage,
}

#[derive(Serialize)]
struct AnimalOut {
    id: u32,
    kind: crate::animals::Kind,
    x: f32,
    y: f32,
    z: u8,
    energy: f32,
    age: u32,
    state: State,
}

#[derive(Serialize)]
#[serde(untagged)]
enum EntityOut {
    Tree(TreeOut),
    Animal(AnimalOut),
}

pub fn snapshot_dir_name(tick: u32) -> String {
    format!("snap_{tick:06}")
}

pub fn format_row(r: &StatsRow) -> String {
    format!(
        "{},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}",
        r.tick,
        r.grazers,
        r.hunters,
        r.trees,
        r.grass_mean,
        r.shrub_mean,
        r.moisture_mean,
        r.fertility_mean,
        r.detritus_total,
        r.temperature
    )
}

fn surface_u8(field: &[f32], sim: &Sim) -> Vec<u8> {
    (0..COLS)
        .map(|c| if sim.world.class[c] == ColClass::Soil { field[c].round().clamp(0.0, 255.0) as u8 } else { 0 })
        .collect()
}

fn entities(sim: &Sim) -> Vec<EntityOut> {
    let mut out = Vec::new();
    let mut trees: Vec<_> = sim.trees.iter().filter(|t| t.alive).collect();
    trees.sort_by_key(|t| t.id);
    for t in trees {
        out.push(EntityOut::Tree(TreeOut {
            id: t.id,
            kind: "tree",
            x: t.x,
            y: t.y,
            z: sim.world.height[t.col()] + 1,
            age: t.age,
            stage: sim.tree_stage(t),
        }));
    }
    for group in [&sim.grazers, &sim.hunters] {
        let mut v: Vec<&Animal> = group.iter().filter(|a| a.alive).collect();
        v.sort_by_key(|a| a.id);
        for a in v {
            out.push(EntityOut::Animal(AnimalOut {
                id: a.id,
                kind: a.kind,
                x: a.x,
                y: a.y,
                z: sim.world.height[Sim::animal_col(a)] + 1,
                energy: a.energy,
                age: a.age,
                state: a.state,
            }));
        }
    }
    out
}

pub fn write_snapshot(sim: &Sim, run_dir: &Path) -> io::Result<()> {
    let dir = run_dir.join(snapshot_dir_name(sim.tick));
    fs::create_dir_all(&dir)?;
    debug_assert_eq!(sim.world.material.len(), WX * WY * WZ);
    fs::write(dir.join("material.bin"), &sim.world.material)?;
    fs::write(dir.join("light.bin"), &sim.world.light)?;
    fs::write(dir.join("moisture.bin"), surface_u8(&sim.moisture, sim))?;
    fs::write(dir.join("fertility.bin"), surface_u8(&sim.fertility, sim))?;
    fs::write(dir.join("height.bin"), &sim.world.height)?;
    fs::write(dir.join("patches.json"), serde_json::to_vec(&sim.patches)?)?;
    fs::write(dir.join("entities.json"), serde_json::to_vec(&entities(sim))?)?;
    Ok(())
}

pub fn write_meta(sim: &Sim, seed: u64, ticks: u32, snapshot_every: u32, run_dir: &Path) -> io::Result<()> {
    let meta = Meta {
        format_version: FORMAT_VERSION,
        dims: Dims { x: WX, y: WY, z: WZ },
        seed,
        ticks,
        snapshot_every,
        year_len: sim.params.climate.year_len,
        water_level: sim.params.world.water_level,
        snapshots: (0..=ticks).filter(|t| t % snapshot_every == 0).collect(),
        species: species_list(),
        params: &sim.params,
    };
    fs::write(run_dir.join("meta.json"), serde_json::to_vec(&meta)?)
}

/// Prepare an output directory: create it, or clear it if it is a previous run directory.
fn prepare_dir(out: &Path) -> io::Result<()> {
    if out.exists() {
        let is_run = out.join("meta.json").exists();
        let is_empty = fs::read_dir(out)?.next().is_none();
        if !is_run && !is_empty {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists and is not a run directory; refusing to overwrite", out.display()),
            ));
        }
        fs::remove_dir_all(out)?;
    }
    fs::create_dir_all(out)
}

pub struct RunSummary {
    pub wall_ms: u128,
    pub rows: Vec<StatsRow>,
}

/// Full run: tick 0 is recorded before any update, then ticks 1..=ticks.
pub fn run(params: Params, seed: u64, ticks: u32, snapshot_every: u32, out: &Path) -> io::Result<RunSummary> {
    assert!(snapshot_every > 0, "snapshot_every must be > 0");
    let start = Instant::now();
    prepare_dir(out)?;
    let mut sim = Sim::new(params, seed);
    write_meta(&sim, seed, ticks, snapshot_every, out)?;
    let mut rows = Vec::with_capacity(ticks as usize + 1);
    let mut csv = String::with_capacity((ticks as usize + 2) * 80);
    csv.push_str(SERIES_HEADER);
    csv.push('\n');
    loop {
        if sim.tick % snapshot_every == 0 {
            write_snapshot(&sim, out)?;
        }
        let row = sim.stats();
        let _ = writeln!(csv, "{}", format_row(&row));
        rows.push(row);
        if sim.tick >= ticks {
            break;
        }
        sim.step();
    }
    fs::write(out.join("series.csv"), csv)?;
    let wall_ms = start.elapsed().as_millis();
    fs::write(out.join("timing.json"), format!("{{\"wall_ms\":{wall_ms}}}"))?;
    Ok(RunSummary { wall_ms, rows })
}
