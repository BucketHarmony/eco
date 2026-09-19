// Checks a film made by scripts/film.mjs: the MP4 has one video frame per PNG, and the frame at tick 10000,
// with the caption bar cropped off, matches screenshot 02 (material, tick 10000, iso) cropped to #view:
// pixelmatch threshold 0.1, at most 2% of pixels differ, as in shot-ref.mjs. The reference is
// shots/reference/02 when it was rendered on this platform, otherwise the fresh shots/02 from `npm run shot`.
//   node scripts/film-check.mjs film/s42-material.mp4
import { spawnSync } from 'node:child_process';
import { readFile, readdir, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { PLATFORM } from './chromium.mjs';
import { VIEW, cropPng } from './film-lib.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REF = '02_material_t10000_iso.png';
const TICK = 10000;
const MAX_DIFF = 0.02;

const mp4 = path.resolve(root, process.argv[2] ?? 'film/s42-material.mp4');
const framesDir = mp4.replace(/\.mp4$/i, '') + '-frames';
const { ticks } = JSON.parse(await readFile(path.join(framesDir, 'frames.json'), 'utf8'));
const pngs = (await readdir(framesDir)).filter((f) => f.endsWith('.png'));
let failed = 0;
const fail = (msg) => {
  failed++;
  console.error(`FAIL ${msg}`);
};

if (pngs.length !== ticks.length) fail(`${pngs.length} PNGs but frames.json lists ${ticks.length} ticks`);
const probe = spawnSync('ffprobe', ['-v', 'error', '-select_streams', 'v:0', '-count_frames',
  '-show_entries', 'stream=nb_read_frames,width,height', '-of', 'json', mp4], { encoding: 'utf8' });
if (probe.status !== 0) fail(`ffprobe ${mp4}: ${probe.error?.message ?? probe.stderr}`);
else {
  const s = JSON.parse(probe.stdout).streams[0];
  console.log(`${path.relative(root, mp4)}: ${s.nb_read_frames} frames, ${s.width}x${s.height}`);
  if (Number(s.nb_read_frames) !== ticks.length) fail(`MP4 has ${s.nb_read_frames} frames, expected ${ticks.length}`);
}

const i = ticks.indexOf(TICK);
if (i < 0) fail(`no frame at tick ${TICK}`);
else {
  const recorded = (await readFile(path.join(root, 'shots/reference/PLATFORM'), 'utf8')).trim();
  const refPath = recorded === PLATFORM ? path.join(root, 'shots/reference', REF) : path.join(root, 'shots', REF);
  if (recorded !== PLATFORM) console.log(`note: references are from ${recorded}; comparing with the fresh shots/${REF}`);
  const rect = { x: 0, y: 0, ...VIEW };
  const frame = cropPng(PNG.sync.read(await readFile(path.join(framesDir, `${String(i).padStart(5, '0')}.png`))), rect);
  const ref = cropPng(PNG.sync.read(await readFile(refPath)), rect);
  const diff = new PNG(VIEW);
  const n = pixelmatch(frame.data, ref.data, diff.data, VIEW.width, VIEW.height, { threshold: 0.1 });
  const frac = n / (VIEW.width * VIEW.height);
  await mkdir(path.join(root, 'shots/diff'), { recursive: true });
  await writeFile(path.join(root, 'shots/diff/film-frame-tick10000.png'), PNG.sync.write(diff));
  console.log(`frame ${i} (tick ${TICK}) vs ${path.relative(root, refPath)}: ${(frac * 100).toFixed(3)}% pixels differ`);
  if (frac > MAX_DIFF) fail(`frame ${i} differs from ${REF} by more than ${MAX_DIFF * 100}%`);
}
process.exit(failed ? 1 : 0);
