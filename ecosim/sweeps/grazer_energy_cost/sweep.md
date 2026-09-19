# Sweep `grazer_energy_cost`

- params: `params.toml`
- `grazer.energy_cost`: 0.04, 0.06, 0.08, 0.10, 0.12, 0.14, 0.16
- seeds: 1, 2, 3; ticks: 20000; cells: 21; jobs: 8
- wall time: 43.8 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `grazer.energy_cost`

- default: 0.08
- safe band: **[0.04, 0.08]** (3 of 7 values)
- other safe runs: [0.12, 0.12], [0.16, 0.16]
- lower edge: passes to the end of the grid
- upper edge: at 0.10 first fails `no_extinction` (seed 1; worst margin -1.000)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.04 | 3/3 | — |
| 0.06 | 3/3 | — |
| 0.08 (default) | 3/3 | — |
| 0.10 | 2/3 | `no_extinction` (seed 1; worst margin -1.000) |
| 0.12 | 3/3 | — |
| 0.14 | 2/3 | `no_extinction` (seed 3; worst margin -1.000) |
| 0.16 | 3/3 | — |

