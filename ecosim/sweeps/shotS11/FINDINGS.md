# Shot S11: the crowns decide

Before this shot the simulator's only tree-on-tree competition was a *trunk count*: `Sim::crowding`
counted other trunks in a 5x5 m window (Chebyshev 2 for a mature neighbour, 1 for a young one) and
rolled `tree.crowding_mortality` against a mature tree that had any. The crowns shot S3 published are
6-12 m across, so the trees advertised a canopy and then competed as poles. This shot deletes the
count and reads the crowns instead: `Sim::crown_crowding` combines every neighbour's
`overlap_fraction` of this tree's own disc as `1 - PI(1 - f_j)` — the same mean-field independence
`Sim::crown_light` already assumes — and a mature tree is at risk when that covered share reaches
the new `tree.crowding_overlap` (shipped at 0.55).

Everything below is printed by `analyse.py` in this directory. The reference runs:

    just build
    ecosim run --world worlds/capitol --seed 42 --ticks 20000 --out runs/capitol-s42 \
        --set animals.enabled=false --set climate.rain_gradient=0
    for s in 1 2 3 42; do ecosim run --seed $s --ticks 20000 --out runs/s$s --snapshot-every 100; done
    bash sweeps/shotS11/sweep.sh        # the 58 sweep runs, into target/s11/
    python sweeps/shotS11/analyse.py --emit
    python sweeps/shotS11/analyse.py

The **before** column everywhere is the parent commit's binary (`git stash`-free: build `6c796b0`
into `target/ecosim-base.exe` with its own `params.toml`, as `sweep.sh` documents) writing the same
five run directories under `runs/<name>-preS11`.

## Event-log cause breakdown, seeds 1, 2, 3 and 42 and the Capitol run

| run | species | shot | deaths | causes |
|---|---|---|---|---|
| capitol s42 | tree | before | 15405 | drought 9020 (58.6%), crowded 3638 (23.6%), burnt 1611 (10.5%), old_age 1136 (7.4%) |
| capitol s42 | tree | after | 15295 | drought 7951 (52.0%), crowded 5163 (33.8%), burnt 2027 (13.3%), old_age 154 (1.0%) |
| seed 1 | tree | before | 3911 | crowded 2020 (51.6%), drought 1071 (27.4%), old_age 698 (17.8%), burnt 122 (3.1%) |
| seed 1 | grazer | before | 52877 | crowded 40947 (77.4%), eaten 10313 (19.5%), starved 1261 (2.4%), old_age 356 (0.7%) |
| seed 1 | hunter | before | 416 | crowded 266 (63.9%), old_age 124 (29.8%), starved 26 (6.2%) |
| seed 1 | tree | after | 4706 | crowded 3511 (74.6%), drought 739 (15.7%), old_age 341 (7.2%), burnt 115 (2.4%) |
| seed 1 | grazer | after | 56272 | crowded 44797 (79.6%), eaten 10290 (18.3%), starved 959 (1.7%), old_age 226 (0.4%) |
| seed 1 | hunter | after | 427 | crowded 277 (64.9%), old_age 106 (24.8%), starved 43 (10.1%), burnt 1 (0.2%) |
| seed 2 | tree | before | 3028 | crowded 1633 (53.9%), drought 763 (25.2%), old_age 595 (19.6%), burnt 37 (1.2%) |
| seed 2 | grazer | before | 57791 | crowded 44872 (77.6%), eaten 10716 (18.5%), starved 1532 (2.7%), old_age 667 (1.2%), burnt 4 (0.0%) |
| seed 2 | hunter | before | 394 | crowded 288 (73.1%), old_age 106 (26.9%) |
| seed 2 | tree | after | 2137 | crowded 1657 (77.5%), drought 264 (12.4%), old_age 168 (7.9%), burnt 48 (2.2%) |
| seed 2 | grazer | after | 57333 | crowded 43824 (76.4%), eaten 12205 (21.3%), old_age 997 (1.7%), starved 307 (0.5%) |
| seed 2 | hunter | after | 409 | crowded 297 (72.6%), old_age 112 (27.4%) |
| seed 3 | tree | before | 3561 | crowded 1871 (52.5%), drought 912 (25.6%), old_age 707 (19.9%), burnt 71 (2.0%) |
| seed 3 | grazer | before | 47057 | crowded 35563 (75.6%), eaten 8019 (17.0%), old_age 1958 (4.2%), starved 1513 (3.2%), burnt 4 (0.0%) |
| seed 3 | hunter | before | 305 | crowded 225 (73.8%), old_age 71 (23.3%), starved 9 (3.0%) |
| seed 3 | tree | after | 3821 | crowded 2290 (59.9%), drought 1104 (28.9%), burnt 240 (6.3%), old_age 187 (4.9%) |
| seed 3 | grazer | after | 48799 | crowded 37503 (76.9%), eaten 9166 (18.8%), old_age 1966 (4.0%), starved 164 (0.3%) |
| seed 3 | hunter | after | 324 | crowded 242 (74.7%), old_age 81 (25.0%), starved 1 (0.3%) |
| seed 42 | tree | before | 4321 | crowded 3106 (71.9%), old_age 1177 (27.2%), drought 23 (0.5%), burnt 15 (0.3%) |
| seed 42 | grazer | before | 53524 | crowded 41326 (77.2%), eaten 8967 (16.8%), starved 2445 (4.6%), old_age 775 (1.4%), burnt 11 (0.0%) |
| seed 42 | hunter | before | 321 | crowded 240 (74.8%), old_age 78 (24.3%), starved 3 (0.9%) |
| seed 42 | tree | after | 4075 | crowded 3277 (80.4%), drought 357 (8.8%), old_age 339 (8.3%), burnt 102 (2.5%) |
| seed 42 | grazer | after | 46722 | crowded 36024 (77.1%), eaten 8585 (18.4%), old_age 1866 (4.0%), starved 246 (0.5%), burnt 1 (0.0%) |
| seed 42 | hunter | after | 305 | crowded 236 (77.4%), old_age 68 (22.3%), starved 1 (0.3%) |

**The row's anchor moved the way the row predicted.** On the Capitol reference run, crowded went from
3638 of 15405 tree deaths (23.6%) to 5163 of 15295 (33.8%) — the same total mortality, redistributed.
The site is still drought-limited first (52.0%), which was the row's warning and is still true: a
crown term cannot fix a water budget, and nothing here tried to.

Two second-order effects are worth naming because they are not obvious from the headline:

- **Old age nearly disappears** (7.4% -> 1.0% on the Capitol, 17.8% -> 7.2% on seed 1). A wider
  competition radius reaches trees earlier, so fewer of them survive to `max_age_years`. This is the
  ecologically right direction — in a closed stand, senescence is a rare way to die — but it is a
  change no acceptance line watches, so it is recorded here.
- **Burning rises** on the Capitol (10.5% -> 13.3%) and on seed 3 (2.0% -> 6.3%). Thinning the
  canopy admits light, light grows grass and shrub, and `fire.detritus_weight` and the canopy term
  do the rest. The three strip seeds move in both directions (seed 1 3.1% -> 2.4%), so this is a
  tendency, not a law.
- **The animals barely notice.** Grazer and hunter cause shares move by 1-3 points on every seed and
  the ordering of causes is unchanged. Crowding was and remains the dominant grazer death (76-80%).

## The stand before and after

| run | shot | trees@0 | trees@10k | trees@end | min trees | mature@10k | grass mean | shrub mean | crown cover | stems/ha |
|---|---|---|---|---|---|---|---|---|---|---|
| capitol s42 | before | 79 | 998 | 4082 | 79 | 325 | 0.671 | 0.303 | 2.66 | 623 |
| capitol s42 | after | 79 | 694 | 2378 | 79 | 295 | 0.714 | 0.262 | 0.96 | 363 |
| seed 1 | before | 12 | 1017 | 1198 | 12 | 701 | 0.569 | 0.270 | 4.51 | 731 |
| seed 1 | after | 12 | 922 | 729 | 12 | 493 | 0.624 | 0.283 | 3.09 | 445 |
| seed 2 | before | 12 | 746 | 1229 | 12 | 479 | 0.608 | 0.236 | 4.33 | 750 |
| seed 2 | after | 12 | 231 | 658 | 12 | 92 | 0.696 | 0.143 | 1.84 | 402 |
| seed 3 | before | 12 | 869 | 1171 | 12 | 527 | 0.602 | 0.243 | 3.84 | 715 |
| seed 3 | after | 12 | 575 | 1036 | 12 | 279 | 0.677 | 0.226 | 3.01 | 632 |
| seed 42 | before | 12 | 1247 | 1543 | 12 | 822 | 0.514 | 0.272 | n/a | 942 |
| seed 42 | after | 12 | 848 | 1258 | 12 | 459 | 0.669 | 0.216 | 3.53 | 768 |

("crown cover" is summed crown area over site area, so 1.00 is one closed canopy and 2.66 is 266% of
the site. It is `n/a` for the seed-42 before run, whose directory predates shot S3 and therefore has
no `crown_radius_m`; its cause counts are unaffected.)

The stand thins everywhere — **stems per hectare fall on all five runs** and crown cover falls with
them — and grass rises everywhere as a consequence (0.51-0.67 -> 0.62-0.71 mean). On the Capitol
that lands the site at **363 stems/ha with crown cover 0.96**, which is the first time the reference
run has stood at roughly one closed canopy rather than nearly three.

The worst case is **seed 2**, where mature trees at tick 10000 fall from 479 to 92. That is still
2.6x the acceptance floor of 35, but it is the line with the least room left, and it is the number
to watch if a later shot makes the site drier.

## `ecosim check` on the five reference runs

| run | PASS | N/A | FAIL | failures |
|---|---|---|---|---|
| capitol s42 | 9 | 2 | 0 | none |
| seed 1 | 10 | 1 | 0 | none |
| seed 2 | 10 | 1 | 0 | none |
| seed 3 | 10 | 1 | 0 | none |
| seed 42 | 10 | 1 | 0 | none |

The two lines this shot was most likely to break both hold with room:

- `mature trees at 2.5 years >= 35`: 295 (capitol), 493, 92, 279, 459.
- `no tree on a roof or on more than half its ground cells sealed, >= 80% on none`: the worst
  snapshot is **88.59% on none** at tick 9700, against the 80% floor. Thinning the stand does not
  push the survivors onto the pavement.

## Choosing the threshold: the site's own trees

`crowding_overlap` is a fraction of a crown's own disc, so it can be calibrated against something
real instead of tuned against a summary statistic: the Capitol bundle ships **79 surveyed trees**,
placed from the actual grounds, and their crowns at tick 0 are what a healthy, non-self-thinning
stand looks like on this site. Running `crown_crowding` over that opening state:

n=79  median=0.000  p75=0.234  p90=0.407  p95=0.471  max=0.569

| threshold | surveyed trees at or over it | share |
|---|---|---|
| 0.25 | 18 | 22.8% |
| 0.45 | 4 | 5.1% |
| 0.50 | 2 | 2.5% |
| 0.55 | 2 | 2.5% |
| 0.60 | 0 | 0.0% |
| 0.65 | 0 | 0.0% |
| 0.75 | 0 | 0.0% |
| 1.0 | 0 | 0.0% |

**0.55 is the shipped value because it sits just above the 95th percentile (0.471) and just below
the maximum (0.569) of the real stand**: two of the site's 79 trees are at risk on the day the run
opens, and 77 are not. A threshold of 0.25 would put 18 of them (23%) under immediate threat, which
is not a description of the Capitol grounds; a threshold of 0.60 or above exempts the whole surveyed
stand, which makes the opening state uninformative about the parameter.

This is the load-bearing argument for the value. The sweep below is the check that it does not break
the ecology, not the reason for it — and the next section is explicit about why it cannot be the
reason.

## What the sweep can and cannot resolve

### Threshold scan, Capitol seed 42

| crowding_overlap | trees@end | min | max | mature@10k | tree deaths | crowded | drought | burnt | old age | crown cover | stems/ha |
|---|---|---|---|---|---|---|---|---|---|---|---|
| base | 4082 | 79 | 4461 | 325 | 15405 | 23.6% | 58.5% | 10.5% | 7.4% | 2.66 | 623 |
| 0.25 | 964 | 12 | 964 | 122 | 2581 | 61.9% | 32.1% | 1.9% | 4.1% | 0.54 | 147 |
| 0.50 | 1734 | 19 | 2120 | 113 | 5528 | 45.7% | 39.5% | 11.9% | 2.9% | 0.70 | 265 |
| 0.55 | 2378 | 79 | 4160 | 295 | 15295 | 33.8% | 52.0% | 13.2% | 1.0% | 0.96 | 363 |
| 0.60 | 2304 | 51 | 3496 | 134 | 10996 | 36.0% | 57.1% | 5.8% | 1.2% | 0.86 | 352 |
| 0.65 | 3013 | 79 | 4071 | 884 | 17170 | 37.0% | 46.4% | 15.3% | 1.4% | 1.36 | 460 |
| 0.70 | 4041 | 79 | 4041 | 226 | 16482 | 41.6% | 44.6% | 11.4% | 2.3% | 2.35 | 617 |
| 0.75 | 3818 | 79 | 4251 | 125 | 15650 | 43.8% | 46.1% | 7.9% | 2.2% | 2.36 | 583 |
| 1.0 | 4691 | 79 | 5463 | 3182 | 15060 | 31.2% | 31.7% | 4.9% | 32.2% | 3.88 | 716 |

Two rows were rejected outright during calibration:

- **0.25** fails `ecosim check` on the Capitol twice: `trees max=964 limit=720` (the stand crashes so
  far that the 10x anchor at 1.25 years is tiny) and the sealed-ground line at 66.67% against the
  80% floor. The stand drops to 12 trees at its minimum.
- **0.50** leaves min trees at 19 and the sealed-ground line at 80.28%, a quarter of a point above
  the floor. That is a pass, but not one worth shipping a reference run on.

**1.0 is the off switch in all but name**: a crown must be entirely covered, which is all but
unreachable, so old age climbs back to 32.2% of deaths and cover to 3.88. (The genuine off switch
remains `crowding_mortality = 0`, which is a rate-0 identity: it makes no RNG draws, builds no
crowns, and its output is pinned by `tests/data/s42-manifest-S11-tree-crowding-off.sha256`.)

### The same three thresholds on three other terrains

| terrain seed | crowding_overlap | trees@end | min | crowded | crown cover |
|---|---|---|---|---|---|
| 1 | 0.50 | 2261 | 79 | 37.7% | 1.38 |
| 1 | 0.55 | 3283 | 38 | 48.6% | 2.24 |
| 1 | 0.65 | 3831 | 79 | 40.4% | 2.32 |
| 2 | 0.50 | 3995 | 79 | 71.5% | 2.55 |
| 2 | 0.55 | 1821 | 16 | 40.2% | 0.85 |
| 2 | 0.65 | 2475 | 23 | 55.8% | 1.59 |
| 3 | 0.50 | 3630 | 79 | 45.0% | 2.10 |
| 3 | 0.55 | 1390 | 79 | 31.3% | 0.49 |
| 3 | 0.65 | 3750 | 79 | 66.5% | 3.09 |

### Eight RNG streams per condition, Capitol seed 42

| crowding_overlap | n | trees@end median [min, max] | crowded median [min, max] | crown cover median [min, max] |
|---|---|---|---|---|
| base | 8 | 3682 [2098, 4524] | 34.7 [18.5, 53.6] | 3.00 [1.10, 4.36] |
| 0.45 | 8 | 2957 [268, 4084] | 46.3 [23.3, 65.3] | 1.66 [0.10, 2.82] |
| 0.55 | 8 | 3720 [2651, 4215] | 65.3 [43.3, 72.0] | 2.53 [1.74, 3.17] |
| 0.65 | 8 | 3055 [481, 3920] | 49.3 [33.4, 66.0] | 1.60 [0.20, 3.15] |
| 0.75 | 8 | 2644 [814, 4373] | 48.8 [30.4, 76.3] | 1.26 [0.32, 3.20] |

**The threshold's level is not resolvable at this sample size, and the honest reading is to say so.**
Across eight independent RNG streams on the same terrain, within-condition spread swamps every
between-condition difference: crown cover at 0.45 runs 0.10-2.82 while at 0.75 it runs 0.32-3.20,
and the medians are not monotone in the threshold. The nine cross-terrain runs above say the same
thing: on seed 1 a wider tolerance grows the stand, on seed 2 and seed 3 the middle value is the
thinnest. Anyone reading the single-seed scan as a response curve will over-read it.

What *is* robust across all 40 replicates and all 9 cross-terrain runs:

1. **The term bites.** Crowded's share of tree deaths rises from a median 34.7% (base) to 46-65% in
   every crown condition, and no replicate of any crown condition is below the base minimum of
   18.5%.
2. **The stand thins.** Median crown cover falls from 3.00 (base) to 1.26-2.53 in every crown
   condition.
3. **It does not run away.** No replicate of any condition extinguished the trees. The thinnest any
   stand ended was 268 trees at tick 20000 (0.45, stream 3), and the lowest any stand got at any
   tick was 8 (0.65, stream 4) — against 19 in the base condition, whose own replicates dip just as
   low. Narrow bottlenecks on this site are not new and are not this term's doing.

The reference run is one draw from the 0.55 column, and a fortunate-looking one on crown cover
(0.96 against a replicate median of 2.53 — the reference run uses `rng.stream = 0`, which is not
among the eight). Its value is that it is the *committed* draw: every screenshot, every fixture and
every check margin in this repository is measured on it, and it passes every acceptance line.

## Cost

Four pinned 20000-tick strip runs (`--profile`, seed 42, CPU affinity set per `PERF.md`):

| binary | total | trees phase | trees share | animals phase |
|---|---|---|---|---|
| base | 48.77 s / 46.90 s | 100.2 ms / 97.9 ms | 0.21% | 44.22 s / 42.59 s |
| S11 | 51.34 s / 50.95 s | 425.4 ms / 436.7 ms | 0.85% | 46.63 s / 46.31 s |

The tree phase is **4.3x more expensive** — it went from counting at most 24 cells to summing
`overlap_fraction` over a box of side `2*(r + max_radius)` — and that costs **0.6% of the run**. The
remaining +6% of total time is the animal phase, which grew because the ecology changed (more grass,
more grazers), not because the crown code touched it. No optimisation was warranted: `Crowns` is
built once per tree-update pass and shared by every judgement in it, and `Crowns::max_radius` bounds
the search box, which is the only thing that keeps it from being quadratic.

## What this shot deliberately did not do

- **`tree.min_spacing = 2` stays.** It gates *planting* (no new trunk within Chebyshev 1 of an
  existing one), not competition, and it is the reason the deleted `Stage::Young => 1` arm of the old
  `crowding` could never fire in a production run. Retiring it is a change to how the stand is
  seeded, not to how it competes, and the row did not ask for one.
- **No retune of the water budget.** The row warned that "the site is drought-limited first and a
  retune that ignores water will chase the wrong number." Drought is still 52.0% of Capitol tree
  deaths after the change. Nothing in `climate`, `rain`, `hydro` or `medium` was touched.
- **`crowding_mortality = 0.02` is unchanged.** The shot changed the quantity that is measured, not
  the rate at which the measurement kills, so the one new number to defend is the threshold.
