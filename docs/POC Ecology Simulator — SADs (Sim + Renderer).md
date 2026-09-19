# POC Ecology Simulator — SADs (Sim + Renderer)

2026-09-18 · @Someone

## Scope rules and stack

Two repos, one file contract between them. The sim writes a run directory; the renderer plays it back. No IPC, no shared code, so each can be one-shot independently and debugged without the other.

**One-shot criteria.** Every item below is a hard constraint on both SADs.

- One prompt, one session, no human decision mid-build. Anything that would require a design choice at build time is decided in the SAD.
- Target 3,000–6,000 lines per repo including tests. Above that, one-shot reliability drops sharply.
- Every verification is a single command with a non-zero exit on failure. Claude Code runs it, reads the output, fixes, reruns.
- No GPU, no display server, no external service, no account. Everything runs in a headless container.
- Scale is small: 64×64×32 voxels, 2 ground-cover species, 1 tree species, 2 animal species, 20,000-tick runs. Big enough to show succession and a predator–prey cycle; small enough that a full run finishes in seconds.
- Deferred features are listed, not stubbed. No `TODO` hooks, no plugin interfaces.

**Stack.**

| Component | Choice | Why this over alternatives |
| --- | --- | --- |
| Simulator | Rust, single crate, CLI binary + lib | Compiler catches most bugs before a test runs; `cargo test` is the whole loop; deterministic and fast enough that 20k ticks run in seconds. C# would also work and is a drop-in swap if reuse in the colony sim matters. Python was rejected: 130k voxels × field updates is too slow without numpy gymnastics that hurt one-shot reliability. |
| Renderer | TypeScript, Vite, Three.js, static site | Headless Chromium renders WebGL via SwiftShader with no GPU. Playwright takes screenshots Claude Code can view directly as images. Godot/Bevy were rejected: Godot `--headless` does not render, and Bevy offscreen rendering needs Vulkan/lavapipe setup that fails often enough to break a one-shot. |
| Debug loop (sim) | `cargo test` + `ecosim check` invariants + CSV time series | Pass/fail with numbers, no human reading required. |
| Debug loop (renderer) | `npm run shot` → PNGs via Playwright; `npm test` → pixel and DOM assertions | Claude Code views the PNG, compares against the stated expectations, fixes. Playwright MCP is optional on top; the CLI path is sufficient. |
| Contract | Run directory on disk (spec in SAD 1) | Files are inspectable with `ls`, `head`, `jq`. Either side can be regenerated or faked. |

**Build order.** Sim first, to completion. Then renderer, pointed at a committed sample run directory so it never depends on the sim being run locally.

## SAD 1 — Ecology Simulator (`ecosim`)

A headless, deterministic voxel ecology that runs 20,000 ticks from a seed, writes snapshots and a time series, and passes a stated set of stability invariants. Done means `cargo test` and `ecosim check` both exit 0 on three seeds.

### Purpose and non-goals

Prove that light, moisture, fertility, plant succession, grazing, predation and decomposition close into a loop that stays alive for 20k ticks without hand-holding. Not a game, no player, no input, no rendering, no networking.

Out of scope: flowing water, digging or terrain change, weather beyond a sine-wave season, more than the species listed, pathfinding beyond greedy steps, persistence of runs across process restarts.

### World and tiers

World is 64×64 columns × 32 layers. Voxel edge is 1 m. Coordinates are `(x, y, z)` with `z` up. Terrain is heightmap-generated from seeded 2D noise, height 8–24, with a water table at z=10 so low areas hold static pond voxels.

| Tier | Resolution | Holds | Update rate |
| --- | --- | --- | --- |
| Voxel | 1 m³, 131,072 cells | material (`Air`, `Soil`, `Rock`, `Water`), light (u8), moisture (u8), fertility (u8) | light on world change only (once); moisture and fertility every 10 ticks |
| Patch | 8×8 columns, 64 patches | grass density (f32 0–1), shrub density (f32 0–1), detritus (f32), temperature (f32 °C) | every 10 ticks, staggered so \~6 patches update per tick |
| Entity | individual | trees, grazers, hunters | trees every 50 ticks; animals every tick |

Field arrays are flat `Vec<u8>` / `Vec<f32>` indexed `x + 64*(y + 64*z)`. No chunking, no octree. No ECS crate; entities are `Vec<Animal>` and `Vec<Tree>` with a `alive` flag and periodic compaction.

### Abiotic systems

**Light.** Per column, walk down from the top. Air above the first solid voxel gets 255. Each tree canopy voxel in the column subtracts 96 from everything below it. Ground surface light = light of the air voxel above it. Computed once at world gen and recomputed only when a tree is planted or dies (recompute that column only).

**Moisture.** Surface soil voxels only (one per column). Rain adds `rain(season)` every 10 ticks. Water-adjacent surface voxels are clamped to 255. Lateral diffusion: each update, a voxel moves 10% of its difference toward each of its 4 neighbors. Evaporation subtracts `2 + temperature/8` per update. Plants draw down moisture as they grow (see producers).

**Fertility.** Surface soil voxels only. Starts at 128. Detritus in the patch converts to fertility at `decay_rate(temperature, moisture)` per update, spread evenly over the patch's surface voxels. Plants draw it down. No runoff in the POC.

**Temperature.** Per patch. `base + 12·sin(2π·tick/year_len)` with `year_len = 4000` ticks, `base = 12 °C`, minus `0.3` per canopy-covered column fraction. Drives rain, decay, and species suitability.

### Producers

Two density fields per patch and one tree entity type.

| Species | Representation | Growth needs (peak) | Notes |
| --- | --- | --- | --- |
| Grass | patch density | light 200+, moisture 80+, temp 5–30 | fast growth, shaded out by shrub and canopy, eaten by grazers |
| Shrub | patch density | light 120+, moisture 60+, temp 0–28 | slower; suppresses grass by `0.5 × shrub_density`; seeds into neighbor patches when >0.6 |
| Tree | entity, 1 trunk voxel + 3×3×2 canopy at maturity | light 150+ at seed, moisture 100+, temp 2–26 | 3 growth stages by age (sapling, young, mature); mature trees drop 1 seed per 200 ticks into radius 6; die at age 6000 or when moisture <30 for 500 ticks; death adds 40 detritus to the patch |

Growth per update for a density field:

```latex
\Delta d = r \cdot f_L(L) \cdot f_M(M) \cdot f_T(T) \cdot (1 - d) - g \cdot d - \text{grazing}
```

Each `f` is a piecewise-linear suitability curve in \[0, 1\] defined by four numbers (min, low-opt, high-opt, max), stored in a `SpeciesParams` struct. Growth draws moisture and fertility proportionally to `Δd`. All species parameters live in one `params.toml` loaded at start so tuning does not require recompiling.

### Consumers

Two animal species. Each is a struct with position (f32 x, y; z snapped to surface), energy (0–100), age, reproduction cooldown, and a state enum.

| Species | Eats | Start count | Reproduction | Death |
| --- | --- | --- | --- | --- |
| Grazer | grass density in its patch | 60 | energy >70 and cooldown 0 and fewer than 8 grazers in patch → spawn 1, energy −35, cooldown 300 | energy 0, or age 5000, or eaten |
| Hunter | grazers within 2 m | 6 | energy >75 and cooldown 0 → spawn 1, energy −40, cooldown 800 | energy 0, or age 8000 |

Per-tick behavior is a fixed priority list, not a planner: flee if a hunter is within 4 m (grazer only), eat if food is here and energy <90, move toward the best-scoring patch within radius 2 patches (score = food density − crowding), else random walk. Movement is one voxel per tick along the surface.

Stability rules that must be implemented exactly, because without them the POC oscillates to extinction:

1. Grazing intake = `min(3, 20 · grass_density)` energy per tick. Intake collapses as grass thins (type II response) so the last 10% of grass is not stripped.
2. Hunters ignore prey when energy >85. Kill succeeds with probability 0.3 per attempt; a failed attempt costs the hunter 2 energy and the grazer moves 3 voxels away.
3. Grazers standing in a patch with shrub density >0.5 cannot be attacked (refugium).
4. Energy cost per tick: 0.08 grazer, 0.12 hunter, doubled while moving.
5. Corpses add 15 (grazer) or 25 (hunter) detritus to the patch.

### Scheduling and determinism

Fixed tick order: animals → producers (staggered patches) → moisture/fertility (every 10) → temperature/season (every 100) → snapshot (every N) → stats row. One `ChaCha8Rng` seeded from the CLI seed; no other randomness, no `HashMap` iteration in sim logic (use `Vec` or `BTreeMap`). Two runs with the same seed and params must produce byte-identical run directories.

### CLI

| Command | Effect |
| --- | --- |
| `ecosim run --seed 42 --ticks 20000 --out runs/s42 --snapshot-every 100` | Full run; writes the run directory |
| `ecosim check runs/s42` | Applies invariants to `series.csv`; prints each with pass/fail; exit 1 on any failure |
| `ecosim stats runs/s42` | Prints min/max/mean per series column, and tick of first extinction if any |
| `ecosim diff runs/a runs/b` | Byte-compares two run dirs; used by the determinism test |

### Run directory format (the contract with the renderer)

```
runs/s42/
  meta.json           world dims, seed, params snapshot, tick count, snapshot_every, species list with ids and colors
  series.csv          one row per tick: tick, grazers, hunters, trees, grass_mean, shrub_mean, moisture_mean, fertility_mean, detritus_total, temperature
  events.csv          (format_version 3) one row per event: tick,kind,species,patch_x,patch_y,x,y,cause,detail
  snap_000000/
    material.bin      x·y·z × u8, x-fastest (dims from meta.json; 131072 on 64×64×32)
    light.bin         x·y·z × u8
    moisture.bin      x·y × u8 (surface voxels, one per column; 4096 on 64×64)
    fertility.bin     x·y × u8 (surface)
    height.bin        x·y × u8 (surface z per column)
    patches.json      64 entries: grass, shrub, detritus, temperature
    entities.json     [{id, kind, x, y, z, energy, age, state}] for animals and trees (trees carry stage)
  snap_000100/ …
```

All integers little-endian. `meta.json` carries a `format_version: 1`. The renderer treats anything else as a hard error.

Later versions only add files (`ecosim/DECISIONS.md` has the details). Version 2 adds `state.bin` to each snapshot and `forked_from` to `meta.json`. Version 3 adds `events.csv` (plain CSV; a seed-42 20000-tick run writes about 1.7 MB, so it is not compressed):

- Header `tick,kind,species,patch_x,patch_y,x,y,cause,detail`, then one row per event in the order they happen within the run. Absent fields are empty.
- `kind` is one of `death`, `birth`, `ignition`, `spread`, `burnout`, `germination`, `tree_death`, `immigration`, or `seed_drop` (reserved, not written yet).
- `species` is `grazer`, `hunter` or `tree`, and empty for the three fire kinds. `patch_x`, `patch_y` are 0–7. `x`, `y` are the column, and empty for fire kinds.
- `cause`: for `death`, one of `starved`, `eaten`, `old_age`, `crowded`, `burnt`; for `tree_death`, one of `old_age`, `drought`, `crowded`, `burnt`; empty otherwise.
- `detail`: the entity's id for entity kinds (the newborn's for `birth`); for `spread`, the source patch index `patch_x + 8·patch_y`; empty for `ignition` and `burnout`.
- Tick 0 has no events. The file is appended at each snapshot, so rows up to a snapshot's tick are on disk when its directory is.

Version 4 is a run whose world was built from a world bundle (`ecosim run --world`, `docs/SCENE-CONTRACT.md`) rather than from noise. Everything above is unchanged; the bundle's static ground grid is added once at the run root, not per snapshot:

```
runs/g1/
  world/
    ground_h.bin      gw·gd × f32 LE, x-fastest, metres above the crop minimum
    medium.bin        gw·gd × u8, an index into meta.json's world.media
    building_h.bin    gw·gd × f32 LE, roof height above ground, 0 where there is no roof
    pipes.json        [{id, inlet: [x, y], outlet: [x, y], capacity_m3h, illustrative}], metres from the SW corner
```

`meta.json` gains `"world": {"name", "ground_cell_m", "ground_width", "ground_depth", "media": [...]}`, where `ground_width` × `ground_depth` is the ground grid (`gw`, `gd` above) and `media` maps a `medium.bin` code to its name. The ground grid is finer than the ecology grid: `ground_width = dims.x / ground_cell_m`, with cell (0, 0) at the south-west corner, x east and y north, the same orientation as the voxel fields.

### Debug loop and tests

Claude Code's loop is: `cargo build` → `cargo test` → `ecosim run` on seeds 1, 2, 3 → `ecosim check` on each → read failures → adjust `params.toml` or code → repeat. No step needs a human.

Unit tests (each a function under `#[cfg(test)]`): suitability curves return expected values at the four breakpoints; light column with one canopy voxel; moisture diffusion conserves total mass minus evaporation; type II intake caps at 3; refugium blocks attack; snapshot round-trips through the reader in `tests/`.

Integration tests: determinism (two runs, `ecosim diff` clean); a run with rain forced to zero ends with grass\_mean <0.05 by tick 5000 (drought sanity); a run with hunters disabled reaches grazer carrying capacity and stays within ±30% of it after tick 8000.

### Acceptance invariants (`ecosim check`)

Evaluated on `series.csv` over ticks 2,000–20,000 unless stated:

- No species count reaches 0.
- No species count exceeds 10× its tick-2000 value.
- Grazer count has at least 2 local maxima separated by ≥1500 ticks (a cycle exists, not a flat line).
- `fertility_mean` stays in \[40, 220\].
- `grass_mean` stays in \[0.05, 0.95\].
- Tree count at tick 20,000 is ≥ 1.5× tree count at tick 0 (succession happened).
- Run time under 30 s on one core in release mode.

All seven must pass on seeds 1, 2 and 3. Tuning `params.toml` until they do is part of the one-shot; the SAD's starting values are a guess and Claude Code is expected to iterate on them.

## SAD 2 — Renderer (`ecoview`)

A static web page that loads a run directory, draws the voxel world in 3D with switchable field overlays and entity markers, scrubs through snapshots, and shows the population chart. Done means `npm test` exits 0 and the eight reference screenshots in `shots/` match their stated descriptions.

### Purpose and non-goals

Let a person, or Claude Code looking at a PNG, see whether the ecology looks right: shade under trees, green where wet, grazers on grass, hunters near grazers, succession over time. Not an editor, no live connection to the sim, no mobile layout, no saving of camera state.

Out of scope: greedy meshing, ambient occlusion, textures, shadows, animation between snapshots, more than one run loaded at a time, any server beyond `vite preview`.

### Architecture

Single-page TypeScript app, Vite build, Three.js r16x pinned in `package.json`. No framework; DOM controls are plain HTML with \~10 event listeners. Four modules:

| Module | Responsibility | Size target |
| --- | --- | --- |
| `loader.ts` | Fetch `meta.json`, `series.csv`, and one `snap_NNNNNN/` on demand. Parse `.bin` into typed arrays. Reject `format_version ≠ 1`. | \~200 lines |
| `world.ts` | Build one `InstancedMesh` of unit cubes for surface voxels only (top voxel per column plus any exposed side faces are skipped; the POC draws top voxels and water voxels). Per-instance color from the active overlay. Rebuild on snapshot change. | \~250 lines |
| `entities.ts` | Trees as stacked cubes (trunk + canopy per stage); animals as colored spheres at surface height. Rebuilt on snapshot change. | \~150 lines |
| `ui.ts` | Overlay select, snapshot slider, play/pause, tick readout, population chart (Canvas 2D, no chart library), URL-parameter parsing. | \~250 lines |

Camera: `OrbitControls` from Three's examples, default isometric-ish position looking at world center. Render loop only redraws on change (`renderer.render` on demand), so screenshots are deterministic.

### Views

| Overlay | Color mapping |
| --- | --- |
| `material` | Soil brown, rock gray, water blue; grass density tints soil toward green, shrub toward dark green |
| `light` | Grayscale, 0 = black, 255 = white |
| `moisture` | White → blue |
| `fertility` | White → dark brown |
| `temperature` | Patch-level, blue → red across 0–30 °C |

Entities draw on every overlay: grazer = yellow sphere, hunter = red sphere, tree = trunk brown + canopy green, canopy size by stage. Colors come from `meta.json` species list so the sim owns them.

URL parameters drive state so the debug loop needs no clicking: `?run=runs/s42&tick=5000&overlay=moisture&cam=iso|top|side`. Missing params default to tick 0, `material`, `iso`.

### Debug loop

The loop is: `npm run build` → `npm run shot` → Claude Code views each PNG in `shots/` and compares it to the expectation column below → fix → repeat. Playwright runs headless Chromium with `--use-gl=swiftshader --enable-unsafe-swiftshader`; no GPU. Playwright MCP can be used for ad-hoc inspection but nothing in the build depends on it.

`npm run shot` starts `vite preview`, opens the URL for each row, waits for `window.__ecoviewReady === true`, screenshots at 1280×800, and exits.

| File | URL params | What Claude Code should see |
| --- | --- | --- |
| `01_material_t0_iso.png` | tick=0, material, iso | Heightmapped terrain, ponds in low areas, sparse trees, no green tint yet |
| `02_material_t10000_iso.png` | tick=10000, material, iso | More trees, green tint on most soil, dark-green shrub bands, yellow and red dots |
| `03_light_t10000_top.png` | tick=10000, light, top | Dark squares under each canopy, white elsewhere |
| `04_moisture_t10000_top.png` | tick=10000, moisture, top | Blue halos around ponds fading outward |
| `05_fertility_t10000_top.png` | tick=10000, fertility, top | Darker patches where trees or corpses have decayed |
| `06_temperature_t1000_top.png` | tick=1000, temperature, top | Near-uniform warm color (summer), slightly cooler under canopy |
| `07_temperature_t3000_top.png` | tick=3000, temperature, top | Near-uniform cool color (winter) |
| `08_chart_t20000.png` | tick=20000, material, iso | Chart panel shows grazer curve oscillating, hunter curve lagging it, tree curve rising |

### Tests (`npm test`, Vitest + Playwright)

- `loader` parses the committed sample run `fixtures/s42-mini/` (a 2-snapshot run checked into the repo) and returns arrays of the right lengths and value ranges.
- `loader` throws on `format_version: 2`.
- Overlay color functions return the stated endpoint colors at 0 and 255.
- Playwright: page loads fixture, `__ecoviewReady` becomes true within 5 s, no console errors.
- Playwright: the `03_light` screenshot has at least 5% of pixels darker than RGB(60,60,60) (canopy shade exists) and at least 40% lighter than RGB(200,200,200).
- Playwright: switching `overlay` param changes the mean pixel color of the canvas by more than 20 units in at least one channel.
- Playwright: chart canvas is non-blank at tick=20000 (more than 1% non-background pixels).

Pixel assertions are deliberately coarse. Exact-image golden tests break on font or driver changes and would cost the one-shot more than they return.

### Acceptance

- `npm run build` clean, `npm test` exit 0, `npm run shot` produces all 8 PNGs in under 60 s.
- The 8 PNGs match their expectation column when viewed. Claude Code makes this judgment itself by viewing the files; it records a one-line verdict per file in `shots/REPORT.md`.
- Loading a full 200-snapshot run and scrubbing the slider end to end does not drop below 20 fps in headless Chromium (measured by Playwright, `requestAnimationFrame` count over 2 s after a slider change).

## Handoff to Claude Code

Run two sessions, sim first. Each session's prompt is the corresponding SAD pasted whole, plus the three lines below. Do not paste both SADs into one session; the renderer session gets a committed `fixtures/s42-mini/` instead of a sim binary.

**Sim session prompt suffix.**

```markdown
Build this in a fresh `ecosim/` directory. Work until `cargo test` passes and `ecosim check` passes on seeds 1, 2, 3. Tune `params.toml` as needed; log every parameter change and its effect in `TUNING.md`. When done, run seed 42 for 20000 ticks with snapshot-every 100 into `runs/s42/` and a 2-snapshot mini run into `fixtures/s42-mini/`. Do not ask questions; make the call and record it in `DECISIONS.md`.
```

**Renderer session prompt suffix.**

```markdown
Build this in a fresh `ecoview/` directory. `fixtures/s42-mini/` and `runs/s42/` are provided and are the only data. Work until `npm test` passes and all 8 screenshots in `shots/` match their expectation column; write your verdict per screenshot in `shots/REPORT.md`. Do not ask questions; make the call and record it in `DECISIONS.md`.
```

**Where a one-shot is most likely to fail, and the fallback.**

| Risk | Signal | Fallback already allowed by the SAD |
| --- | --- | --- |
| Sim never passes the cycle invariant (grazers flatline or crash) | `ecosim check` fails invariant 3 or 1 after \~10 tuning rounds | Relax hunter kill probability to 0.2 and raise refugium threshold to shrub >0.4; both are `params.toml` values |
| Tree count does not rise (invariant 6) | trees stuck near start count | Seed radius 6 → 10, sapling light need 150 → 100 |
| SwiftShader WebGL fails in the container | Playwright screenshots are blank or `WebGL not supported` in console | Add `--ignore-gpu-blocklist --disable-gpu-sandbox`; if still blank, use `three`'s `WebGLRenderer` with `powerPreference: 'low-power'` and `antialias: false` |
| Screenshot judgment is wrong | REPORT.md verdicts disagree with the PNGs on inspection | The pixel tests are the floor; a wrong verdict on 01–08 is caught by a person in 5 minutes, which is the intended review cost |

**Deferred, in order of value once the POC holds.** Flowing water (cellular, Timberborn-style) feeding moisture; terrain modification with light recompute; a third animal at a different trophic level; loading a run over HTTP from a live sim process; greedy meshing once world size exceeds 128×128.
