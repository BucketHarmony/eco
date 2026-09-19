//! All tunable parameters, loaded from `params.toml`. Nothing tunable is hard-coded elsewhere.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Piecewise-linear suitability curve: (min, low-opt, high-opt, max).
pub type Curve = [f32; 4];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub world: WorldParams,
    pub climate: ClimateParams,
    pub cover: CoverParams,
    pub grass: CoverSpecies,
    pub shrub: CoverSpecies,
    pub tree: TreeParams,
    pub grazer: GrazerParams,
    pub hunter: HunterParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldParams {
    pub height_min: u8,
    pub height_max: u8,
    pub water_level: u8,
    pub rock_top_height: u8,
    pub soil_depth: u8,
    /// Fraction of columns whose terrain is normalized to below the water line.
    pub water_fraction: f32,
    pub canopy_absorb: u8,
    /// Entity compaction interval in ticks.
    pub compact_every: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClimateParams {
    pub year_len: u32,
    pub temp_base: f32,
    pub temp_amp: f32,
    pub canopy_cool: f32,
    pub rain_base: f32,
    pub rain_amp: f32,
    pub evap_base: f32,
    pub evap_div: f32,
    pub pond_moisture: f32,
    pub diffusion: f32,
    pub decay_k: f32,
    pub decay_temp_full: f32,
    pub initial_moisture: f32,
    pub initial_fertility: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverParams {
    pub moisture_draw: f32,
    pub fertility_draw: f32,
    pub litter_factor: f32,
    /// f_F = clamp(F / fertility_full, 0, 1).
    pub fertility_full: f32,
    /// Grass growth is scaled by (1 - grass_suppression * shrub).
    pub grass_suppression: f32,
    pub shrub_spread_threshold: f32,
    pub shrub_spread_seed: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverSpecies {
    pub r: f32,
    pub g: f32,
    pub initial: f32,
    pub light: Curve,
    pub moisture: Curve,
    pub temp: Curve,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeParams {
    pub initial_count: u32,
    pub initial_age: u32,
    pub update_every: u32,
    pub young_age: u32,
    pub mature_age: u32,
    pub max_age: u32,
    pub moisture_draw: f32,
    pub dry_moisture: f32,
    pub dry_death_ticks: u32,
    pub seed_every: u32,
    pub seed_radius: f32,
    /// Light low-opt of the germination curve; replaces `light[1]`.
    pub sapling_light: f32,
    pub min_spacing: i32,
    pub death_detritus: f32,
    pub light: Curve,
    pub moisture: Curve,
    pub temp: Curve,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrazerParams {
    pub start_count: u32,
    pub start_energy: f32,
    pub start_age_max: u32,
    pub energy_cost: f32,
    pub max_age: u32,
    pub repro_energy: f32,
    pub repro_cost: f32,
    pub cooldown: u32,
    pub newborn_energy: f32,
    pub max_grazers_per_patch: u32,
    pub corpse_detritus: f32,
    pub intake_max: f32,
    pub intake_k: f32,
    pub grass_per_energy: f32,
    pub eat_below: f32,
    /// Grass level above which the patch counts as "food is here" for the eat priority.
    pub eat_min_grass: f32,
    pub flee_radius: f32,
    pub search_patches: i32,
    /// Crowding divisor in the patch score `grass - grazers / crowding`.
    pub crowding: f32,
    /// Patches scoring within this of the best are all acceptable move targets.
    pub choice_tolerance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HunterParams {
    pub start_count: u32,
    pub start_energy: f32,
    pub start_age_max: u32,
    pub energy_cost: f32,
    pub max_age: u32,
    pub repro_energy: f32,
    pub repro_cost: f32,
    pub cooldown: u32,
    pub newborn_energy: f32,
    pub corpse_detritus: f32,
    pub satiation: f32,
    pub attack_radius: f32,
    pub kill_prob: f64,
    pub kill_energy: f32,
    pub fail_cost: f32,
    pub displace_steps: u32,
    pub refugium_shrub: f32,
    pub seek_radius: f32,
}

impl Params {
    pub fn from_toml_str(s: &str) -> Result<Params, String> {
        toml::from_str(s).map_err(|e| format!("params: {e}"))
    }

    pub fn load(path: &Path) -> Result<Params, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Params::from_toml_str(&s)
    }

    /// The crate's own `params.toml`, used by tests so tuning applies to them too.
    pub fn load_default() -> Params {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml");
        Params::load(&p).expect("load params.toml")
    }

    /// Germination light curve with the sapling light need applied as its low-opt.
    pub fn tree_light_curve(&self) -> Curve {
        let mut c = self.tree.light;
        c[1] = self.tree.sapling_light.max(c[0] + 1.0);
        c
    }
}
