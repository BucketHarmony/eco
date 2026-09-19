# Sweep `energy_x_fire`

- params: `params.toml`
- `grazer.energy_cost`: 0.04, 0.06, 0.08, 0.10, 0.12, 0.14, 0.16
- `fire.base_rate`: 0.000, 0.001, 0.002, 0.003, 0.004
- seeds: 1, 2, 3; ticks: 20000; cells: 105; jobs: 22
- wall time: 86.2 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `grazer.energy_cost`

- default: 0.1
- safe band: **[0.10, 0.14]** (3 of 7 values)
- lower edge: at 0.08 first fails `no_extinction` (seed 3; worst margin -1.000)
  - also failing there: `animals_10k` (seed 3; worst margin -1.000); `fertility_band` (seed 3; worst margin -0.017)
- upper edge: at 0.16 first fails `no_extinction` (seed 3; worst margin -1.000)
  - also failing there: `animals_10k` (seed 3; worst margin -1.000)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.04 | 8/15 | `fertility_band` (seeds 1, 2, 3; worst margin -0.025) |
| 0.06 | 13/15 | `fertility_band` (seeds 1, 2; worst margin -0.013) |
| 0.08 | 14/15 | `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000); `fertility_band` (seed 3; worst margin -0.017) |
| 0.10 (default) | 15/15 | — |
| 0.12 | 15/15 | — |
| 0.14 | 15/15 | — |
| 0.16 | 12/15 | `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |

## `fire.base_rate`

- default: 0.002
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.000 | 16/21 | `fertility_band` (seeds 1, 2, 3; worst margin -0.025) |
| 0.001 | 17/21 | `fertility_band` (seeds 1, 2, 3; worst margin -0.017); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.002 (default) | 19/21 | `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000); `fertility_band` (seed 2; worst margin -0.000) |
| 0.003 | 20/21 | `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.004 | 20/21 | `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |

## Matrix: seeds passing out of 3

| `grazer.energy_cost` \ `fire.base_rate` | 0.000 | 0.001 | 0.002 | 0.003 | 0.004 |
|---|---|---|---|---|---|
| 0.04 | 0/3 | 0/3 | 2/3 | 3/3 | 3/3 |
| 0.06 | 1/3 | 3/3 | 3/3 | 3/3 | 3/3 |
| 0.08 | 3/3 | 2/3 | 3/3 | 3/3 | 3/3 |
| 0.10 | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 |
| 0.12 | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 |
| 0.14 | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 |
| 0.16 | 3/3 | 3/3 | 2/3 | 2/3 | 2/3 |

## Extinctions by cause

- cells failing an invariant: 13 of 105
- cells in which a species reaches 0 at any tick: 4 of 105
- failing cells with no extinction: 9; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 4 | grazer.energy_cost=0.08_fire.base_rate=0.001_s=3 @9580, grazer.energy_cost=0.16_fire.base_rate=0.002_s=3 @9906, grazer.energy_cost=0.16_fire.base_rate=0.003_s=3 @8793, grazer.energy_cost=0.16_fire.base_rate=0.004_s=3 @8793 |
