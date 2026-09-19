import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  FORMAT_VERSIONS, Grid, SOIL, WATER, burntPatches, loadRun, loadSnapshot, parseEvents, parseMeta, parseSeries,
  pickSnapshot, type Fetcher,
} from '../../src/loader';
import { binDeaths } from '../../src/ui';
import { COLORS, OVERLAYS, columnColor, hexToRgb, soilColor } from '../../src/world';

const PUBLIC = path.resolve(__dirname, '../../public');

/** Serves files from public/ the way vite does, so the loader runs unchanged under Node. */
const fsFetcher: Fetcher = async (url) => {
  try {
    const buf = await readFile(path.join(PUBLIC, url));
    return new Response(buf, { status: 200 });
  } catch {
    return new Response('not found', { status: 404 });
  }
};

const fakeFetcher = (files: Record<string, string | Uint8Array>): Fetcher => async (url) =>
  url in files ? new Response(files[url] as BodyInit, { status: 200 }) : new Response('', { status: 404 });

/** Serves `dir` from public/ with `meta.json` passed through `f`, recording every URL fetched. */
function withMeta(dir: string, f: (meta: Record<string, unknown>) => Record<string, unknown>, fetched: string[] = []): Fetcher {
  return async (url) => {
    fetched.push(url);
    const res = await fsFetcher(url);
    if (url !== `/${dir}/meta.json`) return res;
    return new Response(JSON.stringify(f(await res.json())), { status: 200 });
  };
}

// The square world's mini fixture (ecosim fixtures/s42-mini-v2, format 2) and the 256×64 strip's (format 3,
// with events.csv), both seed 42 at 100 ticks with a snapshot every 100.
const FIXTURES = [
  { dir: 'fixtures/s42-mini', width: 64, version: 2, waterColumns: 163 },
  { dir: 'fixtures/s42-strip-mini', width: 256, version: 3, waterColumns: 293 },
] as const;

describe.each(FIXTURES)('loader on $dir', ({ dir, width, version, waterColumns }) => {
  it('reads dims and format_version from meta.json', async () => {
    const run = await loadRun(`/${dir}`, fsFetcher);
    expect(run.meta.format_version).toBe(version);
    expect(run.meta.dims).toMatchObject({ x: width, y: 64, z: 32 });
    expect(run.grid).toMatchObject({ x: width, y: 64, z: 32, patch: 8, px: width / 8, py: 8 });
    expect(run.grid.voxels).toBe(width * 64 * 32);
    expect(run.grid.patches).toBe(width);
  });

  it('parses meta and series', async () => {
    const run = await loadRun(`/${dir}`, fsFetcher);
    expect(run.meta.snapshots).toEqual([0, 100]);
    expect(run.meta.species.map((s) => s.name)).toEqual(['grass', 'shrub', 'tree', 'grazer', 'hunter']);
    expect(run.series.tick.length).toBe(101);
    expect(run.series.tick[0]).toBe(0);
    expect(run.series.tick[100]).toBe(100);
    for (const v of run.series.grass_mean) expect(v).toBeGreaterThanOrEqual(0);
    expect(run.series.grazers[0]).toBeGreaterThan(0);
    expect(Object.keys(run.series.deaths)).toEqual(['starved', 'eaten', 'old_age', 'crowded', 'burnt']);
    for (const col of Object.values(run.series.deaths)) expect(col.length).toBe(101);
  });

  it('reads tree lifespan and ignores unknown entity keys', async () => {
    const run = await loadRun(`/${dir}`, fsFetcher);
    const s = await loadSnapshot(run, 0, fsFetcher);
    const trees = s.entities.filter((e) => e.kind === 'tree');
    expect(trees.length).toBeGreaterThan(0);
    for (const t of trees) expect(t.lifespan).toBeGreaterThan(t.age);
    const extra: Fetcher = async (url) => {
      if (!url.endsWith('entities.json')) return fsFetcher(url);
      const ents = await (await fsFetcher(url)).json();
      return new Response(JSON.stringify(ents.map((e: object) => ({ ...e, genome: [1, 2], future: 'x' }))));
    };
    const t = await loadSnapshot(run, 0, extra);
    expect(t.entities.length).toBe(s.entities.length);
    expect(t.entities[0].x).toBe(s.entities[0].x);
  });

  it('parses both snapshots with the right lengths and value ranges', async () => {
    const run = await loadRun(`/${dir}`, fsFetcher);
    const g = run.grid;
    for (const tick of run.meta.snapshots) {
      const s = await loadSnapshot(run, tick, fsFetcher);
      expect(s.grid).toBe(g);
      expect(s.material.length).toBe(width * 64 * 32);
      expect(s.light.length).toBe(width * 64 * 32);
      for (const a of [s.moisture, s.fertility, s.height]) expect(a.length).toBe(width * 64);
      expect(s.material.reduce((a, b) => Math.max(a, b), 0)).toBeLessThanOrEqual(3);
      expect(Math.min(...s.height)).toBeGreaterThanOrEqual(8);
      expect(Math.max(...s.height)).toBeLessThanOrEqual(24);
      let water = 0;
      for (let y = 0; y < g.y; y++) {
        for (let x = 0; x < g.x; x++) {
          const h = s.height[g.column(x, y)];
          const top = s.material[g.voxel(x, y, h)];
          expect(top).not.toBe(0);
          expect(s.material[g.voxel(x, y, h + 1)]).toBe(0);
          if (top === WATER) water++;
          if (top !== SOIL) expect(s.moisture[g.column(x, y)]).toBe(0);
        }
      }
      expect(water).toBe(waterColumns);
      expect(s.patches.length).toBe(width);
      for (const p of s.patches) {
        expect(p.grass).toBeGreaterThanOrEqual(0);
        expect(p.grass).toBeLessThanOrEqual(1);
        expect(p.shrub).toBeGreaterThanOrEqual(0);
        expect(p.shrub).toBeLessThanOrEqual(1);
      }
      const kinds = new Set(s.entities.map((e) => e.kind));
      expect([...kinds].sort()).toEqual(['grazer', 'hunter', 'tree']);
      for (const e of s.entities) {
        expect(e.x).toBeGreaterThanOrEqual(0);
        expect(e.x).toBeLessThan(width);
        expect(e.y).toBeLessThan(64);
        expect(e.z).toBe(s.height[g.column(e.x, e.y)] + 1);
      }
      expect([...s.burnt]).toEqual(new Array(width).fill(0));
      for (const overlay of OVERLAYS) expect(columnColor(s, width - 1, 63, overlay)).toHaveLength(3);
    }
  });

  it('throws on wrong binary sizes', async () => {
    const run = await loadRun(`/${dir}`, fsFetcher);
    const short: Fetcher = async (url) =>
      url.endsWith('light.bin') ? new Response(new Uint8Array(10)) : fsFetcher(url);
    await expect(loadSnapshot(run, 0, short)).rejects.toThrow(`expected ${width * 64 * 32} bytes, got 10`);
  });

  it('throws when meta.json dims disagree with the .bin sizes', async () => {
    const other = width === 64 ? 128 : 64;
    const run = await loadRun(`/${dir}`, withMeta(dir, (m) => ({ ...m, dims: { ...(m.dims as object), x: other } })));
    expect(run.grid.x).toBe(other);
    // Whichever .bin arrives first fails: every one is sized from dims.
    const sizes = `(${other * 64 * 32} bytes, got ${width * 64 * 32}|${other * 64} bytes, got ${width * 64})`;
    await expect(loadSnapshot(run, 0, fsFetcher)).rejects.toThrow(new RegExp(`\\.bin: expected ${sizes}$`));
  });
});

describe('loader formats and dims', () => {
  it('accepts format_version 1, 2 and 3 only', () => {
    expect(FORMAT_VERSIONS).toEqual([1, 2, 3]);
  });

  it('loads a format_version 2 run to the same scene data as version 1, never fetching state.bin', async () => {
    const v1 = withMeta('fixtures/s42-mini', (m) => {
      const meta: Record<string, unknown> = { ...m, format_version: 1 };
      delete meta.forked_from;
      return meta;
    });
    const fetched: string[] = [];
    const v2 = withMeta('fixtures/s42-mini', (m) => ({ ...m, format_version: 2, forked_from: { run: 'runs/s42', tick: 5000 } }), fetched);
    const a = await loadRun('/fixtures/s42-mini', v1);
    const b = await loadRun('/fixtures/s42-mini', v2);
    expect(a.meta.format_version).toBe(1);
    expect(b.meta.format_version).toBe(2);
    expect(b.meta.forked_from).toEqual({ run: 'runs/s42', tick: 5000 });
    expect({ ...b.meta, format_version: 1, forked_from: undefined }).toEqual({ ...a.meta, forked_from: undefined });
    expect(b.series).toEqual(a.series);
    for (const tick of a.meta.snapshots) {
      const sa = await loadSnapshot(a, tick, v1);
      const sb = await loadSnapshot(b, tick, v2);
      expect(sb).toEqual(sa);
      for (const overlay of OVERLAYS) {
        for (let y = 0; y < 64; y++) {
          for (let x = 0; x < 64; x++) {
            expect(columnColor(sb, x, y, overlay)).toEqual(columnColor(sa, x, y, overlay));
          }
        }
      }
    }
    expect(fetched.some((u) => u.endsWith('state.bin') || u.endsWith('events.csv'))).toBe(false);
  });

  it('reads a pre-shot-15 dims object without patch as 8×8 patches', () => {
    const grid = new Grid({ x: 64, y: 64, z: 32 });
    expect(grid.patch).toBe(8);
    expect(grid.patchOf(63, 63)).toBe(63);
    expect(new Grid({ x: 256, y: 64, z: 32, patch: 8 }).patchOf(255, 63)).toBe(255);
    expect(new Grid({ x: 64, y: 32, z: 16, patch: 16 }).patchOf(40, 20)).toBe(2 + 4 * 1);
  });

  it('throws on format_version 4 and on dims the sim never writes', async () => {
    const meta = JSON.parse(await readFile(path.join(PUBLIC, 'fixtures/s42-strip-mini/meta.json'), 'utf8'));
    expect(() => parseMeta({ ...meta, format_version: 4 })).toThrow(/format_version 4/);
    const f = fakeFetcher({ '/bad/meta.json': JSON.stringify({ ...meta, format_version: 4 }), '/bad/series.csv': 'tick\n0\n' });
    await expect(loadRun('/bad', f)).rejects.toThrow(/format_version/);
    for (const dims of [
      undefined, { x: 64, y: 64 }, { x: 0, y: 64, z: 32 }, { x: 257, y: 64, z: 32 }, { x: 64.5, y: 64, z: 32 },
      { x: 60, y: 64, z: 32, patch: 8 }, { x: 64, y: 64, z: 32, patch: 0 },
    ]) {
      expect(() => parseMeta({ ...meta, dims }), JSON.stringify(dims)).toThrow(/unsupported dims/);
    }
    expect(parseMeta({ ...meta, dims: { x: 60, y: 60, z: 32, patch: 6 } }).dims.x).toBe(60);
  });

  it('throws on missing files', async () => {
    await expect(loadRun('/nope', fakeFetcher({}))).rejects.toThrow(/404/);
    const noEvents: Fetcher = async (url) => (url.endsWith('events.csv') ? new Response('', { status: 404 }) : fsFetcher(url));
    await expect(loadRun('/fixtures/s42-strip-mini', noEvents)).rejects.toThrow('events.csv: HTTP 404');
  });
});

describe('events.csv', () => {
  const HEADER = 'tick,kind,species,patch_x,patch_y,x,y,cause,detail';

  it('parses the strip fixture: one event per row, empty fields as null or empty', async () => {
    const text = await readFile(path.join(PUBLIC, 'fixtures/s42-strip-mini/events.csv'), 'utf8');
    const run = await loadRun('/fixtures/s42-strip-mini', fsFetcher);
    expect(run.events.length).toBe(text.trim().split('\n').length - 1);
    expect(run.events[0]).toEqual({
      tick: 6, kind: 'birth', species: 'grazer', patch_x: 18, patch_y: 1, x: 145, y: 10, cause: '', detail: 332,
    });
    const deaths = run.events.filter((e) => e.kind === 'death');
    expect(deaths.length).toBeGreaterThan(0);
    for (const e of deaths) expect(['starved', 'eaten', 'old_age', 'crowded', 'burnt']).toContain(e.cause);
    for (const e of run.events) {
      expect(e.tick).toBeGreaterThanOrEqual(0);
      expect(e.tick).toBeLessThanOrEqual(100);
      expect(e.patch_x).toBe(Math.floor(e.x! / 8));
      expect(e.patch_y).toBe(Math.floor(e.y! / 8));
    }
    // Deaths in events.csv match the series' death columns tick by tick.
    const perTick = new Array(101).fill(0);
    for (const e of deaths) perTick[e.tick]++;
    const series = new Array(101).fill(0);
    for (const col of Object.values(run.series.deaths)) col.forEach((v, i) => (series[i] += v));
    expect(perTick).toEqual(series);
    expect(run.burnouts).toEqual([]);
  });

  it('reads fire rows with empty fields, by header name in any order', () => {
    const rows = parseEvents(`detail,tick,kind,species,patch_x,patch_y,x,y,cause,extra
,700,ignition,,3,1,,,,x
12,701,spread,,4,1,,,,
,703,burnout,,3,1,,,,
40,705,tree_death,tree,4,1,33,12,burnt,
`);
    expect(rows.map((e) => e.kind)).toEqual(['ignition', 'spread', 'burnout', 'tree_death']);
    expect(rows[0]).toEqual({
      tick: 700, kind: 'ignition', species: '', patch_x: 3, patch_y: 1, x: null, y: null, cause: '', detail: null,
    });
    expect(rows[1].detail).toBe(12);
    expect(rows[3]).toMatchObject({ species: 'tree', x: 33, y: 12, cause: 'burnt', detail: 40 });
    expect(parseEvents(`${HEADER}\n`)).toEqual([]);
    expect(() => parseEvents('tick,kind\n1,birth\n')).toThrow('events.csv: missing column species');
  });

  it('burnt patches are the burnouts since the previous snapshot', () => {
    const grid = new Grid({ x: 32, y: 16, z: 8, patch: 8 });
    const rows = parseEvents(`${HEADER}
50,burnout,,1,0,,,,
99,ignition,,2,0,,,,
100,burnout,,2,0,,,,
150,burnout,,3,1,,,,
250,burnout,,9,9,,,,
`).filter((e) => e.kind === 'burnout');
    const burnt = (from: number, to: number) => [...burntPatches(rows, grid, from, to)];
    expect(burnt(-1, 0)).toEqual([0, 0, 0, 0, 0, 0, 0, 0]);
    expect(burnt(0, 100)).toEqual([0, 1, 1, 0, 0, 0, 0, 0]);
    expect(burnt(100, 200)).toEqual([0, 0, 0, 0, 0, 0, 0, 1]);
    expect(burnt(200, 300)).toEqual([0, 0, 0, 0, 0, 0, 0, 0]); // off the grid
  });

  it('the fire overlay draws a patch burnt after a burnout since the last snapshot, and a bare patch without one as soil', async () => {
    // A 24×8×4 world of three 8×8 patches, all soil at height 1 and bare (grass and shrub 0); snapshots 0, 50, 100.
    const grid = new Grid({ x: 24, y: 8, z: 4, patch: 8 });
    const material = new Uint8Array(grid.voxels);
    for (let y = 0; y < 8; y++) {
      for (let x = 0; x < 24; x++) material[grid.voxel(x, y, 0)] = material[grid.voxel(x, y, 1)] = SOIL;
    }
    const cols = () => new Uint8Array(grid.columns).fill(1);
    const bare = { grass: 0, shrub: 0, detritus: 0, temperature: 10, burning_ticks_left: 0 };
    const meta = {
      format_version: 3, dims: { x: 24, y: 8, z: 4, patch: 8 }, seed: 1, ticks: 100, snapshot_every: 50, year_len: 4000,
      water_level: 0, snapshots: [0, 50, 100], species: [],
    };
    const files: Record<string, string | Uint8Array> = {
      '/w/meta.json': JSON.stringify(meta),
      '/w/series.csv': 'tick,grazers,hunters,trees,grass_mean,shrub_mean,moisture_mean,fertility_mean,detritus_total,temperature\n'
        + '0,0,0,0,0,0,0,0,0,10\n',
      // Patch 1 burnt out at 20 (before snapshot 50), patch 0 at 70 (after it), patch 2 never.
      '/w/events.csv': `${HEADER}\n18,ignition,,1,0,,,,\n20,burnout,,1,0,,,,\n67,ignition,,0,0,,,,\n70,burnout,,0,0,,,,\n`,
    };
    for (const t of ['000050', '000100']) {
      const d = `/w/snap_${t}`;
      Object.assign(files, {
        [`${d}/material.bin`]: material, [`${d}/light.bin`]: new Uint8Array(grid.voxels),
        [`${d}/moisture.bin`]: cols(), [`${d}/fertility.bin`]: cols(), [`${d}/height.bin`]: cols(),
        [`${d}/patches.json`]: JSON.stringify([bare, bare, bare]), [`${d}/entities.json`]: '[]',
      });
    }
    const run = await loadRun('/w', fakeFetcher(files));
    expect(run.burnouts.map((e) => e.tick)).toEqual([20, 70]);
    const s50 = await loadSnapshot(run, 50, fakeFetcher(files));
    const s100 = await loadSnapshot(run, 100, fakeFetcher(files));
    const charcoal = hexToRgb(COLORS.burnt);
    expect(columnColor(s100, 3, 3, 'fire'), 'patch 0 burnt out at 70').toEqual(charcoal);
    expect(columnColor(s100, 11, 3, 'fire'), 'patch 1 burnt out at 20, before snapshot 50').toEqual(soilColor(0, 0));
    expect(columnColor(s100, 19, 3, 'fire'), 'patch 2, bare with no burnout').toEqual(soilColor(0, 0));
    expect(columnColor(s50, 11, 3, 'fire'), 'patch 1 at snapshot 50').toEqual(charcoal);
    expect(columnColor(s50, 3, 3, 'fire')).toEqual(soilColor(0, 0));
    // Burning wins over burnt, and the other overlays ignore burnt ground.
    s100.patches[0] = { ...bare, burning_ticks_left: 3 };
    expect(columnColor(s100, 3, 3, 'fire')).toEqual(hexToRgb(COLORS.fireHi));
    expect(columnColor(s50, 11, 3, 'material')).toEqual(soilColor(0, 0));
  });
});

describe('parseSeries', () => {
  const CORE = 'tick,grazers,hunters,trees,grass_mean,shrub_mean,moisture_mean,fertility_mean,detritus_total,temperature';

  it('reads columns by header name, whatever their order, and ignores unknown ones', () => {
    const a = parseSeries(`${CORE}
0,5,2,1,0.1,0.2,3,4,5,6
1,6,3,1,0.1,0.2,3,4,5,7
`);
    const cols = CORE.split(',');
    const order = [...cols.keys()].reverse();
    const row = (r: string) => order.map((i) => r.split(',')[i]).join(',');
    const b = parseSeries(
      `new_col,${order.map((i) => cols[i]).join(',')}
9,${row('0,5,2,1,0.1,0.2,3,4,5,6')}
9,${row('1,6,3,1,0.1,0.2,3,4,5,7')}
`,
    );
    for (const c of cols) expect([...b[c as keyof typeof b] as Float64Array]).toEqual([...a[c as keyof typeof a] as Float64Array]);
    expect(a.deaths).toEqual({});
  });

  it('sums each death cause over the species that have the column', () => {
    const s = parseSeries(`hunter_eaten,grazer_starved,${CORE},hunter_starved,hunter_immigrants
` +
      `0,4,0,5,2,1,0,0,0,0,0,0,1,7
0,0,1,5,2,1,0,0,0,0,0,0,2,7
`);
    expect(Object.keys(s.deaths)).toEqual(['starved', 'eaten']);
    expect([...s.deaths.starved!]).toEqual([5, 2]);
    expect([...s.deaths.eaten!]).toEqual([0, 0]);
  });

  it('reads the fire and trait columns when present and leaves them undefined when not', () => {
    const s = parseSeries(`${CORE},patches_burning,total_burnt,grazer_energy_cost_mult_mean,hunter_repro_threshold_sd
0,5,2,1,0,0,0,0,0,0,0,0,1.0000,0.0000
1,5,2,1,0,0,0,0,0,0,3,1,0.9812,2.5000
`);
    expect([...s.extra.patches_burning!]).toEqual([0, 3]);
    expect([...s.extra.total_burnt!]).toEqual([0, 1]);
    expect([...s.extra.grazer_energy_cost_mult_mean!]).toEqual([1, 0.9812]);
    expect([...s.extra.hunter_repro_threshold_sd!]).toEqual([0, 2.5]);
    expect(s.extra.grazer_flee_distance_mean).toBeUndefined();
    expect(parseSeries(`${CORE}
0,5,2,1,0,0,0,0,0,0
`).extra).toEqual({});
  });

  it('bins deaths per 100 ticks', () => {
    const ticks = Float64Array.from({ length: 250 }, (_, i) => i);
    const ones = new Float64Array(250).fill(1);
    const { start, by } = binDeaths(ticks, { old_age: ones });
    expect(start).toEqual([0, 100, 200]);
    expect(by).toEqual([['old_age', [100, 100, 50]]]);
  });
});

describe('pickSnapshot', () => {
  const snaps = [0, 100, 200];
  it('picks the largest tick <= requested and clamps', () => {
    expect(pickSnapshot(snaps, 0)).toBe(0);
    expect(pickSnapshot(snaps, 150)).toBe(100);
    expect(pickSnapshot(snaps, 200)).toBe(200);
    expect(pickSnapshot(snaps, 99999)).toBe(200);
    expect(pickSnapshot(snaps, -5)).toBe(0);
  });
});
