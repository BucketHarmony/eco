// Edit mode (shot E1): corrects a world bundle's ground height, surface medium and building volume.
// The operation list is the only source of truth for undo, the save diff and the tests.
import * as THREE from 'three';
import { PointerLockControls } from 'three/examples/jsm/controls/PointerLockControls.js';
import { BUNDLE_VERBATIM, type Bundle, type WorldData } from './loader';
import { GroundChunks, Outline, groundLevelAt, levelOf, topLevelAt } from './world';
import { BundlePlants } from './entities';

export const MAX_BRUSH = 9;

/**
 * The medium a dig exposes. A column carries one medium, which is its surface, so taking its top cube
 * away has to leave what is under the surface: `media[0]`, which `ecosim`'s `Bundle::load` requires to be
 * `soil` and the loader checks (DECISIONS.md, shot E3).
 */
export const SUBSURFACE = 0;

/**
 * The hotbar, keys 1-8. The six medium slots name a medium of the scene contract; the labels read in site
 * terms, because "walk" and "road" are what a garden plan calls concrete and asphalt (shots/E1).
 */
export const SLOTS = [
  { key: 'lawn', label: 'lawn' },
  { key: 'bed', label: 'bed' },
  { key: 'concrete', label: 'walk (concrete)' },
  { key: 'asphalt', label: 'road (asphalt)' },
  { key: 'water', label: 'water' },
  { key: 'gravel', label: 'gravel' },
  { key: 'ground', label: 'ground' },
  { key: 'building', label: 'building' },
] as const;
export type Slot = (typeof SLOTS)[number]['key'];
export const SLOT_KEYS = SLOTS.map((s) => s.key) as readonly Slot[];
export const isSlot = (s: string): s is Slot => (SLOT_KEYS as readonly string[]).includes(s);

export type Action = 'remove' | 'place' | 'flatten';

/** Everything one ground cell holds that an edit can change. */
export interface CellState {
  ground_h: number;
  medium: number;
  building_h: number;
}

export interface CellEdit {
  /** Ground-grid index, `gx + gw * gy`. */
  i: number;
  before: CellState;
  after: CellState;
}

/** One edit. `applyOp` is the only thing that writes the grids. */
export interface Op {
  kind: Action;
  cells: CellEdit[];
}

export const cellState = (w: WorldData, i: number): CellState => ({
  ground_h: w.ground_h[i],
  medium: w.medium[i],
  building_h: w.building_h[i],
});

export const sameState = (a: CellState, b: CellState): boolean =>
  a.ground_h === b.ground_h && a.medium === b.medium && a.building_h === b.building_h;

export function applyOp(w: WorldData, op: Op, side: 'before' | 'after' = 'after'): void {
  for (const c of op.cells) {
    const s = c[side];
    w.ground_h[c.i] = s.ground_h;
    w.medium[c.i] = s.medium;
    w.building_h[c.i] = s.building_h;
  }
}

/** An op over `cells`, keeping only the cells `next` actually changes; null when it changes nothing. */
export function makeOp(
  w: WorldData,
  kind: Action,
  cells: number[],
  next: (s: CellState, i: number) => CellState,
): Op | null {
  const out: CellEdit[] = [];
  for (const i of cells) {
    const before = cellState(w, i);
    const after = next({ ...before }, i);
    // The grids are f32, so the op records what the grid will actually hold: 1.03 + 0.5 rounds when it
    // crosses into the next binade, and an op whose `after` missed that would never undo exactly.
    after.ground_h = Math.fround(after.ground_h);
    after.building_h = Math.fround(after.building_h);
    if (!sameState(before, after)) out.push({ i, before, after });
  }
  return out.length ? { kind, cells: out } : null;
}

/**
 * The brush: cells within `r - 1` of (gx, gy) by Euclidean distance, clipped to the grid. Radius 1 is the one
 * targeted cell, radius 9 is 197 cells.
 */
export function disc(w: WorldData, gx: number, gy: number, r: number): number[] {
  const reach = Math.max(0, Math.min(MAX_BRUSH, Math.round(r)) - 1);
  const out: number[] = [];
  for (let dy = -reach; dy <= reach; dy++) {
    for (let dx = -reach; dx <= reach; dx++) {
      if (dx * dx + dy * dy > reach * reach) continue;
      const x = gx + dx;
      const y = gy + dy;
      if (x >= 0 && y >= 0 && x < w.gw && y < w.gd) out.push(x + w.gw * y);
    }
  }
  return out;
}

/** The cell the crosshair points at. */
export interface Target {
  i: number;
  gx: number;
  gy: number;
  /** True when the ray came down onto a top face; false when it struck a vertical face. */
  top: boolean;
  /** The cell a `building` place acts on: the target itself, or the neighbour a side hit came from. */
  adj: number;
}

/**
 * Marches `ray` in quarter-cell steps and returns the first cell whose drawn top is above the sample. A
 * heightfield march rather than a raycast against the instanced meshes: the Capitol's 364029 cubes would
 * be 364029 box intersections per pick (DECISIONS.md, shots E1 and E3). The march tests the *drawn* top,
 * quantised to the cube lattice, so the crosshair lands on the cube the eye sees and not on the
 * continuous height under it.
 */
export function pickCell(w: WorldData, depthM: number, ray: THREE.Ray, ceiling: number, maxDist = 1600): Target | null {
  const step = w.cell / 4;
  const p = new THREE.Vector3();
  let prev = -1;
  for (let t = 0; t <= maxDist; t += step) {
    ray.at(t, p);
    if (p.y > ceiling && ray.direction.y >= 0) return null;
    const gx = Math.floor(p.x / w.cell);
    const gy = Math.floor((depthM - p.z) / w.cell);
    if (gx < 0 || gy < 0 || gx >= w.gw || gy >= w.gd) {
      prev = -1;
      continue;
    }
    const i = gx + w.gw * gy;
    if (p.y <= topLevelAt(w, i) * w.cell) {
      const top = prev < 0 || prev === i;
      return { i, gx, gy, top, adj: top ? i : prev };
    }
    prev = i;
  }
  return null;
}

/**
 * The op an action produces, or null when it would change nothing (an unknown medium, a floor at 0).
 *
 * A place adds exactly one cube of the selected slot and a remove takes exactly one away, whichever slot
 * is held: the two are mirrors, and the cube's edge is the bundle's `ground_cell_m` in every direction
 * (shot E3). The heights stay continuous in the grids; a place snaps the column onto the cube lattice on
 * its way up, which is the snap the user sees the first time they click on unedited LiDAR ground.
 */
export function opFor(w: WorldData, action: Action, t: Target, slot: Slot, brush: number): Op | null {
  const c = w.cell;
  const brushAt = (i: number): number[] => disc(w, i % w.gw, Math.floor(i / w.gw), brush);
  if (action === 'remove') {
    // The top cube of the stack goes: the building's if it has one, otherwise the ground's. Digging into
    // a column exposes what is under the surface, which is the bundle's first medium, soil.
    return makeOp(w, 'remove', brushAt(t.i), (s) => {
      if (s.building_h > 0) {
        s.building_h = Math.max(0, (levelOf(s.ground_h + s.building_h, c) - 1) * c - s.ground_h);
      } else if (levelOf(s.ground_h, c) > 0) {
        s.ground_h = (levelOf(s.ground_h, c) - 1) * c;
        s.medium = SUBSURFACE; // what a dig exposes: the bundle's first medium, which ecosim pins to soil
      }
      return s;
    });
  }
  if (action === 'flatten') {
    const h = w.ground_h[t.i];
    const bh = w.building_h[t.i];
    return makeOp(w, 'flatten', brushAt(t.i), (s) => {
      s.ground_h = h;
      if (s.building_h > 0) s.building_h = bh;
      return s;
    });
  }
  // A hit on a vertical face places into the cell the ray came from, so a cube lands on the face you can
  // see rather than inside the column behind it.
  const cells = brushAt(t.top ? t.i : t.adj);
  if (slot === 'building') {
    return makeOp(w, 'place', cells, (s) => {
      s.building_h = (levelOf(s.ground_h + s.building_h, c) + 1) * c - s.ground_h;
      return s;
    });
  }
  // `ground` raises the column in its own material; the six medium slots raise it and paint it as well.
  const code = slot === 'ground' ? -1 : w.meta.media.indexOf(slot);
  if (slot !== 'ground' && code < 0) return null;
  return makeOp(w, 'place', cells, (s) => {
    s.ground_h = (levelOf(s.ground_h, c) + 1) * c;
    if (code >= 0) s.medium = code;
    return s;
  });
}

/**
 * The slot the cube under the crosshair is made of, for the middle-click pick: `building` for a building
 * cube, the column's medium when a slot names it, and `ground` for a medium no slot can place (soil,
 * mulch or a roof cell whose building has been taken away).
 */
export function slotAt(w: WorldData, t: Target): Slot {
  if (topLevelAt(w, t.i) > groundLevelAt(w, t.i)) return 'building';
  const name = w.meta.media[w.medium[t.i]] ?? '';
  return isSlot(name) ? name : 'ground';
}

/** The op stack: unbounded within the session, and the record a save writes out. */
export class History {
  private readonly stack: Op[] = [];
  private at = 0;
  private savedAt = 0;

  constructor(private readonly w: WorldData) {}

  push(op: Op): void {
    this.stack.length = this.at;
    this.stack.push(op);
    applyOp(this.w, op);
    this.at++;
  }

  undo(): Op | null {
    if (this.at === 0) return null;
    const op = this.stack[--this.at];
    applyOp(this.w, op, 'before');
    return op;
  }

  redo(): Op | null {
    if (this.at >= this.stack.length) return null;
    const op = this.stack[this.at++];
    applyOp(this.w, op);
    return op;
  }

  markSaved(): void {
    this.savedAt = this.at;
  }

  get applied(): Op[] {
    return this.stack.slice(0, this.at);
  }

  get dirty(): boolean {
    return this.at !== this.savedAt;
  }

  /** Undoable and redoable counts, for the panel. */
  get counts(): [number, number] {
    return [this.at, this.stack.length - this.at];
  }

  /** Cells whose current state differs from the first `before` any applied op recorded for them. */
  get changed(): number[] {
    const first = new Map<number, CellState>();
    for (const op of this.applied) for (const c of op.cells) if (!first.has(c.i)) first.set(c.i, c.before);
    const out: number[] = [];
    for (const [i, s] of first) if (!sameState(s, cellState(this.w, i))) out.push(i);
    return out.sort((a, b) => a - b);
  }
}

// ---- saving ----

export interface SaveFile {
  name: string;
  data: Uint8Array | string;
  type: string;
}

export function f32Bytes(a: Float32Array): Uint8Array {
  const out = new Uint8Array(a.length * 4);
  const v = new DataView(out.buffer);
  for (let i = 0; i < a.length; i++) v.setFloat32(i * 4, a[i], true);
  return out;
}

export const changesJson = (b: Bundle, n: number, ops: Op[]): string =>
  `${JSON.stringify({ bundle: b.json.name, save: n, ops }, null, 1)}\n`;

/**
 * The seven files a world bundle directory holds, under their own names: the four the editor never touches,
 * verbatim so a no-edit save is byte-identical, and the three grids little-endian as the bundle stores them.
 * This is what `ecosim run --world` reads, so it is also what the sim helper is posted (shot E4).
 */
export function bundleFiles(b: Bundle): SaveFile[] {
  const bin = 'application/octet-stream';
  return [
    ...BUNDLE_VERBATIM.map((f) => ({ name: f, data: b.raw[f], type: 'application/json' })),
    { name: 'ground_h.f32', data: f32Bytes(b.world.ground_h), type: bin },
    { name: 'medium.u8', data: new Uint8Array(b.world.medium), type: bin },
    { name: 'building_h.f32', data: f32Bytes(b.world.building_h), type: bin },
  ];
}

/**
 * A save's files: the bundle's seven, plus the operation list. The seven carry a `<name>-edit-N-` prefix,
 * because a browser download cannot make a directory and must not overwrite the bundle it loaded. Drop the
 * prefix to get a directory `ecosim run --world` reads (DECISIONS.md, shot E1).
 */
export function saveFiles(b: Bundle, n: number, ops: Op[]): SaveFile[] {
  const p = `${b.json.name}-edit-${n}-`;
  return [
    ...bundleFiles(b).map((f) => ({ ...f, name: p + f.name })),
    { name: `changes-${n}.json`, data: changesJson(b, n, ops), type: 'application/json' },
  ];
}

function download(f: SaveFile): void {
  const url = URL.createObjectURL(new Blob([f.data as BlobPart], { type: f.type }));
  const a = document.createElement('a');
  a.href = url;
  a.download = f.name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 30_000); // revoking at once can cancel the download
}

// ---- the first-person camera ----

/** Movement keys, as a direction in (right, up, forward). */
const FLY_KEYS: Record<string, [number, number, number]> = {
  KeyW: [0, 0, 1],
  KeyS: [0, 0, -1],
  KeyA: [-1, 0, 0],
  KeyD: [1, 0, 0],
  Space: [0, 1, 0],
  ShiftLeft: [0, -1, 0],
  ShiftRight: [0, -1, 0],
};
export const FLY_SPEED = 12;
export const FLY_SPEED_MIN = 1;
export const FLY_SPEED_MAX = 120;

/** One wheel notch's factor on the fly speed. */
export const FLY_SPEED_STEP = 1.25;

export const flySpeed = (speed: number, wheelDeltaY: number): number =>
  Math.min(FLY_SPEED_MAX, Math.max(FLY_SPEED_MIN, wheelDeltaY > 0 ? speed / FLY_SPEED_STEP : speed * FLY_SPEED_STEP));

/**
 * The first-person camera: pointer-lock mouse look, WASD along the ground, Space and Shift vertical, and the
 * wheel on fly speed. There is no render loop in this viewer, so a held key runs a short rAF loop of its own
 * and stops as soon as the last key comes up.
 */
export class Fly {
  readonly camera: THREE.PerspectiveCamera;
  readonly controls: PointerLockControls;
  speed = FLY_SPEED;
  private readonly held = new Set<string>();
  private raf = 0;
  private last = 0;

  constructor(from: THREE.Camera, dom: HTMLElement, aspect: number, private readonly onMove: () => void) {
    this.camera = new THREE.PerspectiveCamera(60, aspect, 0.1, 4000);
    this.camera.position.copy(from.position);
    this.camera.quaternion.copy(from.quaternion);
    this.controls = new PointerLockControls(this.camera, dom);
    this.controls.addEventListener('change', onMove);
  }

  get locked(): boolean {
    return this.controls.isLocked;
  }

  lock(): void {
    this.controls.lock();
  }

  /** True when the key was a movement key, so the caller knows not to treat it as a hotkey. */
  key(code: string, down: boolean): boolean {
    if (!(code in FLY_KEYS)) return false;
    if (down) this.held.add(code);
    else this.held.delete(code);
    if (this.held.size > 0 && this.raf === 0) {
      this.last = performance.now();
      this.raf = requestAnimationFrame(this.step);
    }
    return true;
  }

  private step = (): void => {
    const now = performance.now();
    const dt = Math.min(0.1, (now - this.last) / 1000);
    this.last = now;
    let [x, y, z] = [0, 0, 0];
    for (const code of this.held) {
      const [dx, dy, dz] = FLY_KEYS[code];
      x += dx;
      y += dy;
      z += dz;
    }
    if (x !== 0 || y !== 0 || z !== 0) {
      const d = this.speed * dt;
      this.controls.moveRight(x * d);
      this.camera.position.y += y * d;
      this.controls.moveForward(z * d);
      this.onMove();
    }
    this.raf = this.held.size > 0 ? requestAnimationFrame(this.step) : 0;
  };

  dispose(): void {
    if (this.raf !== 0) cancelAnimationFrame(this.raf);
    this.raf = 0;
    this.held.clear();
    if (this.locked) this.controls.unlock();
    this.controls.dispose();
  }
}

// ---- the editor ----

/** What the sidebar shows; `ui.ts` draws it and `edit.ts` never touches the DOM outside the canvas. */
export interface PanelView {
  editing: boolean;
  fly: boolean;
  slot: Slot;
  brush: number;
  dirty: boolean;
  undo: number;
  redo: number;
  saves: number;
  target: Target | null;
  /** The targeted cell's current state, so the panel reads it without touching the grids. */
  state: CellState | null;
  /** Where to put the crosshair inside the canvas, in CSS pixels. */
  crosshair: [number, number];
  message: string;
}

export interface EditorHost {
  scene: THREE.Scene;
  canvas: HTMLCanvasElement;
  render(): void;
  camera(): THREE.Camera;
  /** Swaps the fly camera in, or null to go back to the orbit camera. */
  useCamera(c: THREE.PerspectiveCamera | null): void;
  /** Orbit controls are off in edit mode, so a click can never also be a drag. */
  setOrbit(on: boolean): void;
  panel(v: PanelView): void;
  /** Mirrors edit state into the URL. */
  pushState(s: { edit: boolean; slot: Slot; brush: number; aim: [number, number] | null }): void;
}

/** Test and screenshot hook. `aim` pins the crosshair to a cell so a shot needs no mouse. */
export interface EditApi {
  world: WorldData;
  target(): Target | null;
  aim(gx: number | null, gy?: number): void;
  act(action: Action): boolean;
  slot(s: Slot): void;
  brush(r: number): void;
  undo(): boolean;
  redo(): boolean;
  save(): void;
  ops(): Op[];
  changed(): number[];
  cell(i: number): CellState;
  /** The middle-click pick: takes the hotbar to the slot of the cube under the crosshair. */
  pickSlot(): Slot | null;
  /** Applies `n` one-cell raise ops down a diagonal and returns the milliseconds they took. */
  stroke(n: number): number;
}

declare global {
  interface Window {
    /** The bundle's grids, live: the e2e tests read cells straight out of them. */
    __ecoviewWorld?: WorldData;
    __ecoviewEdit?: EditApi;
  }
}

export class Editor {
  readonly chunks: GroundChunks;
  readonly plants: BundlePlants;
  readonly outline: Outline;
  readonly history: History;
  private editing = false;
  private slotKey: Slot = 'lawn';
  private brushR = 1;
  private aimed: [number, number] | null = null;
  private target: Target | null = null;
  private fly: Fly | null = null;
  private saves = 0;
  private message = '';
  /** While a run is on screen the editor is asleep: its keys and its clicks do nothing (shot E4). */
  private asleep = false;
  private readonly raycaster = new THREE.Raycaster();

  constructor(readonly bundle: Bundle, private readonly host: EditorHost) {
    const w = bundle.world;
    const depth = bundle.grid.y;
    this.chunks = new GroundChunks(w, depth);
    this.plants = new BundlePlants(w, depth, bundle.trees, bundle.shrubs);
    this.outline = new Outline(w, depth, disc(w, w.gw >> 1, w.gd >> 1, MAX_BRUSH).length);
    this.history = new History(w);
    host.scene.add(this.chunks.group, this.plants.group, this.outline.lines);
    host.canvas.addEventListener('pointerdown', this.onPointerDown);
    host.canvas.addEventListener('contextmenu', this.onContextMenu);
    host.canvas.addEventListener('wheel', this.onWheel, { passive: true });
    window.addEventListener('keydown', this.onKey);
    window.addEventListener('keyup', this.onKey);
    window.__ecoviewWorld = w;
    window.__ecoviewEdit = {
      world: w,
      target: () => this.target,
      aim: (gx, gy) => this.setAim(gx === null ? null : [gx, gy ?? 0]),
      act: (a) => this.act(a),
      slot: (s) => this.setSlot(s),
      brush: (r) => this.setBrush(r),
      undo: () => this.step('undo'),
      redo: () => this.step('redo'),
      save: () => this.save(),
      ops: () => this.history.applied,
      changed: () => this.history.changed,
      cell: (i) => cellState(w, i),
      pickSlot: () => this.pickSlot(),
      stroke: (n) => this.stroke(n),
    };
  }

  dispose(): void {
    this.fly?.dispose();
    this.host.canvas.removeEventListener('pointerdown', this.onPointerDown);
    this.host.canvas.removeEventListener('contextmenu', this.onContextMenu);
    this.host.canvas.removeEventListener('wheel', this.onWheel);
    window.removeEventListener('keydown', this.onKey);
    window.removeEventListener('keyup', this.onKey);
    delete window.__ecoviewWorld;
    delete window.__ecoviewEdit;
  }

  setAsleep(on: boolean): void {
    this.asleep = on;
    if (on) this.fly?.dispose();
  }

  setLit(lit: boolean): void {
    this.chunks.setLit(lit);
    this.plants.setLit(lit);
  }

  /** Applies the URL's edit state without pushing it back. */
  setState(s: { edit: boolean; slot: Slot; brush: number; aim: [number, number] | null }): void {
    this.editing = s.edit;
    this.slotKey = s.slot;
    this.brushR = s.brush;
    this.aimed = s.aim;
    this.host.setOrbit(!this.editing);
  }

  /** Re-picks the target, redraws the outline and the panel, and renders. */
  refresh(): void {
    this.target = this.editing ? this.pick() : null;
    this.outline.set(this.target ? disc(this.bundle.world, this.target.gx, this.target.gy, this.brushR) : []);
    this.host.render();
    this.host.panel({
      editing: this.editing,
      fly: this.fly !== null,
      slot: this.slotKey,
      brush: this.brushR,
      dirty: this.history.dirty,
      undo: this.history.counts[0],
      redo: this.history.counts[1],
      saves: this.saves,
      target: this.target,
      state: this.target ? cellState(this.bundle.world, this.target.i) : null,
      crosshair: this.crosshair(),
      message: this.message,
    });
  }

  private pick(): Target | null {
    const w = this.bundle.world;
    if (this.aimed) {
      const [gx, gy] = this.aimed;
      if (gx < 0 || gy < 0 || gx >= w.gw || gy >= w.gd) return null;
      return { i: gx + w.gw * gy, gx, gy, top: true, adj: gx + w.gw * gy };
    }
    const cam = this.host.camera();
    cam.updateMatrixWorld(); // the pick runs before the frame, so the camera's matrix may be a frame behind
    this.raycaster.setFromCamera(new THREE.Vector2(0, 0), cam);
    return pickCell(w, this.bundle.grid.y, this.raycaster.ray, this.bundle.grid.z);
  }

  /** The crosshair sits at the view centre, or over the aimed cell when one is pinned. */
  private crosshair(): [number, number] {
    const canvas = this.host.canvas;
    const mid: [number, number] = [canvas.clientWidth / 2, canvas.clientHeight / 2];
    const t = this.target;
    if (!this.aimed || !t) return mid;
    const w = this.bundle.world;
    this.host.camera().updateMatrixWorld();
    const p = new THREE.Vector3(
      (t.gx + 0.5) * w.cell,
      topLevelAt(w, t.i) * w.cell,
      this.bundle.grid.y - (t.gy + 0.5) * w.cell,
    ).project(this.host.camera());
    return [((p.x + 1) / 2) * canvas.clientWidth, ((1 - p.y) / 2) * canvas.clientHeight];
  }

  setEdit(on: boolean): void {
    this.editing = on;
    this.message = '';
    if (!on && this.fly) this.setFly(false);
    this.host.setOrbit(!on);
    this.push();
  }

  setSlot(s: Slot): void {
    this.slotKey = s;
    this.push();
  }

  setBrush(r: number): void {
    this.brushR = Math.min(MAX_BRUSH, Math.max(1, Math.round(r)));
    this.push();
  }

  private setAim(a: [number, number] | null): void {
    this.aimed = a;
    this.push();
  }

  private push(): void {
    this.host.pushState({ edit: this.editing, slot: this.slotKey, brush: this.brushR, aim: this.aimed });
    this.refresh();
  }

  /**
   * Stands the fly camera at a viewpoint given in metres east, north and up, with yaw and pitch in degrees
   * (yaw 0 looks north, positive turns west). `?eye=` uses it, so a close-up shot needs no pointer lock.
   */
  setEye(x: number, y: number, z: number, yaw: number, pitch: number): void {
    if (!this.fly) this.takeFly();
    const cam = this.fly!.camera;
    cam.position.set(x, z, this.bundle.grid.y - y);
    cam.rotation.set(THREE.MathUtils.degToRad(pitch), THREE.MathUtils.degToRad(yaw), 0, 'YXZ');
    cam.updateMatrixWorld();
  }

  /** Builds the fly camera from wherever the current one stands, without drawing: `setEye` draws once. */
  private takeFly(): void {
    const canvas = this.host.canvas;
    this.fly = new Fly(this.host.camera(), canvas, canvas.clientWidth / canvas.clientHeight, () => this.refresh());
    this.host.useCamera(this.fly.camera);
  }

  private setFly(on: boolean): void {
    const was = this.fly;
    if (on === (was !== null)) return;
    if (!was) {
      this.takeFly();
    } else {
      was.dispose();
      this.fly = null;
      this.host.useCamera(null);
    }
    this.refresh();
  }

  /** Runs one action at the current target. Returns whether anything changed. */
  act(action: Action): boolean {
    if (!this.editing) return false;
    const t = this.target ?? this.pick();
    if (!t) {
      this.message = 'nothing under the crosshair';
      this.refresh();
      return false;
    }
    const op = opFor(this.bundle.world, action, t, this.slotKey, this.brushR);
    if (!op) {
      this.message = `${action} changed nothing`;
      this.refresh();
      return false;
    }
    this.history.push(op);
    this.message = `${op.kind} ${op.cells.length} cell${op.cells.length === 1 ? '' : 's'}`;
    this.rebuild(op);
    this.refresh();
    return true;
  }

  /** Middle-click: takes the hotbar to the slot the cube under the crosshair is made of (shot E3). */
  pickSlot(): Slot | null {
    if (!this.editing) return null;
    const t = this.target ?? this.pick();
    if (!t) {
      this.message = 'nothing under the crosshair';
      this.refresh();
      return null;
    }
    const slot = slotAt(this.bundle.world, t);
    this.message = `picked ${slot}`;
    this.setSlot(slot);
    return slot;
  }

  private step(which: 'undo' | 'redo'): boolean {
    const op = which === 'undo' ? this.history.undo() : this.history.redo();
    if (!op) {
      this.message = `nothing to ${which}`;
      this.refresh();
      return false;
    }
    this.message = `${which} ${op.kind}`;
    this.rebuild(op);
    this.refresh();
    return true;
  }

  /** Only the chunks the op touched are written again. */
  private rebuild(op: Op): void {
    for (const c of this.chunks.chunksOf(op.cells.map((e) => e.i))) this.chunks.rebuild(c);
  }

  save(): void {
    this.saves++;
    for (const f of saveFiles(this.bundle, this.saves, this.history.applied)) download(f);
    this.history.markSaved();
    this.message = `saved ${this.bundle.json.name}-edit-${this.saves}`;
    this.refresh();
  }

  private stroke(n: number): number {
    const w = this.bundle.world;
    const t0 = performance.now();
    for (let k = 0; k < n; k++) {
      const gx = (k * 3) % w.gw;
      const gy = (k * 5) % w.gd;
      const op = opFor(w, 'place', { i: gx + w.gw * gy, gx, gy, top: true, adj: gx + w.gw * gy }, 'ground', 1);
      if (op) {
        this.history.push(op);
        this.rebuild(op);
      }
    }
    const ms = performance.now() - t0;
    this.refresh();
    return ms;
  }

  private readonly onPointerDown = (e: PointerEvent): void => {
    if (!this.editing || this.asleep) return;
    if (this.fly && !this.fly.locked) {
      this.fly.lock(); // the first click in fly mode takes the pointer, later ones edit
      return;
    }
    e.preventDefault();
    if (e.button === 1) this.pickSlot();
    else this.act(e.button === 2 ? 'place' : e.ctrlKey ? 'flatten' : 'remove');
  };

  private readonly onContextMenu = (e: MouseEvent): void => {
    if (this.editing) e.preventDefault();
  };

  private readonly onWheel = (e: WheelEvent): void => {
    if (!this.fly || this.asleep) return;
    this.fly.speed = flySpeed(this.fly.speed, e.deltaY);
    this.message = `fly ${this.fly.speed.toFixed(1)} m/s`;
    this.refresh();
  };

  private readonly onKey = (e: KeyboardEvent): void => {
    if (this.asleep) return;
    const down = e.type === 'keydown';
    if (this.editing && this.fly?.key(e.code, down)) {
      e.preventDefault();
      return;
    }
    if (!down) return;
    if (e.ctrlKey || e.metaKey) {
      const hit = { KeyZ: 'undo', KeyY: 'redo', KeyS: 'save' }[e.code];
      if (!hit || !this.editing) return;
      e.preventDefault();
      if (hit === 'save') this.save();
      else this.step(hit as 'undo' | 'redo');
      return;
    }
    if (e.code === 'KeyE') {
      this.setEdit(!this.editing);
      return;
    }
    if (!this.editing) return;
    if (e.code === 'KeyF') this.setFly(this.fly === null);
    else if (e.code === 'BracketLeft') this.setBrush(this.brushR - 1);
    else if (e.code === 'BracketRight') this.setBrush(this.brushR + 1);
    else if (/^Digit[1-8]$/.test(e.code)) this.setSlot(SLOT_KEYS[Number(e.code.slice(5)) - 1]);
  };
}
