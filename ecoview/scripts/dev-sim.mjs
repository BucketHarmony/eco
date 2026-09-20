// `npm run preview:sim` (shot E4): the preview server and the sim helper together, so one command gives a
// page that can run the simulator on what it is editing. Both are children of this process; when one stops
// the other is stopped too, so no helper is left holding port 4174 after a Ctrl-C.
//
// It spawns node directly rather than npx, which needs a shell on Windows and would leave orphans behind.
import { spawn } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..');

const children = [];
let stopping = false;

function start(name, args) {
  const child = spawn(process.execPath, args, { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  for (const stream of [child.stdout, child.stderr]) {
    stream.setEncoding('utf8');
    stream.on('data', (text) => {
      for (const line of text.split('\n')) if (line.trim()) console.log(`[${name}] ${line}`);
    });
  }
  child.on('exit', (code, signal) => {
    if (stopping) return;
    console.log(`[${name}] exited ${code ?? signal}`);
    stop(code ?? 1);
  });
  children.push(child);
  return child;
}

function stop(code) {
  if (stopping) return;
  stopping = true;
  for (const child of children) child.kill();
  process.exitCode = code;
  setTimeout(() => process.exit(code), 500).unref();
}

start('sim', [join(HERE, 'sim-server.mjs')]);
start('preview', [join(ROOT, 'node_modules', 'vite', 'bin', 'vite.js'), 'preview', '--port', '4173', '--strictPort']);

for (const sig of ['SIGINT', 'SIGTERM']) process.on(sig, () => stop(0));
