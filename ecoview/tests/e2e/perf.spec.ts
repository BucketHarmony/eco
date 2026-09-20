import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { FULL, open, trackErrors } from './helpers';

// Coarse gates on SwiftShader, a software GPU: they catch gross regressions, not real-world fps (DECISIONS.md, Shot 28).
// The draw gate is per surface column, so it means the same thing on any world size (DECISIONS.md, shot 16):
// shot 28's 250 ms over the 64x64 world's 4096 columns, which is 1000 ms on the 256x64 strip.
const MAX_DRAW_MS_PER_COLUMN = 250 / (64 * 64);
const MAX_STEP_MEDIAN_MS = 1000;
const OVERLAYS = ['material', 'light', 'moisture', 'fertility', 'temperature', 'fire', 'crowding', 'traits'];
const CAMS = ['iso', 'top'];
const FRAMES = 60;
const STEP_TICKS = Array.from({ length: 50 }, (_, i) => 10000 + i * 100);

// Timeout budget (DECISIONS.md, "E2 perf budget"). Until shot E2 one test carried the whole
// 8-overlay x 2-camera x 60-frame matrix plus the step loop under a flat 300 s, and shot G4b's
// heavier scene ran out of wall clock while every gate it asserts was still passing. Each test now
// derives its own budget from the work it does, on the same per-column basis as the draw gate:
//
//     timeout = SETUP_MS + units * budget_per_unit * RUNNER_SLACK
//
// A draw test's unit is one rendered frame and its budget is maxDrawMs, the gate the test already
// asserts; the step test's unit is one snapshot load and its budget is MAX_STEP_MEDIAN_MS. Deriving
// the budget from the gate keeps the property the old comment claimed: a run slow enough to breach a
// gate still reaches the assertion and fails on the gate, never on the clock. SETUP_MS covers the
// first page load and the per-pair overlay and camera switches, which are not measured frames, and
// RUNNER_SLACK covers the 10-20% by which the CI runner measures slower than the development
// machine, so the half-budget alarm below fires on scene weight and not on runner variance.
//
// Measured at the scene weight of shot G4b (runs/s42, 1541 trees), Windows + SwiftShader:
// draw/iso 140.7 s and draw/top 92.8 s of their 630 s budget, step 20.1 s of its 92.5 s — the
// thinnest margin is 4.5x, and each is well inside the half-budget alarm.
const SETUP_MS = 30_000;
const RUNNER_SLACK = 1.25;

/** Per-test measurements, written by each test and merged into perf/perf.json by afterAll. */
const PARTS = 'perf/parts';

interface Summary { median_ms: number; p95_ms: number; fps: number }
interface Part {
  elapsed_ms: number;
  timeout_ms: number;
  world?: string;
  chromium?: string;
  gl_renderer?: string;
  thresholds?: { draw_median_ms: number; step_median_ms: number };
  first_load_ms?: number;
  draw?: Record<string, Summary>;
  step?: Summary & { ticks: string };
}

function summarize(ms: number[]): Summary {
  const s = [...ms].sort((a, b) => a - b);
  const q = (p: number) => s[Math.min(s.length - 1, Math.ceil(p * s.length) - 1)];
  const r = (x: number) => Math.round(x * 100) / 100;
  const median = s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
  return { median_ms: r(median), p95_ms: r(q(0.95)), fps: r(1000 / median) };
}

function writePart(name: string, part: Part): void {
  mkdirSync(PARTS, { recursive: true });
  writeFileSync(`${PARTS}/${name}.json`, `${JSON.stringify(part, null, 2)}\n`);
  console.log(JSON.stringify({ part: name, ...part }));
}

function readParts(): Record<string, Part> {
  let names: string[] = [];
  try {
    names = readdirSync(PARTS).filter((f) => f.endsWith('.json'));
  } catch {
    return {};
  }
  return Object.fromEntries(names.map((f) => [f.slice(0, -'.json'.length), JSON.parse(readFileSync(`${PARTS}/${f}`, 'utf8')) as Part]));
}

/**
 * A test that spends more than half its derived budget fails by name. The next scene-weight increase
 * then reports its cause instead of a bare Playwright timeout, which is how shot G4b's failure read.
 */
function expectWithinHalfBudget(name: string, part: Part): void {
  const msg =
    `${name} used ${part.elapsed_ms} ms of its ${part.timeout_ms} ms budget, over half. The scene got heavier: ` +
    're-derive the budget in perf.spec.ts against the new measurement, or split the test again. Do not raise a gate.';
  expect(part.elapsed_ms, msg).toBeLessThanOrEqual(part.timeout_ms / 2);
}

test.describe('perf', () => {
  // Stale shards from an earlier run would otherwise merge into this run's perf.json.
  test.beforeAll(() => rmSync(PARTS, { recursive: true, force: true }));

  for (const cam of CAMS) {
    test(`perf: draw rate per overlay, cam=${cam}`, async ({ page, browser }) => {
      const t0 = Date.now();
      const errors = trackErrors(page);

      const dims = await page.request.get('/runs/s42/meta.json').then(async (r) => (await r.json()).dims as { x: number; y: number; z: number });
      const maxDrawMs = Math.round(MAX_DRAW_MS_PER_COLUMN * dims.x * dims.y);
      const timeoutMs = SETUP_MS + Math.round(OVERLAYS.length * FRAMES * maxDrawMs * RUNNER_SLACK);
      test.setTimeout(timeoutMs);

      const tLoad = Date.now();
      await open(page, `${FULL}&tick=10000&overlay=material&cam=${cam}`);
      const firstLoadMs = Date.now() - tLoad;

      // A pair over the gate ends the measurement there, so a gross regression fails on the gate, not on the test timeout.
      const draw: Record<string, Summary> = {};
      const slow = () => Object.values(draw).some((s) => s.median_ms > maxDrawMs);
      for (const overlay of OVERLAYS) {
        if (slow()) break;
        await page.evaluate(() => { window.__ecoviewReady = false; });
        await page.selectOption('#cam', cam);
        await page.selectOption('#overlay', overlay);
        await page.waitForFunction(() => window.__ecoviewReady === true);
        expect(page.url()).toContain(`overlay=${overlay}`);
        expect(page.url()).toContain(`cam=${cam}`);
        draw[`${overlay}/${cam}`] = summarize(await page.evaluate((n) => window.__ecoviewBench(n), FRAMES));
      }

      const gl = await page.evaluate(() => {
        const ctx = document.createElement('canvas').getContext('webgl2')!;
        const ext = ctx.getExtension('WEBGL_debug_renderer_info');
        return ext ? String(ctx.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : String(ctx.getParameter(ctx.RENDERER));
      });
      const part: Part = {
        elapsed_ms: Date.now() - t0,
        timeout_ms: timeoutMs,
        world: `${dims.x}x${dims.y}x${dims.z}`,
        chromium: browser.version(),
        gl_renderer: gl,
        thresholds: { draw_median_ms: maxDrawMs, step_median_ms: MAX_STEP_MEDIAN_MS },
        first_load_ms: firstLoadMs,
        draw,
      };
      writePart(`draw-${cam}`, part);

      expect(errors).toEqual([]);
      for (const [k, s] of Object.entries(draw)) expect(s.median_ms, `draw ${k}`).toBeLessThanOrEqual(maxDrawMs);
      expect(Object.keys(draw)).toHaveLength(OVERLAYS.length);
      expectWithinHalfBudget(`draw/${cam}`, part);
    });
  }

  test('perf: step rate through consecutive snapshots', async ({ page }) => {
    const t0 = Date.now();
    const errors = trackErrors(page);
    const timeoutMs = SETUP_MS + Math.round(STEP_TICKS.length * MAX_STEP_MEDIAN_MS * RUNNER_SLACK);
    test.setTimeout(timeoutMs);

    // The default view, then 50 consecutive snapshots as Play would.
    await open(page, `${FULL}&tick=${STEP_TICKS[0]}&overlay=material&cam=iso`);
    const stepMs = await page.evaluate(async (ticks) => {
      const out: number[] = [];
      for (const t of ticks) {
        const t0 = performance.now();
        window.__ecoviewGoto(t);
        while (!window.__ecoviewReady) await new Promise((r) => setTimeout(r, 0));
        out.push(performance.now() - t0);
      }
      return out;
    }, STEP_TICKS);
    expect(await page.locator('#readout').textContent()).toContain(`tick ${STEP_TICKS.at(-1)}`);

    const part: Part = {
      elapsed_ms: Date.now() - t0,
      timeout_ms: timeoutMs,
      step: { ticks: `${STEP_TICKS[0]}-${STEP_TICKS.at(-1)}`, ...summarize(stepMs) },
    };
    writePart('step', part);

    expect(errors).toEqual([]);
    expect(part.step!.median_ms, 'step').toBeLessThanOrEqual(MAX_STEP_MEDIAN_MS);
    expectWithinHalfBudget('step', part);
  });

  // perf/perf.json keeps the shape it had before the split, so its numbers stay comparable across
  // shots. Merging in afterAll rather than in a final test means it is written even when a gate
  // fails, which CI's upload-artifact step requires (if-no-files-found: error).
  test.afterAll(() => {
    const parts = readParts();
    const head = parts['draw-iso'] ?? parts['draw-top'];
    const draw: Record<string, Summary> = {};
    for (const cam of CAMS) Object.assign(draw, parts[`draw-${cam}`]?.draw ?? {});
    const report = {
      run: 'runs/s42',
      world: head?.world ?? null,
      platform: `${process.platform}-${process.arch}`,
      chromium: head?.chromium ?? null,
      gl_renderer: head?.gl_renderer ?? null,
      frames_per_draw: FRAMES,
      thresholds: head?.thresholds ?? null,
      first_load_ms: head?.first_load_ms ?? null,
      step: parts['step']?.step ?? null,
      draw,
    };
    mkdirSync('perf', { recursive: true });
    writeFileSync('perf/perf.json', `${JSON.stringify(report, null, 2)}\n`);
    console.log(JSON.stringify(report));
  });
});
