# Shot G4: storms, runoff and soil water

The sweep is the mechanism's main parameter, `rain.storm_mean_mm`, at 5, 10, 20 and 40 mm, each with
`rain.storm_p` set so that the long-run rainfall is the same 1 mm per tick in every cell:

```
ecosim sweep --param rain.storm_mean_mm --values 5  --set rain.storm_p=0.2   --seeds 1,2,3 --ticks 20000 --jobs 6 --out sweeps/shotG4/mean5
ecosim sweep --param rain.storm_mean_mm --values 10 --set rain.storm_p=0.1   --seeds 1,2,3 --ticks 20000 --jobs 6 --out sweeps/shotG4/mean10
ecosim sweep --param rain.storm_mean_mm --values 20 --set rain.storm_p=0.05  --seeds 1,2,3 --ticks 20000 --jobs 6 --out sweeps/shotG4/mean20
ecosim sweep --param rain.storm_mean_mm --values 40 --set rain.storm_p=0.025 --seeds 1,2,3 --ticks 20000 --jobs 6 --out sweeps/shotG4/mean40
```

Four invocations rather than one grid, because the two parameters have to move together to hold the
total constant, and the sweep harness takes a cartesian product. Wall time 52.3 s, 51.0 s, 50.4 s and
54.4 s on 6 jobs; 5 min 2 s for all four, with the Capitol runs in between.

The same four storm sizes were also run on the Capitol (256 m, seed 42, animals off, flat rainfall,
20000 ticks) into `target/g4/cap*`, which is not committed. The tables below and `ponding.png` come
from `analyse.py` in this directory, which regenerates everything it needs.

## Event-log cause breakdown, seeds 1, 2, 3 and 42

Deaths by cause from `events.csv`, over 20000 ticks at the defaults on the reference strip. "off" is
the same seed with `--set hydro.enabled=false`, which is the pre-G4 moisture model byte for byte, so
the two rows of a pair are exactly what the water tier changed.

| seed | water | species | deaths | causes |
|---|---|---|---|---|
| 1 | off | grazer | 53204 | crowded 40661, eaten 11570, starved 570, old_age 401, burnt 2 |
| 1 | on | grazer | 52831 | crowded 39597, eaten 12171, starved 843, old_age 220 |
| 1 | off | hunter | 434 | crowded 281, old_age 110, starved 43 |
| 1 | on | hunter | 447 | crowded 287, old_age 125, starved 35 |
| 1 | off | tree | 3366 | crowded 1590, drought 833, old_age 560, burnt 383 |
| 1 | on | tree | 3032 | crowded 1544, drought 928, old_age 559, burnt 1 |
| 2 | off | grazer | 57592 | crowded 44449, eaten 11677, starved 800, old_age 633, burnt 33 |
| 2 | on | grazer | 59412 | crowded 47398, eaten 11054, old_age 558, starved 402 |
| 2 | off | hunter | 399 | crowded 288, old_age 109, starved 2 |
| 2 | on | hunter | 391 | crowded 282, old_age 105, starved 4 |
| 2 | off | tree | 2934 | crowded 1599, old_age 585, drought 536, burnt 214 |
| 2 | on | tree | 2958 | crowded 1763, old_age 690, drought 448, burnt 57 |
| 3 | off | grazer | 47469 | crowded 35256, eaten 9161, old_age 1652, starved 1339, burnt 61 |
| 3 | on | grazer | 45483 | crowded 34633, eaten 8495, old_age 2055, starved 300 |
| 3 | off | hunter | 353 | crowded 273, old_age 68, starved 12 |
| 3 | on | hunter | 307 | crowded 222, old_age 77, starved 8 |
| 3 | off | tree | 3212 | crowded 1831, old_age 718, drought 494, burnt 169 |
| 3 | on | tree | 3832 | drought 2136, crowded 1276, old_age 412, burnt 8 |
| 42 | off | grazer | 45865 | crowded 35333, eaten 7867, old_age 1844, starved 791, burnt 30 |
| 42 | on | grazer | 47212 | crowded 36276, eaten 8805, old_age 1733, starved 398 |
| 42 | off | hunter | 301 | crowded 234, old_age 67 |
| 42 | on | hunter | 318 | crowded 242, old_age 76 |
| 42 | off | tree | 3882 | crowded 2128, old_age 713, drought 664, burnt 377 |
| 42 | on | tree | 4329 | crowded 2115, drought 1587, old_age 620, burnt 7 |

- **No extinctions on any of the eight runs.** Seeds 1, 2, 3 and 42 all pass `ecosim check` 9/9 at
  20000 ticks with the tier on, which is the regression anchor.
- **Fire nearly stops.** Burnt tree deaths fall from 383, 214, 169 and 377 to 1, 57, 8 and 7. Ignition
  is proportional to the square of dryness, and the tier holds the mean moisture near saturation
  (234 of 255 over 200000 ticks on seed 42, against 208 before), so the ignition probability drops by
  about 25x. This is a consequence of the rainfall calibration, not of the fire model; see the note
  at the end.
- **Drought deaths go up, not down, on three of the four seeds** (833 -> 928, 494 -> 2136, 664 ->
  1587), even though the world is wetter on average. Rain now arrives in storms with dry gaps between
  them, and there is no lateral flow between soil columns, so a column under a tree draws itself down
  between storms instead of being refilled by diffusion from its neighbours. Wetter on average, drier
  where it matters: this is the single biggest behavioural change of the shot.
- **Grazers starve less on three seeds of four** (1339 -> 300, 800 -> 402, 791 -> 398; seed 1 goes the
  other way, 570 -> 843) and crowding stays the dominant cause everywhere. The animal tier is
  otherwise untouched.
- **Storms are logged.** 2101, 2067, 2048 and 2117 `storm` rows on the four seeds, mean depth 10.08,
  10.21, 9.65 and 10.12 mm against a parameter of 10.0, largest 74.1, 86.9, 64.7 and 82.6 mm.

## The sweep: storm size at a fixed total

| `rain.storm_mean_mm` | `rain.storm_p` | cells passing | first failing invariants | extinctions |
|---|---|---|---|---|
| 5 | 0.2 | 3/3 | — | none |
| 10 (default) | 0.1 | 3/3 | — | none |
| 20 | 0.05 | 2/3 | `no_extinction`, `tree_growth`, `mature_trees_10k` (seed 1) | trees at tick 1450, seed 1 |
| 40 | 0.025 | 0/3 | `no_extinction`, `tree_growth`, `mature_trees_10k` (seeds 1, 2, 3) | trees at 1700, 1700, 9850 |

- **The failures are contiguous in the parameter and in the seeds.** Every cell at 5 and 10 mm passes;
  one of three fails at 20 mm; all three fail at 40 mm. There is no seed that fails at a small storm
  size and passes at a large one. The safe band is 5-10 mm and the default sits at its edge, which is
  recorded in TUNING.md.
- **Everything that dies is trees, and it dies early.** Tree death is not attributed by cause in the
  event log (`tree_death` rows carry the cause, but a species that reaches 0 is reported by the
  dominant cause over the 500 ticks before, which for trees the sweep reports as `unrecorded`). Reading
  the event log of seed 1 at 40 mm directly, 39 of the 40 tree deaths before tick 2000 are `drought`
  and the fortieth is `burnt`: the 12 starting trees and the first generations of saplings sit through
  gaps of 40 ticks and more between storms, and a sapling's water need is checked every 50 ticks. Once the trees are gone the grass keeps going, which is why the
  animal invariants still pass in the failing cells.
- **The same happens on the Capitol.** Trees at tick 20000: 956, 1324, 668 and 0 at 5, 10, 20 and
  40 mm. The site loses its trees at 40 mm exactly as the strip does.

## Where the water goes

Flat single-medium worlds (32 m, one medium, no slope), so `runoff_mm / rain_mm` is exactly the
fraction of a storm that the surface sheds, with no run-on from anywhere uphill. 20000 ticks each.

| surface | storm mean (mm) | runoff fraction | of storms above the mean | soil water at the end (mm) |
|---|---|---|---|---|
| lawn | 5 | 0.051 | 0.063 | 148.0 |
| lawn | 10 | 0.207 | 0.272 | 143.9 |
| lawn | 20 | 0.485 | 0.639 | 147.9 |
| lawn | 40 | 0.710 | 0.935 | 124.6 |
| street (asphalt) | 5 | 1.000 | 1.000 | 0.0 |
| street (asphalt) | 10 | 1.000 | 1.000 | 0.0 |
| street (asphalt) | 20 | 1.000 | 1.000 | 0.0 |
| street (asphalt) | 40 | 1.000 | 1.000 | 0.0 |
| roof | 5 | 1.000 | 1.000 | 0.0 |
| roof | 10 | 1.000 | 1.000 | 0.0 |
| roof | 20 | 1.000 | 1.000 | 0.0 |
| roof | 40 | 1.000 | 1.000 | 0.0 |

Asphalt at 0.5 mm/h and roof at 0 mm/h shed everything at every storm size, as the table of media
intends. Lawn at 15 mm/h can take 32.9 mm in one tick (one tick is 2.19 hours), so it sheds nothing
up to about 33 mm and the fraction above that is the tail of the exponential: 5% of the rain at a
5 mm mean, 71% at a 40 mm mean. The lawn's soil sits near its 150 mm field capacity in every case,
which is why the fraction is set by the infiltration rate rather than by storage.

On the whole Capitol site, with its roofs, walks, streets and slopes:

| storm mean (mm) | storms | rain (mm) | runoff | outflow | drainage (mm) | largest storm |
|---|---|---|---|---|---|---|
| 5 | 3994 | 19657 | 40.3% | 28.7% | 10668 | 39.5 mm @ 19186 |
| 10 | 2014 | 20110 | 48.3% | 36.1% | 9493 | 68.4 mm @ 2117 |
| 20 | 1018 | 21353 | 61.5% | 47.6% | 7912 | 131.5 mm @ 19362 |
| 40 | 486 | 20775 | 76.0% | 61.8% | 4857 | 221.6 mm @ 9465 |

Runoff is the share of the rain that left the cell it fell on; outflow is the share that left the
site altogether. The gap between the two is run-on that soaked in or ponded further downhill, and it
is between 11 and 14 points at every storm size. Deep drainage falls by half from the smallest storms
to the largest: big storms leave less water in the soil to percolate, because more of it never gets
in.

## Ponding after the largest storm

The largest storm in the four Capitol runs is 221.6 mm at tick 9465 of the 40 mm run — a once-in-a-run
event, about 8.7 inches. `ponding.png` is the site from above just after it: grey by medium (dark grey
roofs, near-black asphalt, light grey walks), ponded water in blue, deepening to bright blue at 50 mm
and over. North is up, the site is 256 m across.

Ponded volume over the whole site: 1323.8 m3 in 47384 of 262144 ground cells.

| rank | volume (m3) | cells | deepest (mm) | at (m) | medium |
|---|---|---|---|---|---|
| 1 | 977.24 | 5989 | 5324 | (116.5, 51.5) | lawn |
| 2 | 90.59 | 1201 | 1137 | (91.5, 186.0) | lawn |
| 3 | 31.47 | 1959 | 189 | (129.5, 216.0) | lawn |
| 4 | 17.33 | 476 | 479 | (108.0, 94.5) | lawn |
| 5 | 12.07 | 1121 | 206 | (156.5, 94.5) | lawn |

- **One basin holds three quarters of the standing water.** The top depression is a closed bowl in the
  scanned terrain south of the building, 51 m by 45 m, whose floor is 5.4 m below its rim. The D8
  receivers send the whole south lawn into it and the priority-flood fill gives it one spill level, so
  it fills before anything leaves. 5.3 m of standing water is not a thing that happens on the real
  Capitol lawn; what the model is saying is that this basin has no outlet at all, which is true of the
  model until shot G6 gives the site its storm drains. The four `pipes.json` inlets are already in the
  bundle and are still ignored.
- **All five are on lawn**, because the impervious media shed their water rather than holding it, and
  the places it collects are the soft low ground.
- **Ponding is common but mostly shallow.** 18% of the site has standing water after this storm. The
  median wet cell holds 17 mm and the ninetieth percentile 189 mm, while the mean is 112 mm: the
  volume is concentrated in the one basin.

## Lawn cover in wet and dry ground

Patches whose columns are mostly lawn (683 of them on the Capitol), sorted by the mean soil water of
those columns at tick 20000 of the default 10 mm run, split into quarters:

| quarter | patches | soil water (mm) | grass | shrub | trees |
|---|---|---|---|---|---|
| driest quarter | 170 | 140.0 | 0.545 | 0.489 | 1137 |
| wettest quarter | 170 | 146.4 | 0.671 | 0.017 | 0 |

The spread across the lawn is small (140 to 146 mm, against a 150 mm field capacity), and the cover
difference is not mostly about the rain: the driest quarter is dry **because** it is the wooded
quarter. All 1137 trees on the lawn are in it, each drawing water from its own column every 50 ticks,
and shrubs are 30x denser there because the shade suits them. Grass is 23% thinner under the trees.
Read the other way round, the model has no lawn so dry that grass fails on it; at the default storm
size the Capitol lawn is wet everywhere.

## Performance

One storm pass over the 512 x 512 ground grid costs **2.85 ms**: 2000 ticks of the Capitol with a
storm every tick take 6670 ms against 962 ms with no storms at all. At the default `storm_p` of 0.1 a
storm falls on one tick in ten, so the tier adds about 0.3 ms per tick, next to a Capitol tick's
roughly 1 ms. The flow graph — the filled surface, the receivers, the topological order, the
downspouts and the depression capacities — is built once when the world loads and never again.

The full 20000-tick Capitol run takes 22.5 s at the default storm size; the reference strip takes
39-52 s on seeds 1, 2, 3 and 42, inside the 90 s that `ecosim check` allows.

## Two honest notes

**The rainfall is about five times Michigan's.** `storm_p = 0.1` at a 10 mm mean over a 4000-tick year
is roughly 4000 mm a year, against about 800 mm at Lansing. The reason is that the plant water draws
are pre-G4 numbers in arbitrary units — a tree takes 50 units of the 0-255 moisture scale every 50
ticks and ground cover takes 15 x growth every 10 — and those are far larger than real transpiration.
Calibrating the rain down to a real year with those draws unchanged kills the trees; G4's acceptance
allows moving only `rain.*` and `hydro.*`, so the rain was calibrated to the draws rather than the
draws to the rain. This is the thing to fix in a later tuning shot, and until it is fixed the absolute
depths in `series.csv` are model millimetres, not Lansing millimetres. The relative behaviour — which
surfaces shed, where water collects, how storm size changes the split — does not depend on it.

**Fertility no longer saturates.** The operator's note for this shot pointed out that `fertility_mean`
runs to its 255 ceiling in every long run and always has (`runs/long42` reaches 255.0 by tick 53100),
and that `check --long` never tested it. Percolation now leaches fertility out of the soil with the
water that drains past the roots (`hydro.leach_k`), which gives fertility its first sink, and
`check --long` has a `fertility_mean in [40, 220] over the whole run` line. Over **200000 ticks** (50
simulated years) of seed 42 at the defaults, `fertility_mean` peaks at **128.0, which is its value at
tick 0**, bottoms at 46.9 at tick 39899 and averages 65.6; the last 20000 ticks average 66.7. It rises
to nothing and settles. `ecosim check --long` passes all three of its lines on that run.
