# Sweep `hunter_hunt_cost`

- params: `params.toml`
- fixed overrides: `disease.hunter_rate=0`
- `hunter.hunt_cost`: 0, 1, 2, 3, 4, 5
- seeds: 1, 2, 3; ticks: 20000; cells: 18; jobs: 22
- wall time: 8.4 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.hunt_cost`

- default: 0.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 (default) | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 3; worst margin -1.000); `fertility_band` (seeds 1, 3; worst margin -0.037) |
| 1 | 1/3 | `no_extinction` (seeds 1, 3; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000) |
| 2 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000); `fertility_band` (seed 1; worst margin -0.027) |
| 3 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000) |
| 4 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000) |
| 5 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 17 of 18
- cells in which a species reaches 0 at any tick: 17 of 18
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| hunters | `starved` | 12 | hunter.hunt_cost=1_s=3 @16319, hunter.hunt_cost=2_s=2 @7541, hunter.hunt_cost=2_s=3 @9349, hunter.hunt_cost=3_s=1 @10002, hunter.hunt_cost=3_s=2 @8214, hunter.hunt_cost=3_s=3 @8407, hunter.hunt_cost=4_s=1 @8606, hunter.hunt_cost=4_s=2 @6271, hunter.hunt_cost=4_s=3 @4955, hunter.hunt_cost=5_s=1 @9828, hunter.hunt_cost=5_s=2 @5086, hunter.hunt_cost=5_s=3 @5778 |
| grazers | `eaten` | 5 | hunter.hunt_cost=0_s=1 @7283, hunter.hunt_cost=0_s=2 @7198, hunter.hunt_cost=0_s=3 @7330, hunter.hunt_cost=1_s=1 @8604, hunter.hunt_cost=2_s=1 @7105 |
