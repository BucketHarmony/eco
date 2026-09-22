"""Shot S3 measurements. Run from ecosim/ after `just build` and the reference runs:

    ecosim run --world worlds/capitol --seed 42 --ticks 20000 --out runs/capitol-s42 \
        --set animals.enabled=false --set climate.rain_gradient=0
    for s in 1 2 3; do ecosim run --seed $s --ticks 20000 --out runs/s$s --snapshot-every 100; done
    python sweeps/shotS3/analyse.py

Everything it prints is read from those run directories; nothing is hard-coded from an earlier pass.
"""

import collections, json, math, os, statistics, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def run(name):
    return os.path.join(ROOT, "runs", name)


def meta(d):
    with open(os.path.join(d, "meta.json")) as f:
        return json.load(f)


def trees(d, tick):
    snap = os.path.join(d, "snap_%06d" % tick)
    with open(os.path.join(snap, "entities.json")) as f:
        return [e for e in json.load(f) if e["kind"] == "tree"], snap


def light_bin(snap, dims):
    with open(os.path.join(snap, "light.bin"), "rb") as f:
        return f.read()


def describe(vals):
    vals = sorted(vals)
    if not vals:
        return "n=0"
    sd = statistics.pstdev(vals) if len(vals) > 1 else 0.0
    return "n=%d distinct=%d min=%.4f med=%.4f max=%.4f sd=%.4f" % (
        len(vals), len(set(vals)), vals[0], statistics.median(vals), vals[-1], sd)


def causes(d):
    """Deaths by cause per species from events.csv."""
    out = collections.defaultdict(collections.Counter)
    path = os.path.join(d, "events.csv")
    with open(path) as f:
        head = f.readline().rstrip("\n").split(",")
        ki, si, ci = head.index("kind"), head.index("species"), head.index("cause")
        for line in f:
            r = line.rstrip("\n").split(",")
            if r[ki] in ("death", "tree_death"):
                sp = r[si] or ("tree" if r[ki] == "tree_death" else "?")
                out[sp][r[ci]] += 1
    return out


def cause_table(runs):
    print("## Event-log cause breakdown")
    print()
    print("| run | species | deaths | causes |")
    print("|---|---|---|---|")
    for label, d in runs:
        c = causes(d)
        for sp in sorted(c):
            n = sum(c[sp].values())
            txt = ", ".join("%s %d" % (k, v) for k, v in c[sp].most_common())
            print("| %s | %s | %d | %s |" % (label, sp, n, txt))
    print()


def stage_table(d, ticks, label):
    print("### %s" % label)
    print()
    print("| tick | stage | published crown_light | sampled light.bin |")
    print("|---|---|---|---|")
    dims = meta(d)["dims"]
    for t in ticks:
        ts, snap = trees(d, t)
        lb = light_bin(snap, dims)
        by = collections.defaultdict(list)
        sampled = collections.defaultdict(list)
        for e in ts:
            by[e["stage"]].append(e["crown_light"])
            i = e["x"] + dims["x"] * (e["y"] + dims["y"] * e["z"])
            sampled[e["stage"]].append(round(lb[i] / 255.0, 4))
        for st in ("sapling", "young", "mature"):
            if by[st]:
                print("| %d | %s | %s | %s |" % (t, st, describe(by[st]), describe(sampled[st])))
    print()


def crowding(d, tick, frac):
    """How many crowns cover a given crown's centre, and total canopy area per ground area."""
    ts, _ = trees(d, tick)
    dims = meta(d)["dims"]
    cover = []
    area = 0.0
    pts = [(e["x"] + 0.5, e["y"] + 0.5, frac * e["height_m"]) for e in ts]
    for x, y, r in pts:
        area += math.pi * r * r
    for x, y, r in pts:
        n = 0
        for ox, oy, orr in pts:
            if (ox - x) ** 2 + (oy - y) ** 2 <= orr * orr:
                n += 1
        cover.append(n)
    ground = dims["x"] * dims["y"]
    return len(ts), statistics.mean(cover), ground, area / ground


def main():
    cap = run("capitol-s42")
    cause_table([("capitol s42", cap)] + [("seed %d" % s, run("s%d" % s)) for s in (1, 2, 3)])

    print("## Crown light by stage")
    print()
    stage_table(cap, [0, 10000, 20000], "Capitol, seed 42, animals off")
    stage_table(run("s1"), [0, 10000, 20000], "Noise strip, seed 1")

    print("## Crowding on the Capitol")
    print()
    print("| tick | trees | mean crowns over a crown centre | ground m2 | crown area / ground |")
    print("|---|---|---|---|---|")
    p = meta(cap)["params"]["tree"]["crown_radius_frac"]
    for t in (0, 10000, 20000):
        n, cov, ground, ratio = crowding(cap, t, p)
        print("| %d | %d | %.2f | %d | %.2f |" % (t, n, cov, ground, ratio))
    print()

    # tick 0 of the Capitol is the surveyed trees the bundle imports.
    ts, _ = trees(cap, 0)
    lit = [e for e in ts if e["crown_light"] >= 0.999]
    print("Capitol tick 0: %d imported trees, %d in full sun, dimmest %.4f"
          % (len(ts), len(lit), min(e["crown_light"] for e in ts)))


main()
