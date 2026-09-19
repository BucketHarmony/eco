import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  COLUMNS, SOIL, VOXELS, WATER, columnIndex, loadRun, loadSnapshot, parseMeta, parseSeries, pickSnapshot,
  voxelIndex, type Fetcher,
} from '../../src/loader';
import { binDeaths } from '../../src/ui';
import { OVERLAYS, columnColor } from '../../src/world';

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

const fakeFetcher = (files: Record<string, string>): Fetcher => async (url) =>
  url in files ? new Response(files[url], { status: 200 }) : new Response('', { status: 404 });

describe('loader on fixtures/s42-mini', () => {
  it('parses meta and series', async () => {
    const run = await loadRun('/fixtures/s42-mini', fsFetcher);
    expect(run.meta.format_version).toBe(1);
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
    const run = await loadRun('/fixtures/s42-mini', fsFetcher);
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
    const run = await loadRun('/fixtures/s42-mini', fsFetcher);
    for (const tick of run.meta.snapshots) {
      const s = await loadSnapshot(run, tick, fsFetcher);
      expect(s.material.length).toBe(VOXELS);
      expect(s.light.length).toBe(VOXELS);
      for (const a of [s.moisture, s.fertility, s.height]) expect(a.length).toBe(COLUMNS);
      expect(s.material.reduce((a, b) => Math.max(a, b), 0)).toBeLessThanOrEqual(3);
      expect(Math.min(...s.height)).toBeGreaterThanOrEqual(8);
      expect(Math.max(...s.height)).toBeLessThanOrEqual(24);
      let water = 0;
      for (let y = 0; y < 64; y++) {
        for (let x = 0; x < 64; x++) {
          const h = s.height[columnIndex(x, y)];
          const top = s.material[voxelIndex(x, y, h)];
          expect(top).not.toBe(0);
          expect(s.material[voxelIndex(x, y, h + 1)]).toBe(0);
          if (top === WATER) water++;
          if (top !== SOIL) expect(s.moisture[columnIndex(x, y)]).toBe(0);
        }
      }
      expect(water).toBeGreaterThanOrEqual(100);
      expect(s.patches.length).toBe(64);
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
        expect(e.x).toBeLessThan(64);
        expect(e.z).toBe(s.height[columnIndex(e.x, e.y)] + 1);
      }
    }
  });

  it('loads a format_version 2 run to the same scene data as version 1, never fetching state.bin', async () => {
    const fetched: string[] = [];
    const v2: Fetcher = async (url) => {
      fetched.push(url);
      if (url.endsWith('/state.bin')) return new Response(new Uint8Array(64), { status: 200 });
      const res = await fsFetcher(url);
      if (!url.endsWith('/meta.json')) return res;
      const meta = { ...(await res.json()), format_version: 2, forked_from: { run: 'runs/s42', tick: 5000 } };
      return new Response(JSON.stringify(meta), { status: 200 });
    };
    const a = await loadRun('/fixtures/s42-mini', fsFetcher);
    const b = await loadRun('/fixtures/s42-mini', v2);
    expect(a.meta.format_version).toBe(1);
    expect(b.meta.format_version).toBe(2);
    expect(b.meta.forked_from).toEqual({ run: 'runs/s42', tick: 5000 });
    expect({ ...b.meta, format_version: 1, forked_from: undefined }).toEqual({ ...a.meta, forked_from: undefined });
    expect(b.series).toEqual(a.series);
    for (const tick of a.meta.snapshots) {
      const sa = await loadSnapshot(a, tick, fsFetcher);
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
    expect(fetched.some((u) => u.endsWith('state.bin'))).toBe(false);
  });

  it('throws on format_version 3', async () => {
    const meta = JSON.parse(await readFile(path.join(PUBLIC, 'fixtures/s42-mini/meta.json'), 'utf8'));
    meta.format_version = 3;
    expect(() => parseMeta(meta)).toThrow(/format_version 3/);
    const f = fakeFetcher({ '/bad/meta.json': JSON.stringify(meta), '/bad/series.csv': 'tick\n0\n' });
    await expect(loadRun('/bad', f)).rejects.toThrow(/format_version/);
  });

  it('throws on missing files and wrong binary sizes', async () => {
    await expect(loadRun('/nope', fakeFetcher({}))).rejects.toThrow(/404/);
    const run = await loadRun('/fixtures/s42-mini', fsFetcher);
    const short: Fetcher = async (url) =>
      url.endsWith('light.bin') ? new Response(new Uint8Array(10)) : fsFetcher(url);
    await expect(loadSnapshot(run, 0, short)).rejects.toThrow(/expected 131072 bytes/);
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
