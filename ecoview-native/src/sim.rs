//! The round trip: write the edited site out as a bundle, run `ecosim` on it, and wait.
//!
//! This is the half of shot V5 that has no engine in it, so the CI gate exercises it with
//! `--no-default-features`. The viewer's job here is small and it is deliberately kept that way:
//! **the simulator decides, the viewer expresses** (`overnight/DIRECTION-native-viewer.md`). Nothing
//! in this module models anything. It writes files, spawns a process, counts the snapshot
//! directories that process leaves behind, and reports what the process said when it stopped.
//!
//! **CLAUDE.md's "share no code and have no IPC" is untouched.** `ecosim` is started the way shot
//! E4's browser helper starts it -- as a command, with arguments, in a directory -- and the run
//! directory it writes is the only thing that comes back. There is no socket, no pipe carrying
//! simulation state, no linked library and no shared type. The command line below is *copied* from
//! `ecoview/scripts/sim-lib.mjs`, not shared with it, for the same reason every other constant in
//! this crate is copied: the two viewers have no common code either.

use crate::bundle::Bundle;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

/// Ticks a round trip runs by default. Shot E4 chose 1,000 for a browser helper; this is 2,000
/// because the thing V5 exists to show is growth, and on the Capitol the simulator gets through
/// 1,000 ticks in under a second (MEASUREMENTS.md, V5). The reference runs stay on the command line.
pub const DEFAULT_TICKS: u32 = 2000;
pub const DEFAULT_SEED: u64 = 42;
/// The longest run the viewer will start. 20,000 is the project's own run length (CLAUDE.md), and
/// it is about 18 s of simulator on this site -- long enough to want a progress bar, short enough
/// that a person will wait for it.
pub const MAX_TICKS: u32 = 20000;

/// Ten snapshots over the run, whatever its length: enough to animate, small enough to keep several
/// round trips on disk. E4's rule, and the reason the timeline has something to scrub either way.
pub fn snapshot_every(ticks: u32) -> u32 {
    (ticks as f64 / 10.0).round().max(1.0) as u32
}

/// Where the simulator is. `ECOSIM_BIN` overrides it, so nothing here depends on the repo layout.
pub fn binary() -> PathBuf {
    if let Ok(p) = std::env::var("ECOSIM_BIN") {
        return PathBuf::from(p);
    }
    let exe = if cfg!(windows) {
        "ecosim.exe"
    } else {
        "ecosim"
    };
    Path::new("../ecosim/target/release").join(exe)
}

/// The simulator's parameter file, or `None` when it is not where it is expected. `ecosim` defaults
/// to `params.toml` beside its working directory, and the viewer's working directory is not that,
/// so a round trip either passes the real file or does not run at all.
pub fn params_file() -> Option<PathBuf> {
    let p = match std::env::var("ECOSIM_PARAMS") {
        Ok(p) => PathBuf::from(p),
        Err(_) => PathBuf::from("../ecosim/params.toml"),
    };
    p.exists().then_some(p)
}

/// The command line, which is the one a hand-run garden run uses.
///
/// `animals.enabled=false` and `climate.rain_gradient=0` are MASTER.md's standing rule for every
/// garden-series run on a bundle world: a rainfall ramp makes no sense on a 256 m photographed site,
/// and animals are not what this project is about any more. `--snapshot-state false` leaves out
/// `state.bin`, which only a fork reads, and takes about 40% off the directory.
pub fn ecosim_args(
    world: &Path,
    out: &Path,
    seed: u64,
    ticks: u32,
    every: u32,
    params: Option<&Path>,
) -> Vec<String> {
    let mut a: Vec<String> = [
        "run",
        "--world",
        &world.display().to_string(),
        "--out",
        &out.display().to_string(),
        "--seed",
        &seed.to_string(),
        "--ticks",
        &ticks.to_string(),
        "--snapshot-every",
        &every.to_string(),
        "--snapshot-state",
        "false",
        "--set",
        "animals.enabled=false",
        "--set",
        "climate.rain_gradient=0",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Some(p) = params {
        a.push("--params".into());
        a.push(p.display().to_string());
    }
    a
}

/// The furthest tick written so far, read from the snapshot directories the simulator leaves on
/// disk as it goes. The simulator says nothing about its progress on stdout, and asking the
/// filesystem is what E4's helper does; a name that is not `snap_NNNNNN` is ignored rather than
/// guessed at.
pub fn tick_of_snapshots<S: AsRef<str>>(names: impl Iterator<Item = S>) -> u32 {
    let mut tick = 0;
    for n in names {
        let n = n.as_ref();
        if let Some(digits) = n.strip_prefix("snap_") {
            if digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_digit()) {
                tick = tick.max(digits.parse().unwrap_or(0));
            }
        }
    }
    tick
}

/// Where a round trip stands. `Failed` carries what to put on the screen, which is the simulator's
/// own last line whenever there is one: a viewer that says "it failed" and keeps the reason in a
/// file nobody opens is a viewer that wastes an afternoon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimState {
    Running,
    Done,
    Failed(String),
}

/// One `ecosim` process, its scratch directories, and its log.
pub struct SimJob {
    child: Option<Child>,
    state: SimState,
    /// The edited bundle this run was started on. Written by [`SimJob::start`], never the source.
    pub world: PathBuf,
    /// The run directory the simulator is writing, which is what the viewer loads when it is done.
    pub out: PathBuf,
    /// Everything the child said, on both streams.
    pub log: PathBuf,
    pub seed: u64,
    pub ticks: u32,
    pub every: u32,
    /// The command line, for the HUD and the write-up. A round trip nobody can reproduce by hand is
    /// a round trip nobody can check.
    pub command: String,
    started: Instant,
    finished: Option<Duration>,
}

impl SimJob {
    /// Writes `bundle` with `grids` into `root/<stamp>/world`, then starts `ecosim` on it, writing
    /// its run into `root/<stamp>/run`.
    ///
    /// The scratch directory is per round trip and named from the clock, so two runs in one session
    /// never share a directory and a stale snapshot from the last one can never be counted as this
    /// one's progress. The simulator's working directory is that scratch directory, so a relative
    /// path inside it could only ever write inside what the viewer made.
    pub fn start(
        bundle: &Bundle,
        grids: (&[f32], &[u8], &[f32]),
        root: &Path,
        seed: u64,
        ticks: u32,
    ) -> io::Result<SimJob> {
        let ticks = ticks.clamp(1, MAX_TICKS);
        let every = snapshot_every(ticks);
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        // Absolute, because the child's working directory is the scratch directory and every path
        // it is handed has to mean the same thing from there as it does here. `absolute` rather
        // than `canonicalize`: the run directory does not exist yet, and on Windows canonicalising
        // would hand the simulator a `\?\` path it has no reason to have to understand.
        let root = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
        let dir = root.join(format!("sim-{stamp}"));
        let world = dir.join("world");
        let out = dir.join("run");
        std::fs::create_dir_all(&dir)?;
        bundle.save(
            &world,
            grids,
            &format!("ecoview-native V5: edited in the viewer, seed {seed}, {ticks} ticks"),
        )?;
        let bin = binary();
        if !bin.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "no ecosim binary at {} -- build it with `cargo build --release` in ecosim/, \
                     or point ECOSIM_BIN at one",
                    bin.display()
                ),
            ));
        }
        let params = params_file().map(|p| std::path::absolute(&p).unwrap_or(p));
        let args = ecosim_args(&world, &out, seed, ticks, every, params.as_deref());
        let log = dir.join("ecosim.log");
        let child = spawn(&bin, &args, &dir, &log)?;
        let mut job = SimJob::attach(child, world, out, log, ticks);
        job.seed = seed;
        job.command = format!("{} {}", bin.display(), args.join(" "));
        Ok(job)
    }

    /// Wraps a process this module did not spawn, so everything after the spawn can be tested.
    ///
    /// `ecosim` is not built in the `ecoview-native` CI job -- the gate compiles with
    /// `--no-default-features` and never leaves this crate -- so the test that checks a run is
    /// polled, finished, timed and reported has to attach to a process that exists on every
    /// machine. [`SimJob::start`] goes through here too, so the tested path is the flown path.
    pub fn attach(child: Child, world: PathBuf, out: PathBuf, log: PathBuf, ticks: u32) -> SimJob {
        SimJob {
            child: Some(child),
            state: SimState::Running,
            world,
            out,
            log,
            seed: DEFAULT_SEED,
            ticks,
            every: snapshot_every(ticks),
            command: String::new(),
            started: Instant::now(),
            finished: None,
        }
    }

    /// Asks the child whether it has stopped, without blocking the frame it is asked on.
    pub fn poll(&mut self) -> SimState {
        if self.state != SimState::Running {
            return self.state.clone();
        }
        let Some(child) = self.child.as_mut() else {
            return self.state.clone();
        };
        match child.try_wait() {
            Ok(None) => SimState::Running,
            Ok(Some(status)) => {
                self.child = None;
                self.finished = Some(self.started.elapsed());
                self.state = if status.success() {
                    SimState::Done
                } else {
                    SimState::Failed(format!(
                        "ecosim exited {}: {}",
                        status
                            .code()
                            .map_or("on a signal".into(), |c| c.to_string()),
                        last_line(&self.log)
                    ))
                };
                self.state.clone()
            }
            Err(e) => {
                self.child = None;
                self.finished = Some(self.started.elapsed());
                self.state = SimState::Failed(format!("cannot wait for ecosim: {e}"));
                self.state.clone()
            }
        }
    }

    /// The furthest tick on disk, and how far through the run that is.
    pub fn progress(&self) -> (u32, f32) {
        let names: Vec<String> = std::fs::read_dir(&self.out)
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        let tick = tick_of_snapshots(names.iter());
        (
            tick,
            (tick as f32 / self.ticks.max(1) as f32).clamp(0.0, 1.0),
        )
    }

    pub fn state(&self) -> SimState {
        self.state.clone()
    }

    /// Wall time so far, or the wall time it took if it has stopped.
    pub fn elapsed(&self) -> Duration {
        self.finished.unwrap_or_else(|| self.started.elapsed())
    }

    /// Stops the run. A round trip the user has changed their mind about should not keep a core
    /// busy behind the window they are still flying.
    pub fn cancel(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
        self.finished = Some(self.started.elapsed());
        self.state = SimState::Failed("cancelled".into());
    }
}

impl Drop for SimJob {
    /// A viewer that exits while a simulator it started is still running leaves a process behind.
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Starts `program` with both streams redirected into `log`.
///
/// **A file, not a pipe.** The viewer polls this child from inside a frame and never reads its
/// output, and an unread pipe fills and blocks the writer -- the simulator would stop halfway
/// through a long run with no error anywhere. A file cannot fill, and it leaves the whole of what
/// the simulator said on disk beside the run it wrote.
///
/// Public because the test that exercises the job's lifecycle has to spawn something that exists on
/// every machine, and `ecosim` is not built in the `ecoview-native` CI job.
pub fn spawn(program: &Path, args: &[String], cwd: &Path, log: &Path) -> io::Result<Child> {
    let out = std::fs::File::create(log)?;
    let err = out.try_clone()?;
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn()
}

/// The last non-empty line of a log file, or a note saying where to look. What the simulator prints
/// when it refuses a bundle is one line, and that line is the whole of the diagnosis.
pub fn last_line(log: &Path) -> String {
    match std::fs::read_to_string(log) {
        Ok(s) => s
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("it said nothing")
            .to_string(),
        Err(_) => format!("see {}", log.display()),
    }
}
