"""Grazer peak and mean per patch, plus death causes, for each cell of a shot-10 sweep.

usage: python sweeps/shot10/per_patch.py sweeps/shot10/<name>

Reads sweep.csv and cells/*.csv (regenerate the cells by rerunning the logged sweep command).
"Per patch" is the live count divided by the 64 patches. Peak and mean are over ticks 2000-20000
(the burn-in excluded, as check's 10x anchor does). Deaths are run totals.
"""
import csv
import os
import sys

d = sys.argv[1]
rows = list(csv.DictReader(open(os.path.join(d, 'sweep.csv'))))
param = next(k for k in rows[0] if '.' in k)
print(f'{param} seed | grazers/patch peak mean | hunters peak mean | grazer deaths crowded eaten starved | '
      f'hunter deaths old_age starved crowded | first extinction')
for r in rows:
    cid = f"{param}={r[param]}_s={r['seed']}"
    cell = list(csv.DictReader(open(os.path.join(d, 'cells', cid + '.csv'))))
    late = cell[2000:]
    g = [int(x['grazers']) for x in late]
    h = [int(x['hunters']) for x in late]
    tot = lambda k: sum(int(x[k]) for x in cell)
    ext = r['first_extinction_tick'] or '-'
    if ext != '-':
        ext = f"{r['first_extinction_species']} @{ext} ({r['first_extinction_dominant_cause']})"
    print(f"{r[param]} s{r['seed']} | {max(g) / 64:.1f} {sum(g) / len(g) / 64:.1f} | {max(h)} {sum(h) / len(h):.1f} | "
          f"{tot('grazer_crowded')} {tot('grazer_eaten')} {tot('grazer_starved')} | "
          f"{tot('hunter_old_age')} {tot('hunter_starved')} {tot('hunter_crowded')} | {ext}")
