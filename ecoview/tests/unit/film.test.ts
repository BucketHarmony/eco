import { describe, expect, it } from 'vitest';
// @ts-expect-error plain .mjs shared with scripts/film.mjs
import * as film from '../../scripts/film-lib.mjs';

const {
  BG_RGB, captionText, cropPng, frameTicks, gridFor, h264Level, parseArgs, parseCounts, parseTiles, pastePng, tiledFrameSize, tileRect,
} = film;

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

  it('parses tiles and layouts, defaulting to the smallest near-square grid', () => {
    expect(parseTiles('material:iso,fire:top')).toEqual([{ overlay: 'material', cam: 'iso' }, { overlay: 'fire', cam: 'top' }]);
    expect(() => parseTiles('fire')).toThrow(/overlay:cam/);
    expect(() => parseTiles('fire:front')).toThrow(/overlay:cam/);
    expect(() => parseTiles('fire:top:x')).toThrow(/overlay:cam/);
    expect([1, 2, 3, 4, 5, 7].map((n) => gridFor(n))).toEqual([
      { cols: 1, rows: 1 }, { cols: 2, rows: 1 }, { cols: 2, rows: 2 }, { cols: 2, rows: 2 }, { cols: 3, rows: 2 }, { cols: 3, rows: 3 },
    ]);
    expect(gridFor(3, '3x1')).toEqual({ cols: 3, rows: 1 });
    expect(() => gridFor(5, '2x2')).toThrow(/4 cells for 5 tiles/);
    expect(() => gridFor(1, '2by2')).toThrow(/CxR/);
    expect(() => gridFor(1, '0x2')).toThrow(/CxR/);
  });

  it('accepts --tiles, --layout and --scale together and rejects them without tiles', () => {
    const o = parseArgs(['--tiles', 'material:iso,fire:top', '--layout', '1x2', '--scale', '2']);
    expect(o).toMatchObject({ tiles: 'material:iso,fire:top', layout: '1x2', scale: 2 });
    expect(() => parseArgs(['--layout', '2x2'])).toThrow(/need --tiles/);
    expect(() => parseArgs(['--scale', '2'])).toThrow(/need --tiles/);
    expect(() => parseArgs(['--tiles', 'material:iso', '--scale', '1.5'])).toThrow(/whole number/);
    expect(() => parseArgs(['--tiles', 'a:iso,b:iso,c:iso', '--layout', '1x2'])).toThrow(/cells/);
  });

  it('sizes tiled frames and places tiles row-major', () => {
    const g = { cols: 2, rows: 2 };
    expect(tiledFrameSize(g, 1)).toEqual({ width: 1920, height: 1632 });
    expect(tiledFrameSize(g, 2)).toEqual({ width: 3840, height: 3264 });
    expect(tileRect(3, g, 1)).toEqual({ x: 960, y: 800, width: 960, height: 800 });
    expect(tileRect(2, g, 2)).toEqual({ x: 0, y: 1600, width: 1920, height: 1600 });
  });

  it('picks the lowest H.264 level that fits, and refuses odd or oversized frames', () => {
    expect(h264Level(960, 832, 12)).toBe('3.1');
    expect(h264Level(1920, 1632, 12)).toBe('5.0');
    expect(h264Level(3840, 3264, 12)).toBe('6.0');
    expect(h264Level(1920, 1632, 60)).toBe('5.1'); // the macroblock rate, not the frame size, sets this one
    expect(() => h264Level(961, 832, 12)).toThrow(/even/);
    expect(() => h264Level(7680, 6464, 12)).toThrow(/beyond H.264 level 6.2/);
    expect(() => h264Level(19200, 1664, 12)).toThrow(/beyond/); // fits the frame size, too wide for any level
  });

  it('pastes an image into a larger one and refuses one that overhangs', () => {
    const dst = { width: 3, height: 2, data: Buffer.alloc(24) };
    const src = { width: 2, height: 1, data: Buffer.from([1, 2, 3, 4, 5, 6, 7, 8]) };
    pastePng(dst, src, { x: 1, y: 1 });
    expect([...dst.data.subarray(16)]).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
    expect([...dst.data.subarray(0, 16)].every((v) => v === 0)).toBe(true);
    expect(() => pastePng(dst, src, { x: 2, y: 0 })).toThrow(/outside/);
    expect(BG_RGB).toEqual([0xe8, 0xec, 0xf0]); // index.html's body background
  });
});
