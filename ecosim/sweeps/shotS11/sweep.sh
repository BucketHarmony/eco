#!/usr/bin/env bash
# Shot S11's sweep: 58 twenty-thousand-tick runs into target/s11/, which `analyse.py --emit` reads
# into scan.csv. Run from ecosim/ after `just build`. It takes a few hours on one machine.
#
# `cmd //c start //affinity FFFF //wait //b` pins each run to the performance cores; on Windows an
# unpinned run drifts onto the E-cores and takes about twice as long (PERF.md). On any other OS,
# drop the prefix — the runs are deterministic either way, only the timings move.
#
# The `base` condition is the shot's parent commit, whose crowding term counted trunks and had no
# `crowding_overlap` at all. Build it first, with its own params (the current params.toml carries a
# key that binary rejects, so it must be passed `--params`):
#
#     git worktree add ../eco-preS11 6c796b0
#     (cd ../eco-preS11/ecosim && cargo build --release)
#     cp ../eco-preS11/ecosim/target/release/ecosim.exe target/ecosim-base.exe
#     cp ../eco-preS11/ecosim/params.toml target/params-base.toml
#
set -euo pipefail
cd "$(dirname "$0")/../.."

NEW='target\release\ecosim.exe'
BASE='target\ecosim-base.exe'
OUT=target/s11
mkdir -p "$OUT"

pin() { cmd //c start //affinity FFFF //wait //b "$@" > /dev/null; }

# Every bundle-world run carries the two overrides shot G3a fixed: animals off, rainfall flat.
CAP=(--world worlds/capitol --set animals.enabled=false --set climate.rain_gradient=0)

# 1. The Capitol reference run, before and after, at the shipped default.
pin "$BASE" run --params target/params-base.toml "${CAP[@]}" --seed 42 --ticks 20000 \
    --out "$OUT/cap-base" --snapshot-every 100
pin "$NEW" run "${CAP[@]}" --seed 42 --ticks 20000 --out "$OUT/cap-new" --snapshot-every 100

# 2. The threshold scan on the reference terrain.
for v in 0.25 0.50 0.55 0.60 0.65 0.70 0.75 1.0; do
    pin "$NEW" run "${CAP[@]}" --seed 42 --ticks 20000 --out "$OUT/cap-o$v" --snapshot-every 100 \
        --set "tree.crowding_overlap=$v"
done

# 3. Three thresholds on three other terrains: does the ordering survive a change of ground?
for v in 0.50 0.55 0.65; do
    for s in 1 2 3; do
        pin "$NEW" run "${CAP[@]}" --seed "$s" --ticks 20000 --out "$OUT/capS$s-o$v" \
            --snapshot-every 1000 --set "tree.crowding_overlap=$v"
    done
done

# 4. Eight independent RNG streams per condition on the reference terrain. These are the runs that
# say the threshold's level is not resolvable at this sample size. Snapshots only at the ends.
mkdir -p "$OUT/rep"
for v in base 0.45 0.55 0.65 0.75; do
    for st in 1 2 3 4 5 6 7 8; do
        if [ "$v" = base ]; then
            pin "$BASE" run --params target/params-base.toml "${CAP[@]}" --seed 42 --ticks 20000 \
                --out "$OUT/rep/base-$st" --snapshot-every 20000 --set "rng.stream=$st"
        else
            pin "$NEW" run "${CAP[@]}" --seed 42 --ticks 20000 --out "$OUT/rep/$v-$st" \
                --snapshot-every 20000 --set "rng.stream=$st" --set "tree.crowding_overlap=$v"
        fi
    done
done

# 5. The cost table: two profiled strip runs per binary, on the default world.
for i in 1 2; do
    pin "$BASE" run --params target/params-base.toml --seed 42 --ticks 20000 \
        --out "$OUT/p-base$i" --snapshot-every 20000 --profile "$OUT/prof-base$i.json"
    pin "$NEW" run --seed 42 --ticks 20000 --out "$OUT/p-new$i" --snapshot-every 20000 \
        --profile "$OUT/prof-new$i.json"
done

echo "done; now: python sweeps/shotS11/analyse.py --emit"
