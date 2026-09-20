//! The one overlay V0 has: surface type. The colours are the G7 legend, copied from
//! `ecoview/src/world.ts` `MEDIUM_COLORS` so the two viewers show the same site in the same colours.
//! They are copied and not shared: the two components share no code (CLAUDE.md).

use crate::voxel::{BUILDING, CANOPY, ID_COUNT, TRUNK};

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
    "#6b4a2b", // trunk
    "#3f7a2e", // canopy
];

/// sRGB channel to linear, the conversion Bevy's `LinearRgba` vertex colours want.
fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The surface-type palette, one linear RGBA per voxel id.
pub fn surface_palette() -> [[f32; 4]; ID_COUNT] {
    let mut out = [[0.0f32; 4]; ID_COUNT];
    for (i, hex) in HEX.iter().enumerate() {
        let n = u32::from_str_radix(&hex[1..], 16).unwrap_or(0xff00ff);
        out[i] = [
            to_linear(((n >> 16) & 0xff) as f32 / 255.0),
            to_linear(((n >> 8) & 0xff) as f32 / 255.0),
            to_linear((n & 0xff) as f32 / 255.0),
            1.0,
        ];
    }
    out
}

/// The legend, for the measurement write-up and for a caller that wants to name an id.
pub fn id_name(id: u16) -> &'static str {
    match id {
        0 => "air",
        BUILDING => "building",
        TRUNK => "trunk",
        CANOPY => "canopy",
        n if (n as usize) < ID_COUNT => "medium",
        _ => "unknown",
    }
}
