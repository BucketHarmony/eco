Build the renderer (`ecoview`) as a one-shot.

Read these in full before writing any code:
1. `docs/POC Ecology Simulator — SADs (Sim + Renderer).md`: the "Scope rules and stack" section, "Run directory format" under SAD 1 (the data contract), and all of "SAD 2 — Renderer".
2. `docs/SAD-addendum.md`, especially the Environment, Dependencies, Output files and Renderer sections. It is authoritative wherever it differs from the SAD. Its Chromium flags are verified on this machine, and the SAD's flags render blank here.

Build this in `ecoview/`. `ecoview/public/fixtures/s42-mini/` and `ecoview/public/runs/s42/` are already present and are the only data. Do not read or modify `ecosim/`, and do not regenerate the data.

Work until `npm run build` is clean, `npm test` passes, and all 8 screenshots in `shots/` match their expectation column. View each PNG yourself and write a one-line verdict per screenshot in `ecoview/shots/REPORT.md`. Commit `ecoview/` when done (public/runs/ is gitignored).

Do not ask questions; make the call and record it in `ecoview/DECISIONS.md`.
