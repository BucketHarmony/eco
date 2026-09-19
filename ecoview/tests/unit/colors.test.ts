import { describe, expect, it } from 'vitest';
import {
  burningColor, crowdingColor, fertilityColor, grazersPerPatch, hexToRgb, isBurnt, lightColor, moistureColor,
  soilColor, temperatureColor, traitColor,
} from '../../src/world';
import type { Entity, Snapshot } from '../../src/loader';
import { canopyVoxels } from '../../src/entities';
import { parseParams, toSearch } from '../../src/ui';

describe('fire, crowding and traits colors', () => {
  it('burning goes #b3300a at 1 tick left to #ffb020 at 3, clamped', () => {
    expect(burningColor(1)).toEqual(hexToRgb('#b3300a'));
    expect(burningColor(3)).toEqual(hexToRgb('#ffb020'));
    expect(burningColor(10)).toEqual(hexToRgb('#ffb020'));
  });
  it('a patch is burnt when not burning and nearly bare', () => {
    const p = { grass: 0.01, shrub: 0.02, detritus: 0, temperature: 10 };
    expect(isBurnt(p)).toBe(true);
    expect(isBurnt({ ...p, burning_ticks_left: 0 })).toBe(true);
    expect(isBurnt({ ...p, burning_ticks_left: 2 })).toBe(false);
    expect(isBurnt({ ...p, grass: 0.1 })).toBe(false);
  });
  it('crowding goes white at 0 to #d81b9c at 32, clamped', () => {
    expect(crowdingColor(0)).toEqual([255, 255, 255]);
    expect(crowdingColor(32)).toEqual(hexToRgb('#d81b9c'));
    expect(crowdingColor(100)).toEqual(hexToRgb('#d81b9c'));
  });
  it('traits are white at the default 1 or when absent, blue below and red above, full at 0.25 away', () => {
    expect(traitColor(1)).toEqual([255, 255, 255]);
    expect(traitColor()).toEqual([255, 255, 255]);
    expect(traitColor(0.75)).toEqual(hexToRgb('#1f5bff'));
    expect(traitColor(0.3)).toEqual(hexToRgb('#1f5bff'));
    expect(traitColor(1.25)).toEqual(hexToRgb('#ff1f1f'));
    const slight = traitColor(1.05);
    expect(slight[0]).toBe(255);
    expect(slight[2]).toBeLessThan(255);
  });
  it('counts live grazers per patch from float positions, ignoring hunters and trees', () => {
    const a = (kind: string, x: number, y: number) => ({ id: 0, kind, x, y, z: 12, energy: 1, age: 1, state: 'wander' });
    const entities = [a('grazer', 7.0, 0.0), a('grazer', 8.0, 0.0), a('grazer', 63.0, 63.0), a('hunter', 7, 0),
      { id: 9, kind: 'tree', x: 0, y: 0, z: 12, age: 1, stage: 'young' }] as Entity[];
    const n = grazersPerPatch({ entities } as Snapshot);
    expect(n.length).toBe(64);
    expect([n[0], n[1], n[63]]).toEqual([1, 1, 1]);
    expect(n.reduce((s, v) => s + v, 0)).toBe(3);
  });
});

describe('overlay colors', () => {
  it('light is gray v', () => {
    expect(lightColor(0)).toEqual([0, 0, 0]);
    expect(lightColor(255)).toEqual([255, 255, 255]);
    expect(lightColor(55)).toEqual([55, 55, 55]);
  });
  it('moisture goes white to #1f4fd1', () => {
    expect(moistureColor(0)).toEqual([255, 255, 255]);
    expect(moistureColor(255)).toEqual(hexToRgb('#1f4fd1'));
  });
  it('fertility goes white to #4a2c12', () => {
    expect(fertilityColor(0)).toEqual([255, 255, 255]);
    expect(fertilityColor(255)).toEqual(hexToRgb('#4a2c12'));
  });
  it('temperature goes #2040ff at 0 C to #ff3020 at 30 C, clamped', () => {
    expect(temperatureColor(0)).toEqual(hexToRgb('#2040ff'));
    expect(temperatureColor(30)).toEqual(hexToRgb('#ff3020'));
    expect(temperatureColor(-10)).toEqual(hexToRgb('#2040ff'));
    expect(temperatureColor(45)).toEqual(hexToRgb('#ff3020'));
  });
  it('material soil lerps toward grass then shrub', () => {
    expect(soilColor(0, 0)).toEqual(hexToRgb('#8b6b47'));
    expect(soilColor(1, 0)).toEqual(hexToRgb('#7cc242'));
    const full = soilColor(1, 1);
    const g = hexToRgb('#7cc242');
    const s = hexToRgb('#2f6b2a');
    expect(full).toEqual(g.map((c, i) => Math.round(c + (s[i] - c) * 0.8)));
  });
});

describe('canopy geometry', () => {
  it('follows the stage rules and clips to the world', () => {
    expect(canopyVoxels(5, 5, 12, 'sapling')).toEqual([]);
    expect(canopyVoxels(5, 5, 12, 'young')).toEqual([[5, 5, 13]]);
    expect(canopyVoxels(5, 5, 12, 'mature').length).toBe(18);
    expect(canopyVoxels(0, 63, 12, 'mature').length).toBe(8);
  });
});

describe('url params', () => {
  it('defaults to fixture, tick 0, material, iso', () => {
    expect(parseParams('')).toEqual({ run: 'fixtures/s42-mini', tick: 0, overlay: 'material', cam: 'iso' });
  });
  it('round-trips and rejects junk', () => {
    const s = parseParams('?run=runs/s42&tick=5000&overlay=moisture&cam=top');
    expect(s).toEqual({ run: 'runs/s42', tick: 5000, overlay: 'moisture', cam: 'top' });
    expect(parseParams(toSearch(s))).toEqual(s);
    expect(parseParams('?overlay=nope&cam=fish&tick=abc')).toMatchObject({ overlay: 'material', cam: 'iso', tick: 0 });
  });
});
