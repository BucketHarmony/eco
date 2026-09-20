// Serves dist/ with Vite's preview() API and writes the reference screenshots to shots/.
// Assumes `npm run build` has already run.
import { mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { preview } from 'vite';
import { chromium } from '@playwright/test';
import { CHROMIUM_ARGS, VIEWPORT } from './chromium.mjs';
import { BUDGET_S, SHOTS } from './shots.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outDir = path.join(root, 'shots');
const PORT = 4174;

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
    const start = Date.now();
    await page.goto(`http://localhost:${PORT}/?${params}`);
    await page.waitForFunction(() => window.__ecoviewReady === true || !!window.__ecoviewError, null, {
      timeout: 60000,
    });
    const err = await page.evaluate(() => window.__ecoviewError);
    if (err) throw new Error(`${file}: ${err}`);
    await page.screenshot({ path: path.join(outDir, file), fullPage: true });
    console.log(`wrote shots/${file} (${((Date.now() - start) / 1000).toFixed(1)} s)`);
  }
} catch (e) {
  failed = true;
  console.error(e);
} finally {
  await browser?.close();
  await new Promise((r) => server.httpServer.close(r));
}
const elapsed = (Date.now() - t0) / 1000;
const inBudget = elapsed <= BUDGET_S;
if (!inBudget) failed = true;
console.log(`${inBudget ? 'ok  ' : 'FAIL'} ${SHOTS.length} screenshots in ${elapsed.toFixed(1)} s of ${BUDGET_S} s`);
process.exit(failed ? 1 : 0);
