//! The viewer: a world bundle as voxels, a run directory over the top of it, a fly camera, seven
//! overlays, one edit action set, and a remote-control surface an agent can drive.
//!
//! Usage:
//! ```text
//! ecoview-native [--world DIR | --stress] [--run DIR] [--tick N] [--overlay NAME] [--no-cover]
//!                [--eye X,Y,Z] [--look X,Y,Z] [--headless] [--frames N] [--screenshot PATH]
//!                [--bench SECS] [--port N]
//! ```
//!
//! Keys: WASD, Space and Shift to fly; right mouse to look; wheel for speed; **R** to reset the view;
//! **P** to play or pause; **,** and **.** to step a snapshot; **Home** and **End** for the ends of
//! the run; **[** and **]** for the play rate; left mouse on the timeline to scrub; **1**-**7** for
//! the overlay; **V** for the ground cover and vines.
//!
//! The ground cover and the vines are **expression, not simulation** (`src/cover.rs`): the run says
//! how much grass and shrub a patch holds and how wet and shaded its columns are, and the viewer
//! decides only where the blades stand and how far a climber gets. No vine is an entity in any run.

use std::path::Path;
use std::time::{Duration, Instant};

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::image::Image;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::remote::{BrpError, BrpResult, RemotePlugin};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::tasks::ComputeTaskPool;
use bevy::window::{ExitCondition, PrimaryWindow, WindowPlugin};
use bevy::winit::WinitPlugin;
use bevy_brp_extras::BrpExtrasPlugin;
use serde_json::{json, Value};

use ecoview_native::cover::{Cover, CoverStats};
use ecoview_native::mesh::{mesh_chunk, ChunkMesh, Scratch};
use ecoview_native::overlay::{FieldStats, Fields, Scale};
use ecoview_native::palette::{palette, Overlay, BANDS};
use ecoview_native::run::Run;
use ecoview_native::voxel::{ChunkPos, ColumnBands, EditAction, VoxelWorld};
use ecoview_native::{brp, stress_world, Bundle, CAPITOL};

/// Fly speed and its wheel step, copied from `ecoview`'s `edit.ts` so the two viewers feel the same.
const FLY_SPEED: f32 = 12.0;
const FLY_SPEED_MIN: f32 = 1.0;
const FLY_SPEED_MAX: f32 = 120.0;
const FLY_SPEED_STEP: f32 = 1.25;
const MOUSE_SENSITIVITY: f32 = 0.002;

/// Wall-clock seconds per snapshot when the timeline is playing, and the range `[` and `]` cover.
const PLAY_SECS: f32 = 0.5;
const PLAY_SECS_MIN: f32 = 0.05;
const PLAY_SECS_MAX: f32 = 4.0;

/// The timeline bar's place on the screen, in logical pixels from the window's edges. The scrub
/// hit-test recomputes this rather than reading the laid-out node back, so the two must agree; they
/// are the same four constants in both places.
const BAR_MARGIN: f32 = 24.0;
const BAR_BOTTOM: f32 = 24.0;
const BAR_HEIGHT: f32 = 16.0;
const HUD_FONT: f32 = 15.0;

/// The overlay legend: a strip of one swatch per band, sitting just above the timeline bar, with the
/// scale's two ends written beside it. A ramp with no numbers on it is decoration.
const LEGEND_BOTTOM: f32 = BAR_BOTTOM + BAR_HEIGHT + 14.0;
const LEGEND_HEIGHT: f32 = 12.0;
const LEGEND_WIDTH: f32 = 320.0;

#[derive(Resource, Clone)]
struct Args {
    world: String,
    run: Option<String>,
    tick: Option<u64>,
    stress: bool,
    headless: bool,
    frames: u32,
    screenshot: Option<String>,
    bench: f32,
    port: u16,
    overlay: Overlay,
    cover: bool,
    /// Where the camera stands and what it looks at, in metres. Both default to the overview pose
    /// `setup` computes from the site's size.
    ///
    /// A headless screenshot has no one to fly it, so a picture of anything but the whole site --
    /// a wall with a climber on it, a tree at eye level -- could only be taken by hand. A shot
    /// report that says "look at this" has to be re-runnable, so the pose is an argument
    /// (DECISIONS.md, V4).
    eye: Option<Vec3>,
    look: Option<Vec3>,
}

/// `x,y,z` in metres. A pose that does not parse is fatal for the same reason a mistyped overlay is:
/// a screenshot script would otherwise file the overview picture under the close-up's name.
fn vec3(s: &str) -> Vec3 {
    let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    assert!(v.len() == 3, "expected x,y,z in metres, got {s:?}");
    Vec3::new(v[0], v[1], v[2])
}

fn args() -> Args {
    let mut a = Args {
        world: CAPITOL.to_string(),
        run: None,
        tick: None,
        stress: false,
        headless: false,
        frames: 300,
        screenshot: None,
        bench: 0.0,
        port: brp::PORT,
        overlay: Overlay::Surface,
        cover: true,
        eye: None,
        look: None,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let next = || argv.get(i + 1).cloned().unwrap_or_default();
        match argv[i].as_str() {
            "--world" => {
                a.world = next();
                i += 1;
            }
            "--run" => {
                a.run = Some(next());
                i += 1;
            }
            "--tick" => {
                a.tick = next().parse().ok();
                i += 1;
            }
            "--stress" => a.stress = true,
            "--headless" => a.headless = true,
            "--frames" => {
                a.frames = next().parse().unwrap_or(300);
                i += 1;
            }
            "--screenshot" => {
                a.screenshot = Some(next());
                i += 1;
            }
            "--bench" => {
                a.bench = next().parse().unwrap_or(0.0);
                i += 1;
            }
            "--port" => {
                a.port = next().parse().unwrap_or(brp::PORT);
                i += 1;
            }
            // A mistyped overlay is fatal rather than ignored: the screenshot scripts pass this, and
            // a silent fall back to the surface map would file the wrong picture under the right name.
            "--overlay" => {
                let n = next();
                a.overlay = Overlay::parse(&n).unwrap_or_else(|| {
                    panic!(
                        "unknown overlay {n}; expected one of {}",
                        Overlay::ALL.map(|o| o.name()).join(", ")
                    )
                });
                i += 1;
            }
            "--no-cover" => a.cover = false,
            "--eye" => {
                a.eye = Some(vec3(&next()));
                i += 1;
            }
            "--look" => {
                a.look = Some(vec3(&next()));
                i += 1;
            }
            other => eprintln!("ignoring unknown argument {other}"),
        }
        i += 1;
    }
    a
}

/// The loaded world plus the entity drawing each chunk.
#[derive(Resource)]
struct Site {
    world: VoxelWorld,
    entities: Vec<Option<Entity>>,
    material: Handle<StandardMaterial>,
    /// The colour of every voxel id under the active overlay. Rebuilt when the overlay changes, and
    /// built from the run's `meta.json` when there is one (palette.rs).
    palette: Vec<[f32; 4]>,
    /// Milliseconds the last single-edit remesh took, reported over BRP.
    last_remesh_ms: f64,
    edits: u64,
}

/// The queue a BRP `ecoview.edit` call appends to. One explicit method, so an agent never has to
/// discover a component schema (V0-spike.md, build item 5).
#[derive(Resource, Default)]
struct EditQueue(Vec<(usize, usize, EditAction)>);

/// The camera pose a BRP `ecoview.camera` call asks for.
#[derive(Resource, Default)]
struct CameraQueue(Option<(Vec3, Vec3)>);

#[derive(Component)]
struct Fly {
    speed: f32,
    yaw: f32,
    pitch: f32,
}

#[derive(Resource)]
struct Headless {
    frames: u32,
    target: Handle<Image>,
    screenshot: Option<String>,
    shot_taken: bool,
}

#[derive(Resource, Default)]
struct Bench {
    until: f32,
    samples: Vec<f32>,
}

/// The camera pose `setup` chose, so **R** can put it back. A reset key is only useful if there is
/// one pose it always means, so this is written once and never updated by flying.
#[derive(Resource, Clone, Copy)]
struct HomeView {
    eye: Vec3,
    look: Vec3,
}

/// The run directory and where we are in it. `applied` is the snapshot whose trees are currently in
/// the voxel world; `index` is the one the user has asked for. They differ for exactly one frame.
#[derive(Resource, Default)]
struct Timeline {
    run: Option<Run>,
    index: usize,
    applied: Option<usize>,
    playing: bool,
    secs: f32,
    accum: f32,
    scrubbing: bool,
    /// What the last snapshot change cost: chunks remeshed and milliseconds, reported over BRP.
    last_chunks: usize,
    last_ms: f64,
    trees: usize,
    unknown_stage: usize,
    /// Shot V3's tree model, as the snapshot reported it: how many trees the simulator calls
    /// sapling, young and mature, the height span its ages map to, the light their crowns get, and
    /// how many voxels of wood and leaf that came to. All of it printed, because a procedural tree
    /// is the one thing on screen a reader cannot check against the run by eye.
    stages: [usize; 3],
    height: (f32, f32, f32),
    light: (f32, f32, f32),
    light_source: String,
    wood: usize,
    leaves: usize,
    /// Shot V4's cover layer. `cover` is what the user asked for, **V** or `--no-cover`;
    /// `cover_applied` is the `(ground, vines)` the drawn world actually has, which is not the same
    /// thing, because a field overlay takes the ground cover off without the user asking.
    cover: bool,
    cover_applied: Option<(bool, bool)>,
    /// Grass, shrub and vine voxels, and the site means of the four run fields that drove them.
    /// Printed wherever the cover is, so no picture of it stands on its own.
    cover_counts: (usize, usize, usize),
    cover_stats: CoverStats,
}

impl Timeline {
    fn count(&self) -> usize {
        self.run.as_ref().map_or(0, |r| r.snapshot_count())
    }

    fn tick(&self) -> u64 {
        self.run.as_ref().map_or(0, |r| r.tick_at(self.index))
    }

    /// Moves to snapshot `i`, clamped. Returns false if there is no run or nothing moved.
    fn seek(&mut self, i: usize) -> bool {
        let n = self.count();
        if n == 0 {
            return false;
        }
        let i = i.min(n - 1);
        if i == self.index {
            return false;
        }
        self.index = i;
        true
    }

    fn step(&mut self, by: i64) -> bool {
        let n = self.count() as i64;
        if n == 0 {
            return false;
        }
        self.seek((self.index as i64 + by).rem_euclid(n) as usize)
    }
}

/// Which overlay is on, what its scale is, and what the field it draws actually held.
///
/// `applied` is the `(overlay, snapshot)` pair whose bands are in the voxel world. It differs from
/// the pair the user has asked for for exactly one frame, the same way `Timeline::applied` does.
#[derive(Resource)]
struct OverlayState {
    active: Overlay,
    applied: Option<(Overlay, usize)>,
    /// The ramp's two ends and where they came from, from the run's `meta.json`. `None` until a
    /// field overlay is on with a run behind it.
    scale: Option<Scale>,
    stats: Option<FieldStats>,
    /// Patches alight, and patches burnt out since the previous snapshot.
    fire: (usize, usize),
    /// Why the last overlay could not be drawn, if it could not.
    error: Option<String>,
}

impl OverlayState {
    fn new(active: Overlay) -> OverlayState {
        OverlayState {
            active,
            applied: None,
            scale: None,
            stats: None,
            fire: (0, 0),
            error: None,
        }
    }
}

/// Reads the snapshot's fields, bands them and puts them on the world. Returns the stale chunks.
///
/// Everything that can go wrong here -- a missing field file, a short one, a `patches.json` of the
/// wrong length -- takes the overlay off and puts the reason on the screen. A viewer that draws a
/// stale map after a failed read is worse than one that admits it is showing the surface.
fn apply_overlay_bands(
    world: &mut VoxelWorld,
    ov: &mut OverlayState,
    t: &Timeline,
    fields: Option<&Result<Fields, String>>,
) -> Vec<ChunkPos> {
    ov.scale = None;
    ov.stats = None;
    ov.fire = (0, 0);
    ov.error = None;
    let stale = match (&t.run, ov.active.is_field()) {
        (Some(run), true) => {
            let scale = Scale::of(ov.active, &run.meta);
            let out = match fields {
                Some(Ok(f)) => {
                    let d = run.meta.dims;
                    let (bands, stats) = f.bands(ov.active, &d, &scale);
                    ov.stats = Some(stats);
                    ov.fire = f.fire_counts();
                    world.set_overlay(Some(&ColumnBands {
                        x: d.x,
                        y: d.y,
                        bands,
                    }))
                }
                other => {
                    ov.error = Some(match other {
                        Some(Err(e)) => e.clone(),
                        _ => "the snapshot's fields were not read".into(),
                    });
                    world.set_overlay(None)
                }
            };
            ov.scale = Some(scale);
            out
        }
        (None, true) => {
            ov.error = Some("no run loaded -- pass --run DIR for the ecological overlays".into());
            world.set_overlay(None)
        }
        _ => world.set_overlay(None),
    };
    ov.applied = Some((ov.active, t.index));
    stale
}

fn main() {
    let a = args();
    let load = Instant::now();
    let bundle = if a.stress {
        stress_world()
    } else {
        Bundle::load(std::path::Path::new(&a.world))
            .unwrap_or_else(|e| panic!("cannot read world bundle {}: {e}", a.world))
    };
    let mut timeline = Timeline {
        secs: PLAY_SECS,
        cover: a.cover,
        ..default()
    };
    if let Some(run) = open_run(&a, &bundle) {
        if let Some(t) = a.tick {
            timeline.index = run.index_of_tick(t);
        }
        timeline.run = Some(run);
    }
    let voxelise = Instant::now();
    // The chunk grid is sized once, and a run tree taller than the bundle's own tallest survey
    // would be cut off at the top of it. The run says how tall its species ever gets, so ask.
    let headroom = timeline.run.as_ref().map_or(0.0, |r| r.life.tall_height_m);
    let world = VoxelWorld::from_bundle_with_headroom(&bundle, headroom);
    println!(
        "world {} {}x{} cells at {} m, {} chunks, {} levels; read {:.0} ms, voxelise {:.0} ms",
        bundle.name,
        bundle.width,
        bundle.depth,
        bundle.ground_cell_m,
        world.chunk_count(),
        world.levels,
        (voxelise - load).as_secs_f64() * 1000.0,
        voxelise.elapsed().as_secs_f64() * 1000.0
    );

    let mut app = App::new();
    if a.headless {
        app.add_plugins(
            DefaultPlugins
                .build()
                .disable::<WinitPlugin>()
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    close_when_requested: false,
                    ..default()
                }),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::ZERO));
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("ecoview-native: {}", bundle.name),
                resolution: (1280u32, 800u32).into(),
                // Uncapped, so `--bench` measures the renderer and not the 60 Hz display.
                present_mode: bevy::window::PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }));
    }
    app.add_plugins(
        RemotePlugin::default()
            .with_method_main("ecoview.edit", edit_method)
            .with_method_main("ecoview.camera", camera_method)
            .with_method_main("ecoview.stats", stats_method)
            .with_method_main("ecoview.timeline", timeline_method)
            .with_method_main("ecoview.overlay", overlay_method),
    )
    .add_plugins(BrpExtrasPlugin::with_port(a.port))
    .insert_resource(Site {
        world,
        entities: Vec::new(),
        material: Handle::default(),
        palette: palette(Overlay::Surface, None),
        last_remesh_ms: 0.0,
        edits: 0,
    })
    .init_resource::<EditQueue>()
    .init_resource::<CameraQueue>()
    .insert_resource(OverlayState::new(a.overlay))
    .insert_resource(timeline)
    .insert_resource(Bench {
        until: a.bench,
        samples: Vec::new(),
    })
    .insert_resource(a.clone())
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            timeline_keys,
            timeline_scrub,
            timeline_play,
            overlay_keys,
            apply_world_state,
            apply_edits,
            move_camera,
            fly_camera,
            hud,
            bench_frames,
        )
            .chain(),
    );
    if a.headless {
        app.add_systems(Update, headless_frames);
    }
    app.run();
}

/// Finds the run directory: `--run DIR`, or a `run/` directory beside the bundle if one is there.
///
/// An explicit `--run` that cannot be used is fatal, because the alternative is flying a site that
/// silently is not the one asked for. A `run/` that merely happens to sit beside the bundle and does
/// not match it is reported and skipped.
fn open_run(a: &Args, b: &Bundle) -> Option<Run> {
    let (dir, explicit) = match &a.run {
        Some(d) => (std::path::PathBuf::from(d), true),
        None => {
            if a.stress {
                return None;
            }
            let p = Path::new(&a.world).join("run");
            if !p.join("meta.json").exists() {
                return None;
            }
            (p, false)
        }
    };
    let refuse = |what: String| -> Option<Run> {
        if explicit {
            panic!("{what}");
        }
        eprintln!("ignoring the run beside the bundle: {what}");
        None
    };
    let run = match Run::load(&dir) {
        Ok(r) => r,
        Err(e) => return refuse(format!("cannot read run {}: {e}", dir.display())),
    };
    if let Err(e) = run.check_against(b) {
        return refuse(e);
    }
    println!(
        "run {}: {} snapshots, ticks {}..{}, seed {}",
        dir.display(),
        run.snapshot_count(),
        run.tick_at(0),
        run.tick_at(run.snapshot_count() - 1),
        run.meta.seed
    );
    Some(run)
}

/// Reads snapshot `t.index`'s fields once, for the cover and the overlay both.
///
/// The two want the same files, and `light.bin` alone is 2 MB on the Capitol, so reading it twice a
/// scrub would double the one cost a snapshot change is already measured by. The error is kept as a
/// string rather than an `io::Error` so the value can be handed to both callers.
fn read_fields(t: &Timeline) -> Option<Result<Fields, String>> {
    t.run
        .as_ref()
        .map(|r| r.fields_at(t.index).map_err(|e| e.to_string()))
}

/// What the drawn world should hold: the ground cover, and the vines.
///
/// A **field overlay takes the ground cover off**, because an overlay is a map of the ground and a
/// site two thirds under grass would be a map of the grass instead. The vines stay: no overlay
/// colours a wall, and the moisture and light maps are exactly what explains where they are
/// (DECISIONS.md, V4).
fn want_cover(t: &Timeline, ov: &OverlayState) -> (bool, bool) {
    (t.cover && !ov.active.is_field(), t.cover)
}

/// Puts snapshot `t.index`'s trees and its ground cover into the world. Returns the stale chunks.
fn load_snapshot(
    world: &mut VoxelWorld,
    t: &mut Timeline,
    fields: Option<&Fields>,
    want: (bool, bool),
) -> Vec<ChunkPos> {
    let Some(run) = &t.run else {
        return Vec::new();
    };
    let start = Instant::now();
    let snap = match run.trees_at(t.index) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("snapshot {} at tick {}: {e}", t.index, run.tick_at(t.index));
            t.applied = Some(t.index);
            return Vec::new();
        }
    };
    // A run's vegetation replaces the bundle's, rather than adding to it: the bundle's trees are the
    // site as it was photographed and the run's are the same site as the simulator grew it, so
    // drawing both would stand two trees in every spot. Bundle shrubs go with them, for the same
    // reason (DECISIONS.md, "V1 whose trees these are").
    // The cover is built from the snapshot's own fields and the run's own seed, so scrubbing back
    // to a tick puts every blade back exactly where it was. Its means are read off the run whether
    // or not anything is drawn from them, so a picture with the cover switched off still says what
    // the cover would have been.
    let cover = fields.map(|f| {
        let mut c = Cover::of(f, run.meta.dims, run.meta.seed);
        c.ground = want.0;
        c.vines = want.1;
        c
    });
    t.cover_stats = cover.as_ref().map(|c| c.means()).unwrap_or_default();
    let stale = world.set_scene(&snap.trees, &[], cover.as_ref());
    t.cover_applied = Some(want);
    t.cover_counts = world.cover_counts();
    t.trees = snap.trees.len();
    t.unknown_stage = snap.unknown_stage;
    t.stages = snap.stages;
    t.height = snap.height;
    t.light = snap.light;
    t.light_source = snap.light_source.clone();
    (t.wood, t.leaves) = world.plant_counts();
    t.last_chunks = stale.len();
    t.last_ms = start.elapsed().as_secs_f64() * 1000.0;
    t.applied = Some(t.index);
    stale
}

/// Builds every chunk mesh on the compute task pool, then spawns one entity per non-empty chunk.
#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    mut site: ResMut<Site>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut timeline: ResMut<Timeline>,
    mut overlays: ResMut<OverlayState>,
    args: Res<Args>,
) {
    // The run's trees and the overlay both go in before the first mesh, so the site is never drawn
    // with the bundle's vegetation under the wrong palette and then corrected a frame later.
    let fields = read_fields(&timeline);
    let want = want_cover(&timeline, &overlays);
    load_snapshot(
        &mut site.world,
        &mut timeline,
        fields.as_ref().and_then(|f| f.as_ref().ok()),
        want,
    );
    // The tree model on stdout as well as in the HUD: a procedural tree is the one thing in the
    // picture a reader cannot check against the run by eye, so a scripted run leaves the numbers
    // behind it (MEASUREMENTS.md, V3).
    if let Some(run) = &timeline.run {
        println!(
            "trees: {} at tick {} ({} sapling, {} young, {} mature), {:.1}..{:.1} m median {:.1}; crown light {:.2}..{:.2} mean {:.2} from {}; {} wood and {} leaf voxels, voxelised in {:.0} ms; height from age: {}",
            timeline.trees,
            run.tick_at(timeline.index),
            timeline.stages[0],
            timeline.stages[1],
            timeline.stages[2],
            timeline.height.0,
            timeline.height.2,
            timeline.height.1,
            timeline.light.0,
            timeline.light.2,
            timeline.light.1,
            timeline.light_source,
            timeline.wood,
            timeline.leaves,
            timeline.last_ms,
            run.life.source,
        );
        // And the cover, with what it is written next to it. A scripted run's stdout is what the
        // shot report quotes, so the caveat travels with the numbers rather than being remembered.
        let (g, sh, v) = timeline.cover_counts;
        let c = timeline.cover_stats;
        println!(
            "cover: {g} grass, {sh} shrub, {v} vine voxels (ground {}, vines {}); run drivers -- grass {:.3}, shrub {:.3}, water {:.3}, shade {:.3}, vine vigour {:.3}. Expression, not simulation: the run owns those four numbers, the viewer owns only where a blade stands and how far a climber gets; no vine is an entity in any run and nothing here feeds back into the simulation.",
            timeline.cover_applied.map(|a| a.0).unwrap_or(false),
            timeline.cover_applied.map(|a| a.1).unwrap_or(false),
            c.grass,
            c.shrub,
            c.water,
            c.shade,
            c.vigour,
        );
    }
    apply_overlay_bands(&mut site.world, &mut overlays, &timeline, fields.as_ref());
    site.palette = palette(overlays.active, timeline.run.as_ref().map(|r| &r.meta));
    // One line on stdout for a headless or scripted run, so a screenshot is never the only record of
    // what the picture means.
    // Fire's field is the ticks left on a patch that is alight, and most snapshots have none; the
    // patches drawn burnt are a different count and go on the line too, or a run with 155 burn scars
    // in the picture would report "0.00..0.00" and nothing else.
    let fire = if overlays.active == Overlay::Fire {
        format!(
            "; {} patches alight, {} burnt since the previous snapshot",
            overlays.fire.0, overlays.fire.1
        )
    } else {
        String::new()
    };
    match (&overlays.scale, overlays.stats, &overlays.error) {
        (Some(sc), Some(st), _) => println!(
            "overlay {}: {:.2}..{:.2} {} from {}; field {:.2}..{:.2} mean {:.2}{fire}",
            overlays.active.name(),
            sc.lo,
            sc.hi,
            sc.unit,
            sc.source,
            st.min,
            st.max,
            st.mean
        ),
        (_, _, Some(e)) => println!("overlay {}: off -- {e}", overlays.active.name()),
        _ => println!("overlay {}", overlays.active.name()),
    }
    let chunks = site.world.all_chunks();
    let t = Instant::now();
    let built = mesh_all(&site.world, &chunks, &site.palette);
    let mesh_ms = t.elapsed().as_secs_f64() * 1000.0;
    let quads: usize = built.iter().map(|m| m.indices.len() / 6).sum();
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        reflectance: 0.05,
        ..default()
    });
    site.material = material.clone();
    site.entities = vec![None; chunks.len()];
    let mut drawn = 0;
    for (i, m) in built.into_iter().enumerate() {
        if m.is_empty() {
            continue;
        }
        drawn += 1;
        let e = commands
            .spawn((
                Mesh3d(meshes.add(to_bevy_mesh(&m))),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        site.entities[i] = Some(e);
    }
    println!(
        "mesh: {mesh_ms:.0} ms for {} chunks, {drawn} drawn, {quads} quads",
        chunks.len()
    );

    let size = site.world.width as f32 * site.world.cell_m;
    let eye = args
        .eye
        .unwrap_or_else(|| Vec3::new(-0.25 * size, 0.45 * size, -0.25 * size));
    let look = args
        .look
        .unwrap_or_else(|| Vec3::new(0.5 * size, 0.0, 0.5 * size));
    commands.insert_resource(HomeView { eye, look });
    let mut cam = commands.spawn((
        Camera3d::default(),
        // Without this the HUD is invisible headless. `DefaultUiCamera::get` only falls back to a
        // camera whose `RenderTarget` is a *window*, and the headless camera's target is an image,
        // so no root node is ever assigned a camera and the whole UI is laid out nowhere.
        IsDefaultUiCamera,
        Transform::from_translation(eye).looking_at(look, Vec3::Y),
        Fly {
            speed: FLY_SPEED,
            yaw: 0.0,
            pitch: 0.0,
        },
        AmbientLight {
            color: Color::WHITE,
            brightness: 700.0,
            affects_lightmapped_meshes: false,
        },
    ));
    if args.headless {
        // Render to an image instead of a window, the way Bevy's headless_renderer example does.
        let mut image = Image::new_fill(
            bevy::render::render_resource::Extent3d {
                width: 1280,
                height: 800,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            &[0, 0, 0, 255],
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC
            | bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT;
        let handle = images.add(image);
        // The exit below waits for the screenshot file to exist, so a stale file from a previous run
        // makes it exit before the capture is sent ("Failed to send screenshot: sending on a closed
        // channel"). Clear it first: waiting on a path is only a signal if the path starts empty.
        if let Some(p) = &args.screenshot {
            let _ = std::fs::remove_file(p);
        }
        // In Bevy 0.19 the render target is its own component, not a `Camera` field.
        // In Bevy 0.19 the render target is its own component, not a `Camera` field.
        cam.insert(RenderTarget::Image(handle.clone().into()));
        commands.insert_resource(Headless {
            frames: args.frames,
            target: handle,
            screenshot: args.screenshot.clone(),
            shot_taken: false,
        });
    }
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        // The G1 fixed sun: 45 degrees, from the south-west. G9 replaces it with a real path.
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.8, -0.8, 0.0)),
    ));
    // `AmbientLight` is a camera component in 0.19, not a resource, so it goes on the camera above.
    spawn_hud(&mut commands, timeline.count() > 0);
}

#[derive(Component)]
struct HudText;

/// The scrubbed part of the timeline bar: a child whose width is the fraction of the run played.
#[derive(Component)]
struct TimelineFill;

/// The legend strip and its two parts: one swatch per band, and the range written beside them.
#[derive(Component)]
struct LegendRoot;
#[derive(Component)]
struct LegendBand(usize);
#[derive(Component)]
struct LegendLabel;

/// One text block top-left and one bar along the bottom. Both exist whether or not a run is loaded:
/// the text still has the fly speed in it, which is one of the three things V1 was asked to make
/// visible, and an empty bar says plainly that there is no run rather than leaving the screen silent.
fn spawn_hud(commands: &mut Commands, has_run: bool) {
    commands.spawn((
        Text::new(""),
        TextFont {
            // 0.19 made this an enum: a bare f32 is no longer a font size.
            font_size: bevy::text::FontSize::Px(HUD_FONT),
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(BAR_MARGIN),
                bottom: Val::Px(LEGEND_BOTTOM),
                width: Val::Px(LEGEND_WIDTH),
                height: Val::Px(LEGEND_HEIGHT),
                display: Display::None,
                ..default()
            },
            LegendRoot,
        ))
        .with_children(|strip| {
            for b in 0..BANDS {
                strip.spawn((
                    Node {
                        width: Val::Percent(100.0 / BANDS as f32),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK),
                    LegendBand(b),
                ));
            }
        });
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(HUD_FONT),
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(BAR_MARGIN + LEGEND_WIDTH + 12.0),
            bottom: Val::Px(LEGEND_BOTTOM - 4.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            ..default()
        },
        // The label sits over the site rather than over the sky, and under the light overlay the
        // site is white: white text with a shadow on it was unreadable in `shots/v2-light.png`.
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        LegendLabel,
    ));
    // No run, no bar. An empty track along the bottom of the screen offers a scrub that would do
    // nothing; the HUD says so in words instead.
    if !has_run {
        return;
    }
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(BAR_MARGIN),
                right: Val::Px(BAR_MARGIN),
                bottom: Val::Px(BAR_BOTTOM),
                height: Val::Px(BAR_HEIGHT),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.48, 0.71, 0.29)),
                TimelineFill,
            ));
        });
}

/// Meshes a list of chunks in parallel on Bevy's compute pool, one scratch buffer per task.
fn mesh_all(world: &VoxelWorld, chunks: &[ChunkPos], palette: &[[f32; 4]]) -> Vec<ChunkMesh> {
    ComputeTaskPool::get().scope(|s| {
        for &c in chunks {
            s.spawn(async move {
                let mut scratch = Scratch::new();
                mesh_chunk(world, c, palette, &mut scratch)
            });
        }
    })
}

fn to_bevy_mesh(m: &ChunkMesh) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, m.colors.clone())
    .with_inserted_indices(Indices::U32(m.indices.clone()))
}

/// Drains the edit queue: mutate the columns, remesh only the chunks the edit touched.
fn apply_edits(
    mut commands: Commands,
    mut queue: ResMut<EditQueue>,
    mut site: ResMut<Site>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if queue.0.is_empty() {
        return;
    }
    let palette = site.palette.clone();
    let mut scratch = Scratch::new();
    for (x, y, action) in std::mem::take(&mut queue.0) {
        let t = Instant::now();
        let stale = site.world.apply(x, y, action);
        let material = site.material.clone();
        for c in stale {
            let i = site.world.chunk_index(c);
            let m = mesh_chunk(&site.world, c, &palette, &mut scratch);
            sync_chunk(&mut commands, &mut site, &mut meshes, i, &m, &material);
        }
        site.last_remesh_ms = t.elapsed().as_secs_f64() * 1000.0;
        site.edits += 1;
    }
}

/// **P** plays and pauses, **,** and **.** step one snapshot, **Home** and **End** are the ends of the
/// run, **[** and **]** change the play rate.
///
/// Space is not the play key, because Space already flies the camera up and a viewer you cannot rise
/// in is a worse viewer than one whose play key is P.
fn timeline_keys(keys: Res<ButtonInput<KeyCode>>, mut t: ResMut<Timeline>) {
    if t.count() == 0 {
        return;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        t.playing = !t.playing;
    }
    if keys.just_pressed(KeyCode::Comma) {
        t.playing = false;
        t.step(-1);
    }
    if keys.just_pressed(KeyCode::Period) {
        t.playing = false;
        t.step(1);
    }
    if keys.just_pressed(KeyCode::Home) {
        t.playing = false;
        t.seek(0);
    }
    if keys.just_pressed(KeyCode::End) {
        t.playing = false;
        let last = t.count() - 1;
        t.seek(last);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        t.secs = (t.secs * 2.0).clamp(PLAY_SECS_MIN, PLAY_SECS_MAX);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        t.secs = (t.secs * 0.5).clamp(PLAY_SECS_MIN, PLAY_SECS_MAX);
    }
}

/// Left mouse anywhere on the bar jumps to that snapshot, and holding it drags.
///
/// The hit-test recomputes the bar's rectangle from the same four constants `spawn_hud` lays it out
/// with, rather than reading the computed node back: the bar is a fixed rectangle in window space,
/// and one arithmetic expression is easier to keep honest than a query into the layout tree.
fn timeline_scrub(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut t: ResMut<Timeline>,
) {
    if t.count() == 0 {
        return;
    }
    if !buttons.pressed(MouseButton::Left) {
        t.scrubbing = false;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let (w, h) = (window.width(), window.height());
    let top = h - BAR_BOTTOM - BAR_HEIGHT;
    // Once a drag starts on the bar it keeps control until the button comes up, so the snapshot does
    // not stop following the mouse the moment the pointer slips off the 16-pixel strip.
    if !t.scrubbing {
        let on_bar = cursor.y >= top
            && cursor.y <= top + BAR_HEIGHT
            && cursor.x >= BAR_MARGIN
            && cursor.x <= w - BAR_MARGIN;
        if !on_bar {
            return;
        }
        t.scrubbing = true;
        t.playing = false;
    }
    let span = (w - 2.0 * BAR_MARGIN).max(1.0);
    let frac = ((cursor.x - BAR_MARGIN) / span).clamp(0.0, 1.0);
    let i = (frac * (t.count() - 1) as f32).round() as usize;
    t.seek(i);
}

fn timeline_play(time: Res<Time>, mut t: ResMut<Timeline>) {
    if !t.playing || t.count() == 0 {
        return;
    }
    t.accum += time.delta_secs();
    if t.accum < t.secs {
        return;
    }
    t.accum = 0.0;
    t.step(1);
}

/// Brings the drawn world up to the snapshot and the overlay the user has asked for, and remeshes
/// once for both.
///
/// The two were one system rather than two because they overlap: scrubbing under a field overlay
/// changes the trees *and* the ground's bands, and two systems would remesh the surface chunks twice
/// in the same frame.
fn apply_world_state(
    mut commands: Commands,
    mut site: ResMut<Site>,
    mut timeline: ResMut<Timeline>,
    mut overlays: ResMut<OverlayState>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let want = want_cover(&timeline, &overlays);
    let snapshot_moved = timeline.run.is_some() && timeline.applied != Some(timeline.index);
    // Toggling **V**, or turning a field overlay on over a covered site, rewrites the same voxels a
    // scrub does, so it goes down the same path rather than getting one of its own.
    let cover_moved = timeline.run.is_some() && timeline.cover_applied != Some(want);
    let overlay_moved = overlays.applied != Some((overlays.active, timeline.index));
    if !snapshot_moved && !cover_moved && !overlay_moved {
        return;
    }
    let start = Instant::now();
    let repalette = overlays.applied.map(|(o, _)| o) != Some(overlays.active);
    let fields = read_fields(&timeline);
    let mut stale = if snapshot_moved || cover_moved {
        load_snapshot(
            &mut site.world,
            &mut timeline,
            fields.as_ref().and_then(|f| f.as_ref().ok()),
            want,
        )
    } else {
        Vec::new()
    };
    for c in apply_overlay_bands(&mut site.world, &mut overlays, &timeline, fields.as_ref()) {
        if !stale.contains(&c) {
            stale.push(c);
        }
    }
    // A new overlay is a new palette, and a band index that happens not to have moved is still a
    // different colour, so `set_overlay`'s stale set is not enough on its own: everything is remeshed.
    // It costs about as much as the first frame did and happens on a key press, not on a scrub.
    if repalette {
        site.palette = palette(overlays.active, timeline.run.as_ref().map(|r| &r.meta));
        stale = site.world.all_chunks();
    }
    if !stale.is_empty() {
        let pal = site.palette.clone();
        let built = mesh_all(&site.world, &stale, &pal);
        let material = site.material.clone();
        for (c, m) in stale.iter().zip(built) {
            let i = site.world.chunk_index(*c);
            sync_chunk(&mut commands, &mut site, &mut meshes, i, &m, &material);
        }
    }
    timeline.last_chunks = stale.len();
    timeline.last_ms = start.elapsed().as_secs_f64() * 1000.0;
}

/// **1**-**7** pick the overlay, in `Overlay::ALL` order; **V** turns the cover and vines on and off.
fn overlay_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut ov: ResMut<OverlayState>,
    mut t: ResMut<Timeline>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        t.cover = !t.cover;
    }
    const DIGITS: [KeyCode; 7] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
    ];
    for (i, k) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*k) {
            ov.active = Overlay::ALL[i];
        }
    }
}

/// Puts a freshly built mesh on the entity for chunk `i`, spawning or despawning as the chunk gains
/// or loses geometry.
fn sync_chunk(
    commands: &mut Commands,
    site: &mut Site,
    meshes: &mut Assets<Mesh>,
    i: usize,
    m: &ChunkMesh,
    material: &Handle<StandardMaterial>,
) {
    match (site.entities[i], m.is_empty()) {
        (Some(e), false) => {
            commands
                .entity(e)
                .insert(Mesh3d(meshes.add(to_bevy_mesh(m))));
        }
        (Some(e), true) => {
            commands.entity(e).despawn();
            site.entities[i] = None;
        }
        (None, false) => {
            let e = commands
                .spawn((
                    Mesh3d(meshes.add(to_bevy_mesh(m))),
                    MeshMaterial3d(material.clone()),
                ))
                .id();
            site.entities[i] = Some(e);
        }
        (None, true) => {}
    }
}

/// Everything on the screen: the text block, the bar's fill, and the overlay legend.
#[allow(clippy::too_many_arguments)]
fn hud(
    timeline: Res<Timeline>,
    overlays: Res<OverlayState>,
    site: Res<Site>,
    fly: Query<&Fly>,
    mut text: Query<&mut Text, With<HudText>>,
    mut fill: Query<&mut Node, (With<TimelineFill>, Without<LegendRoot>)>,
    mut legend: Query<&mut Node, With<LegendRoot>>,
    mut swatches: Query<(&LegendBand, &mut BackgroundColor)>,
    mut label: Query<
        (&mut Text, &mut Node),
        (
            With<LegendLabel>,
            Without<HudText>,
            Without<LegendRoot>,
            Without<TimelineFill>,
        ),
    >,
) {
    let scale = overlays.scale.as_ref();
    for mut node in &mut legend {
        node.display = if scale.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if scale.is_some() {
        for (band, mut bg) in &mut swatches {
            let c = site.palette[ecoview_native::voxel::ID_COUNT + band.0];
            bg.0 = Color::linear_rgb(c[0], c[1], c[2]);
        }
    }
    let legend_text = match scale {
        Some(s) => format!("{:.2} to {:.2} {}", s.lo, s.hi, s.unit),
        None => String::new(),
    };
    for (mut t, mut node) in &mut label {
        if t.0 != legend_text {
            t.0 = legend_text.clone();
        }
        // The label's own background is padded, so an empty string is still a visible grey tab in
        // the corner of a surface screenshot. Hide the node, not just its text.
        node.display = if scale.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    let speed = fly.iter().next().map_or(FLY_SPEED, |f| f.speed);
    let frac = if timeline.count() > 1 {
        timeline.index as f32 / (timeline.count() - 1) as f32
    } else {
        0.0
    };
    for mut node in &mut fill {
        node.width = Val::Percent(frac * 100.0);
    }
    let mut s = format!("fly {speed:.1} m/s   [wheel] speed  [R] reset view\n");
    s.push_str(&format!(
        "overlay {} ({} of {})   [1-7] switch\n",
        overlays.active.name(),
        Overlay::ALL
            .iter()
            .position(|o| *o == overlays.active)
            .unwrap_or(0)
            + 1,
        Overlay::ALL.len()
    ));
    // Where the scale came from, in words, every frame it is on the screen. The simulator owns these
    // numbers, and the viewer says so rather than asking to be trusted.
    if let Some(sc) = scale {
        s.push_str(&format!(
            "  scale from {}{}\n",
            sc.source,
            if sc.from_meta() { "" } else { "  (!)" }
        ));
        if let Some(st) = overlays.stats {
            s.push_str(&format!(
                "  field {:.2} to {:.2}, mean {:.2} {}\n",
                st.min, st.max, st.mean, sc.unit
            ));
        }
        if overlays.active == Overlay::Fire {
            s.push_str(&format!(
                "  {} patches alight, {} burnt since the last snapshot\n",
                overlays.fire.0, overlays.fire.1
            ));
        }
    }
    if let Some(e) = &overlays.error {
        s.push_str(&format!("  overlay off: {e}\n"));
    }
    // What the cover is, and what it is not, on the screen rather than only in the write-up: a
    // screenshot travels further than a report does, and a viewer that draws vines without saying
    // they are its own has let a picture make a claim the simulator never made.
    if timeline.run.is_some() {
        let (g, sh, v) = timeline.cover_counts;
        let c = timeline.cover_stats;
        s.push_str(&match timeline.cover_applied {
            Some((false, false)) => "cover off   [V] on\n".to_string(),
            applied => format!(
                "cover {g} grass, {sh} shrub, {v} vine voxels{}   [V] off\n  \
                 expression, not simulation -- run drivers: grass {:.2}, shrub {:.2}, \
                 water {:.2}, shade {:.2}; vine vigour {:.2}\n",
                if applied == Some((false, true)) {
                    " (ground cover hidden under the overlay)"
                } else {
                    ""
                },
                c.grass,
                c.shrub,
                c.water,
                c.shade,
                c.vigour,
            ),
        });
    }
    match &timeline.run {
        Some(run) => {
            s.push_str(&format!(
                "tick {} of {}   snapshot {}/{}   {}   {:.1} s/snap\n\
                 {} trees   remesh {} chunks in {:.1} ms\n\
                 [P] play  [,] [.] step  [Home] [End]  [[ ]] rate  drag the bar to scrub",
                timeline.tick(),
                run.meta.ticks,
                timeline.index + 1,
                timeline.count(),
                if timeline.playing {
                    "playing"
                } else {
                    "paused"
                },
                timeline.secs,
                timeline.trees,
                timeline.last_chunks,
                timeline.last_ms,
            ));
            s.push_str(&format!(
                "\n{} sapling, {} young, {} mature   {:.1} to {:.1} m, median {:.1}\n\
                 crown light {:.2} to {:.2}, mean {:.2} of full sun -- {}\n\
                 {} wood and {} leaf voxels   height from age: {}",
                timeline.stages[0],
                timeline.stages[1],
                timeline.stages[2],
                timeline.height.0,
                timeline.height.2,
                timeline.height.1,
                timeline.light.0,
                timeline.light.2,
                timeline.light.1,
                timeline.light_source,
                timeline.wood,
                timeline.leaves,
                run.life.source,
            ));
            if timeline.unknown_stage > 0 {
                s.push_str(&format!(
                    "\n{} trees have a stage this viewer does not know and are not drawn",
                    timeline.unknown_stage
                ));
            }
        }
        None => s.push_str("no run loaded -- pass --run DIR to put a simulation over this site"),
    }
    for mut t in &mut text {
        if t.0 != s {
            t.0 = s.clone();
        }
    }
}

fn move_camera(mut queue: ResMut<CameraQueue>, mut cam: Query<(&mut Transform, &mut Fly)>) {
    let Some((pos, look)) = queue.0.take() else {
        return;
    };
    for (mut t, mut fly) in &mut cam {
        *t = Transform::from_translation(pos).looking_at(look, Vec3::Y);
        let (y, p, _) = t.rotation.to_euler(EulerRot::YXZ);
        fly.yaw = y;
        fly.pitch = p;
    }
}

/// First-person fly: mouse look while the right button is held, WASD, Space and Shift, wheel on speed.
#[allow(clippy::too_many_arguments)]
fn fly_camera(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    home: Option<Res<HomeView>>,
    mut cam: Query<(&mut Transform, &mut Fly)>,
) {
    if windows.is_empty() {
        return;
    }
    for (mut t, mut fly) in &mut cam {
        // **R** puts the camera back where `setup` left it. Ten minutes of real flying is enough to
        // get lost under the terrain with no way back but restarting the viewer.
        if keys.just_pressed(KeyCode::KeyR) {
            if let Some(h) = &home {
                *t = Transform::from_translation(h.eye).looking_at(h.look, Vec3::Y);
                let (y, p, _) = t.rotation.to_euler(EulerRot::YXZ);
                fly.yaw = y;
                fly.pitch = p;
                fly.speed = FLY_SPEED;
            }
        }
        if scroll.delta.y != 0.0 {
            // Wheel away from you is faster. V0 had it the other way round, and the cause is a sign
            // convention, not a preference: it copied `ecoview`'s expression verbatim
            // (`edit.ts:348`, `wheelDeltaY > 0 ? slower : faster`), but a DOM wheel event's `deltaY`
            // is positive scrolling *towards* you while Bevy's `AccumulatedMouseScroll.delta.y` is
            // positive scrolling *away*. Same expression, opposite feel.
            fly.speed = (if scroll.delta.y > 0.0 {
                fly.speed * FLY_SPEED_STEP
            } else {
                fly.speed / FLY_SPEED_STEP
            })
            .clamp(FLY_SPEED_MIN, FLY_SPEED_MAX);
        }
        if buttons.pressed(MouseButton::Right) && motion.delta != Vec2::ZERO {
            fly.yaw -= motion.delta.x * MOUSE_SENSITIVITY;
            fly.pitch = (fly.pitch - motion.delta.y * MOUSE_SENSITIVITY).clamp(-1.54, 1.54);
            t.rotation = Quat::from_euler(EulerRot::YXZ, fly.yaw, fly.pitch, 0.0);
        }
        let mut dir = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            dir += *t.forward();
        }
        if keys.pressed(KeyCode::KeyS) {
            dir += *t.back();
        }
        if keys.pressed(KeyCode::KeyA) {
            dir += *t.left();
        }
        if keys.pressed(KeyCode::KeyD) {
            dir += *t.right();
        }
        if keys.pressed(KeyCode::Space) {
            dir += Vec3::Y;
        }
        if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            dir -= Vec3::Y;
        }
        if dir != Vec3::ZERO {
            t.translation += dir.normalize() * fly.speed * time.delta_secs();
        }
    }
}

/// `--bench SECS`: fly a fixed circle over the site and report the frame times, so the frame-rate
/// gate is measured without a human at the keyboard.
fn bench_frames(
    mut bench: ResMut<Bench>,
    time: Res<Time<Real>>,
    site: Res<Site>,
    mut cam: Query<&mut Transform, With<Fly>>,
    mut exit: MessageWriter<AppExit>,
) {
    if bench.until <= 0.0 {
        return;
    }
    let elapsed = time.elapsed_secs();
    let size = site.world.width as f32 * site.world.cell_m;
    let a = elapsed * 0.5;
    for mut t in &mut cam {
        *t = Transform::from_translation(Vec3::new(
            size * (0.5 + 0.6 * a.cos()),
            size * 0.25,
            size * (0.5 + 0.6 * a.sin()),
        ))
        .looking_at(Vec3::new(size * 0.5, size * 0.05, size * 0.5), Vec3::Y);
    }
    // The first second is warm-up: pipeline compilation and the first present are not frame time.
    if elapsed > 1.0 {
        bench.samples.push(time.delta_secs() * 1000.0);
    }
    if elapsed > bench.until + 1.0 {
        let mut s = std::mem::take(&mut bench.samples);
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean = s.iter().sum::<f32>() / s.len().max(1) as f32;
        let p95 = s[(s.len() as f32 * 0.95) as usize % s.len().max(1)];
        println!(
            "bench: {} frames, mean {mean:.2} ms ({:.1} fps), p95 {p95:.2} ms ({:.1} fps)",
            s.len(),
            1000.0 / mean,
            1000.0 / p95
        );
        exit.write(AppExit::Success);
    }
}

/// `--headless --frames N --screenshot PATH`: render N frames to an image, save it, exit.
///
/// N defaults to 300 because the PBR pipeline compiles in the background: a screenshot at frame 20
/// comes back as the clear colour with every mesh missing, and the same scene at frame 300 is
/// complete. This is measured, not guessed -- see MEASUREMENTS.md.
fn headless_frames(
    mut commands: Commands,
    mut hl: ResMut<Headless>,
    mut frame: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    if *frame < hl.frames {
        return;
    }
    if !hl.shot_taken {
        hl.shot_taken = true;
        if let Some(path) = hl.screenshot.clone() {
            commands
                .spawn(Screenshot::image(hl.target.clone()))
                .observe(save_to_disk(path));
            return;
        }
    }
    // `save_to_disk` finishes on the IO task pool, so exiting a frame later kills the write. Wait for
    // the file to appear, with a frame cap so a failed capture cannot hang the run.
    let written = hl
        .screenshot
        .as_ref()
        .is_none_or(|p| std::fs::metadata(p).is_ok_and(|m| m.len() > 0));
    if written || *frame > hl.frames + 600 {
        exit.write(AppExit::Success);
    }
}

// ---- the remote-control surface ----

fn brp_err(msg: impl Into<String>) -> BrpError {
    BrpError {
        code: bevy::remote::error_codes::INVALID_REQUEST,
        message: msg.into(),
        data: None,
    }
}

/// `ecoview.edit {"x": u, "y": v, "action": "RaiseGround"|..., "medium": u8}`
fn edit_method(In(params): In<Option<Value>>, mut queue: ResMut<EditQueue>) -> BrpResult {
    let p = params.ok_or_else(|| brp_err("ecoview.edit needs {x, y, action}"))?;
    let x = p["x"]
        .as_u64()
        .ok_or_else(|| brp_err("x must be an integer"))? as usize;
    let y = p["y"]
        .as_u64()
        .ok_or_else(|| brp_err("y must be an integer"))? as usize;
    let name = p["action"]
        .as_str()
        .ok_or_else(|| brp_err("action must be a string"))?;
    let medium = p.get("medium").and_then(|m| m.as_u64()).map(|m| m as u8);
    let action = EditAction::parse(name, medium).ok_or_else(|| {
        brp_err(format!(
            "unknown action {name}; expected RaiseGround, LowerGround, SetSurface, RaiseBuilding or LowerBuilding"
        ))
    })?;
    queue.0.push((x, y, action));
    Ok(json!({"queued": {"x": x, "y": y, "action": name}}))
}

/// `ecoview.camera {"pos": [x, y, z], "look_at": [x, y, z]}`
fn camera_method(In(params): In<Option<Value>>, mut queue: ResMut<CameraQueue>) -> BrpResult {
    let p = params.ok_or_else(|| brp_err("ecoview.camera needs {pos, look_at}"))?;
    let vec3 = |k: &str| -> Result<Vec3, BrpError> {
        let a = p[k]
            .as_array()
            .ok_or_else(|| brp_err(format!("{k} must be [x, y, z]")))?;
        if a.len() != 3 {
            return Err(brp_err(format!("{k} must have three numbers")));
        }
        let n = |i: usize| a[i].as_f64().unwrap_or(0.0) as f32;
        Ok(Vec3::new(n(0), n(1), n(2)))
    };
    let (pos, look) = (vec3("pos")?, vec3("look_at")?);
    queue.0 = Some((pos, look));
    Ok(json!({"pos": [pos.x, pos.y, pos.z], "look_at": [look.x, look.y, look.z]}))
}

/// `ecoview.stats` -- what the world is, what the last edit cost, where the timeline is, and which
/// overlay is on with what scale.
fn stats_method(
    In(_): In<Option<Value>>,
    site: Res<Site>,
    t: Res<Timeline>,
    ov: Res<OverlayState>,
) -> BrpResult {
    Ok(json!({
        "cell_m": site.world.cell_m,
        "width": site.world.width,
        "depth": site.world.depth,
        "levels": site.world.levels,
        "chunks": site.world.chunk_count(),
        "drawn_chunks": site.entities.iter().filter(|e| e.is_some()).count(),
        "edits": site.edits,
        "last_remesh_ms": site.last_remesh_ms,
        "run": t.run.as_ref().map(|r| json!({
            "dir": r.dir.display().to_string(),
            "seed": r.meta.seed,
            "ticks": r.meta.ticks,
            "snapshots": r.snapshot_count(),
        })),
        "snapshot": t.index,
        "tick": t.tick(),
        "playing": t.playing,
        "trees": t.trees,
        "tree_model": {
            "sapling": t.stages[0],
            "young": t.stages[1],
            "mature": t.stages[2],
            "unknown_stage": t.unknown_stage,
            "height_m": {"min": t.height.0, "median": t.height.1, "max": t.height.2},
            "crown_light": {"min": t.light.0, "mean": t.light.1, "max": t.light.2},
            "crown_light_source": t.light_source,
            "wood_voxels": t.wood,
            "leaf_voxels": t.leaves,
            "height_curve": t.run.as_ref().map(|r| json!({
                "source": r.life.source,
                "from_meta": r.life.from_meta,
                "year_ticks": r.life.year_ticks,
                "mature_height_m": r.life.mature_height_m,
                "tall_height_m": r.life.tall_height_m,
                "tall_years": r.life.tall_years,
            })),
        },
        // The cover's own numbers, and the sentence that keeps them honest. An agent reading this
        // over BRP gets the same caveat a person reading the HUD does.
        "cover": {
            "on": t.cover,
            "ground_drawn": t.cover_applied.map(|c| c.0),
            "vines_drawn": t.cover_applied.map(|c| c.1),
            "grass_voxels": t.cover_counts.0,
            "shrub_voxels": t.cover_counts.1,
            "vine_voxels": t.cover_counts.2,
            "drivers": {
                "grass": t.cover_stats.grass,
                "shrub": t.cover_stats.shrub,
                "water": t.cover_stats.water,
                "shade": t.cover_stats.shade,
                "vine_vigour": t.cover_stats.vigour,
            },
            "note": "expression, not simulation: the run owns grass, shrub, moisture and light; the viewer owns only where a blade stands and how far a vine climbs. No vine is an entity in any run and nothing here feeds back into the simulation.",
        },
        "snapshot_chunks": t.last_chunks,
        "snapshot_ms": t.last_ms,
        "overlay": ov.active.name(),
        "overlay_bands": site.world.has_overlay().then_some(BANDS),
        "scale": ov.scale.as_ref().map(|s| json!({
            "lo": s.lo,
            "hi": s.hi,
            "unit": s.unit,
            "source": s.source,
            "from_meta": s.from_meta(),
        })),
        "field": ov.stats.map(|s| json!({"min": s.min, "max": s.max, "mean": s.mean})),
        "fire": {"alight": ov.fire.0, "burnt_since_last_snapshot": ov.fire.1},
        "overlay_error": ov.error,
    }))
}

/// `ecoview.overlay {"name": "moisture"}`, or no parameters to read the current one.
///
/// An explicit method for the same reason the timeline has one: an agent that has to discover a
/// component schema retries, and one that is handed a named method does not (MEASUREMENTS.md, the
/// agent loop). The reply carries the scale, so a caller can check what the colours mean without a
/// second call.
fn overlay_method(In(params): In<Option<Value>>, mut ov: ResMut<OverlayState>) -> BrpResult {
    let p = params.unwrap_or(Value::Null);
    if let Some(name) = p.get("name").and_then(|v| v.as_str()) {
        let next = Overlay::parse(name).ok_or_else(|| {
            brp_err(format!(
                "unknown overlay {name}; expected one of {}",
                Overlay::ALL.map(|o| o.name()).join(", ")
            ))
        })?;
        ov.active = next;
    }
    Ok(json!({
        "overlay": ov.active.name(),
        "overlays": Overlay::ALL.map(|o| o.name()),
        "scale": ov.scale.as_ref().map(|s| json!({
            "lo": s.lo, "hi": s.hi, "unit": s.unit, "source": s.source, "from_meta": s.from_meta(),
        })),
    }))
}

/// `ecoview.timeline {"snapshot": i} | {"tick": n} | {"playing": bool} | {"step": n}`
///
/// Every field is optional and they apply in that order, so one call can seek and start playing. An
/// agent gets the same timeline the keyboard does, which is what made V0's agent loop need no
/// retries: a documented method rather than a component schema to discover.
fn timeline_method(In(params): In<Option<Value>>, mut t: ResMut<Timeline>) -> BrpResult {
    let p = params.unwrap_or(Value::Null);
    if t.count() == 0 {
        return Err(brp_err("no run is loaded; start the viewer with --run DIR"));
    }
    if let Some(i) = p.get("snapshot").and_then(|v| v.as_u64()) {
        t.seek(i as usize);
    }
    if let Some(tick) = p.get("tick").and_then(|v| v.as_u64()) {
        let i = t.run.as_ref().map_or(0, |r| r.index_of_tick(tick));
        t.seek(i);
    }
    if let Some(by) = p.get("step").and_then(|v| v.as_i64()) {
        t.step(by);
    }
    if let Some(play) = p.get("playing").and_then(|v| v.as_bool()) {
        t.playing = play;
    }
    Ok(json!({"snapshot": t.index, "tick": t.tick(), "playing": t.playing}))
}
