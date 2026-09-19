# Sweep `season_amplitude_no_rain_season`

- params: `params.toml`
- fixed overrides: `climate.rain_amp=0`
- `season.amplitude`: 0, 3, 6, 9, 12
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 14.6 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `season.amplitude`

- default: 12.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 | 0/3 | `grass_band` (seeds 1, 2, 3; worst margin -0.280); `fertility_band` (seeds 1, 2, 3; worst margin -0.022); `no_extinction` (seeds 1, 3; worst margin -1.000) |
| 3 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.014); `grass_band` (seed 2; worst margin -0.018) |
| 6 | 0/3 | `grass_band` (seeds 1, 3; worst margin -0.352); `fertility_band` (seeds 1, 2; worst margin -0.009); `no_extinction` (seed 3; worst margin -1.000) |
| 9 | 0/3 | `grass_band` (seeds 1, 2, 3; worst margin -0.562) |
| 12 (default) | 0/3 | `grass_band` (seeds 1, 2, 3; worst margin -0.630); `no_extinction` (seed 2; worst margin -1.000) |

