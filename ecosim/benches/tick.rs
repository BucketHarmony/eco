//! Tick throughput (shot 15a): 2000 ticks of seed 42 in memory, as a sweep cell runs them (steps
//! and stats rows, no files), on the 64×64 square world and on the 256×64 reference strip.
//!
//! Criterion does the sampling and prints its report. This harness also takes the median over
//! every timed run, reports it as ticks per second, writes `ci-runs/bench-tick.json`, and compares
//! it with `benches/baseline.json`: a world more than 20% slower than its baseline fails the bench
//! (exit 1). The baseline holds numbers from the GitHub CI runner, not from a workstation
//! (DECISIONS.md, shot 15a); with no baseline file the bench only reports.

use criterion::Criterion;
use ecosim::output::simulate;
use ecosim::{Params, Sim};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

const SEED: u64 = 42;
const TICKS: u32 = 2000;
/// The largest slowdown against the baseline that still passes.
const MAX_REGRESSION: f64 = 0.20;

/// Bench name and `--set` overrides of each world.
const WORLDS: [(&str, &[&str]); 2] =
    [("64x64", &["world.width=64", "climate.rain_gradient=0", "world.slope_bias=0"]), ("256x64", &[])];

fn params(sets: &[&str]) -> Params {
    let sets: Vec<String> = sets.iter().map(|s| s.to_string()).collect();
    Params::load_with(&Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml"), &sets).expect("params.toml")
}

/// Bench one world; returns the wall time of every timed run, warm-up included.
fn bench_world(c: &mut Criterion, name: &str, sets: &[&str]) -> Vec<Duration> {
    let p = params(sets);
    let runs = RefCell::new(Vec::new());
    c.bench_function(&format!("tick/{name}/2000"), |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let mut sim = Sim::new(p.clone(), SEED);
                let t = Instant::now();
                std::hint::black_box(simulate(&mut sim, TICKS, |_| Ok(())).expect("in-memory run"));
                let d = t.elapsed();
                runs.borrow_mut().push(d);
                total += d;
            }
            total
        })
    });
    runs.into_inner()
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn main() {
    let mut c = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5))
        .configure_from_args();
    let mut worlds = serde_json::Map::new();
    for (name, sets) in WORLDS {
        let runs = bench_world(&mut c, name, sets);
        if runs.is_empty() {
            continue; // filtered out on the command line
        }
        let n = runs.len();
        let tps = f64::from(TICKS) / median(runs).as_secs_f64();
        println!("{name}: {tps:.1} ticks/s (median of {n} runs of {TICKS} ticks)");
        worlds.insert(name.to_string(), json!({ "ticks_per_second": tps, "runs": n }));
    }
    c.final_summary();

    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let report = json!({ "seed": SEED, "ticks": TICKS, "worlds": worlds });
    fs::create_dir_all(dir.join("ci-runs")).expect("ci-runs/");
    fs::write(dir.join("ci-runs/bench-tick.json"), serde_json::to_string_pretty(&report).unwrap() + "\n")
        .expect("ci-runs/bench-tick.json");

    let Ok(text) = fs::read_to_string(dir.join("benches/baseline.json")) else {
        println!("no benches/baseline.json: reporting only");
        return;
    };
    let base: Value = serde_json::from_str(&text).expect("benches/baseline.json");
    let mut failed = false;
    for (name, now) in &worlds {
        let Some(was) = base["worlds"][name]["ticks_per_second"].as_f64() else {
            println!("{name}: no baseline");
            continue;
        };
        let now = now["ticks_per_second"].as_f64().unwrap();
        let change = now / was - 1.0;
        let ok = change >= -MAX_REGRESSION;
        failed |= !ok;
        let verdict = if ok { "ok" } else { "REGRESSION" };
        println!("{name}: {now:.1} ticks/s against baseline {was:.1} ({:+.1}%): {verdict}", change * 100.0);
    }
    if failed {
        eprintln!("tick throughput fell more than {:.0}% below benches/baseline.json", MAX_REGRESSION * 100.0);
        std::process::exit(1);
    }
}
