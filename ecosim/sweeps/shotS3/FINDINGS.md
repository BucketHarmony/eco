# Shot S3: the light a crown actually receives

The shot publishes a per-tree `crown_light` (with the `height_m` and `crown_radius_m` it is derived
from) and changes no ecology. Every table below is printed by `analyse.py` in this directory from the
reference runs it names at the top; nothing here is carried over from an earlier pass.

## Event-log cause breakdown, seeds 1, 2, 3 and 42

Deaths by cause from `events.csv` over 20000 ticks at the defaults: the three noise-strip seeds, and
the Capitol reference run (bundle world, seed 42, animals off, flat rain — hence no animal rows).

| run | species | deaths | causes |
|---|---|---|---|
| capitol s42 | tree | 15405 | drought 9020, crowded 3638, burnt 1611, old_age 1136 |
| seed 1 | grazer | 52877 | crowded 40947, eaten 10313, starved 1261, old_age 356 |
| seed 1 | hunter | 416 | crowded 266, old_age 124, starved 26 |
| seed 1 | tree | 3911 | crowded 2020, drought 1071, old_age 698, burnt 122 |
| seed 2 | grazer | 57791 | crowded 44872, eaten 10716, starved 1532, old_age 667, burnt 4 |
| seed 2 | hunter | 394 | crowded 288, old_age 106 |
| seed 2 | tree | 3028 | crowded 1633, drought 763, old_age 595, burnt 37 |
| seed 3 | grazer | 47057 | crowded 35563, eaten 8019, old_age 1958, starved 1513, burnt 4 |
| seed 3 | hunter | 305 | crowded 225, old_age 71, starved 9 |
| seed 3 | tree | 3561 | crowded 1871, drought 912, old_age 707, burnt 71 |

**Every one of these numbers is the pre-shot one, to the byte.** `events.csv` is not among the files
this shot touches: the regenerated `s42` manifests differ from `tests/data/s42-manifest-preS3.sha256`
in exactly 201 lines and all 201 are `entities.json`, which
`the_crown_fields_changed_only_entities_json` asserts. The table is reproduced here because the
reporting rule asks every sim shot to lead with it, and because it is the evidence that this shot did
not move it.

One row in it is worth reading next to the rest of this document: **`crowded` is already the largest
non-drought cause of tree death**, and it is a *count* rule, not a light rule. `Sim::crowding` counts
other trunks within Chebyshev 2 (a mature neighbour) or 1 (a young one) and kills at
`tree.crowding_mortality` per update. So the simulator's only tree-on-tree competition works at 5 m,
while the crowns it now publishes are 6–12 m across.

## What `light.bin` says about a tree, and what its crown says

`published crown_light` is the new field; `sampled light.bin` is what a reader gets today by looking
the tree's own voxel up in the light field, which is what both viewers do.

### Capitol, seed 42, animals off

| tick | stage | published crown_light | sampled light.bin |
|---|---|---|---|
| 0 | mature | n=79 distinct=30 min=0.6200 med=1.0000 max=1.0000 sd=0.1060 | n=79 distinct=1 min=0.1373 med=0.1373 max=0.1373 sd=0.0000 |
| 10000 | sapling | n=321 distinct=28 min=0.0003 med=1.0000 max=1.0000 sd=0.4324 | n=321 distinct=1 min=1.0000 med=1.0000 max=1.0000 sd=0.0000 |
| 10000 | young | n=352 distinct=69 min=0.0000 med=1.0000 max=1.0000 sd=0.4079 | n=352 distinct=1 min=0.3686 med=0.3686 max=0.3686 sd=0.0000 |
| 10000 | mature | n=325 distinct=222 min=0.0002 med=0.6129 max=1.0000 sd=0.3562 | n=325 distinct=1 min=0.1373 med=0.1373 max=0.1373 sd=0.0000 |
| 20000 | sapling | n=1117 distinct=131 min=0.0000 med=0.0003 max=1.0000 sd=0.1518 | n=1117 distinct=1 min=1.0000 med=1.0000 max=1.0000 sd=0.0000 |
| 20000 | young | n=96 distinct=48 min=0.0000 med=0.0007 max=1.0000 sd=0.1051 | n=96 distinct=1 min=0.3686 med=0.3686 max=0.3686 sd=0.0000 |
| 20000 | mature | n=2869 distinct=1498 min=0.0000 med=0.0414 max=1.0000 sd=0.1320 | n=2869 distinct=1 min=0.1373 med=0.1373 max=0.1373 sd=0.0000 |

### Noise strip, seed 1

| tick | stage | published crown_light | sampled light.bin |
|---|---|---|---|
| 0 | young | n=12 distinct=1 min=1.0000 med=1.0000 max=1.0000 sd=0.0000 | n=12 distinct=1 min=0.3686 med=0.3686 max=0.3686 sd=0.0000 |
| 10000 | sapling | n=180 distinct=30 min=0.0000 med=0.0001 max=1.0000 sd=0.3651 | n=180 distinct=1 min=1.0000 med=1.0000 max=1.0000 sd=0.0000 |
| 10000 | young | n=136 distinct=54 min=0.0000 med=0.0001 max=1.0000 sd=0.2268 | n=136 distinct=1 min=0.3686 med=0.3686 max=0.3686 sd=0.0000 |
| 10000 | mature | n=701 distinct=404 min=0.0000 med=0.0126 max=1.0000 sd=0.1293 | n=701 distinct=1 min=0.1373 med=0.1373 max=0.1373 sd=0.0000 |
| 20000 | sapling | n=251 distinct=21 min=0.0000 med=0.0000 max=1.0000 sd=0.1545 | n=251 distinct=1 min=1.0000 med=1.0000 max=1.0000 sd=0.0000 |
| 20000 | young | n=7 distinct=7 min=0.0000 med=0.0106 max=1.0000 sd=0.4216 | n=7 distinct=1 min=0.3686 med=0.3686 max=0.3686 sd=0.0000 |
| 20000 | mature | n=940 distinct=470 min=0.0000 med=0.0109 max=0.9518 sd=0.1052 | n=940 distinct=1 min=0.1373 med=0.1373 max=0.1373 sd=0.0000 |

The row's claim reproduces exactly: **one distinct sampled value per stage, sd 0.0000, on both
worlds at every tick**, 0.1373 / 0.3686 / 1.0000. Against it the published field has 1498 distinct
values among the Capitol's 2869 mature trees. The two also disagree about direction, not only about
spread: `light.bin` says every mature tree sits at 0.1373, while the crowns say the median mature
tree is at 0.0414 (three times darker) and the best-lit one is in full sun (seven times brighter).
A reader colouring trees by the old number had no way to tell a specimen on the lawn from a stem in
the middle of the wood; they were the same byte.

**Sapling and young are the pair that shows it best.** A sapling has no canopy, so `light.bin` reads
1.0000 for all 1117 of them; their crowns read a median of 0.0003, because a sapling at 20000 is
almost always under something. That is the difference between "this tree casts no shade" and "this
tree is in the shade", which the old field could not express at all.

## The Capitol at 20000 ticks is a closed thicket

| tick | trees | mean crowns over a crown centre | ground m2 | crown area / ground |
|---|---|---|---|---|
| 0 | 79 | 1.08 | 65536 | 0.07 |
| 10000 | 998 | 1.65 | 65536 | 0.29 |
| 20000 | 4082 | 4.87 | 65536 | 2.66 |

4082 stems on 6.55 ha is **623 stems per hectare**, and their crowns cover 2.66 times the site. A
mature closed temperate stand is in the low hundreds of stems per hectare with a crown cover of about
1. So the reference run ends roughly four times too dense, and the published light is the first number
in the run directory that says so: the median mature crown is at 4% of full sun.

The cause is the one this shot deliberately did not fix. The ecology's shade is the 3×3 stamp, so a
tree's light never falls because of a neighbour 4 m away; its only density feedback is the Chebyshev-2
trunk count above, which saturates once the trunks are 1 m apart no matter how wide the crowns are.
**The follow-up this argues for is the other half of the row's "sample or publish": make the tree tier
read `crown_light` — germination, growth and drought mortality — and retune.** It is a real ecology
change with a tuning pass behind it, which is why it is not in this shot, and it now has a published
diagnostic to be judged against.

## The sweep: `tree.crown_radius_frac` at 0.15, 0.30, 0.60, 1.20

```
ecosim sweep --param tree.crown_radius_frac --values 0.15,0.30,0.60,1.20 --seeds 1,2,3 \
    --ticks 20000 --jobs 6 --out target/s3sweep
```

12 of 12 cells pass, 150.7 s. The safe band is the whole grid on every invariant, and that is not the
interesting part: **every column of `sweep.csv` is byte-identical across all four radii on each seed**
— every invariant's value and margin, the peak counts, the predator-prey lag and correlation. The
sweep is reporting the absence of a mechanism rather than the shape of one, which is the result this
shot wants from it. The same four radii on the Capitol give `trees=4082` at tick 20000 in all four,
and `ecosim diff` between any two reports `meta.json` and the snapshots' `entities.json` and nothing
else.

What the radius does move is the number being published. Capitol, seed 42, tick 20000, all 4082 trees:

| `crown_radius_frac` | trees | distinct values | median | median of mature | in full sun | mean |
|---|---|---|---|---|---|---|
| 0.15 | 4082 | 2413 | 0.6230 | 0.6446 | 597 | 0.5885 |
| 0.30 (default) | 4082 | 1527 | 0.0173 | 0.0414 | 26 | 0.0766 |
| 0.60 | 4082 | 186 | 0.0000 | 0.0000 | 0 | 0.0022 |
| 1.20 | 4082 | 12 | 0.0000 | 0.0000 | 0 | 0.0000 |

At twice the surveyed radius the field saturates — everything is dark and the distinct-value count
collapses from 1527 to 186 — which is a second reason not to guess the constant: **the measured 0.30
is close to the edge of where this diagnostic still carries information at this stand density.** Half
the surveyed radius, 0.15, leaves 597 trees in full sun on a site whose canopy touches everywhere; too
bright to be believed. The survey median is the value with a reason behind it, and it lands where the
distribution is widest.

## The building term

Raising `bundle.sun_altitude_deg` from its default 45° to 89° on the Capitol at tick 0 changes the
published light of **exactly one** of the 79 imported trees, 0.8482 → 0.8752. The other 78 never touch
a roof's shadow, and in a noise world the term is identically 1 because there are no buildings. So
buildings are a real but small correction on this site, and the spread in the tables above is
neighbouring crowns almost entirely. Of the 79 trees at tick 0, 46 are in full sun and the dimmest is
at 0.6200 — the site's planting already has trees close enough together to shade each other, before
the simulator grows a single new one.

## Cost

| measurement | before | after |
|---|---|---|
| seed 1, 20000 ticks, 201 snapshots (release, quiet machine, controlled pair) | 49.70 s | 50.40 s (+1.4%) |
| Capitol seed 42, 20000 ticks, 201 snapshots | — | 27.36 s (`check` runtime margin +0.6960) |
| Capitol seed 42, 20000 ticks, 3 snapshots | — | 22.28 s |

The before column is a binary built from the same tree with `src/` and `params.toml` stashed. The
crown pass is per snapshot, not per tick, and it is bounded by a box of side `2(r + r_max) + 1`
columns around each trunk, so the dense Capitol with 4082 trees and 201 snapshots is the worst case in
the project: 201 snapshots cost it 5.1 s over the same run written at 3, and the crown pass is part of
that 5.1 s rather than all of it.

## Reported, not fixed

Both are in `ecoview-native/`, which an `ecosim` shot does not edit.

1. **`src/tree.rs` calls its crown constants means when they are medians.** `CROWN_RADIUS_FRACTION`
   = 0.30 and `CROWN_BASE_FRACTION` = 0.37 are documented as "the mean of that ratio" over the
   surveyed trees. Recomputed from `worlds/capitol/trees.json` (n=81): the means are **0.3287** and
   **0.3851**, the medians **0.3040** and **0.3684**. The values are right and the word is wrong. Now
   that `params.toml` carries both, the viewer should read them from `meta.json` rather than hold a
   copy — which is the same complaint as row S4, one file over.
2. **Two comments say `params.bundle` reaches `meta.json` only when it is non-default** —
   `src/tree.rs:49` and `src/run.rs:141`, both in bold. Shot S2 removed that `skip_serializing_if`;
   every run has written every params section since. The fallback they describe is no longer
   reachable, and `tree.rs`'s HUD line about it can now only ever report the run's own values.
