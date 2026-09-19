# Sweep `season_amplitude`

- params: `params.toml`
- `season.amplitude`: 0, 3, 6, 9, 12, 15, 18
- seeds: 1, 2, 3; ticks: 20000; cells: 21; jobs: 22
- wall time: 28.6 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `season.amplitude`

- default: 15.0
- safe band: **[12, 18]** (3 of 7 values)
- lower edge: at 9 first fails `fertility_band` (seeds 1, 2, 3; worst margin -0.026)
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.080) |
| 3 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.057) |
| 6 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.059) |
| 9 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.026) |
| 12 | 3/3 | — |
| 15 (default) | 3/3 | — |
| 18 | 3/3 | — |

