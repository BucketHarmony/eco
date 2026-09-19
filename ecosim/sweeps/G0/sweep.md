# Sweep `G0`

- params: `params.toml`
- `animals.enabled`: true, false
- seeds: 1, 2, 3; ticks: 20000; cells: 6; jobs: 6
- wall time: 46.6 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `animals.enabled`

- default: true
- safe band: **[true, false]** (2 of 2 values) — **fragile**
- lower edge: passes to the end of the grid
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| true (default) | 3/3 | — |
| false | 3/3 | — |

## Extinctions by cause

- cells failing an invariant: 0 of 6
- cells in which a species reaches 0 at any tick: 0 of 6
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

