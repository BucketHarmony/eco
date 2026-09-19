# Sweep `climate.rain_amp=7_hunter.kill_prob=0.3`

- params: `params.toml`
- fixed overrides: `climate.rain_amp=7`, `hunter.kill_prob=0.3`
- `rng.stream`: 1, 2, 3, 4, 5
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 11.6 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rng.stream`

- default: 0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 1 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.943); `animals_10k` (seeds 1, 2; worst margin -1.000) |
| 2 | 0/3 | `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.857); `no_extinction` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 3 | 0/3 | `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.686); `no_extinction` (seeds 2, 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 4 | 0/3 | `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `no_extinction` (seeds 2, 3; worst margin -1.000); `tree_growth` (seed 2; worst margin -1.000) |
| 5 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `tree_growth` (seed 1; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 15 of 15
- cells in which a species reaches 0 at any tick: 11 of 15
- failing cells with no extinction: 4; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 9 | rng.stream=1_s=1 @10017, rng.stream=1_s=2 @9135, rng.stream=1_s=3 @11101, rng.stream=2_s=3 @6796, rng.stream=3_s=2 @12843, rng.stream=3_s=3 @9952, rng.stream=4_s=3 @15702, rng.stream=5_s=2 @11110, rng.stream=5_s=3 @7561 |
| trees | `unrecorded` | 2 | rng.stream=4_s=2 @1650, rng.stream=5_s=1 @1400 |
