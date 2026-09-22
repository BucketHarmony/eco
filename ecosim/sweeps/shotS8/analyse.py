#!/usr/bin/env python3
"""Every table in FINDINGS.md, printed from the run directories shot S8 made.

The run directories themselves are not committed (`runs/` is gitignored) but they are
deterministic, so the commands at the top of FINDINGS.md rebuild them byte for byte. What is
committed is `replicates.csv`, which this script writes with `--emit` and reads back with
`--from-csv`, so the replicate tables can be reprinted without the multi-gigabyte rebuild.

    python analyse.py                       # everything, from ../../runs
    python analyse.py --from-csv            # the replicate tables only, from replicates.csv
    python analyse.py --emit                # rewrite replicates.csv from the run directories
"""

import argparse
import collections
import csv
import json
import math
import os
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RUNS = os.path.normpath(os.path.join(HERE, "..", "..", "runs"))
CSV = os.path.join(HERE, "replicates.csv")

# The 20000-tick Capitol runs the shot compares, and the three noise-strip runs it compares them
# against. `capitol-s42` is the committed reference run (animals off); the rest are S8's.
LONG = [
    ("capitol, animals on", "42", "capitol-s42-animals-20k"),
    ("capitol, animals on", "1", "s8/seeds/on-s1"),
    ("capitol, animals on", "2", "s8/seeds/on-s2"),
    ("capitol, animals on", "3", "s8/seeds/on-s3"),
    ("capitol, animals off", "42", "capitol-s42"),
    ("capitol, animals off", "1", "s8/seeds/off-s1"),
    ("capitol, animals off", "2", "s8/seeds/off-s2"),
    ("capitol, animals off", "3", "s8/seeds/off-s3"),
    ("strip, animals on", "1", "s8/strip-s1"),
    ("strip, animals on", "2", "s8/strip-s2"),
    ("strip, animals on", "3", "s8/strip-s3"),
]
REPLICATES = [("on", 16), ("off", 16), ("fill", 8)]
NAMES = {"on": "animals on", "off": "animals off", "fill": "animals on, initial_fill 1.0"}


def series(run):
    with open(os.path.join(RUNS, run, "series.csv"), encoding="utf-8") as fh:
        return list(csv.DictReader(fh))


def events(run):
    with open(os.path.join(RUNS, run, "events.csv"), encoding="utf-8") as fh:
        return list(csv.DictReader(fh))


def col(rows, key, lo=0, hi=None):
    return [float(r[key]) for r in rows[lo:hi]]


def crash(rows, window=2000, until=4000):
    """The establishment-year tree crash: the peak inside `window`, then the trough after it.

    Reported this way because the obvious metric -- the peak over [0, 2000] against the minimum
    over [800, 3000] -- reads a *rising* series as a crash, by taking its minimum before its peak.
    Two of the eight long runs rise monotonically through year one, and this is what tells them
    apart from the six that do not.
    """
    trees = [int(r["trees"]) for r in rows]
    peak = max(range(0, min(window, len(trees) - 1) + 1), key=lambda i: trees[i])
    tail = trees[peak:until + 1]
    trough = min(tail)
    return peak, trees[peak], peak + tail.index(trough), trough, 100.0 * (1 - trough / trees[peak])


def pearson(xs, ys):
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sx = math.sqrt(sum((x - mx) ** 2 for x in xs))
    sy = math.sqrt(sum((y - my) ** 2 for y in ys))
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / (sx * sy)


def causes_table(title, until=None, species=("grazer", "hunter", "tree")):
    print("## %s\n" % title)
    print("| run | seed | species | deaths | causes |")
    print("|---|---|---|---|---|")
    for label, seed, run in LONG:
        per = collections.defaultdict(collections.Counter)
        for e in events(run):
            if until is not None and int(e["tick"]) >= until:
                continue
            if e["kind"] == "death":
                per[e["species"]][e["cause"]] += 1
            elif e["kind"] == "tree_death":
                per["tree"][e["cause"]] += 1
        for sp in species:
            if not per[sp]:
                continue
            c = per[sp]
            body = ", ".join("%s %d" % (k, v) for k, v in c.most_common())
            print("| %s | %s | %s | %d | %s |" % (label, seed, sp, sum(c.values()), body))
    print()


def long_table():
    print("## The 20000-tick runs\n")
    print("| run | seed | trees t1000 | t2000 | t5000 | t10000 | t20000 | grazers t20000 | hunters t20000 |")
    print("|---|---|---|---|---|---|---|---|---|")
    for label, seed, run in LONG:
        rows = series(run)
        print("| %s | %s | %s | %s | %s | %s | %s | %s | %s |" % (
            label, seed, rows[1000]["trees"], rows[2000]["trees"], rows[5000]["trees"],
            rows[10000]["trees"], rows[20000]["trees"], rows[20000]["grazers"],
            rows[20000]["hunters"]))
    print()
    print("## The establishment-year tree crash, in the same runs\n")
    print("| run | seed | peak | then trough | cohort lost | rain, ticks 0-2000 | min moisture, ticks 200-1500 | min soil water, same window |")
    print("|---|---|---|---|---|---|---|---|")
    for label, seed, run in LONG:
        rows = series(run)
        pt, pv, tt, tv, lost = crash(rows)
        rain = sum(col(rows, "rain_mm", 0, 2000))
        print("| %s | %s | %d at %d | %d at %d | %.0f%% | %.0f mm | %.1f | %.1f mm |"
              % (label, seed, pv, pt, tv, tt, lost, rain,
                 min(col(rows, "moisture_mean", 200, 1500)),
                 min(col(rows, "soil_water_mm", 200, 1500))))
    print()


def stages_table(run="capitol-s42-animals-20k"):
    print("## Tree stage structure through the crash and the recovery (%s)\n" % run)
    germ = collections.Counter()
    for e in events(run):
        if e["kind"] == "germination":
            germ[int(e["tick"]) // 100] += 1
    print("| tick | trees | mature | young | sapling | germinations in the next 100 ticks |")
    print("|---|---|---|---|---|---|")
    for t in list(range(0, 3001, 200)) + [4000, 6000, 10000, 20000]:
        path = os.path.join(RUNS, run, "snap_%06d" % t, "entities.json")
        if not os.path.exists(path):
            continue
        with open(path, encoding="utf-8") as fh:
            ents = json.load(fh)
        c = collections.Counter(e["stage"] for e in ents if e["kind"] == "tree")
        print("| %d | %d | %d | %d | %d | %d |"
              % (t, sum(c.values()), c["mature"], c["young"], c["sapling"], germ[t // 100]))
    print()


def density_table():
    print("## Grazers per patch, Capitol against strip\n")
    print("| run | tick | patches | patches with grass | grazers | per patch | per grassy patch | median patch | busiest patch |")
    print("|---|---|---|---|---|---|---|---|---|")
    for run, tick in [("capitol-s42-animals-20k", 2000), ("capitol-s42-animals-20k", 10000),
                      ("capitol-s42-animals-20k", 20000), ("s8/strip-s1", 10000),
                      ("s8/strip-s2", 10000), ("s8/strip-s3", 10000)]:
        with open(os.path.join(RUNS, run, "meta.json"), encoding="utf-8") as fh:
            dims = json.load(fh)["dims"]
        p = dims["patch"]
        total = (dims["x"] // p) * (dims["y"] // p)
        with open(os.path.join(RUNS, run, "snap_%06d" % tick, "entities.json"), encoding="utf-8") as fh:
            ents = json.load(fh)
        with open(os.path.join(RUNS, run, "snap_%06d" % tick, "patches.json"), encoding="utf-8") as fh:
            patches = json.load(fh)
        grass = patches["grass"] if isinstance(patches, dict) else [q["grass"] for q in patches]
        grassy = sum(1 for g in grass if g > 0.01)
        occ = collections.Counter((e["x"] // p, e["y"] // p) for e in ents if e["kind"] == "grazer")
        n = sum(occ.values())
        print("| %s | %d | %d | %d | %d | %.2f | %.2f | %.0f | %d |"
              % (run, tick, total, grassy, n, n / total, n / grassy,
                 statistics.median(sorted(occ.values())), max(occ.values())))
    print()


def kills_table():
    """Kills per hunter per 1000 ticks, and grazers per hunter, in three windows.

    The first number is the one the row already had (it is the same on both worlds, because
    `hunter.satiation` limits it); the second is the one that tells the worlds apart.
    """
    print("## Kill rate and predator lag\n")
    print("| run | window | kills | mean hunters | mean grazers | kills per hunter per 1000 ticks | grazers per hunter |")
    print("|---|---|---|---|---|---|---|")
    runs = [("capitol, animals on, seed 42", "capitol-s42-animals-20k"),
            ("strip, seed 1", "s8/strip-s1"), ("strip, seed 2", "s8/strip-s2"),
            ("strip, seed 3", "s8/strip-s3")]
    for label, run in runs:
        rows = series(run)
        eaten = collections.Counter()
        for e in events(run):
            if e["kind"] == "death" and e["species"] == "grazer" and e["cause"] == "eaten":
                eaten[int(e["tick"])] += 1
        for lo, hi in ((0, 2000), (2000, 10000), (10000, 20000)):
            n = sum(v for t, v in eaten.items() if lo <= t < hi)
            h = sum(int(r["hunters"]) for r in rows[lo:hi]) / (hi - lo)
            g = sum(int(r["grazers"]) for r in rows[lo:hi]) / (hi - lo)
            print("| %s | %d-%d | %d | %.0f | %.0f | %.1f | %.1f |"
                  % (label, lo, hi, n, h, g, 1000 * n / h / (hi - lo), g / h))
    print()


def ratios():
    """The Capitol against the strip at tick 10000, as ratios, so the text never rounds by hand."""
    def one(run, tick):
        with open(os.path.join(RUNS, run, "meta.json"), encoding="utf-8") as fh:
            dims = json.load(fh)["dims"]
        p = dims["patch"]
        with open(os.path.join(RUNS, run, "snap_%06d" % tick, "patches.json"), encoding="utf-8") as fh:
            patches = json.load(fh)
        grass = patches["grass"] if isinstance(patches, dict) else [q["grass"] for q in patches]
        with open(os.path.join(RUNS, run, "snap_%06d" % tick, "entities.json"), encoding="utf-8") as fh:
            ents = json.load(fh)
        return (sum(1 for g in grass if g > 0.01),
                sum(1 for e in ents if e["kind"] == "grazer"))
    cap = one("capitol-s42-animals-20k", 10000)
    strip = [one("s8/strip-s%d" % s, 10000) for s in (1, 2, 3)]
    gp = sorted(s[0] for s in strip)
    gz = sorted(s[1] for s in strip)
    print("## Capitol against strip at tick 10000, as ratios\n")
    print("Capitol %d grassy patches and %d grazers; strip %d-%d and %d-%d."
          % (cap[0], cap[1], gp[0], gp[-1], gz[0], gz[-1]))
    print("Ratios: **%.1f-%.1fx the grassy patches** and **%.1f-%.1fx the grazers**.\n"
          % (cap[0] / gp[-1], cap[0] / gp[0], cap[1] / gz[-1], cap[1] / gz[0]))


def replicate_rows(from_csv):
    if from_csv:
        with open(CSV, encoding="utf-8") as fh:
            return [dict(cond=r["condition"], stream=int(r["stream"]), peak=int(r["peak"]),
                         peak_t=int(r["peak_tick"]), trough=int(r["trough"]),
                         trough_t=int(r["trough_tick"]), lost=float(r["lost_pct"]),
                         moist=float(r["min_moisture_200_1500"]),
                         sw_early=float(r["min_soil_water_200_1500_mm"]),
                         rain=float(r["rain_0_2000_mm"]), drought=int(r["drought_deaths"]),
                         trees=int(r["trees_2500"]))
                    for r in csv.DictReader(fh)]
    out = []
    for cond, n in REPLICATES:
        for st in range(1, n + 1):
            run = "s8/replicates/%s-st%d" % (cond, st)
            rows = series(run)
            pt, pv, tt, tv, lost = crash(rows, until=len(rows) - 1)
            drought = sum(1 for e in events(run)
                          if e["kind"] == "tree_death" and e["cause"] == "drought")
            out.append(dict(cond=cond, stream=st, peak=pv, peak_t=pt, trough=tv, trough_t=tt,
                            lost=lost, moist=min(col(rows, "moisture_mean", 200, 1500)),
                            sw_early=min(col(rows, "soil_water_mm", 200, 1500)),
                            rain=sum(col(rows, "rain_mm", 0, 2000)), drought=drought,
                            trees=int(rows[-1]["trees"])))
    return out


def emit(rows):
    with open(CSV, "w", encoding="utf-8", newline="\n") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(["condition", "stream", "peak", "peak_tick", "trough", "trough_tick",
                    "lost_pct", "min_moisture_200_1500", "min_soil_water_200_1500_mm",
                    "rain_0_2000_mm",
                    "drought_deaths", "trees_2500"])
        for r in rows:
            w.writerow([r["cond"], r["stream"], r["peak"], r["peak_t"], r["trough"], r["trough_t"],
                        "%.1f" % r["lost"], "%.1f" % r["moist"], "%.1f" % r["sw_early"],
                        "%.1f" % r["rain"],
                        r["drought"], r["trees"]])
    print("wrote %s (%d rows)" % (CSV, len(rows)))


def replicate_tables(rows):
    groups = collections.OrderedDict((c, [r for r in rows if r["cond"] == c]) for c, _ in REPLICATES)
    print("## Replicates: one world, one seed, 16 weather streams, 2500 ticks\n")
    print("| condition | runs | crash over 50% | median cohort lost | median min moisture (ticks 200-1500) | median rain (ticks 0-2000) | median drought deaths | median trees at 2500 |")
    print("|---|---|---|---|---|---|---|---|")
    for cond, rs in groups.items():
        med = lambda k: statistics.median([r[k] for r in rs])  # noqa: E731
        print("| %s | %d | %d of %d | %.0f%% | %.1f | %.0f mm | %.0f | %.0f |"
              % (NAMES[cond], len(rs), sum(1 for r in rs if r["lost"] > 50), len(rs),
                 med("lost"), med("moist"), med("rain"), med("drought"), med("trees")))
    print()
    pool = groups["on"] + groups["off"]
    r = pearson([x["moist"] for x in pool], [x["lost"] for x in pool])
    print("Pooled over the 32 animals-on and animals-off replicates, the correlation between the")
    print("establishment-window moisture minimum and the share of the tree cohort lost is")
    print("**r = %.3f**.\n" % r)
    # 61 of 255 is twice tree.dry_fraction (0.12 of available water capacity, so 30.6 on this
    # scale), which is the threshold the drought clock itself reads. The split is at twice it
    # because moisture_mean is a mean over soil columns: half of them are drier than it.
    dry = [x for x in pool if x["moist"] < 61]
    wet = [x for x in pool if x["moist"] >= 61]
    print("| establishment-window moisture minimum | runs | crash over 50% | median cohort lost |")
    print("|---|---|---|---|")
    for name, part in (("below 61 of 255", dry), ("61 of 255 or more", wet)):
        print("| %s | %d | %d | %.0f%% |" % (name, len(part), sum(1 for x in part if x["lost"] > 50),
                                             statistics.median([x["lost"] for x in part])))
    print()
    print("### Every replicate\n")
    print("| condition | stream | peak | trough | cohort lost | min moisture | min soil water | rain 0-2000 | drought deaths | trees at 2500 |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for cond, rs in groups.items():
        for x in rs:
            print("| %s | %d | %d at %d | %d at %d | %.0f%% | %.1f | %.1f mm | %.0f mm | %d | %d |"
                  % (NAMES[cond], x["stream"], x["peak"], x["peak_t"], x["trough"], x["trough_t"],
                     x["lost"], x["moist"], x["sw_early"], x["rain"], x["drought"], x["trees"]))
    print()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--from-csv", action="store_true", help="replicate tables from replicates.csv")
    ap.add_argument("--emit", action="store_true", help="rewrite replicates.csv and stop")
    a = ap.parse_args()
    if a.emit:
        emit(replicate_rows(False))
        return 0
    if not a.from_csv:
        causes_table("Event-log cause breakdown, whole runs")
        causes_table("Tree deaths in the establishment year only (ticks 0-2500)",
                     until=2500, species=("tree",))
        long_table()
        stages_table()
        density_table()
        ratios()
        kills_table()
    replicate_tables(replicate_rows(a.from_csv))
    return 0


if __name__ == "__main__":
    sys.exit(main())
