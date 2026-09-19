# Sweep `rain_x_kill`

- params: `params.toml`
- `climate.rain_amp`: 0, 1, 2, 3, 4, 5, 6, 7, 8
- `hunter.kill_prob`: 0.1, 0.2, 0.3, 0.4, 0.5
- seeds: 1, 2, 3; ticks: 20000; cells: 135; jobs: 22
- wall time: 63.1 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `climate.rain_amp`

- default: 4.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 | 0/15 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.047); `animals_10k` (seed 3; worst margin -0.500) |
| 1 | 0/15 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.047); `animals_10k` (seed 3; worst margin -0.500) |
| 2 | 3/15 | `fertility_band` (seeds 1, 2, 3; worst margin -0.039); `no_extinction` (seeds 1, 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -0.600) |
| 3 | 11/15 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000); `fertility_band` (seed 3; worst margin -0.024) |
| 4 (default) | 14/15 | `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -0.800); `fertility_band` (seed 3; worst margin -0.014) |
| 5 | 3/15 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.829) |
| 6 | 0/15 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 2, 3; worst margin -1.000); `grazer_cycle` (seed 2; worst margin -1.000); `tree_growth` (seed 2; worst margin -1.000) |
| 7 | 0/15 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `tree_growth` (seeds 2, 3; worst margin -1.000) |
| 8 | 0/15 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `tree_growth` (seeds 2, 3; worst margin -1.000) |

## `hunter.kill_prob`

- default: 0.3
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.1 | 5/27 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.943); `fertility_band` (seeds 1, 2, 3; worst margin -0.016); `tree_growth` (seed 2; worst margin -0.778) |
| 0.2 | 8/27 | `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.013); `no_extinction` (seed 2; worst margin -1.000); `tree_growth` (seed 2; worst margin -1.000) |
| 0.3 (default) | 7/27 | `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.020); `no_extinction` (seeds 2, 3; worst margin -1.000); `animals_10k` (seeds 2, 3; worst margin -1.000); `tree_growth` (seed 2; worst margin -1.000) |
| 0.4 | 5/27 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.829); `fertility_band` (seeds 1, 2, 3; worst margin -0.024); `tree_growth` (seed 3; worst margin -0.056) |
| 0.5 | 6/27 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.047); `tree_growth` (seed 2; worst margin -1.000) |

## Matrix: seeds passing out of 3

| `climate.rain_amp` \ `hunter.kill_prob` | 0.1 | 0.2 | 0.3 | 0.4 | 0.5 |
|---|---|---|---|---|---|
| 0 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 |
| 1 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 |
| 2 | 0/3 | 1/3 | 0/3 | 1/3 | 1/3 |
| 3 | 1/3 | 3/3 | 3/3 | 2/3 | 2/3 |
| 4 | 3/3 | 3/3 | 3/3 | 2/3 | 3/3 |
| 5 | 1/3 | 1/3 | 1/3 | 0/3 | 0/3 |
| 6 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 |
| 7 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 |
| 8 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 |

## Extinctions by cause

- cells failing an invariant: 104 of 135
- cells in which a species reaches 0 at any tick: 48 of 135
- failing cells with no extinction: 56; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 32 | climate.rain_amp=0_hunter.kill_prob=0.5_s=3 @10121, climate.rain_amp=1_hunter.kill_prob=0.5_s=3 @10121, climate.rain_amp=2_hunter.kill_prob=0.5_s=3 @10093, climate.rain_amp=3_hunter.kill_prob=0.4_s=3 @10581, climate.rain_amp=3_hunter.kill_prob=0.5_s=3 @9700, climate.rain_amp=4_hunter.kill_prob=0.4_s=3 @10036, climate.rain_amp=5_hunter.kill_prob=0.4_s=1 @12760, climate.rain_amp=5_hunter.kill_prob=0.4_s=2 @9739, climate.rain_amp=5_hunter.kill_prob=0.4_s=3 @8795, climate.rain_amp=5_hunter.kill_prob=0.5_s=1 @8259, climate.rain_amp=5_hunter.kill_prob=0.5_s=2 @10859, climate.rain_amp=5_hunter.kill_prob=0.5_s=3 @7815, climate.rain_amp=6_hunter.kill_prob=0.3_s=3 @9461, climate.rain_amp=6_hunter.kill_prob=0.4_s=1 @10798, climate.rain_amp=6_hunter.kill_prob=0.4_s=2 @9967, climate.rain_amp=6_hunter.kill_prob=0.4_s=3 @8902, climate.rain_amp=6_hunter.kill_prob=0.5_s=1 @15445, climate.rain_amp=6_hunter.kill_prob=0.5_s=2 @7024, climate.rain_amp=6_hunter.kill_prob=0.5_s=3 @8404, climate.rain_amp=7_hunter.kill_prob=0.3_s=3 @11912, climate.rain_amp=7_hunter.kill_prob=0.4_s=1 @7169, climate.rain_amp=7_hunter.kill_prob=0.4_s=2 @8942, climate.rain_amp=7_hunter.kill_prob=0.4_s=3 @7289, climate.rain_amp=7_hunter.kill_prob=0.5_s=1 @9571, climate.rain_amp=7_hunter.kill_prob=0.5_s=3 @7080, climate.rain_amp=8_hunter.kill_prob=0.3_s=2 @14574, climate.rain_amp=8_hunter.kill_prob=0.3_s=3 @10977, climate.rain_amp=8_hunter.kill_prob=0.4_s=1 @16713, climate.rain_amp=8_hunter.kill_prob=0.4_s=2 @8406, climate.rain_amp=8_hunter.kill_prob=0.4_s=3 @9346, climate.rain_amp=8_hunter.kill_prob=0.5_s=1 @7301, climate.rain_amp=8_hunter.kill_prob=0.5_s=3 @7491 |
| hunters | `starved` | 10 | climate.rain_amp=0_hunter.kill_prob=0.1_s=1 @16923, climate.rain_amp=0_hunter.kill_prob=0.1_s=2 @18344, climate.rain_amp=0_hunter.kill_prob=0.1_s=3 @18332, climate.rain_amp=1_hunter.kill_prob=0.1_s=1 @16923, climate.rain_amp=1_hunter.kill_prob=0.1_s=2 @18344, climate.rain_amp=1_hunter.kill_prob=0.1_s=3 @18332, climate.rain_amp=2_hunter.kill_prob=0.1_s=1 @16923, climate.rain_amp=2_hunter.kill_prob=0.1_s=3 @18211, climate.rain_amp=3_hunter.kill_prob=0.1_s=1 @15001, climate.rain_amp=3_hunter.kill_prob=0.1_s=2 @19371 |
| trees | `unrecorded` | 6 | climate.rain_amp=6_hunter.kill_prob=0.2_s=2 @1550, climate.rain_amp=6_hunter.kill_prob=0.3_s=2 @1600, climate.rain_amp=7_hunter.kill_prob=0.2_s=2 @1450, climate.rain_amp=7_hunter.kill_prob=0.3_s=2 @1400, climate.rain_amp=7_hunter.kill_prob=0.5_s=2 @1500, climate.rain_amp=8_hunter.kill_prob=0.5_s=2 @1350 |
