# Sweep `hunter_refugium_k`

- params: `params.toml`
- `hunter.refugium_k`: 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0
- seeds: 1, 2, 3; ticks: 20000; cells: 24; jobs: 22
- wall time: 40.2 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.refugium_k`

- default: 2.0
- safe band: **[0.5, 3.0]** (6 of 8 values)
- lower edge: passes to the end of the grid
- upper edge: at 3.5 first fails `no_extinction` (seeds 1, 3; worst margin -1.000)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.5 | 3/3 | — |
| 1.0 | 3/3 | — |
| 1.5 | 3/3 | — |
| 2.0 (default) | 3/3 | — |
| 2.5 | 3/3 | — |
| 3.0 | 3/3 | — |
| 3.5 | 1/3 | `no_extinction` (seeds 1, 3; worst margin -1.000) |
| 4.0 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000) |

