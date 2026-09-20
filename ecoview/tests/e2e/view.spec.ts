import { expect, test } from '@playwright/test';
import { FULL, open, trackErrors, viewStats } from './helpers';
// @ts-expect-error plain .mjs shared with scripts/shot-ref.mjs
import { REGIONS } from '../../scripts/shot-diff.mjs';

type Rect = { name: string; x: number; y: number; width: number; height: number };
const region = (name: string): Rect => (REGIONS as Rect[]).find((r) => r.name === name)!;

test('fixture loads, becomes ready within 5 s, no console errors', async ({ page }) => {
  const errors = trackErrors(page);
  await page.goto('/');
  await page.waitForFunction(() => window.__ecoviewReady === true, null, { timeout: 5_000 });
  await expect(page.locator('#readout')).toContainText('tick 0');
  // shot:check gates these two rectangles separately, so the page had better still be laid out that
  // way: the canvas where the view region says, the sidebar's chart inside the sidebar region (shot E5).
  const view = region('view');
  const box = await page.locator('#view').boundingBox();
  expect(box).toEqual({ x: view.x, y: view.y, width: view.width, height: view.height });
  const sidebar = region('sidebar');
  const aside = await page.locator('#sidebar').boundingBox();
  expect(aside).toEqual({ x: sidebar.x, y: sidebar.y, width: sidebar.width, height: sidebar.height });
  const chart = await page.locator('#chart').boundingBox();
  expect(chart!.x).toBeGreaterThanOrEqual(sidebar.x);
  expect(errors).toEqual([]);
});

test('03_light: canopy shade is dark and open ground is light', async ({ page }) => {
  await open(page, `${FULL}&tick=10000&overlay=light&cam=top`);
  // Measured over the world, not the letterbox around the strip (DECISIONS.md, shot 16).
  const s = await viewStats(page, true);
  expect(s.n).toBe(960 * 240);
  console.log(`03_light: dark ${(s.dark / s.n * 100).toFixed(1)}%, light ${(s.light / s.n * 100).toFixed(1)}%`);
  expect(s.dark / s.n).toBeGreaterThanOrEqual(0.05);
  expect(s.light / s.n).toBeGreaterThanOrEqual(0.4);
});

test('switching overlay changes mean view color by > 20 in some channel', async ({ page }) => {
  await open(page, `${FULL}&tick=10000&overlay=material&cam=top`);
  const a = await viewStats(page);
  await open(page, `${FULL}&tick=10000&overlay=moisture&cam=top`);
  const b = await viewStats(page);
  const delta = Math.max(...a.mean.map((v, i) => Math.abs(v - b.mean[i])));
  expect(delta).toBeGreaterThan(20);
});

test('overlay select control re-renders and updates the URL', async ({ page }) => {
  await open(page, `/?tick=0&overlay=material&cam=top`);
  const a = await viewStats(page);
  await page.selectOption('#overlay', 'fertility');
  await page.waitForFunction(() => window.__ecoviewReady === true);
  expect(page.url()).toContain('overlay=fertility');
  const b = await viewStats(page);
  const delta = Math.max(...a.mean.map((v, i) => Math.abs(v - b.mean[i])));
  expect(delta).toBeGreaterThan(20);
});

test('chart is non-blank at tick 20000', async ({ page }) => {
  await open(page, `${FULL}&tick=20000`);
  await expect(page.locator('#readout')).toContainText('tick 20000');
  const frac = await page.evaluate(() => {
    const c = document.getElementById('chart') as HTMLCanvasElement;
    const d = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data;
    let non = 0;
    for (let i = 0; i < d.length; i += 4) if (d[i] !== 255 || d[i + 1] !== 255 || d[i + 2] !== 255) non++;
    return non / (d.length / 4);
  });
  expect(frac).toBeGreaterThan(0.01);
});

test('scrubbing the full run keeps >= 40 rAF callbacks in 2 s', async ({ page }) => {
  const errors = trackErrors(page);
  await open(page, FULL);
  const frames = await page.evaluate(async () => {
    const slider = document.getElementById('slider') as HTMLInputElement;
    slider.value = String(Math.floor(Number(slider.max) / 2));
    slider.dispatchEvent(new Event('input', { bubbles: true }));
    let count = 0;
    const t0 = performance.now();
    await new Promise<void>((resolve) => {
      const tick = () => {
        count++;
        if (performance.now() - t0 < 2000) requestAnimationFrame(tick);
        else resolve();
      };
      requestAnimationFrame(tick);
    });
    return count;
  });
  expect(frames).toBeGreaterThanOrEqual(40);
  await page.waitForFunction(() => window.__ecoviewReady === true);
  await expect(page.locator('#readout')).toContainText('tick 10000');
  expect(errors).toEqual([]);
});

test('scrubbing end to end renders every tenth snapshot without errors', async ({ page }) => {
  const errors = trackErrors(page);
  await open(page, `${FULL}&overlay=light&cam=side`);
  const max = Number(await page.locator('#slider').getAttribute('max'));
  expect(max).toBe(200);
  for (let i = 0; i <= max; i += 20) {
    await page.locator('#slider').fill(String(i));
    await page.waitForFunction(() => window.__ecoviewReady === true);
    await expect(page.locator('#readout')).toContainText(`tick ${i * 100} `);
  }
  expect(errors).toEqual([]);
});
