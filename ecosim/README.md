# ecosim

This is a deterministic, headless voxel ecology simulator (SAD 1).
- The spec is in `../docs/`.
- Design calls are in `DECISIONS.md` and parameter history is in `TUNING.md`.
- The sweep results are in `SWEEP_FINDINGS.md` (shot 4), `sweeps/shot5/FINDINGS.md` (the dynamics fixes) and `sweeps/shot05/FINDINGS.md` (extinctions by cause). The measured coverage is in `COVERAGE.md`.
- `series.csv` records each tick's animal deaths by species and cause (`starved`, `eaten`, `old_age`, `crowded`, `burnt`). `ecosim stats` attributes every extinction to the dominant cause over the 500 ticks before it.

## Commands

```bash
cargo build --release
cargo test                                   # debug: property tests + integration tests, ~30 s
cargo test --release
ecosim run --seed 1 --ticks 20000 --out runs/s1 --snapshot-every 100 [--set key=value ...]
ecosim check runs/s1                         # invariants; exit 1 on any failure
ecosim check --long runs/l1                  # long-run invariants for runs of >= 60000 ticks
ecosim stats runs/s1                         # column ranges; each extinction with its death causes
ecosim diff runs/a runs/b                    # byte-compare two run directories
ecosim sweep --baseline --seeds 1,2,3        # margin table; see `ecosim sweep --help`
```

## CI gate

CI is defined in `../.github/workflows/ci.yml`. The `justfile` in this directory runs the same steps in the same order, and `tests/ci.rs` checks that both contain them:

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`, then (2b) `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"`
3. `cargo test` (debug)
4. `cargo llvm-cov` with a floor of 85% line coverage (`main.rs` excluded), writing `lcov.info`
5. `cargo build --release`
6. `ecosim run` on seeds 1, 2 and 3 into `ci-runs/`, then `ecosim check` on each
7. `ecosim sweep --baseline --seeds 1,2,3`, writing `ci-runs/baseline-margins.txt`
8. `cargo test --release` of the determinism tests with `ECOSIM_REQUIRE_CROSS=1`. This requires the debug and release binaries to write identical run directories.
9. `ecosim run` on seed 1 for 60000 ticks, then `ecosim check --long` on it

To run it locally, first install the two tools once:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked      # 0.9.1 was used
cargo install just --locked                # 1.58.0 was used; `make` isn't available on Windows
```

Then, from `ecosim/`, run:

```bash
just ci          # on Windows, run it from Git Bash: the recipes are bash
just coverage    # any single step works by name
```

Both CI and `just` set `PROPTEST_RNG_SEED`, so the property tests explore the same cases on every run. A plain `cargo test` uses a fresh random seed each time. If one of those random runs finds a failure, fix it, then add the shrunk case as a named regression test next to the property. `proptest-regressions/` is gitignored.

## Behaviour guard

`tests/data/s42-manifest.sha256` holds the sha256 of `series.csv` and every snapshot file of `ecosim run --seed 42 --ticks 20000 --snapshot-every 100`. The test `fresh_s42_matches_committed_manifest` regenerates that run and compares the hashes.

A change that alters simulation behaviour on purpose must do three things:
- Regenerate the manifest from the run directory, with `(cd runs/s42 && sha256sum series.csv snap_*/* | LC_ALL=C sort -k2)`, then normalize the output to two-space `hash  path` lines.
- Update `tests/data/s42-check.txt`.
- Say so in `DECISIONS.md`.
