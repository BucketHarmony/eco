# Tuning log

**Method.** Each round ran the four acceptance seeds (1, 2, 3, 42) at 20000 ticks with a scratch `params.toml` variant, then ran `ecosim check` and a trajectory summary.

**Reading the results.**
- "Fails" is the total of failed check lines across the 4 seeds, excluding the runtime line: running four at once inflates wall time about 5×.
- "Stab" is the hunters-disabled test (`hunter.start_count = 0`): whether every 200-tick moving average over ticks 8000–20000 stays in [0.7K, 1.3K].
- Round letters were reused after round z, so the second series is written a′, b′, c′.

**How the rounds ran.** Rounds a–h tuned water and trees. Rounds i–k tried to keep hunters alive. Rounds n–x worked on grazer stability without hunters, with code changes between them (see `DECISIONS.md`). Rounds y–c′ re-tuned hunters on the stable grazer base.

## Final values vs. addendum starting values

| param | start | final | why |
|---|---|---|---|
| climate.rain_base | 8.0 | 8.0 | explored 5–7.5 in rounds a–g; everything below 7.5 killed off the trees, so it went back to 8 |
| climate.decay_k | 0.02 | 0.015 | fertility_mean crept above 220 once detritus built up (round y) |
| cover.moisture_draw | 40 | 15 | cover dried the soil too fast for trees to establish (e, g) |
| tree.moisture_draw | 2 | 50 | forest closed completely: renderer light test needs ≥40% bright ground (f, g) |
| tree.mature_age | 2000 | 1000 | ≥35 mature at 10k together with a sparse enough canopy (f, g) |
| shrub.light | [40,120,255,256] | [40,100,200,254] | understory: shrub stops covering every patch (h) |
| shrub.g | 0.002 | 0.004 | keeps refugia present under canopy with the new curve (h) |
| grazer.grass_per_energy | 0.0004 | 0.00003 | cheap grass, so the birth cap regulates instead of starvation crashes (v, x) |
| grazer.choice_tolerance | (new) | 0.75 | spreads grazers across acceptable patches, no herding (w, x) |
| grazer.eat_min_grass | (new) | 0.0 | tried > 0, rejected (m, r, v) |
| hunter.refugium_shrub | 0.5 | 0.58 | 0.5 protected too many grazers, 0.62+ too few (j, y) |
| hunter.kill_prob | 0.3 | 0.2 | the SAD's risk-table fallback; damps the hunter boom (y, a′) |
| hunter.energy_cost | 0.12 | 0.04 | hunters survive grazer lows (z, a′) |
| hunter.cooldown | 800 | 3000 | slow hunter numeric response, which cuts the hunter peak (a′–c′) |
| hunter.start_count | 6 | 12 | higher tick-2000 hunter count, so the 10× cap is met (c′) |

`world.water_fraction = 0.04` and `world.canopy_absorb = 100` were set at build time (the latter from the addendum).

## Rounds

### 0: baseline (addendum values)
Trees were fine: 675 max, 334 mature at tick 10000. Hunters went extinct at tick 1120 and grazers crashed to 0 on s42. Shrub covered all 64 patches by tick 2000, so the refugium protected every grazer.

### a: `rain_base 5, rain_amp 3, shrub.g 0.008` (trying to hold shrub back by drying the world)
Trees collapsed to 1–2, with mature@10k between 3 and 23. **Rejected**, because it was too dry.

### b: `rain_base 6.5–7`, some with `tree.moisture [80,200,255,256]`
Trees fell to 2–6 at tick 2000, then recovered slowly (fails "10× tick-2000 count" for trees). Grazers went to 0 on some seeds.

### c: `rain_base 7.5–8`, `rain_amp 5–6`, `tree.moisture_draw 10–20`
- `rain_base ≥ 7.5` gave healthy trees (tree@2k about 28, mature@10k 300+).
- `rain_amp` 6 with low draw killed trees again.
- Hunters were still extinct everywhere.

### d: `rain_base 7.5–8` with `tree.moisture_draw 40–120`
Higher tree draw dried the soil under trees, and forest stayed small (max 30–600). Draw 40 was the upper usable value.

### e: `cover.moisture_draw 10–15` with `rain_base 6–7.5`
Lower cover draw alone didn't save trees at rain < 7.5.

### f: `rain_base 7.5` with `tree.mature_age 1000–1200`
- mature@10k was about 520, but the view was 58% dark and 34% bright, which fails the renderer's ≥40% bright need.
- `rain 7.0` with `mature_age 1000` came close: 38% dark, 58% bright.

### g: `cover.moisture_draw 15` combined with `tree.moisture_draw 30–70`, `rain 7–8`, `mature_age 1000`
- **g4** (`rain 8, cover draw 15, tree draw 50, mature 1000`): tree@2k about 100, mature@10k 205–247, s42 view 29% dark and 66% bright. **Adopted as the tree base.**
- g5 and g6 were too sparse (s42 view 6% dark).

### h: shrub curve on g4
- `shrub.light [40,100,200,254]` with `shrub.g 0.004` (h2) made shrub > 0.5 on about 55 patches at tick 10000, mostly shaded ones. **Adopted.**
- `[40,100,170,250]` cut shrub too far where the canopy was open.
- Hunters now reach 17–35 at tick 2000 but still hit 0 later.

### i: hunter fallback knobs
Tried `kill_prob 0.2 / 0.15`, `cooldown 1200–1500`, `kill_energy 25`, `seek_radius 8` and `refugium 0.35`. Hunters died on every seed, with h@2k at 4–13.

### j: `refugium_shrub 0.6–0.65`
Hunters peaked at 45–73 but still reached 0. Grazers also touched 0–13 on some seeds: the grazer dynamics were themselves unstable.

### k: hunter `energy_cost 0.05–0.08` with `cooldown 1200–2000`
Hunters peaked at 23–83 and all still reached 0. **Diagnosis:** grazer boom-bust without hunters is the root problem, so the next rounds used the hunters-disabled test.

### n: grazer energetics (stab: all FAIL)
- Tried `energy_cost 0.04–0.15`, `grass.r 0.1`, `grass_per_energy 0.0001–0.0002`, `crowding 1–2`, `intake_k 100` and a wider grass temperature curve.
- K ranged 50–1350, with moving averages at 0.0–0.4× to 1.5–3× K on every variant.
- Trajectories showed herds trapped against rock ridges and water.
- **Code change:** BFS walking-distance pathing to target patches.

### m: `eat_min_grass 0.02–0.1`
This stopped near-empty patches from holding grazers, but formed "deserts" where grass sat at the threshold while starving grazers shuttled between them. Stab: FAIL.

### q, p: diagnostics
- **q** (no initial trees; no seasons, i.e. `temp_amp = rain_amp = 0`) and **p** (repeats with `eat_min_grass`) still failed.
- So the instability does not come from trees or seasons; it is grazer behaviour.

### r: `intake_k 1–4`, `eat_min_grass` combos
Stab: all FAIL. MA ranged 0.1–2.5× K.

### s, t, u: crowding and life history
- Tried `crowding 2–100`, `max_grazers_per_patch 4`, `grazer.cooldown 800`, `search_patches 3–4` and `max_age 2000`.
- All FAIL.
- Trajectories showed streams of about 1000 grazers all choosing the same "best" patch (the lowest-index tie-break) and stripping it.
- **Code change:** choice tolerance with a per-grazer hash preference.

### v: `grass_per_energy 0.00005`
First partial pass: 1 of 4 seeds OK. Cheap grass means the per-patch birth cap, not starvation, bounds the population.

### w: `choice_tolerance 0.25–1.0`
- 0.5–1.0 with cheap grass passed on most seeds; w3 (1.0) passed 4 of 4 on one draw.
- 0.25 was too little to break up herds.

### x: `grass_per_energy 0.00002–0.0001` × `choice_tolerance 0.3–0.75`
**x8** (`grass_per_energy 0.00003`, `choice_tolerance 0.75`) passed stab on all 4 seeds, with MA 0.73–1.26 K. **Adopted as grazer base "B".**

**Code change:** per-column animal grids for neighbour queries. The runs got 3–5× faster with identical output, because grazers now number in the thousands.

### y: hunters on B
- Base hunters rose from about 30 at tick 2000 to 150–200, then went extinct.
- `refugium 0.58–0.6` gave hunters min 0–7. `refugium 0.58` with `cooldown 2000` gave hunters min 0 on 3 seeds.
- `climate.decay_k 0.02 → 0.015` fixed fertility_mean peaking at 225–228 (limit 220): with the grazer boom, corpse detritus rose. **Adopted, along with refugium 0.58.**

### z: `cooldown 1000–1500` × `energy_cost 0.05–0.08`
Real predator–prey cycles appeared (hunter peaks 140–260), but hunters went extinct in troughs on 2–3 seeds.

### a′: `energy_cost 0.03–0.04`, `cooldown 1200–2000`, `kill_prob 0.2`
**a′2** (`cooldown 1500, energy_cost 0.04, kill_prob 0.2`) kept hunters alive on all seeds (min 6–16). The only remaining fail was hunters max 158–217 against the 10×@2k limit of 110–180.

### b′: `cooldown 1500–2500`, `max_age 6000`, `repro_energy 90`, `kill_prob 0.15`
- Slower reproduction lowered the hunter peak: `cooldown 2500` gave a max of 96–132 against a limit of 100–120, still just over.
- `repro_energy 90` made the cycle unstable (extinctions).

### c′: `start_count 10–12` with `cooldown 2000–3000`
- **c′4** (`start_count 12, cooldown 3000, energy_cost 0.04, kill_prob 0.2`): hunters 19–101 against h@2k 19–22. Every check passed on seeds 1, 2, 3 and 42 (runtime aside). **Adopted as final.**
- c′3 (`cooldown 2000`) let s42 hunters reach 0.

## Final verification (final `params.toml`)

- Seeds 1, 2, 3 and 42: `ecosim check` passes every line. Single runs take 11.8–12.8 s wall time.
- Seeds 4–7 (robustness): `ecosim check` passes every line.
- Hunters-disabled stab, all four seeds pass:

  | seed | K | MA / K |
  |---|---|---|
  | 1 | 1657 | 0.83–1.25 |
  | 2 | 1487 | 0.81–1.21 |
  | 3 | 1693 | 0.73–1.19 |
  | 42 | 1626 | 0.82–1.26 |

- `cargo test --release`: 11 unit and 5 integration tests pass.

## Dynamics fixes (shot 5)

The rule changes (continuous refugium, hunter immigration floor, tree lifespan jitter, canopy self-thinning) are recorded in `DECISIONS.md` under "Dynamics fixes". All rounds were run with the sweep harness. Scratch rounds (r1–r8, L1–L3) ran into a scratch directory, while the official rounds 1–4 ran all seven required sweeps. Round 4's output is committed in `sweeps/shot5/`, and its findings are in `sweeps/shot5/FINDINGS.md`.

In the tables below:
- "hext" is a cell where hunters reach 0 at some tick.
- "pass" means every 20000-tick invariant passes. Runtime is excluded.

### Values changed in this shot

| param | before | after | why |
|---|---|---|---|
| hunter.start_count | 12 | 20 | brief (`hunter.initial = 20`) |
| hunter.refugium_shrub → refugium_k | 0.58 | 2.0 | brief; 2.0 is the middle of the round-2/3 band [0.5, 3.5] and the brief's default |
| hunter.immigration_floor / interval | (new) | 8 / 500 | brief |
| grazer.immigration_floor / interval | (new) | 0 / 500 | brief (off for grazers) |
| tree.lifespan_jitter | (new) | 0.2 | brief |
| tree.crowding_mortality | (new) | 0.02 | brief; the band is [0, 0.05], reported only |
| grazer.start_count | 60 | 300 | 20 hunters against 60 grazers with no shrub yet → grazer extinction by tick 1100–2500 on every seed (r1, r3) |
| hunter.fail_cost | 2.0 | 0.25 | at mean shrub about 0.65 success is about 0.02–0.06, and a 2-energy miss starved hunters (r2, r4–r6) |
| hunter.cooldown | 3000 | 5000 | slower hunter growth; with fail 0.25, cooldown 3000 overshoots and crashes (r6) |
| hunter.kill_prob | 0.2 | 0.3 | middle of the round-1 band [0.1, 0.5] |
| grazer.energy_cost | 0.08 | 0.10 | middle of the round-1 band [0.04, 0.16] |
| season.amplitude | 12 | 15 | middle of the round-1 band [12, 18] |
| tree.mature_age | 1000 | 1000 | round 2 tried 1500, the middle of round 1's [1000, 2500], and the band shrank to [500, 1500]; back to 1000, where round 4's band is the full grid |
| grazer.max_grazers_per_patch | 8 | 5 | 60k-tick grazer swings; seed 1 failed `check --long` at 8 (L1–L3) |

### r1: `grazer.start_count 60/300` × `refugium_k 0.5/1/1.5` × `kill_prob 0.2/0.3` (brief defaults otherwise)
- start 60 fails everywhere, with grazer extinction early.
- start 300, k 1.0, kp 0.2 passed seeds 2 and 3. Seed 1 had hext at 15394.
- k 0.5 fails through over-predation.
- Immigrants per run were 11–36, so the floor was usually binding.

### r2: start 300; `fail_cost 0.5/1/2` × `k 1/2` × `kp 0.2/0.3`
- 4 of 36 cells pass: (0.5, 2, 0.2) on seeds 2 and 3, and (2, 1, 0.2) on seeds 2 and 3.
- At k 1, grazers crash around tick 7000–9000. At k 2 with fail ≥ 1, hunters starve around tick 10000–15000.
- Diagnosis (trajectory `a1`): with the threshold refugium gone, hunters chase grazers in every patch, and flight plus failed-attack displacement starve grazers even with grass at 0.5.

### r3: start 60; `grazer.flee_radius 1/2/3` × `kp 0.2/0.4` × `k 1/2`
Almost every cell fails, with grazers extinct by tick 1000–4000. The early phase (shrub ≈ 0) is the bottleneck, and flee radius doesn't fix it. Flee radius was left at 4.

### r4: start 300; `flee_radius 2/3/4` × `hunter.cooldown 3000/5000` × `fail_cost 1/2`
- 0 of 36 cells pass. In every cell hunters go extinct first, at tick 6000–13700.
- Trajectory `a4` (flee 3, fail 1): hunters fall from 78 to 1 between ticks 7000 and 12000, while grazers stay at 700–2300. Hunters starve beside abundant prey, because each miss costs more than the expected kill return at success ≈ 0.02.

### r5: start 300; `hunter.energy_cost 0.015/0.025/0.035` × `k 1/2/3` × `fail_cost 0.5/1`
- Only (k 2, fail 0.5) passes, on 2 of 3 seeds, at any energy cost.
- Energy cost is a weak lever. The attack budget `kp·(1−s)^k·kill_energy − fail_cost` dominates.

### r6: start 300; 2^6 factorial `fail_cost 0.25/0.5` × `k 1.5/2.5` × `kp 0.15/0.3` × `cooldown 3000/5000` × `flee 2/4` × `satiation 70/85`
- 18 of 64 combinations pass 3 of 3.
- fail 0.25 with k 1.5 and cooldown 5000 passes at every kp, flee and satiation value.
- cooldown 3000 with fail 0.25 fails in most cells: the faster hunter growth overshoots.

### r7, r8: candidate (fail 0.25, cooldown 5000)
- `k` 0.5:4.0:0.5 at start 60: `max_10x` fails on seeds 1 and 3 at k ≤ 2.0 (seed 1 only at 2.5), and hext at k ≥ 3.
- At start 300: pass for k 0.5–2.5 (15 of 15 cells), hext at k ≥ 3.
- `kill_prob` 0.1:0.5:0.1 at start 300, k 1.5: 15 of 15 pass.
- Adopted: start 300, fail 0.25, cooldown 5000, k 1.5 (band middle), kp 0.3 (band middle).

### Round 1 (official sweeps, defaults as adopted after r8)

| sweep | band | notes |
|---|---|---|
| refugium_k | [0.5, 3.0] | hext at 3.5 and 4.0 (6 cells) |
| kill_prob | [0.1, 0.5] | |
| energy_cost | [0.04, 0.16] | middle 0.10 → adopted |
| mature_age | [1000, 2500] | 500 fails `no_extinction` (seed 2) |
| amplitude | [12, 18] | middle 15 → adopted; 9 fails `fertility_band` −0.020 |
| immigration_floor | [0, 16] | |
| crowding_mortality | [0, 0.05] | |

hext: 6 of 129 (4.7%).

### Round 2: `energy_cost 0.10`, `mature_age 1500`, `amplitude 15`
- Bands: k [0.5, 3.5], kp [0.1, 0.5], energy [0.04, 0.16], mature_age **[500, 1500]** (2000 fails `mature_trees_10k` on seed 2, −0.086), amplitude [9, 18], floor [0, 16], crowding [0, 0.05].
- hext: 3 of 129 (2.3%).
- Next: k 2.0 (middle of [0.5, 3.5]); mature_age back to 1000 (middle of [500, 1500]).

### Round 3: `refugium_k 2.0`, `mature_age 1000`
- Bands: k [0.5, 3.5], kp [0.1, 0.5], energy [0.04, 0.16], mature_age [500, 2500], amplitude [9, 18], floor [0, 16], crowding [0, 0.05].
- hext: 3 of 129 (2.3%).
- `check` passes on seeds 1, 2, 3 and 42.
- 60k `check --long`:
  - seeds 2 and 3 pass (margins +0.36, +0.45)
  - **seed 1 fails `long_band`**: the tick-20000 anchor sat on a grazer peak (2130), then grazers fell to 371 < 426
- Hunters were flat at 28–57 in all three runs.

### L1–L3: 60k-tick sweeps, scored with the long-run rule (seeds 1–6 or 1–8)
- **L1**, `hunter.displace_steps 1/3` × `grazer.cooldown 300/600` × `max_grazers_per_patch 5/8`:
  - cooldown 600 kills grazers everywhere.
  - mgpp 5 passes 6 of 6 at both displacement values. mgpp 8 passes 5 of 6 and 4 of 6.
- **L2**, mgpp 4/5/6 on seeds 1–8: passes 6, 7 and 6 of 8.
  - mgpp 5 fails only seed 8, where a post-summer crash takes grazers from about 1900 to 164.
- **L3**, `grazer.max_age 3000/8000/12000` × mgpp 5/8: 6–8 of 8 with no trend in `max_age`. The crashes aren't age cohorts: they follow peak summer, when grazers overshoot their grass supply.
- Adopted mgpp 5. Across every 60k cell it passed 41 of 44 seed runs.

### Round 4 (final; committed in `sweeps/shot5/`)
- Bands: k [0.5, 3.0], kp [0.1, 0.5], energy [0.04, 0.16], mature_age [500, 2500], amplitude [12, 18], floor [0, 16], crowding [0, 0.05].
- hext: 5 of 129 (3.9%), all at k ≥ 3.5.
- `check` passes on seeds 1, 2, 3 and 42.
- 60k `check --long` passes on seeds 1, 2 and 3 (margins +0.50, +0.30, +0.48).
- The hunters-disabled capacity test and the drought test pass.
- Defaults were not moved again: every default is inside its band, and 3 of 5 sit at the band middle.
  - `refugium_k` 2.0 is 0.25 from the middle, 1.75. Moving it would put the default off the grid.
  - `mature_age` 1000 is below the middle, 1500. Round 2 showed that 1500 shrinks this band.

## Extinction attribution (shot 05)

### Values changed in this shot

| param | before | after | effect |
|---|---|---|---|
| hunter.immigration_floor | 8 | 0 | None at the defaults. On seeds 1, 2, 3 and 42 the series and snapshots are byte-identical; only `meta.json` differs, because the floor never fired (0 immigrants). At the band edge it matters: `refugium_k` 3.0 now fails on seeds 2 and 3, and the band shrank from [0.5, 3.0] to [0.5, 2.5] (`sweeps/shot05/FINDINGS.md`). |

Nothing else was tuned. Seeds 1, 2, 3 and 42 pass `ecosim check` at 20000 ticks, and `runs/long42` passes `check --long` (margin +0.415).

### The four anchor values

`grazer.start_count` 300, `hunter.fail_cost` 0.25, `hunter.cooldown` 5000 and `grazer.max_grazers_per_patch` 5 are the regression anchor. They are the values at which seeds 1, 2, 3 and 42 persist at the defaults, and they are not a claim about the model.
- Each was changed in "Dynamics fixes (shot 5)" above, with its round of evidence: r1/r3, r2/r4–r6, r6 and L1–L3.
- `DECISIONS.md` ("Extinction attribution") repeats them with the failure each one prevents.
- Later shots leave them alone. If a new mechanism breaks the anchor, the sim-shot rules pick the smallest default for that mechanism, not a retune of these four.

The death causes show that `hunter.cooldown` 5000 together with `max_age` 8000 is what sets the hunter count at the defaults: about 97% of hunter deaths are old age.

## Fire (shot 09)

### Starting values
The `[fire]` section is new. These are guesses from the shot prompt's sketch: base_rate 0.002, temp_min 15 °C, temp_full 30 °C, detritus_weight 0.0002, canopy_weight 1.0, duration 10, spread 0.1, tree_kill 0.5, detritus_yield 10, ash 5 and animal_damage 2.

### Round 1: duration 10 breaks the anchor
- **Sweep:** base_rate 0:0.004:0.0005 on seeds 1, 2, 3 and 42 (exploratory, not committed).
- **Result:** every rate above 0 failed `mature_trees_10k` on some seed. At 0.0005, seeds 1 and 3 failed with margin −0.886. At 0.002, 1 of 4 passed.
- **Cause:** a patch that burns for 10 ticks gets 10 spread rolls at 0.1 against each neighbour. Every fire reached most of the grid and re-burned it, giving 600–1300 burn-outs per run. Each burn-out killed half the trees in the patch, so no cohort reached maturity by tick 10000.
- **Conclusion:** no ignition rate low enough to keep the anchor also leaves fire doing anything, so ignition is not the knob to turn.

### Round 2: spread × duration at base_rate 0.002
Cells pass out of 4 seeds (1, 2, 3, 42):

| spread \ duration | 3 | 5 | 10 |
|---|---|---|---|
| 0.02 | 4/4 | 4/4 | 4/4 |
| 0.05 | 4/4 | 4/4 | 4/4 |
| 0.1 | 4/4 | 2/4 | 1/4 |

**duration 10 → 3.** This is the smallest change to the new mechanism that keeps the anchor at the prompt's spread (0.1) and base_rate (0.002). With 3 ticks, a burning patch rolls 3 times per neighbour, and fires stay below percolation: at most 12 of 64 patches burn at once, with 40–90 burn-outs per run at the default.

### Round 3: base_rate at duration 3
- **Sweep:** base_rate 0:0.004:0.0005 on seeds 1, 2, 3 and 42.
- **Result:** every cell passes except 0.0025 on seed 42, where mature_trees_10k is 34 (margin −0.029). Seeds 1–3 pass everywhere, as the committed sweep `sweeps/shot09/fire_base_rate` confirms.
- **Default base_rate stays 0.002.** It is the middle of the grid, and seed 42 passes it with 61 mature trees at tick 10000. The 0.0025 miss is a single-seed dip at one grid point with passes on both sides, not an edge.

### Final
- Values changed: `fire.duration` 10 → 3. No other parameter moved, including the four anchor values.
- Seeds 1, 2, 3 and 42 pass `ecosim check` at 20000 ticks with the defaults.
- `fire.spread` has a fragile safe band, [0.0, 0.1], with the default at its upper edge: the percolation threshold lies between 0.1 and 0.2 (`sweeps/shot09/FINDINGS.md`). Spread was not lowered, because the default passes on every seed and the sim-shot rule moves a default only when the anchor fails.

## Density-dependent mortality (shot 10)

### Starting values
- `[disease]` is new: grazer_rate 0.001 with grazer_threshold 16, and hunter_rate 0.001 with hunter_threshold 4.
- `hunter.cooldown` 5000 is replaced by `hunter.refractory`, and the prompt's value is 300.

### Round 1: refractory 300 breaks the anchor
At the disease defaults, all four anchor seeds (1, 2, 3, 42) fail. Hunters boom and eat every grazer. The committed sweep `sweeps/shot10/hunter_refractory` confirms this: every cell from 100 to 1000 is a grazer extinction, `eaten`.

### Round 2: smallest refractory that keeps the anchor
Run on a 250-tick grid, at the disease defaults:

| refractory | seeds passing (1, 2, 3, 42) |
|---|---|
| 1000, 2000, 2250 | fail |
| 2500 | 3/4 (seed 3 fails) |
| **2750** | **4/4** |
| 3000, 4000, 5000 | 4/4 |

**refractory 300 → 2750.** A 60 000-tick `check --long` on seed 1 at 2750 also passes, with margin +0.77.

### Round 3: is 2750 carried by hunter crowding?
- With hunter_rate 0, refractory 2750 and 3500 fail, and 5000 passes.
- At refractory 2750, hunter_rate 0.0005 fails 2 seeds, and 0.0002 fails too.
- hunter_rate 0.05 with threshold 2 passes at refractory 300, but was not adopted (`DECISIONS.md`).
- **Result:** the disease defaults stay at the smallest grid values that keep the anchor. With grazer_rate 0 at refractory 2750, seed 3 fails (`sweeps/shot10/disease_grazer_rate`).

### Final
- Values changed: `hunter.refractory` = 2750, which replaces `hunter.cooldown` 5000.
- `[disease]` is at its starting values. None of the four anchor values moved.
- Seeds 1, 2, 3 and 42 pass `ecosim check` at 20000 ticks with the defaults.
- `disease.grazer_rate` has a fragile safe band, [0.001, 0.002] (`sweeps/shot10/FINDINGS.md`).

## Heritable traits (shot 11)

### Starting values
- `heredity.mutation` 0.05, since the prompt gives only the sweep grid (0–0.2).
- All immigration floors stay 0, and the new `tree.immigration_interval` is 500, the same as the animals'.
- `hunter.flee_radius` is 4.0, the grazer value. It is the default of a trait no hunter behaviour reads, so its value changes nothing.

### Anchor at the default
Seeds 1, 2, 3 and 42 pass `ecosim check` at 20000 ticks with mutation 0.05. Seed 42 now has 85 mature trees at tick 10000; it had 58 before the shot.

### Sweep
The whole grid, 0:0.2:0.025, passes on seeds 1–3 (`sweeps/shot11/FINDINGS.md`).

### Final
No value changed. Mutation stays at its starting value of 0.05, and no other parameter was touched.

## Collapse atlas (shot 14)

No parameter changed. The shot adds `[rng] stream = 0`, which leaves every default run byte-identical, and reports the atlas in `sweeps/atlas/ATLAS.md`.

## Food-limited hunters (shot 14a, Blocked)

**No value changed.** `hunter.hunt_cost` was added at 0.0, the pre-shot rule. `disease.hunter_rate` stays 0.001 and `kill_energy` stays 40. The sweeps are in `sweeps/shot14a/` and the analysis in its `FINDINGS.md`.

| round | grid (seeds) | result |
|---|---|---|
| x1 | hunter_rate 0; kill_energy 20:80:10 × hunt_cost 0:5:1 (1, 2, 3, 42) | 10 of 168 cells pass. hunt_cost 0 means grazers are eaten, and 2 or more means hunters starve. The band is around 1. |
| x2 = `kill_x_cost_fine` | hunter_rate 0; kill_energy 40:80:10 × hunt_cost 0.5:1.5:0.1 (1, 2, 3, 42) | 46 of 220 cells pass, and no cell passes all four seeds. Seed 1 is `eaten` in 54 of 55 cells. |
| `crowding_on_kill_x_cost` | hunter_rate 0.001; kill_energy 40:80:10 × hunt_cost 0:1.5:0.25 (1, 2, 3, 42) | The anchor holds for hunt_cost ≤ 0.5–0.75, but no cell reaches the signature target. The best cell is 70/0: lag > 0 on every seed, corr 0.00–0.34. |
| `diag_refractory500_kill_x_cost` | hunter_rate 0, refractory 500; kill_energy 20:80:20 × hunt_cost 0.5:2:0.5 (1, 2, 3, 42) | 0 of 64 cells pass. Every cell is a grazer extinction, `eaten`. |

Required sweeps (seeds 1–3, hunter_rate 0): `hunter_kill_energy` passes 7 of 21 cells, `hunter_hunt_cost` 1 of 18 and `kill_x_cost` 5 of 48. None reaches the target region.


## Food-limited hunters, second attempt (shot 14a-rev, Blocked)

**No value changed.** `hunter.handling_ticks` was added at 0, the pre-shot rule. `disease.hunter_rate` stays 0.001: change 4 asks for 0, but at 0 no cell keeps seeds 1, 2, 3 and 42 alive to 60000 ticks. The sweeps are in `sweeps/shot14a-rev/`, and the analysis is in its `FINDINGS.md`.

All rounds ran with `disease.hunter_rate=0` and `kill_energy=60`, for 60000 ticks.

| round | grid (seeds) | result |
|---|---|---|
| `handling_ticks` | 0:200:25 at hunt_cost 1.0 (1, 2, 3) | 1 of 27 cell-seeds persists (200, seed 1). Seeds 2 and 3 lose their hunters to starvation in every cell. |
| `refractory` | 300:1500:300 at handling 200, hunt_cost 1.0 (1, 2, 3) | 0 of 15. Every cell ends with grazers `eaten` by tick 1400–7000. |
| `handling_x_cost` | {25, 50, 100, 150} × hunt_cost {0.6, 0.8, 1.0, 1.2} (1, 2, 3, 42) | 7 of 64 cell-seeds persist, and no cell has more than 2 of 4 seeds. None shows pp_corr > 0.3 with 0 < pp_lag < pp_period/2. |

## World dimensions, rain gradient and slope (shot 15)

The shot sets the reference world, and it changed four defaults. The anchor held at every one of them, so no other value changed.

| key | before | after | reason |
|---|---|---|---|
| `world.width` | 64 (implicit) | 256 | shot 15 change 5: the reference world is a 256×64×32 strip |
| `world.depth`, `world.height`, `world.patch` | 64, 32, 8 (implicit) | 64, 32, 8 | made explicit (change 1), no change in value |
| `climate.rain_gradient` | 0 (new) | 0.6 | change 5: a dry west and a wet east (west edge 0.4 × rain, east edge 1.6 × rain) |
| `world.slope_bias` | 0 (new) | 4 | change 5: the west edge raised about 4 voxels and the east lowered about 4 |

- **Anchor on the strip (20000 ticks, release).** Seeds 1, 2, 3 and 42 pass every `ecosim check` invariant. The run times are 44.2, 44.4, 58.9 and 50.2 s (the 58.9 s ran alongside the other seeds), against the area-scaled 90 s limit (DECISIONS.md, shot 15).
  - Minimum grazers are 1478, 2302, 2553 and 2803, and minimum hunters 32, 37, 36 and 34.
  - Mature trees at tick 10000 are 424, 441, 505 and 515.
- **Why no anchor change.** The anchor line that could have forced one is "the reference seeds persist at 20k on the strip", and it held.
- **The sweep** (`sweeps/shot15/`) is `climate.rain_gradient` 0:1.0:0.2 on seeds 1–3. All 18 cells pass, so the default 0.6 sits inside a safe band covering the whole grid.

## Animals off (shot G0)

**No default changed.** The shot added one key, `animals.enabled`, and its default is `true`, which is what every run before this shot did. Every committed manifest and fixture is byte-identical, and seeds 1, 2, 3 and 42 pass `ecosim check` with the same numbers as before.

The shot prompt says not to retune anything to compensate for the missing grazing pressure, and the measurements say nothing needs it: with animals off, mean grass cover rises 8–16%, while shrub cover, tree counts and mature-tree counts stay inside the seed-to-seed spread, and no seed loses a species. The sweep over `animals.enabled` (both values, seeds 1–3, 20000 ticks) passes 6 of 6 cells. Numbers and the event-log cause breakdown are in `sweeps/G0/FINDINGS.md`.

## World bundles (shot G1)

**No default changed.** The shot adds a `[bundle]` section whose two keys are read only by a run
built from a world bundle (`ecosim run --world`). A noise run never reads them, and the section is
left out of `meta.json` at its defaults (`skip_serializing_if`, the pattern `[rng]` and `[animals]`
already use), so every committed manifest and fixture is byte-identical and seeds 1, 2, 3 and 42
pass `ecosim check` with the same numbers as before.

| key | value | why |
|---|---|---|
| `bundle.base_z` | 8 | Soil layers below the crop's lowest ground. The bundle's heights are metres above that minimum, so a column's surface layer is `8 + round(its mean ground height)`. Same headroom the noise world's terrain sits on, and it leaves 24 of the 32 layers for ground, buildings and canopy. |
| `bundle.shade_slope` | 1.0 | Columns of shadow per metre of roof height: a fixed sun due south at `atan(1.0)` = 45°, about the equinox noon sun at the reference site's latitude. 0 turns building shade off. |

No acceptance line forced either value; both are new knobs on a path no run took before this shot.
The acceptance that pins `base_z` is "a slope gives surface layers `8 + round(h)`", from the shot
prompt, and the one that pins `shade_slope > 0` is "a tall block shades the columns on its shadow
side and not the others". Both are tested on synthetic bundles in `src/bundle.rs` and
`src/world.rs`; there is no sweep, because neither key affects a noise run and there is no real
bundle in the repo until G2.

## The Blender exporter and the Capitol bundle (shot G2)

**No default changed.** The shot adds a tool (`tools/blend_export.py`) and data
(`worlds/capitol/`); it touches no simulation code and no parameter. Every committed manifest and
fixture is byte-identical, and seeds 1, 2, 3 and 42 pass `ecosim check` with the same numbers as
before.

No acceptance line came close to forcing one. The one worth recording as *not* forced is
`[bundle] base_z` = 8, set in G1 without a real world to try it on: the Capitol's 512 × 512 ground
spans 0.000–8.589 m, so its columns land on surface layers 8–16 and leave 15 of the 32 layers above
the highest ground. Nothing needed widening. `[world] height` stays 32 even though the dome stands
76 m above its footprint, because building height is a shade input and not a voxel column; a world
tall enough to hold the dome as voxels would be 2.4 × the memory for one building nothing can grow
on. `[bundle] shade_slope` = 1.0 is likewise untouched; what the Capitol's shadow does to the
ecology is G3's business, not this shot's.

## Plants from the scene, and the Capitol reference run (shot G3)

**No existing default changed.** The shot adds four keys to `[bundle]`, all read only when a run is
built from a world bundle. Their values equal the struct defaults, so `[bundle]` is still left out of
`meta.json`, every committed manifest and fixture is byte-identical, and seeds 1, 2, 3 and 42 pass
`ecosim check` with the same numbers as before (`fresh_s42_matches_committed_manifest` is green).

| key | value | why |
|---|---|---|
| `bundle.tree_mature_height` | 3.0 | Scene metres that map onto `tree.mature_age`. A mature sim tree's canopy voxels sit at `h + 2` and `h + 3` over a surface at `h`, so 3 m is the height of a mature canopy above the ground it stands on. This is the corner that makes the prompt's rule — a tree at least as tall as the sim's mature height starts at least `tree.mature_age` — hold by construction. |
| `bundle.tree_tall_height` | 20.0 | Scene metres above which every tree imports at the same age: a full-grown street tree. The Capitol's tallest is 23.59 m, its mean 13.70 m, so the scene lands mostly on the segment between the two corners and only a handful of trees sit on the flat. |
| `bundle.tree_tall_age` | 3000 | Age a tree of `tree_tall_height` or more starts at: half of `tree.max_age` = 6000. Lifespans are `max_age × (1 ± 0.2)`, i.e. 4800–7200 ticks, so the oldest imported tree still has 1800 ticks of life and the imported cohort thins out over thousands of ticks instead of dying together. At the Capitol the cohort thins from 79 at tick 0 to 4 at tick 5000, and the last imported tree dies between ticks 5600 and 5700, by which time 779 of the sim's own trees stand. Read as `max(tree_tall_age, tree.mature_age)`, so it cannot make the height-to-age map fall. |
| `bundle.tree_move_radius` | 2.0 | Columns (metres) a trunk may be moved off an unplantable column before it is dropped, straight from the shot prompt's "within 2 m". At the Capitol 2 of 81 trees stand on Rock and neither has a plantable column within the radius, so both are dropped and nothing moves. |

The acceptance line that forces `tree_mature_height` and `tree_tall_age` is "a flat bundle with one
15 m tree gives one mature tree at the right column" — 15 m has to import as Mature, which any
`tree_mature_height` ≤ 15 satisfies, and the pair is pinned exactly by
`import_age_hits_the_growth_curve_at_its_corners` in `src/plants.rs`. The line that forces
`tree_move_radius` is "a tree on a roof block moves or drops as rule 1 says", and the Capitol line
"loading gives the documented tree count, moves and drops" pins all four at once through
`the_capitol_scene_plants_its_trees_and_shrubs` in `tests/bundle.rs`: 81 trees → 79 planted, 0 moved,
2 dropped, 0 merged.

There is no sweep: none of the four affects a noise run, and the one real bundle in the repo is the
Capitol, which `sweeps/capitolG3/FINDINGS.md` reports in full.

**One existing default this shot deliberately did not change.** `climate.rain_gradient` = 0.6, set
in shot 15 for the 256 × 64 strip, gives the west edge of a world 40% of the mean rain and the east
edge 160%. On the 256 m Capitol that is a 2.5× rainfall difference across two city blocks: 24 of the
30 trees that ever stood west of the middle die of drought in the first 2000 ticks, and with
`tree.seed_radius` = 6 and `tree.immigration_floor` = 0 nothing can disperse back, so the western
half ends the run treeless while the east closes into woodland. `ecosim check` still passes on the
Capitol at every line, and the shot prompt says not to retune when it does, so 0.6 stands. The
question of whether a real site should run at `rain_gradient` = 0 belongs to G4, which puts storms
and runoff on the same field.
## Flat rainfall on a bundle world (shot G3a)

**No default changed.** The one change is an override carried by every garden-series run on a bundle
world.

| key | default | bundle-world runs | why |
|---|---|---|---|
| `climate.rain_gradient` | 0.6 (shot 15, unchanged) | 0, via `--set climate.rain_gradient=0` | 0.6 is a synthetic west-to-east ramp built for the 256 × 64 noise strip, where sorting an ecology along a climate gradient is the world's purpose. A bundle world is 256 m of photographed ground; the same value there is a 2.5× rainfall difference between one edge of the Capitol square and the other, with nothing measured behind it. |

Before and after on the reference run (seed 42, 20000 ticks, animals off; `sweeps/capitolG3-flat/FINDINGS.md`
has the full side-by-side):

| | ramp 0.6 (G3) | flat (G3a) |
|---|---|---|
| trees at tick 20000 | 2278 | 1982 |
| trees west of x = 128 at tick 20000 | 0 | 424 |
| canopy, % of plantable | 27.0% | 20.1% |
| mean moisture west / east at tick 10000 | 58.0 / 172.8 | 162.6 / 139.3 |
| tree deaths: crowded / drought / burnt / old age | 4075 / 25 / 17 / 1617 | 1656 / 3315 / 2055 / 311 |
| fire ignitions / spreads / burnouts | 89 / 81 / 170 | 75 / 576 / 651 |
| `ecosim check` | PASS, thinnest margin `fertility_mean` +0.0538 | PASS, thinnest margin `fertility_mean` +0.0076 |
| wall time | 9584 ms | 9694 ms |

**The acceptance line that forced it** is G3a's "the west half is populated: trees stand west of
x = 128 at tick 20000". It is the only acceptance line in the shot that any parameter could move, and
the override clears it with 424 trees. Nothing else was touched: no `[tree]`, `[fire]` or `[climate]`
default moved, the noise worlds keep the ramp, and seeds 1, 2, 3 and 42 pass `ecosim check` with the
same numbers and the same committed manifest as before.

**Why the ramp stays the default.** Flattening `climate.rain_gradient` in `params.toml` would retune
the 256 × 64 strip — the regression anchor — to fix a bundle world, which the sim-shot rules forbid.
`DECISIONS.md` (shot G3a) has why this is a `--set` rather than a `[bundle]` key or a special case
inside `World::from_bundle`.

**Not tuned, and worth watching.** `fertility_mean` now peaks at 218.33 against its 220 ceiling. The
honest reading is that the invariant's band was drawn for a half-vegetated site; the first of G4, G5
or G9 to touch that field should look at it, with the number above as the before.

## Shot G4 — the water tier

Every value here is new in this shot, so "before" is the first draft that went into `params.toml`
and "after" is what the acceptance lines left standing. No pre-existing default moved: G4's
acceptance allows moving only `rain.*` and `hydro.*`, and nothing else was touched.

| Parameter | Before | After | Why, and the acceptance line that forced it |
|---|---|---|---|
| `rain.et_mm_h` → `hydro.et_mm_h` | 0.25 | 0.12 | 0.25 mm/h is 5.5 mm over a 10-tick soil update (0.55 mm in one tick) against a lawn's 150 mm field capacity, so soil water fell to zero between storms and the strip's trees died of drought in the first few thousand ticks. (Shot G4b corrected this cell, which said 5.5 mm a tick and 40 mm of field capacity; the numbers behind the change are unaffected.) Forced by the regression anchor, "seeds 1, 2, 3 and 42 still pass `ecosim check` at 20000 ticks at the defaults". |
| `rain.storm_p` | 0.047 | 0.10 | The draft kept the pre-G4 mean of 1 mm per tick at a 21 mm storm, which is the wrong end of the sweep's safe band (see below): trees died of drought on two of the three seeds. At a 10 mm mean the same long-run total arrives often enough that the soil never empties. Same acceptance line. |
| `hydro.leach_k` | 0.02 | 0.0002 | Leaching is fertility's only sink (the operator's note for this shot). At 0.02 a typical 20 mm percolation takes 40% of a column's fertility per soil update and `fertility_mean` fell through the floor of 40 within 2000 ticks. 0.0002 is the largest round value that holds both ends. Forced by `ecosim check`'s `fertility_mean in [40, 220]`, on the anchor seeds and on the new `check --long` line. |
| `hydro.saturation` | (none: field capacity was the ceiling) | 1.2 | With the ceiling at field capacity there is no water above it to drain, so percolation was always zero and so was leaching — fertility kept saturating. 1.2 gives each column 20% of its capacity as the transient store that drains and leaches. Forced by the same `fertility_mean` line, via `check --long`. |

**The result of the `fertility_mean` work**, which is the operator's note in full: over 200000 ticks
of seed 42 at the defaults, `fertility_mean` peaks at 128.0 — its value at tick 0 — bottoms at 46.9
at tick 39899 and averages 65.6, against a pre-G4 `runs/long42` that reached the 255 ceiling by tick
53100 and stayed there. `check --long` now has a `fertility_mean` line, so a future regression is a
test failure rather than a reading of the CSV.

**`rain.storm_mean_mm` stays at 10 and the safe band is 5-10.** The sweep (`sweeps/shotG4/FINDINGS.md`)
holds the long-run rainfall fixed and varies storm size: 3 of 3 cells pass at means 5 and 10, 2 of 3
at 20 (seed 1 loses its trees at tick 1450) and 0 of 3 at 40, where 39 of the first 40 tree deaths are
`drought`. Bigger storms deliver the same water in fewer, larger pulses, most of which runs off
instead of soaking in — the lawn's runoff fraction goes 0.051, 0.207, 0.485, 0.710 across the four
sizes — so the mean is unchanged but the dry spells between storms get longer. The default sits at
the wet edge of the band, not its middle, and that is deliberate: it is the value at which the 1 mm
per tick of the pre-G4 rain arrives in storms a real site would recognise.

## Shot G4b — units calibration

Every row below is a default that moved, and most of them moved **without changing behaviour**: a rate
that was "0.05 per 10-tick update" is written "20 per year" because the shipped cadence is 400 updates
a year, and the sim computes the same increment either way. Those rows say *unit only*. The rows that
are a real change of value are the ones the audit found to be wrong rather than mis-scaled, and each
names the published reference it was set against (`UNITS.md`, R1–R13) — not the health check it had to
pass, which is the calibration discipline the prompt asked for.

The acceptance lines referred to below, from `overnight/shots/G4b-units-calibration.md`:

- **(A)** "Annual rainfall at the site's defaults falls within the normal range for the site's region."
- **(B)** "Annual plant water use falls within the published range for its cover type."
- **(C)** "The soil moisture field sits between wilting point and field capacity for the texture on
  most days" — the new `moisture_band` check.
- **(D)** "Fertility does not pin at either bound over a 50-year run."
- **(E)** "Reference worlds pass the re-derived health checks" — the replacement anchor of override 2.
- **(F)** "Changing the staggered update interval for any subsystem by a factor of two changes that
  subsystem's totals over a year by less than a stated tolerance."

| Parameter | Before | After | Kind | Why, and the acceptance line that forced it |
|---|---|---|---|---|
| `[schedule]` (4 keys) | hard-coded 10, 10, 100, 10 | `cover_every` 10, `soil_every` 10, `temperature_every` 100, `fire_every` 10 | new, no change | The cadences had to be reachable before **(F)** could be written at all, and `sim.rs` and `abiotic.rs` encoded the soil cadence twice. Same values, so no run moves. |
| `rain.storm_p` → `rain.annual_mm` | 0.10 per tick | 800.0 mm/yr | value | The old triple was 0.1 × 4000 × 10 = **4000 mm a year**, five times the site's normal. The per-tick probability is now derived from the annual depth, so it no longer drifts when `year_len` does. **(A)**, R1. |
| `rain.storm_mean_mm` | 10.0 | 6.0 | value | 800 mm in 10 mm storms is 80 rain days; the site records about 130. 6 mm gives 133. **(A)**, R2. |
| `hydro.et_mm_h` | 0.12 | 0.05 | value | 0.12 mm/h is 1052 mm a year at full cover, about double the published range for well-watered temperate grass; 0.05 is 438 mm. **(B)**, R4. |
| `hydro.evap_mm_h` | 0.05 | 0.08 | value | 0.05 mm/h is 438 mm a year off open water, well under the region's ~700 mm; 0.08 is 701 mm. **(B)**, R5. |
| `hydro.leach_k` | 0.0002 | 0.0008 | value | The old value was fitted against 4000 mm of rain a year. At the corrected 800 mm the site drains ~320 mm a year, and 0.0008 leaches 26% of a column's fertility over that — inside the published 15–40% for nitrate loss. Measured: at 0.0002 the 50-year Capitol run peaks at 238.7, over `check`'s 220 ceiling, where at 0.0008 the same run holds [52.2, 129.0]. **(D)**, R13. |
| `climate.decay_k` | 0.015 per soil update | 6.0 per year | unit only | 400 soil updates a year. Deliberately not retuned: 6.0 a year is a two-month litter turnover against a published 1–3 years (R11), and that 10× discrepancy is a finding handed to G5 with the field it acts on. |
| `cover.moisture_draw` → `cover.water_per_growth_mm` | 15 (index units) | 8.8 mm | unit only at the reference soil, rule fixed | 15 of 255 on a 150 mm soil is 8.8 mm, so the reference world sees the same draw. The **rule** changed: the old draw scaled with the column's own capacity, so a plant on a deeper soil paid more water for the same growth. **(B)**, and `UNITS.md` finding 1. |
| `grass.r`, `grass.g` | 0.05, 0.005 per update | 20.0, 2.0 per year | unit only | 400 cover updates a year. **(F)**. |
| `shrub.r`, `shrub.g` | 0.01, 0.004 per update | 4.0, 1.6 per year | unit only | Same. |
| `grass.moisture` | [20, 80, 255, 256] | [0.0784, 0.3137, 1.0, 1.004] | unit only | The curve is now a fraction of available water capacity: 20/255, 80/255, capacity, just above capacity. **(C)**. |
| `shrub.moisture` | [15, 60, 255, 256] | [0.0588, 0.2353, 1.0, 1.004] | unit only | Same. |
| `tree.moisture` | [30, 100, 255, 256] | [0.1176, 0.3922, 1.0, 1.004] | unit only | Same. |
| `tree.moisture_draw` → `tree.transpiration_mm_h` | 50 (index units) / 50 ticks | 0.0342 mm/h | value | The old draw was **2352 mm a year** over the trunk column — three times the top of the published range for an open-grown deciduous tree, and the largest single unit error the audit found. 0.0342 mm/h is 300 mm a year, the bottom of that range, chosen low because the draw is charged to one column while a mature crown covers nine (`UNITS.md` finding 3). **(B)**, R8. |
| `tree.dry_moisture` → `tree.dry_fraction` | 30 (of 255) | 0.12 | unit only | 30/255 = 0.1176, rounded. Drought now means "below 12% of available water capacity", which is inside the published severe-stress band of 10–20% (R9). **(C)**. |
| `fire.base_rate` | 0.002 per fire update | 0.8 per patch per year | unit only | 400 fire updates a year. **Not retuned**: the operator's note of 2026-09-20 02:36 asks for the resulting ignition count to be reported, not fixed. `sweeps/shotG4b/FINDINGS.md` has it before and after; backlog row G4d is where the direction question lives. |

**What moved in the reference runs** is in `sweeps/shotG4b/FINDINGS.md` in full, including the
per-species event-cause breakdown the reporting rule asks for. The short version, on the 256 × 64 strip
at seeds 1, 2, 3 and 42: rain falls from about 4000 to 763–857 mm a year, storms from ~2100 to 653–703
in a 20000-tick run, soil water settles at a whole-run mean of 82–113 mm of the soil's 150 mm available
capacity instead of sitting near saturation, and 413 trees are mature at 2.5 years on seed 42 where the
check needs 35. All four seeds pass `ecosim check` on all 12 short invariants including the new
`moisture_band`. The thinnest margin in the set is seed 1's `mature_trees_10k` at +0.0857 (38 mature
against 35), which is named in FINDINGS as the number to watch.

**Two test-local forcings moved, and no default moved with them.** Both are `--set` values inside
forced-extinction tests, whose job is to prove that a mechanism taken to an extreme kills a population
without panicking:

| Test | Forcing | Before | After | Why |
|---|---|---|---|---|
| `forced_grazer_extinction_on_the_strip_runs_to_the_end` | `grazer.energy_cost` | 2.0 | 3.5 | At 2.0 the strip's grazers now survive the run (75 left at tick 20000) because the corrected rain grows more grass. 3.0 kills the hunters first and leaves the last grazers to die of old age, which is not what the test is named for; 3.5 starves them out at tick 116, which is. |
| `forced_fire_extinction_runs_to_the_end_and_is_attributed_to_fire` | `fire.base_rate` | 20 | 8000 | The same rate in the new unit: 20 per 10-tick update is 8000 a year. At 20 per year the per-update probability is 0.05 and nothing burns. The assertions (both animal species extinct, `total_burnt` > 1000 — measured 27870) are unchanged. |

The six forced-extinction tests also moved world, from `common::SQUARE` to the new `common::SMALL`
(the same 64-world at the default rain gradient instead of flat). That is not a tuning change — no
parameter's default moved — but it is the reason four of them went red mid-shot, and
`DECISIONS.md` has why a flat, evenly watered world cannot keep a tree at 800 mm of rain a year.

## Shot G4c — units calibration, part two: light and tree demography

Everything in this section is a **unit only** change. Not one default moved in value; the seven
tree-tier keys are exactly the tick counts they replace at the shipped `year_len = 4000`, and the
five light keys are the old 0–255 index values divided by 255. Two **rules** changed with them, and
they are the only reason any reference run moved: `canopy_absorb`'s subtraction became an
exponential, which moves every world, and seeding stopped being a schedule that could only fire at
ages divisible by `tree.update_every`, which moves bundle worlds only.

The acceptance lines referred to below, from `overnight/shots/G4c-units-calibration-rest.md`:

- **(L)** "a column under a closed canopy receives a transmittance inside the published band for its
  leaf area index, and the germination threshold is stated as a fraction of full sun"
- **(R)** "the reference site worlds and the test strip still run to their full length with plants
  surviving at the converted defaults" and "reference worlds pass the re-derived health checks"
- **(U)** "every row this shot converts is marked `converted` in `UNITS.md`, with its old and new
  unit and its reference or `model` status"

| Parameter | Before | After | Kind | Why, and the acceptance line that forced it |
|---|---|---|---|---|
| `world.canopy_absorb` → `world.canopy_k` + `world.canopy_lai` | 100, subtracted per canopy voxel from 255 | 0.5 and 2.0, an optical depth of 1.0 per voxel in `exp(−k·LAI·layers)` | rule | The old rule was a subtraction where the physics is a product, so a canopy could only be black, never dark: three voxels saturated at 0. At k·LAI = 1.0 a mature two-voxel crown is LAI 4 and transmits 13.5%, inside R10's 10–25% at LAI 3–5. **(L)**, R10. This is the only key in the shot that changes a reference run. |
| `grass.light` | [100.0, 200.0, 255.0, 256.0] | [0.3922, 0.7843, 1.0, 1.0039] | unit only | The same curve as a fraction of full sun: each value ÷ 255. **(L)**. |
| `shrub.light` | [40.0, 100.0, 200.0, 254.0] | [0.1569, 0.3922, 0.7843, 0.9961] | unit only | Same. |
| `tree.light` | [60.0, 150.0, 255.0, 256.0] | [0.2353, 0.5882, 1.0, 1.0039] | unit only | Same. |
| `tree.sapling_light` | 150.0 | 0.5882 | unit only | 150/255. The germination threshold **(L)** asks to be stated: light suitability is 0 below 23.5% of full sun and 1 at or above 58.8%. |
| `bundle.shade_slope` → `bundle.sun_altitude_deg` | 1.0 | 45.0 | unit only | `1/tan(45°) = 1` exactly, so no building's shadow moved by a single voxel. The old name hid a sun angle inside a slope; Lansing's noon sun near the equinox is 47°. **(U)**. |
| `tree.initial_age` → `initial_age_years` | 500 ticks | 0.125 yr | unit only | 0.125 × 4000 = 500. **(U)**. |
| `tree.young_age` → `young_age_years` | 500 ticks | 0.125 yr | unit only | Same. |
| `tree.mature_age` → `mature_age_years` | 1000 ticks | 0.25 yr | unit only | Same. **Deliberately not retuned**: 0.25 years to a 3 m canopy is ~25× the published 5–8 (R12), and the decision to convert the unit and leave the value is in `DECISIONS.md` and `UNITS.md` section 7. |
| `tree.max_age` → `max_age_years` | 6000 ticks | 1.5 yr | unit only | Same, against a published 60–150 yr lifespan (R12). |
| `tree.dry_death_ticks` → `dry_death_days` | 500 ticks | 45.66 d | unit only | 45.66 × 24 / 2.1915 = 500.0. The one tree constant the audit found already right (R9), and therefore the calibration point the other six are measured against. |
| `tree.seed_every` → `tree.seeds_per_year` | 200 ticks | 20.0 /yr | unit only, rule fixed | 4000/20 = 200. The **rule** changed with it: seeding fired on `age % seed_every == 0`, and a tree's age advances in whole `update_every` steps, so a rate whose interval `update_every` does not divide seeded essentially never. Now the test is "the update whose age crosses a multiple", identical at 50 and 200 **for a tree whose age starts at a multiple of `update_every`** and correct everywhere else. That is every tree on the strip but not one imported from a bundle: 63 of the Capitol's 79 imported trees have an age that is not a multiple of 50 and so never seeded at all before this shot. Capitol germination 7645 → 19408, trees at 20000 1619 → 4082; the strip is untouched by it. `UNITS.md` finding 12. |
| `bundle.tree_tall_age` → `tree_tall_age_years` | 3000 ticks | 0.75 yr | unit only | 0.75 × 4000 = 3000. |

**What moved in the reference runs.** `sweeps/shotG4c/FINDINGS.md` has it in full, including the
per-species event-cause breakdown the reporting rule asks for. The short version, on the 256 × 64
strip at seeds 1, 2, 3 and 42 and on the Capitol: every seed still passes every `ecosim check`
invariant **(R)**, and the world gets woodier. Germination rises on all four strip seeds (seed 1
2107 → 5097, seed 42 3027 → 3953), trees at tick 20000 rise with it (seed 1 570 → 1198, seed 42
793 → 1123), and the thinnest margin in the whole G4b set — seed 1's `mature_trees_10k` at 38
against a floor of 35 — becomes 701. The Capitol moves much further (germination 7645 → 19408,
trees at 20000 1619 → 4082, fire roughly doubled), and that is the seeding fix rather than the
light; its tree count now oscillates, and `mature_trees_10k` is read in a trough, so it falls
944 → 325 while every other Capitol tree number rises. 325 is still 9× the floor. The cost is paid by the ground cover
and by the grazers that eat it: minimum `grass_mean` falls on every seed (seed 42 0.3027 → 0.2441)
and grazer starvation deaths rise sharply (seed 42 253 → 1644). Both follow from the same change:
a young one-voxel canopy used to pass 61% of full sun and now passes 37%, so the shade under a
young tree is real for the first time, grass under it thins, and the tree seedlings that used to
lose to that grass now win.

No parameter was retuned in response. **(R)** held at the converted defaults on all five reference
runs, which is what the shot's override 2 substitutes for the byte-identical anchor.

## Shot G4e — closing the units series

One key, and it is a **unit-only** change with no rule behind it. The acceptance line that forced it
is `overnight/shots/G4e-units-close.md` (a): "convert `tree.immigration_interval` … to a rate per
year, following what G4c did to `seed_every`", with "defaults must not move: the derived cadence at
the shipped params must come out at exactly 500, and every reference run must stay byte-identical".

| Parameter | Before | After | Kind | Why, and the acceptance line that forced it |
|---|---|---|---|---|
| `tree.immigration_interval` → `tree.immigrants_per_year` | 500 ticks | 8.0 /yr | unit only | 4000/8 = 500 exactly, so the cadence does not move a tick. It is the last schedule in the tree tier, and a rate per year is what the tier's other schedule (`seeds_per_year`, G4c) already is. Acceptance (a) and (4). |
| `grazer.immigration_interval`, `hunter.immigration_interval` | 500 ticks | 500 ticks | **not touched** | Deferred with the parked animal tier, on the same decision as its other 45 keys (`UNITS.md` section 6, `DECISIONS.md` shot G4e). Converting a parked tier's cadence is work its unparking shot would re-judge. |

**Nothing moved in any reference run, and that is measured rather than assumed.** A binary built from
179762d, reading the pre-shot `params.toml`, was compared with this one by `ecosim diff` on seed 42
on the 256 × 64 strip (20000 ticks, snapshots every 100) and on the Capitol (20000 ticks, animals
off, flat rain). Both report exactly one line, `differs: meta.json`, and the only difference inside
`meta.json` is `params.tree.immigration_interval: 500` becoming `params.tree.immigrants_per_year:
8.0`. `series.csv`, `events.csv` and every snapshot file are byte-identical, so **the event-log cause
breakdown this project's reporting rule asks for is unchanged from G4c's**, to the byte:
`sweeps/shotG4c/FINDINGS.md` is still the current one. No sweep was run, and the shot prompt asks for
none.

The identity is also weaker than it looks, and the shot did not lean on it: `tree.immigration_floor`
is 0 at the defaults, so no immigration check has ever planted anything in a reference run. The real
check on the derivation is `animals.rs`'s `a_tree_immigrates_at_its_rate_whatever_the_rate_is`, which
raises the floor and lands the first arrival on the derived tick at 8, 4, 20, 1 and 4000 checks a
year, and gets no arrival at all in 20000 ticks at a rate of 0.

## Shot S3 — the crown fractions

Two new keys and **no changed default**: nothing that existed before this shot has a different value
after it, and every reference run's `series.csv`, `events.csv` and snapshot fields are byte-identical
(`sweeps/shotS3/FINDINGS.md`, and the two tests that assert it). The row is the specification and it
is the line that forced these keys: "*Shot V3 meanwhile draws a crown of radius `0.30 x height` (6 m
on a 20 m tree), and **that constant is the well-founded one**: V3 measured it from the 81 surveyed
trees in `ecosim/worlds/capitol/trees.json`*". A number that decides what the simulator publishes
cannot live only in a viewer, and CLAUDE.md's "all species and tuning parameters live in
`params.toml`" leaves nowhere else to put it.

| Parameter | Before | After | Kind | Why, and the line that forced it |
|---|---|---|---|---|
| `tree.crown_radius_frac` | — (did not exist; `ecoview-native` held 0.30 privately) | 0.30 | new, measured | Median of `crown_radius/height` over the 81 trees in `worlds/capitol/trees.json`: 0.3040. The row, naming V3's constant as the well-founded one. |
| `tree.crown_base_frac` | — (same, 0.37) | 0.37 | new, measured | Median of `crown_base/height` over the same 81 trees: 0.3684. Needed with the radius to fix the crown envelope, and so how deep a neighbour's crown a ray passes through. |

Both are rounded to two places from the survey median, which is within the 0.005 tolerance
`the_crown_fractions_are_the_surveyed_medians` allows and well inside the survey's own spread
(radius/height sd 0.126 over n=81). The mean-of-ratios would be 0.3287 and 0.3851; the medians were
taken because the survey's ratio distribution has a long right tail of young stems whose crowns are
wide for their height, and V3 took the medians too (its comment calling them means is a documentation
error, recorded in `sweeps/shotS3/FINDINGS.md`).

**Neither key can move an ecology number.** They are read only by `Sim::crown_of`, which is called
only from `Sim::crowns`, which is called only by the snapshot writer. The sweep over
`tree.crown_radius_frac` at 0.15, 0.30, 0.60 and 1.20 on seeds 1–3 is in `sweeps/shotS3/FINDINGS.md`:
every `check` margin is identical to four decimal places at all four radii, which is the sweep
reporting the absence of a mechanism rather than the shape of one. What the radius does move is the
published `crown_light`, and that table is there too.
