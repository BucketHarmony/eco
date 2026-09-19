// Time-lapse export: serves dist/ with Vite's preview() API, steps headless Chromium through every snapshot
// whose tick is a multiple of --every, writes one PNG per frame, and encodes them with ffmpeg.
//   npm run film -- --run runs/s42 --overlay material --cam iso --every 100 --fps 12 --out film/s42-material.mp4
// Frames go to <out without .mp4>-frames/NNNNN.png, with frames.json listing their ticks.
// Optional: --limit N (first N frames), --max-seconds S (fail if the whole export takes longer).
// Tiled: --tiles overlay:cam,… [--layout CxR] [--scale N] replaces --overlay/--cam with a grid of views in lockstep:
//   npm run film -- --run runs/s42 --tiles material:iso,fire:top,crowding:top,traits:top --layout 2x2 --out film/s42-tiled.mp4
import { spawnSync } from 'node:child_process';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { preview } from 'vite';
import { chromium } from '@playwright/test';
import { CHROMIUM_ARGS } from './chromium.mjs';
import {
  FRAME, captureFrames, captureTiledFrames, gridFor, h264Level, parseArgs, parseTiles, tiledFrameSize,
} from './film-lib.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const PORT = 4175;

const t0 = Date.now();
const opts = parseArgs(process.argv.slice(2));
const out = path.resolve(root, opts.out);
const framesDir = out.replace(/\.mp4$/i, '') + '-frames';
const size = opts.tiles ? tiledFrameSize(gridFor(parseTiles(opts.tiles).length, opts.layout), opts.scale) : FRAME;
const level = h264Level(size.width, size.height, opts.fps);
await rm(framesDir, { recursive: true, force: true });
await mkdir(framesDir, { recursive: true });

const server = await preview({ root, logLevel: 'warn', preview: { port: PORT, strictPort: true } });
let browser;
let failed = false;
let ticks = [];
try {
  browser = await chromium.launch({ args: CHROMIUM_ARGS });
  const context = await browser.newContext({ deviceScaleFactor: opts.scale });
  context.on('weberror', (e) => {
    failed = true;
    console.error(`page error: ${e.error().message}`);
  });
  const onFrame = async (i, tick, png) => {
    await writeFile(path.join(framesDir, `${String(i).padStart(5, '0')}.png`), png);
    if (i % 50 === 0) console.log(`frame ${i} (tick ${tick})`);
  };
  const url = `http://localhost:${PORT}`;
  ticks = opts.tiles
    ? await captureTiledFrames(context, url, opts, onFrame)
    : await captureFrames(await context.newPage(), url, opts, onFrame);
  await writeFile(path.join(framesDir, 'frames.json'), `${JSON.stringify({ ...opts, limit: undefined, maxSeconds: undefined, ticks })}\n`);
} catch (e) {
  failed = true;
  console.error(e);
} finally {
  await browser?.close();
  await new Promise((r) => server.httpServer.close(r));
}

if (!failed) {
  const args = ['-y', '-loglevel', 'error', '-framerate', String(opts.fps), '-i', path.join(framesDir, '%05d.png'),
    '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-level', level, '-crf', '20', '-preset', 'medium', out];
  const r = spawnSync('ffmpeg', args, { stdio: 'inherit' });
  if (r.error || r.status !== 0) {
    failed = true;
    console.error(r.error?.code === 'ENOENT'
      ? 'ffmpeg not found on PATH (Linux: apt-get install ffmpeg; Windows: winget install Gyan.FFmpeg)'
      : `ffmpeg exited with ${r.status}`);
  }
}

const secs = (Date.now() - t0) / 1000;
console.log(`${ticks.length} frames${failed ? '' : ` → ${path.relative(root, out)}, ${size.width}x${size.height}, H.264 level ${level}`} in ${secs.toFixed(1)} s`);
if (secs > opts.maxSeconds) {
  failed = true;
  console.error(`took ${secs.toFixed(1)} s, over --max-seconds ${opts.maxSeconds}`);
}
process.exit(failed ? 1 : 0);
