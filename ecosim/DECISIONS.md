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
- **Snapshot round trip.** Every field a snapshot records reads back, through a test-side reader, as the sim held it at that tick, at the stored precision. (Shot 7 replaced this property with "restore steps identically for 500 ticks"; see "Full-state snapshots and fork".)
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
- **"Snapshot → restore gives an identical `Sim`."** A snapshot doesn't store the RNG state, the next id, cooldowns, `dry_ticks`, `trunk_at`, `canopy_cover` or the spatial grids, and it rounds moisture and fertility to `u8`. The files can't restore a sim that steps identically, and adding a restore path would be a format change. Stated instead as "every recorded field reads back as recorded". Shot 7 made that format change (`state.bin`), and the property is now stated in full.
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

## Extinction attribution (shot 05)

This shot records why every animal dies and reports what drove each extinction. No rule that moves an animal, or that decides whether one lives or dies, changed. The sweep results are in `sweeps/shot05/FINDINGS.md`.

**Death causes**
- **The five causes.** `animals::Cause` has `starved`, `eaten`, `old_age`, `crowded` and `burnt`.
  - `crowded` is reserved for shot 10 and `burnt` for shot 9. No rule records them yet, so their columns are always 0.
  - A property test asserts that.
- **Precedence.** An animal whose energy reaches 0 on the tick it also reaches `max_age` is counted once, as `starved`. The check was already `energy <= 0 || age >= max_age`, so this only names the cause and changes no outcome. A named regression pins it.
- **Where each cause is recorded.**
  - `eaten` is recorded by the successful-attack branch of `attack`, through `kill_grazer(i, Cause::Eaten)`.
  - `starved` and `old_age` are recorded at the end of each animal's update.
  - Hunters have no predator, so their `eaten` column is always 0.
- **Counting.** `Sim::deaths` holds the counts for the current tick. `step` clears it first thing, and `stats()` copies it into the row. The counts are per tick, not cumulative, so a window sum is a plain sum. Tick 0 is all zeros.

**Series format**
- **Ten columns are appended to `series.csv`**: `grazer_starved`, `grazer_eaten`, `grazer_old_age`, `grazer_crowded` and `grazer_burnt`, then the same five for `hunter_`.
- **Header check.** `parse_series` requires the 21-column header, so run directories written before this shot no longer pass `ecosim check`. That includes ecoview's copies: refresh them with `bash scripts/sync-data.sh` in a renderer session.
- **No format-version bump.** `format_version` stays 1: the columns are added, and ecoview finds `series.csv` columns by header name.

**The property: counts sum to deaths**
- **How deaths are counted independently.** The property compares the recorded causes with an independent death count. It sets `world.compact_every = u32::MAX`, so dead animals stay in their Vec; the deaths of a tick are then the growth of each species' dead count.
- **Why not compare live-id sets before and after a tick.** A grazer born on a tick can be eaten by a hunter on the same tick (grazers update before hunters). That grazer is missing from the "alive before" set, so an id-set comparison would undercount its death.
- **How the property drives all three causes.** It draws energy costs, max ages and `kill_prob` so that starvation, predation and old age all occur within a few hundred ticks. Its named sibling `death_causes_regression_every_cause_in_one_run` asserts that all three occur.

**`ecosim stats`**
- **One line per species that reaches 0**, over the whole run including the burn-in, in tick order. This is the same rule as `first_extinction`.
- **The window.** For grazers and hunters the line gives the deaths by cause over ticks `[t − 499, t]`, which is 500 ticks including the extinction tick itself. That tick's row holds the deaths that emptied the population.
- **The dominant cause** is the cause with the most deaths in the window. Ties go to the first cause in `Cause` order. With no deaths in the window it is `none`, for example when a species starts at 0.
- **The food supply.** The line also gives the mean food over the same window:
  - for hunters, the mean grazer count, since there is no per-patch prey density to report
  - for grazers, the mean `grass_mean`
- **Trees** reach 0 with a plain line: their death causes aren't recorded, and the shot doesn't ask for them.

**`ecosim sweep`**
- **New CSV columns.** `sweep.csv` gains `first_extinction_species` and `first_extinction_dominant_cause`, right after `first_extinction_tick`.
  - Species that reach 0 on the same tick are joined with `+`, in the order grazers, hunters, trees.
  - The cause is that of the first species listed; for trees it is `unrecorded`.
  - `cycle_ratio.py` skips both columns.
- **New `sweep.md` section.** It ends with "Extinctions by cause", which counts cells three ways:
  - cells failing an invariant
  - cells with any extinction
  - the two cross-counts: failing cells without an extinction, and extinction cells that pass

  It then lists the extinction cells grouped by (first species, dominant cause).
- **Extinction and failure stay separate on purpose.** An extinction in the burn-in, or one that immigration reverses, need not fail `no_extinction`. A failing cell need not have an extinction: at amplitude 0 every failure is `fertility_band`.
- **`no_extinction` in `ecosim check` is unchanged.**

**Limitation: the window cause is the last animals' cause**
- Near an extinction only the last 1–4 hunters are left, so the 500-tick window names what killed them.
- In `hunter.refugium_k = 3.0` on seed 2 the last hunter died of old age. The window cause is therefore `old_age`, although over that run starvation killed 45 hunters and old age 20.
- The report states the window rule. FINDINGS gives the run totals beside it rather than changing the rule.

**What the causes show about hunter regulation**
- The "Hunter regulation" finding above left open whether old age or starvation binds hunters at the defaults. **It is old age.**
- On seeds 1, 2, 3 and 42, 75–88 of each seed's 75–91 hunter deaths are `old_age`, and 0–3 are `starved`.
- The sweep shows starvation taking over only as `refugium_k` approaches the collapse edge. The details are in `sweeps/shot05/FINDINGS.md`.

**`hunter.immigration_floor` 8 → 0**
- Confirmed before any code change. With the pre-shot binary, seeds 1, 2, 3 and 42 were run for 20000 ticks at floor 8 and at floor 0, and `ecosim diff` reported only `meta.json` for each seed.
- Every series and every snapshot was byte-identical, with 0 immigrants in each run.
- The floor code and its tests stay. `Sim::bare` and the hunters-disabled test already set the floor to 0.

**The four anchor values**

These values are the regression anchor for seeds 1, 2, 3 and 42. They were chosen so that those seeds persist at the defaults, and they are not a claim about the model. The evidence for each is in `TUNING.md` ("Dynamics fixes (shot 5)"). This shot doesn't touch them, and later shots shouldn't retune them to make a new mechanism pass.

| param | value | what it holds up |
|---|---|---|
| `grazer.start_count` | 300 | With 20 hunters and no shrub yet, 60 grazers are hunted out by tick 1100–2500 on every seed (r1, r3). |
| `hunter.fail_cost` | 0.25 | Attack success at the typical shrub is about 0.02–0.06, so a 2-energy miss starved hunters (r2, r4–r6). |
| `hunter.cooldown` | 5000 | At fail cost 0.25, cooldown 3000 lets hunters overshoot and crash (r6). This value is also what makes old age the binding hunter death. |
| `grazer.max_grazers_per_patch` | 5 | At 8, the 60k-tick grazer swings failed `check --long` on seed 1 (L1–L3). |

**Regenerated artifacts**

The series gained columns, so one commit regenerates the following:
- `runs/s42` and `runs/long42`, both gitignored
- `fixtures/s42-mini`, whose `series.csv` has the new columns and whose `meta.json` has the floor at 0 and the `overrides` key
- `tests/data/s42-manifest.sha256`: only the `series.csv` line changed, and all 1407 snapshot hashes are unchanged
- `tests/data/s42-check.txt`: only the runtime line changed

With the ten new columns stripped, the regenerated seed-42 `series.csv` is byte-identical to the pre-shot one.

## Full-state snapshots and fork (shot 7)

This shot adds `state.bin` to every snapshot, `Sim::restore`, and `ecosim fork`. No sim rule or `params.toml` default changed, and every file a run wrote before this shot keeps its exact bytes.

**`state.bin`**
- **What it holds.** The layout is in the `state.rs` module doc. It holds everything a `Sim` needs that the other snapshot files don't hold exactly:
  - the tick, the next id and the immigrant count
  - the RNG as seed, stream and word position (`rand_chacha`'s own accessors, so the restored stream continues at the exact word)
  - this tick's deaths, which the tick's `series.csv` row reports
  - moisture and fertility as `f32`, since the `.bin` files round them to `u8`
  - the patch fields as `f32` bits
  - every tree and animal with all its fields, in `Vec` order, dead ones included
  - the grazer grid, cell by cell
- **Patches and entities are stored again, bit for bit.** `patches.json` and `entities.json` hold these values as decimal text. Getting the exact `f32` back would depend on the JSON reader's float parsing (serde_json parses to `f64`, then narrows), and animals' cooldowns aren't in `entities.json` at all. So restore never reads the JSON files.
- **Dead entities and Vec order are kept.** Between compactions, `trunk_at` and the grids hold indices into the Vecs, and updates run in Vec order. A compacted Vec would renumber them.
- **The grazer grid is stored, not rebuilt.** Swap-removal leaves each cell in an order that a rebuild in index order doesn't reproduce, and nearest-neighbour scans visit cells in that order.
- **Recomputed on load, each tested bit-identical by `prop_restore_steps_identically`:**
  - the terrain fields (`ground`, `class`, `patch_soil`, `patch_dist`), rebuilt from the topmost solid voxel of `material.bin` through `World::from_heights`; the result must reproduce `material.bin` and `height.bin`, or restore fails
  - `light` read from `light.bin`, which is exact
  - `trunk_at` from the live trees
  - `canopy_cover` from `canopy_z`
  - `grazers_in_patch` from the live grazers
  - the seek and flee offset lists from the params
  - the hunter grid, rebuilt. The sim only reads it after `update_animals` rebuilds it, so the test compares it as rebuilt on both sides.
- **Versioning.** `state.bin` starts with the magic `ECOSTATE` and its own layout version (1), independent of `format_version`. The decoder rejects bad magic, other versions, truncation, trailing bytes, bad flags, off-world positions and grid entries that aren't live grazers.
- **`format_version` is 2.** `meta.json` gains `forked_from`, which is null for a run started at tick 0. Nothing else in `meta.json` changed.
- **`ecosim run --snapshot-state false`** writes no `state.bin`. The directory is still format 2, and `fork` refuses it, naming the missing file.

**`ecosim fork`**
- **Params.** The parent's params are read back from its `meta.json`. They are then written out again, and the result must equal what the parent recorded, which proves no float moved in the JSON round trip. The fork's `--set` overrides are applied to them through the same `apply_override` as `run --set`, and they take effect from the first step after the fork tick.
- **The terrain is rebuilt with the parent's params,** before the overrides. A fork that overrides a terrain key (`world.height_*`, `soil_depth`, `rock_top_height`, `water_level`, `water_fraction`) keeps the parent's terrain, because terrain is generated once at tick 0.
- **The fork directory is a complete run directory.** It holds the parent's `series.csv` rows and snapshot directories before the fork tick, copied verbatim. Everything from the fork tick on is simulated from the restored state, including that tick's row and snapshot.
  - So `check`, `stats` and the renderer see the whole history: `check` needs the tick-10000 snapshot, which a fork at 12300 would otherwise lack.
  - With no overrides, a fork run to the parent's end differs from the parent only in `meta.json`.
  - The fork's snapshot at the fork tick is written with the fork's params, so an override of a stage age changes the stages in that tick's `entities.json`. The state itself is the parent's.
- **`meta.json` of a fork.**
  - `ticks` is the fork's last tick (`--at` + `--ticks`).
  - `snapshots` lists the copied snapshots and the new ones.
  - `overrides` holds only the fork's own `--set` strings. The parent's overrides are already in the params it recorded.
  - `forked_from` is `{run, tick}`, with the parent path as given, in forward slashes.
- **Refusals, all before anything is written:**
  - format_version 1, with a message saying to rerun it
  - an unsupported format_version
  - a tick with no snapshot
  - a missing `state.bin`
  - a `series.csv` with a different header or fewer rows than the fork tick
  - params that don't read back exactly
  - an unknown or ill-typed override
  - `--out` equal to the parent
- **Version-1 directories** still work with `check`, `stats` and `diff`, which never read `format_version`. A test runs all three on the committed v1 fixture.

**Tests**
- `fork_matches_the_uninterrupted_run_from_the_fork_tick_on` (`tests/sweep.rs`) is the acceptance property. It forks seeds 1 and 42 at ticks 100, 5000, 12300 and 19900 through the CLI, runs each to 20000, and requires `ecosim diff` against the uninterrupted run to report only `meta.json`. It reuses the shared seed-42 run, runs the eight forks in parallel, and is skipped under coverage like the other full-length tests.
- `prop_fork_equals_uninterrupted` states the same thing for random seeds and fork ticks on 400-tick runs. Its sibling pins forks at the first and last snapshot.
- `prop_restore_steps_identically` replaces shot 4's snapshot round-trip property. A sim is restored from its own snapshot at a random tick. It must equal the original field for field, recomputed fields included, and the two must produce identical stats rows and identical state for 500 more ticks. Its siblings pin tick 50 (the first tree update) and tick 1234 on seed 7 (dead entities not yet compacted).
- `fork_with_overrides_changes_only_the_future` checks four things: the rows up to the fork tick are unchanged, `meta.json` records the fork, the overrides take effect, and a fork of a fork continues exactly.

**Regenerated artifacts**
- **`tests/data/s42-manifest.sha256`:** 201 `state.bin` lines were added, and none changed or were removed (`git diff` shows 201 insertions, 0 deletions). The fresh run's other 1 + 201 × 7 hashes were compared before the file was rewritten. The manifest test now expects 8 files per snapshot, 201 of them `state.bin`.
- **`fixtures/s42-mini` was not regenerated.** It stays a format-1 fixture, and the v1 test uses it.
- **`fixtures/s42-mini-v2` is new,** made by the same command (`--seed 42 --ticks 100 --snapshot-every 100`). A test asserts that it differs from the v1 fixture only by `meta.json`'s version and `forked_from`, plus the two `state.bin` files. The renderer's v2 shot can use it.
- `tests/data/s42-check.txt` is unchanged.
- **`runs/s42` (gitignored) was regenerated as format 2,** so that it can be forked. ecoview's copy in `ecoview/public/` is still the format-1 run. This shot doesn't touch the renderer, and the renderer's shot 8 teaches it format 2.

**Size and speed.** `state.bin` is about 100 KB for seed 42 at tick 10000, so that snapshot grows from 452 KB to 554 KB. A 20000-tick release run of seed 42 takes 6.1 s, and the fork demo takes 2.6 s. Both are well inside the 30 s runtime invariant.

## Fire (shot 09)

**Where fire sits in the tick**
- Tick order: animals → immigration → producers → trees (every `tree.update_every`) → **fire (every tick)** → soil (every 10) → temperature (every 100) → compaction.
- Fire runs after producers so that burn-out zeroes grass and shrub after that tick's growth. That makes "grass = shrub = 0 the tick after burn-out" hold.
- It runs before soil, so the ash lands in the fertility that the next soil update diffuses.
- Animals act first in each tick, so they take damage from the burning state left by the previous tick.

**Fuel** (`Sim::fuel`)
- Fuel is grass×0.5 + shrub×1.0 + detritus×`detritus_weight` + canopy_fraction×`canopy_weight`, where canopy_fraction = canopied columns / 64 (the same count the temperature update uses, now `canopy_columns`).
- A patch with no soil columns (all water or all rock) has fuel 0 whatever its detritus or canopy. So it never ignites, and fire never spreads into it.
- A mixed patch burns, but burn-out touches only its soil columns: fertility and ash go to those columns, and detritus scales by their count.

**Ignition, spread and burn-out** (`Sim::update_fire`)
Each tick the phase works in this order:
1. Collect the patches burning at the start of the phase.
2. Each of them rolls against each non-burning 4-neighbour, in the order +x, −x, +y, −y. There is no roll when the probability is 0, so fuel-0 neighbours and spread 0 draw nothing.
3. The collected patches count down, and a patch that reaches 0 burns out.
4. Every 10 ticks, when `base_rate` > 0, there is one draw per patch in patch order. A non-burning patch ignites when the draw is below p.

Consequences of that order:
- A patch lit by spread starts counting down on the next tick, so it burns exactly `duration` ticks.
- A patch that has burnt out can be relit at once. Its fuel is then only detritus and canopy, which is small but not zero.
- `duration` 0 is read as 1, so an ignited patch always burns out once.

Probabilities:
- The temperature ramp f(T) is linear from `temp_min` (15 °C) to `temp_full` (30 °C), both exposed as params so the tests can force ignition.
- The ignition p is clamped to [0, 1].
- Moisture is the patch's mean surface moisture, the same as the grass-growth input.
- The rate-0 check sits outside the patch loop, so at base_rate 0 nothing ever burns and the fire phase makes no draws and no writes.

Burn-out:
- Tree deaths go through `kill_tree` (now `pub(crate)`), which updates light and canopy cover for the 3×3 columns the same way age deaths do. That is the "light is recomputed for the affected columns" requirement.
- There is one `tree_kill` draw per live tree in the patch, in Vec order, and none when `tree_kill` is 0.

**Animals**
- An animal in a burning patch loses `animal_damage` energy that tick before anything else. It then flees: one greedy step directly away from the patch centre.
- Fleeing fire comes before fleeing hunters (grazers) and before satiation or hunting (hunters). Hunters flee too, because the rule says "animals".
- A death at energy ≤ 0 in a burning patch records `burnt`. Burnt takes precedence over `starved`, since the fire damage is what took the energy below 0 that tick.
- The death-cause property used to assert that `burnt` and `crowded` are both 0. It now asserts only `crowded` is 0, since `burnt` is a normal cause.

**Outputs**
- **series.csv** gains `patches_burning` (patches burning after the fire phase) and `total_burnt` (cumulative burn-outs) as its last two columns, giving 23 fields.
  - `check`, `stats` and `sweep` also accept the 21-field pre-fire header and read the fire columns as 0, so version-1 and shot-8 run directories still check.
- **patches.json** gains `burning_ticks_left` per patch, as its last field.
- **`format_version` stays 2.** Both changes only append fields, which JSON and CSV readers keyed by name ignore.
- **state.bin** goes to `STATE_VERSION` 2: layout v1 plus a fire section of `total_burnt` (u32) and 64 × `burning_ticks_left` (u32).
  - Restore rejects v1 `state.bin`, so a pre-fire run directory can no longer be forked. Rerun it instead; shot-7 runs were throwaway.
  - The fixture was regenerated, so no committed file carries v1 state.

**Tests**
- There are four fire properties in `src/fire.rs`, each with a named regression sibling:
  - fuel is 0 on water and rock
  - spread never crosses water: a full water column band, 400 ticks at certain spread
  - a burnt patch is bare the tick after burn-out
  - ignition is monotone in fuel and temperature
- Unit tests pin:
  - the one-draw-per-patch rule on ignition ticks only
  - no draws and no writes at rate 0
  - the burn-out effects
  - certain spread to the four neighbours
  - animal damage, fleeing and `burnt`
- **Rate-0 identity:** `fire_off_reproduces_the_pre_fire_manifest` runs seed 42 at `fire.base_rate=0` and compares it with the pre-shot manifest, kept as `tests/data/s42-manifest-prefire.sha256`. Each file first passes through `tests/common::without_fire`, which strips the two series columns, the `burning_ticks_left` field and the state.bin fire section, asserting they are all zero, and sets the state version back to 1. The run is byte-identical otherwise.
- `format_2_and_fire_only_add_to_version_1_files` checks the fixtures:
  - A fire-off 100-tick run, passed through `without_fire`, still matches the v1 fixture `s42-mini`.
  - A default run matches the regenerated `s42-mini-v2`.
- `forced_fire_extinction_runs_to_the_end_and_is_attributed_to_fire` runs at base_rate 1, a ramp from −50 to −40 °C (always hot) and damage 100. It checks:
  - both animal species die out, with `burnt` as their dominant cause
  - the run reaches 20000 with valid snapshots and no NaN
  - the last snapshot restores
- The forced-starvation test now also sets `fire.base_rate=0`. Otherwise fire deaths change its window's dominant cause.

**Regenerated artifacts (fire changes behaviour at the defaults)**
- `tests/data/s42-manifest.sha256`: 1609 lines, same file set. The snapshots before the first fire are unchanged.
- `tests/data/s42-check.txt`: seed 42 still passes every invariant, with 61 mature trees at tick 10000.
- `fixtures/s42-mini-v2`
- `runs/s42` (gitignored)
- `fixtures/s42-mini` (v1) is unchanged.
- ecoview's copy in `ecoview/public/` is now stale: it lacks the fire columns and field, and an ecosim shot doesn't edit ecoview. A renderer shot has to rerun `scripts/sync-data.sh`.


## Density-dependent mortality and hunter regulation (shot 10)

**Crowding mortality (`[disease]`)**
- **The rule.** Each animal update, after the starvation, burn and old-age check, an animal in a patch holding n of its own species (itself included) dies with p = `rate · max(0, n − threshold) / threshold` (`animals::crowding_death_p`), clamped to [0, 1]. Grazers use `disease.grazer_rate` and `grazer_threshold`, hunters `hunter_rate` and `hunter_threshold`.
- **Which n.** The count is taken after the animal's move this update, so it is the patch the animal ends up in. Grazers already had `grazers_in_patch`, kept current through moves, births and deaths.
- **Hunters get `hunters_in_patch`.** It is rebuilt with the hunter grid at the start of the animal phase and kept current as hunters move, die, give birth and immigrate. It is derived state: `state.bin` doesn't store it, restore recomputes it, and `prop_restore_steps_identically` compares it. The death-cause property checks it against a recount after every tick.
- **Death.** A crowding death records `crowded` and adds the species' corpse detritus, through `kill_grazer` or the new `kill_hunter`. A crowded animal doesn't reproduce that tick.
- **Draws.** `update_animals` checks each species' rate once, before its loop. At rate 0 the per-animal check is skipped entirely, so there are no draws and no writes. At a positive rate there is one `gen_bool` draw per animal in a patch above the threshold, and none at or below it, where p is 0.
- **Threshold 0** divides by 1 rather than 0, so any count above 0 dies with p = rate. `params.toml` never uses 0; the rule only needs a defined value for `--set`.
- **Series.** No new columns. The `grazer_crowded` and `hunter_crowded` columns from shot 5 now fill. Shot 5's death-cause property asserted `crowded` was always 0; that assertion is gone.
- **Why "disease".** The section name comes from the shot prompt. The model is only "too many in one patch raises the death rate"; nothing is transmitted between animals.

**Hunter refractory**
- **`hunter.cooldown` is renamed to `hunter.refractory`.** It keeps the same meaning: a parent waits `refractory` ticks after a birth, a newborn waits `refractory` ticks before its first, and the initial hunters draw a cooldown in 0..=refractory. Births are otherwise gated only by the existing `repro_energy` (75). So `refractory=5000` is exactly the pre-shot rule, which the rate-0 identity test relies on.
- **The prompt's 300 breaks the anchor, so the default is 2750.** At 300, hunters boom and eat every grazer on all four anchor seeds. The smallest value on a 250-tick grid that keeps seeds 1, 2, 3 and 42 passing is 2750. The rounds are in `TUNING.md`.
- **2750 depends on hunter crowding.** The search ran at the hunter crowding default (0.001, threshold 4). With `disease.hunter_rate=0`, refractory 2750 and 3500 fail all four seeds and only about 5000 holds. At the defaults, 81–86% of hunter deaths are `crowded`, so crowding, not hunting success, sets hunter numbers. The prompt's aim is reported in `sweeps/shot10/FINDINGS.md`, not achieved. Making the energy gate bind would need `repro_cost`, `kill_energy` or `repro_energy` to move, which is outside this shot's mechanism.
- **Hunter crowding at 0.05 with threshold 2 also keeps the anchor at refractory 300** (hunters at 1–26). It was not adopted: the anchor rule moves the new mechanism's default by the smallest step, and that would have made crowding dominate even harder.
- **`grazer.cooldown` is unchanged.** The prompt only replaces the hunter's.

**Defaults**
- `disease.grazer_rate` 0.001 with `grazer_threshold` 16, and `disease.hunter_rate` 0.001 with `hunter_threshold` 4. These are the smallest grid values that keep the anchor at refractory 2750: `grazer_rate` 0 fails seed 3 (grazers eaten at tick 11451), and `hunter_rate` 0.0005 fails 2 of the 4 seeds. There is no grazer target. At these values the grazer peak per patch falls from 50 (rate 0) to about 14 (`sweeps/shot10/FINDINGS.md`).
- 16 is the middle of the prompt's threshold grid (4–32). Grazers average about 10–25 per patch at their pre-shot peaks, so 16 bites at peaks and seldom in troughs.
- Hunters are at most a few per patch, so a hunter threshold of 4 bites only where hunters bunch on prey.

**Tests**
- `prop_crowding_zero_below_threshold_and_monotone` states the prompt's property: p is 0 at or below the threshold and above 0 above it (for a positive rate), lies in [0, 1], and never falls as n rises. Its sibling `crowding_regression_threshold_edge_and_clamp` pins n = threshold, the clamp, and threshold 0.
- `crowding_thins_a_patch_to_its_threshold`: 20 grazers and 6 hunters in one patch each, at a rate that makes any excess certain death, are thinned to exactly 10 and 2, every death `crowded`.
- `crowding_at_rate_zero_kills_nothing_and_draws_nothing` compares the RNG word position with a run where crowding can't fire.
- `hunter_births_are_energy_gated_with_a_refractory` checks the birth gate and the refractory on parent and newborn.
- **Rate-0 identity:** `crowding_off_reproduces_the_pre_shot_10_manifest` runs seed 42 with both rates 0 and `refractory=5000`, and compares it with the pre-shot manifest, kept as `tests/data/s42-manifest-preshot10.sha256`. All 1609 hashes match with nothing cut, because the shot adds no columns or fields. The prompt limits rate-0 identity to disease; the refractory is set to the old cooldown only so that the one comparison covers both changes.
- `fire_off_reproduces_the_pre_fire_manifest` and `format_2_and_fire_only_add_to_version_1_files` switch shot 10 off the same way, since they compare with pre-shot data. The v1 meta comparison maps `cooldown` to `refractory` and drops `[disease]`.
- **Forced extinction:** `forced_hunter_extinction_by_refractory_runs_to_the_end` sets `hunter.refractory=1000000`. Hunters almost never breed and die out of old age at tick 7986 on seed 1. The run goes to 20000 with valid snapshots, and the last snapshot restores. `ecosim stats` names `old_age`, and the hunter-free grazers keep dying of crowding to the end (4861 crowded deaths, 100 of them after tick 15000). Crowding alone can't force an extinction: it stops at the threshold, and never removes the last animals of a patch.
- **`forced_grazer_extinction…` now uses `grazer.energy_cost=1.0` with grazer crowding off** (it was 0.5). At 0.5, crowding thins grazers enough for the survivors to feed. With crowding off as well, the faster-breeding hunters ate the last starving grazers, so `eaten` became the window's dominant cause. At 1.0 the grazers starve out by tick 4447, and the test still forces only starvation.

**Regenerated artifacts (both changes alter behaviour at the defaults)**
These were regenerated in their own commit:
- `tests/data/s42-manifest.sha256`: 1609 lines, same file set.
- `tests/data/s42-check.txt`: seed 42 passes every invariant, with 58 mature trees at tick 10000.
- `fixtures/s42-mini-v2`
- `runs/s42` (gitignored)

`fixtures/s42-mini` (v1) is unchanged. ecoview's copy in `ecoview/public/` is stale again, and the renderer's shot 12 re-syncs it.


## Heritable traits and open boundaries (shot 11)

**Traits (`src/heredity.rs`)**
- **What an animal carries.** Each grazer and hunter carries `energy_cost_mult`, `flee_distance` and `repro_threshold` (`heredity::Traits`). Initial animals and immigrants get the species defaults (`Params::default_traits`): `energy_cost_mult` 1, `flee_distance` the species' `flee_radius`, and `repro_threshold` its `repro_energy`.
- **Where they are used.** Each trait replaces the parameter it names, everywhere that parameter was read:
  - The per-tick cost is `energy_cost · energy_cost_mult`, still doubled on a tick the animal moves.
  - The birth gate is `energy > repro_threshold`.
  - A grazer flees the nearest hunter within its own `flee_distance`.
  - `repro_cost`, `newborn_energy` and the cooldowns are not traits and stay species parameters.
- **Hunter `flee_distance` is a neutral trait, with a new param `hunter.flee_radius` (4.0) as its default.** Hunters flee fire, but the flee is one greedy step with no distance, so nothing a hunter does reads it. The prompt gives every animal all three traits, and the clamp needs a species default. A neutral trait is also a useful control: its drift shows what sampling alone does in a population of 30–60 (`sweeps/shot11/FINDINGS.md`).
- **Inheritance.** A newborn takes its parent's value × (1 + `heredity.mutation` · u) for each trait, u uniform in [−1, 1]. There is one `gen_range(-1.0..=1.0)` f32 draw per trait, in `NAMES` order, taken at the birth after the parent pays `repro_cost`. The result is clamped to [0.25, 4] × the species default (`heredity::inherit`). Mutation is relative to the parent, not the default, so traits random-walk over generations.
- **Rate 0.** `update_animals` checks `mutation > 0` once, before the loops, and passes the flag in, like the crowding switch. At 0 a newborn copies its parent's traits and makes no draw. Every animal is then a default animal, so the run is the pre-shot run.
- **The flee offsets.** `Sim::flee_offsets` is now built for the largest distance the clamp allows (4 × `grazer.flee_radius`). Each grazer uses the prefix within its own distance, found with `partition_point` on the sorted list. That prefix is exactly `offsets_within(flee_distance)` (a property test checks it), so a default grazer scans the same offsets in the same order as before.
- **Default `heredity.mutation` 0.05.** The prompt gives no default, only the sweep grid. 0.05 is low in the grid and keeps the anchor on seeds 1, 2, 3 and 42 at 20 000 ticks. The whole grid passes (`sweeps/shot11/FINDINGS.md`), so the anchor rule didn't move it.
- **Hunters may evolve a `repro_threshold` below `repro_cost`, and grazers one above 100.** A parent below `repro_cost` goes negative when it breeds and starves on its next update. A grazer threshold above 100 can never be crossed, because energy is capped at 100. Both are left to selection. The clamp is the only bound.

**Outputs**
- **series.csv** gains 12 columns at the end: the mean and population standard deviation of each trait, per species, `grazer_energy_cost_mult_mean, grazer_energy_cost_mult_sd, …, hunter_repro_threshold_sd`. That makes 35 fields. They are computed over live animals in f64 and written with 4 decimals. A species with no live animals writes 0 for all six, never NaN.
  - `check`, `stats` and `sweep` still read the 23-field (pre-trait) and 21-field (pre-fire) headers, with the missing columns read as 0.
- **entities.json** animal records gain `energy_cost_mult`, `flee_distance` and `repro_threshold`, as their last three fields.
- **`format_version` stays 2.** Both changes only append columns and fields.
- **state.bin** goes to `STATE_VERSION` 3: layout v2 plus a traits section of 3 × f32 per animal, grazers then hunters, in `Vec` order. Restore rejects v2 files, so shot-10 run directories can no longer be forked; rerun them instead.

**Open boundaries**
- **Grazer and hunter immigration already existed** (the small-number floor from the dynamics fixes), with `immigration_floor` 0 by default. The shot's changes to it:
  - Immigrants carry the default traits.
  - **No edge soil means no immigrant.** The old fallback to any soil column is gone, because the rule is "at a random edge soil column". The prompt's property states it too. No generated world lacks edge soil, so no run changes.
  - **Trees gain `tree.immigration_floor` (0) and `tree.immigration_interval` (500).** A tree immigrant is a sapling (age 0) on a random edge soil column. It is planted only if that column keeps `min_spacing`, like a seed; otherwise the draw is spent and nothing arrives.
- **Grass and shrub get no floor.** They are continuous densities per patch with no "live count", and they already regrow from their own seeding term (`g`). "Every species" is read as every species with individuals.
- **Order.** In `immigrate`: grazers, hunters, then trees, each drawing only when due. A floor of 0 never draws.

**Tests**
- **Properties in `src/heredity.rs`, each with a named regression sibling:**
  - `prop_inherit_stays_in_bounds` (the clamp for any parent, mutation and draw), with `inherit_regression_clamp_at_both_ends`.
  - `prop_traits_stay_in_bounds` (every animal in bounds after every tick of a run at mutation 0.05–1), with `traits_regression_seed_42_mutation_one`.
  - `prop_mutation_zero_keeps_defaults` (at mutation 0 every animal, immigrants included, equals the default, and the sd columns are 0), with `mutation_zero_regression_with_immigrants`.
  - `prop_flee_prefix_is_offsets_within`, with `flee_prefix_regression_default_and_clamp_ends`.
- **Immigration property.** `prop_immigration_follows_the_floor` now covers trees and traits. An immigrant arrives only when due and below the floor, only on an edge soil column, with the default traits, and with no draw when nothing is due. `immigration_regression_no_edge_soil_brings_nothing` replaces the fallback regression, and `tree_immigrant_respects_min_spacing` is new.
- **Unit tests** pin three draws per birth and none at mutation 0 (`mutation_draws_three_per_birth_and_none_when_off`), the three uses of the traits (`traits_replace_the_species_params`), and the mean and sd over live animals only.
- **Rate-0 identity:** `heredity_off_reproduces_the_pre_shot_11_manifest` runs seed 42 at mutation 0 (floors at their default 0). It compares the run with the pre-shot manifest, kept as `tests/data/s42-manifest-preshot11.sha256`, after `tests/common::without_traits`:
  - `without_traits` strips the 12 series columns, the three `entities.json` fields and the state.bin traits section, and sets the state version back to 2.
  - It asserts every value it cuts is the species default (or 0 for an extinct species), with sd 0.
  - All 1609 hashes match.
  - The shot-10 and pre-fire identity tests and the v1 fixture test now also set mutation 0 and apply `without_traits` first.
- **Forced extinction:** `forced_grazer_extinction_under_heredity_runs_to_the_end` sets `grazer.repro_energy=400` at mutation 0.2. The clamp keeps every grazer's threshold at 100 or more, and energy can't exceed 100, so no grazer is ever born.
  - On seed 1 the grazers are eaten out at tick 2285 and the hunters starve at 3538.
  - The run goes to 20 000 ticks with valid snapshots, and no NaN in the series.
  - Grazer trait sds stay 0 throughout, hunter ones rise above 0, and both species' trait columns read 0 at the end. The last snapshot restores.
- **`forced_fire_extinction…` now also sets `heredity.mutation=0`.** With mutation on, 709 grazers outlast the fires on seed 1. The test forces fire only.

**Regenerated artifacts (mutation 0.05 changes behaviour at the defaults)**
These were regenerated in their own commit:
- `tests/data/s42-manifest.sha256`: 1609 lines, same file set.
- `tests/data/s42-check.txt`: seed 42 passes every invariant, with 85 mature trees at tick 10000.
- `fixtures/s42-mini-v2`: series, entities and state.bin change; `timing.json` is kept.
- `runs/s42` (gitignored)

`fixtures/s42-mini` (v1) is unchanged. ecoview's copy in `ecoview/public/` is stale: it lacks the trait columns and fields. The renderer's shot 12 re-syncs it.

## Collapse atlas (shot 14)

A report-only shot. No rule, default or file format changed. The atlas is `sweeps/atlas/ATLAS.md`.

**`rng.stream`**
- **The new key is `[rng] stream`, default 0.** `Sim::new` seeds the ChaCha8 rng from `--seed` and generates the terrain on stream 0. It then calls `set_stream(stream)`, only when the stream isn't 0, and draws the initial trees and animals and every tick from there.
- **The stream switches after the terrain on purpose.** A seed keeps its world, and each stream is an independent replicate of the dynamics on that world. That is what separates a seed-to-seed flip caused by the terrain from one that is noise. Switching before the terrain would make every stream a new world, and the two causes couldn't be told apart.
- **Stream 0 changes nothing.** `set_stream` isn't called at 0, so no draw moves. The key is also left out of `meta.json` at 0 (`skip_serializing_if`, with `serde(default)` when reading), so default runs are byte-identical to the manifest, `meta.json` included, and older `meta.json` files still fork.
  - Forks restore the stream from `state.bin`, which has always recorded it.
  - `--set rng.stream=N` on a fork is rejected, because the key isn't in the parent's params at 0. A fork continues its parent's stream by design.
- **Test.** `rng_stream_0_is_the_default_stream_and_others_differ` checks three things. Explicit stream 0 gives a byte-identical run directory with no `rng` in `meta.json`. Stream 5 changes the series and entities. It records `rng.stream` in `meta.json` and never changes `height.bin`. The manifest test covers the default run at 20 000 ticks.

**Atlas definitions** (`sweeps/atlas/atlas.py`, which reads the gitignored per-cell series)
- **Outcome classes.** A cell-seed is *persist* when neither animal species reaches 0 at any tick. It is *grazer* or *hunter* collapse when only that species reaches 0, and *both* when both do. Reaching 0 once counts, which is the `first_extinction` rule. With every immigration floor at 0, no species comes back.
- **Trees aren't a class.** The prompt names four outcomes. A † marks cells in which trees reach 0.
- **Dominant cause.** For each first collapse, the cause is the one with the most deaths in the 500 ticks ending at its extinction tick. This is the `ecosim stats` rule. The grid shows the most common (first species, cause) over the cell's collapsed seeds.
- **Region.** A 4-connected set of cells in which all three seeds have the same outcome.
- **Flip.** A cell whose three seeds don't all have the same outcome.
- **Noise test.** Every flipping cell is rerun on streams 1–5 for each of seeds 1–3, which gives 6 replicates per seed with stream 0 included.
  - A flip is *seed-determined* when every seed repeats its stream-0 outcome on all 5 extra streams. Then the world, not the draws, decides the outcome.
  - It is *noise* when any seed changes outcome across its streams.
- **Output layout.** The reruns use `ecosim sweep --param rng.stream --values 1,2,3,4,5` with the cell's two values as `--set`. Their output goes to `sweeps/atlas/<grid>/flips/<cell>/`. `sweep.csv` and `sweep.md` are committed, and the per-cell series are gitignored like every other sweep's.

## Food-limited hunters (shot 14a, Blocked)

The shot's code is in, but its defaults are not: **no kill_energy × hunt_cost cell meets the target, so every default is unchanged**, and the run is byte-identical to the shot-14 manifest. The evidence is in `sweeps/shot14a/FINDINGS.md`, and the block is in `overnight/shots/14a.BLOCKED.md` (outside the repo).

**`hunter.hunt_cost` and `fail_cost`**
- **`hunt_cost` is new (default 0.0) and is charged on every attack attempt, hit or miss.** A kill leaves `min(energy + kill_energy − hunt_cost, 100)`, so the cost is taken before the clamp. A miss leaves `energy − hunt_cost − fail_cost`.
- **`fail_cost` is kept as an extra charge on a miss, not folded into `hunt_cost`.** Folding it would charge the old 0.25 on hits as well, and the identity case could not reproduce the pre-shot rule through `--set`. Keeping it also leaves the four anchor values untouched.
  - The shot's property reads "after a failed attack = before − hunt_cost". The test states it as `before − hunt_cost − fail_cost`, and the regression sibling pins the bare form at `fail_cost` 0.
- **Energy is clamped only at 100.** A miss can take a hunter below 0, and it then starves in the same update, as before.
- `hunt_cost` 0 is the pre-shot rule to the bit: `x − 0.0` and `x + k − 0.0` are exact.

**`disease.hunter_rate` stays 0.001.** Change 1 asks for 0. With it at 0, no cell of any grid keeps the anchor. Seed 1 loses its grazers to predation in 54 of the 55 fine-grid cells, and the shot forbids tuning anything else. So the change was not made. The shot is blocked instead, with the best region reported.

**The signature (`check::signature`)**
- **Lag convention.** Lag L correlates grazers(t) with hunters(t + L), so a positive L means hunters follow grazers.
- **Window.** Ticks 2000 to 20000, or to the end of a shorter run.
- **The sample for each lag.** Each lag uses only the ticks where both t and t + L lie in the window, with the means taken over that overlap (plain Pearson). A lag with fewer than 2 samples, or with zero variance on either side, is skipped.
- **Ties.** Lags run from −2000 upward, and only a strictly larger correlation replaces the best, so ties go to the most negative lag.
- **Undefined cases.**
  - Grazers or hunters are 0 at any tick of the window. This includes a species that died out before tick 2000. The result is `Extinct`, carrying that species' `Extinction`, so the report names the cause with the `ecosim stats` rule.
  - Trees reaching 0 doesn't count.
  - No lag has variance on both sides: `Flat`.
- **`ecosim stats --signature` prints only the signature line.** Plain `stats` output is unchanged, so scripts that parse it are unaffected.
- **Sweep columns.** `sweep.csv` appends `pp_lag,pp_corr,pp_undefined`. When the signature is defined, the first two are filled and `pp_undefined` is empty. Otherwise the first two are empty and `pp_undefined` holds `<species> <cause>` or `flat`. `cycle_ratio.py` skips all three.

**Tests**
- `prop_attack_energy_accounting` (with `attack_energy_regression_clamp_and_zero_costs`) calls `attack` directly, now `pub(crate)`, at kill_prob 1 and at kill_prob 0.
- `prop_signature_finds_the_delay` (with `signature_regression_delay_edges_extinct_and_flat`): hunters that copy the grazer series d ticks later give pp_lag = d. The sibling pins ±2000, extinction inside the window and before it, trees ignored, and a flat series.
- **Identity:** `old_hunting_economics_via_set_reproduce_the_shot_14_manifest` sets `disease.hunter_rate=0.001`, `kill_energy=40` and `hunt_cost=0` through `--set`. It compares the run with `tests/data/s42-manifest-preshot14a.sha256`, a copy of the shot-14 manifest taken before any change. Because the defaults didn't move, that file equals `s42-manifest.sha256` for now. The test is there for a later shot that changes the defaults.
- **Forced extinction:** `forced_hunter_starvation_by_hunt_cost_runs_to_the_end` runs `hunt_cost=5` on seed 1. The hunters starve out, the run reaches 20000 with valid snapshots, `stats` names `starved`, `stats --signature` reports undefined with `starved`, and the last snapshot restores. It is `#[cfg_attr(coverage, ignore)]`, like the five full-length tests: its CLI path is in `main.rs`, which coverage excludes, and the signature's branches have unit tests.

**Not regenerated.** The manifest, the golden check output and the fixtures are unchanged, because behaviour at the defaults is unchanged.


## Event log (shot 14b)

**File.** Every run directory now has `events.csv`, and `format_version` goes up to 3. Versions 2 and 3 only add files. The file is plain CSV: seed 42 at 20000 ticks writes 1,666,750 bytes, well under the 20 MB that would call for zstd. The header is `tick,kind,species,patch_x,patch_y,x,y,cause,detail`, and each event gets one row, in the order it happens. Fields that don't apply are left empty.

| kind | species | x,y | cause | detail |
|---|---|---|---|---|
| `death` | grazer / hunter | column | `starved` `eaten` `old_age` `crowded` `burnt` | animal id |
| `birth` | grazer / hunter | newborn's column | | newborn id |
| `immigration` | grazer / hunter / tree | column | | id |
| `germination` | tree | column | | tree id |
| `tree_death` | tree | column | `old_age` `drought` `crowded` `burnt` | tree id |
| `ignition` | | | | |
| `spread` | | | | source patch `patch_x + 8·patch_y` |
| `burnout` | | | | |
| `seed_drop` | reserved, never written | | | |

- **Tree causes.** Trees used to have one combined age-or-drought check. It is now two checks, age first and then drought, with the same condition and no RNG, so behaviour is unchanged. Crowding and fire deaths get their own causes.
- **Flushing.** Recording happens only when `Sim::log_events` is on; the writer turns it on for format 3. Events go into `Sim::events`. The writer appends them to the file at every snapshot tick, after the snapshot is written, and once more at the end. So when a snapshot directory exists, every event up to its tick is already on disk.
- **No RNG draws.** Logging only reads state that is already there. `logging_draws_nothing_and_changes_nothing` checks that the stats rows and the RNG word position are the same with logging on and off.
- **Fork** copies the parent's rows with tick ≤ `at`, which are the ticks the restored state has already stepped, and then logs its own. It refuses a format-2 parent, because the parent has no event history to copy.
- **`stats`** reads death causes from `events.csv` when the file is there (`check::read_series_for_stats`), and from the series columns otherwise (versions 1 and 2). The two agree:
  - `s42_event_log_matches_the_series_and_stays_small` checks per-tick death counts, the extinction lines, and that every burnout has a matching ignition or spread. It also checks the size and that all four tree causes occur.
  - `assert_valid_run` makes the same checks for every forced-extinction run.
  - `prop_event_deaths_match_series` and `prop_every_burnout_was_lit` cover fire-heavy and die-off parameters, together with the regression sibling `event_deaths_regression_every_kind_and_cause`.
- **Writing v2.** `ecosim run --format-version 2` writes the version-2 directory, which has no `events.csv`. The fixture test checks that it still equals `fixtures/s42-mini-v2` byte for byte.
- **Temporary ecoview pin.** The renderer rejects format 3, so the ecoview CI job's s42 run is pinned to `--format-version 2` in `.github/workflows/ci.yml` only. **Shot 16 removes that pin** when the renderer learns version 3.
- **No v3 mini fixture.** `sync-data.sh` copies the highest-versioned mini fixture, so committing `fixtures/s42-mini-v3` now would break ecoview. The shot that teaches the renderer version 3 can add it.
- **Manifest.** `tests/data/s42-manifest.sha256` was regenerated for `events.csv` only: one line was added and every existing hash is unchanged. `manifest_regeneration_for_the_event_log_changed_no_existing_line` asserts this. The older manifests are compared with the `events.csv` line dropped.

## Food-limited hunters, second attempt (shot 14a-rev, Blocked)

The shot's code is in, but its defaults are not. No cell meets the acceptance, so `hunter.handling_ticks` is 0 and every other default is unchanged. Default runs are byte-identical to the manifest shot 14b left, `meta.json` included. The evidence is in `sweeps/shot14a-rev/FINDINGS.md`. The block is in `overnight/shots/14a-rev.BLOCKED.md`, outside the repo.

**Handling time (`hunter.handling_ticks`)**
- **When it starts.** A successful attack sets the hunter's `handling` counter to `handling_ticks`.
- **What happens while it runs.** Each of the next `handling_ticks` updates is in state `Handling`, which is new and is written to `entities.json` as `"handling"`. In that state the hunter makes no attack, does not move and pays the resting cost (`energy_cost · energy_cost_mult`, ×1). The counter then drops by one.
- **After it ends.** The update after the last Handling one is a normal update. Kills are therefore at least `handling_ticks + 1` updates apart. Over any W ticks a hunter makes at most W/`handling_ticks` + 1 kills.
- **What handling does not stop.**
  - **Fire.** A handling hunter does not flee fire. It stays put and takes fire damage, so the duration is exact and a fire can't cut handling short.
  - **Displacement.** It can't be displaced, since only grazers are ever displaced.
  - **Death and birth.** It can still starve, burn, die of old age or of crowding, and give birth. Those checks come after the behaviour, as before.
- **No RNG.** Handling reads and writes no RNG. At `handling_ticks` 0 the counter is never set, so the update is the pre-shot one, bit for bit.
- **`meta.json`.** `handling_ticks` is left out of `meta.json` at 0 (`skip_serializing_if`), and a missing key reads back as 0. So default runs keep their exact `meta.json`, and the fixtures and ecoview's copies don't change.
  - `fork` puts the key back into the parent's params before applying overrides, so `fork --set hunter.handling_ticks=N` works on a parent that ran without handling.
  - The `--set` round-trip test treats a key that is absent at its off value as absent, not null.
- **`state.bin` version 4** appends a u32 handling counter per hunter, in `Vec` order, and adds animal state 6 (Handling).
  - It is written only when handling is in use: `handling_ticks` > 0, or a hunter still has handling left, which can happen after a fork sets it back to 0.
  - Otherwise the file is version 3, byte for byte what the ecosim before this shot wrote. Version 3 decodes with every hunter's handling at 0.
  - So the manifest, the golden check output and the fixtures are unchanged, and nothing was regenerated.
- **`disease.hunter_rate` stays 0.001, as in 14a.** Change 4 ("stays 0") assumed 14a had moved it. It hadn't, and at 0 no cell keeps the four seeds alive to 60000 ticks.

**The signature, redefined** (`check::signature`, `ecosim stats --signature`)
- **The new definition is the default. The old one is gone, with no flag to bring it back.** 14a's committed FINDINGS tables keep the old definition (±2000, ticks 2000–20000, not detrended), and their text says so.
- **Detrending.** Each series has its centred moving average over t − 2000..=t + 2000 subtracted. The average is computed over the whole run, and near the ends it uses the ticks that exist. The right edge of a 60000-tick run is therefore detrended with a shorter average.
- **Window and lags.** The window is ticks 5000–60000, or to the run's end. Lags run over ±8000 in steps of 50. The lag convention, the per-lag overlap sample and the tie rule (most negative lag) are 14a's.
- **Extinction.** Grazers or hunters at 0 inside the window make the signature undefined, as before. The window now starts at 5000, so a species at 0 only before tick 5000 no longer counts.
- **pp_period.**
  - **Lobes and peaks.** A lobe is a maximal run of lags with correlation above 0, and its peak is its largest value (the first one on a tie). Lobes that touch either end of the lag range are left out, because their true peak may lie beyond it.
  - **The value.** pp_period is the lag distance from the peak nearest lag 0 (ties go to the positive side) to the peak nearest that one. With fewer than two lobes it is undefined.
  - **Why lobes.** The prompt suggests consecutive local maxima. On real runs, those include noise wiggles on a single lobe, which gave periods of 250–2000. Lobes count each positive hump once.
- **Sweep column.** `sweep.csv` appends `pp_period` after `pp_undefined`, and it is empty when undefined. `cycle_ratio.py` skips it.

**Tests**
- **Handling property.** `prop_handling_bounds_kills` has the sibling `handling_regression_one_tick_longer_than_the_run_and_off`. It drives three always-hungry hunters in a herd of 300 through `update_hunter` and checks every update:
  - A hunter with handling left is in Handling, kills nothing, stays put, loses exactly the resting cost (so it made no attack) and counts down by one.
  - Kills happen only in state Hunt, and each one sets handling to `handling_ticks`.
  - Every kill is followed by exactly `handling_ticks` Handling updates.
  - Kill ticks are more than `handling_ticks` apart, and every window respects the W/h + 1 bound.
  - The sibling pins handling 1 (a kill every other update while prey is in reach), a handling time longer than the run (one kill), and handling 0 (a kill on every update, never Handling).
- **Signature.**
  - `prop_signature_finds_the_delay` now covers ±8000. Its test series has four incommensurate cycles, because with two, a delay could alias onto a lag one period away. The correlation bound is 0.97, since the shortened moving average at the run's end detrends the two series slightly differently.
  - `prop_signature_period_is_the_cycle`, with the sibling `signature_period_regression_lobes_edges_and_one_peak`, checks three things. A pure cycle gives its period within two lag steps, which absorbs the integer rounding of counts. A notch on one lobe is not a second peak. An edge lobe and a single lobe give no period.
- **Restore.** `restore_regression_mid_handling` restores a sim while hunters are handling, and it must step identically for 500 ticks. It also checks that the file is version 4 only when handling is on. `fork_can_switch_handling_on` checks the fork path.
- **Identity.** `old_hunting_economics_via_set_reproduce_the_shot_14_manifest` now also sets `hunter.handling_ticks=0`. It compares the run with `s42-manifest.sha256` (14b's manifest, with `events.csv`) and with 14a's `s42-manifest-preshot14a.sha256`. Both match.
- **Forced extinction.** `forced_hunter_starvation_by_handling_time_runs_to_the_end` sets `handling_ticks=1000000`, so a hunter's first kill is its last. On seed 1 the hunters starve out at tick 2582.
  - The run reaches 20000 ticks with valid snapshots, and the tick-1000 snapshot has hunters in `handling`.
  - `stats` and the signature both name `starved`, and the last snapshot restores.

## Signature fix (shot 14c)

14a-rev's signature detrended with a one-year (4000-tick) moving average. That cancelled the seasonal cycle out of the trend, left it in the detrended series, and made pp_period measure seasons (2050–4300). This shot replaces the definition. 14a-rev's is gone, with no flag to bring it back. No sim behaviour changes, so the manifest, golden files and fixtures are untouched. The re-score is `sweeps/shot14c/RESCORE.md`.

**The signature** (`check::signature(rows, year_len)`, `ecosim stats --signature`)
- **Detrending.** Each series has its centred moving average over t − 6000..=t + 6000 subtracted. The average is cut short at the run's ends, as before.
- **Seasonal removal.** Each detrended series then has its mean at the same phase subtracted: phase is tick mod `climate.year_len`, and each phase's mean is taken over the whole run.
  - The year length comes from the run's `meta.json` (`year_len`) for a run directory, and from `params.climate.year_len` in a sweep.
  - A bare series file has no meta, so it needs `--year-len` (below).
- **Lag and correlation.** These are unchanged: ticks 5000–60000, lags ±8000 in steps of 50, and ties go to the most negative lag.
- **pp_period.** This is now the hunter series' own autocorrelation, not the cross-correlation. It is computed after seasonal removal, over the same window.
  - **Lags.** The prompt's "1..20000 step 50" is read as 50, 100, …, 20000. Lag 0 is trivially 1, and one extra lag, 20050, is computed so that 20000 can be a maximum.
  - **Local maximum.** A local maximum is strictly above the lag before it and at least the lag after it, so a flat top counts once, at its first lag.
  - **The value.** pp_period is the first positive local maximum after the first negative value. When there is none, or the series is too short, pp_period is undefined and the run fails.
- **pp_pass** is 0 < pp_lag < pp_period / 2 with pp_corr > 0.3, where lag < period / 2 is tested as 2·lag < period. An extinction in the window, a flat series or an undefined period gives false.
  - `stats --signature` prints `pp_pass` on every line, including the undefined ones.
  - `sweep.csv` appends a `pp_pass` column after `pp_period`, and `cycle_ratio.py` skips it.
- **Scoring a sweep cell.** `stats --signature` also accepts a bare `series.csv`-format file, such as a sweep's `cells/*.csv`, with `--year-len N`. For a run directory, `--year-len` overrides meta.json. This is the smallest path to re-scoring cells without rerunning them, and it needed no new subcommand. The cause for an extinction then comes from the series' own death columns.

**Tests**
- **Trailing cycle.** `prop_signature_trailing_cycle_passes` has the sibling `signature_regression_trailing_cycle_seasons_and_leading`. Hunters trail a P-tick grazer cycle (P from 6000 to 12000) by L between 0.05P and 0.45P. Both series carry a 4000-tick seasonal term (the hunters' shifted by 700 ticks) and a linear trend. The test requires pp_lag within 100 of L, pp_period within 5% of P, and pp_pass.
  - **Why an envelope.** The synthetic cycle's amplitude swells and fades over 25000 ticks. A constant-amplitude sinusoid correlates as well at L − P and L + P as at L, so the best lag was a coin toss between lobes. The first draft failed at P = 6000 with a lag of 6300 for a delay of 300. Real cycles aren't strictly periodic.
  - **Why not a phase random walk.** It was tried first and dropped, because it shifted the measured period by more than 5%.
  - **Why 66001 ticks.** The synthetic series runs 6000 ticks past the window's end, so the moving average isn't cut short inside it. At a 60001-tick length, the cut average at the right edge detrends the two shifted series differently. That moved flat 11000-tick peaks by 2 lag steps (lag 1800 for a delay of 1901). Real 60000-tick runs carry this edge effect, which is on the order of 100 ticks.
  - **Sibling cases.** The sibling pins four (P, L) cases. It checks that seasons plus independent noise on both species don't pass, and that hunters leading by less than P/4 give a negative lag and no pass (three cases). It also pins the pass rule's boundaries.
- **Known delay.** `prop_signature_finds_the_delay` and `signature_regression_delay_edges_extinct_and_flat` are kept, now over 60001 ticks with 9000/5700/3100/1300-tick cycles. None of those periods is a whole number of years, so seasonal removal doesn't erase them. The correlation bound drops from 0.97 to 0.95, because the cut moving average at the end is now 6000 ticks.
- **Removed.** `prop_signature_period_is_the_cycle` and `signature_period_regression_lobes_edges_and_one_peak` tested the lobe-spacing period, which no longer exists.

## World dimensions, rain gradient and slope (shot 15)

The world size is now a parameter. `[world] width`, `depth`, `height` and `patch` (defaults 64, 64, 32, 8 when absent) replace every hard-coded size, and `climate.rain_gradient` and `world.slope_bias` (both 0 when absent) tilt rain and terrain west to east. The reference world becomes a 256×64×32 strip with gradient 0.6 and slope bias 4. At 64×64×32, patch 8, gradient 0 and slope 0 every run is byte-identical to shot 14.

**Dimensions**
- **`world::Dims`** (`wx`, `wy`, `wz`, `patch`) is built from the params (`Dims::of`) and carried by `World`. Index helpers that were free functions over constants (`cidx`, `vidx`, `patch_of`, bounds checks) are now its methods. Nothing is sized from a constant any more.
- **Loader checks.** `Params::check_dims` runs on every load and override. `patch` must be at least 1, `width` and `depth` must be multiples of `patch` in 1..=256, and `height` must be in 2..=256.
  - **Why 256.** Tree positions, event columns and patch coordinates, and `height.bin` are stored as u8. 256 is the most they hold, and the strip is 256 wide.
  - The error names the key, as `--set` errors already do.
- **Missing keys.** The keys have serde defaults: 64, 64, 32, 8 and 0, 0. So a `meta.json` from before this shot reads as the square world, and old snapshots restore and fork unchanged.
- **`meta.json` `dims`** gains `patch`: `{x, y, z, patch}`, with x = width, y = depth, z = height. `patch` is added to the existing object rather than a new key, so readers that look at `dims.x/y/z` keep working. `format_version` stays 3: no file is added or removed, and on the square world every file is the same. The ecoview loader checks `dims` against 64×64×32 and rejects the strip. That is the renderer's shot (16).
- **Snapshot sizes on read.** `Sim::restore` checks `material.bin` and `light.bin` against x·y·z and `height.bin` against x·y from the params it restores with, and `state.bin` decodes its column and patch sections at those sizes. A file of the wrong size is an error. `forced_grazer_extinction_on_the_strip_runs_to_the_end` restores a strip snapshot onto square-world params and expects the error.
- **Unit tests stay on the square world.** A `cfg(test)` module `world::sq` holds the 64×64×32 dims and the old index helpers, and `Params::load_square()` is the default params on that world. Their facts and literals were measured there. The strip gets its own tests: brute-force `patch_dist` on a 128×32 strip and on the seed-42 reference terrain, strip index coverage, and slope tilt.
- **The value-noise lattice** is `(wx/period + 1) × (wy/period + 1)` (rounded up) points, drawn row by row. On 64×64 that is the same size and order as before, which the identity test confirms.
- **Events** store the patch as (patch_x, patch_y), not as an index. The `spread` detail is the source patch index `patch_x + patches_x·patch_y`, which is `+ 8·patch_y` on the square world as before. `unlit_burnout` tracks lit patches in a `BTreeSet`.

**Rain gradient and slope**
- **Rain.** Rain at column x is `rain × (1 + rain_gradient × (2x/(width − 1) − 1))`, clamped at 0 (`abiotic::rain_at`). West (x = 0) is dry, east is wet. `update_soil` checks `rain_gradient == 0` once, outside the column loop. At 0 it runs the old loop unchanged, so no float operation differs.
- **Gradient range.** The loader accepts `rain_gradient` in [-1, 1] only. In that range the clamp never bites, rain is in [0, 2 × rain], and a row's total is exactly width × rain because the west–east term is odd about the middle. So the two properties the shot asks for hold for every accepted gradient. Beyond ±1 the clamp would add rain to the row, and the sweep only needs 0–1.
- **Slope.** Height normalisation adds `slope_bias × (1 − 2x/(width − 1))` before rounding and clamping, so at slope 4 the west edge sits about 4 voxels higher and the east edge about 4 lower. It is also skipped at 0.

**Runtime invariant**
- **Limit.** The check's limit scales with world area: 30 s per 64×64 of columns, never below 30 s and capped at 90 s (`check::runtime_limit_ms`). The area comes from `meta.json` `dims`, defaulting to 64×64 when absent.
- **Why.** The shot sets the strip's budget at 20k ticks in under 90 s release. The strip has 4× the columns and runs in about 45 s on this machine and on CI, which the old flat 30 s would fail. The square world keeps exactly the old limit and line text (`run time < 30 s`). Other sizes print `run time < 30 s per 64x64, at most 90 s`. Speeding the sim up is shot 15a's.

**Manifests, golden files and fixtures (regenerated once, in the shot-15 commit)**
- **`tests/data/s42-64-manifest.sha256`** is the shot-14 manifest, copied unchanged. `square_world_reproduces_the_shot_14_manifest` runs seed 42 at the square settings (`common::SQUARE`: `world.width=64`, `climate.rain_gradient=0`, `world.slope_bias=0`, plus depth, height and patch set explicitly) and matches it byte for byte, and checks `meta.json` `dims`.
- **Identity tests on the square world.** The older identity tests (pre-fire, pre-shot-10, pre-shot-11, old hunting economics) and the tests whose documented facts were measured on the square world now run there through `common::SQUARE`. That covers drought, carrying capacity, the forced extinctions and the format tests. The old-hunting test compares with `s42-64-manifest.sha256`.
- **Regenerated for the strip:** `tests/data/s42-manifest.sha256` and `tests/data/s42-check.txt`, both from `runs/s42` at the new defaults.
- **Fixtures stay square.** `fixtures/s42-mini` (v1) and `fixtures/s42-mini-v2` stay on the square world, because they are what `scripts/sync-data.sh` hands the renderer and the renderer rejects other dims until shot 16. The v1 fixture can't be rewritten anyway: the v1 writer is gone.
  - Only `s42-mini-v2/meta.json` changed: its params gain the new keys, `dims` gains `patch`, and `overrides` lists the square settings it is now generated with. Every other file of it is byte-identical.
  - Shot 16 moves the renderer's fixture to the strip.
- **ecoview CI pin.** The ecoview job's generate step (`.github/workflows/ci.yml`, "run s42 and sync") is pinned to the square world with `--set world.width=64 --set climate.rain_gradient=0 --set world.slope_bias=0`, next to the existing format-version pin. Shot 16 removes both.

**CI: the seed matrix**
- **Why.** On the strip, a seed's 20k run takes about 45 s and the 60k long run about 140 s on the CI runner, 7× the square world. The sweep-harness tests that run seed 42 in debug grow the same way. Run one after another, the ecosim job would take about 13 minutes against the shot's 10.
- **The split.** The job becomes three that run in parallel, as the shot allows ("or the seed matrix is used"):
  - `ecosim`: fmt, clippy, docs, debug tests, and the cross-profile test. `cargo test --release` builds the release binary itself.
  - `ecosim-coverage`: coverage, with its own cache.
  - `ecosim-sims`: a matrix over seeds 1, 2 and 3. Each builds release and runs and checks its seed; seed 1 also runs the long run, and seed 2 the baseline sweep.
- **What stays the same.** Every step and command is unchanged. The step order in the file still satisfies `tests/ci.rs`. `just ci` still runs every step in sequence: about 8 minutes without coverage on this machine (the local gate), plus coverage.
- **Baseline test.** `baseline_margins_equal_check_margins` runs on the square world to keep the coverage run short.

**Signature test**
- **New case.** `prop_signature_recovers_a_6000_tick_lag_on_a_long_cycle`, with the sibling `signature_regression_6000_tick_lag_on_a_14000_tick_cycle`: hunters trail a 12500–14000-tick cycle by 6000 ticks, under the 4000-tick season and a trend. The check recovers the lag within 200 and the period within 500. The check itself is unchanged.
- **Limits found while writing it.**
  - With the 25000-tick amplitude envelope that the shot-14c cases use, the hunter autocorrelation's peak moves by up to 600 ticks on these long cycles: 14600 for 14000.
  - Periods of 15000–16000 read up to 550 short even under slower envelopes (40000 and 60000). The window holds only three or four such cycles, and the 12000-tick detrend removes part of each.
  - So the case uses a 60000-tick envelope and periods of 12500–14000, where 200 draws stay within 500. The shot's "a hunter cycle over 12000 ticks" is read as that range.

## Performance baseline and tick profile (shot 15a)

No sim rule, default or file format changed, and nothing was optimised. The manifest, golden files and fixtures are untouched. The measurements and recommendations are in `PERF.md`.

**`ecosim run --profile <file>`** (`src/profile.rs`)
- **A lap timer.** `Profiler::lap(phase)` charges the time since the previous lap to that phase. The laps sit between the steps of `Sim::step_profiled` and of the run driver (`output::run_profiled`), so the phases tile the run with no gaps. Their sum equals the total except for the time between the last lap and the report, which is dropping the `Sim`. That is under 0.1% on every measured run; the acceptance allows 5%.
- **The phases.** The prompt names eight. The profile splits three more out of them and adds the two ends of the run:
  - `setup`: the output directory, world generation with the BFS distances, initial populations and `meta.json`.
  - `immigration`, `trees` and `compaction` are their own phases rather than folded into `animals` or `producers`, because they sit between them in the tick order.
  - `series_write`: `series.csv` and `timing.json` at the end.
  - `events` covers both the flush at each snapshot and the last flush.
- **The totals.** `ticks_per_second` is over the whole run, writing included. `step_ticks_per_second` is over the `Sim::step` phases alone.
- **What it never touches.** The timer reads only `Instant`. `Sim::step` is `step_profiled(None)`, and `simulate` is `simulate_profiled` with no profiler, so every other caller (sweeps and forks) runs exactly the code it ran before.
- **Where the file goes.** `ecosim run` refuses a `--profile` path inside `--out` (compared as absolute paths, before anything is written), so the run directory is byte-identical with and without it. `profile_leaves_the_run_directory_unchanged_and_accounts_for_the_run` checks this, and checks the 5% sum.

**The bench** (`benches/tick.rs`, criterion 0.8.2 without default features)
- **What it runs.** 2000 ticks of seed 42 in memory through `output::simulate`, with stats rows and no files, on 64×64 (`common::SQUARE`'s overrides) and 256×64.
  - `Sim::new` is outside the timed part, because world generation is a per-run constant and not a tick cost.
- **Sampling.** Criterion runs 10 samples per world with 1 s of warm-up. Each sample is one 2000-tick run, since a run takes 0.5–2 s.
- **The number compared.** The harness records every timed run, warm-up included, and compares their median as ticks per second. Criterion's own estimates are printed but not compared: its mean is pulled by outliers, and reading its JSON files would tie the check to its output layout.
- **The gate.** A world fails when its ticks per second is below 0.8 × its baseline. Faster is never a failure. A shot that makes the sim faster on purpose should update the baseline, or the gate drifts loose.
- **The CI job.** `ecosim-bench` is its own job with its own cache, so it runs beside the gate, and it uploads `ci-runs/bench-tick.json`.
  - It is not a numbered gate step and not in `just ci`: its baseline belongs to the CI runner, and this machine's speed differs by more than 20%, including 2× from core migration (`PERF.md`).
  - `just bench` runs it locally.
  - `tests/ci.rs` checks that `ci.yml` runs it and uploads its numbers.

**How `benches/baseline.json` was taken**
1. The first shot-15a push had no `benches/baseline.json`, and the bench only reported.
2. The file was then written from that CI run's `bench-tick.json` artifact, with the run id and commit recorded in it.
3. The second push compares against it.

A baseline taken on a GitHub runner carries the runner's variance. If the job flakes near 20% on unchanged code, the remedy is more samples, not a wider threshold.

**Measuring on this machine.** The i9-12900KF has P- and E-cores. Windows moves a long single-threaded run onto an E-core after a few seconds, and it then runs about 2× slower. So `PERF.md`'s numbers come from runs pinned to the P-cores (`start /affinity FFFF`). Unpinned wall times from earlier shots, such as shot 15's 44–59 s strip runs, overstate the cost by up to 2×.

## Animals off (shot G0)

The project is moving toward a garden planner, where the animal tier is neither wanted nor affordable: shot 15a measured it at 76–93% of every run, and a 256×256 world took 170 s against a 90 s limit. `[animals] enabled` turns it off.

**The switch.** `enabled = true` in `params.toml`, so every earlier run is unaffected, and `--set animals.enabled=false` turns it off for a run. When it is false:
- `place_initial_animals` returns before placing anything, and grazer and hunter immigration are gated in `immigrate`;
- `Sim::step_profiled` skips `update_animals` outright.

Each check sits **outside** the loop it guards, so an animals-off run draws nothing from the RNG that an animals-on run would draw, and nothing else changes. `animals.enabled = false` is bit-identical to a run with the animal tier configured empty (`start_count = 0`, `immigration_floor = 0`), proved against `state::encode` — which includes the RNG's stream and word position — by the property test `prop_animals_off_is_an_empty_animal_tier` and its named regression sibling on seeds 1–3.

**Nothing was retuned to compensate.** The shot prompt forbids it, and the numbers say it isn't needed: removing grazing raises mean grass cover 8–16% and leaves shrub, tree and mature-tree counts inside the seed-to-seed spread (`sweeps/G0/FINDINGS.md`).

**Why the SAD's "2 ground-cover species, 1 tree species, 2 animal species" still holds.** The two animal species are still in the code, in `params.toml` and in `meta.json`'s species table, with every behaviour the SAD specifies. The switch decides only whether a run places any individuals of them, exactly as `grazer.start_count` always could for one species. It is a run setting, not a change to the species set.

**`meta.json` and the format.** A run with animals off adds `"animals": false` at the top level. The field is skipped when animals are on (`skip_serializing_if`, the same pattern as `[rng]`), so a default run writes the bytes it always did — every committed manifest and fixture is unchanged. `format_version` stays 3: a reader that ignores the field sees a run with no animals in `series.csv` and in `entities.json`, which is a state the format already allowed.

**Which invariants `ecosim check` marks n/a.** An animal invariant on a run with no animals is neither passed nor failed; it is not a question. `CheckLine` gained an `na` flag, `CheckReport::pass()` ignores n/a lines, and `ecosim check` prints them as `N/A  <name>: n/a (animals off)`. This does not widen anything for an animals-on run: those are evaluated exactly as before.

| invariant | with animals off | why |
|---|---|---|
| `no_extinction` | **kept, trees only** | It asks whether a species that is in the run reaches 0. Trees are still in the run, and a garden run that loses its trees must still fail. |
| `max_10x` | **kept, trees only** | It measures runaway growth per species, so its grazer and hunter parts are animal invariants and its tree part is not. |
| `grazer_cycle` | **n/a** | The whole line is about grazer population dynamics. |
| `animals_10k` | **n/a** | "At tick 10000, grazers ≥ 10 and hunters ≥ 2" is unsatisfiable by construction. |
| `long_band` (`check --long` only) | **n/a** | It is a band on the grazer column. With all-zero animal columns it would otherwise pass vacuously, which is worse than saying nothing. |
| `long_no_extinction` (`check --long`) | **kept, trees only** | Same reason as `no_extinction`. |
| every plant and soil invariant | **kept unchanged** | `fertility_band`, `grass_band`, `tree_growth` and `mature_trees_10k` don't mention animals. |

`ecosim stats` follows the same rule: `first_extinction` and the extinction list consider trees only, and the signature (a grazer-cycle measure) reports `signature: not applicable, the run has no animals (animals off) pp_pass false`. Sweep cells write `n/a` in the columns of n/a invariants and `animals off` in the signature column, and neither counts as a failure.

**`ecosim fork --set animals.enabled=…` does not work on a parent run that had animals.** `fork` reads the parent's `meta.json` params, and an animals-on parent has no `[animals]` section there, so the key is rejected as unknown. This is the same limitation `hunter.handling_ticks` has, and it is left alone on purpose: whether a run has animals is a decision taken when the world is built, not one to change from a mid-run snapshot. Forking an animals-**off** parent does work, because its `meta.json` does carry the section; the fork starts with no animals in its state, so any that appear come in through immigration.

**The sweep.** `sweeps/G0/` is `animals.enabled` over `true, false` on seeds 1–3 at 20000 ticks; 6/6 cells pass, 46.6 s wall on 6 jobs. `sweep.md` calls the two-value band "fragile" because its rule is "fewer than 3 grid values"; a boolean can never have three, so the label carries no meaning here.

## World bundles: a world from real ground (shot G1)

**The direction.** The project is moving toward an urban garden planner: plants, water and soil
nutrients on real ground (operator decision, 2026-09-19 evening). Shots G1–G7 are that work.
Predator–prey is parked: the animal tier stays in the code and in `params.toml`, unchanged and
untuned, and the garden runs simply switch it off (`--set animals.enabled=false`, shot G0).

This lifts one piece of the SAD's scope, which says the world is procedural: "terrain from 2-D
value noise". It stays true of a default run — noise is still how `ecosim run` builds a world, and
every committed manifest, fixture and reference run is byte-identical. A **world bundle** is a
second source of terrain, chosen with `ecosim run --world <dir>`, and nothing else about the sim
changes: the same tick order, the same fields, the same species.

**The format is not ours to define.** Bundle v2 is specified by the scene contract, copied verbatim
into the repo as `../docs/SCENE-CONTRACT.md` and authoritative there. `bundle.rs` reads version 2
and refuses every other version by number, rather than guessing at a compatible subset: a bundle
that this ecosim half-understands would produce a world nobody could reason about.

**Two grids, not one.** The ecology grid keeps 1 m columns (`ECO_CELL_M`), because every species
parameter in `params.toml` — seed radius, move distance, flee distance, patch size — is written in
those units, and rescaling the world would silently rescale all of them. The ground grid is the
bundle's own (0.5 m at the Capitol), and is kept at full resolution in `World::ground_grid` for the
surface water and nutrient work in G4–G6. One column covers `ratio² = (1 / ground_cell_m)²` ground
cells, and `ground_cell_m` must divide 1 m exactly, so the mapping is a whole block of cells with no
partial coverage anywhere. `Bundle::apply_to` sets `[world] width` and `depth` from `size_m` and
then runs the normal `check_dims`, so a bundle whose size is not a multiple of `[world] patch` is
rejected with the message that rule always gave.

**Surface layer = `base_z + round(mean ground height)`.** The mean over the column's ground cells,
not the minimum or the maximum: it is the only choice that makes a smooth slope monotone in the
column index without a bias up or down. `[bundle] base_z` (8) is soil below the crop's lowest
ground, so there is something to root into and to drain through; the bundle's heights are metres
above the crop minimum, so the two add. A column whose top does not fit under `[world] height`
is an error naming the column, not a clamp — a silently flattened building or hill is worse than a
failed run.

**Media decide what tops a column, not heights.** More than half the column's ground cells sealed
(`roof`, `asphalt`, `concrete`) makes it Rock, which the whole sim already treats as unplantable
and impassable to roots; otherwise more than half `water` makes it a Water column; otherwise soil.
Strictly more than half, so a 2 × 2 column split two-and-two is neither Rock nor Water but soil —
the tie goes to the living surface, because a half-paved cell does have somewhere for a plant to
be. The noise world's two height rules (`rock_top_height` and flooding to `water_level`) are
**not** applied to a bundle world: the bundle says where the water is, and a quantile rule layered
on top of real data would invent ponds the scene does not have.

**Building shade is a per-column floor on light.** The SAD's light model has no sun direction: light
falls straight down, `255 − canopy_absorb × canopy above`. A building is not canopy, and giving it
canopy voxels would have made it shade only its own footprint, which is exactly the columns that are
already Rock — so the acceptance "a tall block shades the columns on its shadow side and not the
others" would have been unobservable. Instead each column gets `shade_top`, the highest voxel a
building darkens; `set_column_light` treats every voxel at or below it as dark, whatever the canopy
does. A roof of height `h` shades the `h × shade_slope` columns **north** of it (+y), the shadow
falling by 1 m of blocked height per `1/shade_slope` columns: a fixed sun due south at
`atan(shade_slope)` above the horizon, 45° at the default `shade_slope = 1.0`, which is about
Lansing's equinox noon sun. A fixed sun, not a moving one, because the sim has no time of day and
the rest of the light model has no direction at all; a real sun path belongs with a real light
model, and this shot was not asked for one. `shade_slope = 0` turns shade off. A noise world has no
buildings, so its `shade_top` is all zeros and its light is bit-identical to before.

**`format_version` 4, and the ground written once.** The ground grid is static for the whole run, so
repeating it in 201 snapshot directories would multiply a 256 × 256-cell grid by 201 for no
information. It goes in `world/` at the run root, with `meta.json.world` describing it, and the
snapshot directories keep exactly the files they had. Version 4 only adds files, like 2 and 3 before
it, so a version-3 reader that checks `format_version` sees a version it does not know, and one that
reads the ecology grid finds it where it always was. `--format-version 4` and `--world` require each
other: a format-4 run directory without `world/` would be a lie about its own contents.

**A bundle run cannot be forked.** `ecosim fork` restores `state.bin` and rebuilds the terrain from
the parent's params with `World::from_heights` — noise terrain, which a bundle world is not, and the
parent's `meta.json` does not carry the bundle. Rather than rebuild a wrong world, `fork` rejects
format 4 by name and says to rerun the bundle with `ecosim run --world`. Making forks work would
mean either storing the whole ground grid in `state.bin` or resolving the bundle path from
`meta.json`; both are a shot's worth of work that nothing yet needs.

**Trees, shrubs and pipes are validated on load and then ignored.** They are used from G3 (plants)
and G6 (drains). Validating them at load is not a stub for those shots: a bad export must fail at
the bundle, in one message naming the file and the offending entry, rather than halfway through a
20000-tick run. A pipe `outlet` is allowed past the crop edge, since that is how a drain leaves the
world; every other position must be inside it. `Pipe.id` is a `String` rather than an index because
the contract says it is the scene's object name, and the name is what a person reading `pipes.json`
next to the scene will match on.

## The Blender exporter and the Capitol bundle (shot G2)

**The rasteriser is a scanline over triangles, not one ray cast per cell.** The scene contract
describes the sampling as a downward ray cast at each ground-cell centre, and that is the result
`rasterise_tops` produces — but it produces it by walking triangles and filling the cells each one
covers, keeping the highest z per cell, rather than by asking a BVH tree for a ray hit 262,144
times. Three reasons. It is deterministic in a way a ray cast is not required to be: a maximum does
not depend on the order the triangles arrive in, so nothing about the scene's internal ordering can
reach the bundle. It is robust exactly where this scene is worst: `EcoGround`'s vertices *are* the
ground-cell centres, so every ray would be aimed at a shared vertex, and the audit below shows
Blender's own ray caster missing the mesh at 8 of 4,096 sampled cells — the rasteriser has no holes,
because a point on a vertex is inside all the triangles meeting there. And it is testable without
Blender, which is what lets CI check the code that decides the committed bundle's contents.

**BVHTree ray casts stay, as an audit.** `--audit N` casts N real downward `BVHTree.ray_cast` rays
at evenly spaced cell centres and compares them with the rasteriser, failing the export if they
disagree by more than a millimetre. On the Capitol: 4,088 hits, 8 misses (the ray caster's, at the
crop edge), worst disagreement 9.5 × 10⁻⁷ m. That is the cross-check the contract's wording is
really asking for, and it costs one Blender API call per sample instead of one per cell.

**The exporter is split at the `bpy` import.** Everything above it — the grid, the rasteriser, the
merge, the rounding, the sort, the JSON and binary layout — is plain Python that imports with
`bpy` set to `None`, and `tools/test_blend_export.py` covers it with 22 unit tests using no
third-party package. Below it is only Blender plumbing: reading scene properties, turning objects
into world-space triangles, and pulling the ends off a curve. This is CI step 0, in front of
`cargo fmt`, and `tests/ci.rs` pins it in both `ci.yml` and the justfile like the other steps. The
alternative — no CI coverage of the exporter at all, since CI has no Blender — would have made the
one piece of code that decides what a committed world contains the only untested code in the repo.

**Determinism is a property of the file, not of a promise.** The same `.blend` must export to a
byte-identical bundle, so: objects are read in name order, never in `bpy.data` order; positions and
lengths are rounded to millimetres and angles to microradians; `-0.0` is normalised to `0.0`,
because it is a different byte pattern for the same number; text is written with LF newlines so a
Windows export matches a Linux one; JSON keys are written in the contract's order, one entity per
line so a diff is readable; and an exact tie between two overlapping surfaces keeps the earlier
layer, which by the name ordering is a decision and not an accident. Verified by re-exporting to a
second directory and diffing: identical.

**Media and building height.** A cell's medium is the winning surface's, and its building height is
that surface's top above the ground *only where the winner is a roof*, clamped at zero. Reading
building height off a losing roof would have put a building under a plaza; leaving a wall that dips
below the terrain negative would have failed G1's loader, which rejects negative building heights.
Cells no surface covers take `eco_default_medium` (lawn at the Capitol). `bundle.json` always lists
all nine media, even though this scene uses four, so a medium code means the same thing in every
bundle; `counts` carries only the three entity counts, since the loader ignores it and the cell
counts live in the world's README where a person will look for them.

**Heights are relative to the crop minimum**, as the contract says, so the exporter subtracts the
sampled minimum rather than trusting the scene's Z origin to be at it. At the Capitol the scene's
lowest ground is 0.011 m, so the two differ by 11 mm — small, and exactly the kind of drift that
would make one scene's bundle incomparable with another's.

**The bundle is committed, all 3.1 MB of it.** It is data, like `fixtures/`: regenerating it needs a
17 MB `.blend`, a LiDAR pipeline and Blender, none of which are in the repo or on CI. What CI checks
instead is the bundle as committed — `tests/bundle.rs` loads it and pins its shape, so a careless
re-export shows up as a failed test and not as a changed simulation three shots later:
256 × 256 ecology columns over a 512 × 512 ground grid at 0.5 m; media in ground cells lawn 169,876,
asphalt 44,879, roof 24,705, concrete 22,684 and nothing else; ground 0.000–8.589 m; building height
up to 76.011 m (the dome); 81 trees, 64 shrubs, 4 pipes; 21,705 of 65,536 columns (33.1%) sealed to
`Rock`; surface layers 8–16. The medium counts match the operator's independent rasterisation of the
same scene cell for cell, which is the strongest evidence available that the exporter reads the
contract the way the scene writes it.

**Licensing travels with the data.** The Capitol's walks, plazas and two parking lots come from
OpenStreetMap, so `medium.u8` is a derivative database under the ODbL while everything else in the
bundle is public-domain USGS LiDAR. `worlds/capitol/README.md` gives the credit word for word and
says which file is under which licence; the same credit is inside `bundle.json`'s `source` string,
which G1 already copies into every run's `meta.json`, so a run directory handed to someone else
carries its attribution without the README. The pipes are marked `illustrative` in the data itself,
not only in prose: they are low points of the real ground joined to the crop edge, not storm-sewer
records.

**The scene met the contract.** `check_tags.py` passes on `capitol.blend`, and the export needed no
exception for it, so there is no mismatch to report and no BLOCKED file. The scene's known rough
edges — speckled walk detection, the Capitol as a stepped heightfield, the dome 8 m off the crop
centre — are recorded in the world's README as limitations of the data, which is where a reader of a
future Capitol run will need them.

## Plants from the scene, and the Capitol reference run (shot G3)

**A scene tree's age is piecewise linear in its height.** The map runs through (0 m, age 0),
(`bundle.tree_mature_height` = 3 m, `tree.mature_age` = 1000) and (`bundle.tree_tall_height` = 20 m,
`bundle.tree_tall_age` = 3000), and is flat above 20 m. 3 m is where a mature sim canopy sits (a
mature tree's canopy voxels are at `h + 2` and `h + 3` over a surface at `h`), so the prompt's rule —
a tree as tall as the sim's mature height or taller starts at least `tree.mature_age` — falls out of
the corner rather than being clamped on afterwards. The map is read as
`max(tree_tall_age, tree.mature_age)` at the top corner, so no parameter setting can make it
non-monotone; `prop_import_age_is_monotone` in `src/plants.rs` is the property, with
`import_age_hits_the_growth_curve_at_its_corners` as its named regression sibling. Heights of 0 or
less, and NaN, import at age 0 rather than being rejected: the loader already validates the bundle,
and a report of a degenerate tree is more useful than a run that will not start.

**"Non-Rock" is read as "plantable", which means `ColClass::Soil`.** The prompt's rule 1 says to move
a tree off a Rock column, and rules 2 and 3 say shrubs and grass start on plantable columns. Water is
neither Rock nor plantable, and a tree standing in a pond is no better than one on a roof, so all
three use one predicate, `World::is_plantable`. The Capitol has no water column, so the two readings
give the same answer on the reference world; the difference only shows on a future scene with a pond.

**Moving is a nearest-first scan, dropping is silent, and the taller tree wins a tie.** A tree's
column is `floor(x), floor(y)` clamped into the grid. If it is not plantable, the scan visits the
offsets within `bundle.tree_move_radius` = 2 columns in order of distance, then y, then x, and takes
the first plantable one; if there is none the tree is dropped. Two trees that land on one column keep
the taller, and an exact tie keeps the one earlier in `trees.json`, so the result does not depend on
float comparison order. `tree.min_spacing` is deliberately **not** enforced on import: the scene's
spacing is the scene's, and a photographed avenue of trees 2 m apart should import as it stands. It
applies from the first germination onwards, as it always did.

**Planting runs in two passes, in ascending column order.** Pass one resolves every tree to a column
and keeps the winner per column; pass two walks the columns in index order and plants. That way the
entity ids, and therefore the RNG draws for lifespan, depend on the grid and not on the order
`trees.json` happens to list its trees. `PlantImport` counts `planted + dropped + merged = the
scene's tree count`, with `moved` a subset of `planted`, and `ecosim run` prints it:
`scene: 81 trees -> 79 planted (0 moved, 2 dropped, 0 merged); 64 shrubs over 750 columns in 87
patches`.

**Scene trees replace the noise world's random ones.** A bundle run does not call
`place_initial_trees`, so `tree.initial_count` has no effect on it. Grass needs no code at all: a
patch's initial grass already goes to its soil columns, which on a bundle world are exactly the
plantable ones.

**A shrub ellipse adds cover by column, not by area.** A patch's initial shrub density is
`shrub.initial` plus the fraction of its *plantable* columns whose centre `(x + 0.5, y + 0.5)` falls
inside any ellipse, clamped to 1. Counting columns rather than integrating area keeps the number the
same quantity the sim carries, makes a bed over a plaza contribute nothing, and makes the acceptance
("half a patch raises its density by 0.5") exact to within one column's share, 1/64.

**The import counts stay out of `meta.json`.** They are on stdout, in `RunSummary.import` and pinned
by `tests/bundle.rs`; the run-directory contract does not change, so no renderer or reader has to
learn anything new for G3. The pipes are loaded into `World::pipes` and nothing reads them yet —
that is G6 — but they are carried as data rather than left in the bundle, because the world is what
later shots hold.

**`fixtures/capitol-mini` is the Capitol at seed 42, 100 ticks, snapshots at 0 and 100**, named the
way `s42-mini` is named, 13 MB of it — a format-4 fixture is bigger than a format-3 one because it
carries the 2.3 MB `world/` grids once and a 256 × 256 × 32 `material.bin` twice.
`the_committed_capitol_mini_fixture_matches_a_fresh_run` re-runs it and compares with
`check::diff_runs`, so a change in planting shows up as a failed test. It is not synced into
`ecoview/public/`: `scripts/sync-data.sh` copies `s42-mini*`, and teaching it a second fixture is
G7's business, not an ecosim shot's.

**CI runs the Capitol at its full 20000 ticks.** It takes 5935 ms locally — 16 × the reference area
and still a fifth of the `runtime` invariant's budget — so there was no need for the shorter CI run
the prompt allows. It rides on the seed-3 leg of `ecosim-sims`, where seeds 1 and 2 already carry the
long run and the baseline sweep, and uploads its `ecosim check` output as an artifact.

**Nothing was tuned to make the Capitol check pass, and it passes anyway** (`sweeps/capitolG3/`).
The one result worth carrying forward is that the west half of the site ends the run treeless: with
`climate.rain_gradient` = 0.6 the west edge gets 40% of the mean rain, 24 of the 30 trees that ever
stood west of the middle died of drought in the first 2000 ticks, and with `tree.seed_radius` = 6 and
`tree.immigration_floor` = 0 nothing can disperse back. That gradient was chosen for the 256 × 64
strip, where a west–east climate ramp is the world's point; on a 256 m photographed site it is a 2.5×
rainfall difference across two city blocks with nothing behind it. G3 does not change it — the prompt
says not to retune — but G4 should settle whether the Capitol runs at `rain_gradient` = 0 before it
puts storms and runoff on top.
## Flat rainfall on a bundle world (shot G3a)

**A bundle-world run overrides `climate.rain_gradient` to 0; the default stays 0.6.** Shot 15 gave
the 256 × 64 noise strip a west-to-east rain ramp, and a ramp is the whole point of that world: it is
a synthetic gradient the ecology is meant to sort itself along. A world bundle is a photograph of 256
metres of real ground, and there the same 0.6 is a 2.5× rainfall difference between one edge of the
Capitol square and the other, with no measurement behind it. Shot G3's run shows what that costs:
every tree west of x = 128 dead of drought by tick 5050, and with `tree.seed_radius` = 6 and
`tree.immigration_floor` = 0, nothing can disperse back across the gap.

**Why an override rather than a new default, and why not a `[bundle]` key.** Three ways to do this
were on the table:

1. change `climate.rain_gradient` to 0 in `params.toml`;
2. have `World::from_bundle` (or `Bundle::apply_to`) force the gradient to 0 for bundle worlds;
3. pass `--set climate.rain_gradient=0` on every bundle-world run, as G0's `animals.enabled=false`
   is already passed.

(1) is wrong because it re-tunes the noise worlds, which are the regression anchor: seeds 1, 2, 3 and
42 and every committed manifest are what they are *because* of the ramp, and flattening it would
retune the strip to fix a bundle. (2) is wrong because it makes a parameter mean different things in
different worlds — the value in `meta.json` would no longer be the value the run used unless the
override were written back, and a reader comparing two runs' params could not tell why they differ.
It is also the sort of hidden special case that makes a later shot's "at rate 0 nothing changes"
argument hard to check. (3) keeps one parameter with one meaning, puts the choice where a reader of
the command can see it, records it in `meta.json` like any other `--set`, and leaves the door open to
a site that really does have a rainfall gradient. So the two overrides travel together: **every
garden-series run on a bundle world is `--set animals.enabled=false --set climate.rain_gradient=0`.**

**Where that pair is written down,** so it cannot drift: the `capitol` recipe in the `justfile` and
step 10 of `ci.yml` (`tests/ci.rs` pins the full command text of both, so dropping a flag fails a
test), `bundle_params` in `tests/bundle.rs` (which is what makes `fixtures/capitol-mini` and the
synthetic-bundle runs carry it), `README.md`, and `worlds/capitol/README.md`.

**The fixture and the reference run were regenerated, and nothing else was.** `fixtures/capitol-mini`
is the same command as before with the flag added, so its bytes change; `the_committed_capitol_mini_fixture_matches_a_fresh_run`
re-runs it and pins the new ones. The noise-world manifests, goldens and fixtures are untouched, and
`fresh_s42_matches_committed_manifest` is green: the flag exists only on the bundle path.

**What the flat run says about the site** (`sweeps/capitolG3-flat/FINDINGS.md`; `sweeps/capitolG3/`
stays as it is, as the evidence for the decision). The west half is populated — 424 trees at tick
20000 against 0 — and the two halves' mean moisture stays within 17% of each other all run instead of
58 against 173. The site is 13% smaller in trees and a quarter smaller in canopy, and its mortality
mix turns over: G3 was crowding-limited (4075 crowded, 25 drought, 17 burnt), G3a is
disturbance-limited (3315 drought, 2055 burnt, 1656 crowded). Fire is the mechanism that changes most
— 81 spreads become 576 — because ignition and spread both need fuel and dryness in the same patch,
and only a flat site has them everywhere at once. The west is still three times thinner than the east
at tick 20000, but that is dispersal from the scene's own 25-versus-54 starting trees at
`seed_radius` = 6, not climate: the west's density is still climbing (3.4, 10.8, 19.7 trees per 1000
plantable columns at ticks 10000, 15000, 20000). Nothing was tuned to improve any of this.

**The thinnest margin in the flat run is `fertility_mean` at +0.0076** (peak 218.33 against a ceiling
of 220, where the ramp peaked at 208.15): a site that all grows also all decays. It passes, so it
stands, but G4 and G5 work on that same field and G5 replaces fertility outright, so that is the
invariant to expect complaints from next.

**Building shade is left alone.** It is a hard exclusion on 2056 plantable columns (4.7%) in both
runs, since `tree.light` = 60 and a shaded column's surface light is 0. The operator's call is to
revisit it when the water work makes it matter (and shot G9 replaces the fixed sun outright), so this
shot does not touch it.

## Shot G4 — surface water, soil water and storms

- **Water is f64 in memory and small on disk.** The ledger is f64 and every transfer inside a storm pass is f64, because the acceptance is a 1e-9 relative balance and f32 cannot hold it over 262144 cells. The stored fields are as small as they can be read: `water.bin` is u16 tenths of a millimetre per ground cell (6.5 m of ponding, 0.1 mm resolution), `soil_water.bin` is f32 millimetres per ecology column. A snapshot of the Capitol costs 0.8 MB for the pair.
- **"Soil deficit" is read as a saturation ceiling above field capacity,** `hydro.saturation` × capacity. A column takes water up to the ceiling, holds capacity against gravity, and percolates the difference away at the medium's rate. Without the gap there is no drainage and so no leaching, so this is what gives fertility a sink.
- **Fertility loses what percolates** (`hydro.leach_k` per mm drained), and `check --long` gained a `fertility_mean` line so the old saturation cannot come back unnoticed. `TUNING.md` has the 200000-tick numbers.
- **Every run with the water tier on writes format 4,** noise worlds included, with a synthesized all-soil `world/` grid; `meta.json`'s `world.bundle` distinguishes the two. The alternative — a fifth version for "water but no bundle" — would have split the version line over an axis a reader does not care about, since what a reader needs to know is whether a ground grid is there. `ecosim fork` therefore accepts format 4 on a noise world (it can rebuild a synthesized grid from params) and still refuses a bundle one.
- **`--format-version 2` still writes a strictly version-2 directory,** water files and all, because "each version only adds files" is the contract the renderer's loader is written against. `write_snapshot` takes a `water: bool` for it; the ledger check runs either way.
- **`runoff_mm` is attributed to the rain that fell on the cell,** not summed over every cell-to-cell transfer: what leaves a cell is split between its own rain and its run-on in proportion. Summing transfers counted water crossing 300 cells 300 times and reported runoff at 22164% of rain. Attributed, it is at most `rain_mm` and the ratio of the two is the storm's runoff coefficient, which is the number the FINDINGS tables want.
- **Moisture is derived, not stored.** The field is a `Vec<f32>` holding `255 × min(1, soil_water / field_capacity)`, recomputed on the soil update and quantised to u8 only when `moisture.bin` is written, so soil water is the single source of truth and the renderer's overlay keeps working unchanged. (Shot G4b corrected this line, which called the field u8, and replaced `draw_moisture` with `draw_water_mm`: a plant's demand is in millimetres, so there is no longer a conversion back from the index.)
- **The flow graph is built once at load and never rebuilt.** Depressions are filled by priority-flood, receivers are D8 over 4 neighbours, and the order is Kahn's; roofs are lifted by their building height so water leaves them by their downspouts (BFS from each pipe inlet) rather than down their walls. Any cycle the fill leaves is cut by routing its entry cell to `OUT`, chosen by lowest index so the cut is deterministic. Buildings and terrain do not move during a run, so a static graph is exact, and it makes the storm pass one linear sweep: 2.85 ms at 512 × 512.
- **A storm is one whole-world event** with `detail` `"<depth> <runoff> <outflow>"` in millimetres, matching the tick's series columns, rather than one event per rained-on cell. Per-cell rain is a field, not an event.
- **The manifest, `s42-check.txt` and the `s42-mini-v2` and `capitol-mini` fixtures were regenerated,** because the water tier changes behaviour on every world. `fixtures/s42-mini` stays as it is: it is the format-1 fixture.
- **Two forced-extinction tests needed stronger forcing** (`fire.base_rate` 1 → 20, `hunter.hunt_cost` 5 → 8, both test-local `--set` values, not defaults). The tier holds moisture near saturation and ignition scales with the square of dryness, so fires are roughly 25× rarer than they were. Raising the forcing keeps the test testing what it names — that extinction runs to 20000 ticks without a panic — instead of quietly passing because nothing died.
- **The ecoview CI job pins its two ecosim runs with `--set hydro.enabled=false`.** Rate-0 identity makes that data byte-identical to what the committed reference screenshots were rendered from, so the renderer's pixel tests stay honest until a renderer shot teaches it the water files. **Shot G8 removes the pin.** `tests/ci.rs` pins the ecosim job's commands only, so the pin does not fight it.

## Shot G4b — units calibration

The audit is `UNITS.md`, committed on its own before any conversion. This section is the judgement
record: what the unit system is, what was converted, what was deliberately left, and which standing
rules the operator suspended for the shot.

**One tick has one duration, and it is written down once.** `hydro::HOURS_PER_YEAR = 8766.0` is the
only place the length of a year appears, and `hydro::tick_hours(p) = HOURS_PER_YEAR / year_len` is the
only place a tick's length is computed; `hydro::years(p, n)` is the same division for a span of `n`
ticks. At the shipped `climate.year_len = 4000` a tick is **2.1915 h**, so a 20000-tick reference run
is **5 simulated years**. That duration was not chosen by this shot — it was already implied by the
water tier's mm/h rates — and the prompt's instruction was to adopt it rather than contradict it.
Everything else follows from it: **every rate in `params.toml` is per hour or per year**, and a
subsystem that updates every `N` ticks turns its rate into an amount by multiplying by `years(p, N)`
or `N × tick_hours(p)`. `tests/units.rs` pins both halves of that sentence — the constant appears
exactly once, only `hydro.rs` divides by `year_len`, and five named call sites charge their rate over
the update's own length.

**The consequence is that a cadence is a schedule and not part of a rate,** so the four hard-coded
cadences became parameters in a new `[schedule]` section: `cover_every` (was the two literal `10`s on
one line in `producers.rs`), `soil_every` (was `is_multiple_of(10)` in `sim.rs` *and*, separately, the
`10.0` in `abiotic.rs` that converted it to hours — the desynchronisation the prompt warned about, now
one read), `temperature_every` and `fire_every` (was `fire::FIRE_EVERY`). The other four cadences were
already parameters and stay where they are. None of this is a mechanism: the same code runs, it just
asks where it used to assume.

**Soil water is the physical quantity; the 0–255 moisture index is a reading of it.**
`medium.*.field_capacity_mm` is declared as the **available water capacity of the rooting zone** — a
store of 0 is the permanent wilting point, not oven-dry soil — and the derived index is
`255 × soil_water / field_capacity`. Every threshold that used to be a number on that index is now a
**fraction of available water capacity**: the three species' `moisture` curves, `tree.dry_fraction`
(was `dry_moisture` = 30 of 255, now 0.12) and fire's dryness. This is the conversion that touches the
most rules, and it is why the curves' fourth element is 1.004 rather than 1: a curve is "0 at or above
max", so the max has to sit just above full capacity.

**Plant water demand is in millimetres, drawn from the soil store.** `cover.moisture_draw` became
`cover.water_per_growth_mm` (mm of water per unit of new cover) and `tree.moisture_draw` became
`tree.transpiration_mm_h`. The old draws were amounts of the index, which the water tier then
multiplied by the column's own capacity, so a plant on a deep soil drew more water for the same growth
— that rule was wrong rather than merely mis-scaled, and `UNITS.md` lists it as such.

**A tolerance of 5% on the cadence-doubling test, and what it is not measuring.** A rate charged over a
longer update is charged the same amount per year but lands in fewer, larger steps, so a logistic
increment is evaluated at a slightly different cover and a rate-limited flow can be limited at a
different moment. That error is of the order of one update's share of the year, about 1% at these
cadences; 5% leaves room for it to compound over a year while still failing anything accidentally per
tick, which would be off by a factor of two. The test asserts the five quantities a declared rate is
responsible for (standing cover, litter, rain, evapotranspiration, mean temperature) and deliberately
**excludes** drainage, runoff, ponded evaporation and the standing stores, because none of those is set
by a rate: what drains, runs off or leaves the world is the residual of a store with a ceiling, so how
often the store is emptied decides how much of the next storm it has room to take. Measured at the
shipped defaults, doubling `soil_every` leaves the year's rain untouched and its evapotranspiration
within 0.3%, moves annual drainage by 1.0%, and moves runoff by 14% and outflow over the world's edge
by 11%. That is the water tier's integration error — a property of the model, measured in
`sweeps/shotG4b/FINDINGS.md` — and hiding it behind a loose tolerance would have been the wrong way to
record it.

**Where the line was drawn, and what shot G4c gets.** Converted here: climate and rain, soil water,
plant water demand and growth, fire, and the two nutrient rates that read a clock or a water flow.
Deferred, with every row marked in `UNITS.md` and the list repeated in
`overnight/shots/G4c-units-calibration-rest.md`: light (four curves, the sapling threshold and building
shade have to move together), the fertility and detritus indices (shot G5 replaces that field outright,
so converting the index would be converting a quantity about to be deleted), lifespans and phenology,
the legacy non-water moisture model, the whole animal tier, the immigration intervals, and the six
`SIG_*` constants. The prompt sanctions stopping at a subsystem boundary, and these are boundaries:
each changes which columns germinate or how long a thing lives, and none can be half done.

**`climate.decay_k` is a unit change with no value change; `hydro.leach_k` is both.** `decay_k` 0.015
per soil update is exactly 6.0 per year at the shipped cadence, so it is rewritten and not retuned —
which leaves the audit's finding standing, that 6.0 a year is a litter turnover of two months against a
published one to three years. `leach_k` moves 0.0002 → 0.0008, because the old value was fitted against
the pre-G4b rain of 4000 mm a year: with rain corrected to 800 mm, drainage falls with it and fertility
loses the sink that held it down. The new value is derived (mobile share of the pool over the rooting
zone's capacity) and lands at 26% of a column's fertility a year against a published 15–40% for nitrate
loss. Both are in `TUNING.md` with the acceptance line that forced them.

**Fire is converted and not retuned.** `fire.base_rate` 0.002 per fire update becomes 0.8 ignitions per
patch per year, the same rate in the declared unit, and fire's dryness term now reads the soil store as
a fraction of available water capacity instead of the 0–255 index. The operator's note of 2026-09-20
02:36 is explicit that the resulting ignition count is to be reported and not tuned, so
`sweeps/shotG4b/FINDINGS.md` gives it before and after and stops there. `fire.spread` stays a
per-attempt probability with the fire's `duration` in ticks: spread is a within-event geometry, not a
rate per unit time, and converting it belongs with the fire model itself (backlog row G4d).

**The health checks are re-expressed in years, read from the run's own `meta.json`.** `WINDOW_START`,
`TREE_ANCHOR`, the 20000-tick run length, the tick-10000 sample, `LONG_TICKS` and `LONG_BAND_FROM`
became `WINDOW_YEARS` 0.5, `TREE_ANCHOR_YEARS` 1.25, `RUN_YEARS` 5.0, `SAMPLE_YEARS` 2.5, `LONG_YEARS`
15.0 and `LONG_BAND_FROM_YEARS` 5.0, through `ticks_in(years, year_len)`. At the shipped year length
every one of them is the tick count it replaced, so no run's verdict changes by arithmetic. **No
threshold was widened and none was retired**: the grazer-cycle windows, `CAUSE_WINDOW` and the `SIG_*`
lags stay in ticks with the reason written at each — they describe the per-tick animal tier, which is
not converted, so a ruler in years would measure a per-tick cycle. One check is **new**, as the prompt
says it must be: `moisture_band` asserts the field mean stays above the wilting point on every tick of
the window and below field capacity on at least 95% of them. Both bounds are the soil's own rather than
chosen numbers — at either one no plant's moisture curve responds to anything — and the 95% is there
because a storm briefly fills every column.

**The regression anchor, per operator override 2, is suspended and replaced** by "the reference worlds
still run to full length with plants surviving and pass the re-derived checks". Recording the
substitution is part of the override, and so is this: **byte identity could not have been kept even in
principle.** Measured on the 100-tick format-1 fixture, the only columns that move are `grass_mean`,
`moisture_mean`, `fertility_mean` and `detritus_total`, in the fourth decimal, because the derived
constants are rounded to three or four significant figures where the old ones were exact; and on any
longer run the tree draw's correction from 2352 to 300 mm a year makes the break structural. A
conversion that preserved bytes would have had to preserve the numbers it exists to change.

**So the manifests are re-cut under new names and the old ones are kept as history.** The five live
manifests are `tests/data/s42-manifest-g4b-{64,heredity-off,crowding-off,fire-off,water-off}.sha256`;
`s42-manifest-preG4`, `-prefire`, `-preshot10`, `-preshot11`, `-preshot14a` and `s42-64-manifest` stay
on disk untouched and are no longer claimed to be reproducible.
`the_pre_conversion_manifests_are_kept_as_history` is what keeps that honest: for each pair it asserts
the same file set, byte-identical tick-0 `material.bin`, `light.bin` and `height.bin` (the terrain is
upstream of every rate, so the world is still the same world), and that `series.csv` and the last
snapshot's `patches.json` differ. An archive nothing checks rots; an archive checked for the wrong
thing is worse.

**Regeneration is a switch, not an edit.** `ECOSIM_REGEN_MANIFEST=1 cargo test --release --test sweep`
rewrites every manifest and `s42-check.txt` from the run the test just made; without it the same test
asserts. The alternative was a throwaway script, which is what the last four manifest regenerations
used, and which cannot be relied on to hash the same file set the assertion reads.

**`format_2_and_fire_only_add_to_version_1_files` is now a claim about the format, not about the
numbers.** `fixtures/s42-mini` is the format-1 fixture and there is no format-1 writer left to rewrite
it with, so the test can no longer compare a fresh run's values to it. It now asserts what it was
always named for: that the version-2 run's tick-0 snapshot is byte-identical to the version-1 one after
the files version 2 adds are cut, that the later snapshot's shared fields still match, that the series
header and row count line up, and that the params that were renamed are exactly the four in its
`RENAMED` table. What it no longer asserts is that the values are unchanged, which is what this shot
changes on purpose.

**Behavioural tests moved off the flat world.** `common::SQUARE` (the 64-world with
`rain_gradient = 0`) keeps the byte fixtures; the six forced-extinction tests now run on a new
`common::SMALL`, the same world at the default west–east rain gradient. The reason is a finding, not a
convenience: at the corrected 800 mm a year a flat, evenly watered world **cannot keep a tree**,
because a tree's 300 mm a year is charged to its trunk column on top of that column's grass
evapotranspiration, so every column is equally marginal and germination falls by 85%. A
forced-extinction test has to force one mechanism in a world that is otherwise healthy, and with the
gradient the same world is healthy (142 mature trees). One test needed its forcing raised as well
(`grazer.energy_cost` 2.0 → 3.5) and one restated in the new units (`fire.base_rate` 20 → 8000, which
is the same 20 per update); both are test-local `--set` values, not defaults, and both are in
`TUNING.md`.

**The tree's double charge is reported, not fixed.** A mature crown covers nine columns and draws from
one, which is why `tree.transpiration_mm_h` is set at the bottom of its published range (300 of
300–700 mm/yr). Spreading the draw over the crown is a change to the water mechanism, and this shot
adds no mechanisms; it is `UNITS.md` finding 3 and it is named in the G4c prompt.

**One acceptance line belongs to a deferred subsystem and is not claimed.** "A tree planted at the
start reaches a height within the published curve's band at the ages sampled" needs a tree height and
an age in years, which is the lifespans-and-phenology subsystem — trees have no height at all today,
only an age in ticks. The prompt's limiter says the acceptance lines that test converted quantities
apply only to the subsystems actually converted, so this one is recorded in `UNITS.md` section 7 as the
shot's largest finding (the tree tier is out by 25–60×) and handed on.

## Shot G4c — units calibration, part two

The judgements of shot G4c, which converted the two subsystems its prompt made non-negotiable —
light, and the tree tier's ages — and handed the other three on. Filed under the heading the prompt
asks for, "Units calibration, part two".

**Light is a fraction of full sun, not PAR in mol m⁻² d⁻¹.** The prompt offered both and asked which
and why. A fraction is what every rule in the model actually needs: the four light curves are
suitability curves, which are comparisons against a ceiling, and a ceiling expressed as "full sun"
needs no absolute irradiance behind it. Introducing a mole would mean choosing a site's daily PAR
total, giving it a season, and writing every curve against a number the model has no other use for —
a calibration with no reader. `light.bin` also stays a byte, and a byte that holds `255 ×` a fraction
is exactly the byte it held before, so no renderer and no format version moves. If a later shot needs
absolute PAR — a photosynthesis rate would — it multiplies the fraction by one site constant, and
nothing written now has to be unwritten.

**The canopy became a product, and that is the one rule this shot changed.** `255 − absorb × layers`
is a subtraction that saturates, so from three canopy voxels down a column received *exactly* zero
light and every suitability curve read 0. Beer–Lambert (`exp(−k·LAI·layers)`) never reaches zero.
`canopy_k = 0.5` and `canopy_lai = 2.0` put a mature two-voxel crown at LAI 4 transmitting 13.5%,
inside the published 10–25% at LAI 3–5 (UNITS.md R10), which is the shot's acceptance line. Worth
recording honestly: the old rule *also* transmitted inside that band at two layers (21.6%). The
conversion earns its keep at one layer (60.8% → 36.8%) and at three or more (0% → 5.1%), and it is
the one-layer row that moves the reference runs — a young tree now casts shade a grass species cannot
grow in, so germination and tree counts rise on every seed and ground cover falls under them
(`sweeps/shotG4c/FINDINGS.md`).

**The fixed sun is now an angle, and the angle is the approximation.** `bundle.shade_slope = 1.0`
became `bundle.sun_altitude_deg = 45.0`, because `1/tan(45°) = 1` exactly and so no building's shadow
moved by a voxel, and because a slope hid what it was a slope of. The value is also defensible on its
own: Lansing's noon sun near the equinox is 47°. Naming the degree exposes the modelling error the
slope concealed — a shaded voxel is fully dark, which is only true with no diffuse sky light. That
belongs to backlog row G9's moving sun, and this shot did not add a sun, as its prompt says twice.

**The tree ages were converted and deliberately not corrected.** This is the shot's largest judgement
and the prompt allowed it explicitly: "either a tree's age in years matches the published
height-by-age band at the ages sampled, or DECISIONS.md says which of the three ways out was taken
and why the line still cannot be claimed." None of the three was taken, because each is closed:

1. **A 200000-tick reference run** fails the runtime invariant, and a standing rule of this series
   forbids widening an invariant to make a gate pass. The prompt's own override 4 says the same in
   narrower words: a check may be retired because its units are gone, never widened because the run
   now fails it.
2. **Per-tier time scales** is a mechanism. The prompt names it and pre-empts it: "If the choice turns
   out to need a mechanism (option 2 does), stop and write it up rather than adding one."
3. **A longer tick** would break every mm/h rate the water tier was calibrated on in shot G4b —
   undoing the only physically calibrated tier in the model to fix one that is not.

So `initial_age`, `young_age`, `mature_age`, `max_age`, `seed_every` and `bundle.tree_tall_age`
became `initial_age_years`, `young_age_years`, `mature_age_years`, `max_age_years`, `seeds_per_year`
and `tree_tall_age_years`, at values that reproduce the old tick counts exactly at `year_len = 4000`.
The gain is not behavioural, it is legibility: `params.toml` now says a tree matures at 0.25 years and
dies at 1.5, which a reader can check against a published 5–8 years and 60–150 years in one step,
where `mature_age = 1000` gave no one anything to check. The error moved from hidden to stated, which
is what an audit is for.

**A second reason the height-by-age line cannot be claimed, independent of the time scale: the
model's trees have no height.** A tree has an age and a stage, and a crown of one voxel or two. There
is no metre to compare with "3 m at 5–8 yr". `bundle.tree_mature_height` maps a *scene* tree's height
onto an import age and is not a property the sim maintains. Any shot that wants that acceptance line
has to give trees a height first, which is a mechanism.

**`Params::tree_ages()` converts once, at the point of use.** The seven converted ages are read
together through one method returning a `TreeAges` of tick counts, rather than each call site dividing
by `year_len` itself. That is the same discipline `HOURS_PER_YEAR` enforces for the water tier — one
place that knows how a year becomes ticks — and `tests/units.rs` already fails if a module divides by
`year_len` on its own.

**Seeding was a schedule and is now a rate, and the schedule was wrong.** A mature tree seeded when
`age % seed_every == 0`. A tree's age advances by whole `tree.update_every` steps, so that test can
only fire when `update_every` divides `seed_every`. It does at the shipped 50 and 200, which is why
nothing had caught it. Turning the parameter into `seeds_per_year` makes the broken values reachable
by an operator — `seeds_per_year = 30` gives `seed_every = 133` and a tree that seeds once per 6650
ticks instead of 133 — so the test became "the update whose age crosses a multiple of `seed_every`".
It picks exactly the same ages at the shipped values **for a tree whose age starts at a multiple of
`tree.update_every`**, because stepping by 50 preserves the residue. That is every tree on the strip
— germinated trees start at 0, initial trees at 500 — and not every tree on a bundle world, where
`plants::import_age` turns a height into an arbitrary age. 63 of the Capitol's 79 imported trees
have an age that is not a multiple of 50 and so could never satisfy `age % 200 == 0`: they never
seeded in any run before this shot. The Capitol's germination count goes 7645 → 19408 and its trees
at tick 20000 1619 → 4082, which makes this the largest behaviour change in the shot on a bundle
world, larger than the light rule that motivated it. It is a bug fix, not a retune: no default
moved, and the run still passes every check. This is the second rule the audit found to be
wrong rather than mis-scaled, after `draw_moisture` in G4b, and it is listed separately as the prompt
requires (UNITS.md finding 12).

**The byte-identical anchor stayed suspended, and what replaced it.** Per the prompt's override 2,
the substitute is: the reference worlds still run to full length with plants surviving and pass the
re-derived checks. They do — seeds 1, 2, 3 and 42 on the strip and the Capitol bundle, all five
passing every `ecosim check` invariant with the thinnest margin in the set improved rather than
eroded (seed 1's `mature_trees_10k` went from 38 against a floor of 35 to 701). Recorded here because
override 2 asks for the substitution to be recorded, not just used.

**One byte comparison was narrowed, and it is a retirement, not a widening.**
`format_2_and_fire_only_add_to_version_1_files` compared the tick-0 `light.bin` of a fresh run with
the version-1 fixture byte for byte. The fixture cannot be re-cut — there is no version-1 writer —
and the quantity has changed rule, so the comparison could not survive as equality. It was replaced
by a *stronger* assertion, not a weaker one: `light_is_the_v1_file_re_extincted` checks that every
byte of the fresh file is the Beer–Lambert re-expression of the byte version 1 wrote at the same
index, so "only the light rule changed, column for column" is now the thing under test. Override 4
asks which of the two kinds of retirement this is: it is "the old units no longer exist", and the
replacement is tighter than what it replaces.

**A second byte comparison was narrowed, the same way and for the same reason.**
`sweep.rs`'s `the_pre_conversion_manifests_are_kept_as_history` asserted that `material.bin`,
`light.bin` and `height.bin` at tick 0 hash the same in the pre-conversion manifests as in the `-g4b`
cuts that replaced them — "the world a seed makes has not moved". Light has moved: the twelve young
trees planted at tick 0 shade their columns to 94 where they shaded them to 155. `light.bin` is now
asserted to **differ**, and the terrain files still to match, so the claim is narrowed to what it was
really about and nothing is merely deleted. Same classification as above: the old units no longer
exist. The `-g4b` file names are left alone; a manifest keeps the name of the shot that cut it, not of
every shot that regenerates it, and renaming five files would make the history harder to read, not
easier.

**`libm`, not `std`, for the new exponential.** `clippy.toml` disallows `f32::exp` and `f64::tan`
because MSVC and glibc do not agree on them, and both are now on the hot path of a deterministic
simulation: the canopy's `exp(-k * LAI * layers)` runs per voxel and the sun's `1 / tan(altitude)`
once per load. Both go through the `libm` crate, which is the rule shot 15a's cross-profile
determinism test exists to protect. Worth naming because it is the kind of thing a units conversion
introduces by accident: the old rule was integer subtraction and needed no transcendental at all.

**Fixture regeneration became part of the same switch.** `fixtures/capitol-mini` and
`fixtures/s42-mini-v2` were the last two committed artefacts still needing a throwaway script to
re-cut, which DECISIONS.md ("Regeneration is a switch, not an edit") already argued against. Both
tests now rewrite their fixture under `ECOSIM_REGEN_MANIFEST=1`, the switch shot G4b left for the
manifests and `s42-check.txt`, so one environment variable re-cuts everything a behaviour-changing
shot has to re-cut. Nothing sets it in CI.

**`docs/SAD-addendum.md`'s Light section was updated; its Trees section was flagged, not fixed.** The
addendum documents what `light.bin` holds, and this shot changed what the byte means (not its range,
not the format version), so the repo rule "if you change the format, update the SAD with it" applies
and the section now states the Beer–Lambert rule and its three transmittances. The addendum's Trees
section was already stale from G4b — it still describes a 0–255 moisture threshold and a
`tree_moisture_draw` that no longer exists — and fixing it is not this shot's scope, so a single
pointer line now says it describes the pre-calibration model and names `params.toml` and `UNITS.md`
as the current source. A stale doc that says it is stale is not the same failure as one that does not.

**The limiter bound, and the shot stopped at the subsystem boundary.** Light and lifespans landed;
the fertility and detritus indices (subsystem 3, with `climate.decay_k`'s 10× error), the parked
animal tier (subsystem 4) and the legacy moisture model (subsystem 5, to be left alone and said so
again) did not. `overnight/shots/G4e-units-calibration-rest2.md` names exactly what remains; every
unconverted row in `UNITS.md` is still marked `deferred` and section 6's heading now says G4e instead
of G4c. Per the prompt's limiter this is a normal "shot G4c:" commit with the backlog row set Done —
"a shot that lands light and hands the rest on has succeeded" — and the handoff is stated in the LOG
line as well as here.

## Shot G11 — the tree-footing invariant moves into `ecosim check`

**The invariant now lives where it can be violated.** `ecoview/tests/e2e/capitol.spec.ts` asserted
that no Capitol tree stands on a roof or a road. It stated the right rule, but read `entities.json`
and `world/medium.bin` over HTTP and examined no pixel: an assertion about *this* component's output,
executed by Playwright. That cost has been paid twice — shot G4c's seeding fix tripled the Capitol's
trees, failed the browser test, and blocked an ecosim shot not allowed to touch it. It is now the
`tree_footing` line of `ecosim check`, fixable by the shot that breaks it.

**One report line, not three.** The rule has three parts — no roof cell under any tree, never more
than half a column's cells sealed, and a floor on the share of trees on no sealed cell — reported as
one `CheckLine` whose margin is the tightest of the three, the way `moisture_band` already combines a
per-tick floor with a share-of-ticks bound. The observed string carries the whole per-tree histogram,
so a failure names which part gave way without three keys in every sweep CSV.

**`Medium::is_sealed()` is the only definition of sealed ground**, and `Ground::cells_of` the only
column-to-cells mapping. The browser copy re-derived the sealed set from CSS colours and, until shot
E6, left `concrete` out of it.

**Every snapshot, because it is cheap.** The prompt allowed falling back to first, last and tick
10000 if all snapshots cost over about two seconds. Measured on the Capitol reference run — 201
snapshots, 41 MB of `entities.json`, 460,880 tree sightings — the footing pass adds **0.13 s** (best
of five, against 0.04 s with the invariant n/a; 0.77 s on the first read after the run is written).
So all 201 are read, and a tree that germinates on a roof at tick 3000 and dies at 4000 is still
seen, which the final snapshot alone cannot do.

**Report order is append-only**: `tree_footing` goes last and `INVARIANT_KEYS` gains it at the end,
so a sweep CSV's invariant columns keep their order and a noise run's not-applicable list is
`ANIMAL_ONLY_KEYS ++ BUNDLE_ONLY_KEYS` with no interleaving.

**Not applicable, not silently passing, without a bundle ground grid.** A format-3 run has no
`world/`; a noise world at format 4 has the synthesised all-soil grid its columns mirror, in which
nothing is sealed. Both report `n/a (no bundle ground grid)` through a new `push_na_because`, so
`push_na`'s hard-coded "animals off" reason is untouched and the two cannot be confused. Seeds 1, 2
and 3 report it n/a and still exit 0. A `world/` that exists but cannot be read, or a tree outside
the grid, is a **failure**, not an n/a.

**`lawn / cells > 0.95` was not carried across; it was restated per tree, per snapshot, at 80%.** The
old third assertion was a share over all cells under all trees — the same scale-dependent family as
the asphalt proxy E6 had just replaced, measured at 95.6% against a 95% bar. Its replacement is a
floor on the share of trees standing on **no** sealed cell. It is taken per snapshot because a run
ends with most of its trees: the Capitol reads 93.38% over all 460,880 sightings but 89.26% in its
worst single snapshot, tick 10200 of 996 trees. 80% leaves nine points of margin there, where the
retired proxy had six-tenths of one. Known fragility: a bundle with a handful of trees, one beside a
walk, could drop under 80% with nothing wrong. The Capitol is the only bundle in the repo and no
snapshot of it holds under 79 trees; the shot that meets a small bundle may restate the floor, which
is the point of moving the check here. A snapshot with no tree is skipped, not counted as 0%.

**One test was deleted from `ecoview/` under the row's narrow override.** The `test(...)` block and
the `PAVED` and `RUN` constants it alone used went with it; `git diff --stat` over `ecoview/` shows
that one file and nothing else, in the same commit that adds the check, so the invariant is never
unguarded for a commit. The Capitol tests that are genuinely about rendering are untouched.

**`tests/data/s42-check.txt` was re-cut and nothing else was.** The golden is the text of `ecosim
check` on seed 42 and this shot adds a line to it on purpose, so it gains one `n/a` row under
`ECOSIM_REGEN_MANIFEST=1` (and a fresh wall time, which that test compares by name only). No run
manifest and no fixture moved: the check only reads, so the run bytes are identical.

**The 250-line budget did not hold, and the shot did not trim tests to fit.** See the LOG line and
`overnight/shots/G11.BLOCKED.md` for the measurement.

## Shot G4e — closing the units series

**`tree.immigration_interval` became `tree.immigrants_per_year` = 8.0, and nothing else converted.**
Three sibling keys held 500 ticks: `grazer.immigration_interval`, `hunter.immigration_interval` and
`tree.immigration_interval`. Only the tree's is now a rate per year, the shape `tree.seed_every` took
in G4c. At the shipped `climate.year_len = 4000`, 8 checks a year derives to exactly 500 ticks, so no
run moved: `ecosim diff` between a binary built at 179762d with the old `params.toml` and this one
reports `differs: meta.json` and nothing else, on seed 42 on the strip and on the Capitol, and inside
`meta.json` the only difference is the key's own name and unit.

**The derivation lives on `Params`, not in `TreeAges`.** `TreeAges` exists because a tree age has to
be read against a tree's own age counter, and its doc comment says "Nothing else converts a tree age".
Immigration is not a tree age: `Sim::immigrate` tests the **world** tick counter in its own phase
right after animals (this file, shot 11). Putting the cadence in `TreeAges` would have meant either
that comment becoming false or a struct quietly holding two different kinds of thing. So the shot
added `Params::ticks_between(per_year)` — the one place a rate per year becomes a cadence in ticks,
`round(year_len / per_year)`, at least 1, and `u32::MAX` at a rate of 0 so the event is simply left
out — and a named accessor `Params::tree_immigration_every()` on top of it. `TreeAges::seed_every`
now calls the same function instead of repeating the arithmetic, which is how the two stay one rule.

**The tree and the two animals are now deliberately inconsistent, and that is the decision.** After
this shot a tree immigrates at a rate per year and a grazer or a hunter still immigrates every N
ticks. The animal tier is parked by the operator's direction of 2026-09-19; converting its parameters
is work that the shot which unparks it would have to re-judge anyway, and `[grazer]` and `[hunter]`
hold 45 keys on an undeclared energy index that no conversion of one cadence would make physical.
The two keys are deferred **with the tier**, not overlooked. `UNITS.md` section 6 says so under its
own heading, with the same owner as the rest of the tier, so a reader who meets the asymmetry in six
months finds a reason rather than an oversight.

**`tree.death_detritus` is deferred with the fertility group, not with the tree tier.** It is the one
other `deferred` row left in `[tree]`. It is a quantity on the detritus scale, which is the fertility
index in another coat, and G5 deletes that field; converting it here would be converting a number
about to be removed. Recorded because `[tree]` read "2 deferred" with only one of the two explained.

**Two fixtures' `meta.json` were regenerated and nothing else was.** `fixtures/capitol-mini` and
`fixtures/s42-mini-v2` embed the params block, so the rename shows up in them: `git diff --stat` over
`fixtures/` is 2 files, 2 insertions, 2 deletions, and the change inside each is the single key. No
manifest and no golden moved — `fresh_s42_matches_committed_manifest` passes against the committed
`tests/data/s42-manifest.sha256`, which is the byte-identity gate on `series.csv`, `events.csv` and
every snapshot file. `timing.json`'s wall time is machine noise and was reverted rather than committed.

**The audit is closed for the garden direction.** `UNITS.md` now says at the top that every remaining
`deferred` row is deferred by decision with a named owner, and the file is a record rather than a work
list. Three owners: G5 for the fertility and detritus group, the shot that unparks the animal tier for
the tier and its six `SIG_*` constants, and **nobody** for the legacy non-water moisture path, which
is retired in place — it exists so that pre-G4 runs still reproduce, and converting it would defeat
the only reason it is still there.

## Shot S1 — a committed bundle-world run that keeps its animals

**The blind spot this closes, stated once.** Every committed bundle-world run — `runs/capitol-s42`,
`fixtures/capitol-mini`, and the copy of that fixture the renderer serves — carries
`--set animals.enabled=false`, because the garden direction parked the animal tier (MASTER.md,
2026-09-19 evening). So until this shot **no committed run had an animal in it**, and anything that
reads an animal was ungated: a reader could mishandle grazers and hunters and still pass CI, both
projects' test suites, every reference screenshot and a human review. That is not hypothetical. Shot
V1's entity reader declared `entities.json`'s `x` and `y` as `i32` and four independent checks missed
it; it surfaced only when shot V2 made a throwaway animals-on run by hand. The checks were
independent in who ran them and not in what they ran on.

**The fix is an added fixture, not a flip of the existing one.** `fixtures/capitol-mini` is read by
the renderer's pixel tests, so turning its animals on would move every one of them and buy nothing:
the point is to have a run with animals, not to stop having one without. `capitol-mini` is untouched
and a test now asserts that it still holds no animal, so the pair stays a pair.

**`fixtures/capitol-animals-mini` is the Capitol at seed 42 with the tier left on**: 2000 ticks,
snapshots at 0 and 2000, `--set climate.rain_gradient=0` and nothing else. Flat rainfall stays
because it is about the *site* — a 2.5x west-to-east rain ramp is the noise strip's point and means
nothing on 256 m of photographed ground (shot G3a) — while `animals.enabled` is left at the file's
own `true`, which is the whole reason the fixture exists.

**2000 ticks, because that is where the second defect lives.** The tier needs time before it is worth
gating anything against. At tick 0 the run holds the 300 grazers and 20 hunters the parameters place,
which is enough to break an integer position reader but not enough to crowd a patch: the busiest of
the 1024 patches holds 3. By tick 2000 it holds **95 grazers, against the 32 a reader gets from
`2 x disease.grazer_threshold`** — the scale top shot V2's crowding overlay picked, having no
committed run to check it against. 2000 is also the length of the private animals-on run the operator
measured 94 on, so the committed fixture reproduces a number that was already written down.

**Two snapshots, matching `capitol-mini`'s shape.** A snapshot of this world is 8.8 MB, so a third
costs more than the timeline it would buy; ticks 0 and 2000 are the two states the fixture is for.
The whole directory is 22 MB, against `capitol-mini`'s 19 — the extra is `entities.json` at tick
2000, which is 1.7 MB of animals, plus 2000 rows of `series.csv` and `events.csv`.

**The fixture is pinned the way every other one is**, by a fresh run of the documented command
compared byte for byte (`timing.json` apart), and regenerated through the same
`ECOSIM_REGEN_MANIFEST=1` switch. A committed picture nobody compares drifts, and this project has
one of those already.

**A second test asserts what the fixture is for, not just what it is.** Byte-equality would still
pass if a future regeneration quietly lost the animals — the fixture would simply become another
animals-off run and the blind spot would reopen silently. So
`the_animals_fixture_carries_what_an_animals_off_run_cannot` reads the committed bytes and asserts
the three properties the row was opened about: the tier is on, both snapshots carry animals whose
positions are JSON *floats*, and the busiest patch is over the crowding scale top. It needs no run,
so it costs 20 ms and is the test that fails first if anything goes wrong.

**The positions are floats in the file and whole numbers in value, and the test says which matters.**
`Animal::x` is documented in `src/animals.rs` as integer-valued, and on this run every animal sits on
a whole metre at both snapshots. So the description of shot V1's bug as "the simulator writes animals
at continuous positions" is true of the *type* and not of the values: what breaks a reader that
declares these fields `i32` is the `.0` serde writes, and that is what the test asserts
(`Value::as_i64` refuses `81.0`). A test written against fractional coordinates would pass today and
prove nothing.

**`meta.json` cannot say that a run has animals — only that it does not.** Shot G0 writes the
top-level `animals` key when the tier is off and omits it otherwise, and `params.animals` is skipped
at its default, so "animals on" is the absence of two things rather than the presence of one. The
test asserts the absences and then proves the positive fact from `entities.json`, which is the only
place it is actually recorded. This is the same shape as the omit-at-defaults problem backlog row S2
describes, met in a different field.

**Nothing is copied into `ecoview/public/`.** The handoff script is not taught about this fixture
either. `ecoview` is frozen and will grow no test that reads it; `ecoview-native`, the primary
viewer, opens paths on disk directly and already reads `../ecosim/worlds/capitol` that way, so it
needs no copy. Adding a line to `scripts/sync-data.sh` without committing its output would leave 22 MB
untracked in a directory whose siblings are all tracked, which is a trap for the next worker's
`git add -A`. The shot that actually serves this fixture in a browser can add both halves together.

**What the fixture's run actually does, and one thing it must not be read as saying.** The event log
over its 2000 ticks: 9545 grazer births against 409 `crowded`, 211 `eaten` and 21 `starved` deaths;
11 hunter births and 3 `starved`; 307 tree germinations against 353 `drought` and 9 `burnt` deaths;
and 34 ignitions, 32 spreads, 66 burnouts and 32 storms. So the grazers go 300 -> 9204 in half a year
with almost nothing removing them, the hunters stay at 20 -> 28, and the tree count peaks at 337
around tick 1000 and falls to 24 by tick 2000, of thirst rather than of grazing.

An animals-off run of the same seed, site and length ends with 532 trees, 689 germinations, 228
drought deaths and 51.4 mm of mean soil water, against this run's 24, 307, 353 and 27.1. **That is not
a controlled comparison and must not be reported as one.** The animal phase draws from the same RNG
stream, so the two runs get different weather: 32 storms against 41 over the same 2000 ticks. The
numbers are recorded because they describe the committed fixture, not because they attribute anything
to the animals — shot V5 found the same limit from the other side, and a site-wide number from a
single differing run is a reading of the weather.

**Nothing here is tuned, deliberately.** The animal tier is parked by the operator's direction of
2026-09-19: it stays in the code, and no shot tunes it. An unregulated grazer population is
therefore reported and left alone. The fixture's value does not depend on the tier being well
behaved — it depends on there being animals in it at all.

## Shot S2 — the simulator publishes the whole palette, and stops omitting params

Two halves of one complaint: `meta.json` was not saying everything it knew.

**The overlay ramps go in `meta.json`, as `overlays`.** The species table has carried colours since
version 1, and the renderers took the species colours from it. But an overlay is a *ramp*, and the run
directory had nowhere to put the two hues one runs between, so both renderers carried their own copy
of the `ecoview` legend: `ecoview/src/world.ts` `COLORS` and, copied from it, `ecoview-native`'s
`palette.rs`. Shot V2 read every *number* of every scale out of the run and then had to write in its own
module doc that the hues were the one thing it could not (component isolation forbids a viewer shot
changing `meta.json`). This closes that: seven entries, `{name, lo, hi}` in sRGB hex, in the order a
renderer lists them, with `mid` on the one diverging ramp (`traits`) and `burnt` on `fire`. The values
are the legend's own, so **no picture changes** — what changes is who owns them.

- **The table lives in `output.rs`, next to `species_list()`, not in `params.toml`.** These are not
  tunable parameters. CLAUDE.md's "nothing tunable is hard-coded" is about the model; the species
  colours have been a writer-side constant since shot 1 and nobody tunes a hue per run. Putting them in
  `params.toml` would also put them in `meta.json`'s `params`, where a renderer would have to read the
  palette out of the tuning dump, and would let `--set` change a colour mid-fork.
- **`format_version` does not move.** Every version so far has only added files; an added `meta.json`
  key is the precedent set by `overrides` (shot 4), `dims.patch` (15), `animals` (G0) and `year_len`.
  A renderer that wants the ramps reads them, and one that does not is unaffected — which is exactly
  what `ecoview` does, and why this shot does not touch it.
- **Surface media, buildings, pipes and the vine hue are not in `overlays`.** A medium is scene
  geometry, not ecology (shot G7), and a vine is not a species the simulator has (V4). The split the
  format now draws is: the simulator owns the *ecology* palette; the bundle and its renderer own the
  *scene* palette. Fire's "quiet ground" band stays the viewer's for the same reason — "nothing to show
  here" is not an ecological quantity — while fire's `burnt` is the run's.

**Every params section is written, at its defaults or not.** `bundle`, `animals` and `rng` carried
`skip_serializing_if`, and `hunter.handling_ticks` carried `skip_serializing_if = "is_zero"`. All four
are gone. The justification each was given was that a default run then writes the `meta.json` it always
did — and that is precisely the defect: the ordinary case, a run at the defaults, was the case in which
the file said nothing. `runs/s42` published 16 sections of 19 and `runs/capitol-s42` 17; both now
publish 19. Shot V3 met this from the reader's side (it needs `bundle.tree_mature_height` and
`bundle.tree_tall_height`, which are at their defaults on every committed run, so it never received
them), and shot S1's own DECISIONS entry names it as the reason `meta.json` could not say a run had
animals.

- **`#[serde(default)]` stays on all four.** Omission on the way *in* is a feature: a `params.toml`
  or an older `meta.json` that lacks a section still loads. Only the writing side changed.
- **Two workarounds came out with it.** `fork_params` had a patch that put `handling_ticks` back into
  the parsed params so a fork could override it; the params round-trip proptest had a branch for a leaf
  that is absent from the base rather than null. Neither has anything left to do.
- **The top-level `animals` key is left exactly as shot G0 defined it** — present and `false` when the
  tier is off, absent when it is on. It is a statement about the run rather than a params section, the
  renderers' pixel tests read it (`fixtures/capitol-mini` asserts `animals == false`), and the positive
  statement it could not make is now available next door in `params.animals.enabled`. Changing both at
  once would have been two format changes wearing one coat.

**What was regenerated, and the proof that nothing else moved.** `fixtures/s42-mini-v2`,
`fixtures/capitol-mini` and `fixtures/capitol-animals-mini` are compared byte for byte with a fresh
run, so all three were re-cut with `ECOSIM_REGEN_MANIFEST=1`; only their `meta.json` (and `timing.json`,
which the comparison ignores) changed. No manifest and no golden `check` output changed, because none of
them hashes `meta.json`. `fixtures/s42-mini` is version 1 and cannot be re-cut (shot G4b), and the test
that reads it checks that every version-1 params *key* is still present, which an addition cannot break.
The anchor for the shot: a fresh 20,000-tick `runs/capitol-s42` against the one on disk from before it,
`ecosim diff` says `differs: meta.json` and nothing else.

## Shot S3 — the light a crown actually receives

The row's finding, confirmed here before anything was written: on `runs/capitol-s42` at tick 20000 the
per-tree light in `light.bin` has **exactly one distinct value per stage** — 0.1373 mature (n=2869),
0.3686 young (n=96), 1.0000 sapling (n=1117) — and the same three numbers on the noise strip at every
tick and on a second, unrelated world. A tree's canopy shades its own 3×3 columns, the tree is sampled
at its own trunk, so every tree of a stage reads the extinction of its own canopy and nothing else.
Light was `stage` wearing a decimal point.

**The shot publishes the right number; it does not make the ecology consume it.** The row offers
"sample or publish", and publish is the whole of what was done: `entities.json` gains `crown_light`
(with `height_m` and `crown_radius_m`, the two numbers it is derived from), and no tick of any
reference run moves. Widening the canopy footprint from 3×3 to the allometric 6–12 m across is an
ecology change: it would darken every seedling under every tree, and CLAUDE.md's own history says what
follows — G4c widened the *young* canopy by one step and germination on the Capitol went 7645 → 19408.
That needs its own tuning pass, its own sweep and its own regression anchors, and MASTER forbids
adding a mechanism the shot did not ask for. The proof that it was not added is in the tests, three
ways: an integration test runs seed 1 at `crown_radius_frac` 0.30 and 0.90 and asserts the two runs
differ in `meta.json` and every `entities.json` and in **nothing else**; a manifest test compares the
pre-shot `s42-manifest-preS3.sha256` with the regenerated one and asserts the differing files are the
201 `entities.json` and exactly those; and `crown_light` draws no RNG and writes no field, so it
cannot.

- **The crown's dimensions are two new `params.toml` keys, and they are measured, not chosen.**
  `tree.crown_radius_frac = 0.30` and `tree.crown_base_frac = 0.37` are the medians of
  `crown_radius/height` and `crown_base/height` over the 81 surveyed trees in
  `worlds/capitol/trees.json` — the only real crown dimensions this project owns, and the same two
  constants shot V3 measured for the viewer. A test in `tests/bundle.rs` re-reads `trees.json` and
  fails if either drifts more than 0.005 from the median it is supposed to be, so the file and the
  parameter cannot silently disagree. Medians rather than means: the mean-of-ratios is 0.3287/0.3851,
  and the survey has a long right tail (radius/height sd 0.126) of young stems whose crowns are wide
  for their height.
- **The measurement is taken at the middle of the crown**, `ground + (base + height)/2`, not at its
  top or at the trunk voxel. The top sees sky almost by construction and the trunk voxel is the
  quantity that was already wrong. The middle is also where the crown's leaf area is, under the
  uniform-density assumption the optics already make.
- **The optics are the voxel model's, at the crown's scale.** A whole crown has optical depth
  `2 × canopy_k × canopy_lai`, the same total a mature tree's two canopy voxels have, spread evenly
  down the crown's depth; a ray to a neighbour's middle gets the share of that depth it passes
  through. So the pair of numbers `params.toml` already calibrates (a full mature crown passes 13.5%
  of full sun) is preserved rather than replaced by a second set of constants that could drift from
  it.
- **Overlapping neighbours multiply, mean-field.** Each neighbour covers `overlap_fraction` of this
  crown's footprint — a circle-circle lens over the disc's area — and contributes a factor
  `1 - f(1 - exp(-τ·depth_above/depth))`. This is the one modelling liberty in the shot and it is
  named as such in the rustdoc: it is exact for one neighbour and for neighbours that do not overlap
  *each other*, and it errs dark when several cover the same side. The alternative, ray-marching a
  real canopy volume, is a rendering problem and costs per tree what the whole light field costs per
  tick.
- **Buildings darken a crown by the share of its footprint columns whose `shade_top` is above the
  crown's middle.** That is the same `shade_top` the voxel light field uses, so a tree in a roof's
  shadow and a voxel in a roof's shadow agree about where the shadow is. It affects exactly 1 of the
  79 imported Capitol trees (1.0000 → 0.8482) and is identically 1 in a noise world, which has no
  buildings.
- **`height_m` and `crown_radius_m` are published next to it** because otherwise every reader
  re-derives the height curve from `bundle.tree_mature_height`, `bundle.tree_tall_height` and the two
  ages, which is what `ecoview-native/src/tree.rs` does today, with its own copy of the two crown
  fractions besides. The run directory is the interface; the derivation belongs on the simulator's
  side of it. `plants::height_of_age` is `import_age` read backwards and is tested as its left
  inverse above the tall age.
- **`format_version` does not move.** Adding fields to an `entities.json` record is the precedent set
  by the heredity traits in shot 11, and version 1 readers are held to it by
  `format_2_and_fire_only_add_to_version_1_files`, whose cut list gains a `without_crown` helper.
  `fixtures/s42-mini` is version 1 and cannot be re-cut, which is exactly why that helper exists.
- **No `check` invariant was added.** `check` reads `series.csv`, `crown_light` is per tree and
  per snapshot, and an invariant on a number the ecology does not consume would be a bound on
  arithmetic, not on the model. The bounds that matter — every value in [0,1], the radius equal to
  the fraction times the height, and a distribution with more than one value in it — are asserted on
  the committed `fixtures/capitol-mini` in `tests/bundle.rs`, where they run on every CI job.

**What was regenerated, and what deliberately was not.** Every `s42` manifest (the default one and the
five `g4b-*` variants) and the three re-cuttable fixtures were re-cut with `ECOSIM_REGEN_MANIFEST=1`;
in each manifest exactly 201 lines moved and all 201 are `entities.json`. `tests/data/s42-check.txt`
was **not** kept: the regeneration pass rewrote its wall-clock line to a `FAIL run time … 107314 ms`
measured under a loaded parallel debug test run, and the golden compares that line by name, so the
file was reverted. This shot changes no `check` behaviour. Two of the new tests return early under
`ECOSIM_REGEN_MANIFEST`, because they read a fixture and a manifest that a sibling test in the same
binary is rewriting at that moment; both run in full on every ordinary invocation, CI included.

**Reported, not fixed** (component isolation — this is an `ecosim` shot): `ecoview-native/src/tree.rs`
documents its `CROWN_RADIUS_FRACTION`/`CROWN_BASE_FRACTION` as "the mean of that ratio" when 0.30 and
0.37 are the *medians* (the means are 0.3287 and 0.3851), and its comment that `params.bundle` reaches
`meta.json` only when non-default — which `src/run.rs` carries too — has been stale since shot S2. Both are in
`sweeps/shotS3/FINDINGS.md`.

## Shot S8 — the Capitol with animals, past tick 2000

**The row was an open question and it closes as one: the Capitol's grazer level is correct
scaling, and its tree crash is drought in the establishment year rather than anything the animals
do.** No code, default, invariant or committed file changed. The evidence is
`sweeps/shotS8/FINDINGS.md`, every table of which is printed by `analyse.py` beside it from run
directories the file's own commands rebuild.

**Why nobody could tell before.** No run of a bundle world with animals had gone past tick 2000,
so the only picture of one was `fixtures/capitol-animals-mini`, which ends at 2000 — and at tick
2000 this run is at the bottom of a transient in every variable that matters. Trees are at 24,
their minimum for the whole run. Hunters are at 28, against the 356 they settle at. Grazers are at
9204 and climbing, on their way to a peak of 11348 near tick 4500. Reading any of those three as a
steady state gives the wrong answer, and reading all three together gives "predation has failed".
Carried to 20000 the same run ends with **2470 trees, 8347 grazers and 356 hunters**, and seeds 1,
2 and 3 end with 4674, 4560 and 3861 trees.

**Grazers: scaling.** Population is set per patch, so it tracks grazable area. At tick 10000 the
Capitol carries 12.97 grazers per grassy patch and the noise strip carries 10.67, 12.44 and 12.05;
the Capitol has 3.5–4.0× the strip's grassy patches and 3.7–4.8× its grazers. The kill rate is
4.5–5.4 grazers per hunter per 1000 ticks on both worlds in every window, because
`hunter.satiation` and not prey density limits it — which the row had already measured and was
right about.

**What the row's measurement could not see is that `grazer.start_count` and `hunter.start_count`
are absolute, not per patch.** 300 grazers and 20 hunters are placed whatever the world's size, so
a world with four times the patches starts four times emptier of both. The grazer refills its
space in a few hundred ticks (`repro_energy` 70, `cooldown` 300); the hunter takes the whole run
(`repro_energy` 75, `refractory` 2750). At tick 2000 the Capitol is at 152 grazers per hunter
against the strip's 61–68 — the predator behind by about the area ratio, exactly as an absolute
start count predicts — and by tick 20000 it is 34 against 14–31. **The overshoot is the transient
of that mismatch, not a failure of the brake.** Left as it is: making the start counts densities
would change every existing run and the row asked for an explanation, not a change.

**Trees: drought, and the animals are not involved.** Drought is 91–98% of every tree death before
tick 2500 in seven of the eight 20000-tick Capitol runs (the eighth has 32 early deaths in total
and no crash). The controlled test is 32 replicates — one world, seed 42, `rng.stream` 1–16 per
condition, so the weather distribution is matched and only `animals.enabled` differs: **9 of 16
crash with animals and 8 of 16 without.** Severity tracks the establishment-window moisture
minimum at r = −0.757; nine of the ten driest replicates lose more than half the cohort against
eight of the other twenty-two.

**The mechanism is the scene's own tree cohort.** Through tick 1000 every mature tree on the site
is one of the 79 the bundle planted, because nothing sown in the run has reached
`tree.mature_age_years` (0.25 years, so 1000 ticks) yet, and only mature trees seed. So the site's
entire seed supply for its first year is one synchronous cohort standing on one profile, and a dry
spell takes it down together: 79, 51, 27, 11, 8 mature trees, with germination following exactly —
34, 32, 37, 31, 26 per 100 ticks, then 10, 1, 4, 3, 1. The recovery is the same mechanism
forwards, and compounds: 8, 12, 13, 15, 19, 20, 40, 131, 551, 1637. **The noise strip cannot do
this**, because it starts with `tree.initial_count = 12` at `initial_age_years = 0.125` and never
builds a 300-tree first-year cohort over a seed supply of 79. The crash is a property of starting
from a surveyed scene, not of the Capitol's size.

**`hydro.initial_fill` is the other half and is deliberately not changed.** The default 0.5 starts
every column at half its available water capacity and the site takes about 2500 ticks to fill, so
year one is structurally the driest year of every run. Eight replicates at `initial_fill = 1.0`
crash 2 of 8 against 9 of 16, median loss 11% against 61%, and median trees at tick 2500 of 811
against 286. That measurement completes the explanation the row asked for. **Whether a garden site
should begin at field capacity, at half of it, or at whatever the season implies is a question
about what the model means by tick 0, and it is a human's to answer** — so nothing was retuned,
`TUNING.md` gains no entry, and no default moved.

**Two things this shot found and did not fix.**
- **An animals-on Capitol run cannot pass `ecosim check`.** `max_10x` fails on hunters (max 362
  against a limit of 280) and trees (2548 against 1620) because the invariant's anchors — animals
  at 0.5 years, trees at 1.25 — land inside a transient that is 15000 ticks long on this world and
  is over by tick 2000 on the strip. The assumption the invariant encodes is that the anchor tick
  is past the initial transient, and on a bundle world with animals it is not. **Widening it is
  forbidden and the row did not ask for it**; what the anchor should be is a judgement, not a
  worker's call. `runtime` fails too, at 314.5 s against a 90 s cap.
- **It is 10× slower than the animals-off reference run**, 314.5 s against 31.5 s, with 89.6% of
  the time in the animal phase. Recorded in `PERF.md` under "The Capitol with animals (shot S8)".
  This is why `just capitol` carries `animals.enabled=false` and should keep carrying it.

**One methodological note worth keeping, because it would silently invalidate any repeat of this
work.** `animals.enabled=false` skips the animal phase, which draws from the shared `ChaCha8Rng`,
so an animals-off run **at the same seed gets different weather** — different storm ticks, not
just different animals. Over ticks 1000–1500 `capitol-s42-animals-20k` got 36.3 mm of rain and
`capitol-s42` got 100.3 mm, which alone would explain the whole difference between them.
`runs/capitol-s42` is therefore not a control for `runs/capitol-s42-animals-20k` and was not used
as one. `rng.stream` is the knob that makes a real control: it replays the same world with an
independent dynamics stream, so replicates can be matched on weather distribution while differing
in one parameter.
