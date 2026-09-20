// Trees as stacked cubes (trunk + canopy by stage) and animals as spheres, one InstancedMesh per part.
import * as THREE from 'three';
import { speciesColor, type BundleShrub, type BundleTree, type Grid, type Meta, type Snapshot, type WorldData } from './loader';
import { COLORS, traitColor, voxelCenter, type RGB } from './world';

export const CANOPY_FIELD_OPACITY = 0.25;
export const ANIMAL_RADIUS = 0.45;
/** Traits overlay: hunters are drawn in this neutral gray so they aren't read as high-cost (red) grazers. */
export const TRAITS_HUNTER_COLOR = '#555555';

/** Canopy voxels (sim coordinates) for a tree whose trunk voxel is at (x, y, z), clipped to the grid. */
export function canopyVoxels(grid: Grid, x: number, y: number, z: number, stage: string): [number, number, number][] {
  if (stage === 'young') return [[x, y, z + 1]];
  if (stage !== 'mature') return [];
  const out: [number, number, number][] = [];
  for (let dz = 1; dz <= 2; dz++) {
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const cx = x + dx;
        const cy = y + dy;
        if (cx >= 0 && cx < grid.x && cy >= 0 && cy < grid.y) out.push([cx, cy, z + dz]);
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

  private readonly grid: Grid;

  constructor(meta: Meta, grid: Grid) {
    this.grid = grid;
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
    const g = this.grid;
    for (const e of snap.entities) {
      if (e.kind === 'tree') {
        trunks.push(voxelCenter(e.x, e.y, e.z, g.y));
        for (const [x, y, z] of canopyVoxels(g, e.x, e.y, e.z, e.stage)) canopies.push(voxelCenter(x, y, z, g.y));
      } else {
        (e.kind === 'grazer' ? grazers : hunters).push(voxelCenter(e.x, e.y, e.z, g.y));
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

// ---- a world bundle's own plants (shot E1) ----

/** Trunk side in metres; the bundle gives a crown radius but no trunk thickness. */
export const TRUNK_SIDE = 0.5;

/** Ground height in metres at a scene position, from the ground cell it falls in. */
export function groundHeightAt(w: WorldData, x: number, y: number): number {
  const gx = Math.min(w.gw - 1, Math.max(0, Math.floor(x / w.cell)));
  const gy = Math.min(w.gd - 1, Math.max(0, Math.floor(y / w.cell)));
  return w.ground_h[gx + w.gw * gy];
}

/**
 * The trees and shrubs a world bundle carries, drawn as a trunk box under an ellipsoid crown and as one
 * ellipsoid per shrub. The editor never changes them; they are here so the ground being edited is read in
 * the scene it belongs to (shot E1). A bundle has no `meta.json`, so the two colours come from `COLORS`.
 */
export class BundlePlants {
  readonly group = new THREE.Group();
  private readonly meshes: [THREE.InstancedMesh, THREE.MeshLambertMaterial, THREE.MeshBasicMaterial][] = [];

  constructor(world: WorldData, depthM: number, trees: BundleTree[], shrubs: BundleShrub[]) {
    const box = new THREE.BoxGeometry(1, 1, 1);
    const ball = new THREE.SphereGeometry(0.5, 12, 8);
    const q = new THREE.Quaternion();
    const pos = new THREE.Vector3();
    const scale = new THREE.Vector3();
    const m = new THREE.Matrix4();
    const add = (geometry: THREE.BufferGeometry, n: number, color: string): THREE.InstancedMesh => {
      const c = new THREE.Color().setStyle(color, THREE.SRGBColorSpace);
      const lit = new THREE.MeshLambertMaterial({ color: c });
      const unlit = new THREE.MeshBasicMaterial({ color: c });
      const mesh = new THREE.InstancedMesh(geometry, lit, Math.max(1, n));
      mesh.frustumCulled = false;
      mesh.count = n;
      this.group.add(mesh);
      this.meshes.push([mesh, lit, unlit]);
      return mesh;
    };
    const trunks = add(box, trees.length, COLORS.soil);
    const crowns = add(ball, trees.length, COLORS.shrub);
    trees.forEach((t, i) => {
      const base = groundHeightAt(world, t.x, t.y);
      const z = depthM - t.y;
      const crownH = Math.max(0.5, t.height - t.crown_base);
      trunks.setMatrixAt(i, m.compose(
        pos.set(t.x, base + t.crown_base / 2, z), q, scale.set(TRUNK_SIDE, Math.max(0.5, t.crown_base), TRUNK_SIDE),
      ));
      crowns.setMatrixAt(i, m.compose(
        pos.set(t.x, base + t.crown_base + crownH / 2, z), q,
        scale.set(2 * t.crown_radius, crownH, 2 * t.crown_radius),
      ));
    });
    const bushes = add(ball, shrubs.length, COLORS.shrub);
    shrubs.forEach((sh, i) => {
      const base = groundHeightAt(world, sh.x, sh.y);
      bushes.setMatrixAt(i, m.compose(
        pos.set(sh.x, base + sh.height / 2, depthM - sh.y),
        q.setFromAxisAngle(new THREE.Vector3(0, 1, 0), sh.angle),
        scale.set(2 * sh.rx, sh.height, 2 * sh.ry),
      ));
    });
    for (const [mesh] of this.meshes) mesh.instanceMatrix.needsUpdate = true;
  }

  setLit(lit: boolean): void {
    for (const [mesh, on, off] of this.meshes) mesh.material = lit ? on : off;
  }
}
