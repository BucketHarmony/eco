use clap::{Parser, Subcommand};
use ecosim::check::{check_run, diff_runs, stats_report};
use ecosim::output::run;
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
    },
    /// Apply the acceptance invariants to a run directory; exit 1 on any failure.
    Check { run_dir: PathBuf },
    /// Print min/max/mean per series column and the first extinction tick.
    Stats { run_dir: PathBuf },
    /// Byte-compare two run directories (ignoring timing.json); exit 1 on any difference.
    Diff { a: PathBuf, b: PathBuf },
}

fn main() -> ExitCode {
    match Cli::parse().cmd {
        Cmd::Run { seed, ticks, out, snapshot_every, params } => {
            if snapshot_every == 0 {
                eprintln!("--snapshot-every must be > 0");
                return ExitCode::FAILURE;
            }
            let p = match Params::load(&params) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            match run(p, seed, ticks, snapshot_every, &out) {
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
        Cmd::Check { run_dir } => match check_run(&run_dir) {
            Ok(lines) => {
                let mut ok = true;
                for l in &lines {
                    println!("{} {}: {}", if l.pass { "PASS" } else { "FAIL" }, l.name, l.observed);
                    ok &= l.pass;
                }
                if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
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
    }
}
