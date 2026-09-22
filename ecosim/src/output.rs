//! Run directory writer: meta.json, series.csv, events.csv, snap_NNNNNN/, timing.json.

use crate::animals::{Animal, State};
use crate::bundle::Bundle;
use crate::events::{parse_events, Event, EVENTS_FILE, EVENTS_HEADER};
use crate::params::Params;
use crate::plants::PlantImport;
use crate::profile::{lap, Phase, Profile, Profiler};
use crate::sim::{Sim, StatsRow};
use crate::trees::Stage;
use crate::world::ColClass;
use serde::Serialize;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

/// `meta.json` format version of a noise-world run; readers reject any other value. Version 2 added
/// `state.bin` to each snapshot and `forked_from` to `meta.json`; version 3 added `events.csv`.
/// Every other file keeps its version-1 bytes.
pub const FORMAT_VERSION: u32 = 3;

/// `meta.json` format version of a run on a world bundle (`ecosim run --world`). Version 4 adds the
/// run root's `world/` directory and `meta.json`'s `world` object; a noise world never writes it.
pub const BUNDLE_FORMAT_VERSION: u32 = 4;

/// The first line of `series.csv`. Columns 11–20 are that tick's deaths by species and cause,
/// grazers then hunters, causes in `Cause` order; 21–22 are the fire columns; then the mean and
/// standard deviation of each heritable trait, grazers then hunters (`TraitStats` order); the last
/// 6 are the water columns (shot G4), world means in millimetres, all zero when the water tier is
/// off. `rain_mm`, `runoff_mm`, `drainage_mm` and `outflow_mm` are this tick's amounts;
/// `ponded_mm` and `soil_water_mm` are what is in store at the end of it.
pub const SERIES_HEADER: &str = "tick,grazers,hunters,trees,grass_mean,shrub_mean,moisture_mean,fertility_mean,detritus_total,temperature,hunter_immigrants,grazer_starved,grazer_eaten,grazer_old_age,grazer_crowded,grazer_burnt,hunter_starved,hunter_eaten,hunter_old_age,hunter_crowded,hunter_burnt,patches_burning,total_burnt,grazer_energy_cost_mult_mean,grazer_energy_cost_mult_sd,grazer_flee_distance_mean,grazer_flee_distance_sd,grazer_repro_threshold_mean,grazer_repro_threshold_sd,hunter_energy_cost_mult_mean,hunter_energy_cost_mult_sd,hunter_flee_distance_mean,hunter_flee_distance_sd,hunter_repro_threshold_mean,hunter_repro_threshold_sd,rain_mm,runoff_mm,ponded_mm,soil_water_mm,drainage_mm,outflow_mm";

/// Number of fields in a `series.csv` line.
pub const SERIES_FIELDS: usize = 41;

/// Number of water columns at the end of a `series.csv` line.
pub const WATER_FIELDS: usize = 6;

/// Number of trait columns at the end of a `series.csv` line.
pub const TRAIT_FIELDS: usize = 12;

/// `meta.json`'s `dims`: the world size in columns (x, y) and voxels (z), and the patch edge in
/// columns. Every `.bin` file of a snapshot has x·y·z (material, light) or x·y (the rest) bytes.
#[derive(Serialize)]
struct Dims {
    x: usize,
    y: usize,
    z: usize,
    patch: usize,
}

/// `meta.json`'s `world`: the ground grid the run's `world/` files are on (format version 4).
/// `bundle` says whether it came from a world bundle; a noise world's grid mirrors its ecology
/// grid, and only a bundle run carries terrain a snapshot cannot rebuild, so only a bundle run
/// refuses to fork.
#[derive(Serialize)]
struct WorldMeta<'a> {
    name: &'a str,
    bundle: bool,
    ground_cell_m: f32,
    ground_width: usize,
    ground_depth: usize,
    media: Vec<&'static str>,
    /// Where on Earth the world is, degrees north (shot S9). It comes from the bundle and nothing
    /// in the simulator reads it; it is here so a renderer's sun path is the world's rather than a
    /// constant of its own.
    ///
    /// Written as `null` rather than omitted when the world has no latitude — a noise world, or a
    /// bundle exported before the key existed. Omitting it would leave a reader unable to tell a
    /// run that has no latitude from one written before the key, which is the distinction shot S2
    /// restored for `params` and this key keeps.
    latitude_deg: Option<f64>,
}

impl<'a> WorldMeta<'a> {
    fn of(sim: &'a Sim, bundle: Option<&'a Bundle>) -> WorldMeta<'a> {
        let g = &sim.world.ground_grid;
        WorldMeta {
            name: bundle.map_or("noise", |b| b.name.as_str()),
            bundle: bundle.is_some(),
            ground_cell_m: g.cell_m(),
            ground_width: g.width,
            ground_depth: g.depth,
            media: g.media.iter().map(|m| m.name()).collect(),
            latitude_deg: bundle.and_then(|b| b.latitude_deg),
        }
    }
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

/// One row of `meta.json`'s `overlays`: an ecological overlay, and the two colours a renderer ramps
/// between when it draws it.
///
/// The simulator owns the palette. It already named the species colours; before shot S2 the ramps
/// were the one part of the ecology palette it did not publish, so each renderer carried its own copy
/// of the legend and two renderers of the same run could disagree about what wet ground looks like.
/// `lo` is the colour of the bottom of the scale `params` defines and `hi` the colour of the top;
/// what those two numbers *are* is the renderer's own reading of the fields (SAD 1, "The palette in
/// `meta.json`").
///
/// Surface media and buildings are not here: they are scene geometry, not ecology, and they belong to
/// the world bundle and its renderer (DECISIONS.md, shot G7).
#[derive(Serialize)]
struct OverlayColors {
    name: &'static str,
    lo: &'static str,
    hi: &'static str,
    /// The middle of a diverging ramp, for the overlays that have one (`traits`); absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    mid: Option<&'static str>,
    /// Ground that burnt out since the previous snapshot: a category, not a point on the ramp.
    #[serde(skip_serializing_if = "Option::is_none")]
    burnt: Option<&'static str>,
}

/// The ecological overlays and their ramps, in the order a renderer lists them.
///
/// These seven are the fields and per-species statistics a run writes; `light`, `moisture`,
/// `fertility` and `temperature` come straight from the snapshot files, `crowding` and `fire` are
/// counted per patch, and `traits` is the heritable-trait map (shot 11), which diverges about a
/// neutral middle rather than running one way.
fn overlay_colors() -> Vec<OverlayColors> {
    let o = |name, lo, hi| OverlayColors { name, lo, hi, mid: None, burnt: None };
    vec![
        // Light is a grey level, deliberately: a hue here would be read as a species.
        o("light", "#000000", "#ffffff"),
        o("moisture", "#ffffff", "#1f4fd1"),
        o("fertility", "#ffffff", "#4a2c12"),
        o("temperature", "#2040ff", "#ff3020"),
        o("crowding", "#ffffff", "#d81b9c"),
        OverlayColors { burnt: Some("#2b2b2b"), ..o("fire", "#b3300a", "#ffb020") },
        OverlayColors { mid: Some("#ffffff"), ..o("traits", "#1f5bff", "#ff1f1f") },
    ]
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
    /// `false` when the run left grazers and hunters out (`animals.enabled = false`); absent
    /// otherwise, so a run with animals writes exactly the `meta.json` it always did.
    #[serde(skip_serializing_if = "Option::is_none")]
    animals: Option<bool>,
    /// The bundle the world was loaded from (format version 4); absent for a noise world.
    #[serde(skip_serializing_if = "Option::is_none")]
    world: Option<WorldMeta<'a>>,
    snapshots: Vec<u32>,
    species: Vec<Species>,
    /// The overlay ramps: the rest of the palette the species table starts (shot S2).
    overlays: Vec<OverlayColors>,
    params: &'a Params,
    /// The `--set key=value` strings applied on top of the params file, in order. For a fork, the
    /// strings `ecosim fork` applied from the fork tick on.
    overrides: &'a [String],
    /// The run a fork continues, or null for a run started at tick 0.
    forked_from: Option<&'a ForkedFrom>,
}

/// `meta.json`'s record of where a fork came from.
#[derive(Debug, Clone, Serialize)]
pub struct ForkedFrom {
    /// The parent run directory, as given on the command line (forward slashes).
    pub run: String,
    /// The parent's tick the fork restored; the fork's own rows and snapshots start here.
    pub tick: u32,
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
    /// Height in metres, from the simulator's own age-to-height curve (shot S3). Nothing in a run
    /// said how big a tree was before this: `stage` is three buckets and `age` needs the curve.
    height_m: f32,
    /// Crown radius in metres, `tree.crown_radius_frac` of the height.
    crown_radius_m: f32,
    /// Fraction of full sun the middle of that crown receives
    /// ([`Sim::crown_light`](crate::sim::Sim::crown_light)). This is the per-tree light `light.bin`
    /// could not carry: the field shades a 3x3 m footprint, so it reads one value per stage for
    /// every tree in the world.
    crown_light: f32,
}

/// Four decimals, the precision `series.csv` writes a float at. It keeps `entities.json` compact
/// (three more fields on every tree of every snapshot) and the last bit of an `expf` out of a file
/// the determinism tests compare byte for byte.
fn dp4(v: f32) -> f32 {
    (v * 10_000.0).round() / 10_000.0
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
    energy_cost_mult: f32,
    flee_distance: f32,
    repro_threshold: f32,
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
    let _ = write!(line, ",{},{}", r.patches_burning, r.total_burnt);
    for v in r.traits.iter().flatten() {
        let _ = write!(line, ",{v:.4}");
    }
    let w = &r.water;
    for v in [w.rain_mm, w.runoff_mm, w.ponded_mm, w.soil_water_mm, w.drainage_mm, w.outflow_mm] {
        let _ = write!(line, ",{v:.4}");
    }
    line
}

/// Ponded water as `water.bin` stores it: u16 little-endian, in tenths of a millimetre, saturating.
fn ponded_u16(sim: &Sim) -> Vec<u8> {
    let h = sim.hydro.as_ref().expect("water tier");
    let mut out = Vec::with_capacity(h.ponded.len() * 2);
    for &v in &h.ponded {
        out.extend_from_slice(&((v * 10.0).clamp(0.0, u16::MAX as f64) as u16).to_le_bytes());
    }
    out
}

/// Soil water as `soil_water.bin` stores it: f32 little-endian millimetres, column order.
fn soil_water_f32(sim: &Sim) -> Vec<u8> {
    let h = sim.hydro.as_ref().expect("water tier");
    h.soil.iter().flat_map(|&v| (v as f32).to_le_bytes()).collect()
}

fn surface_u8(field: &[f32], sim: &Sim) -> Vec<u8> {
    (0..sim.world.dims.cols())
        .map(|c| if sim.world.class[c] == ColClass::Soil { field[c].round().clamp(0.0, 255.0) as u8 } else { 0 })
        .collect()
}

fn entities(sim: &Sim) -> Vec<EntityOut> {
    let mut out = Vec::new();
    // Built once for the whole snapshot: every tree's crown light is read against every other's
    // crown, so the geometry is shared rather than rebuilt 4,000 times (shot S3).
    let crowns = sim.crowns();
    let mut trees: Vec<usize> = (0..sim.trees.len()).filter(|&i| sim.trees[i].alive).collect();
    trees.sort_by_key(|&i| sim.trees[i].id);
    for i in trees {
        let t = &sim.trees[i];
        let crown = crowns.of[i];
        out.push(EntityOut::Tree(TreeOut {
            id: t.id,
            kind: "tree",
            x: t.x,
            y: t.y,
            z: sim.world.height[t.col(sim.world.dims)] + 1,
            age: t.age,
            stage: sim.tree_stage(t),
            lifespan: t.lifespan,
            height_m: dp4(crown.height),
            crown_radius_m: dp4(crown.radius),
            crown_light: dp4(sim.crown_light(i, &crowns)),
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
                z: sim.world.height[a.col_index(sim.world.dims)] + 1,
                energy: a.energy,
                age: a.age,
                state: a.state,
                energy_cost_mult: a.traits.energy_cost_mult,
                flee_distance: a.traits.flee_distance,
                repro_threshold: a.traits.repro_threshold,
            }));
        }
    }
    out
}

/// Write the current state as `snap_NNNNNN/` under `run_dir`, with `state.bin` when `state` is set
/// and the two water files when `water` is set (only format version 4 carries them).
pub fn write_snapshot(sim: &Sim, run_dir: &Path, state: bool, water: bool) -> io::Result<()> {
    let dir = run_dir.join(snapshot_dir_name(sim.tick));
    fs::create_dir_all(&dir)?;
    debug_assert_eq!(sim.world.material.len(), sim.world.dims.voxels());
    fs::write(dir.join("material.bin"), &sim.world.material)?;
    fs::write(dir.join("light.bin"), &sim.world.light)?;
    fs::write(dir.join("moisture.bin"), surface_u8(&sim.moisture, sim))?;
    fs::write(dir.join("fertility.bin"), surface_u8(&sim.fertility, sim))?;
    fs::write(dir.join("height.bin"), &sim.world.height)?;
    if let Some(h) = &sim.hydro {
        // The water balance is checked here rather than every tick: it costs one pass over the
        // grids, and every run this ecosim writes takes snapshots. A run whose ledger has stopped
        // closing is wrong, so it fails rather than finishing quietly (shot G4).
        let e = h.balance_error();
        if e.abs() >= WATER_BALANCE_EPS {
            return Err(io::Error::other(format!(
                "tick {}: the water ledger is off by {e:e} relative, over {WATER_BALANCE_EPS:e}",
                sim.tick
            )));
        }
        if water {
            fs::write(dir.join("water.bin"), ponded_u16(sim))?;
            fs::write(dir.join("soil_water.bin"), soil_water_f32(sim))?;
        }
    }
    fs::write(dir.join("patches.json"), serde_json::to_vec(&sim.patches)?)?;
    fs::write(dir.join("entities.json"), serde_json::to_vec(&entities(sim))?)?;
    if state {
        fs::write(dir.join("state.bin"), crate::state::encode(sim))?;
    }
    Ok(())
}

/// How far the water ledger may be from closing at a snapshot, relative to the rain that has
/// fallen. The acceptance of shot G4 asks for 1e-9.
pub const WATER_BALANCE_EPS: f64 = 1e-9;

/// Write `meta.json`: format version, dimensions, run settings, the palette (species colours and
/// overlay ramps), params and overrides.
pub fn write_meta(
    sim: &Sim,
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    overrides: &[String],
    run_dir: &Path,
) -> io::Result<()> {
    let snapshots = (0..=ticks).filter(|t| t % snapshot_every == 0).collect();
    let info = RunInfo {
        format_version: FORMAT_VERSION,
        seed,
        ticks,
        snapshot_every,
        snapshots,
        overrides,
        forked_from: None,
        bundle: None,
    };
    write_meta_info(sim, &info, run_dir)
}

/// The run settings `meta.json` records besides the sim's params.
struct RunInfo<'a> {
    format_version: u32,
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    snapshots: Vec<u32>,
    overrides: &'a [String],
    forked_from: Option<&'a ForkedFrom>,
    /// The bundle a format-4 run was built from.
    bundle: Option<&'a Bundle>,
}

fn write_meta_info(sim: &Sim, info: &RunInfo, run_dir: &Path) -> io::Result<()> {
    let RunInfo { format_version, seed, ticks, snapshot_every, overrides, forked_from, .. } = *info;
    let meta = Meta {
        format_version,
        dims: Dims { x: sim.world.dims.wx, y: sim.world.dims.wy, z: sim.world.dims.wz, patch: sim.world.dims.patch },
        seed,
        ticks,
        snapshot_every,
        year_len: sim.params.climate.year_len,
        water_level: sim.params.world.water_level,
        animals: (!sim.params.animals.enabled).then_some(false),
        world: (format_version == BUNDLE_FORMAT_VERSION).then(|| WorldMeta::of(sim, info.bundle)),
        snapshots: info.snapshots.clone(),
        species: species_list(),
        overlays: overlay_colors(),
        params: &sim.params,
        overrides,
        forked_from,
    };
    fs::write(run_dir.join("meta.json"), serde_json::to_vec(&meta)?)
}

/// Write the run root's `world/` directory (format version 4): the ground grid's height, medium
/// and building fields and the scene's pipes. A bundle run copies its own files, so its bytes are
/// the bundle's; a noise world writes the 1 m grid its columns mirror, and no pipes.
pub fn write_world_dir(world: &crate::world::World, bundle: Option<&Bundle>, run_dir: &Path) -> io::Result<()> {
    if let Some(b) = bundle {
        return b.write_world_dir(run_dir);
    }
    let dir = run_dir.join("world");
    fs::create_dir_all(&dir)?;
    let f32s = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    fs::write(dir.join("ground_h.bin"), f32s(&world.ground_h))?;
    fs::write(dir.join("medium.bin"), &world.ground_grid.medium)?;
    fs::write(dir.join("building_h.bin"), f32s(&world.building_h))?;
    fs::write(dir.join("pipes.json"), b"[]")
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
    /// What the bundle's scene planted (shot G3); all zeroes for a noise world.
    pub import: PlantImport,
}

/// The one simulation driver shared by `run` and `sweep`: tick 0 is recorded before any update,
/// then ticks 1..=ticks. `on_tick` sees the state each row is taken from, before it is taken, and
/// may drain `Sim::events`.
pub fn simulate(
    sim: &mut Sim,
    ticks: u32,
    mut on_tick: impl FnMut(&mut Sim) -> io::Result<()>,
) -> io::Result<Vec<StatsRow>> {
    simulate_profiled(sim, ticks, None, |s, _| on_tick(s))
}

/// [`simulate`], charging the step phases and the stats rows to `prof` when given; `on_tick` gets
/// the profiler too, to charge its own work.
pub fn simulate_profiled(
    sim: &mut Sim,
    ticks: u32,
    mut prof: Option<&mut Profiler>,
    mut on_tick: impl FnMut(&mut Sim, Option<&mut Profiler>) -> io::Result<()>,
) -> io::Result<Vec<StatsRow>> {
    let mut rows = Vec::with_capacity(ticks as usize + 1);
    loop {
        on_tick(sim, prof.as_deref_mut())?;
        rows.push(sim.stats());
        lap(&mut prof, Phase::StatsRow);
        if sim.tick >= ticks {
            return Ok(rows);
        }
        sim.step_profiled(prof.as_deref_mut());
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

/// How `run_with` writes a run directory.
#[derive(Debug, Clone, Copy)]
pub struct RunOptions<'a> {
    /// Write `state.bin` into every snapshot (`ecosim run --snapshot-state`).
    pub state: bool,
    /// 3 (`FORMAT_VERSION`) writes `events.csv`; 2 writes the version-2 directory without it
    /// (`ecosim run --format-version 2`), for readers that know only versions 1 and 2; 4
    /// (`BUNDLE_FORMAT_VERSION`) is the bundle world's, and needs `bundle`.
    pub format_version: u32,
    /// The world bundle to build the world from (`ecosim run --world`), instead of noise terrain.
    pub bundle: Option<&'a Bundle>,
}

impl Default for RunOptions<'_> {
    fn default() -> Self {
        RunOptions { state: true, format_version: FORMAT_VERSION, bundle: None }
    }
}

/// Append the events recorded since the last call to `events.csv`, draining them.
fn flush_events(events: &mut Vec<Event>, out: &Path) -> io::Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let mut text = String::with_capacity(events.len() * 32);
    events.iter().for_each(|e| e.write_line(&mut text));
    events.clear();
    let mut f = fs::OpenOptions::new().append(true).open(out.join(EVENTS_FILE))?;
    io::Write::write_all(&mut f, text.as_bytes())
}

/// Full run into a run directory (format version 3), with `state.bin` in every snapshot.
/// `overrides` are only recorded; they must already be applied to `params`.
pub fn run(
    params: Params,
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    overrides: &[String],
    out: &Path,
) -> io::Result<RunSummary> {
    run_with(params, seed, ticks, snapshot_every, overrides, out, RunOptions::default())
}

/// `run` with the given options. Events are appended to `events.csv` at each snapshot and at the end.
pub fn run_with(
    params: Params,
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    overrides: &[String],
    out: &Path,
    opts: RunOptions<'_>,
) -> io::Result<RunSummary> {
    run_inner(params, &RunSpec { seed, ticks, snapshot_every, overrides }, out, opts, None)
}

/// [`run_with`] under a [`Profiler`] (`ecosim run --profile`): the same run directory, plus the
/// wall time of each phase. The profile is returned, not written, so it never enters the run directory.
pub fn run_profiled(
    params: Params,
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    overrides: &[String],
    out: &Path,
    opts: RunOptions<'_>,
) -> io::Result<(RunSummary, Profile)> {
    let dims = crate::world::Dims::of(&params);
    let mut prof = Profiler::start();
    let spec = RunSpec { seed, ticks, snapshot_every, overrides };
    let summary = run_inner(params, &spec, out, opts, Some(&mut prof))?;
    Ok((summary, prof.report(seed, ticks, dims)))
}

/// What a fresh run simulates and records.
struct RunSpec<'a> {
    seed: u64,
    ticks: u32,
    snapshot_every: u32,
    overrides: &'a [String],
}

fn run_inner(
    params: Params,
    spec: &RunSpec,
    out: &Path,
    opts: RunOptions<'_>,
    mut prof: Option<&mut Profiler>,
) -> io::Result<RunSummary> {
    let &RunSpec { seed, ticks, snapshot_every, overrides } = spec;
    assert!(snapshot_every > 0, "snapshot_every must be > 0");
    assert!(
        matches!(opts.format_version, 2 | FORMAT_VERSION | BUNDLE_FORMAT_VERSION),
        "format_version must be 2, {FORMAT_VERSION} or {BUNDLE_FORMAT_VERSION}"
    );
    assert!(
        opts.bundle.is_none() || opts.format_version == BUNDLE_FORMAT_VERSION,
        "a world bundle writes format_version {BUNDLE_FORMAT_VERSION}"
    );
    let start = Instant::now();
    prepare_dir(out)?;
    let (mut sim, import) = match opts.bundle {
        None => (Sim::new(params, seed), PlantImport::default()),
        Some(b) => Sim::from_bundle(params, seed, b).map_err(io::Error::other)?,
    };
    // Version 4 carries the `world/` directory and the water files. A bundle always needs it; so
    // does the water tier, whatever the world. Only `--format-version 2` opts out of both.
    let format_version = match opts.format_version {
        2 => 2,
        _ if opts.bundle.is_some() || sim.hydro.is_some() => BUNDLE_FORMAT_VERSION,
        _ => FORMAT_VERSION,
    };
    if format_version == BUNDLE_FORMAT_VERSION {
        write_world_dir(&sim.world, opts.bundle, out)?;
    }
    let snapshots = (0..=ticks).filter(|t| t % snapshot_every == 0).collect();
    let info = RunInfo {
        format_version,
        seed,
        ticks,
        snapshot_every,
        snapshots,
        overrides,
        forked_from: None,
        bundle: opts.bundle,
    };
    write_meta_info(&sim, &info, out)?;
    sim.log_events = format_version >= 3;
    if sim.log_events {
        fs::write(
            out.join(EVENTS_FILE),
            format!(
                "{EVENTS_HEADER}
"
            ),
        )?;
    }
    lap(&mut prof, Phase::Setup);
    let rows = simulate_profiled(&mut sim, ticks, prof.as_deref_mut(), |s, mut p| {
        if s.tick % snapshot_every == 0 {
            write_snapshot(s, out, opts.state, format_version == BUNDLE_FORMAT_VERSION)?;
            lap(&mut p, Phase::SnapshotWrite);
            flush_events(&mut s.events, out)?;
            lap(&mut p, Phase::Events);
        }
        Ok(())
    })?;
    flush_events(&mut sim.events, out)?;
    lap(&mut prof, Phase::Events);
    fs::write(out.join("series.csv"), series_csv(&rows))?;
    let wall_ms = start.elapsed().as_millis();
    fs::write(out.join("timing.json"), format!("{{\"wall_ms\":{wall_ms}}}"))?;
    lap(&mut prof, Phase::SeriesWrite);
    Ok(RunSummary { wall_ms, rows, import })
}

/// What `ecosim fork` continues and how.
pub struct ForkSpec<'a> {
    /// The parent run directory (format version 3, with `state.bin` in its snapshots).
    pub parent: &'a Path,
    /// The snapshot tick to restore.
    pub at: u32,
    /// `--set key=value` overrides applied on top of the parent's params from the fork tick on.
    pub overrides: &'a [String],
    /// Ticks to run after `at`.
    pub ticks: u32,
}

fn read_meta(run_dir: &Path) -> Result<serde_json::Value, String> {
    let p = run_dir.join("meta.json");
    let text = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_slice(&text).map_err(|e| format!("{}: {e}", p.display()))
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        fs::copy(e.path(), to.join(e.file_name()))?;
    }
    Ok(())
}

/// The fork's params: the parent's, read back exactly from its `meta.json`, plus the fork's overrides.
fn fork_params(parent: &Path, meta: &serde_json::Value, overrides: &[String]) -> Result<(Params, Params), String> {
    let base: Params = serde_json::from_value(meta["params"].clone())
        .map_err(|e| format!("{}: meta.json params: {e}", parent.display()))?;
    // Written back out, the params must print as the parent recorded them: shortest round-trip floats
    // name exactly one f32 each, so this proves no value moved through the JSON float parser.
    let again = serde_json::to_string(&base).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    if again.as_ref() != Some(&meta["params"]) {
        return Err(format!("{}: meta.json params don't read back exactly", parent.display()));
    }
    let mut root = toml::Value::try_from(&base).map_err(|e| format!("params: {e}"))?;
    // Shot S2 removed the patch that put `hunter.handling_ticks` back here: `meta.json` no longer
    // leaves a key out at its off value, so every key an override can name is already in `root`.
    for o in overrides {
        crate::params::apply_override(&mut root, o)?;
    }
    Ok((base, Params::from_value(root)?))
}

/// Continue a run from one of its snapshots into a new run directory, optionally with changed params.
///
/// The fork directory is a complete run directory: the parent's `series.csv` rows and snapshots
/// before `at` are copied verbatim, and so are its `events.csv` rows up to and including tick `at`
/// (the events of the ticks the restored state has already stepped). Everything after is simulated
/// from the restored state.
/// With no overrides and `at + ticks` equal to the parent's length, it differs from the parent only
/// in `meta.json` (and `timing.json`). Returns the summary of the simulated part (rows from `at`).
pub fn fork(spec: &ForkSpec, out: &Path) -> Result<RunSummary, String> {
    let start = Instant::now();
    let parent = spec.parent;
    let meta = read_meta(parent)?;
    let parent_version = meta["format_version"].as_u64().unwrap_or(0) as u32;
    match meta["format_version"].as_u64() {
        Some(3) => {}
        // A noise world at format 4 (the water tier is on): its `world/` files mirror the ecology
        // grid, which the restored snapshot rebuilds, so it forks like a version-3 run.
        Some(4) if meta["world"]["bundle"] != serde_json::Value::Bool(true) => {}
        Some(2) => {
            return Err(format!(
                "{}: format_version 2 run directories have no events.csv, so a fork's event log would \
                 lack the parent's history; rerun it with this ecosim (format_version 3)",
                parent.display()
            ))
        }
        Some(1) => {
            return Err(format!(
                "{}: format_version 1 run directories have no state.bin and can't be forked; \
                 rerun it with this ecosim (format_version 3)",
                parent.display()
            ))
        }
        Some(v) if v == BUNDLE_FORMAT_VERSION as u64 => {
            return Err(format!(
                "{}: format_version {v} runs are built from a world bundle, whose ground grid and building \
                 shade a snapshot does not carry; rerun the bundle with `ecosim run --world`",
                parent.display()
            ))
        }
        v => return Err(format!("{}: unsupported format_version {v:?}", parent.display())),
    }
    let (base, params) = fork_params(parent, &meta, spec.overrides)?;
    let (seed, every) = (meta["seed"].as_u64().unwrap_or(0), meta["snapshot_every"].as_u64().unwrap_or(0) as u32);
    let parent_snaps: Vec<u32> =
        meta["snapshots"].as_array().into_iter().flatten().filter_map(|v| v.as_u64()).map(|t| t as u32).collect();
    if every == 0 || !parent_snaps.contains(&spec.at) {
        return Err(format!("{}: no snapshot at tick {} (snapshot_every {every})", parent.display(), spec.at));
    }
    let snap = parent.join(snapshot_dir_name(spec.at));
    if !snap.join("state.bin").exists() {
        return Err(format!("{}: no state.bin (was the run written with --snapshot-state false?)", snap.display()));
    }
    let csv = fs::read_to_string(parent.join("series.csv")).map_err(|e| format!("{}: {e}", parent.display()))?;
    let events_text = fs::read_to_string(parent.join(EVENTS_FILE)).map_err(|e| format!("{}: {e}", parent.display()))?;
    let events_before: Vec<&str> = events_text
        .lines()
        .skip(1)
        .take_while(|l| l.split(',').next().and_then(|t| t.parse::<u32>().ok()).is_some_and(|t| t <= spec.at))
        .collect();
    parse_events(&events_text).map_err(|e| format!("{}: {e}", parent.display()))?;
    let mut lines = csv.lines();
    if lines.next() != Some(SERIES_HEADER) {
        return Err(format!("{}: series.csv header is not this ecosim's", parent.display()));
    }
    let before: Vec<&str> = lines.take(spec.at as usize).collect();
    if before.len() != spec.at as usize || before.iter().enumerate().any(|(t, l)| !l.starts_with(&format!("{t},"))) {
        return Err(format!("{}: series.csv lacks rows 0..{}", parent.display(), spec.at));
    }
    if let (Ok(a), Ok(b)) = (fs::canonicalize(parent), fs::canonicalize(out)) {
        if a == b {
            return Err("--out must not be the parent run directory".into());
        }
    }

    let mut sim = Sim::restore(base, &snap)?;
    if sim.tick != spec.at {
        return Err(format!("{}: state.bin holds tick {}", snap.display(), sim.tick));
    }
    sim.set_params(params);
    let end = spec.at + spec.ticks;
    let io = |e: io::Error| format!("{}: {e}", out.display());
    prepare_dir(out).map_err(io)?;
    for &t in parent_snaps.iter().filter(|&&t| t < spec.at) {
        copy_dir(&parent.join(snapshot_dir_name(t)), &out.join(snapshot_dir_name(t))).map_err(io)?;
    }
    if parent_version == BUNDLE_FORMAT_VERSION {
        copy_dir(&parent.join("world"), &out.join("world")).map_err(io)?;
    }
    let snapshots =
        parent_snaps.iter().copied().filter(|&t| t < spec.at).chain((spec.at..=end).filter(|t| t % every == 0));
    let from = ForkedFrom { run: parent.to_string_lossy().replace('\\', "/"), tick: spec.at };
    let info = RunInfo {
        format_version: parent_version,
        seed,
        ticks: end,
        snapshot_every: every,
        snapshots: snapshots.collect(),
        overrides: spec.overrides,
        forked_from: Some(&from),
        bundle: None,
    };
    write_meta_info(&sim, &info, out).map_err(io)?;
    let mut log =
        String::with_capacity(EVENTS_HEADER.len() + 1 + events_before.iter().map(|l| l.len() + 1).sum::<usize>());
    for l in std::iter::once(EVENTS_HEADER).chain(events_before) {
        log.push_str(l);
        log.push('\n');
    }
    fs::write(out.join(EVENTS_FILE), log).map_err(io)?;
    sim.log_events = true;
    let rows = simulate(&mut sim, end, |s| {
        if s.tick % every == 0 {
            write_snapshot(s, out, true, true)?;
            flush_events(&mut s.events, out)?;
        }
        Ok(())
    })
    .map_err(io)?;
    flush_events(&mut sim.events, out).map_err(io)?;
    let mut text = String::with_capacity(csv.len());
    text.push_str(SERIES_HEADER);
    text.push('\n');
    for l in before {
        text.push_str(l);
        text.push('\n');
    }
    text.push_str(&series_csv(&rows)[SERIES_HEADER.len() + 1..]);
    fs::write(out.join("series.csv"), text).map_err(io)?;
    let wall_ms = start.elapsed().as_millis();
    fs::write(out.join("timing.json"), format!("{{\"wall_ms\":{wall_ms}}}")).map_err(io)?;
    // A fork continues a noise run: `ecosim fork` rejects the bundle format outright.
    Ok(RunSummary { wall_ms, rows, import: PlantImport::default() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::diff_runs;
    use proptest::prelude::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn scratch_dir() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "ecosim-fork-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn meta(dir: &Path) -> serde_json::Value {
        read_meta(dir).unwrap()
    }

    /// A fork at `at` with no overrides, run to the parent's end, differs from the parent only in
    /// `meta.json`, and that only by `forked_from`.
    fn fork_equals_uninterrupted(seed: u64, ticks: u32, every: u32, at: u32) -> Result<(), TestCaseError> {
        let (parent, child) = (scratch_dir(), scratch_dir());
        run(Params::load_square(), seed, ticks, every, &[], &parent).unwrap();
        let s = fork(&ForkSpec { parent: &parent, at, overrides: &[], ticks: ticks - at }, &child).unwrap();
        prop_assert_eq!(s.rows.len() as u32, ticks - at + 1);
        prop_assert_eq!(diff_runs(&parent, &child).unwrap(), vec!["differs: meta.json".to_string()]);
        let (mut a, mut b) = (meta(&parent), meta(&child));
        prop_assert_eq!(b["forked_from"]["tick"].as_u64(), Some(at as u64));
        prop_assert!(a["forked_from"].is_null());
        a.as_object_mut().unwrap().remove("forked_from");
        b.as_object_mut().unwrap().remove("forked_from");
        prop_assert_eq!(a, b);
        fs::remove_dir_all(&parent).unwrap();
        fs::remove_dir_all(&child).unwrap();
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(8)))]

        #[test]
        fn prop_fork_equals_uninterrupted(seed in any::<u64>(), k in 0u32..=8) {
            fork_equals_uninterrupted(seed, 400, 50, k * 50)?;
        }
    }

    /// Forking at the last tick simulates nothing but the tick-400 row and snapshot.
    #[test]
    fn fork_regression_at_first_and_last_snapshot() {
        fork_equals_uninterrupted(5, 400, 50, 0).unwrap();
        fork_equals_uninterrupted(5, 400, 50, 400).unwrap();
    }

    /// Overrides apply from the fork tick on: the rows before it are the parent's, the run after it
    /// differs, and `meta.json` records the overrides, the new params and where the fork came from.
    #[test]
    fn fork_with_overrides_changes_only_the_future() {
        let (parent, child) = (scratch_dir(), scratch_dir());
        run(Params::load_square(), 3, 600, 100, &[], &parent).unwrap();
        let set = ["grazer.energy_cost=10".to_string()];
        let s = fork(&ForkSpec { parent: &parent, at: 200, overrides: &set, ticks: 400 }, &child).unwrap();
        let (pa, ch) = (
            fs::read_to_string(parent.join("series.csv")).unwrap(),
            fs::read_to_string(child.join("series.csv")).unwrap(),
        );
        let (pa, ch): (Vec<&str>, Vec<&str>) = (pa.lines().collect(), ch.lines().collect());
        assert_eq!(pa.len(), ch.len());
        assert_eq!(pa[..=201], ch[..=201], "rows up to and including the fork tick are unchanged");
        assert_ne!(pa[601], ch[601]);
        assert_eq!(s.rows.last().unwrap().grazers, 0, "grazers starve at energy cost 10");
        let m = meta(&child);
        assert_eq!(m["overrides"], serde_json::json!(set));
        assert_eq!(m["params"]["grazer"]["energy_cost"].as_f64(), Some(10.0));
        assert_eq!(m["forked_from"]["run"].as_str().unwrap(), parent.to_string_lossy().replace('\\', "/"));
        assert_eq!(m["ticks"], 600);
        assert_eq!(m["snapshots"], serde_json::json!([0, 100, 200, 300, 400, 500, 600]));
        let diff = diff_runs(&parent, &child).unwrap();
        assert!(!diff.iter().any(|d| d.contains("snap_000100") || d.contains("snap_000000")), "{diff:?}");
        assert!(diff.contains(&"differs: snap_000600/state.bin".to_string()), "{diff:?}");
        // A fork of a fork: the chain continues from the child's own snapshots.
        let grandchild = scratch_dir();
        fork(&ForkSpec { parent: &child, at: 400, overrides: &[], ticks: 200 }, &grandchild).unwrap();
        assert_eq!(diff_runs(&child, &grandchild).unwrap(), vec!["differs: meta.json".to_string()]);
        for d in [parent, child, grandchild] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    /// Handling time can be switched on by a fork: the fork records it, its `state.bin` files from
    /// the fork tick on (written with the fork's params) are version 4, and a fork of that fork
    /// continues exactly.
    ///
    /// The parent used to carry *no* `handling_ticks` key at all -- it was skipped at 0 -- and
    /// `fork_params` patched it back in so an override had something to change. Shot S2 publishes the
    /// 0, so the assertion below is now the plain reading of the file rather than the absence the
    /// patch worked around.
    #[test]
    fn fork_can_switch_handling_on() {
        let (parent, child, grandchild) = (scratch_dir(), scratch_dir(), scratch_dir());
        // The water tier is off here: with it on every `state.bin` is version 5, whatever handling
        // does, and this test is about the handling section.
        let mut p = Params::load_square();
        p.hydro.enabled = false;
        run(p, 3, 400, 100, &[], &parent).unwrap();
        assert_eq!(meta(&parent)["params"]["hunter"]["handling_ticks"], 0, "handling off is written, not omitted");
        let set = ["hunter.handling_ticks=60".to_string()];
        fork(&ForkSpec { parent: &parent, at: 100, overrides: &set, ticks: 300 }, &child).unwrap();
        assert_eq!(meta(&child)["params"]["hunter"]["handling_ticks"], 60);
        let version = |d: &Path, t: &str| fs::read(d.join(t).join("state.bin")).unwrap()[8];
        assert_eq!((version(&parent, "snap_000100"), version(&child, "snap_000100")), (3, 4));
        fork(&ForkSpec { parent: &child, at: 200, overrides: &[], ticks: 200 }, &grandchild).unwrap();
        assert_eq!(diff_runs(&child, &grandchild).unwrap(), vec!["differs: meta.json".to_string()]);
        for d in [parent, child, grandchild] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn fork_refuses_what_it_cannot_continue() {
        let err = |parent: &Path, at: u32, set: &[String], out: &Path| {
            fork(&ForkSpec { parent, at, overrides: set, ticks: 10 }, out).err().expect("fork should fail")
        };
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/s42-mini");
        let out = scratch_dir();
        let e = err(&fixture, 100, &[], &out);
        assert!(e.contains("format_version 1") && e.contains("can't be forked"), "{e}");
        assert!(!out.exists());

        let parent = scratch_dir();
        run(Params::load_square(), 1, 100, 50, &[], &parent).unwrap();
        let e = err(&parent, 60, &[], &out);
        assert!(e.contains("no snapshot at tick 60"), "{e}");
        assert!(err(&parent, 50, &["grazer.no_such_key=1".into()], &out).contains("grazer.no_such_key"));
        assert!(err(&parent, 50, &[], &parent).contains("must not be the parent"));
        assert!(!out.exists(), "nothing is written when the fork is refused");

        let stateless = scratch_dir();
        let opts = RunOptions { state: false, ..RunOptions::default() };
        run_with(Params::load_square(), 1, 100, 50, &[], &stateless, opts).unwrap();
        assert!(!stateless.join("snap_000050/state.bin").exists());
        assert_eq!(diff_runs(&parent, &stateless).unwrap().len(), 3, "only the three state.bin files differ");
        assert!(err(&stateless, 50, &[], &out).contains("no state.bin"));

        let mut m = meta(&parent);
        m["format_version"] = 5.into();
        fs::write(parent.join("meta.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(err(&parent, 50, &[], &out).contains("unsupported format_version"));
        // A noise run is format_version 4 too since the water tier (shot G4); it is a bundle run
        // that can't be forked, and `meta.json` says which it is.
        m["format_version"] = BUNDLE_FORMAT_VERSION.into();
        m["world"]["bundle"] = true.into();
        fs::write(parent.join("meta.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(err(&parent, 50, &[], &out).contains("built from a world bundle"));
        m["world"]["bundle"] = false.into();
        m["format_version"] = 2.into();
        fs::write(parent.join("meta.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(err(&parent, 50, &[], &out).contains("no events.csv"));
        m["format_version"] = 3.into();
        m["params"]["grazer"]["energy_cost"] = "cheap".into();
        fs::write(parent.join("meta.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(err(&parent, 50, &[], &out).contains("meta.json params"));
        for d in [parent, stateless] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    /// Every params section is in `meta.json`, named, on an ordinary run at the defaults.
    ///
    /// This is shot S2's second half. Three sections (`bundle`, `animals`, `rng`) and one key
    /// (`hunter.handling_ticks`) used to be skipped when they held their default, so the file said
    /// nothing at all about them in the one case that is most common -- a run at the defaults -- and
    /// a reader could not tell a default from a parameter the run predated. The section list is
    /// written out here rather than derived from the struct, because deriving it from the same serde
    /// impl that does the omitting is what let the defect live: a serializer that skips a section
    /// skips it from the expectation too.
    #[test]
    fn meta_json_names_every_params_section() {
        const SECTIONS: [&str; 19] = [
            "animals", "bundle", "climate", "cover", "disease", "fire", "grass", "grazer", "heredity", "hunter",
            "hydro", "medium", "rain", "rng", "schedule", "season", "shrub", "tree", "world",
        ];
        let dir = scratch_dir();
        run(Params::load_square(), 7, 100, 100, &[], &dir).unwrap();
        let m = meta(&dir);
        let got: Vec<&String> = m["params"].as_object().unwrap().keys().collect();
        assert_eq!(got, SECTIONS.iter().collect::<Vec<_>>(), "params sections in meta.json");
        // The four values that used to be missing, read positively: a viewer can now ask the file
        // whether animals are on instead of inferring it from an absent key.
        assert_eq!(m["params"]["animals"]["enabled"], true);
        assert_eq!(m["params"]["rng"]["stream"], 0);
        assert_eq!(m["params"]["hunter"]["handling_ticks"], 0);
        assert_eq!(m["params"]["bundle"]["base_z"], 8);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// `meta.json` carries the overlay ramps, and each one is a pair of sRGB hex colours.
    ///
    /// Shot S2's first half: the simulator named the species colours from the start, and this is the
    /// rest of the ecology palette. The categorical extras are checked to be on exactly the overlays
    /// that have them -- a `mid` on a one-way ramp, or a missing `burnt` on fire, would each be a
    /// renderer drawing something it invented.
    #[test]
    fn meta_json_carries_the_overlay_ramps() {
        let dir = scratch_dir();
        run(Params::load_square(), 8, 100, 100, &[], &dir).unwrap();
        let m = meta(&dir);
        let rows = m["overlays"].as_array().unwrap();
        let names: Vec<&str> = rows.iter().map(|r| r["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            ["light", "moisture", "fertility", "temperature", "crowding", "fire", "traits"],
            "the ecological overlays, in order"
        );
        let hex = |v: &serde_json::Value| {
            let h = v.as_str().unwrap_or_else(|| panic!("not a string: {v}"));
            assert_eq!(h.len(), 7, "{h}");
            assert!(h.starts_with('#') && h[1..].chars().all(|c| c.is_ascii_hexdigit()), "{h}");
        };
        for r in rows {
            hex(&r["lo"]);
            hex(&r["hi"]);
            let name = r["name"].as_str().unwrap();
            assert_eq!(r.get("mid").is_some(), name == "traits", "mid on {name}");
            assert_eq!(r.get("burnt").is_some(), name == "fire", "burnt on {name}");
            if let Some(mid) = r.get("mid") {
                hex(mid);
            }
            if let Some(b) = r.get("burnt") {
                hex(b);
            }
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
