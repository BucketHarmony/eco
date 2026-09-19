# Shot 14a findings: food-limited hunters

**No cell reaches the target.** The target is pp_lag > 0 and pp_corr > 0.3 on every seed, with the anchor intact (seeds 1, 2, 3 and 42 pass `ecosim check`). Every sweep below misses it, and so does every exploratory grid. With `disease.hunter_rate` 0, which is change 1 of the shot, no kill_energy × hunt_cost cell even keeps the anchor. The shot is Blocked with this finding. The defaults are unchanged: `disease.hunter_rate` 0.001, `kill_energy` 40, and the new `hunt_cost` 0, which is the pre-shot rule.

## The signature

`ecosim stats --signature` and the sweep columns `pp_lag`, `pp_corr` give the lag L in −2000..2000 (step 50) that maximises the Pearson correlation of grazers(t) with hunters(t + L), over ticks 2000–20000. A positive L means hunters follow grazers. When grazers or hunters reach 0 inside the window, the signature is undefined. The cell is then reported as extinct with the dominant cause of that extinction (`pp_undefined`, and "X extinct, cause" in the tables).

Each table cell reads `verdict pp_lag / pp_corr`, where the verdict is P or F for `ecosim check`. "(wide L / c)" recomputes the correlation over lags −6000..6000 from the per-cell series (`signature.py --wide`). It shows where the maximum lies when the official one sits at the ±2000 edge. It is a diagnostic only; the signature itself is the ±2000 one.

## Required sweeps (seeds 1–3, `disease.hunter_rate=0`)

### hunter.kill_energy 20:80:10 (half to double the old 40), hunt_cost 1.0

hunt_cost is fixed at 1.0, the middle of the only band where any seed survives (see the fine grid). At the old default, hunt_cost 0, every cell is a grazer extinction.

| hunter.kill_energy | s1 | s2 | s3 |
|---|---|---|---|
| 20 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved |
| 30 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved |
| 40 | F grazers extinct, eaten | P 2000 / -0.22 | F hunters extinct, starved |
| 50 | F grazers extinct, eaten | F hunters extinct, starved | P 2000 / -0.52 |
| 60 | F grazers extinct, eaten | P 2000 / -0.35 | P 2000 / -0.53 |
| 70 | F grazers extinct, eaten | P 2000 / -0.27 | F grazers extinct, eaten |
| 80 | F grazers extinct, eaten | P 2000 / -0.19 | P 2000 / -0.57 |

target region (pp_lag > 0 and pp_corr > 0.3 on every seed): none

### hunter.hunt_cost 0:5:1, kill_energy 40

| hunter.hunt_cost | s1 | s2 | s3 |
|---|---|---|---|
| 0 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 1 | F grazers extinct, eaten | P 2000 / -0.22 | F hunters extinct, starved |
| 2 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved |
| 3 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 4 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 5 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |

target region (pp_lag > 0 and pp_corr > 0.3 on every seed): none

### 4×4 grid: kill_energy 20, 40, 60, 80 × hunt_cost 0, 1, 2, 3

| hunter.kill_energy | hunter.hunt_cost | s1 | s2 | s3 |
|---|---|---|---|---|
| 20 | 0 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 20 | 1 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved |
| 20 | 2 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 20 | 3 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 0 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 40 | 1 | F grazers extinct, eaten | P 2000 / -0.22 | F hunters extinct, starved |
| 40 | 2 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 3 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 60 | 0 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 60 | 1 | F grazers extinct, eaten | P 2000 / -0.35 | P 2000 / -0.53 |
| 60 | 2 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved |
| 60 | 3 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 80 | 0 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 80 | 1 | F grazers extinct, eaten | P 2000 / -0.19 | P 2000 / -0.57 |
| 80 | 2 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved |
| 80 | 3 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |

target region (pp_lag > 0 and pp_corr > 0.3 on every seed): none

**Regions.** Outcomes follow hunt_cost far more than kill_energy. At 0 every seed is a grazer extinction (`eaten`), and at 3 every seed is a hunter extinction (`starved`). They form two contiguous regions, one on each side, and they hold on all three seeds. Everything between them is a thin, seed-dependent band around hunt_cost 1. Seed 1 is a grazer extinction there too. Survivors are isolated cells, not a region: no cell survives on all three seeds.

## The step is too coarse: fine grid (seeds 1, 2, 3 and 42, `disease.hunter_rate=0`)

This is kill_energy 40:80:10 × hunt_cost 0.5:1.5:0.1, which fills in the band the unit step skips. Outcome counts per seed over the 55 cells:

| seed | pass | grazers eaten | hunters starved | hunters old_age | fail, both alive |
|---|---|---|---|---|---|
| 1 | 1 | 54 | 0 | 0 | 0 |
| 2 | 25 | 10 | 18 | 2 | 0 |
| 3 | 7 | 19 | 25 | 4 | 0 |
| 42 | 13 | 13 | 26 | 2 | 1 |

| hunter.kill_energy | hunter.hunt_cost | s1 | s2 | s3 | s42 |
|---|---|---|---|---|---|
| 40 | 0.5 | F grazers extinct, eaten | P 2000 / -0.08 (wide 5150 / 0.43) | F grazers extinct, eaten | F 2000 / 0.39 (wide 4450 / 0.84) |
| 40 | 0.6 | F grazers extinct, eaten | P 2000 / -0.18 (wide -5250 / 0.05) | F grazers extinct, eaten | F grazers extinct, eaten |
| 40 | 0.7 | F grazers extinct, eaten | F grazers extinct, eaten | F hunters extinct, starved | P 2000 / -0.58 (wide 6000 / 0.18) |
| 40 | 0.8 | F grazers extinct, eaten | P 2000 / -0.26 (wide 6000 / 0.35) | P 2000 / -0.54 (wide -6000 / 0.01) | F hunters extinct, starved |
| 40 | 0.9 | F grazers extinct, eaten | P 2000 / -0.55 (wide -5850 / 0.03) | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 1.0 | F grazers extinct, eaten | P 2000 / -0.22 (wide 6000 / 0.04) | F hunters extinct, starved | F grazers extinct, eaten |
| 40 | 1.1 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 1.2 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 1.3 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 1.4 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 1.5 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, old_age | F hunters extinct, starved |
| 50 | 0.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 50 | 0.6 | F grazers extinct, eaten | P 2000 / -0.50 (wide 5950 / 0.07) | F grazers extinct, eaten | F grazers extinct, eaten |
| 50 | 0.7 | F grazers extinct, eaten | P 2000 / -0.55 (wide -6000 / -0.08) | P 2000 / -0.54 (wide 6000 / 0.43) | F grazers extinct, eaten |
| 50 | 0.8 | F grazers extinct, eaten | P 2000 / 0.07 (wide 3950 / 0.61) | F grazers extinct, eaten | P 2000 / -0.54 (wide -6000 / 0.06) |
| 50 | 0.9 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 50 | 1.0 | F grazers extinct, eaten | F hunters extinct, starved | P 2000 / -0.52 (wide -6000 / 0.14) | F hunters extinct, starved |
| 50 | 1.1 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 50 | 1.2 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 50 | 1.3 | F grazers extinct, eaten | F hunters extinct, old_age | F hunters extinct, starved | F hunters extinct, starved |
| 50 | 1.4 | F grazers extinct, eaten | P -2000 / 0.66 (wide -3750 / 0.94) | F hunters extinct, starved | F hunters extinct, starved |
| 50 | 1.5 | P 2000 / -0.30 (wide -6000 / 0.07) | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 60 | 0.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | P 2000 / -0.13 (wide 5250 / 0.82) |
| 60 | 0.6 | F grazers extinct, eaten | P 2000 / 0.02 (wide 4350 / 0.55) | F grazers extinct, eaten | P 2000 / 0.38 (wide -5850 / 0.70) |
| 60 | 0.7 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 60 | 0.8 | F grazers extinct, eaten | P 2000 / -0.53 (wide -6000 / 0.11) | F grazers extinct, eaten | F grazers extinct, eaten |
| 60 | 0.9 | F grazers extinct, eaten | P 2000 / 0.01 (wide 5500 / 0.73) | F hunters extinct, starved | P 1650 / -0.53 (wide -6000 / -0.16) |
| 60 | 1.0 | F grazers extinct, eaten | P 2000 / -0.35 (wide -5700 / 0.80) | P 2000 / -0.53 (wide -6000 / 0.05) | P 2000 / -0.30 (wide -5400 / 0.69) |
| 60 | 1.1 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, old_age | F hunters extinct, starved |
| 60 | 1.2 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 60 | 1.3 | F grazers extinct, eaten | F hunters extinct, old_age | F hunters extinct, starved | F hunters extinct, starved |
| 60 | 1.4 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 60 | 1.5 | F grazers extinct, eaten | P -2000 / 0.33 (wide -3800 / 0.83) | F hunters extinct, starved | F hunters extinct, starved |
| 70 | 0.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 70 | 0.6 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 70 | 0.7 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 70 | 0.8 | F grazers extinct, eaten | P 2000 / -0.40 (wide -6000 / -0.15) | F grazers extinct, eaten | P 2000 / -0.51 (wide 6000 / -0.03) |
| 70 | 0.9 | F grazers extinct, eaten | P 2000 / -0.50 (wide -6000 / 0.01) | F grazers extinct, eaten | F hunters extinct, starved |
| 70 | 1.0 | F grazers extinct, eaten | P 2000 / -0.27 (wide 6000 / 0.48) | F grazers extinct, eaten | P 2000 / -0.44 (wide 5800 / -0.13) |
| 70 | 1.1 | F grazers extinct, eaten | P 2000 / -0.48 (wide -6000 / 0.30) | F hunters extinct, starved | F hunters extinct, starved |
| 70 | 1.2 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 70 | 1.3 | F grazers extinct, eaten | P 2000 / -0.66 (wide -6000 / 0.19) | F hunters extinct, old_age | F hunters extinct, old_age |
| 70 | 1.4 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 70 | 1.5 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 80 | 0.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 80 | 0.6 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 80 | 0.7 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 80 | 0.8 | F grazers extinct, eaten | P 2000 / 0.49 (wide -4450 / 0.67) | F grazers extinct, eaten | P 2000 / -0.25 (wide -6000 / 0.13) |
| 80 | 0.9 | F grazers extinct, eaten | P 2000 / -0.39 (wide -5850 / -0.23) | P 2000 / -0.58 (wide 6000 / -0.11) | P 2000 / -0.53 (wide 5550 / -0.25) |
| 80 | 1.0 | F grazers extinct, eaten | P 2000 / -0.19 (wide -6000 / 0.61) | P 2000 / -0.57 (wide -6000 / -0.08) | P 2000 / -0.51 (wide -5650 / 0.27) |
| 80 | 1.1 | F grazers extinct, eaten | P 2000 / -0.46 (wide 6000 / 0.24) | F hunters extinct, starved | P 2000 / -0.56 (wide -6000 / -0.03) |
| 80 | 1.2 | F grazers extinct, eaten | P 2000 / -0.28 (wide -6000 / 0.11) | F hunters extinct, starved | F hunters extinct, old_age |
| 80 | 1.3 | F grazers extinct, eaten | F hunters extinct, starved | P -2000 / -0.45 (wide -6000 / 0.78) | P 2000 / -0.65 (wide 6000 / 0.05) |
| 80 | 1.4 | F grazers extinct, eaten | F hunters extinct, starved | F hunters extinct, old_age | F hunters extinct, starved |
| 80 | 1.5 | F grazers extinct, eaten | P 2000 / -0.53 (wide -6000 / -0.01) | F hunters extinct, starved | F hunters extinct, starved |

target region (pp_lag > 0 and pp_corr > 0.3 on every seed): none

- **No cell passes on all four seeds.** Seed 1 loses its grazers to predation in 54 of 55 cells, and its one pass (kill_energy 50, hunt_cost 1.5) is a hunter-starvation cell on every other seed.
- **Among the cells where the anchor holds, the signature is almost never positive.** 45 of the 46 have pp_lag at the +2000 or −2000 edge, and only four have pp_corr > 0.3, each on a single seed. Two of those are at lag +2000 (kill_energy/hunt_cost 80/0.8 on seed 2, 0.49; 60/0.6 on seed 42, 0.38) and two at −2000 (50/1.4 and 60/1.5 on seed 2).
- **Regions.** Collapse by predation (low hunt_cost) and collapse by starvation (high hunt_cost) are each contiguous. Between them lies a strip about 0.3–0.5 wide in hunt_cost. Inside the strip the outcomes flip from seed to seed and from one 0.1 step to the next, so surviving cells are scattered rather than contiguous.

**Best region.** With hunter crowding off, the best cells are kill_energy 60–80 at hunt_cost 1.0. Three of the four seeds pass there (all but seed 1), and every pp_corr is negative (−0.19 to −0.57) at the +2000 edge.

## Diagnostic 1: with hunter crowding left on (0.001), seeds 1, 2, 3 and 42

This isn't an adoptable region, because change 1 sets `disease.hunter_rate` to 0. The sweep asks whether hunt_cost gives a signature once crowding holds the anchor.

| hunter.kill_energy | hunter.hunt_cost | s1 | s2 | s3 | s42 |
|---|---|---|---|---|---|
| 40 | 0.00 | P 2000 / -0.08 (wide -6000 / 0.43) | P 2000 / 0.34 (wide 4000 / 0.71) | P -2000 / -0.50 (wide 6000 / 0.37) | P 2000 / -0.15 (wide 5800 / 0.51) |
| 40 | 0.25 | P 2000 / -0.22 (wide 3850 / 0.07) | P 1050 / -0.04 (wide 6000 / 0.16) | P 2000 / 0.12 (wide 3300 / 0.38) | P 2000 / -0.49 (wide -5250 / -0.05) |
| 40 | 0.50 | P 2000 / -0.28 (wide 4500 / 0.04) | P 1300 / -0.42 (wide -6000 / 0.36) | P 2000 / -0.56 (wide 6000 / 0.29) | P 2000 / -0.26 (wide -6000 / -0.05) |
| 40 | 0.75 | P 2000 / -0.52 (wide 6000 / 0.20) | P -2000 / 0.13 (wide -4150 / 0.71) | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 1.00 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 1.25 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 40 | 1.50 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 50 | 0.00 | P 2000 / 0.02 (wide 3850 / 0.34) | P 1800 / -0.00 (wide 4850 / 0.35) | P 2000 / 0.20 (wide -4000 / 0.39) | P 2000 / 0.06 (wide 3200 / 0.32) |
| 50 | 0.25 | P 2000 / 0.06 (wide 3150 / 0.45) | P 1150 / -0.18 (wide 4300 / 0.07) | P 2000 / 0.25 (wide 3300 / 0.29) | P 2000 / -0.09 (wide 4350 / 0.27) |
| 50 | 0.50 | P 2000 / -0.32 (wide 6000 / 0.38) | P 2000 / -0.05 (wide 4600 / 0.33) | P 2000 / -0.25 (wide 5200 / 0.47) | P 2000 / -0.44 (wide 5100 / -0.02) |
| 50 | 0.75 | P 2000 / -0.50 (wide -5100 / -0.25) | P -2000 / 0.69 (wide -6000 / 0.88) | P -2000 / -0.61 (wide -6000 / 0.43) | F hunters extinct, starved |
| 50 | 1.00 | P 1900 / -0.59 (wide -6000 / 0.10) | P -2000 / -0.16 (wide -6000 / 0.57) | P -2000 / -0.39 (wide -6000 / 0.33) | F hunters extinct, starved |
| 50 | 1.25 | P 2000 / -0.45 (wide -6000 / 0.64) | P -1850 / 0.61 (wide -4850 / 0.80) | F hunters extinct, starved | F hunters extinct, starved |
| 50 | 1.50 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 60 | 0.00 | P 2000 / -0.22 (wide 6000 / 0.18) | P 2000 / -0.15 (wide 4150 / 0.31) | P 2000 / -0.26 (wide 6000 / 0.54) | P 2000 / 0.07 (wide 3800 / 0.23) |
| 60 | 0.25 | P 2000 / -0.39 (wide 6000 / 0.36) | P 2000 / 0.22 (wide 5800 / 0.59) | P 2000 / 0.19 (wide 4300 / 0.72) | P 2000 / -0.14 (wide 5750 / 0.63) |
| 60 | 0.50 | P 2000 / -0.18 (wide 6000 / 0.31) | P 2000 / 0.03 (wide -4950 / 0.37) | P 2000 / -0.39 (wide -4700 / -0.07) | P 2000 / -0.34 (wide 6000 / -0.15) |
| 60 | 0.75 | P 2000 / -0.39 (wide 5950 / 0.08) | P 1800 / -0.26 (wide 6000 / 0.03) | P 2000 / -0.71 (wide 6000 / -0.07) | P 2000 / -0.44 (wide -6000 / -0.16) |
| 60 | 1.00 | F hunters extinct, starved | P 2000 / -0.39 (wide -6000 / 0.23) | P 2000 / -0.43 (wide -6000 / 0.79) | F hunters extinct, starved |
| 60 | 1.25 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 60 | 1.50 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, old_age |
| 70 | 0.00 | P 2000 / 0.00 (wide 3650 / 0.27) | P 2000 / 0.27 (wide 4750 / 0.50) | P 2000 / 0.34 (wide 3000 / 0.73) | P 1350 / 0.24 (wide 4450 / 0.34) |
| 70 | 0.25 | P 1800 / 0.01 (wide 3750 / 0.17) | P 2000 / 0.27 (wide 2450 / 0.37) | P 2000 / 0.18 (wide 6000 / 0.52) | P 2000 / 0.12 (wide 3550 / 0.25) |
| 70 | 0.50 | P 2000 / -0.34 (wide 4650 / 0.09) | P 1950 / -0.31 (wide 3850 / -0.16) | P 2000 / -0.48 (wide 5950 / 0.14) | P 2000 / -0.40 (wide 6000 / 0.27) |
| 70 | 0.75 | P 2000 / -0.52 (wide 6000 / 0.09) | P 2000 / -0.30 (wide 6000 / 0.29) | P -2000 / -0.67 (wide -6000 / -0.11) | P -2000 / -0.39 (wide -6000 / 0.68) |
| 70 | 1.00 | F hunters extinct, starved | P -2000 / -0.46 (wide -6000 / -0.08) | F hunters extinct, starved | F hunters extinct, starved |
| 70 | 1.25 | F hunters extinct, old_age | P -2000 / 0.13 (wide -5300 / 0.61) | F hunters extinct, starved | F hunters extinct, old_age |
| 70 | 1.50 | F hunters extinct, starved | P -2000 / -0.19 (wide -5800 / 0.57) | F hunters extinct, starved | F hunters extinct, starved |
| 80 | 0.00 | P 1400 / -0.05 (wide 5450 / 0.61) | P 1950 / -0.19 (wide 5950 / 0.09) | P 2000 / 0.34 (wide 3100 / 0.55) | P 2000 / -0.15 (wide 6000 / 0.56) |
| 80 | 0.25 | P 1550 / -0.11 (wide 4000 / 0.19) | P 1300 / 0.06 (wide 4300 / 0.41) | P 2000 / 0.01 (wide 6000 / 0.33) | P 1950 / -0.21 (wide 4250 / 0.04) |
| 80 | 0.50 | P 2000 / 0.11 (wide 4150 / 0.46) | P 2000 / -0.60 (wide 4450 / -0.22) | P 2000 / -0.33 (wide 6000 / 0.42) | P 2000 / -0.57 (wide -5950 / -0.28) |
| 80 | 0.75 | P 2000 / 0.17 (wide -4400 / 0.52) | P 2000 / -0.34 (wide -6000 / 0.02) | P 2000 / -0.60 (wide -6000 / -0.12) | P 2000 / 0.33 (wide -4900 / 0.61) |
| 80 | 1.00 | F hunters extinct, starved | P 1700 / 0.12 (wide -5150 / 0.74) | F hunters extinct, starved | F hunters extinct, starved |
| 80 | 1.25 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved |
| 80 | 1.50 | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, starved | F hunters extinct, old_age |

target region (pp_lag > 0 and pp_corr > 0.3 on every seed): none

The anchor now holds over a contiguous block, hunt_cost ≤ 0.5 to 0.75 at every kill_energy. Even so, no cell reaches the target. The closest cell is kill_energy 70 at hunt_cost 0: every seed has a positive lag (1350–2000), but pp_corr is 0.00, 0.27, 0.34 and 0.24. Its wide maxima fall at lags 3000–4750, with corr 0.27–0.73.

## Diagnostic 2: a short refractory (500), `disease.hunter_rate=0`

`hunter.refractory` is outside this shot's tuning surface. This diagnostic asks whether the slow hunter response is what hides the signature.

| hunter.kill_energy | hunter.hunt_cost | s1 | s2 | s3 | s42 |
|---|---|---|---|---|---|
| 20 | 0.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 20 | 1 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 20 | 1.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 20 | 2 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 40 | 0.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 40 | 1 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 40 | 1.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 40 | 2 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 60 | 0.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 60 | 1 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 60 | 1.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 60 | 2 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 80 | 0.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 80 | 1 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 80 | 1.5 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |
| 80 | 2 | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten | F grazers extinct, eaten |

target region (pp_lag > 0 and pp_corr > 0.3 on every seed): none

Every cell is a grazer extinction, `eaten`, even at hunt_cost 2. When hunters can breed every 500 ticks, they convert a grazer peak into hunters faster than the hunting cost can starve them, and the grazers are eaten out before the hunters decline.

## Why no cell works (best guess)

- **The hunter response is slower than the lag window.** Births are capped at one per `refractory` (2750) ticks, and a hunter lives up to 8000. Across the grids, the wide maxima sit at lags of 3000–6000, and hunter excursions last about 8000–10000 ticks, so a run holds only about two of them in 18 000 ticks. The ±2000 range doesn't reach the response, and two cycles are too few for a stable correlation. That is why the best lag keeps landing on the ±2000 edge, with a sign that changes from seed to seed.
- **Without crowding there is nothing to stop hunters overshooting.** Seasonal grazer swings are 3–4×. hunt_cost has to be low enough for hunters to survive the troughs, and it is then too low to stop them eating out the peaks. Seed 1 shows this most clearly. hunt_cost 0.1 separates grazer extinction from hunter extinction, and seed noise is larger than that gap.
- **A shorter refractory makes it worse, not better** (diagnostic 2). Prey limitation needs something that holds hunter numbers down when grazers are dense: a hunter handling time, stronger prey refuges, or crowding. The shot's two knobs don't provide one.

## Files

- `_log.txt`: the command for each sweep.
- `signature.py`: the tables above. `--wide` needs the gitignored `cells/`; rerun the sweep to regenerate them.
- `cycle_ratio.py` (`sweeps/`) still works on these sweeps. It ignores the new columns.
