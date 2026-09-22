# Blender scene contract for ecosim (world bundle v2)

A `.blend` file is an ecosim world when its objects carry the tags below. `ecosim/tools/blend_export.py`, run in headless Blender, turns it into a world bundle:

    blender -b <scene.blend> --python ecosim/tools/blend_export.py -- <out_dir>

Untagged objects (cameras, lights, decoration, point clouds) are ignored.

## Scene custom properties
| Property | Type | Meaning |
|---|---|---|
| `eco_name` | string | world name, e.g. "capitol" |
| `eco_origin` | float[2] | scene XY of the crop's south-west corner, in metres |
| `eco_size_m` | float | crop edge length in metres (square crop), e.g. 256 |
| `eco_ground_cell_m` | float | ground/water grid cell size, e.g. 0.5 |
| `eco_default_medium` | string | medium for ground cells no surface covers (default "lawn") |
| `eco_source` | string | provenance: data source, licence, crop centre |
| `eco_latitude_deg` | float | where the crop is, degrees north of the equator (negative south). Required since shot S9: the exporter stops rather than write a bundle without one |

Scene units are metres, with +X east, +Y north and +Z up.

## Object custom property `eco_kind`
| `eco_kind` | Object | What the exporter reads |
|---|---|---|
| `ground` | exactly one mesh | ground height at each ground-cell centre, by a downward ray cast |
| `surface` | mesh, plus `eco_medium` | XY footprint sets the cell's medium. Where surfaces overlap, the one with the higher top at that cell wins |
| `tree` | any object | origin = trunk base. `height`, `crown_radius`, `crown_base` custom props, else from the bounding box |
| `shrub` | any object | origin = centre. `height`, `rx`, `ry`, `angle` props, else from the bounding box |
| `pipe` | curve | first point = inlet, last point = outlet (an outlet on or past the crop edge drains out of the world). `capacity_m3h` prop |

For a `surface` with `eco_medium = "roof"`, the exporter also records building height: the surface's top Z minus ground height at that cell.

## Media (`eco_medium`)
`soil`, `lawn`, `bed`, `mulch`, `gravel`, `concrete`, `asphalt`, `roof`, `water`. Their behaviour (infiltration, runoff, whether plants can grow) is in ecosim's `params.toml`, not in the scene.

## Bundle v2 (what the exporter writes)
- `bundle.json`: `format` "ecosim-world-bundle", `version` 2, `name`, `size_m`, `ground_cell_m`, `ground_width`, `ground_depth`, `media` (code → name, in the order listed above, soil = 0), `source`, `latitude_deg`, counts.
  `latitude_deg` was added in shot S9 and is additive, so it does not bump `version`: a bundle written before it has no such key and ecosim still reads it, with no latitude. The exporter, by contrast, requires one — a reader has to cope with old bundles, a writer never has to make a new one.
  Only the sun's path needs it. Nothing in ecosim reads it; the loader carries it into the run's `meta.json` `world` object so a renderer draws this site's sun instead of a constant of its own.
- `ground_h.f32`: ground grid, LE f32, metres above the crop minimum.
- `medium.u8`: ground grid, medium codes.
- `building_h.f32`: ground grid, LE f32, building height above ground (0 where there's no roof).
- `trees.json` `[{x, y, height, crown_radius, crown_base}]`, `shrubs.json` `[{x, y, height, rx, ry, angle}]`, `pipes.json` `[{id, inlet: [x, y], outlet: [x, y], capacity_m3h, illustrative}]`. Positions are metres from the south-west corner.

Grids are indexed `x + width*y`, with x east, y north and cell (0,0) at the south-west corner. The same `.blend` must always export to a byte-identical bundle.
