# Sweep `season_amplitude`

- params: `params.toml`
- `season.amplitude`: 0, 3, 6, 9, 12
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 16.4 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `season.amplitude`

- default: 12.0
- safe band: **[12, 12]** (1 of 5 values) — **fragile**
- lower edge: at 9 first fails `no_extinction` (seed 2; worst margin -1.000)
  - also failing there: `grass_band` (seed 3; worst margin -0.006)
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.029); `no_extinction` (seeds 1, 3; worst margin -1.000); `grass_band` (seeds 1, 2; worst margin -0.104) |
| 3 | 1/3 | `grass_band` (seeds 2, 3; worst margin -0.530); `fertility_band` (seeds 2, 3; worst margin -0.010) |
| 6 | 1/3 | `no_extinction` (seed 3; worst margin -1.000); `grass_band` (seed 1; worst margin -0.144) |
| 9 | 1/3 | `no_extinction` (seed 2; worst margin -1.000); `grass_band` (seed 3; worst margin -0.006) |
| 12 (default) | 3/3 | — |

