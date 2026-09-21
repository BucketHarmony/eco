//! The unit-system tests (shot G4b): one tick duration in the code, and cadences that are schedules
//! rather than rates.
//!
//! `UNITS.md` is the audit these two tests hold in place. Everything in `params.toml` that is a rate
//! is per hour or per year, and a staggered update turns it into an amount by multiplying by the
//! update's own length, so the cadence is free to change. These tests are what fails if a later shot
//! writes a per-tick rate again.

use ecosim::{Params, Sim};
use std::fs;
use std::path::{Path, PathBuf};

/// Every `.rs` file under `src/`, as (path, text).
fn sources() -> Vec<(PathBuf, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out: Vec<(PathBuf, String)> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .map(|p| (p.clone(), fs::read_to_string(&p).unwrap()))
        .collect();
    out.sort();
    assert!(out.len() > 10, "found {} source files", out.len());
    out
}

/// Acceptance: the tick duration constant appears once in the code, and every rate that has to know
/// how long a tick is derives it from that constant.
///
/// Three things are asserted. The hours in a year are written down once, as
/// `hydro::HOURS_PER_YEAR`. The tick's length and the length of a span of ticks come only from it,
/// through `hydro::tick_hours` and `hydro::years`, which are the only functions that divide by
/// `climate.year_len`. And each subsystem with a staggered update charges its rate over the update's
/// own length through one of those two functions, so no module holds a per-tick rate of its own.
#[test]
fn the_tick_duration_constant_appears_once_and_every_rate_derives_from_it() {
    let src = sources();
    let hits: Vec<(&Path, usize)> =
        src.iter().map(|(p, t)| (p.as_path(), t.matches("8766").count())).filter(|&(_, n)| n > 0).collect();
    assert_eq!(
        hits.iter().map(|&(_, n)| n).sum::<usize>(),
        1,
        "the hours in a year are written down once, not in {hits:?}"
    );
    let (path, text) = src.iter().find(|(_, t)| t.contains("8766")).unwrap();
    assert!(path.ends_with("hydro.rs"), "the constant lives in hydro.rs, not {}", path.display());
    assert!(text.contains("pub const HOURS_PER_YEAR: f64 = 8766.0;"), "it is a named constant");
    assert!(
        text.contains("HOURS_PER_YEAR * years(p, 1)"),
        "a tick's length in hours is derived from it and from the year length"
    );

    // The only division by year_len is `hydro::years`; every other module asks it.
    for (p, t) in &src {
        let divides = t.contains("/ p.climate.year_len") || t.contains("/ self.params.climate.year_len");
        assert!(
            !divides || p.ends_with("hydro.rs"),
            "{} derives a duration from year_len itself; call hydro::years",
            p.display()
        );
    }

    // Every staggered update charges its rate over its own length.
    let charged: [(&str, &str); 5] = [
        ("producers.rs", "years(&self.params, self.params.schedule.cover_every.max(1))"),
        ("abiotic.rs", "self.params.schedule.soil_every.max(1) as f64 * crate::hydro::tick_hours"),
        ("fire.rs", "crate::hydro::years(&self.params, every)"),
        ("trees.rs", "tp.update_every as f64 * crate::hydro::tick_hours(&self.params)"),
        ("hydro.rs", "self.params.climate.year_len.max(1) as f64 * rp.storm_mean_mm as f64"),
    ];
    for (file, expr) in charged {
        let (_, text) = src.iter().find(|(p, _)| p.ends_with(file)).unwrap();
        assert!(text.contains(expr), "{file} no longer charges its rate over the update's length: {expr}");
    }
}

/// Params for the doubling test: a small square world, the shipped year length, and nothing
/// stochastic left in the run, so the only differences between two cadences are the cadences. The
/// year length is not shortened to make the test quick: `schedule.temperature_every` samples a sine,
/// and at 400 ticks a year a 100-tick interval samples it four times a year, so doubling it aliases
/// the seasons away instead of halving a rate.
fn doubling_params(overrides: &[&str]) -> Params {
    let base = [
        "world.width=32",
        "world.depth=32",
        "world.slope_bias=0",
        "climate.rain_gradient=0",
        "animals.enabled=false",
        "fire.base_rate=0",
        "tree.initial_count=0",
        "tree.immigration_floor=0",
    ];
    let set: Vec<String> = base.iter().chain(overrides).map(|s| s.to_string()).collect();
    Params::load_with(&Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml"), &set).unwrap()
}

/// What one simulated year of a subsystem adds up to, for each quantity a declared rate is
/// responsible for: the producers' standing cover and the litter they shed, the rain that fell, the
/// water that evaporated and transpired, and the mean temperature they all saw.
///
/// Grass and shrub are one total: over two years from a bare start the shrub is a decaying transient
/// three orders of magnitude below the grass, and a relative tolerance on a quantity that small
/// measures the discretisation of an exponential decay rather than the year's growth.
///
/// Drainage, runoff, ponded evaporation and the standing stores are deliberately **not** here, and
/// they are the reason this list is explicit rather than "everything in the row". None of them is
/// charged by a rate. What drains is the residual of a store with a ceiling (`hydro.saturation` of
/// field capacity), so how often the store is emptied decides how much of a storm it has room to
/// take; and a pond is almost always shallower than one update's evaporation allowance, so what
/// evaporates off the surface measures how much was standing rather than how fast it leaves.
/// Measured on this world at the shipped defaults, doubling `schedule.soil_every` leaves the year's
/// rain untouched and its evapotranspiration within 0.3%, and moves annual drainage by 1.0% — but it
/// moves runoff by 14% and outflow over the world's edge by 11%, because a store emptied half as
/// often has less room for the storm that arrives next. That is the water tier's integration error:
/// a property of the model, not evidence about its units, and it is measured in
/// `sweeps/shotG4b/FINDINGS.md` instead of asserted here.
#[derive(Debug, Clone, Copy)]
struct YearTotals {
    cover: f64,
    detritus: f64,
    rain: f64,
    et: f64,
    temperature: f64,
}

impl YearTotals {
    /// The five totals as (name, value) pairs.
    fn parts(&self) -> [(&'static str, f64); 5] {
        [
            ("cover", self.cover),
            ("detritus", self.detritus),
            ("rain", self.rain),
            ("et", self.et),
            ("temperature", self.temperature),
        ]
    }
}

/// Run two years and report the second year's totals: sums over the year for what flows (the water
/// ones from the run-long ledger, differenced across the year) and the year-end value for what stands.
fn second_year(params: Params) -> YearTotals {
    let year = params.climate.year_len;
    let mut sim = Sim::new(params, 7);
    let et = |s: &Sim| s.hydro.as_ref().expect("the water tier is on").ledger.et;
    let (mut rain, mut temperature) = (0.0, 0.0);
    let mut first = et(&sim);
    for t in 1..=2 * year {
        sim.step();
        if t == year {
            first = et(&sim);
        }
        if t > year {
            rain += sim.stats().water.rain_mm as f64;
            temperature += sim.stats().temperature as f64 / year as f64;
        }
    }
    let last = et(&sim);
    let s = sim.stats();
    YearTotals {
        cover: (s.grass_mean + s.shrub_mean) as f64,
        detritus: s.detritus_total as f64,
        rain,
        et: last - first,
        temperature,
    }
}

/// How far two cadences may leave a year's totals apart: 5%. A rate charged over a longer update is
/// still charged the same amount per year, but it lands in fewer, larger steps, so a logistic
/// increment is evaluated at a slightly different cover and a rate-limited flow (infiltration out of
/// a pond, percolation out of a column) can be limited at a different moment. The error is of the
/// order of one update's share of the year, which at these cadences and rates is about 1%; 5% leaves
/// room for that to compound over a year without admitting a rate that is still per tick, which
/// would be off by a factor of two (DECISIONS.md, "Units calibration").
const DOUBLING_TOLERANCE: f64 = 0.05;

/// Acceptance: doubling a subsystem's staggered update interval changes that subsystem's totals over
/// a year by less than [`DOUBLING_TOLERANCE`].
///
/// `schedule.fire_every` is not driven here. Its per-update outcome is a Bernoulli draw whose
/// probability the cadence divides — `fire::ignition_prob` is linear in the update's length in
/// years, which `fire::ignition_regression_ramp_ends_and_clamp` asserts directly — and a realised
/// ignition count over one simulated year has a Poisson spread far wider than any tolerance worth
/// stating. `tree.update_every` is not driven either, and shot G4c did not make it reachable: a tree
/// ages by one whole cadence per update, so the tier's totals are already invariant under doubling
/// by construction rather than by calibration, and there is nothing for this test to catch. What the
/// tier is not invariant to is its own time scale against the water tier's, which is a finding
/// (UNITS.md section 7) and not a cadence.
#[test]
fn doubling_an_update_interval_leaves_a_years_totals_within_5_percent() {
    let base = second_year(doubling_params(&[]));
    // The same year in physical units, which is the calibration the cadences are charged against:
    // 800 mm of rain a year at the site (Lansing normal; a single simulated year lands anywhere in
    // the station's 600-1100 mm spread), and the evapotranspiration of a partly covered temperate
    // site, which sits under the 400-600 mm published range for full grass cover because this world
    // is not fully covered and because open-water evaporation is counted separately (UNITS.md R1, R5).
    let cols = 32.0 * 32.0;
    assert!((600.0..1100.0).contains(&base.rain), "{} mm of rain in the year", base.rain);
    assert!((250.0..650.0).contains(&(base.et / cols)), "{} mm of ET in the year", base.et / cols);
    for (key, doubled) in
        [("schedule.cover_every", "20"), ("schedule.soil_every", "20"), ("schedule.temperature_every", "200")]
    {
        let set = format!("{key}={doubled}");
        let got = second_year(doubling_params(&[&set]));
        for ((name, a), (_, b)) in base.parts().into_iter().zip(got.parts()) {
            let scale = a.abs().max(b.abs()).max(1e-6);
            let rel = (a - b).abs() / scale;
            assert!(rel < DOUBLING_TOLERANCE, "{key} doubled moved {name} by {:.2}%: {a} -> {b}", 100.0 * rel);
        }
    }
}
