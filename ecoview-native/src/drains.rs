//! Shot G8: the storm drains, and the storms that fill them, as the run recorded them.
//!
//! `ecosim` shot G6 made a bundle's `pipes.json` live: each inlet captures what reaches its ground
//! cell during a storm and hands it to its outlet. The run says so in three places, and this module
//! reads all three and computes nothing the simulator did not:
//!
//! - `meta.json`'s `world.pipes`: where each pipe runs, in metres from the site's south-west corner,
//!   and whether it is **illustrative** -- placed by whoever built the scene rather than surveyed;
//! - `events.csv`'s `pipe` rows, one per pipe per storm, with the water it took in m3;
//! - `series.csv`'s `rain_mm`, `outflow_mm` and `pipe_in_mm`, world means per tick.
//!
//! A pipe is drawn **solid while it is carrying water at the shown tick** -- its row at the
//! snapshot's own tick captured something -- and dashed otherwise. The tick and not the interval
//! before it, because a snapshot is the state after that tick's storm pass (SAD 1, the fixed tick
//! order) and the ponds drawn beside the pipe are that tick's ponds; the interval's total is printed
//! beside it, so a pipe that ran an hour before the snapshot is not hidden (DECISIONS.md, G8).
//!
//! Like the rest of the library half this module never mentions Bevy.

use crate::mesh::ChunkMesh;
use crate::palette::linear_rgba;
use crate::run::Run;
use serde::Deserialize;
use std::path::Path;

/// One entry of `world.pipes`, as `pipes.json` gives it (SAD 1, "Storm drains").
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Pipe {
    pub id: String,
    /// Metres from the site's south-west corner, `[x, y]`.
    pub inlet: [f32; 2],
    pub outlet: [f32; 2],
    #[serde(default)]
    pub capacity_m3h: f32,
    /// Placed by the scene's author, not surveyed. Every pipe on the Capitol is.
    #[serde(default)]
    pub illustrative: bool,
}

/// One `pipe` row of `events.csv`: at `tick`, pipe `pipe` (its index in `world.pipes`) took
/// `captured_m3` and let `overflow_m3` go on past it over the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipeRow {
    pub tick: u64,
    pub pipe: usize,
    pub captured_m3: f32,
    pub overflow_m3: f32,
}

/// Every `pipe` row, found by header name. `detail` is `"<pipe> <captured> <overflow> <n> <p>"`.
///
/// Missing is not fatal, for the reason a missing burnout list is not: a run older than `ecosim` G6
/// has no rows, and its pipes are then drawn dry, which is what they were -- the simulator did not
/// move water through them.
pub(crate) fn read_pipe_rows(path: &Path) -> Vec<PipeRow> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = raw.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let cols: Vec<&str> = header.split(',').map(str::trim).collect();
    let at = |name: &str| cols.iter().position(|c| *c == name);
    let (Some(i_tick), Some(i_kind), Some(i_detail)) = (at("tick"), at("kind"), at("detail"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() <= i_detail || f[i_kind] != "pipe" {
            continue;
        }
        let d: Vec<&str> = f[i_detail].split_whitespace().collect();
        if let (Ok(tick), Some(Ok(pipe)), Some(Ok(c)), Some(Ok(o))) = (
            f[i_tick].parse::<u64>(),
            d.first().map(|s| s.parse::<usize>()),
            d.get(1).map(|s| s.parse::<f32>()),
            d.get(2).map(|s| s.parse::<f32>()),
        ) {
            out.push(PipeRow {
                tick,
                pipe,
                captured_m3: c,
                overflow_m3: o,
            });
        }
    }
    out
}

/// What each pipe did at one snapshot: at its own tick, and over the interval since the previous
/// snapshot. Indexed like `world.pipes`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PipeFlow {
    /// Water taken at the snapshot's own tick, m3. Non-zero means the pipe is drawn solid.
    pub now_m3: Vec<f32>,
    /// Water taken over `(previous snapshot, this snapshot]`, m3.
    pub since_m3: Vec<f32>,
    /// Water that reached an inlet over the same interval and went on past it, m3.
    pub overflow_m3: Vec<f32>,
    /// Storm ticks in that interval that sent any pipe a row.
    pub storms: usize,
}

impl PipeFlow {
    /// Is pipe `k` carrying water at the shown tick? A row printed as `0.0000` took less than a
    /// twentieth of a litre, which is the file's own resolution, and is dry.
    pub fn carrying(&self, k: usize) -> bool {
        self.now_m3.get(k).is_some_and(|v| *v > 0.0)
    }

    pub fn carrying_count(&self) -> usize {
        (0..self.now_m3.len()).filter(|k| self.carrying(*k)).count()
    }
}

/// One snapshot interval's water, from `series.csv`: world means in mm, summed over the ticks
/// `(previous snapshot, this snapshot]`, and the wettest single tick in it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct IntervalWater {
    pub rain_mm: f32,
    pub outflow_mm: f32,
    pub pipe_mm: f32,
    pub peak_mm: f32,
    pub peak_tick: u64,
}

/// `series.csv`'s rain, surface outflow and drain intake, summed per snapshot interval.
///
/// An `Err` says why there is no chart, in words the HUD prints: a run with no `series.csv`, or one
/// written before `ecosim` G4 gave it water columns. `pipe_in_mm` is optional, because a run between
/// G4 and G6 has rain and no drains, and that run's drains took nothing.
pub fn read_water_series(path: &Path, snapshots: &[u64]) -> Result<Vec<IntervalWater>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = raw.lines();
    let header = lines
        .next()
        .ok_or_else(|| format!("{} is empty", path.display()))?;
    let cols: Vec<&str> = header.split(',').map(str::trim).collect();
    let at = |name: &str| cols.iter().position(|c| *c == name);
    let (Some(i_tick), Some(i_rain), Some(i_out)) = (at("tick"), at("rain_mm"), at("outflow_mm"))
    else {
        return Err(format!(
            "{} has no rain_mm and outflow_mm columns (a run older than ecosim G4)",
            path.display()
        ));
    };
    let i_pipe = at("pipe_in_mm");
    let mut out = vec![IntervalWater::default(); snapshots.len()];
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let num = |i: usize| f.get(i).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
        let Some(tick) = f.get(i_tick).and_then(|s| s.parse::<u64>().ok()) else {
            continue;
        };
        // The first snapshot at or after this tick: the interval it closes.
        let k = snapshots.partition_point(|&s| s < tick);
        let Some(w) = out.get_mut(k) else { continue };
        let rain = num(i_rain);
        w.rain_mm += rain;
        w.outflow_mm += num(i_out);
        w.pipe_mm += i_pipe.map_or(0.0, num);
        if rain > w.peak_mm {
            w.peak_mm = rain;
            w.peak_tick = tick;
        }
    }
    Ok(out)
}

impl Run {
    /// The run's drains, from `meta.json`'s `world.pipes`. Empty on a noise world and on a run older
    /// than `ecosim` G6.
    pub fn pipes(&self) -> &[Pipe] {
        self.meta.world.as_ref().map_or(&[], |w| &w.pipes)
    }

    /// What each pipe did at snapshot `i`.
    pub fn pipe_flow(&self, i: usize) -> PipeFlow {
        let n = self.pipes().len();
        let mut f = PipeFlow {
            now_m3: vec![0.0; n],
            since_m3: vec![0.0; n],
            overflow_m3: vec![0.0; n],
            storms: 0,
        };
        let to = self.tick_at(i);
        // Before the first snapshot there is no previous one, so its interval is its own tick.
        let from = if i > 0 {
            Some(self.tick_at(i - 1))
        } else {
            None
        };
        let mut last = None;
        for r in &self.pipe_rows {
            if r.pipe >= n || r.tick > to || from.is_some_and(|t| r.tick <= t) {
                continue;
            }
            if from.is_none() && r.tick != to {
                continue;
            }
            if r.tick == to {
                f.now_m3[r.pipe] += r.captured_m3;
            }
            f.since_m3[r.pipe] += r.captured_m3;
            f.overflow_m3[r.pipe] += r.overflow_m3;
            if last != Some(r.tick) {
                f.storms += 1;
                last = Some(r.tick);
            }
        }
        f
    }

    /// The snapshot straight after the run's wettest tick: the first one whose picture holds that
    /// storm's water. `None` when the run has no water series or it never rained.
    pub fn largest_storm_snapshot(&self) -> Option<usize> {
        let w = self.water.as_ref().ok()?;
        let (i, best) = w
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.peak_mm.total_cmp(&b.1.peak_mm))?;
        (best.peak_mm > 0.0).then_some(i)
    }

    /// The snapshot at whose own tick the drains took the most water between them, which is the
    /// one frame where they are drawn carrying the most. `None` when no pipe ever carried at a
    /// snapshot tick.
    pub fn busiest_drain_snapshot(&self) -> Option<usize> {
        (0..self.snapshot_count())
            .map(|i| (i, self.pipe_flow(i).now_m3.iter().sum::<f32>()))
            .filter(|(_, v)| *v > 0.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
}

/// The length of one dash of a dry pipe, and one piece of a wet one, in metres.
pub const DASH_M: f32 = 1.0;
/// A carrying pipe is drawn this wide and this tall, and a dry one narrower and lower, so the two
/// read apart in a top-down frame as well as by their dashes.
pub const WET_WIDTH_M: f32 = 1.5;
pub const DRY_WIDTH_M: f32 = 0.8;
pub const WET_HEIGHT_M: f32 = 0.6;
pub const DRY_HEIGHT_M: f32 = 0.3;
/// Colours are the viewer's own: `meta.json` has no row for a drain. The wet one is a saturated
/// blue that is neither a pond's pale cyan nor the `water` medium's; the dry one is orange, a hue
/// nothing on the surface map is drawn in (potassium's ramp is orange too, so under that overlay a
/// dry pipe is faint). It was near white first, and a dry pipe over the Capitol's pale concrete
/// paths was all but invisible (shot G8's first close-up).
pub const WET_HEX: &str = "#1f4dff";
pub const DRY_HEX: &str = "#ff8c1a";

/// The drains as geometry: a straight run of boxes from inlet to outlet at the ground's top, solid
/// where [`PipeFlow::carrying`] says the pipe is carrying water and every other [`DASH_M`] piece
/// where it is not.
///
/// `top(x, y)` is the height of the drawn ground's top face at a point in metres, in the frame the
/// chunk meshes use (x east, y up, z the site's y). A pipe is a schematic laid on the surface: the
/// real ones are underground, and nothing in the run says how deep.
pub fn pipe_mesh(pipes: &[Pipe], flow: &PipeFlow, top: impl Fn(f32, f32) -> f32) -> ChunkMesh {
    let mut m = ChunkMesh::default();
    let (wet, dry) = (linear_rgba(WET_HEX), linear_rgba(DRY_HEX));
    for (k, p) in pipes.iter().enumerate() {
        let carrying = flow.carrying(k);
        let (w, h, colour) = if carrying {
            (WET_WIDTH_M, WET_HEIGHT_M, wet)
        } else {
            (DRY_WIDTH_M, DRY_HEIGHT_M, dry)
        };
        let (dx, dy) = (p.outlet[0] - p.inlet[0], p.outlet[1] - p.inlet[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 0.0 {
            continue;
        }
        let pieces = (len / DASH_M).ceil() as usize;
        for j in 0..pieces {
            if !carrying && j % 2 == 1 {
                continue;
            }
            let (t0, t1) = (j as f32 / pieces as f32, (j + 1) as f32 / pieces as f32);
            let a = [p.inlet[0] + dx * t0, p.inlet[1] + dy * t0];
            let b = [p.inlet[0] + dx * t1, p.inlet[1] + dy * t1];
            let base = top(a[0], a[1]).max(top(b[0], b[1])) + 0.05;
            push_box(&mut m, a, b, w, base, base + h, colour);
        }
    }
    m
}

/// A box along the ground from `a` to `b` (metres, site frame), `w` wide, from `y0` to `y1`.
fn push_box(m: &mut ChunkMesh, a: [f32; 2], b: [f32; 2], w: f32, y0: f32, y1: f32, c: [f32; 4]) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    // Across the pipe, half a width each side.
    let (px, py) = (-dy / len * w / 2.0, dx / len * w / 2.0);
    // The four footprint corners in (east, north), counter-clockwise seen from above.
    let foot = [
        [a[0] - px, a[1] - py],
        [b[0] - px, b[1] - py],
        [b[0] + px, b[1] + py],
        [a[0] + px, a[1] + py],
    ];
    let v = |i: usize, y: f32| [foot[i][0], y, foot[i][1]];
    let mut quad = |q: [[f32; 3]; 4], n: [f32; 3]| {
        let base = m.positions.len() as u32;
        m.positions.extend(q);
        m.normals.extend([n; 4]);
        m.colors.extend([c; 4]);
        m.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    quad([v(0, y1), v(1, y1), v(2, y1), v(3, y1)], [0.0, 1.0, 0.0]);
    quad([v(0, y0), v(3, y0), v(2, y0), v(1, y0)], [0.0, -1.0, 0.0]);
    for i in 0..4 {
        let j = (i + 1) % 4;
        let (ex, ez) = (foot[j][0] - foot[i][0], foot[j][1] - foot[i][1]);
        let el = (ex * ex + ez * ez).sqrt().max(1e-6);
        quad(
            [v(i, y0), v(j, y0), v(j, y1), v(i, y1)],
            [ez / el, 0.0, -ex / el],
        );
    }
}
