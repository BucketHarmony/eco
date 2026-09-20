// Scenario tests: controls, URL state, error states and degenerate series, driven through the real page.
import { expect, test, type Page } from '@playwright/test';
import { PNG } from 'pngjs';
import { FULL, open, trackErrors, viewStats } from './helpers';

const snapDir = (tick: number) => `snap_${String(tick).padStart(6, '0')}`;

async function sliderIndex(page: Page): Promise<number> {
  return Number(await page.locator('#slider').inputValue());
}

async function chartMarker(page: Page): Promise<{ tick: number; x: number }> {
  return page.locator('#chart').evaluate((c) => ({ tick: Number(c.dataset.markerTick), x: Number(c.dataset.markerX) }));
}

test('slider scrub moves tick readout, entity count and chart marker together', async ({ page, request }) => {
  const errors = trackErrors(page);
  await open(page, FULL);
  let lastX = (await chartMarker(page)).x;
  for (const i of [50, 120, 200]) {
    await page.locator('#slider').fill(String(i));
    await page.waitForFunction(() => window.__ecoviewReady === true);
    const tick = i * 100;
    const ents = (await (await request.get(`/runs/s42/${snapDir(tick)}/entities.json`)).json()) as unknown[];
    await expect(page.locator('#readout')).toHaveText(`tick ${tick} / 20000`);
    await expect(page.locator('#status')).toContainText(`${ents.length} entities`);
    const m = await chartMarker(page);
    expect(m.tick).toBe(tick);
    expect(m.x).toBeGreaterThan(lastX);
    lastX = m.x;
  }
  expect(errors).toEqual([]);
});

test('Play advances >= 3 snapshots in 1 s and Pause stops it', async ({ page }) => {
  await open(page, FULL);
  await page.locator('#play').click();
  await expect(page.locator('#play')).toHaveText('Pause');
  const start = await sliderIndex(page);
  await page.waitForTimeout(1000);
  expect((await sliderIndex(page)) - start).toBeGreaterThanOrEqual(3);
  await page.locator('#play').click();
  await expect(page.locator('#play')).toHaveText('Play');
  await page.waitForFunction(() => window.__ecoviewReady === true);
  const paused = await sliderIndex(page);
  await page.waitForTimeout(800);
  expect(await sliderIndex(page)).toBe(paused);
  await expect(page.locator('#readout')).toHaveText(`tick ${paused * 100} / 20000`);
});

test('each camera changes the canvas mean colour', async ({ page }) => {
  await open(page, `${FULL}&tick=10000&overlay=material&cam=iso`);
  let prev = await viewStats(page);
  for (const cam of ['top', 'side', 'iso']) {
    await page.selectOption('#cam', cam);
    await page.waitForFunction(() => window.__ecoviewReady === true);
    expect(page.url()).toContain(`cam=${cam}`);
    const cur = await viewStats(page);
    const delta = Math.max(...cur.mean.map((v, i) => Math.abs(v - prev.mean[i])));
    expect(delta, `mean colour change switching to ${cam}`).toBeGreaterThan(5);
    prev = cur;
  }
});

test('every URL parameter is reflected in the DOM on load', async ({ page }) => {
  await open(page, '/?run=runs/s42&tick=10050&overlay=moisture&cam=side');
  await expect(page.locator('#overlay')).toHaveValue('moisture');
  await expect(page.locator('#cam')).toHaveValue('side');
  await expect(page.locator('#slider')).toHaveValue('100');
  await expect(page.locator('#readout')).toHaveText('tick 10000 / 20000');
  await expect(page.locator('#status')).toContainText('runs/s42 · seed 42');
  expect(await chartMarker(page)).toMatchObject({ tick: 10000 });
});

test('a format_version 2 fixture loads, ignores state.bin and shows forked_from', async ({ page }) => {
  const errors = trackErrors(page);
  const stateFetches: string[] = [];
  page.on('request', (r) => {
    if (r.url().endsWith('/state.bin')) stateFetches.push(r.url());
  });
  await page.route('**/fixtures/s42-mini/meta.json', async (route) => {
    const res = await route.fetch();
    const forked_from = { run: 'runs/s42', tick: 5000 };
    await route.fulfill({ response: res, json: { ...(await res.json()), format_version: 2, forked_from } });
  });
  await open(page, '/');
  const v2 = await page.locator('#view').screenshot();
  await expect(page.locator('#status')).not.toHaveClass(/error/);
  await expect(page.locator('#status')).toContainText('forked from runs/s42 @ tick 5000');
  expect(await page.evaluate(() => window.__ecoviewError)).toBeUndefined();
  await page.unrouteAll();
  await open(page, '/');
  await expect(page.locator('#status')).not.toContainText('forked from');
  expect((await page.locator('#view').screenshot()).equals(v2)).toBe(true);
  expect(stateFetches).toEqual([]);
  expect(errors).toEqual([]);
});

/** Traps every write to __ecoviewReady, so a test can tell whether it was ever set true. */
async function trapReady(page: Page): Promise<void> {
  await page.addInitScript(() => {
    let v = false;
    const w = window as unknown as { __readyEverTrue: boolean };
    w.__readyEverTrue = false;
    Object.defineProperty(window, '__ecoviewReady', {
      configurable: true,
      get: () => v,
      set: (x: boolean) => {
        v = x;
        if (x) w.__readyEverTrue = true;
      },
    });
  });
}

/** Serves the mini fixture's meta.json through `f`, loads the page and waits for the error state. */
async function openWithMeta(page: Page, f: (meta: Record<string, unknown>) => Record<string, unknown>): Promise<void> {
  await trapReady(page);
  await page.route('**/fixtures/s42-mini/meta.json', async (route) => {
    const res = await route.fetch();
    await route.fulfill({ response: res, json: f(await res.json()) });
  });
  await page.goto('/');
  await page.waitForFunction(() => !!window.__ecoviewError, null, { timeout: 5_000 });
  await expect(page.locator('#status')).toHaveClass(/error/);
}

async function readyEverTrue(page: Page): Promise<boolean> {
  await page.waitForTimeout(500);
  return page.evaluate(() => (window as unknown as { __readyEverTrue: boolean }).__readyEverTrue);
}

test('a format_version 5 fixture shows the error state and never sets __ecoviewReady', async ({ page }) => {
  await openWithMeta(page, (m) => ({ ...m, format_version: 5 }));
  await expect(page.locator('#status')).toContainText('unsupported format_version 5');
  expect(await readyEverTrue(page)).toBe(false);
});

test('meta.json dims that disagree with the .bin sizes show the error state', async ({ page }) => {
  // The fixture is 64×64×32; claim the 256×64 strip.
  await openWithMeta(page, (m) => ({ ...m, dims: { x: 256, y: 64, z: 32, patch: 8 } }));
  await expect(page.locator('#status')).toContainText(/Error: .*snap_000000\/\w+\.bin: expected (524288|16384) bytes, got (131072|4096)/);
  expect(await readyEverTrue(page)).toBe(false);
});

test('a format 3 run with no events.csv shows the error state', async ({ page }) => {
  await page.route('**/fixtures/s42-mini/events.csv', (route) => route.fulfill({ status: 404 }));
  await openWithMeta(page, (m) => ({ ...m, format_version: 3 }));
  await expect(page.locator('#status')).toContainText('events.csv: HTTP 404');
  expect(await readyEverTrue(page)).toBe(false);
});

test('the 256×64 strip fixture loads at format 3 and the top camera letterboxes it', async ({ page }) => {
  const errors = trackErrors(page);
  await open(page, '/?run=fixtures/s42-strip-mini&tick=100&overlay=moisture&cam=top');
  await expect(page.locator('#status')).toContainText('fixtures/s42-strip-mini · seed 42');
  const { width: w, height: h, data } = PNG.sync.read(await page.locator('#view').screenshot());
  // Rows with any pixel that isn't the background #e8ecf0.
  const rows: number[] = [];
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = 4 * (y * w + x);
      if (data[i] !== 0xe8 || data[i + 1] !== 0xec || data[i + 2] !== 0xf0) {
        rows.push(y);
        break;
      }
    }
  }
  // 256 columns fill the 960 px width at 3.75 px each, so the 64 rows are 240 px, centred: 280..519.
  expect(rows[0]).toBe(280);
  expect(rows.at(-1)).toBe(519);
  expect(rows.length).toBe(240);
  expect(errors).toEqual([]);
});

test('a missing snapshot file shows the error and keeps the last good frame', async ({ page }) => {
  await open(page, '/?tick=0&overlay=light&cam=top');
  const good = await page.locator('#view').screenshot();
  await page.route('**/fixtures/s42-mini/snap_000100/light.bin', (route) => route.fulfill({ status: 404 }));
  await page.locator('#slider').fill('1');
  await page.waitForFunction(() => !!window.__ecoviewError, null, { timeout: 5_000 });
  await expect(page.locator('#status')).toContainText('snap_000100/light.bin: HTTP 404');
  expect((await page.locator('#view').screenshot()).equals(good)).toBe(true);
  await expect(page.locator('#readout')).toHaveText('tick 0 / 100');
  await expect(page.locator('#slider')).toHaveValue('0');
  expect(page.url()).toContain('tick=0');
  expect(await page.evaluate(() => window.__ecoviewReady)).toBe(false);
});

test('a species reaching 0 draws a flat line at zero with no NaN in the DOM', async ({ page }) => {
  const ZERO_FROM = 30;
  await page.route('**/fixtures/s42-mini/series.csv', async (route) => {
    const res = await route.fetch();
    const lines = (await res.text()).trim().split('\n');
    const col = lines[0].split(',').indexOf('hunters');
    const out = lines.map((l, r) => {
      if (r <= ZERO_FROM) return l;
      const cells = l.split(',');
      cells[col] = '0';
      return cells.join(',');
    });
    await route.fulfill({ response: res, body: out.join('\n') + '\n' });
  });
  const errors = trackErrors(page);
  await open(page, '/');
  const result = await page.locator('#chart').evaluate((c: HTMLCanvasElement) => {
    const r = (JSON.parse(c.dataset.panels!) as { x: number; y: number; w: number; h: number }[])[0];
    const d = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data;
    const reddish = (x: number, y: number) => {
      const i = 4 * (y * c.width + x);
      return d[i] > 150 && d[i + 1] < 130 && d[i + 2] < 130;
    };
    const bottom = r.y + r.h - 1;
    const offZero: number[] = [];
    const above: number[] = [];
    // Columns well past the drop to zero (row 30 of 100) and clear of the frame.
    for (let x = Math.ceil(r.x + 0.45 * r.w); x < r.x + r.w - 3; x++) {
      if (![bottom - 2, bottom - 1, bottom].some((y) => reddish(x, y))) offZero.push(x);
      for (let y = r.y + 2; y < bottom - 4; y++) if (reddish(x, y)) above.push(x);
    }
    return { offZero, above };
  });
  expect(result.offZero, 'columns with no hunter pixel on the zero line').toEqual([]);
  expect(result.above, 'columns with hunter pixels above zero').toEqual([]);
  const html = await page.evaluate(() => document.documentElement.outerHTML);
  expect(html).not.toContain('NaN');
  expect(errors).toEqual([]);
});
