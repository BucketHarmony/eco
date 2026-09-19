# Sweep `hunter_refractory`

- params: `params.toml`
- `hunter.refractory`: 100, 250, 400, 550, 700, 850, 1000
- seeds: 1, 2, 3; ticks: 20000; cells: 21; jobs: 22
- wall time: 1.3 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.refractory`

- default: 2750
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 100 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.045) |
| 250 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.043) |
| 400 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.021) |
| 550 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.037) |
| 700 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 2, 3; worst margin -0.029) |
| 850 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.037) |
| 1000 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000); `fertility_band` (seeds 1, 2, 3; worst margin -0.018) |

## Extinctions by cause

- cells failing an invariant: 21 of 21
- cells in which a species reaches 0 at any tick: 21 of 21
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| grazers | `eaten` | 21 | hunter.refractory=100_s=1 @649, hunter.refractory=100_s=2 @519, hunter.refractory=100_s=3 @450, hunter.refractory=250_s=1 @1031, hunter.refractory=250_s=2 @844, hunter.refractory=250_s=3 @959, hunter.refractory=400_s=1 @1483, hunter.refractory=400_s=2 @1494, hunter.refractory=400_s=3 @1303, hunter.refractory=550_s=1 @2202, hunter.refractory=550_s=2 @2009, hunter.refractory=550_s=3 @2028, hunter.refractory=700_s=1 @2832, hunter.refractory=700_s=2 @2675, hunter.refractory=700_s=3 @2676, hunter.refractory=850_s=1 @2868, hunter.refractory=850_s=2 @3154, hunter.refractory=850_s=3 @2489, hunter.refractory=1000_s=1 @3542, hunter.refractory=1000_s=2 @4493, hunter.refractory=1000_s=3 @3367 |
