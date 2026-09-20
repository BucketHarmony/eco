// The bundle world in the browser (shot G7): the medium overlay on the 0.5 m ground grid, its legend, the
// extruded buildings, and the shade they cast. Colours are read straight off #view, which is safe because the
// top camera draws the world unlit and the renderer runs with antialias: false, so every pixel is exact.
import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import { PNG } from 'pngjs';
import { trackErrors, open } from './helpers';

const MINI = '/?run=fixtures/capitol-mini';
const RUN = '/?run=runs/capitol-s42';
type Png = ReturnType<typeof PNG.sync.read>;

const VIEW_W = 960;
const VIEW_H = 800;

/** The palette of src/world.ts, repeated here so a silent change to it fails a test. */
const MEDIUM_CSS: Record<string, [number, number, number]> = {
  soil: [0x8b, 0x6b, 0x47],
  lawn: [0x79, 0xb4, 0x49],
  bed: [0xa8, 0x72, 0x4a],
  mulch: [0x6b, 0x4a, 0x2b],
  gravel: [0xb9, 0xb2, 0xa3],
  concrete: [0xd7, 0xd3, 0xcb],
  asphalt: [0x4a, 0x4a, 0x4e],
  roof: [0x9a, 0x9a, 0x9e],
  water: [0x3a, 0x6f, 0xd8],
};
const BUILDING: [number, number, number] = [0xc6, 0xc6, 0xcb];

interface World {
  gw: number;
  gd: number;
  cell: number;
  media: string[];
  medium: Uint8Array;
  /** Ecology columns per side; the Capitol is 256 × 256 at 1 m. */
  dim: number;
}

async function fetchWorld(request: APIRequestContext, base: string): Promise<World> {
  const meta = (await (await request.get(`${base}/meta.json`)).json()) as {
    dims: { x: number; y: number };
    world: { ground_width: number; ground_depth: number; ground_cell_m: number; media: string[] };
  };
  const w = meta.world;
  const medium = new Uint8Array(await (await request.get(`${base}/world/medium.bin`)).body());
  return { gw: w.ground_width, gd: w.ground_depth, cell: w.ground_cell_m, media: w.media, medium, dim: meta.dims.x };
}

/**
 * The top camera is orthographic and fits depth into the view height: 3.125 px per metre on a 256 m world,
 * with the world spanning screen x 80..880. Sim +y runs up the screen.
 */
function topPixel(dim: number, simX: number, simY: number): [number, number] {
  const halfH = dim / 2;
  const halfW = (halfH * VIEW_W) / VIEW_H;
  const px = ((simX - (dim / 2 - halfW)) / (2 * halfW)) * VIEW_W;
  const py = ((dim / 2 + halfH - simY) / (2 * halfH)) * VIEW_H;
  return [Math.floor(px), Math.floor(py)];
}

/** The pixel a ground cell's centre falls in, and the cell that pixel's centre falls back into. */
function cellPixel(w: World, gx: number, gy: number): { px: number; py: number; back: [number, number] } {
  const [px, py] = topPixel(w.dim, (gx + 0.5) * w.cell, (gy + 0.5) * w.cell);
  const halfH = w.dim / 2;
  const halfW = (halfH * VIEW_W) / VIEW_H;
  const simX = ((px + 0.5) / VIEW_W) * 2 * halfW + (w.dim / 2 - halfW);
  const simY = w.dim / 2 + halfH - ((py + 0.5) / VIEW_H) * 2 * halfH;
  return { px, py, back: [Math.floor(simX / w.cell), Math.floor(simY / w.cell)] };
}

const mediumOf = (w: World, gx: number, gy: number) => w.media[w.medium[gx + w.gw * gy]];

/** True when every cell within `r` of (gx, gy) shares its medium, so one screen pixel cannot straddle two. */
function uniform(w: World, gx: number, gy: number, r: number): boolean {
  const m = mediumOf(w, gx, gy);
  for (let y = gy - r; y <= gy + r; y++) {
    for (let x = gx - r; x <= gx + r; x++) {
      if (x < 0 || y < 0 || x >= w.gw || y >= w.gd || mediumOf(w, x, y) !== m) return false;
    }
  }
  return true;
}

/** The first cell of `name` with a uniform neighbourhood whose sample pixel maps back to itself. */
function sampleCell(w: World, name: string): { px: number; py: number; gx: number; gy: number } {
  for (let gy = 2; gy < w.gd - 2; gy++) {
    for (let gx = 2; gx < w.gw - 2; gx++) {
      if (mediumOf(w, gx, gy) !== name || !uniform(w, gx, gy, 2)) continue;
      const { px, py, back } = cellPixel(w, gx, gy);
      if (back[0] === gx && back[1] === gy) return { px, py, gx, gy };
    }
  }
  throw new Error(`no uniform ${name} cell`);
}

async function viewPng(page: Page): Promise<Png> {
  return PNG.sync.read(await page.locator('#view').screenshot());
}

const pixel = (png: Png, x: number, y: number): [number, number, number] => {
  const i = (png.width * y + x) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2]];
};

/** Pixels exactly equal to `rgb`, as a fraction of the view. */
function share(png: Png, rgb: [number, number, number]): number {
  let n = 0;
  for (let i = 0; i < png.data.length; i += 4) {
    if (png.data[i] === rgb[0] && png.data[i + 1] === rgb[1] && png.data[i + 2] === rgb[2]) n++;
  }
  return n / (png.width * png.height);
}

/** Mean brightness over a screen-space box, inclusive. */
function meanIn(png: Png, x0: number, x1: number, y0: number, y1: number): number {
  let s = 0;
  let n = 0;
  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const [r, g, b] = pixel(png, x, y);
      s += (r + g + b) / 3;
      n++;
    }
  }
  return s / n;
}

test('the Capitol bundle loads at format 4 and the medium legend names every medium', async ({ page, request }) => {
  const errors = trackErrors(page);
  const w = await fetchWorld(request, '/fixtures/capitol-mini');
  await open(page, `${MINI}&tick=0&overlay=medium&cam=top`);
  await expect(page.locator('#status')).toContainText('fixtures/capitol-mini · seed 42');
  const legend = page.locator('#legend');
  await expect(legend).toBeVisible();
  await expect(legend.locator('.chip')).toHaveCount(w.media.length);
  expect(await legend.locator('.chip').allInnerTexts()).toEqual(w.media);
  // The legend belongs to the medium overlay alone, which is why the other reference shots are unchanged.
  await page.selectOption('#overlay', 'material');
  await page.waitForFunction(() => window.__ecoviewReady === true);
  await expect(legend).toBeHidden();
  expect(errors).toEqual([]);
});

test('the medium overlay paints each ground cell its palette colour', async ({ page, request }) => {
  const w = await fetchWorld(request, '/fixtures/capitol-mini');
  await open(page, `${MINI}&tick=0&overlay=medium&cam=top`);
  const png = await viewPng(page);
  expect([png.width, png.height]).toEqual([VIEW_W, VIEW_H]);
  for (const name of ['lawn', 'concrete', 'asphalt']) {
    const c = sampleCell(w, name);
    expect(pixel(png, c.px, c.py), `${name} at ground cell ${c.gx},${c.gy}`).toEqual(MEDIUM_CSS[name]);
  }
});

test('the medium overlay resolves detail finer than the 1 m ecology grid', async ({ page, request }) => {
  const w = await fetchWorld(request, '/fixtures/capitol-mini');
  await open(page, `${MINI}&tick=0&overlay=medium&cam=top`);
  const png = await viewPng(page);
  // Ecology column x covers ground cells 2x and 2x+1. A pair of neighbours inside one column with different
  // media can only show as two colours if the drape carries the ground grid rather than the voxel grid.
  let pairs = 0;
  for (let gy = 4; gy < w.gd - 4 && pairs < 8; gy++) {
    for (let gx = 4; gx < w.gw - 5 && pairs < 8; gx += 2) {
      const [a, b] = [mediumOf(w, gx, gy), mediumOf(w, gx + 1, gy)];
      if (a === b || !uniform(w, gx, gy, 0) || !uniform(w, gx + 1, gy, 0)) continue;
      // Both cells must own their sample pixel and have a like-medium cell above and below, so a pixel that
      // spills a third of a cell vertically still lands on the same colour.
      if (mediumOf(w, gx, gy - 1) !== a || mediumOf(w, gx, gy + 1) !== a) continue;
      if (mediumOf(w, gx + 1, gy - 1) !== b || mediumOf(w, gx + 1, gy + 1) !== b) continue;
      const ca = cellPixel(w, gx, gy);
      const cb = cellPixel(w, gx + 1, gy);
      if (ca.back[0] !== gx || cb.back[0] !== gx + 1 || ca.px === cb.px) continue;
      expect(pixel(png, ca.px, ca.py), `${a} at ${gx},${gy}`).toEqual(MEDIUM_CSS[a]);
      expect(pixel(png, cb.px, cb.py), `${b} at ${gx + 1},${gy}`).toEqual(MEDIUM_CSS[b]);
      pairs++;
    }
  }
  expect(pairs).toBe(8);
});

test('asphalt runs along at least two sides of the site', async ({ page, request }) => {
  const w = await fetchWorld(request, '/fixtures/capitol-mini');
  await open(page, `${MINI}&tick=0&overlay=medium&cam=top`);
  const png = await viewPng(page);
  const band = 6; // metres of the world's edge
  const sides: Record<string, number> = {};
  for (const [side, box] of Object.entries({
    west: [0, band, 0, w.dim],
    east: [w.dim - band, w.dim, 0, w.dim],
    south: [0, w.dim, 0, band],
    north: [0, w.dim, w.dim - band, w.dim],
  })) {
    const [x0, y0] = topPixel(w.dim, box[0], box[3]);
    const [x1, y1] = topPixel(w.dim, box[1], box[2]);
    let n = 0;
    let hit = 0;
    for (let y = y0; y < y1; y++) {
      for (let x = x0; x < x1; x++) {
        n++;
        const p = pixel(png, x, y);
        if (p.every((v, i) => v === MEDIUM_CSS.asphalt[i])) hit++;
      }
    }
    sides[side] = hit / n;
  }
  const paved = Object.entries(sides).filter(([, f]) => f > 0.5);
  expect(paved.length, `asphalt share per side ${JSON.stringify(sides)}`).toBeGreaterThanOrEqual(2);
});

test('the buildings cover every roof cell in the top camera', async ({ page, request }) => {
  const w = await fetchWorld(request, '/fixtures/capitol-mini');
  await open(page, `${MINI}&tick=0&overlay=medium&cam=top`);
  const png = await viewPng(page);
  const roofs = w.medium.reduce((n, c) => (w.media[c] === 'roof' ? n + 1 : n), 0);
  // Straight down, the roof of an extruded building projects onto exactly its footprint.
  const pxPerCell = (VIEW_H / w.dim) * w.cell;
  const want = (roofs * pxPerCell * pxPerCell) / (VIEW_W * VIEW_H);
  expect(share(png, BUILDING)).toBeGreaterThan(want * 0.9);
  expect(share(png, BUILDING)).toBeLessThan(want * 1.1);
  // The roof medium is hidden underneath them, so its palette colour barely shows.
  expect(share(png, MEDIUM_CSS.roof)).toBeLessThan(want * 0.05);
});

test('the buildings stand up in the iso camera and come from building_h.bin', async ({ page, request }) => {
  const w = await fetchWorld(request, '/fixtures/capitol-mini');
  await open(page, `${MINI}&tick=0&overlay=medium&cam=iso`);
  const withBuildings = await viewPng(page);
  // Flatten every building and render the same frame: what changes is the buildings and their shadowing.
  await page.route('**/fixtures/capitol-mini/world/building_h.bin', (route) =>
    route.fulfill({ contentType: 'application/octet-stream', body: Buffer.alloc(w.gw * w.gd * 4) }),
  );
  await open(page, `${MINI}&tick=0&overlay=medium&cam=iso`);
  const flat = await viewPng(page);
  let diff = 0;
  for (let i = 0; i < flat.data.length; i += 4) {
    if (withBuildings.data[i] !== flat.data[i] || withBuildings.data[i + 1] !== flat.data[i + 1]) diff++;
  }
  const frac = diff / (VIEW_W * VIEW_H);
  expect(frac, 'share of the iso view the buildings occupy').toBeGreaterThan(0.02);
  expect(frac).toBeLessThan(0.5);
});

test('a building casts a block of shade in the light overlay', async ({ page, request }) => {
  const w = await fetchWorld(request, '/fixtures/capitol-mini');
  await open(page, `${MINI}&tick=0&overlay=light&cam=top`);
  const png = await viewPng(page);
  // The main building fills ground cells 194..339 east and 100..359 north; light comes from the south, so the
  // strip just north of it sits in permanent shade and the open lawn in the south-west does not.
  const [sx0, sy0] = topPixel(w.dim, 194 * w.cell, 368 * w.cell);
  const [sx1, sy1] = topPixel(w.dim, 339 * w.cell, 361 * w.cell);
  const [lx0, ly0] = topPixel(w.dim, 20, 60);
  const [lx1, ly1] = topPixel(w.dim, 60, 20);
  const shade = meanIn(png, sx0, sx1, sy0, sy1);
  const lawn = meanIn(png, lx0, lx1, ly0, ly1);
  expect(shade, `shade ${shade.toFixed(1)} vs open ${lawn.toFixed(1)}`).toBeLessThan(160);
  expect(lawn).toBeGreaterThan(200);
});

test('the reference run stands no tree on a roof or road column at tick 20000', async ({ page, request }) => {
  const errors = trackErrors(page);
  const w = await fetchWorld(request, '/runs/capitol-s42');
  await open(page, `${RUN}&tick=20000&overlay=material&cam=iso`, 60_000);
  const ents = (await (await request.get('/runs/capitol-s42/snap_020000/entities.json')).json()) as {
    kind: string;
    x: number;
    y: number;
  }[];
  const trees = ents.filter((e) => e.kind === 'tree');
  expect(trees.length).toBeGreaterThan(20);
  const counts: Record<string, number> = {};
  for (const t of trees) {
    for (const [dx, dy] of [
      [0, 0],
      [1, 0],
      [0, 1],
      [1, 1],
    ]) {
      const m = mediumOf(w, Math.floor(t.x) * 2 + dx, Math.floor(t.y) * 2 + dy);
      counts[m] = (counts[m] ?? 0) + 1;
    }
  }
  // A tree stands on a 1 m column that spans four 0.5 m ground cells, so a trunk beside a walk can overlap one
  // paved cell; what must not happen is a tree on a roof.
  const cells = trees.length * 4;
  expect(counts.roof ?? 0, `medium under trees ${JSON.stringify(counts)}`).toBe(0);
  expect((counts.asphalt ?? 0) / cells).toBeLessThan(0.01);
  expect((counts.lawn ?? 0) / cells).toBeGreaterThan(0.95);
  expect(errors).toEqual([]);
});
