import { createHash } from 'node:crypto';
import { expect, test, type Browser } from '@playwright/test';
import { PNG } from 'pngjs';
// @ts-expect-error plain .mjs shared with scripts/film.mjs
import { captureFrames, captureTiledFrames } from '../../scripts/film-lib.mjs';

const OPTS = { run: 'runs/s42', overlay: 'material', cam: 'iso', every: 1000, limit: 20 };
const TILED = { run: 'runs/s42', tiles: 'material:iso,fire:top,crowding:top,traits:top', layout: '2x2', scale: 1, every: 1000, limit: 10 };

/** Captures the frame set in a fresh browser context and returns [tick, sha256 of the PNG] per frame. */
async function frameHashes(browser: Browser, baseURL: string, tiled = false): Promise<[number, string][]> {
  const ctx = await browser.newContext({ deviceScaleFactor: 1 });
  const out: [number, string][] = [];
  const onFrame = (_i: number, tick: number, png: Buffer) => {
    if (tiled && out.length === 0) {
      const { width, height } = PNG.sync.read(png);
      expect([width, height]).toEqual([1920, 1632]);
    }
    out.push([tick, createHash('sha256').update(png).digest('hex')]);
  };
  if (tiled) await captureTiledFrames(ctx, baseURL, TILED, onFrame);
  else await captureFrames(await ctx.newPage(), baseURL, OPTS, onFrame);
  await ctx.close();
  return out;
}

test('film: a 20-frame set is byte-identical across two captures', async ({ browser, baseURL }) => {
  const a = await frameHashes(browser, baseURL!);
  const b = await frameHashes(browser, baseURL!);
  expect(a.map(([t]) => t)).toEqual(Array.from({ length: 20 }, (_, i) => i * 1000));
  expect(b).toEqual(a);
  // The frames really change over the run, so identical sets aren't identical blanks.
  expect(new Set(a.map(([, h]) => h)).size).toBe(20);
});

test('film: a 10-frame 2x2 tiled set is byte-identical across two captures', async ({ browser, baseURL }) => {
  test.setTimeout(180_000);
  const a = await frameHashes(browser, baseURL!, true);
  const b = await frameHashes(browser, baseURL!, true);
  expect(a.map(([t]) => t)).toEqual(Array.from({ length: 10 }, (_, i) => i * 1000));
  expect(b).toEqual(a);
  expect(new Set(a.map(([, h]) => h)).size).toBe(10);
});
