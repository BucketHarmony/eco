# Sweep `hunter_refugium_shrub`

- params: `params.toml`
- `hunter.refugium_shrub`: 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8
- seeds: 1, 2, 3; ticks: 20000; cells: 21; jobs: 8
- wall time: 24.5 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.refugium_shrub`

- default: 0.58
- safe band: **[0.6, 0.6]** (1 of 7 values) — **fragile**
- lower edge: at 0.5 first fails `no_extinction` (seeds 1, 2, 3; worst margin -1.000)
- upper edge: at 0.7 first fails `no_extinction` (seeds 1, 2, 3; worst margin -1.000)
  - also failing there: `grazer_cycle` (seeds 1, 3; worst margin -1.000); `animals_10k` (seeds 1, 3; worst margin -1.000)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.2 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seed 2; worst margin -1.000) |
| 0.3 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seed 2; worst margin -1.000) |
| 0.4 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seed 2; worst margin -1.000) |
| 0.5 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000) |
| 0.6 | 3/3 | — |
| 0.7 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 3; worst margin -1.000); `animals_10k` (seeds 1, 3; worst margin -1.000) |
| 0.8 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `grazer_cycle` (seeds 1, 3; worst margin -1.000); `animals_10k` (seed 3; worst margin -1.000) |

