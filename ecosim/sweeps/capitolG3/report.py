#!/usr/bin/env python3
"""Shot G3's report on the Capitol reference run: the numbers in FINDINGS.md and the vegetation PNG.

Reads a format-4 run directory (the world bundle's grids sit in world/) and writes
vegetation-20000.png plus a markdown block on stdout. Pure CPython, no third-party package, the
same rule as tools/blend_export.py.

    python sweeps/capitolG3/report.py runs/capitol-s42 sweeps/capitolG3
"""

import json
import os
import struct
import sys
import zlib
from collections import Counter

SEALED = {"roof", "asphalt", "concrete"}
# The stages ecosim records in entities.json, in growth order.
STAGES = ["sapling", "young", "mature"]


def load(run):
    meta = json.load(open(os.path.join(run, "meta.json")))
    d = meta["dims"]
    w = meta["world"]
    n = w["ground_width"] * w["ground_depth"]
    g = {
        "width": w["ground_width"],
        "depth": w["ground_depth"],
        "ratio": w["ground_width"] // d["x"],
        "media": w["media"],
        "medium": open(os.path.join(run, "world", "medium.bin"), "rb").read(),
        "building_h": struct.unpack("<%df" % n, open(os.path.join(run, "world", "building_h.bin"), "rb").read()),
    }
    return meta, d, g


def snap(run, tick):
    p = os.path.join(run, "snap_%06d" % tick)
    return {
        "height": open(os.path.join(p, "height.bin"), "rb").read(),
        "entities": json.load(open(os.path.join(p, "entities.json"))),
        "patches": json.load(open(os.path.join(p, "patches.json"))),
    }


def column_fields(d, g):
    """Per column: the Rock/Water/Soil class and the tallest roof over it, as World::from_bundle."""
    n = g["ratio"] * g["ratio"]
    klass, roof = [], []
    for y in range(d["y"]):
        for x in range(d["x"]):
            sealed = water = 0
            top = 0.0
            for gy in range(y * g["ratio"], (y + 1) * g["ratio"]):
                for gx in range(x * g["ratio"], (x + 1) * g["ratio"]):
                    i = gx + g["width"] * gy
                    m = g["media"][g["medium"][i]]
                    sealed += m in SEALED
                    water += m == "water"
                    top = max(top, g["building_h"][i])
            klass.append("rock" if 2 * sealed > n else "water" if 2 * water > n else "soil")
            roof.append(top)
    return klass, roof


def building_shade(d, heights, roof, slope=1.0):
    """world::building_shade: the lowest z a roof does not darken, per column."""
    shade = [0] * (d["x"] * d["y"])
    top_z = d["z"] - 1
    for y in range(d["y"]):
        for x in range(d["x"]):
            bh = roof[x + d["x"] * y]
            if bh <= 0.0:
                continue
            t = heights[x + d["x"] * y] + bh
            reach = min(int(bh * slope), d["y"])
            for dy in range(reach + 1):
                if y + dy >= d["y"]:
                    break
                blocked = max(0, min(top_z, int(t - dy / slope) + 1))
                i = x + d["x"] * (y + dy)
                shade[i] = max(shade[i], blocked)
    return shade


def canopy_cover(d, trees, heights):
    """Columns with a canopy voxel over them: 3x3 under a mature tree, its own column under a young one."""
    cover = bytearray(d["x"] * d["y"])
    for t in trees:
        if t["stage"] == "sapling":
            continue
        r = 1 if t["stage"] == "mature" else 0
        for yy in range(t["y"] - r, t["y"] + r + 1):
            for xx in range(t["x"] - r, t["x"] + r + 1):
                if 0 <= xx < d["x"] and 0 <= yy < d["y"]:
                    cover[xx + d["x"] * yy] = 1
    return cover


def write_png(path, w, h, pix):
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


# The ground media, muted, so the vegetation drawn over them reads as the brighter layer.
MEDIUM_RGB = {
    "soil": (92, 74, 58),
    "lawn": (86, 92, 62),
    "bed": (98, 80, 62),
    "mulch": (74, 58, 46),
    "gravel": (120, 116, 108),
    "concrete": (150, 148, 144),
    "asphalt": (78, 78, 82),
    "roof": (112, 96, 92),
    "water": (56, 82, 110),
}
TRUNK_RGB = {"sapling": (170, 150, 90), "young": (140, 100, 50), "mature": (60, 34, 18)}


def vegetation_png(path, d, g, sn, cover):
    """Vegetation over the ground media: one pixel per ground cell, north up."""
    gw, gh, r = g["width"], g["depth"], g["ratio"]
    pix = bytearray(gw * gh * 3)
    px_n = d["x"] // d["patch"]
    trunks = {(t["x"], t["y"]): t["stage"] for t in sn["entities"] if t["kind"] == "tree"}
    for y in range(d["y"]):
        for x in range(d["x"]):
            c = x + d["x"] * y
            p = sn["patches"][(x // d["patch"]) + px_n * (y // d["patch"])]
            grass, shrub = p["grass"], p["shrub"]
            for gy in range(y * r, (y + 1) * r):
                for gx in range(x * r, (x + 1) * r):
                    i = gx + gw * gy
                    m = g["media"][g["medium"][i]]
                    red, grn, blu = MEDIUM_RGB[m]
                    if m not in SEALED:
                        # Grass greens the column, shrub darkens it, canopy shades it further.
                        red = int(red * (1 - 0.6 * grass) + 60 * grass)
                        grn = int(grn * (1 - 0.6 * grass) + 190 * grass)
                        blu = int(blu * (1 - 0.6 * grass) + 60 * grass)
                        red = int(red * (1 - 0.7 * shrub) + 46 * shrub)
                        grn = int(grn * (1 - 0.7 * shrub) + 104 * shrub)
                        blu = int(blu * (1 - 0.7 * shrub) + 52 * shrub)
                    if cover[c]:
                        red, grn, blu = int(red * 0.45 + 18), int(grn * 0.45 + 74), int(blu * 0.45 + 22)
                    # North up: PNG row 0 is the highest y.
                    o = (gx + gw * (gh - 1 - gy)) * 3
                    pix[o], pix[o + 1], pix[o + 2] = red, grn, blu
            if (x, y) in trunks:
                col = TRUNK_RGB[trunks[(x, y)]]
                for gy in range(y * r, (y + 1) * r):
                    for gx in range(x * r, (x + 1) * r):
                        o = (gx + gw * (gh - 1 - gy)) * 3
                        pix[o], pix[o + 1], pix[o + 2] = col
    write_png(path, gw, gh, pix)
    return len(trunks)


def main():
    run, out = sys.argv[1], sys.argv[2]
    meta, d, g = load(run)
    cols = d["x"] * d["y"]
    ticks = [0, 10000, meta["ticks"]]
    snaps = {t: snap(run, t) for t in ticks}
    klass, roof = column_fields(d, g)
    heights = list(snaps[0]["height"])
    shade = building_shade(d, heights, roof, 1.0)
    shaded = [shade[c] > heights[c] + 1 for c in range(cols)]
    imported = {t["id"] for t in snaps[0]["entities"] if t["kind"] == "tree"}
    plantable = sum(k == "soil" for k in klass)

    print("## Event-log cause breakdown\n")
    rows = Counter()
    for line in open(os.path.join(run, "events.csv")).read().splitlines()[1:]:
        f = line.split(",")
        rows[(f[2] or "-", f[1], f[7] or "-")] += 1
    print("| species | kind | cause | events |")
    print("| --- | --- | --- | --- |")
    for (sp, kind, cause), n in sorted(rows.items(), key=lambda kv: (kv[0][0], -kv[1])):
        print("| %s | %s | %s | %d |" % (sp, kind, cause, n))

    print("\n## Trees\n")
    print("| tick | total | imported alive | germinated alive | mature | young | sapling | canopy cols | canopy %% of %d plantable |" % plantable)
    print("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    covers = {}
    for t in ticks:
        trees = [e for e in snaps[t]["entities"] if e["kind"] == "tree"]
        imp = sum(e["id"] in imported for e in trees)
        st = Counter(e["stage"] for e in trees)
        covers[t] = canopy_cover(d, trees, heights)
        cov = sum(covers[t])
        print(
            "| %d | %d | %d | %d | %s | %d | %.1f%% |"
            % (
                t,
                len(trees),
                imp,
                len(trees) - imp,
                " | ".join(str(st[s]) for s in reversed(STAGES)),
                cov,
                100.0 * cov / plantable,
            )
        )

    print("\n## Ground\n")
    kc = Counter(klass)
    for k in ("rock", "water", "soil"):
        print("- %s columns: %d of %d (%.1f%%)" % (k, kc[k], cols, 100.0 * kc[k] / cols))
    mc = Counter(g["media"][b] for b in g["medium"])
    gcells = g["width"] * g["depth"]
    print("- ground cells by medium: " + ", ".join("%s %d (%.1f%%)" % (m, n, 100.0 * n / gcells) for m, n in mc.most_common()))
    for t in ticks:
        ps = snaps[t]["patches"]
        print(
            "- tick %d: mean grass %.3f, mean shrub %.3f, mean detritus %.1f"
            % (
                t,
                sum(p["grass"] for p in ps) / len(ps),
                sum(p["shrub"] for p in ps) / len(ps),
                sum(p["detritus"] for p in ps) / len(ps),
            )
        )

    print("\n## Building shade\n")
    ns = sum(shaded)
    print("- shaded columns (the air voxel over the surface is dark): %d of %d (%.1f%%)" % (ns, cols, 100.0 * ns / cols))
    sp = sum(shaded[c] and klass[c] == "soil" for c in range(cols))
    print("- of those, plantable: %d (%.1f%% of the %d plantable columns)" % (sp, 100.0 * sp / plantable, plantable))
    for t in ticks:
        trees = [e for e in snaps[t]["entities"] if e["kind"] == "tree"]
        ins = [e for e in trees if shaded[e["x"] + d["x"] * e["y"]]]
        imp_in = sum(e["id"] in imported for e in ins)
        print("- tick %d: %d trees stand in shade (%d of them imported), %d in the open" % (t, len(ins), imp_in, len(trees) - len(ins)))
    alive = {e["id"] for e in snaps[ticks[-1]]["entities"]}
    for name, keep in (("shade", True), ("open", False)):
        group = [e for e in snaps[0]["entities"] if shaded[e["x"] + d["x"] * e["y"]] == keep]
        if group:
            n = sum(e["id"] in alive for e in group)
            print("- imported trees planted in the %s: %d, still alive at %d: %d" % (name, len(group), ticks[-1], n))
    # Cross-check the shade above against the run's own light field: at tick 0 a column is dark at
    # the air voxel over its surface only where a roof darkens it or three canopy voxels stack there.
    light = open(os.path.join(run, "snap_000000", "light.bin"), "rb").read()
    dark = [light[c + cols * (heights[c] + 1)] == 0 for c in range(cols)]
    print("- cross-check against snap_000000/light.bin: %d dark surface columns, %d of them outside the computed shade" % (sum(dark), sum(dark[c] and not shaded[c] for c in range(cols))))

    print("\n## The imported trees over time\n")
    print("| tick | imported alive | germinated alive |")
    print("| --- | --- | --- |")
    prev = None
    for t in meta["snapshots"]:
        trees = json.load(open(os.path.join(run, "snap_%06d" % t, "entities.json")))
        imp = sum(e["id"] in imported for e in trees)
        if t % 1000 == 0 or imp == 0:
            print("| %d | %d | %d |" % (t, imp, len(trees) - imp))
        if imp == 0:
            print("\nThe last imported tree dies between ticks %d and %d." % (prev, t))
            break
        prev = t

    png = os.path.join(out, "vegetation-%d.png" % ticks[-1])
    n = vegetation_png(png, d, g, snaps[ticks[-1]], covers[ticks[-1]])
    print("\nwrote %s: %dx%d px, %d trunks, north up" % (png, g["width"], g["depth"], n))


main()
