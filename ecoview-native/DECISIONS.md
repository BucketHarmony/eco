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
