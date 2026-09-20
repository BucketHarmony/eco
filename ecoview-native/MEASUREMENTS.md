# ecoview-native V0: the measurements

Every number below was taken on the machine this repo builds on: Windows 11, Ryzen (24 logical cores),
RTX 4090, Vulkan, `cargo build --release` unless the line says otherwise. Two worlds are measured
throughout:

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
| Clean dependency build, `--release`, no `dynamic_linking` | 771 s (12.9 min) |
| Incremental rebuild of the binary after a one-line edit, `--release` | 6.2–7.3 s |
