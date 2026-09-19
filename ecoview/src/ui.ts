// Controls, URL-parameter state, and the Canvas 2D population chart.
import { OVERLAYS, type Overlay } from './world';
import { DEATH_CAUSES, type DeathCause, type Series } from './loader';

export const CAMS = ['iso', 'top', 'side'] as const;
export type Cam = (typeof CAMS)[number];

export const DEFAULT_RUN = 'fixtures/s42-mini';

export interface ViewState {
  run: string;
  tick: number;
  overlay: Overlay;
  cam: Cam;
}

export function parseParams(search: string): ViewState {
  const p = new URLSearchParams(search);
  const run = (p.get('run') ?? DEFAULT_RUN).replace(/^\/+|\/+$/g, '') || DEFAULT_RUN;
  const tickRaw = Number(p.get('tick') ?? 0);
  const tick = Number.isFinite(tickRaw) && tickRaw > 0 ? Math.floor(tickRaw) : 0;
  const o = p.get('overlay');
  const overlay = (OVERLAYS as readonly string[]).includes(o ?? '') ? (o as Overlay) : 'material';
  const c = p.get('cam');
  const cam = (CAMS as readonly string[]).includes(c ?? '') ? (c as Cam) : 'iso';
  return { run, tick, overlay, cam };
}

export function toSearch(s: ViewState): string {
  return `?run=${s.run}&tick=${s.tick}&overlay=${s.overlay}&cam=${s.cam}`;
}

// ---- chart ----

interface ChartLine {
  label: string;
  values: Float64Array;
  color: string;
}

export const CHART_BG = '#ffffff';
/** One colour per death cause; the sim doesn't own these, since causes aren't species. */
export const CAUSE_COLORS: Record<DeathCause, string> = {
  starved: '#e08a1e',
  eaten: '#8e1f4f',
  old_age: '#7d8b99',
  crowded: '#7b52ab',
  burnt: '#222222',
};
export const DEATH_BIN = 100;

const PAD = { l: 8, r: 8, t: 4, b: 16 };
const LEGEND_H = 15;
const PANEL_GAP = 6;

/** A panel's plot rectangle in canvas pixels. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Stacks `n` panels, each a legend row above a plot, leaving the bottom strip for the tick axis. */
export function chartLayout(w: number, h: number, n: number): Rect[] {
  const each = (h - PAD.t - PAD.b - PANEL_GAP * (n - 1)) / n;
  return Array.from({ length: n }, (_, i) => {
    const top = Math.round(PAD.t + i * (each + PANEL_GAP));
    return { x: PAD.l, y: top + LEGEND_H, w: w - PAD.l - PAD.r, h: Math.round(each) - LEGEND_H };
  });
}

interface Axis {
  t0: number;
  t1: number;
  xOf(t: number): number;
}

function legend(ctx: CanvasRenderingContext2D, r: Rect, items: [string, string][]): void {
  const step = Math.floor(r.w / items.length);
  items.forEach(([label, color], i) => {
    const lx = r.x + i * step;
    const ly = r.y - LEGEND_H + 2;
    ctx.fillStyle = color;
    ctx.fillRect(lx, ly + 1, 9, 9);
    ctx.fillStyle = '#222';
    ctx.font = '10px sans-serif';
    ctx.textBaseline = 'top';
    ctx.fillText(label, lx + 12, ly);
  });
}

/** Plots each line normalized to its own max, one averaged point per horizontal pixel. */
function linePanel(ctx: CanvasRenderingContext2D, r: Rect, ax: Axis, ticks: Float64Array, lines: ChartLine[]): void {
  const n = ticks.length;
  const labels: [string, string][] = [];
  for (const line of lines) {
    let max = 0;
    for (const v of line.values) if (v > max) max = v;
    const scale = max > 0 ? 1 / max : 0;
    ctx.strokeStyle = line.color;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    // Bucketing keeps a 20k-row series cheap to draw and damps per-tick noise.
    const buckets = Math.min(n, Math.max(1, Math.floor(r.w)));
    for (let b = 0; b < buckets; b++) {
      const i0 = Math.floor((b * n) / buckets);
      const i1 = Math.max(i0 + 1, Math.floor(((b + 1) * n) / buckets));
      let sum = 0;
      for (let i = i0; i < i1; i++) sum += line.values[i];
      const v = (sum / (i1 - i0)) * scale;
      const x = ax.xOf(ticks[Math.floor((i0 + i1 - 1) / 2)]);
      const y = r.y + r.h - v * (r.h - 2) - 1;
      if (b === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    labels.push([`${line.label} ≤${Math.round(max)}`, line.color]);
  }
  legend(ctx, r, labels);
}

/** Sums per-tick deaths into DEATH_BIN-tick bins, one array per cause present. */
export function binDeaths(ticks: Float64Array, deaths: Series['deaths']): { start: number[]; by: [DeathCause, number[]][] } {
  const t0 = ticks.length ? ticks[0] : 0;
  const bins = ticks.length ? Math.floor((ticks[ticks.length - 1] - t0) / DEATH_BIN) + 1 : 0;
  const start = Array.from({ length: bins }, (_, b) => t0 + b * DEATH_BIN);
  const by: [DeathCause, number[]][] = [];
  for (const cause of DEATH_CAUSES) {
    const col = deaths[cause];
    if (!col) continue;
    const sums = new Array<number>(bins).fill(0);
    for (let i = 0; i < ticks.length; i++) sums[Math.floor((ticks[i] - t0) / DEATH_BIN)] += col[i];
    by.push([cause, sums]);
  }
  return { start, by };
}

/** Deaths per DEATH_BIN ticks as stacked columns, causes bottom to top in the sim's order. */
function deathPanel(ctx: CanvasRenderingContext2D, r: Rect, ax: Axis, ticks: Float64Array, deaths: Series['deaths']): void {
  const { start, by } = binDeaths(ticks, deaths);
  let max = 0;
  for (let b = 0; b < start.length; b++) {
    let sum = 0;
    for (const [, sums] of by) sum += sums[b];
    if (sum > max) max = sum;
  }
  const scale = max > 0 ? (r.h - 2) / max : 0;
  for (let b = 0; b < start.length; b++) {
    const x0 = ax.xOf(start[b]);
    const x1 = ax.xOf(Math.min(start[b] + DEATH_BIN, ax.t1));
    let y = r.y + r.h - 1;
    for (const [cause, sums] of by) {
      const hgt = sums[b] * scale;
      if (hgt <= 0) continue;
      ctx.fillStyle = CAUSE_COLORS[cause];
      ctx.fillRect(x0, y - hgt, Math.max(1, x1 - x0), hgt);
      y -= hgt;
    }
  }
  legend(ctx, r, by.map(([cause]) => [cause.replace('_', ' '), CAUSE_COLORS[cause]]));
  ctx.fillStyle = '#555';
  ctx.font = '10px sans-serif';
  ctx.textAlign = 'right';
  ctx.textBaseline = 'top';
  ctx.fillText(`deaths ≤${max} / ${DEATH_BIN} ticks`, r.x + r.w - 3, r.y + 2);
  ctx.textAlign = 'left';
}

export interface ChartColors {
  grazer: string;
  hunter: string;
  tree: string;
}

/**
 * Three stacked panels over one tick axis: grazers and hunters, trees, and deaths by cause.
 * A dashed marker at `tick` crosses all three. The marker tick and x and the plot rectangles are
 * mirrored into data attributes so tests can find them.
 */
export function drawChart(canvas: HTMLCanvasElement, series: Series, colors: ChartColors, tick: number): void {
  const ctx = canvas.getContext('2d')!;
  const { width: w, height: h } = canvas;
  ctx.fillStyle = CHART_BG;
  ctx.fillRect(0, 0, w, h);
  const ticks = series.tick;
  const n = ticks.length;
  if (n === 0) return;
  const panels = chartLayout(w, h, 3);
  const t0 = ticks[0];
  const t1 = Math.max(ticks[n - 1], t0 + 1);
  const ax: Axis = { t0, t1, xOf: (t) => panels[0].x + ((t - t0) / (t1 - t0)) * panels[0].w };

  ctx.strokeStyle = '#d0d4d8';
  ctx.lineWidth = 1;
  for (const r of panels) ctx.strokeRect(r.x + 0.5, r.y + 0.5, r.w - 1, r.h - 1);

  linePanel(ctx, panels[0], ax, ticks, [
    { label: 'grazers', values: series.grazers, color: colors.grazer },
    { label: 'hunters', values: series.hunters, color: colors.hunter },
  ]);
  linePanel(ctx, panels[1], ax, ticks, [{ label: 'trees', values: series.trees, color: colors.tree }]);
  deathPanel(ctx, panels[2], ax, ticks, series.deaths);

  const mt = Math.min(Math.max(tick, t0), t1);
  const mx = Math.round(ax.xOf(mt)) + 0.5;
  ctx.strokeStyle = '#333';
  ctx.lineWidth = 1;
  ctx.setLineDash([3, 2]);
  ctx.beginPath();
  ctx.moveTo(mx, panels[0].y);
  ctx.lineTo(mx, panels[2].y + panels[2].h);
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.fillStyle = '#555';
  ctx.font = '10px sans-serif';
  ctx.textBaseline = 'top';
  ctx.fillText(`${t0}`, PAD.l, h - PAD.b + 3);
  ctx.textAlign = 'right';
  ctx.fillText(`${t1} ticks`, w - PAD.r, h - PAD.b + 3);
  ctx.textAlign = 'left';
  canvas.dataset.markerTick = String(mt);
  canvas.dataset.markerX = String(mx - 0.5);
  canvas.dataset.panels = JSON.stringify(panels);
}

// ---- controls ----

export interface Controls {
  overlay: HTMLSelectElement;
  cam: HTMLSelectElement;
  slider: HTMLInputElement;
  play: HTMLButtonElement;
  readout: HTMLElement;
  chart: HTMLCanvasElement;
  status: HTMLElement;
}

export function getControls(): Controls {
  const q = <T extends HTMLElement>(id: string) => {
    const el = document.getElementById(id);
    if (!el) throw new Error(`missing #${id}`);
    return el as T;
  };
  return {
    overlay: q('overlay'),
    cam: q('cam'),
    slider: q('slider'),
    play: q('play'),
    readout: q('readout'),
    chart: q('chart'),
    status: q('status'),
  };
}

export interface ControlHandlers {
  onOverlay(o: Overlay): void;
  onCam(c: Cam): void;
  onSnapshotIndex(i: number): void;
  onPlayToggle(): void;
}

export function initControls(c: Controls, h: ControlHandlers): void {
  c.overlay.replaceChildren(...OVERLAYS.map((o) => new Option(o, o)));
  c.cam.replaceChildren(...CAMS.map((o) => new Option(o, o)));
  c.overlay.addEventListener('change', () => h.onOverlay(c.overlay.value as Overlay));
  c.cam.addEventListener('change', () => h.onCam(c.cam.value as Cam));
  c.slider.addEventListener('input', () => h.onSnapshotIndex(Number(c.slider.value)));
  c.play.addEventListener('click', () => h.onPlayToggle());
}

export function syncControls(c: Controls, s: ViewState, snapshots: number[], snapTick: number, playing: boolean): void {
  c.overlay.value = s.overlay;
  c.cam.value = s.cam;
  c.slider.max = String(snapshots.length - 1);
  c.slider.value = String(Math.max(0, snapshots.indexOf(snapTick)));
  c.readout.textContent = `tick ${snapTick} / ${snapshots[snapshots.length - 1]}`;
  c.play.textContent = playing ? 'Pause' : 'Play';
}
