# Screenshot verdicts

`npm run shot` produces 17 PNGs: 01–11 from `runs/s42` (seed 42), the reference world at defaults — the 256×64×32 strip with 8×8 patches, now format 4 with the water tier on (ecosim shot G4) — 12–15 from `runs/capitol-s42`, the Capitol bundle world (ecoview shot G7), and 16–17 from the committed Capitol bundle in edit mode (shot E1). The three groups are tabled separately. Each verdict comes from viewing the PNG. These PNGs are also the visual-regression references in `shots/reference/`. Since shot E6 only **eight** of them are gated by `shot:check` — the ones that picture the renderer; the other nine picture the simulation, are measured and reported but cannot redden a job, and were refreshed here to ecosim shot G4c's output (`REACCEPT-E6.md`). Shot E7 then moved 09 to a tick where the strip actually burns and added its reference (`REACCEPT-E7.md`); the other eight of the nine are untouched. Before that, shot E2 re-accepted 15 of 17 for shot G4b (`REACCEPT-E2.md`) and shot E1 all 17 for shot G4 (`REACCEPT-E1.md`). Each row below says **gated** or **shown**. The top camera fits the whole strip, so the 4:1 world is letterboxed to rows 280–519 of `#view`, with the page background above and below it.

| File | Expectation | Verdict |
| --- | --- | --- |
| `01_material_t0_iso.png` **gated** | Heightmapped terrain, ponds, sparse trees, animals, no green yet | **PASS.** The strip runs diagonally across the view, west end at the upper left, east end nearest the camera. Bare brown stepped terrain, grey rock outcrops in the west and north, two blue ponds, scattered dark-green young trees and 332 entities as yellow grazer and red hunter dots. Grass at 0.1 barely tints the soil. |
| `02_material_t10000_iso.png` **gated** | Many more trees, green ground | **PASS.** A closed dark canopy now covers everything but the western sixth, bright light-green grass and tan bare ground hold the far west end, and the two ponds still read through it. 1247 trees, 822 of them mature — far above the addendum's floor of 35 — and 4357 entities as yellow grazer and red hunter specks. Grass mean 0.477, down from 0.563: ecosim shot G4c's Beer-Lambert canopy is darker, and the grass under it pays. |
| `03_light_t10000_top.png` **shown** | ≥5% of pixels dark, ≥40% light | **PASS.** Shade is three dense black masses over the east, the centre and the near-west, heavier than before and pushed east, with open white ground from the west end to about a third across. Measured over world pixels only (letterbox excluded): 24.8% dark, 57.2% light, against 23.1% and 59.4% before G4c. `view.spec.ts` asserts this relationally and still passes. |
| `04_moisture_t10000_top.png` **shown** | Wet halos around the ponds | **PASS.** The same clean west-to-east ramp, mean 174.9 of 255: white dry ground over the western third grading through pale blue to deep blue in the east, with grey rock outcrops and the two ponds on top. The field is G4b's calibrated rainfall (887 mm a simulated year against Lansing's ~800 mm) and G4c did not touch it; only the trees and animals drawn over it moved. |
| `05_fertility_t10000_top.png` **shown** | Darker patches where litter decays | **PASS.** Mid-brown over the whole strip (mean 150.6), darkest in a south-west block and under the eastern canopy, with the pale west corner the only light ground left. Grey rock and blue ponds sit on top. Unchanged in level from before G4c (150.8); what differs is the scatter of entities. |
| `06_temperature_t1000_top.png` **shown** | Warm (summer) | **PASS.** Near-uniform red across the strip at 26.98 °C, with the two ponds in water blue and 1945 entities scattered over it. |
| `07_temperature_t3000_top.png` **shown** | Cold (winter) | **PASS.** Uniform saturated blue at −3.10 °C; the ponds show as a slightly lighter blue and 3493 entities dot the field. |
| `08_chart_t20000.png` **gated** | Grazers oscillate, hunters lag, trees rise; four panels | **PASS.** The iso view at tick 20000 is a closed dark canopy over everything but the western sixth, which stays light green over grey rock. 3655 entities. Four chart panels. **Top:** grazers rise to 3385 at tick 2826 and then decline in a long oscillation; hunters lag them and peak at 129 at tick 15084. **Second:** trees rise in a sawtooth to 1804 at tick 18300, ending at 1543. **Third:** deaths per 100 ticks, dominated by purple `crowded` (41,566) over `eaten` (8,967), with smaller `starved` (2,448), `old_age` (853) and a little `burnt` (11). **Fourth:** a dozen orange spikes of patches burning, at most 6 at tick 8924; the blue mean grazer `energy_cost_mult` falls across the run to 0.952. The tick marker is at the right edge. |
| `09_fire_t13800_top.png` **shown** | The tick where the strip is actually on fire (13800): burning patches orange by ticks left, burnt-out patches charcoal, the rest material | **PASS, and the subject is back.** Both things this overlay draws are in the frame. Two patches are alight at the west end: the corner patch at (0, 0) in dark red `#b3300a` with one tick left, and the patch 8 m east and 16 m north of it in mid-orange `#d97015` with two — one 30x30 px block each, exactly one patch. Eleven more are charcoal, from the 16 `burnout` events between ticks 13701 and 13800. Ten of those eleven and both live fires sit inside one contiguous scar over the western 40 x 32 m of the strip; the eleventh is an isolated patch just past its north-east corner, which is the tail of the same spread. The rest of the strip is the material colour with grey rock, the two blue ponds and 4516 entities over it. Shot E7 moved this picture here from tick 17100, which after ecosim shot G4c had neither flame nor scar. |
| `10_crowding_t20000_top.png` **shown** | Grazers per patch, white → magenta | **PASS.** Pale pink over most of the strip with saturated magenta blocks scattered through it and clustered at the west end, and a scatter of white (empty) patches. 2006 grazers at this tick, against 2898 before G4c, so the picture is thinner and its magenta more concentrated. |
| `11_traits_t20000_top.png` **shown** | Grazers blue below the default cost, red above | **PASS.** The ground is the material colour, hunters are grey. Grazers are a mix of blue, white and red dots with blue clearly ahead — 1386 below 1.0 and 620 above, against a series mean of 0.952. No species-yellow grazers are left. |

**Expectation wording.** The SAD's "grazer curve oscillating, hunter curve lagging it, tree curve rising" still holds, in a damped form on this world. Since ecosim shot 10, crowding mortality, not starvation, is the main cause of death, and the chart's third panel shows it. Burnt ground comes from `events.csv` burnouts since the previous snapshot (DECISIONS.md, shot 16).

## The Capitol (shots 12–15)

`runs/capitol-s42`: the committed `worlds/capitol` bundle over 256 m of the Michigan State Capitol grounds, run at seed 42 for 20000 ticks with `animals.enabled=false` and `climate.rain_gradient=0` (ecosim shot G3a). The world is square, so the top camera fills the view height and the world spans screen x 80–880 of `#view`. The `medium` overlay reads the 0.5 m ground grid through a draped texture, so it shows twice the detail of the 1 m voxel columns underneath it; its legend is the only overlay legend, which is why shots 01–11 carry none. Shots 12–14 are tick 0, and ecosim shot G4c moved almost nothing in them: 12 and 13 are byte-identical to their references inside `#view`, 14 differs by 0.021% of the page where the new light rule meets the trees' own shade, and the sidebar chart moved 0.178% because the run behind it changed. All three stay gated. 15 is at tick 20000 and is a picture of the ecology, so it is shown rather than gated.

| File | Expectation | Verdict |
| --- | --- | --- |
| `12_capitol_medium_t0_iso.png` **gated** | Iso, `medium` at t=0: a large grey building block in the centre, asphalt bands along at least two sides | **PASS.** The Capitol reads as a building from the diagonal: the stepped wings, the drum and the ribbed dome all stand above a green lawn, with five office blocks along the north and east streets. Dark asphalt bands run the full length of all four edges, with the pale walks and the circular drive in front of the west steps drawn on the lawn between them. The legend lists all nine media. |
| `13_capitol_medium_t0_top.png` **gated** | Top, `medium` at t=0: the site plan, every medium its palette colour | **PASS.** Straight down it reads as the real grounds: lawn green over most of the site, the cross-shaped Capitol footprint in building grey, pale concrete walks radiating from it to the corners, dark asphalt streets framing all four sides, and dark tree beds scattered over the lawn. The four pipes are dashed blue lines inlet→outlet, drawn only on this camera. |
| `14_capitol_light_t0_top.png` **gated** | Top, `light` at t=0: the buildings' shade | **PASS.** Open ground is white. Solid black blocks sit immediately north of the dome, of both wings and of the north office blocks — the columns those buildings shade all day. The building footprints themselves are the light grey of the extruded roofs; the small scattered black squares are the 79 young trees' own shade. G4c's new light rule moved 0.021% of the page here and nothing structural. |
| `15_capitol_material_t20000_iso.png` **shown** | Iso, `material` at t=20000: trees on the lawn, none on roof or road | **PASS.** 5207 trees, up from 2875 before ecosim shot G4c, whose seeding fix let 63 of the 79 imported trees seed for the first time: the canopy closes into a near-continuous dark mass on all four sides of the building. Not one stands on a roof or in a street — and since this shot that is asserted per tree rather than as a share of cells (`tests/e2e/capitol.spec.ts`): none of the 5207 has three or four of its four 0.5 m ground cells paved, and none has a roof cell at all. |

## Edit mode (shots 16–17)

`fixtures/capitol-world`: the same bundle, loaded through `?world=` with no run behind it, in edit mode (`&edit=1`). `?eye=` stands the first-person camera at a fixed point in metres east, north and up with a yaw and a pitch, so the crosshair picks a cell without pointer lock and the shot stays deterministic (DECISIONS.md, "E1 editor"). Both viewpoints pick a **top** face, because the outline is always drawn on the picked cell's top face.

| File | Expectation | Verdict |
| --- | --- | --- |
| `16_edit_hotbar.png` **gated** | Edit mode on, the hotbar visible with the building slot lit, the crosshair outline on a cell | **PASS.** The Capitol now stands as stacked cubes: the dome and drum step up in half-metre courses, the wings are block walls with their string courses legible as steps, and the lawn terraces in cube-wide contours down to the walks. Nothing of the E1 composition moved — same camera, same trees, same walks. The sidebar shows all eight hotbar slots with `8 building` boxed, `edit on (E) · camera fly (F) · brush 3 ([ ])`, `cell (256, 151) roof · ground 7.15 m · building 25.87 m · side face`, `0 undo (Ctrl-Z) · 0 redo (Ctrl-Y) · 0 saved (Ctrl-S)`, and the note derived from the bundle's cell: one 0.5 m cube a click, 2 clicks to a voxel. The magenta outline over the roof is nine cube wireframes, not nine flat squares. Below the note is the sim helper's line (shot E4), `sim: press R to run the simulator on these edits`; `npm run shot` never starts the helper, so this is what a screenshot always shows. |
| `17_edit_brush5.png` **gated** | Brush 5 on the lawn, the 49-cell disc outlined under the crosshair | **PASS.** From the same eye point on the lawn: the road fills the foreground as one flat asphalt terrace, the lawn behind it rises in clean cube steps instead of E1's stippled prism tops, and the dome and office blocks stand over the trees at the horizon. The brush reads as blocks — 49 magenta cube wireframes in the disc shape, each one twelve edges, standing proud of the grass rather than lying on it. The sidebar reads `1 lawn` boxed, `brush 5`, `cell (140, 35) lawn · ground 5.24 m · building 0.00 m · top face`, and the footer `256×256 m over 512×512 ground cells at 0.5 m · 81 trees, 64 shrubs, 4 pipes`. Below the note is the sim helper's line (shot E4), `sim: press R to run the simulator on these edits`; `npm run shot` never starts the helper, so this is what a screenshot always shows. |

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
| material | 265.50 / 274.0 | 3.77 | 285.50 / 292.4 | 3.50 |
| light | 265.15 / 274.9 | 3.77 | 163.90 / 168.1 | 6.10 |
| moisture | 264.30 / 270.9 | 3.78 | 164.75 / 168.1 | 6.07 |
| fertility | 284.45 / 300.9 | 3.52 | 167.80 / 183.9 | 5.96 |
| temperature | 287.30 / 306.2 | 3.48 | 180.70 / 188.3 | 5.53 |
| fire | 298.80 / 306.2 | 3.35 | 154.70 / 158.7 | 6.46 |
| crowding | 293.65 / 307.4 | 3.41 | 153.60 / 160.1 | 6.51 |
| traits | 299.10 / 304.1 | 3.34 | 157.35 / 177.5 | 6.36 |

| Step (50 snapshots) median / p95 | Step fps | First load |
| --- | --- | --- |
| 65.9 / 629.1 ms | 15.19 | 515 ms |

The numbers above are shot E6's run on ecosim shot G4c's data (1247 trees at tick 10000, against 1145 on
G4b and 608 before it). Draw is flat to a few per cent against E2's figures; step got three times faster,
because a denser canopy loads fewer distinct column heights per snapshot. The cause of all of it is the
scene and not the renderer, and SwiftShader is paid in pixels. The gates are unchanged and unmoved — a median draw of at most 1000 ms per pair (250 ms
per 4096 columns, shot 28's gate made per-column in shot 16) and a median step of at most 1000 ms. At
277 ms the worst pair still has 3.6× headroom on its gate. What ran out in CI was the single test's flat
wall clock, which is what shot E2 replaced.

| Test | Measured | Budget | Margin | Half-budget alarm |
| --- | --- | --- | --- | --- |
| draw, cam=iso | 141.2 s | 630 s | 4.5× | 315 s |
| draw, cam=top | 90.2 s | 630 s | 7.0× | 315 s |
| step | 12.1 s | 92.5 s | 7.7× | 46.2 s |

## Films

`npm run film:check` and `npm run film:tiled:check` on the strip, at **1.204%** and **1.241%** against
`shots/reference/02_material_t10000_iso.png`, of the same 2% bar `shot:check` uses. Shot E6 re-ran both
on ecosim shot G4c's data and left reference 02 alone — it stays gated and inside tolerance, so the film
is still measured against a committed picture and not against its own output.

| Film | Frames | Size | Time | Budget |
| --- | --- | --- | --- | --- |
| `s42-material.mp4` (material:iso) | 201 | 960×832 | 67.9 s | 300 s |
| `s42-tiled.mp4` (2×2) | 201 | 1920×1632 | 309.3 s | 700 s |

## Shot E5: the same pictures, now checked on Linux too

No screenshot changed in this shot and no reference was re-accepted. All seventeen were re-rendered and
viewed again, and every verdict above still reads true of the image on disk: `npm run shot` wrote them in
20.9 s of the 60 s budget and `node scripts/shot-ref.mjs check` passed 17/17, with **0.000% of the `#view`
canvas differing on every row**. The two editor shots still carry E4's sim line in the sidebar (0.663% of
the page, 2.652% of the sidebar), inside the same budget that accepted it.

What is new is where the check runs. It compares the `#view` canvas and the sidebar separately, gates the
canvas on every platform and the sidebar only on the platform the references came from, so the CI job on
Linux now runs it instead of skipping it — the hole that let fifteen references go stale until shot E2
noticed. The Windows references and the Linux CI renders agree **to the pixel** inside `#view` on all
seventeen; the whole disagreement is the sidebar's text, 5.5–9.0% of it. The numbers, the two options this
rejected and the proof that a perturbed reference reddens the job — CI run 35539520660, red at `shot:check`
on one deliberately spoiled reference and green on the other sixteen — are in `DECISIONS.md` under "E5
reference screenshots in CI".

Re-rendering also measured this machine against itself: two runs of `npm run shot` at the same commit
differ by 20 pixels of 1,024,000 at pixelmatch threshold 0, and by none at the 0.1 the check uses.

## Shot E6: eight of these pictures gate, nine are shown

Nothing in the renderer changed here either. What changed is which pictures can redden a job. The nine
that show simulated ecology at a nonzero tick — 03–07, 09–11 and 15 — left the gate and were refreshed to
ecosim shot G4c's output (`REACCEPT-E6.md`); the eight that picture the renderer stay gated at the same
2% in the same regions, and all eight pass, the worst at 0.903% (02). They are still rendered, still
compared and still reported here, because a human reading a verdict is the point of this file.

The split is honest about three of the eight: 01, 12, 13, 16 and 17 cannot move with the simulation at
all, but 02, 08 and 14 can and simply do not move much. Each of those three is the only gate on something
the renderer owns, so they stay in, with their drift printed every run as the early warning
(`scripts/shots.mjs`).
