#!/usr/bin/env python3
"""The east-west split of a Capitol run (shot G3a), which report.py's whole-site numbers hide.

The strip world's `climate.rain_gradient` made the west half of the Capitol a desert in shot G3;
this script is what says whether a run is one site or two. Everything is split at x = 128, the
middle column of the 256 m crop.

    python sweeps/capitolG3-flat/eastwest.py runs/capitol-s42
"""

import json
import os
import sys
from collections import Counter

HALVES = ("west", "east")


def half(x):
    return HALVES[x >= 128]


def main():
    run = sys.argv[1]
    meta = json.load(open(os.path.join(run, "meta.json")))
    d = meta["dims"]
    cols = d["x"] * d["y"]

    # Plantable columns per half, from the run's own tick-0 material field: a column is plantable
    # where its surface voxel is soil (1), as world::is_plantable has it.
    mat = open(os.path.join(run, "snap_000000", "material.bin"), "rb").read()
    hgt = open(os.path.join(run, "snap_000000", "height.bin"), "rb").read()
    plantable = Counter(half(c % d["x"]) for c in range(cols) if mat[c + cols * hgt[c]] == 1)

    print("## Plantable columns\n")
    for h in HALVES:
        print("- %s of x = 128: %d" % (h, plantable[h]))

    print("\n## Trees by half\n")
    print("| tick | west | east | west per 1000 plantable | east per 1000 plantable |")
    print("| --- | --- | --- | --- | --- |")
    for t in meta["snapshots"]:
        if t % 5000:
            continue
        trees = [e for e in json.load(open(os.path.join(run, "snap_%06d" % t, "entities.json"))) if e["kind"] == "tree"]
        n = Counter(half(e["x"]) for e in trees)
        print(
            "| %d | %d | %d | %.1f | %.1f |"
            % (t, n["west"], n["east"], 1000.0 * n["west"] / plantable["west"], 1000.0 * n["east"] / plantable["east"])
        )

    print("\n## Tree events by half and cause\n")
    ev = Counter()
    first = {}
    for line in open(os.path.join(run, "events.csv")).read().splitlines()[1:]:
        f = line.split(",")
        if f[2] != "tree":
            continue
        h = half(int(f[5]))
        key = (h, f[7] or f[1])
        ev[key] += 1
        first.setdefault(key, int(f[0]))
    print("| event | west | east |")
    print("| --- | --- | --- |")
    for key in sorted({k for _, k in ev}):
        print("| %s | %d | %d |" % (key, ev[("west", key)], ev[("east", key)]))

    print("\n## Mean moisture by half\n")
    print("| tick | west | east |")
    print("| --- | --- | --- |")
    for t in (0, 5000, 10000, 20000):
        m = open(os.path.join(run, "snap_%06d" % t, "moisture.bin"), "rb").read()
        s = Counter()
        for c in range(cols):
            s[half(c % d["x"])] += m[c]
        print("| %d | %.1f | %.1f |" % (t, s["west"] / (cols / 2), s["east"] / (cols / 2)))


main()
