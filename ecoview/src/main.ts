// Entry point: wires loader, world, entities and ui together. Renders on demand only.
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { loadRun, loadSnapshot, pickSnapshot, speciesColor, type Run, type Snapshot } from './loader';
import { World } from './world';
import { Entities } from './entities';
import {
  drawChart, getControls, initControls, parseParams, seriesLines, syncControls, toSearch,
  type Cam, type ViewState,
} from './ui';

declare global {
  interface Window {
    __ecoviewReady: boolean;
    __ecoviewError?: string;
  }
}

const VIEW_W = 960;
const VIEW_H = 800;
const BG = 0xe8ecf0;
const TARGET = new THREE.Vector3(32, 12, 32);
const PLAY_MS = 250;
const CACHE_SIZE = 8;

window.__ecoviewReady = false;

const canvas = document.getElementById('view') as HTMLCanvasElement;
const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, powerPreference: 'low-power' });
renderer.setPixelRatio(1);
renderer.setSize(VIEW_W, VIEW_H, false);
renderer.outputColorSpace = THREE.SRGBColorSpace;
renderer.setClearColor(new THREE.Color().setHex(BG, THREE.SRGBColorSpace));

const scene = new THREE.Scene();
// The addendum's ambient 0.6 / directional 1.0 are legacy-lighting values; three r155+ divides
// Lambert diffuse by pi, so scale by pi to get the intended 0.6 + 1.0*cos(theta) of albedo.
const ambient = new THREE.AmbientLight(0xffffff, 0.6 * Math.PI);
const sun = new THREE.DirectionalLight(0xffffff, 1.0 * Math.PI);
sun.position.set(32 + 40, 100, 32 + 60);
sun.target.position.copy(TARGET);
scene.add(ambient, sun, sun.target);

const world = new World();
scene.add(world.mesh);
let entities: Entities | null = null;

function makeCamera(cam: Cam): THREE.Camera {
  if (cam === 'top') {
    // Frame exactly the 64 world rows to the view height: 12.5 px per column.
    const halfH = 32;
    const halfW = (halfH * VIEW_W) / VIEW_H;
    const c = new THREE.OrthographicCamera(-halfW, halfW, halfH, -halfH, 1, 200);
    c.position.set(32, 100, 32);
    c.up.set(0, 0, -1); // sim +y points up the screen
    c.lookAt(32, 0, 32);
    return c;
  }
  const c = new THREE.PerspectiveCamera(45, VIEW_W / VIEW_H, 0.5, 500);
  if (cam === 'iso') c.position.set(32 + 70, 16 + 75, 32 + 70);
  else c.position.set(32, 30, -60);
  c.lookAt(TARGET);
  return c;
}

let camera = makeCamera('iso');
let orbit: OrbitControls | null = null;

function setCamera(cam: Cam): void {
  orbit?.dispose();
  camera = makeCamera(cam);
  orbit = new OrbitControls(camera, canvas);
  if (cam === 'top') {
    orbit.target.set(32, 0, 32);
    orbit.enableRotate = false;
  } else {
    orbit.target.copy(TARGET);
  }
  orbit.update();
  orbit.addEventListener('change', render);
}

function render(): void {
  renderer.render(scene, camera);
}

// ---- state ----

const ui = getControls();
let state: ViewState = parseParams(location.search);
let run: Run | null = null;
let snapTick = 0;
let cam: Cam | null = null;
let playing = false;
let playTimer: number | undefined;
let seq = 0;
const cache = new Map<string, Promise<Snapshot>>();

function getSnapshot(r: Run, tick: number): Promise<Snapshot> {
  const key = `${r.base}@${tick}`;
  let p = cache.get(key);
  if (!p) {
    p = loadSnapshot(r, tick);
    p.catch(() => cache.delete(key));
    cache.set(key, p);
    while (cache.size > CACHE_SIZE) cache.delete(cache.keys().next().value!);
  }
  return p;
}

function fail(err: unknown): void {
  const msg = err instanceof Error ? err.message : String(err);
  window.__ecoviewError = msg;
  ui.status.textContent = `Error: ${msg}`;
  ui.status.classList.add('error');
  console.error(err);
}

async function apply(next: ViewState): Promise<void> {
  const token = ++seq;
  window.__ecoviewReady = false;
  state = next;
  history.replaceState(null, '', toSearch(state));
  try {
    if (!run || run.base !== `/${state.run}`) {
      ui.status.textContent = `Loading ${state.run}…`;
      const r = await loadRun(`/${state.run}`);
      if (token !== seq) return;
      run = r;
      cache.clear();
      entities?.group.removeFromParent();
      entities = new Entities(r.meta);
      scene.add(entities.group);
    }
    const r = run;
    const tick = pickSnapshot(r.meta.snapshots, state.tick);
    const snap = await getSnapshot(r, tick);
    if (token !== seq) return;
    snapTick = tick;
    if (cam !== state.cam) {
      setCamera(state.cam);
      cam = state.cam;
    }
    const top = state.cam === 'top';
    world.setLit(!top);
    world.build(snap, state.overlay);
    entities!.build(snap, { fieldOverlay: state.overlay !== 'material', top });
    const m = r.meta;
    drawChart(ui.chart, r.series.tick, seriesLines(r.series, {
      grazer: speciesColor(m, 'grazer'),
      hunter: speciesColor(m, 'hunter'),
      tree: speciesColor(m, 'tree'),
    }), snapTick);
    syncControls(ui, state, m.snapshots, snapTick, playing);
    ui.status.textContent = `${state.run} · seed ${m.seed} · ${snap.entities.length} entities`;
    render();
    requestAnimationFrame(() => {
      if (token === seq) window.__ecoviewReady = true;
    });
  } catch (err) {
    if (token === seq) fail(err);
  }
}

function stepTo(index: number): void {
  if (!run) return;
  const snaps = run.meta.snapshots;
  const i = Math.max(0, Math.min(snaps.length - 1, index));
  void apply({ ...state, tick: snaps[i] });
}

function togglePlay(): void {
  playing = !playing;
  ui.play.textContent = playing ? 'Pause' : 'Play';
  clearInterval(playTimer);
  if (!playing || !run) return;
  playTimer = window.setInterval(() => {
    const snaps = run!.meta.snapshots;
    const i = snaps.indexOf(snapTick);
    if (i >= snaps.length - 1) togglePlay();
    else stepTo(i + 1);
  }, PLAY_MS);
}

initControls(ui, {
  onOverlay: (overlay) => void apply({ ...state, overlay }),
  onCam: (c) => void apply({ ...state, cam: c }),
  onSnapshotIndex: stepTo,
  onPlayToggle: togglePlay,
});

void apply(state);
