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

# Shot V2: the ecological overlays

## V2 an overlay is a set of voxel ids, not a colour per column

The mesher merges neighbouring faces that share a voxel id. A per-column colour would give it nothing
to merge and turn a 79,504-quad site into one quad per ground cell — 262,144 on the Capitol at 0.5 m.
So each overlay is quantised into **32 bands**, the bands get their own ids above the scene contract's
media (`BAND_BASE = ID_COUNT`), and the palette grows from 13 entries to 45. Merging survives inside a
band, and the cost is one band boundary's worth of extra quads: the worst overlay on the Capitol is
moisture at 97,046 quads against surface's 79,504, and three of the six are *cheaper* than the surface
map (MEASUREMENTS.md). 32 bands over a 0–255 field is 8 index units a band, below what the eye
separates on a lit surface.

Only the top voxel of a ground column carries the band. Everything under it stays soil, so taking the
overlay off restores the surface mesh exactly — asserted by hash in
`set_overlay_reports_exactly_the_chunks_whose_mesh_changed`.

## V2 what `meta.json` owns, and the one thing it does not

The row asks for overlays "coloured from `meta.json` so the simulator still owns the palette". The run
directory format has no overlay palette in it, and adding one is an `ecosim` change an
`ecoview-native` shot may not make (MASTER.md, component isolation). So ownership is split, and the
split is stated rather than blurred:

- **Every number comes from the run.** Temperature's ends are the union of `params.{grass,shrub,tree}
  .temp`, the species tolerance curves — outside them every curve is zero, so that is exactly the
  interval where a colour means anything. Crowding's top is `2 × params.disease.grazer_threshold`.
  Fire's is `params.fire.duration`. Light, moisture and fertility come from the file format itself
  (`ecosim/UNITS.md`): 255 × the fraction of full sun, 255 × soil water / AWC, and the 0–255 index
  UNITS.md still marks deferred.
- **The two species colours come from the run.** `palette()` takes the trunk from the tree species'
  `color` and the canopy from its `canopy_color`. V0 hard-coded both, and got one wrong: the constant
  said `#3f7a2e` and `meta.json` says `#2e8b3d`. With a run loaded the file wins, and
  `the_scale_and_the_species_colours_come_from_meta_json` fails if that ever regresses.
- **The two ramp hues are the viewer's**, copied from `ecoview/src/world.ts` so the two viewers ramp
  the same way — copied, not shared, because the components share no code. This is the piece
  `meta.json` cannot yet supply, and a worker note in `overnight/BACKLOG.md` says so, because
  publishing an overlay palette is an `ecosim` row.

A run whose `meta.json` carries no `params` still draws. It falls back to `ecoview`'s constants and
**says so on screen** — `Scale::source` reads "this viewer's fallback: meta.json has no …" and the HUD
marks the line `(!)`. A silent constant is the failure this is built to avoid.

## V2 the scale is the simulator's range, never the data's

The ramp is not stretched to the values present at the tick being viewed. If it were, the same colour
would mean different things at different snapshots and the timeline would be unreadable. The cost is
that a field occupying a small part of its range looks flat: temperature on `runs/capitol-s42` spans
10.31–12.00 °C inside a −5…35 °C scale, and the picture is one shade of magenta. So the HUD and the
headless stdout line always print the field's **actual** min, max and mean beside the scale. A flat
picture and a narrow range are the same fact, and only one of them is visible.

## V2 fire is three things, not one ramp

Fire's band 0 is quiet ground and band 1 is ground that **burnt out since the previous snapshot**,
both off the ramp; the orange ramp covers the burning bands only, so a patch with one tick left is
already dull orange rather than a third of the way up a ramp whose bottom means "not on fire". The
burn scars are read once at load from `events.csv`'s `burnout` rows — 600 of them in the 1.6 MB file
of the 20,000-tick Capitol run, so filtering once beats re-reading at every scrub. A missing
`events.csv` is not fatal: the overlay then shows what is alight and nothing that has already burnt,
which is a smaller picture and not a wrong one.

This is also why the headless line carries the fire counts. At tick 10000 the field is `0.00..0.00` —
nothing is alight — while 155 patches are drawn burnt. The stats alone would have reported an empty
overlay over a picture full of scars.

## V2 light is sampled one voxel above the surface

`light.bin` is a voxel field, and the surface voxel is ground: it is dark everywhere. The sample is
`height[c] + 1`, the voxel a seedling would sit in, which is what `ecoview` does in `world.ts`. The
test writes 99 into every surface voxel and asserts it appears nowhere in the sampled field.

## V2 animals are written at continuous positions

Found while measuring, not while coding. V1's `entities.json` reader took `x` and `y` as `i32`; the
simulator writes animals at continuous positions (`"x":82.0`, and fractions between columns). Every
Capitol run has `animals.enabled=false`, so this parsed for two shots and then failed on the first run
with a grazer in it — and because serde fails the whole array, it took the trees with it. Both readers
now take floats and floor them to a column. The V2 test fixture writes its grazers at `1.75`, `0.5`
and `2.5` for that reason, and asserts the trees in the same file still load.

## V2 switching an overlay on remeshes the site; scrubbing under one does not

Turning an overlay on changes the top voxel of every ground column, so every chunk really is stale and
`main.rs` rebuilds all of them — 28–38 ms for the Capitol's 324 chunks. Changing snapshot under an
overlay is the common case and touches only what the field changed: 162 chunks in 2–3 ms. `set_overlay`
compares the resampled band map against the previous one and dilates only the cells that sit on a
chunk's boundary layer, and the test hashes every chunk before and after to prove the reported set
covers every mesh that moved.

## V2 the new tests live in `mesh_golden.rs` too

Nine more, for V1's reason: the CI gate runs exactly one target, and a second file means editing
`.github/workflows/ci.yml`, which belongs to a `ci` row. The file is still engine-free — all twenty
tests compile with `--no-default-features`. `golden_banded` is a fourth golden hash, taken on a chunk
meshed under an overlay, so a change in the band ids, the resampling or the ramp fails a test rather
than quietly changing a screenshot.

## V3 the row names five inputs, and the run directory has four

The row asks for geometry "from seed, species, age, biomass and light". Four of those exist; the
fifth does not, anywhere:

| Input | Where it comes from |
|---|---|
| seed | `meta.json`'s `seed`, mixed with the tree's `id` from `entities.json` — so the same tree is the same shape at every tick and in every process, and two trees of the same age differ |
| species | `params.tree`'s stage ages and the species colours already in `meta.json` |
| age | `entities.json`'s `age` in ticks over `meta.json`'s `year_len` |
| light | `light.bin` at the tree's own column |
| **biomass** | **nowhere.** `entities.json` carries `id`, `x`, `y`, `z`, `age`, `stage` and `lifespan`. The simulator holds no mass, diameter or leaf area for a tree |

So the viewer does not invent one. Age through the simulator's own height curve is the only
dimensional quantity that is genuinely the simulator's, every derived quantity is a stated fraction of
it, and the HUD names the source of each number the way V2's overlays do. A worker note in
`overnight/BACKLOG.md` proposes the `ecosim` row that would publish a real per-tree size — inventing a
biomass here would have put a number on screen that no invariant in `ecosim check` can contradict.

## V3 height comes from ecosim's own age curve, inverted, with no vigour term

`Life::height_of` is `ecosim/src/plants.rs::import_age` run backwards: linear from 0 to
`bundle.tree_mature_height` over the sapling and young stages, then to `tree_tall_height` at
`tree_tall_age_years`, then flat. Inverting the simulator's own function is what makes the tick-0
picture agree with the bundle's survey to the metre (MEASUREMENTS.md), because that survey is what
`import_age` consumed in the first place.

It was tempting to scale height by `lifespan / max_age`, so a short-lived tree would look stunted.
Rejected: that agreement is the one check this model has, and a vigour term would break it for a
prettier idea the simulator does not hold. `params.bundle` is absent from `meta.json` at the defaults
(ecosim's `skip_serializing_if`), so on every reference run the curve's breakpoints are this viewer's
fallback and the HUD says so on its own line.

## V3 the light sample is ecosim's `surface_light`, not the light where the leaves are

Measured and rejected, in that order (MEASUREMENTS.md): sampling `light.bin` at the middle of the
procedural crown returns 1.00 of full sun for every tree at every tick, because the simulator's canopy
is one to three voxels tall and the procedural crown's middle is 13.7 m of open sky above it. The
sample is `light[height[c] + 1]` — V2's surface sample, and the number the simulator's own germination
and growth curves read. Its doc comment records the rejected alternative so the next shot does not
re-derive it.

The direction it drives is deliberately one-way: light sets **crown density**, never height or limb
count. Height is the simulator's (above); a shaded tree in this viewer is a thinner tree, which is
expression of a number the simulator already acted on, not a second simulation of shading on top of
it. `CrownLight` falls back to full sun with a named fallback string when `light.bin` or `height.bin`
does not match `dims`.

## V3 the envelope is an ellipsoid and a tip is pulled along its radius

Crown radius is 0.30 of height and crown base 0.37 of it — the means over the Capitol bundle's 81
surveyed trees, measured rather than chosen, so the procedural crowns sit where the survey's do.

The first version clamped a branch tip per axis (x and z to the crown radius, y to the height) and
every tree over 10 m rendered as a bare post: three generations of branch, each rising about three
quarters of its own length, land all 27 tips at the apex, where the ellipsoid is a point, and the leaf
clip threw their blobs away. `pull_into_envelope` scales a point along the envelope's own radius to
`TIP_FRAC` = 0.78 of the wall instead, so a leaf blob centred on a tip is mostly inside the crown, and
the upward bias dropped from 0.30 to 0.18. The measurement is in MEASUREMENTS.md; the lesson is that a
clamp in the wrong coordinates is invisible in the test suite and obvious in one close-up, which is
why this shot has a close-up.

## V3 no transcendental function appears in the tree model

The golden hash is taken on Windows and checked in CI on Linux, so the model uses `f32::sqrt` and
nothing else: branch directions come from **rejection sampling a cube** rather than from `sin`/`cos`,
and there is no `powf` or `exp` anywhere. `ecosim` solved the same problem by routing its trig through
`libm`; this component has no such dependency and does not need one. `golden_procedural_tree` would be
a cross-platform flake if it did.

## V3 wood beats leaves on a shared voxel, and a small crown gets a floor

Limbs are stamped as capsules of `TRUNK` at half-cell steps and a leaf blob of `CANOPY` at each tip.
`fill_chunk` writes only into `AIR` and the id buckets are sorted, so `TRUNK` (11) wins over `CANOPY`
(12) wherever a blob covers a limb — a branch stays visible through its own foliage, which is the
whole point of replacing the solid ellipsoid. The leaf radius has a floor of `0.9 × cell_m`: without
it a sapling's three tips are three single voxels, which is both invisible and the worst case for the
greedy mesher (six unmergeable quads each).

## V3 a run tree gets headroom above the bundle's survey

`VoxelWorld::from_bundle` now delegates to `from_bundle_with_headroom`, and `main.rs` passes
`life.tall_height_m`. The chunk grid's height used to be the bundle's own tallest feature, so a run
tree taller than anything surveyed would have been silently beheaded at the top chunk. The Capitol
still reports 324 chunks and 214 levels, because its dome is taller than 20 m; a flatter bundle would
have grown a chunk layer, which the test `a_run_tree_gets_room_above_the_bundle` asserts.

## V3 `Stage::shape` is gone, and its test with it

V0 mapped a stage to a shape (a sapling was a stick, a mature tree an ellipsoid). Stage no longer
decides geometry — age does, continuously — so the function is deleted rather than left unused, and
`Stage` keeps only `index()`, which the HUD's three counters need.
`the_three_stages_are_three_sizes` is replaced by `a_stage_is_parsed_or_refused`, which asserts what
the field still does: name a stage or fail the load. Deleting a test is worth a line here; this one
asserted a behaviour the row asked to remove.

## V3 `RunMeta` derives `Default` but is not `#[serde(default)]`

`Life::default()` needs a `RunMeta`, so `RunMeta` and `Dims` derive `Default`. A struct-level
`#[serde(default)]` would have been the easy way to give `year_len` a fallback and would also have made
a `meta.json` with no `dims` parse as a silent 0 × 0 world. `#[serde(default)]` sits on `year_len`
alone, and a doc comment says why it is not on the struct.

## V3 the new tests live in `mesh_golden.rs`, for the third time

Nine more, 29 in all, every one compiling with `--no-default-features`: the CI gate runs exactly one
target and a second file means editing `.github/workflows/ci.yml`, which belongs to a `ci` row.
`golden_procedural_tree` is a fifth golden hash, on a chunk holding one tree grown from a run, so a
change in the branching, the height curve, the light sample or the leaf radius fails a test instead of
quietly changing every screenshot. The four earlier goldens are untouched, which is the evidence that
this shot changed plants and nothing else.

# Shot V4: ground cover and vines

The row: *"Native viewer: vines and fine ground cover as expression -- climbers on wall voxels driven
by the simulator's moisture, shade and cover; grass and shrub as fine voxel texture. Expression, not
simulated competition: say so in every report that shows one."* There is no prompt file for the V
rows, so the row is the specification (MASTER, step 5).

## V4 what "expression, not simulation" is actually enforced by

The phrase is easy to write in a report and easy for a picture to contradict, so it is worth being
precise about what holds it up here. Three things do:

1. **Nothing in `src/cover.rs` has state.** `Cover` is built from one snapshot's fields and thrown
   away. There is no accumulator, no previous tick, no growth and no death. Rebuilding the cover for
   tick 10000 after visiting tick 20000 gives the same voxels, byte for byte, because the only
   inputs are that snapshot's four numbers and the run's seed.
2. **Nothing the viewer computes goes anywhere.** No file is written, no field is fed back, and the
   simulator is a separate process that has already finished. The run directory is the only
   interface between the two projects and it is read-only here (CLAUDE.md).
3. **The words are in the output, not just in the docs.** The HUD, the stdout line a scripted run
   leaves behind, and `ecoview.stats` over BRP all carry the counts *and* the sentence. A screenshot
   travels further than a report; the caveat is inside the frame.

What that leaves genuinely driven by the simulator is: how much of a patch is grass and how much is
shrub, how wet each column is, and how shaded it is. What the viewer adds is which of the patch's
ground cells show it, how tall a shrub is drawn, and how far up a wall a climber gets. The second
list is the viewer's and is labelled as such everywhere it appears.

## V4 the four drivers, and why those four

| Driver | File | Grid | Used for |
|---|---|---|---|
| grass, shrub | `patches.json` | 8 m patch | which cells carry a blade; gates the vines |
| moisture | `moisture.bin` | 1 m column | vine vigour |
| light | `light.bin` at `height[c] + 1` | 1 m column | vine vigour, as `1 - light` |

The row names moisture, shade and cover, and those are exactly the three the run publishes on a grid
the viewer can resample. The light sample is the same one shot V3 settled on and the same one the
simulator's own germination and growth curves read — `surface_light`, one voxel above the surface —
so the shade a vine answers to is the shade the simulator computed, not a number invented here.

`patches.json` already had `grass` and `shrub` in it; `overlay.rs` simply stopped ignoring them, on
the pass it was already making. No file, field or format version changed, and nothing in `ecosim/`
was touched.

## V4 a vine grows on the open side of a wall, not on the wall

The obvious implementation puts vine voxels in the building's own columns. Those voxels are never
drawn: `fill_chunk` writes the terrain and the building first and a plant only fills air. So a vine
is rooted in the **open ground cell beside** a wall and climbs the air column there, which is also
the physically right answer — a climber is on the outside of a building, not inside it.

One consequence worth stating: the vine is one cell thick and stands a cell away from the wall face,
so at the Capitol's 0.5 m it reads as a band of foliage against the stone rather than as a skin on
it. That is the resolution the bundle has, not a stylistic choice.

The climb is capped at the wall's top level, so a garden wall disappears under a vine and the
Capitol's dome does not. `VINE_REACH_M` is 12 m at full vigour, which is the viewer's number: the
run says nothing at all about climbers, because the simulator has none.

## V4 sealed ground grows nothing, and that rule is copied rather than shared

A cell of concrete, asphalt, roof or open water carries no grass, no shrub, and no vine on the wall
beside it. That is the simulator's own `Medium::is_sealed` (`ecosim/src/bundle.rs`) plus open water,
matched here on the **name** the bundle publishes rather than on a numeric code, and copied into
`VoxelWorld::grows` rather than shared — the two projects share no code (CLAUDE.md). A medium this
viewer does not recognise grows things, which is the harmless way to be wrong.

This is the only place the viewer decides *whether* a plant is there rather than where. It earns its
place twice: it is what makes the Capitol's paths and forecourt read as paths under the cover, and
it is what makes an edit legible — pave a lawn over BRP and its blades and the climbers on the wall
beside it both go.

## V4 one uniform per cell, partitioned, not one draw per species

A cell draws a single number in `[0, 1)`. The patch's shrub fraction takes the bottom of the
interval and grass takes what is left under it. Two independent draws would have let a cell come up
grass *and* shrub, and a cell cannot hold two plants; picking one at random afterwards would have
made the realised fractions something other than the run's.

Where a patch's grass and shrub sum past 1 — which happens on this run — it is **grass** that loses
the cell. That is the same direction as the simulator's own grass suppression, arrived at here for a
different reason, and it is why the drawn grass fraction can sit below the reported one. The
measured numbers are in MEASUREMENTS.md.

## V4 a field overlay takes the ground cover off and leaves the vines on

An overlay is a map of the ground. At tick 10000 this run is 64% grass and 28% shrub, so leaving the
cover on would have painted most of a moisture map green and made it a map of the grass instead.
Turning it off is not a compromise: the overlay is the picture the user asked for.

The vines stay, for two reasons. No overlay colours a wall, so they cost the map nothing. And the
moisture and light maps are exactly the two fields that explain where the climbers are — the one
view where a vine and its cause are on screen together. The HUD says which of the two is happening:
*"(ground cover hidden under the overlay)"*.

## V4 three voxel ids, in precedence order

`VINE` 13, `SHRUB` 14, `GRASS` 15, so `ID_COUNT` is 16 and `BAND_BASE` moves from 13 to 16 with it.
The ids are in precedence order on purpose: where two plants want the same voxel the lower id takes
it, because the buckets sort on `(x, z, y, id)` and `fill_chunk` writes the first entry. So wood
shows through leaves, leaves through a vine, and a vine through the ground cover it is rooted in —
V3's rule, extended rather than replaced.

Moving `BAND_BASE` does not move the V2 golden hash. A band's colour is indexed from `ID_COUNT`, so
the whole ramp shifted with it and the colours a chunk's vertices get are unchanged. The five
existing goldens are all untouched, which is the evidence that this shot adds a layer and does not
change the site under it — and `--no-cover` at tick 10000 meshes to 162,966 quads, V3's number
exactly.

## V4 grass and shrub take the run's colours; a vine takes the viewer's

`meta.json`'s species table names five species and gives each a colour. Two of them are the ground
covers, so `palette()` substitutes them the same way it already substituted the trunk and canopy —
matched on `name`, because both have `kind: "cover"` and the kind cannot tell them apart.

There is no climber in that table, because the simulator has no climbers. Dressing a vine in another
species' colour would let a screenshot imply the run grew it, so it gets a hue of its own,
`VINE_HEX` `#3d7d2e`, a little yellower than the canopy, named in `palette.rs` as the viewer's with
the reason beside it. It is the only plant colour in this viewer that is not the simulator's.

## V4 the snapshot's fields are read once, for the cover and the overlay both

Before this shot `apply_overlay_bands` read the snapshot's fields itself. The cover wants the same
three files, and `light.bin` alone is 2 MB on the Capitol, so a scrub under a field overlay would
have read it twice. `read_fields` now reads once in `apply_world_state` and hands the result to
both. The error is carried as a `String` rather than an `io::Error` so one value can go to two
callers; the overlay's failure behaviour — take the overlay off, put the reason on the screen — is
unchanged.

## V4 toggling the cover goes down the scrub path

**V** and `--no-cover` rewrite the same voxels a snapshot change does, so `Timeline` gained
`cover_applied` and `apply_world_state` treats a cover change exactly like a scrub: one
re-voxelisation, one diff, one remesh of the chunks that actually differ. There is no second code
path and no third kind of staleness. The cost is that toggling the cover re-voxelises the trees too,
which MEASUREMENTS.md measures rather than assumes.

## V4 `--eye` and `--look`, so a close-up is re-runnable

The headless screenshot path had one camera pose, computed from the site's size. A picture of a wall
with a climber on it could therefore only be taken by hand, and a shot report that says "look at
this" has to be a command someone else can run. Two arguments, `x,y,z` in metres, defaulting to the
pose `setup` already chose; a pose that does not parse is fatal, for the same reason a mistyped
overlay is — a screenshot script would otherwise file the overview picture under the close-up's
name. The exact commands are in MEASUREMENTS.md.

## V4 the new tests live in `mesh_golden.rs`, for the fourth time

Nine more, 38 in all, every one compiling with `--no-default-features`: the CI gate runs exactly one
target and a second file means editing `.github/workflows/ci.yml`, which belongs to a `ci` row.
`golden_cover` is a sixth golden hash, on a chunk holding a covered lawn and a wall with climbers on
it, so a change to the scatter, the climb or the three colours fails a test rather than quietly
changing every screenshot. No transcendental function appears in the cover model — the draw is
`tree.rs`'s integer `mix` and the climb is one multiply and a `round` — so that hash is the same on
Windows and on Linux, the same argument V3 made.

## V5 the simulator is started as a command, and the run directory is still the whole interface

The row asks for E4's round trip natively, and E4 ran the browser's helper as a child process. This
does the same thing: `SimJob::start` writes the edited site out as a world bundle, spawns
`ecosim run` on it, and loads the run directory it writes. No code is shared with `ecosim` and no
IPC is used — CLAUDE.md forbids both — so the only thing crossing between the two processes is the
directory on disk and the child's exit status.

Two consequences of that choice are worth naming. **Both of the child's streams go to a file**, not
to a pipe: a pipe nobody drains fills its buffer and stops the child, and the viewer only reads it
once per frame, so the failure would have looked like a hung simulation. And **progress is counted
from the snapshot directories on disk** rather than parsed from stdout, because the number of
`snap_NNNNNN/` directories is part of the format contract and a progress line is not.

## V5 the edited site is written to a scratch bundle, never back over the source

`Bundle::save` refuses to write into the directory its bundle was read from, by comparing the two
paths. `ecosim/worlds/capitol/` is a committed reference world whose `medium.u8` is ODbL data with a
credit attached; a viewer that could overwrite it in place while someone was dragging a key down is
a viewer that will eventually do it. Each round trip gets `sim-<millis>/world` and `sim-<millis>/run`
under `--sim-root`, and nothing else is ever written.

The saved bundle recounts its own `trees`, `shrubs` and `pipes` rather than copying the source's
counts, and records `"edited_by"` with the number of edits behind it. A bundle that says where it
came from is the difference between a run somebody can explain later and one they cannot.

## V5 every path handed to the child is absolute

The child's working directory is the scratch directory, so a relative `--world worlds/capitol` means
something different to it than it does to the viewer — the first round trip failed with os error 3
for exactly that reason. `std::path::absolute` rather than `canonicalize`: the run directory does not
exist yet when the command line is built, and on Windows canonicalising would hand the simulator a
`\?\` path it has no reason to have to understand.

## V5 the command line is the one a person would type

`--seed`, `--ticks`, `--snapshot-every`, `--snapshot-state false`, `--set animals.enabled=false`,
`--set climate.rain_gradient=0`, `--params ../ecosim/params.toml`. The two `--set` flags are the
garden-series direction, not this shot's invention. The whole line is printed to stdout when the run
starts and returned by the `ecoview.sim` BRP method, so anyone who doubts a picture can run the same
command in a terminal and compare directories. That is the honesty the file-on-disk interface buys,
and it is worth two lines of code to keep it.

`ECOSIM_BIN` and `ECOSIM_PARAMS` override the binary and the parameters, which is how the tests
drive the whole post-spawn path with a stand-in child, and how CI would drive it if a later row ever
puts the round trip in a job.

## V5 ten snapshots, whatever the run is worth

`snapshot_every(ticks)` is `ticks / 10` clamped to at least 1, so a 1,000-tick run and a 20,000-tick
run both come back with eleven stops on the timeline. The alternative — a fixed interval — makes a
short run a single frame and a long one a thousand, and the timeline's play rate is per snapshot, so
the same keypress would mean four seconds in one case and eight minutes in the other. The cost is
that the snapshot tick is not a round number for an arbitrary run length, which nothing depends on.

## V5 undo stores the column that was there, not the inverse action

`RaiseGround` and `LowerGround` clamp at the site's floor and ceiling, and `SetSurface` throws the
old medium away, so an inverse action does not restore a state — undoing six lowers at the floor
would raise the ground above where it started. Each edit therefore pushes the three numbers that
made up that column (`ground_h`, `medium`, `building_h`) onto a 512-deep stack, and undo writes them
back. It is more memory per edit and it is the only version that is correct.

512 because a `--edit` rectangle is thousands of cells and an undo stack that swallows them all is a
memory leak with a nicer name; the HUD prints how many steps are actually undoable, so the limit is
visible rather than surprising.

## V5 the whole edit queue is drained before anything is remeshed

The first version remeshed after each cell, which made `--edit 80,150,119,189,LowerGround` — 1,600
cells over four chunks — 1,600 remeshes of the same four chunks and took it out of interactive time
entirely. The queue is now drained in one go, the touched chunks are deduplicated, and one remesh
loop runs afterwards. A held key and a scripted rectangle go down the same path, which is why the
scripted one is fast: the 11,200-edit basin in this shot's screenshots remeshes once.

## V5 the crosshair picks a column, and plants are not pickable

`VoxelWorld::pick_cell` marches the camera ray at a quarter of a cell and stops where it is at or
below `ground_h + building_h`. Trees, cover and vines are all above that test, so the crosshair
passes through a canopy and lands on the ground under it. You edit terrain with this tool, and a
picker that let you dig a tree would be answering a question nobody asked.

## V5 twenty metres of headroom, reserved at load

`RaiseGround` and `RaiseBuilding` need somewhere to put a voxel, and the voxel world was exactly as
tall as the site it was built from. The world now reserves the tallest thing the tree model can grow
(`Life::default().tall_height_m`, 20 m) above the terrain. On the stress world that is 486 chunks
where V4 had 405, all of the new ones empty: **247 chunks drawn and 335,544 quads, both unchanged**.
Empty chunks cost a mesh call that returns nothing, which is the cheap half of the trade.

## V5 `--tick` overrides the round trip's auto-play

When a run finishes, the timeline rewinds to zero and plays: the point of the round trip is watching
the site grow out of the edit, not arriving at the end of it. A scripted screenshot wants the
opposite, so `--tick N` seeks and stays put. Both screenshot commands in MEASUREMENTS.md rely on it,
and it is the reason those pictures are reproducible rather than a race against the play rate.

For the same reason the headless frame counter restarts while a round trip is in flight, so
`--sim --screenshot` photographs the site that grew and never the site before it.

## V5 the HUD says whose numbers are on the screen

`the edit is the viewer's; the water, light, fertility and growth are ecosim's` sits under the round
trip's status whenever a run has been grown, and the `ecoview.sim` method carries the same sentence
as `note_on_authorship`. This shot is the first time the viewer causes a simulation rather than
reading one, which is exactly when a screenshot starts to be able to lie about who computed what.
V4 put the same discipline on the cover lines; this is the sentence for the round trip.

## V5 the new tests live in `mesh_golden.rs`, for the fifth time

Thirteen more, 50 in all, every one compiling with `--no-default-features`: the CI gate runs exactly
one target. The interesting half is that `SimJob` grew `attach`, which wraps a child this module did
not spawn — so the tests drive the whole poll-progress-finish-fail-cancel path with the test binary
itself standing in for the simulator, and `SimJob::start` goes through the same constructor, so the
tested path is the flown path. Nothing in the gate runs `ecosim`; the round trip's own timings are
measured by hand and written down in MEASUREMENTS.md instead.

## V6 ambient occlusion is a voxel id, not a vertex attribute

`binary-greedy-meshing` merges neighbouring faces that share a voxel id. Per-corner ambient
occlusion, which is the usual way, would give every corner of every face its own number and break
every merge on the site; the quad count is the whole reason this viewer can draw 450,000 faces at
200 fps. So occlusion is quantised to four levels and packed **into the id**: `id + PALETTE_LEN *
level`, and the palette is the same 48 colours four times over, each block dimmer than the last
(`sky::AO_SHADE`). Two voxels of the same material at the same shade still merge; only the boundary
between two shades costs a quad, which is 452,385 against 322,068 on the Capitol at tick 10000.

Level 0 is exactly 1.0, so the first block of the shaded palette is the palette. That is what makes
`--no-ao` give back V5's mesh **to the byte**, and it is the assertion the gate leads with.

What counts as an occluder is the eight cells in the layer **directly above** a voxel, and nothing
else. Not the coplanar ring: on flat ground every voxel has eight coplanar neighbours, so that would
dim a lawn uniformly and dim nothing relative to anything else. Not the layer below, which is solid
under every ground voxel there is. The layer above is what a face looks into, so this darkens
exactly the concave places -- the foot of a wall, the inside of a step, the inside of a crown. One
voxel of reach is also the whole reach, which matters because the mesher's padded buffer is exactly
one voxel wider than its chunk: every neighbour a voxel needs is already in the pad, including the
ones belonging to the chunk next door, so there is no seam along a chunk boundary and no second
pass.

## V6 the day of the year is the run's, the hour of the day is the viewer's

A tick is `8766 / year_len` hours (`ecosim/UNITS.md`), 2.1915 h at the shipped `year_len`, so the
viewer could read an hour of the day straight off the tick. It does not. Half of every run's
snapshots would then be photographed in the dark, for a diurnal cycle **the simulator does not
model** -- its light field is computed under a fixed 45 degree sun and has no time of day in it at
all. Drawing night would be the viewer inventing a claim the run never made, and an expensive one:
the pictures are the product.

The day of the year is a different case, because the simulator's temperature and rain both swing on
it. So that half is the run's, and the alignment is the simulator's own: `abiotic.rs` peaks
temperature a quarter of the way through its year, and that tick is drawn as the summer solstice.
Tick 0 is then the spring equinox, and the viewer's autumn is the run's autumn. The hour is a key
(**K**, **L**) and a flag (`--hour`), and every HUD line and every `ecoview.stats` reply says which
half is whose.

## V6 the latitude is the viewer's, and nothing in the project can tell it otherwise

No world bundle and no run directory carries a latitude. `bundle.json` has a `source` string naming
the site in prose and nothing machine-readable, and `meta.json` has no place for one. So `--lat`
defaults to 42.7 N -- Lansing, Michigan, where the committed bundle's own `source` line says its
LiDAR was flown -- and the HUD names it as the viewer's, the way V2 named the overlay hues and V4
named the vine's. **Handed up**: a latitude in the bundle format is an `ecosim` change and this row
may not make one (MASTER.md, component isolation).

## V6 the season is quantised into 64 steps because colour is baked into vertices

Seasonal colour is a palette change, and this viewer bakes colour into vertex attributes, so moving
the palette means remeshing every chunk. Sixty-four steps is 5.7 days a step, finer than one
snapshot of the reference run at 9.1 days, so nothing visible is lost -- and a camera move, an hour
key or a HUD tick never rebuilds the world, because none of them changes the step. With the tint off
the step is not part of the key at all, so `--no-sky` scrubs the timeline at exactly V5's cost.

**Colour only.** The trees keep every leaf in December. Leaf fall is a change of geometry and it is
the simulator's to make, not the viewer's -- row G10 is that shot. A bare winter tree drawn here
would be this viewer inventing a plant behaviour, which is the one thing the direction file forbids.

## S2 the ramp hues come from the run, and a fallback says so

`ecosim` shot S2 added `overlays` to `meta.json`, so `Overlay::ramp` takes the run's metadata and
returns the two hues the file names. This is the follow-up half of that row, and it finishes the
sentence V2's module doc had to leave hanging: **every part of an overlay now comes from the run** --
the scale's two numbers (V2), the field itself, and now the two colours.

- **The `ecoview` legend stays in this file, as the fallback.** Every committed run written before S2
  has no `overlays`, and a viewer that drew a blank overlay for them would be trading a working picture
  for a point of principle. So `fallback_ramp()` is the old table under a name that says what it is.
- **A fallback is named on screen, in the same words a fallback scale uses.** `Ramp::source` reads
  either `meta.json overlays.moisture` or `this viewer's fallback: meta.json has no overlays.moisture`,
  `Ramp::from_meta()` is the same one-line test `Scale::from_meta()` is, and the HUD prints a `(!)` next
  to a fallback. One more line under the scale line, and the same pair of fields in `ecoview.stats` and
  in the `ecoview.overlay` reply, so an agent sees it too.
- **Fire's `burnt` is read, its quiet band is not.** The run names the colour of ground that burnt out;
  "nothing to show here" is not an ecological quantity and stays this viewer's, beside the vine hue.
- **`mid` is parsed and unused.** The only overlay that has one is `traits`, which this viewer has no
  overlay for; it is read into `OverlayColors` because leaving a field out of a struct that mirrors a
  file is how a reader silently disagrees with a writer, and `serde` would ignore it either way.
- **No picture changes.** The seven ramps the simulator publishes are the values this file already held,
  so this is a provenance change and not a visual one. Measured rather than asserted: MEASUREMENTS.md,
  "S2".

## S5 standing water is drawn, and the decision the row asked for

Backlog row S5, which has no prompt file: the row itself is the specification, and it asked for two
things. **(a)** draw ponded water, so the wettest ground stops being painted as the driest.
**(b)** decide whether the simulator should pond in soil depressions at all, or whether CLAUDE.md's
"pond in depressions" should be reworded. (a) is below; (b) is the last entry, and it changed a
sentence rather than a model.

Every run since `ecosim` shot G4 has written `water.bin` -- ponded depth per **ground** cell, u16 in
tenths of a millimetre -- and until this shot no viewer read it. On `runs/capitol-s42` at tick 10000
that was 13,206 wet cells and 166 m3 of water the picture did not have.

## S5 the pond id sits past the 32 bands, not among the media

A new drawable thing needs a voxel id, and the obvious place for one is beside the other surfaces,
after `WATER` at the end of the media. That would have moved `BAND_BASE` by one, which moves every
overlay band, which recolours every banded mesh: four of this file's golden hashes, and every
committed PNG. So `POND` is `BAND_BASE + BANDS`, one past the last band, and `PALETTE_LEN` is one
longer. Every id from V0 through V6 is exactly where it was and no golden moved -- `the_pond_id_sits_past_the_bands`
asserts that directly, so the next person to add an id finds out why before they find out how.

The colour is **not** the `water` medium's `#3a6fd8`. A surveyed pond in the bundle and water this
run ponded this tick are different claims about the site, and a viewer that painted them the same
colour would be answering "is there water here" when the question is "did the run put it there".

## S5 the depth ramp is logarithmic, over a fixed 1 mm to 10 m

The distribution is the argument. On `runs/capitol-s42` at tick 10000 the median wet cell holds
10.0 mm, the ninetieth percentile 47.7, the ninety-ninth 480, and the deepest 5,074. A linear ramp
to the maximum puts 99% of the standing water in the bottom two bands and draws a wet site as a dry
one. So: log10, four decades, 1 mm to 10 m.

- **Fixed ends, not the snapshot's own range.** A per-snapshot maximum would make two ticks of the
  same run incomparable -- the same puddle would change colour because a different corner filled --
  and scrubbing the timeline is what this overlay is for.
- **Dry ground is a band of its own, off the ramp.** The same shape fire's quiet band has. A tenth
  of a millimetre is water the simulator decided to put there; "none at all" is not the bottom of a
  depth scale, it is a different answer. Band 0 is grey, band 1 is the palest blue.
- **The ends are this viewer's, and the HUD says so.** `meta.json`'s `overlays` array has seven rows
  and none of them is `water`, and `params.hydro` carries no ramp either, so `Scale::of` and
  `Overlay::ramp` both report the viewer's fallback and the HUD prints `(!)`. That is a row for the
  simulator -- publish an `overlays.water` row, and these two constants come from the run like the
  other seven. Two tests in `mesh_golden.rs` assert the fallback **is** reported rather than
  excluding water from the from-the-file loops silently.

## S5 anything drawn at all is at least one voxel deep

The lattice is 0.5 m and the median pond is 10 mm: quantising honestly would draw nothing at all.
So `POND_MIN_MM` is 5 mm -- below it nothing is drawn, at or above it at least one voxel stands --
and the HUD prints both counts and the millimetres beside them: `13206 of 262144 ground cells wet,
10469 over 5 mm; max 5074 mm, mean over wet 50 mm, 166.2 m3 ... 11035 voxels drawn, the run's depth
on a 0.50 m lattice`. The quantity is the run's and the lattice is the viewer's, and the line names
which is which. 5 mm is where a wet surface becomes a puddle; nothing in any file could supply it.

## S5 water is read every snapshot, and drawn on the ground grid

Two smaller calls, both of which went the other way first.

- **Read whether or not it is drawn.** `read_ponds` first skipped the file when the layer was off
  and no water overlay was up, which saved half a megabyte a snapshot and cost the HUD its numbers:
  a site with the water switched off reported `0 of 0 cells wet`, which is a picture of a dry site
  rather than a dry-looking picture. It now reads every snapshot. `water.bin` is a quarter of
  `material.bin` on the Capitol and is read once per snapshot, not per frame.
- **The ground grid, at its own resolution.** The other six overlays are per 1 m ecology column and
  each covers 2 x 2 Capitol ground cells. `water.bin` is per ground cell, because the ground grid is
  what the water ran over. `ColumnBands` had no way to say which grid it was on, so a water field
  handed to it would have been stretched to twice its size in silence; it now carries `cell_m`, and
  `the_water_map_is_not_stretched_from_the_ecology_grid` pins both directions.

## S5 the simulator ponds in depressions; the sentence describing it was wrong

Part (b) of the row. Read `ecosim/src/hydro.rs` and measured the output; **no simulator change, and
none is recommended.**

The mechanism is right. `flow.pond_cap[i]` is `filled[i] - elev[i]` from a priority flood, so every
cell that sits in a depression has storage, and `route_storm` fills it on every cell after
infiltration takes its share. What decides whether water is *still there* at the next snapshot is
`settle_water`, which drains ponded water into the soil at the medium's own rate and against the
column's remaining room: concrete and asphalt have `field_capacity_mm = 0.0`, so there is no room,
so nothing soaks in and only evaporation touches it. Roofs get no depression storage at all.

So the observable behaviour is "ponds on sealed ground", and the measurement agrees: 22.3% of
asphalt cells and 11.7% of concrete are ponded at tick 10000, against 0.3% of lawn and 0.0% of roof.
That is the model being right about a paved site, not a defect. **CLAUDE.md's clause is the thing
that was misleading** -- "pond in depressions" describes the mechanism and leaves a reader expecting
puddles in the lawn -- so it now says both: depression storage is filled everywhere, and only a
surface the soil cannot drink from still holds it. The four screenshots are the evidence a reader
can check without running anything: the water is in the gutters, the car park and the paths.

# S4 -- the plantable gate is read from the run, not re-derived from a list of names

## S4 the run directory is an interface, and reading it is not sharing code

Shot V4 decided which ground grows things by matching medium **names** against a list it wrote down
-- `concrete`, `asphalt`, `roof`, `water` -- and its recorded reason was that the two projects share
no code. That is the reason to read the file rather than the reason to copy the value. CLAUDE.md
makes the run directory the whole interface between them, and `meta.json` has carried the answer as
data since shot G4: `params.medium.<name>.plantable`, false for exactly those four. So the gate now
reads it. `Plantable::of` is the reading, `VoxelWorld::read_plantable` puts it on the world, and
`Plantable::grows` is the single call site the cover loop uses.

Nothing about the picture changes -- see MEASUREMENTS.md, where the frame below the HUD is diffed
pixel for pixel against the same frame from the commit before this shot and comes out identical.
This is a correctness-of-source fix. What it buys is that the day someone edits `params.toml`, the
viewer follows the simulator instead of drifting from it, and `the_capitol_run_and_the_name_list_agree`
goes red on the way rather than after.

## S4 the name list stays as the fallback, because the viewer opens sites with no run

`--world DIR` on its own and `--stress` draw a site with no `meta.json` anywhere, and a site with
nothing to ask still has to decide whether a lawn grows. The V4 list is kept for exactly that, as
`SEALED_NAMES`, and it is now reachable from one place instead of being the rule.

Which of the two is in force is on the screen, the way shot V3's height curve and V2's ramp hues
are: `sealed ground grows nothing -- from the run's meta.json, params.medium.<name>.plantable` when
it was read, and `from this viewer's fallback name list, because no run is loaded to ask  (!)` when
it was not. A fallback that does not say it is a fallback is indistinguishable on screen from a
reading, and the `(!)` is the same marker the other two provenance lines use. `ecoview.stats` gains
a `plantable` object with the same four facts, so an agent can check it without reading pixels.

## S4 per medium, not all or nothing -- and a run that says nothing is not a source

A run is read medium by medium. If it names eight of the nine, the eight are the run's and only the
ninth falls back, and the source line names the ones it guessed at. Throwing away eight true answers
to avoid one guess would be the worse trade.

The edge that matters is the other end of that scale: a run that carries **no** `params.medium` at
all must not read as "from the run's meta.json", because that credits a file with an answer it never
gave -- a worse lie than V4's list was, since it names a file a reader could go and check. It is a
live case, not a defensive one. `ecosim/runs/capitol-s42-grad06` is a format-4 run written before
shot S2 removed the `skip_serializing_if` that omitted defaulted sections, so its medium table is
empty; the viewer loads it happily, because the loader's only version rule is format 4. That run now
reads `from this viewer's fallback name list, because the run carries no params.medium  (!)`, and
`a_run_with_no_medium_table_is_not_the_source` pins it.

## S4 only `plantable` is deserialised out of `params.medium`

`MediumRow` has one field. The three hydrology numbers beside it in the file -- `infiltration_mm_h`,
`field_capacity_mm`, `percolation_mm_h` -- are the simulator's business; this viewer draws no
infiltration, it draws the `water.bin` the simulator already wrote (shot S5). Leaving them out of
the struct is what keeps that true rather than merely unimplemented. The table itself is a
`BTreeMap` rather than a struct of nine fields, because the scene contract's media are the
simulator's to name and a tenth one should arrive here as data rather than as a parse error.

## S4 the gate is re-read on every snapshot, not once at load

`read_plantable` is called from `load_snapshot`, which runs whenever a snapshot is applied, and it
replaces the value only when it differs. Reading once at startup would be wrong for the case shot E4
built: the round trip grows a run from the edited site and adopts it mid-session, and a gate read
before that run existed would still be the fallback's while the HUD claimed otherwise.

## S6 the lattice floor moves; the ground does not

`LowerGround` clamped at zero because the voxel lattice has a floor at level 0 and a bundle's
`ground_h` is relative to its own lowest point. On a low-relief site those two facts multiply badly:
the deepest hole anywhere is the site's total relief, and only at its single highest point. The
operator raised it from a low-relief site (backlog row S6) and it is in the committed Capitol bundle
too: 1.3% of its lawn could not be dug one 0.5 m cell and 11.8% could not reach 3 m, and the flat
lawn terrace the shot's screenshots dig sits 0.29 m over the bundle's zero.

The clamp was not a wrong line. It is what a lattice with a floor can honestly do. So the fix gives
the lattice somewhere to put the hole instead of deleting the clamp: `VoxelWorld` carries a
`datum_m`, `open_dig_room` raises every column by `DIG_ROOM_LEVELS` levels when a dig reaches the
floor, and `apply` still clamps — at a floor that has just moved.

**`ground_h` stays in the lattice frame and nothing else does.** Every number the world reports
outside itself is in the bundle's frame: `column`, `dig_room_m`, `lowest_ground_m`, `ground_export`,
and the ray in `pick_cell` all convert. `mesh_chunk` subtracts the datum from the Y of every vertex,
so **world space is the bundle's frame too** — the camera, the crosshair, the HUD's metres and the
meshes are all in the same units they were before this shot, and digging a pond in one corner does
not lift the site under the camera. `opening_dig_room_leaves_every_undug_column_where_it_was_in_world_space`
is that invariant as a test: the lawn's top face is at 1.00 m before the dig and 1.00 m after it,
while the world's underside drops from 0.00 m to −4.00 m.

The alternative was to keep the heights in the bundle's frame and let them go negative, which reads
better and draws a bottomless pit: `voxel()` answers air below level 0, so a dug column would be a
hole punched through the floor of the world with nothing under it.

## S6 the datum is opened lazily, and that is what saves V0's goldens

Reserving the room at load would be simpler code and would change the mesh of every world that has
ever been loaded — the floor and the side walls move even when nothing is dug — so the five golden
hashes carried from V0 through V6 would have had to be regenerated. They are the only continuity
evidence this component has, and spending them on a feature that does not need them would be a bad
trade. `from_bundle` therefore never opens room; only an edit does, and `mesh_chunk`'s new term is
`- 0.0` on every world that existed before this shot.
`an_undug_world_still_meshes_to_the_bytes_it_always_did` says so where a reader will see it.

Eight levels at a time, rather than one, because opening room is a full remesh and a per-stroke lift
would remesh the world twelve times over one pond. Eight covers the operator's measured case in two
lifts. A lift also stales every chunk and can grow the chunk grid a layer, which invalidates every
index into `Site::entities`, so `apply_edits` despawns the chunk entities and rebuilds the vector
rather than reassigning it. The plant buckets are re-bucketed (`shift_plants`) rather than
re-voxelised: re-voxelising would be correct and would drop the run's trees until the next snapshot.

## S6 the depth limit is the simulator's `base_z`, and nothing in the format changes

The row called this "a real format question, not a one-line change", on the grounds that the run
directory's `height.bin` is unsigned. It turns out not to bind, twice over.

Nothing in the viewer reads a run's *absolute* height. `height.bin` is used to pick the light sample
above a column, inside the run's own grid; a tree stands on the viewer's own ground; a pond stands on
the column's own top solid. So the run's datum is the run's business.

And the bundle needs no new field either. `ground_h.f32` is signed, and `ecosim` already carries the
datum this needs: `[bundle] base_z` is the ecology layers of soil it puts under the bundle's lowest
ground, and a column's surface layer is `base_z + round(its mean height in metres)`
(`ecosim/src/world.rs:422`). A hole therefore goes out as a negative height and the simulator puts it
in the soil it already had. `hydro.rs` routes on `1000.0 * ground_h` through an order-preserving float
key, so a negative elevation orders correctly; the one place a bare `0.0` appears is `peak`, which
only lifts roofs above everything.

That makes `base_z` the real limit: at `base_z` metres down a column's surface layer is 0 and there is
nothing left underneath. So the viewer takes the limit **from the run**, out of `meta.json`'s
`params.bundle.base_z`, exactly the way shot S4 takes the plantable gate and S2 takes the overlay
ramps — read in `load_snapshot`, so a run that arrives mid-session (E4's round trip) is adopted. With
no run loaded it is `DEFAULT_DIG_LIMIT_M`, that parameter's own default, and shrinking the limit
afterwards stops the next stroke rather than undoing a hole already dug.

## S6 part (b) is the reading before the stroke as well as after it

The row asked, at minimum, that an edit which hit the floor say so. It does: the stroke is refused
and the HUD names why, in the simulator's terms ("8 m of soil sits under the bundle's lowest ground,
the run's params.bundle.base_z, and a hole cannot go under it"). But a message only after the fact would still let the
operator take twelve strokes to find out, so the crosshair line carries `N.NN m left to dig` at all
times, and `at the dig floor: [Z] does nothing here` when there is none. `ecoview.stats` reports
`dig_room_m` and `at_dig_limit` on the crosshair and `datum_m`, `dig_limit_m` and `lowest_ground_m`
on the site, so an agent digging a pond can see it stop without reading pixels.

## S6 the refusal names its source, because on a bare bundle the limit is the viewer's own

The first wording of the refusal said "the run puts 8 m of soil under the bundle's lowest ground" on
every site, including one with no run loaded at all — where the number is `DEFAULT_DIG_LIMIT_M` and no
run has said anything. That is the same mistake shot S4 found in its own first draft (a run with no
`params.medium` still read as "from the run's meta.json"), so it is fixed the same way: `VoxelWorld`
carries `dig_limit_from_run`, the note ends with either `the run's params.bundle.base_z` or `this
viewer's default, with no run loaded to ask`, and `ecoview.stats` publishes the flag beside the
number. `shots/s6-dig-floor.png` and `shots/s6-dig-floor-no-run.png` are the same dig with the two
wordings, which is why there are four screenshots and not three.

## V8 the held date is an override on the run's day, not a second clock

The viewer could have grown a clock of its own -- a date it owns, with the run's date as a starting
value -- and that is the shape most "let the user set X" features take. It would have been wrong
here. The day of the year is the run's (V6, above) because the simulator's temperature and rain
swing on it, and a viewer that keeps its own date has quietly made the run's date advisory.

So the day is still computed from the tick, every frame, and `Clock::with_day` overrides the one
number afterwards. Three consequences, all of them wanted:

- `with_day(None)` is the identity, so a viewer holding nothing is bit for bit the viewer of the
  first seven shots. The test `no_held_date_is_the_clock_the_last_seven_shots_had` is that claim,
  and it is the sibling of the three golden hashes.
- Everything else on the clock -- the tick, the `year_len`, `tick_hours`, `years()` -- is untouched
  and still the run's, so a held frame can still say which tick it is a picture of. That is the
  whole reason to hold one.
- The override is releasable. **\\** drops it and the run's own day is underneath, unchanged,
  because it was never overwritten.

`Clock::from_run` became a method over a three-case `DaySource`, rather than a second bool beside
it. Two bools would have had a fourth state that cannot happen, and someone would eventually have
read it.

## V8 a week a press, and a third key to hand the date back

The hour keys move half an hour a press. A day a press would have been the analogous choice and it
would have been a key that usually does nothing: seasonal colour is quantised into
`sky::SEASON_STEPS` (V6, above), 5.7 days a step, so a one-day press draws the same frame four times
out of five. A week always crosses a step, which the test asserts at five points around the year.
Fifty-two presses walk the whole year, and `--date` is there for the frame you actually want.

**;** and **'** because they are the two keys to the right of **K** and **L**: the hour and the date
sit next to each other on the keyboard as well as in the HUD. The third key is **\\**, and it earns
its place -- a frame with a held date is a frame whose date is not the run's, and getting back to the
honest picture should not mean restarting the viewer.

## V8 the flag counts days from 1 and the clock counts from 0

`--day 1` is 1 January, because every almanac, `date +%j` and spreadsheet in the world calls
1 January day 1. `Clock::day` is 0-based, because `month_day` walks a table. `parse_date` is the one
place the two meet and it subtracts the one, which is why the conversion has a test that walks all
365 days of the table in both directions rather than checking two endpoints.

`--date 6-22` and `--date 6/22` are the same date, and `--day 172` is too. A year field is not
accepted: the clock has no year, a run is at a tick, and which calendar year a tick is in is not a
thing this project knows. Anything else -- `366`, `2-30`, `june`, `6-22-2026` -- is fatal rather than
ignored, for the reason `--overlay` is fatal: a screenshot script that mistypes a date must not
quietly file the run's own season under the held date's name.

## V8 the date is a flag and a key, and not a BRP method

`ecoview.camera`, `ecoview.edit`, `ecoview.timeline` and `ecoview.sim` exist because an agent cannot
fly, dig, scrub or run a simulator from a command line. It can set a date from one: the four
screenshots in this shot are four `--date` flags. So the date is reported over BRP -- `day_source`
and `day_held` are in every `ecoview.stats` reply -- and not settable there. A method would be a
second way to do a thing the first way already does, and V0's rule about the remote surface is that
every method on it is one an agent needs.

## V8 a held frame says so twice, and a third time when nothing draws it

`SkyState::line` already named whose the date was, in one of two ways; it now has a third, and the
HUD prints a second clause beside the release key. Naming it twice is deliberate. A screenshot
travels further than the report that came with it, and this is the first frame this viewer can draw
that is a picture of two moments at once: tick 9000's wood under 21 December's light. One clause
says whose the date is and the other says the tick did not move, which is the half a reader is most
likely to get wrong.

The third place is the one where the flag draws nothing. With `--no-sky` there is no sun path and no
season in the palette, so a held date changes not one pixel -- and the HUD and the console summary
both say so rather than let a `--date` screenshot look as though the date had been applied.

## V8 `sky_json` exists because two keys hit the `json!` expansion depth

`ecoview.stats` builds its whole reply in one `json!`, and adding `day_source` and `day_held` took
that macro past the default recursion limit. The compiler's own suggestion is
`#![recursion_limit = "256"]`; the `sky` object was lifted into `sky_json` instead. A crate-wide
knob raised to make one function compile is the kind of thing that is never lowered again, and the
object was the largest thing in that macro anyway.

# S7 -- the animals fixture, and a crowding ramp taken off a measurement

Backlog row S7, no prompt file: the row is the specification. It has two halves and they are the
same blind spot twice -- `ecosim` shot S1 committed an animals-on world so the viewer track would
stop testing against worlds with nobody in them, and then nothing in this component opened it.

## S7 the fixture is opened here, not only in `ecosim`

S1 committed `ecosim/fixtures/capitol-animals-mini/` and pinned it with two `ecosim` tests, which is
all an `ecosim` row may do. The row's stated purpose was the viewer's, though, and until this shot
`ecoview-native` had no test, no screenshot and no line of any kind that touched it: every crowding
field this viewer had ever banded was all zeros, on `capitol-mini` or on `runs/capitol-s42`, because
every bundle-world run carries `--set animals.enabled=false` (`ecosim` DECISIONS, shot G3a).

So `the_viewer_reads_the_committed_animals_run` opens it and asserts the counts S1 recorded off the
same bytes -- 300 grazers over 251 patches at tick 0, 9,204 over 869 at tick 2000, busiest 95 -- and
five of the six screenshots in this shot are taken on it.

**The pair stays a pair.** `the_animals_off_sibling_still_has_nobody_on_it` asserts that
`capitol-mini` still holds no grazer and that every one of its patches draws in the empty band. S1
asserts the same fact from `ecosim`'s side; this is not duplication, it is what stops a later shot
"fixing" the pair by turning animals on in `capitol-mini`, which would move every picture in this
component at once.

**Why the strip run cannot stand in for the fixture.** `runs/s42` has had animals on since shot 1 and
it reaches 229 grazers on a patch, so the field was never unobserved -- it was unopenable. This
viewer requires a `world/` bundle and `runs/s42` is format 3. That is a narrower explanation of the
blind spot than "no world has animals", and it is the row's own reading.

## S7 the crowding ramp is measured, and it is logarithmic

V2 set crowding's top to `2 x params.disease.grazer_threshold`, which is 32. Measured on every run
with the animal tier on: the fixture's busiest patch holds **95** and `runs/s42` reaches **229**, so
everything from 32 up was one flat colour -- and on the fixture that is 32, 34, 35, 73 and 95 drawn
identically, at the top of the range, which is precisely the population the map exists to find.

A disease threshold is a statement about one animal's health. It says where the simulator starts
killing grazers for crowding; it says nothing about how many this viewer will be asked to draw, and
the two turned out to differ by nearly a decade. So the ramp comes off the field now:
`CROWDING_RAMP = (1.0, 256.0)`, **log2**, eight doublings over the thirty bands above the empty one.

- **256** is the first power of two above the largest patch count ever measured, which is the
  strip's 229. A rule, so the next shot to measure a bigger one knows what to do with it.
- **Log, for the reason S5's water ramp is log.** The field's median occupied patch holds 9 or 10
  and its maximum is 229: a linear ramp to 32 clamps the top and a linear ramp to 229 puts the
  median in band 1 of 32 and draws a busy site as an empty one.
- **Still not the data's own range.** "V2 the scale is the simulator's range, never the data's" is
  untouched: these are fixed constants, the same in every snapshot of every run, chosen once from a
  distribution and written down with it. A ramp stretched to each snapshot's own maximum is the
  thing both that decision and this one refuse.

**The cost is stated and asserted.** A log ramp spends bands on the top of the range, so the bulk
loses separation: on the fixture's tick 2000, 27 distinct bands become 18, and 32, 34 and 35 now
share a band. That is the scale being right rather than the clamp coming back -- those three are
within 10% of each other -- and neighbouring bands are already below what the eye separates on a lit
surface (V2, above), while 32-against-95 was not.
`the_crowding_ramp_no_longer_flattens_the_patches_it_exists_to_show` asserts both halves of that
trade, so a later shot that tries to widen the ramp sees what it is spending.

## S7 the scale is the viewer's, and the HUD says so

Crowding's `Scale::source` now reads `this viewer's fallback: meta.json has no scale for grazers per
patch (log2 1-256, measured); disease starts at 16`, and the HUD marks the line `(!)`, which is the
machinery V2 built for exactly this. `meta.json` publishes no number saying how many grazers a patch
holds -- there is no such parameter -- so crowding joins standing water on the viewer's side of that
line, for the same reason and not a weaker one.

**The threshold stays on the line as a landmark.** It is a true and useful fact about the ramp --
above 16 the simulator is killing grazers on that patch -- and printing it beside the range costs
nothing, where letting it *decide* the range cost the top decade of the field. That is the only
surviving use of `disease.grazer_threshold` in this viewer.

## S7 an empty patch is its own band

Crowding's ramp runs from white, so before this shot a patch holding one grazer and a patch holding
nobody were the same white: at tick 0, with 251 of 1024 patches occupied, the map drew one flat white
site. `CROWDING_EMPTY` is band 0, off the ramp, with the ramp starting above it -- the third overlay
to take this shape, after fire's quiet ground and water's dry ground.

It is drawn in the same neutral `WATER_DRY` uses, on purpose: both bands mean "the simulator put
nothing here", and a reader who has learnt one map reads the other without being told. The two
tick-0 frames in `shots/` are the whole argument -- 45.7% of that frame changes.

## S7 nothing was copied into `ecoview/public/`

The row offered `sync-data.sh` as one of the three things that could point at the fixture. It is
still the wrong one, for the reason S1 gave: `ecoview` is frozen, this component opens
`../ecosim/...` paths directly, and syncing without committing the 22 MB copy would leave it
untracked beside tracked siblings. An `ecoview-native` shot may not edit `ecoview/` in any case
(MASTER.md, component isolation). The test and the screenshots are the two halves that were
available, and they are the two the row asked for first.

# V7 -- the eye's adaptation

## V7 the eye is added, the sun is not corrected

The row calls this "not a wrong number -- a missing one", and the implementation takes that
literally. `Sun::at` still puts 10,000 lux on a surface facing it and the shadow map still does
what V6 made it do; nothing about the physics is touched. What is added is the observer: ambient
is raised toward the level at which a shadowed horizontal surface reads at `SHADE_FLOOR` of a lit
one.

The alternative was a camera exposure -- a post-process tone curve, or a GPU luminance histogram
feeding an auto-exposure. Two things ruled it out. **The gate is engine-free**: CI builds this
crate `--no-default-features`, so nothing on the GPU side of the `viewer` feature can be asserted,
and a term this shot exists to justify has to be testable. And a histogram exposure reads the
frame, so it changes when the camera turns, which would make every screenshot in `shots/` a
function of where the camera happened to point. Moving one engine light off a property of the
world keeps both: `SkyState::adapt` is a pure function of `(closure, manual)` and is tested as one.

## V7 closure is leaves over ground, measured site-wide

`VoxelWorld::canopy` counts ground columns with at least one `CANOPY` voxel above the ground level
of that column, over the whole site. Four choices are folded into that sentence.

**Leaves only.** `TRUNK`, `SHRUB`, `GRASS`, `BUILDING` and the pond id are not canopy. A trunk
casts a shadow a metre wide that an eye does not adapt to; a building's shadow is the case where
the correct answer really is "it is dark in there", and a site that is 30% roof should not have its
shadows lifted for it. Cover voxels sit on the ground rather than over it and would count every
lawn as closed.

**Above the column's own ground, not above sea level.** The Capitol has 4 m of relief across it, so
a fixed height would call a crown in the low corner a canopy over the high one.

**Site-wide, not camera-relative.** A closure measured in the view frustum would make the exposure
change as the camera flies, and a screenshot script that sets `--eye` and `--look` would be setting
the exposure too without saying so. Site-wide costs one full sweep of the plant buckets, which is
why it is recomputed only when the snapshot or the cover changes, not per frame.

**Ground columns, not leaf voxels.** 1,058,244 leaf voxels at tick 9000 says how much wood there
is; 34% says how much of the floor is under it, and the floor is what the defect is about.

## V7 two stops, and where the number comes from

`SHADE_FLOOR = 0.25` -- a shadowed surface is held at no worse than a quarter of a lit one, two
stops down. The three numbers this sits between:

| | shade : lit | stops |
| --- | --- | --- |
| V0-V5, no shadow maps | 1 : 1 | 0 |
| V6 as shipped, June at 10:00 | 1 : 13 | 3.7 |
| this, under a closed canopy | 1 : 4 | 2 |

Two stops is the range a print holds, and it is deliberately not a return to V5: the shadow stays a
shadow and is still the darkest thing in the frame. The lift is `closure` of the way from V6's
level to that floor, so it is proportional to the measurement rather than a switch, and the whole
term is `max(0, need - was)` -- if the sun is low enough that V6's 740 lux already beats the floor,
nothing happens at all. That is why a December morning at 18 degrees of sun and 35% closure moves
by `+0.00` stops and a dusk frame moves by nothing: there is no sun to hide in.

## V7 the lift is on the ambient only, so a clearing does not brighten with the wood

Lit ground receives `ambient + sun`, so raising ambient raises it a little -- at a closed canopy and
the default hour, by 1.23x, which is about three tenths of a stop. A test pins that below 1.3x, beside an
equality that the sun's own figure is untouched. This is what keeps the lawn beside a stand of
trees looking like a lawn rather than like an overexposed one, and it is the property that makes a
site-wide measurement defensible: the correction is small everywhere the sun reaches and large only
where it does not.

## V7 the measurement is a default, not a verdict

`--exposure auto|STOPS`, the **-** and **=** keys in half-stop steps, and **0** to hand it back.
Manual replaces the measurement outright rather than adding to it -- `--exposure 0` is therefore
the exact V6 picture, which is what makes the before/after pairs in `shots/` a fair comparison and
is how `v7-canopy-off.png` was taken. The range is clamped to +/- `EXPOSURE_LIMIT` = 4 stops at
both the flag and the key, because 16x either way is past any use and an unclamped stop count run
through `2^n` reaches infinity.

The HUD says which of the two it is on every frame, in the word `measured` or `by hand`, beside the
closure it read and the shadow ratio either side of the lift -- the same shape V6 used for the
latitude and V8 for the held date. `ecoview.sky` carries all of it under `exposure`, so the agent
loop can read the number it is looking at.

## V7 the reference site's shadows do move, and this shot does not pretend otherwise

`runs/capitol-s42` is not an open site at every tick. Closure runs 1% at tick 2000 and 34% at 9000,
so the June frames at the dense ticks gain up to a stop of ambient and their shadows are lighter
than V6 drew them. Measured on the two overview frames, below the HUD: YLOW 60 -> 75, YAVG 146.8 ->
151.0, and **YHIGH unchanged at 194**. That is the shot working -- the shadows open, the lit
picture does not -- but it is a change to committed reference frames and it is recorded here rather
than buried. No gated screenshot is re-accepted, because `ecoview-native` has none: its goldens are
meshes, and no mesh moves. `ecoview/shots/REACCEPT-NN.md` is therefore not written and not needed.

## V7 the closed-canopy case is built, because no committed run has one

The row says plainly that the operator measured the defect off-repo and that `capitol-s42` does not
close its canopy. Rather than take that as a reason to guess, the closed canopy is **constructed**
in the test from the viewer's own `TreeForm::grown` -- a tree every 6 m over a 32 m site, 3,816 of
4,096 columns covered, 93.2% -- and the committed fixtures carry the complementary half: they are
open (0.23% and 4.54%), so they show that an open site is left alone. Between them the two halves
cover the claim without inventing data, and the built wood is the same procedural wood a run grows,
not a block of leaf voxels stood in for one.

# V9 -- the season word

## V9 the word comes from the calendar, and the colour stays continuous

`Season::of` named the season by testing the three colour weights in turn -- dormancy, then
senescence, then flush -- and anything matching none of them fell through to `"summer"`. Three
Gaussian bumps do not cover a year, so that else-branch was not an edge case: it was **60 days**,
7 March to 21 April and 14 to 27 November, and it left `spring` with only 49 days of the year.

The name now comes from the date (`season_name`), and the three weights are untouched. Two reasons
for that direction rather than the other one the row offered, a fourth name for the gap:

- A fourth name would need to cover both holes, and they are not the same thing -- March is
  dormancy fading and November is dormancy rising. Any name true of one is wrong about the other.
- The picture already carries the colour. What a reader cannot see in a frame is where in the year
  it sits, and that is exactly what they read the date for, so the word is worth more agreeing with
  the date than duplicating the leaves. The continuous truth is still published three ways: the
  HUD prints the season step out of 64 beside the name, `ecoview.stats` carries all three weights,
  and the palette is built from them and never from the word.

The cost, stated plainly: the word and the colour may now disagree at the shoulders. Early
September is named `autumn` over a canopy still drawn mostly green, and early March is named
`spring` over a dormant one. That is a reduction of a continuous model to four words, and it is
better placed on a boundary a reader can check against the date printed next to it than on a
threshold nobody can see.

## V9 meteorological boundaries, not the equinoxes

Three whole months each, December to February being winter. The equinox boundary was measured and
rejected for one reason: it moves `runs/capitol-s42` at tick 10000 -- 21 September, the tick the
project photographs most -- from `autumn` to `summer`, in a frame whose canopy has already begun to
turn (`senescence` 0.40, and its living green measures 80 degrees of hue against 94 on the same
site in March). The month boundary leaves every seasonal frame this project has published reading
the word it was published with (22 June, 15 May, 15 October, 21 December, 21 September), and it
contains all three bump centres -- 16 January, 16 May and 16 October -- so the colour model's own
opinion is never contradicted where it has one.

## V9 one function, three surfaces

The HUD line, the headless console summary and `ecoview.stats` all take the word from
`Season::name`, and the first two share `SkyState::line()`, so the fix reaches all three at once
and they cannot drift apart. Nothing computes from the name -- `Season::step()` drives remeshing
and `tint_palette` drives the colour, both off the day and the weights -- so no mesh golden moves
and no reference screenshot is re-accepted.

## V9 `month_index` is shared rather than copied

`Clock::month_day` already walked the `MONTHS` table; `season_name` needs the same walk. It is one
function now, used by both, so a calendar with two answers in it is not possible. `month_index`
also normalises with `rem_euclid` before the clamp, where `month_day` used a bare `as u32` cast --
a day of `-1.0` named 1 January and now names 31 December. No caller passes one; the clock
normalises its own day.

## S12 the sentence is a function, not a string in two places

The row is one missing line in the headless console summary, and a `println!` beside the
height-curve line would have closed it. What went in instead is `Plantable::line()`, called by both
surfaces. The row's own diagnosis is the reason: "They are separate code paths." S4 wrote the
sentence into the HUD and nothing made the console's author write it too, so the fix that puts a
second copy of the same words in the second path leaves the third path -- whatever it turns out to
be -- exactly as unprotected as the second one was. One function is the same size as one `println!`
and it cannot drift. This is the shape V9 used for the season word over three surfaces.

`ecoview.stats` deliberately does **not** call it. The BRP payload publishes `source` and
`from_meta` as separate JSON fields, which is what a machine reader wants; the sentence is for a
human, and gluing a `(!)` into a JSON string would make it harder to read, not easier.

## S12 the console line is printed with no run open, where the HUD's is

The tree, cover and water lines of the startup summary all sit inside `if let Some(run)`, because
each of them is a number the run owns. The plantable line is not: it answers a question that has an
answer with no run at all, and the answer then is always the fallback -- the case the `(!)` exists
to mark. Printing it inside the run block would have put the sentence in the one situation where it
says nothing surprising and left it out of the one where it does. The HUD has been outside the
equivalent branch since S4 for this exact reason, so this also makes the two surfaces agree about
*when* they say it, not only about *what* they say.

Measured on this machine, all three sources now reach stdout: the reference run reads
`from the run's meta.json`, `--world` alone reads `from this viewer's fallback name list, because
no run is loaded to ask ... (!)`, and `runs/capitol-s42-grad06` -- a format-4 run written before S2
stopped omitting defaults -- reads `because the run carries no params.medium ... (!)`.

## S12 the regression test reads `main.rs` as text

`both_surfaces_print_the_plantable_sentence` opens `src/main.rs` and counts. That is an unusual
test and it is deliberate: the defect is *one surface forgot*, and neither surface can be exercised
by the test binary the CI gate runs. The HUD builder needs a Bevy `World` and the console summary
needs a window, and the gate is `--no-default-features`, with the renderer compiled out. The choice
was between a test that pins the helper (which was never the bug) and a test that pins the two call
sites. It pins both: the helper's behaviour in its own test, and the call sites here. What it
asserts is deliberately narrow -- `plantable.line()` appears twice in `main.rs`, and the sentence's
words appear in exactly one file in `src/` -- so it goes red for a deleted console line or a
hand-rolled second copy, and stays green for any refactor that keeps one sentence in one place.

## V11 a test here asserts what the viewer claims, not what the simulator counted

Worked from the BACKLOG row; V11 has no prompt file. G5 regenerated
`ecosim/fixtures/capitol-animals-mini`, as the sim-shot rules require of a behaviour change, and two
tests here went red over seven literals. That was the fourth time (14c, G4b, G4c, G5), and V10 was
the third re-cut. The re-cut is not the fix. The cause is that a test in this component hard-coded
numbers the other component computes. The two components are supposed to share only a file format,
yet they shared grazer counts, and no `ecosim` shot is allowed to edit them.

**Re-derived before replacing.** V10 set the precedent that the numbers are re-read and the claim
re-checked, not pasted. On the regenerated fixture the values are exactly G5.BLOCKED.md's table:
9240 grazers at tick 2000, busiest 61, 866 patches occupied, seven patches at or over 32 spanning
33 to 61, and 28 distinct bands on the old scale against 18 on the ramp. The ramp test's claim still
holds. The old scale draws a range of nearly two to one in one colour, and the ramp spreads those
patches over at least three bands with none at the top.

**What each pin became.**
- The tick 0 count (300) is now compared with `params.grazer.start_count` from the same `meta.json`.
  It is still exact, but it is no longer a copy of anything.
- The tick 0 `(busiest, occupied) == (3, 251)` became "scattered": more than one patch occupied.
- The tick 2000 total and `(busiest, occupied)` became three checks: the tier is present (both
  above zero), it has gathered (busiest now above busiest at tick 0), and the overlay reads the
  busiest patch, whatever its count, at the patch's corner columns.
- The clamped range `(37, 70)` and its count of seven became the test's precondition: at least two
  patches over 32, the top at least 1.5 times the bottom. If a regeneration stops showing that case,
  the failure message says so, because at that point the test's input has to change, not its
  expected value.
- `distinct == 28` and `distinct == 20` became the ratio they were there to show: the ramp has fewer
  bands and keeps more than half. The history is 27 -> 18, 28 -> 20, 28 -> 18.
- `(stats.min, stats.max) == (0, 70)` now compares with the field's own min and max. The point of
  that check was always that the scale does not alter the reading.

**Checked across an ecology move, not only on today's bytes.** I ran both rewritten tests against the
pre-G5 fixture (`git archive 13c318c`, extracted outside the repo, constant pointed at it for one
run and then restored). They pass there too, on 9169 / 70 / 37 to 70 / 28 -> 20. The same assertions
therefore hold on both sides of the change that broke the old ones. I also pointed the first test at
`capitol-mini`, which has animals off. It fails, but earlier than the grazer checks: on the snapshot
ticks, `(0, 100)` against `(0, 2000)`. So I have no run that exercises the "tier dropped" message
itself. The argument that it would fire is the arithmetic: a run with no animals has a tick 0 total
of 0, against a `start_count` that the same assertion first requires to be above zero.

**Pins on committed fixtures that stay, and why.** Each one is exact because its exact value is the
thing under test, and none of them is ecology:
- `the_capitol_run_and_the_name_list_agree`: the sealed set `concrete, asphalt, roof, water` comes
  from `params.medium`. Its own comment says it should go red if a default changes, so that the
  viewer follows the change.
- `base_z_m() == Some(8.0)` is `params.bundle.base_z`, a property of the bundle.
- Fixture shape (256 x 256, patch 8, snapshots at 0 and 2000) is set by the fixture's recipe, not by
  the ecology.
- `the_animals_off_sibling_still_has_nobody_on_it`: zero grazers is what `animals.enabled=false`
  means.

**One ecology-derived band stays, with its headroom measured.**
`the_committed_reference_site_is_left_where_v6_drew_it` asserts canopy closure below 10%, exposure
below 0.25 stops, and more than 100 trees on the denser fixture. Measured at V11: 290 trees at 0.30%
and 111 trees at 4.54% / +0.177 stops. The binding case is the quiet fixture at tick 100, whose trees
are almost all imported from the scene rather than grown, so an ecology shot moves it little. Its
comment quoted 39 trees for the animals fixture, which the fixture has since outgrown. The comment
now says the counts are printed, not pinned.

**The rule going forward.** A test in this component that opens a run written by `ecosim` asserts
one of two things. Either it is a value the run itself publishes (params, dims, the palette), compared
with where the run publishes it, or it is a shape: an ordering, a ratio, a band, a presence. It never
asserts a count the simulator arrived at by simulating. The test may print that count, and the two
rewritten tests do, so the numbers stay visible in `--nocapture` output without being gates.

**Screenshots** (the cadence rule; this shot changes no pixel, so they show the data the rewritten
tests read, drawn by the viewer). Both are `--run ../ecosim/fixtures/capitol-animals-mini --overlay
crowding --headless`.
- `shots/v11-crowding-t0.png`, tick 0: the 300 placed grazers show as scattered pale-pink patches
  of 1 to 3 on grey empty-band ground, and the HUD reads `field 0.00 to 3.00`. You would know it was
  wrong if the site were uniformly pink, meaning the empty band was lost, or if the patches were not
  8 m squares.
- `shots/v11-crowding-t2000.png`, tick 2000: nearly every patch is occupied, deeper magenta patches
  stand out from the pale bulk, and the HUD reads `field 0.00 to 61.00`, the busiest count the test
  now derives rather than pins. You would know it was wrong if the site were one flat colour, which
  is the V2 scale's failure that the ramp test guards against.
- An observation, not this shot's to act on: the same HUD shows 79 mature trees at tick 0 and 15
  mature trees (of 290) at tick 2000 on this fixture. That is the simulator's result, and the viewer
  draws it as published.

## V13 the heartbeat is a binary behind a cargo alias

Worked from the BACKLOG row; V13 has no prompt file. The row asks for "a recipe" and this component
has no justfile, so the command is `cargo heartbeat`, an alias in `.cargo/config.toml` for
`cargo run --release --bin heartbeat --`. A binary rather than a script because it has to decode
PNGs and compare them, it has to run the same on Windows and on a Linux runner, and the component
already has two helper binaries (`agent_loop`, `mesh_measure`) shaped this way. It is behind the
`viewer` feature like the viewer itself, so the CI gate's `--no-default-features` build never
compiles it. `cargo run --bin heartbeat` builds only the heartbeat, so the heartbeat builds the viewer
first (`cargo build --release --bin ecoview-native`, a no-op when it is current) rather than
photograph whatever stale binary is on disk. The one new dependency is `png`, pinned at the 0.18.1
Bevy's own PNG support already locks, so no crate was added to the build.

## V13 the fixed set: eight views, two poses, one tick

The reference run `ecosim/runs/capitol-s42` at tick 10000 (21 September, the tick the project
photographs most), and the Capitol bundle. `--run`, `--world`, `--tick` and `--out` move it; nothing
else does, so two heartbeats are comparable. The run is gitignored and regenerable, and the heartbeat
does not make it: a viewer command that starts a 40 s simulation to take a picture would hide a
stale run behind a fresh one. When it is missing the heartbeat exits 2 and prints the command that
writes it.

- `iso-surface`, `iso-moisture`: the viewer's own overview pose, beauty pass on, HUD on. These are
  the pictures a person recognises, and the HUD in them says which run and tick they are.
- `top-no-run`, `top-surface`, `top-light`, `top-moisture`, `top-fertility`, `top-water`: straight
  down over the centre, flat-lit (`--no-sky --no-ao`) so a top face carries its band and nothing
  else, and with the HUD's text block off.

`INDEX.md` beside the images is written by the same command: for each view the flags, what it shows,
how you would tell it was wrong, and what was measured. It is regenerated, never edited.

## V13 `--no-hud`

A small viewer flag, not a feature: it hides the HUD's text block and keeps the legend and the
timeline. On the first heartbeat the text covered the top 400 of 800 rows of every top-down map --
the whole north third of the site. The iso views keep the HUD, so the set still carries the run,
tick and scale lines in pictures.

## V13 what the gate checks, and the frame that fooled the first version

The row forbids a pixel-golden gate, so the checks are coarse: every view renders (the viewer exits 0
and a PNG is there), is not blank (at least 32 colours at 5 bits a channel, no colour over 90%), and
differs from the views it names by at least 2% of pixels at more than 8/255.

The first version measured the whole frame, and a negative test broke it. A wrapper that appends
`--frames 3` photographs the site before any mesh is ready. That frame is sky and HUD only, and it
passed: the HUD's anti-aliased text alone gave it 131 colours with the sky at 51%, and the overlay's
HUD lines made `iso-moisture` "differ" from `iso-surface` by 21%. The checks now look only at rows
55% to 92% of the height, below the HUD text and above the legend. In that band the same broken frame
is one colour at 100%, and the heartbeat exits 1 with 11 failures. The real set measures 247 to
1569 colours with the commonest at 38-50%. The commonest colour in the top views is the black clear
colour around the square site.

The second weakness was also measured. Against `top-surface` alone, all four top-down overlays
differ by the same 46.6-46.7%, because an overlay also hides the ground cover. A palette that drew
every overlay alike would still pass. So each overlay also has to differ from the one before it:
moisture from light by 25.0%, fertility from moisture by 34.2%, water from fertility by 46.3%. The
run-loader check is `top-surface` against `top-no-run`: 21.1%.

What it cannot catch, stated rather than implied: a palette that is wrong but still distinct, a
tree in the wrong place, a field read from the wrong snapshot. Those are for the person looking at
the pictures, which is what the row says the images are for.

## V13 found by looking: the water legend's `(!)` is false

`top-water`'s legend says `scale from this viewer's fallback: meta.json has no scale for ponded
depth (!)`. The regenerated reference run's `meta.json` does carry it, as
`overlays[water].scale = {lo 1, hi 10000, unit mm, curve log10}`, and has since S10. The viewer never
reads that object: `overlay.rs` builds the water `Scale` from its own `WATER_RAMP_MM` every time. The
numbers happen to agree, so the picture is right and only the claim about its source is wrong. It is
not fixed here, because this shot is the heartbeat and the fix changes what an overlay reads. V12
has to read `scale` objects for the three nutrients G13 publishes the way `water` does, and reading
water's there is the natural place. Recorded so that row starts from it.

## V13 the reference run was regenerated first

`ecosim/runs/capitol-s42` on disk predated G5 (written 02:04 on 2026-09-22; G5 landed 14:15). It
was moved to `runs/capitol-s42-preG5` and rewritten at `d57cd50` with the `capitol` recipe's flags
under `runs/` (`--world worlds/capitol --seed 42 --ticks 20000 --set animals.enabled=false --set
climate.rain_gradient=0`). The run took 39.3 s and `ecosim check` passed. Nothing under `ecosim/` was
edited; `runs/` is gitignored.

## V13 deterministic apart from one HUD number

Three heartbeats on this machine gave the same measurements to the digit. The six top-down PNGs were
byte-identical between runs. The two iso PNGs differed in 233 and 222 pixels, all inside one text
line (rows 323-390, columns 324-354): the HUD's `remesh ... in N ms`, a wall-clock timing. That line
is left alone because the timing is what it is for. The check band starts below it, so it cannot
move a verdict. Warm, the whole set takes 15 s. The first build of the heartbeat took 11-13 minutes,
because a new dependency rebuilt the viewer's tree.

## V12 the three nutrients are three overlays on one key

Worked from the BACKLOG row; V12 has no prompt file. `nitrogen`, `phosphorus` and `potassium` are
overlays 9 to 11 in `Overlay::ALL`, read from `npk.bin` (three f32 planes per ecology column, N, P,
K) and coloured and scaled from the rows `ecosim` shot G13 published. The viewer reads, bands and
draws them. It models no nutrients, and the HUD says so on every frame a nutrient map is on:
`npk.bin as the simulator computed it; the viewer models no nutrients`. Phosphorus adds `the whole
stored pool, not the tenth of it growth can reach`, which G13 found the SAD getting wrong.

**The key is 9, pressed again to cycle N, P and K.** Keys 1 to 8 were taken and 0 is the exposure's.
The three maps answer one question, which element limits growth here, so they share one key and
are compared by pressing it repeatedly. `--overlay nitrogen` and the BRP `ecoview.overlay` method
take the names, as for every other overlay.

**Band 0 is "no soil", off the ramp.** `npk.bin` writes 0 in a column the ecology does not plant
(roof, paving, open water). On a log ramp, 0 is not a small amount of nitrogen. It is off the scale
entirely. So it gets its own category, the neutral grey that dry ground and empty patches already
use, and a nonzero value below the ramp clamps to band 1, the palest tint. For the same reason the
field's min, max and mean are taken over soil columns only; otherwise every minimum would be 0 and
every mean would be diluted by the paved third of the Capitol. The HUD line says `over the columns
with soil`.

**A run without `npk.bin` does not draw a nutrient map.** The overlay switches off with the
reason on screen. The alternative, drawing the whole site as "no soil", would be a false picture of a
run that simply did not compute nutrients. Tested by `npk_bin_is_read_plane_by_plane` (snapshot 0
has no file) and seen on `runs/capitol-s42-preG5`.

## V12 published scales are read, water's included, and only log10 is drawn

`OverlayColors` gains `scale`, and `Scale::of` reads it for `water` and the three nutrients through
one function, `overlay::published`. This fixes the false claim V13 found: until now the water
legend said `(!) meta.json has no scale` while the run had published one since S10. The numbers
agreed, so no picture changes. Only the source line does.

The viewer draws only `curve: "log10"`. Every published scale today is log10. A `linear` one would
be drawn wrong on log bands, so it is refused by name, and the overlay falls back with the reason in
its source line (`...scale is linear, and this viewer draws log10 only`), which the tests check.
Drawing the wrong curve silently is the failure being avoided. A run older than G13 that carries
`npk.bin` falls back to `NUTRIENT_RAMP_G_M2`, which is G13's published numbers copied and named as
the viewer's own. G13's hues are copied the same way, because `ecoview` has no nutrient legend to
fall back on.

Numbers on screen now print with three significant figures below 0.1 (`overlay::sig`).
Phosphorus's scale starts at 0.001 g/m2, and two decimals printed that as `0.00`, a ramp from
nothing.

## V12 what the three maps show, measured

`ecosim/runs/capitol-s42` was regenerated at 1536022 so that its `meta.json` carries G13's rows. The
old copy is `runs/capitol-s42-preG13`. G13 changed `meta.json` only: `npk.bin`, `water.bin` and
`series.csv` are byte-identical between the two. On that run at tick 10000, over the 43,831 soil
columns (21,705 have none):

| pool | p2 | median | p98 | median under a standing tree | elsewhere |
| --- | --- | --- | --- | --- | --- |
| N g/m2 | 0.0138 | 2.93 | 5.64 | **0.0143** | 2.94 |
| P g/m2 | 0.0557 | 51.4 | 52.9 | 50.6 | 51.4 |
| K g/m2 | 1.82 | 34.1 | 41.6 | **26.8** | 34.1 |

As G5 said, these are three different pictures, and they can be told apart without a caption:
- **Nitrogen is speckled.** Of the 3,317 columns under 0.1 g/m2, 2,048 have held a tree at some
  point by tick 10000. The east lawns are also paler in patch-grid squares.
- **Phosphorus is nearly uniform and dark**, with the drainage network etched pale: runoff flow
  lines stripped of particulate P. The p2-to-p98 spread is 3% of the median.
- **Potassium is in between**: an even orange with pale specks. 1,434 of the 1,560 columns under
  10 g/m2 have held a tree. Nothing in the model adds potassium, so a tree's column stays drawn down
  after the tree has gone.

The heartbeat gains `top-nitrogen`, `top-phosphorus` and `top-potassium`. Each must differ from
`top-surface` and from the view before it; they differ from each other by 35.6-35.9% of pixels.
