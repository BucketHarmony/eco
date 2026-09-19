"""Markdown tables for the shot 14a-rev sweeps: python table.py <sweep dir>.

Each cell reads `P lag / corr / period` when both animal species persist to the end of the run
(first_extinction empty), else `species extinct @tick, cause`. A * marks the acceptance signature:
pp_corr > 0.3 with 0 < pp_lag < pp_period / 2.
"""
import csv
import os
import sys


def cell(r):
    if r['first_extinction_tick']:
        return f"{r['first_extinction_species']} extinct @{r['first_extinction_tick']}, {r['first_extinction_dominant_cause']}"
    lag, corr, per = r['pp_lag'], r['pp_corr'], r['pp_period']
    ok = per and float(corr) > 0.3 and 0 < int(lag) < int(per) / 2
    return f"P {lag} / {float(corr):.2f} / {per or 'undef'}" + (' *' if ok else '')


d = sys.argv[1]
rows = list(csv.DictReader(open(os.path.join(d, 'sweep.csv'))))
params = [k for k in rows[0] if k.startswith(('hunter.', 'disease.'))]
seeds = sorted({r['seed'] for r in rows}, key=int)
grid = {}
for r in rows:
    grid.setdefault(tuple(r[p] for p in params), {})[r['seed']] = r
print('| ' + ' | '.join(params + [f's{s}' for s in seeds]) + ' | persist |')
print('|' + '---|' * (len(params) + len(seeds) + 1))
for key, by_seed in grid.items():
    n = sum(1 for s in seeds if not by_seed[s]['first_extinction_tick'])
    print('| ' + ' | '.join(list(key) + [cell(by_seed[s]) for s in seeds]) + f' | {n}/{len(seeds)} |')
