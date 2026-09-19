# Sweep `climate.rain_amp=8_hunter.kill_prob=0.3`

- params: `params.toml`
- fixed overrides: `climate.rain_amp=8`, `hunter.kill_prob=0.3`
- `rng.stream`: 1, 2, 3, 4, 5
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 12.8 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rng.stream`

- default: 0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 1 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.914); `grazer_cycle` (seeds 1, 3; worst margin -1.000); `animals_10k` (seeds 1, 3; worst margin -1.000) |
| 2 | 0/3 | `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.943); `no_extinction` (seeds 1, 3; worst margin -1.000) |
| 3 | 0/3 | `mature_trees_10k` (seeds 1, 2, 3; worst margin -0.771); `no_extinction` (seeds 2, 3; worst margin -1.000) |
| 4 | 0/3 | `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `no_extinction` (seeds 2, 3; worst margin -1.000); `tree_growth` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 5 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `mature_trees_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 2, 3; worst margin -1.000); `animals_10k` (seeds 2, 3; worst margin -1.000); `tree_growth` (seed 1; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 15 of 15
- cells in which a species reaches 0 at any tick: 12 of 15
- failing cells with no extinction: 3; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 10 | rng.stream=1_s=1 @8545, rng.stream=1_s=2 @10568, rng.stream=1_s=3 @7924, rng.stream=2_s=1 @14079, rng.stream=2_s=3 @10562, rng.stream=3_s=2 @11373, rng.stream=3_s=3 @12134, rng.stream=4_s=2 @13911, rng.stream=5_s=2 @7818, rng.stream=5_s=3 @7416 |
| trees | `unrecorded` | 2 | rng.stream=4_s=3 @1450, rng.stream=5_s=1 @1250 |
