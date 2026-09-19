# ecoview decisions

Choices the SAD and addendum leave open, or where the two conflicted with the data.

## Dependencies
- `three@0.169.0` and `@types/three@0.169.0` pinned exactly, as is `@playwright/test@1.63.0`. `vite`, `vitest` and
  `typescript` are pinned to the latest versions at build time (vite 8.3.0, vitest 5.0.1, TypeScript 7.0.2).
  `@types/node` was added so the unit tests can read the fixture from disk.

## Canopies are hidden in the top camera on field overlays
The addendum says canopy cubes render at opacity 0.25 on every overlay except `material`, and it also requires
5% of `03_light` to be darker than RGB(60,60,60) in **all three channels**. These conflict. A mature canopy
leaves the ground at light 55, but a 0.25-opacity `#2e8b3d` canopy over it gives green ≈ 0.25·139 + 0.75·55 ≈ 76.
With the two stacked canopy layers seen from above it's ≈ 92. Every shaded pixel would then fail the green
channel, so the test couldn't pass. The addendum's own 5% budget ("9 columns × 156 px") also assumes the shaded
ground is what's visible.

Decision: in the `top` camera on field overlays, canopies are not drawn. Trunks, which are opaque, still mark
every tree, and the shaded squares show exactly where the canopies are. In `iso` and `side` on field overlays,
canopies render at opacity 0.25 with `depthWrite` off, as specified. On `material` they are opaque in every camera.
Measured on `03_light`: 20.6% dark and 66.5% light, against the 5% and 40% floors.

## Lighting intensity scaled by π
The addendum's ambient 0.6 / directional 1.0 are legacy-lighting numbers. three r155+ uses physically based
units, where Lambert diffuse is divided by π, so those values render the iso scene about 30% too dark (soil comes
out near-black olive). Both intensities are multiplied by π, which gives the intended `albedo · (0.6 + cosθ)`.
The top camera uses `MeshBasicMaterial` everywhere, so it's unaffected and pixel colors equal overlay colors exactly.

## Overlay details
- **Rock columns** on `moisture` and `fertility` draw rock gray `#8a8a8a`. The sim writes 0 there, meaning "no
  data", which would otherwise render as white (dry, or infertile) and look like real data.
- **`temperature`** colors every non-water column, rock included, by its patch temperature, because temperature
  exists for all patches.
- **`light`** samples `light.bin` at `(x, y, height+1)` for every column, water and rock included, as the addendum says.
- Entities use unlit `MeshBasicMaterial` in the top camera and Lambert in `iso`/`side`, matching the surface.

## Chart
Grazers, hunters and trees are each normalized to their own max, because grazers reach ~4,400 while hunters
peak under 100. The legend shows each max (`grazers ≤4453`). Each horizontal pixel plots the mean of its bucket
of rows, which keeps drawing 20,001 rows cheap and damps per-tick noise. The line colors are the species colors
from `meta.json`. A dashed vertical line marks the current snapshot tick.

## Cameras
- `iso` sits at world center (32, 16, 32) + (70, 75, 70) and looks at (32, 12, 32). The addendum doesn't say what
  "world center" is, so this uses the geometric center of the 64×32×64 volume.
- `side` sits at (32, 30, −60) and looks at (32, 12, 32), as specified. From that close it crops the world's left
  and right edges. It is left as specified.
- `top` is an orthographic camera with `up = (0, 0, −1)`, so sim +x points right and +y points up, framing 64
  world units into 800 px.
- OrbitControls are attached to every camera, with rotation disabled in `top`. They re-render only on their
  `change` event.

## Runtime
- Loaded snapshots are cached in memory (8 most recent) so playback and scrubbing back and forth don't refetch.
  A sequence token drops stale async loads when the state changes mid-fetch.
- Play steps one snapshot every 250 ms using `setInterval`. This is a timer, not a render loop: each step
  triggers one on-demand render.
- A bad `format_version`, a missing file or a wrong binary size shows `Error: …` in the sidebar, sets
  `window.__ecoviewError`, and leaves `__ecoviewReady` false.
- The page is fixed at 1280×800: `#view` 960×800 at pixel ratio 1, plus the 320 px sidebar.

## Scripts and tests
- `npm test` has a `pretest` hook that runs `npm run build`, so the Playwright `webServer` (`vite preview`) never
  serves a stale `dist/`.
- `npm run shot` uses port 4174, not 4173, so it can run while a preview server for tests is up.
- Pixel tests screenshot only the `#view` element and decode the PNG inside the page through a 2D canvas. This
  avoids an image-decoding dependency.
- Extra tests beyond the SAD's list:
  - loader errors on missing files and short binaries;
  - `pickSnapshot` clamping;
  - URL parameter round-trip;
  - canopy geometry clipping;
  - a DOM-driven overlay switch;
  - an end-to-end scrub over every 20th snapshot of the full run.

## Shot 6: sync to ecosim shot 5, scenario tests, visual regression

**Data**
- `public/fixtures/s42-mini` and `public/runs/s42` were refreshed with `bash scripts/sync-data.sh`. The shot-5 run passes ecosim's committed seed-42 manifest.
- **Tree `lifespan`** is an optional number on `TreeEntity`. Nothing draws it yet.
- **Unknown JSON keys** on entities, patches or meta are ignored, so extra keys never break the loader. A unit test adds made-up keys to every entity.
- **`series.csv` is read by header name.** Column order doesn't matter, and unknown columns such as `hunter_immigrants` are skipped. A unit test reverses the columns and adds an unknown one.

**Death causes**
- The loader looks for `<species>_<cause>` columns, with species `grazer` and `hunter` and causes `starved`, `eaten`, `old_age`, `crowded` and `burnt`. These are the sim's `Cause` enum, in its order.
- Each cause present is summed over both species into `series.deaths[cause]`. A run without the columns simply has no death data.
- The cause list is fixed rather than matched by pattern, because a pattern like `hunter_*` would also catch `hunter_immigrants`. A new sim cause needs one line here.
- Cause colours are fixed in `ui.ts` (`CAUSE_COLORS`). The sim owns species colours, but causes aren't species, and `meta.json` has no place for them.

**Chart: three panels**
- The chart canvas is now 300×390 (was 300×220) and stacks three panels over one tick axis:
  1. grazers and hunters, each normalized to its own max as before;
  2. trees;
  3. deaths per 100 ticks, stacked by cause bottom to top in `Cause` order, on a linear axis labelled with the tallest bin.
- Grazers and hunters share a panel; trees get their own. That's how "a third panel" is read.
- **The death axis is linear.** At seed 42 one bin at the grazer crash (1211 deaths) dwarfs the rest. It is left linear because the crash is the event worth seeing.
- **The marker** crosses all three panels.
- **Test hooks.** The chart canvas mirrors the marker tick and x, and the panel rectangles, into `data-marker-tick`, `data-marker-x` and `data-panels`, so scenario tests can check the chart without decoding text.

**Runtime changes**
- **Play steps every 150 ms** (was 250) and skips a beat while a snapshot is still loading. Before, a slow load could be cancelled by the next step and playback stalled. At 150 ms, "≥3 snapshots in 1 s" holds with room to spare.
- **An error keeps the last good frame.** A failed snapshot load leaves the canvas untouched, puts the slider, readout and URL back to the frame on screen, stops playback, and shows the error. `__ecoviewReady` stays false.
- **A later successful load clears the error state.** Before, the red status never cleared.

**Scenario tests** are in `tests/e2e/scenarios.spec.ts`, and the helpers shared with `view.spec.ts` are in `tests/e2e/helpers.ts`.
- **Format version 2.** The "never sets `__ecoviewReady`" test traps every write to the flag with a property setter installed before the page loads. The v2 fixture is the committed mini fixture with `meta.json` rewritten in flight by `page.route`, so there's no second copy of the fixture to keep in sync. This test replaces the POC's bad-format test.
- **Missing snapshot file.** The test 404s one `.bin` of snapshot 100 and requires the `#view` screenshot to be byte-identical to the one before.
- **Species at zero.** The test zeroes `hunters` from row 30 of the fixture series. It then requires hunter-red pixels on the panel's zero line in every column past the drop, none above it, and no `NaN` anywhere in the page HTML. A mutation check confirmed the test fails when the series doesn't reach 0.

**Visual regression**
- **References.** `shots/reference/*.png` are the 8 shots on the new data, with `shots/reference/PLATFORM` = `win32-x64 swiftshader`.
  - This shot is the one that creates them. After this, only a shot that says it changes rendering runs `shot:accept`.
  - The top-level `shots/*.png` stay committed. `REPORT.md` describes them.
- **The scripts.**
  - `npm run shot:check` builds, takes fresh shots and compares them with `scripts/shot-ref.mjs check`: pixelmatch at threshold 0.1, failing when more than 2% of an image's pixels differ. Diff images go to `shots/diff/`, which is gitignored.
  - `npm run shot:accept` builds, takes fresh shots and copies them over the references.
  - The shot list moved to `scripts/shots.mjs`, so `shot.mjs` and `shot-ref.mjs` share it.
- **Platform.** SwiftShader on Windows and on Linux CI won't agree within 2%: fonts, antialiasing and ANGLE backends differ. So `shot:check` against the committed references is the local gate.
  - CI runs `npm run shot` and uploads the PNGs.
  - CI runs `shot:check` only when the first line of `PLATFORM` matches the runner (`<platform>-<arch> swiftshader`). Otherwise it skips the check with a `::notice`.
  - `shot-ref.mjs check` itself prints a note on a mismatch but still compares, so a local run on another machine fails loudly rather than skipping.

**CI.** An `ecoview` job sits beside the unchanged `ecosim` job in `.github/workflows/ci.yml`, with an 8-minute timeout. Steps:
1. Build ecosim release and run seed 42 at 20000 ticks, snapshot every 100.
2. Copy the run into `public/` with `scripts/sync-data.sh`. `public/runs/` is gitignored, and `runs/s42` is deterministic across platforms (libm).
3. `npm ci` and `npx playwright install --with-deps chromium`.
4. `npm test`, which runs the build, the unit tests, and the page and scenario tests.
5. `npm run shot`, then the platform-gated `shot:check`.
6. Upload `shots/*.png` and any diff images.

The job sets its own `defaults.run.working-directory: ecoview`, which overrides the workflow-level `ecosim` default.
