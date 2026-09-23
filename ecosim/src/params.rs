//! All tunable parameters, loaded from `params.toml`. Nothing tunable is hard-coded elsewhere.

use crate::animals::Kind;
use crate::heredity::Traits;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Piecewise-linear suitability curve: (min, low-opt, high-opt, max).
pub type Curve = [f32; 4];

/// All tunable parameters, one field per `params.toml` section.
///
/// **Every section is serialized, at its defaults or not** (shot S2). Three of them used to be left
/// out of `meta.json` when they were at their defaults, which read as economy and was a defect: a
/// reader of a run at the defaults -- the ordinary case -- could not tell whether the run had the
/// default or whether the key predated the run. Each section is still `#[serde(default)]` on the way
/// *in*, so a `params.toml` or an older `meta.json` that omits one still loads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// `[world]`
    pub world: WorldParams,
    /// `[climate]`
    pub climate: ClimateParams,
    /// `[schedule]`
    pub schedule: ScheduleParams,
    /// `[rain]`
    pub rain: RainParams,
    /// `[hydro]`
    pub hydro: HydroParams,
    /// `[npk]`. Always written to `meta.json`, even at its defaults (shot S2's rule, shot G5's
    /// section).
    #[serde(default)]
    pub npk: NpkParams,
    /// `[pipes]`. Always written to `meta.json`, even at its defaults (shot S2's rule, shot G6's
    /// section).
    #[serde(default)]
    pub pipes: PipesParams,
    /// `[medium.*]`
    pub medium: MediaParams,
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
    /// `[bundle]`. Always written to `meta.json`, even at its defaults (shot S2).
    #[serde(default)]
    pub bundle: BundleParams,
    /// `[sun]`: the sun that moves and the light budget per column (shot G9). Always written to
    /// `meta.json`, even at its defaults (shot S2's rule).
    #[serde(default)]
    pub sun: SunParams,
    /// `[animals]`. Always written to `meta.json`, even at its defaults (shot S2).
    #[serde(default)]
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
    /// `[rng]`. Always written to `meta.json`, even at its defaults (shot S2).
    #[serde(default)]
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
    /// Beer-Lambert extinction coefficient of the canopy (shot G4c): one canopy voxel transmits
    /// `exp(-canopy_k x canopy_lai)` of the light that reaches it. 0.4-0.7 for a broadleaf canopy
    /// (UNITS.md R10).
    pub canopy_k: f32,
    /// Leaf area index of one canopy voxel, m2 of leaf per m2 of ground. A mature sim crown is two
    /// voxels deep, so its whole-canopy LAI is twice this.
    pub canopy_lai: f32,
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
    /// Scene height in metres that maps onto `tree.mature_age` when the bundle's trees are planted
    /// (shot G3): the height of a mature sim canopy above the ground it stands on.
    pub tree_mature_height: f32,
    /// Scene height in metres that maps onto `tree_tall_age`: a full-grown street tree. Heights
    /// above it all import at `tree_tall_age`.
    pub tree_tall_height: f32,
    /// Age in **years** an imported tree of `tree_tall_height` or more starts at (shot G4c: was
    /// `tree_tall_age`, in ticks). Held at `tree.mature_age_years` or more, so the height-to-age
    /// map stays monotone whatever it is set to.
    pub tree_tall_age_years: f32,
    /// How far in columns an imported tree may be moved off an unplantable column to the nearest
    /// plantable one; past it the tree is dropped.
    pub tree_move_radius: f32,
}

impl Default for BundleParams {
    fn default() -> Self {
        BundleParams {
            base_z: 8,
            tree_mature_height: 3.0,
            tree_tall_height: 20.0,
            tree_tall_age_years: 0.75,
            tree_move_radius: 2.0,
        }
    }
}

/// The sun that moves (shot G9, `src/sun.rs`): a light budget per column from the sun's path
/// over the day and the year. Only a world with buildings -- a bundle world -- has anything to
/// shade; a noise world's light is the open sky's whatever these are.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SunParams {
    /// Degrees north of the equator (negative south). A bundle that carries its own
    /// `latitude_deg` uses that instead; this is the latitude of a world that has none. Like every
    /// key here it is taken as given and clamped where it is used: a latitude past a pole is the
    /// pole, a share outside 0 to 1 is the nearer end, and 0 samples is 1.
    pub latitude: f32,
    /// Mean share of daylight lost to cloud, 0 to 1.
    pub cloud_cover: f32,
    /// Sun positions per sampled day, sunrise to sunset.
    pub day_samples: u32,
    /// Sampled days per year, evenly spaced from the spring equinox: 4 is the two equinoxes and the
    /// two solstices. The run's light follows the slice whose day is nearest.
    pub season_samples: u32,
    /// Share of daylight that arrives from the whole sky rather than straight from the sun, 0 to 1.
    pub diffuse_fraction: f32,
}

impl Default for SunParams {
    fn default() -> Self {
        SunParams { latitude: 42.7, cloud_cover: 0.5, day_samples: 9, season_samples: 4, diffuse_fraction: 0.4 }
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
    /// Detritus decay constant, per year (shot G4b: it was per 10-tick soil update, and 0.015 per
    /// update is 6.0 a year). The share of a patch's detritus that decays into the fertility of its
    /// soil columns over one soil update is `decay_k` times the update's length in years, times the
    /// temperature and moisture factors. A model constant: 6.0 a year is an order of magnitude above
    /// the published 0.3-0.6 a year for temperate grass litter, which is a finding recorded for the
    /// nutrient shot rather than something this shot retunes (UNITS.md R11 and finding 9).
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

/// How often each staggered tier updates, in ticks (shot G4b). Every cadence lives here so that a
/// tier's rate and the cadence it is charged over are read from the same value: a rate is per hour
/// or per year and is multiplied by `cadence × tick_hours`, so doubling a cadence leaves an annual
/// total alone. `tree.update_every`, `world.compact_every` and the two animal
/// `immigration_interval`s were already parameters and stay where they are; `tree.seed_every` and
/// `tree.immigration_interval` became rates per year in shots G4c and G4e, and their tick cadences
/// are derived by [`Params::ticks_between`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleParams {
    /// Ticks between a patch's grass-and-shrub updates; also the stagger, so about
    /// `patches / cover_every` patches update per tick.
    pub cover_every: u32,
    /// Ticks between soil updates: the water tier's settling half (or the pre-G4 moisture steps),
    /// detritus decay and the fertility clamp.
    pub soil_every: u32,
    /// Ticks between temperature and season updates.
    pub temperature_every: u32,
    /// Ticks between ignition draws.
    pub fire_every: u32,
}

/// Storms (shot G4). Rain arrives as whole storms instead of a trickle every soil update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RainParams {
    /// Rain a year, in millimetres, at `climate.year_len` ticks to the year (shot G4b). The chance
    /// per tick that a storm starts is `annual_mm / (year_len × storm_mean_mm)` before the season's
    /// rain factor, whose mean over a year is 1, so this is the expected annual depth whatever
    /// `year_len` and `storm_mean_mm` are. 0 leaves the rain out and keeps the draw.
    pub annual_mm: f32,
    /// Mean storm depth in millimetres; the depth is drawn exponentially.
    pub storm_mean_mm: f32,
}

/// Surface water and soil water (shot G4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HydroParams {
    /// Whether the water tier runs at all. False restores the pre-G4 moisture update.
    pub enabled: bool,
    /// Evaporation of ponded water, mm per hour at the mean temperature.
    pub evap_mm_h: f32,
    /// Evaporation and transpiration from soil water, mm per hour at the mean temperature and
    /// full plant cover.
    pub et_mm_h: f32,
    /// Soil water at tick 0, as a fraction of the available water capacity.
    pub initial_fill: f32,
    /// The most a column's soil holds, as a multiple of field capacity: what is above field
    /// capacity is the part that percolates away as drainage.
    pub saturation: f32,
    /// Fertility leached per millimetre drained out of the bottom of a column. 0 turns it off.
    /// Read only on the pre-G5 fertility path: with `npk.enabled` the three nutrient pools leach
    /// by the mixing rule in [`crate::npk`] instead, which needs no coefficient.
    pub leach_k: f32,
    /// Soil water above this fraction of field capacity is waterlogging (shot G5).
    #[serde(default = "waterlog_frac_default")]
    pub waterlog_frac: f32,
    /// Ticks a column must stay that wet before it counts as waterlogged. 0 would make every wet
    /// tick count, so the mechanism's off switch is `npk.enabled`, not this.
    #[serde(default = "waterlog_ticks_default")]
    pub waterlog_ticks: u32,
}

fn waterlog_frac_default() -> f32 {
    0.95
}

fn waterlog_ticks_default() -> u32 {
    400
}

/// Soil nitrogen, phosphorus and potassium (shot G5): the three pools that replace the 0-255
/// fertility index.
///
/// `enabled` is the shot's one rate switch, and it covers everything the shot adds -- the three
/// pools, Liebig-limited growth, nutrient leaching and runoff, and waterlogging. With it false the
/// pre-G5 fertility index runs instead, exactly as `hydro.enabled` false runs the pre-G4 moisture
/// update, and a run is byte-identical to one written before this shot apart from the six new
/// always-zero `series.csv` columns (DECISIONS.md, "One switch for the whole of G5").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpkParams {
    /// Whether the nutrient tier runs at all. False restores the fertility index.
    pub enabled: bool,
    /// Grams of nitrogen per square metre, per unit of `climate.initial_fertility`, at tick 0.
    /// Plant-available (mineral) nitrogen only, which in a temperate soil is a small fraction of
    /// the total: 0.02 × 128 is 2.6 g/m², about 26 kg/ha (UNITS.md R14).
    pub init_n: f32,
    /// Grams of phosphorus per square metre per unit of initial fertility, at tick 0. This is the
    /// whole pool, nearly all of it bound to soil particles; `p_avail_frac` of it is what a plant
    /// can reach.
    pub init_p: f32,
    /// Grams of potassium per square metre per unit of initial fertility, at tick 0.
    pub init_k: f32,
    /// Nitrogen, phosphorus and potassium in the litter each square metre of plantable ground
    /// starts under, in g/m². The organic stock the mineral pool is fed from, not part of it.
    #[serde(default = "init_detritus_default")]
    pub init_detritus: [f32; 3],
    /// The share of the phosphorus pool a plant can take up and runoff can carry.
    pub p_avail_frac: f32,
    /// Half-saturation multipliers, N, P, K. Growth's nutrient factor is
    /// `min_i(a_i / (a_i + half_sat_i × need_i))` over the available pools `a_i`, so
    /// `half_sat_i × need_i` is the pool at which that nutrient alone halves growth: the unit is
    /// units-of-growth, and a larger number is a hungrier plant.
    pub half_sat: [f32; 3],
    /// Nitrogen deposited from the atmosphere, g/m² per year, spread over every plantable column.
    /// The only external input any of the three pools has.
    pub n_deposition: f32,
    /// Potassium leaches at this multiple of the nitrogen rate.
    pub k_leach_ratio: f32,
    /// Grams of phosphorus one millimetre of runoff lifts off a square metre of ground, capped at
    /// the available phosphorus under it.
    pub p_runoff_g_per_mm: f32,
    /// The share of a column's nitrogen that sits in the layer runoff mixes with.
    pub n_runoff_frac: f32,
    /// The share of a burnt plant's nitrogen that goes up as smoke. What is left of it, and all of
    /// its phosphorus and potassium, lands on the patch as ash. Fire is the only sink any of the
    /// three pools has that water does not open.
    pub fire_n_volatilised: f32,
}

impl Default for NpkParams {
    fn default() -> Self {
        NpkParams {
            enabled: true,
            init_n: 0.012,
            init_p: 0.4,
            init_k: 0.3,
            init_detritus: init_detritus_default(),
            p_avail_frac: 0.1,
            half_sat: [0.28, 2.5, 1.0],
            n_deposition: 2.5,
            k_leach_ratio: 0.03,
            p_runoff_g_per_mm: 0.002,
            n_runoff_frac: 0.1,
            fire_n_volatilised: 0.9,
        }
    }
}

/// Storm drains (shot G6): the scene's pipes as a working network.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipesParams {
    /// Multiplier on every pipe's `capacity_m3h`. The shot's rate switch: 0 captures nothing,
    /// routes no extra pass and logs no pipe event, and the run is byte-identical to one written
    /// before the network existed apart from the two always-zero `series.csv` columns.
    pub capacity_scale: f32,
}

impl Default for PipesParams {
    fn default() -> Self {
        PipesParams { capacity_scale: 1.0 }
    }
}

/// One row of the media table: what a surface does with water, and whether anything grows in it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediumParams {
    /// Fastest rate water soaks in, mm per hour.
    pub infiltration_mm_h: f32,
    /// Plant-available water a column of this medium holds, mm: the available water capacity of the
    /// rooting zone, the water between field capacity and the permanent wilting point. A store of 0
    /// is the wilting point (UNITS.md section 1). The name is kept from shot G4.
    pub field_capacity_mm: f32,
    /// Drainage out of the bottom above field capacity, mm per hour.
    pub percolation_mm_h: f32,
    /// Whether a plant roots in it.
    pub plantable: bool,
}

/// The media table, one entry per medium of the scene contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaParams {
    /// `[medium.soil]`
    pub soil: MediumParams,
    /// `[medium.lawn]`
    pub lawn: MediumParams,
    /// `[medium.bed]`
    pub bed: MediumParams,
    /// `[medium.mulch]`
    pub mulch: MediumParams,
    /// `[medium.gravel]`
    pub gravel: MediumParams,
    /// `[medium.concrete]`
    pub concrete: MediumParams,
    /// `[medium.asphalt]`
    pub asphalt: MediumParams,
    /// `[medium.roof]`
    pub roof: MediumParams,
    /// `[medium.water]`
    pub water: MediumParams,
}

impl MediaParams {
    /// The row for a medium.
    pub fn get(&self, m: crate::bundle::Medium) -> &MediumParams {
        use crate::bundle::Medium as M;
        match m {
            M::Soil => &self.soil,
            M::Lawn => &self.lawn,
            M::Bed => &self.bed,
            M::Mulch => &self.mulch,
            M::Gravel => &self.gravel,
            M::Concrete => &self.concrete,
            M::Asphalt => &self.asphalt,
            M::Roof => &self.roof,
            M::Water => &self.water,
        }
    }
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
    /// Water taken from each soil column per unit of cover fraction gained, in millimetres. The
    /// standing transpiration of the cover that is already there is `hydro.et_mm_h`; this is only
    /// what new growth costs, and it is a declared model constant (UNITS.md section 3.3).
    pub water_per_growth_mm: f32,
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
    /// Maximum growth rate, per year. One update grows by `r × f_L f_M f_T f_F × (1 − density)`
    /// times the update's own length in years, so the cadence does not change a year's growth.
    pub r: f32,
    /// Mortality rate, per year, charged over the update's length like `r`.
    pub g: f32,
    /// Density on every soil patch at tick 0.
    pub initial: f32,
    /// Suitability over patch light, as a fraction of full sun (shot G4c: was the 0-255 index).
    pub light: Curve,
    /// Suitability over patch soil water, as a fraction of the available water capacity.
    pub moisture: Curve,
    /// Suitability over patch temperature.
    pub temp: Curve,
    /// Nutrient needs and waterlogging tolerance (shot G5), `[grass.npk]` and `[shrub.npk]`.
    #[serde(default)]
    pub npk: SpeciesNpk,
}

/// What one producer species does with nitrogen, phosphorus, potassium and standing water
/// (shot G5).
///
/// The three needs are grams of the element per unit of growth, where a unit of growth is one unit
/// of cover density over one square metre for the two ground covers, and one metre of height for
/// the tree. They are both the species' demand and the composition of its litter: what a plant
/// takes up per unit of growth is what its dead tissue returns per unit of death, so the plant pool
/// is `need × biomass` exactly and never has to be tracked alongside it (DECISIONS.md, shot G5).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeciesNpk {
    /// Grams of nitrogen per unit of growth.
    pub need_n: f32,
    /// Grams of phosphorus per unit of growth.
    pub need_p: f32,
    /// Grams of potassium per unit of growth.
    pub need_k: f32,
    /// Growth on a waterlogged column is multiplied by this, in [0, 1]: 0 is a species that stops
    /// dead in standing water and 1 one that does not notice it.
    pub waterlog_tolerance: f32,
}

impl Default for SpeciesNpk {
    fn default() -> Self {
        SpeciesNpk { need_n: 2.5, need_p: 0.2, need_k: 2.0, waterlog_tolerance: 0.8 }
    }
}

impl SpeciesNpk {
    /// The three needs in N, P, K order.
    pub fn needs(&self) -> [f32; 3] {
        [self.need_n, self.need_p, self.need_k]
    }
}

/// The tree species.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeParams {
    /// Trees planted at tick 0.
    pub initial_count: u32,
    /// Age of the initial trees, in years (shot G4c: was `initial_age`, in ticks).
    pub initial_age_years: f32,
    /// Ticks between tree updates: a cadence, like the `[schedule]` keys, and not a rate.
    pub update_every: u32,
    /// Age in years at which a sapling becomes young (was `young_age`).
    pub young_age_years: f32,
    /// Age in years at which a tree becomes mature and seeds (was `mature_age`).
    pub mature_age_years: f32,
    /// Mean lifespan in years: each tree dies at its own `max_age_years · (1 + lifespan_jitter · u)`,
    /// u uniform in [−1, 1] (was `max_age`, in ticks).
    pub max_age_years: f32,
    /// Relative spread of per-tree lifespans around `max_age`.
    pub lifespan_jitter: f32,
    /// Per-update death chance of a mature tree whose crown is at least `crowding_overlap` covered
    /// by other live crowns (shot S11: the count of trunks this used to read is gone).
    pub crowding_mortality: f32,
    /// Fraction of a mature tree's crown footprint that other crowns must cover before
    /// `crowding_mortality` is rolled against it (shot S11).
    ///
    /// The quantity is [`Sim::crown_crowding`](crate::sim::Sim::crown_crowding): the same crowns
    /// shot S3 published and the same mean-field combination [`Sim::crown_light`](crate::sim::Sim::crown_light) makes, so a tree
    /// is judged at the scale of the crown it has (6–12 m across when mature) and not at the scale
    /// of the 3×3 m stamp its canopy voxels shade. At 1.0 nothing but a completely buried crown is
    /// ever at risk; at 0.0 every mature tree with a live neighbour anywhere is.
    pub crowding_overlap: f32,
    /// Transpiration of one tree, millimetres per hour over its trunk column. An update takes
    /// `transpiration_mm_h × update_every × tick_hours` millimetres, so the cadence does not change
    /// a year's water use. It is charged to the trunk column alone, though a mature crown covers
    /// nine, which is why it is calibrated at the low end of the published range (UNITS.md 3.3).
    pub transpiration_mm_h: f32,
    /// How much of its transpiration a leafless tree gives up (shot G10). A tree's draw is
    /// `transpiration_mm_h × (1 − deciduous × (1 − leaf_on)) / mean`, where `leaf_on` is its
    /// patch's share of a full canopy ([`Sim::leaf_on`](crate::sim::Sim::leaf_on)) and `mean` is
    /// the numerator's mean over the model year
    /// ([`Sim::leaf_draw_mean`](crate::sim::Sim::leaf_draw_mean)), so a year's transpiration is
    /// unchanged and only its season moves. 0 is an evergreen and the model before G10, and at 0
    /// the patch temperature is not even read; 1 is a tree that transpires nothing in leaf-off.
    #[serde(default = "tree_deciduous_default")]
    pub deciduous: f32,
    /// Patch temperature in °C at and below which a tree carries no leaves (shot G10).
    #[serde(default = "tree_leaf_off_temp_default")]
    pub leaf_off_temp: f32,
    /// Patch temperature in °C at and above which a tree is in full leaf (shot G10). Between the
    /// two the canopy is linear in temperature; if this is not above `leaf_off_temp` the change is
    /// a step at this temperature.
    #[serde(default = "tree_leaf_on_temp_default")]
    pub leaf_on_temp: f32,
    /// Soil water below which a tree counts as dry, as a fraction of the column's available water
    /// capacity. 0 is the permanent wilting point.
    pub dry_fraction: f32,
    /// Consecutive days of drought after which a tree dies (shot G4c: was `dry_death_ticks` = 500
    /// ticks, which is the same 45.7 days; the one tree constant the units audit found already
    /// right, and so the calibration point for the rest — UNITS.md section 7).
    pub dry_death_days: f32,
    /// Seeding attempts a mature tree makes per year (was `seed_every`, one attempt every 200
    /// ticks). One attempt every `round(year_len / seeds_per_year)` ticks of its age.
    pub seeds_per_year: f32,
    /// Maximum seed distance in columns.
    pub seed_radius: f32,
    /// Light low-opt of the germination curve, as a fraction of full sun; replaces `light[1]`
    /// (shot G4c: was a level on the 0-255 light index).
    pub sapling_light: f32,
    /// Minimum Chebyshev distance between trunks.
    pub min_spacing: i32,
    /// Detritus a dead tree adds to its patch.
    pub death_detritus: f32,
    /// Crown radius as a fraction of the tree's height (shot S3). Measured, not chosen: the median
    /// of that ratio over the 81 surveyed trees in `worlds/capitol/trees.json`, the only real crown
    /// dimensions this project owns. It describes the crown the tree *has*, which is not the 3x3
    /// column footprint its canopy voxels *shade*; see [`Sim::crown_light`](crate::sim::Sim::crown_light).
    pub crown_radius_frac: f32,
    /// Height of the lowest branch as a fraction of the tree's height, measured the same way. With
    /// `crown_radius_frac` it fixes the crown envelope, and so how deep a neighbour's crown a ray
    /// to this one's middle passes through.
    pub crown_base_frac: f32,
    /// Nutrient needs per metre of height growth and waterlogging tolerance (shot G5), `[tree.npk]`.
    #[serde(default)]
    pub npk: SpeciesNpk,
    /// Chance per tree update that a tree on a waterlogged column dies, before its tolerance: the
    /// roll is `waterlog_mortality × (1 − tolerance)` and the death is logged with cause
    /// `waterlog` (shot G5). 0 leaves the roll out and touches no RNG.
    #[serde(default = "tree_waterlog_mortality_default")]
    pub waterlog_mortality: f32,
    /// Germination suitability over surface light, as a fraction of full sun (low-opt replaced by
    /// `sapling_light`).
    pub light: Curve,
    /// Germination suitability over the column's soil water, as a fraction of its available water
    /// capacity.
    pub moisture: Curve,
    /// Germination suitability over patch temperature.
    pub temp: Curve,
    /// One sapling arrives when fewer than this many trees are alive (0 disables immigration).
    pub immigration_floor: u32,
    /// Tree immigration checks a year (shot G4e: was `immigration_interval`, one check every 500
    /// ticks, and 4000/500 = 8). One check every `round(year_len / immigrants_per_year)` ticks of
    /// the **world** clock, not of a tree's age; 0 leaves immigration out. A check that finds the
    /// population at or above `immigration_floor` arrives at nothing, so this is a ceiling on
    /// arrivals, not a rate of them.
    pub immigrants_per_year: f32,
}

/// Whether the two animal species take part in a run at all (shot G0). Garden runs turn them off;
/// the species themselves are unchanged, and `enabled = true` is the default every earlier run had.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimalsParams {
    /// False leaves every grazer and hunter out: none are placed, none immigrate, and the animal
    /// phase is skipped, so it draws nothing from the RNG.
    pub enabled: bool,
    /// Grams of nitrogen, phosphorus and potassium an animal's body can hold per unit of energy
    /// (shot G5). Every animal is born, placed and immigrates empty and fills up by eating;
    /// whatever it eats above this it passes straight through into the dung of the patch it is
    /// standing on, and whatever is left in it at death goes to that patch as a corpse. So the
    /// number is a residence time, not a stock: raise it and the herd holds the site's nitrogen
    /// for longer before the soil gets it back.
    #[serde(default = "animal_npk_default")]
    pub npk_content: [f32; 3],
}

/// Starting litter: 55 g/m² of nitrogen, 4 of phosphorus and 35 of potassium.
fn init_detritus_default() -> [f32; 3] {
    [55.0, 4.0, 35.0]
}

fn animal_npk_default() -> [f32; 3] {
    [0.02, 0.002, 0.015]
}

impl Default for AnimalsParams {
    fn default() -> Self {
        AnimalsParams { enabled: true, npk_content: animal_npk_default() }
    }
}

fn tree_waterlog_mortality_default() -> f32 {
    0.01
}

fn tree_deciduous_default() -> f32 {
    0.5
}

fn tree_leaf_off_temp_default() -> f32 {
    5.0
}

fn tree_leaf_on_temp_default() -> f32 {
    10.0
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
    /// cost (type II functional response). 0 switches handling off, and 0 is written to
    /// `meta.json` like any other value (shot S2).
    #[serde(default)]
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
    /// Ignitions per patch per year, before the weather: one fire update draws against
    /// `base_rate × f(T) × dryness² × fuel` times the update's own length in years, where dryness is
    /// `1 − soil water / available water capacity`. 0 means nothing ever ignites.
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
    /// Per-tick spread chance to each 4-neighbour: `spread × neighbour fuel × neighbour dryness`.
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

    /// Germination light curve with the sapling light need applied as its low-opt. Both are
    /// fractions of full sun since shot G4c, so a need at or below the curve's minimum leaves the
    /// curve a step at that minimum rather than a ramp (`suitability` resolves the tie to 0).
    pub fn tree_light_curve(&self) -> Curve {
        let mut c = self.tree.light;
        c[1] = self.tree.sapling_light.max(c[0]);
        c
    }

    /// Optical depth of one canopy voxel, `canopy_k x canopy_lai`: the exponent of the
    /// Beer-Lambert transmittance `exp(-k·LAI)` each layer of canopy applies (shot G4c).
    pub fn canopy_extinction(&self) -> f32 {
        (self.world.canopy_k * self.world.canopy_lai).max(0.0)
    }

    /// Ticks between two events that happen `per_year` times a year, from `climate.year_len`:
    /// `round(year_len / per_year)`, never less than one tick. A rate of 0 gives [`u32::MAX`], which
    /// leaves the event out rather than dividing by zero — no tick and no tree age is a multiple of
    /// it. This is the one place a rate per year becomes a cadence in ticks (shots G4c and G4e).
    pub fn ticks_between(&self, per_year: f32) -> u32 {
        if per_year > 0.0 {
            (self.climate.year_len as f64 / per_year as f64).round().clamp(1.0, u32::MAX as f64) as u32
        } else {
            u32::MAX
        }
    }

    /// Ticks between tree immigration checks, from `tree.immigrants_per_year` (shot G4e). A **world**
    /// cadence, not a tree age, which is why it is here and not in [`TreeAges`]: `immigrate` tests it
    /// against the tick counter in its own phase right after animals (DECISIONS.md, shot 11).
    pub fn tree_immigration_every(&self) -> u32 {
        self.ticks_between(self.tree.immigrants_per_year)
    }

    /// Ticks in `years` years of this run, from `climate.year_len`.
    pub fn ticks_in_years(&self, years: f32) -> u32 {
        (years as f64 * self.climate.year_len as f64).round().clamp(0.0, u32::MAX as f64) as u32
    }

    /// The tree tier's ages and schedules in ticks (shot G4c). Every one of them is a number of
    /// years or days in `params.toml`, turned into ticks here and nowhere else, so that the tree
    /// tier means the same amount of simulated time whatever `climate.year_len` is.
    pub fn tree_ages(&self) -> TreeAges {
        let t = &self.tree;
        let dry = t.dry_death_days as f64 * 24.0 / crate::hydro::tick_hours(self);
        TreeAges {
            initial: self.ticks_in_years(t.initial_age_years),
            young: self.ticks_in_years(t.young_age_years),
            mature: self.ticks_in_years(t.mature_age_years),
            max: self.ticks_in_years(t.max_age_years),
            tall_import: self.ticks_in_years(self.bundle.tree_tall_age_years),
            dry_death: dry.round().clamp(0.0, u32::MAX as f64) as u32,
            seed_every: self.ticks_between(t.seeds_per_year),
        }
    }
}

/// The tree tier's ages and schedules in ticks, from [`Params::tree_ages`]. Nothing else converts a
/// tree age: the parameters are years and days, and these are what the tier runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeAges {
    /// Age the initial trees are planted at.
    pub initial: u32,
    /// Age a sapling becomes young at.
    pub young: u32,
    /// Age a tree becomes mature and starts seeding at.
    pub mature: u32,
    /// Mean lifespan, before `lifespan_jitter`.
    pub max: u32,
    /// Age an imported scene tree of `bundle.tree_tall_height` or more starts at.
    pub tall_import: u32,
    /// Consecutive dry ticks that kill a tree.
    pub dry_death: u32,
    /// Ticks between a mature tree's seeding attempts; `u32::MAX` when `seeds_per_year` is 0, which
    /// leaves seeding out, no tree age being a multiple of it.
    pub seed_every: u32,
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

/// `--set` overrides, the world-dimension checks, and the serialized params, every section of
/// which reaches `meta.json` whether or not it is at its defaults (shot S2).
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
            ["hunter.kill_prob=0.25", "tree.mature_age_years=1.5", "grass.initial=1", "shrub.light=[1, 2.5, 3, 4]"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        let p = Params::from_toml_str_with(&defaults(), &sets).unwrap();
        assert_eq!(p.hunter.kill_prob, 0.25);
        assert_eq!(p.tree.mature_age_years, 1.5);
        assert_eq!(p.tree_ages().mature, 6000, "1.5 years is 6000 ticks at year_len 4000");
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
        for bad in ["tree.initial_count=1.5", "hunter.kill_prob=high", "shrub.light=[1, 2]", "grazer.cooldown=abc"] {
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
        // Serializing is enough: since shot S2 `[rng]`, `[animals]` and `[bundle]` are written at
        // their defaults like every other section, so no section has to be put back by hand here.
        let (mut got, base) =
            (serde_json::to_value(&set).unwrap(), serde_json::to_value(Params::load_default()).unwrap());
        prop_assert_eq!(lookup(&got, key), &want, "{} = {}", key, text);
        // Put the leaf back the way it was, then nothing else may have moved. Keys nest more than
        // one level deep since the media table (shot G4), so this walks the whole path.
        let path: Vec<&str> = key.split('.').collect();
        let (field, parents) = path.split_last().unwrap();
        let (mut g, mut b) = (&mut got, &base);
        for p in parents {
            g = &mut g[*p];
            b = &b[*p];
        }
        // Since shot S2 no key is ever left out of the serialized params, so every leaf the
        // override names is in the base too; an absent one is a bug in this test's key list.
        let was = b.get(field).unwrap_or_else(|| panic!("{key} is not in the serialized params"));
        g[*field] = was.clone();
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
