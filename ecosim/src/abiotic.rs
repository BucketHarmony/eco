//! Moisture, fertility, detritus decay and temperature.

use crate::sim::Sim;
use crate::world::{west_east, ColClass, World};
use std::f32::consts::PI;

/// One Jacobi diffusion step over 4-neighbour soil columns: m' = m + rate·Σ(m_n − m).
/// Flux is pairwise-symmetric, so total mass over soil columns is conserved.
pub fn diffuse(field: &mut [f32], world: &World, rate: f32, scratch: &mut Vec<f32>) {
    let d = world.dims;
    scratch.clear();
    scratch.extend_from_slice(field);
    for y in 0..d.wy {
        for x in 0..d.wx {
            let c = d.cidx(x, y);
            if world.class[c] != ColClass::Soil {
                continue;
            }
            let m = scratch[c];
            let mut flux = 0.0;
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if world.is_soil(nx, ny) {
                    flux += scratch[d.cidx(nx as usize, ny as usize)] - m;
                }
            }
            field[c] = m + rate * flux;
        }
    }
}

/// Rain on column x of a world `width` columns wide, given the season's rain:
/// `rain × (1 + gradient × (2x/(width − 1) − 1))`, clamped at 0. For |gradient| ≤ 1 (all the params
/// loader accepts) and rain ≥ 0 it lies in [0, 2 × rain] and a row of `width` columns gets
/// `width × rain` in total: the west–east term is odd about the row's middle.
pub fn rain_at(rain: f32, gradient: f32, x: usize, width: usize) -> f32 {
    (rain * (1.0 + gradient * west_east(x, width))).max(0.0)
}

impl Sim {
    /// Season rain on a soil update: rain_base − rain_amp·sin(2π·tick/year_len). With a
    /// `climate.rain_gradient` each column gets `rain_at` of it instead.
    pub fn rain(&self, tick: u32) -> f32 {
        let c = &self.params.climate;
        c.rain_base - c.rain_amp * libm::sinf(2.0 * PI * tick as f32 / c.year_len as f32)
    }

    /// Evaporation amount for a column, from its patch temperature.
    pub fn evaporation(&self, c: usize) -> f32 {
        let (x, y) = self.world.dims.xy(c);
        let t = self.patches[self.world.dims.patch_of(x, y)].temperature;
        let cl = &self.params.climate;
        (cl.evap_base + t / cl.evap_div).max(0.0)
    }

    /// Step 3 of the soil update: remove each listed column's evaporation, flooring moisture at 0.
    pub fn evaporate(&mut self, soil: &[usize]) {
        for &c in soil {
            self.moisture[c] = (self.moisture[c] - self.evaporation(c)).max(0.0);
        }
    }

    /// The soil update, every `schedule.soil_every` ticks. With the water tier on (shot G4) the water half is
    /// [`Sim::settle_water`] — ponded water soaking in and evaporating, percolation and ET — and
    /// the moisture field is derived from what is left; with it off, the pre-G4 steps 1–5 run
    /// instead (rain, diffusion, evaporation, pond wetting, clamp). Detritus decay and the
    /// fertility clamp follow either way.
    pub fn update_soil(&mut self, tick: u32) {
        let d = self.world.dims;
        let soil: Vec<usize> = (0..d.cols()).filter(|&c| self.world.class[c] == ColClass::Soil).collect();
        if self.hydro.is_some() {
            let hours = self.params.schedule.soil_every.max(1) as f64 * crate::hydro::tick_hours(&self.params);
            let drained = self.settle_water(hours);
            self.decay_detritus(&soil, &drained);
            self.update_npk(&drained);
            return;
        }
        // 1. Rain, uniform unless there is a gradient (checked once, outside the loop)
        let r = self.rain(tick);
        let gradient = self.params.climate.rain_gradient;
        if gradient == 0.0 {
            for &c in &soil {
                self.moisture[c] += r;
            }
        } else {
            for &c in &soil {
                self.moisture[c] += rain_at(r, gradient, c % d.wx, d.wx);
            }
        }
        // 2. Diffusion
        let mut scratch = Vec::with_capacity(d.cols());
        diffuse(&mut self.moisture, &self.world, self.params.climate.diffusion, &mut scratch);
        // 3. Evaporation
        self.evaporate(&soil);
        // 4. Pond wetting
        let pond = self.params.climate.pond_moisture;
        for &c in &soil {
            let (x, y) = ((c % d.wx) as i32, (c / d.wx) as i32);
            let wet = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                d.in_bounds(nx, ny) && self.world.class[d.cidx(nx as usize, ny as usize)] == ColClass::Water
            });
            if wet {
                self.moisture[c] = pond;
            }
        }
        // 5. Clamp moisture
        for &c in &soil {
            self.moisture[c] = self.moisture[c].clamp(0.0, 255.0);
        }
        self.decay_detritus(&soil, &[]);
        self.update_npk(&[]);
    }

    /// Steps 6–7 of the soil update: detritus decays into the soil under its own patch, leaching
    /// takes a share of what a column drained away with it (`hydro.leach_k`, 0 when the water tier
    /// is off, which is what `drained` empty means), and fertility is clamped.
    ///
    /// What decay releases depends on the tier that is running. With `npk.enabled` the decayed
    /// share of the patch's *nutrients* moves into its columns' three pools, and the fertility
    /// field is not touched at all -- it is derived from those pools (shot G5), so steps 6b and 7
    /// have nothing left to do and are skipped. With it off, the pre-G5 index gains the decayed
    /// detritus directly and both steps run as they did.
    pub(crate) fn decay_detritus(&mut self, soil: &[usize], drained: &[f32]) {
        let d = self.world.dims;
        // 6. Decay detritus into fertility
        let cl = self.params.climate.clone();
        for p in 0..d.patches() {
            let n = self.world.patch_soil[p].len();
            if n == 0 {
                continue;
            }
            let m = self.patch_moisture(p);
            let t = self.patches[p].temperature;
            // decay_k is per year, charged over this update's own length (shot G4b).
            let dt = crate::hydro::years(&self.params, self.params.schedule.soil_every.max(1)) as f32;
            let rate = cl.decay_k * dt * (t / cl.decay_temp_full).clamp(0.0, 1.0) * m / 255.0;
            let converted = self.patches[p].detritus * rate;
            self.patches[p].detritus -= converted;
            if self.npk.is_some() {
                // The same share of the same pile, so the nutrients follow the mass they were in.
                let mut released = [0.0f64; 3];
                let pile = &mut self.npk.as_mut().expect("nutrient tier").detritus[p];
                for (i, v) in pile.iter_mut().enumerate() {
                    released[i] = *v * rate as f64;
                    *v -= released[i];
                }
                self.npk_return(p, released);
                continue;
            }
            let per = converted / n as f32;
            for i in 0..n {
                let c = self.world.patch_soil[p][i];
                self.fertility[c] += per;
            }
        }
        if self.npk.is_some() {
            return;
        }
        // 6b. Leaching: fertility leaves with the water that drained out of the column.
        if !drained.is_empty() {
            let k = self.params.hydro.leach_k;
            for &c in soil {
                self.fertility[c] -= self.fertility[c] * (k * drained[c]).clamp(0.0, 1.0);
            }
        }
        // 7. Clamp fertility
        for &c in soil {
            self.fertility[c] = self.fertility[c].clamp(0.0, 255.0);
        }
    }

    /// Patch temperature: base + amp·sin(2π·tick/year_len) − canopy_cool·canopy_fraction.
    pub fn update_temperature(&mut self, tick: u32) {
        let cl = &self.params.climate;
        let season =
            cl.temp_base + self.params.season.amplitude * libm::sinf(2.0 * PI * tick as f32 / cl.year_len as f32);
        let area = (self.world.dims.patch * self.world.dims.patch) as f32;
        for p in 0..self.world.dims.patches() {
            let covered = self.canopy_columns(p);
            self.patches[p].temperature = season - cl.canopy_cool * covered as f32 / area;
        }
    }

    /// Columns of patch `p` (of its patch × patch) that lie under any canopy voxel.
    pub fn canopy_columns(&self, p: usize) -> u32 {
        let d = self.world.dims;
        let ((px, py), n) = (d.patch_xy(p), d.patch);
        let mut covered = 0;
        for y in py * n..py * n + n {
            for x in px * n..px * n + n {
                if self.canopy_cover[d.cidx(x, y)] {
                    covered += 1;
                }
            }
        }
        covered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;
    use crate::world::sq::*;
    use proptest::prelude::*;

    #[test]
    fn diffusion_conserves_mass_minus_evaporation() {
        let params = Params::load_square();
        // All-soil world, well above water level: no ponds.
        let world = World::from_heights(&vec![14u8; COLS], &params);
        let mut m: Vec<f32> = (0..COLS).map(|c| ((c * 37) % 200) as f32 + 30.0).collect();
        let before: f64 = m.iter().map(|&v| v as f64).sum();
        let mut scratch = Vec::new();
        diffuse(&mut m, &world, 0.10, &mut scratch);
        let after: f64 = m.iter().map(|&v| v as f64).sum();
        assert!((before - after).abs() / before < 1e-5, "diffusion changed mass: {before} → {after}");

        // Full soil update with rain off: total drops by exactly Σ evaporation. This is the
        // pre-G4 moisture path, so the water tier is off.
        let mut p = params.clone();
        p.climate.rain_base = 0.0;
        p.climate.rain_amp = 0.0;
        p.hydro.enabled = false;
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
        let world = World::from_heights(heights, &Params::load_square());
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

    /// Rain along a row: every column's rain is in [0, 2 × rain], and the row's total is
    /// `width × rain` (to f32 rounding), for any accepted gradient.
    fn rain_row_holds(rain: f32, gradient: f32, width: usize) -> Result<(), TestCaseError> {
        let row: Vec<f32> = (0..width).map(|x| rain_at(rain, gradient, x, width)).collect();
        for (x, &r) in row.iter().enumerate() {
            prop_assert!((0.0..=2.0 * rain * (1.0 + 1e-6)).contains(&r), "x {}: {} of {}", x, r, rain);
        }
        let (sum, want) = (row.iter().map(|&r| r as f64).sum::<f64>(), width as f64 * rain as f64);
        prop_assert!((sum - want).abs() <= 1e-5 * want.max(1.0), "row sum {} vs {}", sum, want);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(256)))]

        #[test]
        fn prop_rain_is_bounded_and_conserved_across_a_row(
            rain in 0.0f32..10.0,
            gradient in -1.0f32..=1.0,
            width in (1usize..=32).prop_map(|k| 8 * k),
        ) {
            rain_row_holds(rain, gradient, width)?;
        }
    }

    /// The reference strip (256 wide, gradient 0.6): the west edge gets 0.4 × rain, the east edge
    /// 1.6 × rain; gradients ±1 put 0 and 2 × rain at the edges; gradient 0 is uniform.
    #[test]
    fn rain_regression_reference_strip_and_edges() {
        let rain = crate::params::Params::load_default().climate.rain_base;
        for g in [0.6, 1.0, -1.0, 0.0] {
            rain_row_holds(rain, g, 256).unwrap();
        }
        assert!((rain_at(1.0, 0.6, 0, 256) - 0.4).abs() < 1e-6 && (rain_at(1.0, 0.6, 255, 256) - 1.6).abs() < 1e-6);
        assert_eq!((rain_at(2.0, 1.0, 0, 64), rain_at(2.0, 1.0, 63, 64)), (0.0, 4.0));
        assert_eq!((rain_at(2.0, -1.0, 0, 64), rain_at(2.0, -1.0, 63, 64)), (4.0, 0.0));
        assert!((0..256).all(|x| rain_at(1.5, 0.0, x, 256) == 1.5));
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
