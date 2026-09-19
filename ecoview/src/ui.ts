// Controls, URL-parameter state, and the Canvas 2D population chart.
import { OVERLAYS, type Overlay } from './world';
import type { Series } from './loader';

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

export interface ChartLine {
  label: string;
  values: Float64Array;
  color: string;
}

export const CHART_BG = '#ffffff';
const PAD = { l: 8, r: 8, t: 8, b: 44 };

/** Plots each line normalized to its own max against the tick axis, with a marker at `tick`. */
export function drawChart(canvas: HTMLCanvasElement, ticks: Float64Array, lines: ChartLine[], tick: number): void {
  const ctx = canvas.getContext('2d')!;
  const { width: w, height: h } = canvas;
  ctx.fillStyle = CHART_BG;
  ctx.fillRect(0, 0, w, h);
  const pw = w - PAD.l - PAD.r;
  const ph = h - PAD.t - PAD.b;
  const n = ticks.length;
  if (n === 0) return;
  const t0 = ticks[0];
  const t1 = Math.max(ticks[n - 1], t0 + 1);
  const xOf = (t: number) => PAD.l + ((t - t0) / (t1 - t0)) * pw;

  ctx.strokeStyle = '#d0d4d8';
  ctx.lineWidth = 1;
  ctx.strokeRect(PAD.l + 0.5, PAD.t + 0.5, pw - 1, ph - 1);

  lines.forEach((line, li) => {
    let max = 0;
    for (const v of line.values) if (v > max) max = v;
    const scale = max > 0 ? 1 / max : 0;
    ctx.strokeStyle = line.color;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    // One averaged point per horizontal pixel keeps a 20k-row series cheap to draw.
    const buckets = Math.min(n, Math.max(1, Math.floor(pw)));
    for (let b = 0; b < buckets; b++) {
      const i0 = Math.floor((b * n) / buckets);
      const i1 = Math.max(i0 + 1, Math.floor(((b + 1) * n) / buckets));
      let sum = 0;
      for (let i = i0; i < i1; i++) sum += line.values[i];
      const v = (sum / (i1 - i0)) * scale;
      const x = xOf(ticks[Math.floor((i0 + i1 - 1) / 2)]);
      const y = PAD.t + ph - v * (ph - 2) - 1;
      if (b === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    const lx = PAD.l + (li % 3) * Math.floor(pw / 3);
    const ly = h - 26;
    ctx.fillStyle = line.color;
    ctx.fillRect(lx, ly, 10, 10);
    ctx.fillStyle = '#222';
    ctx.font = '11px sans-serif';
    ctx.textBaseline = 'top';
    ctx.fillText(`${line.label} ≤${Math.round(max)}`, lx + 13, ly - 1);
  });

  const mx = Math.round(xOf(Math.min(Math.max(tick, t0), t1))) + 0.5;
  ctx.strokeStyle = '#333';
  ctx.lineWidth = 1;
  ctx.setLineDash([3, 2]);
  ctx.beginPath();
  ctx.moveTo(mx, PAD.t);
  ctx.lineTo(mx, PAD.t + ph);
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.fillStyle = '#555';
  ctx.font = '10px sans-serif';
  ctx.fillText(`0`, PAD.l, h - 12);
  ctx.textAlign = 'right';
  ctx.fillText(`${t1} ticks`, w - PAD.r, h - 12);
  ctx.textAlign = 'left';
}

export function seriesLines(series: Series, colors: { grazer: string; hunter: string; tree: string }): ChartLine[] {
  return [
    { label: 'grazers', values: series.grazers, color: colors.grazer },
    { label: 'hunters', values: series.hunters, color: colors.hunter },
    { label: 'trees', values: series.trees, color: colors.tree },
  ];
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
