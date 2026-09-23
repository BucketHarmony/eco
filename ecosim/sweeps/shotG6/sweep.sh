#!/usr/bin/env bash
# Shot G6's sweep: pipes.capacity_scale at 0, 0.25, 0.5, 1, 2 and 4 on the Capitol, seeds 1, 2
# and 3, with the two overrides every bundle-world run carries (shot G3a): animals off, rainfall
# flat. None of those binds -- the largest storm any pipe sees is 5% of its capacity at 1 -- so
# 0.001, 0.005 and 0.02 are added to show the overflow path on the real site. Twenty-seven
# 20000-tick runs into target/g6/, eighteen at a time; `analyse.py` reads them into
# scan.csv and prints the tables FINDINGS.md quotes. Run from ecosim/ after `just build`.
set -euo pipefail
cd "$(dirname "$0")/../.."
E=./target/release/ecosim
OUT=target/g6
mkdir -p "$OUT"
jobs=()
for v in ${G6_SCALES:-0 0.001 0.005 0.02 0.25 0.5 1 2 4}; do
    for s in 1 2 3; do
        jobs+=("run --world worlds/capitol --seed $s --ticks 20000 --snapshot-every 1000 --out $OUT/c$v-s$s \
--set animals.enabled=false --set climate.rain_gradient=0 --set pipes.capacity_scale=$v")
    done
done
printf '%s\n' "${jobs[@]}" | xargs -P 18 -I{} sh -c "$E {} > /dev/null"
