// Edit mode (shot E1): the bundle reader, the operation model, the brush and the chunk index.
import { describe, expect, it } from 'vitest';
import {
  BUNDLE_FORMAT, BUNDLE_JSON_VERSION, BUNDLE_VERBATIM, loadBundle, type Bundle, type Fetcher, type WorldData,
} from '../../src/loader';
import {
  History, MAX_BRUSH, SLOTS, SLOT_KEYS, STEP, applyOp, cellState, changesJson, disc, f32Bytes, flySpeed,
  isSlot, makeOp, opFor, sameState, saveFiles, type Action, type Op, type Slot, type Target,
} from '../../src/edit';
import { CHUNK, GroundChunks } from '../../src/world';

/** The media of docs/SCENE-CONTRACT.md, in the order a bundle lists them. */
const MEDIA = ['soil', 'lawn', 'bed', 'mulch', 'gravel', 'concrete', 'asphalt', 'roof', 'water'];
const SIZE = 16;
const CELL = 0.5;
const GW = SIZE / CELL; // 32
const N = GW * GW;
/** Ground cells 12..19 in both axes carry a roof, so a pick can hit a wall and a remove can cut a building. */
const ROOF = { lo: 12, hi: 19, height: 6 };

interface Synthetic {
  files: Record<string, string | Uint8Array>;
  json: Record<string, unknown>;
}

/** A 16 m bundle over a 32 x 32 ground grid: lawn on a slope, with one roof block. */
function synthetic(patch: (s: Synthetic) => void = () => {}): Synthetic {
  const ground = new Float32Array(N);
  const medium = new Uint8Array(N).fill(MEDIA.indexOf('lawn'));
  const building = new Float32Array(N);
  for (let gy = 0; gy < GW; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      const i = gx + GW * gy;
      ground[i] = 1 + gx / 100; // a west-to-east slope, so a flatten has something to level
      if (gx >= ROOF.lo && gx <= ROOF.hi && gy >= ROOF.lo && gy <= ROOF.hi) {
        medium[i] = MEDIA.indexOf('roof');
        building[i] = ROOF.height;
      }
    }
  }
  const s: Synthetic = {
    json: {
      format: BUNDLE_FORMAT, version: BUNDLE_JSON_VERSION, name: 'syn', size_m: SIZE, ground_cell_m: CELL,
      ground_width: GW, ground_depth: GW, media: MEDIA, source: 'a test',
    },
    files: {
      '/syn/ground_h.f32': f32Bytes(ground),
      '/syn/medium.u8': medium,
      '/syn/building_h.f32': f32Bytes(building),
      '/syn/trees.json': '[{"x":2,"y":3,"height":9,"crown_radius":3,"crown_base":4}]',
      '/syn/shrubs.json': '[{"x":5,"y":5,"height":1,"rx":0.8,"ry":0.5,"angle":0.4}]',
      '/syn/pipes.json': '[{"id":"p","inlet":[1,2],"outlet":[0,2],"capacity_m3h":50,"illustrative":true}]',
    },
  };
  patch(s);
  return s;
}

function serve(s: Synthetic): Fetcher {
  const files: Record<string, string | Uint8Array> = { ...s.files, '/syn/bundle.json': JSON.stringify(s.json) };
  return async (url) => (url in files
    ? new Response(files[url] as BodyInit, { status: 200 })
    : new Response('', { status: 404 }));
}

const load = (patch?: (s: Synthetic) => void): Promise<Bundle> => loadBundle('/syn', serve(synthetic(patch)));

/** A deterministic generator, so a failing random-op case is reproducible. */
function rng(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

const target = (w: WorldData, gx: number, gy: number, top = true): Target =>
  ({ i: gx + w.gw * gy, gx, gy, top, adj: gx + w.gw * gy });

const grids = (w: WorldData): [number[], number[], number[]] =>
  [[...w.ground_h], [...w.medium], [...w.building_h]];

describe('the bundle reader', () => {
  it('reads a version-2 bundle into the same WorldData the run reader uses', async () => {
    const b = await load();
    expect(b.json.name).toBe('syn');
    const w = b.world;
    expect([w.gw, w.gd, w.cell]).toEqual([GW, GW, CELL]);
    expect(w.meta).toEqual({ name: 'syn', ground_cell_m: CELL, ground_width: GW, ground_depth: GW, media: MEDIA });
    expect(w.ground_h.length).toBe(N);
    expect(w.medium.length).toBe(N);
    expect(w.building_h.length).toBe(N);
    // Written little-endian by hand above, so a native-endianness read would give nonsense.
    expect(w.ground_h[0]).toBeCloseTo(1, 6);
    expect(w.ground_h[GW - 1]).toBeCloseTo(1.31, 5);
    expect(w.building_h[ROOF.lo + GW * ROOF.lo]).toBe(ROOF.height);
    expect(w.building_h[0]).toBe(0);
    expect(w.pipes).toEqual([{ id: 'p', inlet: [1, 2], outlet: [0, 2], capacity_m3h: 50, illustrative: true }]);
    expect(b.trees).toEqual([{ x: 2, y: 3, height: 9, crown_radius: 3, crown_base: 4 }]);
    expect(b.shrubs).toEqual([{ x: 5, y: 5, height: 1, rx: 0.8, ry: 0.5, angle: 0.4 }]);
    // The view is the scene's own size, and as tall as the highest roof plus a metre: 1.19 + 6 m here.
    expect([b.grid.x, b.grid.y]).toEqual([SIZE, SIZE]);
    expect(b.grid.z).toBe(9);
    // Every file the editor writes back unchanged is kept verbatim, byte for byte.
    for (const f of BUNDLE_VERBATIM) expect(typeof b.raw[f]).toBe('string');
    expect(JSON.parse(b.raw['bundle.json'])).toEqual(synthetic().json);
  });

  it('rejects a bundle.json the sim would reject', async () => {
    const cases: [(s: Synthetic) => void, RegExp][] = [
      [(s) => { s.json.format = 'ecosim-run'; }, /format "ecosim-run", expected "ecosim-world-bundle"/],
      [(s) => { s.json.version = 1; }, /version 1, expected 2/],
      [(s) => { delete s.json.version; }, /version undefined, expected 2/],
      [(s) => { s.json.size_m = 16.5; }, /size_m 16.5 is not 1..=256 whole metres/],
      [(s) => { s.json.size_m = 0; }, /size_m 0 is not 1..=256 whole metres/],
      [(s) => { s.json.size_m = 300; }, /size_m 300 is not 1..=256 whole metres/],
      [(s) => { s.json.ground_cell_m = 0; }, /ground_cell_m 0/],
      [(s) => { s.json.ground_width = GW - 1; }, /ground_width 31, expected 32 for size_m 16 at 0.5 m cells/],
      [(s) => { s.json.ground_depth = 64; }, /ground_depth 64, expected 32 for size_m 16 at 0.5 m cells/],
      [(s) => { s.json.media = 'lawn'; }, /media is not a list of names/],
      [(s) => { s.json.media = ['soil', 7]; }, /media is not a list of names/],
      // ecosim's Bundle::load requires media[0] == "soil", so a save could never be read back.
      [(s) => { s.json.media = ['lawn', 'soil']; }, /media\[0\] is "lawn", expected "soil"/],
      [(s) => { delete s.files['/syn/trees.json']; }, /trees\.json: HTTP 404/],
    ];
    for (const [patch, msg] of cases) {
      await expect(load(patch), msg.source).rejects.toThrow(msg);
    }
  });

  it('checks every grid file against bundle.json', async () => {
    const cases: [(s: Synthetic) => void, RegExp][] = [
      [(s) => { s.files['/syn/ground_h.f32'] = new Uint8Array(8); }, /ground_h\.f32: expected 4096 bytes, got 8/],
      [(s) => { s.files['/syn/ground_h.f32'] = new Uint8Array(4 * N + 4); }, /ground_h\.f32: expected 4096 bytes, got 4100/],
      [(s) => { s.files['/syn/medium.u8'] = new Uint8Array(N - 1); }, /medium\.u8: expected 1024 bytes, got 1023/],
      [(s) => { s.files['/syn/building_h.f32'] = new Uint8Array(N); }, /building_h\.f32: expected 4096 bytes, got 1024/],
      // A one-metre bundle is legal, so a size_m change alone leaves every grid the wrong size (the three
      // fetches race, so any of them may be the one that rejects).
      [(s) => { s.json.size_m = 1; s.json.ground_width = 2; s.json.ground_depth = 2; }, /expected (16|4) bytes, got (4096|1024)/],
      [(s) => { (s.files['/syn/medium.u8'] as Uint8Array)[3] = MEDIA.length; }, /medium\.u8: code 9 is not in bundle.json media/],
      [(s) => { (s.files['/syn/medium.u8'] as Uint8Array)[N - 1] = 200; }, /medium\.u8: code 200 is not in bundle.json media/],
    ];
    for (const [patch, msg] of cases) {
      await expect(load(patch), msg.source).rejects.toThrow(msg);
    }
  });
});

describe('the hotbar', () => {
  it('binds the eight slots the shot names, with walk as concrete and road as asphalt', () => {
    expect(SLOT_KEYS).toEqual(['lawn', 'bed', 'concrete', 'asphalt', 'water', 'gravel', 'ground', 'building']);
    expect(SLOTS.map((s) => s.label)).toEqual([
      'lawn', 'bed', 'walk (concrete)', 'road (asphalt)', 'water', 'gravel', 'ground', 'building',
    ]);
    // Every medium slot names a medium of the scene contract; ground and building are the two height slots.
    for (const s of SLOT_KEYS) expect(MEDIA.includes(s) || s === 'ground' || s === 'building', s).toBe(true);
    expect(isSlot('lawn')).toBe(true);
    expect(isSlot('roof')).toBe(false);
  });

  it('clamps the fly speed the wheel sets', () => {
    expect(flySpeed(12, -100)).toBeGreaterThan(12);
    expect(flySpeed(12, 100)).toBeLessThan(12);
    expect(flySpeed(1, 1000)).toBeGreaterThanOrEqual(1);
    expect(flySpeed(120, -1000)).toBeLessThanOrEqual(120);
  });
});

describe('the operation model', () => {
  it('applies an op and takes it back, cell for cell', async () => {
    const w = (await load()).world;
    const before = grids(w);
    const op = makeOp(w, 'place', disc(w, 5, 5, 3), (s) => ({ ...s, medium: MEDIA.indexOf('asphalt') }))!;
    expect(op.cells).toHaveLength(13);
    applyOp(w, op);
    expect(grids(w)).not.toEqual(before);
    expect(w.medium[5 + GW * 5]).toBe(MEDIA.indexOf('asphalt'));
    applyOp(w, op, 'before');
    expect(grids(w)).toEqual(before);
  });

  it('round-trips a random walk of operations, forwards and backwards', async () => {
    const w = (await load()).world;
    const start = grids(w);
    const rand = rng(42);
    const actions: Action[] = ['remove', 'place', 'flatten'];
    const history = new History(w);
    const ops: Op[] = [];
    for (let k = 0; k < 200; k++) {
      const gx = Math.floor(rand() * GW);
      const gy = Math.floor(rand() * GW);
      const slot = SLOT_KEYS[Math.floor(rand() * SLOT_KEYS.length)] as Slot;
      const t = target(w, gx, gy, rand() < 0.8);
      const op = opFor(w, actions[Math.floor(rand() * 3)], t, slot, 1 + Math.floor(rand() * MAX_BRUSH));
      if (!op) continue;
      // Every cell's recorded `before` is the state the grids hold right now.
      for (const c of op.cells) expect(sameState(c.before, cellState(w, c.i)), `op ${k} cell ${c.i}`).toBe(true);
      history.push(op);
      for (const c of op.cells) expect(sameState(c.after, cellState(w, c.i)), `op ${k} cell ${c.i}`).toBe(true);
      ops.push(op);
    }
    expect(ops.length).toBeGreaterThan(100);
    expect(new Set(ops.map((o) => o.kind))).toEqual(new Set(actions));
    expect(grids(w)).not.toEqual(start);
    expect(history.counts).toEqual([ops.length, 0]);
    expect(history.dirty).toBe(true);
    for (let k = ops.length - 1; k >= 0; k--) expect(history.undo()).toBe(ops[k]);
    expect(history.undo()).toBe(null);
    expect(grids(w)).toEqual(start);
    expect(history.changed).toEqual([]);
    expect(history.counts).toEqual([0, ops.length]);
    // And forwards again, to the same place the walk reached.
    for (const op of ops) expect(history.redo()).toBe(op);
    expect(history.redo()).toBe(null);
    const end = grids(w);
    for (let k = ops.length - 1; k >= 0; k--) applyOp(w, ops[k], 'before');
    expect(grids(w)).toEqual(start);
    for (const op of ops) applyOp(w, op);
    expect(grids(w)).toEqual(end);
  });

  it('keeps only the cells an op changes, and makes no op when it changes nothing', async () => {
    const w = (await load()).world;
    const lawn = MEDIA.indexOf('lawn');
    expect(makeOp(w, 'place', disc(w, 4, 4, 5), (s) => ({ ...s, medium: lawn }))).toBe(null);
    const mixed = opFor(w, 'place', target(w, ROOF.lo, ROOF.lo), 'lawn', 3)!;
    // A radius-3 disc on the roof block's south-west corner: only its 6 roof cells are not lawn already.
    expect(mixed.cells).toHaveLength(6);
    expect(mixed.cells.every((c) => c.before.medium === MEDIA.indexOf('roof'))).toBe(true);
    expect(mixed.cells.every((c) => c.after.medium === lawn)).toBe(true);
    // A medium the bundle does not list has no code, so that slot does nothing at all.
    const short = (await load((s) => {
      s.json.media = ['soil', 'lawn'];
      (s.files['/syn/medium.u8'] as Uint8Array).fill(1);
    })).world;
    expect(opFor(short, 'place', target(short, 4, 4), 'asphalt', 1)).toBe(null);
    expect(opFor(short, 'place', target(short, 4, 4), 'ground', 1)!.cells).toHaveLength(1);
  });

  it('gives each action the meaning the sidebar claims', async () => {
    const w = (await load()).world;
    const roof = target(w, 14, 14);
    const lawn = target(w, 2, 2);
    // Remove takes half a metre off a building, and digs into bare ground instead.
    applyOp(w, opFor(w, 'remove', roof, 'lawn', 1)!);
    expect(w.building_h[roof.i]).toBe(ROOF.height - STEP);
    const h0 = w.ground_h[lawn.i];
    applyOp(w, opFor(w, 'remove', lawn, 'lawn', 1)!);
    expect(w.ground_h[lawn.i]).toBeCloseTo(h0 - STEP, 6);
    // Two ground places make a metre: the bundle's step is half the sim's column (DECISIONS.md, shot E1).
    applyOp(w, opFor(w, 'place', lawn, 'ground', 1)!);
    applyOp(w, opFor(w, 'place', lawn, 'ground', 1)!);
    expect(w.ground_h[lawn.i]).toBeCloseTo(h0 + STEP, 6);
    // A side hit grows the neighbour the ray came from, so a wall extends rather than thickens.
    const side: Target = { i: 20 + GW * 14, gx: 20, gy: 14, top: false, adj: 19 + GW * 14 };
    const wall = opFor(w, 'place', side, 'building', 1)!;
    expect(wall.cells.map((c) => c.i)).toEqual([side.adj]);
    expect(wall.cells[0].after.building_h).toBe(ROOF.height + STEP);
    // Flatten levels a disc onto the targeted cell's height, and leaves the media alone.
    applyOp(w, opFor(w, 'flatten', target(w, 6, 6), 'lawn', 3)!);
    const cells = disc(w, 6, 6, 3);
    expect(new Set(cells.map((i) => w.ground_h[i])).size).toBe(1);
    expect(cells.every((i) => w.medium[i] === MEDIA.indexOf('lawn'))).toBe(true);
  });

  it('tracks the cells a session changed, dropping the ones edited back', async () => {
    const w = (await load()).world;
    const h = new History(w);
    h.push(opFor(w, 'place', target(w, 8, 8), 'asphalt', 1)!);
    h.push(opFor(w, 'place', target(w, 9, 8), 'gravel', 1)!);
    expect(h.changed).toEqual([8 + GW * 8, 9 + GW * 8]);
    h.push(opFor(w, 'place', target(w, 8, 8), 'lawn', 1)!); // back to what it was
    expect(h.changed).toEqual([9 + GW * 8]);
    expect(h.dirty).toBe(true);
    h.markSaved();
    expect(h.dirty).toBe(false);
    h.undo();
    expect(h.dirty).toBe(true);
    h.redo();
    expect(h.dirty).toBe(false);
    // A new op after an undo drops the redo tail.
    h.undo();
    h.push(opFor(w, 'place', target(w, 1, 1), 'bed', 1)!);
    expect(h.counts).toEqual([3, 0]);
  });
});

describe('the brush', () => {
  it('is a disc of exactly the cells within the radius less one', async () => {
    const w = (await load()).world;
    const c = 16;
    const want = (reach: number): number[] => {
      const out: number[] = [];
      for (let dy = -reach; dy <= reach; dy++) {
        for (let dx = -reach; dx <= reach; dx++) {
          if (dx * dx + dy * dy <= reach * reach) out.push(c + dx + GW * (c + dy));
        }
      }
      return out.sort((a, b) => a - b);
    };
    expect(disc(w, c, c, 1)).toEqual([c + GW * c]);
    expect(disc(w, c, c, MAX_BRUSH).sort((a, b) => a - b)).toEqual(want(MAX_BRUSH - 1));
    expect(disc(w, c, c, MAX_BRUSH)).toHaveLength(197);
    expect([1, 2, 3, 4, 5, 6, 7, 8, 9].map((r) => disc(w, c, c, r).length))
      .toEqual([1, 5, 13, 29, 49, 81, 113, 149, 197]);
    // A radius past the hotbar's is clamped, never grown.
    expect(disc(w, c, c, 99)).toHaveLength(197);
    expect(disc(w, c, c, 0)).toHaveLength(1);
    // Clipped at the south-west corner: the quarter disc, with no index wrapping onto the far edge.
    const corner = disc(w, 0, 0, MAX_BRUSH);
    expect(corner).toHaveLength(58);
    expect(corner.every((i) => i % GW <= 8 && Math.floor(i / GW) <= 8)).toBe(true);
    expect(disc(w, GW - 1, GW - 1, MAX_BRUSH)).toHaveLength(58);
    expect(disc(w, GW - 1, GW - 1, 5).every((i) => i >= 0 && i < N)).toBe(true);
  });
});

describe('the chunk index', () => {
  /** A bare ground grid, so the chunk maths is tested at sizes no fixture has. */
  const fake = (gw: number, gd: number): WorldData => ({
    meta: { name: 'f', ground_cell_m: 0.5, ground_width: gw, ground_depth: gd, media: MEDIA },
    gw,
    gd,
    cell: 0.5,
    ground_h: new Float32Array(gw * gd),
    medium: new Uint8Array(gw * gd),
    building_h: new Float32Array(gw * gd),
    pipes: [],
  });

  it('cuts the grid into 32-cell chunks and clips the last one', () => {
    const c = new GroundChunks(fake(128, 96), 48);
    expect(CHUNK).toBe(32);
    expect([c.cx, c.cy]).toEqual([4, 3]);
    expect(c.range(0)).toEqual({ x0: 0, y0: 0, w: 32, d: 32 });
    expect(c.range(5)).toEqual({ x0: 32, y0: 32, w: 32, d: 32 });
    expect(c.range(11)).toEqual({ x0: 96, y0: 64, w: 32, d: 32 });
    expect(c.chunkOf(0, 0)).toBe(0);
    expect(c.chunkOf(31, 31)).toBe(0);
    expect(c.chunkOf(32, 31)).toBe(1);
    expect(c.chunkOf(0, 32)).toBe(4);
    expect(c.chunkOf(127, 95)).toBe(11);
    expect(c.instances).toEqual([128 * 96, 0]);
    // A grid that is not a whole number of chunks: the edge chunks are short, not out of bounds.
    const ragged = new GroundChunks(fake(100, 34), 17);
    expect([ragged.cx, ragged.cy]).toEqual([4, 2]);
    expect(ragged.range(3)).toEqual({ x0: 96, y0: 0, w: 4, d: 32 });
    expect(ragged.range(4)).toEqual({ x0: 0, y0: 32, w: 32, d: 2 });
    expect(ragged.chunkOf(99, 33)).toBe(7);
    expect(ragged.instances).toEqual([100 * 34, 0]);
  });

  it('maps a stroke to at most four chunks, wherever it lands', () => {
    const w = fake(128, 128);
    const c = new GroundChunks(w, 64);
    expect(c.chunksOf([])).toEqual([]);
    expect(c.chunksOf(disc(w, 48, 48, MAX_BRUSH))).toEqual([c.chunkOf(48, 48)]);
    // A radius-9 disc is 17 cells across, so it can straddle one chunk boundary per axis, never two.
    expect(c.chunksOf(disc(w, 32, 32, MAX_BRUSH)).sort((a, b) => a - b)).toEqual([0, 1, 4, 5]);
    expect(c.chunksOf(disc(w, 32, 48, MAX_BRUSH)).sort((a, b) => a - b)).toEqual([4, 5]);
    for (let gy = 0; gy < 128; gy += 7) {
      for (let gx = 0; gx < 128; gx += 5) {
        const n = c.chunksOf(disc(w, gx, gy, MAX_BRUSH)).length;
        expect(n, `${gx},${gy}`).toBeLessThanOrEqual(4);
        expect(n, `${gx},${gy}`).toBeGreaterThan(0);
      }
    }
  });
});

describe('saving', () => {
  it('writes the seven bundle files plus the change list, the untouched ones verbatim', async () => {
    const b = await load();
    const files = saveFiles(b, 1, []);
    expect(files.map((f) => f.name)).toEqual([
      'syn-edit-1-bundle.json', 'syn-edit-1-trees.json', 'syn-edit-1-shrubs.json', 'syn-edit-1-pipes.json',
      'syn-edit-1-ground_h.f32', 'syn-edit-1-medium.u8', 'syn-edit-1-building_h.f32', 'changes-1.json',
    ]);
    // Verbatim: the very text the fetch returned, so bundle.json keeps its key order and its media order.
    for (const f of BUNDLE_VERBATIM) {
      expect(files.find((x) => x.name === `syn-edit-1-${f}`)!.data).toBe(b.raw[f]);
    }
    const syn = synthetic();
    const bytes = (name: string): Uint8Array => files.find((f) => f.name === `syn-edit-1-${name}`)!.data as Uint8Array;
    for (const g of ['ground_h.f32', 'medium.u8', 'building_h.f32']) {
      expect([...bytes(g)], g).toEqual([...(syn.files[`/syn/${g}`] as Uint8Array)]);
    }
    expect(JSON.parse(files[7].data as string)).toEqual({ bundle: 'syn', save: 1, ops: [] });
  });

  it('writes the edited grids and the operations that made them', async () => {
    const b = await load();
    const w = b.world;
    const i = 3 + GW * 4;
    const h0 = w.ground_h[i];
    const h = new History(w);
    h.push(opFor(w, 'place', target(w, 3, 4), 'asphalt', 1)!);
    h.push(opFor(w, 'place', target(w, 3, 4), 'ground', 1)!);
    const files = saveFiles(b, 2, h.applied);
    expect(files.map((f) => f.name).filter((n) => n.startsWith('changes'))).toEqual(['changes-2.json']);
    expect(files[0].name).toBe('syn-edit-2-bundle.json');
    const medium = files.find((f) => f.name.endsWith('medium.u8'))!.data as Uint8Array;
    expect(medium[i]).toBe(MEDIA.indexOf('asphalt'));
    const ground = files.find((f) => f.name.endsWith('ground_h.f32'))!.data as Uint8Array;
    const view = new DataView(ground.buffer, ground.byteOffset, ground.byteLength);
    expect(view.getFloat32(4 * i, true)).toBeCloseTo(h0 + STEP, 5);
    expect(view.getFloat32(0, true)).toBeCloseTo(1, 5); // and every other cell untouched
    const changes = JSON.parse(changesJson(b, 2, h.applied)) as { ops: Op[] };
    expect(changes.ops).toHaveLength(2);
    expect(changes.ops[0].kind).toBe('place');
    expect(changes.ops[0].cells).toEqual([{
      i,
      before: { ground_h: h0, medium: MEDIA.indexOf('lawn'), building_h: 0 },
      after: { ground_h: h0, medium: MEDIA.indexOf('asphalt'), building_h: 0 },
    }]);
    expect(changes.ops[1].cells[0].after.ground_h).toBeCloseTo(h0 + STEP, 5);
  });
});
