// Region-aware screenshot comparison, shared by scripts/shot-ref.mjs and tests/unit/shot-diff.test.ts.
//
// A reference screenshot is the whole 1280x800 page: the 960x800 #view canvas beside the 320 px
// sidebar (index.html). SwiftShader draws the canvas identically on Windows and on Linux CI — 0
// differing pixels in all 17 references, measured in shot E5 — but the sidebar's text is antialiased
// differently, over 5.5-9% of its pixels. So the view region is gated on every platform and the
// sidebar only on the platform the references were rendered on, which is what lets CI run the check
// at all (DECISIONS.md, "E5 reference screenshots in CI").
//
// Each region's gate is the same 2% of the *screenshot* the flat whole-image check used, not 2% of
// the region, so no existing tolerance moves: a difference inside one region fails at exactly the
// pixel count it failed at before. Shot E6 added a second, coarser switch on top of it -- whether
// the shot is gated at all -- and moved no tolerance either.
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { VIEWPORT } from './chromium.mjs';
import { VIEW, cropPng, pastePng } from './film-lib.mjs';

/** pixelmatch's per-pixel colour tolerance, and the share of the screenshot one region may differ by. */
export const THRESHOLD = 0.1;
export const MAX_DIFF = 0.02;

/**
 * The two halves of a screenshot, which tile it exactly. `crossPlatform` says whether the region may
 * be compared with references rendered on another platform's SwiftShader.
 */
export const REGIONS = [
  { name: 'view', x: 0, y: 0, width: VIEW.width, height: VIEW.height, crossPlatform: true },
  {
    name: 'sidebar',
    x: VIEW.width,
    y: 0,
    width: VIEWPORT.width - VIEW.width,
    height: VIEW.height,
    crossPlatform: false,
  },
];

/**
 * Compares two decoded screenshots region by region. With `samePlatform` false the regions that
 * carry platform-dependent text are still measured, but a difference there doesn't fail; with
 * `gated` false no region of this screenshot fails, which is how the nine pictures of the
 * simulation are measured and reported without gating a frozen viewer on a moving simulator
 * (shot E6, scripts/shots.mjs). Gating is per shot and per region, and a shot has to be gated for
 * either to bite.
 * Each result carries `frac`, the differing share of the whole screenshot, which is what the gate
 * reads, and `regionFrac`, the differing share of the region itself, which is what reads clearly in
 * a report. Returns those results and a full-size diff image made of the regions' diffs.
 * Throws if the two images are different sizes.
 */
export function compareRegions(ref, cur, { samePlatform, gated: shotGated = true }) {
  if (ref.width !== cur.width || ref.height !== cur.height) {
    throw new Error(`size ${cur.width}x${cur.height}, reference ${ref.width}x${ref.height}`);
  }
  const page = ref.width * ref.height;
  const diff = new PNG({ width: ref.width, height: ref.height });
  const regions = REGIONS.map((r) => {
    const a = cropPng(ref, r);
    const b = cropPng(cur, r);
    const d = new PNG({ width: r.width, height: r.height });
    const px = pixelmatch(a.data, b.data, d.data, r.width, r.height, { threshold: THRESHOLD });
    pastePng(diff, d, r);
    const gated = shotGated && (r.crossPlatform || samePlatform);
    const frac = px / page;
    return { name: r.name, px, frac, regionFrac: px / (r.width * r.height), gated, ok: !gated || frac <= MAX_DIFF };
  });
  return { regions, diff, ok: regions.every((r) => r.ok) };
}
