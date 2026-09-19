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
- **The s42 golden test regenerates the run.** `runs/s42` is gitignored, so "evaluate on `runs/s42/series.csv`" is tested against `tests/data/s42-check.txt`, the pre-refactor `ecosim check runs/s42` output. The test regenerates seed 42 in the target tmp dir, with snapshots every 10000; snapshots don't affect the series. (Since Test hardening, it shares the manifest test's run, with a snapshot every 100.)

## Test hardening

This section covers the property tests, the coverage floor, the lints, the CI gate and the behaviour manifest. No sim rule, `params.toml` default or file format changed.

`tests/data/s42-manifest.sha256` was generated from `runs/s42`, regenerated at 15ffa78. It holds the sha256 of `series.csv` and every snapshot file. Every run since then matches it: debug and release, before and after the refactors below. So this shot changed no behaviour.

**Build and tooling**
- **`[profile.dev] opt-level = 1`.** At opt-level 0 the debug suite is far over the 60 s budget, because the integration tests run real simulations. Opt-level 1 keeps debug assertions and overflow checks on and brings `cargo test` to about 25 s. Cross-profile determinism is still a real check, because opt-level 1 and opt-level 3 are different code generation.
- **`justfile`, not a Makefile.** `make` isn't installed on the Windows build machine. The recipes use bash, so on Windows `just ci` runs from Git Bash.
- **`rustfmt.toml`** sets `max_width = 120` and `use_small_heuristics = "Max"`, so the existing dense style needs few rewraps. `cargo fmt` was applied once to the whole crate.
- **CI pins toolchain 1.98.1**, the version used here, so a new clippy lint can't break an unchanged commit.
- **CI has an unnumbered step 2b** that runs `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"`. Clippy's `-D warnings` already catches missing doc comments (`#![warn(missing_docs)]` in `lib.rs`), but broken intra-doc links only show up under rustdoc.
- **CI step 8 checks determinism across build profiles.** It runs `ECOSIM_REQUIRE_CROSS=1 cargo test --release --test integration determinism`. Step 3 built the debug binary and step 5 the release binary, so the cross-profile test finds both.
  - A "drop cached binaries" step removes both `ecosim` executables right after the cache restore. Otherwise a binary left over from an older commit could let the cross test compare stale output.

**Proptest policy**
- **A plain `cargo test` uses a fresh random seed**, so local runs keep exploring new cases.
- **CI and `just` set `PROPTEST_RNG_SEED=20260918`**, so a CI failure reproduces exactly.
- **`proptest-regressions/` is gitignored.** A shrunk failing case becomes a hand-written, named `#[test]` next to its property instead. Every property has at least one such sibling.
- **Case counts are set per module with `ProptestConfig::with_cases`**, sized to the cost of each case:

  | Cases | What |
  |---|---|
  | 64 | cheap field and grid checks |
  | 32 | sim-stepping properties |
  | 24 | `evaluate` |
  | 16 | `--set` |
  | 12 | snapshot round trips |
  | 8 | `patch_dist` against a brute-force BFS |

  This keeps the debug suite under 60 s.
- **Coverage runs a lighter suite.** cargo-llvm-cov builds with `--cfg coverage`. Instrumented, the full suite took about 490 s of test time, which doesn't fit the 10-minute CI budget alongside the other steps. So under `cfg(coverage)` two things change:
  - Every property runs a quarter of its cases (at least 2), via `crate::cases`.
  - The five full-length run tests are `#[cfg_attr(coverage, ignore)]`: the s42 golden and manifest tests, both determinism tests, and hunters-disabled.

  Those tests still run in step 3, and the determinism test also in step 8. They reach no lines that the shorter tests miss. The floor wasn't lowered; see COVERAGE.md for the numbers under this regime.

**Test structure**
- **One shared seed-42 run.** The golden `check` test and the manifest test share one run through a `OnceLock` in `tests/sweep.rs`: seed 42, 20000 ticks, a snapshot every 100.
- **The golden test compares every non-runtime line** of `check` to `tests/data/s42-check.txt`, and asserts they all pass. The runtime line is only compared by name. Its value depends on the profile and the machine, and a debug run legitimately takes more than 30 s.
- **The manifest covers `series.csv` plus all 7 files of each of the 201 snapshots**, as specified. `meta.json` and `timing.json` are not hashed.
- **Cross-profile determinism is a test.** `determinism_debug_and_release_binaries_agree` runs this profile's binary and the sibling profile's binary (`target/{debug,release}/ecosim`), then byte-compares the two run dirs.
  - When the sibling binary hasn't been built, the test prints a skip note, unless `ECOSIM_REQUIRE_CROSS=1` is set.
  - It's a test rather than a CI shell diff so that `cargo test` reproduces it.
- **`tests/ci.rs` scans `ci.yml` and the justfile as text** rather than parsing YAML, which avoids a YAML dependency for one test.
  - It recognises each step by its command, not its name, and asserts the seven steps come in order.
  - It also checks the three artifact uploads, the cross step and the `Cargo.lock` cache key.

**Property design**
- **Grids are compared as sorted cell contents plus per-patch counts**, because cell order is an artifact of swap-removal.
- **Column light is tested with the `params.toml` absorb value (100) and with an arbitrary `u8`.** The SAD's example value 96 is a named regression.
- **Suitability-shape properties draw strictly increasing breakpoints.** Ties are a separate regression, which pins that a zero-width ramp returns 0.
- **Snapshot round trip.** Every field a snapshot records reads back, through a test-side reader, as the sim held it at that tick, at the stored precision.
- **`--set` round trip.** Each params leaf is set to a random integer in 0..=255, a range that fits every field type. The test asserts that the leaf changed and that no other leaf did.
- **`evaluate`.** Synthetic series are built to pass every invariant. The property breaks exactly one of 10 violable invariants and asserts that only that one fails. Runtime and run length aren't in the violable set, because they aren't functions of the series values.

**Refactors for testability**

All of these are behaviour-preserving, and the manifest is unchanged.
- **Extractions:**
  - `spawn_grazer`, from the grazer birth path, so the grid property can drive births.
  - `evaporate`, from the moisture update.
- **`Sim::bare`** is a test-only constructor from a height map.
- **Clippy-driven changes:**
  - `x % n == 0` became `x.is_multiple_of(n)`, which is identical for the nonzero divisors used.
  - `map_or(true, …)` became `is_none_or`.
  - Indexed loops became iterators.
- **No `#[allow]` was needed anywhere**, including `too_many_arguments`.
- **Dead code was deleted rather than tested.** Coverage found three `pub fn`s that nothing in the crate or its CLI called: `Params::from_toml_str`, `Sim::count_mature_trees` and `World::count_class`. All three were removed.

**Properties I tried to state and couldn't** (or stated only in a weaker form)
- **"Suitability is monotone on each ramp for any breakpoints."** Tied breakpoints make a zero-width ramp, and the implementation returns 0 at the tie. Stated for strictly increasing breakpoints, plus a tie regression.
- **"A sweep range has `(hi-lo)/step + 1` values for arbitrary floats."** The implementation adds 1e-9 slack before flooring, so a quotient just below an integer, like 0.3/0.1, still counts the intended endpoint. No closed formula holds over arbitrary floats. Stated over integer multiples of a decimal step, plus a regression for the near-integer case.
- **"Snapshot → restore gives an identical `Sim`."** A snapshot doesn't store the RNG state, the next id, cooldowns, `dry_ticks`, `trunk_at`, `canopy_cover` or the spatial grids, and it rounds moisture and fertility to `u8`. The files can't restore a sim that steps identically, and adding a restore path would be a format change. Stated instead as "every recorded field reads back as recorded".
- **"Light follows the formula with absorb 96."** 96 is the SAD's example, but the configured value is 100. It's tested as one regression case rather than as the property's parameter.
- **"Incrementally maintained hunter grid equals a rebuild."** The hunter grid is rebuilt from scratch every tick, so this is trivially true for hunters. It's only meaningful, and only stated, for grazers.
- **"The `check` verdict is a function of the series alone."** The runtime invariant reads wall time from `timing.json`, so a debug run of a passing seed fails it. Stated for every other invariant; the golden test compares the runtime line by name only.
