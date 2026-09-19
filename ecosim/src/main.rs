use clap::{Parser, Subcommand};
use ecosim::check::{self, check_run, check_run_long, diff_runs, signature_line, stats_report};
use ecosim::output::{fork, run_profiled, run_with, ForkSpec, RunOptions, FORMAT_VERSION};
use ecosim::sweep::{baseline, margin_table, parse_range, parse_values, sweep, ParamSpec, SweepConfig};
use ecosim::Params;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "ecosim", about = "POC voxel ecology simulator")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a simulation and write a run directory.
    Run {
        #[arg(long)]
        seed: u64,
        #[arg(long, default_value_t = 20000)]
        ticks: u32,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 100)]
        snapshot_every: u32,
        #[arg(long, default_value = "params.toml")]
        params: PathBuf,
        /// Override a params key before the run: `--set section.key=value` (repeatable).
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Write `state.bin` into every snapshot, so the run can be forked (`--snapshot-state false` to skip).
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        snapshot_state: bool,
        /// Run directory format: 3 writes `events.csv`; 2 writes the version-2 directory without it.
        #[arg(long, default_value_t = FORMAT_VERSION, value_parser = clap::value_parser!(u32).range(2..=3))]
        format_version: u32,
        /// Write the wall time of each tick phase to this JSON file, which must lie outside --out.
        /// The run directory is the same with and without it.
        #[arg(long, value_name = "FILE")]
        profile: Option<PathBuf>,
    },
    /// Continue a run from one of its snapshots into a new run directory, optionally with changed params.
    Fork {
        /// The parent run directory (format_version 3).
        run: PathBuf,
        /// The snapshot tick to continue from.
        #[arg(long)]
        at: u32,
        /// Override a params key from the fork tick on: `--set section.key=value` (repeatable).
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Ticks to run after the fork tick.
        #[arg(long)]
        ticks: u32,
        #[arg(long)]
        out: PathBuf,
    },
    /// Apply the acceptance invariants to a run directory; exit 1 on any failure.
    Check {
        run_dir: PathBuf,
        /// Apply the long-run invariants (runs of at least 60000 ticks) instead of the 20000-tick ones.
        #[arg(long)]
        long: bool,
    },
    /// Print min/max/mean per series column and the first extinction tick.
    Stats {
        run_dir: PathBuf,
        /// Print only the predator–prey signature. RUN_DIR may also be a bare series.csv-format
        /// file (a sweep's cells/*.csv), which needs --year-len. Both series are detrended by a
        /// centred 12000-tick moving average (t-6000..=t+6000, cut short at the run's ends), then
        /// lose their mean by season phase (tick mod climate.year_len, over the whole run).
        /// pp_lag is the lag in -8000..8000 (step 50) of the largest correlation of grazers(t) with
        /// hunters(t+lag) over ticks 5000-60000 (or the run's end), and pp_corr its value.
        /// pp_period is the lag of the first positive local maximum of the hunter series'
        /// autocorrelation (same window) after it first falls below 0, lags 50..=20000 step 50;
        /// undefined when there is none. pp_pass: pp_lag > 0, pp_lag < pp_period/2, pp_corr > 0.3.
        #[arg(long)]
        signature: bool,
        /// Year length for --signature, instead of meta.json's year_len.
        #[arg(long)]
        year_len: Option<u32>,
    },
    /// Byte-compare two run directories (ignoring timing.json); exit 1 on any difference.
    Diff { a: PathBuf, b: PathBuf },
    /// Run a parameter grid across seeds in memory and report where each invariant breaks.
    #[command(disable_help_flag = true)]
    Sweep {
        /// See `ecosim sweep --help`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

const SWEEP_USAGE: &str =
    "usage: ecosim sweep (--param KEY (--range START:STOP:STEP | --values A,B,C))... --out DIR [options]
       ecosim sweep --baseline [options]
options:
  --seeds 1,2,3      seeds per grid point (default 1,2,3)
  --ticks N          ticks per cell (default 20000)
  --jobs N           worker threads (default 8)
  --set KEY=VALUE    fixed override applied to every cell before the swept values (repeatable)
  --params PATH      params file (default params.toml)
Grid order: first --param slowest, last fastest, seeds fastest of all.";

enum SweepCmd {
    Grid(SweepConfig, PathBuf),
    Baseline(SweepConfig),
    Help,
}

fn parse_sweep(args: &[String]) -> Result<SweepCmd, String> {
    let mut cfg = SweepConfig {
        params_path: "params.toml".into(),
        fixed: Vec::new(),
        specs: Vec::new(),
        seeds: vec![1, 2, 3],
        ticks: 20_000,
        jobs: 8,
    };
    let (mut out, mut base) = (None, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--help" | "-h" => return Ok(SweepCmd::Help),
            "--param" => cfg.specs.push(ParamSpec { key: val()?, values: Vec::new() }),
            "--range" | "--values" => {
                let spec = cfg.specs.last_mut().ok_or_else(|| format!("{a} must follow a --param"))?;
                if !spec.values.is_empty() {
                    return Err(format!("--param {} has more than one --range/--values", spec.key));
                }
                let v = val()?;
                spec.values = if a == "--range" { parse_range(&v)? } else { parse_values(&v)? };
            }
            "--seeds" => {
                cfg.seeds = val()?
                    .split(',')
                    .map(|s| s.trim().parse::<u64>().map_err(|_| format!("--seeds: '{s}' is not a seed")))
                    .collect::<Result<_, _>>()?
            }
            "--ticks" => cfg.ticks = val()?.parse().map_err(|_| "--ticks: expected an integer".to_string())?,
            "--jobs" => cfg.jobs = val()?.parse().map_err(|_| "--jobs: expected an integer".to_string())?,
            "--out" => out = Some(PathBuf::from(val()?)),
            "--set" => cfg.fixed.push(val()?),
            "--params" => cfg.params_path = val()?.into(),
            "--baseline" => base = true,
            other => return Err(format!("unknown sweep argument '{other}'")),
        }
    }
    if let Some(p) = cfg.specs.iter().find(|p| p.values.is_empty()) {
        return Err(format!("--param {} needs --range or --values", p.key));
    }
    if cfg.seeds.is_empty() || cfg.jobs == 0 {
        return Err("need at least one seed and one job".into());
    }
    match (base, cfg.specs.is_empty(), out) {
        (true, true, _) => Ok(SweepCmd::Baseline(cfg)),
        (true, false, _) => Err("--baseline takes no --param".into()),
        (false, true, _) => Err("need at least one --param, or --baseline".into()),
        (false, false, None) => Err("--out is required".into()),
        (false, false, Some(o)) => Ok(SweepCmd::Grid(cfg, o)),
    }
}

/// Whether `file` lies inside directory `dir` (compared as absolute paths, before either exists).
fn inside(file: &Path, dir: &Path) -> bool {
    let abs = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    abs(file).starts_with(abs(dir))
}

fn main() -> ExitCode {
    match Cli::parse().cmd {
        Cmd::Run { seed, ticks, out, snapshot_every, params, set, snapshot_state, format_version, profile } => {
            if snapshot_every == 0 {
                eprintln!("--snapshot-every must be > 0");
                return ExitCode::FAILURE;
            }
            if profile.as_deref().is_some_and(|f| inside(f, &out)) {
                eprintln!("--profile must be outside the run directory --out");
                return ExitCode::FAILURE;
            }
            let p = match Params::load_with(&params, &set) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            let opts = RunOptions { state: snapshot_state, format_version };
            let result = match &profile {
                None => run_with(p, seed, ticks, snapshot_every, &set, &out, opts),
                Some(file) => run_profiled(p, seed, ticks, snapshot_every, &set, &out, opts).and_then(|(s, prof)| {
                    let json = serde_json::to_string_pretty(&prof).map_err(std::io::Error::other)?;
                    std::fs::write(
                        file,
                        json + "
",
                    )?;
                    Ok(s)
                }),
            };
            match result {
                Ok(s) => {
                    let last = s.rows.last().unwrap();
                    println!(
                        "wrote {} ({} ticks, {} ms): grazers={} hunters={} trees={}",
                        out.display(),
                        ticks,
                        s.wall_ms,
                        last.grazers,
                        last.hunters,
                        last.trees
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("run failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Cmd::Fork { run, at, set, ticks, out } => {
            match fork(&ForkSpec { parent: &run, at, overrides: &set, ticks }, &out) {
                Ok(s) => {
                    let last = s.rows.last().unwrap();
                    println!(
                        "wrote {} (ticks {at}..={}, {} ms): grazers={} hunters={} trees={}",
                        out.display(),
                        last.tick,
                        s.wall_ms,
                        last.grazers,
                        last.hunters,
                        last.trees
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("fork failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Cmd::Check { run_dir, long } => match if long { check_run_long(&run_dir) } else { check_run(&run_dir) } {
            Ok(report) => {
                for l in &report.lines {
                    let verdict = if l.pass { "PASS" } else { "FAIL" };
                    println!("{verdict} {}: {} [margin {:+.4}]", l.name, l.observed, l.margin);
                }
                if report.pass() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(e) => {
                eprintln!("FAIL {e}");
                ExitCode::FAILURE
            }
        },
        Cmd::Stats { run_dir, signature, year_len } => match if signature {
            check::signature_of(&run_dir, year_len).map(|s| vec![signature_line(&s)])
        } else {
            stats_report(&run_dir)
        } {
            Ok(lines) => {
                lines.iter().for_each(|l| println!("{l}"));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Cmd::Diff { a, b } => match diff_runs(&a, &b) {
            Ok(d) if d.is_empty() => {
                println!("identical");
                ExitCode::SUCCESS
            }
            Ok(d) => {
                d.iter().for_each(|l| println!("{l}"));
                ExitCode::FAILURE
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Cmd::Sweep { args } => match parse_sweep(&args) {
            Ok(SweepCmd::Help) => {
                println!("{SWEEP_USAGE}");
                ExitCode::SUCCESS
            }
            Ok(SweepCmd::Baseline(cfg)) => match baseline(&cfg) {
                Ok(reports) => {
                    print!("{}", margin_table(&reports));
                    if reports.iter().all(|(_, r)| r.pass()) {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            Ok(SweepCmd::Grid(cfg, out)) => match sweep(&cfg, &out) {
                Ok(results) => {
                    let pass = results.iter().filter(|r| r.report.pass()).count();
                    println!("wrote {}: {pass}/{} cells pass", out.display(), results.len());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("{e}\n{SWEEP_USAGE}");
                ExitCode::FAILURE
            }
        },
    }
}
