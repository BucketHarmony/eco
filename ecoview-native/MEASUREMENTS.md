# ecoview-native V0: the measurements

Every number below was taken on the machine this repo builds on: Windows 11, a 12th Gen Intel Core
i9-12900KF (16 cores, 24 threads), RTX 4090, Vulkan, `cargo build --release` unless the line says
otherwise. Two worlds are measured throughout:

- **Capitol** — the committed bundle `ecosim/worlds/capitol/`, 512×512 ground cells at **0.5 m**.
- **Stress** — synthetic, 512×512 at **0.25 m** with noise terrain, 200 buildings and 200 trees. The
  cell size is deliberately not 0.5 m (correction 4), so a hard-coded cube edge would show up as a
  wrong-sized world rather than hide.

| World | Chunks | With geometry | Triangles |
|---|---|---|---|
| Capitol | 324 | 93 | 183,096 |
| Stress | 405 | 247 | 727,612 |

## Frame time

`ecoview-native --bench 30` flies the loaded world for 30 s at 1280×800 and reports the frame-time
distribution. `PresentMode::AutoNoVsync` is set: the first reading was a flat 59.9 fps, which was the
display's 60 Hz refresh and not the renderer.

| World | Frames in 30 s | Mean | 95th percentile |
|---|---|---|---|
| Stress | 8,771 | 3.42 ms (292.3 fps) | 3.98 ms (251.2 fps) |
| Capitol | 8,772 | 3.42 ms (292.4 fps) | 3.99 ms (250.8 fps) |

The two worlds are indistinguishable at four times the triangle count, so at this scale the frame is not
mesh-bound.

## Mesh time

`mesh_measure [capitol|stress]`, no engine and no GPU. "N threads" is 24 worker threads over
`std::thread::scope`; the in-engine number beside it is the same work on Bevy's `ComputeTaskPool` at
startup, printed by the viewer itself.

| World | Read bundle | Voxelise | Mesh, 1 thread | Mesh, 24 threads | In engine |
|---|---|---|---|---|---|
| Capitol | 2–4 ms | 15–18 ms | 257–266 ms | 33–37 ms | 27–29 ms |
| Stress | — (generated) | 185–190 ms | 419 ms | 53 ms | 43 ms |

**Single-edit remesh**, median of 20 alternating RaiseGround/LowerGround edits, each remeshing only the
chunks the edited column touches:

| World | Chunks touched | Median | Worst |
|---|---|---|---|
| Capitol | 3 | 3.52–3.80 ms | 4.79 ms |
| Stress | 3 | 3.70 ms | — |

## The agent loop

`agent_loop` launches the viewer as a child process and drives it over BRP's own JSON-RPC port (15702)
with no MCP server and no human (correction 3). The sequence is: wait for `rpc.discover` → `ecoview.stats`
→ `ecoview.camera` → three `ecoview.edit` calls (RaiseGround, SetSurface, RaiseBuilding) → `ecoview.stats`
again → `brp_extras/screenshot` → read the PNG's IHDR back → `brp_extras/shutdown`.

| | |
|---|---|
| Completed | yes |
| Calls | 8 |
| Retries | **0** |
| Launch → BRP ready | 1.7 s, first `rpc.discover` answered |
| Time to first screenshot | 3.3 s from launch; 26 ms for the screenshot call itself |
| PNG read back | 899,365 bytes, 1280×800 — `shots/agent-loop.png` |
| Edits confirmed | `ecoview.stats` reported `edits: 3`, `last_remesh_ms: 2.47` |

The explicit `ecoview.*` methods are what makes this zero-retry: an agent never has to discover a
component schema, only three documented method names.

One client-side defect was found and fixed on the way: `bevy_remote` answers with HTTP **chunked**
transfer encoding, and the first version of `src/brp.rs` fed the hex chunk-size line to the JSON parser.
Every call had in fact succeeded on the server; only the replies failed to parse. `brp::dechunk` joins
the chunks. This was a fault in the measurement harness, not in the engine or the gate.

## Compile time

| | |
|---|---|
All four dev numbers use a `CARGO_TARGET_DIR` of their own, so "clean" means every dependency compiled
from scratch. `dynamic_linking` is a dev-profile feature and never appears in a release build, so the
release row has no second column.

| Profile | Clean | Incremental, after touching `src/main.rs` |
|---|---|---|
| dev, no `dynamic_linking` | 1,301 s (21.7 min) | 11 s |
| dev, `--features dynamic_linking` | 1,310 s (21.8 min) | **4 s** |
| release | 771 s (12.9 min) | 6.2–7.3 s |

`dynamic_linking` costs nothing at the clean build and takes the edit-compile-run loop from 11 s to 4 s,
which is the whole point of it: it moves the engine out of the final link. The dev profile is slower than
release to build clean because `[profile.dev.package."*"] opt-level = 3` optimises every dependency and
still emits debug info; that setting is what keeps a debug-built viewer usable at interactive frame
rates. The release rebuild beats the plain dev rebuild for the same reason it links less debug data.

## CI

Run 35543162641 on `overnight/2026-09-19`, commit 4d36d6d, ubuntu-latest, with no warm cache — the
`ecoview-native` job's first run ever, so nothing was restored.

| | |
|---|---|
| Mesh golden, `--no-default-features` | **6 s including compile** |
| Same test locally, from a cold `--no-default-features` target | 11 s including compile, 0.03 s to run |
| Lavapipe screenshots, first attempt | 0/10 — the viewer did not build |

The five golden tests are `golden_flat`, `golden_stepped`, `golden_building`,
`cell_size_comes_from_the_bundle` and `a_tree_adds_geometry`. Six seconds is what the engine-free split
buys: the gate compiles `serde`, `serde_json` and `binary-greedy-meshing`, and nothing else.

The gate held at **2 s** on the two runs after it, once the cache was warm.

### Lavapipe screenshots

Three runs were needed to get a number, and each failure was in the runner's setup rather than in the
viewer:

| Attempt | Result | Why |
|---|---|---|
| 35543162641 | 0/10 | the viewer did not build: `wayland-sys` needs `libwayland-client` |
| 35543758425 | 0/10 | built, but "Unable to find a GPU" — no Vulkan driver was loaded |
| 35545818911 | **9/10, ~240 s each** | the ICD on Ubuntu 24.04 is `lvp_icd.json`, not `lvp_icd.x86_64.json` |

The nine that ran took 237, 238, 239, 240, 240, 241, 242, 243 and 244 s and each wrote **exactly
815,129 bytes** — the stress world, 300 frames, 1280×800, rendered entirely on the CPU. The tenth did
not run: at ten runs of four minutes the step overran the job's 45-minute budget and the job was
cancelled, which turned the whole CI run red even though every step in it carries
`continue-on-error: true`. The budget is now 90 minutes.

(Shot V1: those 237-244 s were an AMD runner. A later Intel one drew the same scene in 136-142 s and
wrote a different number of bytes, and the whole of that is explained under "The lavapipe drift" below.
Shot C1 has since cut this step to one screenshot.)

So headless rendering does leave this machine: the same binary that draws at 292 fps on a 4090 draws the
same picture on a GitHub runner with no GPU at all, in four minutes a frame-set. That is the number to
weigh if the track ever wants screenshot tests in CI — four minutes per picture, not four seconds.
Every step of it is measurement: the mesh golden above is the only thing that can fail this job.


## The three gates

| Gate | Threshold | Measured | Verdict |
|---|---|---|---|
| Frame rate | ≥ 60 fps flying the full stress world | 292.3 fps mean, 251.2 fps at the 95th percentile | **PASS** |
| | full remesh < 1 s | 53 ms (stress, 24 threads); 419 ms single-threaded | **PASS** |
| | single-edit remesh < 10 ms | 3.70 ms median (stress), 3.52–3.80 ms (Capitol), 4.79 ms worst | **PASS** |
| Agent loop | launch → camera → three edits → screenshot → PNG read back, no human, ≤ 3 schema retries | completed, 8 calls, **0 retries**, 3.3 s to the first screenshot | **PASS** |
| CI | mesh golden on ubuntu-latest under 60 s | **2–6 s** including compile | **PASS** |

All three gates pass, and none of them is close to its threshold: the slowest is the frame rate at four
times the required rate, on a world with four times the Capitol's triangles.

## What this spike does not tell you

- **One machine, one GPU.** Every frame-time number is an RTX 4090 on Vulkan. The lavapipe row is the
  only other hardware measured, and it is a CPU rasteriser four orders of magnitude slower.
- **The worlds are 512×512 columns.** Both of them. Nothing here says what happens at 4096².
- **Nothing is drawn but surface voxels, buildings and block trees.** No water, no animals, no overlays
  beyond surface type, no UI. The triangle counts are for that scene and no other.
- **The edit path is one column at a time.** A brush that edits a hundred columns has not been measured,
  though its cost is bounded by the full-remesh number above.
- **The frame rate was measured flying, not editing.** `--bench` moves the camera; it does not apply
  edits while it measures.

## Recommendation

**Continue the track.** The three gates pass with room, the agent loop needs no human and no MCP
installation, and the CI gate is engine-free and two seconds long — which means the mesher can be
regression-tested forever without a GPU in the loop.

Two things the operator should decide before V1, neither of which is this shot's to settle:

1. **The in-process linking question.** V4 in `overnight/VIEWER-TRACK.md` links the simulator crate,
   which CLAUDE.md's "share no code and have no IPC" forbids. Nothing in this spike prepares for it
   (V0-spike.md, correction 2). That constraint needs amending, or V4 needs replacing with a run-directory
   reader, before the plan past V3 means anything.
2. **What the lavapipe step is worth.** Four minutes a screenshot, ten screenshots, is 40 minutes of
   runner time on every push for a measurement nothing gates on. It earned its place here by proving the
   headless path is not Windows-only; keeping it at ten runs afterwards is a cost decision.

## Line budget

`git diff --stat 4e9a42e -- ecoview-native/` is 8,308 insertions, of which **6,197 are `Cargo.lock`** — a
lockfile, which CLAUDE.md's budget rule excludes along with manifests and regenerated output. What
remains is **2,111 lines: 1,787 of Rust, 52 of manifest and config, and 272 of these two write-ups.**

That is **over the shot's 1,500-line budget by 611 lines**, or by 339 counting code and config alone,
and it is reported rather than trimmed: the overage is in `src/main.rs` (575) and `src/voxel.rs` (343), which between them hold the viewer, both
camera modes, the headless path, the benchmark, the three BRP methods and the voxel world with its edit
rules — all of it named by the shot prompt's build list, none of it optional to the measurements above.
Cutting 339 lines of it would have meant dropping something the shot asked for; the rest is this file
and `DECISIONS.md`, which the acceptance list requires. The operator should know the spike cost 1.4×
its budget.

---

# ecoview-native V1: time, and a number that drifted for two shots

Same machine as above. The run is `ecosim/runs/capitol-s42` — seed 42, 20,000 ticks, a snapshot every
1,000, so 21 snapshots — laid over the Capitol bundle it was computed on.

| Snapshot | Tick | Trees drawn | `entities.json` |
|---|---|---|---|
| 0 | 0 | 79 | 7 KB |
| 1 | 1,000 | 334 | 30 KB |
| 11 | 10,000 | 998 | 91 KB |
| 20 | 20,000 | (all of them) | 380 KB |

## Changing snapshot

The cost of moving one snapshot is reading `entities.json`, replacing every plant voxel in the world
and remeshing the chunks that changed. `ecoview.stats` reports the last one as `snapshot_ms` and
`snapshot_chunks`; the HUD prints the same pair.

| Where | Plants before → after | Chunks stale | Measured |
|---|---|---|---|
| At startup, `--run` with no `--tick` | bundle's trees → 79 | 162 | **0.44, 0.45, 0.46 ms** |
| At startup, `--tick 10000` | bundle's trees → 998 | 162 | **1.8 ms** |
| Scrubbing, snapshot 0 → 1 | 79 → 334 | 150 | **14.27, 14.83, 14.27, 15.55 ms** |

Four runs of the agent loop produced those figures, so the spread inside each row is under 10% and the
numbers are stable. **The gap between the rows is not.** A scrub to 334 trees costs eight times a
startup load of 998, and tree count is therefore not what it is made of — the two rows run the same
`Run::trees_at` and the same `VoxelWorld::set_plants`, differing only in that the startup measurement is
taken before the first frame and the scrub one inside a running frame. That is where to look, and this
shot did not look: 14 ms is a thirty-fifth of the 500 ms play rate and a fifth of what the remesh after
it costs, so nothing the user can feel depends on the answer. It is written down here rather than
rounded off, because a number nobody writes down is a number nobody corrects — which is the rest of this
page.

Remeshing 150 stale chunks after a scrub is what actually dominates a snapshot change, and it is the
same meshing measured in V0: the full Capitol rebuild is 53 ms of 24-thread work for 324 chunks.

## The agent loop, now with a timeline

`agent_loop --run ../ecosim/runs/capitol-s42` does everything V0's did and then drives
`ecoview.timeline` five times — seek to the last snapshot, seek to the first, step forward one, play,
pause — and re-reads `ecoview.stats` to prove the world moved.

| | V0 | V1 |
|---|---|---|
| Calls | 8 | **14** |
| Retries | **0** | **0** |
| Time to the first screenshot | 3.3 s | **2.7 s** (22–23 ms for the call itself) |
| Screenshot | 1280×800 | 1280×800, 888,441 bytes |

Five more methods' worth of surface and still no retry. The fourth explicit method, `ecoview.timeline`,
is the decision from V0 applied again: an agent is given one documented name that takes `snapshot`,
`tick`, `step` or `playing`, rather than a `Timeline` component to discover.

## The lavapipe drift, explained

Shot C1 found that the lavapipe screenshot figures in circulation — 136–142 s and 811,550 bytes each —
did not match what its own runs measured: 259–267 s and 815,129 bytes, ten times out of ten on b48e7bd,
and 273 s and 815,129 on the one screenshot it kept. V1's row asked for the answer or an admission that
there isn't one. **There is an answer, and the picture did not change.**

**First, where the figures were.** Not in this file. V0 measured the 9/10 attempt at 237–244 s and
815,129 bytes and that is what the CI section above records; the 10/10 confirmation at 136–142 s and
811,550 bytes was taken after V0's final commit and written only into `overnight/LOG.md` (line 301),
whose own last sentence says so. `.github/DECISIONS.md` and `.github/workflows/ci.yml` then cited it as
if it were in MEASUREMENTS.md. So the file was never 1.9× stale about its own measurement — it was
missing one, and two later readers filled the gap from a log line.

**Then, why the two differ.** The `ecoview-native-shots` artifacts from three CI runs were downloaded
and their logs read. Bevy prints the runner's CPU in its own `SystemInfo` line:

| Run | Commit | Runner CPU | Per screenshot | PNG bytes | voxelise | mesh |
|---|---|---|---|---|---|---|
| 35548275114 | 34cfa2e | Intel Xeon 6973P-C | 136–142 s | **811,550** | 174 ms | 251 ms |
| 35556941399 | 2a7a0de | AMD EPYC 7763 | 262–272 s | **815,129** | 249 ms | 337 ms |
| 35562043184 | b48e7bd | AMD EPYC 7763 | 259–267 s | **815,129** | 258 ms | 339 ms |

Everything else is identical across the three: Mesa 25.2.8-0ubuntu0.24.04.2, llvmpipe on LLVM 20.1.2 at
256 bits, the stress world at 405 chunks with 247 drawn and **363,806 quads**, and — checked
character by character — the same shell command in the lavapipe step on 34cfa2e and b48e7bd. The PNGs
are **byte-identical within a CPU type**: md5 `e97cbad9…` on the Intel run across its own ten
screenshots, `a0dc62c6…` on both AMD runs, on different days. This is C2's runner lottery, which drew
Intel once and AMD twice.

**And the picture is the same picture.** Differencing the Intel PNG against the AMD one:

```
pixels 1024000, differing 20014 (1.954%), max channel delta 1, mean delta over differing 1.00
```

Two per cent of the pixels are off by exactly one in one channel, and none by more than that. llvmpipe
JIT-compiles its shaders for the host's vector width, so the two CPUs round the last bit of a few
interpolated values differently; PNG's row filters and deflate turn 20,014 ±1 changes into a 3,579-byte
size change, because a filtered row that was previously a repeat no longer is. **The file size is a
function of the runner's CPU, not of the code.** Nothing about what is drawn changed between 34cfa2e
and b48e7bd, and a size change of that scale is not evidence that anything did.

The 1.93× time ratio has the same cause and is consistent with the rest: the pure-Rust meshing on the
same two runners is 251 ms against 337–339 ms, a ratio of **1.34×**, which is C2's measured 1.36×
machine spread. Rasterisation stretches that to 1.93× because lavapipe is entirely CPU vector work and
has nothing else to be limited by.

**What to take from it.** PNG size is not a signature for this measurement and should never have been
read as one; per-screenshot time is only comparable between runs that drew the same CPU. Anything
watching those screenshots for change has to compare pixels — and to be worth anything, has to allow a
delta of 1.

One more byte count is coming, and this one *is* the code: V1 draws a HUD over every screenshot, so the
stress shot the smoke check takes is a different picture from V0's on purpose — 827,801 bytes on this
machine, and 827,146 in the build before it, whose only difference was that the empty timeline bar was
still drawn. A dark strip 1,232 px wide was worth 655 bytes; suppressing it exposes noisier terrain
underneath and the file gets *bigger*. Whatever lavapipe writes next will match neither 811,550 nor
815,129, and this time the reason is in the diff.

**And it did not, on a third CPU.** V1's own CI run (35612609960, commit 7282cc1) drew the smoke check in
**182 s at 835,373 bytes** on an **AMD EPYC 9V74** — a machine neither of the runs above drew. It sits
between the other two in both halves, and in the same order:

| Runner CPU | voxelise | mesh (pure Rust, 405 chunks) | 300 frames on lavapipe |
|---|---|---|---|
| Intel Xeon 6973P-C | 174 ms | 251 ms (1.00×) | 136–142 s (1.00×) |
| AMD EPYC 9V74 | 213 ms | 285 ms (1.14×) | 182 s (1.31×) |
| AMD EPYC 7763 | 249–258 ms | 337–339 ms (1.35×) | 259–267 s (1.89×) |

Three machines, three times, and rasterisation stretches the CPU's own ratio every time. The lottery has
at least three tickets, so a per-screenshot time is a reading of the runner unless the CPU is quoted
beside it.


# ecoview-native V2: what six overlays cost, and what they actually show

All of it on this machine (Windows 11, a 12th Gen Intel Core i9-12900KF, RTX 4090 -- see the V3
section's correction), release build, headless, against the
committed Capitol bundle (512 × 512 ground cells at 0.5 m) and `ecosim/runs/capitol-s42` at tick
10000 unless another run is named. Each figure is one `ecoview-native --headless --frames 300` run,
read off its own stdout.

## Quads, and the greedy-meshing bill for banding the ground

| Overlay | Quads | Full-site mesh | Against surface |
|---|---|---|---|
| surface | 79,504 | 23 ms | — |
| light | 78,044 | 25 ms | 0.98× |
| moisture | 97,046 | 24 ms | 1.22× |
| fertility | 96,370 | 23 ms | 1.21× |
| temperature | 75,921 | 23 ms | 0.95× |
| crowding | 75,518 | 24 ms | 0.95× |
| fire | 76,068 | 23 ms | 0.96× |

The quad counts are deterministic; the mesh times are one pass each and drift 23–38 ms run to run on
an otherwise busy machine, so the ratio column is the quads and not the milliseconds.

The worst overlay costs **22% more quads** than the surface map, and three of the six cost *less*.
That is the answer to the question the banding was designed around: a field with large smooth
neighbourhoods (temperature, which is per patch; crowding, which is empty on this run; fire, which is
two flat colours over most of the site) merges *better* than the scene contract's media, because the
media map has a path, a kerb and a flowerbed in it. Only moisture and fertility, which vary column to
column along drainage lines, pay for the resolution they show. A per-column colour instead of 32
bands would have cost 262,144 quads — 3.3× the surface map — on every overlay.

## Remeshing when the tick moves

Jumping to tick 10000 remeshes **162 of 324 chunks in 2.1–2.9 ms** under every overlay (the HUD line
in all seven screenshots). Switching the overlay itself is the expensive direction and is not
incremental: it changes the top voxel of every ground column, so all 324 chunks rebuild, which is the
whole-site mesh in the table above. Both are far below a frame the user would notice, so neither was
optimised further.

## What the fields actually held

The point of printing these is that an overlay's picture cannot distinguish "flat field" from "no
data", and three of the six are flat on the reference run.

| Overlay | Scale, and where it came from | Field at tick 10000 |
|---|---|---|
| light | 0–1 of full sun (`light.bin`'s own unit) | 0.00–1.00, mean 0.83 |
| moisture | 0–1 of AWC (`moisture.bin`'s own unit) | 0.00–1.00, mean 0.22 |
| fertility | 0–255 index (deferred in UNITS.md) | 0.00–168.00, mean 61.21 |
| temperature | −5–35 °C, from `params.{grass,shrub,tree}.temp` | 10.31–12.00, mean 11.86 |
| crowding | 0–32 grazers, from 2 × `params.disease.grazer_threshold` | 0.00–0.00 |
| fire | 0–3 ticks left, from `params.fire.duration` | 0 alight, **155 patches burnt** since tick 9000 |

Three readings worth keeping:

**Crowding is empty on every Capitol run, and that is the run, not the overlay.** Bundle-world runs
carry `--set animals.enabled=false` (ecosim DECISIONS, shot G3a), so there are no grazers to count. To
see the overlay work at all, this shot made a 2,000-tick animals-on Capitol run locally
(`runs/capitol-s42-animals`, 15.4 s, gitignored): 9,204 grazers, 28 hunters, 24 trees left standing,
and a field of **0–95 grazers per patch, mean 8.99** against a 0–32 scale, so the busiest patches sit
clamped at the top of the ramp. `shots/v2-crowding.png` is that run. The clamp is correct — the scale
is the simulator's disease threshold, not the data's maximum — and the HUD prints 95 beside it.

**Temperature spans 1.7 °C inside a 40 °C scale**, so the site is one shade of magenta. The scale is
the union of the species' tolerance curves, which is the interval where a colour means anything about
growth; stretching it to the data would make the same colour mean different things at different
ticks. The stats line carries the real range.

**Fire's field is zero at a tick with 155 burn scars on screen.** The field is ticks-left-alight and
the scars are a separate count, which is why the headless line prints both. At tick 1000 the same run
reads 1 patch alight, 8 burnt.

## The screenshots

Seven, all at 1280 × 800, all on the committed Capitol except crowding. One line each, written after
looking at them:

| File | Verdict |
|---|---|
| `v2-surface.png` | The V0/V1 picture unchanged — lawn, paths, asphalt, 998 trees — with the legend correctly absent. |
| `v2-light.png` | Reads as a shadow map: the Capitol's own shade is black to the north, each tree drops a grey square, open lawn is white at 1.00. |
| `v2-moisture.png` | The drainage network is the picture — blue threads along the flow paths, pale everywhere else, mean 0.22 of capacity. |
| `v2-fertility.png` | Faint brown mottling on a mostly white site: honest for a field whose maximum is 168 of 255, and the flattest of the six. |
| `v2-temperature.png` | One magenta site, as the 1.7 °C span predicts; useful only because the HUD says what the span is. |
| `v2-crowding.png` | The animals-on run: the patch grid is plainly visible, hot magenta where grazers pile up, and the site is stripped to 24 trees. |
| `v2-fire.png` | 155 dark burn scars in patch-sized blocks across the olive quiet ground, clustered in the eastern woodland where the fuel is. |

## The stress smoke check's PNG changed again, on purpose

`shots/stress-headless.png` is **835,546 bytes** on this machine against V1's 827,801. The HUD gained a
line — `overlay surface (1 of 7)   [1-7] switch` — and that is the entire difference; the world, the
camera and the 363,806 quads are V0's. Recorded because V1 spent a shot proving that this file's size
is a reading of the runner's CPU unless the code changed, and this time the code changed.

## What this shot does not tell you

Nothing here is a *correctness* check of the simulator's ecology. The viewer reads six files and
colours them; if `moisture.bin` were wrong, this would draw the wrong thing confidently. The
invariants live in `ecosim check`. What the overlays add is that a wrong field is now *visible* —
drainage lines that do not follow the terrain, or shade on the sunny side of a building, are the kind
of error no scalar in `series.csv` reports.


# ecoview-native V3: what a procedural tree costs, and the one input the run does not have

All of it on this machine (Windows 11, a 12th Gen Intel Core i9-12900KF with 16 cores and 24 threads,
RTX 4090, Vulkan), release build, headless, against the committed Capitol bundle (512 × 512 ground
cells at 0.5 m) and `ecosim/runs/capitol-s42`. Every figure is read off one
`ecoview-native --headless` run's own stdout.

## Correction: the V2 section named the wrong machine

V2's header says "Ryzen 9 5950X, RTX 3080". This machine is the i9-12900KF and RTX 4090 that V0's
header names — the same clerical slip shot V0a existed to fix, made again one shot after it. The V2
numbers are not wrong; they were all taken here. Only the name beside them was. Corrected in place,
with a pointer to this note.

## The model, at three ticks

| Tick | Trees (sapling / young / mature) | Height, median | Crown light | Wood voxels | Leaf voxels | Voxelise | Full-site mesh | Quads |
|---|---|---|---|---|---|---|---|---|
| 0 | 79 (0 / 0 / 79) | 4.7–20.0 m, 13.9 | 0.14 flat | 4,940 | 74,319 | 16 ms | 32 ms | 76,513 |
| 10000 | 998 (321 / 352 / 325) | 0.0–20.0 m, 1.9 | 0.14–1.00, mean 0.50 | 21,830 | 292,966 | 60 ms | 32 ms | 162,966 |
| 20000 | 4,082 (1117 / 96 / 2869) | 0.2–20.0 m, 10.6 | 0.14–1.00, mean 0.38 | 185,963 | 2,085,047 | 557 ms | 43 ms | 798,340 |

"Voxelise" is stamping every tree's limbs and leaf blobs into the chunk grid; "full-site mesh" is all
324 chunks, 93 of which hold geometry. The tick-20000 column is the worst case the reference run
reaches: **2.27 million plant voxels, and 0.6 s to put them there**.

## What the branches cost against V0's trunk-and-ellipsoid

| | V0/V2 solid | V3 branches | Ratio |
|---|---|---|---|
| Surface quads, tick 10000 | 79,504 | 162,966 | 2.05× |
| Light-overlay quads, tick 10000 | 78,044 | 161,501 | 2.07× |
| Snapshot change (162 of 324 chunks) | 2.1–2.9 ms | 13.5 ms at tick 0, 62.3 ms at 10000, **537.2 ms at 20000** | up to 200× |

Two quads for one, and the snapshot-change path is the one that moved: V2 could scrub the timeline
inside a frame, and V3 cannot at the dense end. 537 ms is still inside V0's "a full remesh under 1 s"
gate, the scrub stays usable because the bar keeps drawing, and no optimisation was added for it —
this shot's job is the geometry, and a measured 0.5 s is a better thing to hand the next shot than an
unmeasured cache.

## Fuller crowns cost *fewer* quads

The rejected light sample (below) is also the clearest measurement of the greedy mesher's shape
sensitivity, because it produced thinner crowns on the same skeletons:

| | Leaf voxels at tick 20000 | Quads |
|---|---|---|
| Thin crowns (crown-centre light) | ~1.47 million | 1,017,523 |
| Full crowns (surface light) | 2,085,047 | **798,340** |

**42% more leaf voxels, 22% fewer quads.** An isolated voxel is six quads that cannot merge with
anything; a solid cluster is a box. V0 measured this on noise terrain and it holds for foliage: the
expensive crown is the sparse one, so making trees leafier is close to free and thinning them is what
costs.

## Age is the only size the simulator owns, and at tick 0 it agrees with the survey

The bundle's survey of the Capitol's trees and the viewer's age-to-height curve are independent paths
to the same 79 trees: the survey height goes into `entities.json` as an age (ecosim's `import_age`),
and the viewer inverts that curve to get a height back.

| | Trees | Height span | Median |
|---|---|---|---|
| `worlds/capitol/trees.json` (surveyed) | 81 | 4.675–23.59 m | 13.77 m |
| Viewer at tick 0, from `age` | 79 | 4.7–20.0 m | 13.9 m |

The round trip is exact below the curve's ceiling and saturates above it: `params.bundle`'s
`tree_tall_height` is 20 m, so the one surveyed 23.6 m tree comes back 20 m and cannot do otherwise —
the simulator stores no height, only an age, and every age above the tall breakpoint maps to 20 m.
Two of the 81 are not in the run at all (they stand outside the ecology grid). A unit test round-trips
the curve to within 0.02 m.

## Measured and rejected: light sampled at the middle of the crown

The obvious sample is `light.bin` where the leaves are — the centre of the procedural crown, 13.7 m up
on a 20 m tree. Measured over the reference run, that reads **1.00 of full sun for every tree at every
tick**, in all 21 snapshots. The cause is not a bug in either project: the simulator's canopy is one
to three voxels of `CANOPY` above the surface, so 13.7 m is sky, and the light field is only
interesting in the voxel the simulator itself reads. The sample is therefore ecosim's own
`surface_light` — `light[height[c] + 1]` — which is the number its germination and growth curves use.
That gives the 0.14–1.00 spread in the table, and the flat 0.14 at tick 0 is itself a reading: every
surveyed tree stands in its own shade.

## Measured and rejected: the crown clamped per axis

The first envelope clamp was per axis — x and z to the crown radius, y to the tree's height — and
every tree over 10 m rendered as a bare post. Three generations of branch, each rising about
three quarters of its own length, land all 27 tips at the apex, where the ellipsoid is a point, so the
leaf-clip test threw their blobs away. Pulling a tip back along the envelope's own radius instead, to
`TIP_FRAC` = 0.78 of the wall, and lowering the upward bias from 0.30 to 0.18, took tick-0 leaf fill
from **25,853 to 74,319 voxels — 2.9×** with no change to the skeleton.

## The screenshots

Five, all 1280 × 800, all on the committed Capitol. One line each, written after looking at them:

| File | Verdict |
|---|---|
| `v3-capitol-t0.png` | The 79 surveyed trees as recognisable trees — brown trunks, limbs, rounded crowns at their surveyed heights — scattered over the lawn and along the paths, with the Capitol and its dome untouched behind them. |
| `v3-capitol-t10000.png` | The mixed-age stand reads as one: 325 mature crowns, 352 young ones half their size, and 321 saplings as bare poles a metre or two high, which is exactly what the stage counts say. |
| `v3-capitol-t20000.png` | A closed woodland over two thirds of the site — crowns merging into canopy, trunks visible underneath, the lawn surviving only where the paths and the building are. |
| `v3-light.png` | Shows what the crowns are doing to the ground rather than what the ground is doing: white lawn at 0.83 mean, a black wedge north of the building, and a grey square under every tree. |
| `v3-tree-closeup.png` | The point of the shot, at eye level: separate trunks with visible limb structure holding crowns overhead, the ground-cover mottling underfoot, and one legitimately bare 8 m trunk in the middle distance whose crown starts at 0.37 of its height. |

## The stress smoke check's PNG changed again, on purpose

`shots/stress-headless.png` is **839,136 bytes** against V2's 835,546, and its scene now meshes to
**335,544 quads** against V2's 363,806. The stress world's 200 trees are branching now, so unlike V2's
change (a HUD line) this is a real geometry change — and it is the "fuller crowns cost fewer quads"
result again, on a world with no run over it and at 0.25 m cells.

`shots/agent-loop.png`, the agent gate's own capture at tick 1000, changed the same way and for the
same reason: **1,043,886 bytes** against V1's 888,371, with 334 trees (115 sapling, 140 young, 79
mature), 8,858 wood and 152,664 leaf voxels. The gate itself is unmoved — 14 calls, 0 retries, PASS,
3.5 s to the first screenshot, and the scrub it does mid-run remeshes 150 chunks in 46 ms.

## What this shot does not tell you

**Nothing here says a tree is the right size.** The row asks for geometry from "seed, species, age,
biomass and light", and biomass does not exist: no per-tree mass, diameter, leaf area or height is
written anywhere in the run directory — `entities.json` carries `id`, `x`, `y`, `z`, `age`, `stage`
and `lifespan`. Age through the simulator's own curve is the only dimensional quantity that is really
the simulator's, and everything else about a tree's shape here — how many limbs, how far they spread,
how big a leaf blob is — is expression, tuned by eye at these three ticks and held in place by a test
that only asserts leaves stay inside the envelope. A BACKLOG note proposes the ecosim row that would
publish a real per-tree size; until it exists, the HUD says whose numbers these are.


# ecoview-native V4: what ground cover and vines cost, and how much of the picture is the run's

All of it on this machine (Windows 11, a 12th Gen Intel Core i9-12900KF, RTX 4090), release build,
headless, against the committed Capitol bundle (512 × 512 ground cells at 0.5 m) and
`ecosim/runs/capitol-s42`. Each figure is one `ecoview-native --headless --frames 300` run, read off
its own stdout.

**Everything measured here is expression, not simulation.** The run owns four numbers — the grass
and shrub fraction of each 8 m patch, and the moisture and light of each 1 m column. This viewer
owns which of a patch's ground cells carry a blade, how tall a shrub is drawn, and how far a climber
gets up a wall. No vine is an entity in any run, nothing here competes for anything, and nothing
feeds back into the simulation. Every number below should be read in that light.

## The cover at three ticks

| | tick 1000 | tick 10000 | tick 20000 |
|---|---|---|---|
| grass voxels | 138,592 | 99,262 | 73,259 |
| shrub voxels | 48,204 | 172,107 | 248,454 |
| vine voxels | 7,690 | 5,746 | 11,012 |
| run's grass fraction | 0.750 | 0.644 | 0.457 |
| run's shrub fraction | 0.082 | 0.279 | 0.393 |
| run's soil water | 0.343 | 0.219 | 0.663 |
| run's shade | 0.143 | 0.172 | 0.424 |
| vine vigour | 0.217 | 0.167 | 0.392 |

The story in the middle row is the simulator's, not the viewer's: grass gives way to shrub across the
run, and the picture follows it because it has no choice — the fractions are read, not modelled.

## The drawn fraction against the reported fraction

The Capitol bundle is **169,876 growable cells of 262,144** — 64.8% lawn, with 17.1% asphalt, 9.4%
roof and 8.7% concrete growing nothing. A shrub is three voxels tall at 0.5 m (`SHRUB_HEIGHT_M` 1.2
m), so its cell count is the voxel count over three, exactly, at every tick.

| Tick | grass cells / growable | run's grass | shrub cells / growable | run's shrub |
|---|---|---|---|---|
| 1000 | 0.816 | 0.750 | 0.095 | 0.082 |
| 10000 | 0.584 | 0.644 | 0.338 | 0.279 |
| 20000 | 0.431 | 0.457 | 0.488 | 0.393 |

Shrub is drawn **above** its reported fraction at all three ticks, and the cause is not the scatter.
The reported number is the mean over all 65,536 ecology columns, including the 35% of the site that
is pavement and building, where the simulator's cover is low. The drawn number is over the lawn
only. The two are answering different questions, and the gap between them is the simulator's own
pattern rather than a viewer error.

Grass runs the other way at ticks 10000 and 20000 — 0.584 against 0.644, and 0.431 against 0.457 —
and only clears its reported fraction at tick 1000, when shrub is still 8% of the site. That is the
partition: shrub takes the bottom of the interval, so where the two fractions sum past 1 it is grass
that loses the cell (DECISIONS.md, V4). Shrub is never clipped, which is why its two columns move
together and grass's do not.

## What the cover costs

| | tick 10000 | tick 20000 |
|---|---|---|
| quads, cover off | 162,966 | 798,340 |
| quads, cover on | 322,068 | 979,934 |
| cover's share | +159,102 (1.98×) | +181,594 (1.23×) |
| voxelise, cover off | 66 ms | — |
| voxelise, cover on | 81 ms | 593 ms |
| full-site mesh | 30 ms | 44 ms |

`--no-cover` at tick 10000 meshes to **162,966 quads — V3's number to the quad**, which is the
measurement behind the claim that this shot adds a layer and changes nothing under it.

The cover costs about 15 ms of voxelisation and doubles the quad count on an open site. It is much
cheaper proportionally on a closed one: at tick 20000 the trees already dominate, and the cover adds
23%. The absolute quad cost barely moves between the two ticks (159k against 182k) because it is
bounded by the growable ground, which does not change — only the cover's composition does, and a
three-voxel shrub costs more faces than a one-voxel blade, which is the whole of the difference.

A full-site remesh stayed at 30–44 ms throughout, so nothing here changed what the mesher costs per
quad.

## The vines are a small number because the Capitol is paved

**1,528 lawn cells** on this site stand orthogonally against a building — everything else at the
building's foot is concrete or asphalt, which grows nothing. That is 0.9% of the growable ground,
and it is why the vine counts above are more than an order of magnitude under the grass counts.

Per rooting cell that works out at 3.8 voxels (1.9 m) at tick 10000 and 7.2 voxels (3.6 m) at tick
20000, against the `vigour × 12 m` the model asks for: 2.0 m and 4.7 m. The drawn climb sits under
the nominal one at both ticks because vigour is read at each rooting cell, not at the site mean, and
the cells at the Capitol's walls are drier and sunnier than the average column. The tick-20000 gap
is the larger of the two for the same reason: that snapshot's variance is larger.

This is a case where the picture is honest by being unimpressive. A viewer that put a climber on
every wall would have looked better and said something the run does not.

## The screenshots

Five, all 1280 × 800, all on the committed Capitol. One line each, written after looking at them.
**The ground cover and the vines in all five are expression, not simulation** — see the note at the
head of this write-up.

| File | Command | Verdict |
|---|---|---|
| `v4-capitol-t10000.png` | `--run ../ecosim/runs/capitol-s42 --tick 10000` | The lawn has stopped being a flat green sheet: light grass mottled with darker shrub across the whole open ground, the paths and the forecourt still clean grey, and the HUD's "expression, not simulation" line sitting over it with the four drivers it used. |
| `v4-cover-off.png` | the same, plus `--no-cover` | The same tick with the layer off, for comparison: flat lawn, same 998 trees, same paths, `cover off [V] on` in the HUD — and 162,966 quads, exactly what V3 rendered. |
| `v4-vines.png` | `--tick 10000 --eye 152.0,13.0,84.0 --look 155.0,9.5,105.0` | The shot the row is about: a band of dark climbers along the base of the Capitol's wall with a ragged top edge, stopping dead where the lawn gives way to pavement — the sealed-ground rule visible in one frame — with shrub cubes standing in the grass in the foreground. |
| `v4-capitol-t20000.png` | `--tick 20000` | Closed woodland over most of the site, and the cover only legible in the clearing to the east, where shrub has plainly taken the ground from grass; the vines along the building's foot are twice the length they were at tick 10000. |
| `v4-moisture.png` | `--tick 10000 --overlay moisture` | The moisture map, unobstructed: the ground cover is off and the HUD says why — "(ground cover hidden under the overlay)" — while the 5,746 vine voxels stay, which is the point of keeping them. |

## The agent gate, and one committed PNG regenerated

`agent_loop --run ../ecosim/runs/capitol-s42`: **PASS**, 14 calls, 0 retries, 4.4 s to the first
screenshot, and `ecoview.stats` now answers with a `cover` object carrying the counts, the four
drivers and the sentence that says what they are.

`shots/agent-loop.png` is **about 1,319,000 bytes** against V3's 1,043,886. Its run gains a cover
layer — 138,592 grass, 48,204 shrub and 7,690 vine voxels at tick 1000 — so the 275 kB is a real
geometry change and not a HUD line.

**"About", because this PNG is not byte-reproducible, and never has been.** Two gate runs of the
same binary on the same run wrote **1,318,888 and 1,319,071 bytes**, 183 apart. The cause is in the
HUD, not the cover: the status bar renders `remesh {} chunks in {:.1} ms`, a wall-clock measurement,
so the pixels behind those digits change between runs. That text is V1's and the behaviour predates
this shot; V4 found it by regenerating the picture twice. It means the byte size of this file is
evidence of a change of hundreds of kilobytes and of nothing finer, and it is the second reason —
after V3's note that these PNGs are accidental plant-model goldens — that nothing should start
gating them without removing the timing from the HUD first. Handed up as a BACKLOG note.

`shots/stress-headless.png` is **byte-identical** at 839,136. The stress world has no run over it,
so there are no patch fractions and no fields, therefore no cover, and the HUD's cover line is not
printed at all when there is no run. Verified by rendering it again to a scratch path and comparing.

## A finding this shot did not cause and did not fix

`shots/capitol-headless.png` no longer matches what the viewer renders. Git says it was last written
in **shot V0**, and V1 added the timeline bar and V2 the overlay legend, so it has been stale for two
shots. V4 does not touch that picture — with no run loaded there is no cover, and the cover's HUD
line is skipped entirely — so it is left as it is and noted here and in BACKLOG rather than
regenerated inside a row that did not change it.

## A trap worth recording: rustfmt and `\`-continued string literals

Two of this shot's output strings were written with `\`-at-end-of-line continuations, which Rust
strips along with the following indentation. After `cargo fmt` both had been rewritten as single
lines with the indentation **baked in as literal spaces**, so the stdout line and the BRP `note`
came out with fourteen-space gaps in the middle of a sentence. V3's `trees:` line has the same scar,
which is how it was recognised. Both are now single long source lines, which rustfmt leaves alone.
The rule for this crate: do not use `\` continuations inside a format string.

## What this shot does not tell you

**Nothing here says a blade of grass is in the right place, because there is no right place.** The
simulator has no grass entity, no shrub entity and no climber; it has a fraction per patch and two
fields per column. Everything about *where* — which cell, how tall, how far up a wall — is this
viewer's, tuned by eye at three ticks and held in place by tests that only assert the realised
fractions match the run's and that the drivers move the picture in the right direction.

What the picture can be trusted for is the direction and the magnitude of change: grass giving way
to shrub between tick 1000 and tick 20000, vines twice as long on a wetter shadier site, and bare
pavement where the simulator's ground is sealed. Those are the run's. The rest is drawing.

# Shot V5 — edit, run, grow, in place

Everything below was measured on this machine (i9-12900KF, RTX 4090, Windows 11) against the
committed Capitol bundle, with `ecosim` built `--release` beside the viewer. **The ground cover and
the vines in every picture here are expression, not simulation** — V4's note still applies. What is
new in this shot is the other direction: the *terrain* in these pictures is the viewer's, and every
ecological number on top of it was computed by `ecosim` in its own process, from a world bundle the
viewer wrote to disk.

## What a round trip costs

| ticks | snapshot every | ecosim | snapshots | where |
|---|---|---|---|---|
| 1,000 | 100 | 0.9 s | 11 | the agent gate |
| 4,000 | 400 | 3.3–3.8 s | 11 | the five close screenshots |
| 20,000 | 2,000 | 25.6 s | 11 | `v5-site.png`, the SAD's full run length |

The viewer adds a bundle write (six files, about 1.5 MB), a run load and one full re-voxelisation:
324 chunks remesh in 220–400 ms at 4,000 ticks and 2.2 s at 20,000, where the site is closed
woodland. So a 4,000-tick round trip is about four seconds end to end and a full-length one about
half a minute — both inside what a person will sit through, which is why this runs on the main
thread with a polled child rather than on a worker.

An 11,200-edit rectangle (1,600 cells lowered six times and then paved) applies and remeshes in one
batch; the HUD's `remesh 324 chunks` line is the load after the run, not the edit.

## The experiment: dig a basin, then pave it

Three runs, same seed (42), same 4,000 ticks, same bundle, differing only in what the viewer did to
the ground first. The basin is ground cells 80–119 by 150–189 — a 20 m square of unbroken lawn —
lowered six times by 0.5 m, so 3 m deep.

| at tick 4,000 | control | dug, left as lawn | dug and paved |
|---|---|---|---|
| trees | 1,444 | **1,444** | 829 |
| basin moisture (0–255) | 244.0 | 244.0 | **0.0** |
| basin fertility | 80.8 | 76.5 | **0.0** |
| basin soil water | 143.62 mm | 143.62 mm | **0.00 mm** |
| basin ponded water | none | none | **214.85 mm mean, 2,786 mm deepest, 1,600 of 1,600 cells wet** |
| the 5 m ring around it, moisture | 244.0 | 244.0 | 252.0 |
| the ring, soil water | 143.58 mm | 143.58 mm | 148.41 mm |

**Digging a hole in a lawn changes the simulator's water not at all.** `moisture.bin` and
`water.bin` are byte-identical to the control at every snapshot out to tick 4,000, and
`soil_water.bin` and `fertility.bin` first differ at tick 1,250 in the fifth significant figure. The
tree count is identical at every one of the 4,000 ticks. What does change is `height.bin`,
`material.bin` (the cut faces) and `light.bin` — the hole shades itself — and that light difference
is what eventually moves fertility. Rain infiltrates lawn whatever shape the lawn is in.

**Paving the same hole is a different site.** Those 400 ecology columns hold no soil water from tick
0, the basin is ponded by tick 78, and by tick 400 it holds **10.1 mm of standing water on all 1,600
ground cells while the control holds none** — and tick 400 is before anything else about the two runs
has parted, so that one is cause and effect with nothing else in it.

## The honest limit on that comparison

The paved run's **rain schedule leaves the control's at tick 1,011** (19.4253 mm against 0.0000) and
the tree counts part at tick 1,033. One changed column reorders the draws the simulator takes, and
after that the two runs are two different weather histories, not two treatments.

So **829 trees against 1,444 is not what the basin did.** A 20 m square is 0.6% of this site and
cannot halve its forest; what it did was move a storm. Every causal claim above is therefore taken
either from the pre-divergence window (the tick-400 ponding, the byte-identical fields at tick 800)
or from the basin's own columns, where the mechanism is local and mechanical. The site-wide rows of
the table are reported because they are what the screenshots show, and they are labelled here so
nobody reads them as an effect.

This is worth a standing note for the track: **on this simulator an edit is not a controlled
experiment past about a thousand ticks.** A row that wants one will need either a fixed weather
sequence or many seeds.

## The one thing the viewer cannot draw

At tick 4,000 the paved basin holds an average of 214.85 mm of standing water, and the viewer paints
it as **the driest ground on the site** — `v5-water.png` shows it bone white under the moisture
overlay while everything around it is saturated blue. Both are honest readings of different files:
the overlay reads `moisture.bin`, which is soil water in the ecology columns, and a paved column has
none. `water.bin`, the ponded depth on the ground grid, has no overlay and no geometry in this
viewer at all.

The round trip is what makes this visible — before this shot there was no way to put water somewhere
the reference run does not have it. **A water overlay, and ponded water as drawn voxels, is the
largest single thing this viewer now measurably lacks.** Handed up to BACKLOG; it is not among this
row's four pieces and this shot does not invent it.

## The screenshots

Six, all 1280 by 800, all on the committed Capitol, all reproducible from the commands below. The
five close ones share `--eye 50,45,135 --look 50,4,85`; `$DIG` is
`--edit 80,150,119,189,LowerGround` six times and `$PAVE` is `--edit 80,150,119,189,SetSurface,6`.

| File | Command, after `ecoview-native --headless --frames 200` | Verdict |
|---|---|---|
| `v5-dig.png` | `$DIG $PAVE --eye … --look …` | The edit before anything has been run on it: a clean 20 m pit with brown cut soil at its lip and an asphalt floor, sitting in unbroken Capitol lawn with the bundle's own trees around it; the HUD reads `11200 edits, 512 undoable`, the crosshair names the column under it (`asphalt ground 3.10 m`), and the round-trip line says `idle [Enter] grow`. |
| `v5-grown.png` | `$DIG $PAVE --sim --sim-ticks 4000 --sim-root … --tick 4000` | The same pit after the 4,000 ticks the viewer asked for: grass and shrub have grown to the lip and stopped dead at the asphalt, saplings are scattered over the lawn outside it, and the HUD carries the whole provenance — the run directory, `4000 ticks in 3.5 s, 11 snapshots`, and the line saying the edit is the viewer's and the growth is ecosim's. |
| `v5-water.png` | the same, plus `--overlay moisture` | The finding in one frame: the basin reads white — 0.00 of available water capacity — inside a site painted saturated blue, which is both correct and incomplete, because the simulator has 215 mm of water standing in it that this viewer has no way to draw. |
| `v5-lawn.png` | `$DIG --sim --sim-ticks 4000 --sim-root … --tick 4000 --overlay moisture` | The control for the water claim: the same 3 m basin left as lawn disappears into the moisture map — only its two cut walls give it away — and the run behind it has the same 1,444 trees as the untouched site. |
| `v5-untouched.png` | `--sim --sim-ticks 4000 --sim-root … --tick 4000 --overlay moisture` | No edit at all, same camera, same tick: uniform blue where the basin would be, `0 edits, 0 undoable`, 1,444 trees. The picture that makes the other two mean something. |
| `v5-site.png` | `$DIG $PAVE --sim --sim-ticks 20000 --sim-root … --tick 20000`, default camera | The full-length round trip: 20,000 ticks in 25.6 s, 4,785 trees, the Capitol under closed canopy with the dome standing out of it. Honest caveat — at this zoom the basin is under the woodland and cannot be seen, so this picture is evidence that a 20,000-tick round trip works and draws, not evidence about the edit. |

## The agent gate, and two committed PNGs regenerated

`agent_loop` with **no `--run` argument at all**: **PASS**, 18 calls, 0 retries (2 of them polls
waiting on the round trip), 4.0 s to the first screenshot. The agent edits three cells, calls
`ecoview.sim {"ticks": 1000, "seed": 42}`, polls until the phase reads `grown`, and then scrubs the
timeline of a run **it caused**, where every previous shot's loop could only read a run somebody else
had made. `ecoview.stats` answers with the round trip's state, the crosshair's column, the undo depth
and the authorship sentence.

`shots/agent-loop.png` is **1,531,622 bytes** against V4's roughly 1,319,000, and its run is now a
1,000-tick round trip of the agent's own rather than `capitol-s42` — a different picture of a
different run, not drift. (V4's note stands: this file is not byte-reproducible, because the HUD
prints a wall-clock remesh time.)

`shots/stress-headless.png` is **873,792 bytes** against V4's 839,136. The scene is untouched —
**335,544 quads, 247 chunks drawn, both identical to V3 and V4** — and the 35 kB is HUD text: the
crosshair line, the editor line and the round-trip line, plus the crosshair in the middle of the
frame. Regenerated because this shot changed what that picture shows, which is the rule V3 and V4
used.

`shots/capitol-headless.png` is **still stale, now across four shots** — V0 wrote it, and V1, V2 and
V5 have each changed the HUD over it since. V4 left it and handed it up; this shot does the same
rather than fold four shots' worth of drift into a row that did not ask for it. The BACKLOG note
stands.

## Line budget

`git diff --stat 4d18d76 -- ecoview-native/` is **1,223 insertions and 48 deletions**, plus the new
untracked `src/sim.rs` at **347 lines**, which `--stat` cannot see: 1,522 net before the write-ups,
and **1,782 net with them, against the row's 1,500** — 282 over, 18.8%. The two write-ups are 260 of
those lines (143 here, 117 in DECISIONS.md), more than V1's 200, because this shot has a three-run
experiment and six pictures to account for.

The split is **1,134 non-test and 388 test**. Comments are 222 of the 1,223 added tracked lines, 18%,
and 79 of 347 in `sim.rs`, 23%, which is this component's usual density; `sim.rs` runs higher because
it documents a child process and a file another program reads, where the why is the whole value.

**The trim that would fit does not exist without cutting tests.** Dropping every test this shot adds
would land it at 1,394, under the limit and at the cost of the 388 lines the rule exists to protect;
nothing else in the shot is large enough to cut instead. MASTER's budget rule says to block
rather than trim tests to fit, so this shot blocks on the number and hands the operator the
arithmetic, exactly as V2 did. `overnight/shots/V5.BLOCKED.md` has the options.

# V6: the beauty pass

Every figure below is this machine (RTX 4090, release build), the committed Capitol bundle and
`ecosim/runs/capitol-s42` unless another world is named. `--no-ao --no-sky` is the control: it is
the picture V5 took, from the V6 binary.

## What occlusion costs, and what it buys

| World, tick | Quads, pass off | Quads, pass on | First mesh, off | on |
|---|---|---|---|---|
| Capitol, tick 10000 | 322,068 | 452,385 (+40.5%) | 30 ms | 39 ms |
| stress world | 335,544 | 452,272 (+34.8%) | 50 ms | 63 ms |

The extra quads are all boundary: a run of lawn at one shade still merges into one quad, and only
the seam between two shades splits. 40% is the price of the merge key being the voxel id, which is
also the reason the mesh is this small to begin with (DECISIONS.md, V6).

**Frame rate, flying the full stress world for 10 s** (`--stress --bench 10`, the V0 gate of 60 fps):

| | mean | p95 |
|---|---|---|
| beauty pass off | 4.44 ms, **225.5 fps** | 5.55 ms, 180.2 fps |
| on, with cascaded shadows | 4.95 ms, **201.8 fps** | 6.21 ms, **161.0 fps** |

Shadows and 35% more quads cost 0.51 ms a frame. The gate wanted 60; the p95 is 161.

## What a season step costs

Seasonal colour is baked into vertex colours, so crossing a season step is a full-site remesh.
Measured over BRP, scrubbing `capitol-s42` from snapshot 10 to 11 (tick 10000 to 11000):

| | chunks remeshed | total |
|---|---|---|
| `--no-ao --no-sky` | 162 | 327.7 ms |
| `--no-sky` (occlusion on, season off) | 162 | 335.3 ms |
| beauty pass on, crossing 16 season steps | 324 | 395.3 ms |

**+68 ms, +21%, and only when the step actually moves.** This run is the worst case for it:
`snapshot_every` is 1000 against a `year_len` of 4000, so every snapshot is a quarter of a year and
every scrub crosses sixteen steps. The reference `runs/s42` snapshots every 100 ticks, 0.16 of a
step, so most of its scrubs cross none and cost exactly the V5 number. An hour key, a camera move
and a HUD tick never remesh at any snapshot rate, which is what the quantising is for.

That also means this run only ever shows four days of the year: tick 0 is 22 March, and the
snapshots land on 22 June, 21 September and 21 December and then repeat. The seasonal colour is real
and the dates are the run's, but this run samples the year four times, not sixty-four.

## The screenshots

Seven, all 1280 by 800. The six new ones are one `ecoview-native --headless --frames 120 --run
../ecosim/runs/capitol-s42` each, with the flags in the table; the default camera unless named.

| File | Flags | Verdict |
|---|---|---|
| `v6-off.png` | `--tick 10000 --no-ao --no-sky` | The control, and the case for the shot: V5's picture, flat dark green on black, every face of every tree lit identically, no shadow anywhere, the building a grey slab. Nothing is wrong with it and nothing in it reads as depth. |
| `v6-on.png` | `--tick 10000` | The same frame, same tick, same trees: a pale sky behind the site, tree shadows lying north-west across the lawn, the crowns split into lit gold and shaded olive, and the HUD saying `21 September, autumn` with `42.7 N, the hour, the latitude and the hue are the viewer's, the date is the run's`. The site reads as a place with a time of day. |
| `v6-close.png` | `--tick 10000 --eye 40,40,20 --look 140,6,140` | Ground level, looking at the Capitol's west face: the building shadows itself down one wall, the colonnade's recesses are dark where occlusion put them, and two foreground crowns in full sun have gone gold against their shaded neighbours. The one picture where all three halves of the pass are visible at once. |
| `v6-summer.png` | `--tick 9000` | 22 June, sun 59 degrees up, shadows short and directly under the crowns; canopy at its base green, the lawn at its lightest. 3,867 trees -- a different tick, so the wood is different too; the season here is the colour, not the count. |
| `v6-winter.png` | `--tick 11000` | 21 December, sun 18 degrees up: shadows reach half across the lawn, the grass has gone straw-olive and the canopy grey-green and dull. Every leaf is still on the trees, which is correct and is the one thing about this picture to be careful with -- the viewer does not model leaf fall and does not pretend to. |
| `v6-dusk.png` | `--tick 10000 --hour 17.75` | Sun 3 degrees up, fifteen minutes from setting: the sky burns salmon, the west faces catch the last of it and everything else is in shadow that runs the full length of the site. Honest limit -- the warm band is the whole horizon ring, not a glow around the sun's own azimuth, so the sunset looks the same in every direction. |
| `shots/stress-headless.png` | `--stress --frames 200` | Regenerated: 486 chunks, **247 drawn, unchanged from V3-V5**, now 452,272 quads instead of 335,544, under a blue sky with the tower bases and the ground between them darkened. The HUD carries the new sky line and says `no run loaded, so the date is the viewer's too`, which is the right thing for a world with no year. |

`shots/agent-loop.png` is **1,556,163 bytes** against V5's 1,531,622, regenerated by the gate run
below; `shots/stress-headless.png` is **970,259** against 873,792. `shots/capitol-headless.png` is
**still stale, now across five shots** -- V4 and V5 each left it and handed it up, and this row does
the same rather than fold five shots of drift into one that did not ask for it.

**Owed to the operator session:** the private home scene has not been photographed under this pass.
A worker never does that pass and never looks at that data; it is the operator's to run.

## The gates

`cargo test --release --no-default-features --test mesh_golden`: **58 pass**, 50 of them V0-V5's,
unchanged. All six earlier golden hashes stand untouched with occlusion off, which is the claim this
shot rests on. The eight new ones are the occluded golden, the off-is-V5 proof, what occlusion does
to a wall foot and a roof, the shaded palette's shape, solar geometry against the almanac at three
days and four hours and two hemispheres, the tick-to-day mapping, the season's continuity over all
64 steps, and the dome's winding and vertex count.

`cargo test --release`: pass. `cargo fmt --check`: clean. `cargo clippy --release --all-targets`:
the three pre-existing `src/bundle.rs` findings from V1 and nothing else; this shot adds none.

`agent_loop --run ../ecosim/runs/capitol-s42`: **PASS**, 19 calls, 0 retries, 4.7 s to the first
screenshot. `ecoview.stats` now carries a `sky` object -- the sun's elevation, azimuth and
declination, the day of the year, whether it came from the run, the season's three numbers and the
step, and the sentence saying which half is whose.

## One thing this shot changed that it was not asked to

The HUD text block got a dark translucent backing. White text on black needed none; white text on a
bright sky is unreadable, and the top left of the frame is exactly where the sky is brightest. Every
screenshot in this shot would otherwise have an illegible HUD, which would have undone what V2, V4
and V5 spent lines on.

## Line budget

`git diff --stat 052aa61 -- ecoview-native/` is **1,494 insertions and 14 deletions, 1,480 net,
against the row's 1,500** -- 20 to spare, with both write-ups in it. The new `src/sky.rs` is 568 of
them, `src/main.rs` 368, the gate 301, and the two write-ups 167.

The split is **1,193 non-test and 301 test**. Of the 1,335 code lines added, 309 are comments, 23%,
which is this component's usual density; `sky.rs` runs higher because every constant in it is a
number no file in the project could supply, and the reader's first question about each one is whose
it is.

# S2 -- the ramp hues, read from the run

The reading half of `ecosim` shot S2. One measurement, and it is a negative: **the colours do not
change, only their provenance does.**

The same frame, twice: the Capitol at tick 10000 under the moisture overlay, once against the
20,000-tick run on disk from before the shot and once against a fresh one written by the new
simulator, `ecoview-native --world ../ecosim/worlds/capitol --run <dir> --tick 10000 --overlay
moisture --headless --frames 120`.

| Run | The viewer's own stdout |
| --- | --- |
| before S2 | `overlay moisture: 0.00..1.00 of available water capacity from moisture.bin: 255 x soil water / AWC; field 0.00..1.00 mean 0.22; ramp #ffffff to #1f4fd1 from this viewer's fallback: meta.json has no overlays.moisture` |
| after S2 | `overlay moisture: 0.00..1.00 of available water capacity from moisture.bin: 255 x soil water / AWC; field 0.00..1.00 mean 0.22; ramp #ffffff to #1f4fd1 from meta.json overlays.moisture` |

Same two hues, so the same 32 band colours, so the same ground. The two PNGs differ in **1.88% of the
frame (19,206 of 1,024,000 pixels), every one of them inside the HUD's text block** (the difference's
bounding box is x 19..1139, y 10..375, which is the HUD panel; the source string on the new line is
longer). Below it the picture is pixel-identical, which is why **no new reference screenshot was
committed and none was re-accepted**: `shots/v2-moisture.png` still shows what the viewer draws.

On the simulator's side the same claim holds for the run itself: a fresh 20,000-tick Capitol run
against the one written before the shot gives `ecosim diff` -> `differs: meta.json`, nothing else.

`cargo test --release --no-default-features --test mesh_golden` is **60 pass**, V6's 58 plus the two
this shot adds; every one of the 50 mesh golden hashes is untouched, because a palette colours a mesh
and does not shape one.

# S5 -- standing water

Backlog row S5, no prompt file: the row is the specification. Everything below is
`runs/capitol-s42`, the committed 20,000-tick Capitol run, read through
`ecoview-native --world ../ecosim/worlds/capitol --run ../ecosim/runs/capitol-s42`.

## What was in the file the viewer was not reading

`water.bin` is 512 x 512 u16 in tenths of a millimetre, half a megabyte a snapshot, written every
snapshot since `ecosim` shot G4. At tick 10000:

| | |
| --- | --- |
| wet ground cells | **13,206 of 262,144** (5.0% of the site) |
| deep enough to draw (>= 5 mm) | 10,469 |
| depth: median / q0.90 / q0.99 / max | **10.0 / 47.7 / 480.3 / 5,074.1 mm** |
| standing volume | 166.2 m3 |
| mean over the whole site | 2.54 mm |

The median-to-maximum spread of 500x is the whole argument for a log ramp. A linear ramp to 5,074 mm
gives the median wet cell band 0 of 31 -- the same band as dry ground.

It is not a still picture, either. Wet cells and volume over the run:

| tick | 0 | 2000 | 5000 | 8000 | 10000 | 12000 | 16000 | 20000 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| wet cells | 0 | 6,571 | 6,662 | 5,316 | 13,206 | 8,529 | 10,804 | 14,273 |
| m3 standing | 0.0 | 90.0 | 170.2 | 192.7 | 166.2 | 224.5 | 256.2 | 287.0 |

## Where it stands, which is the answer to part (b)

Ponded cells at tick 10000, by the bundle's own medium:

| medium | site cells | wet | share of that medium |
| --- | --- | --- | --- |
| lawn | 169,876 | 523 | **0.3%** |
| concrete | 22,684 | 2,664 | **11.7%** |
| asphalt | 44,879 | 10,019 | **22.3%** |
| roof | 24,705 | 0 | **0.0%** |

Ponded cells sit at a mean ground height of **3.90 m** against the site's **4.95 m**, so the water
is in the low ground -- the depressions are real. But which low ground keeps it is decided by the
medium: `params.toml` gives concrete and asphalt `field_capacity_mm = 0.0`, so `settle_water` finds
no room to drain into and only evaporation removes it, while lawn drinks 15 mm/h. Both halves of
CLAUDE.md's reworded clause are in this table.

## What it costs to draw

| | quads | mesh |
| --- | --- | --- |
| Capitol at tick 10000, water off | 452,385 | 41 ms, 324 chunks, 93 drawn |
| the same frame, water on | **478,601** | 43 ms |

**26,216 quads, 5.8%**, for 11,035 water voxels -- greedy meshing merges a pond into a slab, which is
why a fifth of the site's wet cells costs a twentieth of its geometry. The water overlay is cheaper
still (238,850 quads) because an overlay flattens the ground cover it replaces.

Scrubbing is unchanged in shape: `set_ponds` diffs the previous snapshot's levels and returns only
the columns that moved, and `set_ponds_reports_exactly_the_chunks_whose_mesh_changed` holds it to
that -- a dry snapshot stales nothing, one puddle stales one chunk, and taking the water off
restores every chunk hash to the bit.

## The screenshots

Four, all `--tick 10000 --headless --frames 300`. None of the committed reference PNGs from V0-V6
was re-accepted, and none needed to be: no existing voxel id moved (see the line budget below).

| File | Arguments beyond the run | What it shows |
| --- | --- | --- |
| `s5-water-on.png` | `--overlay surface` | The site as the run left it: pale blue sheets in the gutters along all four streets, across the car park at the south-west corner and in the low paths between the lawns, and none at all on the lawn or on the Capitol's roof. This is the picture the shot exists to make. |
| `s5-water-off.png` | `--overlay surface --no-water` | The same frame with `[F]` off -- dry grey asphalt everywhere the water was. The HUD still reads `13206 of 262144 ground cells wet ... not drawn`, which is the difference between a dry-looking picture and a dry site. |
| `s5-water-overlay.png` | `--overlay water` | The map: grey ground for dry, and the log ramp from `#9fe8ff` to `#08246b` above it. The legend reads `1.00 to 10000.00 mm standing, log10` and the two provenance lines both carry `(!)` -- `meta.json` has neither a scale nor an `overlays.water` row, and the viewer says so rather than implying the numbers are the run's. |
| `s5-pond-close.png` | `--eye 80,42,224 --look 90,4,186` | The largest single pond, on the asphalt at the south edge: a flat sheet a voxel deep filling the crown of the car park, with the crosshair on it reading `asphalt ground 4.11 m`. Beside it, dry lawn at the same height. |

A private pass on the home scene is owed for this shot and is the operator's, not this worker's:
the pictures change, and nothing under `eco-private/` was read or referenced here.

## The gates

`cargo test --release --no-default-features --test mesh_golden` is **68 pass**, S2's 60 plus the
eight this shot adds; `cargo test --release` the same; `cargo fmt --check` clean; `cargo clippy
--all-targets` back to V1's three pre-existing `src/bundle.rs` findings, with the one this shot
introduced (`chunks_exact_to_as_chunks` in the `water.bin` reader) fixed rather than left.

**Every mesh golden hash is unchanged.** `POND` sits past the 32 bands, so no existing voxel id and
no palette slot moved; the only test edits to existing code are three `ColumnBands` initialisers
that gained the new `cell_m` field, and the two from-the-file loops that now assert water reports a
fallback rather than quietly skipping it.

## Line budget

`git diff --stat 8cacae2 -- ecoview-native/` is **1,191 insertions and 72 deletions, 1,119 net,
against the row's 1,500** -- 381 to spare, with both write-ups in it. The gate is 352 of the
insertions, `src/main.rs` 218, `src/overlay.rs` 217, `src/voxel.rs` 162, `src/palette.rs` 56, and
the two write-ups 186. The split is **653 non-test and 352 test** before the write-ups. The reworded
CLAUDE.md clause is outside the component and outside the budget.
