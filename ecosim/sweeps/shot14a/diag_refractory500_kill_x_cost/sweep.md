# Sweep `diag_refractory500_kill_x_cost`

- params: `params.toml`
- fixed overrides: `disease.hunter_rate=0`, `hunter.refractory=500`
- `hunter.kill_energy`: 20, 40, 60, 80
- `hunter.hunt_cost`: 0.5, 1, 1.5, 2
- seeds: 1, 2, 3, 42; ticks: 20000; cells: 64; jobs: 22
- wall time: 4.3 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.kill_energy`

- default: 40.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 20 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.046) |
| 40 (default) | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.060) |
| 60 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.059) |
| 80 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.055); `mature_trees_10k` (seed 42; worst margin -0.143) |

## `hunter.hunt_cost`

- default: 0.0
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.5 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.050) |
| 1 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.060) |
| 1.5 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.049); `mature_trees_10k` (seed 42; worst margin -0.143) |
| 2 | 0/16 | `no_extinction` (seeds 1, 2, 3, 42; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3, 42; worst margin -1.000); `animals_10k` (seeds 1, 2, 3, 42; worst margin -1.000); `fertility_band` (seeds 1, 2, 3, 42; worst margin -0.059) |

## Matrix: seeds passing out of 4

| `hunter.kill_energy` \ `hunter.hunt_cost` | 0.5 | 1 | 1.5 | 2 |
|---|---|---|---|---|
| 20 | 0/4 | 0/4 | 0/4 | 0/4 |
| 40 | 0/4 | 0/4 | 0/4 | 0/4 |
| 60 | 0/4 | 0/4 | 0/4 | 0/4 |
| 80 | 0/4 | 0/4 | 0/4 | 0/4 |

## Extinctions by cause

- cells failing an invariant: 64 of 64
- cells in which a species reaches 0 at any tick: 64 of 64
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 64 | hunter.kill_energy=20_hunter.hunt_cost=0.5_s=1 @1444, hunter.kill_energy=20_hunter.hunt_cost=0.5_s=2 @1249, hunter.kill_energy=20_hunter.hunt_cost=0.5_s=3 @1306, hunter.kill_energy=20_hunter.hunt_cost=0.5_s=42 @1362, hunter.kill_energy=20_hunter.hunt_cost=1_s=1 @1544, hunter.kill_energy=20_hunter.hunt_cost=1_s=2 @1608, hunter.kill_energy=20_hunter.hunt_cost=1_s=3 @1377, hunter.kill_energy=20_hunter.hunt_cost=1_s=42 @1364, hunter.kill_energy=20_hunter.hunt_cost=1.5_s=1 @1297, hunter.kill_energy=20_hunter.hunt_cost=1.5_s=2 @1233, hunter.kill_energy=20_hunter.hunt_cost=1.5_s=3 @1435, hunter.kill_energy=20_hunter.hunt_cost=1.5_s=42 @1298, hunter.kill_energy=20_hunter.hunt_cost=2_s=1 @1285, hunter.kill_energy=20_hunter.hunt_cost=2_s=2 @1304, hunter.kill_energy=20_hunter.hunt_cost=2_s=3 @1225, hunter.kill_energy=20_hunter.hunt_cost=2_s=42 @1216, hunter.kill_energy=40_hunter.hunt_cost=0.5_s=1 @1724, hunter.kill_energy=40_hunter.hunt_cost=0.5_s=2 @1802, hunter.kill_energy=40_hunter.hunt_cost=0.5_s=3 @1691, hunter.kill_energy=40_hunter.hunt_cost=0.5_s=42 @1539, hunter.kill_energy=40_hunter.hunt_cost=1_s=1 @1764, hunter.kill_energy=40_hunter.hunt_cost=1_s=2 @1700, hunter.kill_energy=40_hunter.hunt_cost=1_s=3 @1692, hunter.kill_energy=40_hunter.hunt_cost=1_s=42 @1426, hunter.kill_energy=40_hunter.hunt_cost=1.5_s=1 @1664, hunter.kill_energy=40_hunter.hunt_cost=1.5_s=2 @1593, hunter.kill_energy=40_hunter.hunt_cost=1.5_s=3 @1708, hunter.kill_energy=40_hunter.hunt_cost=1.5_s=42 @1377, hunter.kill_energy=40_hunter.hunt_cost=2_s=1 @1751, hunter.kill_energy=40_hunter.hunt_cost=2_s=2 @1717, hunter.kill_energy=40_hunter.hunt_cost=2_s=3 @1686, hunter.kill_energy=40_hunter.hunt_cost=2_s=42 @1559, hunter.kill_energy=60_hunter.hunt_cost=0.5_s=1 @1923, hunter.kill_energy=60_hunter.hunt_cost=0.5_s=2 @1717, hunter.kill_energy=60_hunter.hunt_cost=0.5_s=3 @1782, hunter.kill_energy=60_hunter.hunt_cost=0.5_s=42 @1749, hunter.kill_energy=60_hunter.hunt_cost=1_s=1 @1883, hunter.kill_energy=60_hunter.hunt_cost=1_s=2 @1857, hunter.kill_energy=60_hunter.hunt_cost=1_s=3 @1767, hunter.kill_energy=60_hunter.hunt_cost=1_s=42 @1568, hunter.kill_energy=60_hunter.hunt_cost=1.5_s=1 @1895, hunter.kill_energy=60_hunter.hunt_cost=1.5_s=2 @1865, hunter.kill_energy=60_hunter.hunt_cost=1.5_s=3 @1837, hunter.kill_energy=60_hunter.hunt_cost=1.5_s=42 @1655, hunter.kill_energy=60_hunter.hunt_cost=2_s=1 @1980, hunter.kill_energy=60_hunter.hunt_cost=2_s=2 @1803, hunter.kill_energy=60_hunter.hunt_cost=2_s=3 @1701, hunter.kill_energy=60_hunter.hunt_cost=2_s=42 @1728, hunter.kill_energy=80_hunter.hunt_cost=0.5_s=1 @1974, hunter.kill_energy=80_hunter.hunt_cost=0.5_s=2 @1891, hunter.kill_energy=80_hunter.hunt_cost=0.5_s=3 @1918, hunter.kill_energy=80_hunter.hunt_cost=0.5_s=42 @1766, hunter.kill_energy=80_hunter.hunt_cost=1_s=1 @1918, hunter.kill_energy=80_hunter.hunt_cost=1_s=2 @1961, hunter.kill_energy=80_hunter.hunt_cost=1_s=3 @1985, hunter.kill_energy=80_hunter.hunt_cost=1_s=42 @1783, hunter.kill_energy=80_hunter.hunt_cost=1.5_s=1 @1793, hunter.kill_energy=80_hunter.hunt_cost=1.5_s=2 @1860, hunter.kill_energy=80_hunter.hunt_cost=1.5_s=3 @1912, hunter.kill_energy=80_hunter.hunt_cost=1.5_s=42 @1650, hunter.kill_energy=80_hunter.hunt_cost=2_s=1 @1937, hunter.kill_energy=80_hunter.hunt_cost=2_s=2 @1785, hunter.kill_energy=80_hunter.hunt_cost=2_s=3 @1721, hunter.kill_energy=80_hunter.hunt_cost=2_s=42 @1783 |
