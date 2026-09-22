# Sweep `strip`

- params: `params.toml`
- `npk.n_deposition`: 0, 0.5, 1, 2, 4
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 6
- wall time: 225.5 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `moisture_band`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k`, `tree_footing` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `npk.n_deposition`

- default: 2.5
- safe band: **[1, 4]** (3 of 5 values)
- lower edge: at 0.5 first fails `fertility_band` (seed 3; worst margin -0.032)
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.273) |
| 0.5 | 2/3 | `fertility_band` (seed 3; worst margin -0.032) |
| 1 | 3/3 | — |
| 2 | 3/3 | — |
| 4 | 3/3 | — |

## Extinctions by cause

- cells failing an invariant: 4 of 15
- cells in which a species reaches 0 at any tick: 0 of 15
- failing cells with no extinction: 4; extinction cells passing every invariant: 0

