import { describe, expect, it } from 'vitest';
// @ts-expect-error plain .mjs shared with scripts/shot.mjs and scripts/shot-ref.mjs
import { BUDGET_S, GATED, SHOTS, SHOWN } from '../../scripts/shots.mjs';

type Shot = [file: string, params: string, gated: boolean];
const shots = SHOTS as Shot[];

/**
 * The gate/show split of shot E6, written out so a later edit to scripts/shots.mjs has to mean it.
 * Gated: the renderer's own work — terrain and buildings at tick 0, the iso material scene, the
 * chart, the editor on the committed bundle. Shown: pictures of the simulation, which move whenever
 * ecosim's ecology moves.
 */
const EXPECTED_GATED = [
  '01_material_t0_iso.png',
  '02_material_t10000_iso.png',
  '08_chart_t20000.png',
  '12_capitol_medium_t0_iso.png',
  '13_capitol_medium_t0_top.png',
  '14_capitol_light_t0_top.png',
  '16_edit_hotbar.png',
  '17_edit_brush5.png',
];

describe('the screenshot table', () => {
  it('names a file, its URL parameters and whether it is gated, for all seventeen', () => {
    expect(shots).toHaveLength(17);
    for (const [file, params, gated] of shots) {
      expect(file).toMatch(/^\d\d_[a-z0-9_]+\.png$/);
      expect(params).toMatch(/^(run|world)=/);
      expect(typeof gated).toBe('boolean');
    }
    expect(GATED).toBe(true);
    expect(SHOWN).toBe(false);
    expect(BUDGET_S).toBe(60);
  });

  it('gates exactly the eight that picture the renderer', () => {
    expect(shots.filter(([, , g]) => g).map(([f]) => f)).toEqual(EXPECTED_GATED);
  });

  it('shows the nine that picture the simulation, and every one of them is a run at a nonzero tick', () => {
    const shown = shots.filter(([, , g]) => !g);
    expect(shown).toHaveLength(9);
    for (const [file, params] of shown) {
      const tick = Number(new URLSearchParams(params).get('tick'));
      expect(tick, `${file} is ungated, so it must be a picture of a simulation that has run`).toBeGreaterThan(0);
      expect(params, `${file}`).toMatch(/^run=runs\//);
    }
  });

  it('gates the frame the film check compares against', () => {
    // scripts/film-check.mjs crops frame 100 of the s42 film to #view and measures it against
    // shots/reference/02. Ungating 02 would leave that comparison with an unguarded reference.
    expect(shots.find(([f]) => f.startsWith('02_'))?.[2]).toBe(true);
  });
});
