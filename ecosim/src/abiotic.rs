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
        for &c in &soil {
            self.moisture[c] = (self.moisture[c] - self.evaporation(c)).max(0.0);
        }
        // 4. Pond wetting
        let pond = self.params.climate.pond_moisture;
        for &c in &soil {
            let (x, y) = ((c % WX) as i32, (c / WX) as i32);
            let wet = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                crate::world::in_bounds(nx, ny)
                    && self.world.class[cidx(nx as usize, ny as usize)] == ColClass::Water
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
        let mut sim = Sim::with_world(p, rand::SeedableRng::seed_from_u64(1), World::from_heights(&vec![14u8; COLS], &params));
        sim.moisture = m.clone();
        let evap: f64 = (0..COLS).map(|c| sim.evaporation(c) as f64).sum();
        sim.update_soil(10);
        let total: f64 = sim.moisture.iter().map(|&v| v as f64).sum();
        let expect = after - evap;
        assert!((total - expect).abs() / expect < 1e-4, "got {total}, expected {expect}");
    }
}
