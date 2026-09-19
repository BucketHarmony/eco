// Scenario tests for the fire, crowding and traits overlays (shot 12). Each rewrites the mini fixture's
// tick-0 snapshot in flight and reads exact overlay colours off the top camera, which is unlit.
import { expect, test, type Page } from '@playwright/test';
import { PNG } from 'pngjs';
import { open, trackErrors } from './helpers';

const SNAP = '**/fixtures/s42-mini/snap_000000';
type Json = Record<string, unknown>;

/** Rewrites one JSON file of the tick-0 snapshot. */
async function rewrite(page: Page, file: string, f: (v: Json[]) => Json[]): Promise<void> {
  await page.route(`${SNAP}/${file}`, async (route) => {
    const res = await route.fetch();
    await route.fulfill({ response: res, json: f((await res.json()) as Json[]) });
  });
}

/** Keeps the trees and replaces every animal with `animals`. */
const withAnimals = (animals: Json[]) => (ents: Json[]) => [...ents.filter((e) => e.kind === 'tree'), ...animals];

const hex = (h: string): number[] => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));

/**
 * Pixels of #view. The top camera draws 12.5 px per column with world x 0 at
 * pixel 80 and sim +y pointing up, so patch (px, py) covers x 80+100px.. and y 700-100py.. .
 */
async function pixels(page: Page): Promise<{ at(x: number, y: number): number[]; patchShare(p: number, rgb: number[]): number; count(rgb: number[]): number }> {
  const { width: w, data } = PNG.sync.read(await page.locator('#view').screenshot());
  const at = (x: number, y: number) => [...data.subarray(4 * (y * w + x), 4 * (y * w + x) + 3)];
  const near = (a: number[], b: number[]) => a.every((v, i) => Math.abs(v - b[i]) <= 2);
  return {
    at,
    patchShare(p, rgb) {
      const x0 = 80 + 100 * (p % 8);
      const y0 = 700 - 100 * (p >> 3);
      let n = 0;
      for (let y = y0; y < y0 + 100; y++) for (let x = x0; x < x0 + 100; x++) if (near(at(x, y), rgb)) n++;
      return n / 10000;
    },
    count(rgb) {
      let n = 0;
      for (let i = 0; i < data.length; i += 4) if (near([data[i], data[i + 1], data[i + 2]], rgb)) n++;
      return n;
    },
  };
}

/** Screen pixel at the centre of column (x, y) in the top camera. */
const columnPixel = (x: number, y: number): [number, number] => [Math.floor(80 + (x + 0.5) * 12.5), Math.floor(800 - (y + 0.5) * 12.5)];

// Patches 2–5 and 10, 11, 13 of the mini fixture are all soil with no trees at tick 0.
// Burnt ground comes from burnout events since the last snapshot (shot 16): the fixture is served as format 3 with
// an events.csv whose only burnout is patch 4's. Patches 4 and 5 are both bare, so only the event tells them apart.
test('fire overlay: burning patches orange by ticks left, burnt-out patches charcoal, the rest material', async ({ page }) => {
  const errors = trackErrors(page);
  await page.route('**/fixtures/s42-mini/meta.json', async (route) => {
    const res = await route.fetch();
    await route.fulfill({ response: res, json: { ...(await res.json()), format_version: 3 } });
  });
  await page.route('**/fixtures/s42-mini/events.csv', (route) => route.fulfill({
    contentType: 'text/csv',
    body: 'tick,kind,species,patch_x,patch_y,x,y,cause,detail\n0,ignition,,4,0,,,,\n0,burnout,,4,0,,,,\n',
  }));
  await rewrite(page, 'entities.json', withAnimals([]));
  await rewrite(page, 'patches.json', (ps) => ps.map((p, i) => {
    if (i === 2) return { ...p, burning_ticks_left: 3 };
    if (i === 3) return { ...p, burning_ticks_left: 1 };
    if (i === 4 || i === 5) return { ...p, grass: 0, shrub: 0, burning_ticks_left: 0 };
    return { ...p, burning_ticks_left: 0 };
  }));
  await open(page, '/?tick=0&overlay=fire&cam=top');
  const px = await pixels(page);
  expect(px.patchShare(2, hex('#ffb020')), 'patch 2, 3 ticks left').toBeGreaterThan(0.95);
  expect(px.patchShare(3, hex('#b3300a')), 'patch 3, 1 tick left').toBeGreaterThan(0.95);
  expect(px.patchShare(4, hex('#2b2b2b')), 'patch 4, burnt out').toBeGreaterThan(0.95);
  expect(px.patchShare(5, hex('#8b6b47')), 'patch 5, bare soil').toBeGreaterThan(0.95);
  for (const c of ['#ffb020', '#b3300a', '#2b2b2b']) {
    expect(px.patchShare(5, hex(c)), `patch 5 (bare, no burnout) has no ${c}`).toBe(0);
    expect(px.count(hex(c)), `${c} stays inside its patch`).toBeLessThanOrEqual(10000);
  }
  expect(errors).toEqual([]);
});

test('crowding overlay: grazers per patch from white to magenta, clamped at 32', async ({ page }) => {
  const errors = trackErrors(page);
  const herd = (n: number, x: number, y: number, z: number) =>
    Array.from({ length: n }, (_, i) => ({ id: 1000 * x + i, kind: 'grazer', x, y, z, energy: 60, age: 1, state: 'wander' }));
  const z = (await (await page.request.get('/fixtures/s42-mini/snap_000000/entities.json')).json()) as Json[];
  const z12 = z.find((e) => e.id === 12)!.z as number; // grazer 12 stands at (20, 5), in patch 2
  await rewrite(page, 'entities.json', withAnimals([...herd(40, 20, 5, z12), ...herd(16, 20, 13, z12)]));
  await open(page, '/?tick=0&overlay=crowding&cam=top');
  const px = await pixels(page);
  expect(px.patchShare(2, hex('#d81b9c')), '40 grazers clamp to full magenta').toBeGreaterThan(0.9);
  expect(px.patchShare(10, [236, 141, 206]), '16 grazers are halfway').toBeGreaterThan(0.9);
  expect(px.patchShare(3, [255, 255, 255]), 'no grazers is white').toBeGreaterThan(0.95);
  expect(px.count(hex('#d81b9c'))).toBeLessThan(10000);
  expect(errors).toEqual([]);
});

test('traits overlay: grazers blue below the default cost, red above, white without traits; hunters gray', async ({ page }) => {
  const errors = trackErrors(page);
  const ents = (await (await page.request.get('/fixtures/s42-mini/snap_000000/entities.json')).json()) as Json[];
  const byId = (id: number) => ents.find((e) => e.id === id)!;
  await rewrite(page, 'entities.json', withAnimals([
    { ...byId(12), energy_cost_mult: 0.5, flee_distance: 4, repro_threshold: 70 },
    { ...byId(13), energy_cost_mult: 1.5, flee_distance: 4, repro_threshold: 70 },
    { ...byId(14) },
    { ...byId(312), energy_cost_mult: 1.5 },
  ]));
  await open(page, '/?tick=0&overlay=traits&cam=top');
  const px = await pixels(page);
  const center = (id: number) => px.at(...columnPixel(byId(id).x as number, byId(id).y as number));
  expect(center(12), 'low-cost grazer').toEqual(hex('#1f5bff'));
  expect(center(13), 'high-cost grazer').toEqual(hex('#ff1f1f'));
  expect(center(14), 'grazer without traits').toEqual([255, 255, 255]);
  expect(center(312), 'hunter').toEqual(hex('#555555'));
  expect(px.count(hex('#f2d024')), 'no species-yellow grazers').toBe(0);
  expect(errors).toEqual([]);
});
