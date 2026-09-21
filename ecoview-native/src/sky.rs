//! The sky, the sun and the season: **expression, driven by the run's own clock**.
//!
//! Shot V6. Nothing here is ecology. The simulator computes its light field under a **fixed 45
//! degree sun** (`ecosim/src/bundle.rs`, shot G1) and has no time of day at all, so the sun drawn
//! here is not the sun the ecology was computed with, and moving it changes no number in any run.
//! What it does change is whether a site is legible: a low sun throws the long shadows that show a
//! terrace is a terrace, and a seasonal tint says which part of the year a snapshot is.
//!
//! | What | Where it comes from | Whose it is |
//! |---|---|---|
//! | day of the year | the snapshot's tick over `meta.json`'s `year_len`, aligned so the simulator's own temperature peak is the summer solstice | the run's |
//! | hour of the day | the viewer's clock, `--hour` and the **K** and **L** keys | the viewer's |
//! | latitude | `--lat`, default [`DEFAULT_LATITUDE_DEG`] | the viewer's -- no bundle or run carries one |
//! | sun elevation and azimuth | the standard solar geometry below, from those three | derived |
//! | leaf and grass colour | the base hue is `meta.json`'s species colour; the seasonal departure from it is this module's | both, and the HUD says which is which |
//!
//! **The hour is deliberately not taken from the tick**, although it could be: a tick is
//! `8766 / year_len` hours (`ecosim/UNITS.md`), so tick 100 really is 219.15 hours into the run and
//! really is 03:09 in the morning. Half of every run's snapshots would then be photographed in the
//! dark, for a diurnal cycle the simulator does not model. The day of the year is a different case
//! -- the simulator's temperature and rain both swing on it -- so that half is the run's.
//!
//! Like the rest of the library half, this module never mentions Bevy: the shapes it returns are
//! [`ChunkMesh`] and plain arrays, and the CI gate exercises it with `--no-default-features`.

use crate::mesh::ChunkMesh;
use crate::palette::{linear_rgba, PALETTE_LEN};
use crate::voxel::{CANOPY, GRASS, SHRUB, VINE};

/// Hours in a mean Julian year, the constant `ecosim/src/hydro.rs` names and `UNITS.md` derives the
/// tick's duration from. Copied, not shared: the two projects have only the files on disk.
pub const HOURS_PER_YEAR: f64 = 8766.0;
/// Days in the same year.
pub const DAYS_PER_YEAR: f32 = 365.25;
/// Ticks in a year when `meta.json` does not say. `ecosim`'s own shipped `climate.year_len`.
pub const DEFAULT_YEAR_LEN: u64 = 4000;

/// The latitude the sun is drawn at, in degrees north. **The viewer's own number.** No world bundle
/// and no run directory carries a latitude -- `bundle.json` has a `source` string naming the site
/// in prose and nothing machine-readable -- so this is a default, `--lat` overrides it, and the HUD
/// says it is the viewer's. 42.7 N is Lansing, Michigan, where the committed reference bundle's own
/// `source` line says its LiDAR was flown.
pub const DEFAULT_LATITUDE_DEG: f32 = 42.7;
/// Earth's axial tilt, which is what gives the year a sun path at all.
pub const AXIAL_TILT_DEG: f32 = 23.44;
/// The hour the viewer opens at: mid-morning, which throws shadows long enough to read a slope by
/// and short enough to leave the ground lit.
pub const DEFAULT_HOUR: f32 = 10.0;

/// The drawn sun's angular radius, in degrees. The real one is 0.27; at 1280 x 800 over a 45 degree
/// field that is under three pixels, so the disc is drawn an order of magnitude too big on purpose
/// and this constant is where that is admitted.
pub const SUN_DISC_DEG: f32 = 2.5;

/// The day the northern summer solstice falls on, and the tick fraction the simulator's own
/// temperature curve peaks at.
///
/// `ecosim/src/abiotic.rs` computes temperature as `base + amp * sin(2 pi tick / year_len)`, so the
/// warmest tick of its year is a quarter of the way through it. Lining that up with the solstice is
/// what makes the viewer's autumn the run's autumn; the alternative -- tick 0 is 1 January -- would
/// have put the simulator's hottest weather in April. It also fixes what tick 0 is: the spring
/// equinox, the tick the simulator's temperature crosses its mean going up.
pub const SOLSTICE_DAY: f32 = 172.0;
const TEMP_PEAK_FRACTION: f32 = 0.25;

/// How many steps the year's colour is quantised into.
///
/// Seasonal colour is baked into vertex colours, so the palette moving means every chunk is
/// remeshed. 64 steps is 5.7 days a step -- finer than one snapshot of the reference run (9.1 days)
/// -- so the quantising never costs a visible jump, and it stops a camera move or a HUD tick from
/// rebuilding the world (MEASUREMENTS.md, V6: what a season step costs).
pub const SEASON_STEPS: u32 = 64;

fn radians(deg: f32) -> f32 {
    deg * std::f32::consts::PI / 180.0
}

fn degrees(rad: f32) -> f32 {
    rad * 180.0 / std::f32::consts::PI
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let k = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
    ]
}

/// `#rrggbb` as a linear RGB triple, dropping the alpha [`linear_rgba`] carries.
fn rgb(hex: &str) -> [f32; 3] {
    let c = linear_rgba(hex);
    [c[0], c[1], c[2]]
}

/// A smooth 0-to-1 ramp between two edges, so nothing in the year changes colour on one tick.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < 1e-6 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Days from `centre` to `day`, the short way round a 365.25-day year.
fn day_gap(day: f32, centre: f32) -> f32 {
    let mut d = (day - centre) % DAYS_PER_YEAR;
    if d > DAYS_PER_YEAR / 2.0 {
        d -= DAYS_PER_YEAR;
    }
    if d < -DAYS_PER_YEAR / 2.0 {
        d += DAYS_PER_YEAR;
    }
    d
}

/// A bump peaking at `centre` and `width` days wide, for the two colour seasons.
fn bump(day: f32, centre: f32, width: f32) -> f32 {
    let d = day_gap(day, centre) / width.max(1e-3);
    (-d * d).exp()
}

/// Where a snapshot sits in the year, and what o'clock the viewer is drawing it at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    /// The snapshot's tick. `None` when no run is loaded.
    pub tick: Option<u64>,
    /// Ticks in a simulated year, from `meta.json`.
    pub year_len: u64,
    /// One tick in hours, `8766 / year_len`. Printed because it is the number that makes the day of
    /// the year a fact about the run rather than a guess.
    pub tick_hours: f32,
    /// Day of the year in `[0, 365.25)`, 0 being 1 January.
    pub day: f32,
    /// The viewer's hour of the day, in `[0, 24)`.
    pub hour: f32,
    /// Did the day come from a run, or is it this module's default?
    pub from_run: bool,
}

impl Clock {
    /// The clock for one snapshot. `tick` is `None` when there is no run, and the day then falls on
    /// the solstice -- a stress world has no year, and a picture of one should still be lit.
    pub fn of(tick: Option<u64>, year_len: u64, hour: f32) -> Clock {
        let year_len = if year_len == 0 {
            DEFAULT_YEAR_LEN
        } else {
            year_len
        };
        let day = match tick {
            Some(t) => {
                let f = (t % year_len) as f32 / year_len as f32;
                (SOLSTICE_DAY + (f - TEMP_PEAK_FRACTION) * DAYS_PER_YEAR).rem_euclid(DAYS_PER_YEAR)
            }
            None => SOLSTICE_DAY,
        };
        Clock {
            tick,
            year_len,
            tick_hours: (HOURS_PER_YEAR / year_len as f64) as f32,
            day,
            hour: hour.rem_euclid(24.0),
            from_run: tick.is_some(),
        }
    }

    /// How many simulated years into the run this tick is.
    pub fn years(&self) -> f32 {
        self.tick.map_or(0.0, |t| t as f32 / self.year_len as f32)
    }

    /// Calendar month and day, on a 365-day table. The year is 365.25 days long, so the last quarter
    /// day is dropped here rather than a leap rule being invented for a label.
    pub fn month_day(&self) -> (&'static str, u32) {
        const MONTHS: [(&str, u32); 12] = [
            ("January", 31),
            ("February", 28),
            ("March", 31),
            ("April", 30),
            ("May", 31),
            ("June", 30),
            ("July", 31),
            ("August", 31),
            ("September", 30),
            ("October", 31),
            ("November", 30),
            ("December", 31),
        ];
        let mut n = (self.day as u32).min(364);
        for (name, len) in MONTHS {
            if n < len {
                return (name, n + 1);
            }
            n -= len;
        }
        ("December", 31)
    }

    /// `14:30`, from the viewer's hour.
    pub fn hhmm(&self) -> String {
        let m = (self.hour * 60.0).round() as u32 % 1440;
        format!("{:02}:{:02}", m / 60, m % 60)
    }
}

/// Where the sun is, how bright it is and what colour, for one clock and one latitude.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sun {
    /// Degrees above the horizon; negative at night.
    pub elevation_deg: f32,
    /// Degrees clockwise from north, so 180 is due south.
    pub azimuth_deg: f32,
    /// Solar declination for the day, in degrees.
    pub declination_deg: f32,
    /// A unit vector **towards** the sun, in the viewer's frame: X east, Y up, Z north.
    pub dir: [f32; 3],
    /// Lux on a surface facing the sun. Zero once it is down.
    pub illuminance: f32,
    /// Linear RGB, warm near the horizon.
    pub color: [f32; 3],
}

impl Sun {
    /// The standard solar geometry: Cooper's declination and the hour-angle triangle, with the
    /// viewer's clock read as solar time.
    ///
    /// No equation of time and no longitude correction: both are worth up to about a quarter of an
    /// hour, which is under one step of the **L** key, and neither can be checked against anything
    /// the project owns. What this is for is a sun in the right part of the sky in the right part of
    /// the year, not an almanac.
    pub fn at(clock: &Clock, lat_deg: f32) -> Sun {
        let decl =
            radians(AXIAL_TILT_DEG) * radians(360.0 * (284.0 + clock.day) / DAYS_PER_YEAR).sin();
        let lat = radians(lat_deg.clamp(-89.5, 89.5));
        let h = radians(15.0 * (clock.hour - 12.0));
        // East, north and up of a unit vector towards the sun. At noon (h = 0) north of the equator
        // the north component is sin(declination - latitude), which is negative: due south.
        let east = -decl.cos() * h.sin();
        let north = decl.sin() * lat.cos() - decl.cos() * lat.sin() * h.cos();
        let up = decl.sin() * lat.sin() + decl.cos() * lat.cos() * h.cos();
        // Brightness follows the air mass the light came through, which is what makes a low sun both
        // dimmer and redder. The exponent is the viewer's, chosen so noon in June and an hour after
        // sunrise are both printable rather than one of them being white.
        let t = up.max(0.0).powf(0.55);
        Sun {
            elevation_deg: degrees(up.clamp(-1.0, 1.0).asin()),
            azimuth_deg: degrees(east.atan2(north)).rem_euclid(360.0),
            declination_deg: degrees(decl),
            dir: [east, up, north],
            illuminance: 11_000.0 * t,
            color: lerp3(rgb("#ff8134"), rgb("#fff4e2"), smoothstep(0.0, 0.35, up)),
        }
    }

    /// Is the sun above the horizon?
    pub fn is_up(&self) -> bool {
        self.elevation_deg > 0.0
    }
}

/// What the year is doing to the leaves.
///
/// Two continuous numbers rather than four named seasons, because a leaf does not change colour on
/// a date. `senescence` is the autumn bump and `dormancy` the winter plateau; `flush` is the pale
/// new growth of spring. The name is for the HUD only and nothing is computed from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Season {
    pub day: f32,
    pub name: &'static str,
    pub senescence: f32,
    pub dormancy: f32,
    pub flush: f32,
}

/// The four colours a plant is pulled towards through the year. **The viewer's**, applied to
/// whatever base hue `meta.json` gave the species, so a run that renames its colours still owns them.
const AUTUMN_HEX: &str = "#c4761b";
const DORMANT_HEX: &str = "#6d5a3e";
const STRAW_HEX: &str = "#9c9059";
const FLUSH_HEX: &str = "#a6d14e";

impl Season {
    pub fn of(day: f32) -> Season {
        let day = day.rem_euclid(DAYS_PER_YEAR);
        // Mid-October for the turn, a January plateau for dormancy, and mid-May for the flush. The
        // widths are days, and they are this module's only free constants.
        let senescence = bump(day, 288.0, 26.0);
        let dormancy = smoothstep(0.25, 0.85, bump(day, 15.0, 62.0));
        let flush = bump(day, 135.0, 22.0);
        let name = if dormancy > 0.45 {
            "winter"
        } else if senescence > 0.3 {
            "autumn"
        } else if flush > 0.3 {
            "spring"
        } else {
            "summer"
        };
        Season {
            day,
            name,
            senescence,
            dormancy,
            flush,
        }
    }

    /// Which of [`SEASON_STEPS`] the year is in. Change detection runs on this, not on the day.
    pub fn step(&self) -> u32 {
        ((self.day / DAYS_PER_YEAR) * SEASON_STEPS as f32) as u32 % SEASON_STEPS
    }

    /// One plant colour, moved through the year. Anything that is not a plant comes back unchanged:
    /// the ground, the paving, the buildings and every overlay band are the same colour in January
    /// as in July, because none of them is alive.
    ///
    /// **Colour only.** The trees keep every leaf in December: leaf fall is a change of geometry and
    /// it is the simulator's to make, not the viewer's -- row G10, "a deciduous year", is that shot.
    /// A bare winter tree drawn here would be a viewer inventing a plant behaviour.
    pub fn tint(&self, id: u16, base: [f32; 4]) -> [f32; 4] {
        let c = [base[0], base[1], base[2]];
        let out = match id {
            CANOPY | VINE => {
                let c = lerp3(c, rgb(FLUSH_HEX), 0.45 * self.flush);
                let c = lerp3(c, rgb(AUTUMN_HEX), 0.85 * self.senescence);
                lerp3(c, rgb(DORMANT_HEX), 0.7 * self.dormancy)
            }
            // Grass goes straw rather than brown, and it goes there for the whole cold half of the
            // year rather than turning and dropping: it is still standing in January.
            GRASS => {
                let c = lerp3(c, rgb(FLUSH_HEX), 0.3 * self.flush);
                lerp3(
                    c,
                    rgb(STRAW_HEX),
                    0.75 * self.dormancy + 0.25 * self.senescence,
                )
            }
            // A shrub holds its colour longer than a tree does; this keeps half the departure.
            SHRUB => {
                let c = lerp3(c, rgb(AUTUMN_HEX), 0.4 * self.senescence);
                lerp3(c, rgb(DORMANT_HEX), 0.45 * self.dormancy)
            }
            _ => return base,
        };
        [out[0], out[1], out[2], base[3]]
    }

    /// Every plant entry of a palette, tinted in place. Four ids of the forty-eight.
    pub fn tint_palette(&self, pal: &mut [[f32; 4]]) {
        for id in [CANOPY, VINE, SHRUB, GRASS] {
            if let Some(c) = pal.get_mut(id as usize) {
                *c = self.tint(id, *c);
            }
        }
    }
}

/// The whole drawn atmosphere for one moment: the clock, the sun, the season and the sky's colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyState {
    pub clock: Clock,
    pub sun: Sun,
    pub season: Season,
    pub latitude_deg: f32,
    /// Linear RGB at the top of the dome, at the horizon, and below it.
    pub zenith: [f32; 3],
    pub horizon: [f32; 3],
    pub ground_haze: [f32; 3],
    /// Ambient lux, and its colour. A night scene is lit by this alone, so it never reaches zero.
    pub ambient: f32,
    pub ambient_color: [f32; 3],
}

impl SkyState {
    pub fn of(clock: Clock, latitude_deg: f32) -> SkyState {
        let sun = Sun::at(&clock, latitude_deg);
        // Twilight is the elevation window from -6 degrees (civil dusk) to +8, which is where the
        // sky does all of its colour changing; `dusk` is the narrow band the horizon burns in.
        let day = smoothstep(-6.0, 8.0, sun.elevation_deg);
        let dusk = bump(sun.elevation_deg, 1.0, 7.0);
        let horizon = lerp3(
            lerp3(rgb("#182038"), rgb("#bcd4ee"), day),
            rgb("#e4885a"),
            0.75 * dusk,
        );
        SkyState {
            clock,
            sun,
            season: Season::of(clock.day),
            latitude_deg,
            zenith: lerp3(rgb("#0b1226"), rgb("#3874c8"), day),
            horizon,
            ground_haze: lerp3(rgb("#0a0d14"), rgb("#8f8674"), day),
            // 120 lux is a full moon's order of magnitude and is what keeps a night snapshot
            // readable; 740 at noon is the 700 V0 through V5 used at every hour of every day.
            ambient: 120.0 + 620.0 * day,
            ambient_color: lerp3(rgb("#2d3c58"), rgb("#cfe0f5"), day),
        }
    }

    /// What has to change before the world is remeshed: the season's step and nothing else. The sun
    /// and the sky's colours are the engine's lights, which cost nothing to move.
    pub fn mesh_key(&self) -> u32 {
        self.season.step()
    }

    /// The sky as geometry: a sphere of radius `r` around the camera, coloured zenith to horizon to
    /// ground haze, with a disc of sun in it, and every triangle facing inwards.
    ///
    /// Geometry rather than a shader because the rest of this crate is: the mesh is built by
    /// engine-free code, hashed in the same test target as the ground, and handed to the same
    /// vertex-coloured material the site uses.
    pub fn dome(&self, r: f32, rings: usize, segments: usize) -> ChunkMesh {
        let mut m = ChunkMesh::default();
        let rings = rings.max(2);
        let segments = segments.max(3);
        for ring in 0..=rings {
            // From the zenith down through the horizon to the nadir.
            let polar = ring as f32 / rings as f32 * std::f32::consts::PI;
            let (sp, cp) = (polar.sin(), polar.cos());
            let color = self.sky_color(cp);
            for seg in 0..=segments {
                let az = seg as f32 / segments as f32 * std::f32::consts::TAU;
                let p = [r * sp * az.sin(), r * cp, r * sp * az.cos()];
                // Inwards, towards the camera at the centre.
                let inv = -1.0 / r.max(1e-3);
                m.positions.push(p);
                m.normals.push([p[0] * inv, p[1] * inv, p[2] * inv]);
                m.colors.push([color[0], color[1], color[2], 1.0]);
            }
        }
        let stride = (segments + 1) as u32;
        for ring in 0..rings as u32 {
            for seg in 0..segments as u32 {
                let a = ring * stride + seg;
                // Wound so the front face points inwards, at the camera.
                m.indices.extend_from_slice(&[
                    a,
                    a + stride,
                    a + 1,
                    a + 1,
                    a + stride,
                    a + stride + 1,
                ]);
            }
        }
        self.push_sun_disc(&mut m, r * 0.97, segments.min(24));
        m
    }

    /// The sky's colour at a height up the dome, `cos(polar)` running 1 at the zenith to -1 below.
    fn sky_color(&self, cos_polar: f32) -> [f32; 3] {
        if cos_polar >= 0.0 {
            lerp3(self.horizon, self.zenith, smoothstep(0.0, 0.55, cos_polar))
        } else {
            lerp3(
                self.horizon,
                self.ground_haze,
                smoothstep(0.0, 0.3, -cos_polar),
            )
        }
    }

    /// A flat disc of sun on the inside of the dome. Drawn a little inside it so it never z-fights
    /// with the sky behind it, and skipped once the sun is down.
    fn push_sun_disc(&self, m: &mut ChunkMesh, r: f32, segments: usize) {
        if !self.sun.is_up() {
            return;
        }
        let d = self.sun.dir;
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let norm = |v: [f32; 3]| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
            [v[0] / l, v[1] / l, v[2] / l]
        };
        // Any two directions perpendicular to the sun, for the disc's own plane. Near the zenith
        // "up" is no longer a usable reference, so north becomes one.
        let up = if d[1].abs() > 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let ex = norm(cross(up, d));
        let ey = norm(cross(d, ex));
        let rad = r * radians(SUN_DISC_DEG).tan();
        let centre = [d[0] * r, d[1] * r, d[2] * r];
        let inward = [-d[0], -d[1], -d[2]];
        // A brighter core than the sun's own light colour, so the disc reads as a source.
        let c = self.sun.color;
        let core = [
            (c[0] * 2.5).min(1.0),
            (c[1] * 2.5).min(1.0),
            (c[2] * 2.2).min(1.0),
            1.0,
        ];
        let base = m.positions.len() as u32;
        m.positions.push(centre);
        m.normals.push(inward);
        m.colors.push(core);
        for seg in 0..=segments {
            let a = seg as f32 / segments as f32 * std::f32::consts::TAU;
            let (s, co) = (a.sin(), a.cos());
            m.positions.push([
                centre[0] + rad * (ex[0] * co + ey[0] * s),
                centre[1] + rad * (ex[1] * co + ey[1] * s),
                centre[2] + rad * (ex[2] * co + ey[2] * s),
            ]);
            m.normals.push(inward);
            m.colors.push(core);
        }
        for seg in 0..segments as u32 {
            m.indices
                .extend_from_slice(&[base, base + 1 + seg, base + 2 + seg]);
        }
    }

    /// The one line the HUD and every scripted run print, naming whose each half is.
    pub fn line(&self) -> String {
        let (month, day) = self.clock.month_day();
        let whose = if self.clock.from_run {
            "the date is the run's"
        } else {
            "no run loaded, so the date is the viewer's too"
        };
        format!(
            "sun {} {:.0} deg up, {:.0} deg from north ({}) -- {day} {month}, {} -- {:.1} N, the hour, the latitude and the hue are the viewer's, {whose}",
            self.clock.hhmm(),
            self.sun.elevation_deg,
            self.sun.azimuth_deg,
            if self.sun.is_up() { "up" } else { "down" },
            self.season.name,
            self.latitude_deg,
        )
    }
}

/// How many brightness steps ambient occlusion is drawn in, and what each one keeps of a colour.
///
/// Four, because the occlusion it measures has nine outcomes (none of the eight neighbours above a
/// voxel, through all eight) and because every extra step costs greedy meshing a merge: a palette
/// entry is an id, so two voxels of the same material at different occlusion no longer merge into
/// one quad. MEASUREMENTS.md, V6 has what that is worth on the Capitol.
pub const AO_LEVELS: usize = 4;
/// What each level keeps of its colour. Level 0 is open sky and is exactly 1.0, which is what makes
/// turning ambient occlusion off give back the V5 mesh to the byte.
pub const AO_SHADE: [f32; AO_LEVELS] = [1.0, 0.80, 0.63, 0.48];

/// Expands a palette into one block per occlusion level, each dimmer than the last.
///
/// Block 0 is the palette unchanged, so an id that carries no occlusion level -- everything the
/// viewer drew before this shot -- means exactly what it always did.
pub fn shaded_palette(base: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut out = Vec::with_capacity(PALETTE_LEN * AO_LEVELS);
    for s in AO_SHADE {
        for i in 0..PALETTE_LEN {
            let c = base.get(i).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]);
            out.push([c[0] * s, c[1] * s, c[2] * s, c[3]]);
        }
    }
    out
}
