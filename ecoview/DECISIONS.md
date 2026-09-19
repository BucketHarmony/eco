# ecoview decisions

Choices the SAD and addendum leave open, or where the two conflicted with the data.

## Dependencies
- `three@0.169.0` and `@types/three@0.169.0` pinned exactly, as is `@playwright/test@1.63.0`. `vite`, `vitest` and
  `typescript` are pinned to the latest versions at build time (vite 8.3.0, vitest 5.0.1, TypeScript 7.0.2).
  `@types/node` was added so the unit tests can read the fixture from disk.

## Canopies are hidden in the top camera on field overlays
The addendum says canopy cubes render at opacity 0.25 on every overlay except `material`, and it also requires
5% of `03_light` to be darker than RGB(60,60,60) in **all three channels**. These conflict. A mature canopy
leaves the ground at light 55, but a 0.25-opacity `#2e8b3d` canopy over it gives green ≈ 0.25·139 + 0.75·55 ≈ 76.
With the two stacked canopy layers seen from above it's ≈ 92. Every shaded pixel would then fail the green
channel, so the test couldn't pass. The addendum's own 5% budget ("9 columns × 156 px") also assumes the shaded
ground is what's visible.

Decision: in the `top` camera on field overlays, canopies are not drawn. Trunks, which are opaque, still mark
every tree, and the shaded squares show exactly where the canopies are. In `iso` and `side` on field overlays,
canopies render at opacity 0.25 with `depthWrite` off, as specified. On `material` they are opaque in every camera.
Measured on `03_light`: 20.6% dark and 66.5% light, against the 5% and 40% floors.

## Lighting intensity scaled by π
The addendum's ambient 0.6 / directional 1.0 are legacy-lighting numbers. three r155+ uses physically based
units, where Lambert diffuse is divided by π, so those values render the iso scene about 30% too dark (soil comes
out near-black olive). Both intensities are multiplied by π, which gives the intended `albedo · (0.6 + cosθ)`.
The top camera uses `MeshBasicMaterial` everywhere, so it's unaffected and pixel colors equal overlay colors exactly.

## Overlay details
- **Rock columns** on `moisture` and `fertility` draw rock gray `#8a8a8a`. The sim writes 0 there, meaning "no
  data", which would otherwise render as white (dry, or infertile) and look like real data.
- **`temperature`** colors every non-water column, rock included, by its patch temperature, because temperature
  exists for all patches.
- **`light`** samples `light.bin` at `(x, y, height+1)` for every column, water and rock included, as the addendum says.
- Entities use unlit `MeshBasicMaterial` in the top camera and Lambert in `iso`/`side`, matching the surface.

## Chart
Grazers, hunters and trees are each normalized to their own max, because grazers reach ~4,400 while hunters
peak under 100. The legend shows each max (`grazers ≤4453`). Each horizontal pixel plots the mean of its bucket
of rows, which keeps drawing 20,001 rows cheap and damps per-tick noise. The line colors are the species colors
from `meta.json`. A dashed vertical line marks the current snapshot tick.

## Cameras
- `iso` sits at world center (32, 16, 32) + (70, 75, 70) and looks at (32, 12, 32). The addendum doesn't say what
  "world center" is, so this uses the geometric center of the 64×32×64 volume.
- `side` sits at (32, 30, −60) and looks at (32, 12, 32), as specified. From that close it crops the world's left
  and right edges. It is left as specified.
- `top` is an orthographic camera with `up = (0, 0, −1)`, so sim +x points right and +y points up, framing 64
  world units into 800 px.
- OrbitControls are attached to every camera, with rotation disabled in `top`. They re-render only on their
  `change` event.

## Runtime
- Loaded snapshots are cached in memory (8 most recent) so playback and scrubbing back and forth don't refetch.
  A sequence token drops stale async loads when the state changes mid-fetch.
- Play steps one snapshot every 250 ms using `setInterval`. This is a timer, not a render loop: each step
  triggers one on-demand render.
- A bad `format_version`, a missing file or a wrong binary size shows `Error: …` in the sidebar, sets
  `window.__ecoviewError`, and leaves `__ecoviewReady` false.
- The page is fixed at 1280×800: `#view` 960×800 at pixel ratio 1, plus the 320 px sidebar.

## Scripts and tests
- `npm test` has a `pretest` hook that runs `npm run build`, so the Playwright `webServer` (`vite preview`) never
  serves a stale `dist/`.
- `npm run shot` uses port 4174, not 4173, so it can run while a preview server for tests is up.
- Pixel tests screenshot only the `#view` element and decode the PNG inside the page through a 2D canvas. This
  avoids an image-decoding dependency.
- Extra tests beyond the SAD's list:
  - loader errors on missing files and short binaries;
  - `pickSnapshot` clamping;
  - URL parameter round-trip;
  - canopy geometry clipping;
  - a DOM-driven overlay switch;
  - an end-to-end scrub over every 20th snapshot of the full run.
