//! The agent gate (V0-spike.md, correction 3 and gate 2), driven over BRP's own port.
//!
//! Launch the viewer, ask it what it loaded, move the camera, apply three edits, take a screenshot
//! and read the PNG back -- with no human and no MCP server installed on this machine. Every call,
//! every retry and every failure is printed, which is the measurement.
//!
//! `agent_loop [--exe PATH] [--world DIR|--stress] [--port N] [--out PATH]`

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use ecoview_native::brp;
use serde_json::json;

struct Loop {
    port: u16,
    calls: u32,
    retries: u32,
    failures: Vec<String>,
}

impl Loop {
    /// One BRP call, retried up to twice. A retry here is exactly what the gate counts: a call an
    /// agent had to make again because the first shape was not accepted.
    fn call(&mut self, method: &str, params: serde_json::Value) -> Option<serde_json::Value> {
        for attempt in 0..3 {
            self.calls += 1;
            let t = Instant::now();
            match brp::call(self.port, method, params.clone()) {
                Ok(v) => {
                    println!("  {method} ok in {:.0} ms{}", t.elapsed().as_secs_f64() * 1000.0, if attempt > 0 { " (after a retry)" } else { "" });
                    return Some(v);
                }
                Err(e) => {
                    println!("  {method} FAILED: {e}");
                    self.failures.push(format!("{method}: {e}"));
                    self.retries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }
        None
    }
}

/// Width and height from a PNG's IHDR, so "read the PNG back" means more than "the file exists".
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let n = |o: usize| u32::from_be_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    Some((n(16), n(20)))
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str, d: &str| -> String {
        argv.iter()
            .position(|a| a == k)
            .and_then(|i| argv.get(i + 1))
            .cloned()
            .unwrap_or_else(|| d.to_string())
    };
    let exe = arg("--exe", "target/release/ecoview-native.exe");
    let port: u16 = arg("--port", "15702").parse().expect("a port number");
    let out = arg("--out", "shots/agent-loop.png");
    let out_abs = std::env::current_dir().unwrap().join(&out);
    let _ = std::fs::create_dir_all(out_abs.parent().unwrap());
    let _ = std::fs::remove_file(&out_abs);

    let started = Instant::now();
    let mut cmd = Command::new(&exe);
    if argv.iter().any(|a| a == "--stress") {
        cmd.arg("--stress");
    } else {
        cmd.args(["--world", &arg("--world", ecoview_native::CAPITOL)]);
    }
    cmd.args(["--port", &port.to_string()])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child: Child = cmd.spawn().unwrap_or_else(|e| panic!("cannot launch {exe}: {e}"));

    let ready = brp::wait_ready(port, 120);
    println!("launch -> BRP ready: {ready:?} after {:.1} s", started.elapsed().as_secs_f64());
    let mut l = Loop {
        port,
        calls: 0,
        retries: 0,
        failures: Vec::new(),
    };
    if ready.is_err() {
        l.failures.push("the viewer never answered on the BRP port".into());
    }

    let stats = l.call("ecoview.stats", json!({}));
    println!("  stats: {}", stats.clone().unwrap_or_default());
    let cell = stats
        .as_ref()
        .and_then(|s| s["cell_m"].as_f64())
        .unwrap_or(0.5) as f32;
    let width = stats.as_ref().and_then(|s| s["width"].as_u64()).unwrap_or(512) as f32;
    let size = width * cell;

    l.call(
        "ecoview.camera",
        json!({
            "pos": [size * 0.5, size * 0.35, -size * 0.15],
            "look_at": [size * 0.5, 0.0, size * 0.5],
        }),
    );

    let (cx, cy) = ((width / 2.0) as u64, (width / 2.0) as u64);
    for (i, action) in ["RaiseGround", "SetSurface", "RaiseBuilding"].iter().enumerate() {
        l.call(
            "ecoview.edit",
            json!({"x": cx + i as u64, "y": cy, "action": action, "medium": 6}),
        );
    }
    let after = l.call("ecoview.stats", json!({}));
    println!("  after three edits: {}", after.clone().unwrap_or_default());

    let shot = Instant::now();
    l.call(
        "brp_extras/screenshot",
        json!({"path": out_abs.to_string_lossy()}),
    );
    let mut png = Vec::new();
    for _ in 0..40 {
        if let Ok(b) = std::fs::read(&out_abs) {
            if png_size(&b).is_some() {
                png = b;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let shot_ms = shot.elapsed().as_secs_f64() * 1000.0;

    l.call("brp_extras/shutdown", json!({}));
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let _ = child.kill();
    let _ = child.wait();

    println!("---");
    println!("calls: {}, retries: {}", l.calls, l.retries);
    println!("time to first screenshot: {:.1} s total, {shot_ms:.0} ms for the call", started.elapsed().as_secs_f64());
    match png_size(&png) {
        Some((w, h)) => println!(
            "screenshot: {} bytes, {w}x{h}, at {}",
            png.len(),
            Path::new(&out).display()
        ),
        None => println!("screenshot: NOT READ BACK"),
    }
    for f in &l.failures {
        println!("failure: {f}");
    }
    let ok = png_size(&png).is_some() && l.retries <= 3 && after.is_some();
    println!("agent gate: {}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(i32::from(!ok));
}
