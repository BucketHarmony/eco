import { expect, test, type Page } from '@playwright/test';

const FULL = '/?run=runs/s42';

/** Collects console errors and page errors for the whole test. */
function trackErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  page.on('pageerror', (e) => errors.push(e.message));
  return errors;
}

async function open(page: Page, url: string, timeout = 30_000): Promise<void> {
  await page.goto(url);
  await page.waitForFunction(() => window.__ecoviewReady === true, null, { timeout });
}

interface Stats {
  n: number;
  dark: number;
  light: number;
  mean: [number, number, number];
}

/** Screenshots #view only and measures its pixels in the page (no image library needed). */
async function viewStats(page: Page): Promise<Stats> {
  const png = await page.locator('#view').screenshot();
  return page.evaluate(async (b64) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width;
    c.height = img.height;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, c.width, c.height).data;
    let dark = 0;
    let light = 0;
    const sum = [0, 0, 0];
    for (let i = 0; i < d.length; i += 4) {
      const r = d[i], g = d[i + 1], b = d[i + 2];
      if (r < 60 && g < 60 && b < 60) dark++;
      if (r > 200 && g > 200 && b > 200) light++;
      sum[0] += r; sum[1] += g; sum[2] += b;
    }
    const n = d.length / 4;
    return { n, dark, light, mean: [sum[0] / n, sum[1] / n, sum[2] / n] as [number, number, number] };
  }, png.toString('base64'));
}

test('fixture loads, becomes ready within 5 s, no console errors', async ({ page }) => {
  const errors = trackErrors(page);
  await page.goto('/');
  await page.waitForFunction(() => window.__ecoviewReady === true, null, { timeout: 5_000 });
  await expect(page.locator('#readout')).toContainText('tick 0');
  const box = await page.locator('#view').boundingBox();
  expect(box).toEqual({ x: 0, y: 0, width: 960, height: 800 });
  const chart = await page.locator('#chart').boundingBox();
  expect(chart!.x).toBeGreaterThanOrEqual(960);
  expect(errors).toEqual([]);
});

test('bad format_version is a hard error', async ({ page }) => {
  await page.route('**/fixtures/s42-mini/meta.json', async (route) => {
    const res = await route.fetch();
    const meta = await res.json();
    await route.fulfill({ response: res, json: { ...meta, format_version: 2 } });
  });
  await page.goto('/');
  await page.waitForFunction(() => !!window.__ecoviewError, null, { timeout: 5_000 });
  await expect(page.locator('#status')).toContainText('format_version');
  expect(await page.evaluate(() => window.__ecoviewReady)).toBe(false);
});

test('03_light: canopy shade is dark and open ground is light', async ({ page }) => {
  await open(page, `${FULL}&tick=10000&overlay=light&cam=top`);
  const s = await viewStats(page);
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
