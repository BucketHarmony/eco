// The reference screenshots: file name, the URL parameters that produce it, and the time budget group.
// Shots 01-11 are the noise world (runs/s42, the 256x64 strip, format 3); 12-15 are the Capitol
// (runs/capitol-s42, a world bundle at format 4, ecoview shot G7).
export const SHOTS = [
  ['01_material_t0_iso.png', 'run=runs/s42&tick=0&overlay=material&cam=iso'],
  ['02_material_t10000_iso.png', 'run=runs/s42&tick=10000&overlay=material&cam=iso'],
  ['03_light_t10000_top.png', 'run=runs/s42&tick=10000&overlay=light&cam=top'],
  ['04_moisture_t10000_top.png', 'run=runs/s42&tick=10000&overlay=moisture&cam=top'],
  ['05_fertility_t10000_top.png', 'run=runs/s42&tick=10000&overlay=fertility&cam=top'],
  ['06_temperature_t1000_top.png', 'run=runs/s42&tick=1000&overlay=temperature&cam=top'],
  ['07_temperature_t3000_top.png', 'run=runs/s42&tick=3000&overlay=temperature&cam=top'],
  ['08_chart_t20000.png', 'run=runs/s42&tick=20000&overlay=material&cam=iso'],
  // 17100 is the snapshot with the most patches burning in the strip's runs/s42 (3), with 27 burnouts since 17000.
  ['09_fire_t17100_top.png', 'run=runs/s42&tick=17100&overlay=fire&cam=top'],
  ['10_crowding_t20000_top.png', 'run=runs/s42&tick=20000&overlay=crowding&cam=top'],
  ['11_traits_t20000_top.png', 'run=runs/s42&tick=20000&overlay=traits&cam=top'],
  ['12_capitol_medium_t0_iso.png', 'run=runs/capitol-s42&tick=0&overlay=medium&cam=iso'],
  ['13_capitol_medium_t0_top.png', 'run=runs/capitol-s42&tick=0&overlay=medium&cam=top'],
  ['14_capitol_light_t0_top.png', 'run=runs/capitol-s42&tick=0&overlay=light&cam=top'],
  ['15_capitol_material_t20000_iso.png', 'run=runs/capitol-s42&tick=20000&overlay=material&cam=iso'],
  // 16 and 17 are edit mode on the committed Capitol bundle (shot E1). ?eye= stands the first-person camera
  // at a fixed spot in metres east, north and up with yaw and pitch, so the crosshair picks without a mouse:
  // 16 looks down the south front onto the roof, 17 stands on the lawn by the east road.
  ['16_edit_hotbar.png', 'world=fixtures/capitol-world&cam=iso&edit=1&slot=building&brush=3&eye=128,6,44,0,-9'],
  ['17_edit_brush5.png', 'world=fixtures/capitol-world&cam=iso&edit=1&slot=lawn&brush=5&eye=70,4,10,0,-20'],
];

/**
 * The SAD's wall-clock budget for the whole set, in seconds, now checked by scripts/shot.mjs rather than
 * only printed. The four Capitol shots did not push the set over it (ecoview/DECISIONS.md, shot G7).
 */
export const BUDGET_S = 60;
