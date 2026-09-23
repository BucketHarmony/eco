#!/usr/bin/env python3
"""Shot G9's report on the moving sun: the numbers in FINDINGS.md and sun-budget.png.

Reads the G9 Capitol reference run and, for comparison, the same command run by the binary of the
commit before G9 (the fixed 45-degree sun). Needs numpy and Pillow; nothing in CI runs it.

    python sweeps/shotG9/report.py runs/capitol-s42 <pre-G9 run dir> sweeps/shotG9
"""

import json
import os
import sys
from collections import Counter

import numpy as np
from PIL import Image, ImageDraw

AIR, SOIL = 0, 1
G3A = {0: (79, 711), 10000: (602, 2173), 20000: (1982, 8821)}  # sweeps/capitolG3-flat/FINDINGS.md
SEASONS = ["spring equinox", "summer solstice", "autumn equinox", "winter solstice"]


def load(run):
    meta = json.load(open(os.path.join(run, "meta.json")))
    d = meta["dims"]
    return meta, d["x"], d["y"], d["z"]


def surface(run, wx, wy, wz, tick=0):
    p = os.path.join(run, "snap_%06d" % tick)
    mat = np.fromfile(os.path.join(p, "material.bin"), np.uint8).reshape(wz, wy, wx)
    light = np.fromfile(os.path.join(p, "light.bin"), np.uint8).reshape(wz, wy, wx)
    h = np.fromfile(os.path.join(p, "height.bin"), np.uint8).reshape(wy, wx).astype(int)
    yy, xx = np.mgrid[0:wy, 0:wx]
    top = mat[h, yy, xx]
    above = light[np.minimum(h + 1, wz - 1), yy, xx]
    return top == SOIL, above


def trees(run, tick):
    ents = json.load(open(os.path.join(run, "snap_%06d" % tick, "entities.json")))
    return [e for e in ents if e["kind"] == "tree"]


def canopy_cols(ts, wx, wy):
    cover = np.zeros((wy, wx), bool)
    for t in ts:
        if t["stage"] == "sapling":
            continue
        r = 1 if t["stage"] == "mature" else 0
        cover[max(0, t["y"] - r) : t["y"] + r + 1, max(0, t["x"] - r) : t["x"] + r + 1] = True
    return cover


def causes(run):
    c = Counter()
    for line in open(os.path.join(run, "events.csv")).read().splitlines()[1:]:
        f = line.split(",")
        if f[1] in ("tree_death", "germination"):
            c[(f[1], f[7] or "-")] += 1
    return c


def main():
    run, base, out = sys.argv[1:4]
    meta, wx, wy, wz = load(run)
    sun = meta["world"]["sun"]
    k, open_b = sun["slices"], sun["open"]
    budget = np.fromfile(os.path.join(run, "world", sun["file"]), np.uint8).reshape(k, wy, wx)
    frac = budget / open_b
    annual = frac.mean(axis=0)
    plantable, _ = surface(run, wx, wy, wz)
    need = meta["params"]["tree"]["sapling_light"]
    n_plant = int(plantable.sum())
    print("Sun: latitude %.5f, %d slices at ticks %s, open byte %d, sapling_light %.4f, %d plantable columns"
          % (sun["latitude_deg"], k, sun["tick_of_year"], open_b, need, n_plant))

    # The fixed sun's exclusion, read from the pre-G9 run's tick-0 light: a shaded column was 0.
    _, base_light = surface(base, wx, wy, wz)
    dark = int((plantable & (base_light == 0)).sum())
    print("\nFixed sun (pre-G9, tick 0): %d plantable columns at surface light 0 = %.2f%% of plantable"
          % (dark, 100 * dark / n_plant))

    print("\n| slice | tick of year | plantable below sapling_light | share | min | 5th pct | mean | shaded at all (<1) |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- |")
    for s in range(k):
        f = frac[s][plantable]
        below = int((f < need).sum())
        print("| %s | %d | %d | %.2f%% | %.3f | %.3f | %.4f | %.2f%% |" % (
            SEASONS[s] if k == 4 else s, sun["tick_of_year"][s], below, 100 * below / n_plant,
            f.min(), np.percentile(f, 5), f.mean(), 100 * (f < 1).mean()))
    fa = annual[plantable]
    below = int((fa < need).sum())
    print("| annual mean | - | %d | %.2f%% | %.3f | %.3f | %.4f | %.2f%% |" % (
        below, 100 * below / n_plant, fa.min(), np.percentile(fa, 5), fa.mean(), 100 * (fa < 1).mean()))
    ever = int((frac[:, plantable] < need).any(axis=0).sum())
    always = int((frac[:, plantable] < need).all(axis=0).sum())
    print("\nBelow sapling_light in at least one slice: %d (%.2f%%); in every slice: %d (%.2f%%)"
          % (ever, 100 * ever / n_plant, always, 100 * always / n_plant))
    # How the old hard exclusion fares now.
    was_dark = plantable & (base_light == 0)
    print("The %d formerly black columns now: annual mean factor min %.3f, mean %.3f, max %.3f; %d below sapling_light all year"
          % (dark, annual[was_dark].min(), annual[was_dark].mean(), annual[was_dark].max(),
             int((frac[:, was_dark] < need).all(axis=0).sum())))
    zero_open = int(((budget == 0) & plantable[None]).sum())
    print("Plantable column-slices at byte 0: %d" % zero_open)

    print("\n| tick | G9 trees | pre-G9 trees | G3a trees | G9 canopy cols | pre-G9 canopy cols | G3a canopy cols | G9 canopy % | pre-G9 canopy % |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for t in (0, 10000, 20000):
        a, b = trees(run, t), trees(base, t)
        ca, cb = canopy_cols(a, wx, wy).sum(), canopy_cols(b, wx, wy).sum()
        print("| %d | %d | %d | %d | %d | %d | %d | %.1f%% | %.1f%% |" % (
            t, len(a), len(b), G3A[t][0], ca, cb, G3A[t][1], 100 * ca / n_plant, 100 * cb / n_plant))
    for label, r in (("G9", run), ("pre-G9", base)):
        ts = trees(r, 20000)
        inshade = sum(1 for t in ts if annual[t["y"], t["x"]] < need)
        wasdark = sum(1 for t in ts if base_light[t["y"], t["x"]] == 0)
        st = Counter(t["stage"] for t in ts)
        print("%s at 20000: %d trees (%d mature, %d young, %d sapling); %d on columns below sapling_light over the year; %d on columns the fixed sun kept black"
              % (label, len(ts), st["mature"], st["young"], st["sapling"], inshade, wasdark))

    print("\n| event | cause | G9 | pre-G9 |")
    print("| --- | --- | --- | --- |")
    ca, cb = causes(run), causes(base)
    for key in sorted(set(ca) | set(cb)):
        print("| %s | %s | %d | %d |" % (key[0], key[1], ca[key], cb[key]))

    # The picture: the four slices and the annual mean, sealed ground marked.
    scale, pad, label_h = 2, 8, 18
    tiles = [(SEASONS[s] if k == 4 else "slice %d" % s, frac[s]) for s in range(k)] + [("annual mean", annual)]
    w = len(tiles) * (wx * scale + pad) + pad
    h = wy * scale + 2 * pad + label_h
    img = Image.new("RGB", (w, h), (24, 24, 24))
    draw = ImageDraw.Draw(img)
    for i, (name, f) in enumerate(tiles):
        g = (np.clip(f, 0, 1) * 235 + 20).astype(np.uint8)
        rgb = np.stack([g, g, (g * 0.85).astype(np.uint8)], axis=-1)
        rgb[~plantable & (budget[0] == 0)] = (150, 60, 60)   # under a roof
        low = plantable & (f < need)
        rgb[low] = (rgb[low] * np.array([0.55, 0.65, 1.0])).astype(np.uint8)  # below sapling_light: blue
        tile = Image.fromarray(rgb[::-1]).resize((wx * scale, wy * scale), Image.NEAREST)  # north up
        x0 = pad + i * (wx * scale + pad)
        img.paste(tile, (x0, pad + label_h))
        draw.text((x0, pad), name, fill=(230, 230, 230))
    img.save(os.path.join(out, "sun-budget.png"))
    print("\nwrote %s" % os.path.join(out, "sun-budget.png"))


if __name__ == "__main__":
    main()
