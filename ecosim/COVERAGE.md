# Coverage

Coverage is measured with this command (CI step 4, `just coverage`):

```bash
cargo llvm-cov --fail-under-lines 85 --ignore-filename-regex 'main\.rs|cli/'
```

It used cargo-llvm-cov 0.9.1 on Rust 1.98.1, and was measured on 2026-09-18. The floor is 85% of lines, and `main.rs`, the CLI argument handling, is excluded.

**Total: 95.85% of lines (2699 of 2816), 93.29% of functions and 94.81% of regions.**

| File | Lines | Missed | Line cover | Function cover |
|---|---:|---:|---:|---:|
| abiotic.rs | 161 | 0 | 100.00% | 100.00% |
| animals.rs | 486 | 1 | 99.79% | 100.00% |
| check.rs | 475 | 20 | 95.79% | 90.12% |
| lib.rs | 7 | 1 | 85.71% | 100.00% |
| output.rs | 243 | 10 | 95.88% | 100.00% |
| params.rs | 143 | 9 | 93.71% | 84.85% |
| producers.rs | 171 | 0 | 100.00% | 100.00% |
| sim.rs | 186 | 2 | 98.92% | 100.00% |
| sweep.rs | 466 | 74 | 84.12% | 80.25% |
| trees.rs | 207 | 0 | 100.00% | 100.00% |
| world.rs | 271 | 0 | 100.00% | 100.00% |
| **Total** | **2816** | **117** | **95.85%** | **93.29%** |

## What is not covered

The first measurement, 94.76%, found three `pub fn`s that nothing called: `Params::from_toml_str`, `Sim::count_mature_trees` and `World::count_class`. They were deleted rather than tested. Every line still missed is reachable, just not reached by the suite:
- **`sweep.rs`** misses the most lines. Its tests sweep one parameter over a passing band, so these parts aren't reached:
  - the `sweep.md` report branches for two-parameter grids and failing band edges
  - the refusals in `prepare_out`
  - the "needs at least one `--param`" error
  - the error-cell text

  The committed sweeps in `sweeps/` exercise these branches through the CLI. `margin_table`, which only `main.rs` calls, was uncovered in the first measurement and now has assertions in `baseline_margins_equal_check_margins`.
- **`check.rs`** misses these arms:
  - the malformed-`series.csv` errors (a bad header, a wrong field count, no rows, a non-contiguous tick)
  - the `timing.json`-missing arm
  - the zero-target edge of the band margins
  - the "no extinction" text
- **`output.rs`** misses the refusal to overwrite a directory that isn't a run directory.
- **`params.rs`** misses the `--set` errors for booleans and TOML datetimes, which no params field uses.
- **`sim.rs` and `animals.rs`** each miss a `None` fallback.
- **`lib.rs`** misses the non-coverage branch of the test-only `crate::cases`.

## The suite under coverage

Instrumentation makes the simulation several times slower. So under `cfg(coverage)`, which cargo-llvm-cov sets:
- properties run a quarter of their cases
- the five full-length run tests are skipped; those tests still run in `cargo test` (step 3), and the determinism test also runs in step 8

On the build machine the instrumented run takes about 160 s wall time, including compilation. The full suite without these trims took about 490 s of test time. See DECISIONS.md, "Test hardening".
