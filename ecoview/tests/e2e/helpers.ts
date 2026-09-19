import type { Page } from '@playwright/test';

export const FULL = '/?run=runs/s42';

/** Collects console errors and page errors for the whole test. */
export function trackErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  page.on('pageerror', (e) => errors.push(e.message));
  return errors;
}

export async function open(page: Page, url: string, timeout = 30_000): Promise<void> {
  await page.goto(url);
  await page.waitForFunction(() => window.__ecoviewReady === true, null, { timeout });
}

export interface Stats {
  n: number;
  dark: number;
  light: number;
  mean: [number, number, number];
}

/**
 * Screenshots #view only and measures its pixels in the page (no image library needed). With `worldOnly`, pixels
 * of the exact background colour #e8ecf0 (the letterbox around a world that doesn't fill the view) are left out.
 */
export async function viewStats(page: Page, worldOnly = false): Promise<Stats> {
  const png = await page.locator('#view').screenshot();
  return page.evaluate(async ([b64, only]) => {
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
    let n = 0;
    for (let i = 0; i < d.length; i += 4) {
      const r = d[i], g = d[i + 1], b = d[i + 2];
      if (only && r === 0xe8 && g === 0xec && b === 0xf0) continue;
      n++;
      if (r < 60 && g < 60 && b < 60) dark++;
      if (r > 200 && g > 200 && b > 200) light++;
      sum[0] += r; sum[1] += g; sum[2] += b;
    }
    return { n, dark, light, mean: [sum[0] / n, sum[1] / n, sum[2] / n] as [number, number, number] };
  }, [png.toString('base64'), worldOnly] as const);
}
