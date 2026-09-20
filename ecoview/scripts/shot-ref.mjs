// Visual regression against shots/reference/. `check` compares the fresh shots/*.png with pixelmatch
// (threshold 0.1) and fails if more than 2% of any image's pixels differ, writing diffs to shots/diff/.
// `accept` copies the fresh shots over the references and records the rendering platform; with name
// substrings after it, only the matching shots are accepted, which is how a shot that adds references
// avoids a mass re-accept (overnight/MASTER.md).
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { SHOTS } from './shots.mjs';
import { PLATFORM } from './chromium.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const shots = path.join(root, 'shots');
const refDir = path.join(shots, 'reference');
const diffDir = path.join(shots, 'diff');
const THRESHOLD = 0.1;
const MAX_DIFF = 0.02;

const mode = process.argv[2];
if (mode === 'accept') {
  const only = process.argv.slice(3);
  const take = SHOTS.filter(([file]) => only.length === 0 || only.some((s) => file.includes(s)));
  if (only.length && take.length === 0) {
    console.error(`no shot matches ${only.join(' ')}`);
    process.exit(2);
  }
  await mkdir(refDir, { recursive: true });
  for (const [file] of take) await copyFile(path.join(shots, file), path.join(refDir, file));
  if (only.length === 0) await writeFile(path.join(refDir, 'PLATFORM'), `${PLATFORM}\n`);
  const which = only.length ? `: ${take.map(([f]) => f).join(', ')}` : '';
  console.log(`accepted ${take.length} references (${PLATFORM})${which}`);
} else if (mode === 'check') {
  const recorded = (await readFile(path.join(refDir, 'PLATFORM'), 'utf8')).trim();
  if (recorded !== PLATFORM) {
    console.log(`note: references were rendered on ${recorded}, this is ${PLATFORM}; expect platform drift`);
  }
  await mkdir(diffDir, { recursive: true });
  let failed = 0;
  for (const [file] of SHOTS) {
    const ref = PNG.sync.read(await readFile(path.join(refDir, file)));
    const cur = PNG.sync.read(await readFile(path.join(shots, file)));
    if (ref.width !== cur.width || ref.height !== cur.height) {
      console.error(`FAIL ${file}: size ${cur.width}x${cur.height}, reference ${ref.width}x${ref.height}`);
      failed++;
      continue;
    }
    const diff = new PNG({ width: ref.width, height: ref.height });
    const n = pixelmatch(ref.data, cur.data, diff.data, ref.width, ref.height, { threshold: THRESHOLD });
    const frac = n / (ref.width * ref.height);
    await writeFile(path.join(diffDir, file), PNG.sync.write(diff));
    const ok = frac <= MAX_DIFF;
    if (!ok) failed++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${file}: ${(frac * 100).toFixed(3)}% pixels differ`);
  }
  if (failed) {
    console.error(`${failed} of ${SHOTS.length} shots differ from shots/reference by more than ${MAX_DIFF * 100}%`);
    process.exit(1);
  }
} else {
  console.error('usage: node scripts/shot-ref.mjs check | accept [name-substring ...]');
  process.exit(2);
}
