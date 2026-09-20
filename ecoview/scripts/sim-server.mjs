// The sim helper (shot E4): a loopback-only dev server that runs the real `ecosim` binary on a bundle the
// editor posts, and serves the resulting run directory back to the page. It is dev tooling — the built site
// never needs it — and it is not part of `npm run shot`.
//
//   npm run sim                      the helper alone, on port 4174
//   npm run preview:sim              the helper and `vite preview` together (scripts/dev-sim.mjs)
//
// `vite.config.ts` proxies /sim to this port, so the page talks to its own origin and the loader needs no
// change: a finished run is served at /sim/runs/<id>/, which is a run directory like any other.
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { MAX_BODY, checkRunRequest, contentType, runArgs, safeJoin, tickOfSnapshots } from './sim-lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const ECOSIM = resolve(HERE, '../../ecosim');
const EXE = process.platform === 'win32' ? '.exe' : '';

/**
 * The binary this helper runs. It is the simulator's own release build, started as a command exactly as a
 * hand run starts it; nothing is linked, shared or copied out of `ecosim/` (DECISIONS.md, shot E4).
 * `ECOSIM_BIN` and `ECOSIM_PARAMS` override both paths, so the helper does not depend on the layout.
 */
export const BIN = process.env.ECOSIM_BIN ?? join(ECOSIM, 'target', 'release', `ecosim${EXE}`);
export const PARAMS = process.env.ECOSIM_PARAMS ?? join(ECOSIM, 'params.toml');
export const PORT = Number(process.env.ECOVIEW_SIM_PORT ?? 4174);
/** Runs kept on disk at once; a 1000-tick Capitol run is about 60 MB, so a fourth run drops the oldest. */
export const KEEP_RUNS = 3;

const json = (res, code, body) => {
  const text = JSON.stringify(body);
  res.writeHead(code, { 'content-type': 'application/json', 'content-length': Buffer.byteLength(text) });
  res.end(text);
};

async function readBody(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > MAX_BODY) throw new Error(`body over ${MAX_BODY} bytes`);
    chunks.push(chunk);
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8'));
}

/** One run: where it lives, how it is going, and the child process while it is alive. */
const runs = new Map();
let counter = 0;
let root = null;

async function runsRoot() {
  if (!root) root = await mkdtemp(join(tmpdir(), 'ecoview-sim-'));
  return root;
}

/** Everything this helper wrote goes when it exits: the runs are scratch, and they are large. */
async function cleanup() {
  for (const rec of runs.values()) rec.child?.kill();
  if (root) await rm(root, { recursive: true, force: true }).catch(() => {});
  root = null;
}

async function prune() {
  const old = [...runs.values()].filter((r) => r.state !== 'running').sort((a, b) => a.at - b.at);
  while (runs.size > KEEP_RUNS && old.length > 0) {
    const rec = old.shift();
    runs.delete(rec.id);
    await rm(rec.dir, { recursive: true, force: true }).catch(() => {});
  }
}

async function start(body) {
  const { ticks, seed, every, files } = checkRunRequest(body);
  if (!existsSync(BIN)) {
    const err = new Error(`no ecosim binary at ${BIN} — build it with "cargo build --release" in ecosim/`);
    err.missing = 'binary';
    throw err;
  }
  const id = `r${Date.now().toString(36)}${counter++}`;
  const dir = join(await runsRoot(), id);
  const world = join(dir, 'world');
  const out = join(dir, 'run');
  await mkdir(world, { recursive: true });
  for (const f of files) await writeFile(join(world, f.name), f.bytes);
  const args = runArgs(world, out, { seed, ticks, every, params: existsSync(PARAMS) ? PARAMS : null });
  // The child's working directory is the run's own, so a relative path inside the simulator could only ever
  // write inside the scratch directory this helper made.
  const child = spawn(BIN, args, { cwd: dir, windowsHide: true });
  const rec = { id, dir, out, ticks, every, state: 'running', at: Date.now(), child, log: '', error: null };
  runs.set(id, rec);
  for (const stream of [child.stdout, child.stderr]) {
    stream.on('data', (b) => {
      rec.log = (rec.log + b.toString()).slice(-4000);
    });
  }
  child.on('error', (e) => {
    rec.state = 'failed';
    rec.error = e.message;
  });
  child.on('close', (code, signal) => {
    rec.child = null;
    if (rec.state === 'cancelled') return;
    rec.state = code === 0 ? 'done' : 'failed';
    if (code !== 0) rec.error = `ecosim exited ${code ?? signal}: ${rec.log.trim().split('\n').pop() ?? ''}`;
  });
  await prune();
  return { id, ticks, every, path: `sim/runs/${id}` };
}

/** Progress comes from the snapshots on disk, which the simulator writes as the run goes. */
async function status(rec) {
  const names = await readdir(rec.out).catch(() => []);
  return {
    id: rec.id,
    state: rec.state,
    tick: tickOfSnapshots(names),
    ticks: rec.ticks,
    path: rec.state === 'done' ? `sim/runs/${rec.id}` : null,
    error: rec.error,
  };
}

async function serveFile(res, rec, rest) {
  const file = safeJoin(rec.out, rest);
  if (!file) return json(res, 400, { ok: false, error: 'path outside the run directory' });
  const info = await stat(file).catch(() => null);
  if (!info || !info.isFile()) return json(res, 404, { ok: false, error: 'no such file' });
  const bytes = await readFile(file);
  res.writeHead(200, { 'content-type': contentType(file), 'content-length': bytes.length });
  return res.end(bytes);
}

export function handler(req, res) {
  const url = new URL(req.url, 'http://127.0.0.1');
  const path = url.pathname;
  const send = (p) => p.catch((e) => json(res, e.missing ? 503 : 400, { ok: false, error: e.message }));
  if (path === '/sim/health') {
    // `root` is here so a test can sweep up after a helper it killed; nothing in the page reads it.
    return json(res, 200, { ok: true, binary: existsSync(BIN) ? BIN : null, params: PARAMS, port: PORT, root });
  }
  if (path === '/sim/run' && req.method === 'POST') {
    return send(readBody(req).then(start).then((r) => json(res, 200, { ok: true, ...r })));
  }
  if (path === '/sim/status') {
    const rec = runs.get(url.searchParams.get('id') ?? '');
    if (!rec) return json(res, 404, { ok: false, error: 'no such run' });
    return send(status(rec).then((s) => json(res, 200, { ok: true, ...s })));
  }
  if (path === '/sim/cancel' && req.method === 'POST') {
    const rec = runs.get(url.searchParams.get('id') ?? '');
    if (!rec) return json(res, 404, { ok: false, error: 'no such run' });
    if (rec.state === 'running') {
      rec.state = 'cancelled';
      rec.child?.kill();
    }
    return json(res, 200, { ok: true, id: rec.id, state: rec.state });
  }
  // The run id is looked up in the table this helper filled, never joined into a path, and what follows it
  // goes through safeJoin. Nothing in a request can name a file outside the temporary directory.
  const m = /^\/sim\/runs\/([^/]+)\/(.*)$/.exec(path);
  if (m && req.method === 'GET') {
    const rec = runs.get(m[1]);
    if (!rec) return json(res, 404, { ok: false, error: 'no such run' });
    return send(serveFile(res, rec, m[2]));
  }
  return json(res, 404, { ok: false, error: `no route for ${req.method} ${path}` });
}

/** Loopback only: this runs a program on the machine, so it must not be reachable from the network. */
export function listen(port = PORT) {
  const server = createServer(handler);
  return new Promise((ok, fail) => {
    server.once('error', fail);
    server.listen(port, '127.0.0.1', () => ok(server));
  });
}

/** True when this file is the program, not an import: `pathToFileURL` spells a Windows path the same way. */
export const isMain = (argv1) => argv1 !== undefined && pathToFileURL(resolve(argv1)).href === import.meta.url;

if (isMain(process.argv[1])) {
  const server = await listen();
  console.log(`ecoview sim helper on http://127.0.0.1:${server.address().port}`);
  console.log(`  ecosim binary ${existsSync(BIN) ? BIN : `MISSING (${BIN})`}`);
  console.log(`  runs under ${await runsRoot()}, removed when this exits`);
  for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    process.on(sig, () => {
      cleanup().finally(() => process.exit(0));
    });
  }
  process.on('exit', () => {
    for (const rec of runs.values()) rec.child?.kill();
  });
}
