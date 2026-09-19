# Shot 11 findings: heritable traits

One sweep, `heredity.mutation` over 0:0.2:0.025, on seeds 1–3 at 20 000 ticks. Every other parameter is at its default: `heredity.mutation` 0.05, and all immigration floors 0. The command and wall time are in `_log.txt`, and the full table is in `heredity_mutation/sweep.md` and `sweep.csv`. The per-cell series are gitignored; rerunning the logged command regenerates them. `traits.py` reads them and writes `heredity_mutation/traits.txt`, which holds the per-seed trait means and standard deviations behind the tables below.

This is a report only. Nothing was tuned on these results.

## Survival: every cell passes

- All 27 cells pass every invariant. The safe band is the whole grid, [0, 0.2].
- **No extinctions.** No species reaches 0 at any tick in any cell, so there are no extinction causes to report.
- **Regions.** Survival forms one region covering the whole grid on every seed. There are no collapsing cells, so there is no edge to describe.
- The anchor holds at the default: seeds 1, 2, 3 and 42 pass `ecosim check` at mutation 0.05 (`TUNING.md`).

## Trait means at tick 20000 against the defaults

The tables give each trait's mean over the live animals at tick 20000, as a % change from the species default, for seeds 1 / 2 / 3. The last column shows the direction on each seed: `+` or `−` for a change over 1%, `0` otherwise. Mutation 0 is omitted: there, every animal carries the defaults and every standard deviation is exactly 0.

### Grazers

| mutation | energy_cost_mult (1.0) | dir | flee_distance (4.0) | dir | repro_threshold (70) | dir |
|---|---|---|---|---|---|---|
| 0.025 | +1.0 / −4.0 / +1.4 | + − + | −4.1 / −0.6 / −3.8 | − 0 − | −0.6 / −1.0 / −3.0 | 0 0 − |
| 0.050 (default) | −3.0 / −0.9 / +6.5 | − 0 + | +4.6 / +2.0 / +1.7 | + + + | −3.1 / +5.1 / −0.3 | − + 0 |
| 0.075 | −2.3 / +2.8 / −2.3 | − + − | −3.0 / +0.2 / +20.9 | − 0 + | −2.8 / −10.0 / +2.9 | − − + |
| 0.100 | −8.2 / −0.7 / +7.0 | − 0 + | +2.6 / +12.8 / +29.0 | + + + | −13.6 / −10.7 / −24.5 | − − − |
| 0.125 | −3.6 / +0.3 / +2.2 | − 0 + | +21.1 / +7.2 / +58.8 | + + + | −10.4 / −9.8 / −25.6 | − − − |
| 0.150 | −14.1 / −3.3 / +6.4 | − − + | +64.3 / +46.0 / +43.1 | + + + | −26.8 / −18.7 / −18.0 | − − − |
| 0.175 | −6.2 / +1.5 / −25.0 | − + − | +48.6 / +26.7 / +99.9 | + + + | −25.5 / −22.8 / −26.1 | − − − |
| 0.200 | −17.1 / −14.9 / −16.0 | − − − | +59.1 / +33.4 / +80.2 | + + + | −24.6 / −28.1 / −28.0 | − − − |

### Hunters

| mutation | energy_cost_mult (1.0) | dir | flee_distance (4.0, neutral) | dir | repro_threshold (75) | dir |
|---|---|---|---|---|---|---|
| 0.025 | +0.0 / −0.6 / +0.9 | 0 0 0 | −1.3 / +0.0 / −0.2 | − 0 0 | +0.5 / +0.3 / +1.5 | 0 0 + |
| 0.050 (default) | −2.0 / +2.6 / −0.1 | − + 0 | +4.6 / −2.1 / +2.8 | + − + | +4.1 / −1.1 / +1.1 | + − + |
| 0.075 | +2.5 / −0.9 / +1.1 | + 0 + | −1.0 / +0.9 / +3.1 | − 0 + | −2.2 / +2.4 / +0.8 | − + 0 |
| 0.100 | −4.6 / −2.3 / +0.3 | − − 0 | −5.8 / −4.0 / +1.4 | − − + | −2.7 / +7.0 / −7.1 | − + − |
| 0.125 | −4.1 / −8.7 / −8.0 | − − − | +1.5 / −7.1 / −5.5 | + − − | −2.4 / +3.9 / +1.8 | − + + |
| 0.150 | −0.4 / −8.4 / +3.6 | 0 − + | +12.6 / +2.0 / −13.7 | + + − | +2.1 / −3.5 / −7.5 | + − − |
| 0.175 | +1.0 / −15.2 / −5.7 | 0 − − | +9.7 / +10.0 / −2.9 | + + − | −9.8 / −8.5 / +7.0 | − − + |
| 0.200 | −16.7 / −9.2 / −1.8 | − − − | +3.4 / −17.3 / −1.0 | + − − | −11.5 / −6.2 / −0.9 | − − 0 |

## Is the direction of drift the same across seeds?

- **Grazer `flee_distance` rises on all three seeds from mutation 0.1 up** (and at 0.05). At 0.15–0.2 it is 27–100% above the default of 4. Fleeing has no energy price beyond the doubled cost of the step, so a wider flee radius only costs when a flee step replaces an eat.
- **Grazer `repro_threshold` falls on all three seeds from 0.1 up,** by 10–28%, to about 50–60. Grazers that breed at lower energy win. The standard deviation grows with the rate, from 4 at 0.025 to 18 at 0.2.
- **Grazer `energy_cost_mult` has no consistent direction below 0.2.** At 0.2 it falls on all three seeds, by 15–17%. Below that the seeds split, with changes of −25% to +7%. A cheaper animal should always win. That it doesn't show up earlier suggests the per-tick cost (0.1) is small next to intake and the energy gates. That is a reading of the table, not a tested cause.
- **Hunter traits have no consistent direction** at most rates. `energy_cost_mult` falls on all three seeds only at 0.125 and 0.2. `repro_threshold` never agrees across all three seeds away from 0.
- **The neutral control agrees with that.** Nothing a hunter does reads hunter `flee_distance`, so it drifts without selection. It wanders by up to ±17% with mixed signs, which is the same size as the hunter changes in the other two columns. So with 30–60 hunters, drift alone can account for every hunter change here. The grazer shifts in `flee_distance` and `repro_threshold` are several times larger than the neutral control, in a population about 20 times as large, and they agree across seeds.
- **At the default (0.05),** every change is under 7% and only grazer `flee_distance` agrees across seeds (+2 to +5%). Twenty thousand ticks at 0.05 are not enough for selection to show clearly.
