"""Shot G4 findings: where the water goes.

Run from `ecosim/` with the release binary built:

    python sweeps/shotG4/analyse.py

It writes nothing outside `target/g4/` and `sweeps/shotG4/`. The four Capitol runs
(`target/g4/cap5`, `cap10`, `cap20`, `cap40`) and the four sweeps under `sweeps/shotG4/`
must exist already; the rest of the runs it makes itself.
"""

import json
import os
import struct
import subprocess
import sys
import zlib
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target/release/ecosim"
OUT = ROOT / "target/g4"
HERE = Path(__file__).resolve().parent
SIZES = [5, 10, 20, 40]
# rain.storm_p paired with each mean so that p x mean is 1 mm per tick in every column.
P_OF = {5: 0.2, 10: 0.1, 20: 0.05, 40: 0.025}
TICK_H = 8766 / 4000


def run(out, args, ticks=20000, every=10000, seed=42):
    """`ecosim run` into `target/g4/<out>`, skipped when the directory is already there."""
    d = OUT / out
    if d.exists():
        return d
    cmd = [str(EXE), "run", "--seed", str(seed), "--ticks", str(ticks), "--snapshot-every", str(every)]
    cmd += ["--out", str(d)] + args
    subprocess.run(cmd, check=True, cwd=ROOT, stdout=subprocess.DEVNULL)
    return d


def series(d):
    """A run's series.csv as a dict of column name -> float array."""
    rows = np.genfromtxt(d / "series.csv", delimiter=",", names=True)
    return {n: rows[n] for n in rows.dtype.names}


def snap(d, tick):
    return d / f"snap_{tick:06d}"


def meta(d):
    return json.loads((d / "meta.json").read_text())


def media_of(d):
    """The ground medium grid of a run, as (names, codes array of shape (depth, width))."""
    m = meta(d)
    w, h = m["world"]["ground_width"], m["world"]["ground_depth"]
    codes = np.fromfile(d / "world/medium.bin", dtype=np.uint8).reshape(h, w)
    return m["world"]["media"], codes


def flat_bundle(dir, medium, size_m=32, cell=0.5, roof=False):
    """A flat single-medium world bundle, written from scratch: the simplest world in which a
    storm's runoff fraction is the medium's own, with no run-on from anywhere uphill."""
    dir.mkdir(parents=True, exist_ok=True)
    n = int(size_m / cell)
    media = ["soil", "lawn", "bed", "mulch", "gravel", "concrete", "asphalt", "roof", "water"]
    bundle = {
        "format": "ecosim-world-bundle",
        "version": 2,
        "name": f"flat_{medium}",
        "size_m": float(size_m),
        "ground_cell_m": cell,
        "ground_width": n,
        "ground_depth": n,
        "media": media,
        "source": "synthetic, written by sweeps/shotG4/analyse.py",
        "counts": {"trees": 0, "shrubs": 0, "pipes": 0},
    }
    (dir / "bundle.json").write_text(json.dumps(bundle, indent=2))
    np.zeros(n * n, dtype=np.float32).tofile(dir / "ground_h.f32")
    b = np.full(n * n, 4.0 if roof else 0.0, dtype=np.float32)
    b.tofile(dir / "building_h.f32")
    np.full(n * n, media.index(medium), dtype=np.uint8).tofile(dir / "medium.u8")
    for f in ("trees.json", "shrubs.json", "pipes.json"):
        (dir / f).write_text("[]")
    return dir


def runoff_by_medium():
    """Measured: the runoff fraction of one storm on a flat world of a single medium, at each
    storm size, over a full-length run. `runoff_mm / rain_mm` is exact here because a flat world
    gives every cell the same rain and no run-on."""
    rows = []
    for name, medium, roof in [("lawn", "lawn", False), ("street", "asphalt", False), ("roof", "roof", True)]:
        b = flat_bundle(OUT / f"bundle_{name}", medium, roof=roof)
        for m in SIZES:
            d = run(
                f"flat_{name}_{m}",
                ["--world", str(b), "--set", "animals.enabled=false", "--set", "climate.rain_gradient=0",
                 "--set", f"rain.storm_mean_mm={m}", "--set", f"rain.storm_p={P_OF[m]}"],
                every=20000,
            )
            s = series(d)
            wet = s["rain_mm"] > 0
            frac = s["runoff_mm"][wet].sum() / s["rain_mm"][wet].sum()
            big = s["rain_mm"][wet] > m
            rows.append((name, m, frac, s["runoff_mm"][wet][big].sum() / s["rain_mm"][wet][big].sum(),
                         s["soil_water_mm"][-1]))
    return rows


def largest_storm(d):
    s = series(d)
    i = int(np.argmax(s["rain_mm"]))
    return int(s["tick"][i]), float(s["rain_mm"][i])


def ponding(d, tick):
    """The ponded grid in mm at a snapshot, and the top depressions by volume.

    A depression is a connected run of ponded cells (4-neighbour flood fill), which is what the
    priority-flood fill leaves behind: the cells whose filled surface is above the real one."""
    names, codes = media_of(d)
    m = meta(d)
    cell = m["world"]["ground_cell_m"]
    w, h = m["world"]["ground_width"], m["world"]["ground_depth"]
    pond = np.fromfile(snap(d, tick) / "water.bin", dtype="<u2").reshape(h, w).astype(np.float64) / 10.0
    wet = pond > 0.5
    seen = np.zeros_like(wet)
    groups = []
    for sy in range(h):
        for sx in range(w):
            if not wet[sy, sx] or seen[sy, sx]:
                continue
            stack, cells = [(sy, sx)], []
            seen[sy, sx] = True
            while stack:
                y, x = stack.pop()
                cells.append((y, x))
                for ny, nx in ((y - 1, x), (y + 1, x), (y, x - 1), (y, x + 1)):
                    if 0 <= ny < h and 0 <= nx < w and wet[ny, nx] and not seen[ny, nx]:
                        seen[ny, nx] = True
                        stack.append((ny, nx))
            vol = sum(pond[y, x] for y, x in cells) / 1000.0 * cell * cell  # m3
            deep = max(cells, key=lambda c: pond[c])
            top = {}
            for y, x in cells:
                top[names[codes[y, x]]] = top.get(names[codes[y, x]], 0) + 1
            groups.append((vol, len(cells), pond[deep], (deep[1] * cell, deep[0] * cell),
                           max(top, key=top.get)))
    groups.sort(reverse=True)
    return pond, groups


def png(path, rgb):
    """A PNG from an (h, w, 3) uint8 array, without an image library."""
    h, w, _ = rgb.shape
    raw = b"".join(b"\x00" + rgb[y].tobytes() for y in range(h))

    def chunk(kind, data):
        c = kind + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c))

    head = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", head) + chunk(b"IDAT", zlib.compress(raw, 9))
                     + chunk(b"IEND", b""))


def pond_png(d, tick, path):
    """Top-down: the ground in grey by medium, ponded water in blue by depth (north is up)."""
    names, codes = media_of(d)
    pond, _ = ponding(d, tick)
    h, w = codes.shape
    grey = {"roof": 90, "concrete": 150, "asphalt": 70, "water": 40, "gravel": 170}
    base = np.full((h, w), 120, dtype=np.uint8)
    for i, n in enumerate(names):
        base[codes == i] = grey.get(n, 120)
    rgb = np.dstack([base, base, base])
    deep = np.clip(pond / 50.0, 0, 1)  # 50 mm is full blue
    wet = pond > 0.5
    rgb[..., 0] = np.where(wet, (40 * (1 - deep)).astype(np.uint8), rgb[..., 0])
    rgb[..., 1] = np.where(wet, (140 - 100 * deep).astype(np.uint8), rgb[..., 1])
    rgb[..., 2] = np.where(wet, (140 + 115 * deep).astype(np.uint8), rgb[..., 2])
    png(path, rgb[::-1])  # row 0 is y = 0, the south edge


def lawn_quarters(d, tick):
    """Grass and trees in the wettest and driest quarters of the lawn.

    Patches whose columns are mostly lawn, sorted by the mean soil water of those columns at
    `tick`, then split into quarters. Grass cover is the patch's own; trees are counted from
    their positions."""
    m = meta(d)
    dims, patch = m["dims"], m["dims"]["patch"]
    wx, wy = dims["x"], dims["y"]
    names, codes = media_of(d)
    cell = m["world"]["ground_cell_m"]
    per = int(round(1.0 / cell))  # ground cells across one 1 m column
    lawn = (codes == names.index("lawn")).reshape(wy, per, wx, per).mean(axis=(1, 3)) > 0.5
    soil = np.fromfile(snap(d, tick) / "soil_water.bin", dtype="<f4").reshape(wy, wx)
    patches = json.loads((snap(d, tick) / "patches.json").read_text())
    ents = json.loads((snap(d, tick) / "entities.json").read_text())
    px = wx // patch
    rows = []
    for i, p in enumerate(patches):
        y0, x0 = (i // px) * patch, (i % px) * patch
        mask = lawn[y0:y0 + patch, x0:x0 + patch]
        if mask.mean() < 0.5:
            continue
        rows.append((float(soil[y0:y0 + patch, x0:x0 + patch][mask].mean()), i, p["grass"], p["shrub"]))
    rows.sort()
    trees = [t for t in ents if t["kind"] == "tree"]
    tree_patch = {}
    for t in trees:
        pi = (int(t["y"]) // patch) * px + (int(t["x"]) // patch)
        tree_patch[pi] = tree_patch.get(pi, 0) + 1
    q = max(1, len(rows) // 4)
    out = []
    for label, part in (("driest quarter", rows[:q]), ("wettest quarter", rows[-q:])):
        out.append((label, len(part), np.mean([r[0] for r in part]), np.mean([r[2] for r in part]),
                    np.mean([r[3] for r in part]), sum(tree_patch.get(r[1], 0) for r in part)))
    return out, len(rows)


def storm_cost():
    """The cost of one storm pass at 512 x 512: the wall time of a Capitol run with a storm every
    tick, less the same run with no storms at all, over the number of ticks."""
    ticks = 2000
    base = ["--world", str(ROOT / "worlds/capitol"), "--set", "animals.enabled=false",
            "--set", "climate.rain_gradient=0"]
    times = {}
    for name, p in (("wet", 1.0), ("dry", 0.0)):
        d = run(f"cost_{name}", base + ["--set", f"rain.storm_p={p}"], ticks=ticks, every=ticks)
        times[name] = json.loads((d / "timing.json").read_text())["wall_ms"]
    return times, (times["wet"] - times["dry"]) / ticks


def main():
    report = []
    w = report.append
    w("## Measured tables\n")

    w("### Runoff fraction by surface, flat single-medium worlds, 20000 ticks\n")
    w("| surface | storm mean (mm) | runoff fraction | of storms above the mean | soil water at the end (mm) |")
    w("|---|---|---|---|---|")
    for name, m, frac, big, soil in runoff_by_medium():
        w(f"| {name} | {m} | {frac:.3f} | {big:.3f} | {soil:.1f} |")
    w("")

    w("### The Capitol, 20000 ticks, seed 42\n")
    w("| storm mean (mm) | storms | rain (mm) | runoff | outflow | drainage (mm) | largest storm |")
    w("|---|---|---|---|---|---|---|")
    for m in SIZES:
        d = OUT / f"cap{m}"
        s = series(d)
        rain = s["rain_mm"].sum()
        tick, depth = largest_storm(d)
        w(f"| {m} | {int((s['rain_mm'] > 0).sum())} | {rain:.0f} | {100 * s['runoff_mm'].sum() / rain:.1f}% "
          f"| {100 * s['outflow_mm'].sum() / rain:.1f}% | {s['drainage_mm'].sum():.0f} | {depth:.1f} mm @ {tick} |")
    w("")

    big = OUT / "cap40_storm"
    tick, depth = largest_storm(OUT / "cap40")
    run("cap40_storm", ["--world", str(ROOT / "worlds/capitol"), "--set", "animals.enabled=false",
                        "--set", "climate.rain_gradient=0", "--set", "rain.storm_mean_mm=40",
                        "--set", "rain.storm_p=0.025"], ticks=tick, every=tick)
    pond, groups = ponding(big, tick)
    w(f"### Ponding after the largest storm ({depth:.1f} mm at tick {tick}, `rain.storm_mean_mm=40`)\n")
    w(f"Ponded volume over the whole 256 m site: {pond.sum() / 1000 * 0.25:.1f} m3 "
      f"in {int((pond > 0.5).sum())} of {pond.size} ground cells.\n")
    w("| rank | volume (m3) | cells | deepest (mm) | at (m) | medium |")
    w("|---|---|---|---|---|---|")
    for i, (vol, n, deep, (x, y), med) in enumerate(groups[:5], 1):
        w(f"| {i} | {vol:.2f} | {n} | {deep:.0f} | ({x:.1f}, {y:.1f}) | {med} |")
    w("")
    pond_png(big, tick, HERE / "ponding.png")

    rows, n = lawn_quarters(OUT / "cap10", 20000)
    w(f"### Lawn cover by soil water, the default storm size, tick 20000 ({n} lawn patches)\n")
    w("| quarter | patches | soil water (mm) | grass | shrub | trees |")
    w("|---|---|---|---|---|---|")
    for label, k, soil, grass, shrub, trees in rows:
        w(f"| {label} | {k} | {soil:.1f} | {grass:.3f} | {shrub:.3f} | {trees} |")
    w("")

    times, per = storm_cost()
    w("### Cost of the storm pass at 512 x 512\n")
    w(f"2000 ticks of the Capitol with a storm every tick: {times['wet']} ms; with no storms: "
      f"{times['dry']} ms. One storm pass over 262144 ground cells costs **{per:.2f} ms**.\n")

    text = "\n".join(report)
    (HERE / "tables.md").write_text(text)
    print(text)


if __name__ == "__main__":
    sys.exit(main())
