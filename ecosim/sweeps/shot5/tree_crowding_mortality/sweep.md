# Sweep `tree_crowding_mortality`

- params: `params.toml`
- `tree.crowding_mortality`: 0.00, 0.01, 0.02, 0.03, 0.04, 0.05
- seeds: 1, 2, 3; ticks: 20000; cells: 18; jobs: 22
- wall time: 24.6 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `tree.crowding_mortality`

- default: 0.02
- safe band: **[0.00, 0.05]** (6 of 6 values)
- lower edge: passes to the end of the grid
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.00 | 3/3 | — |
| 0.01 | 3/3 | — |
| 0.02 (default) | 3/3 | — |
| 0.03 | 3/3 | — |
| 0.04 | 3/3 | — |
| 0.05 | 3/3 | — |

