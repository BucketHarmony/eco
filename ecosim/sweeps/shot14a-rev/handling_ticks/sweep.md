# Sweep `handling_ticks`

- params: `params.toml`
- fixed overrides: `hunter.hunt_cost=1.0`, `disease.hunter_rate=0`, `hunter.kill_energy=60`
- `hunter.handling_ticks`: 0, 25, 50, 75, 100, 125, 150, 175, 200
- seeds: 1, 2, 3; ticks: 60000; cells: 27; jobs: 24
- wall time: 107.0 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.handling_ticks`

- default: 0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 (default) | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159); `animals_10k` (seed 1; worst margin -1.000) |
| 25 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159); `grazer_cycle` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000) |
| 50 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159); `animals_10k` (seed 1; worst margin -1.000) |
| 75 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159) |
| 100 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159) |
| 125 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159); `animals_10k` (seed 1; worst margin -0.800) |
| 150 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159) |
| 175 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159); `animals_10k` (seed 1; worst margin -1.000) |
| 200 | 0/3 | `fertility_band` (seeds 1, 2, 3; worst margin -0.159); `no_extinction` (seeds 2, 3; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 27 of 27
- cells in which a species reaches 0 at any tick: 26 of 27
- failing cells with no extinction: 1; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| hunters | `starved` | 18 | hunter.handling_ticks=0_s=2 @32725, hunter.handling_ticks=0_s=3 @20779, hunter.handling_ticks=25_s=2 @20825, hunter.handling_ticks=25_s=3 @20985, hunter.handling_ticks=50_s=2 @20691, hunter.handling_ticks=50_s=3 @21498, hunter.handling_ticks=75_s=1 @25533, hunter.handling_ticks=75_s=2 @19535, hunter.handling_ticks=100_s=2 @41484, hunter.handling_ticks=100_s=3 @25585, hunter.handling_ticks=125_s=2 @33661, hunter.handling_ticks=125_s=3 @24796, hunter.handling_ticks=150_s=2 @18255, hunter.handling_ticks=150_s=3 @21348, hunter.handling_ticks=175_s=2 @41846, hunter.handling_ticks=175_s=3 @24032, hunter.handling_ticks=200_s=2 @29975, hunter.handling_ticks=200_s=3 @27100 |
| grazers | `eaten` | 5 | hunter.handling_ticks=0_s=1 @7347, hunter.handling_ticks=25_s=1 @8339, hunter.handling_ticks=50_s=1 @9497, hunter.handling_ticks=125_s=1 @10089, hunter.handling_ticks=175_s=1 @9719 |
| hunters | `old_age` | 3 | hunter.handling_ticks=75_s=3 @23837, hunter.handling_ticks=100_s=1 @31345, hunter.handling_ticks=150_s=1 @27934 |
