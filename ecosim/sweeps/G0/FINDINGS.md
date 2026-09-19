# Shot G0: animals off vs animals on

The sweep is `ecosim sweep --param animals.enabled --values true,false --seeds 1,2,3 --ticks 20000 --jobs 6 --out sweeps/G0` (6/6 cells pass, 46.6 s wall on 6 jobs). The tables below come from separate pinned runs of the same twelve configurations with `events.csv` and snapshots, written to `target/g0/` (not committed).

World: the reference 256×64×32 strip at the defaults. Animals off is `--set animals.enabled=false`; nothing else was retuned.

## Event-log cause breakdown, seeds 1–3, 20000 ticks

Deaths by cause, per species, from `events.csv` (`kind=death` for animals, `kind=tree_death` for trees).

| seed | animals | species | deaths | causes |
|---|---|---|---|---|
| 1 | on | grazer | 53204 | crowded 40661, eaten 11570, starved 570, old_age 401, burnt 2 |
| 1 | on | hunter | 434 | crowded 281, old_age 110, starved 43 |
| 1 | on | tree | 3366 | crowded 1590, drought 833, old_age 560, burnt 383 |
| 1 | off | grazer | 0 | — |
| 1 | off | hunter | 0 | — |
| 1 | off | tree | 3275 | crowded 1623, drought 740, old_age 639, burnt 273 |
| 2 | on | grazer | 57592 | crowded 44449, eaten 11677, starved 800, old_age 633, burnt 33 |
| 2 | on | hunter | 399 | crowded 288, old_age 109, starved 2 |
| 2 | on | tree | 2934 | crowded 1599, drought 536, old_age 585, burnt 214 |
| 2 | off | grazer | 0 | — |
| 2 | off | hunter | 0 | — |
| 2 | off | tree | 3004 | crowded 1604, drought 517, old_age 614, burnt 269 |
| 3 | on | grazer | 47469 | crowded 35256, eaten 9161, old_age 1652, starved 1339, burnt 61 |
| 3 | on | hunter | 353 | crowded 273, old_age 68, starved 12 |
| 3 | on | tree | 3212 | crowded 1831, drought 494, old_age 718, burnt 169 |
| 3 | off | grazer | 0 | — |
| 3 | off | hunter | 0 | — |
| 3 | off | tree | 3258 | crowded 1771, drought 495, old_age 734, burnt 258 |

- **No extinctions in any of the twelve runs.** Every seed keeps trees to tick 20000 with animals on or off, and the animals-on runs keep grazers and hunters. The animals-off runs have no grazers or hunters at any tick by construction, which is why `ecosim check` marks the animal invariants n/a rather than failing them.
- **No animal event is ever logged with animals off** (0 rows with `species=grazer` or `species=hunter`, of any kind, on all three seeds). The whole animal tier, including births and immigration, is skipped.
- **Tree mortality barely moves.** Total tree deaths change by −2.7%, +2.4% and +1.4% on seeds 1, 2 and 3. The mix shifts a little toward `crowded` and away from `drought`, which is consistent with more surviving ground cover holding moisture: `drought` falls 11%, 4% and no change, while `crowded` rises 2%, 0% and −3%. The differences are within the seed-to-seed spread, so this shot claims no effect on trees beyond "none large enough to see on three seeds".
- **Event volume drops 15×** (seed 1: 116680 rows on, 7654 off). With animals on, 94% of rows are grazer births and deaths. With animals off the log is germinations (4163), tree deaths (3275), burnouts, ignitions and spread only.

## Cover and tree counts at ticks 10000 and 20000

From each run's snapshots: `grass_mean`/`shrub_mean` are the mean patch cover over all patches; `trees` and `mature` from `entities.json`; canopy columns are the distinct columns under a crown (1 column for a young tree, 3×3 for a mature one) out of 16384.

| animals | seed | tick | grass_mean | shrub_mean | trees | mature | canopy columns | canopy % | grazers | hunters |
|---|---|---|---|---|---|---|---|---|---|---|
| on | 1 | 10000 | 0.5977 | 0.2779 | 639 | 424 | 3163 | 19.3% | 2389 | 140 |
| on | 1 | 20000 | 0.5570 | 0.3604 | 873 | 605 | 4467 | 27.3% | 1636 | 141 |
| on | 2 | 10000 | 0.5916 | 0.2391 | 643 | 441 | 3267 | 19.9% | 2656 | 124 |
| on | 2 | 20000 | 0.5311 | 0.3480 | 894 | 635 | 4743 | 28.9% | 2306 | 163 |
| on | 3 | 10000 | 0.6036 | 0.2688 | 731 | 505 | 3767 | 23.0% | 2938 | 117 |
| on | 3 | 20000 | 0.5160 | 0.3656 | 978 | 702 | 5227 | 31.9% | 2728 | 85 |
| off | 1 | 10000 | 0.6606 | 0.2714 | 674 | 439 | 3278 | 20.0% | 0 | 0 |
| off | 1 | 20000 | 0.6029 | 0.3588 | 900 | 626 | 4638 | 28.3% | 0 | 0 |
| off | 2 | 10000 | 0.6874 | 0.2605 | 647 | 420 | 3149 | 19.2% | 0 | 0 |
| off | 2 | 20000 | 0.6071 | 0.3550 | 876 | 627 | 4679 | 28.6% | 0 | 0 |
| off | 3 | 10000 | 0.6732 | 0.2875 | 777 | 511 | 3854 | 23.5% | 0 | 0 |
| off | 3 | 20000 | 0.5684 | 0.3741 | 1005 | 719 | 5311 | 32.4% | 0 | 0 |

- **Grass is the only cover that changes much: +8 to +16% relative.** At tick 10000 grass goes 0.598→0.661, 0.592→0.687 and 0.604→0.673 on seeds 1–3; at 20000, 0.557→0.603, 0.531→0.607 and 0.516→0.568. That is the grazing intake removed, and it is smaller than the halving a naive reading of "grazers eat grass" would predict, because grass is already near its logistic ceiling in the wet half of the strip and the dry half is limited by moisture, not by grazers.
- **Shrub is unchanged within noise** (±3% relative at both ticks, in both directions). Shrubs are the refugium species and grazers only crop them when grass runs out, which on these seeds is rare.
- **Trees are unchanged within noise.** Tree counts differ by −1 to +6%, and mature counts by −5 to +4%, with no consistent sign across seeds. Canopy cover at 20000 is 27.3/28.9/31.9% with animals and 28.3/28.6/32.4% without.
- **Detritus loses a quarter to a third, and fertility still rises.** Corpses are a detritus source, so the totals fall: −28.3/−34.8/−32.1% at tick 10000 and −23.4/−27.1/−25.0% at 20000 on seeds 1–3 (seed 1 at 20000: 553188 with animals, 423597 without). Mean fertility nevertheless ends **higher** without animals (+4.5/+4.1/+5.5% at tick 20000, +1.4/+1.5/+3.9% at 10000): the grass that is not eaten dies and decays in place, and that path feeds fertility more than corpses do. Mean moisture is identical to within 1% at every point measured (+0.6/+0.2/+1.5% at 10000, −0.0/+0.0/−0.0% at 20000).
- **Contiguity** of the surviving band is not a question here: the swept parameter is a boolean, so the grid is two values wide and both pass on all three seeds (`sweep.md` reports the band as "fragile" only because "fewer than 3 grid values" is its rule, and a boolean can never have three).

## `ecosim check` and wall times

Pinned to the P-cores (`PERF.md`, "How it was measured"); 20000 ticks, snapshots every 100, `events.csv` on.

| run | wall | `ecosim check` |
|---|---|---|
| on, seed 1 | 22.3 s | pass |
| on, seed 2 | 27.5 s | pass |
| on, seed 3 | 28.1 s | pass |
| off, seed 1 | 1.7 s | pass (2 invariants n/a) |
| off, seed 2 | 1.8 s | pass (2 invariants n/a) |
| off, seed 3 | 1.9 s | pass (2 invariants n/a) |

- The regression anchor is unchanged: with animals on, seeds 1, 2 and 3 pass every invariant exactly as before the shot, and seed 42's manifests are byte-identical.
- The two n/a lines are `grazer cycle` and `at tick 10000 grazers >= 10 and hunters >= 2`, both printed as `N/A  <name>: n/a (animals off)`. `no_extinction` and `max_10x` keep their tree part, so a tree collapse in a garden run still fails. See `DECISIONS.md` (shot G0).
- Animals-off values on the three seeds: min trees 59/55/59; max trees 1163/1124/1272 (`max_10x` limit is 10× the first row); grass band [0.393, 0.889] / [0.395, 0.890] / [0.369, 0.891]; mature trees at 10000: 439/420/511.
- **Animals off is 13–15× faster on the strip** (1.7–1.9 s against 22–28 s). The speed numbers for 64×64, 256×64 and 256×256 are in `PERF.md` (shot G0 section).
