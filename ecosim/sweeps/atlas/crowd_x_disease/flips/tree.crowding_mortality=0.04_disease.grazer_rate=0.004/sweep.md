# Sweep `tree.crowding_mortality=0.04_disease.grazer_rate=0.004`

- params: `params.toml`
- fixed overrides: `tree.crowding_mortality=0.04`, `disease.grazer_rate=0.004`
- `rng.stream`: 1, 2, 3, 4, 5
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 944.7 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rng.stream`

- default: 0
- safe band: **[3, 3]** (1 of 5 values) — **fragile**
- lower edge: at 2 first fails `fertility_band` (seed 3; worst margin -0.017)
- upper edge: at 4 first fails `fertility_band` (seeds 2, 3; worst margin -0.024)
  - also failing there: `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -0.100)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 1 | 2/3 | `fertility_band` (seed 2; worst margin -0.006) |
| 2 | 2/3 | `fertility_band` (seed 3; worst margin -0.017) |
| 3 | 3/3 | — |
| 4 | 1/3 | `fertility_band` (seeds 2, 3; worst margin -0.024); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -0.100) |
| 5 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.033); `no_extinction` (seed 2; worst margin -1.000); `animals_10k` (seed 2; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 7 of 15
- cells in which a species reaches 0 at any tick: 2 of 15
- failing cells with no extinction: 5; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 2 | rng.stream=4_s=3 @10191, rng.stream=5_s=2 @7360 |
