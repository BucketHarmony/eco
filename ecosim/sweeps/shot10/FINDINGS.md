# Shot 10 findings: density-dependent mortality and hunter regulation

Four sweeps on seeds 1–3 at 20 000 ticks. Every other parameter is at its default: `disease.grazer_rate` 0.001, `grazer_threshold` 16, `hunter_rate` 0.001, `hunter_threshold` 4, `hunter.refractory` 2750. The commands and wall times are in `_log.txt`, and each sweep's full table is in its `sweep.md` and `sweep.csv`. The per-cell series are gitignored; rerunning the logged commands regenerates them.

Two scripts read those per-cell series:
- `per_patch.py` writes `per_patch.txt` in each sweep directory. For every cell it gives the grazer peak and mean per patch (the live count ÷ 64, over ticks 2000–20000), the hunter peak and mean, and the run's deaths by cause.
- `../cycle_ratio.py` writes `cycle_ratio.txt`.

## Headline: crowding, not prey, sets hunter numbers at the defaults

The shot wanted hunting success to set hunter numbers, by replacing the fixed 5000-tick cooldown with a short refractory plus the energy gate. That does not happen within the reference anchor.

- **At the prompt's refractory (300), hunters overshoot and eat every grazer.**
  - The `hunter.refractory` sweep (100–1000) fails every cell: grazers are `eaten` to extinction on all 21, at ticks 450–4493.
  - The lower the refractory, the sooner it happens: tick ~500 at 100, tick ~3500 at 1000.
  - After that the hunters starve. Only the gap between a kill and the next birth limits them, and a single kill (40 energy) almost refills a hunter to `repro_energy` (75).
- **With hunter crowding off, the anchor needs a refractory of about 5000.**
  - With `disease.hunter_rate=0`, refractory 2750 and 3500 fail all four anchor seeds. 5000, the old cooldown, passes.
  - So the energy gate never regulates on its own. Only the demographic ceiling does.
- **The default 2750 holds only because hunters crowd.**
  - The anchor rule picked refractory 2750, the smallest value on a 250-tick grid that keeps seeds 1, 2, 3 and 42 passing (see `TUNING.md`). That was at the hunter crowding default (0.001, threshold 4).
  - At that default, `crowded` is 81–86% of all hunter deaths:

    | seed | crowded | old age | starved |
    |---|---|---|---|
    | 1 | 148 | 23 | 1 |
    | 2 | 147 | 31 | 2 |
    | 3 | 161 | 31 | 2 |
    | 42 | 127 | 25 | 5 |

  - Hunters bunch in the patches where prey is. Crowding thins them there, before they strip a patch.
  - Halving the hunter rate to 0.0005 at refractory 2750 already fails 2 of the 4 anchor seeds.
- **What changed compared with shot 5.**
  - Hunter deaths were 97% old age. Now they are 81–86% crowding and 13–16% old age, with starvation still rare (1–5 per run).
  - There is no sign that hunters track prey more closely. At the default, the qualifying grazer peaks carry 34–49 hunters, against 31–59 in shot 5.
  - Hunter numbers are held by crowding in the patches with prey, not by the kill rate.
  - Getting prey-driven hunter numbers would need a knob that makes the energy gate bind: a larger `repro_cost`, a smaller `kill_energy`, or a higher `repro_energy`. Those are outside this shot's mechanism, and `hunter.cooldown` was one of the four anchor values. So this is reported, not tuned.

## `disease.grazer_rate` over 0:0.01:0.001

Grazer peak and mean per patch are averages over seeds 1–3 (ticks 2000–20000, count ÷ 64). The per-seed values are in `per_patch.txt`.

| rate | cells passing | grazers/patch peak | grazers/patch mean | grazer crowded deaths (s1/s2/s3) | extinctions |
|---|---|---|---|---|---|
| 0.000 | 2/3 | 50.0 | 23.2 | 0 / 0 / 0 | s3 grazers @11451, `eaten` |
| 0.001 (default) | 3/3 | 13.7 | 9.4 | 14405 / 15397 / 11311 | — |
| 0.002 | 3/3 | 12.1 | 8.5 | 14581 / 14965 / 11545 | — |
| 0.003 | 2/3 | 11.4 | 7.8 | 12867 / 15419 / 11018 | — |
| 0.004 | 2/3 | 10.8 | 5.9 | 13066 / 14516 / 4152 | s3 grazers @7709, `eaten` |
| 0.005 | 0/3 | 10.2 | 5.1 | 12739 / 11952 / 3735 | s3 grazers @7795, `eaten` |
| 0.006 | 1/3 | 9.8 | 5.1 | 12862 / 11638 / 3799 | s3 grazers @8934, `eaten` |
| 0.007 | 2/3 | 9.6 | 6.5 | 12804 / 11874 / 11621 | — |
| 0.008 | 0/3 | 9.5 | 5.0 | 12553 / 12346 / 4056 | s3 grazers @8411, `eaten` |
| 0.009 | 0/3 | 9.4 | 4.8 | 11670 / 13337 / 3251 | s3 grazers @7958, `eaten` |
| 0.010 | 1/3 | 8.9 | 5.7 | 12658 / 11722 / 7545 | — |

- **Crowding caps grazer peaks hard.**
  - The first step, 0 → 0.001, cuts the peak per patch from 50 to 14 and the mean from 23 to 9.
  - Beyond that, the peak falls only slowly, to 9 at 0.01. The peak settles a few animals above the threshold (16): p rises linearly with the excess, so a higher rate trims the excess but can't push a patch below the threshold.
  - Starvation, which killed 6000–16000 grazers per run at rate 0, falls to a few hundred. Crowding replaces it as the grazers' main death cause: 11 000–15 000 deaths, against 3000–4000 eaten.
- **Extinctions.** All 6 are grazers, with `eaten` as the dominant cause, and 5 of the 6 are on seed 3. In those cells the grazer mean over ticks 2000–20000 is only 1.4–1.9 per patch, and hunters peak at 59–71. The hunters eat the last grazers by tick 7700–9000, then starve.
  - At rate 0 the collapse on seed 3 has a different route. Grazers boom to 48 per patch and crash on their grass, and the hunters take the survivors at tick 11451.
- **Failures without extinction.** All 11 are `fertility_band` over the upper bound, with margins −0.003 to −0.034. A likely cause, not verified here: crowding corpses add detritus, and detritus decays into fertility.
- **Regions.** The safe band is [0.001, 0.002], which the sweep flags as fragile. There is no contiguous region above it.
  - Seed 1 passes 0–0.004 and 0.007, and fails 0.005–0.006 and 0.008–0.01.
  - Seed 2 passes 0–0.002, 0.004 and 0.006.
  - Seed 3 passes 0.001–0.003, 0.007 and 0.010, and collapses at 0, 0.004–0.006, 0.008 and 0.009.
  - Surviving and collapsing cells interleave on every seed above 0.003. Seed 3's collapses form one region with gaps at 0.007 and 0.010, not a clean edge. Above 0.003 the outcome looks timing- and seed-dependent rather than monotone in the rate. That is a reading of the table, not a tested cause.

## `disease.grazer_threshold` over 4:32:4

| threshold | cells passing | grazers/patch peak | grazers/patch mean | extinctions |
|---|---|---|---|---|
| 4 | 0/3 | 6.6 | 2.2 | s2 grazers @7866, s3 grazers @7032, both `eaten` |
| 8 | 2/3 | 10.9 | 7.3 | — |
| 12 | 3/3 | 12.8 | 8.5 | — |
| 16 (default) | 3/3 | 13.7 | 9.4 | — |
| 20 | 3/3 | 15.1 | 11.1 | — |
| 24 | 3/3 | 15.8 | 11.0 | — |
| 28 | 3/3 | 17.7 | 11.2 | — |
| 32 | 3/3 | 17.7 | 12.0 | — |

- The safe band is [12, 32], 6 of 8 values, and it reaches the top of the grid. The surviving cells form one contiguous region from 12 up on every seed, and seeds 2 and 3 already pass at 8.
- Threshold 4 caps grazers below what the hunters need. The same `eaten` collapse as at high rates follows on seeds 2 and 3, and seed 1 fails only `fertility_band`, without an extinction. Threshold 8 fails only seed 1, also on `fertility_band` (−0.011).
- The peak per patch rises with the threshold. At 8–16 it sits near the threshold (10.9, 12.8, 13.7). At 20–32 it stays well below it (15–18), because other limits bind first.

## `hunter.refractory` over 100:1000:150

All 21 cells fail, and every one is a grazer extinction, `eaten`, at ticks 450–4493 (the full list is in `sweep.md`). The grid never reaches the anchor value, 2750. Where the crash comes after tick 2000 (refractory ≥ 400), hunters peak at 53–130 over ticks 2000–20000. Hunter deaths are starvation (69–183) and crowding (39–164), and never old age, because they all die young. The surviving and collapsing regions are trivial: the whole grid is one collapsing region, and survival starts somewhere between 2500 and 2750 (`TUNING.md`).

## Cycle test at season amplitude 0 (`cycle_ratio.py`)

`grazer_rate_amp0` reruns the grazer-rate grid with `--set season.amplitude=0`. `python sweeps/cycle_ratio.py sweeps/shot10/grazer_rate_amp0` output is in `grazer_rate_amp0/cycle_ratio.txt`; the same script on the seasonal sweep is in `disease_grazer_rate/cycle_ratio.txt`.

| rate | amplitude 0: s1 / s2 / s3 (qualifying peaks, answer) | amplitude 15: s1 / s2 / s3 |
|---|---|---|
| 0.000 | 6 YES / 4 YES / 0 NO | 6 YES / 3 YES / 0 NO |
| 0.001 (default) | 1 NO / 0 NO / 2 YES | 2 YES / 2 YES / 2 YES |
| 0.002 | 1 NO / 2 YES / 4 YES | 1 NO / 1 NO / 3 YES |
| 0.003 | 1 NO / 2 YES / 0 NO | 2 YES / 0 NO / 2 YES |
| 0.004 | 2 YES / 1 NO / 2 YES | 1 NO / 1 NO / 0 NO |
| 0.005 | 1 NO / 2 YES / 0 NO | 2 YES / 2 YES / 0 NO |
| 0.006 | 0 NO / 2 YES / 2 YES | 1 NO / 3 YES / 0 NO |
| 0.007 | 2 YES / 1 NO / 2 YES | 2 YES / 0 NO / 2 YES |
| 0.008 | 3 YES / 1 NO / 2 YES | 2 YES / 2 YES / 0 NO |
| 0.009 | 0 NO / 1 NO / 0 NO | 0 NO / 2 YES / 0 NO |
| 0.010 | 1 NO / 2 YES / 0 NO | 2 YES / 3 YES / 2 YES |

- **At the default rate and amplitude 0, the cycle test holds only on seed 3.** Shot 05 had it holding on seeds 1 and 3. Seed 2 has 12 maxima after tick 4000, none with a peak/trough ratio of 1.5, and its median ratio is 1.17.
  - Crowding flattens the grass-driven overshoot that made the unforced cycle. With the peak capped near the threshold, there is little left to crash.
- **At rate 0 (crowding off) and amplitude 0,** the unforced cycle is strong on seeds 1 and 2: 6 and 4 qualifying peaks, with median ratios 1.78 and 1.39. So the oscillation is still there in the model, and crowding is what damps it.
- **At amplitude 15 and the default rate,** all three seeds answer YES, each with exactly 2 qualifying peaks. So seasons, not the grazer–grass overshoot, now make most of the cycle.
- **Every amplitude-0 cell fails an invariant.** In all 33, `fertility_band` fails by −0.135 to −0.156: without a warm-season growth pulse, fertility sits above 220, as in shot 4 and shot 05. There are 7 grazer extinctions, all `eaten`.
- **Hunters are alive at every qualifying peak,** 18–45 of them at amplitude 0.
