# Shot 09 findings: fire disturbance

Two sweeps on seeds 1–3 at 20 000 ticks, with every other parameter at its default (`params.toml`: base_rate 0.002, spread 0.1, duration 3). The commands and wall times are in `_log.txt`, and the full tables are in each sweep's `sweep.md` and `sweep.csv`. Burn-outs are `total_burnt` at tick 20 000, taken from the per-cell series (the cells are gitignored and can be regenerated with the logged commands).

## `fire.base_rate` over 0:0.004:0.0005

- All 27 cells pass every invariant. No species reaches 0 in any cell, so there are no extinctions or causes to report.
- The surviving cells form a single contiguous region, the whole grid. There is no collapsing region.
- Fire activity rises roughly linearly with the rate:

| base_rate | burn-outs s1 / s2 / s3 | most patches burning at once |
|---|---|---|
| 0 | 0 / 0 / 0 | 0 |
| 0.0005 | 16 / 5 / 25 | 5 |
| 0.001 | 26 / 63 / 57 | 10 |
| 0.0015 | 97 / 79 / 54 | 11 |
| 0.002 (default) | 42 / 77 / 88 | 12 |
| 0.0025 | 71 / 104 / 76 | 7 |
| 0.003 | 134 / 104 / 85 | 11 |
| 0.0035 | 76 / 167 / 131 | 9 |
| 0.004 | 144 / 181 / 152 | 9 |

- Even at 0.004, fires stay small: at most 12 of 64 patches burn at once. Spread (0.1) is below the percolation point (see below), so the ignition rate controls how often fires start, not how big they get.
- The reference world `runs/s42` at the default passes `ecosim check` (`tests/data/s42-check.txt`).
- In an exploratory run outside this grid, s42 at 0.0025 falls to 34 mature trees at tick 10 000, one below the addendum's 35. The default is kept at 0.002. TUNING.md has the details.

## `fire.spread` over 0:0.5:0.1

| spread | cells passing | burn-outs s1 / s2 / s3 | most patches burning at once | failing invariants |
|---|---|---|---|---|
| 0.0 | 3/3 | 11 / 19 / 16 | 2 | — |
| 0.1 (default) | 3/3 | 42 / 77 / 88 | 12 | — |
| 0.2 | 2/3 | 471 / 556 / 334 | 28 | `grass_band` s3 |
| 0.3 | 1/3 | 684 / 823 / 619 | 25 | `grass_band` s1, `fertility_band` s2 |
| 0.4 | 0/3 | 1651 / 1542 / 1896 | 31 | `fertility_band` all; `mature_trees_10k` s1, s2; `grass_band` s1 |
| 0.5 | 0/3 | 7525 / 5824 / 5200 | 41 | everything; trees extinct |

- **Extinctions.** Only at spread 0.5, where trees are the first species to reach 0 on every seed: s1 at tick 9306, s2 at 17681, s3 at 13511. The sweep reports the cause as `unrecorded` because tree deaths carry no cause in the series; only animal deaths do. From the burn-out counts (5 200–7 500, that is 80–120 burn-outs per patch) and the tree_kill of 0.5 per burn-out, the trees are killed by fire.
- **Failures without extinction.** At 0.2–0.4 the failures are in the band invariants. Repeated burning strips grass (grass_mean falls below 0.05) and piles ash and detritus into fertility (fertility_mean goes above 220). At 0.4, mature trees at tick 10 000 also fall below 35.
- **Regions.** The safe band is [0.0, 0.1]; the sweep flags it as fragile, with the default at its upper edge.
  - Seeds 1 and 2 are contiguous: they pass at 0–0.2 and fail from 0.3 up.
  - Seed 3 is not: it passes at 0–0.1, fails at 0.2, passes at 0.3, and fails from 0.4 up.
  - The failing cells therefore form one region from 0.4 up, plus a mixed band at 0.2–0.3 where the outcome depends on the seed. The transition is a percolation threshold between 0.1 and 0.2: the number of patches burning at once doubles, and the number of burn-outs rises 5–10×.

## Canopy cover over time at each base_rate (report only)

`canopy.py` reruns the base_rate grid with a snapshot every 1000 ticks and counts canopied columns from `entities.json`. Output is in `canopy.txt`:

| base_rate | 0 | 2000 | 4000 | 6000 | 8000 | 10000 | 12000 | 14000 | 16000 | 18000 | 20000 | ticks 15k–20k mean s1 / s2 / s3 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 0.003 | 0.081 | 0.258 | 0.270 | 0.500 | 0.220 | 0.411 | 0.326 | 0.528 | 0.220 | 0.405 | 0.453 / 0.445 / 0.409 |
| 0.0005 | 0.003 | 0.080 | 0.256 | 0.256 | 0.491 | 0.219 | 0.405 | 0.330 | 0.522 | 0.259 | 0.445 | 0.490 / 0.455 / 0.414 |
| 0.001 | 0.003 | 0.080 | 0.256 | 0.254 | 0.495 | 0.196 | 0.411 | 0.333 | 0.530 | 0.215 | 0.419 | 0.464 / 0.438 / 0.415 |
| 0.0015 | 0.003 | 0.080 | 0.256 | 0.248 | 0.501 | 0.216 | 0.428 | 0.299 | 0.505 | 0.202 | 0.439 | 0.449 / 0.415 / 0.427 |
| 0.002 | 0.003 | 0.080 | 0.256 | 0.248 | 0.501 | 0.202 | 0.415 | 0.304 | 0.516 | 0.209 | 0.390 | 0.464 / 0.402 / 0.398 |
| 0.0025 | 0.003 | 0.081 | 0.259 | 0.263 | 0.491 | 0.209 | 0.421 | 0.241 | 0.463 | 0.228 | 0.446 | 0.440 / 0.405 / 0.410 |
| 0.003 | 0.003 | 0.081 | 0.259 | 0.260 | 0.486 | 0.185 | 0.416 | 0.204 | 0.454 | 0.236 | 0.461 | 0.432 / 0.419 / 0.406 |
| 0.0035 | 0.003 | 0.081 | 0.259 | 0.269 | 0.500 | 0.159 | 0.381 | 0.302 | 0.538 | 0.186 | 0.393 | 0.449 / 0.404 / 0.415 |
| 0.004 | 0.003 | 0.081 | 0.259 | 0.259 | 0.497 | 0.178 | 0.389 | 0.248 | 0.486 | 0.231 | 0.466 | 0.461 / 0.401 / 0.441 |

- With or without fire, the canopy never closes completely. Cover follows cohort waves: the first cohort reaches about 50% at tick 8 000, dies back to about 20% by tick 10 000, and later waves peak at about 0.4–0.55.
- The criterion for "stops closing" here is a late mean (ticks 15k–20k) that falls clearly below the rate-0 run on every seed. No rate in the grid meets it:
  - At rate 0 the per-seed late means are 0.41–0.45.
  - At 0.004 they are 0.40–0.46.
  - Every rate stays within about 0.05 of rate 0 on every seed, which is inside the seed-to-seed spread.
- The effect that does show is in the wave troughs. At tick 10 000, cover goes from 0.22 at rate 0 to 0.16–0.18 at 0.0035–0.004, and the tick-14 000 trough also dips at 0.0025–0.003. Fire deepens the troughs between waves but does not stop the canopy re-forming.
- **Answer:** at no base_rate in 0:0.004 does the reference world's canopy stop closing. Canopy loss at the landscape scale needs spread above the percolation threshold (≥ 0.2), not a higher ignition rate.
