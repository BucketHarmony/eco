# Sweep `hunter_kill_prob`

- params: `params.toml`
- `hunter.kill_prob`: 0.1, 0.2, 0.3, 0.4, 0.5
- seeds: 1, 2, 3; ticks: 20000; cells: 15; jobs: 8
- wall time: 31.7 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.kill_prob`

- default: 0.2
- safe band: **[0.2, 0.4]** (3 of 5 values)
- lower edge: at 0.1 first fails `no_extinction` (seeds 1, 2, 3; worst margin -1.000)
  - also failing there: `animals_10k` (seeds 2, 3; worst margin -1.000)
- upper edge: at 0.5 first fails `no_extinction` (seed 2; worst margin -1.000)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.1 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 2, 3; worst margin -1.000) |
| 0.2 (default) | 3/3 | — |
| 0.3 | 3/3 | — |
| 0.4 | 3/3 | — |
| 0.5 | 2/3 | `no_extinction` (seed 2; worst margin -1.000) |

