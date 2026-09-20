# capitol — Michigan State Capitol grounds

A world bundle (version 2, `../../../docs/SCENE-CONTRACT.md`) of a 256 m square of the Michigan
State Capitol grounds in Lansing, Michigan, centred on the dome. It is the garden series' reference
real world, added in shot G2.

```sh
ecosim run --world worlds/capitol --seed 1 --ticks 20000 --out runs/capitol \
    --snapshot-every 100 --set animals.enabled=false --set climate.rain_gradient=0
```

## Shape

| | |
|---|---|
| Crop | 256 m square, centred on the dome at 42.73365 N, 84.55553 W |
| Ecology grid | 256 × 256 columns of 1 m (`[world] width` and `depth` come from the bundle) |
| Ground grid | 512 × 512 cells of 0.5 m, cell (0, 0) at the south-west corner |
| Ground height | 0.000–8.589 m above the crop minimum, which is 255.00 m NAVD88 |
| Building height | 0–76.011 m above ground (the dome; the highest LiDAR return, the finial, is 80 m) |
| Media, in 0.5 m cells | lawn 169,876, asphalt 44,879, roof 24,705, concrete 22,684. No soil, bed, mulch, gravel or water: nothing in the crop was classified as any of those |
| Surface layers | 8–16, that is `[bundle] base_z` = 8 plus the rounded mean ground height of each column |
| Sealed columns | 21,705 of 65,536 (33.1%) are `Rock`: more than half their ground cells are roof, asphalt or concrete |
| Trees | 81, 4.675–23.59 m tall (10th/50th/90th percentile 6.2, 13.8, 20.9 m) |
| Shrubs | 64, 0.774–2.472 m tall |
| Pipes | 4, illustrative (see below) |

`ecosim/tests/bundle.rs` pins every number in this table, so CI notices if a re-export moves one.

## Provenance

### Elevation, buildings, trees and shrubs — public domain

USGS 3DEP, *USGS_LPC_MI_31Co_Ingham_2017_LAS_2019*, public domain, downloaded from
`https://rockyweb.usgs.gov/vdelivery/Datasets/Staged/Elevation/LPC/Projects/USGS_LPC_MI_31Co_Ingham_2017_LAS_2019/laz/`:

| Tile | sha256 |
|---|---|
| USGS_LPC_MI_31Co_Ingham_2017_070447_LAS_2019.laz | `52de24ca037be1a3bb274ba25be45cfea8f20a92cdcc8af083f6af2a83b71a33` |
| USGS_LPC_MI_31Co_Ingham_2017_070450_LAS_2019.laz | `a95f6976b9fb52fa80dc6d15cf01e68d228f5e6b308eb79423fb532ba6804046` |
| USGS_LPC_MI_31Co_Ingham_2017_072447_LAS_2019.laz | `8fb645ed2e344a59fae154f312bc28eb87653e1f2af859774d4405e697cf8c55` |
| USGS_LPC_MI_31Co_Ingham_2017_072450_LAS_2019.laz | `aa124cee6588aabaa8575d51936670e4f1beae2b8c1a178a901ebd4714eaeb3a` |

The tiles are NAD83(2011) / Michigan South (ft) + NAVD88 height (EPSG:6499), in international feet
on all three axes; the scene converts every axis to metres. The crop holds about 2.5 points/m².

### Walks, plazas and parking — OpenStreetMap, ODbL

> Map data © OpenStreetMap contributors, available under the Open Database License (ODbL)
> https://www.openstreetmap.org/copyright

The walk network, the three entrance plazas and two small parking lots in the medium grid come from
the OpenStreetMap Overpass API, from the attic snapshot of 2017-05-01 — the snapshot contemporary
with the LiDAR, chosen because its walks match the LiDAR's ground intensity better than today's do
(the grounds were re-landscaped after 2017). Fourteen present-day `footway=sidewalk` ways along the
bounding streets were added, because the 2017 snapshot had barely mapped them.

### Licence per file

| File | Source | Licence |
|---|---|---|
| `medium.u8` | LiDAR intensity and roof planes, **merged with OpenStreetMap geometry** | **ODbL**: a derivative database. Redistributing or publicly using it keeps the credit above and the share-alike |
| `ground_h.f32` | LiDAR class-2 ground returns | Public domain |
| `building_h.f32` | LiDAR roof planes, and the Capitol as a heightfield | Public domain |
| `trees.json`, `shrubs.json` | LiDAR canopy segmentation | Public domain |
| `pipes.json` | LiDAR ground sinks on the LiDAR-only asphalt raster, before the OSM merge | Public domain |
| `bundle.json` | Metadata; its `source` string carries the ODbL credit | Same terms as `medium.u8` |

**The pipes are illustrative, not real storm drains.** No storm-sewer data was used. Each of the
four is placed at a low point of the ground on paving and run straight to the nearest crop edge,
with an invented capacity of 50 m³/h. `illustrative: true` says so in `pipes.json` itself, and
anything the sim concludes about drainage here is about the shape of the ground, not about Lansing's
sewers.

No aerial imagery is used. The only NAIP frame covering the crop is from 2022, after the
re-landscaping, so it is not part of the scene.

## Re-exporting

The bundle is exported from a tagged Blender scene the operator keeps outside this repo
(`capitol.blend`, built from the LiDAR by a pipeline that is not part of ecosim):

```sh
blender -b capitol.blend --python tools/blend_export.py -- worlds/capitol --audit 4096
```

The export is deterministic — the same `.blend` gives a byte-identical bundle — so a re-export
should leave `git status` clean and `sha256sum -c SHA256SUMS` passing. `tools/blend_export.py`'s
pure half is unit-tested in plain CPython (`just pyexport`); CI has no Blender and never needs one,
because the bundle is committed.

## Known limitations

These come from the scene, not from the exporter, and this bundle keeps them:

- The Capitol is a 1 m heightfield prism, not a modelled building: the plane fit topped out at the
  roofline and lost the dome, so the footprint's top is the 90th percentile of returns per cell,
  with a 3 × 3 maximum above the roofline for the dome and drum.
- Several of the other 15 buildings are cut by the north crop edge, and three small ones (17–31 m²)
  are probably kiosks or monuments.
- A dozen small dark-intensity blobs in the lawn, a few square metres each, are tagged asphalt; they
  are LiDAR shadows and wet ground rather than pavement.
- A few of the 81 trees along the streets may be young street trees or poles.
