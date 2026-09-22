//! The viewer: a world bundle as voxels, a run directory over the top of it, a fly camera, seven
//! overlays, an editor under the crosshair, the `ecosim` round trip, and a remote-control surface an
//! agent can drive.
//!
//! Usage:
//! ```text
//! ecoview-native [--world DIR | --stress] [--run DIR] [--tick N] [--overlay NAME] [--no-cover]
//!                [--no-water] [--eye X,Y,Z] [--look X,Y,Z] [--headless] [--frames N]
//!                [--screenshot PATH]
//!                [--bench SECS] [--port N] [--no-ao] [--no-sky] [--hour H] [--lat DEG]
//!                [--day N | --date M-D] [--exposure auto|STOPS]
//!                [--edit X0,Y0,X1,Y1,ACTION[,MEDIUM]]... [--sim] [--sim-ticks N] [--sim-seed N]
//!                [--sim-root DIR]
//! ```
//!
//! Keys: WASD, Space and Shift to fly; right mouse to look; wheel for speed; **R** to reset the view;
//! **P** to play or pause; **,** and **.** to step a snapshot; **Home** and **End** for the ends of
//! the run; **[** and **]** for the play rate; left mouse on the timeline to scrub; **1**-**8** for
//! the overlay; **V** for the ground cover and vines; **F** for the standing water; **K** and **L**
//! move the hour of the day; **;** and **'** move the date a week without moving the tick and **\\**
//! hands the date back to the run; **-** and **=** move the exposure and **0** hands it back to
//! the measurement;
//! and **O** turns the beauty pass -- ambient occlusion, sky, sun and seasonal colour -- off.
//!
//! **The shadows have an exposure, and it opens under a canopy** (shot V7). V6 turned shadow maps
//! on, and from that shot a shadowed surface received `ambient / (ambient + sun)` of a lit one --
//! about a thirteenth at the default hour -- where V5 gave it everything, because V5 cast no
//! shadows at all. That ratio is right for the sun and wrong for the eye, and the only control over
//! it was **O**, which takes the whole beauty pass away. The viewer now measures how much of the
//! site's ground has a crown over it ([`ecoview_native::voxel::VoxelWorld::canopy`]) and lifts the
//! ambient level towards [`ecoview_native::sky::SHADE_FLOOR`] in proportion. **An open site does
//! not move**, which is what leaves every frame V6 through V8 took where it was; a site under a
//! closed canopy gets two stops. It moves an engine light and no field in any run.
//!
//! **The date can be held over a tick that does not move** (shot V8). `--day N`, `--date M-D` or
//! the two date keys turn the year over one snapshot, so the same wood stands in a summer frame and
//! a winter one; before this the only way to reach December was to load a December tick, and a
//! different tick is a different world. It moves the sun and the leaf colour and **nothing else** --
//! every tree, every millimetre of water and every overlay band on the frame is still the run's,
//! from the tick the timeline is on, and the HUD says on every frame that the date is not.
//!
//! Editing, under the crosshair: **Q** and **Z** raise and lower the ground, **T** and **G** raise
//! and lower a building, **M** cycles the surface, **U** undoes. **Enter** grows what you have made
//! -- it writes the edited site out as a world bundle, runs `ecosim` on it as a command, reads the
//! run back and plays it from tick 0. **Backspace** stops a run in flight.
//!
//! **Z digs below the site's zero** (shot S6). A bundle's heights are measured from its own lowest
//! ground, so a pond on flat ground has to go under that datum; the world carries the offset instead
//! of clamping, and the floor is the soil the run puts under the bundle
//! (`params.bundle.base_z`, [`ecoview_native::run::RunMeta::base_z_m`]) and no deeper. The HUD says
//! how much is left to dig before the stroke, and says so when a stroke would hit the floor.
//!
//! **The simulator decides and the viewer expresses** (`overnight/DIRECTION-native-viewer.md`). The
//! round trip changes nothing about that: the edit changes the ground, and every consequence of it
//! -- water, light, fertility, what grows and what dies -- is `ecosim`'s. The two projects still
//! share no code and have no IPC (CLAUDE.md); a run directory on disk is the whole interface, and
//! the simulator is started as a command the way shot E4's browser helper starts it.
//!
//! The ground cover and the vines are **expression, not simulation** (`src/cover.rs`): the run says
//! how much grass and shrub a patch holds and how wet and shaded its columns are, and the viewer
//! decides only where the blades stand and how far a climber gets. No vine is an entity in any run.
//!
//! **Standing water is the run's**, all of it (shot S5). `water.bin` is ponded depth on the ground
//! grid and the simulator writes it every snapshot; the viewer reads it, maps it and stands it up as
//! voxels, and the one thing it decides is the lattice -- water shallower than
//! [`ecoview_native::overlay::POND_MIN_MM`] is mapped but not drawn, and anything drawn at all is
//! drawn at least one 0.5 m voxel deep. The HUD prints the millimetres beside the voxel count, every
//! frame, for that reason.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::image::Image;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver};
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
use ecoview_native::overlay::{self, FieldStats, Fields, PondStats, Ponds, Scale};
use ecoview_native::palette::{palette, Overlay, Ramp, BANDS};
use ecoview_native::run::Run;
use ecoview_native::sim::{self, SimJob, SimState};
use ecoview_native::sky::{self, shaded_palette, Adaptation, Clock, SkyState, DAYS_PER_YEAR};
use ecoview_native::tree::Life;
use ecoview_native::voxel::{Canopy, ChunkPos, ColumnBands, EditAction, VoxelWorld};
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

/// How far the crosshair reaches, in metres. Past this it points at nothing rather than at the
/// horizon: an edit 300 m away is never the edit anyone meant.
const PICK_RANGE_M: f32 = 120.0;

/// How many edits can be undone. One entry is a column's three numbers, so this is cheap; it is
/// finite because an editor left running all afternoon should not grow without bound.
const UNDO_DEPTH: usize = 512;

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
    /// Shot S5's standing water, drawn as voxels from the run's `water.bin`. `--no-water`, or **F**.
    water: bool,
    /// Where the camera stands and what it looks at, in metres. Both default to the overview pose
    /// `setup` computes from the site's size.
    ///
    /// A headless screenshot has no one to fly it, so a picture of anything but the whole site --
    /// a wall with a climber on it, a tree at eye level -- could only be taken by hand. A shot
    /// report that says "look at this" has to be re-runnable, so the pose is an argument
    /// (DECISIONS.md, V4).
    eye: Option<Vec3>,
    look: Option<Vec3>,
    /// `--edit X0,Y0,X1,Y1,ACTION[,MEDIUM]`, repeatable: a rectangle of ground cells, applied before
    /// the first frame. A rectangle rather than a cell because the edits worth photographing are
    /// areas -- a paved yard, a dug basin -- and three hundred `--edit` flags is not a command line.
    edits: Vec<(usize, usize, usize, usize, EditAction)>,
    /// `--sim`: do the round trip once, at startup, after those edits. The whole of this shot in one
    /// command, which is what makes a picture of it re-runnable.
    sim: bool,
    sim_ticks: u32,
    sim_seed: u64,
    /// Where the round trip writes. Under `target/` by default, which git ignores: an edited copy of
    /// a site is scratch, and the run beside it is regenerable by definition.
    sim_root: String,
    /// Shot V6's beauty pass, in three parts that switch off separately because they cost different
    /// things: `ao` is baked into the mesh, `sky` is the dome, the sun path and the seasonal tint,
    /// and `hour` and `lat` are the two numbers the sun path needs that no run carries (`sky.rs`).
    ao: bool,
    sky: bool,
    hour: f32,
    lat: f32,
    /// `--day N` (1-based day of the year) or `--date M-D`: the day of the year **held**, over
    /// whatever tick the timeline is on (shot V8). `None` leaves the day the run's, which is where
    /// V6 left it. The keys **;**, **'** and **\** move and release it at runtime.
    day: Option<f32>,
    /// `--exposure STOPS`: the eye's adaptation held by hand, in stops, or `None` for the measured
    /// one (shot V7). `--exposure auto` is the default and says so out loud.
    exposure: Option<f32>,
}

/// `X0,Y0,X1,Y1,ACTION[,MEDIUM]` in ground cells, inclusive. Fatal when it does not parse, for the
/// same reason a mistyped overlay is: a scripted shot must not quietly photograph an unedited site.
fn edit_rect(s: &str) -> (usize, usize, usize, usize, EditAction) {
    let f: Vec<&str> = s.split(',').map(str::trim).collect();
    assert!(
        f.len() == 5 || f.len() == 6,
        "expected --edit X0,Y0,X1,Y1,ACTION[,MEDIUM], got {s:?}"
    );
    let n = |i: usize| {
        f[i].parse::<usize>()
            .unwrap_or_else(|_| panic!("{:?} is not a ground cell index, in {s:?}", f[i]))
    };
    let medium = f.get(5).map(|m| {
        m.parse::<u8>()
            .unwrap_or_else(|_| panic!("{m:?} is not a medium code, in {s:?}"))
    });
    let action = EditAction::parse(f[4], medium)
        .unwrap_or_else(|| panic!("unknown action {:?}, in {s:?}", f[4]));
    (n(0), n(1), n(2), n(3), action)
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
        water: true,
        eye: None,
        look: None,
        edits: Vec::new(),
        sim: false,
        sim_ticks: sim::DEFAULT_TICKS,
        sim_seed: sim::DEFAULT_SEED,
        sim_root: "target/sim".to_string(),
        ao: true,
        sky: true,
        hour: sky::DEFAULT_HOUR,
        lat: sky::DEFAULT_LATITUDE_DEG,
        day: None,
        exposure: None,
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
            "--no-water" => a.water = false,
            "--eye" => {
                a.eye = Some(vec3(&next()));
                i += 1;
            }
            "--look" => {
                a.look = Some(vec3(&next()));
                i += 1;
            }
            "--edit" => {
                a.edits.push(edit_rect(&next()));
                i += 1;
            }
            "--sim" => a.sim = true,
            "--sim-ticks" => {
                a.sim_ticks = next().parse().unwrap_or(sim::DEFAULT_TICKS);
                i += 1;
            }
            "--sim-seed" => {
                a.sim_seed = next().parse().unwrap_or(sim::DEFAULT_SEED);
                i += 1;
            }
            "--sim-root" => {
                a.sim_root = next();
                i += 1;
            }
            "--no-ao" => a.ao = false,
            "--no-sky" => a.sky = false,
            "--hour" => {
                a.hour = next().parse().unwrap_or(sky::DEFAULT_HOUR);
                i += 1;
            }
            "--lat" => {
                a.lat = next().parse().unwrap_or(sky::DEFAULT_LATITUDE_DEG);
                i += 1;
            }
            // Fatal when it does not parse, for the reason `--overlay` is: these two flags decide
            // which season a scripted screenshot is taken in, and a silent fall back to the run's
            // own date would file a summer frame under the winter picture's name.
            "--day" | "--date" => {
                let n = next();
                a.day = Some(sky::parse_date(&n).unwrap_or_else(|| {
                    panic!(
                        "cannot read {n:?} as a date; expected a day of the year in 1-365, or M-D \
                         (6-22 is 22 June)"
                    )
                }));
                i += 1;
            }
            // Fatal for the reason `--day` is: this flag decides how bright a scripted frame is,
            // and a silent fall back to the measured value would file a hand-exposed picture under
            // the automatic one's name.
            "--exposure" => {
                let n = next();
                a.exposure = sky::parse_exposure(&n).unwrap_or_else(|| {
                    panic!(
                        "cannot read {n:?} as an exposure; expected `auto` or a number of stops in                          -{0}..{0}",
                        sky::EXPOSURE_LIMIT
                    )
                });
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

/// One thing to do to the world's columns. `Undo` is not an [`EditAction`]: the opposite of an
/// action is not its inverse, because `LowerGround` clamps at the dig floor and `SetSurface` forgets
/// what was there, so an undo restores the three numbers it saw rather than acting again.
#[derive(Debug, Clone, Copy)]
enum EditOp {
    Cell(usize, usize, EditAction),
    Undo,
}

/// The queue a BRP `ecoview.edit` call, a key press or a `--edit` flag appends to. One explicit
/// method, so an agent never has to discover a component schema (V0-spike.md, build item 5).
#[derive(Resource, Default)]
struct EditQueue(Vec<EditOp>);

/// The bundle the viewer opened, kept whole because the round trip has to write it back out.
///
/// [`VoxelWorld`] holds the three grids and is what the editing changes; everything else about the
/// site -- its name, its size, its media table, its surveyed trees, its provenance -- is only here.
#[derive(Resource)]
struct Scene {
    bundle: Bundle,
}

/// What the crosshair is on, what has been done to the site, and what can be taken back.
#[derive(Resource, Default)]
struct Editor {
    target: Option<(usize, usize)>,
    undo: Vec<(usize, usize, (f32, u8, f32))>,
    /// The last thing worth saying about an edit, for the HUD.
    note: String,
}

/// A BRP `ecoview.sim` call asking for a round trip: `(seed, ticks)`.
#[derive(Resource, Default)]
struct SimRequest(Option<(u64, u32)>);

/// Something is in flight that a headless screenshot must wait for. A `--sim` run takes seconds and
/// `headless_frames` counts frames, so without this the picture is taken of the site before it grew.
#[derive(Resource, Default)]
struct Busy(bool);

/// The round trip: one `ecosim` process at a time, and what to say about it.
#[derive(Resource)]
struct Sim {
    job: Option<SimJob>,
    root: PathBuf,
    ticks: u32,
    seed: u64,
    /// `idle`, `running`, `grown` or `failed`, for the HUD and for `ecoview.stats`.
    phase: &'static str,
    note: String,
    /// The last finished round trip: ticks, wall seconds, snapshots, and the run directory.
    last: Option<(u32, f64, usize, String)>,
    /// `--sim` starts exactly one run, however many frames the app takes to get going.
    auto_started: bool,
}

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
    /// Shot S5's standing water, on the same pattern as the cover above: `water` is what the user
    /// asked for, **F** or `--no-water`, and `water_applied` is the `(on, snapshot)` the drawn world
    /// actually has.
    water: bool,
    water_applied: Option<(bool, usize)>,
    /// What the run's `water.bin` held at this snapshot, and what was drawn from it: cells with any
    /// water at all against cells deep enough to become a voxel, and the depths in millimetres.
    pond_stats: PondStats,
    pond_counts: (usize, usize),
    /// The ground cells `water.bin` covers, which is what the wet count is a fraction of.
    pond_site_cells: usize,
    /// Why the water could not be drawn, if it could not.
    water_error: Option<String>,
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
    /// The two colours the ramp runs between, and whether the run named them (`ecosim` shot S2) or
    /// this viewer fell back on the `ecoview` legend. `None` on the same terms as `scale`.
    ramp: Option<Ramp>,
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
            ramp: None,
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
    ponds: Option<&Result<Ponds, String>>,
) -> Vec<ChunkPos> {
    let cell_m = world.cell_m;
    ov.scale = None;
    ov.ramp = None;
    ov.stats = None;
    ov.fire = (0, 0);
    ov.error = None;
    let stale = match (&t.run, ov.active.is_field()) {
        (Some(run), true) => {
            let scale = Scale::of(ov.active, &run.meta);
            // Standing water is the one field not on the ecology grid: `water.bin` is per ground
            // cell, because that is the grid the water ran over. `ColumnBands` carries the cell
            // size for exactly this reason, so the map is drawn at the resolution of its own file
            // and no other.
            let banded = if ov.active == Overlay::Water {
                match ponds {
                    Some(Ok(p)) => {
                        let (bands, stats) = p.bands(&scale);
                        ov.stats = Some(stats);
                        Some(ColumnBands {
                            x: p.width,
                            y: p.depth,
                            cell_m,
                            bands,
                        })
                    }
                    other => {
                        ov.error = Some(match other {
                            Some(Err(e)) => e.clone(),
                            _ => "the snapshot's water.bin was not read".into(),
                        });
                        None
                    }
                }
            } else {
                match fields {
                    Some(Ok(f)) => {
                        let d = run.meta.dims;
                        let (bands, stats) = f.bands(ov.active, &d, &scale);
                        ov.stats = Some(stats);
                        ov.fire = f.fire_counts();
                        Some(ColumnBands {
                            x: d.x,
                            y: d.y,
                            cell_m: ecoview_native::ECO_CELL_M,
                            bands,
                        })
                    }
                    other => {
                        ov.error = Some(match other {
                            Some(Err(e)) => e.clone(),
                            _ => "the snapshot's fields were not read".into(),
                        });
                        None
                    }
                }
            };
            let out = world.set_overlay(banded.as_ref());
            ov.scale = Some(scale);
            ov.ramp = Some(ov.active.ramp(Some(&run.meta)));
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

/// The drawn atmosphere, and the two numbers no run carries.
///
/// **Nothing here is ecology** (`sky.rs`): the simulator computed its light under a fixed 45 degree
/// sun and has no hour of the day, so moving this sun changes no number in any run. What it changes
/// is whether the shape of the site is readable.
#[derive(Resource)]
struct Sky {
    /// The dome, the sun path and the seasonal tint. `--no-sky`, or **O**.
    on: bool,
    /// Ambient occlusion, baked into the voxel ids. `--no-ao`, or **O**.
    ao: bool,
    hour: f32,
    lat: f32,
    /// The day of the year held over the run's tick, or `None` to take the run's own (shot V8).
    /// `--day`/`--date` sets it, **;** and **'** move it, **\** puts it back to `None`.
    day: Option<f32>,
    state: SkyState,
    /// The `(on, ao, season step)` the drawn world's palette was built for. Anything else is a
    /// remesh, which is why the season is quantised at all (`sky::SEASON_STEPS`).
    applied: Option<(bool, bool, u32)>,
    /// How much of the drawn site has a crown over it (shot V7), measured when the snapshot's
    /// plants change rather than every frame. The one input to the eye's adaptation.
    canopy: Canopy,
    /// The adaptation held by hand, in stops, or `None` to take the measurement. `--exposure`,
    /// **-** and **=** to move it, **0** to hand it back.
    exposure: Option<f32>,
    /// What the last beauty-pass remesh cost, for `ecoview.stats` and the write-up.
    remesh_chunks: usize,
    remesh_ms: f64,
    /// The state the dome mesh was built for, so a camera move does not rebuild the sky.
    dome: Option<SkyState>,
}

impl Sky {
    fn new(a: &Args, t: &Timeline) -> Sky {
        let mut sky = Sky {
            on: a.sky,
            ao: a.ao,
            hour: a.hour,
            lat: a.lat,
            day: a.day,
            state: SkyState::of(Clock::of(None, 0, a.hour).with_day(a.day), a.lat),
            applied: None,
            canopy: Canopy::default(),
            exposure: a.exposure,
            remesh_chunks: 0,
            remesh_ms: 0.0,
            dome: None,
        };
        sky.recompute(t);
        sky
    }

    /// The day of the year is the run's, every time the timeline moves; the hour is the viewer's;
    /// and since shot V8 the day can be **held** by the viewer over a tick that does not move.
    ///
    /// The override goes on last and changes one number. Everything else about the clock -- the
    /// tick, the `year_len`, what a tick is worth in hours -- is still read off the run here, so a
    /// held frame can still say which tick it is a picture of, which is the whole reason to hold it.
    fn recompute(&mut self, t: &Timeline) {
        let (tick, year_len) = match &t.run {
            Some(r) => (Some(r.tick_at(t.index)), r.meta.year_len),
            None => (None, 0),
        };
        self.state = SkyState::of(
            Clock::of(tick, year_len, self.hour).with_day(self.day),
            self.lat,
        );
    }

    /// The eye's adaptation for this frame (shot V7): the measured canopy closure, or whatever
    /// **-** and **=** have been pushed to, through [`SkyState::adapt`].
    fn adapt(&self) -> Adaptation {
        self.state.adapt(self.canopy.closure(), self.exposure)
    }

    /// What the drawn palette depends on. With the seasonal tint off the year is not one of them,
    /// so `--no-sky` scrubs the timeline at exactly the cost V5 did.
    fn key(&self) -> (bool, bool, u32) {
        let season = if self.on { self.state.mesh_key() } else { 0 };
        (self.on, self.ao, season)
    }

    /// The palette the world is meshed with: the overlay's colours from `meta.json`, moved through
    /// the year, then expanded into one block per occlusion level.
    ///
    /// With the beauty pass off this is exactly `palette()`, which is what makes a V6 build draw a
    /// V5 picture to the byte (`tests/mesh_golden.rs`).
    fn palette(&self, ov: Overlay, t: &Timeline) -> Vec<[f32; 4]> {
        let mut p = palette(ov, t.run.as_ref().map(|r| &r.meta));
        if self.on {
            self.state.season.tint_palette(&mut p);
        }
        if self.ao {
            shaded_palette(&p)
        } else {
            p
        }
    }
}

/// The sphere of sky around the camera. Its radius has to sit inside the camera's far plane (1000 m
/// by default) and outside anything on the site, which at 256 m across leaves a wide choice.
const SKY_RADIUS_M: f32 = 600.0;

/// The fixed 45 degree sun V0 through V5 drew, kept for `--no-sky`: the beauty pass has an off
/// switch, and off means the picture the last five shots took.
const FIXED_SUN: (f32, f32) = (0.8, -0.8);
const FIXED_ILLUMINANCE: f32 = 10_000.0;
const FIXED_AMBIENT: f32 = 700.0;

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
        water: a.water,
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
    //
    // With no run there is still the round trip, which can put a full-grown tree on a site whose
    // survey has none, so the fallback curve's ceiling is reserved as well: the grid cannot be
    // resized once the world is built, and an empty chunk layer costs nothing to keep (V5).
    let headroom = timeline
        .run
        .as_ref()
        .map_or(0.0, |r| r.life.tall_height_m)
        .max(Life::default().tall_height_m);
    let mut world = VoxelWorld::from_bundle_with_headroom(&bundle, headroom);
    // Ambient occlusion is a property of the world, not of one mesh: it is baked into the voxel ids
    // the mesher merges on, so it has to be set before the first chunk is filled (`mesh.rs`).
    world.ao = a.ao;
    let sky = Sky::new(&a, &timeline);
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

    let world_name = bundle.name.clone();
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
                title: format!("ecoview-native: {}", world_name),
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
            .with_method_main("ecoview.overlay", overlay_method)
            .with_method_main("ecoview.sim", sim_method),
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
    .init_resource::<Editor>()
    .init_resource::<SimRequest>()
    .init_resource::<Busy>()
    .insert_resource(Sim {
        job: None,
        root: PathBuf::from(&a.sim_root),
        ticks: a.sim_ticks,
        seed: a.sim_seed,
        phase: "idle",
        note: String::new(),
        last: None,
        auto_started: false,
    })
    .insert_resource(Scene { bundle })
    .insert_resource(OverlayState::new(a.overlay))
    .insert_resource(timeline)
    .insert_resource(Bench {
        until: a.bench,
        samples: Vec::new(),
    })
    // The dome covers the whole sky, so this shows only where the dome does not reach; it is set
    // from the horizon anyway, so a gap would not be a black band.
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(sky)
    .insert_resource(a.clone())
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            timeline_keys,
            timeline_scrub,
            timeline_play,
            overlay_keys,
            sky_update,
            apply_world_state,
            edit_keys,
            apply_edits,
            sim_tick,
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

/// Reads snapshot `t.index`'s `water.bin`.
///
/// **Every snapshot, whichever way the switches are set**, because the report is worth more than the
/// read: with the layer off the HUD still says how much water the run put on the site, which is the
/// difference between a dry-looking picture and a picture of a dry site. The file is a quarter of
/// `material.bin` on the Capitol, and it is read once per snapshot rather than per frame.
fn read_ponds(t: &Timeline) -> Option<Result<Ponds, String>> {
    t.run
        .as_ref()
        .map(|r| r.ponds_at(t.index).map_err(|e| e.to_string()))
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

/// Puts snapshot `t.index`'s standing water into the world, or takes it off. Returns the stale
/// chunks.
///
/// The stats are read off the file whether or not anything is drawn from it, the way the cover's
/// means are: a picture with the water switched off still says how much water there was.
fn apply_ponds(
    world: &mut VoxelWorld,
    t: &mut Timeline,
    ponds: Option<&Result<Ponds, String>>,
) -> Vec<ChunkPos> {
    t.water_error = None;
    t.pond_stats = PondStats::default();
    let cell_m = world.cell_m;
    let levels = match (t.run.is_some(), ponds) {
        (true, Some(Ok(p))) => {
            t.pond_stats = p.stats(cell_m);
            t.pond_site_cells = p.cells();
            t.water.then(|| p.levels(cell_m))
        }
        (true, Some(Err(e))) => {
            t.water_error = Some(e.clone());
            None
        }
        _ => None,
    };
    let stale = world.set_ponds(levels.as_ref());
    t.pond_counts = world.pond_counts();
    t.water_applied = Some((t.water, t.index));
    stale
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
    // Which media grow things is the run's answer, not this viewer's (shot S4). Read here rather
    // than at load because a run can arrive later -- the round trip grows one from the edited site.
    world.read_plantable(&run.meta);
    // And how deep a hole the exported world can carry is the run's answer too (shot S6): it is the
    // simulator's `[bundle] base_z`, the soil it puts under the bundle's lowest ground.
    world.read_dig_limit(&run.meta);
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
    mut queue: ResMut<EditQueue>,
    mut sky: ResMut<Sky>,
    args: Res<Args>,
) {
    // The scripted edits go in before anything is meshed, so the first frame is already of the
    // edited site and `--sim` runs on what the picture shows.
    for &(x0, y0, x1, y1, action) in &args.edits {
        for y in y0.min(y1)..=y0.max(y1) {
            for x in x0.min(x1)..=x0.max(x1) {
                queue.0.push(EditOp::Cell(x, y, action));
            }
        }
    }
    // The run's trees and the overlay both go in before the first mesh, so the site is never drawn
    // with the bundle's vegetation under the wrong palette and then corrected a frame later.
    let fields = read_fields(&timeline);
    let ponds = read_ponds(&timeline);
    let want = want_cover(&timeline, &overlays);
    apply_ponds(&mut site.world, &mut timeline, ponds.as_ref());
    load_snapshot(
        &mut site.world,
        &mut timeline,
        fields.as_ref().and_then(|f| f.as_ref().ok()),
        want,
    );
    // How much of the site is under a crown, taken once off the site as it is first drawn (V7).
    // The bundle's own surveyed trees count for this as much as a run's do: a site photographed
    // under a closed canopy is as dark as one grown into it.
    sky.canopy = site.world.canopy();
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
        // And the standing water, on the same terms. This is the line shot S5 exists for: before
        // it, the one field that says where the water is had no number and no picture anywhere in
        // this viewer.
        println!(
            "water: {}",
            water_line(&timeline, site.world.cell_m)
                .replace('\n', " ")
                .trim()
        );
    }
    // And whose answer the plantable gate is, beside the height curve above it (shot S12). The HUD
    // has carried this sentence since S4 and the console has not, and the console is the surface a
    // headless run leaves behind -- the one place nobody is looking at a screen. Outside the `if
    // let` for the reason the HUD prints it with no run open: the case worth naming is the one with
    // nothing to ask, and that case is always the fallback the `(!)` marks.
    println!("plantable: {}", site.world.plantable.line());
    apply_overlay_bands(
        &mut site.world,
        &mut overlays,
        &timeline,
        fields.as_ref(),
        ponds.as_ref(),
    );
    sky.recompute(&timeline);
    site.palette = sky.palette(overlays.active, &timeline);
    sky.applied = Some(sky.key());
    // The sky on stdout for the same reason the trees are: a picture that leans on a low sun should
    // leave behind which hour and which latitude drew it, and whose each of them is.
    println!(
        "sky: {}{}",
        sky.state.line(),
        if sky.on {
            ""
        } else if sky.day.is_some() {
            // The HUD says this on the frame too (shot V8): with the beauty pass off there is no
            // sun path and no season in the palette, so a held date is a flag that draws nothing.
            "  (--no-sky: the fixed 45 degree sun of V0-V5, and nothing draws the held date)"
        } else {
            "  (--no-sky: the fixed 45 degree sun of V0-V5)"
        }
    );
    // And what the eye did with it (shot V7). On stdout for the same reason as the cover's line:
    // the report quotes the scripted run, so the caveat travels with the picture -- this is the
    // viewer's adaptation and not a reading of anything in the run.
    println!(
        "exposure: {}{}",
        sky.adapt().line(),
        if sky.on {
            ""
        } else {
            "  (--no-sky: no shadow maps, so nothing is drawn with it; ambient is V5's fixed 700)"
        }
    );
    println!(
        "ambient occlusion: {}, {} levels, season step {} of {}",
        if sky.ao { "on" } else { "off (--no-ao)" },
        sky::AO_LEVELS,
        sky.state.season.step(),
        sky::SEASON_STEPS
    );
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
    // The colours as well as the numbers: since `ecosim` shot S2 both halves of an overlay come from
    // the run, and a scripted run's stdout is what a report quotes, so it says which.
    let ramp = match &overlays.ramp {
        Some(r) => format!("; ramp {} to {} from {}", r.lo, r.hi, r.source),
        None => String::new(),
    };
    match (&overlays.scale, overlays.stats, &overlays.error) {
        (Some(sc), Some(st), _) => println!(
            "overlay {}: {:.2}..{:.2} {} from {}; field {:.2}..{:.2} mean {:.2}{ramp}{fire}",
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
            brightness: FIXED_AMBIENT,
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
    // The sun. `sky_update` points it, colours it and dims it every frame the sky moves; with
    // `--no-sky` it keeps the fixed 45 degree pose V0 through V5 drew and casts no shadow.
    //
    // Shadows are what the sun is for. A sun that lights every face the same is a sun that hides
    // the site's shape, and the four cascades below cover the Capitol's 256 m twice over.
    commands.spawn((
        DirectionalLight {
            illuminance: FIXED_ILLUMINANCE,
            shadow_maps_enabled: sky.on,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 0.5,
            maximum_distance: 500.0,
            first_cascade_far_bound: 24.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::from_rotation(Quat::from_euler(
            EulerRot::YXZ,
            FIXED_SUN.0,
            FIXED_SUN.1,
            0.0,
        )),
        SunLight,
    ));
    // The sky itself: a vertex-coloured sphere around the camera, built by the same engine-free
    // code the ground is and drawn with the same kind of material, unlit and shadowless because it
    // is the light source's backdrop rather than a thing in the scene.
    let dome = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(to_bevy_mesh(&sky.state.dome(SKY_RADIUS_M, 12, 32)))),
        MeshMaterial3d(dome),
        Transform::from_translation(eye),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        SkyDome,
    ));
    // `AmbientLight` is a camera component in 0.19, not a resource, so it goes on the camera above.
    spawn_hud(&mut commands);
}

#[derive(Component)]
struct HudText;

/// The scrubbed part of the timeline bar: a child whose width is the fraction of the run played.
#[derive(Component)]
struct TimelineFill;

/// The bar itself, hidden until a run exists to scrub.
#[derive(Component)]
struct TimelineBar;

/// The legend strip and its two parts: one swatch per band, and the range written beside them.
#[derive(Component)]
struct LegendRoot;
#[derive(Component)]
struct LegendBand(usize);
#[derive(Component)]
struct LegendLabel;

/// One text block top-left, a crosshair in the middle, and one bar along the bottom.
///
/// V1 skipped the bar when the viewer opened with no run, which was right while a run could only
/// arrive on the command line. A round trip can now grow one while the viewer is running, so the bar
/// is always built and `hud` hides it until there is something to scrub (V5).
fn spawn_hud(commands: &mut Commands) {
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
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            max_width: Val::Percent(95.0),
            ..default()
        },
        // White text on the black of V0-V5 needed no backing. The sky is bright now, and the top
        // left of the frame is exactly where it is brightest, so every line of the HUD in every
        // screenshot this shot took would otherwise be unreadable.
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
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
    // The crosshair. It is what the editor points with, so it is drawn even with no run loaded and
    // it sits exactly at the screen's centre -- which is where the camera's forward vector goes, and
    // therefore where `pick_cell` looks. The margin centres the glyph on that point rather than
    // hanging its top-left corner from it.
    commands.spawn((
        Text::new("+"),
        TextFont {
            font_size: bevy::text::FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            margin: UiRect {
                left: Val::Px(-6.0),
                top: Val::Px(-12.0),
                ..default()
            },
            ..default()
        },
    ));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(BAR_MARGIN),
                right: Val::Px(BAR_MARGIN),
                bottom: Val::Px(BAR_BOTTOM),
                height: Val::Px(BAR_HEIGHT),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            TimelineBar,
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
///
/// Every edit records the column's three numbers before it changes them, which is what **U** puts
/// back. The record is taken here rather than where the edit is queued so a BRP call, a key press
/// and a `--edit` flag are all undoable by the same code.
fn apply_edits(
    mut commands: Commands,
    mut queue: ResMut<EditQueue>,
    mut site: ResMut<Site>,
    mut editor: ResMut<Editor>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if queue.0.is_empty() {
        return;
    }
    let palette = site.palette.clone();
    let mut scratch = Scratch::new();
    let t = Instant::now();
    // The whole queue is drained before anything is remeshed. One key press is one cell and the
    // difference is nothing, but a `--edit` rectangle is thousands of them over the same handful of
    // chunks, and remeshing per cell would mesh each of those chunks thousands of times.
    let mut stale: Vec<ChunkPos> = Vec::new();
    // Set when a dig has moved the lattice floor. Every chunk is then stale and the chunk grid may
    // have grown a layer, so the per-chunk bookkeeping below is rebuilt rather than patched.
    let mut lifted = false;
    for op in std::mem::take(&mut queue.0) {
        let changed = match op {
            EditOp::Cell(x, y, action) => match site.world.column(x, y) {
                Some(was) => {
                    // A hole deeper than the exported world can carry is refused and said out loud,
                    // which is the half of shot S6 the row asked for first: before this, a stroke on
                    // low ground moved the ground a few centimetres and stopped with no indication
                    // why. The note names where the limit came from, because on a bundle with no run
                    // loaded it is this viewer's default and not any run's number.
                    if action == EditAction::LowerGround && site.world.at_dig_limit(x, y) {
                        editor.note = format!(
                            "({x}, {y}) is {:.2} m below the site's zero and that is as deep \
                             as this world goes -- {:.0} m of soil sits under the bundle's \
                             lowest ground, {}, and a hole cannot go under it",
                            -was.0,
                            site.world.dig_limit_m(),
                            if site.world.dig_limit_from_run() {
                                "the run's params.bundle.base_z"
                            } else {
                                "this viewer's default, with no run loaded to ask"
                            }
                        );
                        Vec::new()
                    } else {
                        editor.undo.push((x, y, was));
                        if editor.undo.len() > UNDO_DEPTH {
                            editor.undo.remove(0);
                        }
                        site.edits += 1;
                        if action == EditAction::LowerGround && site.world.open_dig_room(x, y) {
                            lifted = true;
                            editor.note = format!(
                                "digging below the site's zero: opened room under the whole site, \
                                 the lattice floor is now {:.2} m down",
                                site.world.datum_m()
                            );
                        }
                        site.world.apply(x, y, action)
                    }
                }
                None => {
                    editor.note = format!("({x}, {y}) is outside the site");
                    Vec::new()
                }
            },
            EditOp::Undo => match editor.undo.pop() {
                Some((x, y, was)) => {
                    site.edits = site.edits.saturating_sub(1);
                    editor.note = format!("undid the edit at ({x}, {y})");
                    site.world.restore_column(x, y, was)
                }
                None => {
                    editor.note = "nothing left to undo".into();
                    Vec::new()
                }
            },
        };
        for c in changed {
            if !stale.contains(&c) {
                stale.push(c);
            }
        }
    }
    let material = site.material.clone();
    if lifted {
        // Nothing moved in world space, and every voxel moved in the lattice. The chunk grid can
        // also have gained a layer, which invalidates every index into `entities`, so the entities
        // go and come back rather than being reassigned.
        for e in site.entities.iter_mut().filter_map(Option::take) {
            commands.entity(e).despawn();
        }
        site.entities = vec![None; site.world.chunk_count()];
        stale = site.world.all_chunks();
    }
    for c in stale {
        let i = site.world.chunk_index(c);
        let m = mesh_chunk(&site.world, c, &palette, &mut scratch);
        sync_chunk(&mut commands, &mut site, &mut meshes, i, &m, &material);
    }
    site.last_remesh_ms = t.elapsed().as_secs_f64() * 1000.0;
}

/// The editor under the crosshair: what it is pointing at, and the six keys that change it.
///
/// The target is recomputed every frame rather than on a key press, because the HUD prints what is
/// under the crosshair and a stale reading would be worse than none -- you would edit the column the
/// line named a second ago.
fn edit_keys(
    keys: Res<ButtonInput<KeyCode>>,
    site: Res<Site>,
    scene: Res<Scene>,
    mut editor: ResMut<Editor>,
    mut queue: ResMut<EditQueue>,
    cam: Query<&Transform, With<Fly>>,
) {
    let Some(t) = cam.iter().next() else {
        return;
    };
    let (o, f) = (t.translation, *t.forward());
    editor.target = site
        .world
        .pick_cell([o.x, o.y, o.z], [f.x, f.y, f.z], PICK_RANGE_M);
    if keys.just_pressed(KeyCode::KeyU) {
        queue.0.push(EditOp::Undo);
    }
    let Some((x, y)) = editor.target else {
        return;
    };
    for (key, action) in [
        (KeyCode::KeyQ, EditAction::RaiseGround),
        (KeyCode::KeyZ, EditAction::LowerGround),
        (KeyCode::KeyT, EditAction::RaiseBuilding),
        (KeyCode::KeyG, EditAction::LowerBuilding),
    ] {
        if keys.just_pressed(key) {
            queue.0.push(EditOp::Cell(x, y, action));
        }
    }
    // **M** walks the bundle's own media list, in the order the bundle publishes it, so the viewer
    // never invents a surface the site does not have.
    if keys.just_pressed(KeyCode::KeyM) {
        let n = scene.bundle.media.len().max(1);
        if let Some((_, m, _)) = site.world.column(x, y) {
            let next = ((m as usize + 1) % n) as u8;
            queue
                .0
                .push(EditOp::Cell(x, y, EditAction::SetSurface(next)));
        }
    }
}

/// The round trip, one frame at a time: start it, watch it, load what it wrote, play it.
///
/// It is polled rather than waited on, so the viewer keeps drawing and stays flyable while the
/// simulator works. Nothing here models anything: the edit changed the ground, and every
/// consequence of that is computed by `ecosim` in its own process, at its own 1 m grid.
#[allow(clippy::too_many_arguments)]
fn sim_tick(
    keys: Res<ButtonInput<KeyCode>>,
    args: Res<Args>,
    site: Res<Site>,
    scene: Res<Scene>,
    mut sim: ResMut<Sim>,
    mut request: ResMut<SimRequest>,
    mut timeline: ResMut<Timeline>,
    mut overlays: ResMut<OverlayState>,
    mut busy: ResMut<Busy>,
) {
    if keys.just_pressed(KeyCode::Backspace) {
        if let Some(job) = sim.job.as_mut() {
            job.cancel();
            sim.job = None;
            sim.phase = "idle";
            sim.note = "the run was stopped".into();
            busy.0 = false;
        }
    }
    let asked = request.0.take();
    let auto = args.sim && !sim.auto_started;
    if sim.job.is_none() && (asked.is_some() || auto || keys.just_pressed(KeyCode::Enter)) {
        sim.auto_started = true;
        let (seed, ticks) = asked.unwrap_or((sim.seed, sim.ticks));
        sim.seed = seed;
        sim.ticks = ticks;
        let root = sim.root.clone();
        let w = &site.world;
        // The bundle's frame, not the lattice's: a column dug below the bundle's zero goes out as a
        // negative height, and `ecosim` puts `[bundle] base_z` layers of soil under the lowest of
        // them, so the hole is carried by the format as it stands (shot S6).
        let ground = w.ground_export();
        match SimJob::start(
            &scene.bundle,
            (&ground, &w.medium, &w.building_h),
            &root,
            seed,
            ticks,
        ) {
            Ok(job) => {
                println!("sim: {}", job.command);
                sim.phase = "running";
                sim.note = format!("ecosim is running: {ticks} ticks, seed {seed}");
                sim.job = Some(job);
                busy.0 = true;
            }
            Err(e) => {
                sim.phase = "failed";
                sim.note = format!("could not start ecosim: {e}");
                eprintln!("sim: {}", sim.note);
            }
        }
    }
    let Some(mut job) = sim.job.take() else {
        return;
    };
    match job.poll() {
        SimState::Running => {
            let (tick, frac) = job.progress();
            sim.note = format!(
                "ecosim: tick {tick} of {} ({:.0}%), {:.1} s",
                job.ticks,
                frac * 100.0,
                job.elapsed().as_secs_f64()
            );
            sim.job = Some(job);
            busy.0 = true;
        }
        SimState::Done => {
            busy.0 = false;
            let secs = job.elapsed().as_secs_f64();
            let loaded = Run::load(&job.out)
                .map_err(|e| e.to_string())
                .and_then(|r| r.check_against(&scene.bundle).map(|_| r));
            match loaded {
                Ok(run) => {
                    let snaps = run.snapshot_count();
                    // Back to the beginning and play: the point of the round trip is watching the
                    // site grow from the edit, not arriving at the end of it. `--tick` overrides,
                    // because a scripted screenshot asks for one moment and should not be moving.
                    timeline.index = args.tick.map_or(0, |t| run.index_of_tick(t));
                    timeline.playing = args.tick.is_none();
                    timeline.applied = None;
                    timeline.cover_applied = None;
                    timeline.run = Some(run);
                    overlays.applied = None;
                    sim.phase = "grown";
                    sim.last = Some((job.ticks, secs, snaps, job.out.display().to_string()));
                    sim.note = format!(
                        "grown: {} ticks in {:.1} s, {snaps} snapshots in {}",
                        job.ticks,
                        secs,
                        job.out.display()
                    );
                    println!("sim: {}", sim.note);
                }
                Err(e) => {
                    sim.phase = "failed";
                    sim.note = format!("ecosim finished but its run cannot be drawn: {e}");
                    eprintln!("sim: {}", sim.note);
                }
            }
        }
        SimState::Failed(e) => {
            busy.0 = false;
            sim.phase = "failed";
            sim.note = e;
            eprintln!("sim: {} -- see {}", sim.note, job.log.display());
        }
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
    mut sky: ResMut<Sky>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let want = want_cover(&timeline, &overlays);
    let snapshot_moved = timeline.run.is_some() && timeline.applied != Some(timeline.index);
    // Toggling **V**, or turning a field overlay on over a covered site, rewrites the same voxels a
    // scrub does, so it goes down the same path rather than getting one of its own.
    let cover_moved = timeline.run.is_some() && timeline.cover_applied != Some(want);
    let water_moved =
        timeline.run.is_some() && timeline.water_applied != Some((timeline.water, timeline.index));
    let overlay_moved = overlays.applied != Some((overlays.active, timeline.index));
    // The season and the two switches, quantised: a colour that moved less than one step of the
    // year is not worth rebuilding 1,900 chunk meshes for (`sky::SEASON_STEPS`).
    let sky_moved = sky.applied != Some(sky.key());
    if !snapshot_moved && !cover_moved && !overlay_moved && !water_moved && !sky_moved {
        return;
    }
    let start = Instant::now();
    let repalette = overlays.applied.map(|(o, _)| o) != Some(overlays.active) || sky_moved;
    let fields = read_fields(&timeline);
    let ponds = read_ponds(&timeline);
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
    // The plants moved, so the canopy did (shot V7). Taken here rather than every frame: it is one
    // pass over the leaf voxels, which is a couple of milliseconds on the Capitol at tick 20000 and
    // free on anything smaller, but it is not free enough to do 60 times a second for nothing.
    if snapshot_moved || cover_moved {
        sky.canopy = site.world.canopy();
    }
    if water_moved || snapshot_moved {
        for c in apply_ponds(&mut site.world, &mut timeline, ponds.as_ref()) {
            if !stale.contains(&c) {
                stale.push(c);
            }
        }
    }
    for c in apply_overlay_bands(
        &mut site.world,
        &mut overlays,
        &timeline,
        fields.as_ref(),
        ponds.as_ref(),
    ) {
        if !stale.contains(&c) {
            stale.push(c);
        }
    }
    // A new overlay is a new palette, and a band index that happens not to have moved is still a
    // different colour, so `set_overlay`'s stale set is not enough on its own: everything is remeshed.
    // It costs about as much as the first frame did and happens on a key press, not on a scrub.
    if repalette {
        // Ambient occlusion is in the ids, so switching it rewrites the world itself, not just the
        // colours it is drawn in. Either way every chunk is rebuilt, which is what the quantising
        // above is for.
        site.world.set_ao(sky.ao);
        site.palette = sky.palette(overlays.active, &timeline);
        sky.applied = Some(sky.key());
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
    if sky_moved {
        sky.remesh_chunks = stale.len();
        sky.remesh_ms = timeline.last_ms;
    }
}

/// **1**-**8** pick the overlay, in `Overlay::ALL` order; **V** turns the cover and vines on and
/// off, and **F** the standing water.
fn overlay_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut ov: ResMut<OverlayState>,
    mut t: ResMut<Timeline>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        t.cover = !t.cover;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        t.water = !t.water;
    }
    const DIGITS: [KeyCode; 8] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
    ];
    for (i, k) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*k) {
            ov.active = Overlay::ALL[i];
        }
    }
}

#[derive(Component)]
struct SunLight;

#[derive(Component)]
struct SkyDome;

/// Moves the sky: the hour keys, the sun's pose, colour and shadows, the ambient level, and the
/// dome that follows the camera.
///
/// Everything here is free -- one light's transform and, when the sun has actually moved, one
/// 800-triangle mesh. The one expensive half of the beauty pass is the seasonal tint, which is
/// baked into vertex colours; [`apply_world_state`] notices that through [`Sky::key`].
#[allow(clippy::too_many_arguments)]
fn sky_update(
    keys: Res<ButtonInput<KeyCode>>,
    timeline: Res<Timeline>,
    mut sky: ResMut<Sky>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut clear: ResMut<ClearColor>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), (With<SunLight>, Without<SkyDome>)>,
    mut dome: Query<(&mut Mesh3d, &mut Transform, &mut Visibility), With<SkyDome>>,
    mut ambient: Query<&mut AmbientLight>,
    camera: Query<&Transform, (With<Camera3d>, Without<SunLight>, Without<SkyDome>)>,
) {
    // **O** is the whole beauty pass, both halves together: the question a reader of a screenshot
    // asks is "what does this shot add", and answering it means one key that takes all of it away.
    if keys.just_pressed(KeyCode::KeyO) {
        sky.on = !sky.on;
        sky.ao = sky.on;
    }
    // Half an hour a press. The hour is the viewer's (`sky.rs`), so it is a key and not a scrub.
    for (k, by) in [(KeyCode::KeyK, -0.5), (KeyCode::KeyL, 0.5)] {
        if keys.just_pressed(k) {
            sky.hour = (sky.hour + by).rem_euclid(24.0);
        }
    }
    // A week a press, on the two keys next to the hour's (shot V8). A week rather than a day because
    // the palette is quantised into `sky::SEASON_STEPS` -- 5.7 days a step -- so any smaller step
    // can leave the picture exactly as it was, and a key that sometimes does nothing visible reads
    // as a broken key. The first press takes the day from whatever is drawn, which is the run's
    // date when nothing is held yet: the year turns from where the snapshot actually is.
    for (k, by) in [(KeyCode::Semicolon, -7.0), (KeyCode::Quote, 7.0)] {
        if keys.just_pressed(k) {
            sky.day = Some((sky.day.unwrap_or(sky.state.clock.day) + by).rem_euclid(DAYS_PER_YEAR));
        }
    }
    // Half a stop a press, on the two keys next to the overlay digits, and **0** hands the
    // exposure back to the measurement. The first press takes the current adaptation as its
    // starting point, the way the date keys take the drawn date: pushing a picture brighter should
    // start from the picture, not from wherever the automatic value happened to be zero.
    for (k, by) in [(KeyCode::Minus, -0.5), (KeyCode::Equal, 0.5)] {
        if keys.just_pressed(k) {
            let now = sky.exposure.unwrap_or_else(|| sky.adapt().stops);
            sky.exposure = Some((now + by).clamp(-sky::EXPOSURE_LIMIT, sky::EXPOSURE_LIMIT));
        }
    }
    if keys.just_pressed(KeyCode::Digit0) {
        sky.exposure = None;
    }
    // And **\** hands the date back. The override has to be releasable from the keyboard, because
    // the frame it is on is a frame whose date is not the run's, and getting back to the honest
    // picture should not mean restarting the viewer.
    if keys.just_pressed(KeyCode::Backslash) {
        sky.day = None;
    }
    sky.recompute(&timeline);
    let st = sky.state;
    let up = st.sun.dir;
    let (illuminance, color, rotation) = if sky.on {
        let d = Vec3::new(up[0], up[1], up[2]);
        // Near the zenith "up" is no longer a usable reference for the light's roll; north is.
        let reference = if d.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
        (
            st.sun.illuminance,
            Color::linear_rgb(st.sun.color[0], st.sun.color[1], st.sun.color[2]),
            Transform::default().looking_to(-d, reference).rotation,
        )
    } else {
        (
            FIXED_ILLUMINANCE,
            Color::WHITE,
            Quat::from_euler(EulerRot::YXZ, FIXED_SUN.0, FIXED_SUN.1, 0.0),
        )
    };
    for (mut light, mut tf) in &mut sun {
        light.illuminance = illuminance;
        light.color = color;
        // A sun below the horizon casting shadows would draw them from underneath the ground.
        light.shadow_maps_enabled = sky.on && st.sun.is_up();
        tf.rotation = rotation;
    }
    // The eye's adaptation (shot V7). It moves the ambient level and nothing else: the sun keeps
    // the illuminance the geometry gave it, so lit ground stays where the sun put it and only the
    // shadows come up. With the beauty pass off there are no shadow maps to adapt to, so `--no-sky`
    // is still exactly the picture V0 through V5 took.
    let adapt = sky.adapt();
    for mut a in &mut ambient {
        a.brightness = if sky.on { adapt.ambient } else { FIXED_AMBIENT };
        a.color = if sky.on {
            Color::linear_rgb(
                st.ambient_color[0],
                st.ambient_color[1],
                st.ambient_color[2],
            )
        } else {
            Color::WHITE
        };
    }
    clear.0 = if sky.on {
        Color::linear_rgb(st.horizon[0], st.horizon[1], st.horizon[2])
    } else {
        Color::BLACK
    };
    let eye = camera.iter().next().map(|t| t.translation);
    let rebuild = sky.on && sky.dome != Some(st);
    for (mut mesh, mut tf, mut vis) in &mut dome {
        if let Some(e) = eye {
            tf.translation = e;
        }
        *vis = if sky.on {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if rebuild {
            *mesh = Mesh3d(meshes.add(to_bevy_mesh(&st.dome(SKY_RADIUS_M, 12, 32))));
        }
    }
    if rebuild {
        sky.dome = Some(st);
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

/// The two `&mut Node` queries below have to be statically disjoint from each other and from every
/// other one in [`hud`], which means each spells out the markers the others carry. Named here
/// rather than inline: a five-deep filter tuple in an argument list is unreadable.
type TimelineBarFilter = (
    With<TimelineBar>,
    Without<TimelineFill>,
    Without<LegendRoot>,
    Without<LegendLabel>,
    Without<HudText>,
);
type LegendLabelFilter = (
    With<LegendLabel>,
    Without<HudText>,
    Without<LegendRoot>,
    Without<TimelineFill>,
);

/// Everything on the screen: the text block, the bar's fill, and the overlay legend.
#[allow(clippy::too_many_arguments)]
fn hud(
    timeline: Res<Timeline>,
    overlays: Res<OverlayState>,
    site: Res<Site>,
    scene: Res<Scene>,
    editor: Res<Editor>,
    sim: Res<Sim>,
    sky: Res<Sky>,
    fly: Query<&Fly>,
    mut bar: Query<&mut Node, TimelineBarFilter>,
    mut text: Query<&mut Text, With<HudText>>,
    mut fill: Query<&mut Node, (With<TimelineFill>, Without<LegendRoot>)>,
    mut legend: Query<&mut Node, With<LegendRoot>>,
    mut swatches: Query<(&LegendBand, &mut BackgroundColor)>,
    mut label: Query<(&mut Text, &mut Node), LegendLabelFilter>,
) {
    // The bar appears when there is a run to scrub, which a round trip can create while the viewer
    // is running. An empty track offers a scrub that would do nothing.
    for mut node in &mut bar {
        node.display = if timeline.count() > 0 {
            Display::Flex
        } else {
            Display::None
        };
    }
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
        "overlay {} ({} of {})   [1-8] switch\n",
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
        // And where the two colours came from, on the same terms as the numbers above them.
        if let Some(r) = &overlays.ramp {
            s.push_str(&format!(
                "  colours from {}{}\n",
                r.source,
                if r.from_meta() { "" } else { "  (!)" }
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
    // The gate under all of that, and whose answer it is (shot S4). Printed whether or not a run is
    // open, because the case worth naming is the one where there is nothing to ask: a fallback that
    // does not say it is a fallback is indistinguishable on screen from a reading. The sentence
    // itself lives on `Plantable` so this surface and the console summary cannot drift (shot S12).
    s.push_str(&format!("{}\n", site.world.plantable.line()));
    // Standing water, and the one number in it that is this viewer's: the lattice. Printed with the
    // millimetres beside it so a screenshot of a site under 50 mm of water cannot be read as a site
    // under half a metre of it.
    if timeline.run.is_some() {
        s.push_str(&water_line(&timeline, site.world.cell_m));
    }
    // The sky says whose each of its numbers is, on the screen and not only in the write-up, for
    // the same reason the cover does: a screenshot travels further than a report.
    if sky.on {
        s.push_str(&format!(
            "{}
  beauty pass on -- ambient occlusion {}, season {} ({}/{}){}   [K] [L] hour  [;] ['] date  [O] off
",
            sky.state.line(),
            // Read off the switch rather than asserted, in both branches (shot V8). `--no-sky` and
            // `--no-ao` are separate flags and either can be passed alone, so this line has been
            // able to say "4 levels" with occlusion off and "no occlusion" with it on since V6. It
            // is on the frame in `v8-held-no-sky.png`, which is a `--no-sky` frame with occlusion.
            if sky.ao {
                format!("{} levels", sky::AO_LEVELS)
            } else {
                "off (--no-ao)".to_string()
            },
            sky.state.season.name,
            sky.state.season.step(),
            sky::SEASON_STEPS,
            // The held date is named twice on purpose: once in `line()` above, where it says whose
            // the date is, and once here beside the key that releases it. A frame is read as a
            // picture of one moment, and a held frame is a picture of two.
            match sky.day {
                Some(_) => "  date held, the tick has not moved  [\\] back to the run",
                None => "",
            },
        ));
        // And what the eye did with all of that (shot V7). On the frame rather than only in the
        // write-up for the reason the rest of this block is: a picture whose shadows have been
        // opened up two stops looks exactly like a picture taken under a thinner canopy.
        s.push_str(&format!(
            "  {}   [-] [=] stops  [0] {}
",
            sky.adapt().line(),
            if sky.exposure.is_some() {
                "back to measured"
            } else {
                "auto, on"
            },
        ));
    } else {
        s.push_str(&format!(
            "beauty pass off -- the fixed 45 degree sun, no season, ambient occlusion {}   [O] on
",
            if sky.ao { "still on" } else { "off too" },
        ));
        // A held date with the beauty pass off is a flag that draws nothing: the sun is fixed and
        // the palette has no season in it. Say so rather than let a `--date` screenshot look as
        // though the date had been applied (shot V8).
        if sky.day.is_some() {
            s.push_str(
                "  a date is held but nothing draws it while the beauty pass is off   [O] on
",
            );
        }
        // Same case, same treatment (shot V7): with the beauty pass off the sun casts no shadow, so
        // there is nothing for an exposure to open up and the ambient level is V5's fixed one.
        if sky.exposure.is_some() {
            s.push_str(
                "  an exposure is held but nothing draws it while the beauty pass is off   [O] on
",
            );
        }
    }
    s.push_str(&editor_line(&editor, &site, &scene));
    s.push_str(&sim_line(&sim));
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
        None => s.push_str(
            "no run loaded -- pass --run DIR, or edit the ground and press [Enter] to grow one",
        ),
    }
    for mut t in &mut text {
        if t.0 != s {
            t.0 = s.clone();
        }
    }
}

/// What the run's `water.bin` held at this snapshot, and what of it is on the screen.
///
/// Every number here is the run's except the last two, which are the drawing: how many voxels stand
/// on the site and how deep one of them is. The line says which is which, because the difference is
/// large -- the Capitol's mean standing depth is a tenth of a voxel -- and a picture that did not
/// say so would be claiming half a metre of water on a wet pavement.
fn water_line(t: &Timeline, cell_m: f32) -> String {
    if let Some(e) = &t.water_error {
        return format!("standing water: not read -- {e}\n");
    }
    let p = t.pond_stats;
    let head = format!(
        "standing water: {} of {} ground cells wet, {} over {:.0} mm; max {:.0} mm, mean over wet {:.0} mm, {:.1} m3",
        p.wet,
        t.pond_site_cells,
        p.drawn,
        overlay::POND_MIN_MM,
        p.max_mm,
        p.mean_mm,
        p.volume_m3,
    );
    match t.water_applied {
        Some((true, _)) => format!(
            "{head}\n  {} voxels drawn, the run's depth on a {:.2} m lattice -- anything drawn at \
             all is at least one voxel deep   [F] off\n",
            t.pond_counts.1, cell_m,
        ),
        _ => format!("{head}\n  not drawn   [F] on\n"),
    }
}

/// What the crosshair is on and what has been done to the site.
///
/// The medium is named from the **bundle's own media list**, in the order the bundle publishes it,
/// rather than from a table in the viewer: the site says what its surfaces are.
fn editor_line(editor: &Editor, site: &Site, scene: &Scene) -> String {
    let mut s = match editor
        .target
        .and_then(|(x, y)| site.world.column(x, y).map(|c| (x, y, c)))
    {
        Some((x, y, (g, m, b))) => format!(
            "crosshair ({x}, {y})  {}  ground {g:.2} m{}  {}\n",
            scene
                .bundle
                .media
                .get(m as usize)
                .map_or("an unlisted medium", String::as_str),
            if b > 0.0 {
                format!("  building {b:.2} m")
            } else {
                String::new()
            },
            // How much further [Z] will go, which is the reading the row was filed for the want
            // of: strokes on low ground stopped and the HUD said nothing about why (shot S6). Zero
            // is spelled out rather than printed as 0.00.
            match site.world.dig_room_m(x, y) {
                r if r < site.world.cell_m => "at the dig floor: [Z] does nothing here".to_string(),
                r => format!("{r:.2} m left to dig"),
            }
        ),
        None => format!("crosshair on nothing within {PICK_RANGE_M:.0} m\n"),
    };
    s.push_str(&format!(
        "{} edits, {} undoable   [Q] [Z] ground  [T] [G] building  [M] surface  [U] undo{}\n",
        site.edits,
        editor.undo.len(),
        if editor.note.is_empty() {
            String::new()
        } else {
            format!("   -- {}", editor.note)
        }
    ));
    s
}

/// Where the round trip stands, and whose numbers the result is.
fn sim_line(sim: &Sim) -> String {
    let keys = if sim.job.is_some() {
        "[Backspace] stop"
    } else {
        "[Enter] grow"
    };
    let mut s = format!(
        "ecosim round trip: {}   {keys}   {} ticks, seed {}\n",
        sim.phase, sim.ticks, sim.seed
    );
    if !sim.note.is_empty() {
        s.push_str(&format!("  {}\n", sim.note));
    }
    if sim.phase == "grown" {
        // The one sentence that keeps the picture honest. The viewer changed the ground; everything
        // that happened because of it was computed by the simulator, in its own process.
        s.push_str(
            "  the edit is the viewer's; the water, light, fertility and growth are ecosim's\n",
        );
    }
    s
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
    busy: Res<Busy>,
    mut frame: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    // A round trip takes seconds and this counts frames, so the count restarts while one is in
    // flight: `--sim --screenshot` photographs the site that grew, never the site before it.
    if busy.0 {
        *frame = 0;
        return;
    }
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
    queue.0.push(EditOp::Cell(x, y, action));
    Ok(json!({"queued": {"x": x, "y": y, "action": name}}))
}

/// `ecoview.sim {"ticks": n, "seed": n}` to start a round trip, or no parameters to read where the
/// last one got to.
///
/// The same round trip the **Enter** key starts: write the edited site out as a world bundle, run
/// `ecosim` on it as a command, and load the run it writes. An explicit method for the reason every
/// other one here is explicit -- an agent handed a named method does not retry.
fn sim_method(
    In(params): In<Option<Value>>,
    sim: Res<Sim>,
    mut request: ResMut<SimRequest>,
) -> BrpResult {
    let p = params.unwrap_or(Value::Null);
    let start = p.get("ticks").is_some() || p.get("seed").is_some() || p.get("start").is_some();
    if start {
        if sim.job.is_some() {
            return Err(brp_err("a run is already in flight; stop it first"));
        }
        let ticks = p
            .get("ticks")
            .and_then(|v| v.as_u64())
            .unwrap_or(sim.ticks as u64)
            .clamp(1, sim::MAX_TICKS as u64) as u32;
        let seed = p.get("seed").and_then(|v| v.as_u64()).unwrap_or(sim.seed);
        request.0 = Some((seed, ticks));
        return Ok(json!({"started": true, "ticks": ticks, "seed": seed}));
    }
    Ok(sim_json(&sim))
}

/// The round trip's state, for `ecoview.sim` and `ecoview.stats` both.
fn sim_json(sim: &Sim) -> Value {
    json!({
        "phase": sim.phase,
        "note": sim.note,
        "ticks": sim.ticks,
        "seed": sim.seed,
        "running": sim.job.is_some(),
        "progress": sim.job.as_ref().map(|j| {
            let (tick, frac) = j.progress();
            json!({"tick": tick, "fraction": frac, "seconds": j.elapsed().as_secs_f64()})
        }),
        "world": sim.job.as_ref().map(|j| j.world.display().to_string()),
        "last": sim.last.as_ref().map(|(ticks, secs, snaps, dir)| json!({
            "ticks": ticks, "seconds": secs, "snapshots": snaps, "run": dir,
        })),
        "note_on_authorship": "the viewer edits the ground and starts ecosim as a command; every consequence -- water, runoff, light, fertility, growth -- is computed by ecosim in its own process. No code is shared and no IPC is used: the run directory on disk is the whole interface.",
    })
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

/// The `sky` object of an `ecoview.stats` reply (shot V6), lifted out of that reply because shot
/// V8 added the two keys that took the `json!` macro past its expansion depth. One object, built
/// in one place, so an agent reading it and a reader reading this see the same thing.
fn sky_json(sky: &Sky) -> Value {
    let adapt = sky.adapt();
    json!({
        "on": sky.on,
        "ao": sky.ao,
        "ao_levels": sky::AO_LEVELS,
        "hour": sky.state.clock.hour,
        "latitude_deg": sky.state.latitude_deg,
        "day_of_year": sky.state.clock.day,
        "day_from_run": sky.state.clock.from_run(),
        // Shot V8. `day_from_run` above is still exactly what it was -- false whenever the day
        // on the frame is not the run's -- and this says which of the two ways it is not:
        // `default` for no run at all, `override` for a date held over a tick that has not
        // moved. `day_held` is the number the viewer is holding, and null when it holds none.
        "day_source": sky.state.clock.day_source.name(),
        "day_held": sky.day,
        "tick_hours": sky.state.clock.tick_hours,
        "sun": {
            "elevation_deg": sky.state.sun.elevation_deg,
            "azimuth_deg": sky.state.sun.azimuth_deg,
            "declination_deg": sky.state.sun.declination_deg,
            "illuminance": sky.state.sun.illuminance,
            "up": sky.state.sun.is_up(),
        },
        "season": {
            "name": sky.state.season.name,
            "step": sky.state.season.step(),
            "steps": sky::SEASON_STEPS,
            "senescence": sky.state.season.senescence,
            "dormancy": sky.state.season.dormancy,
            "flush": sky.state.season.flush,
        },
        "ambient": sky.state.ambient,
        // Shot V7. `ambient` above is still the sky's own level, unchanged, and this is what the
        // eye did to it before the frame was drawn. `closure` is the measurement it came from and
        // `manual` says whether the measurement was used at all.
        "exposure": {
            "ambient": adapt.ambient,
            "stops": adapt.stops,
            "manual": adapt.manual,
            "held": sky.exposure,
            "closure": adapt.closure,
            "canopy_columns": sky.canopy.covered,
            "ground_columns": sky.canopy.ground,
            "lit_ground_lux": adapt.lit,
            "shade_fraction_was": adapt.ratio_was,
            "shade_fraction": adapt.ratio,
            "shade_floor": sky::SHADE_FLOOR,
            "applied": sky.on,
        },
        "remesh_chunks": sky.remesh_chunks,
        "remesh_ms": sky.remesh_ms,
        "note": "expression, not simulation: the day of the year is the run's tick over its year_len, and the hour, the latitude, the seasonal hues and the exposure are the viewer's. The exposure is the eye's adaptation and not a measurement of anything in the run: it reads how much of the ground is under a crown and lifts the ambient level towards a floor on the shadow-to-sun ratio, which moves an engine light and no field. It is 0 stops on an open site, 0 stops with the sun down, and 0 stops with the beauty pass off. The simulator computes light under a fixed 45 degree sun and has no time of day; moving this one changes no number in any run. With day_source = override the date is the viewer's too, held over a tick that has not moved: the sun and the leaf colour follow the held date, and the trees, the water, the burn scars and every overlay band on the frame are still the tick's.",
    })
}

/// `ecoview.stats` -- what the world is, what the last edit cost, where the timeline is, and which
/// overlay is on with what scale.
fn stats_method(
    In(_): In<Option<Value>>,
    site: Res<Site>,
    t: Res<Timeline>,
    ov: Res<OverlayState>,
    editor: Res<Editor>,
    sim: Res<Sim>,
    sky: Res<Sky>,
) -> BrpResult {
    Ok(json!({
        "cell_m": site.world.cell_m,
        "width": site.world.width,
        "depth": site.world.depth,
        "levels": site.world.levels,
        "chunks": site.world.chunk_count(),
        "drawn_chunks": site.entities.iter().filter(|e| e.is_some()).count(),
        "edits": site.edits,
        "undoable": editor.undo.len(),
        "crosshair": editor.target.map(|(x, y)| json!({
            "x": x,
            "y": y,
            "column": site.world.column(x, y).map(|(g, m, b)| json!({
                "ground_h": g, "medium": m, "building_h": b,
            })),
            // Shot S6: an agent digging a pond can read how much further it will go, and see that
            // it stopped, rather than inferring it from a `ground_h` that did not move.
            "dig_room_m": site.world.dig_room_m(x, y),
            "at_dig_limit": site.world.at_dig_limit(x, y),
        })),
        // The lattice floor and the limit on it, both in metres below the bundle's zero. `datum_m`
        // is 0 until something is dug, which is what keeps an undug world meshing to V0's bytes.
        "datum_m": site.world.datum_m(),
        "dig_limit_m": site.world.dig_limit_m(),
        "dig_limit_from_run": site.world.dig_limit_from_run(),
        "lowest_ground_m": site.world.lowest_ground_m(),
        "sim": sim_json(&sim),
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
        // The one gate that decides *whether* a plant is drawn, and whose answer it is. Since shot
        // S4 it is the run's `params.medium.<name>.plantable`, with the old name list kept only for
        // a bundle opened without a run -- and `from_meta` says which of the two is in force.
        "plantable": {
            "source": site.world.plantable.source,
            "from_meta": site.world.plantable.from_meta,
            "media": site.world.plantable.media,
            "sealed": site.world.plantable.sealed(),
        },
        // Shot S5's standing water. Every number is the run's except `voxels` and `min_drawn_mm`,
        // which are this viewer's lattice, and the note says so.
        "water": {
            "on": t.water,
            "drawn": t.water_applied.map(|w| w.0),
            "ground_cells": t.pond_site_cells,
            "wet_cells": t.pond_stats.wet,
            "drawable_cells": t.pond_stats.drawn,
            "max_mm": t.pond_stats.max_mm,
            "mean_wet_mm": t.pond_stats.mean_mm,
            "volume_m3": t.pond_stats.volume_m3,
            "voxels": t.pond_counts.1,
            "min_drawn_mm": overlay::POND_MIN_MM,
            "error": t.water_error,
            "note": "the run owns every depth here: water.bin is ponded depth on the ground grid, written by ecosim every snapshot. The viewer owns only the lattice -- water under min_drawn_mm is mapped but not drawn, and anything drawn is at least one ground cell deep.",
        },
        // Shot V6's beauty pass, and shot V8's held date. `day_of_year` is the run's unless it
        // is held, and everything beside it is the viewer's, so an agent reading this gets the
        // provenance the HUD line carries.
        "sky": sky_json(&sky),
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
        "ramp": ov.ramp.as_ref().map(|r| json!({
            "lo": r.lo,
            "hi": r.hi,
            "burnt": r.burnt,
            "source": r.source,
            "from_meta": r.from_meta(),
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
        "ramp": ov.ramp.as_ref().map(|r| json!({
            "lo": r.lo, "hi": r.hi, "source": r.source, "from_meta": r.from_meta(),
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
