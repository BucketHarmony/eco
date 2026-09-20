# Screenshot verdicts

`npm run shot` produces 15 PNGs: 01–11 from `runs/s42` (seed 42), which since ecoview shot 16 is the reference world at defaults — the 256×64×32 strip with 8×8 patches, format 3 (ecosim shot 15) — and 12–15 from `runs/capitol-s42`, the Capitol bundle world at format 4 (ecoview shot G7), which are tabled separately below. Each verdict comes from viewing the PNG. These PNGs are also the visual-regression references in `shots/reference/`, re-accepted in shot 16 (`REACCEPT-16.md`). The top camera fits the whole strip, so the 4:1 world is letterboxed to rows 280–519 of `#view`, with the page background above and below it.

| File | Expectation | Verdict |
| --- | --- | --- |
| `01_material_t0_iso.png` | Heightmapped terrain, ponds, sparse trees, animals, no green yet | **PASS.** The strip runs diagonally across the view, with the west end at the upper left and the east end nearest the camera. It shows brown stepped terrain, gray rock outcrops in the west, two blue ponds, young trees, and yellow grazer and red hunter dots. Grass at 0.1 barely tints the soil. |
| `02_material_t10000_iso.png` | Many more trees, green ground | **PASS.** The dry west is light grass with scattered rock. The wet east is dense canopy around a pond. 813 trees, 515 of them mature. Animals show between the stands. |
| `03_light_t10000_top.png` | ≥5% of pixels dark, ≥40% light | **PASS.** Shade under the canopies is dark gray to black, densest in the forested east. Open ground is white. Measured over world pixels only (letterbox excluded): 16.2% dark, 69.3% light. |
| `04_moisture_t10000_top.png` | Wet halos around the ponds | **PASS, with a note.** Moisture runs from near white in the dry west to deep blue in the wet east, set by `climate.rain_gradient` 0.6. Ponds are water blue. Their halos are not distinct against the wet east, because rain, not the ponds, sets moisture across most of the strip. |
| `05_fertility_t10000_top.png` | Darker patches where litter decays | **PASS.** Light tan in the west. A dark brown block in the south-west, and generally darker ground in the east where tree litter decays. |
| `06_temperature_t1000_top.png` | Warm (summer) | **PASS.** Near-uniform red across the strip. |
| `07_temperature_t3000_top.png` | Cold (winter) | **PASS.** Uniform saturated blue; ponds show as the lighter water blue. |
| `08_chart_t20000.png` | Grazers oscillate, hunters lag, trees rise; four panels | **PASS.** Four panels. **Top:** grazers peak at 3421 at tick 3013 and then hold between 2803 and the peak. Hunters rise behind them, peak at 106 at tick 13345 and stay at 70 or more after tick 5000. **Second:** trees rise in a sawtooth to 1472 at tick 17100. **Third:** deaths per 100 ticks, dominated by purple `crowded` (35,567) over `eaten` (7,867), with smaller `old_age` (1,911), `starved` (791) and `burnt` (30). **Fourth:** orange spikes of patches burning (at most 9, at tick 13552); the blue mean grazer `energy_cost_mult` stays between 0.998 and 1.016. The marker is at the right edge. |
| `09_fire_t17100_top.png` | Tick with most patches burning (17100, 3 patches): burning orange, burnt charcoal, rest material | **PASS.** Three burning patches at the west end: patch 0 bright orange (3 ticks left), patch 3 mid orange (2) and patch 32 dark red-orange (1). Charcoal patches beside them are the burnouts since tick 17000 (27 events). The rest is the material colour with trunks and animals on top. |
| `10_crowding_t20000_top.png` | Grazers per patch, white → magenta | **PASS.** Mostly pink across the strip, with saturated magenta patches mostly in the grassy west, where grazers crowd. |
| `11_traits_t20000_top.png` | Grazers blue below the default cost, red above | **PASS.** The ground is the material colour and hunters are gray. Grazers are a near-even mix of blue, pale and red dots (1591 below 1, 1506 above), matching the series mean of 1.005. No species-yellow grazers are left. |

**Expectation wording.** The SAD's "grazer curve oscillating, hunter curve lagging it, tree curve rising" still holds, in a damped form on the larger world. Since ecosim shot 10, crowding mortality, not starvation, is the main cause of death, and the chart's third panel shows it. Burnt ground comes from `events.csv` burnouts since the previous snapshot (DECISIONS.md, shot 16).

## The Capitol (shots 12–15)

`runs/capitol-s42`: the committed `worlds/capitol` bundle over 256 m of the Michigan State Capitol grounds, run at seed 42 for 20000 ticks with `animals.enabled=false` and `climate.rain_gradient=0` (ecosim shot G3a). The world is square, so the top camera fills the view height and the world spans screen x 80–880 of `#view`. The `medium` overlay reads the 0.5 m ground grid through a draped texture, so it shows twice the detail of the 1 m voxel columns underneath it; its legend is the only overlay legend, which is why shots 01–11 are unchanged.

| File | Expectation | Verdict |
| --- | --- | --- |
| `12_capitol_medium_t0_iso.png` | Iso, `medium` at t=0: a large grey building block in the centre, asphalt bands along at least two sides | **PASS.** The Capitol reads as a building from the diagonal: the stepped wings, the drum and the ribbed dome all stand above a green lawn, with five office blocks along the north and east streets. Dark asphalt bands run the full length of all four edges, with the pale walks and the circular drive in front of the west steps drawn on the lawn between them. The legend lists all nine media. |
| `13_capitol_medium_t0_top.png` | Top, `medium` at t=0: the site plan, every medium its palette colour | **PASS.** Straight down it reads as the real grounds: lawn green over most of the site, the cross-shaped Capitol footprint in building grey, pale concrete walks radiating from it to the corners, dark asphalt streets framing all four sides, and dark tree beds scattered over the lawn. The four pipes are dashed blue lines inlet→outlet, drawn only on this camera. |
| `14_capitol_light_t0_top.png` | Top, `light` at t=0: the buildings' shade | **PASS.** Open ground is white at 255. Solid black blocks sit immediately north of the dome, of both wings and of the north office blocks — the columns those buildings shade all day. The building footprints themselves are the light grey of the extruded roofs; the small scattered black squares are the 79 young trees' own shade. |
| `15_capitol_material_t20000_iso.png` | Iso, `material` at t=20000: trees on the lawn, none on roof or road | **PASS.** 1982 trees, a closed canopy over the east and south lawns and a thinner stand in the north-west. Not one stands on a roof or in a street: the grey streets, walks and building footprints are bare, because the bundle lays them down as Rock and the sim only plants on Soil. Of the four ground cells under each trunk, 97.9% are lawn, 0.5% asphalt (trunks beside a walk edge) and none roof. |

## Performance

`tests/e2e/perf.spec.ts` (runs in `npm test`) on `runs/s42` at tick 10000, now the 256×64×32 strip. Draw is 60 back-to-back renders of one view through `window.__ecoviewBench`, each completed with `gl.finish()` plus a one-pixel readback. Step is `__ecoviewGoto(tick)` → `__ecoviewReady` over ticks 10000–14900, which is what Play sees. The numbers are from this machine (win32-x64, Chromium 153.0.8010.12, ANGLE on Vulkan SwiftShader). CI's numbers are in `DECISIONS.md` and in the `ecoview-perf` artifact.

**These come from SwiftShader, a software GPU running on the CPU.** They measure the viewer's own cost and catch regressions. They are not the frame rate on a real graphics card, which would be far higher.

| Overlay | iso median / p95 ms | iso fps | top median / p95 ms | top fps |
| --- | --- | --- | --- | --- |
| material | 239.80 / 244.3 | 4.17 | 238.95 / 241.4 | 4.18 |
| light | 240.85 / 243.9 | 4.15 | 165.20 / 168.7 | 6.05 |
| moisture | 240.60 / 243.1 | 4.16 | 164.70 / 169.2 | 6.07 |
| fertility | 240.80 / 242.6 | 4.15 | 164.75 / 168.1 | 6.07 |
| temperature | 240.80 / 245.7 | 4.15 | 165.05 / 168.6 | 6.06 |
| fire | 241.35 / 244.5 | 4.14 | 164.85 / 169.1 | 6.07 |
| crowding | 241.25 / 244.5 | 4.15 | 164.55 / 168.3 | 6.08 |
| traits | 241.90 / 244.3 | 4.13 | 165.30 / 168.9 | 6.05 |

| Step (50 snapshots) median / p95 | Step fps | First load |
| --- | --- | --- |
| 356.8 / 394.7 ms | 2.80 | 563 ms |

The strip has 4× the surface columns of the old 64×64 world, and the draw cost rose by about the same factor. The gates are a median draw of at most 1000 ms per pair (250 ms per 4096 columns, shot 28's gate made per-column in shot 16) and a median step of at most 1000 ms.

## Films

`npm run film:check` and `npm run film:tiled:check` on the strip, both at 0.000% against `shots/reference/02_material_t10000_iso.png`.

| Film | Frames | Size | Time | Budget |
| --- | --- | --- | --- | --- |
| `s42-material.mp4` (material:iso) | 201 | 960×832 | 77.5 s | 300 s |
| `s42-tiled.mp4` (2×2) | 201 | 1920×1632 | 315.8 s | 700 s |
