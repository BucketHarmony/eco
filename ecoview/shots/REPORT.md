# Screenshot verdicts

`npm run shot` produces 17 PNGs: 01–11 from `runs/s42` (seed 42), the reference world at defaults — the 256×64×32 strip with 8×8 patches, now format 4 with the water tier on (ecosim shot G4) — 12–15 from `runs/capitol-s42`, the Capitol bundle world (ecoview shot G7), and 16–17 from the committed Capitol bundle in edit mode (shot E1). The three groups are tabled separately. Each verdict comes from viewing the PNG. These PNGs are also the visual-regression references in `shots/reference/`, 15 of the 17 re-accepted in shot E2 (`REACCEPT-E2.md`) because ecosim shot G4b's units calibration regenerated both runs; before that, shot E1 re-accepted all 17 after shot G4 (`REACCEPT-E1.md`). The top camera fits the whole strip, so the 4:1 world is letterboxed to rows 280–519 of `#view`, with the page background above and below it.

| File | Expectation | Verdict |
| --- | --- | --- |
| `01_material_t0_iso.png` | Heightmapped terrain, ponds, sparse trees, animals, no green yet | **PASS.** The strip runs diagonally across the view, west end at the upper left, east end nearest the camera. Bare brown stepped terrain, grey rock outcrops in the west and north, two blue ponds, scattered dark-green young trees and 332 entities as yellow grazer and red hunter dots. Grass at 0.1 barely tints the soil. |
| `02_material_t10000_iso.png` | Many more trees, green ground | **PASS.** A closed dark canopy now reaches from the eastern third across the middle of the strip, bright light green grass holds the near-west, and the far west end is tan where the calibrated rainfall no longer keeps grass up (mean 0.563). 1145 trees, 760 of them mature — far above the addendum's floor of 35. Animals show as yellow and red specks between the stands; 4248 entities in all. |
| `03_light_t10000_top.png` | ≥5% of pixels dark, ≥40% light | **PASS.** Shade is three dense black masses — over the east, the centre and the near-west — with scattered single squares between them; open ground is white and the dry west end is nearly all white. Measured over world pixels only (letterbox excluded): 23.1% dark, 59.4% light. Twice as much shade as before ecosim shot G4b, because there are nearly twice as many trees. |
| `04_moisture_t10000_top.png` | Wet halos around the ponds | **PASS.** A clean west-to-east ramp: white dry ground over the western third, grading through pale blue to deep blue in the east, mean 175.0 of 255. Grey rock outcrops and the two ponds sit on top, the ponds in the lighter water blue. Shot E1 saw this overlay flat wet (mean 215.9) because the strip was getting about 4200 mm of rain a simulated year; ecosim shot G4b cut that to 887 mm against Lansing's ~800 mm normal, and at that rate `climate.rain_gradient` shows as a real dry west again. |
| `05_fertility_t10000_top.png` | Darker patches where litter decays | **PASS.** Mid-brown over the whole strip (mean 150.8), darkest in a south-west block and under the eastern canopy, with the pale west corner the only light ground left. Grey rock and blue ponds sit on top. The field more than doubled from shot E1's 69.0 because litter makes fertility and there are nearly twice as many trees dropping it. |
| `06_temperature_t1000_top.png` | Warm (summer) | **PASS.** Near-uniform red across the strip at 26.98 °C, with the two ponds in water blue and 1947 entities scattered over it. |
| `07_temperature_t3000_top.png` | Cold (winter) | **PASS.** Uniform saturated blue at −3.08 °C; the ponds show as a slightly lighter blue and 3429 entities dot the field. |
| `08_chart_t20000.png` | Grazers oscillate, hunters lag, trees rise; four panels | **PASS.** The iso view at tick 20000 is a closed green canopy over everything but the western sixth, which stays light green and tan. 4527 entities. Four chart panels. **Top:** grazers rise to 3421 at tick 3244 and then oscillate below it; hunters lag them and peak at 112 at tick 15864. **Second:** trees rise in a sawtooth to 1805 at tick 18300, ending at 1541. **Third:** deaths per 100 ticks, dominated by purple `crowded` (38,177) over `eaten` (8,475), with smaller `old_age` (1,459), `starved` (1,273) and a little `burnt` (110). **Fourth:** a dozen orange spikes of patches burning, at most 6 at tick 17137 — many more than the three shot E1 saw; the blue mean grazer `energy_cost_mult` falls from 1.001 to 0.973 across the run. The tick marker is at the right edge. |
| `09_fire_t17100_top.png` | Tick with the most patches burning (17100): burning orange, burnt charcoal, the rest material | **PASS on the overlay, FAIL on the subject — still flagged for the operator, but the reason changed.** The overlay draws correctly: two charcoal patches at the far west edge (the two burnouts between ticks 17000 and 17100) over the material colour everywhere else, with trunks and animals on top. There is still no orange, because no patch is burning *at this tick*. What changed under ecosim shot G4b is that fire came back: 37 ignitions, 189 spreads and 226 burnouts in 20,000 ticks against shot E1's 8 / 5 / 13, and `patches_burning` reaches 6 at tick 17137. Tick 17100 was picked when it was the snapshot with the most patches alight; the best snapshot ticks now are 9500 and 13400, with 2 burning each. Moving the tick means editing `scripts/shots.mjs` and adding a reference, which shot E2 was not asked to do, so the tick is left alone and the finding is recorded here and in `REACCEPT-E2.md`. |
| `10_crowding_t20000_top.png` | Grazers per patch, white → magenta | **PASS.** Pink over the whole strip with saturated magenta patches scattered through it and clustered at the west end, and a few white (empty) patches, most of them at the dry western end. 2898 grazers at this tick. |
| `11_traits_t20000_top.png` | Grazers blue below the default cost, red above | **PASS.** The ground is the material colour, hunters are grey. Grazers are a mix of blue, white and red dots with blue clearly ahead — 1743 below 1.0 and 1155 above, against a series mean of 0.974. No species-yellow grazers are left. |

**Expectation wording.** The SAD's "grazer curve oscillating, hunter curve lagging it, tree curve rising" still holds, in a damped form on this world. Since ecosim shot 10, crowding mortality, not starvation, is the main cause of death, and the chart's third panel shows it. Burnt ground comes from `events.csv` burnouts since the previous snapshot (DECISIONS.md, shot 16).

## The Capitol (shots 12–15)

`runs/capitol-s42`: the committed `worlds/capitol` bundle over 256 m of the Michigan State Capitol grounds, run at seed 42 for 20000 ticks with `animals.enabled=false` and `climate.rain_gradient=0` (ecosim shot G3a). The world is square, so the top camera fills the view height and the world spans screen x 80–880 of `#view`. The `medium` overlay reads the 0.5 m ground grid through a draped texture, so it shows twice the detail of the 1 m voxel columns underneath it; its legend is the only overlay legend, which is why shots 01–11 carry none. Shots 12–14 are tick 0 and their `#view` pixels are byte-identical to the previous references; only their sidebar chart moved (0.213% of the page), because ecosim shot G4b regenerated this run too.

| File | Expectation | Verdict |
| --- | --- | --- |
| `12_capitol_medium_t0_iso.png` | Iso, `medium` at t=0: a large grey building block in the centre, asphalt bands along at least two sides | **PASS.** The Capitol reads as a building from the diagonal: the stepped wings, the drum and the ribbed dome all stand above a green lawn, with five office blocks along the north and east streets. Dark asphalt bands run the full length of all four edges, with the pale walks and the circular drive in front of the west steps drawn on the lawn between them. The legend lists all nine media. |
| `13_capitol_medium_t0_top.png` | Top, `medium` at t=0: the site plan, every medium its palette colour | **PASS.** Straight down it reads as the real grounds: lawn green over most of the site, the cross-shaped Capitol footprint in building grey, pale concrete walks radiating from it to the corners, dark asphalt streets framing all four sides, and dark tree beds scattered over the lawn. The four pipes are dashed blue lines inlet→outlet, drawn only on this camera. |
| `14_capitol_light_t0_top.png` | Top, `light` at t=0: the buildings' shade | **PASS.** Open ground is white at 255. Solid black blocks sit immediately north of the dome, of both wings and of the north office blocks — the columns those buildings shade all day. The building footprints themselves are the light grey of the extruded roofs; the small scattered black squares are the 79 young trees' own shade. |
| `15_capitol_material_t20000_iso.png` | Iso, `material` at t=20000: trees on the lawn, none on roof or road | **PASS.** 2875 trees, up from 1982 before ecosim shot G4b: the canopy now closes over the east, south and north-west lawns alike, leaving open only the walks, the drive and the ground immediately around the building. Not one stands on a roof or in a street — the grey streets, walks and building footprints are bare, because the bundle lays them down as Rock and the sim only plants on Soil. |

## Edit mode (shots 16–17)

`fixtures/capitol-world`: the same bundle, loaded through `?world=` with no run behind it, in edit mode (`&edit=1`). `?eye=` stands the first-person camera at a fixed point in metres east, north and up with a yaw and a pitch, so the crosshair picks a cell without pointer lock and the shot stays deterministic (DECISIONS.md, "E1 editor"). Both viewpoints pick a **top** face, because the outline is always drawn on the picked cell's top face.

| File | Expectation | Verdict |
| --- | --- | --- |
| `16_edit_hotbar.png` | Edit mode on, the hotbar visible with the building slot lit, the crosshair outline on a cell | **PASS.** The Capitol now stands as stacked cubes: the dome and drum step up in half-metre courses, the wings are block walls with their string courses legible as steps, and the lawn terraces in cube-wide contours down to the walks. Nothing of the E1 composition moved — same camera, same trees, same walks. The sidebar shows all eight hotbar slots with `8 building` boxed, `edit on (E) · camera fly (F) · brush 3 ([ ])`, `cell (256, 151) roof · ground 7.15 m · building 25.87 m · side face`, `0 undo (Ctrl-Z) · 0 redo (Ctrl-Y) · 0 saved (Ctrl-S)`, and the note derived from the bundle's cell: one 0.5 m cube a click, 2 clicks to a voxel. The magenta outline over the roof is nine cube wireframes, not nine flat squares. Below the note is the sim helper's line (shot E4), `sim: press R to run the simulator on these edits`; `npm run shot` never starts the helper, so this is what a screenshot always shows. |
| `17_edit_brush5.png` | Brush 5 on the lawn, the 49-cell disc outlined under the crosshair | **PASS.** From the same eye point on the lawn: the road fills the foreground as one flat asphalt terrace, the lawn behind it rises in clean cube steps instead of E1's stippled prism tops, and the dome and office blocks stand over the trees at the horizon. The brush reads as blocks — 49 magenta cube wireframes in the disc shape, each one twelve edges, standing proud of the grass rather than lying on it. The sidebar reads `1 lawn` boxed, `brush 5`, `cell (140, 35) lawn · ground 5.24 m · building 0.00 m · top face`, and the footer `256×256 m over 512×512 ground cells at 0.5 m · 81 trees, 64 shrubs, 4 pipes`. Below the note is the sim helper's line (shot E4), `sim: press R to run the simulator on these edits`; `npm run shot` never starts the helper, so this is what a screenshot always shows. |

## Performance

`tests/e2e/perf.spec.ts` (runs in `npm test`) on `runs/s42` at tick 10000. Shot E2 split it into three
Playwright tests — the draw matrix per camera and the step loop on its own — each with a timeout derived
from the work it does instead of one flat 300 s for all of it (`DECISIONS.md`, "E2 perf budget"). The
measurements and `perf/perf.json` are unchanged in shape and meaning. Draw is still 60 back-to-back
renders of one view through `window.__ecoviewBench`, each completed with `gl.finish()` plus a one-pixel
readback; step is still `__ecoviewGoto(tick)` → `__ecoviewReady` over ticks 10000–14900, which is what
Play sees. The numbers below are one run of `npx playwright test tests/e2e/perf.spec.ts` on this machine
(win32-x64, Chromium 153.0.8010.12, ANGLE on Vulkan SwiftShader); run to run the medians move by about
1 ms. CI's are in the `ecoview-perf` artifact.

**These come from SwiftShader, a software GPU running on the CPU.** They measure the viewer's own cost
and catch regressions. They are not the frame rate on a real graphics card, which would be far higher.

| Overlay | iso median / p95 ms | iso fps | top median / p95 ms | top fps |
| --- | --- | --- | --- | --- |
| material | 275.80 / 278.8 | 3.63 | 275.30 / 278.3 | 3.63 |
| light | 276.45 / 279.1 | 3.62 | 166.45 / 169.9 | 6.01 |
| moisture | 277.00 / 279.3 | 3.61 | 166.20 / 168.3 | 6.02 |
| fertility | 276.55 / 278.8 | 3.62 | 166.10 / 168.5 | 6.02 |
| temperature | 276.35 / 279.2 | 3.62 | 166.30 / 168.4 | 6.01 |
| fire | 276.80 / 280.9 | 3.61 | 166.10 / 167.3 | 6.02 |
| crowding | 276.65 / 280.5 | 3.61 | 166.05 / 167.6 | 6.02 |
| traits | 277.45 / 279.9 | 3.60 | 166.95 / 169.6 | 5.99 |

| Step (50 snapshots) median / p95 | Step fps | First load |
| --- | --- | --- |
| 374.7 / 638.9 ms | 2.67 | 442 ms |

Every draw figure is about 29% worse than shot E1's, and the cause is the scene and not the renderer:
ecosim shot G4b's calibration puts 1145 trees at tick 10000 where the old run had 608, and SwiftShader is
paid in pixels. The gates are unchanged and unmoved — a median draw of at most 1000 ms per pair (250 ms
per 4096 columns, shot 28's gate made per-column in shot 16) and a median step of at most 1000 ms. At
277 ms the worst pair still has 3.6× headroom on its gate. What ran out in CI was the single test's flat
wall clock, which is what shot E2 replaced.

| Test | Measured | Budget | Margin | Half-budget alarm |
| --- | --- | --- | --- | --- |
| draw, cam=iso | 140.7 s | 630 s | 4.5× | 315 s |
| draw, cam=top | 92.8 s | 630 s | 6.8× | 315 s |
| step | 20.1 s | 92.5 s | 4.6× | 46.2 s |

## Films

`npm run film:check` and `npm run film:tiled:check` on the strip, both at 0.000% against `shots/reference/02_material_t10000_iso.png`, re-run in shot E2 against the re-accepted reference and ecosim shot G4b's regenerated run.

| Film | Frames | Size | Time | Budget |
| --- | --- | --- | --- | --- |
| `s42-material.mp4` (material:iso) | 201 | 960×832 | 83.1 s | 300 s |
| `s42-tiled.mp4` (2×2) | 201 | 1920×1632 | 342.8 s | 700 s |
