# Sweep `fire_spread`

- params: `params.toml`
- `fire.spread`: 0.0, 0.1, 0.2, 0.3, 0.4, 0.5
- seeds: 1, 2, 3; ticks: 20000; cells: 18; jobs: 22
- wall time: 30.3 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `fire.spread`

- default: 0.1
- safe band: **[0.0, 0.1]** (2 of 6 values) — **fragile**
- lower edge: passes to the end of the grid
- upper edge: at 0.2 first fails `grass_band` (seed 3; worst margin -0.122)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.0 | 3/3 | — |
| 0.1 (default) | 3/3 | — |
| 0.2 | 2/3 | `grass_band` (seed 3; worst margin -0.122) |
| 0.3 | 1/3 | `grass_band` (seed 1; worst margin -0.082); `fertility_band` (seed 2; worst margin -0.001) |
| 0.4 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.093); `mature_trees_10k` (seeds 1, 2; worst margin -0.914); `grass_band` (seed 1; worst margin -0.474) |
| 0.5 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `tree_growth` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.158); `mature_trees_10k` (seeds 1, 2; worst margin -1.000); `grass_band` (seeds 1, 3; worst margin -0.758); `animals_10k` (seed 1; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 9 of 18
- cells in which a species reaches 0 at any tick: 3 of 18
- failing cells with no extinction: 6; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| trees | `unrecorded` | 3 | fire.spread=0.5_s=1 @9306, fire.spread=0.5_s=2 @17681, fire.spread=0.5_s=3 @13511 |
