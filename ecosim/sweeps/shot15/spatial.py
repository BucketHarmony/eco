"""Shot 15 analysis: west/east-quarter cover and event-log death causes.

usage: python spatial.py RUNS_ROOT > spatial.md

RUNS_ROOT holds one run directory per sweep cell, named g<gradient>_s<seed>, each an
`ecosim run --seed <seed> --ticks 20000 --snapshot-every 1000 --set climate.rain_gradient=<gradient>`
on the default strip, plus strip-s<seed> directories for the reference seeds at defaults.
Quarter means are over the snapshots at ticks 10000..20000 (11 snapshots), over every patch of
the quarter (water and rock patches included, as in the series' grass_mean): grass and shrub are
the patch values, tree cover is the fraction of the quarter's columns under
canopy as the sim's `canopy_cover` counts it: the 3x3 around a mature tree, a young tree's own
column, nothing for a sapling.
"""

import csv
import json
import os
import sys
from collections import Counter


def quarters(run):
    meta = json.load(open(os.path.join(run, "meta.json")))
    d = meta["dims"]
    wx, wy, patch = d["x"], d["y"], d["patch"]
    px_n = wx // patch
    q = wx // 4
    acc = {k: [0.0, 0.0, 0.0, 0] for k in ("west", "east")}
    for t in meta["snapshots"]:
        if t < 10000:
            continue
        snap = os.path.join(run, f"snap_{t:06d}")
        patches = json.load(open(os.path.join(snap, "patches.json")))
        ents = json.load(open(os.path.join(snap, "entities.json")))
        cover = set()
        for e in ents:
            if e["kind"] != "tree":
                continue
            x, y = int(e["x"]), int(e["y"])
            r = {"mature": 1, "young": 0}.get(e["stage"], -1)
            for dx in range(-r, r + 1):
                for dy in range(-r, r + 1):
                    if 0 <= x + dx < wx and 0 <= y + dy < wy:
                        cover.add((x + dx, y + dy))
        for name, (lo, hi) in (("west", (0, q)), ("east", (wx - q, wx))):
            ps = [p for i, p in enumerate(patches) if lo <= (i % px_n) * patch < hi]
            a = acc[name]
            a[0] += sum(p["grass"] for p in ps) / len(ps)
            a[1] += sum(p["shrub"] for p in ps) / len(ps)
            a[2] += sum(1 for (x, _) in cover if lo <= x < hi) / (q * wy)
            a[3] += 1
    return {k: (v[0] / v[3], v[1] / v[3], v[2] / v[3]) for k, v in acc.items()}


def causes(run):
    c = Counter()
    with open(os.path.join(run, "events.csv")) as f:
        for r in csv.DictReader(f):
            if r["kind"] == "death":
                c[(r["species"], r["cause"])] += 1
    return c


def main(root):
    names = sorted(os.listdir(root))
    print("## Death causes from events.csv, reference seeds on the strip at defaults (20000 ticks)\n")
    print("| seed | species | deaths by cause |")
    print("|---|---|---|")
    for n in names:
        if not n.startswith("strip-s"):
            continue
        c = causes(os.path.join(root, n))
        for sp in ("grazer", "hunter"):
            parts = sorted(((k[1], v) for k, v in c.items() if k[0] == sp), key=lambda kv: -kv[1])
            total = sum(v for _, v in parts)
            text = ", ".join(f"{k} {v} ({100 * v / total:.1f}%)" for k, v in parts)
            print(f"| {n[7:]} | {sp} | {total} total: {text} |")
    print("\n## West and east quarter cover, ticks 10000-20000 (mean of the snapshots)\n")
    print("| rain_gradient | seed | west grass | east grass | west shrub | east shrub | west tree cover | east tree cover |")
    print("|---|---|---|---|---|---|---|---|")
    for n in names:
        if not n.startswith("g"):
            continue
        g, s = n[1:].split("_s")
        r = quarters(os.path.join(root, n))
        w, e = r["west"], r["east"]
        print(f"| {g} | {s} | {w[0]:.3f} | {e[0]:.3f} | {w[1]:.3f} | {e[1]:.3f} | {w[2]:.3f} | {e[2]:.3f} |")


if __name__ == "__main__":
    main(sys.argv[1])
