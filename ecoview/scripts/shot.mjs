// Serves dist/ with Vite's preview() API and writes the 8 reference screenshots to shots/.
// Assumes `npm run build` has already run.
import { mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { preview } from 'vite';
import { chromium } from '@playwright/test';
import { CHROMIUM_ARGS, VIEWPORT } from './chromium.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outDir = path.join(root, 'shots');
const PORT = 4174;

export const SHOTS = [
  ['01_material_t0_iso.png', 'tick=0&overlay=material&cam=iso'],
  ['02_material_t10000_iso.png', 'tick=10000&overlay=material&cam=iso'],
  ['03_light_t10000_top.png', 'tick=10000&overlay=light&cam=top'],
  ['04_moisture_t10000_top.png', 'tick=10000&overlay=moisture&cam=top'],
  ['05_fertility_t10000_top.png', 'tick=10000&overlay=fertility&cam=top'],
  ['06_temperature_t1000_top.png', 'tick=1000&overlay=temperature&cam=top'],
  ['07_temperature_t3000_top.png', 'tick=3000&overlay=temperature&cam=top'],
  ['08_chart_t20000.png', 'tick=20000&overlay=material&cam=iso'],
];

const t0 = Date.now();
await mkdir(outDir, { recursive: true });
const server = await preview({ root, logLevel: 'warn', preview: { port: PORT, strictPort: true } });
let browser;
let failed = false;
try {
  browser = await chromium.launch({ args: CHROMIUM_ARGS });
  const page = await browser.newPage({ viewport: VIEWPORT, deviceScaleFactor: 1 });
  page.on('console', (m) => {
    if (m.type() === 'error') {
      failed = true;
      console.error(`console error: ${m.text()}`);
    }
  });
  page.on('pageerror', (e) => {
    failed = true;
    console.error(`page error: ${e.message}`);
  });
  for (const [file, params] of SHOTS) {
    await page.goto(`http://localhost:${PORT}/?run=runs/s42&${params}`);
    await page.waitForFunction(() => window.__ecoviewReady === true || !!window.__ecoviewError, null, {
      timeout: 30000,
    });
    const err = await page.evaluate(() => window.__ecoviewError);
    if (err) throw new Error(`${file}: ${err}`);
    await page.screenshot({ path: path.join(outDir, file), fullPage: true });
    console.log(`wrote shots/${file}`);
  }
} catch (e) {
  failed = true;
  console.error(e);
} finally {
  await browser?.close();
  await new Promise((r) => server.httpServer.close(r));
}
console.log(`${SHOTS.length} screenshots in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
process.exit(failed ? 1 : 0);
