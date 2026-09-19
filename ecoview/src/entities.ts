// Trees as stacked cubes (trunk + canopy by stage) and animals as spheres, one InstancedMesh per part.
import * as THREE from 'three';
import { DIM_X, DIM_Y, speciesColor, type Meta, type Snapshot } from './loader';
import { traitColor, voxelCenter, type RGB } from './world';

export const CANOPY_FIELD_OPACITY = 0.25;
export const ANIMAL_RADIUS = 0.45;
/** Traits overlay: hunters are drawn in this neutral gray so they aren't read as high-cost (red) grazers. */
export const TRAITS_HUNTER_COLOR = '#555555';

/** Canopy voxels (sim coordinates) for a tree whose trunk voxel is at (x, y, z). */
export function canopyVoxels(x: number, y: number, z: number, stage: string): [number, number, number][] {
  if (stage === 'young') return [[x, y, z + 1]];
  if (stage !== 'mature') return [];
  const out: [number, number, number][] = [];
  for (let dz = 1; dz <= 2; dz++) {
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const cx = x + dx;
        const cy = y + dy;
        if (cx >= 0 && cx < DIM_X && cy >= 0 && cy < DIM_Y) out.push([cx, cy, z + dz]);
      }
    }
  }
  return out;
}

/** An InstancedMesh with a lit and an unlit material that grows its capacity on demand. */
class Layer {
  mesh: THREE.InstancedMesh;
  private capacity = 0;

  constructor(
    private readonly group: THREE.Group,
    private readonly geometry: THREE.BufferGeometry,
    readonly lit: THREE.MeshLambertMaterial,
    readonly unlit: THREE.MeshBasicMaterial,
  ) {
    this.mesh = this.alloc(256);
  }

  private alloc(n: number): THREE.InstancedMesh {
    const mesh = new THREE.InstancedMesh(this.geometry, this.lit, n);
    mesh.frustumCulled = false;
    this.capacity = n;
    return mesh;
  }

  /** `colors`, if given, are per-instance and should be used with a white material. */
  set(positions: THREE.Vector3[], useLit: boolean, visible: boolean, colors?: RGB[]): void {
    if (positions.length > this.capacity) {
      this.group.remove(this.mesh);
      this.mesh.dispose();
      this.mesh = this.alloc(Math.max(positions.length, this.capacity * 2));
    }
    if (!this.mesh.parent) this.group.add(this.mesh);
    const m = new THREE.Matrix4();
    positions.forEach((p, i) => this.mesh.setMatrixAt(i, m.makeTranslation(p.x, p.y, p.z)));
    this.mesh.count = positions.length;
    this.mesh.instanceMatrix.needsUpdate = true;
    if (colors?.length) {
      const c = new THREE.Color();
      colors.forEach(([r, g, b], i) => this.mesh.setColorAt(i, c.setRGB(r / 255, g / 255, b / 255, THREE.SRGBColorSpace)));
      this.mesh.instanceColor!.needsUpdate = true;
    }
    this.mesh.material = useLit ? this.lit : this.unlit;
    this.mesh.visible = visible && positions.length > 0;
  }
}

function materials(color: string): [THREE.MeshLambertMaterial, THREE.MeshBasicMaterial] {
  const c = new THREE.Color().setStyle(color, THREE.SRGBColorSpace);
  return [new THREE.MeshLambertMaterial({ color: c }), new THREE.MeshBasicMaterial({ color: c })];
}

export interface EntityView {
  /** Material overlay draws opaque canopies; field overlays draw them translucent. */
  fieldOverlay: boolean;
  /** Top camera: unlit materials; canopies hidden on field overlays (see DECISIONS.md). */
  top: boolean;
  /** Traits overlay: grazers colored by `energy_cost_mult`, hunters gray. */
  traits: boolean;
}

export class Entities {
  readonly group = new THREE.Group();
  private readonly trunks: Layer;
  private readonly canopies: Layer;
  private readonly grazers: Layer;
  private readonly hunters: Layer;
  private readonly traitGrazers: Layer;
  private readonly grayHunters: Layer;

  constructor(meta: Meta) {
    const box = new THREE.BoxGeometry(1, 1, 1);
    const sphere = new THREE.SphereGeometry(ANIMAL_RADIUS, 10, 6);
    this.trunks = new Layer(this.group, box, ...materials(speciesColor(meta, 'tree')));
    this.canopies = new Layer(this.group, box, ...materials(speciesColor(meta, 'tree', 'canopy_color')));
    this.grazers = new Layer(this.group, sphere, ...materials(speciesColor(meta, 'grazer')));
    this.hunters = new Layer(this.group, sphere, ...materials(speciesColor(meta, 'hunter')));
    this.traitGrazers = new Layer(this.group, sphere, ...materials('#ffffff'));
    this.grayHunters = new Layer(this.group, sphere, ...materials(TRAITS_HUNTER_COLOR));
  }

  build(snap: Snapshot, view: EntityView): void {
    const trunks: THREE.Vector3[] = [];
    const canopies: THREE.Vector3[] = [];
    const grazers: THREE.Vector3[] = [];
    const hunters: THREE.Vector3[] = [];
    const traits: RGB[] = [];
    for (const e of snap.entities) {
      if (e.kind === 'tree') {
        trunks.push(voxelCenter(e.x, e.y, e.z));
        for (const [x, y, z] of canopyVoxels(e.x, e.y, e.z, e.stage)) canopies.push(voxelCenter(x, y, z));
      } else {
        (e.kind === 'grazer' ? grazers : hunters).push(voxelCenter(e.x, e.y, e.z));
        if (e.kind === 'grazer' && view.traits) traits.push(traitColor(e.energy_cost_mult));
      }
    }
    const lit = !view.top;
    for (const mat of [this.canopies.lit, this.canopies.unlit]) {
      mat.transparent = view.fieldOverlay;
      mat.opacity = view.fieldOverlay ? CANOPY_FIELD_OPACITY : 1;
      mat.depthWrite = !view.fieldOverlay;
      mat.needsUpdate = true;
    }
    this.trunks.set(trunks, lit, true);
    this.canopies.set(canopies, lit, !(view.top && view.fieldOverlay));
    this.grazers.set(grazers, lit, !view.traits);
    this.hunters.set(hunters, lit, !view.traits);
    this.traitGrazers.set(view.traits ? grazers : [], lit, view.traits, traits);
    this.grayHunters.set(view.traits ? hunters : [], lit, view.traits);
  }
}
