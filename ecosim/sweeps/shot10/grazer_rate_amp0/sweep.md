# Sweep `grazer_rate_amp0`

- params: `params.toml`
- fixed overrides: `season.amplitude=0`
- `disease.grazer_rate`: 0.000, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006, 0.007, 0.008, 0.009, 0.010
- seeds: 1, 2, 3; ticks: 20000; cells: 33; jobs: 22
- wall time: 8.2 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `disease.grazer_rate`

- default: 0.001
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.000 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.143); `no_extinction` (seed 3; worst margin -1.000) |
| 0.001 (default) | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.135) |
| 0.002 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.136) |
| 0.003 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.156); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.004 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.148) |
| 0.005 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.154); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.006 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.154); `no_extinction` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000) |
| 0.007 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.149) |
| 0.008 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.148) |
| 0.009 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.155); `no_extinction` (seeds 1, 3; worst margin -1.000); `animals_10k` (seeds 1, 3; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000) |
| 0.010 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.155); `no_extinction` (seed 3; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 33 of 33
- cells in which a species reaches 0 at any tick: 7 of 33
- failing cells with no extinction: 26; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 7 | disease.grazer_rate=0.000_s=3 @12059, disease.grazer_rate=0.003_s=3 @8325, disease.grazer_rate=0.005_s=3 @7964, disease.grazer_rate=0.006_s=1 @8329, disease.grazer_rate=0.009_s=1 @8164, disease.grazer_rate=0.009_s=3 @8692, disease.grazer_rate=0.010_s=3 @12966 |
