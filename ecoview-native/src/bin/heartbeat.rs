//! The screenshot heartbeat (shot V13): one command that proves the viewer still draws the reference
//! run, and leaves the pictures behind to show it.
//!
//! ```text
//! cargo heartbeat [--run DIR] [--world DIR] [--tick N] [--out DIR] [--exe PATH]
//! ```
//!
//! `cargo heartbeat` is an alias (`.cargo/config.toml`) for `cargo run --release --bin heartbeat --`.
//! It builds the viewer, renders the fixed set in [`VIEWS`] one headless process at a time, and
//! writes each PNG and an `INDEX.md` into `shots/heartbeat/`. It exits non-zero if any view fails to
//! render, comes back blank, or draws the same picture as the view it has to differ from.
//!
//! **There is no golden image here, deliberately.** The repo-root rule keeps viewer assertions
//! coarse, and a pixel-exact set would pin the simulator's output inside this component -- the
//! coupling shot V11 removed. What is checked is that each frame is a picture at all (many colours,
//! no colour covering the frame) and that a frame which should differ from another does: an overlay
//! from the surface it is laid over, and the run from the bare bundle under it. Those two
//! comparisons are what go red when the palette or the run loader stops reaching the screen, without
//! saying anything about what the run grew. The images are for a person to look at.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use ecoview_native::{Bundle, CAPITOL};

/// The reference run, as `ecosim/justfile`'s `capitol` recipe writes it but under `runs/`, which is
/// where every V shot has read it from. Regenerable and gitignored; the heartbeat does not make it.
const RUN: &str = "../ecosim/runs/capitol-s42";
/// 21 September, the tick the project photographs most (DECISIONS.md, V9).
const TICK: u32 = 10000;

#[derive(Clone, Copy, PartialEq)]
enum Pose {
    /// The viewer's own overview pose, from the south-west corner.
    Iso,
    /// Straight down over the site's centre, high enough to hold the whole of it in frame, with
    /// the HUD's text block off because it would cover a third of the map. The legend stays.
    Top,
}

struct View {
    name: &'static str,
    pose: Pose,
    overlay: &'static str,
    /// Draw the run over the bundle. Off for the one view that shows what the bundle alone gives.
    run: bool,
    /// Flat light (`--no-sky --no-ao`), so a top face carries its overlay band and nothing else.
    flat: bool,
    /// The views this one has to differ from, and what each difference proves. Every overlay is
    /// checked against the surface and against the overlay before it: against the surface alone, all
    /// the top-down overlays differ from it by the same 47%, because an overlay also hides the
    /// ground cover, so a palette that drew every overlay alike would still pass.
    differs_from: &'static [(&'static str, &'static str)],
    shows: &'static str,
    wrong_if: &'static str,
}

const VIEWS: [View; 11] = [
    View {
        name: "iso-surface",
        pose: Pose::Iso,
        overlay: "surface",
        run: true,
        flat: false,
        differs_from: &[],
        shows:
            "The whole site from the south-west with the beauty pass on: the Capitol, its lawns \
                and paving, the run's trees and ground cover, the sky, the HUD.",
        wrong_if:
            "The frame is sky or clear colour only, the building is missing, or there are no \
                   trees on the lawns.",
    },
    View {
        name: "iso-moisture",
        pose: Pose::Iso,
        overlay: "moisture",
        run: true,
        flat: false,
        differs_from: &[("iso-surface", "the moisture palette reaches the screen")],
        shows: "The same pose under the moisture overlay: pale lawns, and the wettest columns in \
                dark-blue lines along the edges of paths and roads, where runoff from sealed ground \
                collects.",
        wrong_if: "It looks like iso-surface, or the whole site is one band.",
    },
    View {
        name: "top-no-run",
        pose: Pose::Top,
        overlay: "surface",
        run: false,
        flat: true,
        differs_from: &[],
        shows: "The bundle alone from above, flat-lit: ground, media and buildings, and the trees \
                the scene itself carries. No run is loaded.",
        wrong_if: "The frame is blank or one colour, the Capitol's footprint is missing, or the \
                   site is not square in the frame.",
    },
    View {
        name: "top-surface",
        pose: Pose::Top,
        overlay: "surface",
        run: true,
        flat: true,
        differs_from: &[("top-no-run", "the run loader reaches the screen")],
        shows: "The same frame with the run at the named tick over it: its trees, cover and \
                standing water on the bundle.",
        wrong_if: "It looks like top-no-run, which means the run was not drawn.",
    },
    View {
        name: "top-light",
        pose: Pose::Top,
        overlay: "light",
        run: true,
        flat: true,
        differs_from: &[("top-surface", "the light palette reaches the screen")],
        shows: "Light at the ground from above: open ground white, the Capitol's shadow black along \
                one side of it, and small grey squares where a crown shades its own column. The \
                crowns themselves keep their leaf colour.",
        wrong_if: "It looks like top-surface, or there is no black beside the building.",
    },
    View {
        name: "top-moisture",
        pose: Pose::Top,
        overlay: "moisture",
        run: true,
        flat: true,
        differs_from: &[
            ("top-surface", "the moisture palette reaches the screen"),
            ("top-light", "moisture is not drawn with light's colours"),
        ],
        shows: "Moisture from above: pale lawns in the patch grid's squares, and the wettest \
                columns in dark-blue lines along the edges of paths and roads, where runoff from \
                sealed ground collects.",
        wrong_if: "It looks like top-surface or top-light, or the lines along the paths are gone.",
    },
    View {
        name: "top-fertility",
        pose: Pose::Top,
        overlay: "fertility",
        run: true,
        flat: true,
        differs_from: &[
            ("top-surface", "the fertility palette reaches the screen"),
            (
                "top-moisture",
                "fertility is not drawn with moisture's colours",
            ),
        ],
        shows: "Fertility from above, on the run's own ramp: the lawns brown in the patch grid's \
                squares, paths, roads and roofs white.",
        wrong_if: "It looks like top-surface or top-moisture, or the lawns are one flat band.",
    },
    View {
        name: "top-water",
        pose: Pose::Top,
        overlay: "water",
        run: true,
        flat: true,
        differs_from: &[
            ("top-surface", "the water palette reaches the screen"),
            (
                "top-fertility",
                "water is not drawn with fertility's colours",
            ),
        ],
        shows: "Ponded depth from above, on a log ramp: the ground grey where nothing stands, \
                light-blue flecks of standing water along the roads and paths, none on the roofs.",
        wrong_if: "It looks like top-surface or top-fertility, or water stands on a roof.",
    },
    // Shot V12: the three soil pools of `npk.bin`. Each has to differ from the one before it as well
    // as from the surface, for the reason every overlay does, and here the reason is sharper: G5
    // measured three genuinely different maps, and one palette drawn three times would pass the
    // surface comparison alone.
    View {
        name: "top-nitrogen",
        pose: Pose::Top,
        overlay: "nitrogen",
        run: true,
        flat: true,
        differs_from: &[
            ("top-surface", "the nitrogen palette reaches the screen"),
            ("top-water", "nitrogen is not drawn with water's colours"),
        ],
        shows: "Nitrogen from above, on the run's ramp, pale where the soil is poor and green where                 it is rich: lawns in soft patch-grid squares, paler on the east half, and pale specks                 where a tree stands or has stood and drew its own square metre down. Paving, roads                 and roofs grey, which is no soil at all.",
        wrong_if: "It looks like top-phosphorus or top-potassium, the lawns are one flat colour with                    no specks, or a roof is coloured.",
    },
    View {
        name: "top-phosphorus",
        pose: Pose::Top,
        overlay: "phosphorus",
        run: true,
        flat: true,
        differs_from: &[
            ("top-surface", "the phosphorus palette reaches the screen"),
            ("top-nitrogen", "phosphorus is not nitrogen's plane or colours"),
        ],
        shows: "Phosphorus from above: nearly uniform dark purple, because the median column holds                 51 g/m2 of a pool that barely moves, with the drainage network etched pale across                 it -- the flow lines runoff has stripped of particulate P. The whole stored pool,                 not the tenth of it growth can reach.",
        wrong_if: "It is speckled like top-nitrogen, the pale flow lines are missing, or the lawns                    are pale and the lines dark (the ramp read the wrong way round).",
    },
    View {
        name: "top-potassium",
        pose: Pose::Top,
        overlay: "potassium",
        run: true,
        flat: true,
        differs_from: &[
            ("top-surface", "the potassium palette reaches the screen"),
            (
                "top-phosphorus",
                "potassium is not phosphorus's plane or colours",
            ),
        ],
        shows: "Potassium from above, between the other two: an even orange, with pale specks                 where trees stand or have stood. Nothing adds potassium in this model, so a column a                 tree drew down stays drawn down after the tree is gone.",
        wrong_if: "It looks like top-nitrogen's patch squares or top-phosphorus's flow lines, or it                    has no specks.",
    },
];

/// A frame is a picture if it has at least this many colours at 5 bits a channel...
const MIN_COLOURS: usize = 32;
/// ...and no one colour covers more than this share of it. A frame the pipeline never drew into is
/// the clear colour everywhere; one drawn before the meshes arrived is the clear colour and the HUD.
const MAX_ONE_COLOUR: f64 = 0.90;
/// Two frames differ if at least this share of pixels moved by more than [`DIFF_STEP`] in a channel.
const MIN_DIFFER: f64 = 0.02;
const DIFF_STEP: i16 = 8;
/// The rows the checks look at, as fractions of the frame's height: below the HUD's text block and
/// above the legend and the timeline. Measured on a frame taken at `--frames 3`, before any mesh was
/// ready: over the whole frame the HUD's anti-aliased text alone gave it 131 colours with the sky at
/// 51%, and the overlay's HUD lines made it "differ" from its pair by 21%. Both passed. In this band
/// the same frame is sky and nothing else.
const BAND: (f64, f64) = (0.55, 0.92);

/// Decoded to 8-bit RGB, whatever the PNG held.
struct Frame {
    w: u32,
    h: u32,
    rgb: Vec<u8>,
}

fn decode(path: &Path) -> Result<Frame, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(|e| format!("{} is not a PNG: {e}", path.display()))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!("{}: expected 8-bit samples", path.display()));
    }
    let n = info.color_type.samples();
    if n < 3 {
        return Err(format!("{}: expected an RGB or RGBA image", path.display()));
    }
    let rgb = buf[..info.buffer_size()]
        .chunks_exact(n)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    Ok(Frame {
        w: info.width,
        h: info.height,
        rgb,
    })
}

/// The pixels of [`BAND`], as RGB triples.
fn band(f: &Frame) -> &[u8] {
    let row = 3 * f.w as usize;
    let (a, b) = (BAND.0 * f.h as f64, BAND.1 * f.h as f64);
    &f.rgb[a as usize * row..b as usize * row]
}

/// (distinct colours at 5 bits a channel, share of the commonest one), over [`BAND`].
fn colours(f: &Frame) -> (usize, f64) {
    let mut counts = vec![0u32; 1 << 15];
    let px = band(f);
    for p in px.chunks_exact(3) {
        let k = (p[0] as usize >> 3) << 10 | (p[1] as usize >> 3) << 5 | p[2] as usize >> 3;
        counts[k] += 1;
    }
    let distinct = counts.iter().filter(|&&c| c > 0).count();
    let top = *counts.iter().max().unwrap_or(&0) as f64;
    (distinct, top / (px.len() / 3) as f64)
}

fn differ(a: &Frame, b: &Frame) -> f64 {
    if (a.w, a.h) != (b.w, b.h) {
        return 1.0;
    }
    let (pa, pb) = (band(a), band(b));
    let moved = pa
        .chunks_exact(3)
        .zip(pb.chunks_exact(3))
        .filter(|(p, q)| (0..3).any(|i| (p[i] as i16 - q[i] as i16).abs() > DIFF_STEP))
        .count();
    moved as f64 / (pa.len() / 3) as f64
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
    let world = arg("--world", CAPITOL);
    let run = arg("--run", RUN);
    let tick: u32 = arg("--tick", &TICK.to_string())
        .parse()
        .expect("--tick takes a tick number");
    let out = PathBuf::from(arg("--out", "shots/heartbeat"));
    let exe = match argv.iter().position(|a| a == "--exe") {
        Some(i) => PathBuf::from(&argv[i + 1]),
        None => build_viewer(),
    };

    if !Path::new(&run).join("meta.json").is_file() {
        eprintln!(
            "heartbeat: no run at {run}. Write the reference run first, from ecosim/:\n  \
             cargo run --release -- run --world worlds/capitol --seed 42 --ticks 20000 \
             --out runs/capitol-s42 --set animals.enabled=false --set climate.rain_gradient=0"
        );
        std::process::exit(2);
    }
    let bundle = Bundle::load(Path::new(&world))
        .unwrap_or_else(|e| panic!("heartbeat: cannot load the bundle at {world}: {e}"));
    let size = bundle.width as f32 * bundle.ground_cell_m;
    // Bevy's default vertical field of view is 45 degrees and the frame is wider than it is tall,
    // so the site fits when the eye is size / 2 / tan(22.5) above it; 1.2 of that leaves a margin
    // for the legend and the timeline along the bottom.
    // The eye sits a centimetre south of the centre because a camera looking straight down its own
    // up axis has no defined roll.
    let top = format!(
        "{0},{1},{2}",
        size / 2.0,
        1.2 * size / 2.0 / 0.4142,
        size / 2.0 + 0.01
    );
    let centre = format!("{0},0,{0}", size / 2.0);

    std::fs::create_dir_all(&out).expect("create the heartbeat directory");
    let mut failures: Vec<String> = Vec::new();
    let mut frames: Vec<(&str, Option<Frame>)> = Vec::new();
    let mut rows: Vec<String> = Vec::new();
    let started = Instant::now();

    for v in &VIEWS {
        let png = out.join(format!("{}.png", v.name));
        let _ = std::fs::remove_file(&png);
        let mut cmd = Command::new(&exe);
        cmd.args(["--world", &world, "--overlay", v.overlay, "--headless"]);
        if v.run {
            cmd.args(["--run", &run, "--tick", &tick.to_string()]);
        }
        if v.flat {
            cmd.args(["--no-sky", "--no-ao"]);
        }
        if v.pose == Pose::Top {
            cmd.args(["--eye", &top, "--look", &centre, "--no-hud"]);
        }
        cmd.arg("--screenshot").arg(&png);
        let flags: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().replace('\\', "/"))
            .collect();

        let t = Instant::now();
        let status = cmd.output();
        let secs = t.elapsed().as_secs_f64();
        let frame = match status {
            Ok(o) if o.status.success() => decode(&png),
            Ok(o) => Err(format!(
                "the viewer exited with {}: {}",
                o.status,
                String::from_utf8_lossy(&o.stderr)
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
            )),
            Err(e) => Err(format!("cannot start {}: {e}", exe.display())),
        };
        let verdict = match &frame {
            Err(e) => {
                failures.push(format!("{}: {e}", v.name));
                format!("FAILED: {e}")
            }
            Ok(f) => {
                let (distinct, share) = colours(f);
                let mut notes = vec![format!(
                    "{}x{}, {distinct} colours, commonest {:.1}%",
                    f.w,
                    f.h,
                    100.0 * share
                )];
                if distinct < MIN_COLOURS || share > MAX_ONE_COLOUR {
                    failures.push(format!(
                        "{}: blank ({distinct} colours, one covers {:.1}%)",
                        v.name,
                        100.0 * share
                    ));
                    notes.push("BLANK".into());
                }
                for &(other, proves) in v.differs_from {
                    match frames.iter().find(|(n, _)| *n == other) {
                        Some((_, Some(g))) => {
                            let d = differ(f, g);
                            notes.push(format!("{:.1}% differs from {other}", 100.0 * d));
                            if d < MIN_DIFFER {
                                failures.push(format!(
                                    "{}: only {:.2}% differs from {other}, so not proven that {proves}",
                                    v.name,
                                    100.0 * d
                                ));
                                notes.push("SAME PICTURE".into());
                            }
                        }
                        _ => failures.push(format!("{}: {other} did not render", v.name)),
                    }
                }
                notes.join(", ")
            }
        };
        println!("{:<14} {secs:5.1} s  {verdict}", v.name);
        rows.push(format!(
            "| `{0}.png` | `{1}` | {2} | {3} | {verdict} |",
            v.name,
            flags[..flags.len() - 2].join(" "),
            v.shows,
            v.wrong_if,
        ));
        frames.push((v.name, frame.ok()));
    }

    let index = format!(
        "# Viewer heartbeat\n\n\
         Written by `cargo heartbeat` (src/bin/heartbeat.rs), shot V13. Every file in this directory is \
         regenerated by that one command; do not edit it by hand.\n\n\
         Run `{run}` at tick {tick}, world `{world}`. {n} views, 1280 x 800, headless. The checks are \
         coarse on purpose, and measured on the rows from {:.0}% to {:.0}% of the frame's height, \
         below the HUD's text and above the legend: there each frame must have at least \
         {MIN_COLOURS} colours at 5 bits a channel with no colour over {:.0}% of it, and a view \
         that names another must differ from it in at least {:.0}% of those pixels by more than \
         {DIFF_STEP}/255. There is no golden image.\n\n\
         The pictures show what the simulator computed, drawn by the viewer; the viewer models no \
         ecology.\n\n\
         | File | Flags | What it shows | How you would tell it was wrong | Measured |\n\
         |---|---|---|---|---|\n{}\n",
        100.0 * BAND.0,
        100.0 * BAND.1,
        100.0 * MAX_ONE_COLOUR,
        100.0 * MIN_DIFFER,
        rows.join("\n"),
        n = VIEWS.len(),
    )
    .replace('\\', "/");
    std::fs::write(out.join("INDEX.md"), index).expect("write INDEX.md");

    let drawn = frames.iter().filter(|(_, f)| f.is_some()).count();
    println!(
        "heartbeat: {drawn} of {} views rendered in {:.0} s, {} failures, into {}",
        VIEWS.len(),
        started.elapsed().as_secs_f64(),
        failures.len(),
        out.display()
    );
    if !failures.is_empty() {
        for f in &failures {
            eprintln!("heartbeat FAILED: {f}");
        }
        std::process::exit(1);
    }
}

/// `cargo run --bin heartbeat` builds this binary and not the viewer, so build the viewer here --
/// a no-op when it is current -- rather than photograph whatever stale binary is on disk.
fn build_viewer() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let ok = Command::new(cargo)
        .args(["build", "--release", "--bin", "ecoview-native"])
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("heartbeat: building the viewer failed");
        std::process::exit(1);
    }
    let here = std::env::current_exe().expect("the heartbeat's own path");
    here.with_file_name(format!("ecoview-native{}", std::env::consts::EXE_SUFFIX))
}
