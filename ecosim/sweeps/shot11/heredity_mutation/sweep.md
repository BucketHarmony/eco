# Sweep `heredity_mutation`

- params: `params.toml`
- `heredity.mutation`: 0.000, 0.025, 0.050, 0.075, 0.100, 0.125, 0.150, 0.175, 0.200
- seeds: 1, 2, 3; ticks: 20000; cells: 27; jobs: 22
- wall time: 16.5 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `heredity.mutation`

- default: 0.05
- safe band: **[0.000, 0.200]** (9 of 9 values)
- lower edge: passes to the end of the grid
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.000 | 3/3 | — |
| 0.025 | 3/3 | — |
| 0.050 (default) | 3/3 | — |
| 0.075 | 3/3 | — |
| 0.100 | 3/3 | — |
| 0.125 | 3/3 | — |
| 0.150 | 3/3 | — |
| 0.175 | 3/3 | — |
| 0.200 | 3/3 | — |

## Extinctions by cause

- cells failing an invariant: 0 of 27
- cells in which a species reaches 0 at any tick: 0 of 27
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

