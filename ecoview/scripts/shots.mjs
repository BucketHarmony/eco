// The reference screenshots: file name, the URL parameters that produce it, and whether
// `scripts/shot-ref.mjs check` gates the picture or only measures it.
// Shots 01-11 are the noise world (runs/s42, the 256x64 strip, format 3); 12-15 are the Capitol
// (runs/capitol-s42, a world bundle at format 4, ecoview shot G7); 16-17 are edit mode on the
// committed bundle (shot E1).
//
// GATED or SHOWN (shot E6). All seventeen are rendered, compared and reported, and every reference
// stays committed; only the gated ones can redden a job. The split is what a picture is evidence of:
//
//   GATED   the renderer drew the right thing. Terrain, buildings, the material grid, the chart's
//           axes and the editor's chrome are the viewer's own work, so a change here is a renderer
//           regression and must fail.
//   SHOWN   the simulation grew something different. Light, moisture, fertility, temperature, fire,
//           crowding, traits and the Capitol's canopy at tick 20000 are pictures *of ecosim*, and
//           they drift whenever the ecology changes -- 9 of 17 on ecosim shot G4c, 15 of 17 on G4b.
//           ecoview is frozen and ecosim is not, so gating these buys a re-accept shot owed on every
//           future garden shot and catches no renderer bug. They stay in the set because the verdicts
//           in shots/REPORT.md are read by a human, and drift is still printed with its percentage.
//
// What replaces the lost gate is already here and does not move with the ecology: view.spec.ts
// ("03_light: canopy shade is dark and open ground is light"; "switching overlay changes mean view
// color by > 20 in some channel") and overlays.spec.ts's three palette tests assert the same
// overlays relationally. No tolerance moved in either direction to make this split work.
//
// Honest note on three of the eight. 01, 12, 13, 16 and 17 are sim-independent by construction --
// tick 0, or the committed fixtures/capitol-world -- and differ by 0.000% of the view. 02, 08 and 14
// do read the simulation and are merely insensitive to it: on the run that forced this change they
// moved 0.903%, 0.626% and 0.021% of the page against the 2% gate. They are gated because each is
// the only gate on something the renderer owns (02 the iso material scene and the film check's
// reference frame, 08 the chart, 14 the buildings' shade), but a large enough ecology change can
// still push one over, and the percentages the check prints are the early warning.
export const GATED = true;
export const SHOWN = false;

export const SHOTS = [
  ['01_material_t0_iso.png', 'run=runs/s42&tick=0&overlay=material&cam=iso', GATED],
  ['02_material_t10000_iso.png', 'run=runs/s42&tick=10000&overlay=material&cam=iso', GATED],
  ['03_light_t10000_top.png', 'run=runs/s42&tick=10000&overlay=light&cam=top', SHOWN],
  ['04_moisture_t10000_top.png', 'run=runs/s42&tick=10000&overlay=moisture&cam=top', SHOWN],
  ['05_fertility_t10000_top.png', 'run=runs/s42&tick=10000&overlay=fertility&cam=top', SHOWN],
  ['06_temperature_t1000_top.png', 'run=runs/s42&tick=1000&overlay=temperature&cam=top', SHOWN],
  ['07_temperature_t3000_top.png', 'run=runs/s42&tick=3000&overlay=temperature&cam=top', SHOWN],
  ['08_chart_t20000.png', 'run=runs/s42&tick=20000&overlay=material&cam=iso', GATED],
  // 13800 is the only snapshot of the strip's runs/s42 that shows both of the things this overlay draws:
  // 2 patches still alight, and 16 burnouts since 13700 for the charcoal. The run's peak is 6 patches at
  // tick 8924, which is not a snapshot; across all 201 snapshots only 5300 (1 alight, nothing burnt) and
  // 13800 have any flame at all, so there is no second candidate. Shot E7 moved it here from 17100, which
  // had neither flame nor scar after ecosim shot G4c and pictured nothing.
  ['09_fire_t13800_top.png', 'run=runs/s42&tick=13800&overlay=fire&cam=top', SHOWN],
  ['10_crowding_t20000_top.png', 'run=runs/s42&tick=20000&overlay=crowding&cam=top', SHOWN],
  ['11_traits_t20000_top.png', 'run=runs/s42&tick=20000&overlay=traits&cam=top', SHOWN],
  ['12_capitol_medium_t0_iso.png', 'run=runs/capitol-s42&tick=0&overlay=medium&cam=iso', GATED],
  ['13_capitol_medium_t0_top.png', 'run=runs/capitol-s42&tick=0&overlay=medium&cam=top', GATED],
  ['14_capitol_light_t0_top.png', 'run=runs/capitol-s42&tick=0&overlay=light&cam=top', GATED],
  ['15_capitol_material_t20000_iso.png', 'run=runs/capitol-s42&tick=20000&overlay=material&cam=iso', SHOWN],
  // 16 and 17 are edit mode on the committed Capitol bundle (shot E1). ?eye= stands the first-person camera
  // at a fixed spot in metres east, north and up with yaw and pitch, so the crosshair picks without a mouse:
  // 16 looks down the south front onto the roof, 17 stands on the lawn by the east road.
  ['16_edit_hotbar.png', 'world=fixtures/capitol-world&cam=iso&edit=1&slot=building&brush=3&eye=128,6,44,0,-9', GATED],
  ['17_edit_brush5.png', 'world=fixtures/capitol-world&cam=iso&edit=1&slot=lawn&brush=5&eye=70,4,10,0,-20', GATED],
];

/**
 * The SAD's wall-clock budget for the whole set, in seconds, now checked by scripts/shot.mjs rather than
 * only printed. The four Capitol shots did not push the set over it (ecoview/DECISIONS.md, shot G7).
 */
export const BUDGET_S = 60;
