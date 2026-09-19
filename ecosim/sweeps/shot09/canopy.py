"""Canopy cover fraction over time at each fire.base_rate (shot 09 FINDINGS).

Runs seeds 1-3 at each base_rate with a snapshot every 1000 ticks (no state.bin) into
sweeps/shot09/canopy_runs/ (gitignored), then counts canopied columns from entities.json:
a young tree covers its own column, a mature tree the 3x3 around its trunk (src/trees.rs).
Usage, from ecosim/: python sweeps/shot09/canopy.py > sweeps/shot09/canopy.txt
"""
import json, os, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

EXE = os.path.join("target", "release", "ecosim")
OUT = os.path.join("sweeps", "shot09", "canopy_runs")
RATES = ["0", "0.0005", "0.001", "0.0015", "0.002", "0.0025", "0.003", "0.0035", "0.004"]
SEEDS = [1, 2, 3]
TICKS = list(range(0, 20001, 1000))


def run(rate, seed):
    d = os.path.join(OUT, f"r{rate}_s{seed}")
    subprocess.run([EXE, "run", "--seed", str(seed), "--ticks", "20000", "--snapshot-every", "1000",
                    "--snapshot-state", "false", "--set", f"fire.base_rate={rate}", "--out", d],
                   check=True, stdout=subprocess.DEVNULL)
    return d


def cover(d, tick):
    cols = set()
    with open(os.path.join(d, f"snap_{tick:06d}", "entities.json"), encoding="utf-8") as f:
        for e in json.load(f):
            if e["kind"] != "tree":
                continue
            if e["stage"] == "young":
                cols.add((e["x"], e["y"]))
            elif e["stage"] == "mature":
                for dx in (-1, 0, 1):
                    for dy in (-1, 0, 1):
                        x, y = e["x"] + dx, e["y"] + dy
                        if 0 <= x < 64 and 0 <= y < 64:
                            cols.add((x, y))
    return len(cols) / 4096


def main():
    jobs = [(r, s) for r in RATES for s in SEEDS]
    with ThreadPoolExecutor(22) as ex:
        dirs = dict(zip(jobs, ex.map(lambda j: run(*j), jobs)))
    shown = [t for t in TICKS if t % 2000 == 0]
    print("Canopy cover fraction (canopied columns / 4096), mean of seeds 1-3; last column: mean over ticks 15000-20000 per seed")
    print()
    print("| base_rate | " + " | ".join(str(t) for t in shown) + " | late mean s1 / s2 / s3 |")
    print("|---" * (len(shown) + 2) + "|")
    for r in RATES:
        per = {s: [cover(dirs[(r, s)], t) for t in TICKS] for s in SEEDS}
        mean = [sum(per[s][i] for s in SEEDS) / 3 for i in range(len(TICKS))]
        late = [sum(per[s][15:]) / 6 for s in SEEDS]
        print(f"| {r} | " + " | ".join(f"{mean[TICKS.index(t)]:.3f}" for t in shown)
              + " | " + " / ".join(f"{v:.3f}" for v in late) + " |")


if __name__ == "__main__":
    sys.exit(main())
