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
for d in runs/s42 "$mini:fixtures/s42-mini"; do
  src="$root/ecosim/${d%%:*}"
  dst="$root/ecoview/public/${d##*:}"
  [ -f "$src/meta.json" ] || { echo "missing $src/meta.json — run ecosim first" >&2; exit 1; }
  rm -rf "$dst"; mkdir -p "$(dirname "$dst")"; cp -r "$src" "$dst"
  echo "copied ${d%%:*} -> ${d##*:} ($(ls -d "$dst"/snap_* | wc -l) snapshots)"
done
