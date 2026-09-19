# Sweep `tree_mature_age`

- params: `params.toml`
- `tree.mature_age`: 500, 1000, 1500, 2000, 2500
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 28.9 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `tree.mature_age`

- default: 1000
- safe band: **[1000, 1000]** (1 of 5 values) — **fragile**
- lower edge: at 500 first fails `no_extinction` (seed 3; worst margin -1.000)
  - also failing there: `grass_band` (seed 1; worst margin -0.074)
- upper edge: at 1500 first fails `max_10x` (seeds 1, 2, 3; worst margin -0.277)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 500 | 1/3 | `no_extinction` (seed 3; worst margin -1.000); `grass_band` (seed 1; worst margin -0.074) |
| 1000 (default) | 3/3 | — |
| 1500 | 0/3 | `max_10x` (seeds 1, 2, 3; worst margin -0.277) |
| 2000 | 0/3 | `max_10x` (seeds 1, 2, 3; worst margin -0.650); `no_extinction` (seed 2; worst margin -1.000) |
| 2500 | 0/3 | `max_10x` (seeds 1, 2, 3; worst margin -4.050) |

