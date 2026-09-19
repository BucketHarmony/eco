# Shot 5 sweep findings: dynamics fixes

The seven sweeps below ran on the final `params.toml` (round 4 in `TUNING.md`, "Dynamics fixes"). Each ran seeds 1, 2 and 3 for 20000 ticks, using `ecosim sweep --jobs 22`. Each `<name>/sweep.md` holds the band report and `sweep.csv` holds per-cell margins. The `cells/` series are gitignored and come back when you rerun the sweep. The command is in `sweep.md`, and `_log.txt` has the wall times: 195 s in total.

## Bands

A value is safe when every 20000-tick invariant passes on all three seeds. Runtime is excluded.

| # | sweep | grid | default | safe band | values | fragile | limiting rule at the edge |
|---|---|---|---|---|---:|---|---|
| 1 | `hunter_refugium_k` | 0.5:4.0:0.5 | 2.0 | [0.5, 3.0] | 6 of 8 | no | `no_extinction` at 3.5 (seeds 1, 3): hunters starve |
| 2 | `hunter_kill_prob` | 0.1:0.5:0.1 | 0.3 | [0.1, 0.5] | 5 of 5 | no | none: passes the whole grid |
| 3 | `grazer_energy_cost` | 0.04:0.16:0.02 | 0.10 | [0.04, 0.16] | 7 of 7 | no | none: passes the whole grid |
| 4 | `tree_mature_age` | 500:2500:500 | 1000 | [500, 2500] | 5 of 5 | no | none: passes the whole grid |
| 5 | `season_amplitude` | 0:18:3 | 15 | [12, 18] | 3 of 7 | no | `fertility_band` at 9 (all seeds, margin −0.026) |
| 6 | `hunter_immigration_floor` | 0:16:4 | 8 | [0, 16] | 5 of 5 | reported only | none: see below |
| 7 | `tree_crowding_mortality` | 0:0.05:0.01 | 0.02 | [0.00, 0.05] | 6 of 6 | reported only | none: passes the whole grid |

All five required bands have at least 3 values.

**Hunter-extinction cells: 5 of 129 (3.9%).** A cell counts when hunters reach 0 at any tick. All five are in sweep 1:
- at `refugium_k` 3.5: seeds 1 and 3
- at `refugium_k` 4.0: seeds 1, 2 and 3

No other sweep has one. The earlier rounds are in `TUNING.md`: round 1 had 6 of 129, and rounds 2 and 3 had 3 of 129 each.

## Seeds and long runs

These results are for the final `params.toml`:
- `ecosim check` passes every line at 20000 ticks on seeds 1, 2, 3 and 42. That includes the addendum's s42 extras, such as at least 35 mature trees at tick 10000.
- `ecosim check --long` at 60000 ticks (`--snapshot-every 10000`):

  | seed | min grazers / hunters / trees (whole run) | grazers 20k–60k vs band | hunters 20k–60k vs band | margin |
  |---|---|---|---|---:|
  | 1 | 293 / 20 / 12 | [388, 2267] vs [259, 6475] | [34, 54] vs [8.8, 220] | +0.498 |
  | 2 | 296 / 20 / 12 | [298, 2678] vs [229.4, 5735] | [35, 52] vs [9.2, 230] | +0.299 |
  | 3 | 296 / 20 / 12 | [694, 2743] vs [210.6, 5265] | [35, 47] vs [9.2, 230] | +0.479 |

- Robustness beyond the acceptance seeds, 60000 ticks on seeds 1–8, scored by the same long-run rule:
  - 7 of 8 pass.
  - Seed 8 fails the band: a post-summer grazer crash takes it to 164 against an anchor of about 1900.
  - The grazer max/min ratio over ticks 20k–60k is 2.7–9.0 on the passing seeds and 21.8 on seed 8.
- 60000-tick runs drive `fertility_mean` to its cap of 255 by about tick 30000. That breaks the 20000-tick `fertility_band` invariant, which `--long` deliberately doesn't include. See DECISIONS.md.

## Immigration floor (sweep 6, and the extra sweep)

At the defaults the floor never fires:
- Hunters never drop below 24 on seeds 1–3, so zero immigrants arrive at any floor from 0 to 16.
- So floor 0 passes, although the brief expected it to fail.
- The small-number problem the floor exists for only appears when hunters are stressed.

`extra_immigration_floor_at_k3/` reruns sweep 6 with `--set hunter.refugium_k=3.0`, the edge of sweep 1's band:

| floor | cells passing | immigrants (s1, s2, s3) | min hunters |
|---|---|---|---|
| 0 | 1/3 | 0, 0, 0 | 0 (seeds 2, 3 extinct at 18109, 18355) |
| 4 | 3/3 | 2, 2, 5 | 1–3 |
| 8 (default) | 3/3 | 6, 10, 11 | 1–4 |
| 12 | 3/3 | 7, 9, 13 | 6–9 |
| 16 | 3/3 | 10, 22, 15 | 4–11 |

At the band edge the floor is what keeps hunters alive: 0 fails and 4 or more passes. In sweep 1, cells at k 3.0–4.0 received 6–29 immigrants each. At 3.5–4.0 one immigrant per 500 ticks can't outpace starvation, and hunters touch 0 between arrivals.

## Crowding mortality (sweep 7)

Every value from 0 to 0.05 passes on every seed. Self-thinning trims mature trees in closed stands. At these rates it doesn't move any invariant out of its band, including the ≥35 mature trees at tick 10000.

## The amplitude-0 cycle test

> At `season.amplitude=0`, does the grazer series still have ≥ 2 local maxima after tick 4000 with peak-to-trough ratio ≥ 1.5, on all three seeds?

**Answer: NO. It holds on seeds 1 and 3 but not on seed 2**, which has only one qualifying peak.

The method is `python sweeps/cycle_ratio.py sweeps/shot5/season_amplitude`, which uses the same smoothing and maxima as `check.rs`:
- a 200-tick centred moving average
- a maximum ≥ every value within ±500 ticks
- trough = the smoothed minimum since the previous maximum
- a peak qualifies at ratio ≥ 1.5

The script's cross-check against `sweep.csv`'s `grazer_peaks` passes.

| amplitude | seed | peaks > 4000 | qualifying | ratio min/median/max | answer | hunters at the qualifying peaks |
|---|---|---:|---:|---|---|---|
| 0 | 1 | 10 | 3 | 1.12/1.28/1.74 | YES | 48, 43, 42 |
| 0 | 2 | 9 | 1 | 1.18/1.29/1.67 | NO | 46 |
| 0 | 3 | 8 | 2 | 1.15/1.28/1.73 | YES | 42, 38 |
| 15 (default) | 1 | 7 | 4 | 1.08/1.72/2.28 | YES | 42, 46, 48, 45 |
| 15 (default) | 2 | 10 | 3 | 1.00/1.12/1.87 | YES | 46, 44, 45 |
| 15 (default) | 3 | 8 | 3 | 1.09/1.35/2.17 | YES | 43, 43, 43 |

**Hunters are alive at every counted peak.** At amplitude 0 they number 38–48, and across all amplitudes and seeds 31–59 (the full table is in the script output). This is the change from shot 4:
- There, the lone amplitude-0 oscillation was followed by hunter extinction on 2 of 3 seeds.
- Here, hunters persist throughout.

The cycle is weaker without seasons: median ratio about 1.28 against 1.12–1.72 at 15, and 1–3 qualifying peaks against 3–4.

**What the cycle is, and isn't.** Hunter numbers hardly move. After the start they climb from 20 to about 35–55 and stay there, held by `hunter.cooldown` 5000 and `max_age` 8000 rather than by prey, so they don't track grazer peaks. The grazer oscillation comes from grazers overshooting their grass supply, and seasons pace it:
- Crashes cluster just after peak summer temperature.
- Amplitudes 0–9 all fail `fertility_band` (mean fertility just above 220). So the 20000-tick calibration still needs a season of at least 12.

It isn't a Lotka–Volterra predator–prey cycle. The continuous refugium plus the cheap failed attack (`fail_cost` 0.25) makes predation a steady drain rather than a driver.

## Rules that limit a band

- **`refugium_k` upper edge: the continuous refugium itself.**
  - Mean shrub settles near 0.6–0.7, so at k = 3.5 the attack success is `0.3 × 0.35^3.5` ≈ 0.0076.
  - Break-even is `fail_cost / (kill_energy + fail_cost)` = 0.25 / 40.25 ≈ 0.0062, before movement costs.
  - That is barely above break-even per attack. A chasing hunter also pays twice `energy_cost` (0.08) every tick it moves, which puts hunters in dense shrub at a loss, and one immigrant per 500 ticks can't make it up.
- **`season.amplitude` lower edge: `fertility_band`, not any animal rule.** The margins are small: −0.026 at 9 and −0.080 at 0.
