# Sweep `disease_grazer_threshold`

- params: `params.toml`
- `disease.grazer_threshold`: 4, 8, 12, 16, 20, 24, 28, 32
- seeds: 1, 2, 3; ticks: 20000; cells: 24; jobs: 22
- wall time: 7.3 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `disease.grazer_threshold`

- default: 16
- safe band: **[12, 32]** (6 of 8 values)
- lower edge: at 8 first fails `fertility_band` (seed 1; worst margin -0.011)
- upper edge: passes to the end of the grid

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 4 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.035); `no_extinction` (seeds 2, 3; worst margin -1.000); `grazer_cycle` (seeds 2, 3; worst margin -1.000); `animals_10k` (seeds 2, 3; worst margin -1.000) |
| 8 | 2/3 | `fertility_band` (seed 1; worst margin -0.011) |
| 12 | 3/3 | — |
| 16 (default) | 3/3 | — |
| 20 | 3/3 | — |
| 24 | 3/3 | — |
| 28 | 3/3 | — |
| 32 | 3/3 | — |

## Extinctions by cause

- cells failing an invariant: 4 of 24
- cells in which a species reaches 0 at any tick: 2 of 24
- failing cells with no extinction: 2; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 2 | disease.grazer_threshold=4_s=2 @7866, disease.grazer_threshold=4_s=3 @7032 |
