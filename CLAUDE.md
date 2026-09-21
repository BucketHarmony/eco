# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A proof-of-concept voxel ecology simulator, built as two independent projects that share only a file format on disk. The spec is in two files; read both before writing code:
- `docs/POC Ecology Simulator — SADs (Sim + Renderer).md`: the SAD.
- `docs/SAD-addendum.md`: settles the SAD's open details (formulas, starting params, file shapes, tick semantics, renderer layout and cameras). **The addendum is authoritative where the two differ.**

Each component is meant to be built in one session with no questions asked. The kickoff prompt for each session is in `docs/prompts/`. When a design choice comes up that neither doc settles, make the call and record it in that project's `DECISIONS.md`.

```
eco/                 one git repo (branch main)
  docs/              SAD, addendum, session prompts
  scripts/sync-data.sh   copies sim output into ecoview/public/
  ecosim/            SAD 1: Rust simulator (build first, to completion)
  ecoview/           SAD 2: TypeScript/Three.js renderer (build second)
```

**Build order is strict:** finish `ecosim/` first, including `ecosim/runs/s42/` (seed 42, 20000 ticks, snapshot every 100) and the 2-snapshot `ecosim/fixtures/s42-mini/`. Then run `bash scripts/sync-data.sh`, which copies both into `ecoview/public/`, and build `ecoview/` against that copy. The two projects share no code and have no IPC. Build one component per session; don't mix the two in the same session. `runs/` directories are gitignored because they can be regenerated deterministically; `fixtures/` is committed.

## Environment

The build runs natively on Windows (not the Linux container the SAD assumes). The tools are Git Bash, PowerShell, Rust stable MSVC, and Node 22.
- If `cargo` isn't on PATH, it's in `%USERPROFILE%\.cargo\bin`.
- npm scripts must be cross-platform Node, not bash-isms.
- **Headless WebGL needs `--use-angle=swiftshader --enable-unsafe-swiftshader`.** The SAD's `--use-gl=swiftshader` loses the WebGL context and renders blank on this machine. Verified with Playwright 1.63.0 and three 0.169.0.

## Hard constraints (from the SAD, apply to both)

- There is no cap on total project size. Each shot (one session's unit of work) may add at most 1,500 net lines to its component, measured with `git diff --stat <shot's starting commit>` over the component directory. Committed sweep output (`sweeps/**`), regenerated runs, fixtures and manifests don't count.
- Every verification step is a single command that exits non-zero on failure.
- Everything must run headless: no GPU, display server, external service, or account.
- Features the SAD defers are left out entirely. Don't add `TODO` hooks, stubs, or plugin interfaces for them.
- Species and run length are fixed: 2 ground-cover species, 1 tree species, 2 animal species, 20,000 ticks. World dimensions come from params (`[world] width`, `depth`, `height`, `patch`) and are recorded in `meta.json`'s `dims` (`x`, `y`, `z`, `patch`); since shot 15 the reference world is a 256×64×32 strip with 8×8-column patches, and 64×64×32 is the old square world (ecosim/DECISIONS.md, shot 15).

## ecosim (Rust, single crate: lib + `ecosim` CLI binary)

```bash
cargo build --release
cargo test --release                         # canonical: integration tests run long sims
cargo test --release <name>                  # single test by name substring
ecosim run --seed 1 --ticks 20000 --out runs/s1 --snapshot-every 100
ecosim check runs/s1                         # invariants on series.csv; exit 1 on any failure
ecosim stats runs/s1                         # min/max/mean per column, first extinction tick
ecosim diff runs/a runs/b                    # byte-compare two run dirs
```

Done means `cargo test --release` passes and `ecosim check` passes on seeds 1, 2, and 3, with a release-mode run finishing in under 30 s. `runs/s42` must also pass the addendum's extra checks, which exist so the renderer's pixel tests are achievable (for example, at least 35 mature trees at tick 10000). The loop is build → test → run seeds 1/2/3 → check → tune `params.toml` or fix code → repeat. Log every parameter change and its effect in `TUNING.md`. The SAD's starting parameter values are guesses, so iterating on them is expected.

Architecture points that span modules:
- **Three tiers with different update rates.**
  - Voxel fields are flat `Vec<u8>`, indexed `x + width*(y + depth*z)`, with z up. Light is recomputed per column, only when a tree is planted or dies. Moisture and fertility update every 10 ticks, on surface voxels only.
  - Patch fields (grass, shrub, detritus, temperature) update every 10 ticks, staggered so about 6 patches update per tick.
  - Trees update every 50 ticks and animals every tick. Entities are stored as `Vec<Animal>` and `Vec<Tree>` with an `alive` flag and periodic compaction. Don't use an ECS crate, chunking, or an octree.
- **Fixed tick order:** animals → producers → storm (any tick that rains) → moisture/fertility (every `schedule.soil_every` ticks, 10 by default) → temperature/season (every `schedule.temperature_every`, 100) → snapshot (every N) → stats row.
- **Water is its own tier** (`hydro.enabled`, shot G4): storms fall on the ground grid, run downhill along a flow graph built once at load, soak into per-column soil water and pond in depressions. The `moisture` field is a `Vec<f32>` derived from soil water, not stored, and is quantised to u8 only when written to `moisture.bin` (shot G4b corrected this line, which called it a u8 field). `ecosim/sweeps/shotG4/FINDINGS.md` has the measurements.
- **Determinism is tested.** All randomness comes from one `ChaCha8Rng` seeded from the CLI. Never iterate a `HashMap` in sim logic; use `Vec` or `BTreeMap`. Two runs with the same seed and params must produce byte-identical run directories.
- **All species and tuning parameters live in `params.toml`**, loaded at startup. Nothing tunable is hard-coded. That includes the fallback knobs in the SAD's risk table: kill probability, refugium threshold, seed radius, and sapling light need.
- **The five stability rules under "Consumers" must be implemented exactly as written:** type II grazing intake, hunter satiation and kill probability, the shrub refugium, movement energy cost, and corpse detritus. Without them the populations oscillate to extinction.

## ecoview (TypeScript + Vite + Three.js, static site, no framework)

```bash
npm run build
npm test            # Vitest unit tests + Playwright page/pixel tests
npm run shot        # vite preview + headless Chromium → 8 PNGs in shots/
npm run preview:sim # preview + the dev-only sim helper: R runs ecosim on the bundle being edited
npx vitest run <file-or-pattern>        # single unit test
npx playwright test -g "<test name>"    # single Playwright test
```

Done means `npm test` exits 0, `npm run shot` produces all 8 PNGs in under 60 s, and each PNG matches its expectation in the SAD's screenshot table. After running `npm run shot`, view each PNG yourself and write a one-line verdict per file in `shots/REPORT.md`.

Architecture points that span modules:
- The code is split into four modules: `loader.ts` (fetch and parse the run dir, reject `format_version ≠ 1`), `world.ts` (one `InstancedMesh` of the surface and water voxels, colored by the active overlay), `entities.ts`, and `ui.ts` (controls, URL params, a Canvas 2D chart with no chart library).
- **State comes from URL parameters** (`?run=…&tick=…&overlay=…&cam=iso|top|side`), so screenshots need no clicking.
- **Rendering happens on demand only.** There is no continuous render loop, which keeps screenshots deterministic. The page sets `window.__ecoviewReady = true` when it has finished drawing, and the screenshot script waits for that flag.
- Species colors come from `meta.json`, so the sim owns them.
- Data is served from `ecoview/public/`, so `?run=runs/s42` fetches `/runs/s42/meta.json`. Pixel assertions are measured on the `#view` canvas only; the chart sits in a sidebar outside it.
- Playwright launches Chromium with the flags in the Environment section above.
- Pixel assertions are deliberately coarse. Don't add exact-image golden tests.

## The run directory contract

The run directory is the only interface between the two projects. Its full format is in SAD 1 under "Run directory format". The key points:
- `meta.json` must contain `format_version` (currently 4 for every run at the defaults, and 3 for a noise world with the water tier off; each version only adds files: `state.bin` per snapshot in 2, `events.csv` in 3, `world/` and the two water files in 4). `ecosim run --format-version 2` still writes version 2 for readers that know only 1 and 2, water files and all.
- `meta.json` carries the whole ecology palette: the `species` list with its colours, and since shot S2 an `overlays` array of `{name, lo, hi}` sRGB ramp ends (plus `mid` on `traits` and `burnt` on `fire`). Surface media and buildings are not in it — they are scene geometry and belong to the bundle's renderer. Adding a `meta.json` key does not bump `format_version`.
- `meta.json`'s `params` carries **every** section on every run. Shot S2 removed the `skip_serializing_if` that left `bundle`, `animals`, `rng` and `hunter.handling_ticks` out at their defaults: a reader of a run at the defaults could not tell a default from a key the run predated.
- `series.csv` has one row per tick.
- `events.csv` (version 3) has one row per event: `tick,kind,species,patch_x,patch_y,x,y,cause,detail`. The kinds and field meanings are in SAD 1's run directory format.
- Each snapshot is a `snap_NNNNNN/` directory (tick zero-padded to 6 digits) with:
  - `material.bin` and `light.bin`: x·y·z bytes each (from `meta.json` `dims`), x-fastest
  - `moisture.bin`, `fertility.bin`, and `height.bin`: x·y bytes each
  - with the water tier on (shot G4): `water.bin`, ponded depth on the ground grid in 0.1 mm u16, and `soil_water.bin`, soil water per ecology column in f32 mm
  - `patches.json` and `entities.json`
- Any format-4 run writes `world/{ground_h.bin, medium.bin, building_h.bin, pipes.json}` once at the run root, and a `world` object in `meta.json` describing the ground grid; `world.bundle` says whether it came from a bundle or is the synthesized all-soil grid of a noise world. The bundle format is `docs/SCENE-CONTRACT.md`; the ecology grid stays at 1 m columns, the ground grid is finer. `ecosim/worlds/capitol/` is the committed reference bundle (256 m of the Michigan State Capitol grounds, 512×512 ground cells), written by `ecosim/tools/blend_export.py` from a Blender scene kept outside the repo; its `medium.u8` is ODbL, so keep the credit in `ecosim/worlds/capitol/README.md` with it.
- All integers are little-endian.

If you change the format, update both projects and the SAD together.
