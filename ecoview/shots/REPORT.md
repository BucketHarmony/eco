# Screenshot verdicts

`npm run shot` against `runs/s42` (seed 42): 8 PNGs in 1.8 s. Each verdict comes from viewing the PNG.

| File | Verdict |
| --- | --- |
| `01_material_t0_iso.png` | **PASS.** Heightmapped brown terrain with stepped contours, two rock outcrops, and blue ponds in the low south and west corners. There are 12 sparse young trees and scattered yellow/red dots. With grass at 0.1 the soil tint is barely visible, so it reads as no green yet. |
| `02_material_t10000_iso.png` | **PASS.** Far more trees, with dense canopy clusters. Most soil is tinted green. Darker olive bands mark high-shrub patches, and there are many yellow grazer and red hunter dots between the stands. |
| `03_light_t10000_top.png` | **PASS.** Dark gray (55) and black (overlapping canopies) squares sit under every tree cluster, with lighter gray under young canopies and white everywhere else. Measured: 20.6% of pixels below 60 and 66.5% above 200. |
| `04_moisture_t10000_top.png` | **PASS.** Deep blue halos ring the ponds in the south-east and south-west corners and fade outward to pale blue. Rock outcrops are gray. |
| `05_fertility_t10000_top.png` | **PASS.** Light tan background with a clearly darker brown patch in the south-west (heavy decay) and several moderately darker patches around the tree clusters. |
| `06_temperature_t1000_top.png` | **PASS.** Near-uniform warm red (summer peak, about 24 °C). A few patches with more canopy are a faint shade cooler (slightly more magenta). The difference is subtle, as expected for a 3 °C × canopy-fraction effect. |
| `07_temperature_t3000_top.png` | **PASS.** Uniform saturated blue (winter trough at or below 0 °C, clamped). Ponds show as the distinct water blue. |
| `08_chart_t20000.png` | **PASS.** The chart shows grazers (yellow) oscillating through about 4 peaks, and hunters (red) peaking after each grazer peak (lagging). Trees (brown) rise from 12 to a sawtooth of 400–650, with die-back as cohorts reach age 6000. The overall trend rises. The marker is at the right edge, at tick 20000. |
