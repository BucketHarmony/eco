"""Shot G6: read target/g6/ (sweep.sh) into scan.csv and print FINDINGS.md's tables.

Pure CPython. The nitrogen and phosphorus that leave the world are read from the nutrient ledger at
the end of the last snapshot's state.bin (its running `outflow`, which counts the surface flow over
the edge and the pipes that empty over it together) and split with the pipe rows of events.csv,
which carry what went down each pipe. The series' `outflow_p` is a per-column mean at four decimals
and rounds a storm's phosphorus to 0, so it is not used.
"""
import collections
import csv
import json
import os
import struct
import sys

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.join(os.path.dirname(__file__), "..", "..", "target", "g6")
HERE = os.path.dirname(__file__)
YEAR = 4000
SCALES = ["0", "0.001", "0.005", "0.02", "0.25", "0.5", "1", "2", "4"]
SEEDS = [1, 2, 3]
N, P = 0, 1


def series(d):
    with open(os.path.join(d, "series.csv")) as f:
        return list(csv.DictReader(f))


def events(d):
    with open(os.path.join(d, "events.csv")) as f:
        return list(csv.DictReader(f))


def ledger_outflow(d, cols, patches):
    """The nutrient ledger's running outflow (g, N and P) in the last snapshot's state.bin."""
    b = open(os.path.join(d, "snap_020000", "state.bin"), "rb").read()
    assert b[:8] == b"ECOSTATE" and struct.unpack_from("<I", b, 8)[0] == 6
    o = 8 + 4 + 12 + 32 + 8 + 16 + 40 + 2 * cols * 4 + patches * 16
    (trees,) = struct.unpack_from("<I", b, o)
    o += 4 + trees * 19
    animals = 0
    for _ in range(2):
        (k,) = struct.unpack_from("<I", b, o)
        o += 4 + k * 26
        animals += k
    tail = len(b) - 3 * 8 * (trees + animals) - 6 * 4 - 15 * 8
    led = struct.unpack_from("<15d", b, tail)
    return led[9 + N], led[9 + P]


def near(d, snap, file, x0, y0, wx, r=3):
    """Mean of a column byte field within r m of (x0, y0)."""
    v = open(os.path.join(d, snap, file), "rb").read()
    xs = [v[x + wx * y] for x in range(x0 - r, x0 + r + 1) for y in range(y0 - r, y0 + r + 1)
          if 0 <= x < wx and 0 <= y < len(v) // wx]
    return sum(xs) / len(xs)


def row(scale, seed):
    d = os.path.join(ROOT, f"c{scale}-s{seed}")
    meta = json.load(open(os.path.join(d, "meta.json")))
    dims = meta["dims"]
    wx, cols = dims["x"], dims["x"] * dims["y"]
    patches = cols // (dims["patch"] ** 2)
    s = series(d)
    ev = events(d)
    tree_causes = collections.Counter(e["cause"] for e in ev if e["kind"] == "tree_death")
    pipe = [e for e in ev if e["kind"] == "pipe"]
    per = collections.defaultdict(lambda: [0.0, 0.0, 0.0, 0.0, 0])
    for e in pipe:
        k, c, o, n, p = e["detail"].split()
        t = per[int(k)]
        t[0] += float(c); t[1] += float(o); t[2] += float(n); t[3] += float(p)
        t[4] += float(o) > 0
    out_n, out_p = ledger_outflow(d, cols, patches)
    pipe_n = sum(t[2] for t in per.values())
    pipe_p = sum(t[3] for t in per.values())
    tail = s[-2 * YEAR:]
    snaps = [f"snap_{t:06d}" for t in range(12000, 20001, 1000)]
    inlets = [(int(p["inlet"][0]), int(p["inlet"][1])) for p in meta["world"]["pipes"]]
    grass_near = sum(
        json.load(open(os.path.join(d, sn, "patches.json")))[x // 8 + (wx // 8) * (y // 8)]["grass"]
        for sn in snaps for x, y in inlets) / (len(snaps) * len(inlets))
    moist_near = sum(near(d, sn, "moisture.bin", x, y, wx) for sn in snaps for x, y in inlets) / (len(snaps) * len(inlets))
    return {
        "scale": scale,
        "seed": seed,
        "trees_end": int(s[-1]["trees"]),
        "tree_deaths": sum(tree_causes.values()),
        "tree_causes": " ".join(f"{k}:{v}" for k, v in tree_causes.most_common()),
        "pipe_events": len(pipe),
        "storms": sum(e["kind"] == "storm" for e in ev),
        "captured_m3": round(sum(t[0] for t in per.values()), 2),
        "overflow_m3": round(sum(t[1] for t in per.values()), 2),
        "storms_overflowing": sum(t[4] for t in per.values()),
        "captured_by_pipe_m3": " ".join(f"{per[k][0]:.2f}" for k in sorted(per)),
        "pipe_in_mm": round(sum(float(r["pipe_in_mm"]) for r in s), 3),
        "outflow_mm": round(sum(float(r["outflow_mm"]) for r in s), 1),
        "ponded_mm_mean": round(sum(float(r["ponded_mm"]) for r in s) / len(s), 4),
        "ponded_mm_max": round(max(float(r["ponded_mm"]) for r in s), 3),
        "waterlogged_mean": round(sum(float(r["waterlogged_frac"]) for r in s) / len(s), 5),
        "waterlogged_last2y": round(sum(float(r["waterlogged_frac"]) for r in tail) / len(tail), 5),
        "n_pipe_g": round(pipe_n, 1),
        "n_edge_g": round(out_n - pipe_n, 1),
        "p_pipe_g": round(pipe_p, 1),
        "p_edge_g": round(out_p - pipe_p, 1),
        "grass_inlet_patches": round(grass_near, 4),
        "moisture_near_inlets": round(moist_near, 2),
    }


def main():
    rows = [row(v, s) for v in SCALES for s in SEEDS]
    with open(os.path.join(HERE, "scan.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    causes = collections.Counter()
    for r in rows:
        for kv in r["tree_causes"].split():
            k, v = kv.split(":")
            causes[(r["scale"], k)] += int(v)
    print("## tree deaths by cause, summed over seeds 1-3\n")
    names = sorted({k for _, k in causes})
    print("| scale | " + " | ".join(names) + " |")
    print("|---" * (len(names) + 1) + "|")
    for v in SCALES:
        print(f"| {v} | " + " | ".join(str(causes[(v, k)]) for k in names) + " |")
    keys = ["captured_m3", "overflow_m3", "storms_overflowing", "pipe_in_mm", "outflow_mm", "ponded_mm_mean",
            "ponded_mm_max", "waterlogged_mean", "waterlogged_last2y", "n_pipe_g", "n_edge_g", "p_pipe_g",
            "p_edge_g", "grass_inlet_patches", "moisture_near_inlets", "trees_end"]
    print("\n## means over seeds 1-3\n")
    print("| scale | " + " | ".join(keys) + " |")
    print("|---" * (len(keys) + 1) + "|")
    for v in SCALES:
        sel = [r for r in rows if r["scale"] == v]
        print(f"| {v} | " + " | ".join(f"{sum(r[k] for r in sel) / len(sel):.5g}" for k in keys) + " |")
    print("\n## captured per pipe (m3, pipes.json order), seed 1\n")
    for v in SCALES:
        print(v, next(r for r in rows if r["scale"] == v and r["seed"] == 1)["captured_by_pipe_m3"])


main()
