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
