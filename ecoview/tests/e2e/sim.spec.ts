// Running the simulator on the bundle on screen (shot E4). The helper is a dev-only loopback server
// (scripts/sim-server.mjs); these tests start it themselves, so nothing here depends on how it was launched,
// and the first test asserts what the page does when it was not launched at all.
import { spawn, type ChildProcess } from 'node:child_process';
import { rm } from 'node:fs/promises';
import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import { open, trackErrors } from './helpers';

const BUNDLE = 'fixtures/capitol-world';
const GW = 512;
/** Lawn in the committed fixture, well clear of the Capitol's own roof (tests/e2e/edit.spec.ts). */
const LAWN = { gx: 140, gy: 32 };
const AT = LAWN.gx + GW * LAWN.gy;
/** Short enough that a whole run fits in a test, long enough that the trees grow and snapshots pile up. */
const TICKS = 200;
const HELPER = 'http://127.0.0.1:4174';

const url = (params: Record<string, string | number> = {}): string =>
  `/?world=${BUNDLE}&edit=1&ticks=${TICKS}&seed=42&${Object.entries(params).map(([k, v]) => `${k}=${v}`).join('&')}`;

/** Three cubes of roof on a patch of lawn: an edit the simulator's own output has to carry. */
async function buildOnTheLawn(page: Page): Promise<void> {
  await page.evaluate(([gx, gy]) => {
    const e = window.__ecoviewEdit!;
    e.slot('building');
    e.brush(4);
    e.aim(gx, gy);
    for (let i = 0; i < 3; i++) e.act('place');
  }, [LAWN.gx, LAWN.gy] as const);
  expect(await page.evaluate((i) => window.__ecoviewWorld!.building_h[i], AT)).toBeGreaterThan(0);
}

test.describe('with no sim helper listening', () => {
  test('the page says which piece is missing and keeps editing', async ({ page }) => {
    const errors = trackErrors(page);
    await open(page, url());
    expect(await page.locator('#sim').innerText()).toContain('press R');

    await page.keyboard.press('KeyR');
    await page.waitForFunction(() => window.__ecoviewSim?.state === 'absent');
    const line = await page.locator('#sim').innerText();
    expect(line).toContain('no sim helper');
    expect(line).toContain('npm run sim');
    await expect(page.locator('#sim')).toHaveClass(/bad/);

    // The editor is untouched: still the bundle on screen, still editable, and no error anywhere.
    expect(await page.locator('#status').innerText()).toContain(BUNDLE);
    await buildOnTheLawn(page);
    expect(await page.evaluate(() => window.__ecoviewEdit!.ops().length)).toBe(3);
    expect(errors).toEqual([]);
    expect(await page.evaluate(() => window.__ecoviewError)).toBeUndefined();
  });
});

test.describe('with the sim helper running', () => {
  let helper: ChildProcess | null = null;
  let root: string | null = null;

  test.beforeAll(async () => {
    helper = spawn(process.execPath, ['scripts/sim-server.mjs'], { stdio: ['ignore', 'pipe', 'pipe'] });
    helper.stderr?.on('data', (b: Buffer) => console.log(`[sim] ${b.toString().trim()}`));
    const until = Date.now() + 30_000;
    for (;;) {
      const health = await fetch(`${HELPER}/sim/health`).then((r) => r.json()).catch(() => null);
      if (health?.ok) {
        root = health.root as string;
        expect(health.binary, 'the ecosim release binary these tests run').not.toBe(null);
        return;
      }
      if (Date.now() > until) throw new Error('the sim helper did not start');
      await new Promise((r) => setTimeout(r, 200));
    }
  });

  test.afterAll(async () => {
    helper?.kill();
    // A killed helper cannot sweep up after itself, so the test does it: these runs are tens of megabytes.
    if (root) await rm(root, { recursive: true, force: true }).catch(() => {});
  });

  test('R runs the simulator on the edits and draws the run it made', async ({ page }) => {
    test.setTimeout(180_000);
    const errors = trackErrors(page);
    await open(page, url());
    await buildOnTheLawn(page);
    const before = await page.evaluate(() => {
      const b = window.__ecoviewWorld!.building_h;
      let sum = 0;
      for (const h of b) sum += h;
      return { sum, n: b.length };
    });

    await page.keyboard.press('KeyR');
    await page.waitForFunction(() => window.__ecoviewSim?.state === 'running' || window.__ecoviewSim?.state === 'done');
    await page.waitForFunction(() => window.__ecoviewSim?.state === 'done', null, { timeout: 150_000 });
    await page.waitForFunction(() => window.__ecoviewReady === true);

    const view = await page.evaluate(() => window.__ecoviewSim!);
    expect(view.path).toMatch(/^sim\/runs\/r[a-z0-9]+$/);
    expect(view.ticks).toBe(TICKS);
    const base = `/${view.path}`;

    // A run directory like any other: the page loaded it through the ordinary loader, at its own origin.
    const meta = await page.request.get(`${base}/meta.json`).then((r) => r.json());
    expect(meta.format_version).toBe(4);
    expect(meta.seed).toBe(42);
    expect(meta.snapshots.at(-1)).toBe(TICKS);
    expect(meta.world.ground_width).toBe(GW);

    // What is on screen is this run's entities, counted off the file the run wrote.
    const entities = await page.request
      .get(`${base}/snap_${String(TICKS).padStart(6, '0')}/entities.json`)
      .then((r) => r.json());
    expect(entities.length).toBeGreaterThan(0);
    expect(await page.locator('#status').innerText()).toBe(`${view.path} · seed 42 · ${entities.length} entities`);
    expect(await page.locator('#sim').innerText()).toContain(view.path);
    await expect(page.locator('#overlayrow')).toBeVisible();
    await expect(page.locator('#edit')).toBeHidden();

    // And it is a run of the edited bundle, not of the fixture: the roof went with it.
    const roofs = await page.request.get(`${base}/world/building_h.bin`).then(async (r) => {
      const buf = await r.body();
      return new Float32Array(buf.buffer, buf.byteOffset, buf.byteLength / 4);
    });
    expect(roofs.length).toBe(before.n);
    expect(roofs[AT]).toBeGreaterThan(0);
    let sum = 0;
    for (const h of roofs) sum += h;
    expect(sum).toBeCloseTo(before.sum, 1);

    // B goes back to the bundle, with the edits still on it and no reload.
    await page.keyboard.press('KeyB');
    await page.waitForFunction(() => window.__ecoviewReady === true);
    expect(await page.locator('#status').innerText()).toContain(BUNDLE);
    await expect(page.locator('#edit')).toBeVisible();
    expect(await page.evaluate((i) => window.__ecoviewWorld!.building_h[i], AT)).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.__ecoviewEdit!.ops().length)).toBe(3);
    expect(page.url()).toContain(`world=${BUNDLE}`);
    expect(errors).toEqual([]);
  });

  test('the run it serves is the only thing it serves', async ({ request }) => {
    test.setTimeout(120_000);
    const id = await postRun(request);
    expect((await request.get(`/sim/runs/${id}/meta.json`)).status()).toBe(200);

    // Every spelling of "somewhere else" is refused, including the world directory one level up, which is
    // the bundle this very run was made from and so is certainly there.
    const bs = '%5c';
    for (const rest of [
      '%2e%2e%2fworld%2fbundle.json',
      `%2e%2e${bs}world${bs}bundle.json`,
      '%2e%2e%2f%2e%2e%2f%2e%2e%2fpackage.json',
      'snap_000000%2f%2e%2e%2f%2e%2e%2fworld%2fmedium.u8',
    ]) {
      const res = await request.get(`/sim/runs/${id}/${rest}`);
      expect(res.status(), rest).toBe(400);
      expect((await res.json()).error, rest).toContain('outside the run directory');
    }

    // A segment that is only dots never reaches the handler at all: the URL parser resolves it away, and
    // what is left is a path with no run in it.
    expect((await request.get(`/sim/runs/${id}/%2e%2e`)).status()).toBe(404);

    // An id it did not issue is not a path either, so there is nothing to walk out of.
    expect((await request.get('/sim/runs/nosuchrun/meta.json')).status()).toBe(404);
    expect((await request.get(`/sim/runs/${id}/nosuchfile.bin`)).status()).toBe(404);
  });

  /** Posts the committed fixture straight to the endpoint and waits for the short run to finish. */
  async function postRun(request: APIRequestContext): Promise<string> {
    const names = ['bundle.json', 'trees.json', 'shrubs.json', 'pipes.json', 'ground_h.f32', 'medium.u8',
      'building_h.f32'];
    const files: Record<string, string> = {};
    for (const name of names) {
      const res = await request.get(`/${BUNDLE}/${name}`);
      expect(res.status(), name).toBe(200);
      files[name] = (await res.body()).toString('base64');
    }
    const started = await request.post('/sim/run', { data: { ticks: 10, seed: 1, files } }).then((r) => r.json());
    expect(started.ok, started.error).toBe(true);
    const until = Date.now() + 100_000;
    for (;;) {
      const s = await request.get(`/sim/status?id=${started.id}`).then((r) => r.json());
      if (s.state === 'done') return started.id as string;
      expect(s.state, s.error).toBe('running');
      if (Date.now() > until) throw new Error('the short run did not finish');
      await new Promise((r) => setTimeout(r, 200));
    }
  }
});
