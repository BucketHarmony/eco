// Time-lapse capture shared by scripts/film.mjs and the determinism test. Pure helpers plus captureFrames,
// which steps an already-served ecoview page through snapshots and screenshots #view with a caption bar under it.
export const VIEW = { width: 960, height: 800 };
export const CAPTION_H = 32;
export const FRAME = { width: VIEW.width, height: VIEW.height + CAPTION_H };

const DEFAULTS = { run: 'runs/s42', overlay: 'material', cam: 'iso', every: 100, fps: 12, out: 'film/s42-material.mp4' };

/** Parses `--key value` pairs. Numbers: every, fps, limit, max-seconds. */
export function parseArgs(argv) {
  const o = { ...DEFAULTS, limit: Infinity, maxSeconds: Infinity };
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
  return o;
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
