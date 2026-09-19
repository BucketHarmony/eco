# Shot 14c re-score: the fixed predator–prey signature

**The current model does not pass the fixed check: 0 of 4 seeds (1, 2, 3, 42) pass, and the bar is 3 of 4.** All four persist to 60000 ticks, and their hunter periods now read 5500–8250 ticks, where the old check read the 2800–3200-tick seasons. But on every seed the best lag is either negative or longer than half a period. Across 14a-rev's 106 sweep cell-seeds, 2 pass: 100/0.6 on s42 and 150/0.8 on s1. No cell passes on more than 1 of its seeds. Nothing was tuned, and no new experiment was run.

## What was scored, and how

- **The fixed signature** (`ecosim stats --signature`, sweep column `pp_pass`; definition in DECISIONS.md, "Signature fix (shot 14c)"):
  - Both series are detrended by a 12000-tick centred moving average, then deseasonalised by subtracting their mean at each phase (tick mod `climate.year_len`).
  - The window is ticks 5000–60000. pp_lag is the best lag in −8000..8000 (step 50), and pp_corr is its correlation.
  - pp_period is the first positive local maximum of the hunter autocorrelation after it first goes negative, searched over lags 50–20000.
  - pp_pass is 0 < pp_lag < pp_period / 2 with pp_corr > 0.3.
- **14a-rev's 106 runs** were re-scored from the per-cell series on disk (`sweeps/shot14a-rev/*/cells/*.csv`, gitignored). Nothing was rerun. `stats --signature` now also accepts a bare series file, given `--year-len`. That is the smallest path to scoring a cell CSV, and it needed no new subcommand. Every sweep used the default `year_len = 4000`.
- **The current model** is seeds 1, 2, 3 and 42 at defaults (hunter crowding on), 60000 ticks. 14a-rev's Context runs were not on disk, so these four were rerun exactly as 14a-rev ran them: `ecosim run --seed S --ticks 60000 --snapshot-every 10000`. As in 14a-rev, all four persist, and `check --long` passes on each (margins +0.71 to +0.76).
- `rescore.py` produced everything below. `rescore.csv` holds every re-scored run: lag, corr, period, pass, and the undefined line for runs with an extinction in the window. The script takes 9 s.

**How to read the tables.** They use 14a-rev's layout, with persistence unchanged: a cell reads `species extinct @tick, cause` whenever 14a-rev's `first_extinction_tick` is set. Persisting cells read `P lag / corr / period` under the fixed definition, and `PASS` marks pp_pass. The last column counts the passes.

### handling_ticks

| hunter.handling_ticks | s1 | s2 | s3 | persist | pass |
|---|---|---|---|---|---|
| 0 | grazers extinct @7347, eaten | hunters extinct @32725, starved | hunters extinct @20779, starved | 0/3 | 0/3 |
| 25 | grazers extinct @8339, eaten | hunters extinct @20825, starved | hunters extinct @20985, starved | 0/3 | 0/3 |
| 50 | grazers extinct @9497, eaten | hunters extinct @20691, starved | hunters extinct @21498, starved | 0/3 | 0/3 |
| 75 | hunters extinct @25533, starved | hunters extinct @19535, starved | hunters extinct @23837, old_age | 0/3 | 0/3 |
| 100 | hunters extinct @31345, old_age | hunters extinct @41484, starved | hunters extinct @25585, starved | 0/3 | 0/3 |
| 125 | grazers extinct @10089, eaten | hunters extinct @33661, starved | hunters extinct @24796, starved | 0/3 | 0/3 |
| 150 | hunters extinct @27934, old_age | hunters extinct @18255, starved | hunters extinct @21348, starved | 0/3 | 0/3 |
| 175 | grazers extinct @9719, eaten | hunters extinct @41846, starved | hunters extinct @24032, starved | 0/3 | 0/3 |
| 200 | P 4000 / 0.50 / 7300 | hunters extinct @29975, starved | hunters extinct @27100, starved | 1/3 | 0/3 |

### refractory

| hunter.refractory | s1 | s2 | s3 | persist | pass |
|---|---|---|---|---|---|
| 300 | grazers extinct @1450, eaten | grazers extinct @1424, eaten | grazers extinct @1430, eaten | 0/3 | 0/3 |
| 600 | grazers extinct @2675, eaten | grazers extinct @2585, eaten | grazers extinct @2720, eaten | 0/3 | 0/3 |
| 900 | grazers extinct @3834, eaten | grazers extinct @3963, eaten | grazers extinct @3732, eaten | 0/3 | 0/3 |
| 1200 | grazers extinct @4921, eaten | grazers extinct @5196, eaten | grazers extinct @5114, eaten | 0/3 | 0/3 |
| 1500 | grazers extinct @6287, eaten | grazers extinct @7065, eaten | grazers extinct @6479, eaten | 0/3 | 0/3 |

### handling_x_cost

| hunter.handling_ticks | hunter.hunt_cost | s1 | s2 | s3 | s42 | persist | pass |
|---|---|---|---|---|---|---|---|
| 25 | 0.6 | grazers extinct @7390, eaten | grazers extinct @9127, eaten | grazers extinct @8531, eaten | P -5700 / 0.54 / 7550 | 1/4 | 0/4 |
| 25 | 0.8 | grazers extinct @7150, eaten | P -4750 / 0.56 / 8250 | grazers extinct @8607, eaten | hunters extinct @41375, starved | 1/4 | 0/4 |
| 25 | 1.0 | grazers extinct @8339, eaten | hunters extinct @20825, starved | hunters extinct @20985, starved | hunters extinct @19957, starved | 0/4 | 0/4 |
| 25 | 1.2 | grazers extinct @8531, eaten | hunters extinct @15803, starved | hunters extinct @18506, starved | hunters extinct @22725, starved | 0/4 | 0/4 |
| 50 | 0.6 | grazers extinct @8407, eaten | grazers extinct @9028, eaten | grazers extinct @9512, eaten | grazers extinct @23225, eaten | 0/4 | 0/4 |
| 50 | 0.8 | grazers extinct @7575, eaten | grazers extinct @12847, eaten | hunters extinct @25777, starved | hunters extinct @32966, starved | 0/4 | 0/4 |
| 50 | 1.0 | grazers extinct @9497, eaten | hunters extinct @20691, starved | hunters extinct @21498, starved | hunters extinct @20384, starved | 0/4 | 0/4 |
| 50 | 1.2 | hunters extinct @24377, starved | hunters extinct @16337, old_age | hunters extinct @15646, starved | hunters extinct @16232, old_age | 0/4 | 0/4 |
| 100 | 0.6 | grazers extinct @9373, eaten | grazers extinct @12768, eaten | P 8000 / 0.23 / 16550 | P 3450 / 0.37 / 8250 PASS | 2/4 | 1/4 |
| 100 | 0.8 | grazers extinct @9142, eaten | hunters extinct @57919, starved | hunters extinct @29647, starved | hunters extinct @22523, starved | 0/4 | 0/4 |
| 100 | 1.0 | hunters extinct @31345, old_age | hunters extinct @41484, starved | hunters extinct @25585, starved | hunters extinct @14850, starved | 0/4 | 0/4 |
| 100 | 1.2 | hunters extinct @21304, starved | hunters extinct @16808, starved | hunters extinct @14390, starved | hunters extinct @16310, starved | 0/4 | 0/4 |
| 150 | 0.6 | grazers extinct @13129, eaten | grazers extinct @42504, eaten | grazers extinct @22561, eaten | P -4700 / 0.43 / 8250 | 1/4 | 0/4 |
| 150 | 0.8 | P 4700 / 0.50 / 12750 PASS | P -7000 / 0.49 / 13850 | hunters extinct @34602, starved | hunters extinct @25663, starved | 2/4 | 1/4 |
| 150 | 1.0 | hunters extinct @27934, old_age | hunters extinct @18255, starved | hunters extinct @21348, starved | hunters extinct @22869, starved | 0/4 | 0/4 |
| 150 | 1.2 | hunters extinct @19516, old_age | hunters extinct @17409, starved | hunters extinct @22612, old_age | hunters extinct @12448, starved | 0/4 | 0/4 |

### Current model (defaults, hunter crowding on), 60000 ticks

| seed | old pp_lag / pp_corr / pp_period | new pp_lag / pp_corr / pp_period | old period | new period | pp_pass |
|---|---|---|---|---|---|
| 1 | -1950 / 0.35 / 2800 | -4750 / 0.42 / 8250 | 2800 | 8250 | false |
| 2 | -1800 / 0.34 / 2850 | 4250 / 0.36 / 6900 | 2850 | 6900 | false |
| 3 | 850 / 0.39 / 2800 | 3700 / 0.43 / 5500 | 2800 | 5500 | false |
| 42 | 850 / 0.35 / 3200 | -5900 / 0.17 / 8250 | 3200 | 8250 | false |

## The current model under the fixed check

The fixed check now measures the slow hunter cycle, not the seasons. The four seeds' hunter periods are 8250, 6900, 5500 and 8250 ticks, where 14a-rev's lobe spacing gave 2800–3200. So the old periods were seasonal, as 14a-rev's BLOCKED file guessed, and the 6000–10000-tick hunter excursions are real.

The current model still fails on all four seeds, for two reasons:
- **Seeds 1 and 42: hunters lead.** Their best lags are negative (−4750 and −5900), so grazer peaks follow hunter peaks rather than the reverse. Seed 42's correlation, 0.17, is also below 0.3.
- **Seeds 2 and 3: the lag is too long.** Hunters trail grazers by 4250 and 3700 ticks with pp_corr 0.36 and 0.43, above 0.3. But each lag is more than half that seed's hunter period (6900 and 5500), which puts it closer to the next grazer peak than to the one before.

14a-rev's two seeds that "passed" under the old definition, 3 and 42 at lag 850, were read off seasonal lobes. Under the fixed check neither passes.

## Reading the sweep re-score

- **The persisting cells are the same 8 cell-seeds as in 14a-rev.** The handling_ticks sweep adds 200 s1, the 2-D grid has 7, and the refractory sweep has none. The fixed check passes on 2 of them: 100/0.6 s42 (lag 3450, corr 0.37, period 8250) and 150/0.8 s1 (lag 4700, corr 0.50, period 12750).
- **Every other persisting cell has hunters leading, or a lag at or past half its period.** Examples are −5700 at 25/0.6 s42, 8000 (the edge of the lag range) against a 16550 period at 100/0.6 s3, and 4000 against 7300 at 200 s1.
- **The two passes don't form a region.** They are in different cells on different seeds, and each shares its cell with a seed that goes extinct or fails.
- **Persistence is still the binding failure.** The fixed signature changes none of 14a-rev's persistence results: at most 2 of 4 seeds persist in any cell.
