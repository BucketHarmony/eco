# ecosim performance baseline (shot 15a)

This file records what a tick costs at the start of the world's growth: 256×64 in shot 15, 256×256 in 17a. It measures and recommends but changes nothing; no sim code changed in this shot. The numbers come from `ecosim run --profile` (see "How it was measured").

## How it was measured

- **Machine.** Intel i9-12900KF (8 P-cores with 16 threads, plus 8 E-cores), Windows 11, Rust 1.98.1, release build.
- **Seed and cadence.** Seed 42 at the defaults (`params.toml`), with a snapshot every 100 ticks and `events.csv` on, as `ecosim run` writes by default. The 60000-tick run snapshots every 10000, like CI step 9. 100-tick snapshots would write about 1.2 GB.
- **Worlds.** 64×64 is the old square world (`--set world.width=64 --set climate.rain_gradient=0 --set world.slope_bias=0`). 256×64 is the default strip. 256×256 is the strip settings plus `--set world.depth=256`.
- **Pinned to the P-cores.** Every run was started with `cmd /c start /affinity FFFF /wait /b target\release\ecosim.exe run ...`.
  - Unpinned, Windows moves a long single-threaded process onto an E-core after about 3 s, and it then runs about 2× slower. The same 2000-tick in-memory run went from 0.49 s to 1.0 s partway through a loop of 15.
  - That is why shot 15 saw 44–59 s for the strip's 20000 ticks, against 24.5–27.7 s pinned here.
  - Any timing taken on this machine without pinning is unreliable, including `timing.json` and the runtime line of `ecosim check`.
- **Repeatability.** Pinned repeats vary by about 10%: 24.5 s and 27.7 s for the same 20000-tick strip run.

## Profile: 2000 ticks, seed 42

Each cell gives the milliseconds charged to the phase and, in brackets, its share of the total. The phases tile the run: their sum is within 0.1% of the total on every row (acceptance: within 5%).

| phase | 64×64 | 256×64 | 256×256 |
|---|---|---|---|
| setup (world, BFS distances, initial populations, meta.json) | 5 (0.9%) | 59 (3.3%) | 813 (10.4%) |
| **animals** | **463 (83.1%)** | **1496 (82.8%)** | **5972 (76.0%)** |
| immigration | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) |
| producers (grass, shrub) | 3 (0.5%) | 24 (1.3%) | 114 (1.4%) |
| trees | 0 (0.1%) | 1 (0.0%) | 1 (0.0%) |
| fire | 1 (0.2%) | 5 (0.3%) | 19 (0.2%) |
| moisture/fertility | 13 (2.3%) | 55 (3.1%) | 230 (2.9%) |
| temperature/season | 0 (0.0%) | 0 (0.0%) | 1 (0.0%) |
| compaction | 0 (0.0%) | 1 (0.0%) | 3 (0.0%) |
| stats row | 28 (5.1%) | 96 (5.3%) | 496 (6.3%) |
| snapshot write | 34 (6.2%) | 60 (3.3%) | 192 (2.4%) |
| events | 2 (0.4%) | 3 (0.2%) | 4 (0.1%) |
| series write | 6 (1.2%) | 6 (0.3%) | 6 (0.1%) |
| **total** | **558 ms** | **1806 ms** | **7859 ms** |
| ticks per second (whole run) | 3587 | 1107 | 254 |
| ticks per second (`Sim::step` alone) | 4159 | 1265 | 316 |
| grazers / hunters at tick 2000 | 867 / 36 | 3261 / 34 | 10263 / 38 |

## Full runs

| run | wall time | ticks/s | animals share | stats row | snapshot write |
|---|---|---|---|---|---|
| 64×64, 20000 ticks | 5.1 s | 3885 | 82.7% | 5.2% | 6.6% |
| 256×64, 20000 ticks | 27.7 s (24.5 s on a second run) | 723 | 89.3% | 4.0% | 2.6% |
| 256×64, 60000 ticks (snapshot every 10000) | 72.9 s | 823 | 91.2% | 4.4% | 0.1% |
| 256×256, 20000 ticks (measured) | 170.4 s | 117 | 92.8% | 2.6% | 1.8% |

**Projected 20000-tick wall time at 256×256: about 120 s**, measured at 170 s.
- The projection scales the 256×256 2000-tick total by the strip's ratio of 20000 ticks to 2000 ticks: 7.86 s × (27.7 / 1.81) ≈ 120 s.
- Scaling the strip's 20000-tick time by area gives about the same: 4 × 27.7 ≈ 111 s.
- The measured run took 170 s because it grows more animals than area alone predicts. It ends with 11517 grazers and 205 hunters, against 3097 and 74 on the strip, and animals are 93% of its time.
- This run doesn't pass `ecosim check` and doesn't have to; it is a measurement only. The shot-15 runtime limit is capped at 90 s, so a 256×256 world will fail the runtime line at today's speed.

The animal phase costs about the same per grazer on every world: 0.31 µs per grazer-tick on 64×64 (683 grazers on average), 0.42 µs on the strip (2946) and 0.38 µs over the 60000-tick strip run (2909). The model's cost is set by the grazer count, not by the terrain size.

## The three largest hotspots

These all sit inside the animal phase, which takes 76–93% of every run.
- **How the split was found.** The split below comes from a throw-away build that timed the calls inside `update_grazer` and `update_hunter` with `Instant` (20000 strip ticks, pinned). It wasn't committed.
- **Timer overhead.** The timers themselves added about 12 s to a 24.5 s run, so the shares are approximate. The overhead is spread evenly over the timed calls.
- **Sampling profilers.** No sampling profiler is installed on this machine. `cargo flamegraph` needs `perf` or DTrace.

The instrumented run spent about 34 s in the grazer loop, 0.2 s in the hunter loop and 0.17 s rebuilding the hunter grid. The grazer loop split as follows:

1. **The flee check, `nearest_hunter`: about 25% of grazer time.** Every grazer that isn't in a fire scans every column within its flee distance (radius 4, about 49 columns) of the hunter grid, every tick. There are only 35–75 hunters on 16384 columns, so almost every cell it reads is empty.
2. **Patch choice, `target_patch`: about 24%.**
   - Every grazer that isn't eating or fleeing scores up to 25 patches.
   - For each patch it reads `patch_dist[q · cols + c]` to test reachability. The table holds patches × columns `u16`s (8 MB on the strip), so the 25 reads land far apart and mostly miss cache.
   - It also allocates a new `Vec` for the scores on every call.
3. **Movement (`step_toward_patch` or `random_step`, then `move_grazer`): about 30% together.**
   - `valid_neighbours` allocates a `Vec` on every call.
   - `step_toward_patch` makes 8 more scattered `patch_dist` reads.
   - `move_grazer` scans the old column's cell to find and remove the grazer (`grid_remove`), and pushes it onto the new cell, a separate heap `Vec`.

**Other costs.**
- **Stats row: 4–6% of every run.** `Sim::stats` rebuilds the list of soil patches and soil columns from scratch every tick by scanning every column. Terrain never changes, so the lists could be cached.
- **Setup at 256×256: 0.8 s.** `patch_dist` is patches × columns, so it grows with the square of the area: 128 MB at 256×256, and 0.5 GB at 512×512. The BFS that fills it dominates setup. It is 10% of a 2000-tick run and 0.5% of a 20000-tick one, but its memory limits the world size before its time does.

## Recommendations (not implemented)

Estimated gains are shares of a strip run's wall time, from the split above.

| hotspot | change | byte-identical? | estimated gain |
|---|---|---|---|
| flee check | Before scanning, check `hunters_in_patch` over the patches that the grazer's flee box (x ± its `flee_distance`, y ± its `flee_distance`) touches, and return None when they are all 0. At the default distance 4 that box covers 1–4 patches of 8×8. This is the small, obviously safe fix: a few lines, and the scan and its tie-breaking are unchanged whenever a hunter might be in range. | yes | 15–20% |
| patch choice | Precompute a reachability bitmask per column for its 5×5 patch neighbourhood, a `u32` per column: one load instead of 25 scattered reads. Use a fixed-size array instead of the `Vec` of scores. Scores must still be computed per grazer, because `grazers_in_patch` changes as grazers move within a tick. | yes | 10–15% |
| movement | Have `valid_neighbours` return a fixed array plus a count, and precompute the 8 `patch_dist` neighbours of a column toward each reachable nearby patch. Alternatively, store `patch_dist` column-major (`c · patches + q`), so one column's distances to nearby patches share cache lines. | yes | 10–15% |
| stats row | Cache the soil-column and soil-patch lists in `World` (terrain is fixed). | yes | 3–4% |
| `patch_dist` memory | Keep distances only for patches within `search_patches` of each column's own patch (25 per column instead of all), stored per column. That is 3.3 MB at 256×256 instead of 128 MB, and makes the 256×256 setup linear in area. | yes | setup at 256×256 from 0.8 s to under 0.1 s; needed before worlds larger than 256×256 |

Together, the byte-identical changes should give roughly 1.6–2× on the strip. That would bring the 256×256 world near the 90 s runtime limit, but not reliably under it.

**Deterministic parallelism inside a tick.**
- **Per-patch producer updates with per-patch RNG streams:** not worth a shot. Producers are 0.5–1.4% of the time, so parallel producers would buy at most 1%.
- **The animal phase is the only place parallelism pays**, and it is sequential by design:
  - grazers act in `Vec` order and see the moves, grazing and deaths of the grazers before them;
  - every behaviour draws from one ChaCha8 stream in that order;
  - the grid and `grazers_in_patch` are updated in place.
- **What making it parallel means.** It would take a decide-then-apply tick. Each animal decides from the state at the start of the tick, drawing from its own stream derived from (seed, tick, animal id). Conflicts are then resolved in a fixed order: two grazers eating the last grass, two hunters attacking one grazer, a birth into a full patch.
- **What it costs.** It changes the model, not only the speed: simultaneous updates behave differently from sequential ones. Byte-identity with every existing run is lost.
  - `tests/data/s42-manifest.sha256`, `s42-check.txt` and the square-world `s42-64-manifest.sha256` would have to be regenerated.
  - The pre-shot identity manifests (`-prefire`, `-preshot10`, `-preshot11`, `-preshot14a`) can no longer be reproduced at all.
  - The regression anchor (seeds 1, 2, 3 and 42) would need checking again.
- **What it buys.** Up to about 6× on the 8 P-cores for the 93% animal share, so roughly 4–5× per run. That is on top of the 1.6–2× from the single-thread fixes.
- **Recommendation.** Do the byte-identical fixes first, as one shot with a manifest check that proves no byte moved. Revisit parallel animals only if single runs of the 17a world must fit a limit they then still miss.
  - Sweeps and seed matrices already run whole cells in parallel (`ecosim sweep --jobs`), and that parallelism costs nothing in determinism.

## The bench

`benches/tick.rs` runs 2000 ticks of seed 42 in memory, as a sweep cell does: steps and stats rows, no files. It runs on 64×64 and 256×64 under criterion, with 10 samples. It reports the median over every timed run as ticks per second and writes `ci-runs/bench-tick.json`.

The GitHub CI job `ecosim-bench` runs it and fails when a world is more than 20% slower than `benches/baseline.json`. The baseline holds that job's numbers from a CI run, not this machine's; `DECISIONS.md` (shot 15a) says how it was taken.

The baseline is 2827 ticks/s at 64×64 and 858 at 256×64 (CI run 35468533390). The runner is steady: criterion's interval on 13 samples of 64×64 was 705–708 ms. The bench step took 37 s and the whole job 1 min 36 s, well under the 3-minute limit.

For reference, pinned runs on this machine give about 4000 ticks/s at 64×64 and 1200 at 256×64. Unpinned, a long bench run lands on an E-core and reads 1.5–3× lower, so a local `just bench` compared against the CI baseline doesn't mean much.
