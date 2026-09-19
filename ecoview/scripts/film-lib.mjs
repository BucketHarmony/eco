// Time-lapse capture shared by scripts/film.mjs and the determinism test. Pure helpers plus captureFrames,
// which steps an already-served ecoview page through snapshots and screenshots #view with a caption bar under it,
// and captureTiledFrames, which does the same for several overlay:cam views in lockstep and composites them.
import { PNG } from 'pngjs';

export const VIEW = { width: 960, height: 800 };
export const CAPTION_H = 32;
export const FRAME = { width: VIEW.width, height: VIEW.height + CAPTION_H };
export const LABEL_H = 24;
/** The page background (index.html), used for grid cells with no tile. */
export const BG_RGB = [0xe8, 0xec, 0xf0];
const CAMS = ['iso', 'top', 'side'];

const DEFAULTS = { run: 'runs/s42', overlay: 'material', cam: 'iso', every: 100, fps: 12, out: 'film/s42-material.mp4' };

/** Parses `--key value` pairs. Numbers: every, fps, limit, max-seconds, scale. */
export function parseArgs(argv) {
  const o = { ...DEFAULTS, tiles: '', layout: '', scale: 1, limit: Infinity, maxSeconds: Infinity };
  for (let i = 0; i < argv.length; i += 2) {
    const k = argv[i];
    const v = argv[i + 1];
    if (!k.startsWith('--') || v === undefined) throw new Error(`bad argument ${k}`);
    const key = k.slice(2).replace(/-(\w)/g, (_, c) => c.toUpperCase());
    if (!(key in o)) throw new Error(`unknown option ${k}`);
    if (typeof o[key] === 'number') {
      const n = Number(v);
      if (!Number.isFinite(n) || n <= 0) throw new Error(`${k} needs a positive number, got ${v}`);
      o[key] = n;
    } else {
      o[key] = v;
    }
  }
  o.run = o.run.replace(/^\/+|\/+$/g, '');
  if (!Number.isInteger(o.scale)) throw new Error(`--scale needs a whole number, got ${o.scale}`);
  if (o.tiles) gridFor(parseTiles(o.tiles).length, o.layout);
  else if (o.layout || o.scale !== 1) throw new Error('--layout and --scale need --tiles');
  return o;
}

/** `material:iso,fire:top` → [{ overlay, cam }, …]. */
export function parseTiles(spec) {
  return spec.split(',').map((t) => {
    const [overlay, cam, extra] = t.split(':');
    if (!overlay || !CAMS.includes(cam) || extra !== undefined) {
      throw new Error(`bad tile ${t}: expected overlay:cam with cam one of ${CAMS.join('|')}`);
    }
    return { overlay, cam };
  });
}

/** `CxR` → { cols, rows }; with no layout, the smallest near-square grid (cols = ceil √n) that fits n tiles. */
export function gridFor(n, layout = '') {
  if (!layout) {
    const cols = Math.ceil(Math.sqrt(n));
    return { cols, rows: Math.ceil(n / cols) };
  }
  const m = /^(\d+)x(\d+)$/.exec(layout);
  if (!m || +m[1] < 1 || +m[2] < 1) throw new Error(`bad --layout ${layout}: expected CxR, e.g. 2x2`);
  const g = { cols: +m[1], rows: +m[2] };
  if (g.cols * g.rows < n) throw new Error(`--layout ${layout} has ${g.cols * g.rows} cells for ${n} tiles`);
  return g;
}

/** Pixel size of a tiled frame: the grid of scaled #view tiles plus the scaled caption bar. */
export function tiledFrameSize(grid, scale) {
  return { width: grid.cols * VIEW.width * scale, height: (grid.rows * VIEW.height + CAPTION_H) * scale };
}

/** Tile i's rectangle in the frame, row-major. */
export function tileRect(i, grid, scale) {
  const w = VIEW.width * scale;
  const h = VIEW.height * scale;
  return { x: (i % grid.cols) * w, y: Math.floor(i / grid.cols) * h, width: w, height: h };
}

// H.264 Annex A: [level, max macroblocks per second, max frame size in macroblocks].
const H264_LEVELS = [
  ['3.0', 40500, 1620], ['3.1', 108000, 3600], ['3.2', 216000, 5120], ['4.0', 245760, 8192],
  ['4.2', 522240, 8704], ['5.0', 589824, 22080], ['5.1', 983040, 36864], ['5.2', 2073600, 36864],
  ['6.0', 4177920, 139264], ['6.1', 8355840, 139264], ['6.2', 16711680, 139264],
];

/** The lowest H.264 level that allows a w×h stream at fps, or an error naming the limit it breaks. */
export function h264Level(w, h, fps) {
  if (w % 2 || h % 2) throw new Error(`frame ${w}x${h} has an odd side; yuv420p needs even width and height`);
  const mw = Math.ceil(w / 16);
  const mh = Math.ceil(h / 16);
  const fs = mw * mh;
  for (const [level, mbps, maxFs] of H264_LEVELS) {
    const side = Math.sqrt(8 * maxFs); // A.3.1: neither side may exceed sqrt(8 * MaxFS) macroblocks
    if (fs <= maxFs && mw <= side && mh <= side && fs * fps <= mbps) return level;
  }
  throw new Error(`frame ${w}x${h} at ${fps} fps is beyond H.264 level 6.2 (at most 139264 macroblocks per frame, `
    + '16711680 per second); use a smaller --layout, --scale or --fps');
}

/** Snapshot ticks that are multiples of `every`, in order, at most `limit` of them. */
export function frameTicks(snapshots, every, limit = Infinity) {
  return snapshots.filter((t) => t % every === 0).slice(0, limit);
}

/** tick → { grazers, hunters, trees } from series.csv, read by header name. */
export function parseCounts(csv) {
  const lines = csv.trim().split(/\r?\n/);
  const head = lines[0].split(',');
  const col = (name) => {
    const i = head.indexOf(name);
    if (i < 0) throw new Error(`series.csv has no ${name} column`);
    return i;
  };
  const [t, g, h, tr] = ['tick', 'grazers', 'hunters', 'trees'].map(col);
  const m = new Map();
  for (let i = 1; i < lines.length; i++) {
    const f = lines[i].split(',');
    m.set(Number(f[t]), { grazers: Number(f[g]), hunters: Number(f[h]), trees: Number(f[tr]) });
  }
  return m;
}

export function captionText(tick, c) {
  const n = (v) => (c ? String(v) : '–');
  return `tick ${tick}   grazers ${n(c?.grazers)}   hunters ${n(c?.hunters)}   trees ${n(c?.trees)}`;
}

/**
 * Loads `baseUrl/?run=…` in `page`, then for each frame tick moves the view there, sets the caption and
 * calls `onFrame(index, tick, png)` with a FRAME-sized PNG: #view on top, the caption bar below it.
 * Returns the ticks captured.
 */
export async function captureFrames(page, baseUrl, opts, onFrame) {
  await page.setViewportSize({ width: 1280, height: FRAME.height });
  const ready = () => page.waitForFunction(() => window.__ecoviewReady === true || !!window.__ecoviewError, null, {
    timeout: 30_000,
  });
  const checkError = async () => {
    const err = await page.evaluate(() => window.__ecoviewError);
    if (err) throw new Error(`ecoview: ${err}`);
  };
  const base = `${baseUrl}/${opts.run}`;
  const meta = await (await page.request.get(`${base}/meta.json`)).json();
  const counts = parseCounts(await (await page.request.get(`${base}/series.csv`)).text());
  const ticks = frameTicks(meta.snapshots, opts.every, opts.limit);
  if (ticks.length === 0) throw new Error(`no snapshot tick of ${opts.run} is a multiple of ${opts.every}`);

  await page.goto(`${baseUrl}/?run=${opts.run}&tick=${ticks[0]}&overlay=${opts.overlay}&cam=${opts.cam}`);
  await ready();
  await checkError();
  await page.evaluate(({ top, w, h }) => {
    const bar = document.createElement('div');
    bar.id = 'film-caption';
    Object.assign(bar.style, {
      position: 'fixed', left: '0', top: `${top}px`, width: `${w}px`, height: `${h}px`, boxSizing: 'border-box',
      padding: '0 12px', background: '#1d232a', color: '#f2f4f6', font: '600 15px/32px system-ui, sans-serif',
      fontVariantNumeric: 'tabular-nums', whiteSpace: 'pre', overflow: 'hidden',
    });
    document.body.appendChild(bar);
  }, { top: VIEW.height, w: FRAME.width, h: CAPTION_H });

  for (let i = 0; i < ticks.length; i++) {
    const tick = ticks[i];
    if (i > 0) {
      await page.evaluate((t) => window.__ecoviewGoto(t), tick);
      await ready();
      await checkError();
    }
    await page.evaluate((text) => { document.getElementById('film-caption').textContent = text; },
      captionText(tick, counts.get(tick)));
    const png = await page.screenshot({ clip: { x: 0, y: 0, ...FRAME } });
    await onFrame(i, tick, png);
  }
  return ticks;
}

/**
 * The tiled counterpart of captureFrames. Opens one page per tile in `context` (whose deviceScaleFactor is the
 * scale), steps every page to the same tick, and calls `onFrame(index, tick, png)` with the composite: tiles in
 * row-major order, each labelled `overlay · cam` in its top-left corner, empty cells in the page background, and
 * one caption bar across the bottom. Returns the ticks captured.
 */
export async function captureTiledFrames(context, baseUrl, opts, onFrame) {
  const tiles = parseTiles(opts.tiles);
  const grid = gridFor(tiles.length, opts.layout);
  const scale = opts.scale ?? 1;
  const base = `${baseUrl}/${opts.run}`;
  const meta = await (await context.request.get(`${base}/meta.json`)).json();
  const counts = parseCounts(await (await context.request.get(`${base}/series.csv`)).text());
  const ticks = frameTicks(meta.snapshots, opts.every, opts.limit);
  if (ticks.length === 0) throw new Error(`no snapshot tick of ${opts.run} is a multiple of ${opts.every}`);

  const ready = async (page) => {
    await page.waitForFunction(() => window.__ecoviewReady === true || !!window.__ecoviewError, null, { timeout: 30_000 });
    const err = await page.evaluate(() => window.__ecoviewError);
    if (err) throw new Error(`ecoview: ${err}`);
  };
  const barW = grid.cols * VIEW.width;
  const pages = await Promise.all(tiles.map(async ({ overlay, cam }, i) => {
    const page = await context.newPage();
    // Every page is wide enough for the caption bar, which only page 0 draws.
    await page.setViewportSize({ width: Math.max(1280, barW), height: FRAME.height });
    await page.goto(`${baseUrl}/?run=${opts.run}&tick=${ticks[0]}&overlay=${overlay}&cam=${cam}`);
    await ready(page);
    await page.evaluate(({ text, labelH, bar, top, w, h }) => {
      const add = (style, id) => {
        const el = document.createElement('div');
        if (id) el.id = id;
        Object.assign(el.style, { position: 'fixed', left: '0', color: '#f2f4f6', whiteSpace: 'pre', ...style });
        document.body.appendChild(el);
        return el;
      };
      add({ top: '0', height: `${labelH}px`, padding: '0 8px', background: 'rgba(29,35,42,0.8)',
        font: '600 13px/24px system-ui, sans-serif' }).textContent = text;
      if (bar) {
        add({ top: `${top}px`, width: `${w}px`, height: `${h}px`, boxSizing: 'border-box', padding: '0 12px',
          background: '#1d232a', font: '600 15px/32px system-ui, sans-serif', fontVariantNumeric: 'tabular-nums',
          overflow: 'hidden' }, 'film-caption');
      }
    }, { text: `${overlay} · ${cam}`, labelH: LABEL_H, bar: i === 0, top: VIEW.height, w: barW, h: CAPTION_H });
    return page;
  }));

  for (let f = 0; f < ticks.length; f++) {
    const tick = ticks[f];
    if (f > 0) {
      await Promise.all(pages.map(async (page) => {
        await page.evaluate((t) => window.__ecoviewGoto(t), tick);
        await ready(page);
      }));
    }
    await pages[0].evaluate((text) => { document.getElementById('film-caption').textContent = text; },
      captionText(tick, counts.get(tick)));
    const shots = await Promise.all(pages.map((page) => page.screenshot({ clip: { x: 0, y: 0, ...VIEW } })));
    const bar = await pages[0].screenshot({ clip: { x: 0, y: VIEW.height, width: barW, height: CAPTION_H } });

    const out = new PNG(tiledFrameSize(grid, scale));
    const bg = Buffer.from([...BG_RGB, 255]);
    for (let p = 0; p < out.data.length; p += 4) bg.copy(out.data, p);
    shots.forEach((s, i) => pastePng(out, PNG.sync.read(s), tileRect(i, grid, scale)));
    pastePng(out, PNG.sync.read(bar), { x: 0, y: grid.rows * VIEW.height * scale });
    // RGB with the Paeth filter: a third of the time of pngjs's adaptive default, about the same size.
    await onFrame(f, tick, PNG.sync.write(out, { colorType: 2, filterType: 4 }));
  }
  await Promise.all(pages.map((p) => p.close()));
  return ticks;
}

/** Copies all of decoded image `src` into `dst` with its top-left corner at (at.x, at.y). */
export function pastePng(dst, src, at) {
  if (at.x + src.width > dst.width || at.y + src.height > dst.height) {
    throw new Error(`paste ${src.width}x${src.height}+${at.x}+${at.y} is outside a ${dst.width}x${dst.height} image`);
  }
  for (let y = 0; y < src.height; y++) {
    src.data.copy(dst.data, ((at.y + y) * dst.width + at.x) * 4, y * src.width * 4, (y + 1) * src.width * 4);
  }
}

/** Copies rectangle `r` out of a decoded pngjs image into a new one. */
export function cropPng(img, r) {
  if (r.x + r.width > img.width || r.y + r.height > img.height) {
    throw new Error(`crop ${r.width}x${r.height}+${r.x}+${r.y} is outside a ${img.width}x${img.height} image`);
  }
  const out = { width: r.width, height: r.height, data: Buffer.alloc(r.width * r.height * 4) };
  for (let y = 0; y < r.height; y++) {
    const src = ((r.y + y) * img.width + r.x) * 4;
    img.data.copy(out.data, y * r.width * 4, src, src + r.width * 4);
  }
  return out;
}
