Build the ecology simulator (`ecosim`) as a one-shot.

Read these in full before writing any code:
1. `docs/POC Ecology Simulator — SADs (Sim + Renderer).md`: the "Scope rules and stack" section and all of "SAD 1 — Ecology Simulator".
2. `docs/SAD-addendum.md`: all of it. It is authoritative wherever it differs from the SAD, and it settles the open details (formulas, starting params, file shapes, tick semantics, extra acceptance checks).

Build this in a fresh `ecosim/` directory. Work until `cargo test --release` passes and `ecosim check` passes on seeds 1, 2 and 3, including the addendum's extra acceptance checks on `runs/s42`. Tune `params.toml` as needed, and log every parameter change and its effect in `ecosim/TUNING.md`.

When done:
- Run seed 42 for 20000 ticks with snapshot-every 100 into `ecosim/runs/s42/`.
- Run seed 42 for 100 ticks with snapshot-every 100 into `ecosim/fixtures/s42-mini/`.
- Run `bash scripts/sync-data.sh` from the repo root.
- Commit `ecosim/` (runs/ is gitignored; fixtures/ is committed).

Do not create or modify anything under `ecoview/` except through `scripts/sync-data.sh`. Do not ask questions; make the call and record it in `ecosim/DECISIONS.md`.
