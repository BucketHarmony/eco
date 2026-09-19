//! Fire disturbance at patch scale: fuel, ignition, spread to the 4-neighbours and burn-out.
//!
//! A patch ignites on a fire update (every `FIRE_EVERY` ticks) or by spread from a burning
//! neighbour. It burns for `fire.duration` ticks and then burns out: its grass and shrub go to 0,
//! the burnt biomass becomes detritus, each tree whose trunk is in it may die, and its soil gains
//! ash. Animals in a burning patch take damage and flee it (`animals.rs`).

use crate::events::EventKind;
use crate::params::FireParams;
use crate::sim::Sim;
use crate::world::{patch_of, PATCHES, PATCHES_X, PATCH_SIZE};
use rand::Rng;

/// Ticks between ignition updates.
pub const FIRE_EVERY: u32 = 10;

/// f(T): 0 at or below `lo`, 1 at or above `hi`, linear between.
pub fn temp_factor(t: f32, lo: f32, hi: f32) -> f32 {
    if t <= lo {
        0.0
    } else if t >= hi {
        1.0
    } else {
        (t - lo) / (hi - lo)
    }
}

fn dryness(moisture: f32) -> f32 {
    (1.0 - moisture / 255.0).clamp(0.0, 1.0)
}

/// Ignition chance of a patch on one fire update: base_rate · f(T) · (1 − moisture/255)² · fuel,
/// clamped to [0, 1].
pub fn ignition_prob(fp: &FireParams, temperature: f32, moisture: f32, fuel: f32) -> f32 {
    let d = dryness(moisture);
    (fp.base_rate * temp_factor(temperature, fp.temp_min, fp.temp_full) * d * d * fuel).clamp(0.0, 1.0)
}

/// Per-tick chance that a burning patch ignites a neighbour: spread · fuel · (1 − moisture/255),
/// clamped to [0, 1], all of the neighbour.
pub fn spread_prob(fp: &FireParams, moisture: f32, fuel: f32) -> f32 {
    (fp.spread * fuel * dryness(moisture)).clamp(0.0, 1.0)
}

/// The 4-neighbour patches of `p` that exist, in (+x, −x, +y, −y) order.
pub fn patch_neighbours(p: usize) -> impl Iterator<Item = usize> {
    let (px, py) = ((p % PATCHES_X) as i32, (p / PATCHES_X) as i32);
    let rows = (PATCHES / PATCHES_X) as i32;
    [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = (px + dx, py + dy);
        (nx >= 0 && ny >= 0 && nx < PATCHES_X as i32 && ny < rows).then(|| nx as usize + PATCHES_X * ny as usize)
    })
}

/// Centre of patch `p` in column coordinates (between columns, so no column sits on it).
pub fn patch_centre(p: usize) -> (f32, f32) {
    let half = PATCH_SIZE as f32 / 2.0 - 0.5;
    (((p % PATCHES_X) * PATCH_SIZE) as f32 + half, ((p / PATCHES_X) * PATCH_SIZE) as f32 + half)
}

impl Sim {
    /// Patch fuel: grass·0.5 + shrub + detritus·detritus_weight + canopy_fraction·canopy_weight.
    /// A patch with no soil column (all water or rock) has none.
    pub fn fuel(&self, p: usize) -> f32 {
        if self.world.patch_soil[p].is_empty() {
            return 0.0;
        }
        let (fp, pa) = (&self.params.fire, &self.patches[p]);
        let canopy = self.canopy_columns(p) as f32 / (PATCH_SIZE * PATCH_SIZE) as f32;
        pa.grass * 0.5 + pa.shrub + pa.detritus * fp.detritus_weight + canopy * fp.canopy_weight
    }

    /// True while patch `p` burns.
    pub fn is_burning(&self, p: usize) -> bool {
        self.patches[p].burning_ticks_left > 0
    }

    /// True while the patch under column (x, y) burns.
    pub fn burning_at(&self, x: usize, y: usize) -> bool {
        self.is_burning(patch_of(x, y))
    }

    /// Set patch `p` burning; `from` is the burning neighbour it caught from, or None for an ignition.
    fn ignite(&mut self, p: usize, from: Option<usize>) {
        self.patches[p].burning_ticks_left = self.params.fire.duration.max(1);
        match from {
            Some(q) => self.log_with(EventKind::Spread, "", p, None, "", Some(q as u32)),
            None => self.log(EventKind::Ignition, "", p, None),
        }
    }

    /// Fire phase, every tick. First the patches burning at its start spread to each non-burning
    /// 4-neighbour (one draw per neighbour with a nonzero chance), then count down, burning out at 0.
    /// Then, on fire updates with `base_rate` > 0, one ignition draw per patch in patch order.
    /// Patches ignited here burn from the next tick. With `base_rate` 0 nothing ever burns, so the
    /// phase makes no draws and no writes.
    pub fn update_fire(&mut self, t: u32) {
        let burning: Vec<usize> = (0..PATCHES).filter(|&p| self.is_burning(p)).collect();
        for &p in &burning {
            for q in patch_neighbours(p) {
                if self.is_burning(q) {
                    continue;
                }
                let pr = spread_prob(&self.params.fire, self.patch_moisture(q), self.fuel(q));
                if pr > 0.0 && self.rng.gen::<f32>() < pr {
                    self.ignite(q, Some(p));
                }
            }
        }
        for &p in &burning {
            self.patches[p].burning_ticks_left -= 1;
            if self.patches[p].burning_ticks_left == 0 {
                self.burn_out(p);
            }
        }
        if self.params.fire.base_rate > 0.0 && t.is_multiple_of(FIRE_EVERY) {
            for p in 0..PATCHES {
                let pr =
                    ignition_prob(&self.params.fire, self.patches[p].temperature, self.patch_moisture(p), self.fuel(p));
                let u: f32 = self.rng.gen();
                if u < pr && !self.is_burning(p) {
                    self.ignite(p, None);
                }
            }
        }
    }

    /// Burn-out of patch `p`: burnt grass and shrub become detritus, both go to 0, the soil gains
    /// ash, and each tree whose trunk is in the patch dies with chance `tree_kill` (one draw per
    /// tree, in Vec order, only when `tree_kill` > 0).
    pub fn burn_out(&mut self, p: usize) {
        self.log(EventKind::Burnout, "", p, None);
        let fp = self.params.fire.clone();
        let n = self.world.patch_soil[p].len() as f32;
        let pa = &mut self.patches[p];
        pa.detritus += fp.detritus_yield * (pa.grass + pa.shrub) * n;
        pa.grass = 0.0;
        pa.shrub = 0.0;
        for i in 0..self.world.patch_soil[p].len() {
            let c = self.world.patch_soil[p][i];
            self.fertility[c] = (self.fertility[c] + fp.ash).clamp(0.0, 255.0);
        }
        if fp.tree_kill > 0.0 {
            for i in 0..self.trees.len() {
                let t = &self.trees[i];
                if t.alive && patch_of(t.x as usize, t.y as usize) == p && self.rng.gen::<f32>() < fp.tree_kill {
                    self.kill_tree(i, "burnt");
                }
            }
        }
        self.total_burnt += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animals::{Animal, Cause, Kind, State};
    use crate::params::Params;
    use crate::world::{cidx, COLS, WX};
    use proptest::prelude::*;

    fn fire_params() -> FireParams {
        Params::load_default().fire
    }

    /// Heights for one patch kind: 0 water, 1 rock, 2 soil, 3 mixed (per-column from `mix`).
    fn heights_by_patch(kinds: &[u8], mix: &[u8]) -> Vec<u8> {
        (0..COLS)
            .map(|c| match kinds[patch_of(c % WX, c / WX)] {
                0 => 6,
                1 => 24,
                2 => 14,
                _ => mix[c],
            })
            .collect()
    }

    /// Fuel is never negative, and a patch with no soil column (water or rock only) has fuel 0 and
    /// so can neither ignite nor be spread to, whatever its detritus and the canopy over it.
    fn fuel_zero_on_water_and_rock(
        kinds: &[u8],
        mix: &[u8],
        fields: &[(f32, f32, f32)],
        canopy: &[bool],
        weights: (f32, f32),
    ) -> Result<(), TestCaseError> {
        let mut sim = Sim::bare(&heights_by_patch(kinds, mix));
        (sim.params.fire.detritus_weight, sim.params.fire.canopy_weight) = weights;
        sim.params.fire.base_rate = 1.0;
        sim.params.fire.spread = 1.0;
        for (p, &(g, s, d)) in fields.iter().enumerate() {
            (sim.patches[p].grass, sim.patches[p].shrub, sim.patches[p].detritus) = (g, s, d);
        }
        sim.canopy_cover.copy_from_slice(canopy);
        for p in 0..PATCHES {
            let f = sim.fuel(p);
            prop_assert!(f >= 0.0, "patch {} fuel {}", p, f);
            if sim.world.patch_soil[p].is_empty() {
                prop_assert_eq!(f, 0.0, "patch {} has no soil", p);
                prop_assert_eq!(ignition_prob(&sim.params.fire, 30.0, 0.0, f), 0.0);
                prop_assert_eq!(spread_prob(&sim.params.fire, 0.0, f), 0.0);
            }
        }
        Ok(())
    }

    /// A band of all-water patches (patch column `band`) stops every fire: with patches west of it
    /// burning and any spread rate, no patch in or east of the band ever burns.
    fn spread_stops_at_water(
        mix: &[u8],
        band: usize,
        fields: &[(f32, f32, f32)],
        lit: &[bool],
        spread: f32,
        duration: u32,
    ) -> Result<(), TestCaseError> {
        let kinds: Vec<u8> = (0..PATCHES).map(|p| if p % PATCHES_X == band { 0 } else { 3 }).collect();
        let mut sim = Sim::bare(&heights_by_patch(&kinds, mix));
        (sim.params.fire.spread, sim.params.fire.duration) = (spread, duration);
        for (p, &(g, s, d)) in fields.iter().enumerate() {
            (sim.patches[p].grass, sim.patches[p].shrub, sim.patches[p].detritus) = (g, s, d);
        }
        sim.moisture.fill(0.0);
        for (p, &l) in lit.iter().enumerate() {
            if l && p % PATCHES_X < band {
                sim.patches[p].burning_ticks_left = duration.max(1);
            }
        }
        for t in 1..=400 {
            sim.update_fire(t);
            for p in (0..PATCHES).filter(|p| p % PATCHES_X >= band) {
                prop_assert!(!sim.is_burning(p), "patch {} burns at tick {}", p, t);
            }
        }
        Ok(())
    }

    /// A patch that burns out in a tick has grass = shrub = 0 when the tick ends, and each burn-out
    /// counts once in `total_burnt`.
    fn bare_after_burn_out(seed: u64, ticks: u32, spread: f32, base_rate: f32) -> Result<(), TestCaseError> {
        let mut p = Params::load_default();
        (p.fire.spread, p.fire.base_rate, p.fire.temp_min) = (spread, base_rate, -50.0);
        let mut sim = Sim::new(p, seed);
        let mut burnt = 0;
        for _ in 0..ticks {
            let last: Vec<usize> = (0..PATCHES).filter(|&p| sim.patches[p].burning_ticks_left == 1).collect();
            let before = sim.total_burnt;
            sim.step();
            prop_assert_eq!(sim.total_burnt - before, last.len() as u32);
            burnt += last.len();
            for p in last {
                prop_assert!(
                    sim.patches[p].grass == 0.0 && sim.patches[p].shrub == 0.0,
                    "patch {} tick {}",
                    p,
                    sim.tick
                );
            }
        }
        prop_assert_eq!(sim.total_burnt as usize, burnt);
        Ok(())
    }

    /// Ignition chance never falls as fuel rises or as temperature rises, and stays in [0, 1].
    fn ignition_monotone(fp: &FireParams, m: f32, fuels: (f32, f32), temps: (f32, f32)) -> Result<(), TestCaseError> {
        let (f0, f1) = (fuels.0.min(fuels.1), fuels.0.max(fuels.1));
        let (t0, t1) = (temps.0.min(temps.1), temps.0.max(temps.1));
        for &t in &[t0, t1] {
            let (a, b) = (ignition_prob(fp, t, m, f0), ignition_prob(fp, t, m, f1));
            prop_assert!((0.0..=1.0).contains(&a) && a <= b, "fuel {} -> {}: {} -> {}", f0, f1, a, b);
        }
        for &f in &[f0, f1] {
            let (a, b) = (ignition_prob(fp, t0, m, f), ignition_prob(fp, t1, m, f));
            prop_assert!((0.0..=1.0).contains(&b) && a <= b, "temp {} -> {}: {} -> {}", t0, t1, a, b);
        }
        Ok(())
    }

    fn fields() -> impl Strategy<Value = Vec<(f32, f32, f32)>> {
        prop::collection::vec((0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..20_000.0), PATCHES)
    }

    fn mix() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(prop_oneof![6 => 11u8..=20, 1 => 6u8..=9, 1 => 21u8..=24], COLS)
    }

    fn fire() -> impl Strategy<Value = FireParams> {
        (0.0f32..2.0, -10.0f32..30.0, 0.0f32..20.0).prop_map(|(base_rate, lo, span)| FireParams {
            base_rate,
            temp_min: lo,
            temp_full: lo + span,
            ..fire_params()
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(64)))]

        #[test]
        fn prop_fuel_zero_on_water_and_rock(
            kinds in prop::collection::vec(0u8..4, PATCHES),
            mix in mix(),
            fields in fields(),
            canopy in prop::collection::vec(any::<bool>(), COLS),
            weights in (0.0f32..1.0, 0.0f32..5.0),
        ) {
            fuel_zero_on_water_and_rock(&kinds, &mix, &fields, &canopy, weights)?;
        }

        #[test]
        fn prop_ignition_monotone_in_fuel_and_temperature(
            fp in fire(),
            m in 0.0f32..=255.0,
            fuels in (0.0f32..4.0, 0.0f32..4.0),
            temps in (-20.0f32..45.0, -20.0f32..45.0),
        ) {
            ignition_monotone(&fp, m, fuels, temps)?;
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(32)))]

        #[test]
        fn prop_spread_never_crosses_water(
            mix in mix(),
            band in 1usize..PATCHES_X - 1,
            fields in fields(),
            lit in prop::collection::vec(any::<bool>(), PATCHES),
            spread in 0.0f32..5.0,
            duration in 0u32..30,
        ) {
            spread_stops_at_water(&mix, band, &fields, &lit, spread, duration)?;
        }

        #[test]
        fn prop_burnt_patch_is_bare_after_burn_out(
            seed in any::<u64>(),
            ticks in 1u32..300,
            spread in 0.0f32..1.0,
            base_rate in 0.0f32..2.0,
        ) {
            bare_after_burn_out(seed, ticks, spread, base_rate)?;
        }
    }

    #[test]
    fn fuel_regression_canopied_water_and_rock_with_detritus() {
        let kinds: Vec<u8> = (0..PATCHES).map(|p| (p % 3) as u8).collect();
        let fields = vec![(1.0, 1.0, 20_000.0); PATCHES];
        fuel_zero_on_water_and_rock(&kinds, &vec![14; COLS], &fields, &vec![true; COLS], (1.0, 5.0)).unwrap();
    }

    #[test]
    fn spread_regression_certain_spread_from_the_whole_west_side() {
        let fields = vec![(1.0, 1.0, 10_000.0); PATCHES];
        spread_stops_at_water(&vec![14; COLS], 4, &fields, &[true; PATCHES], 1.0e6, 1).unwrap();
    }

    #[test]
    fn burn_out_regression_every_patch_ignites() {
        bare_after_burn_out(42, 250, 1.0, 100.0).unwrap();
    }

    #[test]
    fn ignition_regression_ramp_ends_and_clamp() {
        let fp = FireParams { base_rate: 1.0, ..fire_params() };
        ignition_monotone(&fp, 0.0, (0.0, 1.0e6), (fp.temp_min, fp.temp_full)).unwrap();
        assert_eq!(ignition_prob(&fp, fp.temp_min, 0.0, 1.0), 0.0);
        assert_eq!(ignition_prob(&fp, fp.temp_full, 0.0, 1.0), 1.0);
        assert_eq!(ignition_prob(&fp, fp.temp_full + 10.0, 0.0, 1.0e6), 1.0);
        assert_eq!(ignition_prob(&fp, fp.temp_full, 255.0, 1.0), 0.0, "saturated soil never ignites");
        assert!((ignition_prob(&fp, 22.5, 127.5, 2.0) - 0.25).abs() < 1e-6);
    }

    /// One draw per patch on a fire update and none between them, while nothing burns.
    #[test]
    fn ignition_makes_one_draw_per_patch_on_fire_updates_only() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        sim.params.fire.base_rate = 1.0e-9;
        let w = sim.rng.get_word_pos();
        sim.update_fire(7);
        assert_eq!(sim.rng.get_word_pos(), w);
        sim.update_fire(10);
        assert_eq!(sim.rng.get_word_pos(), w + PATCHES as u128);
        assert!((0..PATCHES).all(|p| !sim.is_burning(p)));
    }

    /// With base_rate 0 the other fire params don't matter: no draw, no write, identical runs.
    #[test]
    fn rate_zero_makes_no_draws_and_no_writes() {
        let mut a = Params::load_default();
        a.fire.base_rate = 0.0;
        let mut b = a.clone();
        b.fire = FireParams { base_rate: 0.0, temp_min: -100.0, spread: 10.0, animal_damage: 50.0, ..b.fire };
        b.fire.ash = 100.0;
        let (mut a, mut b) = (Sim::new(a, 11), Sim::new(b, 11));
        for _ in 0..1500 {
            a.step();
            b.step();
            assert_eq!(a.stats(), b.stats(), "tick {}", a.tick);
            assert_eq!(a.stats().patches_burning, 0);
        }
        assert_eq!(crate::state::encode(&a), crate::state::encode(&b));
    }

    #[test]
    fn burn_out_clears_cover_adds_detritus_and_ash_and_kills_trees_in_the_patch() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        sim.params.fire.tree_kill = 1.0;
        let p = patch_of(10, 10);
        sim.plant_tree(10, 10, 2000);
        sim.plant_tree(20, 20, 2000);
        (sim.patches[p].grass, sim.patches[p].shrub, sim.patches[p].detritus) = (0.5, 0.25, 100.0);
        let f0 = sim.fertility[cidx(12, 12)];
        assert!(sim.fuel(p) > 0.5 * 0.5 + 0.25);
        sim.patches[p].burning_ticks_left = 1;
        sim.update_fire(1);
        let fp = &sim.params.fire;
        assert_eq!((sim.patches[p].grass, sim.patches[p].shrub), (0.0, 0.0));
        let want = 100.0 + fp.detritus_yield * 0.75 * 64.0 + sim.params.tree.death_detritus;
        assert!((sim.patches[p].detritus - want).abs() < 1e-3, "{}", sim.patches[p].detritus);
        assert_eq!(sim.fertility[cidx(12, 12)], f0 + fp.ash);
        assert!(!sim.trees[0].alive && sim.trees[1].alive, "only the tree in the burnt patch dies");
        assert_eq!(sim.world.surface_light(cidx(10, 10)), 255, "light reopens over the dead tree");
        assert_eq!(sim.total_burnt, 1);
        assert!(!sim.is_burning(p));
    }

    #[test]
    fn a_burning_patch_ignites_a_dry_neighbour_at_certain_spread() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        sim.params.fire.spread = 1.0e6;
        sim.params.fire.duration = 3;
        sim.moisture.fill(0.0);
        sim.patches[9].burning_ticks_left = 3;
        sim.update_fire(1);
        let lit: Vec<usize> = (0..PATCHES).filter(|&p| sim.is_burning(p)).collect();
        assert_eq!(lit, vec![1, 8, 9, 10, 17]);
        assert_eq!((sim.patches[9].burning_ticks_left, sim.patches[10].burning_ticks_left), (2, 3));
    }

    /// An animal in a burning patch loses `animal_damage` (plus its move cost), steps away from the
    /// patch centre, and a death there is recorded as `burnt`.
    #[test]
    fn animals_in_a_burning_patch_are_hurt_flee_and_die_burnt() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        sim.params.fire.animal_damage = 5.0;
        let p = patch_of(9, 9);
        sim.patches[p].burning_ticks_left = 5;
        sim.spawn_grazer(9, 9);
        sim.grazers[0].energy = 50.0;
        let id = sim.alloc_id();
        sim.hunters.push(Animal::new(
            id,
            Kind::Hunter,
            (13, 13),
            50.0,
            0,
            100,
            sim.params.default_traits(Kind::Hunter),
        ));
        sim.update_animals();
        let (g, h) = (&sim.grazers[0], &sim.hunters[0]);
        assert_eq!((g.state, h.state), (State::Flee, State::Flee));
        assert_eq!((g.x, g.y, h.x, h.y), (8.0, 8.0, 14.0, 14.0), "both step away from the centre (11.5, 11.5)");
        assert_eq!(g.energy, 50.0 - 5.0 - 2.0 * sim.params.grazer.energy_cost);
        assert_eq!(h.energy, 50.0 - 5.0 - 2.0 * sim.params.hunter.energy_cost);
        sim.grazers[0].energy = 3.0;
        sim.hunters[0].energy = 3.0;
        sim.update_animals();
        assert!(!sim.grazers[0].alive && !sim.hunters[0].alive);
        assert_eq!(sim.deaths[Kind::Grazer as usize][Cause::Burnt as usize], 1);
        assert_eq!(sim.deaths[Kind::Hunter as usize][Cause::Burnt as usize], 1);
    }
}
