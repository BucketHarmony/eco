import { defineConfig } from '@playwright/test';
// @ts-expect-error plain .mjs shared with scripts/shot.mjs
import { CHROMIUM_ARGS, VIEWPORT } from './scripts/chromium.mjs';

export default defineConfig({
  testDir: 'tests/e2e',
  timeout: 60_000,
  workers: 1,
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
