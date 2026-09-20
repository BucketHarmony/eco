# Screenshot verdicts

`npm run shot` produces 17 PNGs: 01–11 from `runs/s42` (seed 42), the reference world at defaults — the 256×64×32 strip with 8×8 patches, now format 4 with the water tier on (ecosim shot G4) — 12–15 from `runs/capitol-s42`, the Capitol bundle world (ecoview shot G7), and 16–17 from the committed Capitol bundle in edit mode (shot E1). The three groups are tabled separately. Each verdict comes from viewing the PNG. These PNGs are also the visual-regression references in `shots/reference/`, all 17 re-accepted in shot E1 (`REACCEPT-E1.md`) because ecosim shot G4 regenerated `runs/s42` under the water tier. The top camera fits the whole strip, so the 4:1 world is letterboxed to rows 280–519 of `#view`, with the page background above and below it.

| File | Expectation | Verdict |
| --- | --- | --- |
| `01_material_t0_iso.png` | Heightmapped terrain, ponds, sparse trees, animals, no green yet | **PASS.** The strip runs diagonally across the view, west end at the upper left, east end nearest the camera. Bare brown stepped terrain, grey rock outcrops in the west and north, two blue ponds, scattered dark-green young trees and 332 entities as yellow grazer and red hunter dots. Grass at 0.1 barely tints the soil. |
| `02_material_t10000_iso.png` | Many more trees, green ground | **PASS.** Grass now covers the whole strip (mean 0.755): bright light green in the west, a closed dark canopy over the eastern third around the pond. 608 trees, 371 of them mature — well above the addendum's floor of 35. Animals show as yellow and red specks between the stands. |
| `03_light_t10000_top.png` | ≥5% of pixels dark, ≥40% light | **PASS.** Shade is a dense black mass over the eastern forest with a few isolated squares further west; open ground is white. Measured over world pixels only (letterbox excluded): 11.4% dark, 75.8% light. The canopy is more concentrated than it was before the water tier, which is why the dark share fell and the light share rose. |
| `04_moisture_t10000_top.png` | Wet halos around the ponds | **PASS, with a note.** Moisture (mean 215.9 of 255) is deep blue over almost the whole strip, with pale, nearly white ground only at the western end and on the high ground around the grey rock outcrops. Under the water tier, soil water — not the rain gradient alone — sets the field, so the picture is wet-flat with dry high ground rather than a west-to-east ramp, and the ponds have no halo to show: their own cells read as the lighter water blue. |
| `05_fertility_t10000_top.png` | Darker patches where litter decays | **PASS.** Pale tan over most of the strip (mean 69.0), a dark brown block in the south-west, and a wash of small dark squares in the east where tree litter falls. Grey rock and blue ponds sit on top. |
| `06_temperature_t1000_top.png` | Warm (summer) | **PASS.** Near-uniform red across the strip, with the two ponds in water blue and grazers scattered over it. |
| `07_temperature_t3000_top.png` | Cold (winter) | **PASS.** Uniform saturated blue; the ponds show as a slightly lighter blue and 3457 entities dot the field. |
| `08_chart_t20000.png` | Grazers oscillate, hunters lag, trees rise; four panels | **PASS.** The iso view at tick 20000 is a closed green canopy over the eastern two thirds, lighter grass in the west. Four chart panels. **Top:** grazers rise to 3385 at tick 2784 and then hold between 2740 and that peak; hunters lag them, peak at 124 at tick 16006 and never fall below 70 after tick 5000. **Second:** trees rise in a sawtooth to 1573 at tick 16900, ending at 1414. **Third:** deaths per 100 ticks, dominated by purple `crowded` (36,518) over `eaten` (8,805), with smaller `old_age` (1,809) and `starved` (398) and no `burnt` at all. **Fourth:** three lone orange spikes of patches burning (at most 3, at tick 17216); the blue mean grazer `energy_cost_mult` wanders between 0.988 and 1.005 and drops at the very end. The tick marker is at the right edge. |
| `09_fire_t17100_top.png` | Tick with the most patches burning (17100): burning orange, burnt charcoal, the rest material | **PASS on the overlay, FAIL on the subject — flagged for the operator.** The overlay draws correctly: one charcoal patch at the far west edge (the single burnout between ticks 17000 and 17100) over the material colour everywhere else, with trunks and animals on top. But no patch is burning, so there is no orange. Under the water tier fire has nearly vanished from `runs/s42`: 8 ignitions, 5 spreads and 13 burnouts in 20,000 ticks, `patches_burning` never above 3 (tick 17216), and no snapshot tick has more than 1 patch alight (tick 5900, one patch). No tick in this run can satisfy the SAD's wording. Changing that needs a sim-side decision — fire parameters under the new hydrology, or a different reference seed — so shot E1 leaves the tick alone and records the finding here. |
| `10_crowding_t20000_top.png` | Grazers per patch, white → magenta | **PASS.** Pink over the whole strip with saturated magenta patches scattered through it and clustered at the west end, and a few white (empty) patches. 2935 grazers at this tick. |
| `11_traits_t20000_top.png` | Grazers blue below the default cost, red above | **PASS.** The ground is the material colour, hunters are grey. Grazers are a near-even mix of blue, white and red dots — 1652 below 1.0 and 1283 above, against a series mean of 0.990, so blue and white outnumber red slightly. No species-yellow grazers are left. |

**Expectation wording.** The SAD's "grazer curve oscillating, hunter curve lagging it, tree curve rising" still holds, in a damped form on this world. Since ecosim shot 10, crowding mortality, not starvation, is the main cause of death, and the chart's third panel shows it. Burnt ground comes from `events.csv` burnouts since the previous snapshot (DECISIONS.md, shot 16).

## The Capitol (shots 12–15)

`runs/capitol-s42`: the committed `worlds/capitol` bundle over 256 m of the Michigan State Capitol grounds, run at seed 42 for 20000 ticks with `animals.enabled=false` and `climate.rain_gradient=0` (ecosim shot G3a). The world is square, so the top camera fills the view height and the world spans screen x 80–880 of `#view`. The `medium` overlay reads the 0.5 m ground grid through a draped texture, so it shows twice the detail of the 1 m voxel columns underneath it; its legend is the only overlay legend, which is why shots 01–11 carry none. All four are byte-identical to their previous references (0.000%): this run was not regenerated by ecosim shot G4.

| File | Expectation | Verdict |
| --- | --- | --- |
| `12_capitol_medium_t0_iso.png` | Iso, `medium` at t=0: a large grey building block in the centre, asphalt bands along at least two sides | **PASS.** The Capitol reads as a building from the diagonal: the stepped wings, the drum and the ribbed dome all stand above a green lawn, with five office blocks along the north and east streets. Dark asphalt bands run the full length of all four edges, with the pale walks and the circular drive in front of the west steps drawn on the lawn between them. The legend lists all nine media. |
| `13_capitol_medium_t0_top.png` | Top, `medium` at t=0: the site plan, every medium its palette colour | **PASS.** Straight down it reads as the real grounds: lawn green over most of the site, the cross-shaped Capitol footprint in building grey, pale concrete walks radiating from it to the corners, dark asphalt streets framing all four sides, and dark tree beds scattered over the lawn. The four pipes are dashed blue lines inlet→outlet, drawn only on this camera. |
| `14_capitol_light_t0_top.png` | Top, `light` at t=0: the buildings' shade | **PASS.** Open ground is white at 255. Solid black blocks sit immediately north of the dome, of both wings and of the north office blocks — the columns those buildings shade all day. The building footprints themselves are the light grey of the extruded roofs; the small scattered black squares are the 79 young trees' own shade. |
| `15_capitol_material_t20000_iso.png` | Iso, `material` at t=20000: trees on the lawn, none on roof or road | **PASS.** 1982 trees, a closed canopy over the east and south lawns and a thinner stand in the north-west. Not one stands on a roof or in a street: the grey streets, walks and building footprints are bare, because the bundle lays them down as Rock and the sim only plants on Soil. Of the four ground cells under each trunk, 97.9% are lawn, 0.5% asphalt (trunks beside a walk edge) and none roof. |

## Edit mode (shots 16–17)

`fixtures/capitol-world`: the same bundle, loaded through `?world=` with no run behind it, in edit mode (`&edit=1`). `?eye=` stands the first-person camera at a fixed point in metres east, north and up with a yaw and a pitch, so the crosshair picks a cell without pointer lock and the shot stays deterministic (DECISIONS.md, "E1 editor"). Both viewpoints pick a **top** face, because the outline is always drawn on the picked cell's top face.

| File | Expectation | Verdict |
| --- | --- | --- |
| `16_edit_hotbar.png` | Edit mode on, the hotbar visible with the building slot lit, the crosshair outline on a cell | **PASS.** The whole Capitol stands in front of the camera from 44 m up on the south front: dome, drum and both wings against the sky, lawn and walks below. The sidebar shows all eight hotbar slots with `8 building` boxed and bold, `edit on (E) · camera fly (F) · brush 3 ([ ])`, `cell (256, 150) roof · ground 7.15 m · building 25.87 m · top face`, `0 undo (Ctrl-Z) · 0 redo (Ctrl-Y) · 0 saved (Ctrl-S)`, and the 0.5 m / 1 m note. The magenta crosshair sits at the view centre with the radius-3 disc outlined on the roof under it, foreshortened by the shallow look-down. No chart, no overlay row and no slider: there is no run behind this page to chart. |
| `17_edit_brush5.png` | Brush 5 on the lawn, the 49-cell disc outlined under the crosshair | **PASS.** Standing on the lawn at 10 m beside the east road and looking down at the grass: the asphalt street fills the foreground, a concrete walk crosses behind it, and the dome and the office blocks rise over the trees at the horizon. The radius-5 disc is unmistakable — 49 magenta cell outlines in the classic disc shape on the lawn at the crosshair. The sidebar reads `1 lawn` boxed, `brush 5`, `cell (140, 34) lawn · ground 5.24 m · building 0.00 m · top face`, and the footer gives the bundle: `256×256 m over 512×512 ground cells at 0.5 m · 81 trees, 64 shrubs, 4 pipes`. |

## Performance

`tests/e2e/perf.spec.ts` (runs in `npm test`) on `runs/s42` at tick 10000. Draw is 60 back-to-back renders of one view through `window.__ecoviewBench`, each completed with `gl.finish()` plus a one-pixel readback. Step is `__ecoviewGoto(tick)` → `__ecoviewReady` over ticks 10000–14900, which is what Play sees. The numbers are from this machine (win32-x64, Chromium 153.0.8010.12, ANGLE on Vulkan SwiftShader), re-measured in shot E1 against the regenerated run. CI's numbers are in `DECISIONS.md` and in the `ecoview-perf` artifact.

**These come from SwiftShader, a software GPU running on the CPU.** They measure the viewer's own cost and catch regressions. They are not the frame rate on a real graphics card, which would be far higher.

| Overlay | iso median / p95 ms | iso fps | top median / p95 ms | top fps |
| --- | --- | --- | --- | --- |
| material | 214.60 / 217.4 | 4.66 | 213.90 / 216.8 | 4.68 |
| light | 214.90 / 217.1 | 4.65 | 160.40 / 162.7 | 6.23 |
| moisture | 214.50 / 216.8 | 4.66 | 160.30 / 162.2 | 6.24 |
| fertility | 215.30 / 217.8 | 4.64 | 160.10 / 163.3 | 6.25 |
| temperature | 215.05 / 218.3 | 4.65 | 160.60 / 163.5 | 6.23 |
| fire | 215.95 / 219.3 | 4.63 | 160.20 / 162.5 | 6.24 |
| crowding | 215.00 / 218.6 | 4.65 | 160.60 / 162.7 | 6.23 |
| traits | 215.55 / 217.6 | 4.64 | 161.30 / 166.1 | 6.20 |

| Step (50 snapshots) median / p95 | Step fps | First load |
| --- | --- | --- |
| 324.9 / 358.9 ms | 3.08 | 449 ms |

Every figure is about 10% better than shot 16's, but this is not a like-for-like comparison: the run under it was regenerated with the water tier and holds 608 trees at tick 10000 where the old one held 813, so there are fewer entity boxes to draw. The gates are unchanged and unmoved — a median draw of at most 1000 ms per pair (250 ms per 4096 columns, shot 28's gate made per-column in shot 16) and a median step of at most 1000 ms.

## Films

`npm run film:check` and `npm run film:tiled:check` on the strip, both at 0.000% against `shots/reference/02_material_t10000_iso.png`, re-run in shot E1 against the regenerated run.

| Film | Frames | Size | Time | Budget |
| --- | --- | --- | --- | --- |
| `s42-material.mp4` (material:iso) | 201 | 960×832 | 77.5 s | 300 s |
| `s42-tiled.mp4` (2×2) | 201 | 1920×1632 | 324.7 s | 700 s |
