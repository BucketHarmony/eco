#!/usr/bin/env bash
# Handoff between the two one-shot sessions: copy sim output into the renderer's public/ dir.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
for d in runs/s42 fixtures/s42-mini; do
  src="$root/ecosim/$d"
  dst="$root/ecoview/public/$d"
  [ -f "$src/meta.json" ] || { echo "missing $src/meta.json — run ecosim first" >&2; exit 1; }
  rm -rf "$dst"; mkdir -p "$(dirname "$dst")"; cp -r "$src" "$dst"
  echo "copied $d ($(ls -d "$dst"/snap_* | wc -l) snapshots)"
done
