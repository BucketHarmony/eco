// Surface voxels: one InstancedMesh with one top voxel per column, colored by the active overlay.
import * as THREE from 'three';
import {
  COLUMNS, DIM_X, DIM_Y, SOIL, WATER, columnIndex, patchOf, voxelIndex,
  type Snapshot,
} from './loader';

export const OVERLAYS = ['material', 'light', 'moisture', 'fertility', 'temperature'] as const;
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
} as const;

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

/** Color of the top voxel of column (x, y) under the given overlay. */
export function columnColor(snap: Snapshot, x: number, y: number, overlay: Overlay): RGB {
  const col = columnIndex(x, y);
  const h = snap.height[col];
  const mat = snap.material[voxelIndex(x, y, h)];
  if (overlay === 'light') {
    return lightColor(snap.light[voxelIndex(x, y, h + 1)]);
  }
  if (mat === WATER) return C.water;
  const patch = snap.patches[patchOf(x, y)];
  switch (overlay) {
    case 'material':
      return mat === SOIL ? soilColor(patch.grass, patch.shrub) : C.rock;
    case 'moisture':
      return mat === SOIL ? moistureColor(snap.moisture[col]) : C.rock;
    case 'fertility':
      return mat === SOIL ? fertilityColor(snap.fertility[col]) : C.rock;
    case 'temperature':
      return temperatureColor(patch.temperature);
  }
}

/** Three-space center of the voxel at sim (x, y, z): sim z is Three y, sim +y is Three -z. */
export function voxelCenter(x: number, y: number, z: number, out = new THREE.Vector3()): THREE.Vector3 {
  return out.set(x + 0.5, z + 0.5, 63.5 - y);
}

export class World {
  readonly mesh: THREE.InstancedMesh;
  private readonly basic = new THREE.MeshBasicMaterial({ color: 0xffffff });
  private readonly lambert = new THREE.MeshLambertMaterial({ color: 0xffffff });

  constructor() {
    this.mesh = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), this.lambert, COLUMNS);
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
    for (let y = 0; y < DIM_Y; y++) {
      for (let x = 0; x < DIM_X; x++) {
        const i = columnIndex(x, y);
        voxelCenter(x, y, snap.height[i], p);
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

