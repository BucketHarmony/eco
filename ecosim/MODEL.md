# MODEL.md: the rules of ecosim

Every rule the simulator runs has one subsection here, giving its equation, the parameters it reads, how often it runs, and which shot introduced it. The garden model comes first: ground, light and climate, then water (media, storms, soil water, drains), then nutrients, ground cover and fire, then trees. The animal tier is parked and comes last, kept short.

This file describes what the code does. Where a code comment, a `params.toml` comment or an older document says something different, the subsection follows the code and says so in a note. The notes are collected in [Discrepancies](#discrepancies-found-while-writing-this) at the end. `tests/model.rs` fails if any key of `params.toml`, or any key of the loaded `Params`, is missing from this file in backticks (shot 26).

Related documents: `UNITS.md` is the units audit and its published sources (R1, R2, ...). `TUNING.md` logs every value change and why. `DECISIONS.md` records the design calls. `PERF.md` covers speed. `../docs/SCENE-CONTRACT.md` is the bundle format.

## How to read this file

- **Parameters** are written as full dotted keys, for example `grass.npk.need_n` for `need_n` under `[grass.npk]` in `params.toml`. The value in brackets is the shipped default. A key used by several rules is described once, and the other rules point to it.
- **Introduced** names the shot that added the rule, from the `shot NN:` commit messages, code comments and `DECISIONS.md` headings. "SAD build" means the first simulator commit, b69016f, which predates shot numbers. "Dynamics fixes" is commit 83de5bf, which shot 05 completed.
- **Time.** A year is `climate.year_len` ticks (4000), and a tick is `8766 / year_len` = 2.1915 hours (`hydro::HOURS_PER_YEAR`, the mean Julian year). Since shot G4b, every garden rate in `params.toml` is per hour or per year. A staggered update charges the rate over its own length, `cadence × tick_hours` or `years(cadence) = cadence / year_len`, so changing a cadence leaves a year's totals alone (`tests/units.rs`). The animal tier was never converted: its rates are per tick.
- **Curves.** A four-point curve `[min, low-opt, high-opt, max]` is a trapezoid: 0 at or below `min` and at or above `max`, rising linearly to 1 at `low-opt`, 1 up to `high-opt`, then falling linearly. Light curves are fractions of full sun. Moisture curves are fractions of the column's available water capacity. Temperature curves are in °C.
- **Grids.** Voxels are 1 m, indexed `x + width·(y + depth·z)` with z up. A column is 1 m² of ground. A patch is `world.patch × world.patch` columns. A bundle world also has a finer ground grid, 0.5 m on the Capitol, which only the water tier and the sun budget read.
- **Randomness.** One `ChaCha8Rng`, seeded from `--seed`, drives everything. Each rule says when it draws. A mechanism at rate 0 checks the rate outside its loop and makes no draw, so switching a mechanism off leaves the random stream of the rest of the run unchanged.

## The tick

`Sim::step_profiled` (`src/sim.rs`) runs these phases in this order every tick. Snapshots and the `series.csv` row are taken by the caller afterwards.

| # | Phase | When | Section |
|---|---|---|---|
| 1 | Animals | every tick, if `animals.enabled` | [Animals](#5-animals-not-the-focus) |
| 2 | Immigration: trees, grazers, hunters | on each species' own interval | [Trees](#4-trees), [Animals](#5-animals-not-the-focus) |
| 3 | Producers: grass and shrub cover | every tick, for the patches whose turn it is (`schedule.cover_every`) | [Ground cover](#3-nutrients-ground-cover-and-fire) |
| 4 | Sun: relight every column | when the season slice changes (bundle worlds) | [Sun light budget](#sun-light-budget) |
| 5 | Trees | every `tree.update_every` ticks | [Trees](#4-trees) |
| 6 | Fire: spread and burn-out, then ignition | every tick; ignition every `schedule.fire_every` ticks | [Fire](#fire-ignition) |
| 7 | Storm | every tick, if `hydro.enabled` | [Water](#2-water) |
| 8 | Soil: water settling, detritus decay, nutrients (or the legacy moisture and fertility paths) | every `schedule.soil_every` ticks | [Water](#2-water), [Nutrients](#3-nutrients-ground-cover-and-fire) |
| 9 | Temperature | every `schedule.temperature_every` ticks | [Temperature and season](#temperature-and-season) |
| 10 | Compaction of dead entities | every `world.compact_every` ticks | [Entity compaction](#entity-compaction) |

The state lives in three tiers with different update rates. Voxel fields (material, light) are rebuilt only on events. Column fields (soil water, the nutrient pools, moisture and fertility) change on storms and soil updates. Patch fields (grass, shrub, detritus, temperature, burning) change on the staggered cover update and on the soil and temperature cadences. Entities (trees and animals) are `Vec`s with an `alive` flag.

## 1. Ground, light and climate

Everything in this section is either built once at load (terrain, bundle ground, the sun budget, patch walking distances) or changes on a fixed calendar (season slice of light, patch temperature). Only the canopy light field changes on events (a tree planted, killed or changing stage). The rules draw from the RNG in exactly two places: the noise terrain and the lifespans of scene trees.

### Tick length and year

**Introduced:** SAD build, commit b69016f (later changes: shot G4b fixed the tick's duration in hours and made every rate per hour or per year; shot G4c moved the tree tier to years). **Runs:** constant. **Code:** `src/hydro.rs`, `tick_hours`, `years`; `src/params.rs`, `ticks_between`, `ticks_in_years`.

A year is `climate.year_len` ticks, and a tick is a fixed number of hours. Rates in params are per hour or per year and are charged over `cadence × tick_hours`, so changing a cadence leaves an annual total alone.

```text
tick_hours       = HOURS_PER_YEAR / year_len        HOURS_PER_YEAR = 8766 (mean Julian year)
                 = 8766 / 4000 = 2.1915 h
ticks_between(r) = max(1, round(year_len / r))      r events per year; r <= 0 gives u32::MAX (never)
ticks_in_years(y)= round(y × year_len)
phase(t)         = (t mod year_len) / year_len       0 = spring equinox
```

Note: the orchestration brief says 8760 h/year (2.19 h); the code uses 8766 h (2.1915 h), as UNITS.md also states.

**Parameters**
- `climate.year_len` (4000, ticks) -- ticks per model year; sets the tick's length, the season period of temperature and rain, and the sun's calendar.

### Noise terrain height

**Introduced:** SAD build, commit b69016f (later changes: shot 15 made the world size a parameter and added `world.slope_bias`). **Runs:** once at load (noise world only). **Code:** `src/world.rs`, `generate_heights`, `value_noise_octave`, `west_east`.

Two octaves of seeded 2D value noise (lattice periods 32 and 8 columns, amplitudes 1.0 and 0.35, smoothstep-interpolated, lattice drawn row by row x-fastest from the run's ChaCha8 RNG) are mapped piecewise-linearly so that the lowest `world.water_fraction` of columns fall below the water line. A west-east tilt is then added and the result rounded and clamped. The `world.height` voxel dimension caps it further when the world is built.

```text
v(c)   = o32(c) + 0.35 × o8(c)                      o_p ∈ [0,1): value noise of period p
lo, hi = min v, max v
q      = sorted(v)[clamp(floor(water_fraction × cols), 1, cols − 2)], clamped into (lo, hi)
hw     = water_level − 0.5
h'(c)  = height_min + (v − lo)/(q − lo) × (hw − height_min)       if v < q
       = hw        + (v − q)/(hi − q) × (height_max − hw)          otherwise
we(x)  = 2x/(width − 1) − 1                                       −1 west edge, +1 east edge
h(c)   = clamp(round(h'(c) − slope_bias × we(x)), height_min, height_max)
ground(c) = min(h(c), height − 1)
```

Because the tilt is applied after the quantile mapping, the realised share of flooded columns is not exactly `world.water_fraction` when `world.slope_bias` ≠ 0 (the high west floods less, the low east more).

**Parameters**
- `world.height_min` (8, voxels) -- lowest terrain height.
- `world.height_max` (24, voxels) -- highest terrain height.
- `world.water_level` (10, voxels) -- the flood line: noise columns with terrain below it are filled with water up to it; the quantile break sits at `water_level − 0.5`.
- `world.water_fraction` (0.04, fraction of columns) -- share of columns the quantile mapping puts below the water line.
- `world.slope_bias` (4.0, voxels) -- west-east tilt: +`slope_bias` at the west edge, −`slope_bias` at the east edge, linear between (higher, drier west). 0 leaves terrain untouched; absent means 0.

### World dimensions and patches

**Introduced:** SAD build, commit b69016f, as fixed 64×64×32 with 8×8 patches (later changes: shot 15 made them parameters; shot G1 lets a bundle set width and depth). **Runs:** once at load. **Code:** `src/world.rs`, `Dims`, `patch_distances`; `src/params.rs`, `check_dims`; `src/bundle.rs`, `Bundle::apply_to`.

The world is `width × depth` columns of `height` voxels, z up, indexed x-fastest; columns are grouped into square patches. At load each patch lists its Soil columns, and a multi-source BFS gives every column's 8-connected walking distance over Soil columns to the nearest Soil column of every patch (used by animals; terrain never changes, so it is computed once).

```text
vidx(x,y,z) = x + width × (y + depth × z)
cidx(x,y)   = x + width × y
patch(x,y)  = floor(x/patch) + (width/patch) × floor(y/patch)
patch_dist[p][c] = min BFS steps (8 neighbours, Soil only) from c to any Soil column of p;
                   u16::MAX if unreachable
Constraints: patch ≥ 1; width, depth multiples of patch in 1..=256; height in 2..=256
```

In a bundle world `world.width` and `world.depth` are overwritten with the bundle's `size_m`; `world.patch` and `world.height` stay as the params set them.

**Parameters**
- `world.width` (256, columns) -- columns along x (west to east). Absent means 64.
- `world.depth` (64, columns) -- columns along y (south to north). Absent means 64.
- `world.height` (32, voxels) -- voxels along z; every column top must be ≤ `height − 1`. Absent means 32.
- `world.patch` (8, columns) -- patch edge length. Absent means 8.

### Voxel fill and column classes (noise world)

**Introduced:** SAD build, commit b69016f. **Runs:** once at load. **Code:** `src/world.rs`, `World::from_heights`, `World::build`.

Each column is filled from z = 0 to its terrain height h: the top `world.soil_depth` voxels are soil, the rest rock. A noise column at or above `world.rock_top_height` has its top voxel turned to rock; air voxels from h + 1 up to `world.water_level` become water. The class is the material of the topmost non-air voxel.

```text
material(z) = SOIL  if h − soil_depth < z ≤ h
            = ROCK  if z ≤ h − soil_depth
material(h) = ROCK  if h ≥ rock_top_height
material(z) = WATER for h < z ≤ min(water_level, height − 1)
top(c)      = max z with material ≠ AIR   (the water surface on a flooded column)
class(c)    = Soil | Water | Rock  from material(top)
```

Only Soil columns are plantable (`World::is_plantable`); grass, shrub and trees live only there and animals walk only over them. With `world.soil_depth = 0` every column is Rock-topped.

**Parameters**
- `world.soil_depth` (3, voxels) -- soil layers on top of the rock fill; applies to bundle columns too.
- `world.rock_top_height` (21, voxels) -- terrain at or above this is rock-capped (noise world only).
- `world.water_level` -- see Noise terrain height; floods only in a noise world.

### Bundle ground and column classes

**Introduced:** shot G1 (later changes: shot G2 wrote the Capitol bundle; shot S9 added `latitude_deg`; shot G12 added the `roofed` mark). **Runs:** once at load (`ecosim run --world <dir>`). **Code:** `src/bundle.rs`, `Bundle::load`, `Bundle::apply_to`, `Bundle::write_world_dir`; `src/world.rs`, `World::from_bundle`.

A bundle (format in `docs/SCENE-CONTRACT.md`, version 2 only) is a square crop of `size_m` metres with a finer ground grid of `ratio = 1/ground_cell_m` cells per metre (a whole number), carrying ground height, medium code and building height per cell, plus trees, shrubs and pipes. The ecology grid stays at 1 m columns, each covering `ratio²` ground cells. A column's surface layer is `base_z` plus its rounded mean ground height; the noise world's `rock_top_height` and flood rules are not applied: the media decide the top by strict majority.

```text
n        = ratio²                                  ground cells per column
h(c)     = bundle.base_z + round( Σ ground_h(i) / n )    must be in 0..=height−1, else load error
sealed   = #{ i : medium(i) ≠ water  and  not medium.<m>.plantable }
water    = #{ i : medium(i) = water }
top(c)   = ROCK   voxel at h   if 2·sealed > n
         = WATER  voxel at h   else if 2·water > n
         = soil fill unchanged otherwise           (a tie is soil)
roofed(c)= any ground cell under c has medium roof
```

The fill below h is the same soil-over-rock fill as the noise world (`world.soil_depth`). A `roofed` column may still be Soil (a lawn under an eave keeps grass and shrub) but no trunk may root there (`World::can_root_a_trunk`, shot G12). The ground grid, `ground_h`, `building_h` and pipes are kept on the world; the bundle's `ground_h.f32`, `medium.u8`, `building_h.f32` and `pipes.json` are copied to the run's `world/` directory. A noise world instead carries a 1 m mirror ground grid: medium soil everywhere except water columns, ground height = terrain height, building height 0.

Code/comment note: the doc comments on `World::from_bundle`, `Medium::is_sealed` and DECISIONS.md (shot G1) say "sealed" means roof, asphalt or concrete. The code counts any non-water medium whose `medium.<m>.plantable` is false. At the shipped params the two sets are identical; they diverge if a params file makes another medium unplantable.

**Parameters**
- `bundle.base_z` (8, voxels) -- soil and rock under the bundle's lowest ground; a column's surface layer is `base_z + round(mean ground height in m)`.
- `medium.<m>.plantable` -- documented with the media (water tier); here it decides which cells count as sealed.
- `world.soil_depth` -- see Voxel fill and column classes.

### Canopy light (Beer-Lambert)

**Introduced:** SAD build, commit b69016f, as linear `255 − canopy_absorb × layers` (later changes: shot G1 added a per-column building-shade floor, shot G4c replaced the linear rule with Beer-Lambert transmittance as a fraction of full sun, shot G9 replaced the shade floor with the sun budget factor). **Runs:** at load for every column; for the 3×3 columns around a trunk when a tree is planted, dies or changes stage; for every column when the sun's season slice changes. **Code:** `src/world.rs`, `set_column_light`, `surface_light_fraction`; `src/trees.rs`, `canopy_z`, `refresh_canopy_columns`, `plant_tree`, `kill_tree`; `src/params.rs`, `canopy_extinction`.

Light is a per-voxel byte, 255 × the fraction of full sun reaching it. Solid voxels are 0; air and water voxels get the column's sun factor times one Beer-Lambert transmittance per distinct canopy layer strictly above them. Canopy voxels come from live trees: a young tree puts one voxel at `h_trunk + 2` over its own column, a mature tree two voxels at `h_trunk + 2` and `h_trunk + 3` over the 3×3 columns around its trunk (h_trunk = the trunk column's top); layers at the same z from different trees count once. Every light curve in the params reads the surface fraction.

```text
τ            = max(0, world.canopy_k × world.canopy_lai)       optical depth per canopy voxel
Z_c          = distinct canopy voxel z over column c
n(z)         = #{ z' ∈ Z_c : z' > z }
light(c, z)  = 0                                        if material is SOIL or ROCK
             = round( 255 × S(c) × exp(−τ × n(z)) )     otherwise
S(c)         = sun factor of c in the current season slice (see Sun light budget); 1 in a noise world
surface_light_fraction(c) = light(c, top(c) + 1) / 255
canopy_cover(c) = (Z_c non-empty)
```

At the defaults τ = 1.0: one canopy voxel passes exp(−1) = 36.8%, a mature crown (two voxels, LAI 4) exp(−2) = 13.5%, inside the published 10-25% for a broadleaf canopy at LAI 3-5 (UNITS.md R10). Leaf-off (shot G10) does not change the light field: a bare crown still shades.

**Parameters**
- `world.canopy_k` (0.5, dimensionless) -- Beer-Lambert extinction coefficient.
- `world.canopy_lai` (2.0, m² leaf per m² ground) -- leaf area index of one canopy voxel.

### Sun light budget

**Introduced:** shot G9 (replacing the fixed 45° building shade of shots G1/G2; later changes: none). **Runs:** budget computed once at load (bundle worlds only); `update_sun` every tick, relighting all columns only when the season slice changes (ticks 500, 1500, 2500, 3500 of each year at the defaults). **Code:** `src/sun.rs`, `SunBudget::compute`, `sun_rays`, `sky_rays`, `Tops::horizon`, `SunBudget::slice_of`, `SunBudget::factor`; `src/trees.rs`, `Sim::update_sun`; `src/world.rs`, `sun_factor`.

For each ecology column and each of `sun.season_samples` days of the year, a light byte combines the share of an evenly bright sky the buildings leave open with the altitude-weighted share of the sun's positions that reach the column, reduced by the share of the column under a building. Rays are walked cell by cell over the ground grid from the column centre at the column's mean ground height, and a ray is blocked by any building top (`ground_h + building_h`) above it; the column's own cells are skipped and the walk stops at the grid edge. The light field uses the byte over the open-sky byte, so an unshaded column has factor exactly 1, and noise worlds (no buildings, no budget) are unchanged.

```text
phase_k   = k / N,  N = season_samples                slice k's day; 0 = spring equinox
δ_k       = 23.44° × sin(2π phase_k)                   declination
cos H0    = clamp(−tan φ tan δ, −1, 1)                 φ = latitude
H_i       = −H0 + (i + 0.5) × 2H0 / M,  i < M = day_samples
sun ray i : unit vector (east, north, up); kept if up > 0; weight w_i = up = sin(altitude)
beam_k(c) = Σ_{i unblocked} w_i / Σ_i w_i              (1 if no buildings or no sun)
sky(c)    = w_cap + Σ_{rings, azimuths} w_ring/16 × [tan(alt_ring) ≥ horizon(c, az)]
            rings: bands 0-30°, 30-60°, 60-82.5° sampled at mid-altitude, 16 azimuths
            w_band = sin²(top) − sin²(bottom);  w_cap = 1 − sin²(82.5°) ≈ 0.017 (always open)
roof(c)   = #{ground cells under c with building_h > 0} / ratio²
d         = clamp(diffuse_fraction, 0, 1);  b = (1 − d)(1 − clamp(cloud_cover, 0, 1))
v_k(c)    = (d × sky(c) + b × beam_k(c)) × (1 − roof(c))
byte_k(c) = clamp(round(255 v), 0, 255), raised to 1 if v > 0
open      = max(1, byte(d + b))                        179 at the defaults
S(c)      = byte_k(c) / open,   k = round(phase(t) × N) mod N
```

The budget is written to `world/sun.bin` (N planes of width × depth bytes, slice-major) and described by `meta.json` `world.sun`. Latitude is the bundle's `latitude_deg` when present, else `sun.latitude`, clamped to ±90°. The budget is per column, not per height: a crown above a low roof is charged the same as the ground under it. `roof(c)` here counts cells with building height > 0, while the `roofed` trunk bar (shot G12) counts cells whose medium is `roof`; the two agree for a consistent export.

**Parameters**
- `sun.latitude` (42.7, degrees north) -- latitude of a bundle world whose `bundle.json` has no `latitude_deg`; never read by a noise world.
- `sun.cloud_cover` (0.5, fraction) -- mean share of the direct beam lost to cloud (Lansing, about 50% of possible sunshine).
- `sun.day_samples` (9, count) -- sun positions per sampled day, sunrise to sunset; 0 is read as 1.
- `sun.season_samples` (4, count) -- sampled days (slices) per year from the spring equinox; 4 = both equinoxes and both solstices; 0 is read as 1.
- `sun.diffuse_fraction` (0.4, fraction) -- share of daylight arriving from the whole sky rather than the sun's disc.
- `climate.year_len` -- see Tick length and year; sets which slice a tick falls in.

### Temperature and season

**Introduced:** SAD build, commit b69016f (later changes: the sweep-harness commit 86e3290 moved the swing from `climate.temp_amp` to `season.amplitude`; its value went 12 to 15 in the dynamics-fixes tuning (TUNING.md); shot G4b made the cadence `schedule.temperature_every`). **Runs:** once at load (tick 0), then every `schedule.temperature_every` ticks. **Code:** `src/abiotic.rs`, `Sim::update_temperature`, `canopy_columns`.

Each patch's temperature is a sinusoidal seasonal value, the same everywhere, cooled in proportion to the share of the patch's columns under any canopy voxel. The sine starts rising at tick 0, the spring equinox (the rain term uses the opposite sign and is documented with the water rules).

```text
season(t)  = temp_base + amplitude × sin(2π t / year_len)
T_p(t)     = season(t) − canopy_cool × covered_p / patch²
covered_p  = #{ columns c in patch p : canopy_cover(c) }
```

`t` is the absolute tick in f32 (not reduced mod `year_len`). The value is held between updates, so everything that reads patch temperature (evaporation, decay, leaf-on, fire) sees a step function refreshed 40 times a year at the defaults.

**Parameters**
- `climate.temp_base` (12.0, °C) -- annual mean temperature.
- `season.amplitude` (15.0, °C) -- half the annual swing: temperature runs `temp_base ± amplitude`.
- `climate.canopy_cool` (3.0, °C) -- cooling of a patch fully under canopy.
- `schedule.temperature_every` (100, ticks) -- update cadence.
- `climate.year_len` -- see Tick length and year.

### Scene planting from a bundle

**Introduced:** shot G3 (later changes: shot G4c expressed ages in years via `tree.mature_age_years` and `bundle.tree_tall_age_years`; shot G12 kept trunks off roofed columns; shot S3 added the inverse, `height_of_age`). **Runs:** once at load (bundle worlds only, in place of `tree.initial_count` random trees). **Code:** `src/plants.rs`, `Sim::import_scene`, `import_age`, `trunk_column`, `in_shrub`, `height_of_age`.

Each scene tree is mapped to a trunk column: its own column (floor of its position, clamped into the grid) if a trunk may root there, else the nearest such column within `bundle.tree_move_radius` (offsets sorted by squared distance, then dy, then dx), else it is dropped. When several reach one column the tallest wins (first listed on a tie). Winners are planted in ascending column order at an age read off a piecewise-linear height-to-age curve; each `plant_tree` draws one lifespan from the RNG. Shrub ellipses then raise the starting shrub density of each patch by the share of its Soil columns whose centre lies inside any ellipse.

```text
A_m = ticks_in_years(tree.mature_age_years)
A_t = max(A_m, ticks_in_years(bundle.tree_tall_age_years))
H_m = bundle.tree_mature_height,  H_t = bundle.tree_tall_height
age(H) = 0                                   if H ≤ 0 or NaN
       = A_m × H / H_m                        if H < H_m
       = A_m + (A_t − A_m)(H − H_m)/(H_t − H_m)  if H < H_t
       = A_t                                  otherwise;   rounded to ticks
trunk site ok(c) = class(c) = Soil and not roofed(c)
shrub cover:  (u, v) = rotate(p − centre, −angle);  inside if (u/rx)² + (v/ry)² ≤ 1
shrub_p ← clamp(shrub_p + covered Soil columns of p / Soil columns of p, 0, 1)
```

At the defaults a 3 m tree starts at 1000 ticks (mature), a tree of 20 m or more at 3000 ticks (0.75 yr), half the mean lifespan (1.5 yr). Surprise: scene planting does not apply `tree.min_spacing`; only one trunk per column is enforced, so imported trunks can stand on neighbouring columns where the random and seeded placements could not.

**Parameters**
- `bundle.tree_mature_height` (3.0, m) -- scene height that starts at `tree.mature_age_years`.
- `bundle.tree_tall_height` (20.0, m) -- scene height at and above which a tree starts at `bundle.tree_tall_age_years`.
- `bundle.tree_tall_age_years` (0.75, years) -- starting age of a tall scene tree; read as at least the mature age.
- `bundle.tree_move_radius` (2.0, columns = m, Euclidean) -- how far a trunk on an unplantable or roofed column may move; beyond it the tree is dropped.
- `tree.mature_age_years` -- documented with the tree rules; the middle point of the curve.

### Random stream

**Introduced:** shot 14. **Runs:** once at load. **Code:** `src/sim.rs`, `Sim::new`, `Sim::from_bundle`.

All randomness comes from one `ChaCha8Rng` seeded from the CLI `--seed`. In a noise world the terrain is drawn first on stream 0, then the stream is switched, so a nonzero stream replays the same terrain with independent dynamics. A bundle world draws nothing for terrain, so the stream is set before anything is drawn.

```text
rng = ChaCha8Rng::seed_from_u64(seed)
noise world:  heights ← draws(rng);  if stream ≠ 0: rng.set_stream(stream)
bundle world: if stream ≠ 0: rng.set_stream(stream)
```

**Parameters**
- `rng.stream` (0, integer) -- ChaCha8 stream for everything after the terrain; 0 reproduces every run before shot 14.

### Entity compaction

**Introduced:** SAD build, commit b69016f. **Runs:** every `world.compact_every` ticks, last in the tick. **Code:** `src/sim.rs`, `Sim::compact`.

Dead grazers, hunters and trees (`alive = false`) are removed from their vectors, preserving order, and the grazer grid and the trunk index (`trunk_at`) are rebuilt. It changes no ecology, only storage; light and canopy cover were already updated when each tree died.

```text
if t mod compact_every = 0:
    grazers, hunters, trees ← retain(alive)
    trunk_at[col(tree_i)] ← i  for the compacted trees
```

With `compact_every = 0` the test `t.is_multiple_of(0)` is false for every t ≥ 1, so compaction never runs.

**Parameters**
- `world.compact_every` (100, ticks) -- compaction interval.

## 2. Water

The water tier (shot G4) moves rain over the ground grid and stores it in the soil of the ecology
grid. There are two stores, both f64 in memory. Ponded water `pond[i]` is in mm over ground cell
`i`: 0.5 m cells in the Capitol bundle, 1 m cells in a noise world. Soil water `soil[c]` is in mm
over the 1 m ecology column `c`. `per_col` is the number of ground cells per column (4 on the
Capitol, 1 on a noise world), so `x` mm over one ground cell is `x / per_col` mm over its column.
Rates are per hour and are charged over `hours = cadence × tick_hours`:

```text
tick_hours = HOURS_PER_YEAR / climate.year_len = 8766 / 4000 = 2.1915 h
```

Note: the code's `hydro::HOURS_PER_YEAR` is 8766 (the Julian year), not 8760.

Each tick, the storm runs after fire and before the soil update. The soil update runs every
`schedule.soil_every` ticks and does the between-storm settling. Every other tier reads the
`moisture` field and `water_fraction` that the storm and the settling derive.

### Water tier switch

**Introduced:** shot G4. **Runs:** once at load (the `Hydro` store is allocated or not), then each tick in `Sim::step_profiled`. **Code:** `src/sim.rs`, `step_profiled`, `assemble`; `src/abiotic.rs`, `update_soil`.

When the tier is on, a storm draw happens every tick and the soil update runs `settle_water`. When
it is off, no store is allocated, no storm is drawn, no water file is written, and the pre-G4
moisture update runs instead (see "Legacy moisture update"). The six `series.csv` water columns
are then always 0.

```text
each tick t:      if hydro.enabled: storm(t)
if t mod soil_every == 0:
                  if hydro.enabled: settle_water(soil_every × tick_hours)
                                    then detritus decay, then nutrient update
                  else:             legacy moisture steps 1-5
                                    then detritus decay, then nutrient update
```

**Parameters**
- `hydro.enabled` (true, bool): runs the water tier. false restores the pre-G4 moisture path.
- `schedule.soil_every` (10, ticks): the cadence of the soil update. With the tier on, the settling step is charged over `soil_every × tick_hours` hours (21.9 h at the defaults). The cadence of the legacy path's steps is the same, but that path charges them per update, not per hour.

### Media table

**Introduced:** shot G4 (later changes: shot G4b redefined `field_capacity_mm` as the available water capacity of the rooting zone, so a store of 0 is the wilting point and not dry soil; no medium value has changed since G4). **Runs:** once at load. **Code:** `src/params.rs`, `MediaParams::get`; `src/hydro.rs`, `Hydro::new`; `src/world.rs`, `World::from_bundle`.

Each ground cell has one of nine media, read from the bundle's `medium.u8`. A noise world is
synthesized as all `soil`, except that its water columns are `water`. Every medium sets three
rates and one flag: how fast water soaks in, how much plant-available water a column of it holds,
how fast water above field capacity drains out of the bottom, and whether anything roots in it.
The `plantable` flag feeds the bundle "Rock rule". It does not enter any water equation.

```text
column class (bundle world), from the n = per_col ground cells of the column:
  sealed = #cells with medium != water and plantable == false
  wet    = #cells with medium == water
  class  = Rock    if 2·sealed > n
           Water   if 2·wet    > n
           Soil    otherwise
```

Rock and Water columns get zero field capacity and zero percolation (see "Column stores"). Trees,
shrub and grass start only on Soil columns.

**Parameters** (one bullet per key)
- `medium.soil.infiltration_mm_h` (20.0, mm/h): the fastest rate water soaks into a soil cell.
- `medium.soil.field_capacity_mm` (150.0, mm): the available water capacity of a soil cell.
- `medium.soil.percolation_mm_h` (4.0, mm/h): drainage out of the bottom of a soil cell when it is above field capacity. With the tier off, the legacy plant draw also uses `medium.soil.field_capacity_mm` to convert mm into the moisture index (see "Plant water draws").
- `medium.soil.plantable` (true, bool): plants root in soil.
- `medium.lawn.infiltration_mm_h` (15.0, mm/h): lawn infiltration rate.
- `medium.lawn.field_capacity_mm` (150.0, mm): lawn available water capacity.
- `medium.lawn.percolation_mm_h` (3.0, mm/h): lawn percolation rate.
- `medium.lawn.plantable` (true, bool): plants root in lawn.
- `medium.bed.infiltration_mm_h` (30.0, mm/h): planting-bed infiltration rate.
- `medium.bed.field_capacity_mm` (200.0, mm): planting-bed available water capacity.
- `medium.bed.percolation_mm_h` (6.0, mm/h): planting-bed percolation rate.
- `medium.bed.plantable` (true, bool): plants root in a bed.
- `medium.mulch.infiltration_mm_h` (40.0, mm/h): mulch infiltration rate.
- `medium.mulch.field_capacity_mm` (200.0, mm): mulch available water capacity.
- `medium.mulch.percolation_mm_h` (8.0, mm/h): mulch percolation rate.
- `medium.mulch.plantable` (true, bool): plants root in mulch.
- `medium.gravel.infiltration_mm_h` (60.0, mm/h): gravel infiltration rate.
- `medium.gravel.field_capacity_mm` (50.0, mm): gravel available water capacity.
- `medium.gravel.percolation_mm_h` (30.0, mm/h): gravel percolation rate.
- `medium.gravel.plantable` (true, bool): plants root in gravel.
- `medium.concrete.infiltration_mm_h` (1.0, mm/h): concrete infiltration rate. It is non-zero but holds nothing, because every sealed-majority column is Rock and has zero capacity. On a Soil column, though, a minority concrete cell can still soak water into that column's store.
- `medium.concrete.field_capacity_mm` (0.0, mm): concrete holds no soil water.
- `medium.concrete.percolation_mm_h` (0.0, mm/h): concrete percolation rate.
- `medium.concrete.plantable` (false, bool): concrete is sealed and counts toward Rock.
- `medium.asphalt.infiltration_mm_h` (0.5, mm/h): asphalt infiltration rate. The same capacity caveat as concrete applies.
- `medium.asphalt.field_capacity_mm` (0.0, mm): asphalt holds no soil water.
- `medium.asphalt.percolation_mm_h` (0.0, mm/h): asphalt percolation rate.
- `medium.asphalt.plantable` (false, bool): asphalt is sealed and counts toward Rock.
- `medium.roof.infiltration_mm_h` (0.0, mm/h): roof infiltration rate. Roof cells also have no depression storage and are routed to a downspout (see "Flow graph").
- `medium.roof.field_capacity_mm` (0.0, mm): a roof holds no soil water.
- `medium.roof.percolation_mm_h` (0.0, mm/h): roof percolation rate.
- `medium.roof.plantable` (false, bool): a roof is sealed and counts toward Rock. A column with any roof cell is also `roofed`, which keeps trunks out of it.
- `medium.water.infiltration_mm_h` (0.0, mm/h): open-water infiltration rate. A water cell is a flow outlet, so what reaches it leaves the world.
- `medium.water.field_capacity_mm` (0.0, mm): open water holds no soil water.
- `medium.water.percolation_mm_h` (0.0, mm/h): open-water percolation rate.
- `medium.water.plantable` (false, bool): this value is never read. The Rock rule excludes `water` explicitly and counts water cells separately.

### Column stores

**Introduced:** shot G4 (later changes: shot G4b made `hydro.initial_fill` a fraction of available water capacity). **Runs:** once at load. **Code:** `src/hydro.rs`, `Hydro::new`.

An ecology column's field capacity and percolation rate are the means over its ground cells of the
medium values. They are zeroed on any column that is not Soil. Infiltration stays per ground cell.
The column's store starts partly full, and nothing starts ponded.

```text
capacity[c]    = Σ_{i in c} medium(i).field_capacity_mm / per_col      (0 if class[c] != Soil)
percolation[c] = Σ_{i in c} medium(i).percolation_mm_h  / per_col      (0 if class[c] != Soil)
store_max[c]   = capacity[c] × max(1, hydro.saturation)
soil[c](0)     = capacity[c] × clamp(hydro.initial_fill, 0, 1)
pond[i](0)     = 0
```

**Parameters**
- `hydro.initial_fill` (0.5, fraction): the soil water of every column at tick 0, as a fraction of its available water capacity.
- `hydro.saturation` (1.2, multiple of field capacity, floored at 1): the most a column's soil can hold. The water between `capacity` and `store_max` is what percolates away. At 1.0 there is no drainage and so no leaching.

### Flow graph

**Introduced:** shot G4 (later changes: shot G6 added pipe placement and the acyclicity check on the same graph, and shot S10 publishes the largest depression storage as the `water` overlay's scale). **Runs:** once at load. The graph is never rebuilt. **Code:** `src/hydro.rs`, `Flow::build`, `topological`, `Flow::pond_ramp_mm`.

A priority flood (Barnes et al. 2014) runs over the ground surface in mm. It starts from every
outlet: each edge cell and each open-water cell. It gives every cell a spill level `filled` and a
receiver `recv`, which is the neighbour it was reached from. A roof is lifted above the highest
ground so that the flood reaches it last. Every cell of a connected roof then sends its water to
one downspout. Kahn's topological sort turns the receiver tree into a visiting order in which
every cell comes before its receiver.

```text
elev[i]     = 1000 × (ground_h[i] + (roof(i) ? max(ground_h) + 1 : 0))          mm
outlet(i)   = i on the grid edge  or  medium(i) == water;   recv[outlet] = OUT
flood:      pop the lowest (key(filled), index) cell c from a min-heap; for each unvisited
            4-neighbour d (order N, W, E, S):
              filled[d] = max(elev[d], filled[c]);   recv[d] = c
roofs:      for each 4-connected roof component R:
              spout = the lowest-index non-roof cell 4-adjacent to R (OUT if none)
              recv[i] = spout  for every i in R
pond_cap[i] = 0                               if roof(i) or outlet(i)
            = max(0, filled[i] − elev[i])     otherwise                          mm
order       = Kahn's sort of recv, upstream first; any cycle left (for example a courtyard
              whose only way out is over a roof) is broken by setting recv of its
              lowest-index cell to OUT
water overlay scale: lo = 1 mm;  hi = the smallest 100 × 10^k ≥ max_i pond_cap[i]
```

Ties in the flood are broken by cell index through an order-preserving u32 key of the f32 level, so
the graph is deterministic. Note: DECISIONS.md (shot G4) says that roofs are "lifted by their
building height" and routed by "BFS from each pipe inlet". The code lifts every roof by the highest
ground plus 1 m. It finds each downspout with a BFS over the roof component that takes the
lowest-index adjacent non-roof cell, and no pipe is involved.

**Parameters**
- None. The graph depends only on the bundle's `ground_h`, its media, and which media are `roof` and `water`.

### Storm draw

**Introduced:** shot G4 (later changes: shot G4b replaced the per-tick `rain.storm_p` with `rain.annual_mm`, recalibrated `rain.storm_mean_mm` from 10 to 6 mm, and added the seasonal factor). **Runs:** every tick, if `hydro.enabled`. **Code:** `src/hydro.rs`, `Sim::storm`; `src/abiotic.rs`, `Sim::rain`.

Each tick is either dry or has one storm, and the whole depth of a storm falls and routes in its
tick. The storm chance keeps the expected annual depth at `annual_mm`, modulated by the season. It
takes one RNG draw on a dry tick and two on a wet one, and the chance draw is made even when
`annual_mm` is 0.

```text
season(t) = max(0, (climate.rain_base − climate.rain_amp × sin(2π t / year_len)) / climate.rain_base)
            (0 if rain_base ≤ 0; its mean over a year is 1)
p(t)      = clamp(rain.annual_mm / (year_len × rain.storm_mean_mm) × season(t), 0, 1)
u ~ U[0,1):  if u ≥ p: no storm
v ~ U[0,1):  depth = −rain.storm_mean_mm × ln(1 − v)          mm, exponential with mean storm_mean_mm
```

At the defaults, p = 800/(4000 × 6) × (1 − 0.5 sin(2πt/Y)), which is 0.0333 on average, or about
133 storms a year. With the tier on, `climate.rain_base` and `climate.rain_amp` enter only through
this ratio, so their absolute size does not matter here.

**Parameters**
- `rain.annual_mm` (800.0, mm/year): the expected rain in a year. 0 turns the rain off but keeps the draw.
- `rain.storm_mean_mm` (6.0, mm): the mean storm depth, drawn exponentially.
- `climate.rain_base` (8.0; with the tier on it is a dimensionless reference level, and on the legacy path it is moisture-index units per soil update): the mean of the seasonal rain curve.
- `climate.rain_amp` (4.0, same units as `climate.rain_base`): the seasonal swing. Rain peaks at t = 3Y/4 and is lowest at t = Y/4, which is when temperature (`+sin`) peaks, so the warm half of the year is the dry half.
- `climate.year_len` (4000, ticks) is referenced here and documented with the climate rules.

### Rain gradient

**Introduced:** shot 15 (later changes: shot G3a runs every bundle world with the value overridden to 0 on the command line, which leaves the default alone; shot G4 applies it to storm depth). **Runs:** inside every storm, per ground cell, and on the legacy path at every soil update. **Code:** `src/abiotic.rs`, `rain_at`; `src/world.rs`, `west_east`.

Storm depth, or legacy rain, is scaled linearly from west to east by the ecology column's x. The
term is odd about the middle of a row, so a row's total rain is unchanged.

```text
west_east(x) = 2x / (width − 1) − 1                          (−1 at the west edge, +1 at the east)
rain_at(r, x) = max(0, r × (1 + climate.rain_gradient × west_east(x)))
```

**Parameters**
- `climate.rain_gradient` (0.6, dimensionless; the loader accepts |g| ≤ 1): the west-to-east rain ramp. 0 gives uniform rain and is what every bundle-world run uses, passed as `--set climate.rain_gradient=0`.

### Storm routing: infiltration, ponding and runoff

**Introduced:** shot G4 (later changes: shot G5 carries nitrogen and phosphorus along with the water, which belongs to the nutrient section; shot G6 adds drain capture and extra passes). **Runs:** on each storm tick. **Code:** `src/hydro.rs`, `Sim::route_storm`.

A single pass over `order` visits every ground cell before its receiver. The water arriving at a
cell is its own rain plus the run-on from upstream. Drains on the cell take their share first (see
"Storm drains"). Then the cell infiltrates, limited by its medium's rate over one tick and by the
room left in its column. Then it fills its depression storage. The remainder moves to `recv`, or
out of the world if `recv` is `OUT`.

```text
for i in order:
  fall   = rain_at(depth, x of col(i))            (0 in a drain follow-up pass)
  w      = fall + runon[i]  − captured_by_drains(i)
  rate   = medium(i).infiltration_mm_h × tick_hours  (− already infiltrated this storm, if tracked)
  room   = min(rate, max(0, store_max[c] − soil[c]) × per_col)
  take   = min(w, room);        soil[c] += take / per_col;    w −= take
  store  = min(w, max(0, pond_cap[i] − pond[i]));  pond[i] += store;  w −= store
  runoff += (w + captured) × fall / (fall + runon[i])       (the cell's own-rain share)
  if recv[i] == OUT: outflow += w   else: runon[recv[i]] += w
```

A cell cannot hold more than `pond_cap` for as long as the next cell visited, so no snapshot shows
standing water deeper than the spill level. After the pass, `moisture` is re-derived for every
column and one `storm` event is logged with `(rain_mm, runoff_mm, outflow_mm)`.

**Parameters**
- `medium.<m>.infiltration_mm_h`, see "Media table" (for example `medium.lawn.infiltration_mm_h`): charged over `tick_hours` inside a storm. At the defaults lawn takes 32.9 mm per tick.
- `hydro.saturation`, see "Column stores": sets `store_max`, which bounds `room`.
- `climate.rain_gradient`, see "Rain gradient".

### Storm drains

**Introduced:** shot G6. **Runs:** placement once at load; capture on each storm tick when `pipes.capacity_scale` > 0 and the world has pipes. **Code:** `src/hydro.rs`, `place_drains`, `Drain::capacity_mm`, `Sim::route_storm`.

Each pipe in the bundle's `pipes.json` has an inlet cell, the ground cell under its first point
(clamped into the grid). Its outlet is the ground cell under its last point, or `OUT` if that
point is on an edge cell or off the grid. When the routing pass reaches an inlet cell, the drains
there take what arrives, rain and run-on together, before anything infiltrates. A drain takes up to
its remaining capacity for the tick. Water sent down a pipe to `OUT` is outflow. Water delivered to
an interior outlet is put down as run-on at that cell and routed in a further rain-free pass. Passes
repeat until no drain delivers anything inside the world, which takes at most one extra pass per
drain. A drain network in which a pipe's outlet runs downhill, directly or through other pipes, to
its own inlet is refused at load, and the cycle is named in the error.

```text
cap_mm(d)   = capacity_m3h(d) × tick_hours × pipes.capacity_scale × 1000 / cell_area    per tick
at inlet i, for each drain d on i (sorted by inlet cell, then pipe index):
  got = min(w, left[d]);  left[d] −= got;  w −= got
  arrived[d] += w_before;  captured[d] += got;  overflow = arrived − captured
after each pass: d.outlet == OUT → pipe_out_edge += taken[d]  (charged to ledger outflow)
                 otherwise       → runon[d.outlet] += taken[d]; run another pass
in a follow-up pass a cell infiltrates at most rate − infiltrated_this_storm[i]
```

`capacity_m3h` comes from `pipes.json`, and a negative or non-finite value is refused by the
bundle loader. Each storm logs one `pipe` event per drain with its captured and overflow volumes
in m³ and the N and P it carried.

**Parameters**
- `pipes.capacity_scale` (1.0, multiplier): multiplies every pipe's `capacity_m3h`. 0 turns the network off: there is no capture, no extra pass and no `pipe` event, and the run is byte-identical to one without drains apart from the two always-zero `series.csv` pipe columns. It changes nothing on a noise world, which has no pipes.

### Settling: ponded infiltration and evaporation

**Introduced:** shot G4 (later changes: shot G4b made the rates per hour and set `hydro.evap_mm_h` to 0.08, up from 0.05). **Runs:** every `schedule.soil_every` ticks, if `hydro.enabled`. **Code:** `src/hydro.rs`, `Sim::settle_water`, `Sim::temp_factor`.

Between storms, ponded water first soaks into its column at the medium's rate over the whole
settling window. Whatever is still standing then evaporates at a rate scaled by the patch
temperature.

```text
hours   = schedule.soil_every × tick_hours
tf(p)   = max(0, evap_base + T_p / evap_div) / max(0, evap_base + temp_base / evap_div)
          (0 if the denominator ≤ 0; 1 at T = climate.temp_base; 2 + 12/8 = 3.5 at the defaults)
for each ground cell i with pond[i] > 0, column c, patch p:
  take     = min(pond[i], min(infiltration(i) × hours, max(0, store_max[c] − soil[c]) × per_col))
  soil[c] += take / per_col;   pond[i] −= take
  evap     = min(pond[i], hydro.evap_mm_h × hours × tf(p));   pond[i] −= evap
```

**Parameters**
- `hydro.evap_mm_h` (0.08, mm/h at the mean temperature): open-water evaporation from ponded cells, 701 mm a year.
- `climate.evap_base` (2.0; on the legacy path it is moisture-index units per soil update): the intercept of the temperature factor. With the tier on only the ratio `tf` matters.
- `climate.evap_div` (8.0, °C per unit): the temperature factor rises by 1 per `evap_div` °C.
- `climate.temp_base` (12.0, °C) is referenced here as the normalising temperature and documented with the climate rules.

### Percolation (drainage)

**Introduced:** shot G4 (later changes: shot G4b made the rates per hour). **Runs:** every `schedule.soil_every` ticks, if `hydro.enabled`, after the ponded step. **Code:** `src/hydro.rs`, `Sim::settle_water`.

Soil water above field capacity drains out of the bottom of the column at the column's mean
percolation rate. The amount drained per column is returned to the soil update for leaching.

```text
for each column c with capacity[c] > 0:
  excess     = max(0, soil[c] − capacity[c])        (soil ≤ store_max = capacity × saturation)
  drained[c] = min(excess, percolation[c] × hours)
  soil[c]   −= drained[c]
```

**Parameters**
- `medium.<m>.percolation_mm_h`, see "Media table" (for example `medium.soil.percolation_mm_h`): averaged per column in "Column stores".
- `medium.<m>.field_capacity_mm`, see "Media table": sets `capacity`.
- `hydro.saturation`, see "Column stores".

### Evapotranspiration

**Introduced:** shot G4 (later changes: shot G4b made it per hour and set `hydro.et_mm_h` to 0.05, down from 0.12). **Runs:** every `schedule.soil_every` ticks, if `hydro.enabled`, after percolation on the same column. **Code:** `src/hydro.rs`, `Sim::settle_water`.

Every column that has capacity loses soil water to evaporation and transpiration. The loss is
scaled by the patch temperature factor and by the patch's grass and shrub cover, with a floor of
0.2 for bare soil. This is the standing transpiration of ground cover. Tree transpiration and the
cost of new cover growth are separate draws (see "Plant water draws").

```text
cover(p) = clamp(grass_p + shrub_p, 0, 1)
want     = hydro.et_mm_h × hours × tf(p) × (0.2 + 0.8 × cover(p))
soil[c] −= min(soil[c], want)
```

**Parameters**
- `hydro.et_mm_h` (0.05, mm/h at the mean temperature and full cover): soil evaporation plus transpiration, 438 mm a year at full cover.
- `climate.evap_base`, `climate.evap_div`, see "Settling" (through `tf`).

### Derived moisture and water fraction

**Introduced:** shot G4 (later changes: shot G4b added `water_fraction`, the scale that every plant moisture curve is read on). **Runs:** at load, after every storm, after every settling step, and after every plant draw on the drawn column. **Code:** `src/hydro.rs`, `Sim::derive_moisture`, `Sim::water_fraction`.

Moisture is not stored with the tier on. It is derived from soil water, kept as f32 in memory, and
quantised to u8 only when `moisture.bin` is written. Plants read the unclamped fraction, which runs
above 1 while a column drains.

```text
moisture[c]       = capacity[c] > 0 ? 255 × min(1, soil[c] / capacity[c]) : 0
water_fraction[c] = hydro on:  capacity[c] > 0 ? soil[c] / capacity[c] : 0     (range 0 to saturation)
                    hydro off: moisture[c] / 255
```

**Parameters**
- `climate.initial_moisture` (128.0, moisture index 0-255): the moisture of every Soil column at tick 0. The legacy path only reads it. With the tier on it is overwritten at load by `derive_moisture`, so it has no effect.

### Plant water draws

**Introduced:** shot G4b (`draw_water_mm` replaced `draw_moisture`; `cover.water_per_growth_mm` replaced `cover.moisture_draw` = 15 index units). **Runs:** at each patch's cover update, every `schedule.cover_every` ticks (staggered). The tree draw belongs to the tree section. **Code:** `src/producers.rs`, `update_patch_cover`; `src/hydro.rs`, `Sim::draw_water_mm`.

New grass and shrub cover costs water. Each Soil column of the patch gives up
`water_per_growth_mm` for each unit of cover-fraction growth. Growth here is the positive part of
each species' net change before clamping. With the tier on, the draw comes out of `soil[c]`, is
charged to the ledger's ET sink, and re-derives moisture. With the tier off, it is converted to
index units using the soil medium's capacity. Tree transpiration (`tree.transpiration_mm_h`) goes
through the same `draw_water_mm` and is documented in the tree section.

```text
growth = max(0, Δgrass) + max(0, Δshrub)
dm     = cover.water_per_growth_mm × growth                               mm per Soil column
hydro on:   taken = clamp(dm, 0, soil[c]);  soil[c] −= taken;  ledger.et += taken
hydro off:  moisture[c] = max(0, moisture[c] − 255 × dm / medium.soil.field_capacity_mm)
```

**Parameters**
- `cover.water_per_growth_mm` (8.8, mm per unit of cover fraction gained): the water cost of new ground-cover growth. Standing cover's transpiration is `hydro.et_mm_h`.

### Leaching coefficient (pre-G5 fertility path)

**Introduced:** shot G4 (later changes: shot G4b made it per mm drained and set it to 0.0008, up from 0.0002; from shot G5 it is read only when `npk.enabled` is false). **Runs:** every `schedule.soil_every` ticks, after detritus decay, only if `hydro.enabled` and not `npk.enabled`. **Code:** `src/abiotic.rs`, `decay_detritus`.

With the water tier on and the nutrient tier off, the 0-255 fertility index loses a share of its
value in proportion to the water that drained out of the column. With `npk.enabled`, nutrient
leaching uses the mixing rule in the nutrient section and never reads this key. That caveat is in
params.rs but not in the params.toml comment.

```text
fertility[c] −= fertility[c] × clamp(hydro.leach_k × drained[c], 0, 1)
```

**Parameters**
- `hydro.leach_k` (0.0008, fraction per mm drained): the share of fertility lost per mm of drainage. 0 turns leaching off.

### Waterlogging detection

**Introduced:** shot G5 (the value was corrected from 0.95 to 1.10 within the shot). **Runs:** every `schedule.soil_every` ticks in the nutrient update, only if `npk.enabled`. **Code:** `src/npk.rs`, `update_npk`.

Each plantable column keeps a clock of how long it has been continuously wet. It reads
`water_fraction` after the settling step and before nutrient deposition and leaching. The effects
of waterlogging on growth and on tree drowning are in the nutrient and tree sections.

```text
wet[c]        = water_fraction[c] ≥ hydro.waterlog_frac
wet_ticks[c]  = wet[c] ? wet_ticks[c] + soil_every : 0
waterlogged[c] = wet_ticks[c] ≥ hydro.waterlog_ticks
```

The clock advances in steps of `soil_every`, so the threshold is effectively rounded up to a
multiple of it: 40 consecutive soil updates at the defaults. With the tier off,
`water_fraction = moisture / 255 ≤ 1`, so at 1.10 no column ever waterlogs. Notes:
- The params.toml comment calls the value "0.95 of field capacity". The value is 1.10.
- The serde default in params.rs is still 0.95, so a params file without the key gets the value that TUNING.md (shot G5) calls wrong.
- The params.rs doc says "above". The code uses `≥`.

**Parameters**
- `hydro.waterlog_frac` (1.10, fraction of field capacity; the serde default is 0.95): the wet threshold. 1.10 is a store 10% above field capacity, against the cap of `hydro.saturation` = 1.2.
- `hydro.waterlog_ticks` (400, ticks, which is 100 days): how long a column must stay continuously wet to count as waterlogged. The mechanism is switched off by `npk.enabled`, not by setting this to 0.

### Legacy moisture update (water tier off)

**Introduced:** SAD build (later changes: shot 15 added `climate.rain_gradient`; shot G4 kept this path behind `hydro.enabled = false`; its units were left per soil update, as recorded under "Deferred to shot G4c" in UNITS.md). **Runs:** every `schedule.soil_every` ticks, if not `hydro.enabled`. **Code:** `src/abiotic.rs`, `update_soil`, `diffuse`, `evaporate`.

The `moisture` field is a 0-255 index on Soil columns. It is updated in five steps. Its amounts are
per soil update and are not scaled by hours.

```text
1 rain:       r = climate.rain_base − climate.rain_amp × sin(2π t / year_len)
              moisture[c] += rain_at(r, x)                 (just r when rain_gradient == 0)
2 diffusion:  moisture[c] += climate.diffusion × Σ_{4-nbr soil n} (m_n − m_c)   (Jacobi, mass-conserving)
3 evaporation: moisture[c] = max(0, moisture[c] − max(0, climate.evap_base + T_p / climate.evap_div))
4 pond wetting: if any 4-neighbour column is Water: moisture[c] = climate.pond_moisture
5 clamp:      moisture[c] = clamp(moisture[c], 0, 255)
tick 0:       moisture[c] = climate.initial_moisture on Soil columns, 0 elsewhere
```

`rain.annual_mm` and `rain.storm_mean_mm` are not read on this path, and no storm is drawn.

**Parameters**
- `climate.rain_base`, `climate.rain_amp`, see "Storm draw": here they are index units added per soil update.
- `climate.evap_base`, `climate.evap_div`, see "Settling": here they are index units removed per soil update.
- `climate.diffusion` (0.10, fraction per soil update): the Jacobi diffusion rate between 4-neighbour Soil columns.
- `climate.pond_moisture` (255.0, moisture index): the moisture forced onto every Soil column next to a Water column.
- `climate.initial_moisture`, see "Derived moisture".
- `climate.rain_gradient`, see "Rain gradient".

### Water ledger and series columns

**Introduced:** shot G4 (later changes: shot G6 added `pipe_in_mm` and `pipe_out_edge_mm`). **Runs:** the storm columns are reset each tick in `storm`, `drainage_mm` is set by the settling step, and the storage columns are refreshed after both. **Code:** `src/hydro.rs`, `Ledger`, `Water`, `PipeRow`, `Hydro::balance_error`; `src/output.rs`, `SERIES_HEADER`.

Every transfer is tallied in f64 mm·m² (litres). The balance closes to 1e-9 of the rain after
every storm and every settling step. The `series.csv` columns are world means in mm over the
world's `width × depth` m². All eight are 0 with the tier off, and the two pipe columns are 0
without drains.

```text
rain = (pond + soil − start) + evap + et + drain + outflow          (outflow includes pipe_out_edge)
rain_mm          this tick's rain
runoff_mm        rain that left the cell it fell on (own-rain share, drain capture included; ≤ rain_mm)
ponded_mm        Σ pond[i] × cell_area / area
soil_water_mm    Σ soil[c] / area
drainage_mm      percolation at this tick's soil update (0 on other ticks)
outflow_mm       surface water that left over the edge or into open water (excludes pipes)
pipe_in_mm       water captured by all inlets this tick
pipe_out_edge_mm captured water that a pipe emptied over the crop edge this tick
```

**Parameters**
- None of its own. The ledger's `et` sink also takes the plant draws described in "Plant water draws".

## 3. Nutrients, ground cover and fire

Time conversion used throughout this section: `years(n) = n / climate.year_len` for a cadence of `n` ticks, so a rate per year `k` is charged as `k × years(n)` on each update (`hydro::years`). At `climate.year_len` = 4000 a 10-tick update is 0.0025 yr. Soil pools are in g/m², and an ecology column is 1 m², so a column's pool value is also a mass in grams. "Plantable" columns are the columns of class `Soil`; `n_soil(p)` is the number of them in patch `p`.

### Nutrient tier switch and initial pools

**Introduced:** shot G5. **Runs:** once at load (`Npk::new`); the ledger's start stock is recorded once everything standing at tick 0 has been placed (`open_npk_ledger`). **Code:** `src/npk.rs`, `Npk::new`, `Sim::open_npk_ledger`.

With `npk.enabled` the 0-255 fertility index is replaced by three pools per plantable column: mineral N, total P and exchangeable K. Every plantable column starts at `npk.init_*` per unit of `climate.initial_fertility`; non-plantable columns hold nothing. Each patch's detritus is seeded with a nutrient content proportional to its plantable area.

```text
soil_N(c) = init_n × initial_fertility          (plantable c; else 0)
soil_P(c) = init_p × initial_fertility
soil_K(c) = init_k × initial_fertility
detritus_i(p) = init_detritus[i] × n_soil(p)    i ∈ {N, P, K}, grams
```

At the defaults this is 1.54 g/m² N, 51.2 g/m² P, 38.4 g/m² K and a litter layer of 55 / 4 / 35 g/m².

**Parameters**
- `npk.enabled` (true, bool) -- turns the whole nutrient tier on. False runs the legacy fertility index (see "Legacy fertility index"): no pools, no `npk.bin`, no waterlogging, and the run is byte-identical to pre-G5 apart from six always-zero `series.csv` columns.
- `npk.init_n` (0.012, g/m² per unit of initial fertility) -- starting plant-available mineral nitrogen.
- `npk.init_p` (0.4, g/m² per unit) -- starting total phosphorus (bound plus available).
- `npk.init_k` (0.3, g/m² per unit) -- starting exchangeable potassium.
- `npk.init_detritus` ([55.0, 4.0, 35.0], g/m² of plantable ground, N/P/K) -- nutrients in the starting litter layer.
- `climate.initial_fertility` (128.0, index units 0-255) -- the tick-0 fertility index of every soil column; with the nutrient tier on it is the multiplier of the three `npk.init_*` keys, with it off it is the index itself.

### Available pool

**Introduced:** shot G5. **Runs:** whenever a pool is read for growth. **Code:** `src/npk.rs`, `Npk::available`, `Sim::patch_available`, `Sim::npk_take_column`.

All of the N and K pools are available to plants; only a fixed fraction of total P is. Negative pools read as 0. Ground covers read the patch mean over its plantable columns; trees read their own trunk column.

```text
a_N(c) = max(soil_N(c), 0)
a_K(c) = max(soil_K(c), 0)
a_P(c) = max(soil_P(c), 0) × p_avail_frac
a_i(p) = Σ_{c ∈ p} a_i(c) / n_soil(p)
```

**Parameters**
- `npk.p_avail_frac` (0.1, fraction) -- share of total soil phosphorus within reach of roots. The bound remainder is what runoff can carry.

### Nutrient growth factor (Liebig minimum)

**Introduced:** shot G5. **Runs:** on every cover update of a patch (every `schedule.cover_every` ticks per patch), and on every soil update for the fertility field. **Code:** `src/npk.rs`, `liebig`, `Sim::npk_growth_factor`.

Growth is limited by the scarcest of the three elements, each through a saturating term whose half-saturation point scales with the species' need. An element the species does not need (need 0) is skipped.

```text
f_N(p, sp) = min over i with half_sat[i]·need_i > 0 of
             a_i(p) / (a_i(p) + half_sat[i] × need_i(sp))
f_N ∈ [0, 1]; f_N = 1 if the species needs nothing
```

At grass's needs the half points are 0.7 g/m² N, 0.5 g/m² available P, 2.0 g/m² K.

**Parameters**
- `npk.half_sat` ([0.28, 2.5, 1.0], multiple of need, N/P/K) -- `half_sat × need` is the available pool (g/m²) at which that element alone halves growth.
- `grass.npk.need_n` (2.5, g/m² per unit cover density) -- nitrogen held by a unit of grass cover; also the uptake per unit of growth and the return per unit of loss.
- `grass.npk.need_p` (0.2, g/m² per unit cover) -- phosphorus, as above.
- `grass.npk.need_k` (2.0, g/m² per unit cover) -- potassium, as above.
- `shrub.npk.need_n` (3.3, g/m² per unit cover) -- shrub nitrogen content.
- `shrub.npk.need_p` (0.27, g/m² per unit cover) -- shrub phosphorus content.
- `shrub.npk.need_k` (2.7, g/m² per unit cover) -- shrub potassium content.

### Waterlogging

**Introduced:** shot G5. **Runs:** clock advanced every `schedule.soil_every` ticks inside `update_npk`; the factor is read on every cover update. **Code:** `src/npk.rs`, `Sim::update_npk` (step 2), `Sim::waterlog_factor`.

A column whose soil water stays at or above a threshold fraction of field capacity for long enough is waterlogged. A patch's growth multiplier falls from 1 towards the species' tolerance in proportion to the share of its columns that are waterlogged. Only exists with `npk.enabled`.

```text
wet(c)        = water_fraction(c) ≥ hydro.waterlog_frac      (water_fraction = soil water / field capacity, unclamped)
wet_ticks(c)  = wet(c) ? wet_ticks(c) + soil_every : 0
logged(c)     = wet_ticks(c) ≥ hydro.waterlog_ticks
f_W(p, sp)    = 1 − (n_logged(p) / n_soil(p)) × (1 − clamp(waterlog_tolerance(sp), 0, 1))
```

With `hydro.enabled` false, `water_fraction` is `moisture/255 ≤ 1`, so at `hydro.waterlog_frac` = 1.10 nothing ever waterlogs.

**Parameters**
- `grass.npk.waterlog_tolerance` (0.8, fraction) -- share of its growth grass keeps on fully waterlogged ground.
- `shrub.npk.waterlog_tolerance` (0.7, fraction) -- the same for shrub.
- `hydro.waterlog_frac` (1.10, fraction of field capacity) -- wetness threshold; documented with the water tier.
- `hydro.waterlog_ticks` (400, ticks) -- how long a column must stay wet; documented with the water tier.

Note: the `params.toml` comment on `hydro.waterlog_frac` still describes 0.95 of field capacity; the value and the code use 1.10, above field capacity (DECISIONS.md, shot G5).

### Ground-cover growth and mortality

**Introduced:** SAD build (later changes: shot G4b made `r` and `g` per year and the moisture curve a fraction of available water capacity; shot G4c made the light curve a fraction of full sun; shot G5 added the nutrient and waterlog factors and split gain from loss). **Runs:** each patch once every `schedule.cover_every` ticks, staggered. **Code:** `src/producers.rs`, `Sim::update_patch_cover`, `Sim::growth_factor`, `suitability`.

Grass and shrub are densities in [0, 1] per patch. Each update adds a gain towards 1, scaled by four environmental factors, and removes a constant fraction as mortality. Grass gain is suppressed by shrub cover; shrub is not suppressed by grass.

```text
dt   = years(schedule.cover_every)
env  = patch means over plantable columns: light L (fraction of full sun), water W (soil water / AWC), T (patch °C)
curve([min, lo, hi, max], v) = 0 if v ≤ min or v ≥ max; (v−min)/(lo−min) if v < lo; 1 if v ≤ hi; (max−v)/(max−hi)
f_nut = f_N(p, sp) × f_W(p, sp)                        (npk.enabled)
      = clamp(fertility_mean / cover.fertility_full, 0, 1)   (npk off)
R(sp) = r(sp) × curve(light, L) × curve(moisture, W) × curve(temp, T) × f_nut

S_supp = max(1 − grass_suppression × S, 0)
gain_G = R(grass) × dt × (1 − G) × S_supp
gain_S = R(shrub) × dt × (1 − S)
(npk on) gain_G, gain_S ×= share         (see "Nutrient uptake")
G' = clamp(G + gain_G − g(grass) × dt × G, 0, 1)
S' = clamp(S + gain_S − g(shrub) × dt × S, 0, 1)
```

Note: the task brief and older docs call this logistic growth; the code's gain term is `r·(1 − G)` with no factor of `G`, so it is a saturating (monomolecular) approach to 1 plus linear mortality, and a density of 0 still grows. Tied curve breakpoints resolve to 0 (`v ≤ min` wins).

**Parameters**
- `grass.r` (20.0, per year) -- maximum grass gain rate.
- `grass.g` (2.0, per year) -- grass mortality rate.
- `grass.initial` (0.10, density) -- grass density on every patch with soil at tick 0 (`sim.rs`); 0 on patches without soil.
- `grass.light` ([0.3922, 0.7843, 1.0, 1.0039], fraction of full sun) -- light curve [min, low-opt, high-opt, max].
- `grass.moisture` ([0.0784, 0.3137, 1.0, 1.004], fraction of available water capacity) -- moisture curve.
- `grass.temp` ([0.0, 5.0, 30.0, 35.0], °C) -- temperature curve on the patch temperature.
- `shrub.r` (4.0, per year) -- maximum shrub gain rate.
- `shrub.g` (1.6, per year) -- shrub mortality rate.
- `shrub.initial` (0.02, density) -- shrub density on every patch with soil at tick 0.
- `shrub.light` ([0.1569, 0.3922, 0.7843, 0.9961], fraction of full sun) -- the shade-tolerant understory curve.
- `shrub.moisture` ([0.0588, 0.2353, 1.0, 1.004], fraction of AWC) -- shrub moisture curve.
- `shrub.temp` ([-5.0, 0.0, 28.0, 33.0], °C) -- shrub temperature curve.
- `cover.grass_suppression` (0.5, per unit shrub density) -- how strongly shrub cover suppresses grass gain.
- `cover.fertility_full` (64.0, index units) -- legacy only: fertility index at which the nutrient factor saturates at 1.

### Cover update stagger

**Introduced:** SAD build (later changes: shot G4b made the cadence the parameter `schedule.cover_every`, previously a fixed 10). **Runs:** every tick, on the patches whose turn it is. **Code:** `src/producers.rs`, `Sim::update_producers`.

Patch `p` is updated on ticks where its index matches the tick modulo the cadence, so each patch is updated exactly once per cadence and about `patches / cover_every` patches update per tick.

```text
update patch p on tick t  ⇔  p mod cover_every = t mod cover_every
```

Note: "about 6 patches per tick" (CLAUDE.md) holds for the old 64-patch square world; on the 256×64 reference strip (256 patches) it is 25-26 per tick.

**Parameters**
- `schedule.cover_every` (10, ticks) -- producer cadence; also the `dt` of each patch's growth.

### Nutrient uptake by ground cover and litter return

**Introduced:** shot G5 (later changes: shot G10 moved the debug-only "grew more than paid for" check to the f64 growth). **Runs:** with each patch's cover update. **Code:** `src/producers.rs`, `Sim::update_patch_cover`, `Sim::cover_demand`; `src/npk.rs`, `Sim::npk_share`, `Sim::npk_spend`, `Sim::npk_to_detritus`.

Cover growth must be paid for from the patch's available pools. Both species share one cut, set by the scarcest element; the soil is then charged for the growth kept, and whatever was paid for but is not standing any more (mortality, and anything the clamp at 1 trimmed) goes to the patch's detritus nutrient pool. A cover's nutrient content is never stored: it is `need × density × n_soil`.

```text
demand_i = n_soil × (gain_G × need_i(grass) + gain_S × need_i(shrub))
A_i      = Σ_{c ∈ p} a_i(c)
share    = min(1, min over i with A_i < demand_i of A_i / demand_i)
got_i    = spend(demand_i after the cut)   (even split over columns, then sweep the shortfall)
litter_i = max(got_i − n_soil × (ΔG × need_i(grass) + ΔS × need_i(shrub)), 0)  → detritus_i(p)
```

**Parameters**
- `grass.npk.need_n`, `grass.npk.need_p`, `grass.npk.need_k`, `shrub.npk.need_n`, `shrub.npk.need_p`, `shrub.npk.need_k` -- see "Nutrient growth factor".
- `npk.p_avail_frac` -- see "Available pool".

### Litter mass and growth costs

**Introduced:** SAD build (later changes: shot G4b turned the water draw into millimetres, `cover.water_per_growth_mm`; shot G5 made the fertility draw legacy-only). **Runs:** with each patch's cover update. **Code:** `src/producers.rs`, `Sim::update_patch_cover`.

Dying cover adds detritus mass to the patch. Net positive growth draws water from every plantable column of the patch, and, with the nutrient tier off, draws down the fertility index.

```text
detritus(p) += litter_factor × (g(grass)·dt·G + g(shrub)·dt·S) × n_soil      (pre-update G, S)
growth       = max(ΔG, 0) + max(ΔS, 0)
per column c: soil water −= water_per_growth_mm × growth                     (draw_water_mm)
per column c: fertility  = max(fertility − fertility_draw × growth, 0)       (npk off only)
```

**Parameters**
- `cover.litter_factor` (20.0, detritus units per unit density per column) -- detritus mass produced by cover mortality.
- `cover.water_per_growth_mm` (8.8, mm per unit of new cover) -- soil water one unit of cover growth costs, per column.
- `cover.fertility_draw` (20.0, index units per unit of new cover) -- legacy only: fertility index removed per unit of growth.

### Shrub spread

**Introduced:** SAD build (later changes: shot G5 made seeded shrub pay for its nutrients). **Runs:** at the end of each patch's cover update. **Code:** `src/producers.rs`, `Sim::update_patch_cover`.

A patch whose updated shrub density exceeds a threshold seeds each existing 4-neighbour patch that has soil, raising its shrub to at least a seed density. With the nutrient tier on the neighbour's soil pays `need × seeded × n_soil(q)`, and the seed is cut by that neighbour's share.

```text
if S'(p) > shrub_spread_threshold:
  for q in 4-neighbours(p) with n_soil(q) > 0:
    seeded = max(S(q), shrub_spread_seed) − S(q)
    S(q)  += seeded × share_q             (share_q = 1 with npk off)
```

**Parameters**
- `cover.shrub_spread_threshold` (0.6, density) -- shrub density above which a patch seeds its neighbours.
- `cover.shrub_spread_seed` (0.02, density) -- floor a neighbour's shrub is raised to.

### Detritus decay

**Introduced:** SAD build (later changes: shot G4b made `climate.decay_k` per year; shot G5 changed its value from 6.0 to 0.45 and routed decayed nutrients into the three pools). **Runs:** every `schedule.soil_every` ticks, in the soil update after the water settles. **Code:** `src/abiotic.rs`, `Sim::decay_detritus`.

Each patch's detritus loses a fraction set by temperature and moisture. The same fraction of the patch's detritus nutrient pools moves, split evenly, into the soil pools of its plantable columns; with the tier off the decayed mass goes straight into the fertility index.

```text
dt       = years(schedule.soil_every)
rate     = decay_k × dt × clamp(T / decay_temp_full, 0, 1) × M / 255      (M = patch mean moisture index 0-255)
detritus(p) −= detritus(p) × rate
(npk on)  released_i = detritus_i(p) × rate;  soil_i(c) += released_i / n_soil  for c ∈ p
(npk off) fertility(c) += detritus(p) × rate / n_soil                     for c ∈ p
```

The moisture term still reads the 0-255 moisture index (with the water tier on, that index is derived from soil water). Rate is not clamped to 1; at the defaults it is at most ~0.0011 per update.

**Parameters**
- `climate.decay_k` (0.45, per year) -- detritus turnover rate at full temperature and moisture.
- `climate.decay_temp_full` (30.0, °C) -- patch temperature at and above which decay runs at full rate; linear from 0 °C, 0 below.

### Nitrogen deposition

**Introduced:** shot G5. **Runs:** every `schedule.soil_every` ticks (`update_npk` step 1). **Code:** `src/npk.rs`, `Sim::update_npk`.

Nitrogen from the air (and standing in for fixation) is added evenly to every plantable column. It is the only input any pool has.

```text
soil_N(c) += n_deposition × years(schedule.soil_every)      (plantable c)
ledger.deposition_N += same, summed
```

**Parameters**
- `npk.n_deposition` (2.5, g/m² per year) -- nitrogen input per plantable column.

### Leaching of N and K

**Introduced:** shot G5. **Runs:** every `schedule.soil_every` ticks (`update_npk` step 3), with the water drained by `settle_water`. **Code:** `src/npk.rs`, `Sim::update_npk`.

Nitrogen leaves a column with the water that drained out of its bottom, at the concentration of a fully mixed store; potassium leaves at a fixed fraction of that rate; phosphorus does not leach. With the water tier off nothing drains and nothing leaches.

```text
d    = drained(c)  (mm, this update);  W = soil water left in the column after settling (mm)
frac = clamp(d / (W + d), 0, 1)
ΔN   = soil_N(c) × frac
ΔK   = soil_K(c) × clamp(frac × k_leach_ratio, 0, 1)
ledger.leached_{N,K} += ΔN, ΔK
```

Note: the task brief and the `params.toml` comment on `hydro.leach_k` both say leaching uses `hydro.leach_k`. With `npk.enabled` the code does not read `hydro.leach_k` at all; the leached share is `d / (W + d)`. `hydro.leach_k` is read only by the legacy fertility index (below).

**Parameters**
- `npk.k_leach_ratio` (0.03, multiple of the nitrogen rate) -- potassium's leaching rate relative to nitrogen's.

### Runoff of P and N

**Introduced:** shot G5 (later changes: shot G6 let storm drains take the load in the share they take of the water). **Runs:** in every storm, as the water is routed along the flow graph (`hydro.enabled` only). **Code:** `src/hydro.rs`, `Sim::route_storm`.

Water leaving a ground cell over the surface lifts phosphorus (with soil particles) and dissolved nitrogen from the top of the profile, and carries the load downhill. A cell keeps the share of an arriving load equal to the share of the arriving water it infiltrated or ponded; a drain takes the share it took of the water; what leaves over the edge or into open water is outflow. Potassium does not run off.

```text
w      = water leaving the cell over the surface after drains, infiltration and ponding (mm)
area   = ground-cell area (m²);  per_col = ground cells per ecology column;  W = column soil water (mm)
lift_P = min(p_runoff_g_per_mm × w × area, soil_P(c) / per_col)
mix    = clamp(n_runoff_frac × w / (w + W), 0, 1)
lift_N = max(soil_N(c), 0) / per_col × mix
kept   = (infiltrated + ponded) / (rain + runon) on the cell
arriving load × kept → soil of the cell's column (plantable only); rest moves on
load leaving the world → ledger.outflow_{P,N}
```

**Parameters**
- `npk.p_runoff_g_per_mm` (0.002, g per m² per mm of runoff) -- phosphorus lifted by surface runoff, capped at the cell's share of the pool.
- `npk.n_runoff_frac` (0.1, fraction) -- share of a column's nitrogen in the surface layer that runoff mixes with.

### Fertility field from the pools

**Introduced:** shot G5 (later changes: shot G13 published the three pools' own overlays with fixed log10 scales in `meta.json`). **Runs:** every `schedule.soil_every` ticks at the end of `update_npk`, and once when the ledger opens. **Code:** `src/npk.rs`, `Sim::refresh_fertility`.

With the tier on, `fertility.bin` holds the growth limitation grass would get in each column rather than a stock of its own.

```text
fertility(c) = 255 × liebig(a(c), grass needs, npk.half_sat)   (plantable c; else 0)
```

**Parameters**
- `npk.half_sat`, `grass.npk.need_n`, `grass.npk.need_p`, `grass.npk.need_k` -- see "Nutrient growth factor".

### Nutrient ledger

**Introduced:** shot G5. **Runs:** stock recorded once at tick 0; fluxes accumulated where they happen; balance checked at every snapshot. **Code:** `src/npk.rs`, `NpkLedger`, `Sim::npk_balance_error`; `src/output.rs`, `NPK_BALANCE_EPS`.

Every gram that entered the world is either in an inventory or in a loss term. Cover content is derived from densities, trees and grazers carry stored `npk`, and hunters hold none (a kill's nutrients go to detritus). The run fails if the relative error reaches 1e-6.

```text
in_i        = start_i + deposition_i
lost_i      = leached_i + outflow_i + volatilised_i
inventory_i = Σ soil_i + Σ detritus_i + Σ_p n_soil(p)(G·need_i(grass) + S·need_i(shrub)) + Σ tree.npk_i + Σ animal.npk_i
error_i     = (in_i − lost_i − inventory_i) / max(|in_i|, 1)      |error_i| < 1e-6
```

**Parameters**
- None of its own; it reads every flux above.

### Legacy fertility index (nutrient tier off)

**Introduced:** SAD build (later changes: shot G4 added leaching with the drained water; shot G5 kept it as the `npk.enabled = false` path). **Runs:** set at load; decayed into every `schedule.soil_every` ticks; drawn on each cover update; raised by ash at fire burn-out. **Code:** `src/sim.rs` (initialisation), `src/abiotic.rs`, `Sim::decay_detritus`; `src/producers.rs`, `Sim::update_patch_cover`; `src/fire.rs`, `Sim::burn_out`.

A 0-255 index per soil column that stands for all nutrients at once. It starts at `climate.initial_fertility`, gains decayed detritus and fire ash, loses a draw per unit of cover growth and a share of itself per millimetre drained, and is clamped to [0, 255] after each soil update. Growth reads it through `cover.fertility_full`.

```text
F(c, 0)   = initial_fertility
decay:    F += detritus(p) × rate / n_soil                  (rate as in "Detritus decay")
leaching: F −= F × clamp(hydro.leach_k × drained(c), 0, 1)   (water tier on only)
clamp:    F = clamp(F, 0, 255)
growth:   F = max(F − fertility_draw × growth, 0)
ash:      F = clamp(F + fire.ash, 0, 255)
f_nut     = clamp(mean F / fertility_full, 0, 1)
```

**Parameters**
- `climate.initial_fertility`, `cover.fertility_draw`, `cover.litter_factor` (litter feeds the detritus that decays into F), `cover.fertility_full` -- see above.
- `hydro.leach_k` (0.0008, fraction per mm drained) -- legacy index only: share of the index lost per millimetre drained out of a column; documented with the water tier.
- `fire.ash` -- see "Fire burn-out".

### Fire fuel

**Introduced:** shot 09. **Runs:** whenever an ignition or spread chance is computed. **Code:** `src/fire.rs`, `Sim::fuel`.

A patch's fuel is a weighted sum of its cover, detritus and canopy share; a patch with no soil column has none and can neither ignite nor be spread to.

```text
fuel(p) = 0.5 × G + S + detritus_weight × detritus(p) + canopy_weight × canopy_cols(p) / patch²
```

Note: the grass weight 0.5 is hard-coded, not a parameter.

**Parameters**
- `fire.detritus_weight` (0.0002, per detritus unit) -- fuel contributed per unit of patch detritus.
- `fire.canopy_weight` (1.0, per canopy fraction) -- fuel contributed by a fully canopied patch.

### Fire ignition

**Introduced:** shot 09 (later changes: shot G4b made `fire.base_rate` per year and the cadence the parameter `schedule.fire_every`, and dryness a fraction of available water capacity). **Runs:** every `schedule.fire_every` ticks when `fire.base_rate` > 0. **Code:** `src/fire.rs`, `Sim::update_fire`, `ignition_prob`, `temp_factor`.

One draw per patch in patch order; a patch not already burning ignites when the draw falls below the chance. A draw is consumed even for a patch already burning. Ignited patches start counting down on the next tick.

```text
f(T)    = 0 if T ≤ temp_min; 1 if T ≥ temp_full; (T − temp_min)/(temp_full − temp_min) between
dry     = clamp(1 − W, 0, 1)            (W = patch mean soil water / AWC)
P_ign   = clamp(base_rate × years(fire_every) × f(T) × dry² × fuel, 0, 1)
```

**Parameters**
- `fire.base_rate` (0.8, ignitions per patch per year) -- ignition rate at full temperature, dryness and unit fuel; 0 switches fire off with no RNG draws.
- `fire.temp_min` (15.0, °C) -- patch temperature at or below which nothing ignites.
- `fire.temp_full` (30.0, °C) -- patch temperature at or above which the temperature factor is 1.
- `schedule.fire_every` (10, ticks) -- cadence of ignition draws.

### Fire spread and duration

**Introduced:** shot 09. **Runs:** every tick. **Code:** `src/fire.rs`, `Sim::update_fire`, `spread_prob`, `Sim::ignite`.

Each patch burning at the start of the fire phase rolls once against each non-burning 4-neighbour (+x, −x, +y, −y), with no roll when the chance is 0. Then every patch that was burning counts down, and burns out at 0.

```text
P_spread(q) = clamp(spread × fuel(q) × dry(q), 0, 1)     per tick, per burning neighbour
burning_ticks_left = max(duration, 1) on ignition; −1 per tick; burn-out at 0
```

`fire.spread` is per tick and `fire.duration` is in ticks; neither was converted to hours in shot G4b.

**Parameters**
- `fire.spread` (0.1, per tick) -- spread chance per burning neighbour at unit fuel and full dryness.
- `fire.duration` (3, ticks) -- how long a patch burns before it burns out (0 is read as 1).

### Fire burn-out

**Introduced:** shot 09 (later changes: shot G5 replaced the ash term with nutrient ash and nitrogen volatilisation when `npk.enabled`). **Runs:** on the tick a patch's burn count reaches 0. **Code:** `src/fire.rs`, `Sim::burn_out`.

The patch's grass and shrub are converted to detritus mass and set to 0. With the tier on, the nutrients the cover held go to the soil as ash except the volatilised share of its nitrogen, which is a ledger loss; with it off, each soil column's index gains a fixed ash amount. Each live tree whose trunk is in the patch dies with a fixed chance (one draw per tree, Vec order, none when the chance is 0).

```text
detritus(p) += detritus_yield × (G + S) × n_soil;  G = S = 0
(npk on)  burnt_i = n_soil × (G·need_i(grass) + S·need_i(shrub))
          volatilised_N += burnt_N × clamp(fire_n_volatilised, 0, 1)
          ash = burnt − volatilised;  soil_i(c) += ash_i / n_soil
(npk off) fertility(c) = clamp(fertility(c) + ash, 0, 255)
tree in p dies with probability tree_kill
```

Note: with the tier on the added detritus mass carries no nutrients (they went to the soil as ash), and `fire.ash` is unused.

**Parameters**
- `fire.detritus_yield` (10.0, detritus units per unit density per column) -- detritus mass left by burnt cover.
- `fire.ash` (5.0, index units) -- legacy only: fertility index added to each soil column at burn-out.
- `fire.tree_kill` (0.5, probability) -- chance each tree in a burnt-out patch dies.
- `npk.fire_n_volatilised` (0.9, fraction) -- share of burnt cover nitrogen lost as smoke; the rest, and all its P and K, lands as ash.

### Fire damage to animals

**Introduced:** shot 09. **Runs:** every tick, in the animal phase (reads the burning state left by the previous tick). **Code:** `src/animals.rs`, `Sim::scorch`.

An animal in a burning patch loses a fixed energy and flees one step away from the patch centre; a death there is recorded as `burnt`.

```text
energy −= animal_damage      (animal's patch burning)
```

**Parameters**
- `fire.animal_damage` (2.0, energy per tick) -- energy lost per tick spent in a burning patch.

## 4. Trees

The tree tier is a `Vec<Tree>` of trunk entities, one per 1 m column, each with an age in ticks, a drought clock, a lifespan drawn at planting, an `alive` flag (dead trees stay in the Vec until the next compaction, every `world.compact_every` ticks, which also rebuilds the `trunk_at` column index) and, since shot G5, the grams of N, P and K it has taken up. A tree's size is a function of its age alone: nothing in the environment speeds or slows it. Environmental factors act on germination (seed or immigrant establishment), on the water it draws, and on whether it dies. All tree ages and clocks are given in years or days in `params.toml` and converted to ticks in one place, `Params::tree_ages` (shot G4c). At the defaults (`climate.year_len` = 4000, `hydro::tick_hours` = 8766 / 4000 = 2.1915 h) the conversions give the tick counts in brackets below.

Note: `hydro::HOURS_PER_YEAR` is 8766 (a Julian year), not 8760, so one tick is 2.1915 h, not 2.19 h.

### Tier conversions (ages and cadences in ticks)
**Introduced:** shot G4c (later changes: shot G4e added `ticks_between` for immigration). **Runs:** computed on demand from params (every call to `tree_ages`). **Code:** `src/params.rs`, `Params::tree_ages`, `Params::ticks_in_years`, `Params::ticks_between`, `Params::tree_immigration_every`.

Every tree age or clock is converted to ticks here and nowhere else, so the tree tier means the same simulated time whatever `climate.year_len` is. A rate per year becomes a cadence in ticks through `ticks_between`; a rate of 0 gives `u32::MAX`, which disables the event.

```text
ticks_in_years(y)   = clamp(round(y * year_len), 0, u32::MAX)
ticks_between(r)    = r > 0 ? clamp(round(year_len / r), 1, u32::MAX) : u32::MAX
tick_hours          = 8766 * (1 / year_len)                       (hours per tick)

initial   = ticks_in_years(tree.initial_age_years)                 (500)
young     = ticks_in_years(tree.young_age_years)                   (500)
mature    = ticks_in_years(tree.mature_age_years)                  (1000)
max       = ticks_in_years(tree.max_age_years)                     (6000)
tall_imp  = ticks_in_years(bundle.tree_tall_age_years)             (3000)
dry_death = clamp(round(tree.dry_death_days * 24 / tick_hours), 0, u32::MAX)   (500)
seed_every     = ticks_between(tree.seeds_per_year)                (200)
immig_every    = ticks_between(tree.immigrants_per_year)           (500)
```

**Parameters**
- `tree.initial_age_years` (0.125, years) -- age of the trees placed at tick 0 in a noise world. Equal to `tree.young_age_years` at the defaults, so initial trees start as young trees with one canopy voxel.
- `tree.young_age_years` (0.125, years) -- age at which a sapling becomes young.
- `tree.mature_age_years` (0.25, years) -- age at which a tree becomes mature, casts its full canopy and starts seeding; also the age of a `bundle.tree_mature_height` tree on the height curve.
- `tree.max_age_years` (1.5, years) -- mean lifespan before jitter. params.toml records that these ages are 25-60x too fast against real trees, kept because real ages would need a 200000-tick run (DECISIONS.md, "Units calibration, part two").
- `tree.dry_death_days` (45.66, days) -- consecutive drought that kills a tree (see Drought clock).
- `tree.seeds_per_year` (20.0, 1/year) -- seeding attempts a mature tree makes per year (see Seeding).
- `tree.immigrants_per_year` (8.0, 1/year) -- tree immigration checks per year (see Immigration).
- `climate.year_len`, `bundle.tree_tall_age_years` -- documented in their own sections.

### Growth stages and height
**Introduced:** SAD build stages; shot S3 added the age-to-height curve (`height_of_age`); shot G4c moved ages to years. **Runs:** stage and height are derived from age whenever read; age advances every `tree.update_every` ticks. **Code:** `src/trees.rs`, `stage_of`, `Sim::tree_stage`; `src/plants.rs`, `height_of_age`, `import_age`; `src/sim.rs`, `step_profiled`.

Each live tree's age increases by `tree.update_every` at every tree update (ticks where `tick % tree.update_every == 0`), unconditionally: there is no growth rate and no limiting factor on size. Stage and height are pure functions of age. The height curve is the bundle-import curve (`import_age`) run backwards.

```text
age <- age + update_every                       (every update_every ticks)

stage(age) = Sapling  if age <  young
             Young    if young <= age < mature
             Mature   if age >= mature

tall = max(tall_imp, mature)
height(age) = 0                                              if age <= 0
            = hm * age / mature                              if age < mature
            = hm + (ht - hm) * (age - mature) / (tall - mature)   if age < tall (and tall > mature, ht > hm)
            = max(ht, hm)                                    otherwise
  hm = max(bundle.tree_mature_height, 0) (3 m), ht = bundle.tree_tall_height (20 m)
```

At the defaults a tree is 3 m at 1000 ticks and 20 m at 3000 ticks and stays 20 m until it dies. Trees planted during an update pass (seedlings) are not updated in the same pass: the loop runs over the length of `Sim::trees` taken at its start.

**Parameters**
- `tree.update_every` (50, ticks) -- tree update cadence; a cadence in ticks, not a rate, and the length of time each update charges (age, transpiration, drought clock).
- `tree.young_age_years`, `tree.mature_age_years` -- see Tier conversions.
- `bundle.tree_mature_height`, `bundle.tree_tall_height`, `bundle.tree_tall_age_years` -- documented in the bundle section.

### Initial placement
**Introduced:** SAD build (later changes: shot G3 made a bundle world plant its scene trees instead; shot G12 barred trunks under roofs). **Runs:** once at load. **Code:** `src/trees.rs`, `Sim::place_initial_trees`; `src/plants.rs`, `Sim::import_scene`.

In a noise world, up to `tree.initial_count` trees are planted at age `initial` on uniformly random soil columns that can root a trunk (plantable and not roofed) and that pass the spacing test; at most 10000 column draws are made, so fewer trees may be placed. In a bundle world `tree.initial_count` and `tree.initial_age_years` are not used: every scene tree is planted at `import_age(height)` on its own column or the nearest trunk-capable column within `bundle.tree_move_radius`, the tallest winning a shared column, with no `tree.min_spacing` test.

```text
repeat while placed < initial_count and attempts < 10000:
    (x, y) = uniform random soil column               (one RNG draw)
    if can_root_a_trunk(x, y) and spacing_ok(x, y): plant_tree(x, y, initial)
```

**Parameters**
- `tree.initial_count` (12, trees) -- trees attempted at tick 0 in a noise world.
- `tree.initial_age_years` -- see Tier conversions.
- `tree.min_spacing` -- see Trunk spacing.

### Planting and lifespan jitter
**Introduced:** SAD build planting; lifespan jitter in shot 5 (the "dynamics fixes" commit). **Runs:** on event (initial placement, bundle import, germination, immigration). **Code:** `src/trees.rs`, `Sim::plant_tree`.

Every planted tree draws its own lifespan once, from one uniform RNG draw, and then refreshes the light of the 3x3 columns around its trunk.

```text
u        ~ Uniform[-1, 1]
lifespan = max(0, round(max * (1 + lifespan_jitter * u)))     (ticks; 4800..7200 at defaults)
```

**Parameters**
- `tree.lifespan_jitter` (0.2, fraction) -- relative half-width of the per-tree lifespan spread around `tree.max_age_years`.
- `tree.max_age_years` -- see Tier conversions.

### Trunk spacing
**Introduced:** SAD build (later changes: shot 11 applied it to tree immigrants). **Runs:** on event (every planting attempt except bundle import). **Code:** `src/trees.rs`, `Sim::spacing_ok`.

A trunk may be planted only if no live trunk lies within Chebyshev distance `min_spacing - 1` of the column.

```text
spacing_ok(x, y) = no trunk at (x+dx, y+dy) for all |dx|, |dy| <= min_spacing - 1
```

**Parameters**
- `tree.min_spacing` (2, columns) -- minimum Chebyshev distance between trunks; at 2 no two trunks are adjacent, including diagonally. It gates planting only, not competition.

### Canopy voxels and light recompute
**Introduced:** SAD build (later changes: shot G4c made light a Beer-Lambert fraction of full sun via `canopy_extinction`; shot G9 multiplied it by the building sun factor and added whole-world recomputes on season-slice changes). **Runs:** on event -- a tree planted, killed, or changing stage refreshes the 3x3 columns around its trunk; `update_sun` recomputes every column when the season slice changes (every tick checks; 4 times a year at `sun.season_samples` = 4 in a bundle world, never in a noise world). **Code:** `src/trees.rs`, `Sim::canopy_z`, `Sim::refresh_canopy_columns`, `Sim::update_sun`; `src/world.rs`, `World::set_column_light`.

A sapling has no canopy. A young tree has one canopy voxel at h + 2 over its own column, and a mature tree has two, at h + 2 and h + 3, over each of the 3x3 columns around its trunk. h is the trunk column's surface height, used for all nine columns. The light each voxel then receives is under [Canopy light (Beer-Lambert)](#canopy-light-beer-lambert).

**Parameters**
- `world.canopy_k`, `world.canopy_lai` -- see Canopy light.

### Transpiration and leaf-off
**Introduced:** SAD build as a moisture-index draw `tree.moisture_draw` (later changes: shot G4b replaced it with `tree.transpiration_mm_h` in mm/h, 300 mm a year; shot G10 added deciduous leaf-off). **Runs:** every `tree.update_every` ticks, per live tree. **Code:** `src/trees.rs`, `Sim::update_trees`, `Sim::leaf_on`, `Sim::leaf_draw_mean`, `leaf_fraction`, `leaf_draw`; `src/hydro.rs`, `Sim::draw_water_mm`.

Each update a tree draws water from its trunk column only (not from the 3x3 its canopy covers). A deciduous tree's draw is scaled by its leaf-on share, read on its patch's temperature, and divided by that factor's mean over one model year, so leaf-off moves a year's water into the leafy season without changing the annual total. With `tree.deciduous` = 0 the patch temperature is not read and the draw is the plain rate.

```text
hours = update_every * tick_hours                                   (109.6 h)
mm    = transpiration_mm_h * hours                                  (3.75 mm per update)

if deciduous > 0:
    leaf(T)   = on <= off ? (T >= on ? 1 : 0) : clamp((T - off) / (on - off), 0, 1)
    draw(l)   = 1 - clamp(deciduous, 0, 1) * (1 - clamp(l, 0, 1))
    n         = ceil(year_len / temperature_every)
    norm      = (1/n) * sum_{k=0}^{n-1} draw(leaf(temp_base + amplitude * sin(2*pi*k*temperature_every / year_len)))
    mm        = norm > 0 ? mm * draw(leaf(T_patch)) / norm : 0

with the water tier on:   taken = min(soil[c], mm); soil[c] -= taken; ledger.et += taken
with the water tier off:  moisture[c] = max(0, moisture[c] - 255 * mm / medium.soil.field_capacity_mm)
```

`norm` samples the open-ground seasonal temperature, while `leaf` reads the patch's actual temperature, so a tree whose canopy cools its patch spends a little longer bare and draws slightly less than the annual figure (noted in the code comment).

**Parameters**
- `tree.transpiration_mm_h` (0.0342, mm/h) -- mean-year transpiration of one tree over its trunk column; 300 mm/year, the low end of the published range because it is charged to one column while a mature crown covers nine.
- `tree.deciduous` (0.5, fraction) -- share of the draw a leafless tree gives up; 0 is an evergreen and reproduces the pre-G10 model byte for byte; above 0.5 the regression anchor fails (TUNING.md, shot G10).
- `tree.leaf_off_temp` (5.0, deg C) -- patch temperature at and below which a tree is bare.
- `tree.leaf_on_temp` (10.0, deg C) -- patch temperature at and above which a tree is in full leaf; linear between the two, a step at this temperature if it is not above `tree.leaf_off_temp`.
- `climate.temp_base`, `season.amplitude`, `schedule.temperature_every`, `medium.soil.field_capacity_mm` -- documented in their own sections.

### Nutrient uptake
**Introduced:** shot G5. **Runs:** every `tree.update_every` ticks, per live tree, only when `npk.enabled`. **Code:** `src/trees.rs`, `Sim::update_trees`; `src/npk.rs`, `Sim::npk_take_column`.

A tree buys each metre of height growth with fixed grams of N, P and K from the soil under its trunk column. It takes what is there and is never cut back when the soil is short (its height is its age), so a starved tree just holds less; what it holds returns to its patch's detritus pool when it dies. Nutrients do not limit tree growth or survival.

```text
grew     = max(0, height(age_new) - height(age_old))          (m; 0.15 m/update below 3 m, 0.425 m/update to 20 m, then 0)
want_e   = grew * need_e                                       for e in {N, P, K}
reach_e  = max(0, soil_e[c])                  (N, K)
reach_P  = max(0, soil_P[c]) * npk.p_avail_frac
got_e    = min(want_e, reach_e);  soil_e[c] -= got_e;  tree.npk_e += got_e
```

**Parameters**
- `tree.npk.need_n` (3.0, g per m of height growth) -- nitrogen the tree takes per metre of growth.
- `tree.npk.need_p` (0.25, g/m) -- phosphorus per metre of growth.
- `tree.npk.need_k` (2.4, g/m) -- potassium per metre of growth.
- `npk.p_avail_frac`, `npk.enabled` -- documented in the nutrient section.

Note: if `[tree.npk]` is missing from a params file the struct default applies, which is the ground-cover default (2.5, 0.2, 2.0, tolerance 0.8), not the values above.

### Drought clock
**Introduced:** SAD build with `tree.dry_moisture` on the 0-255 index and `dry_death_ticks` (later changes: shot G4b made it a fraction of available water capacity, `tree.dry_fraction`; shot G4c made the kill threshold days). **Runs:** every `tree.update_every` ticks, per live tree, after the transpiration draw. **Code:** `src/trees.rs`, `Sim::update_trees`; `src/hydro.rs`, `Sim::water_fraction`.

A tree whose trunk column's water fraction is below `tree.dry_fraction` after its own draw accumulates dry ticks; any update at or above it resets the clock. A tree whose clock reaches `dry_death` ticks dies with cause `drought`.

```text
w = soil[c] / capacity[c]      (water tier on; 0 if capacity is 0)
  = moisture[c] / 255          (water tier off)
dry_ticks = (w < dry_fraction) ? dry_ticks + update_every : 0
dies (drought) if dry_ticks >= dry_death                   (500 ticks = 10 updates)
```

**Parameters**
- `tree.dry_fraction` (0.12, fraction of available water capacity) -- soil water below which a tree counts as dry; 0 is the wilting point.
- `tree.dry_death_days` -- see Tier conversions.

### Old-age death
**Introduced:** SAD build (later changes: shot 5 per-tree jitter). **Runs:** every `tree.update_every` ticks, per live tree, after the drought clock is updated. **Code:** `src/trees.rs`, `Sim::update_trees`.

```text
dies (old_age) if age >= lifespan
```

Checked before drought, so a tree that qualifies for both is logged `old_age`.

**Parameters**
- `tree.max_age_years`, `tree.lifespan_jitter` -- see Tier conversions and Planting.

### Waterlog mortality
**Introduced:** shot G5. **Runs:** every `tree.update_every` ticks, per live tree, after old age and drought. **Code:** `src/trees.rs`, `Sim::update_trees`; `src/npk.rs`, `Sim::is_waterlogged`.

A tree standing on a waterlogged column (a state the nutrient tier keeps; never true with `npk.enabled` off) dies with a fixed chance per update, reduced by its tolerance. The rate and waterlog checks come before the RNG draw, so no draw is made when either is false.

```text
if waterlog_mortality > 0 and waterlogged[c]:
    dies (waterlog) if U[0,1) < waterlog_mortality * (1 - clamp(waterlog_tolerance, 0, 1))
                                                   (0.006 per update, about 0.48 a year at defaults)
```

**Parameters**
- `tree.waterlog_mortality` (0.01, probability per update) -- chance a tree on a waterlogged column drowns, before tolerance; 0 disables the roll.
- `tree.npk.waterlog_tolerance` (0.4, fraction) -- the tree's tolerance of standing water; scales the drowning chance down.

Note: the `SpeciesNpk::waterlog_tolerance` doc comment says growth on a waterlogged column is multiplied by it; that is how the ground covers use it. For the tree it only scales mortality, since tree growth is not modelled as a rate.

### Crown geometry
**Introduced:** shot S3 (later changes: shot S11 made crowding mortality read it). **Runs:** built once at the start of each tree update pass (when `tree.crowding_mortality` > 0) and at snapshot time for published crown light. **Code:** `src/trees.rs`, `Crown`, `Crowns`, `Sim::crown_of`, `Sim::crowns`, `overlap_fraction`.

A tree's crown is a disc centred on its trunk column with radius and base proportional to its height (from its age). It is separate from, and much larger than, the 3x3 canopy voxel stamp: a 20 m tree has a 6 m radius. `Crowns` holds one `Crown` per `Sim::trees` entry and the largest live crown radius, which bounds the neighbour search. Overlap is the exact disc-disc intersection area over the first disc's area.

```text
x, y     = trunk column + 0.5
ground   = surface height h + 1
radius   = max(0, crown_radius_frac) * height(age)
base     = clamp(crown_base_frac, 0, 1) * height(age)
max_radius = max radius over live trees

overlap(a, b) = |disc_a ∩ disc_b| / (pi * ra^2)
   = 0                          if d >= ra + rb
   = 1                          if d <= |ra - rb| and rb >= ra
   = rb^2 / ra^2                if d <= |ra - rb| and rb < ra
   = lens(ra, rb, d) / (pi ra^2) otherwise, where
     lens = ra^2 acos((d^2+ra^2-rb^2)/(2 d ra)) + rb^2 acos((d^2+rb^2-ra^2)/(2 d rb))
            - 0.5 sqrt(((ra+rb)^2 - d^2) (d^2 - (ra-rb)^2))
   (ra = 0: 1 if rb > 0 and d <= rb, else 0)
```

**Parameters**
- `tree.crown_radius_frac` (0.30, fraction of height) -- crown radius over tree height; median of the 81 surveyed Capitol trees (measured, not tuned).
- `tree.crown_base_frac` (0.37, fraction of height) -- lowest-branch height over tree height; same survey. With the radius it fixes the crown envelope used by crown light.

### Crowding mortality (canopy self-thinning)
**Introduced:** shot 5 ("dynamics fixes", as a count of mature trunks nearby) (later changes: shot S11 replaced the trunk count with crown overlap and added `tree.crowding_overlap`). **Runs:** every `tree.update_every` ticks, per live mature tree, after waterlog. **Code:** `src/trees.rs`, `Sim::crown_crowding`, `Sim::update_trees`.

A mature tree whose crown is at least `crowding_overlap` covered by other live crowns dies with chance `crowding_mortality`. Coverage combines neighbours as independent: open share is the product of what each leaves. Crowns are the stand as it was when the pass began (ages before this update's increment); trees planted during the pass are skipped, and trees killed earlier in the pass are skipped immediately via `trunk_at`, so the pass thins in index order.

```text
crowded_i = clamp(1 - prod_{j != i, live, trunk within reach} (1 - overlap(crown_i, crown_j)), 0, 1)
reach     = ceil(radius_i + max_radius)       (square search box around the trunk)
dies (crowded) if stage == Mature and crowding_mortality > 0
               and crowded_i >= crowding_overlap and U[0,1) < crowding_mortality
```

At `tree.crowding_mortality` = 0 no crowns are built and no RNG draw is made. At `tree.crowding_overlap` = 0 even an uncrowded mature tree is at risk (0 >= 0).

**Parameters**
- `tree.crowding_mortality` (0.02, probability per update) -- death chance of a mature tree whose crown is over the overlap threshold; 0 is the off switch.
- `tree.crowding_overlap` (0.55, fraction of crown area) -- crown coverage by others at which the roll is made; just above the 95th percentile (0.471) of the surveyed Capitol trees' coverage; a tuning choice (TUNING.md, shot S11).

### Seeding and germination
**Introduced:** SAD build (later changes: shot 14b logs germination events; shot G4c made the rate `seeds_per_year` and light a fraction of full sun, and fixed the seeding test for cadences that do not divide; shot G12 required a trunk-capable, unroofed target). **Runs:** every `tree.update_every` ticks, per live mature tree whose age crosses a multiple of `seed_every` in this update; the new tree is planted immediately. **Code:** `src/trees.rs`, `Sim::try_seed`, `Sim::germination_prob`; `src/params.rs`, `Params::tree_light_curve`; `src/producers.rs`, `suitability`.

A mature tree that survives its update throws one seed to a uniform random point in a disc of radius `seed_radius`, rounded to a column. If the column can root a trunk and passes spacing, the seed germinates with the product of three suitability curves: surface light (fraction of full sun, including building shade), the column's water fraction, and the patch temperature. A germinated seed is planted at age 0 and logged as a `germination` event.

```text
seeding due  if stage == Mature and age mod seed_every < update_every
r   = seed_radius * sqrt(U1),  theta = 2*pi*U2
(tx, ty) = (round(x + r cos theta), round(y + r sin theta))
if trunk_site_ok(tx, ty) and spacing_ok(tx, ty):
    p = S(light_curve, L_surface) * S(tree.moisture, w) * S(tree.temp, T_patch)
    plant at age 0 if p > 0 and U3 < p

light_curve = tree.light with light_curve[1] = max(sapling_light, light[0])
S([min, lo, hi, max], v) = 0 if v <= min or v >= max;  (v-min)/(lo-min) if v < lo;
                           1 if v <= hi;  (max-v)/(max-hi) otherwise
L_surface = light of the voxel above the surface / 255
```

Note: the `TreeAges::seed_every` comment says a rate of 0 leaves seeding out because "no tree age is a multiple of it", but the test is `age % seed_every < update_every`; with `seed_every` = `u32::MAX` that is `age < update_every`, which a mature tree fails unless `tree.mature_age_years` is under one update, so the claim holds at any sane setting.

**Parameters**
- `tree.seeds_per_year` -- see Tier conversions (one attempt every 200 ticks of age at defaults).
- `tree.seed_radius` (6.0, columns/m) -- maximum seed distance.
- `tree.sapling_light` (0.5882, fraction of full sun) -- germination light need; replaces `tree.light`'s low-opt (index 1), so the value in `tree.light[1]` is not read.
- `tree.light` ([0.2353, 0.5882, 1.0, 1.0039], fraction of full sun) -- germination suitability curve over surface light [min, low-opt, high-opt, max].
- `tree.moisture` ([0.1176, 0.3922, 1.0, 1.004], fraction of available water capacity) -- germination suitability curve over the target column's water fraction.
- `tree.temp` ([-3.0, 2.0, 26.0, 31.0], deg C) -- germination suitability curve over patch temperature.
- `tree.min_spacing` -- see Trunk spacing.

### Immigration
**Introduced:** shot 11 (tree floor and interval) (later changes: shot G4e replaced `tree.immigration_interval` with `tree.immigrants_per_year`; shot G12 rejects roofed edge columns). **Runs:** every tick in the immigration phase (after animals, before producers), acting on ticks that are a multiple of `immig_every` (500 at defaults). **Code:** `src/animals.rs`, `Sim::immigrate`; `src/params.rs`, `Params::tree_immigration_every`.

On the world clock, not a tree's age: if fewer than `immigration_floor` trees are alive, one random edge soil column is drawn; if it can root a trunk and passes spacing, a sapling (age 0) is planted and logged as an `immigration` event, otherwise nothing arrives this check. A floor of 0 (the default) never plants or draws.

```text
if tick mod immig_every == 0 and live_trees < immigration_floor:
    (x, y) = uniform random edge soil column (none -> nothing)
    if can_root_a_trunk(x, y) and spacing_ok(x, y): plant_tree(x, y, 0)
```

**Parameters**
- `tree.immigration_floor` (0, trees) -- population below which one sapling immigrates per check; 0 disables tree immigration.
- `tree.immigrants_per_year` -- see Tier conversions.

### Death and detritus
**Introduced:** SAD build (later changes: shot 14b logs `tree_death` events with a cause; shot G5 returns the tree's stored N, P, K). **Runs:** on event (any tree death: `old_age`, `drought`, `waterlog`, `crowded`, and `burnt` from the fire phase). **Code:** `src/trees.rs`, `Sim::kill_tree`.

A dead tree is flagged, its trunk cleared from `trunk_at`, its patch's detritus raised by a fixed amount plus the nutrients it held, and the light of the 3x3 columns around it recomputed, all at once, so the gap opens within the same pass.

```text
patch.detritus += death_detritus
npk.detritus[patch] += tree.npk          (if npk.enabled)
refresh_canopy_columns(x, y)
event: tree_death, species "tree", cause in {old_age, drought, crowded, burnt, waterlog}
```

**Parameters**
- `tree.death_detritus` (40.0, detritus units) -- detritus a dead tree adds to its patch.

### Update order within a tree update
**Introduced:** SAD build, accreted by later shots. **Runs:** every `tree.update_every` ticks, after producers and `update_sun`, before fire. **Code:** `src/trees.rs`, `Sim::update_trees`.

For each tree alive at the start of the pass, in index order:

```text
1. age += update_every
2. nutrient uptake                         (npk.enabled)
3. transpiration draw                      (with leaf-off if deciduous > 0)
4. drought clock
5. old age -> kill, next tree
6. drought -> kill, next tree
7. waterlog roll -> kill, next tree
8. crowding roll (mature) -> kill, next tree
9. if stage changed: refresh 3x3 canopy light
10. seeding attempt (mature, due)
```

## 5. Animals (not the focus)

Predator-prey work is parked by operator direction of 2026-09-19 (DECISIONS.md, shot G1; UNITS.md section 6). The rules below stay in the code and in `params.toml`, unchanged and untuned. Every garden-series bundle run sets `animals.enabled=false` (`--set animals.enabled=false`); the one deliberate exception is the shot S1/S8 animals-on Capitol fixture, kept so readers of animal data stay tested. Unlike the rest of the model, animal parameters were never converted to physical units: energy is a dimensionless 0-100 index, and every animal rate, cost and duration below is **per tick** (one animal update), not per hour, so none of them is scaled by `hydro::tick_hours`. 

### Animal tier switch
**Introduced:** shot G0. **Runs:** once at load (placement) and every tick (gate). **Code:** `src/sim.rs`, `place_initial_animals`, `step_profiled`; `src/animals.rs`, `immigrate`.
When off, no animal is placed, none immigrates, and `update_animals` is skipped; each check sits outside its loop, so an animals-off run draws nothing from the RNG an animals-on run would. It is bit-identical to an animal tier configured empty.
```text
if !animals.enabled: grazers = hunters = [], skip animal phase, skip animal immigration
```
**Parameters**
- `animals.enabled` (true, bool) -- places and updates grazers and hunters. Garden bundle runs set it false.

### Initial placement
**Introduced:** SAD build. **Runs:** once at load. **Code:** `src/sim.rs`, `place_initial_animals`.
Grazers then hunters are dropped on uniformly random soil columns with default traits and empty N/P/K. Age and remaining cooldown are drawn so the starting population is not in lockstep.
```text
for each of start_count animals:
  (x, y) ~ uniform soil column
  energy   = start_energy
  age      ~ U{0 .. max(start_age_max,1) - 1}
  cooldown ~ U{0 .. C}      C = grazer.cooldown (grazer) | hunter.refractory (hunter)
```
**Parameters**
- `grazer.start_count` (300, animals) -- grazers placed at load.
- `grazer.start_energy` (60.0, energy index) -- energy of an initial grazer; also of a grazer immigrant.
- `grazer.start_age_max` (1000, ticks) -- exclusive upper bound of an initial grazer's age.
- `hunter.start_count` (20, animals) -- hunters placed at load.
- `hunter.start_energy` (60.0, energy index) -- energy of an initial hunter; also of a hunter immigrant.
- `hunter.start_age_max` (1000, ticks) -- exclusive upper bound of an initial hunter's age.

### Update order and movement energy cost (stability rule 4)
**Introduced:** SAD build (shot 11 made the cost a heritable multiplier). **Runs:** every tick, grazers in Vec order then hunters; newborns act from the next tick. **Code:** `src/animals.rs`, `update_animals`, `update_grazer`, `update_hunter`.
Each update: age +1, cooldown -1 (saturating), fire damage if the patch burns (`fire.animal_damage`, documented under Fire), choose one action, pay the tick's cost, then death, crowding and birth checks. Moving costs double.
```text
cost = energy_cost · energy_cost_mult · (2 if the animal stepped this tick else 1)
energy ← energy - cost          (no floor; energy ≤ 0 means death this update)
```
The ×2 move factor and the 100 energy ceiling are hard-coded, not params.
**Parameters**
- `grazer.energy_cost` (0.10, energy/tick) -- a grazer's base metabolic cost.
- `hunter.energy_cost` (0.04, energy/tick) -- a hunter's base metabolic cost.

### Grazer behaviour priority and fleeing
**Introduced:** SAD build (shot 09 added fleeing fire; shot 11 made the flee radius a trait). **Runs:** every tick. **Code:** `src/animals.rs`, `update_grazer`, `nearest_hunter`, `greedy_step`, `flee_fire`.
A grazer does the first of: flee fire, flee the nearest hunter, eat, move to a better patch, wander (random soil neighbour). Fleeing is one greedy 8-neighbour step that strictly increases Euclidean distance from the hunter (or from the burning patch's centre); if no neighbour improves, it stays.
```text
if patch burning:                              Flee (step away from patch centre)
elif ∃ hunter with dist ≤ flee_distance:       Flee (step away from nearest hunter; lowest index on ties)
elif grass_p > eat_min_grass and energy < eat_below:   Eat
elif target_patch ≠ p:                         Move (one BFS-shortest step toward target)
else:                                          Wander
```
**Parameters**
- `grazer.flee_radius` (4.0, columns) -- default of the heritable `flee_distance`; the flee search list is built out to 4 × this (the trait clamp).
- `grazer.eat_below` (90.0, energy index) -- a grazer eats only while its energy is below this.
- `grazer.eat_min_grass` (0.0, grass density) -- a grazer eats only if patch grass exceeds this.

### Type II grazing intake (stability rule 1)
**Introduced:** SAD build (shot G5 added the N/P/K transfer). **Runs:** every tick, on an Eat action. **Code:** `src/animals.rs`, `grazing_intake`, `update_grazer`.
Energy gain saturates with patch grass density; the grass removed is proportional to the energy gained.
```text
intake  = clamp(intake_k · grass_p, 0, intake_max)
energy  ← min(energy + intake, 100)
grass_p ← max(grass_p - intake · grass_per_energy, 0)
```
Note: grass is removed at `intake · grass_per_energy` even when the energy cap at 100 absorbs part of the intake.
**Parameters**
- `grazer.intake_max` (3.0, energy/tick) -- intake ceiling.
- `grazer.intake_k` (20.0, energy/tick per unit grass density) -- initial slope of the response.
- `grazer.grass_per_energy` (0.00003, grass density per energy) -- patch grass density removed per unit of energy eaten.

### Grazer patch choice and crowding
**Introduced:** SAD build (BFS pathing, tolerance and hash preference were SAD-build tuning calls, DECISIONS.md "Grazer movement"). **Runs:** every tick, when the grazer neither flees nor eats. **Code:** `src/animals.rs`, `target_patch`, `step_toward_patch`, `preference`.
Candidate patches lie within Chebyshev patch-distance `search_patches` and must be walkable from the grazer's column. A grazer stays if its own patch is acceptable, else goes to the acceptable patch ranked first by a fixed hash of (id, patch), which spreads herds without RNG draws.
```text
score(q)   = grass_q - n_grazers(q) / crowding
acceptable = { q : score(q) ≥ max_q score(q) - choice_tolerance }
target     = p if p ∈ acceptable else argmin_{q ∈ acceptable} hash(id, q)
```
**Parameters**
- `grazer.search_patches` (2, patches) -- Chebyshev radius of the candidate set.
- `grazer.crowding` (8.0, grazers per unit grass density) -- grazer count that cancels one unit of grass in the score.
- `grazer.choice_tolerance` (0.75, score units) -- slack below the best score still counted acceptable; 0 gives the addendum's "best patch".

### Hunter behaviour and satiation (stability rule 2)
**Introduced:** SAD build (shot 09 added fleeing fire; shot 14a-rev added handling). **Runs:** every tick. **Code:** `src/animals.rs`, `update_hunter`, `nearest_prey`.
A hunter does the first of: handle a kill (no move, no attack), flee fire, rest when satiated (random step, no attack), attack prey in reach, approach the nearest prey, wander. Every live grazer within the seek radius is a legal target whatever its shrub.
```text
if handling > 0:             Handling; handling ← handling - 1
elif patch burning:          Flee
elif energy > satiation:     Rest (random step)
elif nearest grazer j within seek_radius:
    dist_j ≤ attack_radius → Hunt (attack j)
    else                   → Move (greedy step toward j, else random step)
else:                        Wander
```
**Parameters**
- `hunter.satiation` (85.0, energy index) -- above this a hunter makes no attack.
- `hunter.seek_radius` (16.0, columns) -- Euclidean radius of the prey search.
- `hunter.attack_radius` (2.0, columns) -- prey within this distance is attacked instead of approached.
- `hunter.flee_radius` (4.0, columns) -- default of a hunter's `flee_distance` trait (shot 11). Nothing a hunter does reads it; it is a neutral trait that only drifts.

### Attack, kill probability and shrub refugium (stability rules 2 and 3)
**Introduced:** SAD build (dynamics fixes replaced the refugium threshold with the continuous form; shot 14a added `hunt_cost`; shot 14a-rev added `handling_ticks`). **Runs:** on a Hunt action. **Code:** `src/animals.rs`, `attack_success`, `attack`.
One Bernoulli draw decides the attack, with success reduced by the shrub cover of the grazer's patch. A miss costs extra and throws the grazer up to `displace_steps` columns straight away from the hunter (stopping at non-soil).
```text
P_kill = clamp(kill_prob · (1 - clamp(shrub_p, 0, 1))^refugium_k, 0, 1)      (pow(0,0) = 1)
hit : grazer dies (Eaten); energy_h ← min(energy_h + kill_energy - hunt_cost, 100)
      handling ← handling_ticks
miss: energy_h ← energy_h - hunt_cost - fail_cost
      grazer moves ≤ displace_steps along sign(g - h) (random direction if co-located)
```
**Parameters**
- `hunter.kill_prob` (0.3, probability) -- kill chance on bare ground.
- `hunter.refugium_k` (2.0, dimensionless) -- exponent of the shrub refugium; 0 switches it off.
- `hunter.kill_energy` (40.0, energy index) -- energy gained from a kill.
- `hunter.hunt_cost` (0.0, energy) -- charged on every attempt, hit or miss.
- `hunter.fail_cost` (0.25, energy) -- extra charge on a miss.
- `hunter.displace_steps` (3, columns) -- how far a missed grazer is pushed.
- `hunter.handling_ticks` (0, ticks) -- updates spent in Handling after a kill; 0 is the pre-shot-14a-rev rule.

### Death: starvation, burning, old age
**Introduced:** SAD build (shot 05 cause attribution; shot 09 `Burnt`). **Runs:** every tick, after the cost is paid. **Code:** `src/animals.rs`, `death_cause`, `kill_grazer`, `kill_hunter`.
```text
if energy ≤ 0 or age ≥ max_age: die, cause = Burnt (energy ≤ 0 in a burning patch)
                                           | Starved (energy ≤ 0)  | OldAge
```
**Parameters**
- `grazer.max_age` (5000, ticks) -- grazer lifespan.
- `hunter.max_age` (8000, ticks) -- hunter lifespan.

### Corpse detritus (stability rule 5)
**Introduced:** SAD build (shot G5 added the body's N/P/K). **Runs:** on every animal death, any cause. **Code:** `src/animals.rs`, `kill_grazer`, `kill_hunter`.
```text
detritus_p ← detritus_p + corpse_detritus          (p = patch where the animal died)
npk_detritus_p ← npk_detritus_p + npk_body          (only when npk.enabled)
```
**Parameters**
- `grazer.corpse_detritus` (15.0, dimensionless detritus proxy) -- detritus a dead grazer leaves.
- `hunter.corpse_detritus` (25.0, dimensionless detritus proxy) -- detritus a dead hunter leaves.

### Crowding mortality ("disease")
**Introduced:** shot 10. **Runs:** every tick, per animal, after the death check and before birth. **Code:** `src/animals.rs`, `crowding_death_p`, `crowded_out`.
Density-dependent death by own-species count n in the patch the animal ends the update in (itself included). Nothing is transmitted; the name is historical. A rate of 0 skips the check with no RNG draw; no draw either at or below the threshold.
```text
P_crowd = clamp(rate · max(0, n - threshold) / max(threshold, 1), 0, 1)     cause = Crowded
```
**Parameters**
- `disease.grazer_rate` (0.001, probability per tick) -- grazer crowding rate.
- `disease.grazer_threshold` (16, grazers per patch) -- grazer count above which crowding deaths start.
- `disease.hunter_rate` (0.001, probability per tick) -- hunter crowding rate.
- `disease.hunter_threshold` (4, hunters per patch) -- hunter count above which crowding deaths start.

### Reproduction, cooldown and refractory
**Introduced:** SAD build (shot 10 renamed `hunter.cooldown` to `hunter.refractory`; shot 11 made the threshold a trait). **Runs:** every tick, last in the update, if the animal survived. **Code:** `src/animals.rs`, `update_grazer`, `update_hunter`.
Asexual: one newborn on the parent's column. Grazer births are also capped by the patch's grazer count; hunter births are not.
```text
grazer: if energy > repro_threshold and cooldown = 0 and n_grazers(p) < max_grazers_per_patch:
          energy ← energy - grazer.repro_cost; cooldown ← grazer.cooldown
          newborn: energy grazer.newborn_energy, age 0, cooldown grazer.cooldown
hunter: if energy > repro_threshold and cooldown = 0:
          energy ← energy - hunter.repro_cost; cooldown ← hunter.refractory
          newborn: energy hunter.newborn_energy, age 0, cooldown hunter.refractory
```
**Parameters**
- `grazer.repro_energy` (70.0, energy index) -- default of the heritable `repro_threshold`.
- `grazer.repro_cost` (35.0, energy) -- parent's energy spent per birth.
- `grazer.cooldown` (300, ticks) -- wait after a birth, and a newborn's wait before its first.
- `grazer.newborn_energy` (30.0, energy index) -- newborn grazer energy.
- `grazer.max_grazers_per_patch` (5, grazers) -- no grazer birth in a patch holding this many or more.
- `hunter.repro_energy` (75.0, energy index) -- default of the heritable `repro_threshold`.
- `hunter.repro_cost` (40.0, energy) -- parent's energy spent per birth.
- `hunter.refractory` (2750, ticks) -- hunter equivalent of `grazer.cooldown`.
- `hunter.newborn_energy` (40.0, energy index) -- newborn hunter energy.

### Heritable traits and mutation
**Introduced:** shot 11. **Runs:** at each birth. **Code:** `src/heredity.rs`, `inherit`, `Sim::offspring_traits`; `src/params.rs`, `default_traits`.
Each animal carries `energy_cost_mult` (default 1), `flee_distance` (default `flee_radius`) and `repro_threshold` (default `repro_energy`), used in place of those params. A newborn takes one uniform draw per trait; placed animals and immigrants carry the defaults. At mutation 0 the newborn copies its parent with no draw.
```text
u ~ U[-1, 1]
trait_child = clamp(trait_parent · (1 + mutation · u), 0.25 · default, 4 · default)
```
The [0.25, 4] clamp is a hard-coded constant (`TRAIT_CLAMP`), not a param.
**Parameters**
- `heredity.mutation` (0.05, relative step) -- maximum relative change per trait per generation.

### Immigration floor
**Introduced:** dynamics fixes (83de5bf) for hunters and grazers (shot 11 removed the any-soil fallback and gave immigrants default traits). **Runs:** every tick as its own phase after animals, firing on ticks that are a multiple of the interval. **Code:** `src/animals.rs`, `immigrate`, `random_edge_soil_column`.
When a species' live count is below its floor, one immigrant arrives on a uniformly random edge soil column (none, and no draw, if the edge has no soil). A floor of 0 never draws; nothing immigrates when `animals.enabled` is false.
```text
if t mod immigration_interval = 0 and count < immigration_floor:
   add 1 animal: energy start_energy, age 0, cooldown 0, default traits, npk 0
```
**Parameters**
- `grazer.immigration_floor` (0, grazers) -- grazer count below which one immigrates; 0 is off.
- `grazer.immigration_interval` (500, ticks) -- grazer immigration cadence.
- `hunter.immigration_floor` (0, hunters) -- hunter count below which one immigrates; 0 is off.
- `hunter.immigration_interval` (500, ticks) -- hunter immigration cadence.

### Animal N/P/K and dung
**Introduced:** shot G5. **Runs:** on each Eat action (only when `npk.enabled`), and at death. **Code:** `src/animals.rs`, `graze_npk`; `src/npk.rs`, `excrete`, `npk_to_detritus`.
A grazer takes in the nutrients of the grass it crops and holds at most a fixed content per unit of energy; the excess goes straight to the patch's N/P/K detritus pool as dung the same tick. Hunters gain energy, not matter, from kills and always hold zero; the prey's body N/P/K goes to the patch where it fell.
```text
eaten_density = grass_before - grass_after
npk_body[i]  += eaten_density · n_soil_cols(p) · grass.npk.need_i        (g)
cap[i]        = max(npk_content[i] · energy, 0)
dung[i]       = max(npk_body[i] - cap[i], 0);  npk_body[i] -= dung[i];  npk_detritus_p[i] += dung[i]
```
Note: the cap is applied only at eating; a grazer whose energy then falls keeps its body N/P/K above the cap until its next meal or death.
**Parameters**
- `animals.npk_content` ([0.02, 0.002, 0.015], g N/P/K per unit energy) -- body nutrient capacity per energy; a residence time, not a stock. Reads `grass.npk.need_n`, `grass.npk.need_p`, `grass.npk.need_k` (documented under ground cover).

## Discrepancies found while writing this

Shot 26 found these by reading the code against its comments, `params.toml` and the older documents. It fixed none of them, because it is a documentation shot and several of the fixes would change what a run does. Each item is also noted in the subsection it affects. They are listed here so that a later row can pick them up.

**Comments or documents that say something other than the code**
1. `hydro.waterlog_frac`: the `params.toml` comment still describes 0.95 of field capacity, but the value is 1.10. The serde default in `src/params.rs` is still 0.95, the value TUNING.md (shot G5) calls wrong, so a params file without the key gets it. The `params.rs` doc says "above" the threshold, and the code tests `>=`.
2. `hydro.leach_k`: the `params.toml` comment reads as though it drives leaching. With `npk.enabled` it is never read. Nitrogen leaves at `d / (W + d)` of the pool and potassium at `npk.k_leach_ratio` of that. Only the legacy fertility index reads `hydro.leach_k`.
3. `SpeciesNpk::waterlog_tolerance` says growth on a waterlogged column is multiplied by it. That is true for grass and shrub. For the tree it only scales the drowning chance.
4. The `TreeAges::seed_every` comment says a rate of 0 stops seeding because "no tree age is a multiple of it". The actual test is `age % seed_every < update_every`, which at `u32::MAX` becomes `age < update_every`. The claim holds at any sane setting, but not for the stated reason.
5. "Sealed" ground: the comments on `World::from_bundle` and `Medium::is_sealed`, and DECISIONS.md (shot G1), say roof, asphalt or concrete. The code counts any non-water medium whose `medium.<m>.plantable` is false. The two agree at the shipped values.
6. DECISIONS.md (shot G4) says roofs are lifted by their building height and drained by a BFS from each pipe inlet. The code lifts every roof by the highest ground plus 1 m and sends each roof component to its lowest-index adjacent non-roof cell. No pipe is involved.
7. The doc comment on `Sim::step` leaves out the storm phase, which runs after fire.
8. `src/params.rs` still describes `climate.rain_base` as "mean rain per soil update". With the water tier on it only sets the shape of the seasonal rain factor.
9. CLAUDE.md says about 6 patches update per tick. That is true on the old 64-patch square world. The 256-patch reference strip updates 25 or 26 per tick.
10. Ground-cover growth is described in older documents as logistic. The gain is `r · f · dt · (1 − G)` with no factor of `G`, so a patch at density 0 still grows.

**Keys that are read nowhere, or not the way their name suggests**
11. `medium.water.plantable` is never read, because the Rock rule counts water cells separately.
12. `tree.light[1]` is never read, because `tree_light_curve` replaces it with `max(tree.sapling_light, tree.light[0])`.
13. `hunter.flee_radius` only sets the default of a trait that no hunter code reads. It is neutral on purpose (shot 11).
14. `climate.initial_moisture` has no effect with the water tier on. `derive_moisture` overwrites it at load.
15. `fire.ash` is not read with the nutrient tier on.
16. A params file without `[tree.npk]` gets the ground-cover defaults (2.5 / 0.2 / 2.0, tolerance 0.8), not the tree's values.

**Values that are hard-coded although CLAUDE.md says nothing tunable is**
17. The fire fuel weight of grass (0.5), the ×2 energy cost of an animal's move tick, the animal energy ceiling of 100, the trait clamp `[0.25, 4] × default` (`TRAIT_CLAMP`), the 10000 draws allowed for placing the initial trees, and the sun budget's three sky bands and 16 azimuths.

**Behaviour worth knowing**
18. Scene planting does not apply `tree.min_spacing`. Only one trunk per column is enforced, so imported trunks can stand on neighbouring columns.
19. The sun budget counts a column's roof share from cells with building height > 0, while the trunk bar (shot G12) counts cells whose medium is `roof`. The two agree only for a consistent export.
20. Leaf-off does not change the light field, so a bare crown still shades.
21. `fire.spread` is per tick and `fire.duration` is in ticks. Shot G4b did not convert them.
22. Rain peaks when temperature is lowest, so the warm half of the year is the dry half.
23. With `world.compact_every = 0`, compaction never runs, because `is_multiple_of(0)` is false for every tick after 0.
24. With `world.slope_bias` ≠ 0, the share of flooded columns is not exactly `world.water_fraction`, because the tilt is added after the quantile step.
25. A grazer's grass loss ignores the energy cap: it crops `intake × grass_per_energy` even when part of the intake is lost at 100. Its body N/P/K cap is applied only when it eats.
