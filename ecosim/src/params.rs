//! All tunable parameters, loaded from `params.toml`. Nothing tunable is hard-coded elsewhere.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Piecewise-linear suitability curve: (min, low-opt, high-opt, max).
pub type Curve = [f32; 4];

/// All tunable parameters, one field per `params.toml` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// `[world]`
    pub world: WorldParams,
    /// `[climate]`
    pub climate: ClimateParams,
    /// `[season]`
    pub season: SeasonParams,
    /// `[cover]`
    pub cover: CoverParams,
    /// `[grass]`
    pub grass: CoverSpecies,
    /// `[shrub]`
    pub shrub: CoverSpecies,
    /// `[tree]`
    pub tree: TreeParams,
    /// `[grazer]`
    pub grazer: GrazerParams,
    /// `[hunter]`
    pub hunter: HunterParams,
}

/// Terrain generation and world-level settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldParams {
    /// Lowest terrain height.
    pub height_min: u8,
    /// Highest terrain height.
    pub height_max: u8,
    /// Columns with terrain below this are filled with water up to it.
    pub water_level: u8,
    /// Columns at or above this height are topped with rock.
    pub rock_top_height: u8,
    /// Soil layers on top of the rock.
    pub soil_depth: u8,
    /// Fraction of columns whose terrain is normalized to below the water line.
    pub water_fraction: f32,
    /// Light removed by each canopy voxel above a voxel.
    pub canopy_absorb: u8,
    /// Entity compaction interval in ticks.
    pub compact_every: u32,
}

/// Seasons, rain, soil moisture and fertility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClimateParams {
    /// Ticks per year (season period).
    pub year_len: u32,
    /// Mean temperature, °C.
    pub temp_base: f32,
    /// Temperature drop of a fully canopied patch, °C.
    pub canopy_cool: f32,
    /// Mean rain per soil update.
    pub rain_base: f32,
    /// Seasonal rain swing.
    pub rain_amp: f32,
    /// Evaporation per soil update at 0 °C.
    pub evap_base: f32,
    /// Evaporation rises by 1 per this many °C.
    pub evap_div: f32,
    /// Moisture of soil columns next to water.
    pub pond_moisture: f32,
    /// Moisture diffusion rate per soil update.
    pub diffusion: f32,
    /// Detritus decay rate at full temperature and moisture.
    pub decay_k: f32,
    /// Temperature at which decay reaches full rate.
    pub decay_temp_full: f32,
    /// Moisture of every soil column at tick 0.
    pub initial_moisture: f32,
    /// Fertility of every soil column at tick 0.
    pub initial_fertility: f32,
}

/// Seasonal forcing. Temperature is `temp_base + amplitude·sin(2π t / year_len)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeasonParams {
    /// Temperature swing around `climate.temp_base`, °C.
    pub amplitude: f32,
}

/// Ground-cover rules shared by grass and shrub.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverParams {
    /// Moisture taken from each soil column per unit of cover growth.
    pub moisture_draw: f32,
    /// Fertility taken from each soil column per unit of cover growth.
    pub fertility_draw: f32,
    /// Detritus added per unit of cover mortality, per soil column.
    pub litter_factor: f32,
    /// f_F = clamp(F / fertility_full, 0, 1).
    pub fertility_full: f32,
    /// Grass growth is scaled by (1 - grass_suppression * shrub).
    pub grass_suppression: f32,
    /// Shrub above this seeds the 4 neighbouring patches.
    pub shrub_spread_threshold: f32,
    /// Shrub density a seeded neighbour is raised to.
    pub shrub_spread_seed: f32,
}

/// One ground-cover species (grass or shrub).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverSpecies {
    /// Maximum growth rate per update.
    pub r: f32,
    /// Mortality rate per update.
    pub g: f32,
    /// Density on every soil patch at tick 0.
    pub initial: f32,
    /// Suitability over patch light.
    pub light: Curve,
    /// Suitability over patch moisture.
    pub moisture: Curve,
    /// Suitability over patch temperature.
    pub temp: Curve,
}

/// The tree species.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeParams {
    /// Trees planted at tick 0.
    pub initial_count: u32,
    /// Age of the initial trees.
    pub initial_age: u32,
    /// Ticks between tree updates.
    pub update_every: u32,
    /// Age at which a sapling becomes young.
    pub young_age: u32,
    /// Age at which a tree becomes mature and seeds.
    pub mature_age: u32,
    /// Mean lifespan: each tree dies at its own `max_age · (1 + lifespan_jitter · u)`, u uniform in [−1, 1].
    pub max_age: u32,
    /// Relative spread of per-tree lifespans around `max_age`.
    pub lifespan_jitter: f32,
    /// Per-update death chance of a mature tree whose crown is overlapped by the canopy of ≥ 2 other trees.
    pub crowding_mortality: f32,
    /// Moisture a tree takes from its column per update.
    pub moisture_draw: f32,
    /// Moisture below which a tree counts as dry.
    pub dry_moisture: f32,
    /// Consecutive dry ticks after which a tree dies.
    pub dry_death_ticks: u32,
    /// A mature tree tries to seed when its age is a multiple of this.
    pub seed_every: u32,
    /// Maximum seed distance in columns.
    pub seed_radius: f32,
    /// Light low-opt of the germination curve; replaces `light[1]`.
    pub sapling_light: f32,
    /// Minimum Chebyshev distance between trunks.
    pub min_spacing: i32,
    /// Detritus a dead tree adds to its patch.
    pub death_detritus: f32,
    /// Germination suitability over surface light (low-opt replaced by `sapling_light`).
    pub light: Curve,
    /// Germination suitability over column moisture.
    pub moisture: Curve,
    /// Germination suitability over patch temperature.
    pub temp: Curve,
}

/// The grazer species.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrazerParams {
    /// Grazers placed at tick 0.
    pub start_count: u32,
    /// Energy of the initial grazers.
    pub start_energy: f32,
    /// Initial grazers get a random age below this.
    pub start_age_max: u32,
    /// Energy spent per tick (doubled on a tick it moves).
    pub energy_cost: f32,
    /// Age at which a grazer dies.
    pub max_age: u32,
    /// Energy above which a grazer reproduces.
    pub repro_energy: f32,
    /// Energy a parent gives up to reproduce.
    pub repro_cost: f32,
    /// Ticks between births.
    pub cooldown: u32,
    /// Energy of a newborn.
    pub newborn_energy: f32,
    /// No births in a patch holding this many grazers.
    pub max_grazers_per_patch: u32,
    /// Detritus a dead grazer adds to its patch.
    pub corpse_detritus: f32,
    /// Type II intake ceiling per tick (stability rule 1).
    pub intake_max: f32,
    /// Intake per unit of grass density below the ceiling.
    pub intake_k: f32,
    /// Grass density removed per unit of energy eaten.
    pub grass_per_energy: f32,
    /// A grazer eats only while its energy is below this.
    pub eat_below: f32,
    /// Grass level above which the patch counts as "food is here" for the eat priority.
    pub eat_min_grass: f32,
    /// A grazer flees hunters within this distance.
    pub flee_radius: f32,
    /// Chebyshev patch distance searched for a better patch.
    pub search_patches: i32,
    /// Crowding divisor in the patch score `grass - grazers / crowding`.
    pub crowding: f32,
    /// Patches scoring within this of the best are all acceptable move targets.
    pub choice_tolerance: f32,
    /// One grazer immigrates when fewer than this many are alive (0 disables immigration).
    pub immigration_floor: u32,
    /// Ticks between immigration checks.
    pub immigration_interval: u32,
}

/// The hunter species.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HunterParams {
    /// Hunters placed at tick 0.
    pub start_count: u32,
    /// Energy of the initial hunters.
    pub start_energy: f32,
    /// Initial hunters get a random age below this.
    pub start_age_max: u32,
    /// Energy spent per tick (doubled on a tick it moves).
    pub energy_cost: f32,
    /// Age at which a hunter dies.
    pub max_age: u32,
    /// Energy above which a hunter reproduces.
    pub repro_energy: f32,
    /// Energy a parent gives up to reproduce.
    pub repro_cost: f32,
    /// Ticks between births.
    pub cooldown: u32,
    /// Energy of a newborn.
    pub newborn_energy: f32,
    /// Detritus a dead hunter adds to its patch.
    pub corpse_detritus: f32,
    /// A hunter with more energy than this makes no attack (stability rule 2).
    pub satiation: f32,
    /// Prey within this distance is attacked.
    pub attack_radius: f32,
    /// Chance an attack kills (stability rule 2).
    pub kill_prob: f64,
    /// Energy gained from a kill.
    pub kill_energy: f32,
    /// Energy lost on a failed attack.
    pub fail_cost: f32,
    /// Steps a grazer is pushed away by a failed attack.
    pub displace_steps: u32,
    /// Shrub refugium exponent: attack success is `kill_prob · (1 − shrub)^refugium_k` (stability rule 3).
    pub refugium_k: f32,
    /// A hunter looks for prey within this distance.
    pub seek_radius: f32,
    /// One hunter immigrates when fewer than this many are alive (0 disables immigration).
    pub immigration_floor: u32,
    /// Ticks between immigration checks.
    pub immigration_interval: u32,
}

impl Params {
    /// Parse a params file and apply `--set key=value` overrides before deserializing.
    pub fn from_toml_str_with(s: &str, overrides: &[String]) -> Result<Params, String> {
        let mut root = toml::Value::Table(s.parse::<toml::Table>().map_err(|e| format!("params: {e}"))?);
        for o in overrides {
            apply_override(&mut root, o)?;
        }
        Params::from_value(root)
    }

    /// Deserialize a parsed params document.
    pub fn from_value(root: toml::Value) -> Result<Params, String> {
        root.try_into().map_err(|e| format!("params: {e}"))
    }

    /// Read and parse a params file.
    pub fn load(path: &Path) -> Result<Params, String> {
        Params::load_with(path, &[])
    }

    /// Read a params file and apply `--set key=value` overrides.
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
        let table =
            cur.as_table_mut().ok_or_else(|| format!("--set {key}: unknown key ('{part}' is under a non-table)"))?;
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
        V::Boolean(_) => {
            raw.parse::<bool>().map(V::Boolean).map_err(|_| format!("expected true or false, got '{raw}'"))
        }
        V::String(_) => Ok(V::String(raw.to_string())),
        V::Array(a) => {
            let doc =
                format!("v = {raw}").parse::<toml::Table>().map_err(|_| format!("expected an array, got '{raw}'"))?;
            let V::Array(new) = &doc["v"] else { return Err(format!("expected an array, got '{raw}'")) };
            if new.len() != a.len() {
                return Err(format!("expected an array of {} elements, got {}", a.len(), new.len()));
            }
            let items: Result<Vec<_>, _> = a.iter().zip(new).map(|(o, n)| coerce(o, &n.to_string())).collect();
            Ok(V::Array(items?))
        }
        V::Table(_) => Err("is a table; set one of its keys".into()),
        V::Datetime(_) => Err("datetime keys are not supported".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn defaults() -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml")).unwrap()
    }

    #[test]
    fn set_nested_keys_round_trip() {
        let sets: Vec<String> =
            ["hunter.kill_prob=0.25", "tree.mature_age=1500.0", "grass.initial=1", "shrub.light=[1, 2.5, 3, 4]"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        let p = Params::from_toml_str_with(&defaults(), &sets).unwrap();
        assert_eq!(p.hunter.kill_prob, 0.25);
        assert_eq!(p.tree.mature_age, 1500);
        assert_eq!(p.grass.initial, 1.0);
        assert_eq!(p.shrub.light, [1.0, 2.5, 3.0, 4.0]);
        // Setting a key to its current value changes nothing.
        let amp = format!("season.amplitude={}", Params::load_default().season.amplitude);
        let same = Params::from_toml_str_with(&defaults(), &[amp]).unwrap();
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

    /// Every leaf of the default params file: (dotted key, value). Arrays are leaves.
    fn leaves() -> Vec<(String, toml::Value)> {
        fn walk(prefix: &str, v: &toml::Value, out: &mut Vec<(String, toml::Value)>) {
            match v {
                toml::Value::Table(t) => {
                    for (k, v) in t {
                        walk(&if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") }, v, out);
                    }
                }
                _ => out.push((prefix.to_string(), v.clone())),
            }
        }
        let mut out = Vec::new();
        walk("", &toml::Value::Table(defaults().parse().unwrap()), &mut out);
        out
    }

    /// `v` as it reads back from `Params` through JSON: f32 fields hold the nearest f32.
    fn as_stored(key: &str, v: f64) -> serde_json::Value {
        serde_json::json!(if key == "hunter.kill_prob" { v } else { v as f32 as f64 })
    }

    fn lookup<'a>(json: &'a serde_json::Value, key: &str) -> &'a serde_json::Value {
        key.split('.').fold(json, |j, k| &j[k])
    }

    /// `--set key=value` for one leaf changes exactly that leaf in the loaded `Params`, to the given
    /// value. Integers are drawn from `ints` (0..=255 fits every integer field), numbers from `floats`.
    fn set_round_trips(key: &str, old: &toml::Value, ints: &[u8], floats: &[f64]) -> Result<(), TestCaseError> {
        let (text, want) = match old {
            toml::Value::Integer(_) => (ints[0].to_string(), serde_json::json!(ints[0])),
            toml::Value::Float(_) => (floats[0].to_string(), as_stored(key, floats[0])),
            toml::Value::Array(a) => {
                let xs = &floats[..a.len()];
                let text = xs.iter().map(f64::to_string).collect::<Vec<_>>().join(", ");
                (format!("[{text}]"), serde_json::Value::Array(xs.iter().map(|&x| as_stored(key, x)).collect()))
            }
            other => return Err(TestCaseError::fail(format!("{key}: unexpected leaf type {other:?}"))),
        };
        let set = Params::from_toml_str_with(&defaults(), &[format!("{key}={text}")]).map_err(TestCaseError::fail)?;
        let (mut got, base) =
            (serde_json::to_value(&set).unwrap(), serde_json::to_value(Params::load_default()).unwrap());
        prop_assert_eq!(lookup(&got, key), &want, "{} = {}", key, text);
        let (section, field) = key.split_once('.').unwrap();
        got[section][field] = base[section][field].clone();
        prop_assert_eq!(got, base, "setting {} changed another key", key);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(16)))]

        #[test]
        fn prop_set_round_trips_every_leaf(
            ints in prop::collection::vec(any::<u8>(), 1),
            floats in prop::collection::vec(-1.0e4f64..1.0e4, 8),
        ) {
            let all = leaves();
            prop_assert!(all.len() > 60, "only {} leaves found", all.len());
            for (key, old) in &all {
                set_round_trips(key, old, &ints, &floats)?;
            }
        }
    }

    #[test]
    fn set_regression_every_leaf_to_fixed_values() {
        for (key, old) in leaves() {
            set_round_trips(&key, &old, &[7], &[0.1, -2.5, 3.0, 1e-3, 0.0, 0.0, 0.0, 0.0]).unwrap();
        }
    }
}
