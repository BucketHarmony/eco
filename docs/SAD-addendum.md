# SAD addendum: settled decisions

2026-09-18. This file is authoritative wherever it differs from `POC Ecology Simulator — SADs (Sim + Renderer).md`. It settles everything the SAD leaves open, so a one-shot session never has to make a cross-component design choice. Numeric values marked *(param)* are **starting values** in `params.toml`. The sim session may tune them and must log changes in `TUNING.md`. Everything else is fixed.

---

## Environment (verified 2026-09-18)

- The build runs natively on Windows 11 (not a Linux container). Git Bash and PowerShell are both available.
  - Rust 1.98.1 with the stable-x86_64-pc-windows-msvc toolchain.
  - Node 22.12 and npm 10.9.
- If `cargo` isn't on PATH in a new shell, it lives at `%USERPROFILE%\.cargo\bin`.
- npm scripts must be cross-platform. That means Node scripts, not bash-isms (`rm -rf`, `&&` chains that rely on POSIX, env-var prefixes).
- **Chromium WebGL flags.** Use `--use-angle=swiftshader --enable-unsafe-swiftshader`.
  - The SAD's `--use-gl=swiftshader` **fails** here: the context is lost and the page renders blank.
  - Verified with Playwright 1.63.0 (headless shell rev 1243) + three 0.169.0: the cube renders correctly.
  - A 4,700-instance `InstancedMesh`, fully rebuilt every frame at 1280×800, ran at about 41 fps.

## Dependencies (pin exact versions)

- **ecosim:**
  - `rand = "0.8"` and `rand_chacha = "0.3"`. Use the 0.8 API; don't mix in the 0.9 API.
  - `serde` (derive), `serde_json`, `toml`, `clap` (derive).
  - No noise crate: write seeded value noise by hand. No other dependencies.
- **ecoview:**
  - `three@0.169.0`, `@types/three` matching it, `vite`, `typescript`, `vitest`.
  - `@playwright/test@1.63.0`, whose Chromium is already installed. Run `npx playwright install chromium` if it's missing.

---

## Simulator

### Ticks, snapshots, series
- Tick 0 is the generated initial state, recorded before any update runs. The sim then steps ticks `1..=ticks`.
- A snapshot is written whenever `tick % snapshot_every == 0`. `--ticks 20000 --snapshot-every 100` therefore gives `snap_000000` … `snap_020000`, 201 snapshots. The directory name is the tick, zero-padded to 6 digits.
- `series.csv` has a header row plus one row per tick for 0..=20000 (20,001 data rows).
  - Columns, in this order: `tick,grazers,hunters,trees,grass_mean,shrub_mean,moisture_mean,fertility_mean,detritus_total,temperature`.
  - Floats are written with `{:.4}`.
  - `grass_mean` and `shrub_mean` are means over patches that have at least one soil column.
  - `moisture_mean` and `fertility_mean` are means over soil columns.
  - `temperature` is the mean over all patches.
- **Timing.** `ecosim run` writes `timing.json` = `{"wall_ms": N}`.
  - `ecosim diff` ignores `timing.json`; everything else must match byte for byte.
  - `ecosim check` reads `timing.json` for the runtime invariant (< 30,000 ms).
  - The meaningful timing is from a release build.
- The canonical test command is **`cargo test --release`**, because the integration tests run long sims.
  - Integration tests call the library directly with a modified `Params` rather than shelling out. The exception is the determinism test, which may shell out.
  - Tests load `params.toml` from `env!("CARGO_MANIFEST_DIR")`, so tuning applies to them too.

### World generation
- **Heights.** Seeded value noise with 2 octaves (periods 32 and 8 columns, amplitudes 1.0 and 0.35), normalized to height `h ∈ [8, 24]`.
- **Materials.** Codes are `Air=0, Soil=1, Rock=2, Water=3`.
  - `z ≤ h` is solid.
  - The top 3 solid layers are `Soil`, except that if `h ≥ 21` the top voxel is `Rock`. Everything below is `Rock`.
- **Water.** Every `Air` voxel with `h < z ≤ 10` becomes `Water`.
  - The generator must give seed 42 at least 100 water-topped columns. Adjust the normalization, not the noise, if it doesn't.
- **Column classes.**
  - Soil column: the topmost non-Air voxel is `Soil`.
  - Water column: the topmost non-Air voxel is `Water`.
  - Rock column: everything else.
  - Only soil columns carry moisture, fertility, plants, tree trunks and animals. Animals never enter water or rock columns.
- `height.bin` holds, per column, the z of the topmost non-Air voxel, so water columns report the water surface.
- **Patches.** Patch `(px, py) = (x/8, y/8)`, index `px + 8*py`. Patch soil-column count `n_soil` can be 0; such a patch has densities fixed at 0.

### Field storage
- `material` and `light` are 3D `Vec<u8>`s indexed `x + 64*(y + 64*z)`.
- Surface moisture and fertility are stored internally as **`Vec<f32>` of length 4096**, because 10% diffusion and fractional draws don't survive u8 rounding. They are written to the snapshot as `round().clamp(0, 255) as u8`. Water and rock columns write 0.
- Initial moisture and fertility are 128 on soil columns.

### Light
- `light.bin`: solid voxels are 0. Air and water voxels hold `255 ×` the fraction of full sun that reaches them, which since shot G4c is the Beer–Lambert transmittance of the canopy above: `255 × exp(−canopy_k × canopy_lai × (number of canopy voxels strictly above them in the column))`. The byte and its range are unchanged, so `format_version` does not move; only what the byte means is now stated.
  - `canopy_k` *(param)* = **0.5** and `canopy_lai` *(param)* = **2.0**, giving an optical depth of 1.0 per canopy voxel. A 2-layer mature canopy is then LAI 4 and leaves the ground at **35** (13.5% of full sun, inside the published 10–25% for a broadleaf canopy at LAI 3–5), so it still clears the renderer test that needs pixels darker than 60; a 1-layer young canopy leaves 94. Light now thins towards zero instead of saturating at it: three layers leave 13.
  - Before shot G4c this was `255 − canopy_absorb × layers` with `canopy_absorb` *(param)* = 100, which left 155, 55 and 0 for one, two and three layers.
- Surface light for growth is the light of voxel `(x, y, height + 1)`.
- Light is recomputed per column, only for the columns a tree's canopy covers, when that tree changes stage, is planted, or dies. This replaces the SAD's "once".

### Trees
> This section still describes the tree tier as it was before the units calibration (shots G4b and G4c): the ages, the drought clock and the seeding interval below are tick counts and a 0–255 moisture threshold, and the shipped parameters are now years, days, a fraction of available water capacity and a fraction of full sun. The rule shapes are unchanged; for the current names and values read `ecosim/params.toml` and `ecosim/UNITS.md`.

- **Stages by age** *(params)*:
  - sapling, `age < 500`: trunk only.
  - young, `age < 2000`: trunk plus a 1×1×1 canopy.
  - mature: trunk plus a 3×3×2 canopy.
- **Geometry.** For a tree on soil column `(x, y)` with height `h`:
  - The trunk is at `z = h+1`.
  - The canopy is at `z = h+2` (young) or `z = h+2..=h+3` (mature), 3×3 centered on the trunk and clipped to world bounds.
  - Canopy and trunk are **not** material values (material stays Air). They exist only as tree entities.
- **Spacing.** Two trunks must be at least Chebyshev distance 2 apart.
- **Update.** Every tree updates on ticks where `tick % 50 == 0`, gaining 50 age.
  - A tree draws `tree_moisture_draw` *(param, 2.0)* from its column per update.
  - It tracks `dry_ticks`: +50 while its column's moisture is < 30, otherwise reset to 0. At `dry_ticks ≥ 500` it dies.
  - It also dies at age 6000.
  - On death, add 40 detritus to its patch and recompute light for its canopy columns.
- **Seeding.** A mature tree drops a seed when `age % 200 == 0`.
  - The target is uniform in a disc of radius `seed_radius` *(param, 6)*, rounded to integers.
  - The seed germinates only on a soil column that satisfies the spacing rule, with probability `f_L(surface light) × f_M(moisture) × f_T(patch temperature)`. The tree suitability curves have light peak ≥ `sapling_light` *(param, 150)*.
- **Initial trees.** 12 trees at age 500 (young), placed at random on valid soil columns.

### Producers (patch densities)
- **Update schedule.** Patch `p` runs its producer update on ticks where `tick % 10 == p % 10`, which staggers the 64 patches to about 6 per tick.
  - Inputs are patch means over soil columns: surface light, moisture, fertility. Temperature is the patch temperature.
- **Growth.**
  - `Δd = r · f_L · f_M · f_T · f_F · (1 − d) · s − g·d`.
  - `f_F = clamp(F/64, 0, 1)` is added so growth stops, rather than going negative, when fertility is exhausted.
  - `s` is the suppression factor: `1 − 0.5·shrub` for grass, 1 for shrub.
  - Grazing is not part of this update; it's applied immediately when grazers eat (see below).
  - Clamp `d` to `[0, 1]`.
- **Starting params.**

  | | r | g | light (min, lo, hi, max) | moisture | temp °C |
  | --- | --- | --- | --- | --- | --- |
  | grass | 0.05 | 0.005 | 100, 200, 255, 256 | 20, 80, 255, 256 | 0, 5, 30, 35 |
  | shrub | 0.01 | 0.002 | 40, 120, 255, 256 | 15, 60, 255, 256 | −5, 0, 28, 33 |
  | tree (germination) | — | — | 60, 150, 255, 256 | 30, 100, 255, 256 | −3, 2, 26, 31 |

  A suitability curve is 0 at or below `min` and at or above `max`, rises linearly to 1 at `lo`, stays at 1 through `hi`, and falls linearly to `max`.
- **Resource draw.** When `Δd > 0`, every soil column in the patch loses `moisture_draw·Δd` *(param, 40)* of moisture and `fertility_draw·Δd` *(param, 20)* of fertility, each floored at 0.
- **Litter.** When the `−g·d` mortality term applies, `litter_factor·g·d·n_soil` *(param, `litter_factor` = 20)* detritus goes to the patch. This closes the nutrient loop: dead plant mass returns at the same scale it was drawn.
- **Shrub spread.** If `shrub > 0.6` after an update, each 4-neighbor patch with `n_soil > 0` is set to `max(its shrub, 0.02)`.

### Moisture, fertility, detritus (all on ticks where `tick % 10 == 0`)
The steps run in this order:
1. **Rain.** Add `rain(t) = rain_base − rain_amp·sin(2π t / year_len)` *(params: 8, 4)* to every soil column. Winters are wetter.
2. **Diffusion.** A Jacobi (double-buffered) update across 4-neighbor soil columns only: `m' = m + 0.10·Σ(m_n − m)`.
3. **Evaporation.** Subtract `evap_base + T/evap_div` *(params: 2, 8)*, using the patch temperature, floored at 0.
4. **Pond wetting.** Set soil columns 4-adjacent to a water column to `pond_moisture` *(param, 255)*. The drought test sets `rain_base = rain_amp = 0` **and** `pond_moisture = 0`.
5. **Clamp** moisture to `[0, 255]`.
6. **Decay.** For each patch, `converted = detritus · decay_rate(T, M)`, where `decay_rate = decay_k · clamp(T/30, 0, 1) · M/255` *(param, `decay_k` = 0.02)* and M is the patch mean moisture. Each soil column gains `converted / n_soil` fertility, and detritus drops by `converted`.
7. **Clamp** fertility to `[0, 255]`.

### Temperature (on ticks where `tick % 100 == 0`, and at tick 0)
- `T = base + amp·sin(2π·tick/year_len) − canopy_cool·canopy_fraction` *(params: base 12, amp 12, year_len 4000, canopy_cool 3.0)*.
- `canopy_fraction` = the patch's columns under any canopy voxel ÷ 64.
- This puts summer peaks at ticks 1000, 5000, … and winter troughs at 3000, 7000, ….

### Animals
- **Entity fields.**
  - `id`: u32, globally unique across all entity kinds, never reused, assigned in creation order.
  - `x, y`: f32 holding integer values.
  - `energy`, `age`, `cooldown`, `state`.
- **Update rules.**
  - Update order is Vec index order, grazers first and then hunters. Newborns are appended and first act on the next tick.
  - Dead entities are removed during compaction, which runs every 100 ticks.
  - `age += 1` every tick. `cooldown` counts down to 0.
- **Initial population.**
  - 60 grazers and 6 hunters on random soil columns.
  - Energy 60, age uniform in `0..1000`, cooldown uniform in `0..=cooldown_max`, so they don't die or breed in lockstep.
- **Newborns.**
  - Spawn on the parent's column.
  - Energy 30 for a grazer and 40 for a hunter, age 0, cooldown at its full value.
- **Movement.**
  - An animal moves one step to one of its 8 neighboring columns, only onto soil columns inside the world. Distances are Euclidean in xy.
  - Moving costs twice the base energy cost that tick.
  - Random walk picks uniformly among valid neighbors; if there are none, the animal stays.
- **Grazer priorities:**
  1. `flee`: a hunter is within 4. Step to the valid neighbor that maximizes distance from the nearest hunter.
  2. `eat`: patch grass > 0 and energy < 90. Intake is `min(3, 20·grass)`. The patch's grass drops immediately by `intake × grass_per_energy` *(param, 0.0004)*, floored at 0.
  3. `move`: the best patch within Chebyshev patch-distance 2 is not the current patch. Score is `grass − grazers_in_patch / 8`, and ties go to the lowest patch index. The grazer steps toward that patch's center.
  4. `wander`: random walk.
- **Hunter priorities:**
  1. `rest`: energy > 85. Random walk.
  2. `hunt`:
     - A grazer within 2 whose patch has shrub ≤ `refugium_shrub` *(param, 0.5)* triggers one attack per tick.
     - Success probability is `kill_prob` *(param, 0.3)*. A kill gives the hunter `+40` energy *(param `kill_energy`)*, capped at 100, and the grazer dies.
     - On a failure, the hunter loses 2 energy and the grazer is displaced 3 steps directly away from the hunter, stopping early at invalid columns.
  3. `move`: step toward the nearest attackable grazer within 16.
  4. `wander`: random walk.
- **Reproduction.** Checked after the action, using the SAD's thresholds.
  - Grazers also need fewer than 8 grazers in their patch (`max_grazers_per_patch`, a param).
- **Death.**
  - Energy ≤ 0, the SAD's age limits, or being killed.
  - Every grazer death adds 15 detritus and every hunter death adds 25 (params), to the patch where it happened.
- `state` in snapshots is a lowercase string: `flee | eat | move | wander | rest | hunt`.

### Output files
- **`meta.json`**, with keys in this order (serde struct order):
  - `format_version`: 1
  - `dims`: `{x: 64, y: 64, z: 32}` on the original square world. Since sim shot 15 it is `{x, y, z, patch}` from `[world] width, depth, height, patch`: `{x: 256, y: 64, z: 32, patch: 8}` on the reference strip. Every `.bin` size follows from it.
  - `seed`, `ticks`, `snapshot_every`, `year_len`, `water_level`: 10
  - `snapshots`: an array of snapshot ticks
  - `species`
  - `params`: the full loaded params as JSON
- **`species`** entries have fixed ids:
  - `{id:0, name:"grass", kind:"cover", color:"#7cc242"}`
  - `{id:1, name:"shrub", kind:"cover", color:"#2f6b2a"}`
  - `{id:2, name:"tree", kind:"tree", color:"#6b4a2b", canopy_color:"#2e8b3d"}`
  - `{id:3, name:"grazer", kind:"animal", color:"#f2d024"}`
  - `{id:4, name:"hunter", kind:"animal", color:"#d6332a"}`
- **`patches.json`**: 64 objects `{grass, shrub, detritus, temperature}` in patch index order.
- **`entities.json`**: an array with trees first, then grazers, then hunters, each group in ascending id order.
  - Trees: `{id, kind:"tree", x, y, z, age, stage:"sapling"|"young"|"mature"}`. `z` is the trunk voxel.
  - Animals: `{id, kind:"grazer"|"hunter", x, y, z, energy, age, state}`. `z` is `height + 1`.
- JSON is written with `serde_json` (compact), in struct field order. Nothing is hash-ordered.

### CLI details
- `ecosim run` takes `--params <path>`, default `params.toml` in the current working directory.
- `ecosim diff` prints each differing or missing path and exits 1 if there are any.
- `ecosim check` prints one line per invariant: `PASS`/`FAIL`, the name, and the observed value.

### Invariant definitions
- "Species" in invariants 1–2 means grazers, hunters and trees.
- Invariant 3 is about local maxima:
  - First smooth the grazer count with a centered 200-tick moving average.
  - A local maximum is a tick in [2000, 20000] where the smoothed value is ≥ every other smoothed value within ±500 ticks and strictly > at least one of them.
  - Plateau ties collapse to their first tick.
  - Two such maxima must be ≥1500 ticks apart.
- The hunters-disabled test (`hunters.start_count = 0`) defines carrying capacity K as the mean grazer count over ticks 8000–20000. Every 200-tick moving-average value in that window must be within `[0.7K, 1.3K]`.

### Extra acceptance, so the renderer's tests can pass on `runs/s42`
These run against `runs/s42`; add them to `ecosim check`.
- At tick 10000: ≥ 35 mature trees. The renderer's light-shade pixel test needs 5% of the 960×800 view dark. One mature canopy shades 9 columns × 156 px, about 0.18% of the view, so it takes about 30 non-overlapping mature canopies. 35 leaves a margin.
- At tick 10000: grazers ≥ 10 and hunters ≥ 2.

### Deliverables
Everything below is relative to `ecosim/`.
- `runs/s42/`: `ecosim run --seed 42 --ticks 20000 --out runs/s42 --snapshot-every 100`. This is gitignored because it can be regenerated.
- `fixtures/s42-mini/`: `ecosim run --seed 42 --ticks 100 --out fixtures/s42-mini --snapshot-every 100`, which gives `snap_000000` and `snap_000100`. It is committed.

---

## Renderer

### Data location
- Data is served from `ecoview/public/`: `public/runs/s42/` (full run) and `public/fixtures/s42-mini/`. Both are copied in by `scripts/sync-data.sh` at the repo root.
  - The `run` URL param is a path relative to the site root: `?run=runs/s42` fetches `/runs/s42/meta.json`.
  - The default `run` is `fixtures/s42-mini`.
- The `tick` param picks the snapshot with the largest tick ≤ `tick`. Values past the end clamp to the last snapshot.
- `series.csv` is loaded once per run.

### Page layout (1280×800 viewport)
- Left: the 3D view, `<canvas id="view">`, 960×800.
- Right: a 320 px sidebar with controls and the chart, `<canvas id="chart">` about 300×220, white background. The chart plots grazers, hunters and trees (each normalized to its own max, or on a shared axis, the builder's call) with a vertical marker at the current tick.
- The chart never overlaps `#view`. Every pixel assertion about the 3D view is measured on a screenshot of `#view` only.
- View background is `#e8ecf0`.

### Coordinates and cameras
- The sim column at `(x, y)` with height `h` maps to the instance centered at Three `(x + 0.5, h + 0.5, 63.5 − y)`, with sim z mapping to Three y (up). In top view, +x points right and +y points up the screen.
- `iso`: a PerspectiveCamera (fov 45) at world center + (70, 75, 70), looking at `(32, 12, 32)`.
- `side`: a PerspectiveCamera at `(32, 30, −60)` looking at center.
- `top`: an **OrthographicCamera** straight down, framing exactly the 64×64 world to the view height, so each column is 12.5 px.

### Materials and colors
- **Surface voxels**
  - `top` camera: `MeshBasicMaterial` (unlit), so pixel color equals the overlay color.
  - `iso` and `side`: `MeshLambertMaterial` with ambient 0.6 and directional 1.0.
  - Set instance colors with `Color.setRGB(r, g, b, SRGBColorSpace)`, so sRGB round-trips exactly.
- **Water voxels** are drawn blue `#3a6fd8` on every overlay except `light`, where they use their light value.
- **Overlay color functions** are pure, exported, and unit-tested at 0 and 255:
  - `light`: gray `v`.
  - `moisture`: `#ffffff` → `#1f4fd1`.
  - `fertility`: `#ffffff` → `#4a2c12`.
  - `temperature`: `#2040ff` at 0 °C → `#ff3020` at 30 °C, clamped.
  - `material`: soil `#8b6b47`, rock `#8a8a8a`. Soil is lerped toward grass `#7cc242` by `grass`, then toward shrub `#2f6b2a` by `shrub × 0.8`.
- **Light overlay** samples `light.bin` at `(x, y, height + 1)`.
- **Entities**
  - Colors come from `meta.json`.
  - Trees: a trunk cube, plus canopy cubes for young and mature trees.
  - Grazers and hunters: spheres of radius 0.45 at `height + 1`.
  - On every overlay except `material`, **canopy cubes render at opacity 0.25**, so the field under them (especially the light shade) stays visible.

### Readiness and render-on-demand
- There is no continuous render loop. Render once after each state change.
- `window.__ecoviewReady` is set to `false` whenever the URL state or snapshot changes. After the snapshot's meshes are built and one frame is rendered, it's set to `true` inside a `requestAnimationFrame` callback.
- **fps test.** Load `?run=runs/s42` and set the slider to a new snapshot via an `input` event. Then count `requestAnimationFrame` callbacks for 2 s using a counter the test injects. The count must be ≥ 40.

### Scripts
All scripts are cross-platform Node.
- `npm run build`: `tsc --noEmit && vite build`.
- `npm test`: `vitest run && playwright test`. `playwright.config.ts` has a `webServer` running `vite preview --port 4173 --strictPort`, and passes the Chromium flags above through `launchOptions.args`.
- `npm run shot`: `node scripts/shot.mjs`. It starts `vite preview` through Vite's JS `preview()` API, takes the 8 screenshots (full page, 1280×800) into `shots/`, then closes the server. It assumes `npm run build` has already run.

### Fixes to SAD text
- `world.ts` draws exactly one top voxel per column (soil, rock or water). It draws no side faces and no buried voxels.
- Pixel-test color thresholds compare all three channels: "darker than RGB(60, 60, 60)" means r, g and b are all < 60, and "lighter than RGB(200, 200, 200)" means all three are > 200.
