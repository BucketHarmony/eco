import { defineConfig } from '@playwright/test';
// @ts-expect-error plain .mjs shared with scripts/shot.mjs
import { CHROMIUM_ARGS, VIEWPORT } from './scripts/chromium.mjs';

export default defineConfig({
  testDir: 'tests/e2e',
  timeout: 60_000,
  workers: 1,
  // One retry on CI, none locally (shot C2). GitHub's hosted runners vary by about 1.65x in speed --
  // measured test by test across two runs of the same suite, where every one of the 49 tests came in
  // 1.42-2.00x slower on the slow runner -- and `edit.spec.ts:331` measured 168 s of its own 180 s
  // `describe.configure` budget there. A runner 7% slower than that one reddens a shot that changed
  // nothing. The retry converts that into a slower green run and Playwright reports it as flaky, so
  // the drift stays visible. It is a mitigation, not the fix: the fix is that test's budget, which
  // lives in a frozen component (.github/DECISIONS.md, shot C2).
  // `workers` stays 1 for the same measurement: a second worker competes for the few cores
  // SwiftShader rasterises on, which slows every test, and that test has 7% of headroom to give.
  retries: process.env.CI ? 1 : 0,
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:4173',
    viewport: VIEWPORT,
    deviceScaleFactor: 1,
    launchOptions: { args: CHROMIUM_ARGS },
  },
  webServer: {
    command: 'npx vite preview --port 4173 --strictPort',
    url: 'http://localhost:4173',
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
