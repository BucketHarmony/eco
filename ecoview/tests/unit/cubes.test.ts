// Shot E3: the ground and buildings are drawn as cubes on a lattice, and only the visible ones exist.
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { loadBundle, type Fetcher, type WorldData } from '../../src/loader';
import { CHUNK, GroundChunks, cubeRange, groundLevelAt, levelOf, topLevelAt } from '../../src/world';

const PUBLIC = path.resolve(__dirname, '../../public');
const fsFetcher: Fetcher = async (url) => {
  try {
    return new Response(await readFile(path.join(PUBLIC, url)), { status: 200 });
  } catch {
    return new Response('not found', { status: 404 });
  }
};

/** A grid with the given ground heights in metres, one medium, no buildings. */
function grid(gw: number, gd: number, h: (gx: number, gy: number) => number, b = () => 0): WorldData {
  const ground_h = new Float32Array(gw * gd);
  const building_h = new Float32Array(gw * gd);
  for (let gy = 0; gy < gd; gy++) {
    for (let gx = 0; gx < gw; gx++) {
      ground_h[gx + gw * gy] = h(gx, gy);
      building_h[gx + gw * gy] = b();
    }
  }
  return {
    meta: { name: 'syn', ground_cell_m: 0.5, ground_width: gw, ground_depth: gd, media: ['soil', 'lawn'] },
    gw, gd, cell: 0.5, ground_h, medium: new Uint8Array(gw * gd).fill(1), building_h, pipes: [],
  };
}

describe('the cube lattice', () => {
  it('rounds a height down to the cube whose top face it is', () => {
    expect(levelOf(0, 0.5)).toBe(0);
    expect(levelOf(0.49, 0.5)).toBe(0);
    expect(levelOf(0.5, 0.5)).toBe(1); // a height exactly on the lattice belongs to the cube below it
    expect(levelOf(1.0, 0.5)).toBe(2);
    expect(levelOf(1.03, 0.5)).toBe(2);
    expect(levelOf(2.5, 0.25)).toBe(10);
  });

  it('draws a column down to its lowest neighbour, and no further', () => {
    // A staircase: each row is one cube higher than the row south of it.
    const w = grid(4, 4, (_gx, gy) => gy * 0.5);
    expect(topLevelAt(w, 1 + 4 * 2)).toBe(2);
    expect(groundLevelAt(w, 1 + 4 * 2)).toBe(2);
    // Row 2 stands one cube proud of row 1, so it draws its top cube and the one below it: two.
    expect(cubeRange(w, 1, 2)).toEqual({ g0: 1, g1: 2, b0: 2, b1: 2 });
    // Off the grid is a column below the floor, so the rim is a wall that reaches level 0.
    expect(cubeRange(w, 1, 0)).toEqual({ g0: -1, g1: 0, b0: 0, b1: 0 }); // one cube, the floor one
    // A pit: the cells around it are drawn down to the cube above its floor, which its own top hides.
    const pit = grid(5, 5, (gx, gy) => (gx === 2 && gy === 2 ? 0 : 1));
    expect(cubeRange(pit, 2, 1)).toEqual({ g0: 0, g1: 2, b0: 2, b1: 2 });
    expect(cubeRange(pit, 2, 2)).toEqual({ g0: -1, g1: 0, b0: 0, b1: 0 }); // the pit floor: one cube
    // A building stacks its own cubes on top of the ground's, never below them.
    const block = grid(3, 3, () => 1, () => 1.5);
    expect(cubeRange(block, 1, 1)).toEqual({ g0: 1, g1: 2, b0: 4, b1: 5 }); // inside the block: a roof
    expect(cubeRange(block, 0, 1)).toEqual({ g0: -1, g1: 2, b0: 2, b1: 5 }); // at its edge: a wall too
  });

  it('counts the Capitol bundle exactly, ground and buildings apart', async () => {
    const b = await loadBundle('fixtures/capitol-world', fsFetcher);
    const w = b.world;
    expect([w.gw, w.gd]).toEqual([512, 512]);
    const chunks = new GroundChunks(w, 256);
    expect([chunks.cx, chunks.cy]).toEqual([16, 16]);
    expect(chunks.cx * chunks.cy * CHUNK * CHUNK).toBe(512 * 512);
    // The whole scene, measured on this bundle (ecoview/DECISIONS.md, shot E3).
    expect(chunks.instances).toEqual([278286, 87791]);
    expect(chunks.instances[0] + chunks.instances[1]).toBe(366077);
  });

  it('is one cube a column on ground with no relief at all', () => {
    const flat = new GroundChunks(grid(40, 40, () => 0), 20);
    expect(flat.instances).toEqual([40 * 40, 0]);
    // Lift that ground two cubes and only the rim grows, by the wall it now needs down to the floor.
    const raised = new GroundChunks(grid(40, 40, () => 1), 20);
    expect(raised.instances).toEqual([38 * 38 + (40 * 40 - 38 * 38) * 3, 0]);
  });
});
