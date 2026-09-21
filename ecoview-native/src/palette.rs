//! The colours, and where each number in them comes from.
//!
//! Two halves, and the split is the point of shot V2. The **surface-type** palette is scene geometry:
//! one colour per voxel id, the G7 legend copied from `ecoview/src/world.ts` `MEDIUM_COLORS` so the
//! two viewers show the same site in the same colours. They are copied and not shared: the two
//! components share no code (CLAUDE.md). The **overlay** palettes are ecology, and every number in
//! them -- what the bottom of the ramp means, what the top means, and which species colour a trunk or
//! a canopy takes -- is read from the run's own `meta.json`, so the simulator owns the scale.
//!
//! V2 left one thing out: `meta.json` had nowhere to put the two hues an overlay ramps between, so
//! those two were still the `ecoview` legend copied into this file. `ecosim` shot S2 added
//! `meta.json`'s `overlays` array, and [`Overlay::ramp`] reads it, so **the whole ecology palette now
//! comes from the run**. The legend below stays as the fallback for a run written before that shot,
//! and a fallback says so on screen rather than passing itself off as the simulator's
//! (DECISIONS.md, "V2: what meta.json owns and what it does not", and "S2").

use crate::run::RunMeta;
use crate::voxel::{BUILDING, CANOPY, GRASS, ID_COUNT, SHRUB, TRUNK, VINE};

/// How many steps an overlay ramp is drawn in.
///
/// The mesher merges neighbouring faces that share a voxel id, so an overlay has to be a *set of
/// ids* rather than a colour per column, or greedy meshing would have nothing left to merge. 32
/// bands over a 0-255 field is 8 index units a band, which is below what the eye separates on a lit
/// surface, and it keeps the merging: MEASUREMENTS.md has the quad count each overlay costs.
pub const BANDS: usize = 32;
/// The first band's voxel id. Bands occupy `BAND_BASE .. BAND_BASE + BANDS`.
pub const BAND_BASE: u16 = ID_COUNT as u16;
/// Standing water: ponded depth from the run's `water.bin`, drawn as voxels on top of the ground
/// (shot S5, `crate::overlay::Ponds`).
///
/// **Its id is past the bands, not among the media.** A pond is not a surface the bundle surveyed
/// and it is not an overlay band, so it needs an id of its own; putting it at the end is what keeps
/// every existing id -- and therefore every mesh golden hash from V0 through V6 -- exactly where it
/// was. It is also why [`PALETTE_LEN`] is `+ 1` rather than the round number it used to be.
pub const POND: u16 = BAND_BASE + BANDS as u16;
/// Palette length: the surface ids, then the bands, then standing water.
pub const PALETTE_LEN: usize = ID_COUNT + BANDS + 1;

/// sRGB hex, in the scene contract's medium order, offset by one so id 0 stays air.
const HEX: [&str; ID_COUNT] = [
    "#000000", // 0 air, never drawn
    "#8b6b47", // soil
    "#79b449", // lawn
    "#a8724a", // bed
    "#6b4a2b", // mulch
    "#b9b2a3", // gravel
    "#d7d3cb", // concrete
    "#4a4a4e", // asphalt
    "#9a9a9e", // roof
    "#3a6fd8", // water
    "#c6c6cb", // building volume (ecoview BUILDING_COLOR)
    "#6b4a2b", // trunk, replaced by meta.json's tree species colour when a run is loaded
    "#3f7a2e", // canopy, replaced by meta.json's `canopy_color` when a run is loaded
    VINE_HEX,  // vine -- the viewer's own hue, see below
    "#2f6b2a", // shrub, replaced by meta.json's shrub species colour when a run is loaded
    "#7cc242", // grass, replaced by meta.json's grass species colour when a run is loaded
];

/// A vine's colour is **the viewer's**, not the run's, and it is the only plant colour here that is.
///
/// `meta.json` lists the five species the simulator has -- two ground covers, one tree, two animals
/// -- and a climber is not among them, because the simulator has no climbers (`cover.rs`). Rather
/// than dress a vine in some other species' colour and let a screenshot imply the run grew it, it
/// gets a hue of its own, a little yellower than the canopy, and every report says where it came
/// from (DECISIONS.md, V4).
pub const VINE_HEX: &str = "#3d7d2e";

/// Standing water's colour, and it is **not** the `water` medium's.
///
/// The bundle's `water` medium (`#3a6fd8`) is open water somebody surveyed -- a pool that is part of
/// the site. Ponded water is what this run did to the site since tick 0, and a reader has to be able
/// to tell the two apart at a glance, so the pond is the paler, greener blue and the legend below
/// names both. Like the vine hue it is the viewer's own: `meta.json` carries no `water` overlay row
/// for the simulator to own it with (DECISIONS.md, S5).
pub const POND_HEX: &str = "#5fc8e8";

/// The water overlay's dry band: ground with no standing water at all.
///
/// Categorical, like fire's quiet band and for the same reason -- "no water here" is not a depth, and
/// putting it on the bottom of a depth ramp would make dry ground and a one-millimetre film the same
/// colour. This viewer's, for the same reason `FIRE_QUIET_HEX` is.
const WATER_DRY_HEX: &str = "#6d6f66";
/// The water overlay's first drawn band: dry ground is band 0 and depth starts at band 1.
pub const WATER_DRY: u8 = 0;

/// The eight things the viewer can colour the ground by. `Surface` is V0's surface-type map, six are
/// the ecological overlays shot V2 was asked for, and `Water` is shot S5's: ponded depth, the one
/// field the simulator writes every snapshot that had no picture at all.
///
/// `Water` is the only one that is **not** read on the ecology grid. Its file, `water.bin`, is on the
/// bundle's finer ground grid, which is the grid the water actually ran over
/// (`crate::overlay::Ponds`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Surface,
    Light,
    Moisture,
    Fertility,
    Temperature,
    Crowding,
    Fire,
    Water,
}

impl Overlay {
    /// In the order the number keys select them, `Surface` first.
    pub const ALL: [Overlay; 8] = [
        Overlay::Surface,
        Overlay::Light,
        Overlay::Moisture,
        Overlay::Fertility,
        Overlay::Temperature,
        Overlay::Crowding,
        Overlay::Fire,
        Overlay::Water,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Overlay::Surface => "surface",
            Overlay::Light => "light",
            Overlay::Moisture => "moisture",
            Overlay::Fertility => "fertility",
            Overlay::Temperature => "temperature",
            Overlay::Crowding => "crowding",
            Overlay::Fire => "fire",
            Overlay::Water => "water",
        }
    }

    /// The wire and command-line name. An unknown name is rejected rather than guessed at, the way
    /// `EditAction::parse` rejects one.
    pub fn parse(s: &str) -> Option<Overlay> {
        Overlay::ALL.into_iter().find(|o| o.name() == s)
    }

    /// Does this overlay need the run's fields, or is it the bundle's own colours?
    pub fn is_field(self) -> bool {
        self != Overlay::Surface
    }

    /// The ramp this viewer falls back on when the run does not carry one: `ecoview/src/world.ts`
    /// `COLORS`, so a run too old to state its own palette is drawn the way the browser viewer draws
    /// it rather than in some third way.
    fn fallback_ramp(self) -> (&'static str, &'static str) {
        match self {
            // Light is a grey level in both viewers: a hue would be read as a species.
            Overlay::Light => ("#000000", "#ffffff"),
            Overlay::Moisture => ("#ffffff", "#1f4fd1"),
            Overlay::Fertility => ("#ffffff", "#4a2c12"),
            Overlay::Temperature => ("#2040ff", "#ff3020"),
            Overlay::Crowding => ("#ffffff", "#d81b9c"),
            // Fire's band 0 is quiet ground and band 1 is burnt, both off this ramp; the ramp runs
            // over the burning bands only (`Fields::bands`).
            Overlay::Fire | Overlay::Surface => ("#b3300a", "#ffb020"),
            // Water's band 0 is dry ground, off this ramp, and the ramp above it runs from a film
            // to a metre-deep pond. Deliberately not the moisture ramp's white-to-blue: one map is
            // water in the soil and the other is water standing on top of it, and a screenshot of
            // either has to be recognisable as which (`Fields` against `Ponds`).
            Overlay::Water => ("#9fe8ff", "#08246b"),
        }
    }

    /// The colours this overlay is drawn in, read from the run's `meta.json` `overlays` array.
    ///
    /// This is the reading half of `ecosim` shot S2. A run that carries a row for this overlay decides
    /// its two hues; one that does not gets [`Self::fallback_ramp`], and [`Ramp::source`] says which
    /// happened, in the same words [`crate::overlay::Scale`] uses for a number it had to guess.
    pub fn ramp(self, meta: Option<&RunMeta>) -> Ramp {
        let (lo, hi) = self.fallback_ramp();
        let mut r = Ramp {
            lo: lo.to_string(),
            hi: hi.to_string(),
            burnt: FIRE_BURNT_HEX.to_string(),
            source: format!(
                "this viewer's fallback: meta.json has no overlays.{}",
                self.name()
            ),
        };
        // `Surface` is the bundle's own media and has no ramp at all; its bands are never drawn, so
        // there is nothing for the run to own and nothing to apologise for.
        if self == Overlay::Surface {
            r.source = "the scene contract's media, not the run".into();
            return r;
        }
        let Some(row) = meta.and_then(|m| m.overlays.iter().find(|o| o.name == self.name())) else {
            return r;
        };
        r.lo = row.lo.clone();
        r.hi = row.hi.clone();
        if let Some(b) = &row.burnt {
            r.burnt = b.clone();
        }
        r.source = format!("meta.json overlays.{}", self.name());
        r
    }
}

/// What one overlay is drawn in, and where those colours came from.
///
/// The twin of [`crate::overlay::Scale`], which carries the same question about the *numbers*: a
/// viewer that knows the answer and does not show it is asking to be trusted.
#[derive(Debug, Clone, PartialEq)]
pub struct Ramp {
    /// The bottom of the scale, in sRGB hex.
    pub lo: String,
    /// The top of the scale.
    pub hi: String,
    /// Ground that burnt out since the previous snapshot: the fire overlay's second categorical band,
    /// off the ramp. The run may name it; nothing else here reads it.
    pub burnt: String,
    pub source: String,
}

impl Ramp {
    /// Are these the simulator's colours, or this viewer's copy of the `ecoview` legend?
    pub fn from_meta(&self) -> bool {
        !self.source.starts_with("this viewer's fallback")
    }
}

/// Fire's two categorical bands, which are not on the ramp: quiet ground, and ground that burnt out
/// since the previous snapshot.
///
/// The burnt colour is the run's since shot S2 (`meta.json` `overlays`, the `fire` row's `burnt`), and
/// `FIRE_BURNT_HEX` is the fallback for a run that does not carry one. Quiet ground has no entry in
/// the run, because "nothing to show here" is not an ecological quantity: it stays this viewer's,
/// like the vine hue above.
pub const FIRE_QUIET: u8 = 0;
pub const FIRE_BURNT: u8 = 1;
const FIRE_QUIET_HEX: &str = "#5a5f52";
const FIRE_BURNT_HEX: &str = "#2b2b2b";

/// sRGB channel to linear, the conversion Bevy's `LinearRgba` vertex colours want.
fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `#rrggbb` to a linear RGBA. An unparseable string is magenta, which is visible rather than silent.
pub fn linear_rgba(hex: &str) -> [f32; 4] {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xff_00ff);
    [
        to_linear(((n >> 16) & 0xff) as f32 / 255.0),
        to_linear(((n >> 8) & 0xff) as f32 / 255.0),
        to_linear((n & 0xff) as f32 / 255.0),
        1.0,
    ]
}

fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let k = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
        1.0,
    ]
}

/// The surface-type palette, one linear RGBA per voxel id. Unchanged from V0, and the mesh golden
/// test hashes meshes built with it.
pub fn surface_palette() -> [[f32; 4]; ID_COUNT] {
    let mut out = [[0.0f32; 4]; ID_COUNT];
    for (i, hex) in HEX.iter().enumerate() {
        out[i] = linear_rgba(hex);
    }
    out
}

/// The full palette for one overlay: the surface ids, with the species colours `meta.json` owns
/// substituted, then the 32 bands.
///
/// With no run loaded the surface half is exactly [`surface_palette`], so a bundle on its own looks
/// the way it did in V0 and V1.
pub fn palette(overlay: Overlay, meta: Option<&RunMeta>) -> Vec<[f32; 4]> {
    let mut out = vec![[0.0f32; 4]; PALETTE_LEN];
    for (i, hex) in HEX.iter().enumerate() {
        out[i] = linear_rgba(hex);
    }
    // The simulator owns the species colours (CLAUDE.md), and V0 hard-coded two of them. The trunk
    // it guessed right and the canopy it did not: `meta.json` says `#2e8b3d` and the constant above
    // says `#3f7a2e`. With a run loaded, the file wins.
    if let Some(m) = meta {
        if let Some(t) = m.species.iter().find(|s| s.kind == "tree") {
            out[TRUNK as usize] = linear_rgba(&t.color);
            if let Some(c) = &t.canopy_color {
                out[CANOPY as usize] = linear_rgba(c);
            }
        }
        // The two ground covers likewise. They are voxels here rather than entities, but the
        // colour question is the same one: the simulator named the species, so it names the colour.
        // Both have kind `cover`, so this matches on `name`, which is the field that tells them
        // apart; a run that renames them falls back to the legend above rather than mixing them up.
        for (name, id) in [("grass", GRASS), ("shrub", SHRUB)] {
            if let Some(s) = m.species.iter().find(|s| s.name == name) {
                out[id as usize] = linear_rgba(&s.color);
            }
        }
    }
    let ramp = overlay.ramp(meta);
    let (lo, hi) = (linear_rgba(&ramp.lo), linear_rgba(&ramp.hi));
    for b in 0..BANDS {
        out[ID_COUNT + b] = lerp(lo, hi, b as f32 / (BANDS - 1) as f32);
    }
    out[POND as usize] = linear_rgba(POND_HEX);
    if overlay == Overlay::Water {
        // The same shape as fire's: band 0 is a categorical "none here" and the ramp starts above
        // it, so a film of water is already the palest blue rather than a third of the way up a
        // ramp whose bottom means dry.
        let first = WATER_DRY as usize + 1;
        for b in first..BANDS {
            out[ID_COUNT + b] = lerp(lo, hi, (b - first) as f32 / (BANDS - first - 1) as f32);
        }
        out[ID_COUNT + WATER_DRY as usize] = linear_rgba(WATER_DRY_HEX);
    }
    if overlay == Overlay::Fire {
        // Fire's ramp covers the burning bands only, so a patch with one tick left is already dull
        // orange rather than a third of the way up a ramp whose bottom means "not on fire".
        let first = FIRE_BURNT as usize + 1;
        for b in first..BANDS {
            out[ID_COUNT + b] = lerp(lo, hi, (b - first) as f32 / (BANDS - first - 1) as f32);
        }
        out[ID_COUNT + FIRE_QUIET as usize] = linear_rgba(FIRE_QUIET_HEX);
        out[ID_COUNT + FIRE_BURNT as usize] = linear_rgba(&ramp.burnt);
    }
    out
}

/// The voxel id under an occlusion level. With ambient occlusion on, a voxel's id is its own plus
/// [`PALETTE_LEN`] per level of shade (`mesh::bake_occlusion`), so anything that wants to know what
/// a voxel *is* rather than how dark it is takes this first.
pub fn base_id(id: u16) -> u16 {
    id % PALETTE_LEN as u16
}

/// The legend, for the measurement write-up and for a caller that wants to name an id.
pub fn id_name(id: u16) -> &'static str {
    match base_id(id) {
        0 => "air",
        BUILDING => "building",
        TRUNK => "trunk",
        CANOPY => "canopy",
        VINE => "vine",
        POND => "standing water",
        SHRUB => "shrub",
        GRASS => "grass",
        n if (n as usize) < ID_COUNT => "medium",
        n if (n as usize) < PALETTE_LEN => "overlay band",
        _ => "unknown",
    }
}
