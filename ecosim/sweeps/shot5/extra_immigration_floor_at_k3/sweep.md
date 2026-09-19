# Sweep `floor_k3`

- params: `params.toml`
- fixed overrides: `hunter.refugium_k=3.0`
- `hunter.immigration_floor`: 0, 4, 8, 12, 16
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 22
- wall time: 12.4 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.immigration_floor`

- default: 8
- safe band: **[4, 16]** (4 of 5 values)
- lower edge: at 0 first fails `no_extinction` (seeds 2, 3; worst margin -1.000)
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 | 1/3 | `no_extinction` (seeds 2, 3; worst margin -1.000) |
| 4 | 3/3 | — |
| 8 (default) | 3/3 | — |
| 12 | 3/3 | — |
| 16 | 3/3 | — |

