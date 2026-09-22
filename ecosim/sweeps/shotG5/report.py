#!/usr/bin/env python3
"""Shot G5's report on the Capitol nutrient runs: the numbers in FINDINGS.md and one PNG per pool.

Reads a format-4 run directory with the nutrient tier on (`snap_NNNNNN/npk.bin`) and writes
n-20000.png, p-20000.png and k-20000.png plus a markdown block on stdout. Pure CPython, no
third-party package, the same rule as tools/blend_export.py and sweeps/capitolG3/report.py.

    python sweeps/shotG5/report.py ci-runs/g5cap/dep2.5 sweeps/shotG5

The groups it reports the pools over are ground-cell neighbourhoods of a plantable ecology column:
a roof edge is where a roof sheds onto soil, a street edge where asphalt does, and mid-lawn is lawn
with no sealed cell within 3 m. `pipes.json`'s inlets are reported beside the roof edge and are not
the same thing: nothing routes a pipe before shot G6, so an inlet is only a marked spot on a roof,
and the water still runs off the roof's own edge.
"""

import json
import math
import os
import struct
import sys
import zlib
from collections import Counter

SEALED = {"roof", "asphalt", "concrete"}
NAMES = ["N", "P", "K"]
YEAR = 4000  # params.toml climate.year_len


def write_png(path, w, h, pix):
    """PNG writer, the one in sweeps/capitolG3/report.py."""
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        raw += pix[y * w * 3 : (y + 1) * w * 3]

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    head = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", head))
        f.write(chunk(b"IDAT", zlib.compress(bytes(raw), 9)))
        f.write(chunk(b"IEND", b""))


def load(run):
    meta = json.load(open(os.path.join(run, "meta.json")))
    d, w = meta["dims"], meta["world"]
    n = w["ground_width"] * w["ground_depth"]
    assert n > 0
    g = {
        "width": w["ground_width"],
        "depth": w["ground_depth"],
        "ratio": w["ground_width"] // d["x"],
        "media": w["media"],
        "medium": open(os.path.join(run, "world", "medium.bin"), "rb").read(),
        "pipes": json.load(open(os.path.join(run, "world", "pipes.json"))),
        "cell_m": w["ground_cell_m"],
    }
    return meta, d, g


def pools(run, tick, cols):
    """The three planes of `npk.bin`, in grams per square metre, as written."""
    b = open(os.path.join(run, "snap_%06d" % tick, "npk.bin"), "rb").read()
    assert len(b) == cols * 3 * 4, "npk.bin is three f32 planes"
    return [list(struct.unpack("<%df" % cols, b[i * cols * 4 : (i + 1) * cols * 4])) for i in range(3)]


def groups(d, g):
    """Each plantable column's group: roof edge, street edge, mid-lawn, other soil, or None.

    Plantable is the Rock/Water/Soil rule `World::from_bundle` applies -- a column is soil unless
    half its ground cells are sealed or half are water -- so the groups partition the columns the
    nutrient tier actually holds a pool for.
    """
    r, gw, gd = g["ratio"], g["width"], g["depth"]
    media = [g["media"][b] for b in g["medium"]]
    n = r * r
    klass, near = [], []
    reach = max(1, int(round(3.0 / g["cell_m"])))  # 3 m in ground cells
    for y in range(d["y"]):
        for x in range(d["x"]):
            sealed = water = 0
            for gy in range(y * r, (y + 1) * r):
                for gx in range(x * r, (x + 1) * r):
                    m = media[gx + gw * gy]
                    sealed += m in SEALED
                    water += m == "water"
            klass.append("rock" if 2 * sealed > n else "water" if 2 * water > n else "soil")
            seen = set()
            for gy in range(max(0, y * r - reach), min(gd, (y + 1) * r + reach)):
                for gx in range(max(0, x * r - reach), min(gw, (x + 1) * r + reach)):
                    seen.add(media[gx + gw * gy])
            near.append(seen)
    inlet = set()
    for p in g["pipes"]:
        cx, cy = int(p["inlet"][0]), int(p["inlet"][1])
        for y in range(max(0, cy - 3), min(d["y"], cy + 4)):
            for x in range(max(0, cx - 3), min(d["x"], cx + 4)):
                inlet.add(x + d["x"] * y)
    out = []
    for c in range(d["x"] * d["y"]):
        if klass[c] != "soil":
            out.append(None)
        elif "roof" in near[c]:
            out.append("roof edge")
        elif "asphalt" in near[c]:
            out.append("street edge")
        elif not (near[c] & SEALED) and "lawn" in near[c]:
            out.append("mid-lawn")
        else:
            out.append("other soil")
    return klass, out, inlet


def liebig_limit(avail, needs, half_sat):
    """`npk::limiting`: the element whose saturating term is smallest. Ties to the lower index."""
    best = None
    for i in range(3):
        half = half_sat[i] * needs[i]
        if half <= 0.0:
            continue
        f = max(avail[i], 0.0) / (max(avail[i], 0.0) + half)
        if best is None or f < best[1]:
            best = (i, f)
    return best


def ramp(path, d, vals, group, lo, hi, rgb):
    """One pool, north up, one pixel per ecology column: log-scaled, non-plantable columns grey."""
    pix = bytearray(d["x"] * d["y"] * 3)
    span = math.log10(hi) - math.log10(lo)
    for y in range(d["y"]):
        for x in range(d["x"]):
            c = x + d["x"] * y
            if group[c] is None:
                col = (52, 52, 56)
            else:
                t = (math.log10(max(vals[c], lo)) - math.log10(lo)) / span
                t = min(1.0, max(0.0, t))
                col = tuple(int(24 + (v - 24) * (0.15 + 0.85 * t)) for v in rgb)
            o = (x + d["x"] * (d["y"] - 1 - y)) * 3
            pix[o], pix[o + 1], pix[o + 2] = col
    write_png(path, d["x"], d["y"], pix)


def series(run):
    lines = open(os.path.join(run, "series.csv")).read().splitlines()
    head = lines[0].split(",")
    idx = {k: head.index(k) for k in ("soil_n", "soil_p", "soil_k", "leached_n", "outflow_p", "waterlogged_frac")}
    return idx, [line.split(",") for line in lines[1:]]


def main():
    run, out = sys.argv[1], sys.argv[2]
    meta, d, g = load(run)
    cols = d["x"] * d["y"]
    tick = meta["ticks"]
    np_ = meta["params"]["npk"]
    _klass, group, inlet = groups(d, g)
    soil = pools(run, tick, cols)
    avail = [[soil[0][c], soil[1][c] * np_["p_avail_frac"], soil[2][c]] for c in range(cols)]
    plantable = [c for c in range(cols) if group[c] is not None]

    print("## Event-log cause breakdown\n")
    rows = Counter()
    for line in open(os.path.join(run, "events.csv")).read().splitlines()[1:]:
        f = line.split(",")
        rows[(f[2] or "-", f[1], f[7] or "-")] += 1
    print("| species | kind | cause | events |")
    print("| --- | --- | --- | --- |")
    for (sp, kind, cause), n in sorted(rows.items(), key=lambda kv: (kv[0][0], -kv[1])):
        print("| %s | %s | %s | %d |" % (sp, kind, cause, n))

    print("\n## Which nutrient limits growth, tick %d\n" % tick)
    print("| species | limited by N | by P | by K | mean Liebig factor |")
    print("| --- | --- | --- | --- | --- |")
    for name in ("grass", "shrub", "tree"):
        sp = meta["params"][name]["npk"]
        needs = [sp["need_n"], sp["need_p"], sp["need_k"]]
        lim = Counter()
        f = 0.0
        for c in plantable:
            i, v = liebig_limit(avail[c], needs, np_["half_sat"])
            lim[i] += 1
            f += v
        share = [100.0 * lim[i] / len(plantable) for i in range(3)]
        print("| %s | %.1f%% | %.1f%% | %.1f%% | %.3f |" % (name, share[0], share[1], share[2], f / len(plantable)))
    print("\nOver the %d plantable columns of %d. Growth reads the patch mean, not the column," % (len(plantable), cols))
    print("so the same question per patch is the one the sim acts on:\n")
    px = d["x"] // d["patch"]
    patches = {}
    for c in plantable:
        patches.setdefault((c % d["x"]) // d["patch"] + px * ((c // d["x"]) // d["patch"]), []).append(c)
    sp = meta["params"]["grass"]["npk"]
    needs = [sp["need_n"], sp["need_p"], sp["need_k"]]
    lim = Counter()
    for _p, cs in patches.items():
        mean = [sum(avail[c][i] for c in cs) / len(cs) for i in range(3)]
        lim[liebig_limit(mean, needs, np_["half_sat"])[0]] += 1
    print("- grass, per patch: " + ", ".join("%s %d of %d" % (NAMES[i], lim[i], len(patches)) for i in range(3)))

    print("\n## The pools by where the column stands, tick %d\n" % tick)
    print("| group | columns | N g/m2 | available P g/m2 | K g/m2 |")
    print("| --- | --- | --- | --- | --- |")
    for name in ["roof edge", "street edge", "mid-lawn", "other soil"]:
        cs = [c for c in plantable if group[c] == name]
        if not cs:
            continue
        m = [sum(avail[c][i] for c in cs) / len(cs) for i in range(3)]
        print("| %s | %d | %.3f | %.3f | %.3f |" % (name, len(cs), m[0], m[1], m[2]))
    cs = [c for c in inlet if group[c] is not None]
    if cs:
        m = [sum(avail[c][i] for c in cs) / len(cs) for i in range(3)]
        print("| (pipe inlet, 3 m) | %d | %.3f | %.3f | %.3f |" % (len(cs), m[0], m[1], m[2]))

    print("\n## Leaching, runoff and waterlogging\n")
    idx, rows = series(run)
    print("| year | leached N g | runoff P g | mean waterlogged frac | max waterlogged frac | end soil N kg |")
    print("| --- | --- | --- | --- | --- | --- |")
    for y in range(len(rows) // YEAR):
        # Row 0 is tick 0, before any tick has run, so year 1 is rows 1..YEAR.
        part = rows[y * YEAR + 1 : (y + 1) * YEAR + 1]
        wl = [float(r[idx["waterlogged_frac"]]) for r in part]
        print(
            "| %d | %.0f | %.0f | %.4f | %.4f | %.3f |"
            % (
                y + 1,
                sum(float(r[idx["leached_n"]]) for r in part),
                sum(float(r[idx["outflow_p"]]) for r in part),
                sum(wl) / len(wl),
                max(wl),
                float(part[-1][idx["soil_n"]]),
            )
        )

    for i, rgb in enumerate([(120, 200, 255), (255, 190, 110), (200, 140, 255)]):
        seen = sorted(soil[i][c] for c in plantable)
        lo = max(1e-4, seen[len(seen) // 50])
        hi = max(lo * 1.01, seen[-len(seen) // 50 - 1])
        path = os.path.join(out, "%s-%d.png" % (NAMES[i].lower(), tick))
        ramp(path, d, soil[i], group, lo, hi, rgb)
        print(
            "\nwrote %s: %dx%d px, north up, log ramp over the 2nd to 98th percentile of the"
            " plantable columns, %.4f to %.2f g/m2" % (path, d["x"], d["y"], lo, hi)
        )


main()
