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
- **World size.** Shot 16 hasn't landed, so `runs/` has only the 64×64×32 world, and `perf.json` records `world: "64x64x32"`.

**Thresholds.** The test fails only if a pair's median draw is over 250 ms or the median step is over 1000 ms, which are the prompt's numbers. The first pair over the draw gate stops the measurement, so a gross regression fails on the gate within seconds instead of on the 5-minute test timeout. `perf.json` is written either way.

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

**Measured on this machine** (win32-x64, SwiftShader):

| Film | Frames | Size | Level | Time |
| --- | --- | --- | --- | --- |
| single view, material/iso (shot 13) | 201 | 960×832 | 3.1 | 25 s |
| 2x2 tiled, scale 1 | 201 | 1920×1632 | 5.0 | 126 s |
| 2x2 tiled, scale 2 (local only) | 201 | 3840×3264 | 6.0 | 305 s |

At scale 2, the tiles are rendered at 2× in WebGL, not upscaled. The iso tile's voxel edges and agents are sharp at 3840×3264. CI makes only the scale-1 tiled film, as the shot allows.

**CI.** After the single-view film, `npm run film:tiled:check` runs and `film/s42-tiled.mp4` is uploaded as `ecoview-film-tiled`. The ecoview job timeout went from 12 to 18 minutes. Before this shot the job took 4.5 minutes, and the tiled film is allowed 6.
