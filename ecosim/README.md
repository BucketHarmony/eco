# ecosim

This is a deterministic, headless voxel ecology simulator (SAD 1).
- The spec is in `../docs/`.
- Design calls are in `DECISIONS.md` and parameter history is in `TUNING.md`.
- The sweep results are in `SWEEP_FINDINGS.md` (shot 4), `sweeps/shot5/FINDINGS.md` (the dynamics fixes), `sweeps/shot05/FINDINGS.md` (extinctions by cause), `sweeps/shot09/FINDINGS.md` (fire), `sweeps/shot10/FINDINGS.md` (crowding mortality and the hunter refractory), `sweeps/shot11/FINDINGS.md` (heritable traits) and `sweeps/fork-demo/FINDINGS.md` (a fork with seasonal rain off). The measured coverage is in `COVERAGE.md`.
- `series.csv` records each tick's animal deaths by species and cause (`starved`, `eaten`, `old_age`, `crowded` from `[disease]`, `burnt`). `ecosim stats` attributes every extinction to the dominant cause over the 500 ticks before it.
- Each animal carries three heritable traits (`energy_cost_mult`, `flee_distance`, `repro_threshold`), mutated at birth by `heredity.mutation`. `series.csv` ends with their per-species means and standard deviations, and `entities.json` animals carry them. Every species with individuals (grazers, hunters, trees) can immigrate from the world edge when its live count is below `immigration_floor` (default 0, off).

## Commands

```bash
cargo build --release
cargo test                                   # debug: property tests + integration tests, ~30 s
cargo test --release
ecosim run --seed 1 --ticks 20000 --out runs/s1 --snapshot-every 100 [--set key=value ...]
ecosim check runs/s1                         # invariants; exit 1 on any failure
ecosim check --long runs/l1                  # long-run invariants for runs of >= 60000 ticks
ecosim stats runs/s1                         # column ranges; each extinction with its death causes
ecosim stats --signature runs/s1             # predator–prey signature: lag, value and period of the detrended hunter–grazer cross-correlation
ecosim diff runs/a runs/b                    # byte-compare two run directories
ecosim fork runs/s1 --at 10000 --ticks 10000 --out runs/f1 [--set key=value ...]   # continue from a snapshot
ecosim sweep --baseline --seeds 1,2,3        # margin table; see `ecosim sweep --help`
ecosim run --seed 42 --ticks 2000 --out runs/p --profile p.json   # also write wall time per tick phase (outside --out)
ecosim run --world worlds/capitol --seed 1 --ticks 20000 --out runs/g1 --set animals.enabled=false --set climate.rain_gradient=0   # build the world from a world bundle instead of noise
cargo bench --bench tick                     # ticks/s on 64x64 and 256x64 against benches/baseline.json
```

`PERF.md` has the performance baseline: per-phase profiles, full-run wall times, hotspots and recommendations.

## CI gate

CI is defined in `../.github/workflows/ci.yml`. The `justfile` in this directory runs the same steps in the same order, and `tests/ci.rs` checks that both contain them:

0. `python3 -m unittest discover -s tools -t tools`: the Blender exporter's pure half (`just pyexport`)
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`, then (2b) `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"`
3. `cargo test` (debug)
4. `cargo llvm-cov` with a floor of 85% line coverage (`main.rs` excluded), writing `lcov.info`
5. `cargo build --release`
6. `ecosim run` on seeds 1, 2 and 3 into `ci-runs/`, then `ecosim check` on each
7. `ecosim sweep --baseline --seeds 1,2,3`, writing `ci-runs/baseline-margins.txt`
8. `cargo test --release` of the determinism tests with `ECOSIM_REQUIRE_CROSS=1`. This requires the debug and release binaries to write identical run directories.
9. `ecosim run` on seed 1 for 60000 ticks, then `ecosim check --long` on it

A separate job, `ecosim-bench`, runs `cargo bench --bench tick` and fails when either world is more than 20% slower than `benches/baseline.json` (CI-runner numbers). It is not part of `just ci`; `just bench` runs it locally.

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

## Snapshots and forks

Each snapshot directory holds `state.bin` beside the renderer's files (`format_version` 2). `state.bin` is the exact sim state: RNG position, full-precision fields and every entity. `ecosim fork` restores it and continues the run into a new directory, optionally with `--set` overrides from that tick on. The new directory is a complete run directory: rows and snapshots before the fork tick are copied from the parent, and `meta.json` records `forked_from`. With no overrides, a fork is byte-identical to the uninterrupted run from the fork tick on. `ecosim run --snapshot-state false` skips `state.bin`. Format-1 directories still work with `check`, `stats` and `diff`, but can't be forked. `sweeps/fork-demo/` has an example; the layout and the design calls are in `DECISIONS.md`.

## World bundles

`ecosim run --world <dir>` builds the world from a **world bundle** instead of procedural noise: real
ground height, a surface medium per ground cell (lawn, bed, gravel, asphalt, roof, water, ...) and
building heights, exported from a tagged Blender scene. The format is `../docs/SCENE-CONTRACT.md`,
which is authoritative; this ecosim reads version 2 of it.

The bundle has two grids. The ecology grid keeps its 1 m columns, so every species parameter keeps
its units, and the bundle's `size_m` sets `[world] width` and `depth`. The ground grid is finer
(0.5 m in the Capitol bundle), and is carried at full resolution in the world state. A column's
surface layer is `[bundle] base_z + round(the mean ground height under it)`; it is Rock when more
than half of its ground cells are sealed (`roof`, `asphalt`, `concrete`), Water when more than half
are `water`, otherwise soil. A roof shades the columns north of it, `[bundle] shade_slope` columns
per metre of height.

A bundle's plants are planted too, in place of the noise world's random ones. Each scene tree
becomes a tree entity on the column its trunk stands in, aged by its height: piecewise linear
through (0 m, 0), (`[bundle] tree_mature_height`, `tree.mature_age`) and (`tree_tall_height`,
`tree_tall_age`), flat above. A trunk on an unplantable column moves to the nearest plantable one
within `tree_move_radius`, or is dropped; when two trees land on one column the taller stays. Each
shrub ellipse raises its patches' starting shrub density by the fraction of their plantable columns
it covers. Grass starts as it does in a noise world. `ecosim run --world` prints what happened:

```
scene: 81 trees -> 79 planted (0 moved, 2 dropped, 0 merged); 64 shrubs over 750 columns in 87 patches
```

The bundle's pipes are loaded into the world state and are not used yet.

A bundle run writes `format_version` 4 (`--format-version` picks 2, 3 or 4 explicitly; 4 needs
`--world` and `--world` needs 4). It cannot be forked: `ecosim fork` rebuilds the terrain, and only
the bundle has it. `DECISIONS.md` (shots G1 and G3) has the design calls.

`worlds/capitol/` is the committed reference world: a 256 m square of the Michigan State Capitol
grounds in Lansing, from public-domain USGS LiDAR, with its walks and plazas from OpenStreetMap
under the ODbL. Its README has the provenance, the licence per file and the numbers
`tests/bundle.rs` pins. The garden series runs it with `--set animals.enabled=false --set
climate.rain_gradient=0`; CI runs it for 20000 ticks as step 10, and `fixtures/capitol-mini/` is the
first 100 ticks of that run, committed for the renderer. `sweeps/capitolG3/FINDINGS.md` reports the
run as shot G3 made it — what dies of what, how the imported wood gives way to the sim's own, canopy
fraction, and what building shade and the rain gradient do to a real site — and
`sweeps/capitolG3-flat/FINDINGS.md` is the reference run as it stands, with the rain ramp off.

`tools/blend_export.py` writes a bundle from a tagged `.blend`:

```sh
blender -b <scene.blend> --python tools/blend_export.py -- worlds/<name> --audit 4096
```

The same scene always exports to a byte-identical bundle. Its rasteriser and file layout are plain
Python — no Blender, no third-party package — and CI runs their unit tests as step 0
(`just pyexport`); `DECISIONS.md` (shot G2) says why the sampling is a scanline and what `--audit`
checks.

## Behaviour guard

`tests/data/s42-manifest.sha256` holds the sha256 of `series.csv` and every snapshot file of `ecosim run --seed 42 --ticks 20000 --snapshot-every 100`. The test `fresh_s42_matches_committed_manifest` regenerates that run and compares the hashes.

A change that alters simulation behaviour on purpose must do three things:
- Regenerate the manifest from the run directory, with `(cd runs/s42 && sha256sum series.csv snap_*/* | LC_ALL=C sort -k2)`, then normalize the output to two-space `hash  path` lines.
- Update `tests/data/s42-check.txt`.
- Say so in `DECISIONS.md`.
