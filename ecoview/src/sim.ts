// The editor's side of the sim helper (shot E4): posts the bundle on screen to `scripts/sim-server.mjs`,
// watches the run, and hands back the path the finished run directory is served at. Everything here is
// dev-only — with no helper listening the page says so and nothing else changes.
/** Where `vite.config.ts` proxies the helper. The built site has no such route, which reads as "absent". */
export const SIM_BASE = '/sim';
export const POLL_MS = 400;
export const DEFAULT_TICKS = 1000;
export const DEFAULT_SEED = 42;
/** The helper's own cap (`scripts/sim-lib.mjs`), repeated so the page can say no before sending 3 MB. */
export const MAX_TICKS = 20000;

export type SimState = 'idle' | 'absent' | 'starting' | 'running' | 'done' | 'failed' | 'cancelled';

export interface SimView {
  state: SimState;
  message: string;
  tick: number;
  ticks: number;
  /** The run directory's served path once the run is done, e.g. `sim/runs/r5k3x0`. */
  path: string | null;
}

/** The one call this module makes; a test can pass its own instead of a network. */
export type SimFetch = (url: string, init?: RequestInit) => Promise<Response>;

declare global {
  interface Window {
    /** What the sidebar's sim line says, in fields: tests/e2e/sim.spec.ts waits on this. */
    __ecoviewSim?: SimView;
  }
}

export interface SimOptions {
  ticks: number;
  seed: number;
}

/** The seven bundle files by name, as the editor holds them: text for the four it never touches. */
export type SimFiles = Record<string, Uint8Array | string>;

const CHUNK = 0x8000;

/** base64 of a file's bytes; a string is encoded as UTF-8 first, so the bytes are the ones a save writes. */
export function toBase64(data: Uint8Array | string): string {
  const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
  let s = '';
  for (let i = 0; i < bytes.length; i += CHUNK) s += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  return btoa(s);
}

/** The sidebar's line: the state in words, with the progress or the reason after it. */
export function simMessage(v: SimView): string {
  switch (v.state) {
    case 'idle':
      return 'sim: press R to run the simulator on these edits';
    case 'starting':
      return 'sim: sending the edited bundle…';
    case 'running':
      return `sim: running, tick ${v.tick} / ${v.ticks} (C cancels)`;
    case 'done':
      return `sim: ${v.ticks} ticks done · ${v.path} (B goes back to the bundle)`;
    case 'cancelled':
      return 'sim: cancelled · press R to run again';
    default:
      return `sim: ${v.message}`;
  }
}

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/**
 * One run at a time. `start` resolves when the run has finished, failed, been cancelled, or turned out to
 * have no helper to run on; the view is pushed to the caller at every step so the sidebar can follow it.
 */
export class SimRunner {
  view: SimView = { state: 'idle', message: '', tick: 0, ticks: 0, path: null };
  private id: string | null = null;

  constructor(
    private readonly onChange: (v: SimView) => void,
    private readonly base: string = SIM_BASE,
    private readonly fetcher: SimFetch = (u, init) => fetch(u, init),
  ) {}

  get busy(): boolean {
    return this.view.state === 'starting' || this.view.state === 'running';
  }

  private set(v: Partial<SimView>): void {
    this.view = { ...this.view, ...v };
    this.onChange(this.view);
  }

  /**
   * The helper answers `/sim/health` with its own state. When it is not listening the preview server's proxy
   * answers `{ ok: false }` itself, and a built site with no proxy answers 404 — both read as absent, and
   * neither puts an error in the console.
   */
  private async json(url: string, init?: RequestInit): Promise<Record<string, unknown>> {
    const res = await this.fetcher(url, init);
    if (!res.ok && res.status !== 503 && res.status !== 400) return { ok: false, error: `HTTP ${res.status}` };
    return (await res.json()) as Record<string, unknown>;
  }

  private async reach(url: string, init?: RequestInit): Promise<Record<string, unknown>> {
    try {
      return await this.json(url, init);
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) };
    }
  }

  async start(files: SimFiles, opts: SimOptions): Promise<SimView> {
    if (this.busy) return this.view;
    this.id = null;
    this.set({ state: 'starting', message: '', tick: 0, ticks: opts.ticks, path: null });
    const health = await this.reach(`${this.base}/health`);
    if (health.ok !== true) {
      return this.fail('absent', `no sim helper behind ${this.base} — start it with \`npm run sim\``);
    }
    if (!health.binary) {
      return this.fail('absent', 'the sim helper is running but has no ecosim binary'
        + ' — build it with `cargo build --release` in ecosim/');
    }
    const body = JSON.stringify({
      ticks: opts.ticks,
      seed: opts.seed,
      files: Object.fromEntries(Object.entries(files).map(([name, data]) => [name, toBase64(data)])),
    });
    const started = await this.reach(`${this.base}/run`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body,
    });
    if (started.ok !== true || typeof started.id !== 'string') {
      return this.fail('failed', String(started.error ?? 'the helper refused the bundle'));
    }
    this.id = started.id;
    this.set({ state: 'running', tick: 0 });
    return this.poll();
  }

  private async poll(): Promise<SimView> {
    const id = this.id;
    while (this.id === id && this.view.state === 'running') {
      await sleep(POLL_MS);
      if (this.id !== id) break;
      const s = await this.reach(`${this.base}/status?id=${encodeURIComponent(id!)}`);
      if (s.ok !== true) return this.fail('failed', String(s.error ?? 'the helper stopped answering'));
      if (s.state === 'running') this.set({ tick: Number(s.tick ?? 0) });
      else if (s.state === 'done') this.set({ state: 'done', tick: Number(s.tick ?? 0), path: String(s.path) });
      else if (s.state === 'cancelled') this.set({ state: 'cancelled' });
      else return this.fail('failed', String(s.error ?? 'the run failed'));
    }
    return this.view;
  }

  private fail(state: SimState, message: string): SimView {
    this.id = null;
    this.set({ state, message, path: null });
    return this.view;
  }

  /** Stops the run on the helper as well as in the page, so no orphan `ecosim` keeps writing snapshots. */
  async cancel(): Promise<void> {
    const id = this.id;
    if (!id || !this.busy) return;
    this.id = null;
    this.set({ state: 'cancelled' });
    await this.reach(`${this.base}/cancel?id=${encodeURIComponent(id)}`, { method: 'POST' });
  }
}
