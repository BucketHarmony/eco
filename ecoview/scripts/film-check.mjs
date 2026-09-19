// Checks a film made by scripts/film.mjs: the MP4 has one video frame per PNG, and the frame at tick 10000,
// with the caption bar cropped off, matches screenshot 02 (material, tick 10000, iso) cropped to #view:
// pixelmatch threshold 0.1, at most 2% of pixels differ, as in shot-ref.mjs. The reference is
// shots/reference/02 when it was rendered on this platform, otherwise the fresh shots/02 from `npm run shot`.
// For a tiled film it compares the material:iso tile of that frame, minus its label rows, with the same rows of 02.
//   node scripts/film-check.mjs film/s42-material.mp4
import { spawnSync } from 'node:child_process';
import { readFile, readdir, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { PLATFORM } from './chromium.mjs';
import { LABEL_H, VIEW, cropPng, gridFor, parseTiles, tileRect } from './film-lib.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REF = '02_material_t10000_iso.png';
const TICK = 10000;
const MAX_DIFF = 0.02;

const mp4 = path.resolve(root, process.argv[2] ?? 'film/s42-material.mp4');
const framesDir = mp4.replace(/\.mp4$/i, '') + '-frames';
const { ticks, tiles, layout, scale = 1 } = JSON.parse(await readFile(path.join(framesDir, 'frames.json'), 'utf8'));
const pngs = (await readdir(framesDir)).filter((f) => f.endsWith('.png'));
let failed = 0;
const fail = (msg) => {
  failed++;
  console.error(`FAIL ${msg}`);
};

// A tiled film is checked on its material:iso tile, at scale 1, with the label rows cropped off both images.
let tile = null;
if (tiles) {
  const k = parseTiles(tiles).findIndex((t) => t.overlay === 'material' && t.cam === 'iso');
  if (k < 0) fail('a tiled film needs a material:iso tile to compare with screenshot 02');
  else if (scale !== 1) fail(`the tile check compares at scale 1; this film has --scale ${scale}`);
  else tile = tileRect(k, gridFor(parseTiles(tiles).length, layout), 1);
}
const size = PNG.sync.read(await readFile(path.join(framesDir, pngs[0])));

if (pngs.length !== ticks.length) fail(`${pngs.length} PNGs but frames.json lists ${ticks.length} ticks`);
const probe = spawnSync('ffprobe', ['-v', 'error', '-select_streams', 'v:0', '-count_frames',
  '-show_entries', 'stream=nb_read_frames,width,height', '-of', 'json', mp4], { encoding: 'utf8' });
if (probe.status !== 0) fail(`ffprobe ${mp4}: ${probe.error?.message ?? probe.stderr}`);
else {
  const s = JSON.parse(probe.stdout).streams[0];
  console.log(`${path.relative(root, mp4)}: ${s.nb_read_frames} frames, ${s.width}x${s.height}`);
  if (Number(s.nb_read_frames) !== ticks.length) fail(`MP4 has ${s.nb_read_frames} frames, expected ${ticks.length}`);
  if (s.width !== size.width || s.height !== size.height) fail(`MP4 is ${s.width}x${s.height}, frames are ${size.width}x${size.height}`);
}

const i = ticks.indexOf(TICK);
if (i < 0) fail(`no frame at tick ${TICK}`);
else if (!tiles || tile) {
  const recorded = (await readFile(path.join(root, 'shots/reference/PLATFORM'), 'utf8')).trim();
  const refPath = recorded === PLATFORM ? path.join(root, 'shots/reference', REF) : path.join(root, 'shots', REF);
  if (recorded !== PLATFORM) console.log(`note: references are from ${recorded}; comparing with the fresh shots/${REF}`);
  const top = tile ? LABEL_H : 0;
  const w = VIEW.width;
  const h = VIEW.height - top;
  const frame = cropPng(PNG.sync.read(await readFile(path.join(framesDir, `${String(i).padStart(5, '0')}.png`))),
    { x: tile?.x ?? 0, y: (tile?.y ?? 0) + top, width: w, height: h });
  const ref = cropPng(PNG.sync.read(await readFile(refPath)), { x: 0, y: top, width: w, height: h });
  const diff = new PNG({ width: w, height: h });
  const n = pixelmatch(frame.data, ref.data, diff.data, w, h, { threshold: 0.1 });
  const frac = n / (w * h);
  await mkdir(path.join(root, 'shots/diff'), { recursive: true });
  await writeFile(path.join(root, `shots/diff/film-frame-tick10000${tile ? '-tile' : ''}.png`), PNG.sync.write(diff));
  const what = tile ? `frame ${i} (tick ${TICK}) material:iso tile below its label` : `frame ${i} (tick ${TICK})`;
  console.log(`${what} vs ${path.relative(root, refPath)}: ${(frac * 100).toFixed(3)}% pixels differ`);
  if (frac > MAX_DIFF) fail(`frame ${i} differs from ${REF} by more than ${MAX_DIFF * 100}%`);
}
process.exit(failed ? 1 : 0);
