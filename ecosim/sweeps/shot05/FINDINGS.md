# Shot 05 sweep findings: extinction attribution

These sweeps ran on this shot's `params.toml`. The only change from shot 5 is `hunter.immigration_floor` 8 → 0 (`DECISIONS.md`, "Extinction attribution"). The commands and wall times are in `_log.txt`. Each `<name>/sweep.md` holds the band report and the new "Extinctions by cause" section. `sweep.csv` holds per-cell margins plus `first_extinction_tick`, `first_extinction_species` and `first_extinction_dominant_cause`. The `cells/` series are gitignored and come back when you rerun the sweep; since this shot they carry per-tick death counts by cause.

How causes are read:
- **Window cause.** This is what `ecosim stats` and the sweep report: the dominant cause over the 500 ticks ending at the extinction tick.
- **Run totals.** These are the per-cause columns of the cell series summed over all 20000 ticks.

The two can disagree, and for hunters they do (see below).

## `hunter.refugium_k` 0.5:4.0:0.5, seeds 1–3

| k | cells passing | extinctions (seed @ tick, window cause) | hunter deaths over the run, starved / old age (s1, s2, s3) |
|---|---|---|---|
| 0.5 | 3/3 | — | 0/90, 0/82, 0/75 |
| 1.0 | 3/3 | — | |
| 1.5 | 3/3 | — | |
| 2.0 (default) | 3/3 | — | 3/87, 2/82, 0/75 |
| 2.5 | 3/3 | — | 17/78, 27/54, 26/58 |
| 3.0 | 1/3 | s2 @18109 `old_age`; s3 @18355 `starved` | 45/42, 45/20, 39/30 |
| 3.5 | 0/3 | s1 @16052, s2 @13363, s3 @12246, all `starved` | |
| 4.0 | 0/3 | s1 @11289, s2 @6832, s3 @7644, all `starved` | |

(Blank cells weren't tallied; the cell series hold them.)

- **Safe band: [0.5, 2.5]**, 5 of 8 values. The band edge is `no_extinction`, and every failing cell fails it. At 4.0, `animals_10k` also fails on all three seeds: fewer than 2 hunters at tick 10000.
- **The band is one step narrower than shot 5's [0.5, 3.0]. That difference comes from the floor, not from any rule change.**
  - At k 3.0 the floor used to supply 6–11 immigrants per run.
  - With the floor at 0, seeds 2 and 3 go extinct at 18109 and 18355. Those are the same ticks shot 5's `extra_immigration_floor_at_k3` recorded for floor 0.
- **Every extinction is hunters first.** Grazers never reach 0 in any cell; 8 of 24 cells have an extinction, and all 8 fail `no_extinction`. Every invariant failure in this sweep is an extinction: no failing cell lacks one.
- **Window cause: hunters `starved` in 7 cells and `old_age` in 1** (k 3.0, seed 2).
  - The window holds only the last 1–4 hunter deaths, so it names what killed the last few animals.
  - Seed 2 at k 3.0 is the one exception. Its last hunter died of old age, but over the run that cell had 45 starvation deaths against 20 old-age deaths.
  - Mean grazers over the window were 730–1833. Hunters starve beside abundant prey, which is the refugium-limited attack budget described in `sweeps/shot5/FINDINGS.md`, not a prey crash.
- **Surviving and collapsing cells form contiguous regions.** In the k × seed grid, every seed's collapsing cells are an unbroken upper tail:
  - seed 1: k ≥ 3.5, with k 3.0 surviving on a minimum of 1 hunter
  - seeds 2 and 3: k ≥ 3.0

  Within each seed, the collapse comes earlier as k rises:
  - seed 1: 16052 → 11289
  - seed 2: 18109 → 13363 → 6832
  - seed 3: 18355 → 12246 → 7644

  No surviving cell sits above a collapsing one for the same seed.

## What kills hunters at the defaults

**This answers the question the shot-5 hunter-regulation finding left open: old age binds, not starvation.**
- At k 2.0 (default), hunters die of old age in 75–87 of 75–90 deaths per seed. Starvation accounts for 0–3, and nothing eats hunters.
- The same holds on seed 42 (`runs/s42`): 88 old age, 3 starved.

So the hunter count is set by `max_age` and `cooldown`, as `DECISIONS.md` inferred, and prey availability plays almost no part.

The transition is visible across the sweep. Starvation's share of hunter deaths rises with k:
- about 0–3% at k ≤ 2.0
- 18–33% at 2.5
- 52–69% at 3.0

The refugium is the lever that turns hunters from age-limited to food-limited. The collapse sits just past the point where food limitation takes over.

**Grazers die mostly of starvation in every cell.**
- At the defaults, 13238–15246 grazer deaths per seed are starvation (76–78%), 3576–4024 are predation (18–23%), and 139–717 are old age.
- Predation's share falls from 20–24% at k 0.5 to 14–17% at k 3.0.

## Amplitude-0 cycle test (`cycle_ratio.py`, `season_amplitude/`)

`python sweeps/cycle_ratio.py sweeps/shot05/season_amplitude`, at `season.amplitude` 0 and 15 on seeds 1–3:

| amplitude | seed | peaks > 4000 | qualifying (≥ 1.5) | ratio min/median/max | answer | hunters at qualifying peaks |
|---|---|---:|---:|---|---|---|
| 0 | 1 | 10 | 3 | 1.12/1.28/1.74 | YES | 48, 43, 42 |
| 0 | 2 | 9 | 1 | 1.18/1.29/1.67 | NO | 46 |
| 0 | 3 | 8 | 2 | 1.15/1.28/1.73 | YES | 42, 38 |
| 15 (default) | 1 | 7 | 4 | 1.08/1.72/2.28 | YES | 42, 46, 48, 45 |
| 15 (default) | 2 | 10 | 3 | 1.00/1.12/1.87 | YES | 46, 44, 45 |
| 15 (default) | 3 | 8 | 3 | 1.09/1.35/2.17 | YES | 43, 43, 43 |

- **The table is identical to shot 5's**, as it should be: the floor never fired at the defaults, so these series didn't change except for the new columns.
- **At amplitude 0 the cycle test holds on seeds 1 and 3, not on seed 2.** No cell has an extinction.
- **All three amplitude-0 cells fail `fertility_band`** (worst margin −0.080), as in shot 5. It is an extinction-free failure, and the new section of `sweep.md` reports it separately.
- **Hunter deaths at amplitude 0 are also old age**: 90/80/73 old age against 5/3/4 starved. Without seasons the grazer oscillation is still grazers against grass. Hunters neither drive it nor die from its troughs.
