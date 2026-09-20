# Sweep `mean5`

- params: `params.toml`
- fixed overrides: `rain.storm_p=0.2`
- `rain.storm_mean_mm`: 5
- seeds: 1, 2, 3; ticks: 20000; cells: 3; jobs: 6
- wall time: 53.1 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rain.storm_mean_mm`

- default: 10.0
- safe band: **[5, 5]** (1 of 1 values) — **fragile**
- lower edge: passes to the end of the grid
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 5 | 3/3 | — |

## Extinctions by cause

- cells failing an invariant: 0 of 3
- cells in which a species reaches 0 at any tick: 0 of 3
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

