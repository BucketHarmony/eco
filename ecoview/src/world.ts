// Surface voxels: one InstancedMesh with one top voxel per column, colored by the active overlay.
import * as THREE from 'three';
import { SOIL, WATER, type Grid, type Snapshot, type WorldData } from './loader';

export const OVERLAYS = [
  'material', 'light', 'moisture', 'fertility', 'temperature', 'fire', 'crowding', 'traits', 'medium',
] as const;
export type Overlay = (typeof OVERLAYS)[number];

export type RGB = [number, number, number];

export const COLORS = {
  soil: '#8b6b47',
  rock: '#8a8a8a',
  water: '#3a6fd8',
  grass: '#7cc242',
  shrub: '#2f6b2a',
  moistureLo: '#ffffff',
  moistureHi: '#1f4fd1',
  fertilityLo: '#ffffff',
  fertilityHi: '#4a2c12',
  tempLo: '#2040ff',
  tempHi: '#ff3020',
  fireLo: '#b3300a',
  fireHi: '#ffb020',
  burnt: '#2b2b2b',
  crowdLo: '#ffffff',
  crowdHi: '#d81b9c',
  traitLo: '#1f5bff',
  traitMid: '#ffffff',
  traitHi: '#ff1f1f',
} as const;

/**
 * Medium overlay (format 4): one fixed colour per surface medium of the scene contract, in the order
 * `meta.json` `world.media` lists them. The sim owns the species colours, but a medium is scene geometry
 * and not a species, so these live here (DECISIONS.md, shot G7). An unknown name falls back to `unknown`.
 */
export const MEDIUM_COLORS: Record<string, string> = {
  soil: '#8b6b47',
  lawn: '#79b449',
  bed: '#a8724a',
  mulch: '#6b4a2b',
  gravel: '#b9b2a3',
  concrete: '#d7d3cb',
  asphalt: '#4a4a4e',
  roof: '#9a9a9e',
  water: '#3a6fd8',
  unknown: '#ff00ff',
};

/** Light grey of the extruded buildings, lit in the perspective cameras and flat in the top one. */
export const BUILDING_COLOR = '#c6c6cb';
/** Storm-drain pipes: dashed, because the Capitol's are illustrative (docs/SCENE-CONTRACT.md). */
export const PIPE_COLOR = '#1d6fa5';
export const PIPE_DASH = 2;

/** Fire overlay: a burning patch is brightest at this many ticks left (the sim's `fire.duration` default). */
export const FIRE_TICKS_FULL = 3;
/** Crowding overlay: grazers per patch at full magenta (twice the sim's grazer disease threshold of 16). */
export const CROWDING_FULL = 32;
/** Traits overlay: `energy_cost_mult` this far from the default 1 is fully blue (below) or red (above). */
export const TRAIT_SPAN = 0.25;

export function hexToRgb(hex: string): RGB {
  const n = parseInt(hex.replace('#', ''), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

const clamp01 = (t: number): number => (t < 0 ? 0 : t > 1 ? 1 : t);

export function lerpRgb(a: RGB, b: RGB, t: number): RGB {
  const k = clamp01(t);
  return [
    Math.round(a[0] + (b[0] - a[0]) * k),
    Math.round(a[1] + (b[1] - a[1]) * k),
    Math.round(a[2] + (b[2] - a[2]) * k),
  ];
}

const C = Object.fromEntries(Object.entries(COLORS).map(([k, v]) => [k, hexToRgb(v)])) as Record<
  keyof typeof COLORS,
  RGB
>;

/** Light overlay: gray level v (0..255). */
export const lightColor = (v: number): RGB => {
  const g = Math.round(Math.max(0, Math.min(255, v)));
  return [g, g, g];
};
/** Moisture overlay: white at 0 to blue at 255. */
export const moistureColor = (v: number): RGB => lerpRgb(C.moistureLo, C.moistureHi, v / 255);
/** Fertility overlay: white at 0 to dark brown at 255. */
export const fertilityColor = (v: number): RGB => lerpRgb(C.fertilityLo, C.fertilityHi, v / 255);
/** Temperature overlay: blue at 0 °C to red at 30 °C, clamped. */
export const temperatureColor = (celsius: number): RGB => lerpRgb(C.tempLo, C.tempHi, celsius / 30);
/** Material overlay for a soil column: soil tinted toward grass, then toward shrub. */
export const soilColor = (grass: number, shrub: number): RGB =>
  lerpRgb(lerpRgb(C.soil, C.grass, grass), C.shrub, shrub * 0.8);

/** Fire overlay color of a burning patch: dark orange with 1 tick left to bright orange at FIRE_TICKS_FULL. */
export const burningColor = (ticksLeft: number): RGB =>
  lerpRgb(C.fireLo, C.fireHi, (ticksLeft - 1) / (FIRE_TICKS_FULL - 1));
/** Crowding overlay: white at 0 grazers to magenta at CROWDING_FULL, clamped. */
export const crowdingColor = (grazers: number): RGB => lerpRgb(C.crowdLo, C.crowdHi, grazers / CROWDING_FULL);
/** Traits overlay grazer color: white at the default multiplier 1, blue below, red above, full at ±TRAIT_SPAN. */
export const traitColor = (energyCostMult = 1): RGB => {
  const t = (energyCostMult - 1) / TRAIT_SPAN;
  return t < 0 ? lerpRgb(C.traitMid, C.traitLo, -t) : lerpRgb(C.traitMid, C.traitHi, t);
};

/** Colour of a medium by name, `unknown` magenta for a name this palette doesn't have. */
export const mediumColor = (name: string): RGB => hexToRgb(MEDIUM_COLORS[name] ?? MEDIUM_COLORS.unknown);

/** The ground cell under the centre of ecology column (x, y). */
export function groundCell(w: WorldData, x: number, y: number): number {
  const gx = Math.min(w.gw - 1, Math.floor((x + 0.5) / w.cell));
  const gy = Math.min(w.gd - 1, Math.floor((y + 0.5) / w.cell));
  return gx + w.gw * gy;
}

/** The medium name at the centre of ecology column (x, y). */
export const mediumAt = (w: WorldData, x: number, y: number): string =>
  w.meta.media[w.medium[groundCell(w, x, y)]] ?? 'unknown';

const crowdCache = new WeakMap<Snapshot, Uint16Array>();

/** Live grazers per patch, counted once per snapshot. */
export function grazersPerPatch(snap: Snapshot): Uint16Array {
  let n = crowdCache.get(snap);
  if (!n) {
    const g = snap.grid;
    n = new Uint16Array(g.patches);
    for (const e of snap.entities) if (e.kind === 'grazer') n[g.patchOf(Math.floor(e.x), Math.floor(e.y))]++;
    crowdCache.set(snap, n);
  }
  return n;
}

/** Color of the top voxel of column (x, y) under the given overlay. */
export function columnColor(snap: Snapshot, x: number, y: number, overlay: Overlay): RGB {
  const g = snap.grid;
  const col = g.column(x, y);
  const h = snap.height[col];
  const mat = snap.material[g.voxel(x, y, h)];
  if (overlay === 'light') {
    return lightColor(snap.light[g.voxel(x, y, h + 1)]);
  }
  if (overlay === 'medium') {
    // A noise world has no ground grid, so medium falls back to the material colour.
    if (snap.world) return mediumColor(mediumAt(snap.world, x, y));
    const p0 = snap.patches[g.patchOf(x, y)];
    return mat === WATER ? C.water : mat === SOIL ? soilColor(p0.grass, p0.shrub) : C.rock;
  }
  if (mat === WATER) return C.water;
  const p = g.patchOf(x, y);
  const patch = snap.patches[p];
  const burning = patch.burning_ticks_left ?? 0;
  switch (overlay) {
    case 'material':
    case 'traits':
      return mat === SOIL ? soilColor(patch.grass, patch.shrub) : C.rock;
    case 'fire':
      if (mat !== SOIL) return C.rock;
      if (burning > 0) return burningColor(burning);
      // Burnt ground is a burnout event since the last snapshot (DECISIONS.md, shot 16), not low cover.
      return snap.burnt[p] ? C.burnt : soilColor(patch.grass, patch.shrub);
    case 'crowding':
      return crowdingColor(grazersPerPatch(snap)[p]);
    case 'moisture':
      return mat === SOIL ? moistureColor(snap.moisture[col]) : C.rock;
    case 'fertility':
      return mat === SOIL ? fertilityColor(snap.fertility[col]) : C.rock;
    case 'temperature':
      return temperatureColor(patch.temperature);
  }
}

/**
 * Three-space center of the voxel at sim (x, y, z) in a world `depth` columns deep: sim z is Three y and
 * sim +y is Three -z, so the world spans x in [0, width] and z in [0, depth].
 */
export function voxelCenter(x: number, y: number, z: number, depth: number, out = new THREE.Vector3()): THREE.Vector3 {
  return out.set(x + 0.5, z + 0.5, depth - 0.5 - y);
}

export class World {
  readonly mesh: THREE.InstancedMesh;
  private readonly basic = new THREE.MeshBasicMaterial({ color: 0xffffff });
  private readonly lambert = new THREE.MeshLambertMaterial({ color: 0xffffff });

  /** One instance per column of `grid`; a run with other dims gets a new World. */
  constructor(readonly grid: Grid) {
    this.mesh = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), this.lambert, grid.columns);
    this.mesh.instanceMatrix.setUsage(THREE.StaticDrawUsage);
    this.mesh.frustumCulled = false;
    // Allocate the instanceColor buffer up front.
    this.mesh.setColorAt(0, new THREE.Color(1, 1, 1));
  }

  setLit(lit: boolean): void {
    this.mesh.material = lit ? this.lambert : this.basic;
  }

  build(snap: Snapshot, overlay: Overlay): void {
    const m = new THREE.Matrix4();
    const p = new THREE.Vector3();
    const c = new THREE.Color();
    const grid = this.grid;
    for (let y = 0; y < grid.y; y++) {
      for (let x = 0; x < grid.x; x++) {
        const i = grid.column(x, y);
        voxelCenter(x, y, snap.height[i], grid.y, p);
        m.makeTranslation(p.x, p.y, p.z);
        this.mesh.setMatrixAt(i, m);
        const [r, g, b] = columnColor(snap, x, y, overlay);
        c.setRGB(r / 255, g / 255, b / 255, THREE.SRGBColorSpace);
        this.mesh.setColorAt(i, c);
      }
    }
    this.mesh.instanceMatrix.needsUpdate = true;
    this.mesh.instanceColor!.needsUpdate = true;
  }
}


/** Lifted this far above the surface voxels' top faces so the drape never z-fights them. */
export const DRAPE_LIFT = 0.02;

/** The medium grid as an RGB texture, one texel per ground cell, nearest-filtered so media stay flat. */
export function mediumTexture(world: WorldData): THREE.DataTexture {
  const palette = world.meta.media.map((name) => mediumColor(name));
  const data = new Uint8Array(world.gw * world.gd * 4);
  for (let i = 0; i < world.medium.length; i++) {
    const [r, g, b] = palette[world.medium[i]] ?? mediumColor('unknown');
    data[4 * i] = r;
    data[4 * i + 1] = g;
    data[4 * i + 2] = b;
    data[4 * i + 3] = 255;
  }
  const tex = new THREE.DataTexture(data, world.gw, world.gd, THREE.RGBAFormat);
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.needsUpdate = true;
  return tex;
}

/**
 * The `medium` overlay at the ground grid's resolution (0.5 m at the Capitol), draped on the surface: one
 * quad per ecology column at the top face of its surface voxel, textured with the whole medium grid. The
 * columns stay one instance each, so the finer grid costs a texture rather than 4x the instances
 * (DECISIONS.md, shot G7).
 */
export class GroundDrape {
  readonly mesh: THREE.Mesh;
  private readonly basic: THREE.MeshBasicMaterial;
  private readonly lambert: THREE.MeshLambertMaterial;
  private readonly position: THREE.BufferAttribute;
  private builtFor = -1;

  constructor(readonly grid: Grid, readonly world: WorldData) {
    const quads = grid.columns;
    const position = new THREE.BufferAttribute(new Float32Array(quads * 4 * 3), 3);
    const uv = new THREE.BufferAttribute(new Float32Array(quads * 4 * 2), 2);
    const normal = new THREE.BufferAttribute(new Float32Array(quads * 4 * 3), 3);
    const index = new Uint32Array(quads * 6);
    for (let q = 0, y = 0; y < grid.y; y++) {
      for (let x = 0; x < grid.x; x++, q++) {
        // Corners a, b, c, d anticlockwise seen from above; (a, c, d) and (a, b, c) face up.
        uv.setXY(4 * q, x / grid.x, y / grid.y);
        uv.setXY(4 * q + 1, (x + 1) / grid.x, y / grid.y);
        uv.setXY(4 * q + 2, (x + 1) / grid.x, (y + 1) / grid.y);
        uv.setXY(4 * q + 3, x / grid.x, (y + 1) / grid.y);
        for (let k = 0; k < 4; k++) normal.setXYZ(4 * q + k, 0, 1, 0);
        index.set([4 * q, 4 * q + 2, 4 * q + 3, 4 * q, 4 * q + 1, 4 * q + 2], 6 * q);
      }
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', position);
    geometry.setAttribute('uv', uv);
    geometry.setAttribute('normal', normal);
    geometry.setIndex(new THREE.BufferAttribute(index, 1));
    this.position = position;

    const map = mediumTexture(world);
    const opts = { map, polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1 };
    this.basic = new THREE.MeshBasicMaterial(opts);
    this.lambert = new THREE.MeshLambertMaterial(opts);
    this.mesh = new THREE.Mesh(geometry, this.lambert);
    this.mesh.frustumCulled = false;
    this.mesh.visible = false;
  }

  /** Shown only on the `medium` overlay; the quads follow the surface, so they move with the snapshot. */
  set(snap: Snapshot, overlay: Overlay, lit: boolean): void {
    this.mesh.visible = overlay === 'medium';
    this.mesh.material = lit ? this.lambert : this.basic;
    if (!this.mesh.visible || this.builtFor === snap.tick) return;
    const g = this.grid;
    const p = this.position;
    for (let q = 0, y = 0; y < g.y; y++) {
      const z0 = g.y - y;
      for (let x = 0; x < g.x; x++, q++) {
        const h = snap.height[g.column(x, y)] + 1 + DRAPE_LIFT;
        p.setXYZ(4 * q, x, h, z0);
        p.setXYZ(4 * q + 1, x + 1, h, z0);
        p.setXYZ(4 * q + 2, x + 1, h, z0 - 1);
        p.setXYZ(4 * q + 3, x, h, z0 - 1);
      }
    }
    p.needsUpdate = true;
    this.builtFor = snap.tick;
  }
}

/**
 * Roof cells extruded to their building height, as one merged flat-shaded mesh built once per run. Only
 * exposed faces are emitted - every cell's top, and a side only where the neighbour's top is lower - which
 * on the Capitol's 24705 roof cells is 71743 quads against the 123525 of a box per cell.
 */
export class Buildings {
  readonly mesh: THREE.Mesh;
  private readonly lambert: THREE.MeshLambertMaterial;
  private readonly basic: THREE.MeshBasicMaterial;
  private built = false;

  constructor(readonly grid: Grid, readonly world: WorldData) {
    const color = new THREE.Color().setStyle(BUILDING_COLOR, THREE.SRGBColorSpace);
    this.lambert = new THREE.MeshLambertMaterial({ color, flatShading: true });
    this.basic = new THREE.MeshBasicMaterial({ color });
    this.mesh = new THREE.Mesh(new THREE.BufferGeometry(), this.lambert);
    this.mesh.frustumCulled = false;
  }

  /** Builds the geometry on the first snapshot; a bundle world's ground never moves after that. */
  build(snap: Snapshot, lit: boolean): void {
    this.mesh.material = lit ? this.lambert : this.basic;
    if (this.built) return;
    this.built = true;
    const w = this.world;
    const g = this.grid;
    const roof = w.meta.media.indexOf('roof');
    const verts: number[] = [];
    // A cell's floor is the top of the surface voxel of the ecology column it sits in, so a building
    // always stands on the drawn terrain; its roof is that floor plus the bundle's building height.
    const base = (gx: number, gy: number): number => {
      const x = Math.min(g.x - 1, Math.floor(gx * w.cell));
      const y = Math.min(g.y - 1, Math.floor(gy * w.cell));
      return snap.height[g.column(x, y)] + 1;
    };
    const isRoof = (gx: number, gy: number): boolean => w.medium[gx + w.gw * gy] === roof;
    const topOf = (gx: number, gy: number): number => base(gx, gy) + w.building_h[gx + w.gw * gy];
    const quad = (a: number[], b: number[], c: number[], d: number[]): void => {
      verts.push(...a, ...b, ...c, ...a, ...c, ...d);
    };
    for (let gy = 0; gy < w.gd; gy++) {
      for (let gx = 0; gx < w.gw; gx++) {
        if (!isRoof(gx, gy)) continue;
        const top = topOf(gx, gy);
        const x0 = gx * w.cell;
        const x1 = x0 + w.cell;
        const z0 = g.y - gy * w.cell;
        const z1 = z0 - w.cell;
        quad([x0, top, z0], [x1, top, z0], [x1, top, z1], [x0, top, z1]);
        // Sides, each from the neighbour's top (its roof, or its ground) up to this cell's.
        const sides: [number, number, number, number, number, number][] = [
          [1, 0, x1, z0, x1, z1],
          [-1, 0, x0, z1, x0, z0],
          [0, 1, x1, z1, x0, z1],
          [0, -1, x0, z0, x1, z0],
        ];
        for (const [dx, dy, ax, az, bx, bz] of sides) {
          const nx = gx + dx;
          const ny = gy + dy;
          const out = nx < 0 || nx >= w.gw || ny < 0 || ny >= w.gd;
          const nTop = out ? base(gx, gy) : isRoof(nx, ny) ? topOf(nx, ny) : base(nx, ny);
          if (nTop >= top) continue;
          quad([ax, nTop, az], [bx, nTop, bz], [bx, top, bz], [ax, top, az]);
        }
      }
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(verts), 3));
    geometry.computeVertexNormals(); // one normal per face: the geometry is not indexed
    this.mesh.geometry.dispose();
    this.mesh.geometry = geometry;
  }

  /** Triangles in the merged mesh, for the tests. */
  get triangles(): number {
    return (this.mesh.geometry.getAttribute('position')?.count ?? 0) / 3;
  }
}

/** Storm-drain pipes as one dashed line per pipe, inlet to outlet, drawn in the top camera only. */
export class Pipes {
  readonly lines: THREE.LineSegments;

  constructor(grid: Grid, world: WorldData) {
    const pts: number[] = [];
    // Above the terrain and drawn without depth testing, so a pipe under a building is still visible.
    const y = grid.z;
    for (const p of world.pipes) {
      pts.push(p.inlet[0], y, grid.y - p.inlet[1], p.outlet[0], y, grid.y - p.outlet[1]);
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(pts), 3));
    this.lines = new THREE.LineSegments(
      geometry,
      new THREE.LineDashedMaterial({
        color: new THREE.Color().setStyle(PIPE_COLOR, THREE.SRGBColorSpace),
        dashSize: PIPE_DASH,
        gapSize: PIPE_DASH,
        depthTest: false,
      }),
    );
    this.lines.computeLineDistances();
    this.lines.renderOrder = 10;
    this.lines.frustumCulled = false;
    this.lines.visible = false;
  }

  /** The pipes are illustrative and read only from above, so they show in the top camera alone. */
  set(top: boolean): void {
    this.lines.visible = top;
  }
}

// ---- edit mode: the ground grid drawn as itself, in chunks (shot E1) ----

/** Ground cells to a side of one chunk, and so of one InstancedMesh. */
export const CHUNK = 32;
/** How far a ground box reaches below its top face; every box's bottom is the same, so no gap can show. */
export const GROUND_SKIRT = 1;
/** The targeted cell's outline, lifted this far above its top face so it never z-fights it. */
export const OUTLINE_LIFT = 0.05;
export const OUTLINE_COLOR = '#ff2ba6';

/**
 * A world bundle drawn at the ground grid's own resolution: one `InstancedMesh` of ground boxes and one of
 * building boxes per `CHUNK` × `CHUNK` cells, so an edit rebuilds only the chunks it touched. The Capitol is
 * 512 × 512 cells, which is 256 chunks of 1024 boxes (DECISIONS.md, shot E1).
 */
export class GroundChunks {
  readonly group = new THREE.Group();
  /** Chunks along x and along y. */
  readonly cx: number;
  readonly cy: number;
  private readonly ground: THREE.InstancedMesh[] = [];
  private readonly building: THREE.InstancedMesh[] = [];
  private readonly geometry = new THREE.BoxGeometry(1, 1, 1);
  private readonly palette: RGB[];
  private readonly groundMat: [THREE.MeshLambertMaterial, THREE.MeshBasicMaterial];
  private readonly buildingMat: [THREE.MeshLambertMaterial, THREE.MeshBasicMaterial];

  /** `depthM` is the world's north-south extent in metres: sim +y is three -z, as everywhere else. */
  constructor(readonly world: WorldData, readonly depthM: number) {
    this.cx = Math.ceil(world.gw / CHUNK);
    this.cy = Math.ceil(world.gd / CHUNK);
    this.palette = world.meta.media.map((name) => mediumColor(name));
    this.groundMat = [
      new THREE.MeshLambertMaterial({ color: 0xffffff }),
      new THREE.MeshBasicMaterial({ color: 0xffffff }),
    ];
    const b = new THREE.Color().setStyle(BUILDING_COLOR, THREE.SRGBColorSpace);
    this.buildingMat = [
      new THREE.MeshLambertMaterial({ color: b, flatShading: true }),
      new THREE.MeshBasicMaterial({ color: b }),
    ];
    for (let c = 0; c < this.cx * this.cy; c++) {
      const { w, d } = this.range(c);
      const g = new THREE.InstancedMesh(this.geometry, this.groundMat[0], w * d);
      g.setColorAt(0, new THREE.Color(1, 1, 1)); // allocate instanceColor up front
      const bl = new THREE.InstancedMesh(this.geometry, this.buildingMat[0], w * d);
      for (const m of [g, bl]) {
        m.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
        this.group.add(m);
      }
      this.ground.push(g);
      this.building.push(bl);
      this.rebuild(c);
    }
  }

  /** The cell range of chunk `c`, clipped at the grid's edges. */
  range(c: number): { x0: number; y0: number; w: number; d: number } {
    const x0 = (c % this.cx) * CHUNK;
    const y0 = Math.floor(c / this.cx) * CHUNK;
    return { x0, y0, w: Math.min(CHUNK, this.world.gw - x0), d: Math.min(CHUNK, this.world.gd - y0) };
  }

  chunkOf(gx: number, gy: number): number {
    return Math.floor(gx / CHUNK) + this.cx * Math.floor(gy / CHUNK);
  }

  /** The chunks holding the given ground-cell indexes, each once. */
  chunksOf(cells: Iterable<number>): number[] {
    const out = new Set<number>();
    for (const i of cells) out.add(this.chunkOf(i % this.world.gw, Math.floor(i / this.world.gw)));
    return [...out];
  }

  /** Writes chunk `c`'s boxes from the current grids; the only place the geometry follows an edit. */
  rebuild(c: number): void {
    const w = this.world;
    const { x0, y0, w: cw, d: cd } = this.range(c);
    const m = new THREE.Matrix4();
    const col = new THREE.Color();
    const g = this.ground[c];
    const b = this.building[c];
    let nb = 0;
    for (let ly = 0; ly < cd; ly++) {
      for (let lx = 0; lx < cw; lx++) {
        const i = x0 + lx + w.gw * (y0 + ly);
        const h = w.ground_h[i];
        const x = (x0 + lx + 0.5) * w.cell;
        const z = this.depthM - (y0 + ly + 0.5) * w.cell;
        const t = h + GROUND_SKIRT;
        m.makeScale(w.cell, t, w.cell);
        m.setPosition(x, h - t / 2, z);
        g.setMatrixAt(ly * cw + lx, m);
        const [r, gr, bl] = this.palette[w.medium[i]] ?? mediumColor('unknown');
        g.setColorAt(ly * cw + lx, col.setRGB(r / 255, gr / 255, bl / 255, THREE.SRGBColorSpace));
        const bh = w.building_h[i];
        if (bh > 0) {
          m.makeScale(w.cell, bh, w.cell);
          m.setPosition(x, h + bh / 2, z);
          b.setMatrixAt(nb++, m);
        }
      }
    }
    g.instanceMatrix.needsUpdate = true;
    g.instanceColor!.needsUpdate = true;
    b.count = nb;
    b.instanceMatrix.needsUpdate = true;
    // An instanced mesh is culled by its own bounding sphere, which has to be taken from the matrices: with
    // it, a first-person view pays for the chunks it can see rather than all 256 of them.
    for (const m of [g, b]) m.computeBoundingSphere();
  }

  setLit(lit: boolean): void {
    const k = lit ? 0 : 1;
    for (const m of this.ground) m.material = this.groundMat[k];
    for (const m of this.building) m.material = this.buildingMat[k];
  }

  /** Boxes drawn: ground is every cell, buildings only the cells with a height. */
  get instances(): [number, number] {
    return [this.world.gw * this.world.gd, this.building.reduce((n, m) => n + m.count, 0)];
  }
}

/** The brush outline: the top square of every cell in the brush, at that cell's top face. */
export class Outline {
  readonly lines: THREE.LineSegments;
  private readonly capacity: number;
  private readonly position: THREE.BufferAttribute;

  constructor(readonly world: WorldData, readonly depthM: number, maxCells: number) {
    this.capacity = maxCells;
    this.position = new THREE.BufferAttribute(new Float32Array(maxCells * 8 * 3), 3);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', this.position);
    this.lines = new THREE.LineSegments(
      geometry,
      new THREE.LineBasicMaterial({
        color: new THREE.Color().setStyle(OUTLINE_COLOR, THREE.SRGBColorSpace),
        depthTest: false,
      }),
    );
    this.lines.renderOrder = 20;
    this.lines.frustumCulled = false;
    this.lines.visible = false;
  }

  /** `cells` are ground-cell indexes; an empty list hides the outline. */
  set(cells: number[]): void {
    const w = this.world;
    const p = this.position;
    let v = 0;
    for (const i of cells.slice(0, this.capacity)) {
      const gx = i % w.gw;
      const gy = Math.floor(i / w.gw);
      const y = w.ground_h[i] + w.building_h[i] + OUTLINE_LIFT;
      const x0 = gx * w.cell;
      const x1 = x0 + w.cell;
      const z0 = this.depthM - gy * w.cell;
      const z1 = z0 - w.cell;
      const corners: [number, number][] = [[x0, z0], [x1, z0], [x1, z1], [x0, z1]];
      for (let k = 0; k < 4; k++) {
        const [ax, az] = corners[k];
        const [bx, bz] = corners[(k + 1) % 4];
        p.setXYZ(v++, ax, y, az);
        p.setXYZ(v++, bx, y, bz);
      }
    }
    this.lines.geometry.setDrawRange(0, v);
    p.needsUpdate = true;
    this.lines.visible = v > 0;
  }
}
