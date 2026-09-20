//! Wall time per tick phase, for `ecosim run --profile <file>`.
//!
//! A [`Profiler`] is a lap timer: each [`Profiler::lap`] charges the time since the previous lap to
//! one phase, so the phases tile the run with no gaps and their sum equals the total up to the few
//! nanoseconds between the last lap and the report. It reads only `std::time::Instant` and never
//! touches sim state or the RNG, so a profiled run writes the same run directory as an unprofiled one.

use crate::world::Dims;
use serde::Serialize;
use std::time::{Duration, Instant};

/// A part of the run that time is charged to, in the order a run meets them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Clearing the output directory, world generation, initial populations, `meta.json`.
    Setup,
    /// `update_animals` (grazers, then hunters), plus the tick counter and death-count reset.
    Animals,
    /// The immigration floors.
    Immigration,
    /// Grass and shrub growth per patch.
    Producers,
    /// The tree update (every `tree.update_every` ticks).
    Trees,
    /// Fire spread, burn-out and ignition.
    Fire,
    /// Moisture and fertility (every `schedule.soil_every` ticks).
    MoistureFertility,
    /// Temperature and season (every `schedule.temperature_every` ticks).
    TemperatureSeason,
    /// Removing dead entities (every `world.compact_every` ticks).
    Compaction,
    /// Building the `series.csv` row (`Sim::stats`).
    StatsRow,
    /// Writing snapshot directories.
    SnapshotWrite,
    /// Appending to `events.csv`.
    Events,
    /// Writing `series.csv` and `timing.json` at the end.
    SeriesWrite,
}

/// Every phase with its name in the profile file, in [`Phase`] order.
pub const PHASES: [(Phase, &str); 13] = [
    (Phase::Setup, "setup"),
    (Phase::Animals, "animals"),
    (Phase::Immigration, "immigration"),
    (Phase::Producers, "producers"),
    (Phase::Trees, "trees"),
    (Phase::Fire, "fire"),
    (Phase::MoistureFertility, "moisture_fertility"),
    (Phase::TemperatureSeason, "temperature_season"),
    (Phase::Compaction, "compaction"),
    (Phase::StatsRow, "stats_row"),
    (Phase::SnapshotWrite, "snapshot_write"),
    (Phase::Events, "events"),
    (Phase::SeriesWrite, "series_write"),
];

/// The phases that run inside `Sim::step`.
const STEP_PHASES: [Phase; 8] = [
    Phase::Animals,
    Phase::Immigration,
    Phase::Producers,
    Phase::Trees,
    Phase::Fire,
    Phase::MoistureFertility,
    Phase::TemperatureSeason,
    Phase::Compaction,
];

/// Lap timer over the phases of one run.
#[derive(Debug, Clone)]
pub struct Profiler {
    start: Instant,
    last: Instant,
    spent: [Duration; PHASES.len()],
}

impl Profiler {
    /// Start the clock; the first lap is charged from now.
    pub fn start() -> Profiler {
        let now = Instant::now();
        Profiler { start: now, last: now, spent: [Duration::ZERO; PHASES.len()] }
    }

    /// Charge the time since the previous lap (or the start) to `phase`.
    pub fn lap(&mut self, phase: Phase) {
        let now = Instant::now();
        self.spent[phase as usize] += now - self.last;
        self.last = now;
    }

    /// Time charged to `phase` so far.
    pub fn spent(&self, phase: Phase) -> Duration {
        self.spent[phase as usize]
    }

    /// The profile of a run of `ticks` ticks on `dims`, with the total taken now.
    pub fn report(&self, seed: u64, ticks: u32, dims: Dims) -> Profile {
        let total_ms = ms(self.start.elapsed());
        let phases: Vec<PhaseTime> = PHASES
            .iter()
            .map(|&(p, name)| {
                let t = ms(self.spent(p));
                PhaseTime { name, ms: t, share: if total_ms > 0.0 { t / total_ms } else { 0.0 } }
            })
            .collect();
        let phase_sum_ms = phases.iter().map(|p| p.ms).sum();
        let step_ms: f64 = STEP_PHASES.iter().map(|&p| ms(self.spent(p))).sum();
        let per_s = |t: f64| if t > 0.0 { f64::from(ticks) / (t / 1000.0) } else { 0.0 };
        Profile {
            seed,
            ticks,
            dims: [dims.wx, dims.wy, dims.wz, dims.patch],
            total_ms,
            phase_sum_ms,
            ticks_per_second: per_s(total_ms),
            step_ticks_per_second: per_s(step_ms),
            phases,
        }
    }
}

/// Charge the time since the previous lap to `phase`, when profiling.
pub fn lap(prof: &mut Option<&mut Profiler>, phase: Phase) {
    if let Some(p) = prof.as_deref_mut() {
        p.lap(phase);
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// One phase's line in the profile file.
#[derive(Debug, Clone, Serialize)]
pub struct PhaseTime {
    /// The phase's name in [`PHASES`].
    pub name: &'static str,
    /// Wall time charged to it, in milliseconds.
    pub ms: f64,
    /// Its share of `total_ms`.
    pub share: f64,
}

/// The profile file `ecosim run --profile` writes (JSON).
#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    /// The run's seed.
    pub seed: u64,
    /// Ticks simulated.
    pub ticks: u32,
    /// World size: width, depth, height, patch edge.
    pub dims: [usize; 4],
    /// Wall time of the whole run, from before the output directory is cleared to after `timing.json`.
    pub total_ms: f64,
    /// Sum of the phase times; equals `total_ms` but for the moment between the last lap and the report.
    pub phase_sum_ms: f64,
    /// `ticks` over the whole run's wall time, writing included.
    pub ticks_per_second: f64,
    /// `ticks` over the time spent in `Sim::step` alone (no stats rows, no writing).
    pub step_ticks_per_second: f64,
    /// Every phase, in [`PHASES`] order.
    pub phases: Vec<PhaseTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laps_tile_the_run_and_charge_the_named_phase() {
        let mut p = Profiler::start();
        std::thread::sleep(Duration::from_millis(5));
        p.lap(Phase::Animals);
        p.lap(Phase::Fire);
        std::thread::sleep(Duration::from_millis(2));
        let mut opt = Some(&mut p);
        lap(&mut opt, Phase::SeriesWrite);
        lap(&mut None, Phase::Setup);
        assert!(p.spent(Phase::Animals) >= Duration::from_millis(5));
        assert!(p.spent(Phase::SeriesWrite) >= Duration::from_millis(2));
        assert_eq!(p.spent(Phase::Setup), Duration::ZERO);
        let r = p.report(7, 10, crate::world::sq::D);
        assert_eq!(r.phases.len(), PHASES.len());
        assert!(r.phase_sum_ms <= r.total_ms && r.phase_sum_ms >= 0.95 * r.total_ms);
        assert_eq!((r.seed, r.ticks, r.dims), (7, 10, [64, 64, 32, 8]));
        let shares: f64 = r.phases.iter().map(|p| p.share).sum();
        assert!((shares - r.phase_sum_ms / r.total_ms).abs() < 1e-9);
        assert!(r.ticks_per_second > 0.0 && r.step_ticks_per_second >= r.ticks_per_second);
        for (i, &(ph, _)) in PHASES.iter().enumerate() {
            assert_eq!(ph as usize, i);
        }
    }
}
