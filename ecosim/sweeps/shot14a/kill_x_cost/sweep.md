# Sweep `kill_x_cost`

- params: `params.toml`
- fixed overrides: `disease.hunter_rate=0`
- `hunter.kill_energy`: 20, 40, 60, 80
- `hunter.hunt_cost`: 0, 1, 2, 3
- seeds: 1, 2, 3; ticks: 20000; cells: 48; jobs: 22
- wall time: 18.7 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.kill_energy`

- default: 40.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 20 | 0/12 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.038) |
| 40 (default) | 1/12 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 3; worst margin -1.000); `fertility_band` (seeds 1, 3; worst margin -0.037) |
| 60 | 2/12 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.040); `grazer_cycle` (seeds 1, 2; worst margin -0.440) |
| 80 | 2/12 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.020) |

## `hunter.hunt_cost`

- default: 0.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0 (default) | 0/12 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.040) |
| 1 | 5/12 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000); `fertility_band` (seed 1; worst margin -0.021) |
| 2 | 0/12 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -1.000); `fertility_band` (seed 1; worst margin -0.027) |
| 3 | 0/12 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000) |

## Matrix: seeds passing out of 3

| `hunter.kill_energy` \ `hunter.hunt_cost` | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| 20 | 0/3 | 0/3 | 0/3 | 0/3 |
| 40 | 0/3 | 1/3 | 0/3 | 0/3 |
| 60 | 0/3 | 2/3 | 0/3 | 0/3 |
| 80 | 0/3 | 2/3 | 0/3 | 0/3 |

## Extinctions by cause

- cells failing an invariant: 43 of 48
- cells in which a species reaches 0 at any tick: 43 of 48
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| hunters | `starved` | 24 | hunter.kill_energy=20_hunter.hunt_cost=1_s=2 @8326, hunter.kill_energy=20_hunter.hunt_cost=1_s=3 @8082, hunter.kill_energy=20_hunter.hunt_cost=2_s=1 @8723, hunter.kill_energy=20_hunter.hunt_cost=2_s=2 @6348, hunter.kill_energy=20_hunter.hunt_cost=2_s=3 @5361, hunter.kill_energy=20_hunter.hunt_cost=3_s=1 @7369, hunter.kill_energy=20_hunter.hunt_cost=3_s=2 @5496, hunter.kill_energy=20_hunter.hunt_cost=3_s=3 @5202, hunter.kill_energy=40_hunter.hunt_cost=1_s=3 @16319, hunter.kill_energy=40_hunter.hunt_cost=2_s=2 @7541, hunter.kill_energy=40_hunter.hunt_cost=2_s=3 @9349, hunter.kill_energy=40_hunter.hunt_cost=3_s=1 @10002, hunter.kill_energy=40_hunter.hunt_cost=3_s=2 @8214, hunter.kill_energy=40_hunter.hunt_cost=3_s=3 @8407, hunter.kill_energy=60_hunter.hunt_cost=2_s=2 @12831, hunter.kill_energy=60_hunter.hunt_cost=2_s=3 @7597, hunter.kill_energy=60_hunter.hunt_cost=3_s=1 @10515, hunter.kill_energy=60_hunter.hunt_cost=3_s=2 @7466, hunter.kill_energy=60_hunter.hunt_cost=3_s=3 @9040, hunter.kill_energy=80_hunter.hunt_cost=2_s=2 @17381, hunter.kill_energy=80_hunter.hunt_cost=2_s=3 @9491, hunter.kill_energy=80_hunter.hunt_cost=3_s=1 @11241, hunter.kill_energy=80_hunter.hunt_cost=3_s=2 @10138, hunter.kill_energy=80_hunter.hunt_cost=3_s=3 @11625 |
| grazers | `eaten` | 19 | hunter.kill_energy=20_hunter.hunt_cost=0_s=1 @6627, hunter.kill_energy=20_hunter.hunt_cost=0_s=2 @7324, hunter.kill_energy=20_hunter.hunt_cost=0_s=3 @6784, hunter.kill_energy=20_hunter.hunt_cost=1_s=1 @6773, hunter.kill_energy=40_hunter.hunt_cost=0_s=1 @7283, hunter.kill_energy=40_hunter.hunt_cost=0_s=2 @7198, hunter.kill_energy=40_hunter.hunt_cost=0_s=3 @7330, hunter.kill_energy=40_hunter.hunt_cost=1_s=1 @8604, hunter.kill_energy=40_hunter.hunt_cost=2_s=1 @7105, hunter.kill_energy=60_hunter.hunt_cost=0_s=1 @7835, hunter.kill_energy=60_hunter.hunt_cost=0_s=2 @7575, hunter.kill_energy=60_hunter.hunt_cost=0_s=3 @7429, hunter.kill_energy=60_hunter.hunt_cost=1_s=1 @7347, hunter.kill_energy=60_hunter.hunt_cost=2_s=1 @9637, hunter.kill_energy=80_hunter.hunt_cost=0_s=1 @7258, hunter.kill_energy=80_hunter.hunt_cost=0_s=2 @8119, hunter.kill_energy=80_hunter.hunt_cost=0_s=3 @8005, hunter.kill_energy=80_hunter.hunt_cost=1_s=1 @7927, hunter.kill_energy=80_hunter.hunt_cost=2_s=1 @8682 |
