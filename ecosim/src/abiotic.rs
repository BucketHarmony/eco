//! Moisture, fertility, detritus decay and temperature.

use crate::sim::Sim;
use crate::world::{cidx, ColClass, World, COLS, PATCHES, WX, WY};
use std::f32::consts::PI;

/// One Jacobi diffusion step over 4-neighbour soil columns: m' = m + rate·Σ(m_n − m).
/// Flux is pairwise-symmetric, so total mass over soil columns is conserved.
pub fn diffuse(field: &mut [f32], world: &World, rate: f32, scratch: &mut Vec<f32>) {
    scratch.clear();
    scratch.extend_from_slice(field);
    for y in 0..WY {
        for x in 0..WX {
            let c = cidx(x, y);
            if world.class[c] != ColClass::Soil {
                continue;
            }
            let m = scratch[c];
            let mut flux = 0.0;
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if world.is_soil(nx, ny) {
                    flux += scratch[cidx(nx as usize, ny as usize)] - m;
                }
            }
            field[c] = m + rate * flux;
        }
    }
}

impl Sim {
    /// Rain added to every soil column on a soil update: rain_base − rain_amp·sin(2π·tick/year_len).
    pub fn rain(&self, tick: u32) -> f32 {
        let c = &self.params.climate;
        c.rain_base - c.rain_amp * (2.0 * PI * tick as f32 / c.year_len as f32).sin()
    }

    /// Evaporation amount for a column, from its patch temperature.
    pub fn evaporation(&self, c: usize) -> f32 {
        let (x, y) = (c % WX, c / WX);
        let t = self.patches[crate::world::patch_of(x, y)].temperature;
        let cl = &self.params.climate;
        (cl.evap_base + t / cl.evap_div).max(0.0)
    }

    /// Step 3 of the soil update: remove each listed column's evaporation, flooring moisture at 0.
    pub fn evaporate(&mut self, soil: &[usize]) {
        for &c in soil {
            self.moisture[c] = (self.moisture[c] - self.evaporation(c)).max(0.0);
        }
    }

    /// Steps 1–7 of the every-10-ticks soil update, in order.
    pub fn update_soil(&mut self, tick: u32) {
        let soil: Vec<usize> = (0..COLS).filter(|&c| self.world.class[c] == ColClass::Soil).collect();
        // 1. Rain
        let r = self.rain(tick);
        for &c in &soil {
            self.moisture[c] += r;
        }
        // 2. Diffusion
        let mut scratch = Vec::with_capacity(COLS);
        diffuse(&mut self.moisture, &self.world, self.params.climate.diffusion, &mut scratch);
        // 3. Evaporation
        self.evaporate(&soil);
        // 4. Pond wetting
        let pond = self.params.climate.pond_moisture;
        for &c in &soil {
            let (x, y) = ((c % WX) as i32, (c / WX) as i32);
            let wet = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                crate::world::in_bounds(nx, ny) && self.world.class[cidx(nx as usize, ny as usize)] == ColClass::Water
            });
            if wet {
                self.moisture[c] = pond;
            }
        }
        // 5. Clamp moisture
        for &c in &soil {
            self.moisture[c] = self.moisture[c].clamp(0.0, 255.0);
        }
        // 6. Decay detritus into fertility
        let cl = self.params.climate.clone();
        for p in 0..PATCHES {
            let n = self.world.patch_soil[p].len();
            if n == 0 {
                continue;
            }
            let m = self.patch_moisture(p);
            let t = self.patches[p].temperature;
            let rate = cl.decay_k * (t / cl.decay_temp_full).clamp(0.0, 1.0) * m / 255.0;
            let converted = self.patches[p].detritus * rate;
            self.patches[p].detritus -= converted;
            let per = converted / n as f32;
            for i in 0..n {
                let c = self.world.patch_soil[p][i];
                self.fertility[c] += per;
            }
        }
        // 7. Clamp fertility
        for &c in &soil {
            self.fertility[c] = self.fertility[c].clamp(0.0, 255.0);
        }
    }

    /// Patch temperature: base + amp·sin(2π·tick/year_len) − canopy_cool·canopy_fraction.
    pub fn update_temperature(&mut self, tick: u32) {
        let cl = &self.params.climate;
        let season = cl.temp_base + self.params.season.amplitude * (2.0 * PI * tick as f32 / cl.year_len as f32).sin();
        for p in 0..PATCHES {
            let (px, py) = (p % 8, p / 8);
            let mut covered = 0;
            for y in py * 8..py * 8 + 8 {
                for x in px * 8..px * 8 + 8 {
                    if self.canopy_cover[cidx(x, y)] {
                        covered += 1;
                    }
                }
            }
            self.patches[p].temperature = season - cl.canopy_cool * covered as f32 / 64.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;
    use proptest::prelude::*;

    #[test]
    fn diffusion_conserves_mass_minus_evaporation() {
        let params = Params::load_default();
        // All-soil world, well above water level: no ponds.
        let world = World::from_heights(&vec![14u8; COLS], &params);
        let mut m: Vec<f32> = (0..COLS).map(|c| ((c * 37) % 200) as f32 + 30.0).collect();
        let before: f64 = m.iter().map(|&v| v as f64).sum();
        let mut scratch = Vec::new();
        diffuse(&mut m, &world, 0.10, &mut scratch);
        let after: f64 = m.iter().map(|&v| v as f64).sum();
        assert!((before - after).abs() / before < 1e-5, "diffusion changed mass: {before} → {after}");

        // Full soil update with rain off: total drops by exactly Σ evaporation.
        let mut p = params.clone();
        p.climate.rain_base = 0.0;
        p.climate.rain_amp = 0.0;
        let mut sim =
            Sim::with_world(p, rand::SeedableRng::seed_from_u64(1), World::from_heights(&vec![14u8; COLS], &params));
        sim.moisture = m.clone();
        let evap: f64 = (0..COLS).map(|c| sim.evaporation(c) as f64).sum();
        sim.update_soil(10);
        let total: f64 = sim.moisture.iter().map(|&v| v as f64).sum();
        let expect = after - evap;
        assert!((total - expect).abs() / expect < 1e-4, "got {total}, expected {expect}");
    }

    /// Diffusion moves moisture only between soil columns: the total is unchanged (within 1 unit per
    /// 1000 cells of f32 rounding) and rock and water columns are never written.
    fn diffusion_conserves(field: &[u8], heights: &[u8], rate: f32) -> Result<(), TestCaseError> {
        let world = World::from_heights(heights, &Params::load_default());
        let mut m: Vec<f32> = field.iter().map(|&v| v as f32).collect();
        let before: f64 = m.iter().map(|&v| v as f64).sum();
        diffuse(&mut m, &world, rate, &mut Vec::new());
        let after: f64 = m.iter().map(|&v| v as f64).sum();
        prop_assert!((before - after).abs() <= COLS as f64 / 1000.0, "{before} → {after}");
        for c in (0..COLS).filter(|&c| world.class[c] != ColClass::Soil) {
            prop_assert_eq!(m[c], field[c] as f32);
        }
        Ok(())
    }

    /// Evaporation never adds moisture and never takes a column below 0.
    fn evaporation_only_removes(field: &[u8], temps: &[f32], base: f32, div: f32) -> Result<(), TestCaseError> {
        let mut sim = Sim::bare(&vec![14u8; COLS]);
        sim.params.climate.evap_base = base;
        sim.params.climate.evap_div = div;
        for (p, &t) in sim.patches.iter_mut().zip(temps) {
            p.temperature = t;
        }
        sim.moisture = field.iter().map(|&v| v as f32).collect();
        let before = sim.moisture.clone();
        let all: Vec<usize> = (0..COLS).collect();
        sim.evaporate(&all);
        for (c, &b) in before.iter().enumerate() {
            prop_assert!(sim.evaporation(c) >= 0.0);
            prop_assert!(sim.moisture[c] <= b && sim.moisture[c] >= 0.0, "col {c}: {} → {}", b, sim.moisture[c]);
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(64)))]

        #[test]
        fn prop_diffusion_conserves_soil_moisture(
            field in prop::collection::vec(any::<u8>(), COLS),
            heights in prop::collection::vec(6u8..=24, COLS),
            rate in 0.0f32..0.25,
        ) {
            diffusion_conserves(&field, &heights, rate)?;
        }

        #[test]
        fn prop_evaporation_never_adds_or_underflows(
            field in prop::collection::vec(any::<u8>(), COLS),
            temps in prop::collection::vec(-5.0f32..=35.0, PATCHES),
            base in -5.0f32..5.0,
            div in 0.5f32..20.0,
        ) {
            evaporation_only_removes(&field, &temps, base, div)?;
        }
    }

    #[test]
    fn diffusion_regression_checkerboard_with_rock_and_water() {
        let field: Vec<u8> = (0..COLS).map(|c| if (c % WX + c / WX).is_multiple_of(2) { 255 } else { 0 }).collect();
        let heights: Vec<u8> = (0..COLS).map(|c| [6u8, 14, 24][c % 3]).collect();
        diffusion_conserves(&field, &heights, 0.25).unwrap();
    }

    #[test]
    fn evaporation_regression_hot_patch_on_dry_soil() {
        let field = vec![1u8; COLS];
        let temps = vec![35.0; PATCHES];
        evaporation_only_removes(&field, &temps, 2.0, 8.0).unwrap();
        evaporation_only_removes(&field, &vec![-5.0; PATCHES], -5.0, 0.5).unwrap();
    }
}
