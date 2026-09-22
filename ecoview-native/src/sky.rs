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
//! | the day **held**, over a tick that does not move | `--day`/`--date` and the **;** and **'** keys, **\\** hands it back | the viewer's, and the HUD says so on every frame |
//! | hour of the day | the viewer's clock, `--hour` and the **K** and **L** keys | the viewer's |
//! | latitude | `--lat`, default [`DEFAULT_LATITUDE_DEG`] | the viewer's -- no bundle or run carries one |
//! | sun elevation and azimuth | the standard solar geometry below, from those three | derived |
//! | leaf and grass colour | the base hue is `meta.json`'s species colour; the seasonal departure from it is this module's | both, and the HUD says which is which |
//! | the eye's exposure | how much of the ground is under a crown, against [`SHADE_FLOOR`]; `--exposure` and the **-**, **=** and **0** keys override it | the viewer's, and the HUD says so |
//!
//! **The hour is deliberately not taken from the tick**, although it could be: a tick is
//! `8766 / year_len` hours (`ecosim/UNITS.md`), so tick 100 really is 219.15 hours into the run and
//! really is 03:09 in the morning. Half of every run's snapshots would then be photographed in the
//! dark, for a diurnal cycle the simulator does not model. The day of the year is a different case
//! -- the simulator's temperature and rain both swing on it -- so that half is the run's.
//!
//! **The day can be held, though, and that is shot V8.** Until this shot the only way to change the
//! season was to load a different tick, and a different tick is a different world: V6's own evidence
//! is `v6-summer.png` at tick 9000 with 3,867 trees and `v6-winter.png` at tick 11000 with 1,453,
//! two frames that differ mostly by 2,414 missing trees rather than by colour. [`Clock::with_day`]
//! overrides the day on a tick that does not move, so one snapshot can be turned through the year
//! with the same wood standing in every frame. **It changes the colour and the sun, and nothing
//! else**: the trees, the water, the burn scars and every overlay band on the frame are still that
//! tick's, because they are the run's. Whenever the override is on, the clock says so
//! ([`DaySource`]), the HUD says so, and `ecoview.stats` says so.
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

/// The calendar the day of the year is named on: a 365-day table, the last quarter day of the
/// 365.25-day year dropped rather than a leap rule invented for a label.
pub const MONTHS: [(&str, u32); 12] = [
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

/// Whose the day of the year on a frame is. Three cases, because two of them are not the run's and
/// a picture that does not distinguish them is a picture making a claim the run never made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaySource {
    /// The snapshot's tick over the run's `year_len`. The ordinary case, and the honest one.
    Run,
    /// `--day`, `--date` or the **;** and **'** keys: the viewer holding the tick still and turning
    /// the year over it (shot V8).
    Override,
    /// No run is loaded, so there is no year to be anywhere in; the day is [`SOLSTICE_DAY`].
    Default,
}

impl DaySource {
    /// The clause the HUD and every scripted run print. Short, because it is on every frame.
    pub fn line(&self) -> &'static str {
        match self {
            DaySource::Run => "the date is the run's",
            DaySource::Override => "the date is held by the viewer, the tick has not moved",
            DaySource::Default => "no run loaded, so the date is the viewer's too",
        }
    }

    /// The word `ecoview.stats` reports.
    pub fn name(&self) -> &'static str {
        match self {
            DaySource::Run => "run",
            DaySource::Override => "override",
            DaySource::Default => "default",
        }
    }
}

/// A day of the year from a `--day` or `--date` argument, 0-based on the [`MONTHS`] table so it can
/// go straight into [`Clock::with_day`].
///
/// Three spellings, all of them unambiguous: `172` is an ordinary 1-based day of the year, and
/// `6-22` and `6/22` are a month and a day of that month. There is no year field, because the clock
/// has no year -- a run is at a tick, and which calendar year that is is not a thing the project
/// knows. `None` is a string that does not parse, and the caller makes that fatal.
pub fn parse_date(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some((m, d)) = s
        .split_once('-')
        .or_else(|| s.split_once('/'))
        .filter(|(m, _)| !m.is_empty())
    {
        return day_of_year(m.trim().parse().ok()?, d.trim().parse().ok()?);
    }
    let n: f32 = s.parse().ok()?;
    // 1-based in, 0-based out: day 1 is 1 January, which is day 0 on the table. The bound is the
    // [`MONTHS`] table's 365 days rather than the year's 365.25, because what comes back out of
    // this is a date with a name on it.
    (1.0..366.0).contains(&n).then_some(n - 1.0)
}

/// The 0-based day of the year a calendar date falls on, the inverse of [`Clock::month_day`].
/// `None` for a month outside 1-12 or a day outside that month.
pub fn day_of_year(month: u32, day: u32) -> Option<f32> {
    if !(1..=12).contains(&month) || day < 1 || day > MONTHS[month as usize - 1].1 {
        return None;
    }
    let before: u32 = MONTHS[..month as usize - 1].iter().map(|m| m.1).sum();
    Some((before + day - 1) as f32)
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
    /// Whose the `day` above is. Not a bool since shot V8: a held day is neither the run's nor a
    /// default, and the frame has to be able to say which of the three it is.
    pub day_source: DaySource,
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
            day_source: if tick.is_some() {
                DaySource::Run
            } else {
                DaySource::Default
            },
        }
    }

    /// The same clock with the day of the year **held** at `day`, and `None` giving back the clock
    /// unchanged (shot V8).
    ///
    /// An override on a value that already exists, which is all this is: the tick, the `year_len`,
    /// the hours a tick is worth and [`Clock::years`] are untouched and still the run's, and the
    /// only things downstream of the day are the sun's declination and the season's colour. That is
    /// the point -- it is what lets one snapshot be photographed in four seasons with the same
    /// 3,867 trees standing in all four frames, which V6 could not do.
    pub fn with_day(self, day: Option<f32>) -> Clock {
        match day {
            None => self,
            Some(d) => Clock {
                day: d.rem_euclid(DAYS_PER_YEAR),
                day_source: DaySource::Override,
                ..self
            },
        }
    }

    /// Is the day on this frame the run's own? False while it is held, and false with no run.
    pub fn from_run(&self) -> bool {
        self.day_source == DaySource::Run
    }

    /// How many simulated years into the run this tick is.
    pub fn years(&self) -> f32 {
        self.tick.map_or(0.0, |t| t as f32 / self.year_len as f32)
    }

    /// Calendar month and day, on the [`MONTHS`] table.
    pub fn month_day(&self) -> (&'static str, u32) {
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

    /// The eye's adaptation, from how much of the ground is under a crown (shot V7).
    ///
    /// **This is the one thing in this module that changes what a surface receives**, and it is
    /// still expression: it moves an engine light, not a field, and no run has an ambient level in
    /// it to contradict. The reason it exists is that V6 turned shadow maps on, and from that shot
    /// on a shadowed surface got `ambient / (ambient + sun)` of a lit one -- about a thirteenth at
    /// the default hour -- where V5 gave it everything. That ratio is right for the sun and wrong
    /// for the eye, which opens up when it walks under a canopy. Nothing was offered to close the
    /// gap but **O**, which takes the whole beauty pass away.
    ///
    /// So: pick the ambient level that would put a shadowed surface at [`SHADE_FLOOR`] of a lit
    /// one, and take `closure` of the way there. An open site moves by nothing at all, which is
    /// what keeps every frame V6 through V8 took where it was; a site whose ground is entirely
    /// under leaves gets the full two stops. The sun's own level is never touched, so lit ground
    /// stays where the sun put it apart from the ambient it also receives.
    ///
    /// `manual` overrides the measurement outright with a number of stops, because a measurement
    /// good enough to default to is not a measurement anybody should be stuck with.
    pub fn adapt(&self, closure: f32, manual: Option<f32>) -> Adaptation {
        // Lux on horizontal ground: the sun's own figure is for a surface facing it.
        let lit = self.sun.illuminance * self.sun.dir[1].max(0.0);
        let was = self.ambient;
        let ambient = match manual {
            Some(stops) => was * 2f32.powf(stops.clamp(-EXPOSURE_LIMIT, EXPOSURE_LIMIT)),
            None => {
                // `a / (a + lit) = floor` solved for a. With the sun down this is 0 and the max
                // below keeps the night exactly as dark as V6 drew it.
                let need = lit * SHADE_FLOOR / (1.0 - SHADE_FLOOR);
                was + closure.clamp(0.0, 1.0) * (need - was).max(0.0)
            }
        };
        let ratio = |a: f32| if a + lit > 0.0 { a / (a + lit) } else { 1.0 };
        Adaptation {
            closure,
            lit,
            was,
            ambient,
            stops: if was > 0.0 {
                (ambient / was).log2()
            } else {
                0.0
            },
            ratio_was: ratio(was),
            ratio: ratio(ambient),
            manual: manual.is_some(),
        }
    }

    /// The one line the HUD and every scripted run print, naming whose each half is.
    pub fn line(&self) -> String {
        let (month, day) = self.clock.month_day();
        let whose = self.clock.day_source.line();
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

/// How dark the shadows are allowed to get, as a fraction of lit ground, under a closed canopy.
///
/// **The eye's number, not the sun's** (shot V7). It is a target on the illuminance ratio between
/// a shadowed horizontal surface and a lit one:
///
/// | | shade : lit | stops |
/// |---|---|---|
/// | V0 through V5: no shadow maps at all | 1 : 1 | 0 |
/// | V6 as shipped, June at 10:00 | 1 : 13 | 3.7 |
/// | this, under a closed canopy | 1 : 4 | 2 |
///
/// V6's figure is the physically correct one -- 10,000 lux of sun against 740 of sky is roughly
/// what a clear morning does -- and it is the one a site with an open canopy keeps, because the
/// correction is scaled by how much of the ground is actually under a crown. What V6 left out is
/// that an eye under that canopy opens up, and a camera with a fixed exposure does not. Two stops
/// is the range a print holds and is between the two numbers above rather than a return to either.
pub const SHADE_FLOOR: f32 = 0.25;

/// How far [`Adaptation`] will be pushed by hand, in stops either way.
pub const EXPOSURE_LIMIT: f32 = 4.0;

/// What the eye's adaptation did to one frame: the measurement, the lift and the contrast either
/// side of it (shot V7).
///
/// Every field is lux on a **horizontal** surface, which is what ground is, and the whole thing is
/// derived: nothing here is stored between frames and nothing feeds back into a run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Adaptation {
    /// The fraction of the site's ground under a crown ([`crate::voxel::Canopy`]).
    pub closure: f32,
    /// Lux the sun puts on lit ground. Zero once it is down, which is what keeps a dusk or a night
    /// frame exactly where V6 drew it: there is no sun to hide in, so there is nothing to adapt to.
    pub lit: f32,
    /// Ambient lux before and after.
    pub was: f32,
    pub ambient: f32,
    /// The lift, in stops. Zero on an open site.
    pub stops: f32,
    /// Shadowed ground as a fraction of lit ground, before and after.
    pub ratio_was: f32,
    pub ratio: f32,
    /// Was the number set by hand (`--exposure`, the **-** and **=** keys) rather than measured?
    pub manual: bool,
}

impl Adaptation {
    /// The one line the HUD and every scripted run print. `1/14 of lit` rather than `0.069` because
    /// the number a reader can check against the picture is the ratio, not the fraction.
    pub fn line(&self) -> String {
        let one_in = |r: f32| {
            if r > 0.0 {
                format!("1/{:.0}", 1.0 / r)
            } else {
                "none".to_string()
            }
        };
        format!(
            "eye adaptation {} -- canopy closure {:.0}%, ambient {:.0} -> {:.0} lux ({:+.2} stops), \
             shadow {} -> {} of lit ground",
            if self.manual { "by hand" } else { "measured" },
            100.0 * self.closure,
            self.was,
            self.ambient,
            self.stops,
            one_in(self.ratio_was),
            one_in(self.ratio),
        )
    }
}

/// Reads `--exposure`: `auto`, or a number of stops.
pub fn parse_exposure(s: &str) -> Option<Option<f32>> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("auto") {
        return Some(None);
    }
    t.parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| Some(v.clamp(-EXPOSURE_LIMIT, EXPOSURE_LIMIT)))
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
