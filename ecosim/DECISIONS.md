# ecosim design decisions

These are calls made where neither the SAD nor the addendum settles a detail. Parameter values and their history are in `TUNING.md`.

## World

- **Height normalization is piecewise-linear around a quantile.** Two octaves of value noise (periods 32 and 8, amplitudes 1.0 and 0.35) are mapped as follows:
  - The lowest `world.water_fraction` of columns (param, 0.04) goes to `[height_min, water_level − 0.5]`.
  - The rest goes to `[water_level − 0.5, height_max]`.

  A plain min–max rescale left some seeds with no water, and those seeds had no pond wetting.
- **Walking distances are precomputed.** `World::patch_dist` holds, for every patch and column, the 8-neighbour BFS step count over soil columns to the nearest soil column of that patch. It is computed once, because terrain never changes.

## Grazer movement

The addendum's "best patch within Chebyshev 2, ties to lowest index" is kept, with three changes. Each fixes a failure seen in tuning (`TUNING.md`, rounds n–x):

- **Pathing uses the BFS distance field instead of a straight-line greedy step.** A straight step toward a target patch trapped grazers against rock ridges and water, and whole herds starved in place. A patch that can't be reached from the grazer's column is never a candidate.
- **Choice tolerance (`grazer.choice_tolerance`, 0.75).**
  - Every candidate patch scoring within the tolerance of the best counts as acceptable.
  - A grazer stays if its own patch is acceptable.
  - Otherwise it moves to the acceptable patch it personally prefers, using a fixed integer hash of (grazer id, patch index).

  With strict "ties to lowest index", every grazer in a region chose the same target. Streams of about 1000 grazers stripped one patch after another and the population boom-busted. The hash is deterministic and uses no RNG draws. With tolerance 0 the behaviour is the addendum rule.
- **`grazer.eat_min_grass` (0.0) is a knob** for the "grass > 0" test in the eat priority. It stays at 0, which is the addendum rule. Raising it made patches with near-zero grass stop trapping grazers, but produced deserts pinned at the threshold, so it was not adopted.

## Performance

- **Per-column animal grids.** `Sim::grazer_grid` and `Sim::hunter_grid` are `Vec<Vec<u32>>` of animal indices per column. Neighbour queries (flee, seek, attack) scan precomputed offset lists sorted nearest-first by (d², dy, dx), with the lowest index winning ties. This replaced O(hunters × grazers) scans once grazers numbered in the thousands. The results are identical to the brute-force scan: same nearest animal, same tie-break.
  - The grazer grid is maintained incrementally on move, death and birth, and rebuilt at compaction.
  - The hunter grid is rebuilt at the start of each animal update.

## Producers

- **Shrubs are an understory.** Their light curve is `[40, 100, 200, 254]`, so they do best in partial shade and fall off in full sun, and `shrub.g` is 0.004.
  - With the starting curve, shrub covered every patch by tick 2000. The refugium then protected all grazers everywhere and hunters starved.
  - With the understory curve, shrub > refugium mostly occurs under canopy, so refugia are patchy and move as the forest does.
- **`tree.mature_age` is 1000** (was 2000). With 2000, no seed produced 35 mature trees by tick 10000 while also leaving the ground bright enough for the renderer. See `TUNING.md` rounds f–g.
- Initial cover is `grass.initial = 0.10` and `shrub.initial = 0.02` on every soil patch, as the addendum gives.

## Checks

- **`ecosim check` applies the addendum's extra acceptance checks to every run directory**, not only `runs/s42`: at tick 10000, at least 35 mature trees, grazers ≥ 10 and hunters ≥ 2. This makes seeds 1–3 meet the same bar the renderer needs from s42.
- **The runtime check reads `timing.json`** (`wall_ms`, written by `ecosim run`). `timing.json` is the one file `ecosim diff` ignores, because wall time is not deterministic.
- Runs shorter than 20000 ticks fail check with a "run length" line instead of erroring out.

## Tests

- **The hunters-disabled test runs seed 42.** It uses the SAD's centred 200-tick moving averages, taking windows `[t, t+200)` for t in 8000..=19800.
- **The determinism test shells out to the built binary** (`CARGO_BIN_EXE_ecosim`), as the addendum allows. It uses a shortened 3000-tick run with snapshots every 500, then runs `ecosim diff`.
- **The "snapshot round-trips through the reader" test is in `tests/integration.rs`.** It reads the files back with serde_json and raw bytes and compares them with the in-memory `Sim`.

## Sweep harness

- **`season.amplitude` is the temperature swing moved out of `climate.temp_amp`.** The value (12) and the formula are unchanged, and the s42 golden check test confirms identical output. The requested range 0:12:3 matches that temperature amplitude.
  - Rain has its own seasonal term (`climate.rain_amp`, 4), which the key does not touch.
  - Sweep 5 is therefore run twice: once as specified, and once with `--set climate.rain_amp=0`, so "amplitude 0" also means no seasonal forcing at all.
  - Nothing in ecoview reads `meta.json` params, so the renamed key has no effect there.
- **Overrides go through the parsed TOML document before deserialization** (`params::apply_override`). `ecosim run` always takes this path, even without `--set`, so a sweep cell and a run build `Params` identically.
  - The key must already exist in the file.
  - The value is coerced to the existing type. An integer-valued float such as `500.0` is accepted for an integer key, and an integer for a float key, so ranges like `500:2500:500` work.
  - Arrays must keep their length.
  - Errors name the key and are raised before the run directory is created.
- **`meta.json` gains `overrides`** (the `--set` strings in order; `[]` without any). `format_version` stays 1. The committed `fixtures/s42-mini` was not regenerated, so its `meta.json` lacks the key; readers must treat it as optional.
- **The check refactor.**
  - `check::evaluate(&Series) -> Result<CheckReport, String>`. `Series` carries the rows plus the two facts that live outside `series.csv`: the tick-10000 mature-tree count (from the snapshot, or counted in memory by a sweep) and the timing (`Ms`, `Missing`, or `Excluded` for sweeps).
  - For compound invariants, `value`/`threshold`/`margin` come from the tightest part: the binding species for `no_extinction`, `max_10x` and `animals_10k`, and the tighter band side for the fertility and grass bands.
  - `no_extinction` uses threshold 1 (a minimum of at least 1).
  - `max_10x` reports the max count against the limit (10 × the tick-2000 count).
  - `grazer_cycle` reports the first-to-last maxima span against 1500, with 0 when there are fewer than 2 maxima.
  - A threshold of 0 gives margin 0 (pass) or −1 (fail).
- **The `check` margin column.** It is appended to each existing line as ` [margin ±x.xxxx]`; the rest of the line is unchanged.
- **Sweep cells are evaluated on their CSV text**, which is formatted and then parsed back exactly as `check` reads `series.csv`. Evaluating in-memory floats would put 4th-decimal differences into the band margins.
- **Invariant columns in `sweep.csv`** are the invariant keys that appear in at least one cell, in report order. `run_length` and `tick_10000` appear only in sweeps shorter than 20000 or 10000 ticks. `runtime` never appears.
  - `first_extinction_tick` uses the same rule as `ecosim stats`: the first tick at which any species is 0, over the whole run including the burn-in.
- **Grid order.** The first `--param` varies slowest and the last fastest; seeds vary fastest of all. Cells are written in this order no matter which thread finishes first.
- **Range values are strings.** They are formatted with the most precise input's decimal places (`0.04:0.16:0.02` → `0.04 … 0.16`, and `500:2500:500` → `500 … 2500`). The cell id, the CSV value and the `--set` string are therefore the same text, with no float drift.
- **`sweep.md` definitions.**
  - A value is *safe* when every cell with that value passes every invariant, across all seeds and all values of the other swept params.
  - The band is the contiguous run of safe values that contains the default, or the longest run if none does.
  - A band is *fragile* when it has fewer than 3 grid values, i.e. is narrower than two steps; no band at all is also fragile.
  - "First failing" at an edge is the invariant failing in the most cells at the neighbouring value, ties broken by the most negative margin. All other failures there are listed too.
- **Extra sweep options.** `sweep` also accepts `--set` (fixed overrides applied before the swept values) and `--params`. `--baseline` prints the margin table and exits 1 if any seed fails.
  - Sweep arguments are parsed by hand, because clap cannot keep the pairing of each `--param` with its following `--range`/`--values`.
- **Per-cell series (`sweeps/*/cells/`) are gitignored.** They total about 1.6 MB per cell and about 200 MB for the required sweeps, and rerunning the sweep regenerates them deterministically. `sweep.csv` and `sweep.md` are committed.
- **The s42 golden test regenerates the run.** `runs/s42` is gitignored, so "evaluate on `runs/s42/series.csv`" is tested against `tests/data/s42-check.txt`, the pre-refactor `ecosim check runs/s42` output. The test regenerates seed 42 in the target tmp dir, with snapshots every 10000; snapshots don't affect the series.
