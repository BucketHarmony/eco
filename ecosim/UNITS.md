# Units audit (shot G4b)

Every parameter in `params.toml` and every unit-bearing constant in the code, with the unit it is
actually in today, the unit it should be in, and where its value comes from. Written before any
conversion, as the shot prompt asks, so that the audit stands as the finding even if the conversion
has to be split across shots.

**Status column.** `converted` — re-expressed by shot G4b. `unchanged` — already in the declared
system, nothing to do. `deferred` — still in old units, handed to shot G4c (see
`overnight/shots/G4c-units-calibration-rest.md`). `dimensionless` — carries no unit, nothing to
convert. A row marked `deferred` must not be read as physical.

**Reference column.** `ref` — calibrated against a published value, named in the reference list at
the bottom. `model` — a declared model constant with no physical counterpart; its value is a
modelling choice, not a measurement. Every `ref` in this file is from general knowledge and was
**not** fetched from a source on this machine: this machine has no network access during a shot, so
no row here should be read as a citation. The numbers are the ones a soil-science or urban-forestry
handbook would give to one significant figure; where that is not good enough for a decision, the
row says so.

The reference site is the **Michigan State Capitol grounds, Lansing, Michigan** — the only site this
shot calibrates against, and a public location.

## 1. The declared unit system

| quantity | unit | anchor |
|---|---|---|
| time | one **tick** = `HOURS_PER_YEAR / climate.year_len` hours = **2.1915 h** at `year_len = 4000` | `HOURS_PER_YEAR = 8766.0`, the mean Julian year (365.25 d x 24 h), named once in `src/hydro.rs` |
| length, horizontal | **1 m** per ecology column (`bundle::ECO_CELL_M`); the ground grid is finer, `ground_cell_m = ECO_CELL_M / ratio` | the scene contract's ground grid |
| length, vertical | **1 m** per voxel | `world.height` is metres |
| water | **mm** of depth over the area it sits on, i.e. L/m2; volumes are mm·m2 | `[medium.*]`, `hydro.*` |
| temperature | **°C** | `climate.temp_base` |
| area | **m2**; one ecology column is 1 m2 | `Flow::cell_area` |
| plant cover | **dimensionless fraction** of ground covered, 0–1 | `Patch::grass`, `Patch::shrub` |
| rates | **per hour** for water, **per year** for everything biological; a staggered update multiplies the rate by its own duration, so changing a cadence does not change an annual total | `schedule.*` |
| light | **dimensionless 0–255 index** of surface irradiance — not yet physical (`deferred`) | `world.canopy_absorb` |
| soil water index (`moisture`) | **dimensionless 0–255 index** = 255 x (soil water / available water capacity). A **display** scale: no rule reads it after this shot | `Sim::moisture` |
| fertility | **dimensionless 0–255 index** — not yet physical (`deferred`; shot G5 replaces the field with N, P and K) | `Sim::fertility` |
| animal energy | **dimensionless 0–100 index** — not yet physical (`deferred`; the animal tier is parked) | `Animal::energy` |

Two consequences worth stating plainly.

**The soil water store holds plant-available water, not total water.** `field_capacity_mm` is the
**available water capacity (AWC)** of the rooting zone: the water held between field capacity and
the permanent wilting point. So a store of 0 is the wilting point, not oven-dry soil, and a store at
`field_capacity_mm` is field capacity. This is a declaration, not a change: it is the only reading
under which the committed value of 150 mm for lawn and soil is right (loam holds about 150 mm of
available water per metre of rooting depth), and it is what makes "the soil sits between wilting
point and field capacity" a statement about the interval [0, `field_capacity_mm`]. The name
`field_capacity_mm` is kept so that no world file or renderer changes; it means AWC.

**A tick is hours long, and the plant demography is denominated in ticks.** The water tier needs a
tick of hours (storms fall and infiltrate in one tick, at mm/h). The tree tier reaches maturity in
1000 ticks and dies at 6000, which under the declared tick is 0.25 and 1.5 years. A 20000-tick
reference run is **5 years**, not the several decades its grass-to-woodland succession depicts. The
two tiers are out by a factor of about 50–100 and no single tick duration satisfies both. This is
the largest single finding of the audit; see section 7.

## 2. Sections

| section | keys | what it holds | status |
|---|---|---|---|
| `[world]` | 13 | world geometry, in metres and columns | 11 unchanged, 2 dimensionless |
| `[climate]` | 14 | season, the legacy moisture model, decay | 3 converted, 4 unchanged, 7 deferred |
| `[rain]` | 2 | storm frequency and size | 2 converted |
| `[hydro]` | 6 | the water tier's rates and stores | 2 converted, 4 unchanged |
| `[medium.*]` | 9 x 4 | infiltration, AWC, percolation, plantability per medium | 27 unchanged, 9 dimensionless |
| `[season]` | 1 | temperature amplitude, °C | unchanged |
| `[cover]` | 7 | ground-cover water, litter and fertility | 1 converted, 4 dimensionless, 2 deferred |
| `[grass]`, `[shrub]` | 6 each | the two ground-cover species | 2 converted each, 1 dimensionless, 3 curves (1 converted, 2 deferred) |
| `[tree]` | 21 | the tree species | 3 converted, 3 unchanged, 6 dimensionless, 9 deferred |
| `[bundle]` | 6 | how a scene becomes a world | 3 unchanged, 1 dimensionless, 2 deferred |
| `[animals]` | 1 | the animal tier's switch | dimensionless |
| `[grazer]`, `[hunter]` | 22, 23 | the animal tier | deferred (the tier is parked) |
| `[fire]` | 11 | fire disturbance | 1 converted, 2 unchanged, 8 dimensionless or deferred |
| `[disease]` | 4 | density-dependent mortality | deferred (animal tier) |
| `[heredity]` | 1 | trait mutation | dimensionless |
| `[rng]` | 1 | the random stream | dimensionless |
| `[schedule]` *(new)* | 4 | the four cadences that were hard-coded | converted |

26 sections and 181 keys before this shot (208 scalars with the nine four-element curves expanded),
27 sections and 185 keys after it. Every key appears in sections 3–6 below.

## 3. Converted by this shot

### 3.1 Climate and rain

| key | old value | unit today | new unit | new value | ref | status |
|---|---|---|---|---|---|---|
| `rain.storm_p` | 0.10 | probability per tick, at `year_len = 4000` | replaced by `rain.annual_mm` | — | — | converted |
| `rain.annual_mm` | *(new)* | — | **mm of rain per year** | 800.0 | R1 | converted |
| `rain.storm_mean_mm` | 10.0 | mm per storm (already mm) | mm per storm | 6.0 | R2 | converted |
| `climate.year_len` | 4000 | ticks per year | ticks per year (the tick-duration anchor) | 4000 | model | unchanged |
| `climate.temp_base` | 12.0 | °C | °C | 12.0 | R3 | unchanged |
| `season.amplitude` | 15.0 | °C | °C | 15.0 | R3 | unchanged |
| `climate.rain_gradient` | 0.6 | dimensionless west–east ramp | same | 0.6 | model | dimensionless |

The old rainfall was `storm_p x year_len x storm_mean_mm` = 0.1 x 4000 x 10 = **4000 mm a year**,
five times Lansing's normal, which is what shot G4's write-up called out. The per-tick probability is
now derived: `p = annual_mm / (year_len x storm_mean_mm)`, scaled by the season factor, whose mean
over a year is exactly 1, so the expected annual depth is `annual_mm` by construction and does not
move when `year_len` moves. `annual_mm = 0` still leaves the rain out and keeps the draw, as
`storm_p = 0` did.

`storm_mean_mm` moves from 10 to 6 for the same reason `annual_mm` exists: 800 mm delivered in 10 mm
storms is 80 storms a year, while Lansing records rain on about 130 days (R2). 6 mm gives 133.
Smaller storms also shed less runoff, which matters on a site that is a third pavement.

### 3.2 Soil water

| key | old value | unit today | new unit | new value | ref | status |
|---|---|---|---|---|---|---|
| `hydro.et_mm_h` | 0.12 | mm/h of evapotranspiration at full cover | same | 0.05 | R4 | converted |
| `hydro.evap_mm_h` | 0.05 | mm/h from ponded water | same | 0.08 | R5 | converted |
| `hydro.initial_fill` | 0.5 | fraction of AWC at tick 0 | same | 0.5 | model | dimensionless |
| `hydro.saturation` | 1.2 | multiple of AWC the store holds | same | 1.2 | model | unchanged |
| `hydro.leach_k` | 0.0002 | fertility fraction lost per mm drained | — | 0.0002 | model | deferred (with fertility) |
| `hydro.enabled` | true | switch | — | — | — | dimensionless |
| `medium.*.infiltration_mm_h` | 0–60 | mm/h | mm/h | unchanged | R6 | unchanged |
| `medium.*.field_capacity_mm` | 0–200 | mm — **is AWC of the rooting zone** | mm of AWC | unchanged | R7 | unchanged |
| `medium.*.percolation_mm_h` | 0–30 | mm/h out of the bottom | mm/h | unchanged | R6 | unchanged |
| `medium.*.plantable` | bool | switch | — | — | — | dimensionless |

`et_mm_h` at 0.12 is 1052 mm a year at full cover and mean temperature, about double the published
range for well-watered temperate cool-season grass. 0.05 is 438 mm a year, inside it. `evap_mm_h`
at 0.05 is 438 mm a year of open-water evaporation, below the Great Lakes figure of roughly 700;
0.08 gives 701. It applies only to standing water, which is rare, so the change moves almost nothing.

The 27 `[medium.*]` rates were already physical when shot G4 wrote them and are not re-derived here,
as the prompt says. They are recorded so the table is complete, and `field_capacity_mm` gains the
AWC reading above — a documentation change, not a numeric one.

### 3.3 Plant water demand and growth

| key | old value | unit today | new unit | new value | ref | status |
|---|---|---|---|---|---|---|
| `tree.moisture_draw` | 50.0 | index units per 50-tick update, **scaled by the column's AWC** | replaced by `tree.transpiration_mm_h` | — | — | converted |
| `tree.transpiration_mm_h` | *(new)* | — | **mm/h over the trunk column** | 0.0342 | R8 | converted |
| `cover.moisture_draw` | 15.0 | index units per unit of cover gained, **scaled by the column's AWC** | replaced by `cover.water_per_growth_mm` | — | — | converted |
| `cover.water_per_growth_mm` | *(new)* | — | **mm of water per unit of cover fraction gained** | 8.8 | model | converted |
| `tree.dry_moisture` | 30.0 | index units | replaced by `tree.dry_fraction` | — | — | converted |
| `tree.dry_fraction` | *(new)* | — | **fraction of AWC below which a tree is in drought** | 0.12 | R9 | converted |
| `grass.r`, `shrub.r` | 0.05, 0.01 | cover fraction gained per producer update | **per year** | 20.0, 4.0 | model | converted |
| `grass.g`, `shrub.g` | 0.005, 0.004 | cover fraction lost per producer update | **per year** | 2.0, 1.6 | model | converted |
| `grass.moisture`, `shrub.moisture`, `tree.moisture` | 0–256 curves | breakpoints on the 0–255 index | **fraction of AWC** | old / 255 | model | converted |
| `cover.litter_factor` | 20.0 | detritus per unit of cover lost per column | unchanged (detritus is dimensionless) | 20.0 | model | deferred |

Three things here, in order of how much they matter.

**A dimensional error, not a mis-scaling.** `Sim::draw_moisture` converted a draw in index units to
millimetres with `units x capacity / 255`, so the *same plant* drew four times as much water on a
garden bed (200 mm AWC) as on gravel (50 mm), and nothing at all on pavement. A plant's water demand
cannot depend on the water-holding capacity of the soil it stands in. Both plant draws are now in
mm and the conversion is gone. This is the one rule in the shot that was **wrong** rather than
mis-scaled.

**The tree draw was 2352 mm a year.** 50 index units per 50-tick update, at a lawn's 150 mm AWC, is
29.4 mm every 109.6 h — 2352 mm a year from one column. That is the number the rain was calibrated
up to. 0.0342 mm/h is 300 mm a year, the low end of the published range for an open-grown temperate
deciduous tree per unit of crown projection. The rain falls by 5x and the tree's demand by 7.8x, so
a tree is better supplied after this shot than before it, not worse.

**Why the low end.** A sim tree draws from its trunk column alone — 1 m2 — while its mature crown
covers 9 m2. A real tree's roots spread at least as far as its crown, so per unit of *crown* area
300 mm/yr is right, but per unit of *rooting* area the model is charging nine columns' worth of
transpiration to one. Root spread over the crown footprint is a **missing mechanism**, recorded in
section 8 and not added here: the prompt forbids new mechanisms, and spreading the draw is one.

`r` and `g` become per-year rates because they had to for the interval-doubling test to be possible:
they were increments per producer update, so doubling the producer cadence halved a year's growth.
At the default cadence the new values reproduce the old ones exactly (`0.05 x 400 updates/yr =
20/yr`). They stay `model`: a logistic growth coefficient for "ground cover" is not a measured
quantity.

### 3.4 Fire

Converted out of the prompt's order, at the operator's request of 2026-09-20 02:36, because fire
reads the soil water index directly and that index is one of the quantities this shot re-expresses.

| key | old value | unit today | new unit | new value | ref | status |
|---|---|---|---|---|---|---|
| `fire.base_rate` | 0.002 | ignition probability per patch per 10-tick fire update | **ignitions per patch per year** | 0.8 | model | converted |
| `fire.temp_min`, `temp_full` | 15.0, 30.0 | °C | °C | unchanged | model | unchanged |
| `fire.duration` | 3 | ticks alight | ticks (about 6.6 h) | unchanged | model | deferred |
| `fire.spread` | 0.1 | probability per tick per neighbour | per tick | unchanged | model | deferred |
| `fire.detritus_weight`, `canopy_weight`, `tree_kill`, `detritus_yield`, `ash`, `animal_damage` | — | dimensionless, or on the fertility, detritus and energy scales | — | unchanged | model | dimensionless / deferred |

The dryness term `1 - moisture/255` is exactly `1 - soil_water / AWC` and is now written that way:
the same number, read from the physical quantity rather than from the display index.
`base_rate` becomes a per-year rate for the same reason `grass.r` did — `0.002 x 400 updates/yr =
0.8/patch/yr` reproduces it at the default cadence.

**Fire's ignition count is reported, not tuned.** See `sweeps/shotG4b/FINDINGS.md`. Whether a watered
urban garden site should burn at all is backlog row G4d, held for a human.

### 3.5 Cadences

All six update cadences are now parameters, in one place, so that a subsystem's rate and its cadence
can be read from the same value. Each is a tick count; the duration it stands for is
`cadence x tick_hours`.

| key | old home | value | status |
|---|---|---|---|
| `schedule.cover_every` | `producers.rs:63-70`, the literal `10` twice on one line | 10 | converted |
| `schedule.soil_every` | `sim.rs:379` and `abiotic.rs:72`, the literal `10` in two places that had to agree with nothing checking | 10 | converted |
| `schedule.temperature_every` | `sim.rs:383`, the literal `100` | 100 | converted |
| `schedule.fire_every` | `fire.rs:15`, `pub const FIRE_EVERY: u32 = 10` | 10 | converted |
| `tree.update_every` | already a parameter | 50 | unchanged |
| `world.compact_every` | already a parameter | 100 | unchanged |
| `tree.seed_every` | already a parameter | 200 | deferred, see section 6 |
| `grazer/hunter/tree.immigration_interval` | already parameters | 500 | deferred, see section 6 |

## 4. Unchanged: already in the declared system

| key | value | unit |
|---|---|---|
| `world.height_min`, `height_max`, `water_level`, `rock_top_height`, `soil_depth`, `height` | 8, 24, 10, 21, 3, 32 | metres (voxels) |
| `world.width`, `depth`, `patch` | 256, 64, 8 | columns = metres |
| `climate.temp_base`, `canopy_cool`, `decay_temp_full` | 12.0, 3.0, 30.0 | °C |
| `season.amplitude` | 15.0 | °C |
| `tree.seed_radius`, `min_spacing` | 6.0, 2 | columns = metres |
| `tree.dry_death_ticks` | 500 | ticks = 45.6 days of continuous drought |
| `bundle.tree_mature_height`, `tree_tall_height`, `tree_move_radius` | 3.0, 20.0, 2.0 | metres |
| `bundle.base_z` | 8 | metres |
| `grazer.flee_radius`, `search_patches`, `crowding`, `hunter.attack_radius`, `seek_radius`, `flee_radius`, `displace_steps`, `refugium_k` | — | columns = metres, or patches |
| `fire.temp_min`, `temp_full` | 15.0, 30.0 | °C |

`tree.dry_death_ticks` is the one tick-denominated tree constant that survives the audit unchanged:
45.6 days of continuous water stress before death is the right order for a mature broadleaf, and it
is the *only* tree age that is. Every other one is out by 50x or more (section 7).

## 5. Dimensionless: nothing to convert

`world.water_fraction`, `world.slope_bias` (metres of tilt, but a shape parameter), `climate.diffusion`,
`climate.rain_gradient`, `hydro.enabled`, `hydro.initial_fill`, `medium.*.plantable` (9),
`cover.grass_suppression`, `cover.shrub_spread_threshold`, `cover.shrub_spread_seed`,
`grass.initial`, `shrub.initial`, `tree.initial_count`, `tree.lifespan_jitter`,
`tree.crowding_mortality`, `tree.immigration_floor`, `animals.enabled`, `fire.tree_kill`,
`fire.canopy_weight`, `disease.grazer_threshold`, `disease.hunter_threshold`, `heredity.mutation`,
`rng.stream`, `grazer.choice_tolerance`, `grazer.max_grazers_per_patch`, `grazer.start_count`,
`grazer.immigration_floor`, `grazer.eat_min_grass`, `hunter.start_count`, `hunter.kill_prob`,
`hunter.immigration_floor`.

## 6. Deferred to shot G4c — still in old units

Read no row in this list as physical.

**Light (subsystem 4 of the prompt's order, not started).** `world.canopy_absorb` = 100 on a 0–255
index, `grass.light`, `shrub.light`, `tree.light` (three 4-element curves on the same index),
`tree.sapling_light` = 150, `bundle.shade_slope` = 1.0 (a 45° sun), and `world.rs:496`'s
`255 - absorb x layers`. The physical form is Beer–Lambert extinction through a canopy of a given
leaf area index (R10), and the fixed sun is backlog row G9's business. Converting light means
converting all four curves, the sapling threshold and the building shade together, and it changes
which columns germinate, so it is a subsystem on its own.

**Nutrients (subsystem 5, not started).** `cover.fertility_draw` = 20, `cover.fertility_full` = 64,
`cover.litter_factor` = 20, `climate.decay_k` = 0.015 per soil update, `climate.initial_fertility` =
128, `hydro.leach_k`, `fire.ash`, `fire.detritus_yield`, `fire.detritus_weight`, `tree.death_detritus`,
`grazer.corpse_detritus`, `hunter.corpse_detritus`, and the `Patch::detritus` pool itself. All on
the 0–255 fertility index or the arbitrary detritus scale. Shot **G5 replaces this field outright**
with nitrogen, phosphorus and potassium, so converting it here would be converting a quantity that
is about to be deleted; `decay_k` is the one row worth naming, because 0.015 per soil update is a
turnover of about 2 months against a published litter turnover of 1–3 years (R11) — a 10x
discrepancy handed on with the rest.

**Lifespans and phenology (subsystem 7, not started).** `tree.initial_age` = 500, `young_age` = 500,
`mature_age` = 1000, `max_age` = 6000, `seed_every` = 200, `bundle.tree_tall_age` = 3000 — all tick
counts, all out by 50–100x (section 7). A tree is mature in 0.25 years and dead in 1.5. Deciduous
phenology (a leaf-off season with no transpiration) does not exist at all. `tree.seed_every` is
also the one cadence the interval-doubling test cannot cover, because it is a schedule and not a
rate: a mature tree attempts one seed every `seed_every` ticks, so halving it doubles a year's
attempts. Turning it into a per-year attempt rate belongs with the rest of the tree demography.

**The legacy moisture model.** `climate.rain_base` = 8.0, `rain_amp` = 4.0, `evap_base` = 2.0,
`evap_div` = 8.0, `pond_moisture` = 255.0, `initial_moisture` = 128.0, `climate.diffusion` = 0.10.
These drive the pre-G4 moisture path, which runs only when `hydro.enabled = false`. They are index
units per soil update and they stay that way: the path exists so that pre-G4 runs still reproduce,
and converting it would defeat its purpose. `rain_base` has one live use with the tier on — it
normalises the seasonal rain factor, where only the ratio `rain_amp / rain_base` matters — and
`evap_base` and `evap_div` have one, the temperature factor, which is normalised to 1 at
`temp_base` so only their ratio matters there too.

**The animal tier.** All 45 keys of `[grazer]` and `[hunter]`, plus `[disease]`'s two rates.
Energies are on an undeclared 0–100 scale whose ceiling is the bare literal `.min(100.0)` at
`animals.rs:399` and `:533`; `energy_cost` and the `disease.*_rate`s are per tick; `max_age`,
`cooldown`, `refractory`, `handling_ticks` and `start_age_max` are tick counts; `grass_per_energy`
converts cover fraction to energy; `intake_max`, `intake_k`, `eat_below`, `repro_energy`,
`repro_cost`, `newborn_energy`, `start_energy`, `satiation`, `kill_energy`, `hunt_cost` and
`fail_cost` are all on the energy index. The tier is parked by the operator's direction of
2026-09-19, so converting it buys nothing the garden series needs.

**The `immigration_interval`s.** 500 ticks for each of trees, grazers and hunters. Like `seed_every`
they are schedules, not rates, and two of the three belong to the parked animal tier.

**The signature constants of `check.rs`.** `SIG_MAX_LAG` 8000, `SIG_LAG_STEP` 50, `SIG_START` 5000,
`SIG_END` 60000, `SIG_DETREND` 12000, `SIG_MAX_PERIOD` 20000 — six tick counts that score the
predator–prey signature. They stay tick-denominated and move with the animal tier in G4c. Stated
plainly: this is a deferral, not a justification. If `climate.year_len` moves before G4c, the
signature score changes meaning and nothing will complain.

## 7. The finding: two time scales

A tick is 2.1915 hours. That number is forced by the water tier, which is the only physically
calibrated part of the model: storms fall in one tick, infiltration and percolation are mm/h, and
a 32.9 mm infiltration capacity per tick on lawn is exactly 15 mm/h x 2.1915 h.

Under that tick:

| quantity | model | published | out by |
|---|---|---|---|
| tree reaches 3 m (a mature sim canopy) | 1000 ticks = 0.25 yr | 5–8 yr (R12) | ~25x |
| tree reaches 20 m | 3000 ticks = 0.75 yr | 40–60 yr (R12) | ~60x |
| tree lifespan | 6000 ticks = 1.5 yr | 60–150 yr (R12) | ~60x |
| a reference run | 20000 ticks = 5 yr | the succession it shows is decades | ~10x |
| litter turnover | about 2 months | 1–3 yr (R11) | ~10x |
| grazer lifespan | 5000 ticks = 1.25 yr | plausible for a small herbivore | ~1x |

So the animal tier and the water tier agree on the tick, and the plant demography does not. Three
ways out, none of them this shot's to take:

1. **A 200000-tick reference run.** Multiply every tree age by 40 and run 40x longer. The runtime
   invariant (30 s per 64x64, capped at 90 s) forbids it: the strip already takes 39–52 s.
2. **Per-tier time scales.** Let the tree tier advance faster than the water tier — a mechanism, and
   a confusing one.
3. **A longer tick.** Make a tick a day and the trees come right, but then a storm cannot fall in
   one tick and the mm/h rates lose their meaning.

Recorded, not resolved. The honest statement for anyone reading a `series.csv` after this shot: the
water columns are in real millimetres over a real year, and the tree ages are not in real years.

## 8. Audit findings

1. **`draw_moisture` scaled a plant's demand by the soil's water-holding capacity** (section 3.3).
   Fixed. The only rule the audit found to be wrong rather than mis-scaled.
2. **The rainfall was calibrated to the plants, not the plants to the rainfall** — shot G4 said so
   itself, and the audit puts the number on it: a tree drew 2352 mm a year from one square metre.
3. **Root spread is a missing mechanism.** A tree draws from its trunk column alone while its crown
   covers nine. Not added (the prompt forbids mechanisms); it is why the tree's transpiration is
   calibrated at the low end of its published range.
4. **Deciduous phenology is a missing mechanism.** The model's trees transpire at the same rate in
   January and July. `hydro.et_mm_h` has a temperature factor; the tree draw has none.
5. **Two documentation errors, both fixed by this shot.** `CLAUDE.md:58` and `DECISIONS.md:1168`
   called `moisture` a `u8` field; it is `Vec<f32>` in memory and `u8` only in `moisture.bin`.
   `TUNING.md:523` said "0.25 mm/h is 5.5 mm a tick against a lawn's 40 mm field capacity": 5.5 mm
   is one whole 10-tick soil update, a tick is 0.55 mm, and the lawn's figure is 150 mm, not 40.
6. **`8766.0` was a bare literal in a function body** (`hydro.rs:27`) and `hydro.rs` was the only
   module that knew hours existed. It is now `HOURS_PER_YEAR`, named once, and the biological rates
   derive from it.
7. **Two modules encoded the soil cadence independently** (`sim.rs:379` and `abiotic.rs:72`, the
   second multiplying it by `tick_hours`). They now read `schedule.soil_every`. Before this shot they
   could have desynchronised silently, turning every mm/h rate in the soil update into a different
   number with nothing failing.
8. **Not one of the 16 thresholds in `check.rs` read `params`,** and seven were denominated in ticks,
   so they silently changed meaning with `climate.year_len`. The windows are now year fractions
   against the run's own `year_len`, read from `meta.json`. The six `SIG_*` constants are the
   exception and are deferred (section 6).
9. **There was no moisture invariant at all.** `moisture_mean` was parsed, reported and never
   bounded. The new `moisture_band` line is therefore a new check, not a re-expressed one.
10. **`animals.rs` has a maximum animal energy with no parameter behind it** — `.min(100.0)` at two
    call sites. Deferred with the tier, recorded here so it is not lost.

## References

Every one of these is from general knowledge, **not fetched**: this machine had no network access
during the shot. One significant figure is all any of them should be trusted to.

- **R1** Annual precipitation normal, Lansing, Michigan: about **800 mm** (31–33 in).
- **R2** Days a year with measurable precipitation at Lansing: about **130**.
- **R3** Lansing monthly mean temperature: about **-5 °C** in January, **22 °C** in July, annual mean
  about **9 °C**. The model's 12 ± 15 °C spans -3 to 27 — about 3 °C warm and a little wide. Left
  alone: it is inside the region's range and moving it moves every growth curve.
- **R4** Actual evapotranspiration from well-watered temperate cool-season grass: **400–600 mm/yr**.
- **R5** Open-water (lake) evaporation in the Great Lakes region: about **700 mm/yr**.
- **R6** Saturated infiltration: loam **10–20 mm/h**, sand and gravel **> 50 mm/h**, concrete and
  asphalt effectively **0**. Deep percolation below the root zone is a few mm/h in loam.
- **R7** Available water capacity: loam **150–200 mm per metre** of rooting depth, sand **60–80**,
  compost-amended bed higher.
- **R8** Transpiration of an open-grown temperate deciduous tree, per unit of crown projection:
  **300–700 mm/yr**.
- **R9** Plants approach the permanent wilting point as available water approaches 0; drought stress
  begins at roughly **50%** of available water and is severe below **10–20%**.
- **R10** Canopy light extinction follows Beer–Lambert with **k = 0.4–0.7** for broadleaf canopies;
  at LAI 3–5 that transmits **10–25%** of incident light.
- **R11** Temperate deciduous leaf-litter decomposition: **k = 0.3–0.6 /yr**, a half-life of
  **1.2–2.3 yr**.
- **R12** Street and park broadleaf (maple, oak): about **3 m at 5–8 yr**, **10 m at 20 yr**,
  **20 m at 40–60 yr**; lifespan **60–150 yr** in an open urban setting.
