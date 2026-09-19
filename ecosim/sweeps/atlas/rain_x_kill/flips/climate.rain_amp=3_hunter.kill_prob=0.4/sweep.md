# Sweep `climate.rain_amp=3_hunter.kill_prob=0.4`

- params: `params.toml`
- fixed overrides: `climate.rain_amp=3`, `hunter.kill_prob=0.4`
- `rng.stream`: 1, 2, 3, 4, 5
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 392.0 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `rng.stream`

- default: 0
- safe band: **[1, 3]** (3 of 5 values)
- lower edge: passes to the end of the grid
- upper edge: at 4 first fails `no_extinction` (seed 3; worst margin -1.000)
  - also failing there: `animals_10k` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -0.414); `fertility_band` (seed 3; worst margin -0.033)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 1 | 3/3 | — |
| 2 | 3/3 | — |
| 3 | 3/3 | — |
| 4 | 2/3 | `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -0.414); `fertility_band` (seed 3; worst margin -0.033) |
| 5 | 1/3 | `no_extinction` (seeds 2, 3; worst margin -1.000); `fertility_band` (seeds 2, 3; worst margin -0.028) |

## Extinctions by cause

- cells failing an invariant: 3 of 15
- cells in which a species reaches 0 at any tick: 3 of 15
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 3 | rng.stream=4_s=3 @7040, rng.stream=5_s=2 @11769, rng.stream=5_s=3 @10753 |
