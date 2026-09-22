# Shot G10 — a deciduous year

Worked from the backlog row; there is no prompt file. The row: *the model transpires at the same
rate in January and July — `hydro.et_mm_h` has a temperature factor and the tree draw has none
(UNITS.md finding 4).* This shot gives the tree draw a season. It does not give the canopy one:
leaf-off light is G9's (the moving sun), and a leaf-litter pulse is not added (DECISIONS.md, G10).

**The mechanism.** A tree's leaf-on share is linear in its patch temperature, 0 at and below
`tree.leaf_off_temp` (5 °C) and 1 at and above `tree.leaf_on_temp` (10 °C). A leafless tree gives up
`tree.deciduous` of its draw. The draw is then divided by its own mean over the model year, so a
year's transpiration is still `transpiration_mm_h × 8766 h` = 300 mm (UNITS.md R8, an *annual*
figure) and only its season moves. On the model's 12 ± 15 °C year, sampled at every temperature
update as the patches see it, a tree is bare 32.5% of the year and in full leaf 52.5%; the mean
leaf-on share is 0.597.

| `deciduous` | in-leaf draw, mm/h | bare draw, mm/h | year, mm |
| --- | --- | --- | --- |
| 0 (the model before G10) | 0.0342 | 0.0342 | 300 |
| **0.5 (shipped)** | 0.0428 | 0.0214 | 300 |
| 1 | 0.0573 | 0 | 300 |

Everything below is measured. Runs: `sweep.sh` (seeds 1–3 on the strip at five values of
`tree.deciduous`, animals on; the Capitol reference world at 0, 0.5 and 1 with the `just capitol`
overrides), and seed 42 at the shipped default. `analyse.py` makes `scan.csv` and the tables.

## Event-log causes, per species, on the reference seeds

At the shipped default (`tree.deciduous = 0.5`), 20000 ticks:

| seed | grazer deaths | hunter deaths | tree deaths |
| --- | --- | --- | --- |
| 1 | 52857 — crowded 41166, eaten 10596, burnt 445, old_age 358, starved 292 | 395 — crowded 159, old_age 97, starved 81, burnt 58 | 3301 — crowded 1385, drought 1170, burnt 676, old_age 70 |
| 2 | 57395 — crowded 44414, eaten 11846, old_age 778, starved 233, burnt 124 | 380 — crowded 260, old_age 118, burnt 2 | 1908 — crowded 1523, old_age 176, drought 120, burnt 89 |
| 3 | 45760 — crowded 33361, eaten 8395, old_age 2276, burnt 1357, starved 371 | 304 — crowded 151, old_age 74, burnt 65, starved 14 | 3193 — crowded 2398, drought 436, old_age 209, burnt 150 |
| 42 | 44813 — crowded 33382, eaten 7913, old_age 1906, burnt 1329, starved 283 | 273 — crowded 155, old_age 67, burnt 50, starved 1 | 4138 — crowded 2834, drought 876, old_age 245, burnt 183 |
| Capitol | — (animals off) | — | 14679 — crowded 7189, drought 5638, burnt 1464, old_age 388 |

Before G10 (`deciduous = 0`, byte-identical to the pre-G10 binary — `deciduous_off_cuts_to_the_pre_g10_manifest`)
seed 42 was 3498 tree deaths, crowded 2109, drought 725, burnt 499, old_age 165, and the Capitol
10324, crowded 3682, drought 3314, burnt 3144, old_age 184.

The mix did not move. Crowding is still the first cause of death for every species on every seed and
drought is still second for trees everywhere but seed 2. What moved is the size of the drought line
on seed 1, which went from 618 to 1170, and that is the point of the next section.

## The whole sweep

Causes, every cell:

| deciduous | seed | grazer deaths | hunter deaths | tree deaths |
| --- | --- | --- | --- | --- |
| 0 | 1 | 52443 — crowded 39600, eaten 11989, burnt 354, starved 273, old_age 227 | 436 — crowded 252, old_age 117, starved 55, burnt 12 | 4200 — crowded 2245, burnt 1157, drought 618, old_age 180 |
| 0 | 2 | 57153 — crowded 43976, eaten 12057, old_age 627, starved 261, burnt 232 | 407 — crowded 272, old_age 103, burnt 29, starved 3 | 2960 — crowded 1855, drought 683, burnt 261, old_age 161 |
| 0 | 3 | 46334 — crowded 34116, eaten 8742, old_age 2076, burnt 1089, starved 311 | 318 — crowded 170, burnt 70, old_age 69, starved 9 | 4055 — crowded 3087, drought 475, old_age 284, burnt 209 |
| 0.25 | 1 | 52176 — crowded 39448, eaten 11649, starved 467, old_age 326, burnt 286 | 418 — crowded 222, old_age 113, starved 68, burnt 15 | 3798 — crowded 2140, burnt 778, drought 707, old_age 173 |
| 0.25 | 2 | 60706 — crowded 47562, eaten 12097, old_age 588, starved 245, burnt 214 | 373 — crowded 251, old_age 115, burnt 7 | 1976 — crowded 1674, old_age 156, drought 122, burnt 24 |
| 0.25 | 3 | 43196 — crowded 30592, eaten 8538, old_age 2720, burnt 1046, starved 300 | 308 — crowded 181, burnt 55, old_age 54, starved 18 | 4111 — crowded 2917, drought 713, old_age 262, burnt 219 |
| 0.5 | 1 | 52857 — crowded 41166, eaten 10596, burnt 445, old_age 358, starved 292 | 395 — crowded 159, old_age 97, starved 81, burnt 58 | 3301 — crowded 1385, drought 1170, burnt 676, old_age 70 |
| 0.5 | 2 | 57395 — crowded 44414, eaten 11846, old_age 778, starved 233, burnt 124 | 380 — crowded 260, old_age 118, burnt 2 | 1908 — crowded 1523, old_age 176, drought 120, burnt 89 |
| 0.5 | 3 | 45760 — crowded 33361, eaten 8395, old_age 2276, burnt 1357, starved 371 | 304 — crowded 151, old_age 74, burnt 65, starved 14 | 3193 — crowded 2398, drought 436, old_age 209, burnt 150 |
| 0.75 | 1 | 53188 — crowded 39447, eaten 12962, starved 459, old_age 320 | 474 — crowded 280, old_age 126, starved 68 | 2187 — crowded 1531, drought 456, old_age 127, burnt 73 |
| 0.75 | 2 | 60363 — crowded 46179, eaten 13445, old_age 499, starved 185, burnt 55 | 445 — crowded 304, old_age 126, starved 15 | 25 — drought 25 |
| 0.75 | 3 | 47912 — crowded 35235, eaten 9486, old_age 1710, burnt 1127, starved 354 | 344 — crowded 212, old_age 74, burnt 44, starved 14 | 2652 — crowded 1542, burnt 714, drought 258, old_age 138 |
| 1 | 1 | 53492 — crowded 40765, eaten 12115, starved 358, old_age 253, burnt 1 | 458 — crowded 261, old_age 126, starved 71 | 3193 — crowded 1574, drought 1381, burnt 126, old_age 112 |
| 1 | 2 | 57160 — crowded 43312, eaten 12662, old_age 803, starved 259, burnt 124 | 393 — crowded 273, old_age 120 | 25 — drought 25 |
| 1 | 3 | 46095 — crowded 33889, eaten 9275, old_age 1853, burnt 831, starved 247 | 338 — crowded 182, burnt 77, old_age 72, starved 7 | 3141 — crowded 1968, drought 757, burnt 257, old_age 159 |
| 0 | capitol | 0 | 0 | 10324 — crowded 3682, drought 3314, burnt 3144, old_age 184 |
| 0.5 | capitol | 0 | 0 | 14679 — crowded 7189, drought 5638, burnt 1464, old_age 388 |
| 1 | capitol | 0 | 0 | 7234 — crowded 3312, drought 3242, burnt 519, old_age 161 |

Per cell (the last two years are ticks 12000–20000; winter and summer are the quarters of the model
year centred on its coldest and warmest points, over those two years; `soil_water_*` is the site
mean from `series.csv`):

| run | trees_end | trees_mean_last2y | trees_min | grazers_end | hunters_end | soil_water_winter_mm | soil_water_summer_mm | drainage_mm_total | tree_deaths | tree_drought | tree_crowded | tree_burnt | tree_old_age | tree_extinct | grazer_extinct | hunter_extinct |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| d0-s1 | 572 | 781.9 | 12 | 1725 | 110 | 105.88 | 75.99 | 1837.2 | 4200 | 618 | 2245 | 1157 | 180 | None | None | None |
| d0-s2 | 782 | 700.4 | 12 | 2380 | 146 | 106.81 | 81.38 | 1707.9 | 2960 | 683 | 1855 | 261 | 161 | None | None | None |
| d0-s3 | 901 | 1024.3 | 12 | 2763 | 62 | 120.68 | 99.27 | 1756.8 | 4055 | 475 | 3087 | 209 | 284 | None | None | None |
| d0.25-s1 | 666 | 804.2 | 12 | 1459 | 116 | 103.96 | 75.08 | 1703.4 | 3798 | 707 | 2140 | 778 | 173 | None | None | None |
| d0.25-s2 | 647 | 554.4 | 12 | 2389 | 199 | 112.1 | 89.15 | 1636.1 | 1976 | 122 | 1674 | 24 | 156 | None | None | None |
| d0.25-s3 | 1036 | 1025.9 | 12 | 2726 | 66 | 115.33 | 89.57 | 1656.5 | 4111 | 713 | 2917 | 219 | 262 | None | None | None |
| d0.5-s1 | 182 | 303.2 | 12 | 1758 | 98 | 103.59 | 75.4 | 1952.6 | 3301 | 1170 | 1385 | 676 | 70 | None | None | None |
| d0.5-s2 | 658 | 551.7 | 11 | 2374 | 170 | 105.69 | 86.33 | 1461.6 | 1908 | 120 | 1523 | 89 | 176 | None | None | None |
| d0.5-s3 | 924 | 776.2 | 12 | 2733 | 58 | 115.84 | 90.85 | 1805.6 | 3193 | 436 | 2398 | 150 | 209 | None | None | None |
| d0.75-s1 | 503 | 574.5 | 12 | 1341 | 128 | 103.88 | 88.82 | 1822.7 | 2187 | 456 | 1531 | 73 | 127 | None | None | None |
| d0.75-s2 | 0 | 0.0 | 0 | 2150 | 207 | 108.23 | 87.62 | 1888.3 | 25 | 25 | 0 | 0 | 0 | 1600 | None | None |
| d0.75-s3 | 277 | 493.9 | 12 | 3020 | 68 | 119.83 | 88.83 | 1657.1 | 2652 | 258 | 1542 | 714 | 138 | None | None | None |
| d1-s1 | 797 | 675.5 | 12 | 1667 | 116 | 103.1 | 85.3 | 1757.0 | 3193 | 1381 | 1574 | 126 | 112 | None | None | None |
| d1-s2 | 0 | 0.0 | 0 | 2462 | 196 | 107.25 | 85.04 | 1497.6 | 25 | 25 | 0 | 0 | 0 | 1650 | None | None |
| d1-s3 | 889 | 702.1 | 12 | 2746 | 67 | 121.39 | 91.15 | 1678.5 | 3141 | 757 | 1968 | 257 | 159 | None | None | None |
| cap-d0 | 1522 | 1715.3 | 77 | 0 | 0 | 96.4 | 72.52 | 1546.4 | 10324 | 3314 | 3682 | 3144 | 184 | None | None | None |
| cap-d0.5 | 3207 | 3220.9 | 79 | 0 | 0 | 94.86 | 74.12 | 1466.2 | 14679 | 5638 | 7189 | 1464 | 388 | None | None | None |
| cap-d1 | 1690 | 1596.0 | 40 | 0 | 0 | 94.18 | 83.66 | 1517.9 | 7234 | 3242 | 3312 | 519 | 161 | None | None | None |

## What leaf-off does here: it makes summer the dry season, and summer was already the dry season

The row expected leaf-off to be relief — trees stop drinking in winter. It is not, and the reason is
the normalisation. The draw a bare tree gives up in winter is spent in summer, when the soil is
already at its driest (75–100 mm against 100–125 in winter on the strip). Where a tree was near its
drought line in July it is now over it, because in July it draws up to 1.68 times what it did.

**Extinctions.** Two cells lose their trees, both on seed 2: `deciduous` 0.75 (extinct at tick
1600) and 1 (tick 1650). Both are the same event. The twelve starting trees and the thirteen
seedlings they manage in the first spring all die of drought between ticks 800 and 1650 — the first
summer — before the stand has seeded enough to carry on; 25 deaths, every one `drought`. At 0 the
same seed also loses trees to drought at ticks 900–1300, but the evergreen draw is gentler in July
and enough of the cohort survives (61 trees at tick 2000 against 0). No grazer or hunter goes extinct
in any cell.

**Contiguity.** The collapsing cells form one region: seed 2 at `deciduous >= 0.75`. Every cell at
or below 0.5 survives on every seed and passes every `ecosim check` line (seed 42 was checked at 0.25
and 0.5 as well). Above 0.5 the passing region is not contiguous: 0.6 fails seed 1 on `mature trees
at 2.5 years >= 35` (11), 0.7 passes all four seeds, and 0.75 fails seed 2. The fail at 0.6 and the
pass at 0.7 are the same knife edge seen from two sides — whether the first summer's cohort gets
through — and not a structure in the parameter. That is why the default is 0.5, the top of the region
contiguous with 0, and not 0.7 (TUNING.md, G10).

**The Capitol.** Tree counts at the end were 1522 (0), 3207 (0.5) and 1690 (1), with means over the
last two years of 1715, 3221 and 1596. One replicate per condition; the strip's three seeds move
their means by up to ±300 between neighbouring values of the knob with no order to it, so nothing
here supports a direction. Drought deaths are 3314, 5638 and 3242. The robust finding is the one the
strip makes: summer water is what the trees are short of, and the season moves water into summer.

**Why the knob cannot go to 1.** A real temperate broadleaf is close to `deciduous = 1`. The model
cannot carry it because of UNITS.md finding 3: a tree draws from its 1 m² trunk column alone while its
crown covers nine. 0.0573 mm/h is 1.4 mm a day, which is not a large summer rate for a crown, but
it all comes out of one column of a lawn's 150 mm AWC, and 45.7 days below 12% of it kills. Concentrating the year into the summer puts the single-column error
into the season that can least take it. Root spread is the mechanism that would let the knob go to 1;
it is still missing, and it is not added here.

## Rate-0

`tree.deciduous = 0` reads no temperature and multiplies nothing (the rate check is outside the tree
loop). Seed 42 at 20000 ticks then reproduces `tests/data/s42-manifest-preG10.sha256`, a byte copy of
the manifest the parent commit 1add456 reproduces, line for line; G10 adds no column and no file, so
nothing is cut. The two older frozen identities (`tree_crowding_off_cuts_to_its_manifest`,
`npk_off_cuts_to_the_pre_g5_manifest`) set `deciduous = 0` as well, since both binaries predate it.

## Wall time

Eighteen 20000-tick runs, eight at a time: 3 min 30 s for the first seventeen (18:04:58–18:08:27),
plus one Capitol run at 0.5 afterwards.
