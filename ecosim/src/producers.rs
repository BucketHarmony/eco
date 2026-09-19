//! Suitability curves and the staggered patch-density update for grass and shrub.

use crate::params::{CoverSpecies, Curve};
use crate::sim::Sim;
use crate::world::{PATCHES, PATCHES_X};

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
    pub light: f32,
    pub moisture: f32,
    pub fertility: f32,
    pub temperature: f32,
}

impl Sim {
    pub fn patch_env(&self, p: usize) -> PatchEnv {
        let cols = &self.world.patch_soil[p];
        let n = cols.len().max(1) as f32;
        let (mut l, mut m, mut f) = (0.0f32, 0.0f32, 0.0f32);
        for &c in cols {
            l += self.world.surface_light(c) as f32;
            m += self.moisture[c];
            f += self.fertility[c];
        }
        PatchEnv { light: l / n, moisture: m / n, fertility: f / n, temperature: self.patches[p].temperature }
    }

    /// Mean surface moisture of a patch's soil columns.
    pub fn patch_moisture(&self, p: usize) -> f32 {
        let cols = &self.world.patch_soil[p];
        cols.iter().map(|&c| self.moisture[c]).sum::<f32>() / cols.len().max(1) as f32
    }

    /// Unsuppressed, un-limited growth rate factor r·f_L·f_M·f_T·f_F for one species.
    fn growth_factor(&self, sp: &CoverSpecies, env: &PatchEnv) -> f32 {
        let f_f = (env.fertility / self.params.cover.fertility_full).clamp(0.0, 1.0);
        sp.r * suitability(&sp.light, env.light)
            * suitability(&sp.moisture, env.moisture)
            * suitability(&sp.temp, env.temperature)
            * f_f
    }

    /// Producer update for patches with `p % 10 == tick % 10`.
    pub fn update_producers(&mut self, tick: u32) {
        for p in 0..PATCHES {
            if p as u32 % 10 == tick % 10 {
                self.update_patch_cover(p);
            }
        }
    }

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

        let s = (1.0 - cp.grass_suppression * shrub).max(0.0);
        let gg = self.params.grass.g;
        let gs = self.params.shrub.g;
        let dg = self.growth_factor(&self.params.grass, &env) * (1.0 - grass) * s - gg * grass;
        let ds = self.growth_factor(&self.params.shrub, &env) * (1.0 - shrub) - gs * shrub;

        let litter = cp.litter_factor * (gg * grass + gs * shrub) * n_soil as f32;
        self.patches[p].detritus += litter;
        self.patches[p].grass = (grass + dg).clamp(0.0, 1.0);
        self.patches[p].shrub = (shrub + ds).clamp(0.0, 1.0);

        let growth = dg.max(0.0) + ds.max(0.0);
        if growth > 0.0 {
            let (dm, df) = (cp.moisture_draw * growth, cp.fertility_draw * growth);
            for i in 0..n_soil {
                let c = self.world.patch_soil[p][i];
                self.moisture[c] = (self.moisture[c] - dm).max(0.0);
                self.fertility[c] = (self.fertility[c] - df).max(0.0);
            }
        }

        if self.patches[p].shrub > cp.shrub_spread_threshold {
            let (px, py) = ((p % PATCHES_X) as i32, (p / PATCHES_X) as i32);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (px + dx, py + dy);
                if nx < 0 || ny < 0 || nx >= PATCHES_X as i32 || ny >= (PATCHES / PATCHES_X) as i32 {
                    continue;
                }
                let q = nx as usize + PATCHES_X * ny as usize;
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
}
