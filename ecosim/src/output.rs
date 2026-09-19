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

/// `meta.json` format version; readers reject any other value.
pub const FORMAT_VERSION: u32 = 1;

/// The first line of `series.csv`. The last ten columns are that tick's deaths by species and cause,
/// grazers then hunters, causes in `Cause` order.
pub const SERIES_HEADER: &str = "tick,grazers,hunters,trees,grass_mean,shrub_mean,moisture_mean,fertility_mean,detritus_total,temperature,hunter_immigrants,grazer_starved,grazer_eaten,grazer_old_age,grazer_crowded,grazer_burnt,hunter_starved,hunter_eaten,hunter_old_age,hunter_crowded,hunter_burnt";

/// Number of fields in a `series.csv` line.
pub const SERIES_FIELDS: usize = 21;

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
    /// The `--set key=value` strings applied on top of the params file, in order.
    overrides: &'a [String],
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
    lifespan: u32,
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

/// Snapshot directory name for a tick: `snap_NNNNNN`.
pub fn snapshot_dir_name(tick: u32) -> String {
    format!("snap_{tick:06}")
}

/// One `series.csv` data line (no newline).
pub fn format_row(r: &StatsRow) -> String {
    let mut line = format!(
        "{},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{}",
        r.tick,
        r.grazers,
        r.hunters,
        r.trees,
        r.grass_mean,
        r.shrub_mean,
        r.moisture_mean,
        r.fertility_mean,
        r.detritus_total,
        r.temperature,
        r.hunter_immigrants
    );
    for n in r.deaths.iter().flatten() {
        let _ = write!(line, ",{n}");
    }
    line
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
            lifespan: t.lifespan,
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

/// Write the current state as `snap_NNNNNN/` under `run_dir`.
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

/// Write `meta.json`: format version, dimensions, run settings, species colours, params and overrides.
pub fn write_meta(
    sim: &Sim,
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    overrides: &[String],
    run_dir: &Path,
) -> io::Result<()> {
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
        overrides,
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

/// What `run` returns besides the files it wrote.
pub struct RunSummary {
    /// Wall time of the whole run, including writing.
    pub wall_ms: u128,
    /// Every stats row, tick 0 first.
    pub rows: Vec<StatsRow>,
}

/// The one simulation driver shared by `run` and `sweep`: tick 0 is recorded before any update,
/// then ticks 1..=ticks. `on_tick` sees the state each row is taken from, before it is taken.
pub fn simulate(
    sim: &mut Sim,
    ticks: u32,
    mut on_tick: impl FnMut(&Sim) -> io::Result<()>,
) -> io::Result<Vec<StatsRow>> {
    let mut rows = Vec::with_capacity(ticks as usize + 1);
    loop {
        on_tick(sim)?;
        rows.push(sim.stats());
        if sim.tick >= ticks {
            return Ok(rows);
        }
        sim.step();
    }
}

/// `series.csv` contents for a list of rows.
pub fn series_csv(rows: &[StatsRow]) -> String {
    let mut csv = String::with_capacity((rows.len() + 1) * 80);
    csv.push_str(SERIES_HEADER);
    csv.push('\n');
    for r in rows {
        let _ = writeln!(csv, "{}", format_row(r));
    }
    csv
}

/// Full run into a run directory. `overrides` are only recorded; they must already be applied to `params`.
pub fn run(
    params: Params,
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    overrides: &[String],
    out: &Path,
) -> io::Result<RunSummary> {
    assert!(snapshot_every > 0, "snapshot_every must be > 0");
    let start = Instant::now();
    prepare_dir(out)?;
    let mut sim = Sim::new(params, seed);
    write_meta(&sim, seed, ticks, snapshot_every, overrides, out)?;
    let rows = simulate(&mut sim, ticks, |s| {
        if s.tick % snapshot_every == 0 {
            write_snapshot(s, out)?;
        }
        Ok(())
    })?;
    fs::write(out.join("series.csv"), series_csv(&rows))?;
    let wall_ms = start.elapsed().as_millis();
    fs::write(out.join("timing.json"), format!("{{\"wall_ms\":{wall_ms}}}"))?;
    Ok(RunSummary { wall_ms, rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::Value;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    type Entity = (u32, String, f32, f32, u8, u32, String, f32, u32);

    /// Everything a snapshot records, in the snapshot's own encoding. Entity tuples are
    /// (id, kind, x, y, z, age, stage-or-state, energy, lifespan); trees have energy 0, animals lifespan 0.
    #[derive(Debug, PartialEq)]
    struct Recorded {
        tick: u32,
        voxels: [Vec<u8>; 2],
        columns: [Vec<u8>; 3],
        patches: Vec<[f32; 4]>,
        entities: Vec<Entity>,
    }

    fn scratch_dir() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "ecosim-snap-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// The test-side reader: parse a snapshot directory back.
    fn read_snapshot(dir: &Path) -> Recorded {
        let name = dir.file_name().unwrap().to_str().unwrap();
        let bin = |f: &str| fs::read(dir.join(f)).unwrap();
        let json = |f: &str| serde_json::from_slice::<Value>(&bin(f)).unwrap();
        let f = |v: &Value| v.as_f64().unwrap() as f32;
        let s = |v: &Value| v.as_str().unwrap().to_string();
        let u = |v: &Value| v.as_u64().unwrap() as u32;
        let (patches, entities) = (json("patches.json"), json("entities.json"));
        let patches = patches
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [f(&p["grass"]), f(&p["shrub"]), f(&p["detritus"]), f(&p["temperature"])]);
        let entities = entities.as_array().unwrap().iter().map(|e| {
            let tree = e["kind"] == "tree";
            let label = if tree { &e["stage"] } else { &e["state"] };
            let energy = if tree { 0.0 } else { f(&e["energy"]) };
            let lifespan = if tree { u(&e["lifespan"]) } else { 0 };
            let (id, kind, z, age) = (u(&e["id"]), s(&e["kind"]), u(&e["z"]) as u8, u(&e["age"]));
            (id, kind, f(&e["x"]), f(&e["y"]), z, age, s(label), energy, lifespan)
        });
        Recorded {
            tick: name.strip_prefix("snap_").unwrap().parse().unwrap(),
            voxels: [bin("material.bin"), bin("light.bin")],
            columns: [bin("moisture.bin"), bin("fertility.bin"), bin("height.bin")],
            patches: patches.collect(),
            entities: entities.collect(),
        }
    }

    /// What the snapshot of `sim` should record, derived from the sim state independently of the
    /// writer: live entities by id (trees, then grazers, then hunters), surface fields rounded to u8
    /// on soil columns and 0 elsewhere.
    fn expected(sim: &Sim) -> Recorded {
        let surface = |field: &[f32]| -> Vec<u8> {
            (0..COLS)
                .map(
                    |c| if sim.world.class[c] == ColClass::Soil { field[c].round().clamp(0.0, 255.0) as u8 } else { 0 },
                )
                .collect()
        };
        let mut entities = Vec::new();
        let mut trees: Vec<_> = sim.trees.iter().filter(|t| t.alive).collect();
        trees.sort_by_key(|t| t.id);
        for t in trees {
            let z = sim.world.height[t.col()] + 1;
            let stage = label(&sim.tree_stage(t));
            entities.push((t.id, "tree".into(), t.x as f32, t.y as f32, z, t.age, stage, 0.0, t.lifespan));
        }
        for (kind, group) in [("grazer", &sim.grazers), ("hunter", &sim.hunters)] {
            let mut v: Vec<_> = group.iter().filter(|a| a.alive).collect();
            v.sort_by_key(|a| a.id);
            for a in v {
                let z = sim.world.height[Sim::animal_col(a)] + 1;
                entities.push((a.id, kind.into(), a.x, a.y, z, a.age, label(&a.state), a.energy, 0));
            }
        }
        Recorded {
            tick: sim.tick,
            voxels: [sim.world.material.clone(), sim.world.light.clone()],
            columns: [surface(&sim.moisture), surface(&sim.fertility), sim.world.height.clone()],
            patches: sim.patches.iter().map(|p| [p.grass, p.shrub, p.detritus, p.temperature]).collect(),
            entities,
        }
    }

    /// The lowercase name a stage or state serializes to.
    fn label(v: &impl Serialize) -> String {
        serde_json::to_value(v).unwrap().as_str().unwrap().to_string()
    }

    /// Snapshots are lossy by design (see DECISIONS.md), so the round trip is: write, read back,
    /// and compare with every field the format records.
    fn snapshot_round_trips(seed: u64, ticks: u32) -> Result<(), TestCaseError> {
        let mut sim = Sim::new(Params::load_default(), seed);
        while sim.tick < ticks {
            sim.step();
        }
        let run_dir = scratch_dir();
        write_snapshot(&sim, &run_dir).unwrap();
        let got = read_snapshot(&run_dir.join(snapshot_dir_name(sim.tick)));
        fs::remove_dir_all(&run_dir).unwrap();
        prop_assert_eq!(got.voxels[0].len(), WX * WY * WZ);
        prop_assert!(got == expected(&sim), "snapshot at tick {} of seed {} does not read back", ticks, seed);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(12)))]

        #[test]
        fn prop_snapshot_round_trips(seed in any::<u64>(), ticks in 0u32..400) {
            snapshot_round_trips(seed, ticks)?;
        }
    }

    #[test]
    fn snapshot_regression_after_first_tree_update() {
        snapshot_round_trips(42, 50).unwrap();
    }
}
