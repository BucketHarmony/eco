// Edit mode in the browser (shot E1): every assertion reads the live grids through window.__ecoviewWorld,
// which is the same memory a save writes out, so a passing test is a bundle ecosim could load.
import { readFile } from 'node:fs/promises';
import { expect, test, type APIRequestContext, type Download, type Page } from '@playwright/test';
import { trackErrors, open } from './helpers';

const BUNDLE = 'fixtures/capitol-world';
/** The Capitol bundle: 512 x 512 ground cells of 0.5 m. Read from bundle.json in the first test. */
const GW = 512;
const CELL = 0.5;
const CENTRE = { x: 480, y: 400 };

/** Cells chosen off the committed fixture: lawn with a slope, and asphalt with room for a radius-5 disc. */
const LAWN = { gx: 140, gy: 32 };
const SLOPE = { gx: 148, gy: 32 };
const ASPHALT = { gx: 8, gy: 212 };

const url = (params: Record<string, string | number>): string =>
  `/?world=${BUNDLE}&${Object.entries(params).map(([k, v]) => `${k}=${v}`).join('&')}`;

const at = (c: { gx: number; gy: number }): number => c.gx + GW * c.gy;

/** The cube a height sits in, the renderer's rule (src/world.ts `levelOf`), repeated here on purpose. */
const level = (h: number): number => Math.floor(h / CELL + 1e-6);

/** The hotbar, in key order: Digit1 is the first (src/edit.ts SLOTS). */
const SLOTS = ['lawn', 'bed', 'concrete', 'asphalt', 'water', 'gravel', 'ground', 'building'] as const;

/** Cells within `r - 1` of the centre, the brush's shape, computed here rather than asked of the page. */
function disc(gx: number, gy: number, r: number): number[] {
  const reach = r - 1;
  const out: number[] = [];
  for (let dy = -reach; dy <= reach; dy++) {
    for (let dx = -reach; dx <= reach; dx++) {
      if (dx * dx + dy * dy <= reach * reach) out.push(gx + dx + GW * (gy + dy));
    }
  }
  return out.sort((a, b) => a - b);
}

interface Cell {
  ground_h: number;
  medium: number;
  building_h: number;
}

const cell = (page: Page, i: number): Promise<Cell> =>
  page.evaluate((k) => window.__ecoviewEdit!.cell(k), i);

/** A whole-grid fingerprint: the two height sums and a hash of the medium codes, in index order. */
const digest = (page: Page): Promise<{ ground: number; building: number; medium: number; n: number }> =>
  page.evaluate(() => {
    const w = window.__ecoviewWorld!;
    let ground = 0;
    let building = 0;
    let medium = 0;
    for (let i = 0; i < w.ground_h.length; i++) {
      ground += w.ground_h[i];
      building += w.building_h[i];
      medium = (medium * 31 + w.medium[i]) | 0;
    }
    return { ground, building, medium, n: w.ground_h.length };
  });

const media = (page: Page): Promise<string[]> => page.evaluate(() => window.__ecoviewWorld!.meta.media);

const panel = (page: Page): Promise<string> => page.locator('#editinfo').innerText();

/** Collects the files one Ctrl+S writes; a save is eight downloads, seven bundle files and the change list. */
async function save(page: Page, n = 8): Promise<Map<string, Buffer>> {
  const seen: Download[] = [];
  const done = new Promise<void>((resolve) => {
    page.on('download', (d) => {
      seen.push(d);
      if (seen.length === n) resolve();
    });
  });
  await page.keyboard.press('Control+s');
  await done;
  const out = new Map<string, Buffer>();
  for (const d of seen) {
    const path = await d.path();
    out.set(d.suggestedFilename(), await readFile(path!));
  }
  return out;
}

const fixtureFile = async (request: APIRequestContext, name: string): Promise<Buffer> =>
  (await request.get(`/${BUNDLE}/${name}`)).body();

// Every act renders the 262144-box ground once, which swiftshader takes a couple of seconds over.
test.describe.configure({ timeout: 180_000 });

test.describe('edit mode on a world bundle', () => {
  test('loads the bundle the sync script copied, at the cell size bundle.json names', async ({ page, request }) => {
    const errors = trackErrors(page);
    await open(page, url({ cam: 'iso', edit: 1 }));
    const json = (await (await request.get(`/${BUNDLE}/bundle.json`)).json()) as Record<string, unknown>;
    expect(json.format).toBe('ecosim-world-bundle');
    expect(json.version).toBe(2);
    // The horizontal step is the bundle's, never a constant in the renderer.
    expect(json.ground_cell_m).toBe(CELL);
    expect(json.ground_width).toBe(GW);
    const w = await page.evaluate(() => {
      const g = window.__ecoviewWorld!;
      return { gw: g.gw, gd: g.gd, cell: g.cell, n: g.ground_h.length, media: g.meta.media };
    });
    expect(w).toEqual({ gw: GW, gd: GW, cell: CELL, n: GW * GW, media: json.media });
    await expect(page.locator('#status')).toContainText('256×256 m over 512×512 ground cells at 0.5 m');
    // The edit panel replaces the run controls, and says what the half-metre step means for the sim.
    await expect(page.locator('#edit')).toBeVisible();
    await expect(page.locator('#overlayrow')).toBeHidden();
    await expect(page.locator('#playrow')).toBeHidden();
    await expect(page.locator('#cross')).toBeVisible();
    await expect(page.locator('#editnote')).toContainText('0.5 m');
    await expect(page.locator('#editnote')).toContainText('1 m ecology column');
    expect(errors).toEqual([]);
  });

  test('places one cube on one cell and touches nothing else', async ({ page }) => {
    const errors = trackErrors(page);
    await open(page, url({ cam: 'iso', edit: 1, slot: 'asphalt', brush: 1, aim: `${LAWN.gx},${LAWN.gy}` }));
    const names = await media(page);
    const before = await cell(page, at(LAWN));
    const d0 = await digest(page);
    expect(names[before.medium]).toBe('lawn');
    // A right click places; the crosshair is pinned by ?aim=, so the click needs no aiming.
    await page.mouse.click(CENTRE.x, CENTRE.y, { button: 'right' });
    const after = await cell(page, at(LAWN));
    expect(names[after.medium]).toBe('asphalt');
    // Every slot adds a cube now (shot E3), so this one is a road block one cube above the lawn.
    expect(level(after.ground_h)).toBe(level(before.ground_h) + 1);
    expect(after.ground_h).toBeCloseTo((level(before.ground_h) + 1) * CELL, 5); // and on the lattice
    expect(after.building_h).toBe(before.building_h);
    const d1 = await digest(page);
    expect(d1.ground - d0.ground).toBeCloseTo(after.ground_h - before.ground_h, 4); // that cell and no other
    expect(d1.building).toBe(d0.building);
    expect(d1.medium).not.toBe(d0.medium);
    // Exactly one cell in the whole grid changed, and it is the one the crosshair named.
    expect(await page.evaluate(() => window.__ecoviewEdit!.changed())).toEqual([at(LAWN)]);
    const ops = await page.evaluate(() => window.__ecoviewEdit!.ops());
    expect(ops).toHaveLength(1);
    expect(ops[0].kind).toBe('place');
    expect(ops[0].cells).toHaveLength(1);
    expect(ops[0].cells[0].i).toBe(at(LAWN));
    // The panel says which cell, and marks the session dirty.
    expect(await panel(page)).toContain(`(${LAWN.gx}, ${LAWN.gy}) asphalt`);
    expect(await panel(page)).toContain('unsaved');
    await expect(page.locator('#editinfo')).toHaveClass(/dirty/);
    expect(errors).toEqual([]);
  });

  test('raises a cell one cube a click, so two clicks are one sim voxel here', async ({ page }) => {
    await open(page, url({ cam: 'iso', edit: 1, slot: 'ground', brush: 1, aim: `${LAWN.gx},${LAWN.gy}` }));
    const before = await cell(page, at(LAWN));
    expect(await page.evaluate(() => window.__ecoviewEdit!.act('place'))).toBe(true);
    const mid = await cell(page, at(LAWN));
    // The first click snaps the LiDAR height onto the cube lattice as it adds its cube.
    expect(level(mid.ground_h)).toBe(level(before.ground_h) + 1);
    expect(mid.ground_h).toBeCloseTo((level(before.ground_h) + 1) * CELL, 5);
    expect(await page.evaluate(() => window.__ecoviewEdit!.act('place'))).toBe(true);
    const after = await cell(page, at(LAWN));
    expect(after.ground_h - mid.ground_h).toBeCloseTo(CELL, 5); // and every click after it is one cube
    expect(level(after.ground_h)).toBe(level(before.ground_h) + 2);
    expect(after.medium).toBe(before.medium);
    expect(after.building_h).toBe(before.building_h);
    // A neighbour outside the radius-1 brush is untouched.
    expect((await cell(page, at(LAWN) + 1)).ground_h).not.toBe(after.ground_h);
    expect(await panel(page)).toContain(`ground ${after.ground_h.toFixed(2)} m`);
  });

  test('paints exactly the disc a radius-5 brush covers', async ({ page }) => {
    await open(page, url({ cam: 'iso', edit: 1, slot: 'lawn', brush: 5, aim: `${ASPHALT.gx},${ASPHALT.gy}` }));
    const names = await media(page);
    const want = disc(ASPHALT.gx, ASPHALT.gy, 5);
    expect(want).toHaveLength(49);
    const asphalt = names.indexOf('asphalt');
    const lawn = names.indexOf('lawn');
    const codes = await page.evaluate((cells) => cells.map((i) => window.__ecoviewWorld!.medium[i]), want);
    expect(new Set(codes)).toEqual(new Set([asphalt])); // the fixture is all asphalt here
    expect(await page.evaluate(() => window.__ecoviewEdit!.act('place'))).toBe(true);
    expect(await page.evaluate(() => window.__ecoviewEdit!.changed())).toEqual(want);
    const after = await page.evaluate((cells) => cells.map((i) => window.__ecoviewWorld!.medium[i]), want);
    expect(new Set(after)).toEqual(new Set([lawn]));
    // The four cells just off the disc's axes are still asphalt: the brush is a disc, not a square.
    const outside = [
      ASPHALT.gx + 5 + GW * ASPHALT.gy, ASPHALT.gx - 5 + GW * ASPHALT.gy,
      ASPHALT.gx + GW * (ASPHALT.gy + 5), ASPHALT.gx + 4 + GW * (ASPHALT.gy + 4),
    ];
    const edge = await page.evaluate((cells) => cells.map((i) => window.__ecoviewWorld!.medium[i]), outside);
    expect(new Set(edge)).toEqual(new Set([asphalt]));
    expect(await panel(page)).toContain('place 49 cells');
  });

  test('flattens a disc onto the height of the cell it is aimed at', async ({ page }) => {
    await open(page, url({ cam: 'iso', edit: 1, slot: 'lawn', brush: 3, aim: `${SLOPE.gx},${SLOPE.gy}` }));
    const want = disc(SLOPE.gx, SLOPE.gy, 3);
    expect(want).toHaveLength(13);
    const read = (cells: number[]): Promise<[number, number][]> =>
      page.evaluate((cs) => cs.map((i) => [window.__ecoviewWorld!.ground_h[i], window.__ecoviewWorld!.medium[i]] as [number, number]), cells);
    const before = await read(want);
    expect(new Set(before.map((c) => c[0])).size).toBeGreaterThan(5); // a real slope to level
    const centre = (await cell(page, at(SLOPE))).ground_h;
    // Ctrl and the left button is the flatten, the same as the hand does it.
    await page.keyboard.down('Control');
    await page.mouse.click(CENTRE.x, CENTRE.y);
    await page.keyboard.up('Control');
    const after = await read(want);
    expect(new Set(after.map((c) => c[0]))).toEqual(new Set([centre]));
    expect(after.map((c) => c[1])).toEqual(before.map((c) => c[1])); // media untouched
    // And only that disc moved.
    expect(await page.evaluate(() => window.__ecoviewEdit!.changed()))
      .toEqual(want.filter((_, k) => before[k][0] !== centre));
  });

  test('undoes and redoes three edits in a row', async ({ page }) => {
    await open(page, url({ cam: 'iso', edit: 1, slot: 'asphalt', brush: 1, aim: `${LAWN.gx},${LAWN.gy}` }));
    const i = at(LAWN);
    const start = await cell(page, i);
    const d0 = await digest(page);
    await page.evaluate(() => window.__ecoviewEdit!.act('place')); // asphalt
    await page.keyboard.press('Digit7'); // ground
    await page.evaluate(() => window.__ecoviewEdit!.act('place')); // + 0.5 m
    await page.evaluate(() => window.__ecoviewEdit!.act('remove')); // - 0.5 m, and back again
    const edited = await cell(page, i);
    expect(edited.medium).not.toBe(start.medium);
    expect(await page.evaluate(() => window.__ecoviewEdit!.ops())).toHaveLength(3);
    for (let k = 0; k < 3; k++) expect(await page.evaluate(() => window.__ecoviewEdit!.undo())).toBe(true);
    expect(await cell(page, i)).toEqual(start);
    expect(await digest(page)).toEqual(d0);
    expect(await page.evaluate(() => window.__ecoviewEdit!.changed())).toEqual([]);
    expect(await page.evaluate(() => window.__ecoviewEdit!.undo())).toBe(false);
    expect(await panel(page)).toContain('nothing to undo');
    // Ctrl+Y three times puts every edit back, in order.
    for (let k = 0; k < 3; k++) await page.keyboard.press('Control+y');
    expect(await cell(page, i)).toEqual(edited);
    expect(await panel(page)).toContain('3 undo (Ctrl-Z)');
    expect(await panel(page)).toContain('0 redo (Ctrl-Y)');
  });

  test('saves a bundle byte for byte when nothing was edited', async ({ page, request }) => {
    await open(page, url({ cam: 'iso', edit: 1 }));
    const files = await save(page);
    expect([...files.keys()].sort()).toEqual([
      'capitol-edit-1-building_h.f32', 'capitol-edit-1-bundle.json', 'capitol-edit-1-ground_h.f32',
      'capitol-edit-1-medium.u8', 'capitol-edit-1-pipes.json', 'capitol-edit-1-shrubs.json',
      'capitol-edit-1-trees.json', 'changes-1.json',
    ]);
    for (const name of ['bundle.json', 'trees.json', 'shrubs.json', 'pipes.json', 'ground_h.f32', 'medium.u8', 'building_h.f32']) {
      const want = await fixtureFile(request, name);
      expect(files.get(`capitol-edit-1-${name}`)!.equals(want), name).toBe(true);
    }
    expect(JSON.parse(files.get('changes-1.json')!.toString())).toEqual({ bundle: 'capitol', save: 1, ops: [] });
    expect(await panel(page)).toContain('saved capitol-edit-1');
    await expect(page.locator('#editinfo')).not.toHaveClass(/dirty/);
  });

  test('saves the edited grids and the change list that made them', async ({ page, request }) => {
    await open(page, url({ cam: 'iso', edit: 1, slot: 'concrete', brush: 1, aim: `${LAWN.gx},${LAWN.gy}` }));
    const i = at(LAWN);
    const names = await media(page);
    const before = await cell(page, i);
    await page.evaluate(() => window.__ecoviewEdit!.act('place')); // a concrete cube over the lawn
    await page.keyboard.press('Digit7'); // ground
    await page.evaluate(() => window.__ecoviewEdit!.act('place')); // one cube more, bare ground
    const snapped = (level(before.ground_h) + 1) * CELL;
    const files = await save(page);
    const changes = JSON.parse(files.get('changes-1.json')!.toString()) as {
      bundle: string; save: number;
      ops: { kind: string; cells: { i: number; before: Cell; after: Cell }[] }[];
    };
    expect(changes.bundle).toBe('capitol');
    expect(changes.save).toBe(1);
    expect(changes.ops.map((o) => o.kind)).toEqual(['place', 'place']);
    expect(changes.ops[0].cells).toHaveLength(1);
    expect(changes.ops[0].cells[0]).toEqual({ i, before, after: expect.objectContaining({
      medium: names.indexOf('concrete'), building_h: before.building_h,
    }) });
    expect(changes.ops[0].cells[0].after.ground_h).toBeCloseTo(snapped, 5);
    expect(changes.ops[1].cells[0].after.ground_h).toBeCloseTo(snapped + CELL, 5);
    // The saved grids differ from the fixture at that one cell and nowhere else.
    const medium = files.get('capitol-edit-1-medium.u8')!;
    const wasMedium = await fixtureFile(request, 'medium.u8');
    const diffs = [...medium].flatMap((v, k) => (v === wasMedium[k] ? [] : [k]));
    expect(diffs).toEqual([i]);
    expect(medium[i]).toBe(names.indexOf('concrete'));
    const ground = files.get('capitol-edit-1-ground_h.f32')!;
    const wasGround = await fixtureFile(request, 'ground_h.f32');
    const changedCells: number[] = [];
    for (let k = 0; k < ground.length; k += 4) {
      if (!ground.subarray(k, k + 4).equals(wasGround.subarray(k, k + 4))) changedCells.push(k / 4);
    }
    expect(changedCells).toEqual([i]);
    expect(ground.readFloatLE(4 * i)).toBeCloseTo(snapped + CELL, 5);
    // The files the editor never touches are still the fixture's bytes, media order and all.
    expect(files.get('capitol-edit-1-bundle.json')!.equals(await fixtureFile(request, 'bundle.json'))).toBe(true);
    expect(files.get('capitol-edit-1-trees.json')!.equals(await fixtureFile(request, 'trees.json'))).toBe(true);
  });

  // The two first-person tests that follow both drive the editor's pick path, and a two-to-three-minute
  // cost lands on one of them and not reliably the same one. Across CI runs 35585550769, 35587962014 and
  // 35599849884 this one measured 12.8, 19.7 and 192.0 s while 'picks the block under the crosshair'
  // measured 84.0, 168.0 and 37.5 s, with every other test in the suite moving by about 1.2x. So both
  // carry test.slow(): the block's 180 s is not enough for whichever one draws the long straw.
  // They sit in a describe of their own so that CI's one retry does not apply to them (shot E9):
  // test.slow() and retries multiply. An attempt that hangs costs the whole 540 s and the retry costs
  // 540 s more, and 18 minutes on top of a suite that measures 12.9 min on a fast runner gets the
  // 30-minute job killed, where one attempt fails cleanly with a timeout message. Playwright scopes
  // retries to a file or a describe block and never to a single test -- test.describe.configure()
  // inside a test body throws -- so this block holds exactly the two that carry the tripled budget,
  // and the other 47 tests in the suite keep their retry (ecoview/DECISIONS.md, "E9 the retry").
  test.describe('the first-person pick path', () => {
    test.describe.configure({ retries: 0 });

    test('places and removes one cube with every hotbar slot, back to the bundle it loaded', async ({ page, request }) => {
      test.slow(); // 12.8, 19.7 then 192.0 s -- it needs the whole 540 s on the run where it is the slow one
      const errors = trackErrors(page);
      await open(page, url({ cam: 'iso', edit: 1, slot: 'ground', brush: 1, aim: `${LAWN.gx},${LAWN.gy}` }));
      const i = at(LAWN);
      // One dig first: that puts the column on the cube lattice, where a place and a remove are mirrors.
      expect(await page.evaluate(() => window.__ecoviewEdit!.act('remove'))).toBe(true);
      const dug = await cell(page, i);
      const d0 = await digest(page);
      for (let k = 0; k < SLOTS.length; k++) {
        const slot = SLOTS[k];
        await page.keyboard.press(`Digit${k + 1}`);
        expect(await page.evaluate(() => window.__ecoviewEdit!.act('place')), slot).toBe(true);
        const up = await cell(page, i);
        expect(level(up.ground_h + up.building_h), slot).toBe(level(dug.ground_h + dug.building_h) + 1);
        expect(await page.evaluate(() => window.__ecoviewEdit!.act('remove')), slot).toBe(true);
        expect(await cell(page, i), slot).toEqual(dug); // the cube came off exactly as it went on
        expect(await digest(page), slot).toEqual(d0);
      }
      // Undo the lot, including the dig, and the grids are the bundle's own bytes again.
      const ops = await page.evaluate(() => window.__ecoviewEdit!.ops());
      expect(ops).toHaveLength(1 + 2 * SLOTS.length);
      for (let k = 0; k < ops.length; k++) expect(await page.evaluate(() => window.__ecoviewEdit!.undo())).toBe(true);
      expect(await page.evaluate(() => window.__ecoviewEdit!.changed())).toEqual([]);
      const files = await save(page);
      for (const name of ['ground_h.f32', 'medium.u8', 'building_h.f32']) {
        expect(files.get(`capitol-edit-1-${name}`)!.equals(await fixtureFile(request, name)), name).toBe(true);
      }
      expect(errors).toEqual([]);
    });

    test('picks the block under the crosshair with the middle button', async ({ page }) => {
      test.slow(); // 84.0, 168.0 then 37.5 s -- 168.0 is 93% of the block's 180 s, and it hit it exactly once
      const errors = trackErrors(page);
      await open(page, url({ cam: 'iso', edit: 1, slot: 'ground', brush: 1, aim: `${LAWN.gx},${LAWN.gy}` }));
      const slots = page.locator('#hotbar .slot');
      await expect(slots.nth(SLOTS.indexOf('ground'))).toHaveClass(/on/);
      // The crosshair is on lawn, so a middle click takes the hotbar to the lawn slot.
      await page.mouse.click(CENTRE.x, CENTRE.y, { button: 'middle' });
      expect(await panel(page)).toContain('picked lawn');
      await expect(slots.nth(SLOTS.indexOf('lawn'))).toHaveClass(/on/);
      expect(page.url()).toContain('slot=lawn');
      // Put a building cube there and it picks the building slot instead, because that is the top cube.
      await page.keyboard.press(`Digit${SLOTS.indexOf('building') + 1}`);
      expect(await page.evaluate(() => window.__ecoviewEdit!.act('place'))).toBe(true);
      await page.keyboard.press(`Digit${SLOTS.indexOf('gravel') + 1}`);
      expect(await page.evaluate(() => window.__ecoviewEdit!.pickSlot())).toBe('building');
      await expect(slots.nth(SLOTS.indexOf('building'))).toHaveClass(/on/);
      expect(errors).toEqual([]);
    });
  });

  test('turns edit mode and the first-person camera on and off from the keyboard', async ({ page }) => {
    const errors = trackErrors(page);
    await open(page, url({ cam: 'iso' }));
    // A world page without ?edit=1 offers edit mode but shows no hotbar and no crosshair over the view.
    await expect(page.locator('#hotbar')).toBeHidden();
    await expect(page.locator('#cross')).toBeHidden();
    expect(await panel(page)).toBe('edit off · press E to edit this bundle');
    await page.keyboard.press('e');
    await expect(page.locator('#hotbar')).toBeVisible();
    expect(await panel(page)).toContain('edit on');
    expect(await panel(page)).toContain('camera orbit');
    expect(page.url()).toContain('edit=1');
    // The hotbar: eight slots, keys 1 to 8, with the fourth reading as a road.
    const slots = page.locator('#hotbar .slot');
    await expect(slots).toHaveCount(8);
    await expect(slots.nth(3)).toHaveText(/road \(asphalt\)/);
    await expect(slots.nth(0)).toHaveClass(/on/);
    await page.keyboard.press('Digit4');
    await expect(slots.nth(3)).toHaveClass(/on/);
    await expect(slots.nth(0)).not.toHaveClass(/on/);
    expect(page.url()).toContain('slot=asphalt');
    // The brush grows and shrinks with the bracket keys, and stops at the ends.
    await page.keyboard.press(']');
    await page.keyboard.press(']');
    expect(await panel(page)).toContain('brush 3');
    for (let k = 0; k < 4; k++) await page.keyboard.press('[');
    expect(await panel(page)).toContain('brush 1');
    // F flies and F again goes back to the orbit camera; the picked cell changes with the view.
    await page.keyboard.press('f');
    expect(await panel(page)).toContain('camera fly');
    await page.keyboard.press('f');
    expect(await panel(page)).toContain('camera orbit');
    await page.keyboard.press('e');
    await expect(page.locator('#hotbar')).toBeHidden();
    await expect(page.locator('#cross')).toBeHidden();
    expect(page.url()).toContain('edit=0');
    expect(errors).toEqual([]);
  });

  test('stands the camera where ?eye= says, and picks what it looks at', async ({ page }) => {
    const errors = trackErrors(page);
    await open(page, url({ cam: 'iso', edit: 1, slot: 'building', eye: '128,40,48,0,-20' }));
    expect(await panel(page)).toContain('camera fly');
    const t = await page.evaluate(() => window.__ecoviewEdit!.target());
    expect(t).not.toBe(null);
    // Looking north and down from 48 m up at x = 128 m: the Capitol, well north of the eye.
    expect(t!.gx).toBeGreaterThan(240);
    expect(t!.gx).toBeLessThan(270);
    expect(t!.gy).toBeGreaterThan(80);
    const hit = await cell(page, t!.i);
    expect(hit.building_h).toBeGreaterThan(20);
    expect(errors).toEqual([]);
  });

  test('applies a 200-operation stroke well inside two seconds', async ({ page }, info) => {
    await open(page, url({ cam: 'iso', edit: 1, slot: 'ground', brush: 1, aim: `${LAWN.gx},${LAWN.gy}` }));
    const ms = await page.evaluate(() => window.__ecoviewEdit!.stroke(200));
    info.annotations.push({ type: 'stroke', description: `200 ops in ${ms.toFixed(1)} ms` });
    console.log(`200-operation stroke: ${ms.toFixed(1)} ms`);
    expect(await page.evaluate(() => window.__ecoviewEdit!.ops())).toHaveLength(200);
    expect(ms).toBeLessThan(2000);
  });
});
