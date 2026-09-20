#!/usr/bin/env bash
# Handoff between the two one-shot sessions: copy sim output into the renderer's public/ dir.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
# The mini fixture is copied from the highest-format ecosim/fixtures/s42-mini* directory
# (s42-mini is v1, s42-mini-v2 is v2, ...), always to public/fixtures/s42-mini.
mini="" best=-1
for f in "$root"/ecosim/fixtures/s42-mini*/meta.json; do
  v=$(grep -o '"format_version": *[0-9]*' "$f" | grep -o '[0-9]*$')
  if [ "${v:-0}" -gt "$best" ]; then best=$v; mini="fixtures/$(basename "$(dirname "$f")")"; fi
done
# runs/capitol-s42 is the Capitol reference run (ecosim shots G3/G3a, ecoview shot G7). Generate it with
#   ecosim run --world worlds/capitol --seed 42 --ticks 20000 --out runs/capitol-s42 \
#     --snapshot-every 1000 --set animals.enabled=false --set climate.rain_gradient=0
# Every bundle-world run carries those two overrides (MASTER.md, 2026-09-19 21:05).
# worlds/capitol is the committed reference bundle; ecoview shot E1's editor loads it with ?world=
# fixtures/capitol-world, and Playwright needs it committed, since public/runs/ is gitignored.
for d in runs/s42 runs/capitol-s42 "$mini:fixtures/s42-mini" fixtures/capitol-mini "worlds/capitol:fixtures/capitol-world"; do
  src="$root/ecosim/${d%%:*}"
  dst="$root/ecoview/public/${d##*:}"
  # A run directory is known by meta.json and a world bundle by bundle.json.
  [ -f "$src/meta.json" ] || [ -f "$src/bundle.json" ] || { echo "missing $src — run ecosim first" >&2; exit 1; }
  rm -rf "$dst"; mkdir -p "$(dirname "$dst")"; cp -r "$src" "$dst"
  echo "copied ${d%%:*} -> ${d##*:} ($(ls -d "$dst"/snap_* 2>/dev/null | wc -l) snapshots)"
done
