# Units audit (shots G4b, G4c, G4e, G5)

Every parameter in `params.toml` and every unit-bearing constant in the code, with the unit it is
actually in today, the unit it should be in, and where its value comes from. Written before any
conversion, as the shot prompt asks, so that the audit stands as the finding even if the conversion
has to be split across shots.

**This audit is complete for the garden direction (shot G4e).** Every parameter the garden series
reads is in the declared system of section 1, and **every row still marked `deferred` is deferred by
a decision with a named owner**, listed in section 6: the animal tier and its six `SIG_*` constants
belong to whatever shot unparks the tier, and the legacy non-water moisture path is **retired in
place and converts never**. The fertility and detritus group was the third, and **shot G5 closed
it** (section 6.1). Nothing here is outstanding for
want of time. This file is now a record, not a work list; the two findings it hands on are
mechanisms, not units (section 7's two time scales, and deciduous phenology, which is backlog row
G10).

**Status column.** `converted` — re-expressed by shot G4b, G4c or G4e; the shot is named in the row
or the section heading. `unchanged` — already in the declared system, nothing to do. `deferred` —
still in old units, by the decision recorded in section 6, with the shot that resolves it named.
`dimensionless` — carries no unit, nothing to convert. A row marked `deferred` must not be read as
physical, and must not be read as a gap either.

**What each shot converted.** G4b took the water tier, rainfall, the cadences and the decay and leach
rates. G4c took the two subsystems its prompt made non-negotiable: **light** (section 3.6) and **tree
demography** (section 3.7). G4e took the one parameter left that was worth converting, **tree
immigration** (section 3.8), and wrote the standing deferrals in section 6 down as decisions.

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
| light | **dimensionless fraction of full sun**, 0–1, which every light curve is now in; the `light` voxel field and `light.bin` hold `255 x` that fraction as a display index (`converted`, G4c) | `world.canopy_k`, `world.canopy_lai`, `world::FULL_SUN` |
| soil water index (`moisture`) | **dimensionless 0–255 index** = 255 x (soil water / available water capacity). A **display** scale: no rule reads it after this shot | `Sim::moisture` |
| nutrients | **g/m2** of nitrogen, phosphorus and potassium per ecology column, in `f64` pools with a ledger (`converted`, G5) | `Npk::soil`, `Npk::detritus` |
| fertility | **dimensionless 0–255 index**, now **derived** and published only: `255 x min_i(a_i / (a_i + half_sat_i x need_i))` for grass, the share of its growth the soil allows. A display scale on top of the pools above, not a stock (`converted`, G5) | `Sim::fertility` |
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
| `[world]` | 14 | world geometry, in metres and columns, plus the canopy optics | 11 unchanged, 2 dimensionless, 2 converted (G4c: `canopy_absorb` became `canopy_k` and `canopy_lai`) |
| `[climate]` | 14 | season, the legacy moisture model, decay | 1 converted, 4 unchanged, 5 dimensionless, 4 deferred |
| `[rain]` | 2 | storm frequency and size | 2 converted |
| `[hydro]` | 6 | the water tier's rates and stores | 3 converted, 1 unchanged, 2 dimensionless |
| `[medium.*]` | 9 x 4 | infiltration, AWC, percolation, plantability per medium | 27 unchanged, 9 dimensionless |
| `[season]` | 1 | temperature amplitude, °C | unchanged |
| `[cover]` | 7 | ground-cover water, litter and fertility | 1 converted, 6 dimensionless (2 of them closed by G5, and 3 read only with `npk.enabled=false`) |
| `[grass]`, `[shrub]` | 6 each | the two ground-cover species | 2 converted each, 1 dimensionless, 3 curves (2 converted — the light curve in G4c — 1 deferred) |
| `[tree]` | 21 | the tree species | 12 converted (8 in G4c: the light curve, `sapling_light`, the four ages, `seed_every`, `dry_death_ticks`; 1 in G4e: `immigration_interval`), 2 unchanged, 7 dimensionless (`death_detritus` closed by G5, section 6.1), and 5 new `[tree.npk]` and `waterlog_mortality` keys in g per unit of growth and per tree update |
| `[bundle]` | 6 | how a scene becomes a world | 2 converted (G4c), 3 unchanged, 1 dimensionless |
| `[animals]` | 1 | the animal tier's switch | dimensionless |
| `[grazer]`, `[hunter]` | 22, 23 | the animal tier | deferred (the tier is parked) |
| `[fire]` | 11 | fire disturbance | 1 converted, 2 unchanged, 8 dimensionless or deferred |
| `[disease]` | 4 | density-dependent mortality | deferred (animal tier) |
| `[heredity]` | 1 | trait mutation | dimensionless |
| `[rng]` | 1 | the random stream | dimensionless |
| `[schedule]` *(new)* | 4 | the four cadences that were hard-coded | converted |

26 sections and 181 keys before this shot (208 scalars with the nine four-element curves expanded),
27 sections and 185 keys after it. Every key appears in sections 3–6 below.

## 3. Converted (shots G4b, G4c and G4e)

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
| `hydro.leach_k` | 0.0002 | fertility fraction lost per mm drained | same | 0.0008 | R13 | converted; read only with `npk.enabled=false` since G5 |
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
| `tree.transpiration_mm_h` | *(new)* | — | **mm/h over the trunk column, annual mean** (since G10) | 0.0342 | R8 | converted |
| `tree.deciduous` | *(new, G10)* | — | **fraction of the draw a bare tree gives up** | 0.5 | model | dimensionless |
| `tree.leaf_off_temp`, `tree.leaf_on_temp` | *(new, G10)* | — | **°C of patch temperature** | 5.0, 10.0 | model | new |
| `cover.moisture_draw` | 15.0 | index units per unit of cover gained, **scaled by the column's AWC** | replaced by `cover.water_per_growth_mm` | — | — | converted |
| `cover.water_per_growth_mm` | *(new)* | — | **mm of water per unit of cover fraction gained** | 8.8 | model | converted |
| `tree.dry_moisture` | 30.0 | index units | replaced by `tree.dry_fraction` | — | — | converted |
| `tree.dry_fraction` | *(new)* | — | **fraction of AWC below which a tree is in drought** | 0.12 | R9 | converted |
| `grass.r`, `shrub.r` | 0.05, 0.01 | cover fraction gained per producer update | **per year** | 20.0, 4.0 | model | converted |
| `grass.g`, `shrub.g` | 0.005, 0.004 | cover fraction lost per producer update | **per year** | 2.0, 1.6 | model | converted |
| `grass.moisture`, `shrub.moisture`, `tree.moisture` | 0–256 curves | breakpoints on the 0–255 index | **fraction of AWC** | old / 255 | model | converted |
| `cover.litter_factor` | 20.0 | detritus per unit of cover lost per column | unchanged (detritus is dimensionless) | 20.0 | model | dimensionless (G5, section 6.1) |

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
| `fire.detritus_weight`, `canopy_weight`, `tree_kill`, `detritus_yield`, `ash`, `animal_damage` | — | dimensionless, or on the detritus and energy scales | — | unchanged | model | dimensionless (`ash` is read only with `npk.enabled=false` since G5) |

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
| `tree.seed_every` | already a parameter | 200 ticks | **converted in G4c** to `tree.seeds_per_year` = 20, a rate; see section 3.7 |
| `grazer/hunter.immigration_interval` | already parameters | 500 | deferred **with the animal tier**, see section 6 |
| `tree.immigration_interval` | already a parameter | 500 ticks | **converted in G4e** to `tree.immigrants_per_year` = 8, a rate; see section 3.8 |

### 3.6 Light (shot G4c)

The surface-irradiance index is gone. A voxel's light is the **fraction of full sun** that reaches
it, and the canopy attenuates it by Beer–Lambert extinction (R10):

    I / I0 = exp(−k x LAI x layers)

with `k` = `world.canopy_k` and the leaf area index of one canopy voxel = `world.canopy_lai`. The
voxel field and `light.bin` still hold one byte, now defined as `FULL_SUN x` that fraction with
`FULL_SUN = 255` (`src/world.rs`), so **the run directory format does not move**: the byte's range
is the same and only its meaning is now stated. Every curve that reads it is a fraction of full sun.

| key | old value (0–255 index) | new value | unit | ref | status |
|---|---|---|---|---|---|
| `world.canopy_absorb` | 100 subtracted per canopy voxel | — | — | — | **replaced** |
| `world.canopy_k` | — | 0.5 | extinction coefficient, dimensionless | ref (R10: 0.4–0.7) | converted |
| `world.canopy_lai` | — | 2.0 | m2 leaf per m2 ground, per canopy voxel | ref (R10, via the crown's LAI) | converted |
| `grass.light` | [100, 200, 255, 256] | [0.3922, 0.7843, 1.0, 1.0039] | fraction of full sun | model | converted |
| `shrub.light` | [40, 100, 200, 254] | [0.1569, 0.3922, 0.7843, 0.9961] | fraction of full sun | model | converted |
| `tree.light` | [60, 150, 255, 256] | [0.2353, 0.5882, 1.0, 1.0039] | fraction of full sun | model | converted |
| `tree.sapling_light` | 150 | 0.5882 | fraction of full sun | model | converted |
| `bundle.shade_slope` | 1.0 (a 45° sun, unstated) | `bundle.sun_altitude_deg` = 45.0 | degrees above the horizon | ref (Lansing's noon sun near the equinox is 47°) | converted |

**The germination threshold, stated as the prompt asks.** A seed's light suitability is 0 below
**23.5% of full sun**, rises linearly to 1 at **58.8%** (`tree.sapling_light`), and stays 1 above it.
So a tree germinates freely in the open (100%) and under a young one-voxel canopy at 36.8%, its
suitability is 0.38; under a mature two-voxel canopy at 13.5% it is 0.

**The one calibration.** `canopy_k x canopy_lai = 1.0` is an optical depth of 1 per canopy voxel. A
mature sim crown is two voxels, so its LAI is **4.0** — inside R10's 3–5 — and it transmits
**13.5%**, inside R10's 10–25%. That is the acceptance line, and `world.rs`'s
`closed_canopy_transmittance_is_inside_the_published_band` asserts it. The old rule transmitted
21.6% through the same crown, which is also inside the band, so the conversion is not a large change
at two layers; where it changes the world is at **one** layer (60.8% → 36.8%, a young canopy that
used to be nearly open) and at **three or more** (0% → 5.0% and thinning, instead of black).

`bundle.sun_altitude_deg` = 45° reproduces the old `shade_slope` of 1.0 to the bit, because
`1 / tan(45°) = 1`. No building's shadow moved. What the degree does is make the approximation
visible: a shaded voxel is fully dark, which is only true if there is no diffuse sky light, and the
diffuse component arrives with the moving sun of backlog row G9.

### 3.7 Tree demography (shot G4c)

Every tree age is now a **year** and the drought clock a **day**; the values are unchanged in
duration, so the conversion is a pure change of units and the reference runs move only through the
light change above.

| key | old value | new key and value | unit | ref | status |
|---|---|---|---|---|---|
| `tree.initial_age` | 500 ticks | `initial_age_years` = 0.125 | years | model | converted |
| `tree.young_age` | 500 ticks | `young_age_years` = 0.125 | years | **model, and wrong by ~25x** (R12) | converted |
| `tree.mature_age` | 1000 ticks | `mature_age_years` = 0.25 | years | **model, and wrong by ~25x** (R12) | converted |
| `tree.max_age` | 6000 ticks | `max_age_years` = 1.5 | years | **model, and wrong by ~60x** (R12) | converted |
| `tree.dry_death_ticks` | 500 ticks | `dry_death_days` = 45.66 | days | ref (R9) | converted |
| `tree.seed_every` | 200 ticks | `seeds_per_year` = 20.0 | attempts per year | model | converted |
| `bundle.tree_tall_age` | 3000 ticks | `tree_tall_age_years` = 0.75 | years | **model, and wrong by ~60x** (R12) | converted |

`Params::tree_ages()` turns all seven back into tick counts once per use; at the shipped
`year_len = 4000` they come back as exactly 500, 500, 1000, 6000, 500, 200 and 3000, which is why
this conversion is byte-neutral on its own.

### 3.8 Tree immigration (shot G4e)

| key | old value | new key and value | unit | ref | status |
|---|---|---|---|---|---|
| `tree.immigration_interval` | 500 ticks | `immigrants_per_year` = 8.0 | immigration checks per year | model | converted |

The last cadence in the tree tier. It is a **world** cadence, not a tree age: `Sim::immigrate` tests
the tick counter in its own phase right after animals, so the derivation lives on `Params`
(`ticks_between`, and the named `tree_immigration_every()`) and **not** in `TreeAges`, whose whole
reason to exist is that a tree age is read against a tree's own counter. `TreeAges::seed_every` now
calls the same `ticks_between`, so a rate per year becomes a cadence in ticks in exactly one place:
`round(year_len / per_year)`, never less than a tick, and `u32::MAX` at a rate of 0, which leaves the
event out rather than dividing by zero. At the shipped `year_len = 4000`, 8 a year is **exactly 500
ticks**, so nothing moved.

**A ceiling, not a rate.** A check plants a sapling only if fewer than `tree.immigration_floor` trees
are alive, and that floor is 0 at the defaults, so no immigration has ever fired in a reference run.
The conversion is therefore byte-neutral by construction and byte-identity is only a weak check of it;
`animals.rs`'s `a_tree_immigrates_at_its_rate_whatever_the_rate_is` is the real one, raising the floor
and showing the first arrival land on the derived tick for 8, 4, 20, 1 and 4000 checks a year, and no
arrival at all in 20000 ticks at a rate of 0.

**The two animal keys did not convert, and that is deliberate.** See section 6: they are deferred with
the parked tier, which is why three sibling keys now hold two different kinds of number.

**The values are deliberately not corrected.** The unit is now right and the number is now visibly
wrong, which is the whole point: see section 7 for the decision and `DECISIONS.md`, "Units
calibration, part two". Before this shot a reader had to divide 6000 by `year_len` to discover that
the model's trees die at eighteen months; now `max_age_years = 1.5` says so in the file.

## 4. Unchanged: already in the declared system

| key | value | unit |
|---|---|---|
| `world.height_min`, `height_max`, `water_level`, `rock_top_height`, `soil_depth`, `height` | 8, 24, 10, 21, 3, 32 | metres (voxels) |
| `world.width`, `depth`, `patch` | 256, 64, 8 | columns = metres |
| `climate.temp_base`, `canopy_cool`, `decay_temp_full` | 12.0, 3.0, 30.0 | °C |
| `season.amplitude` | 15.0 | °C |
| `tree.seed_radius`, `min_spacing` | 6.0, 2 | columns = metres |
| `tree.dry_death_days` | 45.66 | days of continuous drought (G4c renamed it from `dry_death_ticks` = 500 ticks; the duration is the same) |
| `bundle.tree_mature_height`, `tree_tall_height`, `tree_move_radius` | 3.0, 20.0, 2.0 | metres |
| `bundle.base_z` | 8 | metres |
| `grazer.flee_radius`, `search_patches`, `crowding`, `hunter.attack_radius`, `seek_radius`, `flee_radius`, `displace_steps`, `refugium_k` | — | columns = metres, or patches |
| `fire.temp_min`, `temp_full` | 15.0, 30.0 | °C |

`tree.dry_death_days` is the one tree constant that survives the audit unchanged:
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

## 6. Deferred by decision — still in old units, with an owner each

Read no row in this list as physical. Shot G4c cleared light (section 3.6) and tree demography
(section 3.7) out of it and G4e cleared tree immigration (section 3.8); what is left is here because
someone decided it should be, not because a shot ran out of time. Every group below names the shot
that resolves it, and one of them names nobody on purpose.

| what | why it is not converting | who resolves it |
| --- | --- | --- |
| The animal tier — all 45 keys of `[grazer]` and `[hunter]`, `[disease]`'s two rates, the 0–100 animal energy index, the two animal `immigration_interval` keys and the six `SIG_*` constants of `check.rs` | the tier is parked by the operator's direction of 2026-09-19 | **whatever shot unparks the tier**, and nothing before it |
| The legacy non-water moisture path — `climate.rain_base`, `rain_amp`, `evap_base`, `evap_div`, `pond_moisture`, `initial_moisture`, `climate.diffusion` | it is the pre-water-tier model, kept only so that runs pinned to it still reproduce; converting it would defeat the one reason it still exists | **nobody. It is retired in place**, and converts never |

### 6.1 Closed by shot G5 — the fertility and detritus group

`fertility` is no longer a stock in old units: the quantity is three `f64` pools in g/m2
(`Npk::soil`), and the 0–255 field that kept the name is **derived** from them for display, which is
section 1's `derived` case and not a deferral. `Patch::detritus` and the coefficients that act on it
— `cover.litter_factor`, `tree.death_detritus`, `fire.detritus_yield`, `fire.detritus_weight`,
`grazer.corpse_detritus`, `hunter.corpse_detritus` — stay a **dimensionless mass proxy**, and that is
now a statement rather than a deferral: the nutrients the litter carries are tracked in g/m2 beside
it in `Npk::detritus`, so the proxy no longer stands in for a quantity anything needs in units.
`cover.fertility_draw`, `cover.fertility_full`, `climate.initial_fertility` and `fire.ash` are read
only with `npk.enabled=false`; they are the pre-G5 path the identity manifest pins, which is why
they are still in `params.toml` and why converting them would be converting a museum piece.

The third row is the one a reader cannot work out from the backlog, because it is the only one whose
answer is "never": there is no shot to wait for and none is coming. If the pinned runs are ever
dropped, the path goes with them rather than being converted.

**Nutrients (subsystem 5, closed by shot G5).** This paragraph used to defer the whole group;
section 6.1 is what replaced it. `tree.death_detritus` = 40 was named here twice, as the one
`deferred` row left in `[tree]`, because it is a quantity on the detritus scale rather than a tree
age. It is dimensionless with the rest of that scale now, and the nitrogen, phosphorus and potassium
a dead tree returns are `tree.npk` times what it grew, in g/m2, which is a different quantity in a
different place.

The two rates that read a clock or a water flow were converted anyway, because leaving them on
the old cadence would contradict section 2: a rate is per hour or per year whatever index it
acts on.

- `climate.decay_k`: 0.015 per soil update became **6.0 per year** in G4b, the same behaviour
  written in the declared unit, with the 10x error left standing and handed to G5. **Shot G5 fixed
  it: 0.45 per year**, the middle of R11's 0.3–0.6, a nominal half-life of 1.5 years. The rate the
  model charges is lower again, because `Sim::decay_detritus` multiplies it by a temperature and a
  moisture factor, which on this site take the realised turnover to about a decade — a litter and
  duff layer rather than a leaf on a lawn. It is the one value in this file that changed behaviour
  rather than notation, and `TUNING.md` carries it with the rest of G5's table.
- `hydro.leach_k`: 0.0002 becomes **0.0008** per millimetre drained, and this one is a value
  change, because the old value was set against the pre-G4b rain of 4000 mm a year. It is the
  mobile share of a soil nutrient pool divided by the rooting zone's water capacity, and at the
  site's ~320 mm of annual drainage it leaches 26% of a column's fertility a year — inside
  the published 15–40% for nitrate loss from a humid temperate soil (R13). Left at 0.0002 the
  50-year Capitol run's fertility peaks at **238.7** against `check`'s ceiling of 220 and spends the
  run pinned near it (`sweeps/shotG4b/FINDINGS.md` section 5).

**Deciduous phenology.** Converted units do not create a leaf-off season: the model's trees
transpire at the same rate in January and July, `hydro.et_mm_h` has a temperature factor and the
tree draw has none (finding 4). Adding one is a mechanism, so shot G4c recorded it and stopped, as
its prompt required. Nothing in `params.toml` stands for it — there is no key to mark — which is
exactly why it is written down here.

**Closed by shot G10.** `tree.deciduous`, `tree.leaf_off_temp` and `tree.leaf_on_temp` give the tree
draw a season: a leaf-on share linear in patch temperature between 5 and 10 °C, and a bare tree
giving up `deciduous` of its draw. The draw is renormalised over the model year, so
`tree.transpiration_mm_h` keeps its meaning — the **annual mean** rate, 300 mm a year (R8) — and only
the season of it moves. The shipped `deciduous` is 0.5, not the 1 a broadleaf is; the reason is
finding 3 (TUNING.md, G10).

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

**The two animal `immigration_interval`s.** 500 ticks each for grazers and hunters. They are
schedules, not rates — `seed_every` was the third of that kind and shot G4c turned it into
`tree.seeds_per_year`, and G4e turned `tree.immigration_interval` into `tree.immigrants_per_year`
(section 3.8), which is the shape these two should take too.

**Three sibling keys now hold two different kinds of number, on purpose.** After G4e the tree
immigrates at a rate per year and the two animals still immigrate every N ticks. That reads like an
oversight and is not one: these two keys are deferred **with the tier**, on the same decision and
with the same owner as the other 45. Converting a parked tier's cadence buys the garden series
nothing and would be re-judged anyway by the shot that unparks it — which is also the shot that has
to face `[grazer]` and `[hunter]`'s undeclared 0–100 energy index, beside which one cadence is the
small half of the problem. They convert when the tier unparks, and not before.

**The signature constants of `check.rs`.** `SIG_MAX_LAG` 8000, `SIG_LAG_STEP` 50, `SIG_START` 5000,
`SIG_END` 60000, `SIG_DETREND` 12000, `SIG_MAX_PERIOD` 20000 — six tick counts that score the
predator–prey signature. They stay tick-denominated and move with the animal tier, which shot G4c
left parked on the operator's direction of 2026-09-19. **They are not G4e's**: G4e's scope was one
parameter and this write-up, and it left the tier alone deliberately. They belong to whichever shot
un-parks the tier, with the rest of it. Stated plainly a second time, because a silent deferral is how this kind of thing is lost:
this is a deferral, not a justification. If `climate.year_len` moves before that shot, the signature
score changes meaning and nothing will complain. The grazer-cycle windows in `check.rs` stay in
ticks with them.

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
| litter turnover | 2.2 yr nominal, ~10 yr realised (G5) | 1–3 yr (R11) | ~1x |
| grazer lifespan | 5000 ticks = 1.25 yr | plausible for a small herbivore | ~1x |

So the animal tier and the water tier agree on the tick, and the plant demography does not. Three
ways out, none of them this shot's to take:

1. **A 200000-tick reference run.** Multiply every tree age by 40 and run 40x longer. The runtime
   invariant (30 s per 64x64, capped at 90 s) forbids it: the strip already takes 39–52 s.
2. **Per-tier time scales.** Let the tree tier advance faster than the water tier — a mechanism, and
   a confusing one.
3. **A longer tick.** Make a tick a day and the trees come right, but then a storm cannot fall in
   one tick and the mm/h rates lose their meaning.

**Shot G4c's decision: none of the three, and say so in the file.** The prompt allowed a fourth
outcome — "DECISIONS.md says which of the three ways out was taken and why the line still cannot be
claimed" — and that is what G4c took, having found each of the three closed:

1. A 200000-tick run still fails the runtime invariant, which G4c may not widen (MASTER's standing
   rules, and the shot's own override 4: a check may be retired because its units are gone, never
   widened because the run now fails it). The strip takes 32–53 s at 20000 ticks.
2. Per-tier time scales is a mechanism, and the prompt's constraint is explicit: "If the choice
   turns out to need a mechanism (option 2 does), stop and write it up rather than adding one."
3. A longer tick breaks every mm/h rate the water tier was calibrated on in G4b — it would undo the
   one physically calibrated tier to fix the one that is not.

So G4c converted the **unit** and left the **value**, which moves the error from hidden to stated:
`max_age_years = 1.5` in `params.toml` is a claim a reader can check against R12 in one step, where
`max_age = 6000` was not. The published height-by-age acceptance line still cannot be claimed, and
for a second reason beyond the time scale: **the model's trees have no height**. A tree has an age
and a stage, and its canopy is one voxel or two; there is no metre to compare with R12's "3 m at
5–8 yr". `bundle.tree_mature_height` maps a *scene* tree's height onto an age at import and is not
a property the sim maintains.

Recorded, not resolved. The honest statement for anyone reading a `series.csv` after these shots:
the water columns are in real millimetres over a real year, the light is a real fraction of full
sun, and the tree ages are not in real years.

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
   *Closed by shot G10* (section 6): the draw now has a season, and its annual total is unchanged.
   Leaf-off light is not modelled — the canopy shades the same bare as in leaf — and belongs with the
   moving sun (backlog G9).
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
11. **A canopy could not be dark, it could only be black** (shot G4c). `255 - absorb x layers`
    saturated at 0 from three canopy voxels down, so a column under deep canopy received *exactly*
    no light and every suitability curve returned 0. Beer–Lambert never reaches 0: three layers now
    pass 5.0% and ten pass 0.005%. Mis-scaled rather than wrong — the rule was a subtraction where
    the physics is a product — but the saturation was a real artefact, not just a units error.
12. **Seeding was a schedule that only worked for divisors of `tree.update_every`** (shot G4c). A
    mature tree seeded when `age % seed_every == 0`, and a tree's age advances by whole
    `update_every` steps, so the test could only ever fire if `update_every` divided `seed_every`.
    It does at the shipped 50 and 200. It does not at, say, `seeds_per_year = 30`
    (`seed_every = 133`), where the old rule seeds once per 6650 ticks instead of 133 — a 50x error
    with nothing to warn of it. Turning the schedule into a rate made that reachable, so G4c changed
    the test to "the update whose age crosses a multiple of `seed_every`", which is identical at the
    shipped values for a tree whose age starts at a multiple of `update_every` and correct at every
    other. A tree imported from a world bundle does not start at one: 63 of the Capitol's 79 have an
    age that is not a multiple of 50 and never seeded at all before this shot
    (`sweeps/shotG4c/FINDINGS.md`). This is the second rule the audit found to be wrong
    rather than mis-scaled, after `draw_moisture` (finding 1).

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
- **R13** Nitrate leaching from a humid temperate soil under vegetation: **15–40%** of the
  mineral pool a year, higher under bare or fertilised ground.
- **R14** Temperate topsoil pools: **mineral nitrogen 10–50 kg/ha** (1–5 g/m2) at any moment
  against an organic stock ~100x larger; **total phosphorus 400–1000 kg/ha**, of which **5–15%**
  is plant-available; **exchangeable potassium 200–600 kg/ha**.
- **R15** Atmospheric nitrogen deposition on a temperate site near a city: **5–25 kg/ha/yr**.
  Biological fixation in a mixed sward adds of the same order again where clover is present.
- **R16** Plant tissue composition, dry matter: **1.5–3% N, 0.1–0.3% P, 1–2.5% K** for grass and
  herbaceous leaves; woody tissue is poorer in all three. Standing dry matter of a mown amenity
  lawn: **150–250 g/m2**; an unmown meadow reaches **400–600**.
