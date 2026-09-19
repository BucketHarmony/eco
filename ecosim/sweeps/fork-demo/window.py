import csv,sys
def load(p):
    return list(csv.DictReader(open(p)))
for name,p in [("s42","runs/s42/series.csv"),("fork","sweeps/fork-demo/run/series.csv")]:
    r=[x for x in load(p) if int(x['tick'])>10000]
    cols=[c for c in r[0] if c.startswith('grazer_') or c.startswith('hunter_s') or c.startswith('hunter_o') or c.startswith('hunter_e')]
    tot={c:sum(int(x[c]) for x in r) for c in cols}
    def st(c): v=[float(x[c]) for x in r]; return f"{min(v):.1f}/{sum(v)/len(v):.1f}/{max(v):.1f}"
    print(name, {k:v for k,v in tot.items() if v}, "| g",st('grazers'),"h",st('hunters'),"t",st('trees'),"grass",st('grass_mean'),"moist",st('moisture_mean'),"shrub",st('shrub_mean'),"end trees",r[-1]['trees'])
