use clap::{Parser, Subcommand};
use ecosim::check::{check_run, check_run_long, diff_runs, stats_report};
use ecosim::output::run;
use ecosim::sweep::{baseline, margin_table, parse_range, parse_values, sweep, ParamSpec, SweepConfig};
use ecosim::Params;
use std::path::PathBuf;
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
    },
    /// Apply the acceptance invariants to a run directory; exit 1 on any failure.
    Check {
        run_dir: PathBuf,
        /// Apply the long-run invariants (runs of at least 60000 ticks) instead of the 20000-tick ones.
        #[arg(long)]
        long: bool,
    },
    /// Print min/max/mean per series column and the first extinction tick.
    Stats { run_dir: PathBuf },
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

fn main() -> ExitCode {
    match Cli::parse().cmd {
        Cmd::Run { seed, ticks, out, snapshot_every, params, set } => {
            if snapshot_every == 0 {
                eprintln!("--snapshot-every must be > 0");
                return ExitCode::FAILURE;
            }
            let p = match Params::load_with(&params, &set) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            match run(p, seed, ticks, snapshot_every, &set, &out) {
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
        Cmd::Stats { run_dir } => match stats_report(&run_dir) {
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
