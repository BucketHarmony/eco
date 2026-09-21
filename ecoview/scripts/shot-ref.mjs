// Visual regression against shots/reference/. `check` compares the fresh shots/*.png with pixelmatch
// (threshold 0.1) and fails if a gated region of a gated shot differs by more than 2% of the screenshot,
// writing diffs to shots/diff/. The regions are the #view canvas and the sidebar (scripts/shot-diff.mjs):
// the view is gated everywhere, the sidebar only on the platform the references were rendered on, so the
// check runs on Linux CI instead of being skipped there (shot E5). Which *shots* are gated is
// scripts/shots.mjs's third column: 8 picture the renderer and gate, 9 picture the simulation and are
// measured, printed and reported but cannot fail a job (shot E6). Everything is still compared, so drift
// in the ungated nine is visible in the log the tick before someone asks about it.
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
  const gatedShots = SHOTS.filter(([, , gated]) => gated);
  let failed = 0;
  let worstGated = { file: '(none)', frac: 0 };
  const drifted = [];
  for (const [file, , gated] of SHOTS) {
    const ref = PNG.sync.read(await readFile(path.join(refDir, file)));
    const cur = PNG.sync.read(await readFile(path.join(shots, file)));
    let result;
    try {
      // A size mismatch is the viewport changing, which is the renderer's own doing, so it fails
      // whether the shot is gated or not.
      result = compareRegions(ref, cur, { samePlatform, gated });
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
    const view = result.regions.find((r) => r.name === 'view');
    if (gated) {
      if (view.frac > worstGated.frac) worstGated = { file, frac: view.frac };
    } else if (view.frac > MAX_DIFF) {
      drifted.push(`${file} ${(view.frac * 100).toFixed(3)}%`);
    }
    const mark = result.ok ? (gated ? 'ok  ' : 'show') : 'FAIL';
    console.log(`${mark} ${file}: ${parts.join(', ')}`);
  }
  // The ungated nine are a picture of the simulator, so drift there is news rather than a fault; say so
  // in one line instead of leaving it to be read out of seventeen (shot E6).
  console.log(`gated ${gatedShots.length} of ${SHOTS.length} shots; the other ${SHOTS.length - gatedShots.length} picture the simulation and are measured only`);
  if (drifted.length) console.log(`note: ungated views past ${MAX_DIFF * 100}%: ${drifted.join(', ')}`);
  console.log(`worst gated view ${(worstGated.frac * 100).toFixed(3)}% of ${MAX_DIFF * 100}% (${worstGated.file})`);
  if (failed) {
    console.error(`${failed} of ${gatedShots.length} gated shots differ from shots/reference by more than ${MAX_DIFF * 100}% in a gated region`);
    process.exit(1);
  }
} else {
  console.error('usage: node scripts/shot-ref.mjs check | accept [name-substring ...]');
  process.exit(2);
}
