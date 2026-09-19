# Shot 14a-rev findings: handling time and the long-run signature

**No cell meets the acceptance, so the shot is Blocked and no default moved.** The acceptance asks for seeds 1, 2, 3 and 42 to persist to 60000 ticks, and for pp_corr > 0.3 with 0 < pp_lag < pp_period / 2 on at least 3 of the 4 seeds. Every sweep runs with `disease.hunter_rate=0` (change 4) and `hunter.kill_energy=60`, 14a's best cell. No cell of any sweep has more than 2 of 4 seeds persisting. No persisting seed shows the signature either.

## The signature (new definition)

`ecosim stats --signature` and the sweep columns `pp_lag`, `pp_corr` and `pp_period` (new) are computed as follows:
- **Detrending.** Grazer and hunter counts are detrended over the whole run by subtracting a centred moving average, t − 2000..=t + 2000. The average is cut short at the run's ends.
- **Window.** Ticks 5000–60000, or to the run's end.
- **Lag.** pp_lag is the lag L in −8000..8000 (step 50) that maximises the Pearson correlation of grazers(t) with hunters(t + L), and pp_corr is that correlation.
- **Period.** A *lobe* is a maximal run of lags with correlation above 0, and its *peak* is its largest value. Lobes cut by the ±8000 edge don't count. pp_period is the lag distance from the peak nearest lag 0 to the peak nearest that one. With fewer than two lobes it is undefined, shown as `undef` below.
- **Why lobes.** Lobes replace plain local maxima because the first run of these sweeps used local maxima. Noise wiggles on a single lobe then gave periods of 250 ticks (DECISIONS.md).
- **Undefined signature.** When grazers or hunters reach 0 inside the window, the signature is undefined, as in 14a.

**How to read the tables.** Each cell reads `P lag / corr / period` when both animal species are alive at every tick. This is the `long_no_extinction` condition. Otherwise the cell reads `species extinct @tick, cause`, where the cause is the dominant death cause over the 500 ticks up to the extinction (the `ecosim stats` rule). A `*` would mark the acceptance signature, and no cell has one. `table.py` prints these tables from each sweep's `sweep.csv`. The `pass` column that `sweep.md` reports is the 20000-tick `ecosim check` set, applied to 60000-tick runs, so it is not the acceptance here.

## hunter.handling_ticks 0:200:25 (hunt_cost 1.0; seeds 1–3; 60000 ticks)

| hunter.handling_ticks | s1 | s2 | s3 | persist |
|---|---|---|---|---|
| 0 | grazers extinct @7347, eaten | hunters extinct @32725, starved | hunters extinct @20779, starved | 0/3 |
| 25 | grazers extinct @8339, eaten | hunters extinct @20825, starved | hunters extinct @20985, starved | 0/3 |
| 50 | grazers extinct @9497, eaten | hunters extinct @20691, starved | hunters extinct @21498, starved | 0/3 |
| 75 | hunters extinct @25533, starved | hunters extinct @19535, starved | hunters extinct @23837, old_age | 0/3 |
| 100 | hunters extinct @31345, old_age | hunters extinct @41484, starved | hunters extinct @25585, starved | 0/3 |
| 125 | grazers extinct @10089, eaten | hunters extinct @33661, starved | hunters extinct @24796, starved | 0/3 |
| 150 | hunters extinct @27934, old_age | hunters extinct @18255, starved | hunters extinct @21348, starved | 0/3 |
| 175 | grazers extinct @9719, eaten | hunters extinct @41846, starved | hunters extinct @24032, starved | 0/3 |
| 200 | P -3450 / 0.25 / 3200 | hunters extinct @29975, starved | hunters extinct @27100, starved | 1/3 |

- **Handling time does not rescue persistence at hunt_cost 1.0.** Seeds 2 and 3 lose their hunters to starvation in every cell, between ticks 18000 and 42000.
- **Seed 1 is non-monotone in handling time.** Its grazers are eaten at handling 0–50, 125 and 175. Its hunters die out at 75, 100 and 150, and it persists at 200.
- **200 is the "best" handling time**, with 1 of 3 seeds persisting, so the refractory sweep runs at 200.

## hunter.refractory 300:1500:300 (handling_ticks 200, hunt_cost 1.0; seeds 1–3; 60000 ticks)

| hunter.refractory | s1 | s2 | s3 | persist |
|---|---|---|---|---|
| 300 | grazers extinct @1450, eaten | grazers extinct @1424, eaten | grazers extinct @1430, eaten | 0/3 |
| 600 | grazers extinct @2675, eaten | grazers extinct @2585, eaten | grazers extinct @2720, eaten | 0/3 |
| 900 | grazers extinct @3834, eaten | grazers extinct @3963, eaten | grazers extinct @3732, eaten | 0/3 |
| 1200 | grazers extinct @4921, eaten | grazers extinct @5196, eaten | grazers extinct @5114, eaten | 0/3 |
| 1500 | grazers extinct @6287, eaten | grazers extinct @7065, eaten | grazers extinct @6479, eaten | 0/3 |

- **A shorter refractory is worse in every cell.** Grazers are eaten out, and the extinction tick grows with the refractory, from about 1400 at 300 to about 6500 at 1500.
- **Handling time bounds each hunter's kill rate but not the number of hunters.** With births every 300–1500 ticks, hunter numbers grow faster than any per-hunter cap can hold. This is the same result as 14a's diagnostic 2, now with a handling time of 200.
- The sweep took 5 s, because each cell ends in a double extinction early on.

## 2-D grid: handling_ticks {25, 50, 100, 150} × hunt_cost {0.6, 0.8, 1.0, 1.2} (seeds 1, 2, 3 and 42; 60000 ticks)

Seed 42 is added to the prompt's seeds 1–3, because the acceptance is stated over all four.

| hunter.handling_ticks | hunter.hunt_cost | s1 | s2 | s3 | s42 | persist |
|---|---|---|---|---|---|---|
| 25 | 0.6 | grazers extinct @7390, eaten | grazers extinct @9127, eaten | grazers extinct @8531, eaten | P 4950 / 0.35 / 3700 | 1/4 |
| 25 | 0.8 | grazers extinct @7150, eaten | P -6950 / 0.28 / 2050 | grazers extinct @8607, eaten | hunters extinct @41375, starved | 1/4 |
| 25 | 1.0 | grazers extinct @8339, eaten | hunters extinct @20825, starved | hunters extinct @20985, starved | hunters extinct @19957, starved | 0/4 |
| 25 | 1.2 | grazers extinct @8531, eaten | hunters extinct @15803, starved | hunters extinct @18506, starved | hunters extinct @22725, starved | 0/4 |
| 50 | 0.6 | grazers extinct @8407, eaten | grazers extinct @9028, eaten | grazers extinct @9512, eaten | grazers extinct @23225, eaten | 0/4 |
| 50 | 0.8 | grazers extinct @7575, eaten | grazers extinct @12847, eaten | hunters extinct @25777, starved | hunters extinct @32966, starved | 0/4 |
| 50 | 1.0 | grazers extinct @9497, eaten | hunters extinct @20691, starved | hunters extinct @21498, starved | hunters extinct @20384, starved | 0/4 |
| 50 | 1.2 | hunters extinct @24377, starved | hunters extinct @16337, old_age | hunters extinct @15646, starved | hunters extinct @16232, old_age | 0/4 |
| 100 | 0.6 | grazers extinct @9373, eaten | grazers extinct @12768, eaten | P 4700 / 0.36 / 3600 | P -7500 / 0.28 / 3400 | 2/4 |
| 100 | 0.8 | grazers extinct @9142, eaten | hunters extinct @57919, starved | hunters extinct @29647, starved | hunters extinct @22523, starved | 0/4 |
| 100 | 1.0 | hunters extinct @31345, old_age | hunters extinct @41484, starved | hunters extinct @25585, starved | hunters extinct @14850, starved | 0/4 |
| 100 | 1.2 | hunters extinct @21304, starved | hunters extinct @16808, starved | hunters extinct @14390, starved | hunters extinct @16310, starved | 0/4 |
| 150 | 0.6 | grazers extinct @13129, eaten | grazers extinct @42504, eaten | grazers extinct @22561, eaten | P -2300 / 0.26 / 3800 | 1/4 |
| 150 | 0.8 | P -3150 / 0.21 / 4300 | P 4950 / 0.40 / 2500 | hunters extinct @34602, starved | hunters extinct @25663, starved | 2/4 |
| 150 | 1.0 | hunters extinct @27934, old_age | hunters extinct @18255, starved | hunters extinct @21348, starved | hunters extinct @22869, starved | 0/4 |
| 150 | 1.2 | hunters extinct @19516, old_age | hunters extinct @17409, starved | hunters extinct @22612, old_age | hunters extinct @12448, starved | 0/4 |

**Regions.**
- **hunt_cost ≥ 1.0 is a contiguous hunter-collapse region**, mostly `starved` with some `old_age`, at every handling time and on every seed. The one exception is seed 1 at handling ≤ 50, where the grazers are eaten first.
- **hunt_cost 0.6 at handling ≤ 50 is a contiguous grazer-collapse region** (`eaten`) on seeds 1–3.
- **The survivors are scattered cell-seeds, not a region.** There are 7 of 64: two at 150/0.8, two at 100/0.6, and single seeds at 25/0.6, 25/0.8 and 150/0.6. The only repeat is seed 42 at hunt_cost 0.6, where it persists at handling 25, 100 and 150 but not at 50. No cell has more than 2 of 4 seeds persisting.
- **Seed 1 persists in only one cell (150/0.8).** It still loses its grazers at low hunt_cost, as in 14a.

**The signature where the run persists.**
- Three persisting cell-seeds have pp_corr > 0.3 and a positive lag: 25/0.6 s42 (4950 / 0.35), 100/0.6 s3 (4700 / 0.36) and 150/0.8 s2 (4950 / 0.40).
- In all three, pp_lag is larger than the measured period (2500–3700), so it isn't below half a period. The best lag lies one lobe out, not in the lobe nearest lag 0.

## Context: the current defaults (hunter crowding on) at 60000 ticks

This is not an adoptable cell, because change 4 sets `disease.hunter_rate` to 0. It shows what the new signature reads on the model as it stands.

| seed | long_no_extinction | pp_lag / pp_corr / pp_period |
|---|---|---|
| 1 | pass | −1950 / 0.35 / 2800 |
| 2 | pass | −1800 / 0.34 / 2850 |
| 3 | pass | 850 / 0.39 / 2800 * |
| 42 | pass | 850 / 0.35 / 3200 * |

- **With crowding on, every seed persists and 2 of 4 show the signature.** The acceptance needs 3.
- **The measured periods (2800–3200) are below the 4000-tick year.** They are not the 8000–10000-tick hunter excursions that 14a described.
- **Why.** A centred moving average exactly one year wide cancels the seasonal cycle in the trend, so the detrended series keep the full seasonal swing. The ±8000 cross-correlation then shows lobes at seasonal spacing. A longer detrending window, or a deseasonalised series, would read the slow hunter cycle instead. That is a signature-definition question for the operator. This shot did not change the definition.

## Wall time

Each sweep ran 60000-tick cells with `--jobs 24` on a 24-thread machine. The walls are recorded in `_log.txt`:

| sweep | wall |
|---|---|
| handling_ticks, 27 cells | 107 s |
| refractory, 15 cells | 5 s |
| 2-D grid, 64 cells | 229 s |

Each is well under the 15 minutes the prompt allows.

## Files

- `_log.txt`: the command for each sweep.
- `table.py`: the tables above. It reads `sweep.csv`.
- The per-cell series under `*/cells/` are gitignored. Rerunning a sweep regenerates them.
