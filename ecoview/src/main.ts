// Entry point: wires loader, world, entities and ui together. Renders on demand only.
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { loadRun, loadSnapshot, pickSnapshot, speciesColor, type Grid, type Run, type Snapshot } from './loader';
import { World } from './world';
import { Entities } from './entities';
import {
  drawChart, getControls, initControls, parseParams, syncControls, toSearch,
  type Cam, type ViewState,
} from './ui';

declare global {
  interface Window {
    __ecoviewReady: boolean;
    __ecoviewError?: string;
    /** Moves to the snapshot nearest `tick` without reloading the page (used by scripts/film.mjs). */
    __ecoviewGoto: (tick: number) => void;
    /** Renders the current scene `n` times back to back and returns ms per frame (tests/e2e/perf.spec.ts). */
    __ecoviewBench: (n: number) => number[];
  }
}

const VIEW_W = 960;
const VIEW_H = 800;
const BG = 0xe8ecf0;
const PLAY_MS = 150;
const CACHE_SIZE = 8;

window.__ecoviewReady = false;

const canvas = document.getElementById('view') as HTMLCanvasElement;
const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, powerPreference: 'low-power' });
renderer.setPixelRatio(window.devicePixelRatio); // 1 everywhere except `npm run film -- --scale N`
renderer.setSize(VIEW_W, VIEW_H, false);
renderer.outputColorSpace = THREE.SRGBColorSpace;
renderer.setClearColor(new THREE.Color().setHex(BG, THREE.SRGBColorSpace));

const scene = new THREE.Scene();
// The addendum's ambient 0.6 / directional 1.0 are legacy-lighting values; three r155+ divides
// Lambert diffuse by pi, so scale by pi to get the intended 0.6 + 1.0*cos(theta) of albedo.
const ambient = new THREE.AmbientLight(0xffffff, 0.6 * Math.PI);
const sun = new THREE.DirectionalLight(0xffffff, 1.0 * Math.PI);
scene.add(ambient, sun, sun.target);

let world: World | null = null;
let entities: Entities | null = null;

/** Where the perspective cameras look: the world's centre at 3/8 of its height (12 of 32). */
const target = (g: Grid) => new THREE.Vector3(g.x / 2, (g.z * 3) / 8, g.y / 2);

/**
 * Cameras frame the world's bounding box (DECISIONS.md, shot 16). Perspective offsets are the 64×64 world's
 * scaled by longest side / 64, so the square world is framed exactly as before.
 */
function makeCamera(cam: Cam, g: Grid): THREE.Camera {
  const cx = g.x / 2;
  const cz = g.y / 2;
  if (cam === 'top') {
    // Fit width × depth into the view, letterboxed: the 64×64 world fills the height at 12.5 px per column.
    const aspect = VIEW_W / VIEW_H;
    const halfH = g.x / g.y > aspect ? g.x / 2 / aspect : g.y / 2;
    const halfW = halfH * aspect;
    const c = new THREE.OrthographicCamera(-halfW, halfW, halfH, -halfH, 1, g.z + 168);
    c.position.set(cx, g.z + 68, cz);
    c.up.set(0, 0, -1); // sim +y points up the screen
    c.lookAt(cx, 0, cz);
    return c;
  }
  const k = g.longest / 64;
  const c = new THREE.PerspectiveCamera(45, VIEW_W / VIEW_H, 0.5 * k, 500 * k);
  const t = target(g);
  if (cam === 'iso') {
    c.position.set(cx + 70 * k, g.z / 2 + 75 * k, cz + 70 * k); // down the diagonal from the south-east
  } else {
    // From beyond the south edge looking north, so sim +x (the long axis on the strip) runs left to right as in
    // top. The distance fits the x extent into 90% of the view width, 25 degrees above the horizon.
    const tanHalfW = c.getFilmWidth() / (2 * c.getFocalLength());
    const d = g.x / 2 / (0.9 * tanHalfW);
    const tilt = (25 * Math.PI) / 180;
    c.position.set(cx, t.y + d * Math.sin(tilt), cz + g.y / 2 + d * Math.cos(tilt));
  }
  c.lookAt(t);
  return c;
}

let camera: THREE.Camera = new THREE.PerspectiveCamera();
let orbit: OrbitControls | null = null;

function setCamera(cam: Cam, g: Grid): void {
  orbit?.dispose();
  camera = makeCamera(cam, g);
  orbit = new OrbitControls(camera, canvas);
  if (cam === 'top') {
    orbit.target.set(g.x / 2, 0, g.y / 2);
    orbit.enableRotate = false;
  } else {
    orbit.target.copy(target(g));
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
let loading = false;
/** The state behind the frame on screen; an error falls back to it. */
let shown: ViewState | null = null;
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

/** Shows the error and leaves the last good frame on screen, with the controls and URL put back to match it. */
function fail(err: unknown): void {
  const msg = err instanceof Error ? err.message : String(err);
  window.__ecoviewError = msg;
  if (playing) togglePlay();
  if (shown && run) {
    state = shown;
    history.replaceState(null, '', toSearch(state));
    syncControls(ui, state, run.meta.snapshots, snapTick, playing);
  }
  ui.status.textContent = `Error: ${msg}`;
  ui.status.classList.add('error');
  console.error(err);
}

async function apply(next: ViewState): Promise<void> {
  const token = ++seq;
  window.__ecoviewReady = false;
  loading = true;
  state = next;
  history.replaceState(null, '', toSearch(state));
  try {
    if (!run || run.base !== `/${state.run}`) {
      ui.status.textContent = `Loading ${state.run}…`;
      const r = await loadRun(`/${state.run}`);
      if (token !== seq) return;
      run = r;
      cache.clear();
      world?.mesh.removeFromParent();
      world = new World(r.grid);
      scene.add(world.mesh);
      entities?.group.removeFromParent();
      entities = new Entities(r.meta, r.grid);
      scene.add(entities.group);
      const t = target(r.grid);
      sun.target.position.copy(t);
      sun.position.set(t.x + 40, t.y + 88, t.z + 60);
      cam = null; // the cameras frame this run's world
    }
    const r = run;
    const tick = pickSnapshot(r.meta.snapshots, state.tick);
    const snap = await getSnapshot(r, tick);
    if (token !== seq) return;
    snapTick = tick;
    if (cam !== state.cam) {
      setCamera(state.cam, r.grid);
      cam = state.cam;
    }
    const top = state.cam === 'top';
    world!.setLit(!top);
    world!.build(snap, state.overlay);
    entities!.build(snap, { fieldOverlay: state.overlay !== 'material', top, traits: state.overlay === 'traits' });
    const m = r.meta;
    drawChart(ui.chart, r.series, {
      grazer: speciesColor(m, 'grazer'),
      hunter: speciesColor(m, 'hunter'),
      tree: speciesColor(m, 'tree'),
    }, snapTick);
    syncControls(ui, state, m.snapshots, snapTick, playing);
    const fork = m.forked_from ? ` · forked from ${m.forked_from.run} @ tick ${m.forked_from.tick}` : '';
    ui.status.textContent = `${state.run} · seed ${m.seed}${fork} · ${snap.entities.length} entities`;
    ui.status.classList.remove('error');
    delete window.__ecoviewError;
    render();
    shown = state;
    requestAnimationFrame(() => {
      if (token === seq) window.__ecoviewReady = true;
    });
  } catch (err) {
    if (token === seq) fail(err);
  } finally {
    if (token === seq) loading = false;
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
    if (loading) return; // a slow load skips beats rather than being cancelled by the next step
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

window.__ecoviewGoto = (tick) => void apply({ ...state, tick });

// Runs only when called. It redraws the same scene, so the canvas and __ecoviewReady are unchanged.
window.__ecoviewBench = (n) => {
  const gl = renderer.getContext();
  const px = new Uint8Array(4);
  const ms: number[] = [];
  for (let i = 0; i < n; i++) {
    const t0 = performance.now();
    render();
    gl.finish();
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px); // finish() alone may return before the GPU process is done
    ms.push(performance.now() - t0);
  }
  return ms;
};

void apply(state);
