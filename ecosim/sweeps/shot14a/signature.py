"""Findings tables for shot 14a.

usage: python sweeps/shot14a/signature.py sweeps/shot14a/<sweep> [--wide]

Per grid cell: each seed's check verdict (P/F) and signature, pp_lag/pp_corr from sweep.csv, or
the extinct species and dominant cause when the signature is undefined. A cell is "in the target
region" when every seed has pp_lag > 0 and pp_corr > 0.3.

--wide also recomputes the lagged correlation from the gitignored per-cell series (rerun the sweep
to regenerate them) over lags -6000..6000 step 50, same window and rule as `ecosim stats
--signature`, to show where the maximum lies when it is at the +-2000 edge of the official range.
"""
import csv
import os
import sys
from collections import OrderedDict


def pearson(a, b):
    n = len(a)
    ma, mb = sum(a) / n, sum(b) / n
    sab = saa = sbb = 0.0
    for x, y in zip(a, b):
        dx, dy = x - ma, y - mb
        sab += dx * dy
        saa += dx * dx
        sbb += dy * dy
    return sab / (saa * sbb) ** 0.5 if saa > 0 and sbb > 0 else None


def wide(path):
    rows = list(csv.DictReader(open(path)))[2000:20001]
    g = [float(r["grazers"]) for r in rows]
    h = [float(r["hunters"]) for r in rows]
    if min(g) == 0 or min(h) == 0:
        return None
    best = None
    for lag in range(-6000, 6001, 50):
        s = abs(lag)
        n = len(g) - s
        a, b = (g[:n], h[s:]) if lag >= 0 else (g[s:], h[:n])
        c = pearson(a, b)
        if c is not None and (best is None or c > best[1]):
            best = (lag, c)
    return best


def main():
    d = sys.argv[1]
    rows = list(csv.DictReader(open(os.path.join(d, "sweep.csv"))))
    keys = [k for k in rows[0] if "." in k and not k.endswith(("_pass", "_value", "_margin"))]
    cells = OrderedDict()
    for r in rows:
        cells.setdefault(tuple(r[k] for k in keys), []).append(r)
    target = []
    print("| " + " | ".join(keys) + " | " + " | ".join(f"s{r['seed']}" for r in next(iter(cells.values()))) + " |")
    print("|" + "---|" * (len(keys) + len(next(iter(cells.values())))))
    for c, rs in cells.items():
        out, hit = [], True
        for r in rs:
            ok = all(r[k] == "true" for k in r if k.endswith("_pass") and r[k])
            if r["pp_lag"]:
                lag, corr = int(r["pp_lag"]), float(r["pp_corr"])
                hit &= lag > 0 and corr > 0.3
                sig = f"{lag} / {corr:.2f}"
                if "--wide" in sys.argv:
                    cid = "_".join(f"{k}={v}" for k, v in zip(keys, c)) + f"_s={r['seed']}.csv"
                    w = wide(os.path.join(d, "cells", cid))
                    sig += f" (wide {w[0]} / {w[1]:.2f})" if w else ""
            else:
                hit = False
                sig = r["pp_undefined"].replace(" ", " extinct, ")
            out.append(f"{'P' if ok else 'F'} {sig}")
        if hit:
            target.append(c)
        print("| " + " | ".join(c) + " | " + " | ".join(out) + " |")
    print()
    print("target region (pp_lag > 0 and pp_corr > 0.3 on every seed):", target or "none")


main()
