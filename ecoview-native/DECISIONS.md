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
