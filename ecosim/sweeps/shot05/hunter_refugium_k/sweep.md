# Sweep `hunter_refugium_k`

- params: `params.toml`
- `hunter.refugium_k`: 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0
- seeds: 1, 2, 3; ticks: 20000; cells: 24; jobs: 22
- wall time: 46.7 s
- invariants: `no_extinction`, `max_10x`, `grazer_cycle`, `fertility_band`, `grass_band`, `tree_growth`, `mature_trees_10k`, `animals_10k` (runtime excluded)

A value is **safe** when every invariant passes on every seed in every cell with that value (across all values of the other swept params). The safe band is the contiguous run of safe values containing the default (else the longest). It is **fragile** when it spans fewer than 3 grid values, i.e. is narrower than two grid steps. At each edge, the first failing invariant is the one failing in the most cells at the neighbouring value, ties to the most negative margin.

## `hunter.refugium_k`

- default: 2.0
- safe band: **[0.5, 2.5]** (5 of 8 values)
- lower edge: passes to the end of the grid
- upper edge: at 3.0 first fails `no_extinction` (seeds 2, 3; worst margin -1.000)

| value | cells passing | failing invariants (seeds; worst margin) |
|---|---|---|
| 0.5 | 3/3 | — |
| 1.0 | 3/3 | — |
| 1.5 | 3/3 | — |
| 2.0 (default) | 3/3 | — |
| 2.5 | 3/3 | — |
| 3.0 | 1/3 | `no_extinction` (seeds 2, 3; worst margin -1.000) |
| 3.5 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000) |
| 4.0 | 0/3 | `no_extinction` (seeds 1, 2, 3; worst margin -1.000); `animals_10k` (seeds 1, 2, 3; worst margin -1.000) |

## Extinctions by cause

- cells failing an invariant: 8 of 24
- cells in which a species reaches 0 at any tick: 8 of 24
- failing cells with no extinction: 0; extinction cells passing every invariant: 0

Grouped by the first species to reach 0 and its dominant death cause over the 500 ticks ending there.

| first extinct | dominant cause | cells | cell @ tick |
|---|---|---|---|
| hunters | `starved` | 7 | hunter.refugium_k=3.0_s=3 @18355, hunter.refugium_k=3.5_s=1 @16052, hunter.refugium_k=3.5_s=2 @13363, hunter.refugium_k=3.5_s=3 @12246, hunter.refugium_k=4.0_s=1 @11289, hunter.refugium_k=4.0_s=2 @6832, hunter.refugium_k=4.0_s=3 @7644 |
| hunters | `old_age` | 1 | hunter.refugium_k=3.0_s=2 @18109 |
