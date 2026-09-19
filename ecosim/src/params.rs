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
    pub season: SeasonParams,
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

/// Seasonal forcing. Temperature is `temp_base + amplitude·sin(2π t / year_len)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeasonParams {
    pub amplitude: f32,
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
        Params::from_toml_str_with(s, &[])
    }

    /// Parse a params file and apply `--set key=value` overrides before deserializing.
    pub fn from_toml_str_with(s: &str, overrides: &[String]) -> Result<Params, String> {
        let mut root = toml::Value::Table(s.parse::<toml::Table>().map_err(|e| format!("params: {e}"))?);
        for o in overrides {
            apply_override(&mut root, o)?;
        }
        Params::from_value(root)
    }

    pub fn from_value(root: toml::Value) -> Result<Params, String> {
        root.try_into().map_err(|e| format!("params: {e}"))
    }

    pub fn load(path: &Path) -> Result<Params, String> {
        Params::load_with(path, &[])
    }

    pub fn load_with(path: &Path, overrides: &[String]) -> Result<Params, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Params::from_toml_str_with(&s, overrides).map_err(|e| format!("{}: {e}", path.display()))
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

/// Apply one `dotted.key=value` override to a parsed params document. The key must already exist,
/// and the value must have the existing value's type (an integer-valued float such as `500.0` is
/// accepted for an integer key, and an integer for a float key). Errors name the key.
pub fn apply_override(root: &mut toml::Value, spec: &str) -> Result<(), String> {
    let (key, raw) = spec.split_once('=').ok_or_else(|| format!("--set {spec}: expected key=value"))?;
    let (key, raw) = (key.trim(), raw.trim());
    let mut cur = root;
    for part in key.split('.') {
        let table = cur.as_table_mut().ok_or_else(|| format!("--set {key}: unknown key ('{part}' is under a non-table)"))?;
        cur = table.get_mut(part).ok_or_else(|| format!("--set {key}: unknown key"))?;
    }
    *cur = coerce(cur, raw).map_err(|e| format!("--set {key}: {e}"))?;
    Ok(())
}

fn coerce(old: &toml::Value, raw: &str) -> Result<toml::Value, String> {
    use toml::Value as V;
    match old {
        V::Integer(_) => match (raw.parse::<i64>(), raw.parse::<f64>()) {
            (Ok(i), _) => Ok(V::Integer(i)),
            (_, Ok(f)) if f.is_finite() && f.fract() == 0.0 => Ok(V::Integer(f as i64)),
            _ => Err(format!("expected an integer, got '{raw}'")),
        },
        V::Float(_) => raw.parse::<f64>().map(V::Float).map_err(|_| format!("expected a number, got '{raw}'")),
        V::Boolean(_) => raw.parse::<bool>().map(V::Boolean).map_err(|_| format!("expected true or false, got '{raw}'")),
        V::String(_) => Ok(V::String(raw.to_string())),
        V::Array(a) => {
            let doc = format!("v = {raw}").parse::<toml::Table>().map_err(|_| format!("expected an array, got '{raw}'"))?;
            let V::Array(new) = &doc["v"] else { return Err(format!("expected an array, got '{raw}'")) };
            if new.len() != a.len() {
                return Err(format!("expected an array of {} elements, got {}", a.len(), new.len()));
            }
            let items: Result<Vec<_>, _> =
                a.iter().zip(new).map(|(o, n)| coerce(o, &n.to_string())).collect();
            Ok(V::Array(items?))
        }
        V::Table(_) => Err("is a table; set one of its keys".into()),
        V::Datetime(_) => Err("datetime keys are not supported".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml")).unwrap()
    }

    #[test]
    fn set_nested_keys_round_trip() {
        let sets: Vec<String> = ["hunter.kill_prob=0.25", "tree.mature_age=1500.0", "grass.initial=1", "shrub.light=[1, 2.5, 3, 4]"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let p = Params::from_toml_str_with(&defaults(), &sets).unwrap();
        assert_eq!(p.hunter.kill_prob, 0.25);
        assert_eq!(p.tree.mature_age, 1500);
        assert_eq!(p.grass.initial, 1.0);
        assert_eq!(p.shrub.light, [1.0, 2.5, 3.0, 4.0]);
        // Setting a key to its current value changes nothing.
        let same = Params::from_toml_str_with(&defaults(), &["season.amplitude=12".into()]).unwrap();
        assert_eq!(serde_json::to_string(&same).unwrap(), serde_json::to_string(&Params::load_default()).unwrap());
    }

    #[test]
    fn set_unknown_key_errors_naming_it() {
        for bad in ["hunter.kill_probability=0.2", "nosuch.key=1", "hunter.kill_prob.x=1", "hunter=1"] {
            let e = Params::from_toml_str_with(&defaults(), &[bad.into()]).unwrap_err();
            let key = bad.split('=').next().unwrap();
            assert!(e.contains(key), "{bad}: {e}");
        }
    }

    #[test]
    fn set_wrong_type_errors_naming_it() {
        for bad in ["tree.mature_age=1.5", "hunter.kill_prob=high", "shrub.light=[1, 2]", "grazer.cooldown=abc"] {
            let e = Params::from_toml_str_with(&defaults(), &[bad.into()]).unwrap_err();
            let key = bad.split('=').next().unwrap();
            assert!(e.contains(key), "{bad}: {e}");
        }
        assert!(Params::from_toml_str_with(&defaults(), &["no_equals_sign".into()]).is_err());
    }
}
