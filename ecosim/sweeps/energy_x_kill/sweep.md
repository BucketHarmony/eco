# Sweep `energy_x_kill`

- params: `params.toml`
- `grazer.energy_cost`: 0.06, 0.08, 0.10
- `hunter.kill_prob`: 0.2, 0.3, 0.4
- seeds: 1, 2, 3; ticks: 20000; cells: 27; jobs: 8
- wall time: 34.5 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `grazer.energy_cost`

- default: 0.08
- safe band: **[0.08, 0.08]** (1 of 3 values) — **fragile**
- lower edge: at 0.06 first fails `no_extinction` (seed 1; worst margin -1.000)
- upper edge: at 0.10 first fails `no_extinction` (seeds 1, 3; worst margin -1.000)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.06 | 8/9 | `no_extinction` (seed 1; worst margin -1.000) |
| 0.08 (default) | 9/9 | — |
| 0.10 | 7/9 | `no_extinction` (seeds 1, 3; worst margin -1.000) |

## `hunter.kill_prob`

- default: 0.2
- safe band: **none** — **fragile**

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.2 (default) | 8/9 | `no_extinction` (seed 1; worst margin -1.000) |
| 0.3 | 8/9 | `no_extinction` (seed 1; worst margin -1.000) |
| 0.4 | 8/9 | `no_extinction` (seed 3; worst margin -1.000) |

## Matrix: seeds passing out of 3

| `grazer.energy_cost` \ `hunter.kill_prob` | 0.2 | 0.3 | 0.4 |
|---|---|---|---|
| 0.06 | 3/3 | 2/3 | 3/3 |
| 0.08 | 3/3 | 3/3 | 3/3 |
| 0.10 | 2/3 | 3/3 | 2/3 |
