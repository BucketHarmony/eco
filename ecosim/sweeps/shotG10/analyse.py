"""Shot G10: read target/g10/ (sweep.sh) into scan.csv and print FINDINGS.md's tables."""
import csv, collections, os, sys

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.join(os.path.dirname(__file__), "..", "..", "target", "g10")
HERE = os.path.dirname(__file__)
YEAR = 4000


def series(d):
    with open(os.path.join(d, "series.csv")) as f:
        return list(csv.DictReader(f))


def causes(d):
    out = collections.defaultdict(collections.Counter)
    with open(os.path.join(d, "events.csv")) as f:
        for r in csv.DictReader(f):
            if r["kind"] in ("death", "tree_death"):
                out[r["species"]][r["cause"]] += 1
    return out


def fmt(c):
    return f"{sum(c.values())} — " + ", ".join(f"{k} {v}" for k, v in c.most_common()) if c else "0"


def extinct(rows, col):
    for r in rows:
        if int(r[col]) == 0 and int(r["tick"]) > 0:
            return int(r["tick"])
    return None


def row(name, d):
    s = series(d)
    last = s[-1]
    tail = s[-2 * YEAR:]  # the last two years
    # Soil water by season over the last two years: winter is the quarter centred on the coldest
    # point of the model's year (t = 3/4), summer the one centred on the warmest (t = 1/4).
    def season(centre):
        vals = [float(r["soil_water_mm"]) for r in tail
                if abs(((int(r["tick"]) % YEAR) / YEAR - centre + 0.5) % 1.0 - 0.5) < 0.125]
        return sum(vals) / len(vals)
    c = causes(d)
    return {
        "run": name,
        "trees_end": int(last["trees"]),
        "trees_mean_last2y": round(sum(int(r["trees"]) for r in tail) / len(tail), 1),
        "trees_min": min(int(r["trees"]) for r in s[1:]) if "trees" in s[0] else None,
        "grazers_end": int(last["grazers"]),
        "hunters_end": int(last["hunters"]),
        "soil_water_winter_mm": round(season(0.75), 2),
        "soil_water_summer_mm": round(season(0.25), 2),
        "drainage_mm_total": round(sum(float(r["drainage_mm"]) for r in s), 1),
        "tree_deaths": sum(c["tree"].values()),
        "tree_drought": c["tree"]["drought"],
        "tree_crowded": c["tree"]["crowded"],
        "tree_burnt": c["tree"]["burnt"],
        "tree_old_age": c["tree"]["old_age"],
        "tree_extinct": extinct(s, "trees"),
        # Animals are off on the Capitol, so "extinct at tick 1" there is not an extinction.
        "grazer_extinct": None if name.startswith("cap") else extinct(s, "grazers"),
        "hunter_extinct": None if name.startswith("cap") else extinct(s, "hunters"),
    }, c


rows, cause_rows = [], []
for v in ["0", "0.25", "0.5", "0.75", "1"]:
    for sd in [1, 2, 3]:
        r, c = row(f"d{v}-s{sd}", os.path.join(ROOT, f"d{v}-s{sd}"))
        rows.append(r)
        cause_rows.append((v, sd, c))
for v in ["0", "0.5", "1"]:
    r, c = row(f"cap-d{v}", os.path.join(ROOT, f"cap-d{v}"))
    rows.append(r)
    cause_rows.append((v, "capitol", c))

with open(os.path.join(HERE, "scan.csv"), "w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0]))
    w.writeheader()
    w.writerows(rows)

print("| deciduous | seed | grazer deaths | hunter deaths | tree deaths |")
print("| --- | --- | --- | --- | --- |")
for v, sd, c in cause_rows:
    print(f"| {v} | {sd} | {fmt(c['grazer'])} | {fmt(c['hunter'])} | {fmt(c['tree'])} |")
print()
keys = [k for k in rows[0] if k != "run"]
print("| run | " + " | ".join(keys) + " |")
print("|" + " --- |" * (len(keys) + 1))
for r in rows:
    print(f"| {r['run']} | " + " | ".join(str(r[k]) for k in keys) + " |")
