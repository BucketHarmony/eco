# Sweep `crowding_on_kill_x_cost`

- params: `params.toml`
- `hunter.kill_energy`: 40, 50, 60, 70, 80
- `hunter.hunt_cost`: 0.00, 0.25, 0.50, 0.75, 1.00, 1.25, 1.50
- seeds: 1, 2, 3, 42; ticks: 20000; cells: 140; jobs: 22
- wall time: 65.3 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.kill_energy`

- default: 40.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 40 (default) | 14/28 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 2, 3, 42; worst margin -1.000); `mature_trees_10k` (seed 42; worst margin -0.229) |
| 50 | 20/28 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 2, 3, 42; worst margin -1.000) |
| 60 | 18/28 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seed 42; worst margin -1.000) |
| 70 | 19/28 | `no_extinction` (seeds 1, 3, 42; worst margin -1.000); `animals_10k` (seed 42; worst margin -1.000) |
| 80 | 17/28 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000) |

## `hunter.hunt_cost`

- default: 0.0
- safe band: **[0.00, 0.50]** (3 of 7 values)
- lower edge: passes to the end of the grid
- upper edge: at 0.75 first fails `no_extinction` (seeds 3, 42; worst margin -1.000)
  - also failing there: `mature_trees_10k` (seed 42; worst margin -0.229)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.00 (default) | 20/20 | — |
| 0.25 | 20/20 | — |
| 0.50 | 20/20 | — |
| 0.75 | 17/20 | `no_extinction` (seeds 3, 42; worst margin -1.000); `mature_trees_10k` (seed 42; worst margin -0.229) |
| 1.00 | 7/20 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seed 42; worst margin -0.500) |
| 1.25 | 3/20 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 2, 3; worst margin -1.000) |
| 1.50 | 1/20 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 2, 3, 42; worst margin -1.000) |

## Matrix: seeds passing out of 4

| `hunter.kill_energy` \ `hunter.hunt_cost` | 0.00 | 0.25 | 0.50 | 0.75 | 1.00 | 1.25 | 1.50 |
|---|---|---|---|---|---|---|---|
| 40 | 4/4 | 4/4 | 4/4 | 2/4 | 0/4 | 0/4 | 0/4 |
| 50 | 4/4 | 4/4 | 4/4 | 3/4 | 3/4 | 2/4 | 0/4 |
| 60 | 4/4 | 4/4 | 4/4 | 4/4 | 2/4 | 0/4 | 0/4 |
| 70 | 4/4 | 4/4 | 4/4 | 4/4 | 1/4 | 1/4 | 1/4 |
| 80 | 4/4 | 4/4 | 4/4 | 4/4 | 1/4 | 0/4 | 0/4 |

## Extinctions by cause

- cells failing an invariant: 52 of 140
- cells in which a species reaches 0 at any tick: 52 of 140
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| hunters | `starved` | 48 | hunter.kill_energy=40_hunter.hunt_cost=0.75_s=3 @14884, hunter.kill_energy=40_hunter.hunt_cost=0.75_s=42 @12403, hunter.kill_energy=40_hunter.hunt_cost=1.00_s=1 @18156, hunter.kill_energy=40_hunter.hunt_cost=1.00_s=2 @16222, hunter.kill_energy=40_hunter.hunt_cost=1.00_s=3 @12778, hunter.kill_energy=40_hunter.hunt_cost=1.00_s=42 @10141, hunter.kill_energy=40_hunter.hunt_cost=1.25_s=1 @12682, hunter.kill_energy=40_hunter.hunt_cost=1.25_s=2 @12669, hunter.kill_energy=40_hunter.hunt_cost=1.25_s=3 @9640, hunter.kill_energy=40_hunter.hunt_cost=1.25_s=42 @13271, hunter.kill_energy=40_hunter.hunt_cost=1.50_s=1 @14863, hunter.kill_energy=40_hunter.hunt_cost=1.50_s=2 @8990, hunter.kill_energy=40_hunter.hunt_cost=1.50_s=3 @11214, hunter.kill_energy=40_hunter.hunt_cost=1.50_s=42 @12225, hunter.kill_energy=50_hunter.hunt_cost=0.75_s=42 @17437, hunter.kill_energy=50_hunter.hunt_cost=1.00_s=42 @14683, hunter.kill_energy=50_hunter.hunt_cost=1.25_s=3 @13947, hunter.kill_energy=50_hunter.hunt_cost=1.25_s=42 @14178, hunter.kill_energy=50_hunter.hunt_cost=1.50_s=1 @16442, hunter.kill_energy=50_hunter.hunt_cost=1.50_s=2 @10168, hunter.kill_energy=50_hunter.hunt_cost=1.50_s=3 @9853, hunter.kill_energy=50_hunter.hunt_cost=1.50_s=42 @9223, hunter.kill_energy=60_hunter.hunt_cost=1.00_s=1 @15937, hunter.kill_energy=60_hunter.hunt_cost=1.00_s=42 @15895, hunter.kill_energy=60_hunter.hunt_cost=1.25_s=1 @16844, hunter.kill_energy=60_hunter.hunt_cost=1.25_s=2 @14293, hunter.kill_energy=60_hunter.hunt_cost=1.25_s=3 @17722, hunter.kill_energy=60_hunter.hunt_cost=1.25_s=42 @18659, hunter.kill_energy=60_hunter.hunt_cost=1.50_s=1 @16461, hunter.kill_energy=60_hunter.hunt_cost=1.50_s=2 @12899, hunter.kill_energy=60_hunter.hunt_cost=1.50_s=3 @12796, hunter.kill_energy=70_hunter.hunt_cost=1.00_s=1 @13404, hunter.kill_energy=70_hunter.hunt_cost=1.00_s=3 @17953, hunter.kill_energy=70_hunter.hunt_cost=1.00_s=42 @14837, hunter.kill_energy=70_hunter.hunt_cost=1.25_s=3 @19539, hunter.kill_energy=70_hunter.hunt_cost=1.50_s=1 @12699, hunter.kill_energy=70_hunter.hunt_cost=1.50_s=3 @14341, hunter.kill_energy=70_hunter.hunt_cost=1.50_s=42 @9821, hunter.kill_energy=80_hunter.hunt_cost=1.00_s=1 @18455, hunter.kill_energy=80_hunter.hunt_cost=1.00_s=3 @13847, hunter.kill_energy=80_hunter.hunt_cost=1.00_s=42 @18641, hunter.kill_energy=80_hunter.hunt_cost=1.25_s=1 @16837, hunter.kill_energy=80_hunter.hunt_cost=1.25_s=2 @11356, hunter.kill_energy=80_hunter.hunt_cost=1.25_s=3 @16315, hunter.kill_energy=80_hunter.hunt_cost=1.25_s=42 @14506, hunter.kill_energy=80_hunter.hunt_cost=1.50_s=1 @14811, hunter.kill_energy=80_hunter.hunt_cost=1.50_s=2 @16119, hunter.kill_energy=80_hunter.hunt_cost=1.50_s=3 @10903 |
| hunters | `old_age` | 4 | hunter.kill_energy=60_hunter.hunt_cost=1.50_s=42 @9163, hunter.kill_energy=70_hunter.hunt_cost=1.25_s=1 @17601, hunter.kill_energy=70_hunter.hunt_cost=1.25_s=42 @14571, hunter.kill_energy=80_hunter.hunt_cost=1.50_s=42 @13608 |
