// The sim helper's pure half and the page's side of it (shot E4). The socket, the child process and the
// browser are tests/e2e/sim.spec.ts's business; everything here is a function with an answer.
import { describe, expect, it } from 'vitest';
import { resolve, sep } from 'node:path';
// @ts-expect-error plain .mjs, as scripts/film-lib.mjs is for tests/unit/film.test.ts
import * as lib from '../../scripts/sim-lib.mjs';

import { SimRunner, simMessage, toBase64, type SimView } from '../../src/sim';

const {
  BUNDLE_FILES, DEFAULT_SEED, DEFAULT_TICKS, MAX_TICKS, checkRunRequest, contentType, runArgs, safeJoin,
  snapshotEvery, tickOfSnapshots,
} = lib;

const b64 = (s: string): string => Buffer.from(s, 'utf8').toString('base64');
const bundle = (): Record<string, string> =>
  Object.fromEntries((BUNDLE_FILES as string[]).map((n: string) => [n, b64(n)]));

describe('the helper checks what it is sent', () => {
  it('takes a whole bundle with a tick count and a seed', () => {
    const r = checkRunRequest({ ticks: 500, seed: 7, files: bundle() });
    expect(r.ticks).toBe(500);
    expect(r.seed).toBe(7);
    expect(r.every).toBe(50);
    expect(r.files.map((f: { name: string }) => f.name)).toEqual(BUNDLE_FILES);
    expect(r.files[0].bytes.toString('utf8')).toBe('bundle.json');
  });

  it('falls back to the defaults when neither is given', () => {
    const r = checkRunRequest({ files: bundle() });
    expect([r.ticks, r.seed]).toEqual([DEFAULT_TICKS, DEFAULT_SEED]);
  });

  it('refuses a tick count that is not a whole number in range', () => {
    for (const ticks of [0, -1, 1.5, MAX_TICKS + 1, 'lots']) {
      expect(() => checkRunRequest({ ticks, files: bundle() })).toThrow(/ticks/);
    }
  });

  it('refuses a file set that is not exactly the bundle', () => {
    const extra = { ...bundle(), 'run.sh': b64('rm -rf /') };
    expect(() => checkRunRequest({ files: extra })).toThrow(/files must be exactly/);
    const short = bundle();
    delete short['medium.u8'];
    expect(() => checkRunRequest({ files: short })).toThrow(/files must be exactly/);
    expect(() => checkRunRequest({ files: { ...bundle(), 'bundle.json': 42 } })).toThrow(/base64/);
    expect(() => checkRunRequest({})).toThrow(/files is not an object/);
  });
});

describe('the command line the helper runs', () => {
  const args: string[] = runArgs('/tmp/r1/world', '/tmp/r1/run', { seed: 3, ticks: 200, every: 20 });

  it('is a garden run on the bundle it was given', () => {
    expect(args.slice(0, 5)).toEqual(['run', '--world', '/tmp/r1/world', '--out', '/tmp/r1/run']);
    expect(args.join(' ')).toContain('--seed 3 --ticks 200 --snapshot-every 20');
  });

  it('carries the two overrides every bundle-world run carries, and skips state.bin', () => {
    expect(args.join(' ')).toContain('--set animals.enabled=false');
    expect(args.join(' ')).toContain('--set climate.rain_gradient=0');
    expect(args.join(' ')).toContain('--snapshot-state false');
    expect(args).not.toContain('--params');
    expect(runArgs('w', 'o', { seed: 1, ticks: 1, every: 1, params: 'p.toml' })).toContain('p.toml');
  });

  it('asks for about ten snapshots whatever the run length', () => {
    for (const ticks of [1, 300, 1000, 20000]) {
      expect(Math.round(ticks / snapshotEvery(ticks))).toBeLessThanOrEqual(10);
      expect(snapshotEvery(ticks)).toBeGreaterThanOrEqual(1);
    }
  });

  it('reads the progress off the snapshot directories', () => {
    expect(tickOfSnapshots(['snap_000000', 'snap_000100', 'snap_000200', 'series.csv'])).toBe(200);
    expect(tickOfSnapshots([])).toBe(0);
    expect(tickOfSnapshots(['snap_1', 'meta.json'])).toBe(0);
  });
});

describe('the static half stays inside the run directory', () => {
  const root = resolve('/tmp/ecoview-sim-x/r1/run');
  const bs = String.fromCharCode(92);

  it('resolves a file of the run', () => {
    expect(safeJoin(root, 'meta.json')).toBe(resolve(root, 'meta.json'));
    expect(safeJoin(root, 'snap_000100/entities.json')).toBe(resolve(root, 'snap_000100', 'entities.json'));
    expect(safeJoin(root, '/meta.json')).toBe(resolve(root, 'meta.json')); // a leading slash is not the disk
  });

  it('refuses every way out of it', () => {
    for (const p of [
      '../world/bundle.json', '..', 'snap_000100/../../world/bundle.json', '%2e%2e%2fworld/bundle.json',
      `..${bs}world${bs}bundle.json`, `%2e%2e${bs}world`, 'a/./b', '', '/', 'C:/Windows/win.ini',
      `C:${bs}Windows${bs}win.ini`, 'meta.json%00.png', '%ZZ',
    ]) {
      expect(safeJoin(root, p), p).toBe(null);
    }
  });

  it('never resolves outside the root, whatever it is given', () => {
    for (const p of ['a/b/c', 'snap_000000/material.bin', '////meta.json', 'etc/passwd']) {
      expect(safeJoin(root, p)?.startsWith(resolve(root) + sep), p).toBe(true);
    }
  });

  it('labels what it serves', () => {
    expect(contentType('meta.json')).toBe('application/json');
    expect(contentType('series.csv')).toContain('text/csv');
    expect(contentType('material.bin')).toBe('application/octet-stream');
  });
});

describe('the page side', () => {
  it('encodes text as its UTF-8 bytes and binary as itself', () => {
    expect(toBase64('bundle')).toBe(b64('bundle'));
    expect(toBase64('°C')).toBe(Buffer.from('°C', 'utf8').toString('base64'));
    expect(toBase64(new Uint8Array([0, 1, 255]))).toBe(Buffer.from([0, 1, 255]).toString('base64'));
    // Well over the 32k chunk the encoder works in, to catch an off-by-one in the loop.
    const big = new Uint8Array(70_000).map((_, i) => i % 251);
    expect(toBase64(big)).toBe(Buffer.from(big).toString('base64'));
  });

  const view = (v: Partial<SimView>): SimView =>
    ({ state: 'idle', message: '', tick: 0, ticks: 0, path: null, ...v });

  it('says what the run is doing in one line', () => {
    expect(simMessage(view({}))).toContain('press R');
    expect(simMessage(view({ state: 'running', tick: 300, ticks: 1000 }))).toBe('sim: running, tick 300 / 1000 (C cancels)');
    expect(simMessage(view({ state: 'done', ticks: 1000, path: 'sim/runs/r1' }))).toContain('sim/runs/r1');
    expect(simMessage(view({ state: 'absent', message: 'no sim helper' }))).toBe('sim: no sim helper');
  });

  it('reports an absent helper rather than throwing, and starts nothing', async () => {
    const seen: SimView[] = [];
    const runner = new SimRunner((v) => seen.push({ ...v }), '/sim', () => Promise.reject(new Error('ECONNREFUSED')));
    const v = await runner.start({ 'bundle.json': '{}' }, { ticks: 10, seed: 1 });
    expect(v.state).toBe('absent');
    expect(v.message).toContain('npm run sim');
    expect(seen.map((s) => s.state)).toEqual(['starting', 'absent']);
    expect(runner.busy).toBe(false);
  });

  it('names the missing binary when the helper is up without one', async () => {
    const reply = (body: unknown): Promise<Response> =>
      Promise.resolve(new Response(JSON.stringify(body), { headers: { 'content-type': 'application/json' } }));
    const runner = new SimRunner(() => {}, '/sim', () => reply({ ok: true, binary: null }));
    const v = await runner.start({ 'bundle.json': '{}' }, { ticks: 10, seed: 1 });
    expect(v.state).toBe('absent');
    expect(v.message).toContain('cargo build --release');
  });
});
