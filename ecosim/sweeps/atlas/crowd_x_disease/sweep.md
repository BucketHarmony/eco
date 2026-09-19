# Sweep `crowd_x_disease`

- params: `params.toml`
- `tree.crowding_mortality`: 0.00, 0.01, 0.02, 0.03, 0.04, 0.05
- `disease.grazer_rate`: 0.000, 0.002, 0.004, 0.006, 0.008, 0.010
- seeds: 1, 2, 3; ticks: 20000; cells: 108; jobs: 22
- wall time: 142.2 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `tree.crowding_mortality`

- default: 0.02
- safe band: **[0.00, 0.00]** (1 of 6 values) — **fragile**
- lower edge: passes to the end of the grid
- upper edge: at 0.01 first fails `fertility_band` (seeds 1, 3; worst margin -0.006)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.00 | 18/18 | — |
| 0.01 | 16/18 | `fertility_band` (seeds 1, 3; worst margin -0.006) |
| 0.02 (default) | 12/18 | `fertility_band` (seeds 1, 2, 3; worst margin -0.025); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.03 | 10/18 | `fertility_band` (seeds 1, 2, 3; worst margin -0.023) |
| 0.04 | 10/18 | `fertility_band` (seeds 1, 2, 3; worst margin -0.044); `no_extinction` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.05 | 7/18 | `fertility_band` (seeds 1, 2, 3; worst margin -0.033); `no_extinction` (seed 3; worst margin -1.000) |

## `disease.grazer_rate`

- default: 0.001
- safe band: **[0.000, 0.002]** (2 of 6 values) — **fragile**
- lower edge: passes to the end of the grid
- upper edge: at 0.004 first fails `fertility_band` (seeds 2, 3; worst margin -0.033)
  - also failing there: `no_extinction` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.000 | 18/18 | — |
| 0.002 | 18/18 | — |
| 0.004 | 14/18 | `fertility_band` (seeds 2, 3; worst margin -0.033); `no_extinction` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.006 | 10/18 | `fertility_band` (seeds 1, 2, 3; worst margin -0.044); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.008 | 6/18 | `fertility_band` (seeds 1, 2, 3; worst margin -0.025); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.010 | 7/18 | `fertility_band` (seeds 1, 2, 3; worst margin -0.035) |

## Matrix: seeds passing out of 3

| `tree.crowding_mortality` \ `disease.grazer_rate` | 0.000 | 0.002 | 0.004 | 0.006 | 0.008 | 0.010 |
|---|---|---|---|---|---|---|
| 0.00 | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 |
| 0.01 | 3/3 | 3/3 | 3/3 | 3/3 | 2/3 | 2/3 |
| 0.02 | 3/3 | 3/3 | 3/3 | 2/3 | 0/3 | 1/3 |
| 0.03 | 3/3 | 3/3 | 2/3 | 1/3 | 1/3 | 0/3 |
| 0.04 | 3/3 | 3/3 | 2/3 | 1/3 | 0/3 | 1/3 |
| 0.05 | 3/3 | 3/3 | 1/3 | 0/3 | 0/3 | 0/3 |

## Extinctions by cause

- cells failing an invariant: 35 of 108
- cells in which a species reaches 0 at any tick: 4 of 108
- failing cells with no extinction: 31; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 4 | tree.crowding_mortality=0.02_disease.grazer_rate=0.008_s=3 @9805, tree.crowding_mortality=0.04_disease.grazer_rate=0.004_s=3 @8088, tree.crowding_mortality=0.04_disease.grazer_rate=0.006_s=3 @8654, tree.crowding_mortality=0.05_disease.grazer_rate=0.006_s=3 @13290 |
