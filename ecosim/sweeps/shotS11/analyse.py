#!/usr/bin/env python3
"""Every table in FINDINGS.md, printed from the run directories shot S11 made.

The run directories are not committed (`runs/` and `target/` are gitignored) but they are
deterministic, so `sweep.sh` in this directory rebuilds them byte for byte. What is committed is
`scan.csv`, which this script writes with `--emit` and reads back otherwise, so the 43 sweep runs
need not exist to reprint the tables.

    python analyse.py                 # everything: reference runs from ../../runs, sweep from scan.csv
    python analyse.py --emit          # rewrite scan.csv from the sweep run directories
    python analyse.py --root DIR      # where the sweep runs live (default ../../target/s11)

The "before" column of the reference tables reads `runs/<name>-preS11`, the same runs made by the
binary of the shot's parent commit. Those directories are optional: without them the tables print
the "after" column only.
"""

import argparse
import collections
import csv
import json
import math
import os
import statistics
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
RUNS = os.path.join(ROOT, "runs")
CSV_PATH = os.path.join(HERE, "scan.csv")

# The reference runs, as (label, run directory). The Capitol run is the one the row's calibration
# anchor was counted from; the four noise-strip seeds are the acceptance seeds.
REFERENCE = [
    ("capitol s42", "capitol-s42"),
    ("seed 1", "s1"),
    ("seed 2", "s2"),
    ("seed 3", "s3"),
    ("seed 42", "s42"),
]

# The sweep, as (group, condition, seed, stream, directory under --root). `base` is the parent
# commit's binary: the trunk-count crowding term, with no threshold at all.
THRESHOLDS = ["0.25", "0.50", "0.55", "0.60", "0.65", "0.70", "0.75", "1.0"]
SEED_SCAN = ["0.50", "0.55", "0.65"]
REPLICATES = ["base", "0.45", "0.55", "0.65", "0.75"]


def sweep_dirs():
    out = [("threshold", "base", 42, 0, "cap-base")]
    for v in THRESHOLDS:
        out.append(("threshold", v, 42, 0, "cap-o%s" % v))
    for s in (1, 2, 3):
        for v in SEED_SCAN:
            out.append(("seeds", v, s, 0, "capS%d-o%s" % (s, v)))
    for v in REPLICATES:
        for st in range(1, 9):
            out.append(("replicates", v, 42, st, "rep/%s-%d" % (v, st)))
    return out


def series(d):
    with open(os.path.join(d, "series.csv"), newline="") as f:
        return list(csv.DictReader(f))


def deaths(d):
    """Deaths by species and cause. Trees die under `tree_death`, animals under `death`."""
    out = collections.defaultdict(collections.Counter)
    with open(os.path.join(d, "events.csv"), newline="") as f:
        for r in csv.DictReader(f):
            if r["kind"] in ("death", "tree_death"):
                out[r["species"]][r["cause"]] += 1
    return out


def trees_at(d, tick):
    p = os.path.join(d, "snap_%06d" % tick, "entities.json")
    if not os.path.exists(p):
        return None
    with open(p) as f:
        return [e for e in json.load(f) if e["kind"] == "tree"]


def crown_cover(d, tick, dims):
    """Summed crown area over site area: 1.0 is one full canopy, above 1 is crowns overlapping.

    The cover is `None` for a run made before shot S3, which did not publish `crown_radius_m`; the
    stem count is still countable there.
    """
    tr = trees_at(d, tick)
    if tr is None:
        return None, None
    area = dims["x"] * dims["y"]
    stems = len(tr) / (area / 10000.0)
    if any("crown_radius_m" not in e for e in tr):
        return None, stems
    return sum(math.pi * e["crown_radius_m"] ** 2 for e in tr) / area, stems


def summarise(d):
    with open(os.path.join(d, "meta.json")) as f:
        meta = json.load(f)
    rows = series(d)
    trees = [int(r["trees"]) for r in rows]
    c = deaths(d)["tree"]
    tot = sum(c.values()) or 1
    cover, stems = crown_cover(d, int(meta["ticks"]), meta["dims"])
    mature = trees_at(d, 10000)
    return {
        "trees_end": trees[-1],
        "trees_min": min(trees),
        "trees_max": max(trees),
        "deaths": sum(c.values()),
        "crowded": 100.0 * c["crowded"] / tot,
        "drought": 100.0 * c["drought"] / tot,
        "burnt": 100.0 * c["burnt"] / tot,
        "old_age": 100.0 * c["old_age"] / tot,
        "cover": cover,
        "stems_ha": stems,
        "mature_10k": "" if mature is None else sum(1 for e in mature if e.get("stage") == "mature"),
    }


FIELDS = [
    "group", "cond", "seed", "stream", "trees_end", "trees_min", "trees_max", "deaths",
    "crowded", "drought", "burnt", "old_age", "cover", "stems_ha", "mature_10k",
]


def emit(root):
    rows = []
    for group, cond, seed, stream, rel in sweep_dirs():
        d = os.path.join(root, rel)
        if not os.path.isdir(d):
            print("missing, skipped: %s" % d, file=sys.stderr)
            continue
        r = {"group": group, "cond": cond, "seed": seed, "stream": stream}
        r.update(summarise(d))
        for k in ("crowded", "drought", "burnt", "old_age"):
            r[k] = round(r[k], 2)
        r["cover"] = "" if r["cover"] is None else round(r["cover"], 4)
        r["stems_ha"] = round(r["stems_ha"], 2)
        rows.append(r)
    with open(CSV_PATH, "w", newline="") as f:
        w = csv.DictWriter(f, FIELDS, lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    print("wrote %s (%d rows)" % (CSV_PATH, len(rows)))


def read_scan():
    with open(CSV_PATH, newline="") as f:
        rows = list(csv.DictReader(f))
    for r in rows:
        for k in ("trees_end", "trees_min", "trees_max", "deaths", "seed", "stream"):
            r[k] = int(r[k])
        for k in ("crowded", "drought", "burnt", "old_age", "cover", "stems_ha"):
            r[k] = float(r[k])
    return rows


def table(header, rows):
    print("| " + " | ".join(header) + " |")
    print("|" + "|".join("---" for _ in header) + "|")
    for r in rows:
        print("| " + " | ".join(str(c) for c in r) + " |")
    print()


def causes_table():
    print("## Event-log cause breakdown, seeds 1, 2, 3 and 42 and the Capitol run\n")
    rows = []
    for label, name in REFERENCE:
        for suffix, when in (("-preS11", "before"), ("", "after")):
            d = os.path.join(RUNS, name + suffix)
            if not os.path.isdir(d):
                continue
            c = deaths(d)
            for sp in ("tree", "grazer", "hunter"):
                if sp not in c:
                    continue
                tot = sum(c[sp].values())
                txt = ", ".join("%s %d (%.1f%%)" % (k, v, 100.0 * v / tot) for k, v in c[sp].most_common())
                rows.append([label, sp, when, tot, txt])
    table(["run", "species", "shot", "deaths", "causes"], rows)


def stand_table():
    print("## The stand before and after\n")
    rows = []
    for label, name in REFERENCE:
        for suffix, when in (("-preS11", "before"), ("", "after")):
            d = os.path.join(RUNS, name + suffix)
            if not os.path.isdir(d):
                continue
            s = summarise(d)
            r = series(d)
            idx = {int(x["tick"]): x for x in r}
            grass = statistics.mean(float(x["grass_mean"]) for x in r)
            shrub = statistics.mean(float(x["shrub_mean"]) for x in r)
            rows.append([
                label, when, r[0]["trees"], idx.get(10000, {}).get("trees", "-"),
                s["trees_end"], s["trees_min"], s["mature_10k"], "%.3f" % grass, "%.3f" % shrub,
                "n/a" if s["cover"] is None else "%.2f" % s["cover"], "%.0f" % s["stems_ha"],
            ])
    table(
        ["run", "shot", "trees@0", "trees@10k", "trees@end", "min trees", "mature@10k",
         "grass mean", "shrub mean", "crown cover", "stems/ha"],
        rows,
    )


def lens_area(r, big, d):
    if d >= r + big:
        return 0.0
    if d <= abs(big - r):
        return math.pi * min(r, big) ** 2
    a = math.acos((d * d + r * r - big * big) / (2 * d * r))
    b = math.acos((d * d + big * big - r * r) / (2 * d * big))
    return r * r * (a - math.sin(2 * a) / 2) + big * big * (b - math.sin(2 * b) / 2)


def overlap_fraction(a, b):
    """The port of `ecosim::trees::overlap_fraction`: how much of a's disc b covers."""
    ra, rb = a["crown_radius_m"], b["crown_radius_m"]
    if ra <= 0.0 or rb <= 0.0:
        return 0.0
    d = math.hypot(a["x"] - b["x"], a["y"] - b["y"])
    return lens_area(ra, rb, d) / (math.pi * ra * ra)


def calibration_table():
    print("## What the site's own trees measure, at tick 0\n")
    tr = trees_at(os.path.join(RUNS, "capitol-s42"), 0)
    if tr is None:
        print("(no capitol-s42 run)\n")
        return
    vals = []
    for i, a in enumerate(tr):
        opened = 1.0
        for j, b in enumerate(tr):
            if i != j:
                f = overlap_fraction(a, b)
                if f > 0.0:
                    opened *= 1.0 - f
        vals.append(1.0 - opened)
    vals.sort()
    n = len(vals)
    pct = lambda p: vals[min(n - 1, int(math.ceil(p * n)) - 1)]
    print("n=%d  median=%.3f  p75=%.3f  p90=%.3f  p95=%.3f  max=%.3f\n"
          % (n, statistics.median(vals), pct(0.75), pct(0.90), pct(0.95), max(vals)))
    table(
        ["threshold", "surveyed trees at or over it", "share"],
        [[t, sum(1 for v in vals if v >= t), "%.1f%%" % (100.0 * sum(1 for v in vals if v >= t) / n)]
         for t in (0.25, 0.45, 0.50, 0.55, 0.60, 0.65, 0.75, 1.0)],
    )


def scan_tables():
    rows = read_scan()
    print("## Threshold scan, Capitol seed 42\n")
    table(
        ["crowding_overlap", "trees@end", "min", "max", "mature@10k", "tree deaths",
         "crowded", "drought", "burnt", "old age", "crown cover", "stems/ha"],
        [[r["cond"], r["trees_end"], r["trees_min"], r["trees_max"], r["mature_10k"], r["deaths"],
          "%.1f%%" % r["crowded"], "%.1f%%" % r["drought"], "%.1f%%" % r["burnt"], "%.1f%%" % r["old_age"],
          "%.2f" % r["cover"], "%.0f" % r["stems_ha"]]
         for r in rows if r["group"] == "threshold"],
    )

    print("## The same three thresholds on three other terrains\n")
    table(
        ["terrain seed", "crowding_overlap", "trees@end", "min", "crowded", "crown cover"],
        [[r["seed"], r["cond"], r["trees_end"], r["trees_min"], "%.1f%%" % r["crowded"], "%.2f" % r["cover"]]
         for r in rows if r["group"] == "seeds"],
    )

    print("## Eight RNG streams per condition, Capitol seed 42\n")
    out = []
    for cond in REPLICATES:
        rs = [r for r in rows if r["group"] == "replicates" and r["cond"] == cond]
        if not rs:
            continue

        def span(key, places=1, rs=rs):
            vals = [r[key] for r in rs]
            return "%.*f [%.*f, %.*f]" % (places, statistics.median(vals), places, min(vals), places, max(vals))

        out.append([cond, len(rs), span("trees_end", 0), span("crowded"), span("cover", 2)])
    table(["crowding_overlap", "n", "trees@end median [min, max]", "crowded median [min, max]",
           "crown cover median [min, max]"], out)


def check_table():
    """`ecosim check` on each reference run, as the acceptance gate sees it."""
    exe = os.path.join(ROOT, "target", "release", "ecosim" + (".exe" if os.name == "nt" else ""))
    if not os.path.exists(exe):
        print("## `ecosim check`\n\n(no release binary; run `just build`)\n")
        return
    print("## `ecosim check` on the five reference runs\n")
    rows = []
    for label, name in REFERENCE:
        d = os.path.join(RUNS, name)
        if not os.path.isdir(d):
            continue
        out = subprocess.run([exe, "check", d], capture_output=True, text=True).stdout.splitlines()
        passed = sum(1 for l in out if l.startswith("PASS"))
        na = sum(1 for l in out if l.startswith("N/A"))
        failed = [l for l in out if l.startswith("FAIL")]
        rows.append([label, passed, na, len(failed), "; ".join(failed) or "none"])
    table(["run", "PASS", "N/A", "FAIL", "failures"], rows)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--emit", action="store_true", help="rewrite scan.csv from the sweep run directories")
    ap.add_argument("--root", default=os.path.join(ROOT, "target", "s11"), help="where the sweep runs live")
    args = ap.parse_args()
    if args.emit:
        emit(args.root)
        return
    causes_table()
    stand_table()
    check_table()
    calibration_table()
    scan_tables()


if __name__ == "__main__":
    main()
