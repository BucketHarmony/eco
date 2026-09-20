# Shot G4b — units and time calibration: what the conversion moved

The audit itself is `ecosim/UNITS.md` (one row per parameter and unit-bearing constant, committed
before any conversion). The judgement record is `ecosim/DECISIONS.md`, "Shot G4b — units
calibration", and every default that moved is in `ecosim/TUNING.md` with the reference it was set
against. This file is the measurement: what happened to the reference runs.

**There is no parameter sweep in this directory.** The sim-shot rules, including the one-sweep rule,
are suspended for this shot by operator override 1 (`overnight/shots/G4b-units-calibration.md`): the
shot adds no mechanism, so there is no new mechanism's main parameter to sweep. What replaces it is a
before/after on every reference run, which is what the rest of this file is.

Every "before" below is the same run directory built at commit `b22ffc8` (shot G4, the state this
shot started from); every "after" is built at this shot's commit. Both are 20000 ticks unless the row
says otherwise, and both use the same seeds, world and overrides.

## 1. Event-log cause breakdown, before and after

Reference seeds 1, 2 and 3 on the 256 x 64 strip at the shipped defaults, plus the Capitol at
`animals.enabled=false` and `climate.rain_gradient=0`. Counts are whole-run totals from `events.csv`.

### Trees

| run | germination | drought | crowded | old age | burnt |
|---|---|---|---|---|---|
| s1 | 3908 → **2107** | 928 → **605** | 1544 → **639** | 559 → **252** | 1 → **53** |
| s2 | 4031 → **3709** | 448 → **704** | 1763 → **1389** | 690 → **532** | 57 → **63** |
| s3 | 4552 → **4447** | 2136 → **1016** | 1276 → **1456** | 412 → **563** | 8 → **303** |
| capitol | 3704 → **7645** | 1804 → **3331** | 551 → **1471** | 95 → **410** | 9 → **893** |
| long-s1 (60000) | 14851 → **13986** | 5423 → **3809** | 6083 → **5600** | 2433 → **2540** | 60 → **886** |

Drought is the cause to read first, because it is the one the water conversion acts on directly. On
the strip it falls on two seeds of three (s1 −35%, s3 −52%) and rises on one (s2 +57%), and on the
60000-tick run it falls 30%. **Rain fell by a factor of five and the trees are less thirsty, not
more**, because the tree draw fell further than the rain did: 2352 mm a year to 300, against 4000 mm
of rain to 800 (`UNITS.md` section 3.3). The Capitol is the exception and it is explained by fire,
not by water — see section 4.

Germination on seed 1 halves, and that is the mechanism behind the shot's thinnest margin: seed 1
ends with 38 mature trees at 2.5 years against the check's 35 (margin +0.0857). Seeds 2 and 3 are at
232 and 466. The spread across seeds is much wider after the conversion than before it, because the
site is no longer so wet that every column germinates.

### Grazers

| run | births | starved | eaten | crowded | old age | burnt |
|---|---|---|---|---|---|---|
| s1 | 53995 → **53106** | 843 → **319** | 12171 → **12366** | 39597 → **38779** | 220 → **324** | 0 → 0 |
| s2 | 61633 → **61468** | 402 → **272** | 11054 → **11852** | 47398 → **46699** | 558 → **625** | 0 → **2** |
| s3 | 47918 → **51058** | 300 → **391** | 8495 → **9164** | 34633 → **37496** | 2055 → **1517** | 0 → **2** |
| long-s1 | 137995 → **130101** | 6724 → **2261** | 36731 → **38539** | 93624 → **86754** | 313 → **1154** | 1 → 0 |

The animal tier is **not converted by this shot** and no animal parameter moved. It sees the change
only through its food: starvation falls by a third to two thirds on three of the four runs because
`grass_mean` holds or rises, and crowding stays overwhelmingly the dominant cause, as it was before.
The Capitol runs with animals off and has no rows here.

### Hunters

| run | births | starved | eaten | crowded | old age |
|---|---|---|---|---|---|
| s1 | 575 → **551** | 35 → **56** | 0 → 0 | 287 → **280** | 125 → **119** |
| s2 | 523 → **551** | 4 → **2** | 0 → 0 | 282 → **291** | 105 → **110** |
| s3 | 376 → **414** | 8 → **10** | 0 → 0 | 222 → **258** | 77 → **80** |
| long-s1 | 1732 → **1701** | 68 → **56** | 0 → 0 | 1201 → **1191** | 382 → **376** |

Hunters barely move at all, which is the expected result for a tier the shot does not touch, two
trophic levels from the water.

## 2. Water, in real units for the first time

Annual means over the check's window (from 0.5 years to the end of the run), in millimetres. `ET +
evap` is the residual of the closed ledger — `rain − drainage − outflow − Δstore` — because
evapotranspiration is not a `series.csv` column.

| run | rain | runoff | drainage | outflow | ET + evap |
|---|---|---|---|---|---|
| s1 | 4234 → **835** | 1871 → **259** | 2130 → **368** | 1349 → **183** | — → **276** |
| s2 | 4221 → **787** | 1766 → **227** | 2042 → **291** | 1391 → **188** | — → **294** |
| s3 | 3955 → **860** | 1421 → **201** | 2045 → **382** | 1055 → **156** | — → **319** |
| s42 | 4286 → **887** | 1157 → **83** | 2640 → **483** | 666 → **39** | — → **349** |
| capitol | 4022 → **799** | 1944 → **295** | 1899 → **327** | 1453 → **179** | — → **281** |
| long-s1 (60000) | 4118 → **823** | 1831 → **256** | 2029 → **348** | 1325 → **181** | — → **291** |
| capitol, 50 years | — | **812** | **326** | **185** | **300** |

**Acceptance (A), annual rainfall in the site's normal range: passes.** Lansing's normal is about
800 mm (`UNITS.md` R1). Five reference runs land at 787–887 mm, and the 50-year Capitol run — the
longest sample, and therefore the one closest to the expectation the parameter sets — lands at
**812 mm**. The spread across seeds is the sampling noise of ~650 exponential storms a year, not a
bias. Storm counts fall from about 2100 in a 20000-tick run to 653–703, which is 131–141 storms a
simulated year against the site's ~130 rain days (R2).

**Acceptance (B), annual plant water use in the published range: passes, with the site-cover
caveat.** The parameter is `hydro.et_mm_h = 0.05`, which is 438 mm a year at full cover and inside
the published 400–600 mm for well-watered temperate cool-season grass (R4). The measured site-wide
figure is lower, 276–349 mm, because ET is scaled by `0.2 + 0.8 ×` the patch's cover and these sites
run at 0.55–0.74 grass: 438 × 0.72 is 315 mm, which is where the measurements sit. `tests/units.rs`
pins the same quantity from the other end, asserting 250–650 mm a year on the bare 32 × 32 test world.
A tree's own draw is 0.0342 mm/h = **300 mm a year** over its trunk column, the bottom of R8's
300–700 mm band, and the reason it is at the bottom is `UNITS.md` finding 3 (a mature crown covers
nine columns and draws from one).

**A finding the conversion exposed: the site drains more than it transpires.** Every run above sends
290–483 mm a year out of the bottom of the soil and only 276–349 mm back to the air. A humid
temperate site with 800 mm of rain would normally lose more to evapotranspiration than to deep
drainage. The cause is structural rather than a unit error: `hydro.saturation = 1.2` gives each
column a transient store of 20% of capacity above field capacity, and every soil update drains
whatever is in it, so water that a real soil would hold for a plant to use is gone within 2.2 hours.
**This matters to shot G5 and not to this one**: drainage is what carries leaching, so a nutrient
model calibrated on this drainage will over-leach. Recorded here, not fixed — fixing it means giving
percolation a tension curve, which is a mechanism.

## 3. Soil water, and the new `moisture_band` check

**Acceptance (C), the soil sits between wilting point and field capacity: passes, and it is a new
check.** `moisture_mean` was parsed and reported and never bounded before this shot (`UNITS.md`
finding 9). The new line asserts the field mean stays above the wilting point on **every** tick of
the window and below field capacity on at least **95%** of them.

| run | min moisture (of 255) | ticks below capacity | whole-run mean soil water |
|---|---|---|---|
| s1 | 24.96 | 99.89% | 127 → **82** mm |
| s2 | 73.02 | 100.00% | 109 → **87** mm |
| s3 | 62.56 | 99.96% | 117 → **98** mm |
| s42 | 46.53 | 99.92% | 135 → **113** mm |
| capitol | 71.07 | 98.32% | 92 → **78** mm |
| long-s1 | 24.96 | — (not checked on the long run) | 103 → **86** mm |

Before the conversion `moisture_mean` sat at a whole-run mean of **235–240 of 255**, i.e. the soil
was at 92–94% of capacity essentially all the time and the moisture curves in `params.toml` never
came off their plateau. After it the mean is **190–205**, the minimum drops to a quarter of capacity
on seed 1, and the curves do work. That is the single clearest sign the conversion did what it was
for: the old rainfall was not just five times too large in the abstract, it was keeping the soil
saturated so that soil water could not be a limiting factor for anything.

**The lower bound of this check has no scale, and the margin table shows it as +0.0000 on every
passing run.** The wilting point is 0 by construction (`field_capacity_mm` is available water
capacity, so a store of 0 *is* the wilting point), and `check.rs`'s `margin_at_least` returns 0.0
against a threshold of 0. The check is a real pass/fail — a run whose soil hits the wilting point
fails it — but nothing can be read from the size of its margin. Noted so that nobody reads the
`moisture_band` row of `--baseline` as the tightest invariant in the table.

## 4. Fire: the ignition count, reported and not tuned

The operator's note of 2026-09-20 02:36 asks this shot to convert fire's thresholds, report the
resulting ignition count, and stop. Here it is.

| run | ignitions | spread events | burnouts | tree deaths by fire |
|---|---|---|---|---|
| s42 (the run shot E1 measured) | 8 → **26** | 5 → **26** | 13 → **52** | 7 → **52** |
| s1 | 5 → **28** | 1 → **22** | 6 → **50** | 1 → **53** |
| s2 | 7 → **33** | 9 → **32** | 16 → **65** | 57 → **63** |
| s3 | 7 → **33** | 16 → **78** | 23 → **111** | 8 → **303** |
| capitol | 38 → **104** | 19 → **233** | 57 → **337** | 9 → **893** |
| long-s1 (60000) | 25 → **84** | 18 → **222** | 43 → **306** | 60 → **886** |

**Fire came back on its own, and nothing was tuned to bring it back.** `fire.base_rate` moved from
0.002 per 10-tick update to 0.8 per patch per year, which is the identical rate at the shipped
cadence (0.002 × 400 updates a year). The only other change to fire is that its dryness term
`1 − moisture/255` now reads soil water as a fraction of available water capacity instead of the
0–255 display index — the same arithmetic on the physical quantity.

So the tripling is not fire's doing. It is section 3's: the site is genuinely drier than it was, the
dryness term is no longer pinned near zero by a permanently saturated soil, and fire's temperature
and fuel terms get to matter. Shot E1's observation — 8 ignitions in 20000 ticks of `runs/s42` and
never more than one patch alight in a snapshot — was a symptom of the rainfall error, not of a
mis-scaled fire threshold.

**This does not close backlog row G4d, and it is not meant to.** Whether a watered urban garden site
should burn at all is still a direction question for a human. What this shot changes is the number
the question is asked about: it is 26 ignitions and 52 burnouts in 20000 ticks of `runs/s42`, not 8
and 13, and on the Capitol it is 104 ignitions killing 893 trees over five simulated years, which is
a great deal of fire for a mown lawn between public buildings. The renderer's shot 09 reference
screenshot should show burning patches again.

## 5. Fertility over 50 years

**Acceptance (D), fertility does not pin at either bound over a 50-year run: passes**, on both
reference worlds.

| 50-year run (200000 ticks) | wall time | `fertility_mean` range | mean |
|---|---|---|---|
| Capitol, animals off, flat rain | 219 s | **[52.16, 129.01]** | 94.09 |
| 256 x 64 strip, seed 1, defaults | 3657 s | **[65.00, 133.61]** | 104.36 |
| Capitol, the same run at the **old** `leach_k = 0.0002` | 185 s | [106.95, **238.71**] | 216.03 |

Both ends are clear of the check's `[40, 220]` band on the shipped defaults, and the committed
`check --long` run (60000 ticks, seed 1) agrees at [65.00, 128.00]. The strip run is the slow one
because it carries about 3000 grazers and the animal tier costs 76-93% of tick time (shot 15a); it is
reported here but is not part of any gate.

The third row is the counterfactual that justifies the one nutrient value this shot moved. Left at
`leach_k = 0.0002`, fertility on the same 50-year Capitol run **crosses the 220 ceiling** (peak
238.71) and spends the run pinned near it at a mean of 216 — the pre-G4 runaway coming back, because
the sink G4 built was sized against rain that no longer falls.

This is the line `hydro.leach_k` moved for. Before shot G4 fertility had no sink at all and pinned at
the byte ceiling of 255 on any run long enough to show it (the operator's note of 2026-09-19 22:47).
G4 gave it leaching and set `leach_k = 0.0002` against the then-current 4000 mm of rain a year. With
rain corrected to 800 mm the drainage that carries the leaching falls with it, so the sink weakens by
the same factor; `leach_k` moves to 0.0008, derived as the mobile share of the pool over the rooting
zone's capacity, which removes 26% of a column's fertility a year at the measured ~320 mm of annual
drainage — inside R13's published 15–40% for nitrate loss from a humid temperate soil. **It was set
against the reference, not against the check**, and the check then passed.

## 6. What the cadence parameters bought, and what they exposed

The four hard-coded cadences became `[schedule]` parameters at their existing values, so no reference
run moved by a tick. What they made possible is
`doubling_an_update_interval_leaves_a_years_totals_within_5_percent`, which asserts that doubling
`cover_every`, `soil_every` or `temperature_every` changes a year's standing cover, litter, rain,
evapotranspiration and mean temperature by less than 5%. Before this shot the test could not have
been written for four of the six cadences, because they were literals.

Measured on the test's own world at the shipped defaults, doubling `soil_every` from 10 to 20:

| quantity | soil_every = 10 | = 20 | moved |
|---|---|---|---|
| rain | 876.5 mm | 876.5 mm | 0.0% |
| ET + ponded evaporation | 364.7 mm | 363.8 mm | 0.2% |
| drainage | 458.2 mm | 453.6 mm | 1.0% |
| runoff | 84.1 mm | 97.7 mm | **13.9%** |
| outflow | 49.7 mm | 55.5 mm | **10.5%** |

The rates hold; the routing does not. That asymmetry is the point of the test's explicit list: rain
and ET are charged by a rate per unit time and are now cadence-independent to a fraction of a
percent, while runoff and outflow are residuals of a store with a ceiling, so a store emptied half as
often has less room for the next storm and sheds more of it overland. That is the water tier's
integration error, a property of the model rather than evidence about its units, which is why it is
measured here and not asserted in the test.

`schedule.fire_every` is not driven by that test. Its per-update outcome is a Bernoulli draw whose
probability the cadence divides — `fire::ignition_prob` is linear in the update's length in years,
which `fire::ignition_regression_ramp_ends_and_clamp` asserts directly — and one simulated year's
realised ignition count has a Poisson spread far wider than any tolerance worth stating.

## 7. Rules found to be wrong rather than mis-scaled

Only one, and it is in `UNITS.md` as finding 1.

**`draw_moisture` scaled a plant's water demand by the soil's water-holding capacity.** A draw was
expressed in units of the 0–255 index and converted to millimetres as `units × capacity / 255`, so
the same plant drew four times as much water standing on a garden bed (200 mm AWC) as on gravel
(50 mm), and nothing at all on pavement. A plant's demand cannot depend on the water-holding capacity
of the soil under it. Both plant draws are now in millimetres (`tree.transpiration_mm_h`,
`cover.water_per_growth_mm`) and the conversion is gone.

Everything else the audit changed is a unit, not a rule. Three quantities whose *values* also moved —
`hydro.et_mm_h`, `hydro.evap_mm_h` and `hydro.leach_k` — moved because the old value was outside a
published range or was fitted against the old rainfall, and each is in `TUNING.md` with the reference
that set it.

## 8. Two things that survived the conversion unchanged, and one that did not

**Survived: the world file format.** `format_version` is unchanged, `docs/SCENE-CONTRACT.md` is
untouched, `ecosim/worlds/capitol/` was not re-exported, and the Capitol bundle built before this
shot loads after it. `the_pre_conversion_manifests_are_kept_as_history` asserts the point directly:
for each archived manifest the file set is the same and tick 0's `material.bin`, `light.bin` and
`height.bin` are byte-identical, because the terrain is upstream of every rate — while `series.csv`
and the last snapshot's `patches.json` differ, because the rates are what this shot changed.

**Survived: every health check, re-expressed.** No threshold was widened and none was retired. The
tick-denominated windows became year fractions read from the run's own `year_len`
(`WINDOW_YEARS` 0.5, `TREE_ANCHOR_YEARS` 1.25, `RUN_YEARS` 5.0, `SAMPLE_YEARS` 2.5, `LONG_YEARS` 15.0,
`LONG_BAND_FROM_YEARS` 5.0), and at the shipped year length each is exactly the tick count it
replaced, so no run's verdict changed by arithmetic. The grazer-cycle windows, `CAUSE_WINDOW` and the
six `SIG_*` constants stay in ticks, with the reason at each: they describe the per-tick animal tier,
which this shot does not convert, so a ruler in years would be measuring a per-tick cycle.

**Did not survive: the flat test world's trees.** The six forced-extinction tests ran on
`common::SQUARE`, the 64-world with `climate.rain_gradient = 0`, and four of them went red
mid-conversion. At 800 mm a year a flat, evenly watered world **cannot keep a tree**: a tree's 300 mm
is charged to its trunk column on top of that column's own grass evapotranspiration, so every column
is equally marginal and germination falls by about 85%. Measured on seed 2 at 10500 ticks, the flat
world ends with 19 mature trees and both animal species at zero, while the same world at the default
west–east rain gradient ends with 233 mature trees, 296 grazers and 51 hunters. The tests moved to
`common::SMALL` — the identical world with the gradient left on — because a forced-extinction test
has to force one mechanism in a world that is otherwise healthy. No default moved;
`baseline_margins_equal_check_margins` moved for the same reason, since it reads a margin table and a
failing row carries a `!` marker its format assertion trips over.

This is the clearest statement of `UNITS.md` finding 3 that the runs produce. A real tree's roots
spread at least as far as its crown; this model's tree draws its whole transpiration from one square
metre. Spreading the draw over the crown footprint is a mechanism and therefore not this shot's to
add, but it is why the tree's transpiration had to be calibrated at the bottom of its published range
to work at all, and it is named in `overnight/shots/G4c-units-calibration-rest.md`.

## 9. What is not converted

Light, the fertility and detritus indices, tree lifespans and phenology, the legacy non-water
moisture model, the whole animal tier, the three `immigration_interval`s and the six `SIG_*`
constants are all still in old units. Every one of them is marked `deferred` in `UNITS.md` section 6,
and `overnight/shots/G4c-units-calibration-rest.md` is the prompt that picks them up.

The largest of them is not a subsystem but a contradiction, and it is `UNITS.md` section 7: **a tick
is 2.1915 hours because the water tier says so, and under that tick a tree matures in 0.25 years and
dies at 1.5.** The water tier and the plant demography disagree about the tick by a factor of 25–60,
and no single tick duration satisfies both. This shot could not resolve it — the three ways out are a
40× longer reference run (which the runtime invariant forbids for the strip), per-tier time scales (a
mechanism), or a longer tick (which breaks the mm/h rates) — so it is recorded and handed on. The
honest reading of a `series.csv` after this shot: **the water columns are in real millimetres over a
real year, and the tree ages are not in real years.**

## 10. What the conversion does to the renderer's data, and the one job it broke

The renderer is a separate component and this shot does not touch it, but CI regenerates two runs for
it — `runs/s42` and `runs/capitol-s42`, both with `hydro.enabled=false` — and those runs are the
renderer's input, so the conversion moves them. Measured from the CI logs of the last green run
before this shot (35493848837) and this shot's run (35508181268):

| run | grazers | hunters | trees | total entities |
|---|---|---|---|---|
| `runs/s42` before | 3097 | 74 | 1114 | 4285 |
| `runs/s42` after | 2898 | 88 | 1541 | 4527 |
| `runs/capitol-s42` before | 0 | 0 | 1982 | 1982 |
| `runs/capitol-s42` after | 0 | 0 | 2875 | 2875 |

Trees rise 38% on the strip and 45% on the Capitol for the reason section 7 gives: with the water
tier off, `draw_water_mm` converts a tree's transpiration to index units at the soil medium's
capacity, so a tree that used to take 50 index units per 50-tick update now takes 6.4. The old figure
was the mis-scaling, not the new one. The strip's total entity count moves much less — 4285 to 4527,
5.6% — because the grazers fall as the trees rise.

That 5.6% is the whole of the renderer-facing change, and it is not enough to explain the one red
job. `ecoview`'s `tests/e2e/perf.spec.ts` failed on its own 300 s test timeout while the other 42
tests passed. Its gates were not breached and are not close: the threshold is a 1000 ms draw median
and a 1000 ms step median, and on the last green run the sixteen overlay-camera draw medians were
155.8–246.2 ms and the step median 296.5 ms. What the test costs is fixed by its matrix rather than
by its gates — 16 pairs × 60 frames at those medians is 195 s, plus 50 steps at 296.5 ms is 15 s,
plus sixteen overlay switches and reloads — so a green run already spent roughly 80% of the 300 s
budget, and the run that failed was slower throughout (the 42 passing tests took 4.3 min against
about 3.5 min for the same tests when the suite including perf finished in 7.0 min).

The conclusion for whoever picks this up: the headroom in that test is the problem, not the tree
count, and the fix belongs in `ecoview/` — which an ecosim shot may not edit. It is written up in
`overnight/shots/G4b.BLOCKED.md`.
