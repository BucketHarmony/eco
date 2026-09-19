import { describe, expect, it } from 'vitest';
// @ts-expect-error plain .mjs shared with scripts/film.mjs
import { captionText, cropPng, frameTicks, parseArgs, parseCounts } from '../../scripts/film-lib.mjs';

describe('film helpers', () => {
  it('parses the documented command line', () => {
    const o = parseArgs('--run /runs/s42/ --overlay light --cam top --every 200 --fps 24 --out film/x.mp4'.split(' '));
    expect(o).toMatchObject({ run: 'runs/s42', overlay: 'light', cam: 'top', every: 200, fps: 24, out: 'film/x.mp4' });
    expect(o.limit).toBe(Infinity);
    expect(parseArgs(['--limit', '20', '--max-seconds', '180'])).toMatchObject({ limit: 20, maxSeconds: 180, every: 100 });
  });

  it('rejects unknown options, missing values and non-positive numbers', () => {
    expect(() => parseArgs(['--speed', '2'])).toThrow(/unknown option/);
    expect(() => parseArgs(['--run'])).toThrow(/bad argument/);
    expect(() => parseArgs(['--every', '0'])).toThrow(/positive/);
    expect(() => parseArgs(['--fps', 'x'])).toThrow(/positive/);
  });

  it('picks snapshot ticks that are multiples of every, up to limit', () => {
    const snaps = [0, 100, 200, 300, 400, 500];
    expect(frameTicks(snaps, 100)).toEqual(snaps);
    expect(frameTicks(snaps, 200)).toEqual([0, 200, 400]);
    expect(frameTicks(snaps, 100, 2)).toEqual([0, 100]);
    expect(frameTicks(snaps, 250)).toEqual([0, 500]);
  });

  it('reads counts by header name and captions them', () => {
    const m = parseCounts('trees,tick,hunters,x,grazers\r\n12,0,20,9,300\r\n13,1,21,9,301\r\n');
    expect(m.get(1)).toEqual({ grazers: 301, hunters: 21, trees: 13 });
    expect(captionText(1, m.get(1))).toBe('tick 1   grazers 301   hunters 21   trees 13');
    expect(captionText(7, undefined)).toBe('tick 7   grazers –   hunters –   trees –');
    expect(() => parseCounts('tick,grazers\n0,1\n')).toThrow(/hunters/);
  });

  it('crops a rectangle and refuses one outside the image', () => {
    const img = { width: 3, height: 2, data: Buffer.from(Array.from({ length: 24 }, (_, i) => i)) };
    const c = cropPng(img, { x: 1, y: 1, width: 2, height: 1 });
    expect([...c.data]).toEqual([16, 17, 18, 19, 20, 21, 22, 23]);
    expect(() => cropPng(img, { x: 2, y: 0, width: 2, height: 1 })).toThrow(/outside/);
  });
});
