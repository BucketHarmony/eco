// Surface voxels: one InstancedMesh with one top voxel per column, colored by the active overlay.
import * as THREE from 'three';
import { SOIL, WATER, type Grid, type Snapshot } from './loader';

export const OVERLAYS = [
  'material', 'light', 'moisture', 'fertility', 'temperature', 'fire', 'crowding', 'traits',
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

