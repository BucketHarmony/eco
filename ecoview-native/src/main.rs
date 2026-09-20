//! The V0 viewer: a world bundle as voxels, a fly camera, one overlay, one edit action set, and a
//! remote-control surface an agent can drive.
//!
//! Usage:
//! ```text
//! ecoview-native [--world DIR | --stress] [--headless] [--frames N] [--screenshot PATH]
//!                [--bench SECS] [--port N]
//! ```

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

use ecoview_native::mesh::{mesh_chunk, ChunkMesh, Scratch};
use ecoview_native::palette::surface_palette;
use ecoview_native::voxel::{ChunkPos, EditAction, VoxelWorld};
use ecoview_native::{brp, stress_world, Bundle, CAPITOL};

/// Fly speed and its wheel step, copied from `ecoview`'s `edit.ts` so the two viewers feel the same.
const FLY_SPEED: f32 = 12.0;
const FLY_SPEED_MIN: f32 = 1.0;
const FLY_SPEED_MAX: f32 = 120.0;
const FLY_SPEED_STEP: f32 = 1.25;
const MOUSE_SENSITIVITY: f32 = 0.002;

#[derive(Resource, Clone)]
struct Args {
    world: String,
    stress: bool,
    headless: bool,
    frames: u32,
    screenshot: Option<String>,
    bench: f32,
    port: u16,
}

fn args() -> Args {
    let mut a = Args {
        world: CAPITOL.to_string(),
        stress: false,
        headless: false,
        frames: 300,
        screenshot: None,
        bench: 0.0,
        port: brp::PORT,
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

fn main() {
    let a = args();
    let load = Instant::now();
    let bundle = if a.stress {
        stress_world()
    } else {
        Bundle::load(std::path::Path::new(&a.world))
            .unwrap_or_else(|e| panic!("cannot read world bundle {}: {e}", a.world))
    };
    let voxelise = Instant::now();
    let world = VoxelWorld::from_bundle(&bundle);
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
            .with_method_main("ecoview.stats", stats_method),
    )
    .add_plugins(BrpExtrasPlugin::with_port(a.port))
    .insert_resource(Site {
        world,
        entities: Vec::new(),
        material: Handle::default(),
        last_remesh_ms: 0.0,
        edits: 0,
    })
    .init_resource::<EditQueue>()
    .init_resource::<CameraQueue>()
    .insert_resource(Bench {
        until: a.bench,
        samples: Vec::new(),
    })
    .insert_resource(a.clone())
    .add_systems(Startup, setup)
    .add_systems(Update, (apply_edits, move_camera, fly_camera, bench_frames));
    if a.headless {
        app.add_systems(Update, headless_frames);
    }
    app.run();
}

/// Builds every chunk mesh on the compute task pool, then spawns one entity per non-empty chunk.
fn setup(
    mut commands: Commands,
    mut site: ResMut<Site>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    args: Res<Args>,
) {
    let palette = surface_palette();
    let chunks = site.world.all_chunks();
    let t = Instant::now();
    let built = mesh_all(&site.world, &chunks, &palette);
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
    println!("mesh: {mesh_ms:.0} ms for {} chunks, {drawn} drawn, {quads} quads", chunks.len());

    let size = site.world.width as f32 * site.world.cell_m;
    let eye = Vec3::new(-0.25 * size, 0.45 * size, -0.25 * size);
    let look = Vec3::new(0.5 * size, 0.0, 0.5 * size);
    let mut cam = commands.spawn((
        Camera3d::default(),
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
}

/// Meshes a list of chunks in parallel on Bevy's compute pool, one scratch buffer per task.
fn mesh_all(
    world: &VoxelWorld,
    chunks: &[ChunkPos],
    palette: &[[f32; 4]; ecoview_native::voxel::ID_COUNT],
) -> Vec<ChunkMesh> {
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
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
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
    let palette = surface_palette();
    let mut scratch = Scratch::new();
    for (x, y, action) in std::mem::take(&mut queue.0) {
        let t = Instant::now();
        let stale = site.world.apply(x, y, action);
        let material = site.material.clone();
        for c in stale {
            let i = site.world.chunk_index(c);
            let m = mesh_chunk(&site.world, c, &palette, &mut scratch);
            match (site.entities[i], m.is_empty()) {
                (Some(e), false) => {
                    commands
                        .entity(e)
                        .insert(Mesh3d(meshes.add(to_bevy_mesh(&m))));
                }
                (Some(e), true) => {
                    commands.entity(e).despawn();
                    site.entities[i] = None;
                }
                (None, false) => {
                    let e = commands
                        .spawn((
                            Mesh3d(meshes.add(to_bevy_mesh(&m))),
                            MeshMaterial3d(material.clone()),
                        ))
                        .id();
                    site.entities[i] = Some(e);
                }
                (None, true) => {}
            }
        }
        site.last_remesh_ms = t.elapsed().as_secs_f64() * 1000.0;
        site.edits += 1;
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
    mut cam: Query<(&mut Transform, &mut Fly)>,
) {
    if windows.is_empty() {
        return;
    }
    for (mut t, mut fly) in &mut cam {
        if scroll.delta.y != 0.0 {
            fly.speed = (if scroll.delta.y > 0.0 {
                fly.speed / FLY_SPEED_STEP
            } else {
                fly.speed * FLY_SPEED_STEP
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
    let x = p["x"].as_u64().ok_or_else(|| brp_err("x must be an integer"))? as usize;
    let y = p["y"].as_u64().ok_or_else(|| brp_err("y must be an integer"))? as usize;
    let name = p["action"].as_str().ok_or_else(|| brp_err("action must be a string"))?;
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

/// `ecoview.stats` -- what the world is, and what the last edit cost.
fn stats_method(In(_): In<Option<Value>>, site: Res<Site>) -> BrpResult {
    Ok(json!({
        "cell_m": site.world.cell_m,
        "width": site.world.width,
        "depth": site.world.depth,
        "levels": site.world.levels,
        "chunks": site.world.chunk_count(),
        "drawn_chunks": site.entities.iter().filter(|e| e.is_some()).count(),
        "edits": site.edits,
        "last_remesh_ms": site.last_remesh_ms,
    }))
}
