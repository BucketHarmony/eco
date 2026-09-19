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
- **The sim's trigonometry goes through the pure-Rust `libm` crate.** The first CI run on Linux failed the manifest test: seed 42 diverged from the Windows run at tick 12300.
  - Cause: `f32::sin` and `f32::cos` call the platform's C math library (MSVC on Windows, glibc on Linux), and the two differ in the last bit for some inputs. The calls are in season temperature, rain, and seed-dispersal angles. One flipped `.round()` in seed placement moves a tree, and the runs diverge from there.
  - Fix: `libm::sinf` and `libm::cosf` give the same bits on every platform. On Windows they reproduce the old output exactly: the s42 manifest still matches, and seeds 1–3 are byte-identical. So `runs/s42`, the fixtures and ecoview's copy stay valid, and the manifest was not regenerated.
  - Guard: `clippy.toml` disallows `f32::{sin, cos, tan, exp, ln, powf}`, so a new platform math call fails step 2. `sqrt`, `powi` and basic arithmetic are exact IEEE operations and stay as they are.
- **Dead code was deleted rather than tested.** Coverage found three `pub fn`s that nothing in the crate or its CLI called: `Params::from_toml_str`, `Sim::count_mature_trees` and `World::count_class`. All three were removed.

**Properties I tried to state and couldn't** (or stated only in a weaker form)
- **"Suitability is monotone on each ramp for any breakpoints."** Tied breakpoints make a zero-width ramp, and the implementation returns 0 at the tie. Stated for strictly increasing breakpoints, plus a tie regression.
- **"A sweep range has `(hi-lo)/step + 1` values for arbitrary floats."** The implementation adds 1e-9 slack before flooring, so a quotient just below an integer, like 0.3/0.1, still counts the intended endpoint. No closed formula holds over arbitrary floats. Stated over integer multiples of a decimal step, plus a regression for the near-integer case.
- **"Snapshot → restore gives an identical `Sim`."** A snapshot doesn't store the RNG state, the next id, cooldowns, `dry_ticks`, `trunk_at`, `canopy_cover` or the spatial grids, and it rounds moisture and fertility to `u8`. The files can't restore a sim that steps identically, and adding a restore path would be a format change. Stated instead as "every recorded field reads back as recorded".
- **"Light follows the formula with absorb 96."** 96 is the SAD's example, but the configured value is 100. It's tested as one regression case rather than as the property's parameter.
- **"Incrementally maintained hunter grid equals a rebuild."** The hunter grid is rebuilt from scratch every tick, so this is trivially true for hunters. It's only meaningful, and only stated, for grazers.
- **"The `check` verdict is a function of the series alone."** The runtime invariant reads wall time from `timing.json`, so a debug run of a passing seed fails it. Stated for every other invariant; the golden test compares the runtime line by name only.

## Dynamics fixes

These are the calls made in the shot-5 "dynamics fixes" brief. Tuning is in `TUNING.md` ("Dynamics fixes (shot 5)"), and the sweep results are in `sweeps/shot5/FINDINGS.md`.

**Continuous refugium (stability rule 3)**
- **The formula.** `hunter.refugium_shrub` is gone. An attack now succeeds with probability `kill_prob · (1 − shrub)^refugium_k` (`animals::attack_success`), using the shrub of the grazer's patch.
  - Every live grazer is a legal target. A failed attack keeps its costs: `fail_cost` plus displacement of the grazer.
  - The addendum's line "shrub ≤ `refugium_shrub` triggers one attack" is superseded by this rule. The addendum itself was not edited, because the brief scopes this shot to ecosim.
- **Precision.** `refugium_k` is an `f32` param like the other knobs. The power is computed in `f64` with `libm::pow`, with `1 − shrub` clamped to [0, 1] and the result clamped to [0, 1], then passed to `gen_bool`.
  - `libm::pow(0, 0) = 1`, so k = 0 switches the refugium off even at shrub 1. A regression test pins this.
- **The test rewrite.** The refugium and satiation tests were rewritten:
  - Shrub lowers success but never forbids the attempt.
  - Success equals `kill_prob` at shrub 0 and is monotone non-increasing in shrub (a property).
  - The nearest grazer is targeted whatever its shrub.
  - A satiated hunter rests at any shrub.

**Immigration floor**
- **Reused keys.** The brief's `hunter.initial` is the existing `hunter.start_count`. The brief's tree "lifespan" mean is the existing `tree.max_age`. No keys were duplicated.
- **Timing.** Immigration runs as its own phase right after animals: animals → immigration → producers → … It fires on ticks where `t % immigration_interval == 0` (tick 0 is never stepped).
  - The live count is taken after that tick's deaths and births. The grazer check runs before the hunter check.
  - An immigrant acts from the next tick.
- **The immigrant.** It has `start_energy` (60, the brief's value), age 0 and cooldown 0.
  - It is placed on a uniformly random edge soil column (x or y at 0 or 63), chosen from the edge-soil list in index order with one `gen_range` draw.
  - If no edge column is soil, it falls back to any soil column.
  - There is one draw per immigrant, and none when the floor isn't binding.
- **Grazers.** They get the same two params with floor 0, which is off.
- **Output.** `series.csv` gains a last column, `hunter_immigrants`, the cumulative count. `sweep.csv` gains `hunter_extinction_tick` (the first tick with hunters = 0) and `hunter_immigrants` (the final count).
  - `sweeps/cycle_ratio.py` skips both columns. It also prints the hunter count at each qualifying grazer peak.
- **At the defaults the floor never fires.** Hunters stay at 24 or more on seeds 1–3. It does its job at the edge of the `refugium_k` band; see FINDINGS.

**Tree lifespan jitter**
- **The draw.** The lifespan is `round(max_age · (1 + lifespan_jitter · u))`, with u uniform in [−1, 1]. It is drawn in `plant_tree`, which covers both world generation and germination, and stored per tree as `Tree::lifespan`.
  - The draw happens even when jitter is 0, so the RNG stream doesn't depend on the jitter value.
  - A tree dies at `age >= lifespan`, replacing `max_age`.
- **Format.** `entities.json` tree records gain `lifespan`. `format_version` stays 1: it is an added field. ecoview's loader reads only the fields it types, and it finds `series.csv` columns by header name, so neither addition affects it.

**Canopy self-thinning**
- **The literal rule can't fire, so it was generalised.** The literal rule is "a mature tree whose trunk column is under ≥2 other canopies". A mature canopy covers the 3×3 around its trunk, but `min_spacing = 2` keeps trunks at Chebyshev distance ≥ 2. So no trunk column is ever under another tree's canopy, and the literal rule would never fire.
  - `Sim::crowding(i)` instead counts the other live trees whose canopy lies over any column of tree i's 3×3 crown: mature trees within Chebyshev 2 and young trees on a crown column. Saplings cast no canopy.
  - A mature tree with `crowding ≥ 2` dies with probability `tree.crowding_mortality` at each tree update.
- **When the draw happens.** The check runs after the tree's age, dry-tick and stage update, and only if it survived those. The RNG draw is made only for crowded mature trees, and only when `crowding_mortality > 0`.
- **Death.** It goes through `kill_tree`, which adds `death_detritus` and recomputes the light of the crown columns. Later trees in the same update see the thinned stand.

**Checks**
- **`max_10x` anchor.** Trees are now anchored at tick 5000 (`TREE_ANCHOR`); animals stay at tick 2000. The report line reads "no species exceeds 10x its anchor count (animals: tick 2000, trees: tick 5000)". Runs shorter than 5000 ticks anchor trees at their last tick.
- **`ecosim check --long`** is a separate report (`check::evaluate_long`). It replaces the 20000-tick invariants rather than extending them. Its lines:
  - `long_no_extinction`: grazers, hunters and trees are > 0 at every tick of the whole run.
  - `long_band`: raw grazer and hunter counts over ticks 20000–60000 stay within [0.2×, 5×] of their raw tick-20000 count. No smoothing is applied, which is the literal reading of the brief.
  - `long_run_length`: appears, and fails, only for runs shorter than 60000 ticks.
  - The 20000-tick invariants aren't applied to long runs because some of them don't hold there. For example, `fertility_mean` reaches its cap of 255 by about tick 30000 and would fail `fertility_band`.
- **`parse_series`** requires the new 11-column header. Run directories written before this shot, including ecoview's copies, no longer pass `ecosim check`.

**Tuning outside the brief's listed params**
- To get the required bands, several params outside the brief's list were changed: `grazer.start_count` 300, `hunter.fail_cost` 0.25, `hunter.cooldown` 5000 and `grazer.max_grazers_per_patch` 5. No invariant was changed. Each change is logged, with its evidence, in `TUNING.md`.

**Hunter regulation: what holds hunter numbers (a finding, not a fix)**
- **Hunters are not regulated by prey.** After the start they climb from 20 to about 35–55 and stay flat on every seed, at any season amplitude, through grazer swings of 3–9× (`sweeps/shot5/FINDINGS.md`).
- **The ceiling is demographic.** A hunter can breed at most once per `hunter.cooldown` (5000 ticks) and lives at most `max_age` (8000). So each hunter has one or two litters in its life whatever it eats, and births cap the count.
- **The floor is cheap predation.** Kills run at a steady low rate: continuous refugium, satiation, and a failed attack costing only `fail_cost` 0.25. A prey boom can't raise births (cooldown), and a prey bust hasn't starved hunters fast enough to show in the counts. This is inferred from the counts. The death causes shot 5 adds will show whether old age or starvation actually binds.
- **Consequence.** Predation is a near-constant drain on grazers, not a driver of their cycle. The grazer oscillation is grazers against grass, paced by season. Direction 1 wants hunter numbers set by prey, so the backlog moves this to shot 10: energy-gated reproduction with a short `hunter.refractory` in place of the long fixed cooldown, under the anchor rule. It is not tuned here.

**Platform math**
- `clippy.toml` now also disallows `f64::{sin, cos, tan, exp, ln, powf}`. The only new sim math is `libm::pow` in `attack_success`.
- Two test helpers in `check.rs` that built synthetic sine series moved to `libm::sin`.

**CI**
- Step 9 (`just long`) runs seed 1 for 60000 ticks with `--snapshot-every 10000` and runs `ecosim check --long` on it. It takes about 20 s here, which fits the 10-minute job, so no `nightly.yml` was needed.
- `tests/ci.rs` now requires 8 command-identified steps, adding `check --long ci-runs/long-s1`.

**Regenerated artifacts**
- These rules change behaviour on purpose. One commit regenerates:
  - `runs/s42`
  - `fixtures/s42-mini`
  - `tests/data/s42-manifest.sha256`
  - `tests/data/s42-check.txt`
- **ecoview's fixture is stale.** `ecoview/public/` still holds the pre-shot s42 run and mini fixture. They remain valid format-1 data, just older behaviour, and their tree records have no `lifespan`. This shot doesn't touch the renderer; refresh the copy with `bash scripts/sync-data.sh` in a renderer session.
