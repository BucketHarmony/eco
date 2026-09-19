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
