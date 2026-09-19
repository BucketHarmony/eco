"""Shot 14c re-score: the fixed predator-prey signature on runs that already exist. No new sims.

    python sweeps/shot14c/rescore.py   (from ecosim/; prints the tables in RESCORE.md, writes rescore.csv)

For every cell of shot 14a-rev's three sweeps it runs
`ecosim stats --signature --year-len 4000 <cells/*.csv>` (the sweeps left climate.year_len at its
default, 4000), writes every result to rescore.csv, and prints tables in 14a-rev's layout: a cell
reads `P lag / corr / period` (with PASS when pp_pass) when both animal species persist to the end
of the run, as 14a-rev's `first_extinction_tick` says, else 14a-rev's `species extinct @tick, cause`.
Persistence is 14a-rev's, unchanged. The current-model runs are runs/14c/s{1,2,3,42}: `ecosim run
--seed S --ticks 60000 --snapshot-every 10000` at defaults, the reproduction of 14a-rev's Context
table; their signature is read from the run directory (year_len from meta.json).
"""
import csv
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
EXE = os.path.join(ROOT, 'target', 'release', 'ecosim')
OLD = os.path.join(ROOT, 'sweeps', 'shot14a-rev')
SWEEPS = ['handling_ticks', 'refractory', 'handling_x_cost']
# 14a-rev's Context table (sweeps/shot14a-rev/FINDINGS.md), under its 4000-tick-detrend definition.
OLD_CONTEXT = {'1': (-1950, 0.35, 2800), '2': (-1800, 0.34, 2850), '3': (850, 0.39, 2800), '42': (850, 0.35, 3200)}
LINE = re.compile(r'signature: pp_lag (-?\d+) pp_corr (\S+) pp_period (\S+) pp_pass (\w+)')


def score(path, year_len=True):
    args = [EXE, 'stats', '--signature'] + (['--year-len', '4000'] if year_len else []) + [path]
    out = subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()
    m = LINE.match(out)
    if not m:
        return {'lag': '', 'corr': '', 'period': '', 'pass': 'false', 'undefined': out}
    lag, corr, period, ok = m.groups()
    return {'lag': lag, 'corr': corr, 'period': '' if period == 'undefined' else period, 'pass': ok, 'undefined': ''}


def cell(r, s):
    if r['first_extinction_tick']:
        return f"{r['first_extinction_species']} extinct @{r['first_extinction_tick']}, {r['first_extinction_dominant_cause']}"
    text = f"P {s['lag']} / {float(s['corr']):.2f} / {s['period'] or 'undef'}"
    return text + (' PASS' if s['pass'] == 'true' else '')


def main():
    out_rows = []
    for name in SWEEPS:
        d = os.path.join(OLD, name)
        rows = list(csv.DictReader(open(os.path.join(d, 'sweep.csv'))))
        params = [k for k in rows[0] if k.startswith(('hunter.', 'disease.'))]
        seeds = sorted({r['seed'] for r in rows}, key=int)
        grid = {}
        for r in rows:
            cid = '_'.join(f'{p}={r[p]}' for p in params) + f"_s={r['seed']}"
            s = score(os.path.join(d, 'cells', cid + '.csv'))
            out_rows.append([name, cid, r['first_extinction_tick'], s['lag'], s['corr'], s['period'], s['pass'], s['undefined']])
            grid.setdefault(tuple(r[p] for p in params), {})[r['seed']] = (r, s)
        print(f'## {name}\n')
        print('| ' + ' | '.join(params + [f's{x}' for x in seeds]) + ' | persist | pass |')
        print('|' + '---|' * (len(params) + len(seeds) + 2))
        for key, by_seed in grid.items():
            n = sum(1 for x in seeds if not by_seed[x][0]['first_extinction_tick'])
            k = sum(1 for x in seeds if by_seed[x][1]['pass'] == 'true')
            cells = [cell(*by_seed[x]) for x in seeds]
            print('| ' + ' | '.join(list(key) + cells) + f' | {n}/{len(seeds)} | {k}/{len(seeds)} |')
        print()
    print('## Current model (defaults, hunter crowding on), 60000 ticks\n')
    print('| seed | old pp_lag / pp_corr / pp_period | new pp_lag / pp_corr / pp_period | old period | new period | pp_pass |')
    print('|---|---|---|---|---|---|')
    for seed, (ol, oc, op) in OLD_CONTEXT.items():
        s = score(os.path.join(ROOT, 'runs', '14c', f's{seed}'), year_len=False)
        out_rows.append(['current', f's={seed}', '', s['lag'], s['corr'], s['period'], s['pass'], s['undefined']])
        print(f"| {seed} | {ol} / {oc:.2f} / {op} | {s['lag']} / {float(s['corr']):.2f} / {s['period'] or 'undef'} "
              f"| {op} | {s['period'] or 'undef'} | {s['pass']} |")
    with open(os.path.join(HERE, 'rescore.csv'), 'w', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['sweep', 'cell', 'first_extinction_tick', 'pp_lag', 'pp_corr', 'pp_period', 'pp_pass', 'undefined'])
        w.writerows(out_rows)


if __name__ == '__main__':
    sys.exit(main())
