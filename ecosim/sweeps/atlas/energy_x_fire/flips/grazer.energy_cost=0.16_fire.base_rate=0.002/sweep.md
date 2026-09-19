# Sweep `grazer.energy_cost=0.16_fire.base_rate=0.002`

- params: `params.toml`
- fixed overrides: `grazer.energy_cost=0.16`, `fire.base_rate=0.002`
- `rng.stream`: 1, 2, 3, 4, 5
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 137.6 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rng.stream`

- default: 0
- safe band: **[1, 5]** (5 of 5 values)
- lower edge: passes to the end of the grid
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 1 | 3/3 | — |
| 2 | 3/3 | — |
| 3 | 3/3 | — |
| 4 | 3/3 | — |
| 5 | 3/3 | — |

## Extinctions by cause

- cells failing an invariant: 0 of 15
- cells in which a species reaches 0 at any tick: 0 of 15
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

