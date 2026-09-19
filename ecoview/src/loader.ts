// Fetches and parses a run directory (see SAD 1 "Run directory format").

export const AIR = 0;
export const SOIL = 1;
export const ROCK = 2;
export const WATER = 3;

export interface Species {
  id: number;
  name: string;
  kind: string;
  color: string;
  canopy_color?: string;
}

/**
 * Format versions this reader accepts. Version 2 adds `state.bin` (ignored here) and `forked_from`; version 3
 * adds `events.csv`.
 */
export const FORMAT_VERSIONS = [1, 2, 3];

export interface ForkedFrom {
  run: string;
  tick: number;
}

/** `meta.json` `dims`: x = width, y = depth, z = height, patch = patch side in columns. */
export interface Dims {
  x: number;
  y: number;
  z: number;
  /** Absent before ecosim shot 15, when every patch was 8×8. */
  patch?: number;
}

/** A world's sizes and index helpers, derived once from `Dims`. */
export class Grid {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly patch: number;
  /** Patches along x and along y. */
  readonly px: number;
  readonly py: number;
  readonly voxels: number;
  readonly columns: number;
  readonly patches: number;

  constructor(d: Dims) {
    this.x = d.x;
    this.y = d.y;
    this.z = d.z;
    this.patch = d.patch ?? 8;
    this.px = this.x / this.patch;
    this.py = this.y / this.patch;
    this.voxels = this.x * this.y * this.z;
    this.columns = this.x * this.y;
    this.patches = this.px * this.py;
  }

  /** The longer horizontal side, which the cameras scale with. */
  get longest(): number {
    return Math.max(this.x, this.y);
  }

  voxel(x: number, y: number, z: number): number {
    return x + this.x * (y + this.y * z);
  }

  column(x: number, y: number): number {
    return x + this.x * y;
  }

  patchOf(x: number, y: number): number {
    return Math.floor(x / this.patch) + this.px * Math.floor(y / this.patch);
  }
}

export interface Meta {
  format_version: number;
  dims: Dims;
  seed: number;
  ticks: number;
  snapshot_every: number;
  year_len: number;
  water_level: number;
  snapshots: number[];
  species: Species[];
  /** Version 2 only: the run a fork continues, or null for a run started at tick 0. */
  forked_from?: ForkedFrom | null;
}

export const SERIES_COLUMNS = [
  'tick', 'grazers', 'hunters', 'trees', 'grass_mean', 'shrub_mean',
  'moisture_mean', 'fertility_mean', 'detritus_total', 'temperature',
] as const;
export type SeriesColumn = (typeof SERIES_COLUMNS)[number];

/** Death causes in the sim's `Cause` order; each is an optional `<species>_<cause>` column, deaths that tick. */
export const DEATH_CAUSES = ['starved', 'eaten', 'old_age', 'crowded', 'burnt'] as const;
export type DeathCause = (typeof DEATH_CAUSES)[number];
export const DEATH_SPECIES = ['grazer', 'hunter'] as const;

/**
 * Columns later sim versions append (fire in ecosim shot 9, trait means and SDs in shot 11). Each is read
 * when present and left undefined when not, so older runs still load.
 */
export const OPTIONAL_COLUMNS = [
  'patches_burning', 'total_burnt',
  'grazer_energy_cost_mult_mean', 'grazer_energy_cost_mult_sd',
  'grazer_flee_distance_mean', 'grazer_flee_distance_sd',
  'grazer_repro_threshold_mean', 'grazer_repro_threshold_sd',
  'hunter_energy_cost_mult_mean', 'hunter_energy_cost_mult_sd',
  'hunter_flee_distance_mean', 'hunter_flee_distance_sd',
  'hunter_repro_threshold_mean', 'hunter_repro_threshold_sd',
] as const;
export type OptionalColumn = (typeof OPTIONAL_COLUMNS)[number];

export type Series = Record<SeriesColumn, Float64Array> & {
  /** Deaths per tick by cause, summed over the species that have the column. Absent causes are omitted. */
  deaths: Partial<Record<DeathCause, Float64Array>>;
  /** Optional columns present in the file. */
  extra: Partial<Record<OptionalColumn, Float64Array>>;
};

/** One `events.csv` row (format 3). Empty numeric fields are `null`, empty text fields `''`. */
export interface RunEvent {
  tick: number;
  kind: string;
  species: string;
  patch_x: number | null;
  patch_y: number | null;
  x: number | null;
  y: number | null;
  cause: string;
  detail: number | null;
}

export interface Patch {
  grass: number;
  shrub: number;
  detritus: number;
  temperature: number;
  /** Fire (sim shot 9): ticks until the patch burns out, 0 when not burning. Absent in older runs. */
  burning_ticks_left?: number;
}

export type Stage = 'sapling' | 'young' | 'mature';

export interface TreeEntity {
  id: number;
  kind: 'tree';
  x: number;
  y: number;
  z: number;
  age: number;
  stage: Stage;
  lifespan?: number;
}

export interface AnimalEntity {
  id: number;
  kind: 'grazer' | 'hunter';
  x: number;
  y: number;
  z: number;
  energy: number;
  age: number;
  state: string;
  /** Heritable traits (sim shot 11). Absent in older runs, where every animal has the species defaults. */
  energy_cost_mult?: number;
  flee_distance?: number;
  repro_threshold?: number;
}

export type Entity = TreeEntity | AnimalEntity;

export interface Snapshot {
  tick: number;
  grid: Grid;
  material: Uint8Array;
  light: Uint8Array;
  moisture: Uint8Array;
  fertility: Uint8Array;
  height: Uint8Array;
  patches: Patch[];
  entities: Entity[];
  /** 1 for each patch with a `burnout` event since the previous snapshot; all 0 for a run without `events.csv`. */
  burnt: Uint8Array;
}

export interface Run {
  base: string;
  meta: Meta;
  grid: Grid;
  series: Series;
  /** `events.csv` rows in file order; empty for formats 1 and 2. */
  events: RunEvent[];
  /** The `burnout` rows of `events`. */
  burnouts: RunEvent[];
}

/** Fetches a path relative to the run directory; tests swap in a filesystem reader. */
export type Fetcher = (url: string) => Promise<Response>;

export const snapDir = (tick: number): string => `snap_${String(tick).padStart(6, '0')}`;

async function get(fetcher: Fetcher, url: string): Promise<Response> {
  const res = await fetcher(url);
  if (!res.ok) throw new Error(`fetch ${url}: HTTP ${res.status}`);
  return res;
}

async function getBin(fetcher: Fetcher, url: string, len: number): Promise<Uint8Array> {
  const buf = new Uint8Array(await (await get(fetcher, url)).arrayBuffer());
  if (buf.length !== len) throw new Error(`${url}: expected ${len} bytes, got ${buf.length}`);
  return buf;
}

/** The sim's limits (ecosim `Params::check_dims`): sides in 1..=256, width and depth whole patches. */
function validDims(d: Dims | undefined): d is Dims {
  const side = (v: unknown) => Number.isInteger(v) && (v as number) >= 1 && (v as number) <= 256;
  if (!d || typeof d !== 'object') return false;
  const patch = d.patch ?? 8;
  return side(d.x) && side(d.y) && side(d.z) && side(patch) && d.x % patch === 0 && d.y % patch === 0;
}

export function parseMeta(raw: unknown): Meta {
  const m = raw as Meta;
  if (!m || typeof m !== 'object') throw new Error('meta.json: not an object');
  if (!FORMAT_VERSIONS.includes(m.format_version)) {
    throw new Error(`unsupported format_version ${String(m.format_version)} (expected ${FORMAT_VERSIONS.join(', ')})`);
  }
  if (!validDims(m.dims)) throw new Error(`meta.json: unsupported dims ${JSON.stringify(m.dims)}`);
  if (!Array.isArray(m.snapshots) || m.snapshots.length === 0) {
    throw new Error('meta.json: no snapshots');
  }
  if (!Array.isArray(m.species)) throw new Error('meta.json: no species list');
  return m;
}

/** Reads columns by header name, so column order and unknown extra columns don't matter. */
export function parseSeries(text: string): Series {
  const lines = text.split(/\r?\n/).filter((l) => l.length > 0);
  const header = lines[0].split(',');
  const idx = SERIES_COLUMNS.map((c) => {
    const i = header.indexOf(c);
    if (i < 0) throw new Error(`series.csv: missing column ${c}`);
    return i;
  });
  const n = lines.length - 1;
  const out = { deaths: {}, extra: {} } as Series;
  for (const c of SERIES_COLUMNS) out[c] = new Float64Array(n);
  const deathCols: [Float64Array, number][] = [];
  for (const cause of DEATH_CAUSES) {
    for (const sp of DEATH_SPECIES) {
      const i = header.indexOf(`${sp}_${cause}`);
      if (i < 0) continue;
      const col = (out.deaths[cause] ??= new Float64Array(n));
      deathCols.push([col, i]);
    }
  }
  const extraCols: [Float64Array, number][] = [];
  for (const c of OPTIONAL_COLUMNS) {
    const i = header.indexOf(c);
    if (i >= 0) extraCols.push([(out.extra[c] = new Float64Array(n)), i]);
  }
  for (let r = 0; r < n; r++) {
    const cells = lines[r + 1].split(',');
    for (let k = 0; k < SERIES_COLUMNS.length; k++) {
      out[SERIES_COLUMNS[k]][r] = Number(cells[idx[k]]);
    }
    for (const [col, i] of deathCols) col[r] += Number(cells[i]);
    for (const [col, i] of extraCols) col[r] = Number(cells[i]);
  }
  return out;
}

const EVENT_COLUMNS = ['tick', 'kind', 'species', 'patch_x', 'patch_y', 'x', 'y', 'cause', 'detail'] as const;

/** Parses `events.csv` by header name, like `series.csv`. */
export function parseEvents(text: string): RunEvent[] {
  const lines = text.split(/\r?\n/).filter((l) => l.length > 0);
  if (lines.length === 0) throw new Error('events.csv: no header');
  const header = lines[0].split(',');
  const idx = EVENT_COLUMNS.map((c) => {
    const i = header.indexOf(c);
    if (i < 0) throw new Error(`events.csv: missing column ${c}`);
    return i;
  });
  const num = (v: string | undefined): number | null => (v === undefined || v === '' ? null : Number(v));
  const out: RunEvent[] = new Array(lines.length - 1);
  for (let r = 1; r < lines.length; r++) {
    const c = lines[r].split(',');
    out[r - 1] = {
      tick: Number(c[idx[0]]),
      kind: c[idx[1]] ?? '',
      species: c[idx[2]] ?? '',
      patch_x: num(c[idx[3]]),
      patch_y: num(c[idx[4]]),
      x: num(c[idx[5]]),
      y: num(c[idx[6]]),
      cause: c[idx[7]] ?? '',
      detail: num(c[idx[8]]),
    };
  }
  return out;
}

/**
 * Patches with a `burnout` event in ticks (from, to]. For the snapshot at `to`, `from` is the previous
 * snapshot's tick, or -1 for the first snapshot. These are the patches the fire overlay draws burnt.
 */
export function burntPatches(burnouts: RunEvent[], grid: Grid, from: number, to: number): Uint8Array {
  const out = new Uint8Array(grid.patches);
  for (const e of burnouts) {
    if (e.tick <= from || e.tick > to || e.patch_x === null || e.patch_y === null) continue;
    if (e.patch_x >= 0 && e.patch_x < grid.px && e.patch_y >= 0 && e.patch_y < grid.py) {
      out[e.patch_x + grid.px * e.patch_y] = 1;
    }
  }
  return out;
}

export function joinUrl(base: string, path: string): string {
  return base.endsWith('/') ? base + path : `${base}/${path}`;
}

export async function loadRun(base: string, fetcher: Fetcher = fetch): Promise<Run> {
  const meta = parseMeta(await (await get(fetcher, joinUrl(base, 'meta.json'))).json());
  const series = parseSeries(await (await get(fetcher, joinUrl(base, 'series.csv'))).text());
  const events = meta.format_version >= 3
    ? parseEvents(await (await get(fetcher, joinUrl(base, 'events.csv'))).text())
    : [];
  const burnouts = events.filter((e) => e.kind === 'burnout');
  return { base, meta, grid: new Grid(meta.dims), series, events, burnouts };
}

/** The snapshot with the largest tick <= `tick`, clamped to the first and last snapshot. */
export function pickSnapshot(snapshots: number[], tick: number): number {
  let best = snapshots[0];
  for (const s of snapshots) if (s <= tick && s > best) best = s;
  return best;
}

export async function loadSnapshot(run: Run, tick: number, fetcher: Fetcher = fetch): Promise<Snapshot> {
  const dir = joinUrl(run.base, snapDir(tick));
  const u = (f: string) => joinUrl(dir, f);
  const grid = run.grid;
  const [material, light, moisture, fertility, height, patchesRes, entitiesRes] = await Promise.all([
    getBin(fetcher, u('material.bin'), grid.voxels),
    getBin(fetcher, u('light.bin'), grid.voxels),
    getBin(fetcher, u('moisture.bin'), grid.columns),
    getBin(fetcher, u('fertility.bin'), grid.columns),
    getBin(fetcher, u('height.bin'), grid.columns),
    get(fetcher, u('patches.json')),
    get(fetcher, u('entities.json')),
  ]);
  const patches = (await patchesRes.json()) as Patch[];
  if (!Array.isArray(patches) || patches.length !== grid.patches) {
    throw new Error(`${u('patches.json')}: expected ${grid.patches} patches`);
  }
  const entities = (await entitiesRes.json()) as Entity[];
  if (!Array.isArray(entities)) throw new Error(`${u('entities.json')}: not an array`);
  const snaps = run.meta.snapshots;
  const i = snaps.indexOf(tick);
  const burnt = burntPatches(run.burnouts, grid, i > 0 ? snaps[i - 1] : -1, tick);
  return { tick, grid, material, light, moisture, fertility, height, patches, entities, burnt };
}

export function speciesColor(meta: Meta, name: string, key: 'color' | 'canopy_color' = 'color'): string {
  const s = meta.species.find((sp) => sp.name === name);
  const c = s?.[key];
  if (!c) throw new Error(`meta.json: species ${name} has no ${key}`);
  return c;
}
