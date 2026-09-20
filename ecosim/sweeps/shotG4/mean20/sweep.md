# Sweep `mean20`

- params: `params.toml`
- fixed overrides: `rain.storm_p=0.05`
- `rain.storm_mean_mm`: 20
- seeds: 1, 2, 3; ticks: 20000; cells: 3; jobs: 6
- wall time: 51.3 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rain.storm_mean_mm`

- default: 10.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 20 | 2/3 | `no_extinction` (seed 1; worst margin -1.000); `tree_growth` (seed 1; worst margin -1.000); `mature_trees_10k` (seed 1; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 1 of 3
- cells in which a species reaches 0 at any tick: 1 of 3
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| trees | `unrecorded` | 1 | rain.storm_mean_mm=20_s=1 @1450 |
