//! Heritable animal traits: what each animal carries, how a newborn inherits it, and the per-species
//! trait statistics written to `series.csv`.

use crate::animals::{Animal, Kind};
use crate::sim::Sim;
use rand::Rng;

/// Heritable traits. Each stands in for a species parameter wherever that parameter was used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Traits {
    /// Multiplies the species' `energy_cost` (default 1).
    pub energy_cost_mult: f32,
    /// Replaces `flee_radius`: a grazer flees hunters within this distance. Hunters carry it too,
    /// but nothing a hunter does reads it.
    pub flee_distance: f32,
    /// Replaces `repro_energy`: energy above which the animal reproduces.
    pub repro_threshold: f32,
}

/// Number of heritable traits.
pub const TRAITS: usize = 3;

impl Traits {
    /// Trait names, in `as_array` order, as used in `series.csv` columns and `entities.json`.
    pub const NAMES: [&'static str; TRAITS] = ["energy_cost_mult", "flee_distance", "repro_threshold"];

    /// The traits in `NAMES` order.
    pub fn as_array(self) -> [f32; TRAITS] {
        [self.energy_cost_mult, self.flee_distance, self.repro_threshold]
    }

    /// Traits from values in `NAMES` order.
    pub fn from_array([energy_cost_mult, flee_distance, repro_threshold]: [f32; TRAITS]) -> Traits {
        Traits { energy_cost_mult, flee_distance, repro_threshold }
    }
}

/// Clamp bounds of a trait, as multiples of the species default.
pub const TRAIT_CLAMP: (f32, f32) = (0.25, 4.0);

/// A newborn's value of one trait: `parent · (1 + mutation · u)` for u in [−1, 1], clamped to
/// [0.25, 4] × the species default.
pub fn inherit(parent: f32, default: f32, mutation: f32, u: f32) -> f32 {
    let (a, b) = (TRAIT_CLAMP.0 * default, TRAIT_CLAMP.1 * default);
    (parent * (1.0 + mutation * u)).clamp(a.min(b), a.max(b))
}

/// Mean and population standard deviation of each heritable trait over the live animals, indexed
/// `[Kind as usize][2 · trait + (0 mean, 1 sd)]`, traits in `Traits::NAMES` order. Both are 0 for a
/// species with no live animals.
pub type TraitStats = [[f32; 2 * TRAITS]; 2];

/// Mean and population standard deviation of each trait over `animals` (live ones only).
pub fn trait_stats(animals: &[Animal]) -> [f32; 2 * TRAITS] {
    let live: Vec<[f32; TRAITS]> = animals.iter().filter(|a| a.alive).map(|a| a.traits.as_array()).collect();
    let mut out = [0.0; 2 * TRAITS];
    if live.is_empty() {
        return out;
    }
    let n = live.len() as f64;
    for k in 0..TRAITS {
        let mean = live.iter().map(|t| t[k] as f64).sum::<f64>() / n;
        let var = live.iter().map(|t| (t[k] as f64 - mean) * (t[k] as f64 - mean)).sum::<f64>() / n;
        out[2 * k] = mean as f32;
        out[2 * k + 1] = var.sqrt() as f32;
    }
    out
}

impl Sim {
    /// A newborn's traits: its parent's, each mutated by one draw when `mutate` is on.
    pub(crate) fn offspring_traits(&mut self, kind: Kind, parent: Traits, mutate: bool) -> Traits {
        if !mutate {
            return parent;
        }
        let (m, d) = (self.params.heredity.mutation, self.params.default_traits(kind).as_array());
        let mut v = parent.as_array();
        for (t, d) in v.iter_mut().zip(d) {
            let u: f32 = self.rng.gen_range(-1.0..=1.0);
            *t = inherit(*t, d, m, u);
        }
        Traits::from_array(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;
    use crate::sim::{flee_offsets, offsets_within};
    use crate::world::{patch_of, COLS};
    use proptest::prelude::*;

    /// The clamp: a newborn's trait lies in [0.25, 4] × the species default for any parent value,
    /// mutation and draw.
    fn inherit_stays_in_bounds(parent: f32, default: f32, mutation: f32, u: f32) -> Result<(), TestCaseError> {
        let v = inherit(parent, default, mutation, u);
        prop_assert!((0.25 * default..=4.0 * default).contains(&v), "{v} outside the clamp of default {default}");
        Ok(())
    }

    fn sim_with(seed: u64, set: impl Fn(&mut Params)) -> Sim {
        let mut p = Params::load_default();
        set(&mut p);
        Sim::new(p, seed)
    }

    /// Every animal in the Vecs (live, and dead since the last compaction) against `check`.
    fn all_animals(sim: &Sim, check: impl Fn(Kind, Traits) -> Result<(), TestCaseError>) -> Result<(), TestCaseError> {
        for a in sim.grazers.iter().chain(&sim.hunters) {
            check(a.kind, a.traits)?;
        }
        Ok(())
    }

    /// Over a run at mutation `m`, every animal's traits stay within the clamp of its species default.
    fn traits_stay_in_bounds(seed: u64, m: f32, ticks: u32) -> Result<(), TestCaseError> {
        let mut sim = sim_with(seed, |p| p.heredity.mutation = m);
        let mut varied = false;
        while sim.tick < ticks {
            sim.step();
            all_animals(&sim, |kind, t| {
                let d = sim.params.default_traits(kind).as_array();
                for (v, d) in t.as_array().into_iter().zip(d) {
                    prop_assert!(
                        (0.25 * d..=4.0 * d).contains(&v),
                        "{kind:?} trait {v}, default {d}, tick {}",
                        sim.tick
                    );
                }
                Ok(())
            })?;
            varied |= sim.grazers.iter().any(|g| g.traits != sim.params.default_traits(Kind::Grazer));
        }
        prop_assert!(varied || ticks < 400, "no grazer trait ever moved at mutation {m}");
        Ok(())
    }

    /// At mutation 0 every animal carries the species defaults: the initial ones, the newborns and
    /// the immigrants (grazer and hunter floors high enough to bring some). The trait columns read
    /// the defaults with standard deviation 0.
    fn mutation_zero_keeps_defaults(seed: u64, ticks: u32) -> Result<(), TestCaseError> {
        let mut sim = sim_with(seed, |p| {
            p.heredity.mutation = 0.0;
            (p.grazer.immigration_floor, p.grazer.immigration_interval) = (100_000, 50);
            (p.hunter.immigration_floor, p.hunter.immigration_interval) = (100_000, 50);
        });
        while sim.tick < ticks {
            sim.step();
        }
        all_animals(&sim, |kind, t| {
            prop_assert_eq!(t, sim.params.default_traits(kind));
            Ok(())
        })?;
        let stats = sim.stats().traits;
        for kind in [Kind::Grazer, Kind::Hunter] {
            let d = sim.params.default_traits(kind).as_array();
            let s = stats[kind as usize];
            prop_assert_eq!([s[0], s[2], s[4]], d);
            prop_assert_eq!([s[1], s[3], s[5]], [0.0; 3]);
        }
        Ok(())
    }

    /// Each grazer's flee offsets (the prefix of `flee_offsets` within its distance) are exactly
    /// `offsets_within(distance)`, for every distance the clamp allows.
    fn flee_prefix_is_offsets_within(r: f32) -> Result<(), TestCaseError> {
        let all = flee_offsets(&Params::load_default());
        let n = all.partition_point(|&(_, _, d2)| (d2 as f32).sqrt() <= r);
        prop_assert_eq!(&all[..n], &offsets_within(r)[..]);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(256)))]

        #[test]
        fn prop_inherit_stays_in_bounds(
            default in 0.0f32..100.0,
            k in 0.25f32..=4.0,
            mutation in 0.0f32..=1.0,
            u in -1.0f32..=1.0,
        ) {
            inherit_stays_in_bounds(k * default, default, mutation, u)?;
        }

        #[test]
        fn prop_flee_prefix_is_offsets_within(r in 0.0f32..=16.0) {
            flee_prefix_is_offsets_within(r)?;
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(6)))]

        #[test]
        fn prop_traits_stay_in_bounds(seed in any::<u64>(), m in 0.05f32..=1.0, ticks in 1u32..1200) {
            traits_stay_in_bounds(seed, m, ticks)?;
        }

        #[test]
        fn prop_mutation_zero_keeps_defaults(seed in any::<u64>(), ticks in 1u32..1200) {
            mutation_zero_keeps_defaults(seed, ticks)?;
        }
    }

    #[test]
    fn inherit_regression_clamp_at_both_ends() {
        assert_eq!(inherit(280.0, 70.0, 0.2, 1.0), 280.0, "top of the clamp");
        assert_eq!(inherit(17.5, 70.0, 0.2, -1.0), 17.5, "bottom of the clamp");
        assert_eq!(inherit(1.0, 1.0, 1.0, -1.0), 0.25, "mutation 1 can reach 0; the clamp holds it at 0.25");
        assert_eq!(inherit(70.0, 70.0, 0.0, 1.0), 70.0);
        inherit_stays_in_bounds(4.0, 4.0, 1.0, 1.0).unwrap();
        inherit_stays_in_bounds(0.0, 0.0, 0.5, 1.0).unwrap();
    }

    /// Mutation 1, the largest relative step: parents can halve or double every generation.
    #[test]
    fn traits_regression_seed_42_mutation_one() {
        traits_stay_in_bounds(42, 1.0, 1000).unwrap();
    }

    #[test]
    fn mutation_zero_regression_with_immigrants() {
        mutation_zero_keeps_defaults(7, 1000).unwrap();
    }

    #[test]
    fn flee_prefix_regression_default_and_clamp_ends() {
        for r in [0.0, 1.0, 4.0, 16.0] {
            flee_prefix_is_offsets_within(r).unwrap();
        }
    }

    #[test]
    fn trait_stats_mean_sd_of_live_animals_only() {
        let t = |e: f32| Traits { energy_cost_mult: e, flee_distance: 2.0 * e, repro_threshold: 70.0 };
        let mut v: Vec<Animal> =
            [1.0, 3.0, 50.0].iter().map(|&e| Animal::new(0, Kind::Grazer, (0, 0), 50.0, 0, 0, t(e))).collect();
        v[2].alive = false;
        assert_eq!(trait_stats(&v), [2.0, 1.0, 4.0, 2.0, 70.0, 0.0]);
        v.iter_mut().for_each(|a| a.alive = false);
        assert_eq!(trait_stats(&v), [0.0; 6], "no live animals: all 0, never NaN");
    }

    /// A birth with mutation on makes exactly three draws (one per trait); with it off, none, and the
    /// newborn is a copy of its parent.
    #[test]
    fn mutation_draws_three_per_birth_and_none_when_off() {
        let birth = |mutate: bool| {
            let mut sim = Sim::bare(&vec![14u8; COLS]);
            sim.params.heredity.mutation = 0.2;
            let parent = Traits { energy_cost_mult: 1.5, flee_distance: 3.0, repro_threshold: 60.0 };
            let before = sim.rng.get_word_pos();
            let child = sim.offspring_traits(Kind::Hunter, parent, mutate);
            (child, sim.rng.get_word_pos() - before, parent)
        };
        let (child, draws, parent) = birth(false);
        assert_eq!((child, draws), (parent, 0));
        let (child, draws, parent) = birth(true);
        assert_eq!(draws, 3);
        for (c, p) in child.as_array().into_iter().zip(parent.as_array()) {
            assert!((c - p).abs() <= 0.2 * p + 1e-4 && c != p, "{c} from {p}");
        }
    }

    /// The traits stand in for the species params: `energy_cost_mult` scales the cost,
    /// `repro_threshold` gates births, and `flee_distance` sets how far off a hunter is fled.
    #[test]
    fn traits_replace_the_species_params() {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        let gp = sim.params.grazer.clone();
        let d = sim.params.default_traits(Kind::Grazer);
        // No grass, so the grazer can't eat: it wanders one step and pays the doubled cost.
        sim.patches.iter_mut().for_each(|p| p.grass = 0.0);
        let heavy = Traits { energy_cost_mult: 3.0, repro_threshold: 1000.0, ..d };
        sim.grazers.push(Animal::new(0, Kind::Grazer, (30, 30), 95.0, 0, 0, heavy));
        sim.grazers_in_patch[patch_of(30, 30)] = 1;
        sim.rebuild_grazer_grid();
        sim.update_grazer(0, false, false);
        assert_eq!(sim.grazers[0].energy, 95.0 - gp.energy_cost * 3.0 * 2.0, "3x cost, doubled for the step");
        assert_eq!(sim.grazers.len(), 1, "no birth below a repro_threshold of 1000");
        let eager = Traits { repro_threshold: 10.0, ..d };
        sim.grazers[0].traits = eager;
        sim.grazers[0].energy = 20.0;
        sim.update_grazer(0, false, false);
        assert_eq!(sim.grazers.len(), 2, "a repro_threshold of 10 breeds at energy 20");
        assert_eq!(sim.grazers[1].traits, eager, "the newborn copies its parent with mutation off");
        // A hunter 6 columns off: out of reach of the default flee distance (4), fled at 8.
        let (x, y) = sim.grazers[0].col();
        let id = sim.alloc_id();
        let ht = sim.params.default_traits(Kind::Hunter);
        sim.hunters.push(Animal::new(id, Kind::Hunter, ((x + 6) as usize, y as usize), 50.0, 0, 100, ht));
        sim.rebuild_hunter_grid();
        assert_eq!(sim.nearest_hunter(x, y, 4.0), None);
        assert_eq!(sim.nearest_hunter(x, y, 8.0), Some(((x + 6) as f32, y as f32)));
    }
}
