//! Suitability curves and the staggered patch-density update for grass and shrub.

use crate::params::{CoverSpecies, Curve};
use crate::sim::Sim;

/// Piecewise-linear suitability in [0, 1]: 0 at/below min and at/above max,
/// linear up to 1 at low-opt, 1 through high-opt, linear down to max.
pub fn suitability(c: &Curve, v: f32) -> f32 {
    let [min, lo, hi, max] = *c;
    if v <= min || v >= max {
        0.0
    } else if v < lo {
        (v - min) / (lo - min)
    } else if v <= hi {
        1.0
    } else {
        (max - v) / (max - hi)
    }
}

/// Patch means over soil columns used as producer inputs.
#[derive(Debug, Clone, Copy)]
pub struct PatchEnv {
    /// Mean surface light, as a fraction of full sun (shot G4c: it was the 0–255 index).
    pub light: f32,
    /// Mean soil water, as a fraction of available water capacity: 0 at the wilting point, 1 at
    /// field capacity (shot G4b; it was the 0–255 moisture index).
    pub water: f32,
    /// Mean surface fertility, 0–255.
    pub fertility: f32,
    /// Patch temperature, °C.
    pub temperature: f32,
}

impl Sim {
    /// A patch's producer inputs: means over its soil columns plus its temperature.
    pub fn patch_env(&self, p: usize) -> PatchEnv {
        let cols = &self.world.patch_soil[p];
        let n = cols.len().max(1) as f32;
        let (mut m, mut l, mut f) = (0.0f32, 0.0f32, 0.0f32);
        for &c in cols {
            l += self.world.surface_light_fraction(c);
            m += self.water_fraction(c);
            f += self.fertility[c];
        }
        PatchEnv { light: l / n, water: m / n, fertility: f / n, temperature: self.patches[p].temperature }
    }

    /// Mean surface moisture of a patch's soil columns, on the 0–255 index. Fire and the producers
    /// read [`Sim::patch_water_fraction`] instead; this is what the decay rate, still in its pre-G4
    /// units, uses (UNITS.md "Deferred to shot G4c").
    pub fn patch_moisture(&self, p: usize) -> f32 {
        let cols = &self.world.patch_soil[p];
        cols.iter().map(|&c| self.moisture[c]).sum::<f32>() / cols.len().max(1) as f32
    }

    /// Mean soil water of a patch's soil columns, as a fraction of available water capacity.
    pub fn patch_water_fraction(&self, p: usize) -> f32 {
        let cols = &self.world.patch_soil[p];
        cols.iter().map(|&c| self.water_fraction(c)).sum::<f32>() / cols.len().max(1) as f32
    }

    /// Unsuppressed, un-limited growth rate factor r·f_L·f_M·f_T·f_F for one species, per year.
    fn growth_factor(&self, sp: &CoverSpecies, env: &PatchEnv) -> f32 {
        let f_f = (env.fertility / self.params.cover.fertility_full).clamp(0.0, 1.0);
        sp.r * suitability(&sp.light, env.light)
            * suitability(&sp.moisture, env.water)
            * suitability(&sp.temp, env.temperature)
            * f_f
    }

    /// Producer update for the patches whose turn it is: `p % cover_every == tick % cover_every`, so
    /// every patch is updated once per `schedule.cover_every` ticks.
    pub fn update_producers(&mut self, tick: u32) {
        let every = self.params.schedule.cover_every.max(1);
        for p in 0..self.world.dims.patches() {
            if p as u32 % every == tick % every {
                self.update_patch_cover(p);
            }
        }
    }

    /// Grass and shrub density update for one patch: growth, mortality, litter, soil draw and shrub
    /// spread. The species' `r` and `g` are per year, charged over the `schedule.cover_every` ticks
    /// since this patch's last update (shot G4b).
    pub fn update_patch_cover(&mut self, p: usize) {
        let n_soil = self.world.patch_soil[p].len();
        if n_soil == 0 {
            self.patches[p].grass = 0.0;
            self.patches[p].shrub = 0.0;
            return;
        }
        let env = self.patch_env(p);
        let cp = self.params.cover.clone();
        let (grass, shrub) = (self.patches[p].grass, self.patches[p].shrub);

        let dt = crate::hydro::years(&self.params, self.params.schedule.cover_every.max(1)) as f32;
        let s = (1.0 - cp.grass_suppression * shrub).max(0.0);
        let gg = self.params.grass.g * dt;
        let gs = self.params.shrub.g * dt;
        let dg = self.growth_factor(&self.params.grass, &env) * dt * (1.0 - grass) * s - gg * grass;
        let ds = self.growth_factor(&self.params.shrub, &env) * dt * (1.0 - shrub) - gs * shrub;

        let litter = cp.litter_factor * (gg * grass + gs * shrub) * n_soil as f32;
        self.patches[p].detritus += litter;
        self.patches[p].grass = (grass + dg).clamp(0.0, 1.0);
        self.patches[p].shrub = (shrub + ds).clamp(0.0, 1.0);

        let growth = dg.max(0.0) + ds.max(0.0);
        if growth > 0.0 {
            let (dm, df) = (cp.water_per_growth_mm * growth, cp.fertility_draw * growth);
            for i in 0..n_soil {
                let c = self.world.patch_soil[p][i];
                self.draw_water_mm(c, dm);
                self.fertility[c] = (self.fertility[c] - df).max(0.0);
            }
        }

        if self.patches[p].shrub > cp.shrub_spread_threshold {
            for q in crate::fire::patch_neighbours(self.world.dims, p) {
                if !self.world.patch_soil[q].is_empty() {
                    self.patches[q].shrub = self.patches[q].shrub.max(cp.shrub_spread_seed);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::CoverSpecies;
    use crate::world::sq::*;
    use proptest::prelude::*;

    #[test]
    fn suitability_breakpoints() {
        let c: Curve = [100.0, 200.0, 255.0, 256.0];
        assert_eq!(suitability(&c, 100.0), 0.0);
        assert_eq!(suitability(&c, 200.0), 1.0);
        assert_eq!(suitability(&c, 255.0), 1.0);
        assert_eq!(suitability(&c, 256.0), 0.0);
        assert!((suitability(&c, 150.0) - 0.5).abs() < 1e-6);
        let t: Curve = [0.0, 5.0, 30.0, 35.0];
        assert_eq!(suitability(&t, 0.0), 0.0);
        assert_eq!(suitability(&t, 5.0), 1.0);
        assert_eq!(suitability(&t, 30.0), 1.0);
        assert_eq!(suitability(&t, 35.0), 0.0);
        assert!((suitability(&t, 32.5) - 0.5).abs() < 1e-6);
        assert_eq!(suitability(&t, -10.0), 0.0);
        assert_eq!(suitability(&t, 40.0), 0.0);
    }

    /// The curve's shape: range [0, 1], 0 outside (min, max), 1 on [low-opt, high-opt], rising then
    /// falling. Needs strictly increasing breakpoints (see DECISIONS.md for tied ones).
    fn suitability_shape(c: Curve, x: f32, y: f32) -> Result<(), TestCaseError> {
        let [min, lo, hi, max] = c;
        let f = |v| suitability(&c, v);
        prop_assert!((0.0..=1.0).contains(&f(x)), "f({x}) = {}", f(x));
        for v in [min, max, min - x.abs(), max + x.abs()] {
            prop_assert_eq!(f(v), 0.0, "at {}", v);
        }
        for v in [lo, hi, lo + (hi - lo) * 0.5] {
            prop_assert_eq!(f(v), 1.0, "at {}", v);
        }
        let (a, b) = if x <= y { (x, y) } else { (y, x) };
        if min <= a && b <= lo {
            prop_assert!(f(a) <= f(b), "rising side: f({a}) = {} > f({b}) = {}", f(a), f(b));
        }
        if hi <= a && b <= max {
            prop_assert!(f(a) >= f(b), "falling side: f({a}) = {} < f({b}) = {}", f(a), f(b));
        }
        Ok(())
    }

    /// Four strictly increasing breakpoints in [-50, 300].
    fn curve() -> impl Strategy<Value = Curve> {
        (-50.0f32..300.0, 0.01f32..100.0, 0.0f32..100.0, 0.01f32..100.0)
            .prop_map(|(a, d1, d2, d3)| [a, a + d1, a + d1 + d2, a + d1 + d2 + d3])
            .prop_filter("strictly increasing after rounding", |c| c[0] < c[1] && c[1] <= c[2] && c[2] < c[3])
    }

    fn species() -> impl Strategy<Value = CoverSpecies> {
        (0.0f32..1.0, 0.0f32..0.5, curve(), curve(), curve()).prop_map(|(r, g, light, moisture, temp)| CoverSpecies {
            r,
            g,
            initial: 0.0,
            light,
            moisture,
            temp,
        })
    }

    /// Run `steps` density updates on patch 9 of an all-soil world with no grazers. Returns every
    /// (grass, shrub) pair seen, starting with the initial one.
    fn densities(grass: CoverSpecies, shrub: CoverSpecies, d: (f32, f32), steps: usize) -> Vec<(f32, f32)> {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        sim.params.grass = grass;
        sim.params.shrub = shrub;
        sim.patches[9].grass = d.0;
        sim.patches[9].shrub = d.1;
        let mut seen = vec![d];
        for _ in 0..steps {
            sim.update_patch_cover(9);
            seen.push((sim.patches[9].grass, sim.patches[9].shrub));
            assert!(sim.moisture.iter().chain(&sim.fertility).all(|&v| v >= 0.0), "soil draw went negative");
        }
        seen
    }

    /// Curves that are 1 everywhere the patch environment can reach.
    fn always_suitable(mut s: CoverSpecies) -> CoverSpecies {
        let wide = [-1.0e6, -1.0e6 + 1.0, 1.0e6, 1.0e6 + 1.0];
        (s.light, s.moisture, s.temp, s.g) = (wide, wide, wide, 0.0);
        s
    }

    proptest! {
        #[test]
        fn prop_suitability_shape(c in curve(), x in -100.0f32..450.0, y in -100.0f32..450.0) {
            suitability_shape(c, x, y)?;
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(32)))]

        #[test]
        fn prop_density_stays_in_unit_interval(
            g in species(), s in species(), d in (0.0f32..=1.0, 0.0f32..=1.0), steps in 1usize..=200,
        ) {
            for (grass, shrub) in densities(g, s, d, steps) {
                prop_assert!((0.0..=1.0).contains(&grass) && (0.0..=1.0).contains(&shrub), "{grass} {shrub}");
            }
        }

        #[test]
        fn prop_density_never_falls_without_mortality(
            g in species(), s in species(), d in (0.0f32..=1.0, 0.0f32..=1.0), steps in 1usize..=200,
        ) {
            let seen = densities(always_suitable(g), always_suitable(s), d, steps);
            for w in seen.windows(2) {
                prop_assert!(w[1].0 >= w[0].0 && w[1].1 >= w[0].1, "{:?} → {:?}", w[0], w[1]);
            }
        }
    }

    #[test]
    fn suitability_regression_narrow_curve() {
        suitability_shape([0.0, 0.01, 0.01, 0.02], 0.005, 0.015).unwrap();
    }

    #[test]
    fn suitability_ties_resolve_to_zero() {
        // min == low-opt: "0 at min" and "1 at low-opt" conflict; the 0 rule wins.
        assert_eq!(suitability(&[5.0, 5.0, 10.0, 20.0], 5.0), 0.0);
        assert_eq!(suitability(&[5.0, 8.0, 20.0, 20.0], 20.0), 0.0);
    }

    #[test]
    fn density_regression_full_growth_from_zero() {
        let grass = CoverSpecies {
            r: 0.99,
            g: 0.49,
            initial: 0.0,
            light: [0.0, 1.0, 2.0, 3.0],
            moisture: [0.0, 1.0, 2.0, 3.0],
            temp: [0.0, 1.0, 2.0, 3.0],
        };
        for (a, b) in densities(grass.clone(), grass.clone(), (0.0, 1.0), 200) {
            assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b));
        }
        let seen = densities(always_suitable(grass.clone()), always_suitable(grass), (0.0, 0.0), 50);
        assert!(seen.windows(2).all(|w| w[1].0 >= w[0].0 && w[1].1 >= w[0].1));
    }
}
