// Controls, URL-parameter state, and the Canvas 2D population chart.
import { OVERLAYS, mediumColor, type Overlay } from './world';
import { DEATH_CAUSES, type DeathCause, type Series } from './loader';
import { MAX_BRUSH, SLOTS, isSlot, type PanelView, type Slot } from './edit';

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

/** A first-person viewpoint: metres, then yaw and pitch in degrees (yaw 0 looks north, + turns west). */
export type Eye = [number, number, number, number, number];

/** A `?world=` page: a world bundle and the editor's state (shot E1). A `?run=` page is unchanged. */
export interface WorldState {
  world: string;
  cam: Cam;
  edit: boolean;
  slot: Slot;
  brush: number;
  /** The cell the crosshair is pinned to, so a screenshot needs no mouse; null picks by raycast. */
  aim: [number, number] | null;
  /** Where to stand in the fly camera, so a close-up screenshot needs no pointer lock; null stays in orbit. */
  eye: Eye | null;
}

export function parseWorldParams(search: string): WorldState | null {
  const p = new URLSearchParams(search);
  const world = (p.get('world') ?? '').replace(/^\/+|\/+$/g, '');
  if (!world) return null;
  const c = p.get('cam');
  const slot = p.get('slot') ?? '';
  const brush = Math.round(Number(p.get('brush') ?? 1));
  const a = (p.get('aim') ?? '').split(',').map(Number);
  const aimed = a.length === 2 && a.every((v) => Number.isInteger(v) && v >= 0);
  const e = (p.get('eye') ?? '').split(',').map(Number);
  const eyed = e.length === 5 && e.every((v) => Number.isFinite(v));
  return {
    world,
    cam: (CAMS as readonly string[]).includes(c ?? '') ? (c as Cam) : 'iso',
    edit: p.get('edit') === '1',
    slot: isSlot(slot) ? slot : 'lawn',
    brush: Number.isFinite(brush) ? Math.min(MAX_BRUSH, Math.max(1, brush)) : 1,
    aim: aimed ? [a[0], a[1]] : null,
    eye: eyed ? [e[0], e[1], e[2], e[3], e[4]] : null,
  };
}

export function worldSearch(s: WorldState): string {
  const aim = s.aim ? `&aim=${s.aim[0]},${s.aim[1]}` : '';
  const eye = s.eye ? `&eye=${s.eye.join(',')}` : '';
  return `?world=${s.world}&cam=${s.cam}&edit=${s.edit ? 1 : 0}&slot=${s.slot}&brush=${s.brush}${aim}${eye}`;
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
export const BURNING_COLOR = '#f07818';
export const TRAIT_LINE_COLOR = '#1f5bff';

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

/**
 * Fire and traits: patches burning as bars (the max of each pixel's bucket, since fires last a few ticks and a
 * mean would erase them) on a 0..max axis, and the mean grazer `energy_cost_mult` as a line on its own min..max
 * range, since it moves a few percent around 1. A run without the columns draws an empty panel.
 */
function fireTraitPanel(ctx: CanvasRenderingContext2D, r: Rect, ax: Axis, ticks: Float64Array, extra: Series['extra']): void {
  const n = ticks.length;
  const buckets = Math.min(n, Math.max(1, Math.floor(r.w)));
  const range = (i: number): [number, number] => {
    const i0 = Math.floor((i * n) / buckets);
    return [i0, Math.max(i0 + 1, Math.floor(((i + 1) * n) / buckets))];
  };
  const items: [string, string][] = [];
  const burning = extra.patches_burning;
  if (burning) {
    let max = 0;
    for (const v of burning) if (v > max) max = v;
    ctx.fillStyle = BURNING_COLOR;
    for (let b = 0; b < buckets && max > 0; b++) {
      const [i0, i1] = range(b);
      let m = 0;
      for (let i = i0; i < i1; i++) if (burning[i] > m) m = burning[i];
      const hgt = (m / max) * (r.h - 2);
      if (hgt > 0) ctx.fillRect(Math.floor(ax.xOf(ticks[i0])), r.y + r.h - 1 - hgt, 1, hgt);
    }
    items.push([`burning ≤${max}`, BURNING_COLOR]);
  }
  const mult = extra.grazer_energy_cost_mult_mean;
  if (mult) {
    // A species with no live animals writes 0; leave those rows out of the range and the line.
    let lo = Infinity;
    let hi = -Infinity;
    for (const v of mult) if (v > 0) { lo = Math.min(lo, v); hi = Math.max(hi, v); }
    if (hi >= lo) {
      const span = hi > lo ? hi - lo : 1;
      ctx.strokeStyle = TRAIT_LINE_COLOR;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      let pen = false;
      for (let b = 0; b < buckets; b++) {
        const [i0, i1] = range(b);
        let sum = 0;
        let k = 0;
        for (let i = i0; i < i1; i++) if (mult[i] > 0) { sum += mult[i]; k++; }
        if (k === 0) { pen = false; continue; }
        const x = ax.xOf(ticks[Math.floor((i0 + i1 - 1) / 2)]);
        const y = r.y + r.h - ((sum / k - lo) / span) * (r.h - 2) - 1;
        if (pen) ctx.lineTo(x, y);
        else ctx.moveTo(x, y);
        pen = true;
      }
      ctx.stroke();
      items.push([`grazer cost × ${lo.toFixed(2)}–${hi.toFixed(2)}`, TRAIT_LINE_COLOR]);
    }
  }
  legend(ctx, r, items.length ? items : [['no fire or trait data', '#cccccc']]);
}

export interface ChartColors {
  grazer: string;
  hunter: string;
  tree: string;
}

/**
 * Four stacked panels over one tick axis: grazers and hunters, trees, deaths by cause, and fire and traits.
 * A dashed marker at `tick` crosses all four. The marker tick and x and the plot rectangles are
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
  const panels = chartLayout(w, h, 4);
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
  fireTraitPanel(ctx, panels[3], ax, ticks, series.extra);

  const mt = Math.min(Math.max(tick, t0), t1);
  const mx = Math.round(ax.xOf(mt)) + 0.5;
  ctx.strokeStyle = '#333';
  ctx.lineWidth = 1;
  ctx.setLineDash([3, 2]);
  ctx.beginPath();
  ctx.moveTo(mx, panels[0].y);
  ctx.lineTo(mx, panels[3].y + panels[3].h);
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

/**
 * The `medium` overlay's key: one chip per medium the run's `meta.json` lists, in that order and in the
 * palette the drape and the columns use. It is hidden on every other overlay and on a run with no ground
 * grid, so the noise world's sidebar is unchanged (DECISIONS.md, shot G7).
 */
export function legendItems(overlay: Overlay, media?: string[]): { label: string; css: string }[] {
  if (overlay !== 'medium' || !media?.length) return [];
  return media.map((label) => {
    const [r, g, b] = mediumColor(label);
    return { label, css: `rgb(${r}, ${g}, ${b})` };
  });
}

export function drawLegend(el: HTMLElement, overlay: Overlay, media?: string[]): void {
  const items = legendItems(overlay, media);
  el.hidden = items.length === 0;
  el.replaceChildren(...items.map(({ label, css }) => {
    const chip = document.createElement('span');
    chip.className = 'chip';
    const swatch = document.createElement('i');
    swatch.style.background = css;
    chip.append(swatch, document.createTextNode(label));
    return chip;
  }));
}

// ---- controls ----

export interface Controls {
  overlay: HTMLSelectElement;
  cam: HTMLSelectElement;
  slider: HTMLInputElement;
  play: HTMLButtonElement;
  readout: HTMLElement;
  chart: HTMLCanvasElement;
  legend: HTMLElement;
  status: HTMLElement;
  /** The overlay row and the play row, hidden on a `?world=` page (shot E1). */
  overlayRow: HTMLElement;
  playRow: HTMLElement;
  /** The editor's sidebar panel, its hotbar and readout, and the crosshair over the canvas. */
  edit: HTMLElement;
  hotbar: HTMLElement;
  editInfo: HTMLElement;
  editNote: HTMLElement;
  cross: HTMLElement;
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
    legend: q('legend'),
    status: q('status'),
    overlayRow: q('overlayrow'),
    playRow: q('playrow'),
    edit: q('edit'),
    hotbar: q('hotbar'),
    editInfo: q('editinfo'),
    editNote: q('editnote'),
    cross: q('cross'),
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

// ---- the editor's sidebar (shot E1) ----

/**
 * A world bundle has no snapshots and no series, so the tick slider, the play button and the overlay
 * selector go away and the chart area is left empty rather than drawn with a fake one.
 */
export function prepareWorldSidebar(c: Controls): void {
  for (const el of [c.overlayRow, c.playRow]) el.hidden = true;
  c.slider.hidden = true;
  // The edit panel takes the room four chart panels had, so the empty chart area shrinks to keep the
  // medium legend and the status line on screen.
  c.chart.style.height = `${WORLD_CHART_H}px`;
  const ctx = c.chart.getContext('2d')!;
  ctx.fillStyle = CHART_BG;
  ctx.fillRect(0, 0, c.chart.width, c.chart.height);
  c.edit.hidden = false;
}

/** The empty chart area's height on a world page, in CSS pixels. */
export const WORLD_CHART_H = 150;

/**
 * The note the shot asks for: one click is one cube, and a cube is the bundle's own cell size, so how many
 * clicks the sim notices depends on the bundle (shot E3). At the Capitol's 0.5 m cells it takes two; at
 * 0.25 m it takes four.
 */
export function stepNote(cell: number): string {
  const clicks = Math.max(1, Math.round(1 / cell));
  const moves = clicks === 1
    ? 'each click moves a column by one voxel'
    : `it takes ${clicks} clicks to move a column by one voxel`;
  return `One click is one ${cell} m cube of bundle height. The sim averages ground height over each `
    + `1 m ecology column and rounds it to a whole metre, so a click can change the picture without `
    + `changing the sim: ${moves}.`;
}

/** Draws the hotbar, the target readout and the dirty flag, and puts the crosshair over the picked cell. */
export function drawEditPanel(c: Controls, v: PanelView, media: string[]): void {
  c.cross.hidden = !v.editing;
  c.cross.style.left = `${Math.round(v.crosshair[0])}px`;
  c.cross.style.top = `${Math.round(v.crosshair[1])}px`;
  c.hotbar.replaceChildren(...SLOTS.map((slot, i) => {
    const b = document.createElement('span');
    b.className = slot.key === v.slot ? 'slot on' : 'slot';
    b.dataset.slot = slot.key;
    const sw = document.createElement('i');
    const [r, g, bl] = mediumColor(slot.key === 'building' ? 'roof' : slot.key === 'ground' ? 'soil' : slot.key);
    sw.style.background = `rgb(${r}, ${g}, ${bl})`;
    b.append(document.createTextNode(`${i + 1} `), sw, document.createTextNode(slot.label));
    return b;
  }));
  // Out of edit mode the panel is one line of invitation: a hotbar would offer actions that do nothing.
  c.hotbar.hidden = !v.editing;
  c.editNote.hidden = !v.editing;
  if (!v.editing) {
    c.editInfo.textContent = 'edit off · press E to edit this bundle';
    c.editInfo.classList.remove('dirty');
    return;
  }
  const t = v.target;
  const st = v.state;
  c.editInfo.textContent = [
    `edit ${v.editing ? 'on' : 'off'} (E) · camera ${v.fly ? 'fly' : 'orbit'} (F) · brush ${v.brush} ([ ])`,
    t && st
      ? `cell (${t.gx}, ${t.gy}) ${media[st.medium] ?? '?'} · ground ${st.ground_h.toFixed(2)} m`
        + ` · building ${st.building_h.toFixed(2)} m · ${t.top ? 'top' : 'side'} face`
      : 'no cell under the crosshair',
    `${v.undo} undo (Ctrl-Z) · ${v.redo} redo (Ctrl-Y) · ${v.saves} saved (Ctrl-S)`
      + (v.dirty ? ' · ● unsaved edits' : ''),
    v.message,
  ].join('\n');
  c.editInfo.classList.toggle('dirty', v.dirty);
}
