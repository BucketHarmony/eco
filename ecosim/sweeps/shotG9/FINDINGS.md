# Shot G9 — a sun that moves

The prompt is `overnight/shots/G9-sun-path.md`. The fixed 45° southern sun and its hard
shade rule (`World::shade_top`, `[bundle] sun_altitude_deg`) are gone. In their place is a light
budget per ecology column per season slice, computed once when a bundle loads. It walks the sun
through `sun.day_samples` hours between sunrise and sunset at the bundle's own latitude, and walks
the sky dome for the diffuse share, against the ground grid's building heights. The budget is
`world/sun.bin`, one byte per column per slice. Tree light reads the slice for the current tick of
the year, and the light field is recomputed when the slice changes.

Runs: `runs/capitol-s42` (Capitol, seed 42, 20000 ticks, snapshot every 100, the reference command in
`justfile`) at this commit, and the same command run by the binary of the commit before
(`92db8fc`, the fixed sun). `report.py` prints every number below and draws `sun-budget.png`.
Nothing was tuned.

## Event-log causes

Seed 42 on the Capitol. Animals are off in every bundle run, so trees are the only deaths.

| event | cause | G9 | pre-G9 |
| --- | --- | --- | --- |
| germination | - | 13204 | 17807 |
| tree_death | burnt | 2932 | 1464 |
| tree_death | crowded | 4256 | 7189 |
| tree_death | drought | 3800 | 5638 |
| tree_death | old_age | 135 | 388 |
| fire | spread | 2640 | 589 |

**The difference in the log is one fire, not the sun.** Both runs have one large fire. In the G9 run
it comes between ticks 12000 and 14000 and takes the stand from 2295 trees to 914 (1374 burnt in that
window). In the pre-G9 run it comes between 16000 and 18000 and takes 3462 to 2213. The sun changes
light on about 1 in 20 plantable columns (below), which is enough to change which trees seed where,
and after that the two runs share no trajectory. Fewer germinations, crowding deaths and drought
deaths in G9 follow from the stand being small for the six thousand ticks after its fire.

## How much ground is too dark to plant

`tree.sapling_light` is 0.588 of full sun. Under the fixed sun, 2056 of the 43,831 plantable columns
were at light 0 behind a building, 4.69% (G3a measured 4.7%). No column was anywhere between.

| slice | tick of year | plantable below sapling_light | share | min | 5th pct | mean |
| --- | --- | --- | --- | --- | --- | --- |
| spring equinox | 0 | 2116 | 4.83% | 0.061 | 0.598 | 0.899 |
| summer solstice | 1000 | 1629 | 3.72% | 0.061 | 0.637 | 0.912 |
| autumn equinox | 2000 | 2116 | 4.83% | 0.061 | 0.598 | 0.899 |
| winter solstice | 3000 | 2690 | 6.14% | 0.061 | 0.553 | 0.873 |
| annual mean | - | 1950 | 4.45% | 0.061 | 0.609 | 0.896 |

- **The share too dark to plant is about what it was, 3.7% to 6.1% by season against 4.7% fixed.**
  2848 columns (6.5%) fall below the line in at least one slice and 1507 (3.4%) fall below it in all
  four.
- **The budget runs from 0.061 to 1.0 of an open column.** The darkest plantable column keeps 6% of
  open-ground light in every season. That is the zenith sky, which the budget never hides. No
  plantable column is at 0 in any slice, so the black ground of the fixed sun is gone.
- **Almost all ground is shaded a little.** 99.9% of plantable columns lose something over the year,
  mostly a few percent of low sky to a building a long way off. A column counts as open, factor
  1.0, only when nothing at all stands above its horizon.
- **The fixed sun's black columns are now a gradient.** Their annual mean runs from 0.061 to 0.888,
  mean 0.475. 1191 of the 2056 stay below `sapling_light` all year. The other 865 were blacked out
  by a sun that stood due south at 45° all day, which the real sun does not do.

## Trees and canopy

Canopy columns count every young (1 column) or mature (3×3) crown; percent is of plantable ground.

| tick | G9 trees | pre-G9 trees | G3a trees | G9 canopy | pre-G9 canopy | G3a canopy |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 79 | 79 | 79 | 711 (1.6%) | 711 (1.6%) | 711 |
| 10000 | 1492 | 1195 | 602 | 5426 (12.4%) | 2555 (5.8%) | 2173 |
| 20000 | 2160 | 3207 | 1982 | 10988 (25.1%) | 15886 (36.2%) | 8821 |

At tick 10000, before either fire, G9 has more trees and twice the canopy of the fixed sun. At 20000
it has fewer, because its fire came four thousand ticks earlier and burnt more (1374 trees against
462). Both end above G3a, which predates the water tier. At 20000, 18 G9 trees stand on columns
whose annual mean is below `sapling_light` (24 in the pre-G9 run, which did not have that budget),
and 45 stand on columns the fixed sun kept black, where no tree could have germinated before.

## The picture

`sun-budget.png` is the budget in five tiles: the four slices and their annual mean, north up, the
Capitol's 256 m square in each. Brightness is the factor, 0 to 1. Roofs are red. Plantable ground
below `sapling_light` is tinted blue.

What it shows: the Capitol building is the red cross in the middle, and the shade it casts reaches
north. At the summer solstice the shadow is a short band close around the building, and the blue
ring on its north side is thin. At the equinoxes the shade reaches farther and fans out northeast
and northwest, where the morning and evening sun throw it. At the winter solstice the sun stays low
and the shade reaches nearly to the north edge of the site in a fan of wedges. Blue fills the
courtyards and the ground hard against the north walls, and the small buildings along the south
edge gain blue strips on their north sides that summer does not have. The annual mean is soft
shade round the building with blue only in the courtyards and along the north walls. The wedges are
the nine sun positions per day, each throwing its own shadow. More `day_samples` would blend them
at the cost of load time.

## Cost

Computing the budget adds about 1.45 s to loading the Capitol (150 ms to 1.6 s). The 20000-tick
reference run took 30.4 s against 30.8 s for the fixed sun; the relight four times a year costs less
than the run-to-run noise. The noise worlds have no buildings, so no budget is built, and their
runs and manifests are byte-identical to the commit before.
