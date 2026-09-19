import { createHash } from 'node:crypto';
import { expect, test, type Browser } from '@playwright/test';
// @ts-expect-error plain .mjs shared with scripts/film.mjs
import { captureFrames } from '../../scripts/film-lib.mjs';

const OPTS = { run: 'runs/s42', overlay: 'material', cam: 'iso', every: 1000, limit: 20 };

/** Captures the frame set in a fresh browser context and returns [tick, sha256 of the PNG] per frame. */
async function frameHashes(browser: Browser, baseURL: string): Promise<[number, string][]> {
  const ctx = await browser.newContext({ deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  const out: [number, string][] = [];
  await captureFrames(page, baseURL, OPTS, (_i: number, tick: number, png: Buffer) => {
    out.push([tick, createHash('sha256').update(png).digest('hex')]);
  });
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
