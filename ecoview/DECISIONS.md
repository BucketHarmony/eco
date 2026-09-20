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

## Format version 2 (shot 8)
- **Accepted versions.** The loader accepts `format_version` 1 and 2 (`FORMAT_VERSIONS` in `loader.ts`) and rejects anything else. Version 2 differs only by `state.bin` in each snapshot and `forked_from` in `meta.json`. The loader never fetches `state.bin`, so a v2 run renders exactly like the same run at v1.
- **Sidebar.** When `forked_from` is non-null, the status line reads `<run> · seed N · forked from <parent run> @ tick T · K entities`. A v1 run, or a v2 run with `forked_from: null`, shows the line unchanged.
- **Tests.** The v2 fixture is the committed mini fixture with `meta.json` rewritten in flight, as before. The unit test serves it through a fetcher and the e2e test through `page.route`. The error-state scenario now uses version 3. The committed data under `public/` isn't regenerated, so the shot references are unchanged.

## Fire, crowding and traits (shot 12)

**Data.** `public/runs/s42` was refreshed with `bash scripts/sync-data.sh` to ecosim's shot-11 run (format 2, 35 series columns). `public/fixtures/s42-mini` is unchanged: ecosim still ships it, and the sim's newer `s42-mini-v2` isn't in the sync script, which is outside this component.

**Loader**
- `OPTIONAL_COLUMNS` lists the columns sim shots 9 and 11 appended: `patches_burning`, `total_burnt` and the 12 trait means and SDs. Each is read into `series.extra` by header name when present and left undefined when not, so older runs still load.
- `Patch.burning_ticks_left` and the animal traits `energy_cost_mult`, `flee_distance` and `repro_threshold` are optional fields. An animal without them is drawn as a default animal.

**Overlays.** `fire`, `crowding` and `traits` join the overlay list. All three are "field overlays" for canopies, so in the top camera canopies are hidden and the ground and animals under them show.
- **`fire`.** A burning soil column is orange by ticks left: `#b3300a` at 1 to `#ffb020` at 3 (`FIRE_TICKS_FULL`, the sim's `fire.duration` default), clamped. Rock and water keep their colours, because burn-out only touches soil columns in the sim.
  - **"Burnt" is inferred.** The sim writes no burnt flag. Burn-out leaves a patch bare, so a patch that isn't burning and has grass + shrub below 0.05 (`BURNT_COVER`) is drawn charcoal `#2b2b2b`. Across all 201 snapshots of the s42 run, only tick 17300 has such patches: the four burnt-out patches next to the fire. Grazing never takes a patch that low, and a scar regrows past 0.05 within one 100-tick snapshot interval, so most fires leave no charcoal in any snapshot.
  - Everything else is the material colour.
- **`crowding`.** Live grazers per patch, counted from `entities.json` once per snapshot. The colour goes from white at 0 to magenta `#d81b9c` at 32 (`CROWDING_FULL`), clamped. 32 is twice the sim's grazer disease threshold of 16, so that threshold is half-magenta. The scale is a fixed constant, not read from `meta.json` params, so the colour means the same thing across runs. Like `temperature`, it colours every non-water column of the patch.
- **`traits`.** The ground is the material colour. Grazers are coloured by `energy_cost_mult`: white at the default 1, lerping to blue `#1f5bff` below and red `#ff1f1f` above, fully saturated at ±0.25 (`TRAIT_SPAN`). The s42 grazers have a standard deviation of about 0.13.
  - **Hunters are drawn gray `#555555` on this overlay.** Species red would read as a high-cost grazer.
  - Per-instance colours need a white material, so trait grazers and gray hunters are two extra `Layer`s, shown only on this overlay.

**Chart: fourth panel.** The canvas is now 300×500 (was 300×390), and a fourth panel sits under the deaths.
- `patches_burning` is drawn as orange bars, each the **max** of its pixel's bucket of rows on a 0..max axis. Fires last about 3 ticks, and a bucket mean would erase them.
- The mean grazer `energy_cost_mult` is a blue line on its own min..max range, since it moves a few percent around 1. The legend shows the range (`grazer cost × 0.96–1.00`).
- Rows where the species is extinct are written as 0 by the sim. They are left out of the range and break the line.
- A run with neither column shows an empty panel labelled "no fire or trait data".

**Screenshots**
- `09_fire_t17300_top.png`: 17300 is the snapshot with the most patches burning in `runs/s42` (5). The series peak is 6 at tick 5605, but that isn't a snapshot tick.
- `10_crowding_t20000_top.png` and `11_traits_t20000_top.png` use the top camera, where overlay colours are exact.
- All 11 references were re-accepted with `shot:accept`. The data had moved to the shot-11 run, and the taller chart changes the sidebar in every shot.

**Tests**
- `tests/e2e/overlays.spec.ts` rewrites the mini fixture's tick-0 `patches.json` or `entities.json` in flight and checks exact colours in the top camera, over patches that are all soil with no trees. For fire, it sets burning 3, burning 1 and a bare patch. For crowding, it puts a herd of 40 and one of 16 into two patches. For traits, it sets a grazer below the default cost, one above, one with no traits, and a hunter.
- A mutation check confirmed the traits test fails when blue and red are swapped.
- The tests decode the screenshot with `pngjs`, typed by a small `tests/pngjs.d.ts` rather than a new `@types` dependency.
- Unit tests cover the new colour functions, `grazersPerPatch` and the optional series columns.

## Time-lapse export (shot 13)

**Command.** `npm run film -- --run runs/s42 --overlay material --cam iso --every 100 --fps 12 --out film/s42-material.mp4` builds, serves `dist/` on port 4175 (so it can run beside the test and shot servers), captures the frames and encodes them. `--every` is in ticks: a frame is every snapshot whose tick is a multiple of it, so the command above gives 201 frames (ticks 0–20000). `--limit N` keeps the first N frames and `--max-seconds S` fails the export if it runs longer. `film/` is gitignored.

**Frames**
- **One page load, then stepping.** The script loads the run once and moves between snapshots through `window.__ecoviewGoto(tick)`, a four-line hook in `main.ts` that calls the same `apply()` the slider uses. Reloading the page per frame would re-fetch and re-parse the 20,001-row series every time. Each frame waits for `__ecoviewReady`, as the screenshot script does.
- **The caption bar sits under the view, not on it.** The film page's viewport is 1280×832, and the script adds a fixed 960×32 dark bar at y 800 with `tick N   grazers G   hunters H   trees T`, read from `series.csv` by header name. A frame is the 960×832 clip at the origin. So "crop the caption bar" means taking the top 960×800, which is exactly `#view`, and no part of the world is covered. The bar is added by the script, not the app, so the app has no film mode.
- Frames are PNGs in `<out without .mp4>-frames/NNNNN.png`, with `frames.json` recording the options and the tick of every frame.
- **Encoding** is `ffmpeg -framerate FPS -c:v libx264 -pix_fmt yuv420p -crf 20 -preset medium`. H.264 in yuv420p plays everywhere, and 960 and 832 are both even. The PNG frames are the deterministic artefact. The MP4 is not claimed to be byte-identical across ffmpeg builds.
- **ffmpeg** has to be on PATH. On Linux and in CI it's `apt-get install ffmpeg`, and on Windows `winget install Gyan.FFmpeg`. A missing ffmpeg fails with that hint.

**Checks**
- **Determinism.** `tests/e2e/film.spec.ts` captures 20 frames of `runs/s42` (every 1000 ticks) twice, each in a fresh browser context, and requires the PNG bytes to match frame for frame. It also requires the 20 hashes to be distinct, so two identical blank sets can't pass. It runs in `npm test` and needs no ffmpeg.
- **`npm run film:check`** makes the s42 film with `--max-seconds 180`, then `scripts/film-check.mjs` checks it:
  - `ffprobe` counts one MP4 frame per PNG;
  - frame 100 is tick 10000, and with the caption cropped off it is compared with `02_material_t10000_iso.png` cropped to `#view`. The comparison uses pixelmatch at threshold 0.1 and allows at most 2% of pixels to differ, as `shot-ref.mjs` does.
- **Which reference.** It is `shots/reference/02` when `PLATFORM` matches this machine, otherwise the fresh `shots/02` from `npm run shot`. CI runs Linux against Windows references, so a hard reference check there would fail on platform drift alone (Shot 6). `PLATFORM` moved from `shot-ref.mjs` to `chromium.mjs` so both scripts share it.
- **Measured on this machine:** 201 frames in 31.5 s, and frame 100 vs reference 02 at 0.000%. A mutation check that compared against `03_light` instead failed at 41%.
- **CI.** The ecoview job installs ffmpeg, runs `npm run film:check` after the screenshots, and uploads `film/s42-material.mp4` as the `ecoview-film` artifact. The job timeout went from 8 to 12 minutes to fit the apt install and the film.

## Frame-rate and scrub performance (shot 28)

**The spec.** `tests/e2e/perf.spec.ts` runs in `npm test`, writes `perf/perf.json` (gitignored), and CI uploads the file as the `ecoview-perf` artifact.
- **Draw rate.** `runs/s42` at tick 10000, every overlay in the viewer (the prompt's seven plus `temperature`) with the `iso` and `top` cameras, so 16 pairs. The spec switches through the `#overlay` and `#cam` selects on a single page load, waits for `__ecoviewReady`, then calls `window.__ecoviewBench(60)`.
- **The bench hook** is in `main.ts`. For each frame it calls the same `render()` that on-demand drawing uses, then `gl.finish()`, then reads one pixel with `readPixels`. `finish()` alone isn't guaranteed to block until Chromium's GPU process has finished, and the readback is. The hook runs only when it is called. It redraws the unchanged scene, so it leaves the canvas as it was and never touches `__ecoviewReady`. There is still no render loop, and all 11 references pass `shot:check` at 0.000%.
- **Step rate.** 50 consecutive snapshots, ticks 10000–14900, material/iso. Each step is timed in the page from `__ecoviewGoto(tick)` until `__ecoviewReady` is true, polled with `setTimeout(0)`. Every step fetches and parses a new snapshot, because the 8-entry cache holds none of them, and that is what a viewer sees when they press Play.
- **First load** is timed in Node, from `page.goto` until `__ecoviewReady`. It includes fetching and parsing the 20,001-row `series.csv`.
- **Summary.** The median, the p95 (nearest rank) and fps = 1000 / median, rounded to 0.01.
- **World size.** `perf.json` records the world it measured, read from `meta.json` `dims`. It was `64x64x32` when this was written; since shot 16 it is `256x64x32`, and the draw gate scales with it (see "Renderer dims sync").

**Thresholds.** The test fails only if a pair's median draw is over 250 ms or the median step is over 1000 ms, which are the prompt's numbers. The first pair over the draw gate stops the measurement, so a gross regression fails on the gate within seconds instead of on the test timeout. `perf.json` is written either way. (The single test and its flat 300 s timeout were split in shot E2; see "E2 perf budget". The gate values are unchanged.)

**Baseline.** SwiftShader is a software GPU, so these numbers measure the viewer's own cost and catch regressions. They are not real-world fps on a graphics card.

| | Local (win32-x64, Chromium 153.0.8010.12, ANGLE Vulkan SwiftShader) | CI (ubuntu-latest, linux-x64, same Chromium and GL string), run 35442074573 |
| --- | --- | --- |
| Draw, iso (8 overlays), median / p95 | 45.9–47.0 / ≤49.1 ms (≈21.5 fps) | 52.3–56.8 / ≤73.3 ms (≈18.5 fps) |
| Draw, top, material, median / p95 | 45.3 / 48.0 ms | 46.9 / 58.5 ms |
| Draw, top, field overlays (7), median / p95 | 34.2–34.8 / ≤37.2 ms (≈29 fps) | 34.0–37.9 / ≤44.4 ms (≈28 fps) |
| Step, median / p95 | 66.5 / 126.2 ms (15.0 fps) | 94.5 / 184.0 ms (10.6 fps) |
| First load | 220 ms | 227 ms |

Top on a field overlay is cheaper because canopies are not drawn there (see "Canopies are hidden in the top camera on field overlays"). Both gates have plenty of headroom on the slower machine, CI: 4.4× for draw (250 against 56.75 ms, fertility/iso) and 10.6× for step (1000 against 94.45 ms).

**Mutation check.** A 300 ms busy-wait added at the top of `render()` turned the spec red on the gate: `draw material/iso`, 344.05 ms against ≤ 250. The sleep was reverted. Before the early stop was added, the same mutation failed only on the test timeout, after 5 minutes.

## Tiled multi-view time-lapse (shot 29)

**Command.** `npm run film -- --run runs/s42 --tiles material:iso,fire:top,crowding:top,traits:top --layout 2x2 --every 100 --fps 12 --out film/s42-tiled.mp4`. `--tiles` is a comma-separated list of `overlay:cam` pairs; the cam must be `iso`, `top` or `side`. The overlay isn't checked in the script: an unknown one fails in the page like it does in the viewer. `--layout CxR` defaults to the smallest near-square grid, `cols = ⌈√n⌉` and `rows = ⌈n / cols⌉`, so 3 tiles give 2x2 and 5 give 3x2. `--layout` and `--scale` without `--tiles` are errors, because they would silently do nothing. `npm run film:tiled:check` runs the command above with `--max-seconds 360` and then `film-check.mjs`.

**Capture: one page per tile, stepped in lockstep.** Each tile has its own page in one browser context, loaded with its overlay and camera. For every frame, all pages go to the tick through `__ecoviewGoto` in parallel, the script waits for every `__ecoviewReady`, and then it takes a `#view` screenshot of each page. The alternative, switching overlay and camera between captures on one page, needs two extra on-demand renders and a readiness wait per tile per frame, and it rebuilds the instance colours on every switch. That alternative was not timed. The separate pages share nothing, so each one is deterministic on its own, just like the single-view page. The 10-frame test confirms this. The parallel steps overlap each page's fetch and parse with the other pages' drawing.

**Compositing** is done in Node with pngjs (`captureTiledFrames` in `film-lib.mjs`). Tiles are placed row-major, and empty cells are the page background `#e8ecf0`. The frame is encoded as RGB PNG with the Paeth filter, which takes a third of the time of pngjs's adaptive default at the same size: 46 against 130 ms for a 1920×1632 frame.
- **Labels.** The script adds a `overlay · cam` label to each tile page, drawn in the top-left corner of `#view` (dark, 80% opaque, 24 px tall, `LABEL_H`), so it appears in the tile screenshot. Nothing is added to the app, which still has no film mode.
- **Caption bar.** There is one bar across the whole grid, drawn by page 0 at y 800, `cols × 960` px wide. Every tile page's viewport is `max(1280, cols × 960)` wide so the bar fits. The bar is a separate screenshot pasted under the grid. It has the same style and text as shot 13.

**Resolution.** A tile is 960×800 CSS px, so a frame is `cols·960·N × (rows·800 + 32)·N` at `--scale N`. The browser context's `deviceScaleFactor` is N.
- **Renderer pixel ratio.** For a sharper frame and not just a bigger one, `main.ts` now calls `renderer.setPixelRatio(window.devicePixelRatio)` instead of `1`. The viewer, the tests and `npm run shot` all run at device scale 1, so their frames are unchanged: all 11 references pass `shot:check` at 0.000%, and the single-view film is byte-identical (below).
- **Even sides.** Width and height are always even for a whole-number N. `h264Level` still refuses an odd side, because yuv420p needs even ones.
- **H.264 level.** `h264Level(w, h, fps)` picks the lowest level from Annex A tables A-1/A-3 that allows the frame size in macroblocks, the √(8·MaxFS) limit on each side, and the macroblock rate. It is passed to ffmpeg as `-level`. Examples: 960×832 at 12 fps is 3.1, 1920×1632 is 5.0, and 3840×3264 is 6.0. Anything beyond 6.2 fails before capture, with a message saying to use a smaller layout, scale or fps. The single-view encode now passes `-level 3.1` too, which only changes the MP4 header. The frames are the deterministic artefact.

**Single view is unchanged.** With no `--tiles`, the script calls shot 13's `captureFrames` exactly as before. The page now comes from a context with `deviceScaleFactor: 1` instead of `browser.newPage`, and a context-level `weberror` listener replaces the page's `pageerror` listener. All 201 frames of `--overlay material --cam iso --every 100` were captured with the shot-13 code (stashed) and with the new code, and they compared byte-identical with `cmp`.

**Checks**
- **Determinism.** `tests/e2e/film.spec.ts` captures 10 tiled frames (2x2, scale 1, ticks 0–9000) twice, each in a fresh context. It requires byte-identical PNGs, 10 distinct hashes, and a 1920×1632 frame. It runs in `npm test`.
- **Tile correctness.** `film-check.mjs` reads `tiles`, `layout` and `scale` from `frames.json`. For a tiled film, it compares the `material:iso` tile of frame 100 (tick 10000), minus its top 24 rows (the label band), with the same rows of `02_material_t10000_iso.png`. It uses the shot-13 threshold (pixelmatch 0.1, at most 2%) and the same rule for choosing between the reference and a fresh shot. It also requires the MP4's size to match the PNGs'. The check fails if the film has no `material:iso` tile, or if its scale isn't 1.
  - Local result: 0.000%.
  - **Mutation check.** Swapping the first two tiles in `frames.json`, so the check reads the fire tile, failed at 69.6%.

**Measured** locally (win32-x64, SwiftShader) and on CI:

| Film | Frames | Size | Level | Local | CI (ubuntu-latest, run 35443604889) |
| --- | --- | --- | --- | --- | --- |
| single view, material/iso (shot 13) | 201 | 960×832 | 3.1 | 25 s | 40.4 s |
| 2x2 tiled, scale 1 | 201 | 1920×1632 | 5.0 | 126 s | 182.3 s (budget 360 s) |
| 2x2 tiled, scale 2 | 201 | 3840×3264 | 6.0 | 305 s | not run |

At scale 2, the tiles are rendered at 2× in WebGL, not upscaled. The iso tile's voxel edges and agents are sharp at 3840×3264. CI makes only the scale-1 tiled film, as the shot allows.

**CI.** After the single-view film, `npm run film:tiled:check` runs and `film/s42-tiled.mp4` is uploaded as `ecoview-film-tiled`. The ecoview job timeout went from 12 to 18 minutes. Before this shot the job took 4.5 minutes, and the tiled film is allowed 6. In run 35443604889 the job took 6 min 56 s: the tiled determinism test took 20.4 s, the tiled film step 3 min 7 s, and the tiled check 0.000% against the fresh shots/02 (the references are Windows).

## Renderer dims sync (shot 16)

This shot supersedes the earlier sections wherever they assume the 64×64×32 world, the `BURNT_COVER` inference or the 17300 fire shot.

**Data**
- `public/runs/s42` is ecosim's reference world at defaults: the 256×64×32 strip with 8×8-column patches, format 3. It was regenerated and copied with `bash scripts/sync-data.sh`. CI no longer pins the run to the old square world: the ecoview job runs `ecosim run --seed 42 --ticks 20000 --out runs/s42 --snapshot-every 100` at defaults.
- **`public/fixtures/s42-strip-mini`** is a 2-snapshot strip fixture (ticks 0 and 100, format 3, about 2.8 MB). ecosim ships no strip fixture, this shot can't edit ecosim, and `sync-data.sh` is shared, so the fixture was made with the ecosim CLI directly: `ecosim run --seed 42 --ticks 100 --out <dir> --snapshot-every 100` at defaults. `public/fixtures/s42-mini` stays the 64×64 fixture (format 2). The unit tests run over both.

**Loader**
- All sizes come from `meta.json` `dims` through a `Grid` (`x`, `y`, `z`, `patch`, with the derived `px`, `py`, `voxels`, `columns`, `patches` and `longest`). The module-level `DIM_*` constants are gone.
- A missing `patch` defaults to 8, the value in every run before shot 15.
- `dims` is rejected unless each side is an integer in 1..256 and `x` and `y` are multiples of `patch`.
- Every `.bin` is checked against the grid. A run whose dims disagree with its files fails on the first mismatched file with `expected N bytes, got M`.
- Formats 1, 2 and 3 are accepted. A v3 run must have `events.csv`, which is read by header name, so column order doesn't matter and a missing column is an error. v1 and v2 runs never fetch it.

**Burnt ground comes from events.** The fire overlay draws a non-burning soil column charcoal when its patch has a `burnout` event in (previous snapshot tick, this snapshot tick]. For the first snapshot, the window starts at −1. The old inference, bare cover below 0.05, is removed: on the strip it marked grazed-down ground as burnt and missed most scars. Burning takes precedence over burnt. Runs without events (v1 and v2) show no burnt ground, since claiming a burn from cover alone was the error being fixed.

**Cameras.** All three are computed from the grid, and the target is the world centre.
- **iso** keeps the square world's offsets (70, 75, 70) and scales them, the near plane and the far plane by `longest / 64`. The 64×64 world is framed exactly as before, and the strip is a diagonal band.
- **top** fits the whole width × depth into the 960×800 view (orthographic). The 64×64 world is unchanged at 12.5 px per column. The strip is 3.75 px per column and letterboxed to rows 280–519, with the page background above and below.
- **side** now looks north from beyond the south edge, so +x runs left to right. It is tilted down 25°, and its distance makes the x extent fill 90% of the view width. The old side camera was fixed for 64 and showed the strip as a thin sliver. This changes the side view of the 64 world too; no reference uses it.

**Screenshots**
- `09_fire_t17300_top.png` is now `09_fire_t17100_top.png`. 17100 is the strip snapshot with the most patches burning (3), with 27 burnouts since 17000.
- `03_light` and the letterbox test measure world pixels only, skipping exact page-background pixels (`viewStats(page, true)`). Before this, the background counted as "light", which the letterbox would have inflated.
- All 11 references were re-accepted. `shots/REACCEPT-16.md` shows the old and new images side by side with the diffs.

**Tests**
- The unit tests run the loader suite over both fixtures. They also cover `events.csv` parsing (the strip fixture's rows, and fire rows with empty fields in a shuffled column order), the `burntPatches` window, and a hand-built 24×8×4 world that exercises every fire-overlay case.
- e2e:
  - format 4 errors, and a v3 run with no `events.csv` errors;
  - claiming the mini fixture is 256×64 errors on a `.bin` size;
  - the strip fixture letterboxes to exactly 240 rows in the top camera;
  - the fire overlay test serves an `events.csv` with one burnout, and checks that the bare patch next to it stays soil.
- `perf.spec.ts` reads the world label for `perf.json` from `meta.json`.

**Performance on the strip.** The strip has 16384 surface columns against the 64×64 world's 4096, and every draw cost about 4× more: iso medians went from 45.9–47.0 ms to 239.8–241.9, top field overlays from 34.2–34.8 to 164.6–165.3, and a step from 66.5 to 356.8 ms. Shot 28's draw gate of 250 ms was set on the 64 world and would fail on CI here for the world's size alone, not for a regression. **The draw gate is now per surface column:** 250 ms / 4096 columns, which is the same gate on the 64 world and 1000 ms on the strip. Local headroom is 4.1× (241.9 against 1000), close to shot 28's 5.3×. The step gate stays at an absolute 1000 ms: a step fetches and parses one snapshot, and 356.8 ms still leaves 2.8×. The cost of this is sensitivity: shot 28's 300 ms busy-wait mutation would no longer trip the draw gate on the strip. A mutation that scales with the frame, such as drawing every view twice, still does.

**Film budgets.** `film:check` went from 31.5 s to 77.5 s and `film:tiled:check` from about 130 s to 315.8 s, again with the bigger world. `--max-seconds` went from 180 to 300 and from 360 to 700, so both keep roughly the 2× headroom they had, and the ecoview CI job's timeout went from 18 to 30 minutes. The films themselves are unchanged: both still match `shots/reference/02` at 0.000%.

## The Capitol in the renderer (shot G7)

**Order.** G7 runs before G4–G6. G3a finished the Capitol reference run, so the bundle world had no viewer at all; G4–G6 are further sim work on top of a world nobody could look at. Putting the renderer next means every later garden shot can be checked by eye against a reference screenshot instead of only against `series.csv`. The backlog order is otherwise unchanged.

**The `world` object is validated, not trusted.** `loader.ts` accepts `format_version` 1–4 and, at 4, requires a `world` object in `meta.json` before it fetches anything. `ground_width` and `ground_depth` must equal `dims.x / ground_cell_m` and `dims.y / ground_cell_m` exactly, and each of the four files must be exactly the size those dimensions imply (`ground_h.bin` and `building_h.bin` at 4 bytes a cell, `medium.bin` at 1). Every `medium` code must index `world.media`. A v3 run takes the same path it always did and never fetches `world/`. The f32 files are read through a `DataView` a value at a time rather than by wrapping the buffer in a `Float32Array`, so a big-endian CPU would read them the same way, and so an unaligned `byteOffset` cannot throw.

**The ground grid is a texture, not more voxels.** The Capitol's ground grid is 512×512 over a 256×256 ecology grid: drawing it as voxels would be 4× the instances for a flat surface. Instead `GroundDrape` puts one quad per ecology column at that column's surface height, `DRAPE_LIFT` = 0.02 m above the voxel's top face with `polygonOffset` −1, and samples a 512×512 `DataTexture` of the media (`NearestFilter`, no mipmaps, sRGB) whose UVs span the whole world. That is 65536 quads and one texture, and the medium boundaries stay crisp at half-metre resolution, which the e2e test checks by reading two adjacent ground cells inside one ecology column and finding two different palette colours. The drape is only visible on the `medium` overlay and is rebuilt when the tick changes, because the surface height can change under it.

**The medium palette lives in the renderer.** Species colours come from `meta.json` because the sim owns species. Media are a property of the bundle format, not of a run, so their nine colours are a constant in `world.ts` and the legend is built from `meta.json`'s `world.media` list against it, with a magenta `unknown` for a name the palette doesn't have. That keeps a bundle from having to ship colours and makes a new medium visible rather than silently invisible.

**The legend is hidden on every overlay but `medium`.** It is the only overlay whose colours are nominal rather than a ramp. Keeping it hidden elsewhere is also why shots 01–11 are byte-identical to their references (0.000%) after this shot: adding an `<option>` to the overlay `<select>` and a hidden `<div>` changes no pixel. Only the four new references were accepted (`shot-ref.mjs accept 12_ 13_ 14_ 15_`), so no re-accept file is needed.

**Buildings are extruded from `building_h.bin`, once per run.** Each roof cell gets a top quad at its building height and a side quad on each of the four edges where the neighbour's top is lower, which is 71743 quads at the Capitol against the 123525 a box per cell would take. The base of a cell's sides is the top of the ecology column it falls in (`height[col] + 1`), so a building meets the terrain it stands on rather than floating or sinking on a slope. The geometry is non-indexed with `computeVertexNormals()`, so it shades flat and the dome's steps read as steps. It is built on the first frame of a run and never rebuilt: `building_h.bin` is written once at the run root and cannot change between snapshots.

**Pipes are top-camera only.** Four dashed `LineSegments` inlet→outlet at `y = dims.z`, with `depthTest: false` and `renderOrder` 10 so they draw over the surface. On a perspective camera a line above the world reads as floating and lands nowhere near the ground it describes; straight down it lands exactly on it. They are illustrative in the bundle, and the viewer says so by only drawing them on the plan view.

**Overlays on a bundle world.** All eight existing overlays work unchanged, because they read patches and voxel fields, which a bundle run writes exactly as a noise run does. `medium` falls back to the material colour on a run with no ground grid, so switching to it on `runs/s42` is not a blank screen. `light` is the one worth looking at: shot 14 shows solid black immediately north of the dome and the wings, which is the buildings' shade in the sim's own light field, and `capitol.spec.ts` asserts that strip is under 160 mean brightness where the open lawn is over 200.

**Shot budget.** `scripts/shot.mjs` now fails when the set runs over `BUDGET_S` = 60 s rather than only printing the time, and prints a per-shot time. The 15 shots take 10.0 s, of which the four Capitol shots are 4.5 s — the Capitol's 65536-column world is 4× the strip's, but each shot is still about 1.1 s. No per-world budget split was needed.

**`perf.spec.ts` was left on `runs/s42`.** It measures draw cost per overlay and camera against a per-surface-column gate, and adding `medium` or the Capitol to it would either measure a different world against the strip's gate or double the suite's slowest test (it already runs 3.9 minutes). The Capitol's cost is covered by the shot budget above, which is a real gate.

**Data.** `scripts/sync-data.sh` copies `runs/capitol-s42` and the committed `fixtures/capitol-mini` alongside the strip's run and fixture. The e2e tests use the fixture, which is in git, so they pass on a clean checkout; only the four reference shots need the full run.

**On "remove the CI generator pins that G1–G3 added".** There were none on the ecoview side to remove. G1–G3 added one step to the **ecosim** job (`10 capitol run`) and its pin in `ecosim/tests/ci.rs`; they touched neither `sync-data.sh` nor the ecoview job, and an ecoview shot does not edit `ecosim/` (component isolation). What the ecoview job did need was the opposite: it now generates `runs/capitol-s42` with `ecosim run --world worlds/capitol --seed 42 --ticks 20000 --out runs/capitol-s42 --snapshot-every 1000 --set animals.enabled=false --set climate.rain_gradient=0` before `sync-data.sh`, since `runs/` is gitignored. `shot:check` is still skipped on CI, because the references are rendered on this machine.

## The voxel editor (shot E1)

**The non-goal is overridden, for this shot and the E series only.** SAD 2's non-goals call the renderer "Not an editor". The operator superseded that line on 2026-09-19 for the E series (overnight/shots/E1-voxel-editor.md): the garden direction needs a way to correct what the LiDAR importer gets wrong without going back to Blender, and the viewer already holds the world in memory. Nothing else in the non-goals is relaxed — no live sim connection, no mobile layout, and no camera state kept anywhere but the URL.

**What is editable, exactly.** Three arrays of `WorldData`, one value per **ground** cell: `ground_h` (f32, metres above the crop minimum), `medium` (u8 index into `bundle.json`'s `media`) and `building_h` (f32, roof height above ground, 0 where there is no building). `trees.json`, `shrubs.json`, `pipes.json` and every field of `bundle.json` are loaded, drawn and written back byte for byte. The horizontal cell size is `meta.ground_cell_m` read from the bundle, never the literal 0.5: a bundle may use any whole ratio to the 1 m ecology grid.

**One click is 0.5 m of bundle, which is not always 0.5 m of sim.** `ground_h` is continuous, but `World::from_bundle` averages it over each 1 m ecology column and rounds to a whole metre before storing the `u8` voxel layer. So one click can change the bundle and the picture without changing the sim at all, and two clicks move a column by one voxel. The step stays at 0.5 m — it is what the bundle stores and the right feel for the renderer — and the editor says so in the sidebar in one line rather than quantising itself to 1 m or changing the sim's rounding.

**The operation list is the world's only writer.** Every edit is one `Op` — `{kind, cells: [{i, before, after}]}` over ground-grid indexes — and `applyOp(world, op, 'before'|'after')` is the only function that assigns into the three arrays. Undo applies `before`, redo applies `after`, `changes-N.json` is the list itself, and the tests drive all three through that one path. `makeOp` rounds `after.ground_h` and `after.building_h` through `Math.fround` before storing them, because the grids are f32: without it a value that crosses a binade (1.03 + 0.5) would record an op whose `after` never matches what the grid holds, and undo would be inexact.

**Picking marches a heightfield; it does not raycast the boxes.** `pickCell` steps the crosshair ray in quarter-cell increments and returns the first cell whose ground-plus-building top is above the sample, with a flag for whether the ray came down onto a top face or into a vertical one. Raycasting instead would be 262144 box intersections per mouse move at the Capitol. The side-face case is what makes a wall extend sideways: a `building` place on a vertical hit grows the neighbour the ray came *from*, not the cell it hit. The outline is always drawn on the picked cell's top face, so a side-face pick puts the outline above and behind the crosshair; both new reference shots pick top faces deliberately.

**Chunked ground, and frustum culling with it.** `GroundChunks` draws the ground as one `InstancedMesh` of ground cells and one of building cells per 32×32-cell chunk — 256 chunk pairs at the Capitol — and an op rebuilds only the chunks its cells touch, which is at most 4 for a radius-9 disc. Each rebuilt mesh gets `computeBoundingSphere()` and keeps three's default culling, so a first-person view pays for the chunks it can see rather than all 256: the `?world=` page went from 9.7 s to about 4.0 s to first draw under swiftshader with that one change. A 200-operation scripted stroke on the Capitol bundle applies in well under the 2 s the shot allows (`edit.spec.ts` prints the measured number; it is 29.8 ms on this machine).

**URL parameters, and why `?eye=` exists.** A bundle page is `?world=<path>&cam=iso|top|side&edit=0|1&slot=<hotbar key>&brush=1..9`, with two optional pins: `aim=gx,gy` fixes the picked cell, and `eye=x,y,z,yaw,pitch` stands the first-person camera at a point in metres east, north and up, yaw in degrees with 0 looking north and positive turning west. Without `?eye=` the two new reference screenshots would need pointer lock and a mouse, which a headless screenshot cannot do deterministically; with it they are ordinary URL loads. Both are read at load and written back unchanged, so the URL is still the whole of the page's state.

**Saving writes eight files with a prefix, not a zip.** `Ctrl-S` downloads the seven bundle files under `<name>-edit-N-<file>` plus `changes-N.json`, N counting saves within the session. A browser download cannot make a directory, and writing the loaded names would overwrite the bundle that is open; dropping the prefix from the seven files gives a directory `ecosim run --world` reads. The four files the editor cannot change are written from the bytes that were fetched, so a save with no edits is byte-identical to the input including `bundle.json`'s key order — an e2e test asserts that file by file. `media` is never sorted, deduped or re-indexed: ecosim rejects a bundle whose `media[0]` is not `soil`, and the editor writes medium *codes*, so any reordering would silently repaint the world.

**Hand run.** A building lowered to the ground and a walk painted over lawn, saved, un-prefixed into a directory and run:

```
$ ecosim run --seed 42 --world "$TEMP/capitol-edit" --ticks 1000 --out "$TEMP/capitol-edit-run"     --set animals.enabled=false --set climate.rain_gradient=0
capitol-edit (1000 ticks, 787 ms): grazers=0 hunters=0 trees=133
```

The run completes, and diffing its `world/` against the same run on the unedited bundle shows exactly the 13 building cells and the 13 medium cells the two strokes touched, and nothing else.

**Line budget.** This shot does not fit the 1,500-line budget: it is 2196 net lines over `ecoview/` excluding fixtures (735 of them `edit.ts`, 766 the two new test files, and 695 the changes to the existing five modules, the page, the shot list and the three write-ups). The limiter in the prompt — drop the first-person camera and pointer lock — was measured at about 200 lines, which would not have closed the gap, so it was not applied and the shot is Blocked on the budget with the work complete and green. `overnight/shots/E1.BLOCKED.md` has the breakdown.

## E2 perf budget

**What failed, and what did not.** `perf.spec.ts` went red in CI on shot G4b's regenerated runs, and
it went red on wall clock: one Playwright test carried the whole 8-overlay × 2-camera × 60-frame draw
matrix plus the 50-tick step loop under a flat `test.setTimeout(300_000)`. G4b's units calibration
raised the tree count 38% on the strip, the per-draw median went from about 203 ms to about 310 ms on
the runner, and the matrix alone ate the 300 s. **No gate was breached.** The draw gate is 1000 ms per
pair on this world and the step gate is 1000 ms, so the test died with three times the headroom it
asserts. `overnight/shots/G4b.BLOCKED.md` has the two CI samples.

**The gate values do not move.** `MAX_DRAW_MS_PER_COLUMN` (250 ms over the 64×64 world's 4096 columns,
so 1000 ms on the 256×64 strip) and `MAX_STEP_MEDIAN_MS` are untouched, and so is `FRAMES = 60`. There
is no regression in them to hide, and widening a gate to make CI green is what the standing gate rule
forbids. The fix is to stop charging one Playwright test for the whole matrix.

**The split is per camera, plus the step loop on its own** — three tests where there was one:
`perf: draw rate per overlay, cam=iso`, `… cam=top`, and `perf: step rate through consecutive
snapshots`. Per camera rather than per overlay group because the cameras are what differ in cost
(`iso` draws the surface at 279 ms a frame here, `top` at 168 ms for the field overlays), so each test
measures one cost regime and its budget means something. The old assertion that all 16 pairs were
measured becomes two assertions of 8, one per test; every other assertion and every number in
`perf/perf.json` is unchanged.

**Each test derives its own timeout from its work**, the way the draw gate already derives itself from
the world size:

```
timeout = SETUP_MS + units × budget_per_unit × RUNNER_SLACK
```

A draw test's unit is one rendered frame and its budget is `maxDrawMs`, the gate the test already
asserts, so 8 × 60 × 1000 ms; the step test's unit is one snapshot load at `MAX_STEP_MEDIAN_MS`, so
50 × 1000 ms. Deriving the budget from the gate keeps the property the old comment claimed — a run
slow enough to breach a gate still reaches its assertion and fails on the gate, never on the clock.
`SETUP_MS` is 30 s for the page load and the per-pair select switches, which are not measured frames.
`RUNNER_SLACK` is 1.25: on identical scene weight the CI runner measured 10–20% slower than this
machine (CI draw medians 155.8–246.2 ms against 160.2–214.8 ms locally, runs 35493848837 and the
local `perf.json` of the same day), and the alarm below should fire on scene weight, not on which
machine ran it. The budgets come out at **630 s** for each draw test and **92.5 s** for the step test.

**Measured cost against those budgets**, on `runs/s42` at G4b's weight (1541 trees), win32-x64 with
SwiftShader:

| test | measured | budget | margin | half-budget alarm |
| --- | --- | --- | --- | --- |
| `draw/iso` | 140.7 s | 630 s | 4.5× | 315 s |
| `draw/top` | 92.8 s | 630 s | 6.8× | 315 s |
| `step` | 20.1 s | 92.5 s | 4.6× | 46.2 s |

The thinnest margin is 4.5×, against the 3× the shot asks for.

**And on the runner, where it actually failed.** CI run 35528053254 (all 7 jobs green, the `ecoview` job
19m52s of its 30-minute budget) reports 16 draw medians of 201.8–383.6 ms, mean 296.4 — so the measured
frames alone cost **178.3 s for `cam=iso` and 106.2 s for `cam=top`**, 284 s of the old single test's flat
300 s before any page load or overlay switch. That is the timeout, measured. Against the new budgets the
runner sits at 3.5× on the heavier test and 1.8× clear of its half-budget alarm, which is what
`RUNNER_SLACK` buys: without it the alarm would be 255 s and a runner this close to 185 s would start
failing on variance rather than on scene weight. The worst single pair, `traits/iso` at 383.6 ms, still
has 2.6× headroom on the 1000 ms gate.

**A test that uses more than half its own budget fails by name.** `expectWithinHalfBudget` compares
each test's elapsed wall clock with half its timeout and fails with the test's name, both numbers and
what to do. The next scene-weight increase therefore reports its cause instead of the bare
`Test timeout of 300000ms exceeded` that G4b got. At today's weight `draw/iso` would have to slow by
2.2× to trip it, which is more than shot E3's roughly 47% extra geometry and more than a heavier run
is likely to add in one step.

**`perf/perf.json` keeps its exact shape**, so its numbers stay comparable across shots: same keys,
same order, `draw` still holding all 16 `overlay/cam` entries with `iso` first. Each test writes its
own measurements to `perf/parts/<name>.json` and a `test.afterAll` hook merges them. The merge is in
`afterAll` and not in a fourth test because a hook runs even when a test fails, and CI's
`upload-artifact` step for `ecoview-perf` is `if-no-files-found: error` — a gate failure must still
leave the file behind, as it did before the split. `beforeAll` removes stale shards so a previous
run's numbers cannot leak into this one.

**The step loop no longer skips when the draw matrix is slow.** In the single test, `slow()` also
short-circuited the step measurement; as its own test it has its own gate and its own 92.5 s budget,
so a slow draw matrix no longer hides the step number. Within a draw test the early break is
unchanged: the first pair over the gate ends the measurement there.

**The references were re-accepted, and not because of this shot.** `npm run shot:check` is one of shot
E2's acceptance commands and it exited 1 when the shot started: ecosim shot G4b is on the branch, the
ecoview CI job regenerates `runs/s42` and `runs/capitol-s42` from whatever ecosim is there, and the
calibration moved 15 of the 17 reference screenshots. Nothing went red in CI, because `shot:check` is
skipped on the Linux runner (the references are Windows), so the drift sat unrecorded. E2 itself is
pixel-inert: with `perf.spec.ts` and this file reverted to `0190df9`, `npm run shot` produced all 17 PNGs
byte-identical to the accepted ones. `shots/REACCEPT-E2.md` has every row with its old and new image, its
diff split between `#view` and the sidebar, and why the change is correct, as MASTER's re-accept rule
requires. Two findings for the operator are recorded there: fire came back under G4b (37 ignitions in
20000 ticks against shot E1's 8), so shot 09's tick 17100 is no longer the run's best fire tick, and the
Capitol run's `fertility_mean` ends at 222.85, over the 220 ceiling the operator flagged before G4.

## E3 block world

**What this shot changes, and what it deliberately does not.** The data model is untouched: a ground cell
is still one `ground_h` f32, one `medium` u8 and one `building_h` f32, the bundle format is unchanged, and
`ecosim/` is not touched. What changes is the geometry rule in `GroundChunks.rebuild` and the meaning of a
hotbar click. A heightfield drawn as cubes still has no overhangs, bridges or caves, and nothing here
pretends otherwise — one material per column remains the hard limit, and no change in this shot needed a
second one.

**The culling rule: draw every cube that has an exposed face, and nothing else.** A column of top level
`L` draws levels `n+1 … L`, where `n` is the lowest of its four neighbours' levels (`cubeRange` in
`world.ts`). That is exactly the set of cubes with at least one face against air: the face of column A at
level `k` toward neighbour B is exposed iff `k > level(B)`, which implies `k > n`, so every exposed face
belongs to a drawn cube, and a cube at or below `n` is buried on all four sides with a drawn cube above
it. Off the grid the neighbour counts as level −1, so the world's rim is a wall that reaches the floor
rather than a lip one cube short of it. Buildings use the same rule on `ground_h + building_h` and stop at
their own ground level (`b0 = max(b0, g1)`), so the two meshes never overlap and the ground mesh owns
everything below a building.

**The measured cost, on the committed `fixtures/capitol-world`** (512×512 cells at 0.5 m):

| | cubes |
|---|---|
| ground tops | 262,144 |
| ground side cubes | 16,142 |
| building tops | 24,705 |
| building side cubes | 63,086 |
| **total drawn** | **366,077** |
| E1 drew (one prism per column) | 286,849 |
| full stacks, for comparison | 3,833,617 |

`tests/unit/cubes.test.ts` asserts 278,286 ground and 87,791 building instances against the fixture by
building the real `GroundChunks`, so a change that breaks the culling shows up as a count. The increase
over E1 is **27.6%**, not the 47% the prompt budgeted, which is why `perf.spec.ts` needed nothing from
this shot: the run pages do not use `GroundChunks` at all (they draw through `GroundDrape` and
`Buildings`), and the draw medians are unchanged at 280 ms iso / 168 ms top.

**Deviation from the prompt's count table, on purpose.** `overnight/shots/E3-cubes.md` gives 50,192 ground
and 83,486 building "exposed sides" for a total of 420,527, and asks for that number in a test. Those are
**faces**, not cubes: counting exposed side faces the same way here gives 45,564 ground and 82,948
building, total 415,361, within a few percent of the operator's figures (the rest is the floor and rim
convention). A cube on a two-cube step carries two or three exposed faces and is still one instance, so
the instance count is necessarily lower than the face count — 366,077 against 415,361 here. Drawing one
cube per exposed face would place several cubes in the same lattice cell. The test therefore asserts the
measured instance counts exactly, as the prompt asks, but against the rule that draws each cube once; the
LOG line for this shot says the same.

**Quantise for display; the data stays continuous.** `levelOf(h, cell) = floor(h / cell + 1e-6)` is applied
at draw time only. `ground_h` and `building_h` remain f32 metres from the LiDAR import, so a bundle that is
loaded and saved without an edit is still byte-identical (the e2e test that asserts that is unchanged and
still passes), the sim still averages continuous heights over each 1 m ecology column, and a future
importer with finer data loses nothing. Quantising the stored values instead would throw away up to half a
cube of height per cell on load and make every load-save a lossy operation. The `1e-6` is there so a height
that a place operation just wrote *onto* the lattice — `(L + 1) * cell`, rounded through `Math.fround` —
reads back as level `L + 1` and not `L`.

**The cube edge is `ground_cell_m`.** Vertical step equals horizontal step or the cubes are not cubes, so
every quantisation reads the cell size from the bundle. E1's fixed 0.5 m edit step is gone; `ui.stepNote`
derives its sidebar sentence from the same number (2 clicks to a sim voxel at 0.5 m, 4 at 0.25 m), and
`edit.test.ts` covers a synthetic 0.25 m bundle so a hard-coded 0.5 fails the unit suite.

**Place and remove are mirrors, and a dig exposes soil.** A place takes the column to
`(level(top) + 1) * cell` and paints the slot's medium; the `building` slot does the same to
`building_h`. A remove takes the top building cube if there is one, else the top ground cube, and stops at
the floor. For the two to be exact inverses a remove has to say what the newly exposed ground is made of,
and the only answer a heightfield can give is the sub-surface: `SUBSURFACE = 0`, which the scene contract
fixes as `soil`. So a place on a dug column restores it byte for byte, which is what the acceptance's
round trip asserts — `edit.spec.ts` digs once to put the column on the lattice, then places and removes a
cube with each of the eight slots and compares the whole grid each time, and finally undoes everything and
saves a bundle byte-identical to the fixture. On an unedited LiDAR column the first place also snaps the
height onto the lattice, which is visible and intended; after that every click is one cube.

**Placing against a face.** A hit on a vertical face now adds to the neighbour the ray came from for
*every* slot, not only `building` (E1's `Target.adj`, change 5), so a ground place builds a terrace
outward from a wall instead of pushing the wall up. The outline follows: twelve edges around the cube a
click would add or remove, per brushed cell, lifted `OUTLINE_LIFT` so it does not z-fight with the cube's
own faces.

**Pick block, and the brush default.** Middle-click reads the cube under the crosshair and sets the hotbar
to it (`slotAt`: the building slot if the top cube is a building's, else the column's medium if a slot
names it, else `ground` — which is the slot that leaves the medium alone). The brush already defaulted to
1 from E1, so change 7 needed no code. Nothing from the limiter list was dropped.

**Hand run.** A 49-cell gravel pad placed on the lawn with brush 5, then a three-cube pillar on it and one
cube taken back off, saved with Ctrl-S and un-prefixed into a directory:

```
$ ecosim run --seed 42 --world "$TEMP/capitol-e3-edit" --ticks 1000 --out "$TEMP/capitol-e3-run" \
    --set animals.enabled=false --set climate.rain_gradient=0
scene: 81 trees -> 79 planted (0 moved, 2 dropped, 0 merged); 64 shrubs over 750 columns in 87 patches
wrote C:\Users\kenne\AppData\Local\Temp/capitol-e3-run (1000 ticks, 701 ms): grazers=0 hunters=0 trees=135
```

The edit reaches the sim exactly: diffing this run's `world/` against the same run on the unedited bundle
gives 49 cells in `ground_h.bin`, the same 49 bytes in `medium.bin`, 1 cell in `building_h.bin` and an
identical `pipes.json`. The pad's cells read 5.50 m where the lawn was 5.27 m, which is the snap to the
lattice: one click of gravel is a pad at cube height, not a paint job at LiDAR height.

## E4 run from the editor

**What it is.** `npm run preview:sim` starts the preview server and a second process, the *sim helper*
(`scripts/sim-server.mjs`), on 127.0.0.1:4174. `R` on a bundle page posts the seven bundle files as the
editor holds them, the helper writes them to a temporary directory and runs the `ecosim` release binary on
them, and when the run finishes the page loads it through the ordinary loader and draws it with the same
world, entities and overlays every other run gets. `B` goes back to the bundle, edits and all, without a
reload; `C` cancels a run in flight. It is dev tooling: the built site never needs it, `npm run shot` never
starts it, and with no helper listening the page says so and keeps editing.

**This is not IPC, and it is not shared code.** The helper spawns the same command line a hand run uses and
reads nothing out of `ecosim/` but the binary and `params.toml`, both overridable with `ECOSIM_BIN` and
`ECOSIM_PARAMS`. The two projects still share exactly one thing, the run directory on disk; the helper only
saves the human the round trip of downloading eight files, un-prefixing them and typing the command. Every
run it starts is a garden run (`--set animals.enabled=false --set climate.rain_gradient=0`), the convention
for bundle worlds, with `--snapshot-state false` because nothing in the page reads `state.bin`.

**The endpoint contract** (`scripts/sim-lib.mjs` holds the half that is pure, and `tests/unit/sim.test.ts`
tests it):

| route | takes | gives |
| --- | --- | --- |
| `GET /sim/health` | – | `{ok, binary, params, port, root}`; `binary` is null when it is not built |
| `POST /sim/run` | `{ticks, seed, files}`, `files` base64 by name | `{ok, id, ticks, every, path}` |
| `GET /sim/status?id=` | a run id it issued | `{ok, state, tick, ticks, path, error}` |
| `POST /sim/cancel?id=` | a run id it issued | `{ok, id, state}` |
| `GET /sim/runs/<id>/<rest>` | a run id and a file in it | the file, or 400 outside it, or 404 |

It takes no path from a request that it did not build itself. `files` must be exactly the seven names in
the scene contract or the request is refused, `ticks` must be a whole number in 1..20000, and the run id is
a key in a table the helper filled, never a path segment joined to anything. `<rest>` goes through
`safeJoin`, which refuses `..`, a drive letter, a NUL and a backslash in any encoding and then checks the
resolved path is still under the run. The child's working directory is the run's own. The helper binds
127.0.0.1 only: it starts a program on this machine, so it must not be reachable from the network.

**Why the page talks to its own origin.** `vite.config.ts` proxies `/sim` to the helper in both `server`
and `preview`, so `loader.ts`, `parseParams` and the run-base semantics are untouched — a finished run at
`/sim/runs/<id>/` is a run directory like any other. The proxy's error handler answers `200 {ok: false}`
rather than letting vite return 502, because a 502 is an error in the browser console and change 7 asks for
a missing helper to be a sentence in the sidebar, not a console error. A built site with no proxy answers
404, which the page reads the same way. The sidebar distinguishes the two missing pieces the shot names:
no helper behind `/sim` says `npm run sim`, a helper with no binary says `cargo build --release`.

**Temporary files.** Everything the helper writes lives under one `mkdtemp` directory, removed on SIGINT,
SIGTERM and SIGHUP; a 1000-tick Capitol run is about 60 MB, so it also keeps only the last `KEEP_RUNS` = 3
finished runs and deletes the rest as new ones start. Nothing is written inside the repository, and a run
never outlives the helper that made it. `/sim/health` reports the directory so a test that kills the helper
can sweep up after it, which is what `tests/e2e/sim.spec.ts` does; nothing in the page reads it.

**Ticks and seed are URL parameters** (`&ticks=`, `&seed=`, defaulting to 1000 and 42, clamped on parse),
so a screenshot or a scripted page can choose the run length without clicking, the same rule the rest of
ecoview follows. The snapshot cadence is derived, not configured: about ten snapshots whatever the run
length, which keeps the slider useful at 200 ticks and at 20000.

## E5 reference screenshots in CI

**The hole.** `npm run shot:check` compares the fresh screenshots with `shots/reference/`, and the CI
step ran it only when `shots/reference/PLATFORM` matched the runner. The references are rendered on
Windows and CI runs on Linux, so the step has never done anything but print a notice. Shot E2 found
fifteen of the seventeen references out of date, drifted by ecosim shot G4b's calibration, and no job
had gone red over it. A check that is skipped exactly where it would be enforced is not a check.

**The measurement that decides it.** The Linux renders are the `ecoview-shots` artifact of CI run
35536724097 (commit bcecb6f, the E4 gate); the references are the committed Windows ones at that same
commit, so the only difference is the platform. Split at the page's own seam — the 960×800 `#view`
canvas and the 320×800 sidebar beside it:

| | `#view` | sidebar |
| --- | --- | --- |
| 01–11 (strip) | 0 pixels, 0.000% | 8.31–8.58% of the sidebar |
| 12–15 (Capitol) | 0 pixels, 0.000% | 5.50–6.78% |
| 16–17 (editor) | 0 pixels, 0.000% | 9.02–9.04% |

**SwiftShader draws the WebGL canvas identically on the two platforms — not within a tolerance, but
to the pixel, on all seventeen.** Every differing pixel in all seventeen is text in the sidebar: the
labels, the readout and the chart's numbers, antialiased differently by the two platforms' font
stacks. The note at "Shot 6" that Windows and Linux "won't agree within 2%: fonts, antialiasing and
ANGLE backends differ" is right about the fonts and wrong about the rendering, and it was never
measured.

**The choice.** Of the three ways out the shot prompt names:

1. *Render a second reference set on Linux in CI and commit it.* Correct, but it makes every future
   re-accept a CI round trip: a renderer change cannot produce Linux references until it is pushed, so
   the first run of every such shot would be red by construction. Rejected for that cost, not for
   dishonesty.
2. *Raise the tolerance until one set passes on both platforms.* Dead on the numbers. Whole-image
   platform drift is 1.375–2.260%, and the drift E2 found was 0.213–18.699% whole-image, with five of
   the fifteen changed pictures under 2.260%. A tolerance wide enough to pass the platform would have
   passed real drift.
3. *Compare something coarser than pixels.* Measured and dead too, for the sidebar at least: the
   total-variation distance between 4-bit RGB colour histograms of the sidebar region is **1.24–7.44%
   for the platform pair** and **0.19–0.92% for E2's real drift** — the noise is larger than the
   signal, in every one of the seventeen. No threshold on that statistic separates them.

**What this shot does instead: compare the same references region by region.** `scripts/shot-diff.mjs`
splits a screenshot into the `#view` canvas and the sidebar. The view region is gated on every
platform; the sidebar is gated only when `PLATFORM` matches the references, and elsewhere is measured
and printed but cannot fail. One reference set, rendered on Windows as before, now checked on Linux CI
for everything except its text. This is also the rule the project already states — CLAUDE.md: "Pixel
assertions are measured on the `#view` canvas only; the chart sits in a sidebar outside it" — which
the reference check was the one place not to follow.

**No tolerance moves.** A region fails when it differs by more than `MAX_DIFF` = 2% **of the
screenshot**, not 2% of the region, so each region keeps the same 20,480-pixel budget the flat
whole-image check gave the whole page. Gating the sidebar against its own area would have been four
times stricter and would have failed shots 16 and 17 as they stand (0.663% of the page, 2.652% of the
sidebar — E4's sim line, accepted under the flat check), i.e. it would have forced a re-accept this
shot has no business making. The only loosening is that two regions drifting at once now get a budget
each; `pixelmatch`'s threshold 0.1 is untouched.

**Why the per-pixel threshold stays where it is.** Two runs of `npm run shot` on this machine, same
commit and same data, differ by 20 pixels of the 1,024,000 at pixelmatch threshold 0 and by 0 at the
0.1 the check uses (`shots/01_material_t0_iso.png`, re-rendered in this shot's gate). Software
rendering is not bit-stable even against itself, which is the other reason the SAD's pixel assertions
are coarse and exact-image goldens are forbidden.

**The honesty test.** Replaying E2's drift — the references at 0190df9 against the ones at HEAD, in
CI's mode — the new check fails **11 of the 17**, against the 12 the old flat check would have failed
had it ever run: 03 (9.4% of the page in the view), 04 (11.5%), 05 (17.8%), 06 (2.2%), 07 (3.6%), 09
(7.3%), 10 (10.7%), 11 (6.3%), 15 (3.1%), 16 (2.2%), 17 (6.7%). The one picture it now misses is 02,
whose view drifted 1.563% of the page and which the flat check only caught by adding the sidebar's
0.921%. Eleven red pictures is the same event caught eleven times; the drift could not have hidden.

**The film check loses its fallback too.** `scripts/film-check.mjs` already cropped to `#view`, but
off the reference platform it compared the film frame with the fresh `shots/02` instead of the
committed reference — which could only ever prove the film matched this run's own screenshot. It now
compares with `shots/reference/02` everywhere.

**What a re-accept costs now.** Nothing new: the references stay Windows-rendered, `npm run
shot:accept` is unchanged, and because the view region is platform-identical a re-accept done locally
lands green in CI on the same commit. That is the advantage over option 1.

**Proof that it can fail.** PENDING-DRIFT-EXPERIMENT

**Where the constants live.** `REGIONS` in `scripts/shot-diff.mjs` is the one definition of the page's
two rectangles, built from `VIEW` (`film-lib.mjs`) and `VIEWPORT` (`chromium.mjs`).
`tests/e2e/view.spec.ts` asserts the real `#view` and `#sidebar` bounding boxes equal them, so a
layout change that moved the seam would fail a test rather than quietly gate the wrong pixels.
`tests/unit/shot-diff.test.ts` covers the comparison itself: regions tile the page, a view difference
fails on both platforms, a sidebar difference fails only on the reference platform, and the diff image
carries both regions' marks.
