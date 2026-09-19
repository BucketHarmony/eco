import { mkdirSync, writeFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { FULL, open, trackErrors } from './helpers';

// Coarse gates on SwiftShader, a software GPU: they catch gross regressions, not real-world fps (DECISIONS.md, Shot 28).
const MAX_DRAW_MEDIAN_MS = 250;
const MAX_STEP_MEDIAN_MS = 1000;
const OVERLAYS = ['material', 'light', 'moisture', 'fertility', 'temperature', 'fire', 'crowding', 'traits'];
const CAMS = ['iso', 'top'];
const FRAMES = 60;
const STEP_TICKS = Array.from({ length: 50 }, (_, i) => 10000 + i * 100);

interface Summary { median_ms: number; p95_ms: number; fps: number }

function summarize(ms: number[]): Summary {
  const s = [...ms].sort((a, b) => a - b);
  const q = (p: number) => s[Math.min(s.length - 1, Math.ceil(p * s.length) - 1)];
  const r = (x: number) => Math.round(x * 100) / 100;
  const median = s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
  return { median_ms: r(median), p95_ms: r(q(0.95)), fps: r(1000 / median) };
}

test('perf: draw rate per overlay and camera, step rate and first load', async ({ page, browser }) => {
  test.setTimeout(300_000);
  const errors = trackErrors(page);

  const t0 = Date.now();
  await open(page, `${FULL}&tick=10000&overlay=material&cam=iso`);
  const firstLoadMs = Date.now() - t0;

  // A pair over the gate ends the measurement there, so a gross regression fails on the gate, not on the test timeout.
  const draw: Record<string, Summary> = {};
  const slow = () => Object.values(draw).some((s) => s.median_ms > MAX_DRAW_MEDIAN_MS);
  measure: for (const cam of CAMS) {
    for (const overlay of OVERLAYS) {
      if (slow()) break measure;
      await page.evaluate(() => { window.__ecoviewReady = false; });
      await page.selectOption('#cam', cam);
      await page.selectOption('#overlay', overlay);
      await page.waitForFunction(() => window.__ecoviewReady === true);
      expect(page.url()).toContain(`overlay=${overlay}`);
      expect(page.url()).toContain(`cam=${cam}`);
      draw[`${overlay}/${cam}`] = summarize(await page.evaluate((n) => window.__ecoviewBench(n), FRAMES));
    }
  }

  // Back to the default view, then step through 50 consecutive snapshots as Play would.
  await page.selectOption('#overlay', 'material');
  await page.selectOption('#cam', 'iso');
  await page.waitForFunction(() => window.__ecoviewReady === true);
  const stepMs = slow() ? [] : await page.evaluate(async (ticks) => {
    const out: number[] = [];
    for (const t of ticks) {
      const t0 = performance.now();
      window.__ecoviewGoto(t);
      while (!window.__ecoviewReady) await new Promise((r) => setTimeout(r, 0));
      out.push(performance.now() - t0);
    }
    return out;
  }, STEP_TICKS);
  if (stepMs.length) expect(await page.locator('#readout').textContent()).toContain(`tick ${STEP_TICKS.at(-1)}`);

  const gl = await page.evaluate(() => {
    const ctx = document.createElement('canvas').getContext('webgl2')!;
    const ext = ctx.getExtension('WEBGL_debug_renderer_info');
    return ext ? String(ctx.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : String(ctx.getParameter(ctx.RENDERER));
  });
  const step = stepMs.length ? summarize(stepMs) : null;
  const report = {
    run: 'runs/s42',
    world: '64x64x32',
    platform: `${process.platform}-${process.arch}`,
    chromium: browser.version(),
    gl_renderer: gl,
    frames_per_draw: FRAMES,
    thresholds: { draw_median_ms: MAX_DRAW_MEDIAN_MS, step_median_ms: MAX_STEP_MEDIAN_MS },
    first_load_ms: firstLoadMs,
    step: step && { ticks: `${STEP_TICKS[0]}-${STEP_TICKS.at(-1)}`, ...step },
    draw,
  };
  mkdirSync('perf', { recursive: true });
  writeFileSync('perf/perf.json', `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report));

  expect(errors).toEqual([]);
  for (const [k, s] of Object.entries(draw)) expect(s.median_ms, `draw ${k}`).toBeLessThanOrEqual(MAX_DRAW_MEDIAN_MS);
  expect(Object.keys(draw)).toHaveLength(OVERLAYS.length * CAMS.length);
  expect(step?.median_ms, 'step').toBeLessThanOrEqual(MAX_STEP_MEDIAN_MS);
});
