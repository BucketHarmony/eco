// Visual regression against shots/reference/. `check` compares the fresh shots/*.png with pixelmatch
// (threshold 0.1) and fails if a gated region differs by more than 2% of the screenshot, writing diffs to
// shots/diff/. The regions are the #view canvas and the sidebar (scripts/shot-diff.mjs): the view is
// gated everywhere, the sidebar only on the platform the references were rendered on, so the check
// runs on Linux CI instead of being skipped there (shot E5).
// `accept` copies the fresh shots over the references and records the rendering platform; with name
// substrings after it, only the matching shots are accepted, which is how a shot that adds references
// avoids a mass re-accept (overnight/MASTER.md).
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { PNG } from 'pngjs';
import { SHOTS } from './shots.mjs';
import { PLATFORM } from './chromium.mjs';
import { MAX_DIFF, compareRegions } from './shot-diff.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const shots = path.join(root, 'shots');
const refDir = path.join(shots, 'reference');
const diffDir = path.join(shots, 'diff');
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
  const samePlatform = recorded === PLATFORM;
  if (!samePlatform) {
    console.log(`note: references were rendered on ${recorded}, this is ${PLATFORM};`
      + ' the sidebar is measured but not gated, the view canvas is gated as usual');
  }
  await mkdir(diffDir, { recursive: true });
  let failed = 0;
  for (const [file] of SHOTS) {
    const ref = PNG.sync.read(await readFile(path.join(refDir, file)));
    const cur = PNG.sync.read(await readFile(path.join(shots, file)));
    let result;
    try {
      result = compareRegions(ref, cur, { samePlatform });
    } catch (e) {
      console.error(`FAIL ${file}: ${e.message}`);
      failed++;
      continue;
    }
    await writeFile(path.join(diffDir, file), PNG.sync.write(result.diff));
    if (!result.ok) failed++;
    const parts = result.regions.map(
      (r) => `${r.name} ${(r.frac * 100).toFixed(3)}% of the page`
        + ` (${(r.regionFrac * 100).toFixed(3)}% of the region${r.gated ? '' : ', not gated'})`,
    );
    console.log(`${result.ok ? 'ok  ' : 'FAIL'} ${file}: ${parts.join(', ')}`);
  }
  if (failed) {
    console.error(`${failed} of ${SHOTS.length} shots differ from shots/reference by more than ${MAX_DIFF * 100}% in a gated region`);
    process.exit(1);
  }
} else {
  console.error('usage: node scripts/shot-ref.mjs check | accept [name-substring ...]');
  process.exit(2);
}
