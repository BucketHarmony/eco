# Shot G4c — units calibration, part two: what the conversion changed

Two subsystems converted: **light** (a 0–255 irradiance index → a fraction of full sun, attenuated by
Beer–Lambert extinction) and **tree demography** (tick counts → years, days and a rate per year).
Every converted value is arithmetically neutral at the shipped `year_len = 4000`. What moved the
reference runs is the two **rules** that came out of the conversion, and they moved different worlds:

- `255 − absorb × layers` became `255 × exp(−k · LAI · layers)`. That moves every world, and it is the
  whole of what moved the strip.
- Seeding stopped being a schedule that could only fire at ages divisible by `tree.update_every`
  (section 2, "the rule found to be wrong"). On the strip every tree's age is a multiple of 50 by
  construction, so nothing there changes; on a bundle world, where a tree is imported at an age
  taken from its height, almost everything does. 63 of the Capitol's 79 imported trees could not
  seed at all before this shot, and the Capitol column below is the only place that shows.

Five reference runs, all at the committed defaults: the 256 × 64 strip at seeds 1, 2, 3 and 42
(20000 ticks, snapshot every 100) and the Capitol bundle at seed 42 (20000 ticks, snapshot every
1000, animals off, no rain gradient). "Before" is the same binary built from the commit this shot
starts at; both sets are reproducible with `ecosim run` and `ecosim check`.

## 1. The event-log cause breakdown, per species, on the reference seeds

Counts from each run's `events.csv`, before → after. The reporting rule asks for this first, and on
this shot it is also the clearest statement of what happened: the trees won, and the ground cover and
the grazers paid for it.

### Trees

| seed | germination | death: crowded | death: drought | death: old age | death: burnt |
|---|---|---|---|---|---|
| 1 | 2107 → **5097** | 639 → 2020 | 605 → 1071 | 252 → 698 | 53 → 122 |
| 2 | 3709 → **4245** | 1389 → 1633 | 704 → 763 | 532 → 595 | 63 → 37 |
| 3 | 4447 → **4720** | 1456 → 1871 | 1016 → 912 | 563 → 707 | 303 → 71 |
| 42 | 3027 → **3953** | 1456 → 1919 | 150 → 140 | 611 → 741 | 29 → 42 |
| Capitol | 7645 → **19408** | 1471 → 3638 | 3331 → 9020 | 410 → 1136 | 893 → 1611 |

### Grazers

| seed | births | death: crowded | death: eaten | death: starved | death: old age |
|---|---|---|---|---|---|
| 1 | 53106 → 53943 | 38779 → 40947 | 12366 → 10313 | 319 → **1261** | 324 → 356 |
| 2 | 61468 → 59508 | 46699 → 44872 | 11852 → 10716 | 272 → **1532** | 625 → 667 |
| 3 | 51058 → 49241 | 37496 → 35563 | 9164 → 8019 | 391 → **1513** | 1517 → 1958 |
| 42 | 49366 → 55425 | 36879 → 40993 | 7914 → 9710 | 253 → **1644** | 1566 → 1062 |

### Hunters

| seed | births | death: crowded | death: starved | death: old age |
|---|---|---|---|---|
| 1 | 551 → 493 | 280 → 266 | 56 → 26 | 119 → 124 |
| 2 | 551 → 495 | 291 → 288 | 2 → 0 | 110 → 106 |
| 3 | 414 → 357 | 258 → 225 | 10 → 9 | 80 → 71 |
| 42 | 358 → 432 | 209 → 259 | 0 → 1 | 67 → 82 |

The animal tier is untouched by this shot. Its numbers move because the grass it eats moves.

### Fire and weather

| seed | storms | ignitions | spreads | burnouts |
|---|---|---|---|---|
| 1 | 653 → 685 | 28 → 25 | 22 → 26 | 50 → 51 |
| 2 | 661 → 674 | 33 → 30 | 32 → 32 | 65 → 62 |
| 3 | 703 → 670 | 33 → 35 | 78 → 46 | 111 → 81 |
| 42 | 695 → 648 | 26 → 23 | 26 → 31 | 52 → 54 |
| Capitol | 650 → 645 | **104 → 148** | **233 → 452** | **337 → 600** |

Storms are drawn from the rain parameters and their rate is unchanged; the small differences are the
RNG stream diverging once germination does. On the strip fire barely moves. On the Capitol it
roughly doubles, and that is the seeding fix rather than the light: patch fuel is
`grass·0.5 + shrub + detritus·detritus_weight + canopy_fraction·canopy_weight` (`src/fire.rs`), and a
run that ends with 4082 trees instead of 1619 carries more canopy over its patches and more litter
from their deaths. It is the largest single consequence of this shot anywhere in the reference set,
and it lands on the subsystem whose direction is held for a human in backlog row G4d.

## 2. Subsystem 1 — light

### What changed in the rule

| canopy voxels above | before (index of 255) | after | as a fraction of full sun |
|---|---|---|---|
| 0 (open) | 255 | 255 | 1.000 → 1.000 |
| 1 (a young tree) | 155 | 94 | **0.608 → 0.368** |
| 2 (a mature crown) | 55 | 35 | 0.216 → **0.135** |
| 3 | 0 | 13 | **0.000 → 0.051** |
| 4 | 0 | 5 | 0.000 → 0.019 |

The acceptance line is the two-layer row: a mature sim crown is LAI 4 at `canopy_lai = 2.0`, and
13.5% transmittance at LAI 4 is inside R10's published 10–25% at LAI 3–5; `canopy_k = 0.5` is inside
R10's 0.4–0.7. The old 21.6% was also inside the band, which is worth saying plainly: **the shot's
acceptance line was already satisfied by the old rule at two layers.** What the old rule could not do
is one layer and three.

### The two rows that matter, and why the world got woodier

- **One layer, 60.8% → 36.8%.** A young tree used to cast almost no shade. It now casts real shade,
  and a tree's own light curve tolerates it (suitability 0.38) where grass's does not: `grass.light`
  reaches zero at 39.2% of full sun, so grass under a young canopy now stops growing altogether. The
  tree seedlings that used to lose the column to that grass now win it.
- **Three layers, 0% → 5.1%.** The old subtraction saturated, so deep canopy was not dark but black,
  and *every* suitability curve read exactly 0. Beer–Lambert never reaches 0. This is `UNITS.md`
  finding 11, and it is why light is a product and not a difference.

### What it did to the reference runs

| seed | trees at 20000 | peak trees | mature at 2.5 yr (floor 35) | min grass_mean | min moisture index | runtime ms |
|---|---|---|---|---|---|---|
| 1 | 570 → **1198** | 704 → 1547 | 38 → **701** | 0.3802 → 0.1845 | 24.96 → 79.27 | 40382 → 31934 |
| 2 | 1033 → **1229** | 1122 → 1346 | 232 → 479 | 0.3052 → 0.2260 | 73.02 → 73.17 | 59840 → 37994 |
| 3 | 1121 → **1171** | 1376 → 1461 | 466 → 527 | 0.3182 → 0.2424 | 62.56 → 71.94 | 53802 → 53346 |
| 42 | 793 → **1123** | 1065 → 1344 | 413 → 503 | 0.3027 → 0.2441 | 46.53 → 50.89 | 40879 → 43356 |
| Capitol | 1619 → **4082** | 2313 → 4461 | 944 → **325** | 0.4684 → 0.4195 | 71.07 → 66.13 | 16690 → 14768 |

All five pass every `ecosim check` invariant, before and after. Three things are worth reading off
this table:

1. **The thinnest margin in the G4b set is gone.** Seed 1's `mature_trees_10k` was 38 against a
   floor of 35 — margin +0.0857, and `sweeps/shotG4b/FINDINGS.md` named it as the number to watch.
   It is now 701. No invariant was touched to achieve that; the shade did it.
2. **Soil water rises where the grass thins.** Seed 1's minimum moisture index goes 24.96 → 79.27,
   because ground cover is what transpires most of the strip's water and there is less of it under
   the new canopy. The Capitol goes the other way (71.07 → 66.13) because there the water freed up
   goes into 2463 more trees, which draw more per column than the grass they replaced.
4. **The Capitol's tree count now oscillates, and one number falls because of it.** With 79 seeding
   trees instead of 16 it reaches 2310 by tick 5000, falls to 998 by tick 10000 as crowding and
   drought catch up (crowded deaths 1471 → 3638, drought 3331 → 9020), and recovers to 4082.
   `mature_trees_10k` is read at the bottom of that trough, which is why it falls 944 → 325 while
   every other Capitol tree number rises. It is still 9× the floor of 35, and the peak of 4461 is
   still well inside the 10× anchor limit of 23100.
3. **Nothing approaches a ceiling.** `fertility_mean` stays inside [40, 220] with its band barely
   moving (seed 42: [67.05, 114.07] → [78.59, 114.10]), and `grass_mean` stays inside [0.05, 0.95]
   with 0.1845 the lowest value anywhere — still nearly four times the floor.

### The rule found to be wrong rather than mis-scaled

Seeding. A mature tree seeded when `age % seed_every == 0`, and a tree's age advances by whole
`tree.update_every` steps, so the test can only ever fire when `update_every` divides `seed_every`.
It does at the shipped 50 and 200, which is why nothing had noticed. Turning the schedule into
`tree.seeds_per_year` makes values that break it reachable — `seeds_per_year = 30` gives
`seed_every = 133`, whose first common multiple with 50 is 6650, so a tree would seed 50× less often
than asked. The test is now "the update whose age crosses a multiple of `seed_every`", which picks
exactly the same ages at the shipped values (asserted by
`trees::tests::a_mature_tree_seeds_at_its_rate_whatever_the_rate_is`) and the right ones everywhere
else. `UNITS.md` finding 12.

"The same ages at the shipped values" holds only for a tree whose age *starts* at a multiple of
`tree.update_every`, because stepping by 50 preserves the residue. That is every tree on the strip:
germinated trees start at 0 and the initial ones at `initial_age` = 500. It is not every tree on a
bundle world, where `plants::import_age` turns a height into an arbitrary age — 63 of the Capitol's
79 imported trees have an age that is not a multiple of 50, and no such tree can ever satisfy
`age % 200 == 0`. Those 63 never seeded in any run before this shot. That is why the Capitol's
germination count nearly triples (7645 → 19408) while the strip's rises by a fifth, and it makes
this fix, not the light rule, the largest behaviour change the shot makes to a bundle world.

## 3. Subsystem 2 — tree demography

Nothing to measure in the conversion itself — the seeding *rule* that came out of it does move the
Capitol, and that is in section 2. `Params::tree_ages()` returns exactly 500, 500, 1000, 6000, 500 and 200 ticks at
`year_len = 4000`, which are the six tick counts it replaces, and `bundle.tree_tall_age_years = 0.75`
returns 3000. `bundle.sun_altitude_deg = 45.0` returns a shade slope of 1.0 to the bit. The
conversion moves no run on its own.

What it does is make the model's largest error legible without running anything: `params.toml` now
says a tree matures at **0.25 years** and dies at **1.5**, against a published 5–8 years to 3 m and a
60–150 year lifespan (R12). Shot G4b found this and could not fix it; shot G4c could not fix it
either, and `UNITS.md` section 7 now records which of the three ways out was rejected and why, as the
shot's acceptance line allows in place of the height-by-age claim. The short form: the 200000-tick
run fails the runtime invariant, which may not be widened; per-tier time scales is a mechanism, which
this series does not add; and a day-long tick would undo the water tier that is the only physically
calibrated part of the model. A fourth obstacle, independent of all three: **the model's trees have
no height**, so there is no metre to compare with R12 even at the right ages.

## 4. Subsystems not reached

Three of the prompt's five, plus the parked tier, are handed to
`overnight/shots/G4e-units-calibration-rest2.md`: the fertility and detritus indices (with
`climate.decay_k`'s 10× error riding along), the animal tier and its six `SIG_*` constants, and the
legacy non-water moisture model, which is to be left alone and said so again. Every one of their rows
is still marked `deferred` in `UNITS.md` section 6, and section 6's heading now names G4e.
