"""Collapse atlas (shot 14): outcome grids for the two-parameter sweeps in sweeps/atlas/.

usage: python sweeps/atlas/atlas.py grid <grid>      markdown grid, regions and flips for one sweep
       python sweeps/atlas/atlas.py flips <grid>     one line per flipping cell: `k1=v1 k2=v2`
       python sweeps/atlas/atlas.py noise <grid>     stream-replicate table for the flipping cells

<grid> is a sweep directory under sweeps/atlas/ (rain_x_kill, energy_x_fire, crowd_x_disease). The
per-cell series (cells/*.csv, and flips/<cell>/cells/*.csv for the stream reruns) are gitignored;
rerun the commands in _log.txt to regenerate them.

Trees are not part of the class; a † marks a cell in which trees reach 0 on some seed.

A cell-seed's outcome comes from its series: a species has collapsed when its count reaches 0 at
any tick (the `first_extinction` rule). persist = neither animal species reaches 0; grazer / hunter
= only that one does; both = both do. A collapse's cause is the dominant death cause over the 500
ticks ending at its extinction tick, ties to the first cause in `Cause` order (as `ecosim stats`).
"""
import csv
import os
import sys
from collections import Counter

ROOT = os.path.dirname(os.path.abspath(__file__))
CAUSES = ["starved", "eaten", "old_age", "crowded", "burnt"]
GLYPH = {"persist": "🟩", "grazer": "🟨", "hunter": "🟥", "both": "⬛"}
SHORT = {"starved": "starv", "eaten": "eaten", "old_age": "old", "crowded": "crowd", "burnt": "burnt"}


def series(path):
    with open(path, newline="") as f:
        return list(csv.DictReader(f))


def collapse(rows, sp):
    """(extinction tick, dominant cause) for `sp`, or None if it never reaches 0."""
    for i, r in enumerate(rows):
        if int(r[sp + "s"]) == 0:
            window = rows[max(0, i - 499): i + 1]
            n = [sum(int(w[f"{sp}_{c}"]) for w in window) for c in CAUSES]
            cause = CAUSES[n.index(max(n))] if max(n) > 0 else "none"
            return int(r["tick"]), cause
    return None


def outcome(path):
    """(class, first species, its tick, its cause, trees reach 0) for one cell-seed series."""
    rows = series(path)
    g, h = collapse(rows, "grazer"), collapse(rows, "hunter")
    trees_out = any(int(r["trees"]) == 0 for r in rows)
    if g is None and h is None:
        return "persist", None, None, None, trees_out
    cls = "both" if g and h else ("grazer" if g else "hunter")
    first = min((x for x in [("grazer",) + g if g else None, ("hunter",) + h if h else None] if x),
                key=lambda x: (x[1], x[0] != "grazer"))
    return cls, first[0], first[1], first[2], trees_out


def load(grid):
    """keys, values per key, and {(v1, v2): {seed: outcome}} in sweep.csv order."""
    d = os.path.join(ROOT, grid)
    with open(os.path.join(d, "sweep.csv"), newline="") as f:
        rows = list(csv.reader(f))
    k1, k2 = rows[0][0], rows[0][1]
    cells, v1s, v2s = {}, [], []
    for r in rows[1:]:
        v1, v2, seed = r[0], r[1], r[2]
        v1s += [v1] if v1 not in v1s else []
        v2s += [v2] if v2 not in v2s else []
        path = os.path.join(d, "cells", f"{k1}={v1}_{k2}={v2}_s={seed}.csv")
        cells.setdefault((v1, v2), {})[seed] = outcome(path)
    return k1, k2, v1s, v2s, cells


def cause_label(seeds):
    """The most common (first species, cause) among the collapsed seeds, e.g. `H starv`."""
    c = Counter((o[1], o[3]) for o in seeds.values() if o[1])
    if not c:
        return ""
    (sp, cause), _ = max(c.items(), key=lambda kv: (kv[1], kv[0]))
    return f"{sp[0].upper()} {SHORT.get(cause, cause)}"


def flipping(cells):
    return [k for k, s in cells.items() if len({o[0] for o in s.values()}) > 1]


def regions(v1s, v2s, cells, cls):
    """4-connected components of cells whose seeds are all `cls`, as sizes."""
    todo = {(i, j) for i, a in enumerate(v1s) for j, b in enumerate(v2s)
            if all(o[0] == cls for o in cells[(a, b)].values())}
    sizes = []
    while todo:
        stack, n = [todo.pop()], 0
        while stack:
            i, j = stack.pop()
            n += 1
            for q in [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)]:
                if q in todo:
                    todo.remove(q)
                    stack.append(q)
        sizes.append(n)
    return sorted(sizes, reverse=True)


def grid_md(grid):
    k1, k2, v1s, v2s, cells = load(grid)
    out = [f"`{k1}` (rows) × `{k2}` (columns). Each cell shows seeds 1 / 2 / 3, then the most common "
           "first collapse and its cause (G = grazers, H = hunters).", "",
           f"| {k1} \\ {k2} | " + " | ".join(v2s) + " |", "|---|" + "---|" * len(v2s)]
    for a in v1s:
        line = []
        for b in v2s:
            s = cells[(a, b)]
            glyphs = "".join(GLYPH[s[k][0]] for k in sorted(s))
            trees = "†" if any(o[4] for o in s.values()) else ""
            line.append(f"{glyphs}{trees} {cause_label(s)}".strip())
        out.append(f"| **{a}** | " + " | ".join(line) + " |")
    total = Counter(o[0] for s in cells.values() for o in s.values())
    out += ["", "Cell-seeds by outcome: " + ", ".join(f"{GLYPH[c]} {c} {total[c]}" for c in GLYPH) + "."]
    trees = [f"{a}/{b} s{k}" for (a, b), s in cells.items() for k, o in sorted(s.items()) if o[4]]
    if trees:
        out.append(f"† Trees reach 0 in {len(trees)} cell-seeds: " + ", ".join(trees) + ".")
    causes = Counter((o[1], o[3]) for s in cells.values() for o in s.values() if o[1])
    if causes:
        out.append("First collapses by cause: " + ", ".join(
            f"{sp} {c} {n}" for (sp, c), n in sorted(causes.items(), key=lambda kv: -kv[1])) + ".")
    out.append("Regions (4-connected cells where all three seeds agree), sizes: " + "; ".join(
        f"{c} {regions(v1s, v2s, cells, c) or '-'}" for c in GLYPH) + ".")
    flips = flipping(cells)
    out.append(f"Flipping cells (seeds disagree): {len(flips)} of {len(cells)}.")
    return "\n".join(out)


def noise_md(grid):
    k1, k2, v1s, v2s, cells = load(grid)
    out = ["| cell | seed 1 streams 0–5 | seed 2 streams 0–5 | seed 3 streams 0–5 | verdict |",
           "|---|---|---|---|---|"]
    counts = Counter()
    for a, b in flipping(cells):
        d = os.path.join(ROOT, grid, "flips", f"{k1}={a}_{k2}={b}", "cells")
        per_seed, noisy = [], False
        for seed in sorted(cells[(a, b)]):
            outs = [cells[(a, b)][seed][0]] + [
                outcome(os.path.join(d, f"rng.stream={n}_s={seed}.csv"))[0] for n in range(1, 6)]
            noisy |= len(set(outs)) > 1
            per_seed.append("".join(GLYPH[o] for o in outs))
        verdict = "noise" if noisy else "seed"
        counts[verdict] += 1
        out.append(f"| {k1}={a}, {k2}={b} | " + " | ".join(per_seed) + f" | {verdict} |")
    out += ["", f"Flips: {sum(counts.values())}; noise {counts['noise']}, seed-determined {counts['seed']}."]
    return "\n".join(out)


if __name__ == "__main__":
    mode, grid = sys.argv[1], sys.argv[2]
    if mode == "grid":
        print(grid_md(grid))
    elif mode == "flips":
        k1, k2, _, _, cells = load(grid)
        for a, b in flipping(cells):
            print(f"{k1}={a} {k2}={b}")
    elif mode == "noise":
        print(noise_md(grid))
