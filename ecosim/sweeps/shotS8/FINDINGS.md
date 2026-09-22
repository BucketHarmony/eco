# Shot S8: what the Capitol does after tick 2000, with animals in it

The row asked three things, in order of how sure it was of them. Is the Capitol's grazer level
correct scaling, or a broken brake? Why do its trees peak at 360 near tick 1150 and fall to 24 by
2000? And what happens past 2000, which nobody had ever looked at, because no bundle-world run
with animals had gone further.

**All three answers are in this file and none of them needed a code change.** The shot changes no
default, no invariant and no committed data. It is a measurement.

Short version:

1. **The grazer level is scaling.** The Capitol holds 9.96-12.97 grazers per grassy patch against
   the noise strip's 10.67-12.44, measured with the same parameters. The population is set per
   patch: at tick 10000 the Capitol has 3.5-4.0x the strip's grassy patches and 3.7-4.8x its
   grazers. Nothing is broken.
2. **The tree crash is drought in the establishment year, and it is not caused by the animals.**
   Drought is **91-98% of every tree death before tick 2500** in seven of the eight long runs. The
   crash happens at the same rate with animals off (8 of 16 weather streams) as with them on (9 of
   16). Its severity tracks the establishment-window moisture minimum at **r = -0.757** over 32
   replicates.
3. **It is transient.** The Capitol run reaches 20000 ticks with no extinction and **2470 trees**,
   up from 20 at the trough, with 551 mature trees at tick 10000. Seeds 1, 2 and 3 end at 4674,
   4560 and 3861. The 24 the operator saw was the bottom of a curve, not the end of one.

Two things the shot found that the row did not ask for are in the last two sections: an
animals-on Capitol run **cannot pass `ecosim check`** today, on two invariants, for reasons that
are about the invariants' anchors rather than the ecology; and it takes **314.5 s**, against the
90 s cap and the animals-off run's 31.5 s.

## How to rebuild every number here

Every table below is printed by `analyse.py` in this directory. Nothing is typed in by hand.

```bash
cd ecosim
cargo build --release
# the reference run this shot is about: the Capitol, seed 42, animals ON
./target/release/ecosim run --world worlds/capitol --seed 42 --ticks 20000 \
    --out runs/capitol-s42-animals-20k --snapshot-every 100 --set climate.rain_gradient=0
# the same on seeds 1, 2, 3, and the animals-off sibling of each, at coarser snapshots
for s in 1 2 3; do
  ./target/release/ecosim run --world worlds/capitol --seed $s --ticks 20000 \
      --out runs/s8/seeds/on-s$s  --snapshot-every 2000 --snapshot-state false \
      --set climate.rain_gradient=0
  ./target/release/ecosim run --world worlds/capitol --seed $s --ticks 20000 \
      --out runs/s8/seeds/off-s$s --snapshot-every 2000 --snapshot-state false \
      --set climate.rain_gradient=0 --set animals.enabled=false
done
# the noise strip at today's defaults, for the density comparison
for s in 1 2 3; do
  ./target/release/ecosim run --seed $s --ticks 20000 --out runs/s8/strip-s$s --snapshot-every 100
done
# 40 replicates: one world, one seed, a different weather stream each
for st in $(seq 1 16); do
  ./target/release/ecosim run --world worlds/capitol --seed 42 --ticks 2500 \
      --out runs/s8/replicates/on-st$st  --snapshot-every 2500 --snapshot-state false \
      --set climate.rain_gradient=0 --set rng.stream=$st
  ./target/release/ecosim run --world worlds/capitol --seed 42 --ticks 2500 \
      --out runs/s8/replicates/off-st$st --snapshot-every 2500 --snapshot-state false \
      --set climate.rain_gradient=0 --set rng.stream=$st --set animals.enabled=false
done
for st in $(seq 1 8); do
  ./target/release/ecosim run --world worlds/capitol --seed 42 --ticks 2500 \
      --out runs/s8/replicates/fill-st$st --snapshot-every 2500 --snapshot-state false \
      --set climate.rain_gradient=0 --set rng.stream=$st --set hydro.initial_fill=1.0
done
python sweeps/shotS8/analyse.py            # every table in this file
python sweeps/shotS8/analyse.py --from-csv # the replicate tables from the committed CSV alone
```

`runs/` is gitignored, so the run directories are not committed; `replicates.csv` is, so the
replicate tables can be reprinted without the multi-gigabyte rebuild. The whole set is 40 runs at about
112 s wall clock for the replicates (20 at a time on 24 threads), 5.2 min for the reference run
and 6.0 min for the slowest of the seven seed runs run in parallel.

**`rng.stream` is what makes the replicates a controlled experiment**, and it is worth saying why
the obvious comparison is not one. `animals.enabled=false` skips the animal phase, which draws
from the shared `ChaCha8Rng`, so an animals-off run at the same seed gets **a different weather
sequence** -- different storm ticks, not just different animals. Comparing `capitol-s42-animals`
with `capitol-s42` tells you nothing about animals on its own: in ticks 1000-1500 the first got
36.3 mm of rain and the second 100.3 mm, and that alone would explain everything. The 32
replicates are the answer to that: the same world and seed, 16 independent weather streams per
condition, so the distribution of weather is matched and only the animals differ.

## Event-log cause breakdown, whole runs

The reporting rule asks every sim shot to lead with this. Deaths by cause from `events.csv` over
20000 ticks: four Capitol runs with animals, four without (no animal rows, by construction), and
three noise-strip runs at today's defaults.

| run | seed | species | deaths | causes |
|---|---|---|---|---|
| capitol, animals on | 42 | grazer | 120439 | crowded 83895, eaten 17511, old_age 13074, starved 5957, burnt 2 |
| capitol, animals on | 42 | hunter | 491 | crowded 252, old_age 185, starved 54 |
| capitol, animals on | 42 | tree | 5190 | drought 2103, crowded 1918, old_age 609, burnt 560 |
| capitol, animals on | 1 | grazer | 119700 | crowded 83957, eaten 14886, starved 11254, old_age 9600, burnt 3 |
| capitol, animals on | 1 | hunter | 473 | crowded 307, old_age 158, starved 8 |
| capitol, animals on | 1 | tree | 15085 | crowded 7569, drought 3558, old_age 3246, burnt 712 |
| capitol, animals on | 2 | grazer | 120275 | crowded 85947, eaten 14032, old_age 10831, starved 9465 |
| capitol, animals on | 2 | hunter | 434 | crowded 265, old_age 153, starved 16 |
| capitol, animals on | 2 | tree | 13927 | crowded 6702, drought 4081, old_age 2623, burnt 521 |
| capitol, animals on | 3 | grazer | 127670 | crowded 92146, eaten 18185, old_age 11354, starved 5984, burnt 1 |
| capitol, animals on | 3 | hunter | 538 | crowded 307, old_age 195, starved 36 |
| capitol, animals on | 3 | tree | 7437 | drought 4104, crowded 2386, old_age 509, burnt 438 |
| capitol, animals off | 42 | tree | 15405 | drought 9020, crowded 3638, burnt 1611, old_age 1136 |
| capitol, animals off | 1 | tree | 3070 | drought 1554, crowded 1141, old_age 262, burnt 113 |
| capitol, animals off | 2 | tree | 13253 | crowded 5412, drought 4862, old_age 2189, burnt 790 |
| capitol, animals off | 3 | tree | 10098 | drought 5077, crowded 2958, burnt 1288, old_age 775 |
| strip, animals on | 1 | grazer | 52877 | crowded 40947, eaten 10313, starved 1261, old_age 356 |
| strip, animals on | 1 | hunter | 416 | crowded 266, old_age 124, starved 26 |
| strip, animals on | 1 | tree | 3911 | crowded 2020, drought 1071, old_age 698, burnt 122 |
| strip, animals on | 2 | grazer | 57791 | crowded 44872, eaten 10716, starved 1532, old_age 667, burnt 4 |
| strip, animals on | 2 | hunter | 394 | crowded 288, old_age 106 |
| strip, animals on | 2 | tree | 3028 | crowded 1633, drought 763, old_age 595, burnt 37 |
| strip, animals on | 3 | grazer | 47057 | crowded 35563, eaten 8019, old_age 1958, starved 1513, burnt 4 |
| strip, animals on | 3 | hunter | 305 | crowded 225, old_age 71, starved 9 |
| strip, animals on | 3 | tree | 3561 | crowded 1871, drought 912, old_age 707, burnt 71 |

Read across the grazer rows first: **`crowded` is the largest cause of grazer death on every
world** -- 69.7-72.2% over the Capitol's four runs and 75.6-77.6% over the strip's three. That is
`disease.grazer_rate` acting above `disease.grazer_threshold`, and it is the brake the row
suspected of failing. It is not failing; it is doing the same job on both worlds, and if anything
a slightly smaller share of it on the Capitol.

Total deaths are **not** in proportion to patch count, and it would be easy to read that as a
brake half-working: 119700-127670 on the Capitol against 47057-57791 on the strip is 2.3x, not
4x. The difference is turnover, not population. Per head the Capitol loses **0.63 grazers per
1000 grazer-ticks** against the strip's 0.86-1.16, so its animals live about a third longer --
which is what a lower crowded share means, read the other way round.

The tree rows are the crash, but averaged over 20000 ticks they hide it. The next table does not.

## Tree deaths in the establishment year only (ticks 0-2500)

| run | seed | species | deaths | causes |
|---|---|---|---|---|
| capitol, animals on | 42 | tree | 366 | drought 357, burnt 9 |
| capitol, animals on | 1 | tree | 431 | drought 418, burnt 13 |
| capitol, animals on | 2 | tree | 353 | drought 346, burnt 7 |
| capitol, animals on | 3 | tree | 341 | drought 334, burnt 7 |
| capitol, animals off | 42 | tree | 236 | drought 228, burnt 7, crowded 1 |
| capitol, animals off | 1 | tree | 279 | drought 255, burnt 24 |
| capitol, animals off | 2 | tree | 32 | crowded 14, burnt 13, old_age 5 |
| capitol, animals off | 3 | tree | 486 | drought 473, burnt 13 |
| strip, animals on | 1 | tree | 25 | drought 25 |
| strip, animals on | 2 | tree | 24 | drought 23, crowded 1 |
| strip, animals on | 3 | tree | 12 | drought 12 |

**Drought is 91-98% of every early tree death in seven of the eight Capitol runs.** There is no
crowding to speak of, no old age at all, and fire is single digits. The eighth, animals off on
seed 2, is the run that has no crash: 32 deaths in total and not one of them drought.

The strip's early deaths are all drought too, but there are 12-25 of them, because the strip has
about 60 trees at that point and the Capitol has 300-850. That difference is the whole story of
the next section.

## The 20000-tick runs

| run | seed | trees t1000 | t2000 | t5000 | t10000 | t20000 | grazers t20000 | hunters t20000 |
|---|---|---|---|---|---|---|---|---|
| capitol, animals on | 42 | 337 | 24 | 162 | 892 | 2470 | 8347 | 356 |
| capitol, animals on | 1 | 340 | 313 | 1660 | 3984 | 4674 | 8659 | 283 |
| capitol, animals on | 2 | 320 | 193 | 1499 | 1915 | 4560 | 6472 | 286 |
| capitol, animals on | 3 | 344 | 204 | 1647 | 695 | 3861 | 8726 | 329 |
| capitol, animals off | 42 | 334 | 532 | 2310 | 998 | 4082 | 0 | 0 |
| capitol, animals off | 1 | 195 | 31 | 261 | 359 | 2096 | 0 | 0 |
| capitol, animals off | 2 | 340 | 854 | 2333 | 2756 | 3810 | 0 | 0 |
| capitol, animals off | 3 | 358 | 193 | 695 | 255 | 3692 | 0 | 0 |
| strip, animals on | 1 | 39 | 58 | 417 | 1017 | 1198 | 1366 | 97 |
| strip, animals on | 2 | 40 | 58 | 260 | 746 | 1229 | 2017 | 121 |
| strip, animals on | 3 | 37 | 67 | 288 | 869 | 1171 | 2484 | 72 |

**Seed 42 with animals, the run the row is about, is the worst of the eight and it still ends with
2470 trees.** The row's "24 by 2000" is real and it is the minimum of the series; the recovery to
2470 is 18000 ticks the run had never been allowed to take.

Note the animals-off column too. `capitol, animals off, seed 1` falls from 195 at tick 1000 to
**31 at tick 2000** -- a deeper crash than three of the four animals-on runs, in a run with no
animal in it.

## The establishment-year tree crash, in the same runs

The peak inside the first 2000 ticks, then the trough after it. Measuring it the obvious way --
the peak over [0, 2000] against the minimum over [800, 3000] -- reads a *rising* series as a 65%
crash, by taking its minimum before its peak; two of these eight runs rise monotonically through
year one and an earlier pass of this analysis called both of them crashes. `crash()` in
`analyse.py` carries the corrected definition and the note.

| run | seed | peak | then trough | cohort lost | rain, ticks 0-2000 | min moisture, ticks 200-1500 | min soil water, same window |
|---|---|---|---|---|---|---|---|
| capitol, animals on | 42 | 360 at 1150 | 20 at 2100 | 94% | 221 mm | 40.1 | 15.2 mm |
| capitol, animals on | 1 | 565 at 1550 | 214 at 2100 | 62% | 259 mm | 122.3 | 47.2 mm |
| capitol, animals on | 2 | 494 at 1500 | 193 at 2000 | 61% | 281 mm | 101.1 | 39.2 mm |
| capitol, animals on | 3 | 444 at 1350 | 195 at 1900 | 56% | 261 mm | 102.4 | 39.4 mm |
| capitol, animals off | 42 | 632 at 1850 | 513 at 1950 | 19% | 275 mm | 122.5 | 47.1 mm |
| capitol, animals off | 1 | 288 at 850 | 27 at 1800 | 91% | 222 mm | 14.5 | 5.4 mm |
| capitol, animals off | 2 | 854 at 2000 | 854 at 2000 | 0% | 318 mm | 90.3 | 35.0 mm |
| capitol, animals off | 3 | 461 at 1350 | 125 at 2450 | 73% | 253 mm | 68.5 | 26.7 mm |
| strip, animals on | 1 | 60 at 1500 | 52 at 1850 | 13% | 283 mm | 110.4 | 47.4 mm |
| strip, animals on | 2 | 62 at 1900 | 58 at 1950 | 6% | 240 mm | 97.2 | 43.9 mm |
| strip, animals on | 3 | 67 at 1900 | 67 at 1900 | 0% | 359 mm | 93.0 | 45.4 mm |

**`min moisture` is the column to read across worlds, not `min soil water`.** `moisture_mean` is
the mean over *soil* columns of `255 x min(1, soil/capacity)`, so it is already a fraction of
available water capacity; `soil_water_mm` is millimetres averaged over the *whole* area including
the 35.2% of the Capitol that is asphalt, concrete and roof at a capacity of 0 mm. The Capitol's
soil water reads about 0.65x the strip's for the same wetness, and using it would have made the
Capitol look drier than it is by exactly the sealed fraction.

**The strip never has this crash** -- 0-13% -- and it is not because the strip is wetter; its
establishment-window moisture (93.0-110.4) sits inside the Capitol's range. It is because the
strip has 60 trees there and the Capitol has 300-850. The next table says why the difference is
not merely one of scale.

## Tree stage structure through the crash and the recovery (capitol-s42-animals-20k)

| tick | trees | mature | young | sapling | germinations in the next 100 ticks |
|---|---|---|---|---|---|
| 0 | 79 | 79 | 0 | 0 | 14 |
| 200 | 136 | 79 | 0 | 57 | 34 |
| 400 | 189 | 79 | 0 | 110 | 32 |
| 600 | 245 | 79 | 27 | 139 | 37 |
| 800 | 295 | 79 | 80 | 136 | 31 |
| 1000 | 337 | 79 | 132 | 126 | 26 |
| 1200 | 284 | 51 | 131 | 102 | 10 |
| 1400 | 212 | 27 | 129 | 56 | 1 |
| 1600 | 83 | 11 | 52 | 20 | 4 |
| 1800 | 30 | 10 | 12 | 8 | 3 |
| 2000 | 24 | 8 | 8 | 8 | 1 |
| 2200 | 21 | 12 | 4 | 5 | 1 |
| 2400 | 27 | 13 | 6 | 8 | 7 |
| 2600 | 35 | 15 | 5 | 15 | 8 |
| 2800 | 39 | 19 | 2 | 18 | 1 |
| 3000 | 40 | 20 | 10 | 10 | 1 |
| 4000 | 80 | 40 | 1 | 39 | 17 |
| 6000 | 272 | 131 | 59 | 82 | 41 |
| 10000 | 892 | 551 | 119 | 222 | 78 |
| 20000 | 2470 | 1637 | 58 | 775 | 214 |

**The mature column is the mechanism.** Through tick 1000 every mature tree on the site is one of
the 79 the scene bundle planted -- nothing sown in the run has reached `mature_age_years` (0.25,
so 1000 ticks) yet. Only mature trees seed. So the site's entire seed supply for its first year is
a single cohort of survey trees, standing on the same ground, drinking from the same profile, and
they go down together: 79, 51, 27, 11, 8. Germination follows them exactly -- 34, 32, 37, 31, 26
per 100 ticks, then 10, 1, 4, 3, 1 -- and with no recruitment the saplings already on the ground
are the only thing left to lose.

The recovery is the same mechanism run forwards. Eight survivors mature, seed a little, their
seedlings mature, and the mature count compounds: 8, 12, 13, 15, 19, 20, 40, 131, 551, 1637. It
takes about 4000 ticks to turn the corner and the rest of the run to pay back.

The noise strip cannot do this, because it starts with `tree.initial_count = 12` at
`initial_age_years = 0.125` -- a dozen trees, none of them mature, spread over a world with no
buildings. It never builds a 300-tree first-year cohort under a seed supply of 79, so it has
nothing to lose all at once. **The Capitol's crash is a property of starting from a surveyed
scene, not of the Capitol's size.**

## Replicates: one world, one seed, 16 weather streams, 2500 ticks

The controlled test of "is it the animals?". Same world, same seed 42, `rng.stream` 1-16, so the
weather distribution is matched between conditions and only `animals.enabled` differs.

| condition | runs | crash over 50% | median cohort lost | median min moisture (ticks 200-1500) | median rain (ticks 0-2000) | median drought deaths | median trees at 2500 |
|---|---|---|---|---|---|---|---|
| animals on | 16 | 9 of 16 | 61% | 91.9 | 271 mm | 266 | 286 |
| animals off | 16 | 8 of 16 | 52% | 86.5 | 269 mm | 288 | 427 |
| animals on, initial_fill 1.0 | 8 | 2 of 8 | 11% | 92.5 | 258 mm | 221 | 811 |

**9 of 16 against 8 of 16.** Whatever causes the establishment-year crash, the animals are not it:
the crash appears at the same rate in a run that has no animals in it at all. The medians move a
little in the direction you would expect from n = 16 noise and nothing more.

Pooled over the 32 animals-on and animals-off replicates, the correlation between the
establishment-window moisture minimum and the share of the tree cohort lost is **r = -0.757**.

| establishment-window moisture minimum | runs | crash over 50% | median cohort lost |
|---|---|---|---|
| below 61 of 255 | 10 | 9 | 94% |
| 61 of 255 or more | 22 | 8 | 35% |

The split is at 61 of 255 because that is twice `tree.dry_fraction` (0.12 of available water
capacity, so 30.6 on this scale) -- the threshold the drought clock itself reads -- and
`moisture_mean` is a mean over soil columns, so half of them are drier than it. **Nine of the ten
driest establishment years lose more than half the cohort; only eight of the other twenty-two
do.** That is the drought hypothesis stated as a prediction and passing.

The third row is the same 16-stream design with `hydro.initial_fill = 1.0` instead of the default
0.5, on the first eight streams. **2 of 8 crash, median loss 11%, median trees at 2500 of 811
against 286.** The site starts at half its available water capacity and takes about 2500 ticks to
fill; start it full and most of the crash goes away. That is the last piece: year one is dry
partly because of the weather it draws and partly because the model starts it half empty.

`initial_fill = 1.0` is **not** proposed as a new default and nothing here was retuned. The row
asked for an explanation and this is the measurement that completes it. Whether a garden site
should begin at field capacity, at half of it, or at whatever the season implies is a question
about what the model means by tick 0, and it belongs to a human.

Every replicate, including the per-run numbers behind those medians, is in `replicates.csv` and is
printed by `python analyse.py --from-csv`.

## Grazers per patch, Capitol against strip

| run | tick | patches | patches with grass | grazers | per patch | per grassy patch | median patch | busiest patch |
|---|---|---|---|---|---|---|---|---|
| capitol-s42-animals-20k | 2000 | 1024 | 881 | 9204 | 8.99 | 10.45 | 10 | 95 |
| capitol-s42-animals-20k | 10000 | 1024 | 854 | 11080 | 10.82 | 12.97 | 12 | 124 |
| capitol-s42-animals-20k | 20000 | 1024 | 838 | 8347 | 8.15 | 9.96 | 9 | 126 |
| s8/strip-s1 | 10000 | 256 | 215 | 2295 | 8.96 | 10.67 | 8 | 152 |
| s8/strip-s2 | 10000 | 256 | 231 | 2873 | 11.22 | 12.44 | 9 | 171 |
| s8/strip-s3 | 10000 | 256 | 247 | 2977 | 11.63 | 12.05 | 10 | 113 |

**This is the answer to the row's first question.** At tick 10000 the Capitol carries 12.97
grazers per grassy patch and the strip carries 10.67, 12.44 and 12.05. The median patch holds 12
against 8, 9 and 10, and the busiest holds 124 against 152, 171 and 113. Every one of the
Capitol's figures is inside the strip's range or a step above its top.

Capitol 854 grassy patches and 11080 grazers; strip 215-247 and 2295-2977. Ratios:
**3.5-4.0x the grassy patches** and **3.7-4.8x the grazers**. **The population is proportional to
the grazable area, which is what correct scaling looks like.**

## Why it looked like predation had failed at tick 2000, and had not

The row already knew the kill rate was the same on both worlds and said so. It is, over the whole
run and not just at tick 2000:

| run | window | kills | mean hunters | mean grazers | kills per hunter per 1000 ticks | grazers per hunter |
|---|---|---|---|---|---|---|
| capitol, animals on, seed 42 | 0-2000 | 211 | 24 | 3589 | 4.5 | 152.5 |
| capitol, animals on, seed 42 | 2000-10000 | 3171 | 79 | 10866 | 5.0 | 137.4 |
| capitol, animals on, seed 42 | 10000-20000 | 14125 | 281 | 9647 | 5.0 | 34.4 |
| strip, seed 1 | 0-2000 | 261 | 27 | 1750 | 4.9 | 65.4 |
| strip, seed 1 | 2000-10000 | 3496 | 85 | 2878 | 5.1 | 33.7 |
| strip, seed 1 | 10000-20000 | 6555 | 137 | 1902 | 4.8 | 13.9 |
| strip, seed 2 | 0-2000 | 314 | 30 | 1811 | 5.3 | 61.3 |
| strip, seed 2 | 2000-10000 | 3536 | 83 | 3038 | 5.3 | 36.6 |
| strip, seed 2 | 10000-20000 | 6864 | 129 | 2394 | 5.3 | 18.6 |
| strip, seed 3 | 0-2000 | 280 | 27 | 1825 | 5.2 | 68.3 |
| strip, seed 3 | 2000-10000 | 3190 | 74 | 3044 | 5.4 | 41.3 |
| strip, seed 3 | 10000-20000 | 4548 | 86 | 2667 | 5.3 | 31.2 |

A hunter takes 4.5-5.4 grazers per 1000 ticks on every world in every window, because
`hunter.satiation` and not prey density is what limits it. So the per-hunter rate cannot tell the
two worlds apart, and the row was right that it does not.

**What tells them apart is `grazers per hunter`, and the reason is that the start counts are
absolute.** `grazer.start_count = 300` and `hunter.start_count = 20` are whole-world numbers, not
per-patch ones, so a world with 4x the patches begins 4x emptier of both. The grazer fills its
space in a few hundred ticks -- it reproduces at `repro_energy = 70` with a 300-tick cooldown --
and the hunter, at `repro_energy = 75` with a 2750-tick refractory, takes the whole run. At tick
2000 the Capitol is at 152 grazers per hunter against the strip's 61-68: the predator is behind by
about the area ratio, exactly as an absolute start count predicts. By tick 20000 it is 34 against
14-31, and the grazer curve has turned over (11348 at its peak near tick 4500, 8347 at the end).

So "the grazers are at 9204 and still climbing" was a true observation of a predator that had not
caught up yet, and the run was stopped 18000 ticks before it did.

## Two things that do not pass, which the row did not ask about

### `ecosim check` fails on an animals-on Capitol run

```
$ ./target/release/ecosim check runs/capitol-s42-animals-20k
PASS no species reaches 0: min grazers=8273 min hunters=28 min trees=20 [margin +19.0000]
FAIL no species exceeds 10x its anchor count (animals: 0.5 years, trees: 1.25 years): grazers max=11348 limit=92040 hunters max=362 limit=280 trees max=2548 limit=1620 [margin -0.5728]
PASS grazer cycle (2 maxima >= 1500 ticks apart): 7 maxima at [4974, 9274, 10491, 12972, 16656, 17180, 19416], span 14442 [margin +8.6280]
PASS moisture_mean above the wilting point every tick, below field capacity on >= 95% of them: min 56.30 of 255, below capacity on 98.25% of ticks [margin +0.0000]
PASS fertility_mean in [40, 220]: [59.02, 112.30] [margin +0.4756]
PASS grass_mean in [0.05, 0.95]: [0.3883, 0.8349] [margin +0.1212]
PASS trees at end >= 1.5x trees at tick 0: 2470 vs 79 (need 118.5) [margin +19.8439]
FAIL run time < 30 s per 64x64, at most 90 s: 314521 ms [margin -2.4947]
PASS mature trees at 2.5 years >= 35: 551 [margin +14.7429]
PASS at 2.5 years grazers >= 10 and hunters >= 2: grazers=11080 hunters=149 [margin +73.5000]
PASS no tree on a roof or on more than half its ground cells sealed, >= 80% on none: 189827 sightings over 201 snapshots: 0 over half sealed, 0 roof cells, per-tree sealed [179190, 3138, 7499, 0, 0]; worst snapshot 80.00% on none at tick 2100 of 20 trees [margin +0.0000]
```

**Both `max_10x` failures are the anchor landing inside a transient, not a runaway.** The
invariant takes a species' count at a fixed early tick and asks that it never exceed ten times it:
animals at 0.5 years (tick 2000), trees at 1.25 years (tick 5000). On this run the tree anchor is
162, because tick 5000 is still in the trough, and the run's honest final value of 2470 is 15.2x it and its maximum of 2548 is 15.7x.
The hunter anchor is 28, because at tick 2000 the hunter has barely started its numerical
response, and its maximum of 362 is 12.9x it. The grazer, whose anchor is taken after its overshoot,
passes with three orders of margin.

The assumption the invariant encodes -- that the anchor tick is past the initial transient -- holds
on the noise strip and does not hold on a bundle world with animals, where the transient is 15000
ticks long. **Nothing was changed.** Widening an invariant to pass is forbidden, the row did not
ask for it, and the right fix is a judgement about what the anchor is for. `PERF.md` records that
the animals-on 256x256 noise world failed the same invariant in shots 15a and G0, so this is the
third sighting of the same shape.

### It takes 314.5 s, against a 90 s cap

The animals-off Capitol reference run takes 31.5 s. Adding animals makes it **10x slower**, and
`--profile` says the animal phase is **89.6% of the 314.5 s**. That is the cost of 8000-11000
grazers stepping every tick on a world that carries four times the strip's population, and it is
the ranked finding `PERF.md` already has: the model's cost is set by the grazer count, not by the
terrain size. The measurement is added to `PERF.md` under "The Capitol with animals (shot S8)".

**This is why `just capitol` runs with `animals.enabled=false`, and it should stay that way.** An
animals-on Capitol run is a thing to do deliberately, not a thing to put in CI.
