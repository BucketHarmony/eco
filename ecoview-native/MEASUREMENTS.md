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
