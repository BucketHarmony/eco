import { describe, expect, it } from 'vitest';
import {
  fertilityColor, hexToRgb, lightColor, moistureColor, soilColor, temperatureColor,
} from '../../src/world';
import { canopyVoxels } from '../../src/entities';
import { parseParams, toSearch } from '../../src/ui';

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
