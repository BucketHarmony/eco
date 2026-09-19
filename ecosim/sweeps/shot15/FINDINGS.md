# Shot 15 findings: the 256×64×32 strip with a rain gradient

The reference world is now 256×64×32 with 8×8-column patches, `climate.rain_gradient` 0.6 and `world.slope_bias` 4. That means a raised, dry west and a low, wet east.

- **Extinctions.** None, on any reference seed or sweep cell.
- **Contiguity.** All 18 sweep cells pass every invariant, so the passing cells form one contiguous region: the whole grid.
- **Signature.** Seed 1 passes the fixed signature at 60000 ticks. Seeds 2, 3 and 42 don't.

## Death causes on the reference seeds (events.csv, strip, defaults, 20000 ticks)

| seed | species | deaths by cause |
|---|---|---|
| 1 | grazer | 53204 total: crowded 40661 (76.4%), eaten 11570 (21.7%), starved 570 (1.1%), old_age 401 (0.8%), burnt 2 (0.0%) |
| 1 | hunter | 434 total: crowded 281 (64.7%), old_age 110 (25.3%), starved 43 (9.9%) |
| 2 | grazer | 57592 total: crowded 44449 (77.2%), eaten 11677 (20.3%), starved 800 (1.4%), old_age 633 (1.1%), burnt 33 (0.1%) |
| 2 | hunter | 399 total: crowded 288 (72.2%), old_age 109 (27.3%), starved 2 (0.5%) |
| 3 | grazer | 47469 total: crowded 35256 (74.3%), eaten 9161 (19.3%), old_age 1652 (3.5%), starved 1339 (2.8%), burnt 61 (0.1%) |
| 3 | hunter | 353 total: crowded 273 (77.3%), old_age 68 (19.3%), starved 12 (3.4%) |
| 42 | grazer | 45865 total: crowded 35333 (77.0%), eaten 7867 (17.2%), old_age 1844 (4.0%), starved 791 (1.7%), burnt 30 (0.1%) |
| 42 | hunter | 301 total: crowded 234 (77.7%), old_age 67 (22.3%) |

- **Grazers.** Crowding kills about three quarters of them and hunters about a fifth, the same ordering as on the 64 world.
- **Hunters.** They die mostly of crowding, then old age. Starvation is under 10% on every seed.
- **Anchor.** Every reference seed passes `ecosim check` at 20000 ticks (TUNING.md, shot 15), and no default was changed to get there.

## Sweep: `climate.rain_gradient` 0:1.0:0.2, seeds 1–3, 20000 ticks

- **Wall time.** 166.6 s for 18 cells on 18 threads (`sweep.md`).
- **Result.** 18 of 18 cells pass every invariant, with no extinction and nothing to attribute. The safe band is the whole grid, [0.0, 1.0].
- **Contiguity.** The passing cells form a single contiguous region. There are no failing cells to form a region of their own.

### West and east quarter cover (ticks 10000–20000)

- **Method.** Each cell was rerun standalone with a snapshot every 1000 ticks, and the quarter means are over the 11 snapshots from tick 10000 on (`spatial.py`, output in `spatial.md`).
  - Quarters are the westmost and eastmost 64 columns.
  - Grass and shrub are patch means over every patch in the quarter.
  - Tree cover is the fraction of the quarter's columns under canopy, as the sim counts it.
- **Wall time.** These 18 runs and the four 60000-tick runs below took 340 s together on 22 processes.
- **Values.** The table gives means over seeds 1–3. Per-seed values are in `spatial.md`.

| rain_gradient | west grass | east grass | west shrub | east shrub | west tree cover | east tree cover |
|---|---|---|---|---|---|---|
| 0.0 | 0.582 | 0.396 | 0.383 | 0.463 | 0.249 | 0.349 |
| 0.2 | 0.719 | 0.325 | 0.062 | 0.467 | 0.025 | 0.413 |
| 0.4 | 0.672 | 0.325 | 0.000 | 0.467 | 0.000 | 0.415 |
| 0.6 (default) | 0.557 | 0.322 | 0.000 | 0.465 | 0.000 | 0.411 |
| 0.8 | 0.420 | 0.321 | 0.000 | 0.468 | 0.000 | 0.414 |
| 1.0 | 0.242 | 0.328 | 0.000 | 0.467 | 0.000 | 0.408 |

- **Gradient 0.** Even without a gradient the west differs from the east. The slope bias raises the west, and the high ground carries more grass and less shrub and tree cover than the low east.
- **Woody cover leaves the west quickly.** At 0.2 (west edge 0.8 × rain) shrub and trees are nearly gone from the west quarter. From 0.4 on there are none at any seed, and grass takes the ground they leave.
- **The east barely moves.** More rain than 1.2 × the base changes little there: shrub about 0.47 and tree cover about 0.41 at every gradient from 0.2 up. Its grass stays near 0.32, under the canopy and shrub.
- **West grass peaks and falls.** It is highest at 0.2 and drops from 0.72 to 0.24 by 1.0, where the west edge gets no rain at all. So the default 0.6 gives a west of open grassland (0.56 grass, no woody cover) and a wooded east. That is the strip's intended contrast, with every species persisting.

## Fixed signature at 60000 ticks (strip, defaults)

This is a report, not a pass condition. It is `ecosim stats --signature` over ticks 5000–60000 with a 12000-tick detrend and seasonal removal.

| seed | pp_lag | pp_corr | pp_period | pp_pass | `check --long` | wall (22 processes) |
|---|---|---|---|---|---|---|
| 1 | 3500 | 0.4723 | 10650 | true | pass | 258 s |
| 2 | 3250 | 0.4305 | 2750 | false | pass | 309 s |
| 3 | 8000 | 0.1983 | 5500 | false | pass | 334 s |
| 42 | 1200 | 0.2870 | 5500 | false | pass | 339 s |

- **Seed 1** passes: hunters trail grazers by 3500 ticks, correlation 0.47, on a 10650-tick hunter cycle.
- **Seed 2** has a trailing lag and a correlation above 0.3. Its hunter autocorrelation's first peak is at 2750, so lag < period/2 fails.
- **Seeds 3 and 42** correlate weakly (0.20 and 0.29). Seed 3's best lag is at the edge of the search, 8000.
- **Persistence.** All four seeds persist to 60000 ticks under `check --long`.
- **Against the 64 world.** At shot 14c the current model passed the signature on 0 of 4 seeds there; on the strip it passes 1 of 4. The strip's larger populations are one candidate reason, but this shot doesn't test that.
