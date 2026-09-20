import { defineConfig, type ProxyOptions } from 'vite';

/**
 * The dev-only sim helper (shot E4). `scripts/sim-server.mjs` listens on loopback and the dev and preview
 * servers pass /sim through to it, so the page never leaves its own origin and `src/loader.ts` can read a
 * finished run at /sim/runs/<id>/ like any other run directory. Nothing of this reaches `vite build`: the
 * built site is static and has no /sim route at all.
 *
 * When the helper is not listening the proxy answers the request itself, with the same shape the helper
 * uses for a refusal. Vite's own handler would reply 502, which the browser logs as a console error on a
 * page that is working exactly as intended; this way the page hears "absent" and stays quiet.
 */
const SIM_TARGET = `http://127.0.0.1:${process.env.ECOVIEW_SIM_PORT ?? 4174}`;

const sim: ProxyOptions = {
  target: SIM_TARGET,
  configure: (proxy) => {
    proxy.on('error', (err, _req, res) => {
      if ('writeHead' in res && !res.headersSent && !res.writableEnded) {
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ ok: false, error: `no sim helper on ${SIM_TARGET}: ${err.message}` }));
      } else {
        res.end();
      }
    });
  },
};

export default defineConfig({
  build: { chunkSizeWarningLimit: 1000 },
  server: { proxy: { '/sim': sim } },
  preview: { port: 4173, strictPort: true, proxy: { '/sim': sim } },
});
