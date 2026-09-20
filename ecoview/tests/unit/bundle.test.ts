// Format 4: the world bundle's ground grid, the medium overlay and the geometry it drives (shot G7).
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import * as THREE from 'three';
import {
  Grid, ROCK, SOIL, loadRun, loadSnapshot, parseMeta,
  type Fetcher, type Run, type Snapshot, type WorldMeta,
} from '../../src/loader';
import {
  Buildings, DRAPE_LIFT, GroundDrape, MEDIUM_COLORS, OVERLAYS, Pipes, columnColor, groundCell, hexToRgb,
  mediumAt, mediumColor, mediumTexture, soilColor,
} from '../../src/world';
import { legendItems } from '../../src/ui';

const PUBLIC = path.resolve(__dirname, '../../public');
const CAPITOL = 'fixtures/capitol-mini';

const fsFetcher: Fetcher = async (url) => {
  try {
    return new Response(await readFile(path.join(PUBLIC, url)), { status: 200 });
  } catch {
    return new Response('not found', { status: 404 });
  }
};

/** The media of docs/SCENE-CONTRACT.md, in the order a bundle lists them. */
const CONTRACT_MEDIA = ['soil', 'lawn', 'bed', 'mulch', 'gravel', 'concrete', 'asphalt', 'roof', 'water'];

// ---- a synthetic format-4 run, so the reader is exercised without the 13 MB fixture ----

const SYN = { x: 8, y: 8, z: 8, patch: 8 };
const SYN_CELL = 0.5;
const SYN_GW = SYN.x / SYN_CELL; // 16
const SYN_H = 4; // surface layer of every column, so a column's top face is at y = 5
/** The roof block: ground cells 4..7 in both axes, which is exactly ecology columns 2..3 × 2..3. */
const SYN_ROOF = { lo: 4, hi: 7, height: 10 };

function f32le(values: Float32Array): Uint8Array {
  const out = new Uint8Array(values.length * 4);
  const view = new DataView(out.buffer);
  values.forEach((v, i) => view.setFloat32(i * 4, v, true));
  return out;
}

interface Synthetic {
  files: Record<string, string | Uint8Array>;
  meta: Record<string, unknown>;
}

/** A 8×8×8 world over a 16×16 ground grid: all lawn but a 4×4 roof block, flat ground, one snapshot. */
function synthetic(patch: (s: Synthetic) => void = () => {}): Synthetic {
  const n = SYN_GW * SYN_GW;
  const media = CONTRACT_MEDIA;
  const medium = new Uint8Array(n).fill(media.indexOf('lawn'));
  const building = new Float32Array(n);
  for (let gy = SYN_ROOF.lo; gy <= SYN_ROOF.hi; gy++) {
    for (let gx = SYN_ROOF.lo; gx <= SYN_ROOF.hi; gx++) {
      medium[gx + SYN_GW * gy] = media.indexOf('roof');
      building[gx + SYN_GW * gy] = SYN_ROOF.height;
    }
  }
  const ground = new Float32Array(n);
  for (let i = 0; i < n; i++) ground[i] = i / 100; // distinct values, so a byte-order slip shows
  const world: WorldMeta = {
    name: 'syn', ground_cell_m: SYN_CELL, ground_width: SYN_GW, ground_depth: SYN_GW, media,
  };
  const cols = SYN.x * SYN.y;
  const material = new Uint8Array(SYN.x * SYN.y * SYN.z);
  for (let i = 0; i < cols; i++) for (let z = 0; z <= SYN_H; z++) material[i + cols * z] = SOIL;
  const patchRow = { grass: 0.5, shrub: 0, detritus: 0, temperature: 12, burning_ticks_left: 0 };
  const s: Synthetic = {
    meta: {
      format_version: 4, dims: SYN, seed: 1, ticks: 0, snapshot_every: 100, year_len: 4000, water_level: 0,
      snapshots: [0], species: [], world,
    },
    files: {
      '/syn/series.csv':
        'tick,grazers,hunters,trees,grass_mean,shrub_mean,moisture_mean,fertility_mean,detritus_total,temperature\n'
        + '0,0,0,0,0.5,0,100,100,0,12\n',
      '/syn/events.csv': 'tick,kind,species,patch_x,patch_y,x,y,cause,detail\n',
      '/syn/world/ground_h.bin': f32le(ground),
      '/syn/world/medium.bin': medium,
      '/syn/world/building_h.bin': f32le(building),
      '/syn/world/pipes.json': JSON.stringify([
        { id: 'a', inlet: [1, 2], outlet: [0, 2], capacity_m3h: 50, illustrative: true },
        { id: 'b', inlet: [6, 5], outlet: [8, 5], capacity_m3h: 50, illustrative: true },
      ]),
      '/syn/snap_000000/material.bin': material,
      '/syn/snap_000000/light.bin': new Uint8Array(material.length).fill(255),
      '/syn/snap_000000/moisture.bin': new Uint8Array(cols).fill(120),
      '/syn/snap_000000/fertility.bin': new Uint8Array(cols).fill(130),
      '/syn/snap_000000/height.bin': new Uint8Array(cols).fill(SYN_H),
      '/syn/snap_000000/patches.json': JSON.stringify([patchRow]),
      '/syn/snap_000000/entities.json': '[]',
    },
  };
  patch(s);
  return s;
}

function serve(s: Synthetic): Fetcher {
  const files: Record<string, string | Uint8Array> = { ...s.files, '/syn/meta.json': JSON.stringify(s.meta) };
  return async (url) => (url in files ? new Response(files[url] as BodyInit, { status: 200 }) : new Response('', { status: 404 }));
}

const loadSynthetic = async (patch?: (s: Synthetic) => void): Promise<[Run, Snapshot]> => {
  const f = serve(synthetic(patch));
  const run = await loadRun('/syn', f);
  return [run, await loadSnapshot(run, 0, f)];
};

describe('format 4 loader, on a synthetic bundle run', () => {
  it('reads the ground grid, little-endian f32 and all, and hangs it off the run and the snapshot', async () => {
    const [run, snap] = await loadSynthetic();
    const w = run.world!;
    expect(run.meta.format_version).toBe(4);
    expect(w.gw).toBe(16);
    expect(w.gd).toBe(16);
    expect(w.cell).toBe(0.5);
    expect(w.meta.media).toEqual(CONTRACT_MEDIA);
    expect(w.ground_h.length).toBe(256);
    expect(w.medium.length).toBe(256);
    expect(w.building_h.length).toBe(256);
    // Written little-endian by hand above: a native-endianness read would give nonsense on a big-endian CPU.
    [0, 0.01, 0.02, 0.03].forEach((v, i) => expect(w.ground_h[i]).toBeCloseTo(v, 6));
    expect(w.ground_h[255]).toBeCloseTo(2.55, 5);
    expect(w.building_h[4 + 16 * 4]).toBe(10);
    expect(w.building_h[0]).toBe(0);
    expect(w.pipes.map((p) => p.id)).toEqual(['a', 'b']);
    expect(w.pipes[1]).toEqual({ id: 'b', inlet: [6, 5], outlet: [8, 5], capacity_m3h: 50, illustrative: true });
    expect(snap.world).toBe(w);
  });

  it('rejects a world object that disagrees with dims, and a version-3 run keeps no world', async () => {
    const bad = (m: Record<string, unknown>, msg: RegExp) => expect(() => parseMeta(m)).toThrow(msg);
    const base = synthetic().meta;
    const world = base.world as WorldMeta;
    bad({ ...base, world: undefined }, /format_version 4 without a world object/);
    bad({ ...base, world: { ...world, ground_cell_m: 0 } }, /world.ground_cell_m 0/);
    bad({ ...base, world: { ...world, ground_width: 15 } }, /world.ground_width 15, expected 16 for dims.x 8/);
    bad({ ...base, world: { ...world, ground_depth: 32 } }, /world.ground_depth 32, expected 16 for dims.y 8/);
    bad({ ...base, world: { ...world, media: [] } }, /world.media is not a list of names/);
    bad({ ...base, world: { ...world, media: [1, 2] } }, /world.media is not a list of names/);
    // Format 3 is still the noise world's, and carries no ground grid.
    const [run] = await loadSynthetic((s) => {
      s.meta.format_version = 3;
      delete s.meta.world;
    });
    expect(run.world).toBeUndefined();
  });

  it('checks every world file against meta.json', async () => {
    const cases: [(s: Synthetic) => void, RegExp][] = [
      [(s) => { s.files['/syn/world/ground_h.bin'] = new Uint8Array(8); }, /ground_h\.bin: expected 1024 bytes, got 8/],
      [(s) => { s.files['/syn/world/medium.bin'] = new Uint8Array(255); }, /medium\.bin: expected 256 bytes, got 255/],
      [(s) => { s.files['/syn/world/building_h.bin'] = new Uint8Array(4); }, /building_h\.bin: expected 1024 bytes, got 4/],
      [(s) => { delete s.files['/syn/world/pipes.json']; }, /pipes\.json: HTTP 404/],
      [(s) => { s.files['/syn/world/pipes.json'] = '{}'; }, /pipes\.json: not an array/],
      [(s) => { s.files['/syn/world/pipes.json'] = '[{"id":"a","inlet":[1],"outlet":[0,2]}]'; }, /\[0\]: inlet is not an \[x, y\] pair/],
      [(s) => { s.files['/syn/world/pipes.json'] = '[{"id":"a","inlet":[1,2],"outlet":"x"}]'; }, /\[0\]: outlet is not an \[x, y\] pair/],
      [(s) => { (s.files['/syn/world/medium.bin'] as Uint8Array)[7] = 9; }, /medium\.bin: code 9 is not in world.media/],
    ];
    for (const [patch, msg] of cases) {
      await expect(loadRun('/syn', serve(synthetic(patch))), msg.source).rejects.toThrow(msg);
    }
  });
});

describe('the medium overlay', () => {
  it('has one colour per medium of the scene contract, all distinct', () => {
    for (const name of CONTRACT_MEDIA) expect(MEDIUM_COLORS[name], name).toMatch(/^#[0-9a-f]{6}$/);
    const used = CONTRACT_MEDIA.map((n) => MEDIUM_COLORS[n]);
    expect(new Set(used).size).toBe(used.length);
    expect(mediumColor('lawn')).toEqual(hexToRgb(MEDIUM_COLORS.lawn));
    expect(mediumColor('nothing-like-this')).toEqual(hexToRgb(MEDIUM_COLORS.unknown));
  });

  it('reads the ground cell under a column centre and colours the column with it', async () => {
    const [, snap] = await loadSynthetic();
    const w = snap.world!;
    // 0.5 m cells, so column (x, y) has its centre in ground cell (2x + 1, 2y + 1).
    expect(groundCell(w, 0, 0)).toBe(1 + 16 * 1);
    expect(groundCell(w, 3, 2)).toBe(7 + 16 * 5);
    expect(groundCell(w, 7, 7)).toBe(15 + 16 * 15);
    expect(mediumAt(w, 0, 0)).toBe('lawn');
    expect(mediumAt(w, 2, 2)).toBe('roof');
    expect(mediumAt(w, 3, 3)).toBe('roof');
    expect(mediumAt(w, 4, 4)).toBe('lawn');
    expect(columnColor(snap, 2, 2, 'medium')).toEqual(mediumColor('roof'));
    expect(columnColor(snap, 0, 0, 'medium')).toEqual(mediumColor('lawn'));
  });

  it('falls back to the material colour on a run with no ground grid', async () => {
    const run = await loadRun('/fixtures/s42-strip-mini', fsFetcher);
    const snap = await loadSnapshot(run, 0, fsFetcher);
    expect(snap.world).toBeUndefined();
    for (const [x, y] of [[0, 0], [37, 12], [255, 63]]) {
      const p = snap.patches[snap.grid.patchOf(x, y)];
      const mat = snap.material[snap.grid.voxel(x, y, snap.height[snap.grid.column(x, y)])];
      const want = mat === ROCK ? hexToRgb('#8a8a8a') : mat === SOIL ? soilColor(p.grass, p.shrub) : hexToRgb('#3a6fd8');
      expect(columnColor(snap, x, y, 'medium'), `${x},${y}`).toEqual(want);
    }
  });

  it('is an overlay of its own, listed after the ones that came before it', () => {
    expect([...OVERLAYS]).toEqual([
      'material', 'light', 'moisture', 'fertility', 'temperature', 'fire', 'crowding', 'traits', 'medium',
    ]);
  });

  it('builds one nearest-filtered texel per ground cell', async () => {
    const [run] = await loadSynthetic();
    const tex = mediumTexture(run.world!);
    expect([tex.image.width, tex.image.height]).toEqual([16, 16]);
    expect(tex.magFilter).toBe(THREE.NearestFilter);
    expect(tex.minFilter).toBe(THREE.NearestFilter);
    expect(tex.generateMipmaps).toBe(false);
    expect(tex.colorSpace).toBe(THREE.SRGBColorSpace);
    const texel = (gx: number, gy: number) => [...(tex.image.data as Uint8Array).subarray(4 * (gx + 16 * gy), 4 * (gx + 16 * gy) + 3)];
    expect(texel(0, 0)).toEqual(mediumColor('lawn'));
    expect(texel(5, 5)).toEqual(mediumColor('roof'));
  });

  it('keys the legend on the run media, and shows nothing on any other overlay', () => {
    const items = legendItems('medium', CONTRACT_MEDIA);
    expect(items.map((i) => i.label)).toEqual(CONTRACT_MEDIA);
    expect(items[1]).toEqual({ label: 'lawn', css: `rgb(${mediumColor('lawn').join(', ')})` });
    expect(legendItems('medium', [])).toEqual([]);
    expect(legendItems('medium')).toEqual([]);
    for (const o of OVERLAYS) {
      if (o !== 'medium') expect(legendItems(o, CONTRACT_MEDIA), o).toEqual([]);
    }
  });
});

describe('bundle geometry', () => {
  it('drapes one quad per column on the top face of its surface voxel', async () => {
    const [run, snap] = await loadSynthetic();
    const drape = new GroundDrape(run.grid, run.world!);
    expect(drape.mesh.visible).toBe(false);
    drape.set(snap, 'material', true);
    expect(drape.mesh.visible).toBe(false);
    drape.set(snap, 'medium', false);
    expect(drape.mesh.visible).toBe(true);
    const pos = drape.mesh.geometry.getAttribute('position');
    const uv = drape.mesh.geometry.getAttribute('uv');
    expect(pos.count).toBe(SYN.x * SYN.y * 4);
    expect(drape.mesh.geometry.getIndex()!.count).toBe(SYN.x * SYN.y * 6);
    // Column (0, 0) is the south-west one: x 0..1 and, with sim +y as Three -z, z 8..7.
    expect([pos.getX(0), pos.getZ(0)]).toEqual([0, 8]);
    expect(pos.getY(0)).toBeCloseTo(SYN_H + 1 + DRAPE_LIFT, 5);
    expect([pos.getX(2), pos.getZ(2)]).toEqual([1, 7]);
    expect([uv.getX(0), uv.getY(0)]).toEqual([0, 0]);
    expect([uv.getX(2), uv.getY(2)]).toEqual([1 / 8, 1 / 8]);
    const box = new THREE.Box3().setFromBufferAttribute(pos as THREE.BufferAttribute);
    expect([box.min.x, box.min.z]).toEqual([0, 0]);
    expect([box.max.x, box.max.z]).toEqual([8, 8]);
    expect(box.min.y).toBeCloseTo(SYN_H + 1 + DRAPE_LIFT, 5);
    expect(box.max.y).toBeCloseTo(SYN_H + 1 + DRAPE_LIFT, 5);
  });

  it('extrudes roof cells once, emitting only the exposed faces', async () => {
    const [run, snap] = await loadSynthetic();
    const b = new Buildings(run.grid, run.world!);
    b.build(snap, true);
    // 16 roof cells: 16 top quads, and one side quad per cell of the 4×4 block's perimeter (16), not 64.
    expect(b.triangles).toBe((16 + 16) * 2);
    const box = new THREE.Box3().setFromBufferAttribute(b.mesh.geometry.getAttribute('position') as THREE.BufferAttribute);
    // The block stands on the surface (y = 5) and rises by its building height.
    expect(box.min.y).toBe(SYN_H + 1);
    expect(box.max.y).toBe(SYN_H + 1 + SYN_ROOF.height);
    expect([box.min.x, box.max.x]).toEqual([2, 4]);
    expect([box.min.z, box.max.z]).toEqual([4, 6]);
    // Flat shaded: one normal per face, every top vertex pointing straight up.
    const normal = b.mesh.geometry.getAttribute('normal');
    expect([normal.getX(0), normal.getY(0), normal.getZ(0)]).toEqual([0, 1, 0]);
    const before = b.triangles;
    b.build(snap, false); // built once per run
    expect(b.triangles).toBe(before);
  });

  it('draws one dashed segment per pipe, above the terrain and only in the top camera', async () => {
    const [run] = await loadSynthetic();
    const pipes = new Pipes(run.grid, run.world!);
    const pos = pipes.lines.geometry.getAttribute('position');
    expect(pos.count).toBe(4);
    expect([pos.getX(0), pos.getY(0), pos.getZ(0)]).toEqual([1, SYN.z, 8 - 2]);
    expect([pos.getX(1), pos.getZ(1)]).toEqual([0, 8 - 2]);
    expect(pipes.lines.geometry.getAttribute('lineDistance')).toBeTruthy();
    expect((pipes.lines.material as THREE.LineDashedMaterial).dashSize).toBeGreaterThan(0);
    expect(pipes.lines.visible).toBe(false);
    pipes.set(true);
    expect(pipes.lines.visible).toBe(true);
    pipes.set(false);
    expect(pipes.lines.visible).toBe(false);
  });
});

describe('the committed Capitol fixture', () => {
  it('loads at format 4 with the shape ecosim documents', async () => {
    const run = await loadRun(`/${CAPITOL}`, fsFetcher);
    expect(run.meta.format_version).toBe(4);
    expect(run.meta.dims).toEqual({ x: 256, y: 256, z: 32, patch: 8 });
    const w = run.world!;
    expect(w.meta).toMatchObject({ name: 'capitol', ground_cell_m: 0.5, ground_width: 512, ground_depth: 512 });
    expect(w.meta.media).toEqual(CONTRACT_MEDIA);
    expect(w.ground_h.length).toBe(512 * 512);
    expect(w.medium.length).toBe(512 * 512);
    expect(w.building_h.length).toBe(512 * 512);
    const counts: Record<string, number> = {};
    for (const code of w.medium) counts[w.meta.media[code]] = (counts[w.meta.media[code]] ?? 0) + 1;
    expect(counts).toEqual({ lawn: 169876, concrete: 22684, asphalt: 44879, roof: 24705 });
    // Building height is above ground and non-zero on exactly the roof cells (ecosim shot G2).
    let roofs = 0;
    let maxGround = 0;
    for (let i = 0; i < w.medium.length; i++) {
      expect(w.building_h[i] > 0).toBe(w.meta.media[w.medium[i]] === 'roof');
      if (w.building_h[i] > 0) roofs++;
      maxGround = Math.max(maxGround, w.ground_h[i]);
    }
    expect(roofs).toBe(24705);
    expect(maxGround).toBeCloseTo(8.589, 3);
    expect(w.pipes).toHaveLength(4);
    expect(w.pipes[0]).toEqual({ id: 'pipe_1', inlet: [15.75, 136.75], outlet: [0, 136.75], capacity_m3h: 50, illustrative: true });
  });

  it('stands no plant on a roof or road cell', async () => {
    const run = await loadRun(`/${CAPITOL}`, fsFetcher);
    const w = run.world!;
    for (const tick of run.meta.snapshots) {
      const snap = await loadSnapshot(run, tick, fsFetcher);
      const trees = snap.entities.filter((e) => e.kind === 'tree');
      expect(trees.length).toBeGreaterThan(0);
      let onLawn = 0;
      for (const t of trees) {
        const g = snap.grid;
        const top = snap.material[g.voxel(t.x, t.y, snap.height[g.column(t.x, t.y)])];
        expect(top, `tree ${t.id} at ${t.x},${t.y}`).toBe(SOIL); // never a Rock column
        for (let dy = 0; dy < 2; dy++) {
          for (let dx = 0; dx < 2; dx++) {
            expect(w.meta.media[w.medium[2 * t.x + dx + w.gw * (2 * t.y + dy)]]).not.toBe('roof');
          }
        }
        if (mediumAt(w, t.x, t.y) === 'lawn') onLawn++;
      }
      expect(onLawn / trees.length).toBeGreaterThan(0.95);
    }
  });

  it('builds the Capitol buildings as one mesh reaching the dome height', async () => {
    const run = await loadRun(`/${CAPITOL}`, fsFetcher);
    const snap = await loadSnapshot(run, 0, fsFetcher);
    const b = new Buildings(run.grid, run.world!);
    b.build(snap, true);
    // Every roof cell contributes a top quad; the face culling then saves 42% of the sides, since the
    // dome and the terrain steps under it leave many of them exposed (71743 quads against 24705 x 5).
    expect(b.triangles).toBe(71743 * 2);
    const box = new THREE.Box3().setFromBufferAttribute(b.mesh.geometry.getAttribute('position') as THREE.BufferAttribute);
    expect(box.max.y - box.min.y).toBeGreaterThan(70); // the dome, 76 m above its ground
    expect(box.min.x).toBeGreaterThanOrEqual(0);
    expect(box.max.x).toBeLessThanOrEqual(256);
  });

  it('keeps the grid helpers working at 256 × 256', async () => {
    const run = await loadRun(`/${CAPITOL}`, fsFetcher);
    expect(run.grid).toMatchObject({ x: 256, y: 256, z: 32, patch: 8, px: 32, py: 32 });
    expect(run.grid.patches).toBe(1024);
    expect(run.grid.longest).toBe(256);
    expect(new Grid(run.meta.dims).voxels).toBe(256 * 256 * 32);
  });
});
