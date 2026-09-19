# Sweep `handling_x_cost`

- params: `params.toml`
- fixed overrides: `disease.hunter_rate=0`, `hunter.kill_energy=60`
- `hunter.handling_ticks`: 25, 50, 100, 150
- `hunter.hunt_cost`: 0.6, 0.8, 1.0, 1.2
- seeds: 1, 2, 3, 42; ticks: 60000; cells: 64; jobs: 24
- wall time: 228.9 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.handling_ticks`

- default: 0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 25 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.159); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 3; worst margin -1.000) |
| 50 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.159); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2; worst margin -1.000) |
| 100 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.159); `grazer_cycle` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000) |
| 150 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.159) |

## `hunter.hunt_cost`

- default: 0.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.6 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.159); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seed 2; worst margin -1.000) |
| 0.8 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.159); `grazer_cycle` (seeds 1, 3; worst margin -1.000); `animals_10k` (seeds 1, 3; worst margin -1.000) |
| 1.0 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.159); `grazer_cycle` (seed 1; worst margin -1.000); `animals_10k` (seed 1; worst margin -1.000) |
| 1.2 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.159); `animals_10k` (seed 1; worst margin -1.000); `grazer_cycle` (seed 1; worst margin -0.145) |

## Matrix: seeds passing out of 4

| `hunter.handling_ticks` \ `hunter.hunt_cost` | 0.6 | 0.8 | 1.0 | 1.2 |
|---|---|---|---|---|
| 25 | 0/4 | 0/4 | 0/4 | 0/4 |
| 50 | 0/4 | 0/4 | 0/4 | 0/4 |
| 100 | 0/4 | 0/4 | 0/4 | 0/4 |
| 150 | 0/4 | 0/4 | 0/4 | 0/4 |

## Extinctions by cause

- cells failing an invariant: 64 of 64
- cells in which a species reaches 0 at any tick: 57 of 64
- failing cells with no extinction: 7; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| hunters | `starved` | 31 | hunter.handling_ticks=25_hunter.hunt_cost=0.8_s=42 @41375, hunter.handling_ticks=25_hunter.hunt_cost=1.0_s=2 @20825, hunter.handling_ticks=25_hunter.hunt_cost=1.0_s=3 @20985, hunter.handling_ticks=25_hunter.hunt_cost=1.0_s=42 @19957, hunter.handling_ticks=25_hunter.hunt_cost=1.2_s=2 @15803, hunter.handling_ticks=25_hunter.hunt_cost=1.2_s=3 @18506, hunter.handling_ticks=25_hunter.hunt_cost=1.2_s=42 @22725, hunter.handling_ticks=50_hunter.hunt_cost=0.8_s=3 @25777, hunter.handling_ticks=50_hunter.hunt_cost=0.8_s=42 @32966, hunter.handling_ticks=50_hunter.hunt_cost=1.0_s=2 @20691, hunter.handling_ticks=50_hunter.hunt_cost=1.0_s=3 @21498, hunter.handling_ticks=50_hunter.hunt_cost=1.0_s=42 @20384, hunter.handling_ticks=50_hunter.hunt_cost=1.2_s=1 @24377, hunter.handling_ticks=50_hunter.hunt_cost=1.2_s=3 @15646, hunter.handling_ticks=100_hunter.hunt_cost=0.8_s=2 @57919, hunter.handling_ticks=100_hunter.hunt_cost=0.8_s=3 @29647, hunter.handling_ticks=100_hunter.hunt_cost=0.8_s=42 @22523, hunter.handling_ticks=100_hunter.hunt_cost=1.0_s=2 @41484, hunter.handling_ticks=100_hunter.hunt_cost=1.0_s=3 @25585, hunter.handling_ticks=100_hunter.hunt_cost=1.0_s=42 @14850, hunter.handling_ticks=100_hunter.hunt_cost=1.2_s=1 @21304, hunter.handling_ticks=100_hunter.hunt_cost=1.2_s=2 @16808, hunter.handling_ticks=100_hunter.hunt_cost=1.2_s=3 @14390, hunter.handling_ticks=100_hunter.hunt_cost=1.2_s=42 @16310, hunter.handling_ticks=150_hunter.hunt_cost=0.8_s=3 @34602, hunter.handling_ticks=150_hunter.hunt_cost=0.8_s=42 @25663, hunter.handling_ticks=150_hunter.hunt_cost=1.0_s=2 @18255, hunter.handling_ticks=150_hunter.hunt_cost=1.0_s=3 @21348, hunter.handling_ticks=150_hunter.hunt_cost=1.0_s=42 @22869, hunter.handling_ticks=150_hunter.hunt_cost=1.2_s=2 @17409, hunter.handling_ticks=150_hunter.hunt_cost=1.2_s=42 @12448 |
| grazers | `eaten` | 20 | hunter.handling_ticks=25_hunter.hunt_cost=0.6_s=1 @7390, hunter.handling_ticks=25_hunter.hunt_cost=0.6_s=2 @9127, hunter.handling_ticks=25_hunter.hunt_cost=0.6_s=3 @8531, hunter.handling_ticks=25_hunter.hunt_cost=0.8_s=1 @7150, hunter.handling_ticks=25_hunter.hunt_cost=0.8_s=3 @8607, hunter.handling_ticks=25_hunter.hunt_cost=1.0_s=1 @8339, hunter.handling_ticks=25_hunter.hunt_cost=1.2_s=1 @8531, hunter.handling_ticks=50_hunter.hunt_cost=0.6_s=1 @8407, hunter.handling_ticks=50_hunter.hunt_cost=0.6_s=2 @9028, hunter.handling_ticks=50_hunter.hunt_cost=0.6_s=3 @9512, hunter.handling_ticks=50_hunter.hunt_cost=0.6_s=42 @23225, hunter.handling_ticks=50_hunter.hunt_cost=0.8_s=1 @7575, hunter.handling_ticks=50_hunter.hunt_cost=0.8_s=2 @12847, hunter.handling_ticks=50_hunter.hunt_cost=1.0_s=1 @9497, hunter.handling_ticks=100_hunter.hunt_cost=0.6_s=1 @9373, hunter.handling_ticks=100_hunter.hunt_cost=0.6_s=2 @12768, hunter.handling_ticks=100_hunter.hunt_cost=0.8_s=1 @9142, hunter.handling_ticks=150_hunter.hunt_cost=0.6_s=1 @13129, hunter.handling_ticks=150_hunter.hunt_cost=0.6_s=2 @42504, hunter.handling_ticks=150_hunter.hunt_cost=0.6_s=3 @22561 |
| hunters | `old_age` | 6 | hunter.handling_ticks=50_hunter.hunt_cost=1.2_s=2 @16337, hunter.handling_ticks=50_hunter.hunt_cost=1.2_s=42 @16232, hunter.handling_ticks=100_hunter.hunt_cost=1.0_s=1 @31345, hunter.handling_ticks=150_hunter.hunt_cost=1.0_s=1 @27934, hunter.handling_ticks=150_hunter.hunt_cost=1.2_s=1 @19516, hunter.handling_ticks=150_hunter.hunt_cost=1.2_s=3 @22612 |
