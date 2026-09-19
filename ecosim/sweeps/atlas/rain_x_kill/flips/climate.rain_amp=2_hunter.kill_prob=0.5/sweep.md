# Sweep `climate.rain_amp=2_hunter.kill_prob=0.5`

- params: `params.toml`
- fixed overrides: `climate.rain_amp=2`, `hunter.kill_prob=0.5`
- `rng.stream`: 1, 2, 3, 4, 5
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 395.4 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rng.stream`

- default: 0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 1 | 1/3 | `fertility_band` (seeds 1, 3; worst margin -0.050); `no_extinction` (seed 3; worst margin -1.000) |
| 2 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.049); `no_extinction` (seed 1; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000) |
| 3 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.009) |
| 4 | 1/3 | `fertility_band` (seeds 2, 3; worst margin -0.063); `no_extinction` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 5 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.052); `no_extinction` (seeds 2, 3; worst margin -1.000); `grazer_cycle` (seed 2; worst margin -1.000); `animals_10k` (seed 2; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 13 of 15
- cells in which a species reaches 0 at any tick: 5 of 15
- failing cells with no extinction: 8; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 5 | rng.stream=1_s=3 @11697, rng.stream=2_s=1 @9814, rng.stream=4_s=3 @6714, rng.stream=5_s=2 @8854, rng.stream=5_s=3 @11971 |
