# Sweep `fire_base_rate`

- params: `params.toml`
- `fire.base_rate`: 0.0000, 0.0005, 0.0010, 0.0015, 0.0020, 0.0025, 0.0030, 0.0035, 0.0040
- seeds: 1, 2, 3; ticks: 20000; cells: 27; jobs: 22
- wall time: 40.6 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `fire.base_rate`

- default: 0.002
- safe band: **[0.0000, 0.0040]** (9 of 9 values)
- lower edge: passes to the end of the grid
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.0000 | 3/3 | — |
| 0.0005 | 3/3 | — |
| 0.0010 | 3/3 | — |
| 0.0015 | 3/3 | — |
| 0.0020 (default) | 3/3 | — |
| 0.0025 | 3/3 | — |
| 0.0030 | 3/3 | — |
| 0.0035 | 3/3 | — |
| 0.0040 | 3/3 | — |

## Extinctions by cause

- cells failing an invariant: 0 of 27
- cells in which a species reaches 0 at any tick: 0 of 27
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

