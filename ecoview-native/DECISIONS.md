# ecoview-native decisions

Choices this component made that neither `docs/` nor the shot prompt settles. Shot V0 is a spike: what
follows is what the spike had to decide to take its measurements, not a design for the track.

## V0: what the crate is

**A third top-level component, not a workspace member.** `ecosim/` is a single crate with no root
`Cargo.toml`, so putting `ecoview-native/` in a workspace would change the simulator's build. It sits
beside `ecosim/` and `ecoview/` with its own pinned manifest, and depends on neither (V0-spike.md,
corrections 1 and 2). It reads the committed world bundle from disk the way `ecoview` does. There is no
trait, module or indirection here whose purpose is to make in-process linking easier later; CLAUDE.md's
"share no code and have no IPC" still stands, and amending it is not this shot's to do.

**The library half is engine-free, the binary half is not.** `default = ["viewer"]` pulls in Bevy;
`--no-default-features` leaves `bundle.rs`, `voxel.rs`, `mesh.rs` and `palette.rs`, which are plain Rust.
That split is what lets the CI gate — the mesh golden — run in about a minute with no GPU, no display
and no engine, while the same code is what the viewer draws.

**`rust-lld` is the linker on Windows** (`.cargo/config.toml`). The MSVC linker is the single largest
term in an incremental rebuild of a Bevy binary; with `rust-lld` the rebuild is about 7 s.

## Resolved versions

Pinned with `=` the way `ecosim/Cargo.toml` pins (correction 5). 547 crates resolve in total.

| Crate | Version | Why this one |
|---|---|---|
| `bevy` | `=0.19.1` | named by the prompt; `default-features = false` plus `3d`, `bevy_remote`, `png` |
| `bevy_remote` | 0.19.1 | comes from `bevy`'s feature, never named separately |
| `bevy_brp_extras` | `=0.22.6` | the version whose Bevy dependency is 0.19; supplies `brp_extras/screenshot` and `brp_extras/shutdown` |
| `binary-greedy-meshing` | `=0.5.2` | 62³ chunks padded to 64³, which is the layout `voxel.rs` builds |
| `serde` / `serde_json` | `=1.0.229` / `=1.0.151` | the bundle reader and the BRP client |
| `wgpu` | 29.0.4 | transitive; recorded because the frame-time numbers are its |

Toolchain: rustc 1.98.1, the version CI pins.

## Bevy 0.19 APIs worked around

Written down because they are all places where 0.18-era example code compiles into nothing useful:

1. **`RenderTarget` is a Component**, not a field of `Camera`. Headless render-to-image inserts
   `RenderTarget::Image(handle.into())` on the camera entity.
2. **`AmbientLight` is a Component too**, not a Resource. It goes on the camera.
3. **`DirectionalLight::shadow_maps_enabled`**, not `shadows_enabled`.
4. **`WindowResolution: From<(u32, u32)>`** — the float pair no longer converts.
5. **`WindowPlugin` and `Window` need `..default()`**; headless sets `primary_window: None`,
   `exit_condition: ExitCondition::DontExit` and `close_when_requested: false`, and disables
   `WinitPlugin` in favour of `ScheduleRunnerPlugin::run_loop(Duration::ZERO)`.

Also: `RenderAssetUsages` lives in `bevy::asset`; custom BRP methods register as
`RemotePlugin::default().with_method_main(name, system)` with handlers typed
`In<Option<Value>> -> BrpResult`, and `BrpExtrasPlugin::with_port` must be added after them.

## Two things that looked like bugs and were not

**A headless screenshot at frame 20 is empty because the PBR pipeline is still compiling.** The first
headless run produced a blank image, which a red clear colour and a debug `Cuboid` proved was neither a
winding nor a mesh fault: the same scene at frame 300 is complete. `--frames` therefore defaults to 300,
and `save_to_disk` finishes on the IO task pool, so exit waits for the PNG to exist rather than for a
frame count.

**The renderer was not stuck at 60 fps; the display was.** `--bench` first reported a suspiciously flat
59.9 fps. `PresentMode::AutoNoVsync` on the primary window moved it to ~292 fps. Any frame-time number
taken without that is a refresh-rate reading.

A third, in our own code: `bevy_remote` answers over HTTP with **chunked transfer encoding**, so the
reply body arrives as a hex length line wrapped around the JSON. `brp::dechunk` joins the chunks. Before
that the agent loop reported failure while the viewer was answering every call correctly.

## Conventions

- **The cube edge comes from the bundle** (`ground_cell_m`), never from a constant. The synthetic stress
  world is built at 0.25 m precisely so a hard-coded 0.5 would show up as a wrong-sized world (correction
  4, and `ecoview/DECISIONS.md` under "E3 block world"). `level_of(h, cell) = floor(h / cell + 1e-6)`
  and the top-voxel-only medium rule are E3's, ported unchanged.
- **Axes.** The mesher's buffer strides are 1 = north, `CS_P` = east, `CS_P²` = up; the viewer's frame is
  X east, Y up, Z north. A chunk at `(cx, cy, cz)` therefore has origin
  `[cx·CS·cell, cz·CS·cell, cy·CS·cell]`, and the mapping is otherwise 1:1 — no transform, no flip.
- **An edit remeshes the chunks its column spans, before and after, plus the 8 neighbours in x and y**,
  because a column's own change can uncover faces in the chunk beside it. That is 3 chunks for a typical
  ground edit and is what the single-edit number measures.
- **Three explicit BRP methods — `ecoview.stats`, `ecoview.camera`, `ecoview.edit` — instead of driving
  component reflection.** An agent that must discover a component schema retries; an agent given three
  documented method names does not. The agent loop's zero retries is that decision's measurement.

---

# Shot V1: the run directory, and time

## V1 only version 4

`Run::load` rejects any `format_version` but 4, by name, rather than reading what it recognises.
Version 4 is the first that records the ground grid in `meta.json` (CLAUDE.md, "The run directory
contract"), and without it there is no way to tell whether a run and a bundle describe the same site.
That is the one thing here that must not be guessed at: a run laid over the wrong ground puts trees in
the air with nothing on screen to say so. `Run::check_against` compares the ground grid, the cell size,
the ecology grid's extent in metres and the world's name, and refuses with a sentence naming both sides.

The refusal is a refusal, not a warning that draws anyway: `--run` with a run that does not fit prints
why and the viewer carries on showing the bundle alone. A picture that quietly lies is worse than no
picture.

## V1 whose trees these are

A run's vegetation **replaces** the bundle's, rather than adding to it. The bundle's trees are the site
as it was surveyed; the run's are the same site as the simulator grew it. Drawing both stands two trees
in every spot and reads as neither. Bundle shrubs go with them for the same reason — the simulator has
shrubs of its own as a patch field, and V1 does not draw patch fields, so a snapshot's ground is bare
where the bundle's was planted. That is honest about what the simulator actually decided.

## V1 the simulator owns the tree, the viewer owns the metre

`entities.json` gives a tree a `stage` and an `age` and nothing dimensional, because ecology happens on
a 1 m grid where a tree is one to three voxels tall. So `Stage::shape` turns sapling, young and mature
into a height, a crown base and a crown radius **in metres** — the same three shapes `ecoview` draws
(`ecoview/src/entities.ts`, `canopyVoxels`), read off its 1 m voxels and written as lengths. The
viewer's own lattice then draws them at whatever the bundle's cell size is. No height is invented that
the simulator never computed, and the viewer does not push the simulator to 0.25 m to get one
(`overnight/DIRECTION-native-viewer.md`). Shot V3 replaces all of this with branching geometry.

A tree's `z` is **not** used. That is the ecology grid's surface level in whole metres, and the terrain
under it is drawn on the bundle's finer lattice, so a tree placed at the ecology level floats or sinks
by up to a metre against ground the user can see. The base comes from the viewer's own column, exactly
as it already does for bundle trees.

An unknown `stage` is **counted and not drawn**. `ecoview.stats` and the HUD report the count. A stage
the simulator adds should appear as a number that is not zero, not as a guess at what it might look
like.

## V1 P, not space

Play/pause is `P`. Space already flies the camera up and takes precedence — this is a flying viewer
first. The rest of the timeline keys are `,`/`.` to step a snapshot, `Home`/`End` for the ends, `[`/`]`
for the play rate, and the bar at the bottom of the screen is draggable anywhere along its width. With
no run loaded there is no bar at all: an empty track offers a scrub that would do nothing, so the HUD
says "no run loaded" in words instead.

## V1 the wheel was backwards for a measurable reason

V0's scroll wheel made the camera slower when pushed away from you, which is the wrong way round. The
cause is not taste: `src/main.rs` copied `ecoview/src/edit.ts:348` verbatim, and a DOM wheel event's
`deltaY` is positive scrolling **towards** the user while Bevy's `AccumulatedMouseScroll.delta.y` is
positive scrolling **away**. The same expression, on the other engine, inverts. Fixed by flipping the
comparison, and the sign convention is written beside it so the next port does not re-import it.

## V1 the HUD needs `IsDefaultUiCamera` to exist headless

Not a preference, a finding, recorded because it costs an hour to rediscover. `bevy_ui`'s
`DefaultUiCamera::get` only falls back to a camera whose `RenderTarget` is a **window**. The headless
camera renders to an image, so no root node is ever assigned a camera, and the entire UI is laid out
nowhere — no error, no warning, just a screenshot with no HUD on it. Marking the camera
`IsDefaultUiCamera` fixes it in both modes. The other Bevy 0.19 surprise beside it: `font_size` is now
`FontSize::Px(..)`, not an `f32`.

And one in our own code, the same shape as V0's chunked-transfer bug: `--headless` exits once the
screenshot path exists, so a **stale** PNG from a previous run makes it exit before the new capture is
sent, and the run ends with `Failed to send screenshot: sending on a closed channel`. The path is now
deleted at startup. Waiting on a path is only a signal if the path starts empty.

## V1 the new tests live in `mesh_golden.rs`

The six run-directory and `set_plants` tests are appended to `tests/mesh_golden.rs` rather than given a
file of their own, because the CI gate runs exactly one target —
`cargo test --release --no-default-features --test mesh_golden` — and a second file means editing
`.github/workflows/ci.yml`, which belongs to a `ci` row and not to a viewer shot. The file is still
engine-free: all eleven tests compile with `--no-default-features` and none of them touches a GPU.

## V1 three clippy findings left alone on purpose

`cargo clippy --release --all-targets -- -D warnings` reports three, all of them in `src/bundle.rs` and
all of them older than this shot: a `chunks_exact` suggestion in `read_f32` and two `approx_constant`
hits on the `6.28` in `Bundle::stress`. The TAU ones are not typos to fix — `6.28` is the literal that
generated the stress world every measurement in `MEASUREMENTS.md` was taken on, and replacing it with
`TAU` changes the terrain, the mesh, the triangle counts and the bytes of the lavapipe screenshots. A
shot that was asked for a timeline does not get to move the baseline everything else is compared
against. V1 verified it changed neither line.
