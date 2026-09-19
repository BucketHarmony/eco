# Sweep `hunter_kill_energy`

- params: `params.toml`
- fixed overrides: `disease.hunter_rate=0`, `hunter.hunt_cost=1.0`
- `hunter.kill_energy`: 20, 30, 40, 50, 60, 70, 80
- seeds: 1, 2, 3; ticks: 20000; cells: 21; jobs: 22
- wall time: 8.6 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.kill_energy`

- default: 40.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 20 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000); `fertility_band` (seed 1; worst margin -0.006) |
| 30 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -0.218); `fertility_band` (seed 1; worst margin -0.024) |
| 40 (default) | 1/3 | `no_extinction` (seeds 1, 3; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000) |
| 50 | 1/3 | `no_extinction` (seeds 1, 2; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -0.535) |
| 60 | 2/3 | `no_extinction` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000); `fertility_band` (seed 1; worst margin -0.021) |
| 70 | 1/3 | `no_extinction` (seeds 1, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 3; worst margin -1.000); `animals_10k` (seeds 1, 3; worst margin -1.000); `fertility_band` (seeds 1, 3; worst margin -0.021) |
| 80 | 2/3 | `no_extinction` (seed 1; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000); `fertility_band` (seed 1; worst margin -0.019) |

## Extinctions by cause

- cells failing an invariant: 14 of 21
- cells in which a species reaches 0 at any tick: 14 of 21
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 8 | hunter.kill_energy=20_s=1 @6773, hunter.kill_energy=30_s=1 @6967, hunter.kill_energy=40_s=1 @8604, hunter.kill_energy=50_s=1 @6845, hunter.kill_energy=60_s=1 @7347, hunter.kill_energy=70_s=1 @7163, hunter.kill_energy=70_s=3 @6876, hunter.kill_energy=80_s=1 @7927 |
| hunters | `starved` | 6 | hunter.kill_energy=20_s=2 @8326, hunter.kill_energy=20_s=3 @8082, hunter.kill_energy=30_s=2 @14537, hunter.kill_energy=30_s=3 @14355, hunter.kill_energy=40_s=3 @16319, hunter.kill_energy=50_s=2 @17889 |
