//! The event log, `events.csv` (format_version 3): one row per birth, death, fire change,
//! germination and immigration, in the order they happen.
//!
//! Columns: `tick,kind,species,patch_x,patch_y,x,y,cause,detail`. Empty fields are left empty.
//!
//! | kind | species | x, y | cause | detail |
//! |---|---|---|---|---|
//! | `death` | grazer, hunter | column | `Cause` name | animal id |
//! | `birth` | grazer, hunter | column | | newborn id |
//! | `ignition` | | | | |
//! | `spread` | | | | source patch index (`patch_x + 8·patch_y`) |
//! | `burnout` | | | | |
//! | `germination` | tree | column | | tree id |
//! | `tree_death` | tree | column | `old_age`, `drought`, `crowded`, `burnt` | tree id |
//! | `immigration` | grazer, hunter, tree | column | | immigrant id |
//! | `seed_drop` | reserved: never written yet | | | |
//!
//! Recording is off unless `Sim::log_events` is set, and it never draws from the RNG or writes any
//! other state, so a run is byte-identical with or without it apart from `events.csv`.

use crate::animals::{Cause, Kind};
use crate::sim::{Deaths, Sim};
use crate::world::{patch_of, PATCHES_X};
use std::fmt::Write as _;

/// The file name inside a run directory.
pub const EVENTS_FILE: &str = "events.csv";

/// The first line of `events.csv`.
pub const EVENTS_HEADER: &str = "tick,kind,species,patch_x,patch_y,x,y,cause,detail";

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// A grazer or hunter died; `cause` says why.
    Death,
    /// A grazer or hunter was born.
    Birth,
    /// A patch caught fire on an ignition update.
    Ignition,
    /// A patch caught fire from a burning neighbour.
    Spread,
    /// A patch finished burning.
    Burnout,
    /// A seed took root as a sapling.
    Germination,
    /// A tree died; `cause` says why.
    TreeDeath,
    /// An animal or tree arrived at the world's edge (open boundaries).
    Immigration,
    /// Reserved for animal seed dispersal; never written yet.
    SeedDrop,
}

impl EventKind {
    /// Every kind, in declaration order.
    pub const ALL: [EventKind; 9] = [
        EventKind::Death,
        EventKind::Birth,
        EventKind::Ignition,
        EventKind::Spread,
        EventKind::Burnout,
        EventKind::Germination,
        EventKind::TreeDeath,
        EventKind::Immigration,
        EventKind::SeedDrop,
    ];

    /// The name written in the `kind` column.
    pub fn name(self) -> &'static str {
        ["death", "birth", "ignition", "spread", "burnout", "germination", "tree_death", "immigration", "seed_drop"]
            [self as usize]
    }
}

/// Why a tree died (`tree_death` rows).
pub const TREE_CAUSES: [&str; 4] = ["old_age", "drought", "crowded", "burnt"];

/// Species names in the `species` column; the empty string for patch events.
pub const SPECIES: [&str; 4] = ["grazer", "hunter", "tree", ""];

/// One row of `events.csv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// Tick during which it happened.
    pub tick: u32,
    /// What happened.
    pub kind: EventKind,
    /// One of `SPECIES`.
    pub species: &'static str,
    /// Patch index (`patch_x + 8·patch_y`).
    pub patch: u8,
    /// The column, for events that have one.
    pub col: Option<(u8, u8)>,
    /// A `Cause` name or one of `TREE_CAUSES`; empty for kinds without a cause.
    pub cause: &'static str,
    /// The entity id, or the source patch of a spread (see the module table).
    pub detail: Option<u32>,
}

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or(String::new(), |v| v.to_string())
}

fn lookup(names: &[&'static str], s: &str, what: &str) -> Result<&'static str, String> {
    names.iter().copied().find(|n| *n == s).ok_or_else(|| format!("unknown {what} '{s}'"))
}

impl Event {
    /// Append this event as one `events.csv` line, newline included.
    pub fn write_line(&self, out: &mut String) {
        let p = self.patch as usize;
        let (x, y) = (opt(self.col.map(|c| c.0)), opt(self.col.map(|c| c.1)));
        let _ = writeln!(
            out,
            "{},{},{},{},{},{x},{y},{},{}",
            self.tick,
            self.kind.name(),
            self.species,
            p % PATCHES_X,
            p / PATCHES_X,
            self.cause,
            opt(self.detail)
        );
    }

    /// Parse one `events.csv` data line.
    pub fn parse(line: &str) -> Result<Event, String> {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() != 9 {
            return Err(format!("events.csv: expected 9 fields, got {}: '{line}'", f.len()));
        }
        let num = |s: &str| s.parse::<u32>().map_err(|_| format!("events.csv: '{s}' is not a number in '{line}'"));
        let small = |s: &str| num(s).and_then(|v| u8::try_from(v).map_err(|_| format!("events.csv: {v} out of range")));
        let kind =
            *EventKind::ALL.iter().find(|k| k.name() == f[1]).ok_or_else(|| format!("unknown kind '{}'", f[1]))?;
        let causes: Vec<&'static str> = Cause::ALL.iter().map(|c| c.name()).chain(TREE_CAUSES).chain([""]).collect();
        let (px, py) = (small(f[3])?, small(f[4])?);
        if px as usize >= PATCHES_X || py as usize >= PATCHES_X {
            return Err(format!("events.csv: patch ({px}, {py}) is off the world"));
        }
        let col = match (f[5], f[6]) {
            ("", "") => None,
            (x, y) => Some((small(x)?, small(y)?)),
        };
        Ok(Event {
            tick: num(f[0])?,
            kind,
            species: lookup(&SPECIES, f[2], "species")?,
            patch: px + PATCHES_X as u8 * py,
            col,
            cause: lookup(&causes, f[7], "cause")?,
            detail: if f[8].is_empty() { None } else { Some(num(f[8])?) },
        })
    }
}

/// Parse a whole `events.csv`, header included.
pub fn parse_events(text: &str) -> Result<Vec<Event>, String> {
    let mut lines = text.lines();
    if lines.next() != Some(EVENTS_HEADER) {
        return Err("events.csv: header is not this ecosim's".into());
    }
    lines.map(Event::parse).collect()
}

/// Deaths per tick by species and cause, counted from `death` rows, for ticks `0..ticks`.
pub fn deaths_per_tick(events: &[Event], ticks: usize) -> Result<Vec<Deaths>, String> {
    let mut out = vec![Deaths::default(); ticks];
    for e in events.iter().filter(|e| e.kind == EventKind::Death) {
        let k = match e.species {
            "grazer" => Kind::Grazer,
            "hunter" => Kind::Hunter,
            s => return Err(format!("events.csv: death of '{s}' at tick {}", e.tick)),
        };
        let c = Cause::ALL.iter().find(|c| c.name() == e.cause).ok_or_else(|| format!("death cause '{}'", e.cause))?;
        let row =
            out.get_mut(e.tick as usize).ok_or_else(|| format!("events.csv: tick {} is past the series", e.tick))?;
        row[k as usize][*c as usize] += 1;
    }
    Ok(out)
}

/// The first `burnout` without its own earlier `ignition` or `spread` of that patch, if any. A patch
/// can't be lit while it burns, so each burnout uses up the lighting before it.
pub fn unlit_burnout(events: &[Event]) -> Option<&Event> {
    let mut lit = [false; crate::world::PATCHES];
    for e in events {
        let p = e.patch as usize;
        match e.kind {
            EventKind::Ignition | EventKind::Spread => lit[p] = true,
            EventKind::Burnout if !lit[p] => return Some(e),
            EventKind::Burnout => lit[p] = false,
            _ => {}
        }
    }
    None
}

impl Sim {
    /// Record an event of this tick, when logging is on.
    pub(crate) fn log(&mut self, kind: EventKind, species: &'static str, patch: usize, col: Option<(usize, usize)>) {
        self.log_with(kind, species, patch, col, "", None);
    }

    /// `log` with a cause and a detail.
    pub(crate) fn log_with(
        &mut self,
        kind: EventKind,
        species: &'static str,
        patch: usize,
        col: Option<(usize, usize)>,
        cause: &'static str,
        detail: Option<u32>,
    ) {
        if self.log_events {
            let col = col.map(|(x, y)| (x as u8, y as u8));
            self.events.push(Event { tick: self.tick, kind, species, patch: patch as u8, col, cause, detail });
        }
    }

    /// Record an entity event on column (x, y), with its patch taken from the column.
    pub(crate) fn log_at(
        &mut self,
        kind: EventKind,
        species: &'static str,
        (x, y): (usize, usize),
        cause: &'static str,
        id: u32,
    ) {
        self.log_with(kind, species, patch_of(x, y), Some((x, y)), cause, Some(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{read_series, read_series_for_stats};
    use crate::output::run;
    use crate::params::Params;
    use proptest::prelude::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn scratch_dir() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("ecosim-events-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// Knobs that make every death cause, fire and immigration happen within a few hundred ticks.
    #[derive(Debug, Clone)]
    struct Busy {
        fire_rate: f32,
        spread: f32,
        grazer_cost: f32,
        grazer_max_age: u32,
        hunter_cost: f32,
        hunter_max_age: u32,
        kill_prob: f64,
        crowding: f32,
        floor: u32,
    }

    fn busy_params(b: &Busy) -> Params {
        let mut p = Params::load_default();
        p.fire.base_rate = b.fire_rate;
        p.fire.temp_min = -50.0;
        p.fire.temp_full = -40.0;
        p.fire.spread = b.spread;
        p.fire.duration = 5;
        p.fire.tree_kill = 0.5;
        p.fire.animal_damage = 30.0;
        p.grazer.energy_cost = b.grazer_cost;
        p.grazer.max_age = b.grazer_max_age;
        p.hunter.energy_cost = b.hunter_cost;
        p.hunter.max_age = b.hunter_max_age;
        p.hunter.kill_prob = b.kill_prob;
        p.disease.grazer_rate = b.crowding;
        p.disease.hunter_rate = b.crowding;
        p.disease.grazer_threshold = 4;
        p.disease.hunter_threshold = 1;
        for (floor, every) in [
            (&mut p.grazer.immigration_floor, &mut p.grazer.immigration_interval),
            (&mut p.hunter.immigration_floor, &mut p.hunter.immigration_interval),
            (&mut p.tree.immigration_floor, &mut p.tree.immigration_interval),
        ] {
            *floor = b.floor;
            *every = 20;
        }
        p.tree.initial_age = p.tree.mature_age;
        p.tree.seed_every = p.tree.update_every;
        p
    }

    /// Run `ticks` ticks into a scratch run directory and return its events and the directory.
    fn busy_run(seed: u64, ticks: u32, every: u32, b: &Busy) -> (Vec<Event>, PathBuf) {
        let dir = scratch_dir();
        run(busy_params(b), seed, ticks, every, &[], &dir).unwrap();
        let events = parse_events(&fs::read_to_string(dir.join(EVENTS_FILE)).unwrap()).unwrap();
        (events, dir)
    }

    /// Acceptance property: the `death` rows of `events.csv`, counted per tick, species and cause,
    /// equal the series death columns on every tick; so `stats` reads the same counts either way.
    fn event_deaths_match_series(seed: u64, ticks: u32, every: u32, b: &Busy) -> Result<Vec<Event>, TestCaseError> {
        let (events, dir) = busy_run(seed, ticks, every, b);
        let rows = read_series(&dir).unwrap();
        let counted = deaths_per_tick(&events, rows.len()).unwrap();
        for (r, d) in rows.iter().zip(&counted) {
            prop_assert_eq!(r.deaths, *d, "tick {}", r.tick);
        }
        prop_assert_eq!(read_series_for_stats(&dir).unwrap(), rows);
        prop_assert!(events.windows(2).all(|w| w[0].tick <= w[1].tick), "rows are in tick order");
        prop_assert!(events.iter().all(|e| e.tick >= 1 && e.tick <= ticks));
        fs::remove_dir_all(&dir).unwrap();
        Ok(events)
    }

    fn busy() -> impl Strategy<Value = Busy> {
        let costs = (0.05f32..1.5, 20u32..3000, 0.02f32..1.5, 20u32..3000);
        (0.0f32..2.0, 0.0f32..2.0, costs, 0.0f64..=1.0, 0.0f32..0.2, 0u32..400).prop_map(
            |(
                fire_rate,
                spread,
                (grazer_cost, grazer_max_age, hunter_cost, hunter_max_age),
                kill_prob,
                crowding,
                floor,
            )| {
                Busy {
                    fire_rate,
                    spread,
                    grazer_cost,
                    grazer_max_age,
                    hunter_cost,
                    hunter_max_age,
                    kill_prob,
                    crowding,
                    floor,
                }
            },
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(12)))]

        #[test]
        fn prop_event_deaths_match_series(seed in any::<u64>(), ticks in 1u32..400, every in 1u32..500, b in busy()) {
            event_deaths_match_series(seed, ticks, every, &b)?;
        }

        /// Every `burnout` has an earlier `ignition` or `spread` of the same patch.
        #[test]
        fn prop_every_burnout_was_lit(seed in any::<u64>(), ticks in 1u32..400, b in busy()) {
            let (events, dir) = busy_run(seed, ticks, 100, &b);
            prop_assert_eq!(unlit_burnout(&events), None);
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    const EVERYTHING: Busy = Busy {
        fire_rate: 0.5,
        spread: 0.3,
        grazer_cost: 0.6,
        grazer_max_age: 400,
        hunter_cost: 0.8,
        hunter_max_age: 300,
        kill_prob: 0.3,
        crowding: 0.1,
        floor: 400,
    };

    /// One run in which every kind but the reserved `seed_drop`, and every animal death cause, is
    /// logged, flushed across several snapshots. (All four tree causes need a longer run; the
    /// seed-42 test in `tests/sweep.rs` sees them.)
    #[test]
    fn event_deaths_regression_every_kind_and_cause() {
        let events = event_deaths_match_series(1, 400, 70, &EVERYTHING).unwrap();
        let has = |k: EventKind, s: &str, c: &str| events.iter().any(|e| e.kind == k && e.species == s && e.cause == c);
        for k in [EventKind::Ignition, EventKind::Spread, EventKind::Burnout] {
            assert!(has(k, "", ""), "{}", k.name());
        }
        for s in ["grazer", "hunter"] {
            assert!(has(EventKind::Birth, s, "") && has(EventKind::Immigration, s, ""), "{s}");
        }
        assert!(has(EventKind::Germination, "tree", "") && has(EventKind::Immigration, "tree", ""));
        for c in Cause::ALL.map(Cause::name) {
            assert!(has(EventKind::Death, "grazer", c), "grazer {c}");
        }
        for c in ["starved", "old_age", "crowded", "burnt"] {
            assert!(has(EventKind::Death, "hunter", c), "hunter {c}");
        }
        assert!(has(EventKind::TreeDeath, "tree", "burnt"));
        assert!(!events.iter().any(|e| e.kind == EventKind::SeedDrop));
    }

    /// Fire off: no fire rows at all. The spread source is always a 4-neighbour of the patch it lit.
    #[test]
    fn burnout_regression_fire_off_and_spread_sources() {
        let quiet = Busy { fire_rate: 0.0, ..EVERYTHING };
        let (events, dir) = busy_run(2, 300, 100, &quiet);
        assert!(!events.iter().any(|e| matches!(e.kind, EventKind::Ignition | EventKind::Spread | EventKind::Burnout)));
        fs::remove_dir_all(&dir).unwrap();
        let (events, dir) = busy_run(2, 300, 100, &EVERYTHING);
        assert_eq!(unlit_burnout(&events), None);
        for e in events.iter().filter(|e| e.kind == EventKind::Spread) {
            let from = e.detail.unwrap() as usize;
            assert!(crate::fire::patch_neighbours(from).any(|q| q == e.patch as usize), "{e:?}");
        }
        // A burnout with no ignition before it is caught.
        let lone =
            Event { tick: 3, kind: EventKind::Burnout, species: "", patch: 9, col: None, cause: "", detail: None };
        assert_eq!(unlit_burnout(&[lone]), Some(&lone));
        let lit = Event { kind: EventKind::Ignition, tick: 1, ..lone };
        assert_eq!(unlit_burnout(&[lit, lone]), None);
        assert_eq!(unlit_burnout(&[lit, lone, lone]), Some(&lone), "one lighting, one burnout");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Logging never draws or writes sim state: with it on, every stats row and the RNG position
    /// match a run with it off.
    #[test]
    fn logging_draws_nothing_and_changes_nothing() {
        let (mut a, mut b) = (Sim::new(busy_params(&EVERYTHING), 5), Sim::new(busy_params(&EVERYTHING), 5));
        b.log_events = true;
        for _ in 0..300 {
            a.step();
            b.step();
            assert_eq!(a.stats(), b.stats());
            assert_eq!(a.rng.get_word_pos(), b.rng.get_word_pos());
        }
        assert!(a.events.is_empty() && b.events.len() > 100);
    }

    fn event() -> impl Strategy<Value = Event> {
        let kinds = prop::sample::select(EventKind::ALL.to_vec());
        let species = prop::sample::select(SPECIES.to_vec());
        let causes: Vec<&'static str> = Cause::ALL.iter().map(|c| c.name()).chain(TREE_CAUSES).chain([""]).collect();
        let col = prop::option::of((any::<u8>(), any::<u8>()));
        (any::<u32>(), kinds, species, 0u8..64, col, prop::sample::select(causes), prop::option::of(any::<u32>()))
            .prop_map(|(tick, kind, species, patch, col, cause, detail)| Event {
                tick,
                kind,
                species,
                patch,
                col,
                cause,
                detail,
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(64)))]

        /// A written line parses back to the same event.
        #[test]
        fn prop_event_line_round_trips(e in event()) {
            let mut line = String::new();
            e.write_line(&mut line);
            prop_assert_eq!(Event::parse(line.trim_end()), Ok(e));
        }
    }

    #[test]
    fn event_line_regression_empty_fields_and_bad_lines() {
        let e =
            Event { tick: 7, kind: EventKind::Ignition, species: "", patch: 63, col: None, cause: "", detail: None };
        let mut line = String::new();
        e.write_line(&mut line);
        assert_eq!(line, "7,ignition,,7,7,,,,\n");
        assert_eq!(Event::parse(line.trim_end()), Ok(e));
        for bad in
            ["7,ignition,,7,7,,,", "7,fire,,7,7,,,,", "7,death,cow,0,0,1,1,eaten,3", "7,death,grazer,8,0,1,1,eaten,3"]
        {
            assert!(Event::parse(bad).is_err(), "{bad}");
        }
        assert!(parse_events("tick,kind\n").is_err());
        let past = Event {
            tick: 5,
            kind: EventKind::Death,
            species: "grazer",
            patch: 0,
            col: None,
            cause: "eaten",
            detail: None,
        };
        assert!(deaths_per_tick(&[past], 5).is_err());
        assert!(deaths_per_tick(&[Event { species: "tree", ..past }], 6).is_err());
    }
}
