# Sweep `climate.rain_amp=3_hunter.kill_prob=0.1`

- params: `params.toml`
- fixed overrides: `climate.rain_amp=3`, `hunter.kill_prob=0.1`
- `rng.stream`: 1, 2, 3, 4, 5
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 403.7 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rng.stream`

- default: 0
- safe band: **[5, 5]** (1 of 5 values) — **fragile**
- lower edge: at 4 first fails `fertility_band` (seed 3; worst margin -0.001)
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 1 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000) |
| 2 | 1/3 | `no_extinction` (seeds 1, 3; worst margin -1.000) |
| 3 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000) |
| 4 | 2/3 | `fertility_band` (seed 3; worst margin -0.001) |
| 5 | 3/3 | — |

## Extinctions by cause

- cells failing an invariant: 9 of 15
- cells in which a species reaches 0 at any tick: 8 of 15
- failing cells with no extinction: 1; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| hunters | `starved` | 7 | rng.stream=1_s=1 @18643, rng.stream=1_s=2 @19131, rng.stream=1_s=3 @18601, rng.stream=2_s=1 @15151, rng.stream=2_s=3 @18085, rng.stream=3_s=1 @18211, rng.stream=3_s=2 @16516 |
| hunters | `old_age` | 1 | rng.stream=3_s=3 @19757 |
