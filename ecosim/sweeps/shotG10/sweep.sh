#!/usr/bin/env bash
# Shot G10's sweep: tree.deciduous at 0, 0.25, 0.5, 0.75 and 1 on seeds 1, 2 and 3 (the noise
# strip, defaults otherwise, animals on), plus the Capitol reference run at 0, at the shipped 0.5 and at 1.
# Eighteen 20000-tick runs into target/g10/, eight at a time; `analyse.py` reads them into
# scan.csv and prints the tables FINDINGS.md quotes. Run from ecosim/ after `just build`.
set -euo pipefail
cd "$(dirname "$0")/../.."
E=./target/release/ecosim
OUT=target/g10
mkdir -p "$OUT"
jobs=()
for v in 0 0.25 0.5 0.75 1; do
    for s in 1 2 3; do
        jobs+=("run --seed $s --ticks 20000 --snapshot-every 1000 --out $OUT/d$v-s$s --set tree.deciduous=$v")
    done
done
# Every bundle-world run carries the two overrides shot G3a fixed: animals off, rainfall flat.
for v in 0 0.5 1; do
    jobs+=("run --world worlds/capitol --seed 42 --ticks 20000 --snapshot-every 1000 --out $OUT/cap-d$v \
--set animals.enabled=false --set climate.rain_gradient=0 --set tree.deciduous=$v")
done
printf '%s\n' "${jobs[@]}" | xargs -P 8 -I{} sh -c "$E {} > /dev/null"
