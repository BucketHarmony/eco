//! All tunable parameters, loaded from `params.toml`. Nothing tunable is hard-coded elsewhere.

use crate::animals::Kind;
use crate::heredity::Traits;
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
    /// `[bundle]`. Left out of `meta.json` at its defaults, so noise-world runs print as before.
    #[serde(default, skip_serializing_if = "BundleParams::is_default")]
    pub bundle: BundleParams,
    /// `[animals]`. Left out of `meta.json` when animals are enabled, so default runs print as before.
    #[serde(default, skip_serializing_if = "AnimalsParams::is_default")]
    pub animals: AnimalsParams,
    /// `[grazer]`
    pub grazer: GrazerParams,
    /// `[hunter]`
    pub hunter: HunterParams,
    /// `[fire]`
    pub fire: FireParams,
    /// `[disease]`
    pub disease: DiseaseParams,
    /// `[heredity]`
    pub heredity: HeredityParams,
    /// `[rng]`. Left out of `meta.json` at the default stream, so default runs print as before.
    #[serde(default, skip_serializing_if = "RngParams::is_default")]
    pub rng: RngParams,
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
    /// Columns along x (west to east). A multiple of `patch`, at most 256.
    #[serde(default = "default_side")]
    pub width: u32,
    /// Columns along y. A multiple of `patch`, at most 256.
    #[serde(default = "default_side")]
    pub depth: u32,
    /// Voxels along z (up), at most 256.
    #[serde(default = "default_height")]
    pub height: u32,
    /// Patch edge length in columns.
    #[serde(default = "default_patch")]
    pub patch: u32,
    /// Terrain tilt: the height normalisation adds `slope_bias × (1 − 2x/(width − 1))` before
    /// clamping, so the west (dry) edge sits higher. 0 leaves the terrain untouched.
    #[serde(default)]
    pub slope_bias: f32,
}

/// The dimensions a params document without them means: the 64×64×32 world, 8×8 patches, which
/// every run before shot 15 used (so its `meta.json` restores and forks as it was).
fn default_side() -> u32 {
    64
}
fn default_height() -> u32 {
    32
}
fn default_patch() -> u32 {
    8
}

/// A world loaded from a bundle (`ecosim run --world`, `../docs/SCENE-CONTRACT.md`). A noise world
/// ignores every key here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BundleParams {
    /// Ecology layers under the bundle's lowest ground: a column's surface layer is
    /// `base_z + round(mean ground height)`, in 1 m layers.
    pub base_z: u8,
    /// Shadow length in columns per metre of building height, cast due north (the sun sits due
    /// south; 1.0 is a 45° altitude). 0 turns building shade off.
    pub shade_slope: f32,
}

impl Default for BundleParams {
    fn default() -> Self {
        BundleParams { base_z: 8, shade_slope: 1.0 }
    }
}

impl BundleParams {
    fn is_default(&self) -> bool {
        *self == BundleParams::default()
    }
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
    /// West–east rain gradient: rain at column x is `rain × (1 + rain_gradient × (2x/(width − 1) − 1))`,
    /// clamped at 0, so the west edge is drier and the east edge wetter. 0 is uniform rain.
    #[serde(default)]
    pub rain_gradient: f32,
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
    /// One sapling arrives when fewer than this many trees are alive (0 disables immigration).
    pub immigration_floor: u32,
    /// Ticks between tree immigration checks.
    pub immigration_interval: u32,
}

/// Whether the two animal species take part in a run at all (shot G0). Garden runs turn them off;
/// the species themselves are unchanged, and `enabled = true` is the default every earlier run had.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimalsParams {
    /// False leaves every grazer and hunter out: none are placed, none immigrate, and the animal
    /// phase is skipped, so it draws nothing from the RNG.
    pub enabled: bool,
}

impl Default for AnimalsParams {
    fn default() -> Self {
        AnimalsParams { enabled: true }
    }
}

impl AnimalsParams {
    fn is_default(&self) -> bool {
        *self == AnimalsParams::default()
    }
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
    /// Short refractory period after a birth (and a newborn's wait before its first). Births are
    /// otherwise gated by `repro_energy` alone, so kills set the hunter birth rate.
    pub refractory: u32,
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
    /// Energy every attack attempt costs, hit or miss (food-limited hunters).
    pub hunt_cost: f32,
    /// Energy a failed attack costs on top of `hunt_cost`.
    pub fail_cost: f32,
    /// Ticks a hunter spends handling its prey after a kill: no attack, no move, resting energy
    /// cost (type II functional response). 0 switches handling off; it is then left out of
    /// `meta.json`, which reads back as 0, so runs without handling keep their exact `meta.json`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub handling_ticks: u32,
    /// Steps a grazer is pushed away by a failed attack.
    pub displace_steps: u32,
    /// Shrub refugium exponent: attack success is `kill_prob · (1 − shrub)^refugium_k` (stability rule 3).
    pub refugium_k: f32,
    /// A hunter looks for prey within this distance.
    pub seek_radius: f32,
    /// Default of the hunter's `flee_distance` trait. No hunter behaviour reads it, so the trait
    /// drifts neutrally: the control for the heritable traits that are under selection.
    pub flee_radius: f32,
    /// One hunter immigrates when fewer than this many are alive (0 disables immigration).
    pub immigration_floor: u32,
    /// Ticks between immigration checks.
    pub immigration_interval: u32,
}

/// Fire disturbance, at patch scale. Nothing ignites while `base_rate` is 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireParams {
    /// Ignition: p = base_rate · f(T) · (1 − moisture/255)² · fuel per patch per fire update.
    pub base_rate: f32,
    /// f(T) is 0 at or below this patch temperature, °C.
    pub temp_min: f32,
    /// f(T) is 1 at or above this patch temperature, °C.
    pub temp_full: f32,
    /// Fuel per unit of patch detritus.
    pub detritus_weight: f32,
    /// Fuel of a fully canopied patch.
    pub canopy_weight: f32,
    /// Ticks a patch burns before it burns out.
    pub duration: u32,
    /// Per-tick spread chance to each 4-neighbour: spread · neighbour fuel · (1 − neighbour moisture/255).
    pub spread: f32,
    /// Chance that burn-out kills each tree whose trunk is in the patch.
    pub tree_kill: f32,
    /// Detritus added per unit of burnt grass and shrub density, per soil column.
    pub detritus_yield: f32,
    /// Fertility added to every soil column of a patch at burn-out.
    pub ash: f32,
    /// Energy an animal in a burning patch loses per tick.
    pub animal_damage: f32,
}

/// Heritable animal traits. A newborn's traits are its parent's × (1 + mutation · u), u uniform in
/// [−1, 1], one draw per trait; clamped to [0.25, 4] × the species default. 0 switches mutation off.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeredityParams {
    /// Largest relative change per trait per generation.
    pub mutation: f32,
}

/// The random stream. Every run draws from one `ChaCha8Rng` seeded from `--seed`. The terrain is
/// generated first, on stream 0; the rng then switches to `stream` (at the same word position) for
/// the initial populations and every tick. So one seed keeps its world, and each stream is an
/// independent replicate of the dynamics on it. Stream 0 is what every run used before the key existed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RngParams {
    /// ChaCha stream number (0 = the default stream).
    pub stream: u64,
}

impl RngParams {
    fn is_default(&self) -> bool {
        *self == RngParams::default()
    }
}

/// Density-dependent ("crowded") mortality. Per animal update, an animal in a patch holding n of
/// its species dies with p = rate · max(0, n − threshold) / threshold. A rate of 0 switches it off.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiseaseParams {
    /// Grazer death chance per tick per threshold's worth of grazers above the threshold.
    pub grazer_rate: f32,
    /// Grazers a patch holds before crowding kills any.
    pub grazer_threshold: u32,
    /// Hunter death chance per tick per threshold's worth of hunters above the threshold.
    pub hunter_rate: f32,
    /// Hunters a patch holds before crowding kills any.
    pub hunter_threshold: u32,
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

    /// Deserialize a parsed params document and check the world dimensions.
    pub fn from_value(root: toml::Value) -> Result<Params, String> {
        let p: Params = root.try_into().map_err(|e| format!("params: {e}"))?;
        p.check_dims()?;
        Ok(p)
    }

    /// The world dimensions must tile into whole patches, and fit the u8 column coordinates and
    /// heights that trees, events and `height.bin` store. The rain gradient must be in [-1, 1], where
    /// rain never clamps below 0 and a row's total is the uniform one.
    pub fn check_dims(&self) -> Result<(), String> {
        let w = &self.world;
        let err = |m: String| Err(format!("params: [world] {m}"));
        if w.patch == 0 {
            return err("patch must be at least 1".into());
        }
        for (name, v) in [("width", w.width), ("depth", w.depth)] {
            if v == 0 || v > 256 || v % w.patch != 0 {
                return err(format!("{name} = {v} must be a multiple of patch = {} in 1..=256", w.patch));
            }
        }
        if !(2..=256).contains(&w.height) {
            return err(format!("height = {} must be in 2..=256", w.height));
        }
        let g = self.climate.rain_gradient;
        if !(-1.0..=1.0).contains(&g) {
            return Err(format!("params: [climate] rain_gradient = {g} must be in [-1, 1]"));
        }
        Ok(())
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

    /// The default params on the 64×64×32 world with uniform rain and flat terrain bias: the
    /// world the unit tests were written for.
    #[cfg(test)]
    pub(crate) fn load_square() -> Params {
        let mut p = Params::load_default();
        (p.world.width, p.world.depth, p.world.height, p.world.patch) = (64, 64, 32, 8);
        p.world.slope_bias = 0.0;
        p.climate.rain_gradient = 0.0;
        p
    }

    /// The traits an animal of this species starts with: every initial animal and every immigrant.
    pub fn default_traits(&self, kind: Kind) -> Traits {
        let (energy_cost_mult, flee_distance, repro_threshold) = match kind {
            Kind::Grazer => (1.0, self.grazer.flee_radius, self.grazer.repro_energy),
            Kind::Hunter => (1.0, self.hunter.flee_radius, self.hunter.repro_energy),
        };
        Traits { energy_cost_mult, flee_distance, repro_threshold }
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

/// Serde skip test for keys that are left out of `meta.json` at 0.
fn is_zero(v: &u32) -> bool {
    *v == 0
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

    /// An integer for `key` drawn from `i`: `i` itself, except that the world dimensions are mapped
    /// into the values `check_dims` accepts with the other dimensions at their defaults.
    fn dim_value(key: &str, i: u8) -> u32 {
        match key {
            "world.width" | "world.depth" => 8 * (1 + i as u32 % 32),
            "world.height" => 2 + i as u32 % 255,
            "world.patch" => 1 << (i % 4),
            _ => i as u32,
        }
    }

    #[test]
    fn dims_that_do_not_tile_are_rejected_naming_the_key() {
        for (bad, key) in [
            ("world.width=100", "width"),
            ("world.depth=0", "depth"),
            ("world.width=264", "width"),
            ("world.patch=0", "patch"),
            ("world.patch=5", "width"),
            ("world.height=1", "height"),
            ("climate.rain_gradient=1.5", "rain_gradient"),
            ("climate.rain_gradient=-1.01", "rain_gradient"),
        ] {
            let e = Params::from_toml_str_with(&defaults(), &[bad.into()]).unwrap_err();
            assert!(e.contains(key), "{bad}: {e}");
        }
        let p = Params::from_toml_str_with(&defaults(), &["world.width=64".into(), "world.patch=16".into()]).unwrap();
        assert_eq!((p.world.width, p.world.depth, p.world.patch), (64, 64, 16));
    }

    /// A params document from before shot 15 (no dimension, gradient or slope keys) reads as the
    /// 64×64×32 world, patch 8, uniform rain and no tilt: old `meta.json` files restore unchanged.
    #[test]
    fn missing_dims_default_to_the_square_world() {
        let mut t: toml::Table = defaults().parse().unwrap();
        let w = t["world"].as_table_mut().unwrap();
        for k in ["width", "depth", "height", "patch", "slope_bias"] {
            w.remove(k);
        }
        t["climate"].as_table_mut().unwrap().remove("rain_gradient");
        let p = Params::from_value(toml::Value::Table(t)).unwrap();
        let sq = Params::load_square();
        assert_eq!(serde_json::to_string(&p).unwrap(), serde_json::to_string(&sq).unwrap());
    }

    /// `v` as it reads back from `Params` through JSON: f32 fields hold the nearest f32.
    fn as_stored(key: &str, v: f64) -> serde_json::Value {
        serde_json::json!(if key == "hunter.kill_prob" { v } else { v as f32 as f64 })
    }

    /// `p` as JSON with every section present: `meta.json` leaves `[rng]` out at stream 0,
    /// `[animals]` out when they are enabled and `[bundle]` out at its defaults, so all three are
    /// put back here and every leaf is compared exactly.
    fn stored(p: &Params) -> serde_json::Value {
        let mut v = serde_json::to_value(p).unwrap();
        v["rng"] = serde_json::to_value(&p.rng).unwrap();
        v["animals"] = serde_json::to_value(&p.animals).unwrap();
        v["bundle"] = serde_json::to_value(&p.bundle).unwrap();
        v
    }

    fn lookup<'a>(json: &'a serde_json::Value, key: &str) -> &'a serde_json::Value {
        key.split('.').fold(json, |j, k| &j[k])
    }

    /// `--set key=value` for one leaf changes exactly that leaf in the loaded `Params`, to the given
    /// value. Integers are drawn from `ints` (0..=255 fits every integer field), numbers from `floats`.
    fn set_round_trips(key: &str, old: &toml::Value, ints: &[u8], floats: &[f64]) -> Result<(), TestCaseError> {
        let (text, want) = match old {
            toml::Value::Integer(_) => {
                let i = dim_value(key, ints[0]);
                (i.to_string(), serde_json::json!(i))
            }
            toml::Value::Float(_) => {
                // The gradient is accepted in [-1, 1] only.
                let f = if key == "climate.rain_gradient" { floats[0] / 1.0e4 } else { floats[0] };
                (f.to_string(), as_stored(key, f))
            }
            toml::Value::Boolean(b) => ((!b).to_string(), serde_json::json!(!b)),
            toml::Value::Array(a) => {
                let xs = &floats[..a.len()];
                let text = xs.iter().map(f64::to_string).collect::<Vec<_>>().join(", ");
                (format!("[{text}]"), serde_json::Value::Array(xs.iter().map(|&x| as_stored(key, x)).collect()))
            }
            other => return Err(TestCaseError::fail(format!("{key}: unexpected leaf type {other:?}"))),
        };
        let set = Params::from_toml_str_with(&defaults(), &[format!("{key}={text}")]).map_err(TestCaseError::fail)?;
        let (mut got, base) = (stored(&set), stored(&Params::load_default()));
        prop_assert_eq!(lookup(&got, key), &want, "{} = {}", key, text);
        let (section, field) = key.split_once('.').unwrap();
        // A key left out at its off value (`is_zero`) is absent from the base, not null.
        match base[section].get(field) {
            Some(v) => got[section][field] = v.clone(),
            None => _ = got[section].as_object_mut().unwrap().remove(field),
        }
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
