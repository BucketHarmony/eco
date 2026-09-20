// The sim helper's pure half (shot E4): request checking, the ecosim command line, and the path rules
// that keep the static server inside its own temporary directory. Kept apart from sim-server.mjs so
// tests/unit/sim.test.ts can exercise it without opening a socket.
import { resolve, sep } from 'node:path';

/** The seven files a world bundle is made of, in the order a save writes them (src/edit.ts). */
export const BUNDLE_TEXT = ['bundle.json', 'trees.json', 'shrubs.json', 'pipes.json'];
export const BUNDLE_BIN = ['ground_h.f32', 'medium.u8', 'building_h.f32'];
export const BUNDLE_FILES = [...BUNDLE_TEXT, ...BUNDLE_BIN];

export const DEFAULT_TICKS = 1000;
export const DEFAULT_SEED = 42;
/** A browser-side run is meant to answer a question in seconds; the reference runs stay on the command line. */
export const MAX_TICKS = 20000;
/** 2.3 MB of bundle becomes about 3.1 MB of base64; the cap is there so a stray POST cannot fill the disk. */
export const MAX_BODY = 64 * 1024 * 1024;

/** Ten snapshots over the run, whatever its length: enough to scrub, small enough to keep three runs on disk. */
export const snapshotEvery = (ticks) => Math.max(1, Math.round(ticks / 10));

const whole = (v, lo, hi, fallback) => {
  if (v === undefined || v === null || v === '') return fallback;
  const n = Number(v);
  return Number.isInteger(n) && n >= lo && n <= hi ? n : null;
};

/**
 * Checks a POST body against the wire contract: `{ ticks, seed, files }`, where `files` holds every one of
 * `BUNDLE_FILES` base64-encoded and nothing else. Throws with the reason, which the helper returns as a 400.
 */
export function checkRunRequest(body) {
  const bad = (m) => {
    throw new Error(m);
  };
  if (!body || typeof body !== 'object' || Array.isArray(body)) bad('body is not an object');
  const ticks = whole(body.ticks, 1, MAX_TICKS, DEFAULT_TICKS);
  if (ticks === null) bad(`ticks ${JSON.stringify(body.ticks)} is not a whole number in 1..=${MAX_TICKS}`);
  const seed = whole(body.seed, 0, 0xffffffff, DEFAULT_SEED);
  if (seed === null) bad(`seed ${JSON.stringify(body.seed)} is not a whole number in 0..=4294967295`);
  const files = body.files;
  if (!files || typeof files !== 'object' || Array.isArray(files)) bad('files is not an object');
  const got = Object.keys(files).sort();
  const want = [...BUNDLE_FILES].sort();
  if (got.length !== want.length || got.some((n, i) => n !== want[i])) {
    bad(`files must be exactly ${want.join(', ')}, got ${got.join(', ') || '(none)'}`);
  }
  const out = [];
  for (const name of BUNDLE_FILES) {
    const b64 = files[name];
    if (typeof b64 !== 'string') bad(`${name} is not a base64 string`);
    const bytes = Buffer.from(b64, 'base64');
    if (b64.length > 0 && bytes.length === 0) bad(`${name} is not base64`);
    out.push({ name, bytes });
  }
  return { ticks, seed, every: snapshotEvery(ticks), files: out };
}

/**
 * The command line, which is the one a hand-run garden run uses: the bundle world, animals off and rainfall
 * flat across the site (MASTER.md, 2026-09-19 21:05). `--snapshot-state false` leaves out `state.bin`, which
 * only a fork reads, so a run in a temporary directory is 40% smaller.
 */
export function runArgs(worldDir, outDir, { seed, ticks, every, params }) {
  const args = [
    'run', '--world', worldDir, '--out', outDir,
    '--seed', String(seed), '--ticks', String(ticks), '--snapshot-every', String(every),
    '--snapshot-state', 'false',
    '--set', 'animals.enabled=false', '--set', 'climate.rain_gradient=0',
  ];
  if (params) args.push('--params', params);
  return args;
}

/** The furthest tick written so far, from the snapshot directories on disk; the run's own progress report. */
export function tickOfSnapshots(names) {
  let tick = 0;
  for (const n of names) {
    const m = /^snap_(\d{6})$/.exec(n);
    if (m) tick = Math.max(tick, Number(m[1]));
  }
  return tick;
}

/**
 * Resolves `urlPath` under `root`, or null when it escapes. Every segment is taken literally: `.` and `..`
 * are refused rather than normalised away, both slashes count as separators so a Windows path cannot be
 * smuggled in, and a leading `/` or a drive letter is refused outright. The final `startsWith` is the
 * belt-and-braces check that the result really is inside the helper's own temporary directory (shot E4).
 */
export function safeJoin(root, urlPath) {
  let decoded;
  try {
    decoded = decodeURIComponent(urlPath);
  } catch {
    return null;
  }
  if (decoded.includes('\0')) return null;
  const parts = decoded.split(/[/\u005c]+/).filter((s) => s.length > 0);
  if (parts.length === 0) return null;
  if (parts.some((s) => s === '.' || s === '..' || /^[a-zA-Z]:$/.test(s))) return null;
  const path = resolve(root, ...parts);
  return path.startsWith(resolve(root) + sep) ? path : null;
}

/** Content types for the files a run directory holds; anything else is served as bytes. */
export function contentType(name) {
  if (name.endsWith('.json')) return 'application/json';
  if (name.endsWith('.csv')) return 'text/csv; charset=utf-8';
  return 'application/octet-stream';
}
