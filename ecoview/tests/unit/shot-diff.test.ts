import { describe, expect, it } from 'vitest';
// @ts-expect-error plain .mjs shared with scripts/shot-ref.mjs
import * as shotDiff from '../../scripts/shot-diff.mjs';
// @ts-expect-error plain .mjs shared with scripts/shot.mjs
import { VIEWPORT } from '../../scripts/chromium.mjs';

const { MAX_DIFF, REGIONS, compareRegions } = shotDiff;

type Rect = { name: string; x: number; y: number; width: number; height: number; crossPlatform: boolean };
type RegionResult = { name: string; px: number; frac: number; regionFrac: number; gated: boolean; ok: boolean };
type Png = { width: number; height: number; data: Buffer };

/** A screenshot-shaped image filled with one colour. */
function page(rgb: [number, number, number]): Png {
  const data = Buffer.alloc(VIEWPORT.width * VIEWPORT.height * 4);
  for (let i = 0; i < data.length; i += 4) {
    data[i] = rgb[0];
    data[i + 1] = rgb[1];
    data[i + 2] = rgb[2];
    data[i + 3] = 255;
  }
  return { width: VIEWPORT.width, height: VIEWPORT.height, data };
}

/** Paints `share` of the whole page's pixels black inside the named region, from its top-left corner. */
function smear(img: Png, name: string, share: number): Png {
  const r = (REGIONS as Rect[]).find((q) => q.name === name)!;
  const n = Math.round(img.width * img.height * share);
  if (n > r.width * r.height) throw new Error(`${share} of the page does not fit in ${name}`);
  for (let k = 0; k < n; k++) {
    const x = r.x + (k % r.width);
    const y = r.y + Math.floor(k / r.width);
    const i = (y * img.width + x) * 4;
    img.data[i] = 0;
    img.data[i + 1] = 0;
    img.data[i + 2] = 0;
  }
  return img;
}

/** A light page and black marks: far enough apart that pixelmatch's 0.1 threshold counts every one. */
const PAGE: [number, number, number] = [232, 236, 240];

const by = (result: { regions: RegionResult[] }, name: string) => result.regions.find((r) => r.name === name)!;

describe('screenshot regions', () => {
  it('tile the whole screenshot with no overlap', () => {
    const rects = REGIONS as Rect[];
    expect(rects.reduce((a, r) => a + r.width * r.height, 0)).toBe(VIEWPORT.width * VIEWPORT.height);
    expect(Math.min(...rects.map((r) => r.x))).toBe(0);
    expect(Math.max(...rects.map((r) => r.x + r.width))).toBe(VIEWPORT.width);
    for (const r of rects) {
      expect(r.y).toBe(0);
      expect(r.height).toBe(VIEWPORT.height);
    }
  });

  it('gate the view canvas everywhere and the sidebar only on the reference platform', () => {
    const rects = REGIONS as Rect[];
    expect(rects.filter((r) => r.crossPlatform).map((r) => r.name)).toEqual(['view']);
    expect(rects.filter((r) => !r.crossPlatform).map((r) => r.name)).toEqual(['sidebar']);
    // The gate is a share of the screenshot, so one region's budget is the whole page's old budget.
    const cur = smear(page(PAGE), 'view', MAX_DIFF * 0.99);
    expect(compareRegions(page(PAGE), cur, { samePlatform: true }).ok).toBe(true);
  });
});

describe('compareRegions', () => {
  it('passes identical screenshots on either platform', () => {
    for (const samePlatform of [true, false]) {
      const result = compareRegions(page(PAGE), page(PAGE), { samePlatform });
      expect(result.ok).toBe(true);
      expect(result.regions.map((r: RegionResult) => r.px)).toEqual([0, 0]);
    }
  });

  it('fails a view-canvas difference over the gate on both platforms', () => {
    const cur = smear(page(PAGE), 'view', MAX_DIFF * 2);
    for (const samePlatform of [true, false]) {
      const result = compareRegions(page(PAGE), cur, { samePlatform });
      expect(result.ok).toBe(false);
      expect(by(result, 'view').gated).toBe(true);
      expect(by(result, 'view').frac).toBeGreaterThan(MAX_DIFF);
      expect(by(result, 'sidebar').px).toBe(0);
    }
  });

  it('measures a sidebar difference on a foreign platform but does not fail on it', () => {
    // 2.1% of the page inside the sidebar: the Windows-against-Linux drift shot E5 measured.
    const cur = smear(page(PAGE), 'sidebar', 0.021);
    const foreign = compareRegions(page(PAGE), cur, { samePlatform: false });
    expect(foreign.ok).toBe(true);
    expect(by(foreign, 'sidebar').gated).toBe(false);
    expect(by(foreign, 'sidebar').frac).toBeCloseTo(0.021, 3);
    expect(by(foreign, 'sidebar').regionFrac).toBeCloseTo(0.084, 3);
    const home = compareRegions(page(PAGE), cur, { samePlatform: true });
    expect(home.ok).toBe(false);
    expect(by(home, 'sidebar').gated).toBe(true);
  });

  it('passes a view difference under the gate', () => {
    const cur = smear(page(PAGE), 'view', MAX_DIFF / 2);
    const result = compareRegions(page(PAGE), cur, { samePlatform: false });
    expect(result.ok).toBe(true);
    expect(by(result, 'view').frac).toBeLessThan(MAX_DIFF);
    expect(by(result, 'view').px).toBeGreaterThan(0);
  });

  it('marks the differing pixels of both regions in one full-size diff image', () => {
    const cur = smear(smear(page(PAGE), 'view', 0.01), 'sidebar', 0.01);
    const result = compareRegions(page(PAGE), cur, { samePlatform: true });
    expect(result.diff.width).toBe(VIEWPORT.width);
    expect(result.diff.height).toBe(VIEWPORT.height);
    const marked = (x0: number, x1: number) => {
      let n = 0;
      for (let y = 0; y < result.diff.height; y++) {
        for (let x = x0; x < x1; x++) {
          const i = (y * result.diff.width + x) * 4;
          if (result.diff.data[i] !== result.diff.data[i + 2]) n++;
        }
      }
      return n;
    };
    const view = (REGIONS as Rect[])[0];
    expect(marked(0, view.width)).toBe(by(result, 'view').px);
    expect(marked(view.width, VIEWPORT.width)).toBe(by(result, 'sidebar').px);
  });

  it('measures an ungated shot without failing on it, on either platform', () => {
    // The nine pictures of the simulation (scripts/shots.mjs, shot E6): still compared, still reported
    // with their percentages, never a red job.
    const cur = smear(page(PAGE), 'view', MAX_DIFF * 5);
    for (const samePlatform of [true, false]) {
      const shown = compareRegions(page(PAGE), cur, { samePlatform, gated: false });
      expect(shown.ok).toBe(true);
      expect(shown.regions.every((r: RegionResult) => !r.gated)).toBe(true);
      expect(by(shown, 'view').frac).toBeGreaterThan(MAX_DIFF);
      expect(by(shown, 'view').px).toBe(by(compareRegions(page(PAGE), cur, { samePlatform, gated: true }), 'view').px);
    }
  });

  it('gates by default, so a caller that names no gate keeps the old behaviour', () => {
    const cur = smear(page(PAGE), 'view', MAX_DIFF * 2);
    expect(compareRegions(page(PAGE), cur, { samePlatform: true }).ok).toBe(false);
    expect(compareRegions(page(PAGE), cur, { samePlatform: true, gated: true }).ok).toBe(false);
  });

  it('throws on a size mismatch instead of comparing', () => {
    const small = { width: 640, height: 400, data: Buffer.alloc(640 * 400 * 4) };
    expect(() => compareRegions(page([0, 0, 0]), small, { samePlatform: true })).toThrow(/size 640x400/);
  });
});
