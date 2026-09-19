import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  COLUMNS, SOIL, VOXELS, WATER, columnIndex, loadRun, loadSnapshot, parseMeta, pickSnapshot, voxelIndex,
  type Fetcher,
} from '../../src/loader';

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

  it('throws on format_version 2', async () => {
    const meta = JSON.parse(await readFile(path.join(PUBLIC, 'fixtures/s42-mini/meta.json'), 'utf8'));
    meta.format_version = 2;
    expect(() => parseMeta(meta)).toThrow(/format_version 2/);
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
