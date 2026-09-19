# Sweep `disease_grazer_rate`

- params: `params.toml`
- `disease.grazer_rate`: 0.000, 0.001, 0.002, 0.003, 0.004, 0.005, 0.006, 0.007, 0.008, 0.009, 0.010
- seeds: 1, 2, 3; ticks: 20000; cells: 33; jobs: 22
- wall time: 10.4 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `disease.grazer_rate`

- default: 0.001
- safe band: **[0.001, 0.002]** (2 of 11 values) — **fragile**
- lower edge: at 0.000 first fails `no_extinction` (seed 3; worst margin -1.000)
- upper edge: at 0.003 first fails `fertility_band` (seed 2; worst margin -0.003)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.000 | 2/3 | `no_extinction` (seed 3; worst margin -1.000) |
| 0.001 (default) | 3/3 | — |
| 0.002 | 3/3 | — |
| 0.003 | 2/3 | `fertility_band` (seed 2; worst margin -0.003) |
| 0.004 | 2/3 | `no_extinction` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000); `fertility_band` (seed 3; worst margin -0.008) |
| 0.005 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.020); `no_extinction` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.006 | 1/3 | `fertility_band` (seeds 1, 3; worst margin -0.014); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.007 | 2/3 | `fertility_band` (seed 2; worst margin -0.012) |
| 0.008 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.020); `no_extinction` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |
| 0.009 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.034); `no_extinction` (seed 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000); `grazer_cycle` (seed 3; worst margin -0.333) |
| 0.010 | 1/3 | `fertility_band` (seeds 1, 2; worst margin -0.031) |

## Extinctions by cause

- cells failing an invariant: 17 of 33
- cells in which a species reaches 0 at any tick: 6 of 33
- failing cells with no extinction: 11; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 6 | disease.grazer_rate=0.000_s=3 @11451, disease.grazer_rate=0.004_s=3 @7709, disease.grazer_rate=0.005_s=3 @7795, disease.grazer_rate=0.006_s=3 @8934, disease.grazer_rate=0.008_s=3 @8411, disease.grazer_rate=0.009_s=3 @7958 |
