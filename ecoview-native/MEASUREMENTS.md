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

