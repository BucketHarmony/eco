# Sweep `refractory`

- params: `params.toml`
- fixed overrides: `hunter.handling_ticks=200`, `hunter.hunt_cost=1.0`, `disease.hunter_rate=0`, `hunter.kill_energy=60`
- `hunter.refractory`: 300, 600, 900, 1200, 1500
- seeds: 1, 2, 3; ticks: 60000; cells: 15; jobs: 24
- wall time: 4.3 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.refractory`

- default: 2750
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 300 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159) |
| 600 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159) |
| 900 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159) |
| 1200 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159) |
| 1500 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.159) |

## Extinctions by cause

- cells failing an invariant: 15 of 15
- cells in which a species reaches 0 at any tick: 15 of 15
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 15 | hunter.refractory=300_s=1 @1450, hunter.refractory=300_s=2 @1424, hunter.refractory=300_s=3 @1430, hunter.refractory=600_s=1 @2675, hunter.refractory=600_s=2 @2585, hunter.refractory=600_s=3 @2720, hunter.refractory=900_s=1 @3834, hunter.refractory=900_s=2 @3963, hunter.refractory=900_s=3 @3732, hunter.refractory=1200_s=1 @4921, hunter.refractory=1200_s=2 @5196, hunter.refractory=1200_s=3 @5114, hunter.refractory=1500_s=1 @6287, hunter.refractory=1500_s=2 @7065, hunter.refractory=1500_s=3 @6479 |
