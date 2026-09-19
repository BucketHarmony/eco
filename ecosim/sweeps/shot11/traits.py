"""Trait means at tick 20000 against the species defaults, per cell of the heredity.mutation sweep.

Usage: python sweeps/shot11/traits.py sweeps/shot11/heredity_mutation > sweeps/shot11/heredity_mutation/traits.txt
Reads the gitignored per-cell series (cells/*.csv); rerun the logged sweep command to regenerate them.
"""
import csv
import sys
from pathlib import Path

DEFAULTS = {
    "grazer": {"energy_cost_mult": 1.0, "flee_distance": 4.0, "repro_threshold": 70.0},
    "hunter": {"energy_cost_mult": 1.0, "flee_distance": 4.0, "repro_threshold": 75.0},
}
TRAITS = ["energy_cost_mult", "flee_distance", "repro_threshold"]


def sign(x, eps):
    return "+" if x > eps else "-" if x < -eps else "0"


def main(sweep_dir):
    cells = sorted(Path(sweep_dir, "cells").glob("*.csv"))
    rows = {}
    for c in cells:
        value, seed = c.stem.split("=", 1)[1].split("_s=")
        with open(c, newline="") as f:
            last = list(csv.DictReader(f))[-1]
        rows[(value, int(seed))] = last
    print("Trait means at tick 20000, as a % change from the species default (sd in brackets, in trait units).")
    print("Direction: + / - when the change exceeds 1%, else 0. 'same' = all three seeds agree.\n")
    for species in ["grazer", "hunter"]:
        for t in TRAITS:
            d = DEFAULTS[species][t]
            print(f"## {species} {t} (default {d})")
            print(f"{'mutation':>8}  {'seed 1':>16}  {'seed 2':>16}  {'seed 3':>16}  direction")
            for value in sorted({v for v, _ in rows}):
                cols, dirs = [], []
                for s in (1, 2, 3):
                    r = rows[(value, s)]
                    m, sd = float(r[f"{species}_{t}_mean"]), float(r[f"{species}_{t}_sd"])
                    alive = int(r[species + "s"]) > 0
                    pct = 100.0 * (m - d) / d if alive else float("nan")
                    cols.append(f"{pct:+6.1f}% ({sd:6.3f})" if alive else "extinct")
                    dirs.append(sign(pct, 1.0) if alive else "x")
                agree = "same" if len(set(dirs)) == 1 else "mixed"
                print(f"{value:>8}  " + "  ".join(f"{c:>16}" for c in cols) + f"  {''.join(dirs)} {agree}")
            print()


if __name__ == "__main__":
    main(sys.argv[1])
