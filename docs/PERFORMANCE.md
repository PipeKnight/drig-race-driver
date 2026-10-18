# Frame time and multithreading (P12, part 1)

18 October 2026, Claude Code. Goal: use several CPU cores for the per-frame work on every
platform, **without changing any simulation result**. The original game is the reference, so
parallel code must give bit-identical results to the serial code.

Machine: Intel i7-12700 (8 P-cores + 4 E-cores, 20 logical processors), Windows 11, Rust/cargo
1.99.0, Bevy 0.19.1, release profile. The PC was shared with other sessions throughout, and
part of the time the owner was playing a game; see "Measurement noise".

## Summary

- The 12-car race's simulation was single-threaded. Inside the shared-world tick, the per-car
  vehicle stage (suspension rays, handling, wheel contacts) took about 60–70 % of the time.
  Per-car body contacts with the track, props and boundary took 13–15 %, and car-to-car pairs
  took 5–8 %. The world solver itself is small (2–6 %).
- These three stages now run on Bevy's compute task pool, one task per car, with an ordered
  sequential pass for everything order-dependent (the shared contact pool offset, capacities,
  errors). The results are byte-identical: the 12-car baselines on four circuits, solo
  identity, all 20 grid-soak quick digests, and a new test that runs a deforming 12-car AI
  race serially and in parallel and compares every body and vehicle record on every tick.
- Desktop, quietest round, headless 12 cars: the world tick went from 0.95 ms (serial) to
  0.41 ms with 4 threads, 2.3× faster. In the app (12 cars, Bevy's 12-thread compute pool) the
  world tick went from 2.1 to 1.6 ms and the drive tick from 2.2 to 1.7 ms in the
  lighter-loaded run. Under heavy load from other programs the gain shrinks. The numbers are
  in "Results".
- The desktop app already holds 60 FPS with 12 cars: the main thread works about 5–6 ms and
  the render thread about 5–7 ms per frame. The render thread is the larger CPU cost. About
  1.5 ms of it is spent re-uploading mesh assets that change every frame (follow-up below).
- Solo run (F380), as in the owner's phone log: the render thread is the longest CPU path
  even with one car (8.8 ms against 6.9 ms main plus extract; the simulation about 1 ms).
  CPU light clustering, which Android always uses, costs only 0.34 ms. The biggest items are
  batching (1.9 ms), mesh re-uploads (1.8 ms) and prepare/bind groups (1.3 ms); see
  "Desktop solo run".
- Phone: Bevy's compute pool has 4 threads on an 8-core phone. The solo lap the owner
  measured (F376) does not gain, because there is only one car. 12-car races should gain on
  the main thread. Whether the FPS rises depends on whether the main thread, the render
  thread or the GPU limits a race on the phone; the `[osd] fps` counter (F374) will tell.

## Tools

| Tool | Version | Use |
|---|---|---|
| `GRID_PERF_TIMING=1` (new) | this commit | Wall time per stage, printed by grid-app every 600 frames and at exit, and by grid-opponent-race at exit (stderr). |
| Bevy `trace_chrome` | Bevy 0.19.1, tracing-chrome 0.7.2, tracing 0.1.44, tracing-subscriber 0.3.23 | Per-system spans. grid-app feature `trace` (off by default). |
| `tools/chrome_trace_summary.py` (new) | this commit, bundled Python 3.12.14 | Streams a trace (1.5 GB for 33 s) and reports busy time per thread and the largest spans per frame. |
| `GRID_CPU_CLUSTERING=1` (new, F380) | grid-app | Bevy's CPU light clustering (the Android path) instead of GPU clustering; rendering only. |
| `tools/sample_profile.py` | F305 | User-mode stack sampler, for the headless race. With Bevy's ~30 threads it samples each thread too rarely to time systems, so the app was timed with the two tools above. |

Commands (private paths as in docs/SOAK_TESTING.md):

```
GRID_PERF_TIMING=1 [GRID_THREADS=N] grid-opponent-race ROOT CD01 BP01 12 11200 ai
GRID_PERF_TIMING=1 [GRID_THREADS=1] grid-app --drive --body-contacts CD01 --body-events BP01 --opponents 11 --render local/perf/app/app.png --drive-ticks 3000 --throttle 0.6 --steering 0 --benchmark-frames 1500
cargo build --release -p grid-app --features trace --target-dir target/trace --config 'build.build-dir="C:/cargo-build/perf-trace"'
TRACE_CHROME=local/perf/app/trace-12car.json target/trace/release/grid-app.exe ...same arguments...
python -I tools/chrome_trace_summary.py local/perf/app/trace-12car.json --skip-seconds 18
```

The trace build is a full Bevy rebuild (28 min at 4 jobs) and needs its own build dir;
otherwise it holds the worktree's build-dir lock all that time.

`GRID_THREADS=N` caps the per-car work at N threads, and `GRID_THREADS=1` runs it serially,
in the app too. In the headless tools it also sizes the pool, which they create themselves
(default: every logical processor). The calling thread also runs tasks while it waits.

The offscreen `--render` app runs Bevy's schedule runner at 60 Hz. Frame intervals therefore
hide spare time, so the CPU work is measured as schedule durations rather than FPS.

## Where the time goes (before)

### Headless 12-car race (grid-opponent-race, Donington, 11,200 ticks)

Stack sampler, release build with line tables for our crates only: the race tick is about 66 %
of the race thread over the whole run; loading is most of the rest. Inside the tick, the
vehicle manager is about 58 %, the world advance with its contact provider 28 %, body contacts
15 %. The AI drivers and the race manager barely register (`AiDriver::controls` 0.1 % self).

`GRID_PERF_TIMING`, serial path, quietest run:

| Stage | ms per tick |
|---|---|
| World tick | 0.889 |
| Vehicle stage, all cars | 0.622 |
| Body contacts (track, props, boundary) | 0.128 |
| Car-to-car pairs | 0.072 |
| World advance (solver; contact provider excluded) | 0.033 |
| Prop pieces | 0.000 |
| Commit (prop detection, body events, lap timing) | 0.023 |

Loading 12 cars took 6–10 s (1 tick: 10.2 s; solo: under 1.5 s for the whole lap). The sampler
puts about 85 % of that in `MultiRuntime::load_deforming_on` → `Runtime::prepare_body_geometry`
→ `entity_geometry::Props::read_route`: every car parses the route's prop entities again.
Fixed in F382; see "Loading a field".

### Desktop app frame (12 cars, `--render`, baseline graphics, 1280 × 800)

`GRID_PERF_TIMING`, serial (`GRID_THREADS=1`), 1,743 frames, 1.06 fixed ticks per frame:

| Stage | ms |
|---|---|
| Main schedule (First → Last), per frame | 4.63 |
| Render schedule (render thread), per frame | 4.94 |
| `drive_tick` (per fixed tick) | 2.22 |
| AI drivers and race manager | 0.08 |
| World tick | 2.12 |
| Vehicle stage | 1.34 |
| Body contacts | 0.26 |
| Car pairs | 0.11 |
| Prop pieces | 0.10 |
| World advance | 0.13 |
| Commit | 0.13 |

The app's world tick is about twice the headless one: the app has breakable props on, and
every car shares the CPU with the render thread and Bevy's systems.

Chrome trace of the parallel build (tracing adds overhead; ms per frame over 896 frames):

| Span | Inclusive | Notes |
|---|---|---|
| Render sub-app (render thread) | 7.19 | |
| · render graph | 2.83 | Core3d 1.93, shadow pass 0.31, GPU clustering 0.28 + 0.25 |
| · `allocate_and_free_meshes` | 1.00 | mesh assets changing every frame |
| · `queue_submit` (11 command buffers) | 0.78 | |
| · `prepared_mesh_producer` (13 per frame) | 0.44 | |
| Main app (main thread) | 5.56 | |
| · FixedUpdate | 1.77 | `drive_tick` 1.63 |
| · PostUpdate | 1.66 | mostly waiting for Bevy's parallel visibility/transform systems |
| · Update | 1.09 | `ground_detail::present` 0.51, `drive_present` 0.08 |
| · PreUpdate | 0.37 | |
| ExtractSchedule (main thread, after Last) | 0.71 | |

With pipelined rendering the frame time is set by the slower of main + extract (6.3 ms) and
render (7.2 ms). Both leave the desktop far under 16.7 ms.

Replay and flashback recording do not appear among the top spans.

### Desktop solo run (no opponents), and the phone's CPU light clustering

The owner's first phone log (F376: Xiaomi 13 Ultra, solo Lacetti at Donington, 1280 × 800,
Fifo, mean 35 FPS, range 28–58, simulation at 62.5 ticks/s) also contains Bevy's line "GPU
clustering isn't supported on this device; falling back to CPU clustering". Bevy 0.19.1 hard-codes
that fallback for every Android build (`bevy_pbr::cluster::make_global_cluster_settings`).
`GRID_CPU_CLUSTERING=1` (grid-app diagnostic, rendering only) sets the documented switch
`GlobalClusterSettings::gpu_clustering = None`, so the desktop takes the phone's clustering path.

Chrome traces of a solo run (`--drive --body-contacts --body-events`, no opponents, 900
benchmark frames, steady state), ms per frame:

| | GPU clustering (desktop default) | CPU clustering (as on the phone) |
|---|---|---|
| Render thread (`RenderApp`) | 12.53 | 8.81 |
| Main thread (`main app`) | 8.36 | 5.84 |
| Extract (`RenderExtractApp`, main thread) | 2.98 | 1.10 |
| `drive_tick` (1.07 ticks per frame) | 1.17 | 0.98 |

The GPU-clustering run was disturbed more by other programs: benchmark p95 29.7 ms, against
21.1 ms for the CPU run. Its thread totals are inflated, so compare the clustering rows, not
the totals.

Work by kind is the exclusive time of systems summed over all threads. Bevy runs many render
systems on pool workers, so this is CPU time, not the length of the frame.

| Kind | GPU clustering | CPU clustering |
|---|---|---|
| Batching and instance buffers (`batching::*`, `gpu_preprocess`, `collect_meshes_for_gpu_building`) | 2.24 | 1.85 |
| Mesh upload and allocation (`allocate_and_free_meshes` alone 1.51) | 2.18 | 1.82 |
| Bind groups and other `prepare_*` | 1.79 | 1.28 |
| Specialize and queue | 1.34 | 1.01 |
| Simulation (`drive_tick`) | 1.17 | 0.98 |
| Visibility and culling | 1.04 | 0.99 |
| Other grid-app systems (`ground_detail::present` 0.29, …) | 0.84 | 0.74 |
| Shadows (pass, specialize, queue, cascades) | 0.83 | 0.75 |
| Extraction (other) | 0.65 | 0.64 |
| UI | 0.64 | 0.61 |
| Render pass encoding | 0.64 | 0.59 |
| **Light clustering** | 1.16 | **0.34** |
| Transforms | 0.28 | 0.27 |
| Other | 1.83 | 1.69 |

Findings:
- **CPU clustering is not the phone's problem.** On the CPU it costs 0.34 ms per frame,
  cheaper than the GPU path's CPU-side preparation (1.16 ms). The scene has few clusterable
  lights.
- **Even solo, the render thread is the longest CPU path.** It takes 8.8 ms, against 6.9 ms of
  main thread plus extract; the simulation is about 1 ms of the main thread. A phone core
  about 3–4× slower than an i7-12700 P-core would make the render thread alone about 26–35 ms.
  That fits the measured 35 ms frames, and it is not a vsync multiple [I: the phone itself was
  not profiled].
- **The single largest system is `allocate_and_free_meshes`** (1.51 ms solo). These grid-app
  systems replace mesh assets almost every frame [V code]:
  - `source_trees::*` re-faces every foliage billboard mesh toward the camera each frame;
  - `tyre_smoke::*` rebuilds every smoke texture group's mesh each frame;
  - `ground_detail::present` rebuilds the ground cover whenever the camera has moved or turned
    past its threshold (most frames while driving);
  - `skid_marks::present` rebuilds only when dirty;
  - `drive_present` and the replay project deformed body meshes only on new damage.

  Each replaced mesh is freed, reallocated and uploaded again on the render thread.
- **What multithreading can still do here:** little on the render side. Bevy 0.19 already
  runs render systems in parallel and pipelines the render thread against the main thread.
  The remaining render-thread length comes from the order of its chained system sets. The
  levers are less work: no per-frame mesh replacement (camera-facing done in the shader, or
  meshes updated only when they change); fewer shadow cascades or a smaller shadow map on
  the phone; fewer separate meshes for batching. All of these are rendering changes outside
  the simulation, in the graphics files.

## What runs in parallel now

Code: `grid_tools::parallel` (`map(n, parallel, f)`: results in index order, on Bevy's
`ComputeTaskPool`, which grid-app's systems already use, so the phone gets no second thread
pool), `grid_tools::source_vehicle::multi::MultiRuntime::tick`, `MultiRuntime::parallel`
(default true), `grid_world::wheel_contacts::overlay`, `BodyGeometry::reusable_with`.

1. **Vehicle stage.** A car's stage reads only that car and its own body record in the
   previous world. Only the shared wheel-contact pool offset depends on the cars before it.
   The pool's slot contents never reach the job or the validation: the slots are four in
   either case, `used` is 0 and `shapes` is the same. The contacts are first produced into a
   blank 4-slot pool, then placed in order into the real slots with `wheel_contacts::overlay`.
   That re-initializes the real slot from the stored point, direction, penetration bits and
   token, exactly as production into that slot would. The pool-exhausted check, failures and
   held cars keep their serial order. A unit test checks the overlay against production into
   a different slot.
2. **Body contacts.** Every car is queried at once with the full remaining capacity. In the
   ordered pass a car takes that result only when its real room equals the full capacity, or
   when the result leaves at least `PAIR_CONTACTS` free (`reusable_with`). The capacity reaches
   `Track::body_contacts` only as each pair query's `PAIR_CONTACTS.min(room − found)` and as a
   stop at `found == room`. With that margin both capacities give every pair query the same
   room, so the two queries take identical steps. Otherwise, near pool exhaustion, the car is
   queried again with its real room. Failures and the early stop keep their serial order.
3. **Car pairs.** Hull builds are pure, so they run at once beforehand. A car's hull is first
   needed while its failure state is still the one at the start: only its own build or its own
   pairs, which come later, can change it. Then each car's row of pair queries runs at once
   with capacity 16. A pair query depends on capacity only through `min(capacity, 16)` and
   `capacity == 0`, so the ordered pass reuses a prepared result when at least 16 slots remain,
   and otherwise queries again.

Unchanged and serial: the world advance (solver), the prop-piece contacts, the commit, the AI
drivers and the race manager. The solver is a few percent of the tick and order-dependent.
The AI is 0.08–0.10 ms per tick for 11 drivers, close to the task overhead. The race manager
updates the drivers in order.

## Determinism checks

All in the release profile on this commit's code; the logs are in this worktree's `local/perf/`.

| Check | Result |
|---|---|
| 12-car Donington `grid-opponent-race … 12 11200 ai` | sha256 54abbecb…2a2b, byte-identical to the pre-change binary and the F362 baseline, with `GRID_THREADS` 1, 4, 8 and 20 |
| 12-car Istanbul Park, Long Beach, Nürburgring (`--route`, 6,000 ticks) | byte-identical to the pre-change binary (ed1f3fb4…, d7b34138…, 43340ae6…) |
| Solo identity `grid-opponent-race … 1 11200 ai` | sha256 aad9a07a…e41b, unchanged |
| `grid-soak quick` | 20/20 pass; every scenario's digest equals the pre-change binary's |
| New test `twelve_car_race_is_identical_on_one_thread_and_many` | deforming 12-car AI race with breakable props and a car retired on the grid, 1,500 ticks, serial against 20 threads: every body, vehicle record, frame body and deformation counter equal on every tick; 287 car-to-car contacts compared |
| `parallel::tests::map_keeps_index_order`, `wheel_contacts` overlay test | pass |
| Full suite with absolute GRID_* paths | `cargo test --workspace --release --lib --test "*" --bin grid-app --no-fail-fast -- --include-ignored`: 615 passed, 0 failed |
| Clippy | `cargo clippy --workspace --all-targets --release -- -D warnings` clean; `cargo ndk -t arm64-v8a -P 31 clippy --release --lib -p grid-app -- -D warnings` clean |

Replay and flashback run on the same `MultiRuntime::tick`. Their race tests are in the full
suite, and the soak digests cover reset/replay paths.

## Results (after)

**Headless 12-car race**, quietest of five interleaved rounds (`local/perf/measure/final-headless.txt`
round 3, 4,000 ticks), ms per tick:

| Stage | 1 thread | 4 threads | 8 threads | 20 threads |
|---|---|---|---|---|
| World tick | 0.950 | **0.409** | 0.459 | 0.549 |
| Vehicle stage | 0.657 | 0.242 | 0.271 | 0.316 |
| Body contacts | 0.140 | 0.057 | 0.062 | 0.076 |
| Car pairs | 0.077 | 0.038 | 0.045 | 0.058 |
| World advance (serial) | 0.038 | 0.037 | 0.043 | 0.053 |
| Commit (serial) | 0.025 | 0.023 | 0.025 | 0.031 |

- The world tick is 2.3× faster with 4–8 threads. An earlier quiet pair (11,200 ticks, before
  the car-pair rows were added) gave 0.889 → 0.383 ms with 8 threads.
- Beyond 8 threads it gets slightly worse: 12 cars give at most 12 tasks, and E-cores and
  hyperthreads add stragglers.
- The serial stages are a speed reference for the whole CPU. Their time varied 2.5× between
  rounds. Dividing the world tick by them gives about 15 serial against 6.2–6.8 with 8
  threads in every round that was not disturbed during the run: about 2.3× faster.

**Desktop app, 12 cars** (Bevy's compute pool: 12 threads of 20; offscreen 60 Hz), ms:

| | Serial | Parallel |
|---|---|---|
| World tick, first A/B (lighter load) | 2.12 | **1.55** |
| `drive_tick`, first A/B | 2.22 | 1.68 |
| Vehicle stage, first A/B | 1.34 | 0.67 |
| Main schedule per frame, first A/B | 4.63 | 4.54 |
| World tick, 3 rounds with the owner's game running | 3.18 / 3.14 / 2.26 | 2.31 / 2.79 / 2.17 |
| Main schedule, the same rounds | 6.96 / 6.99 / 5.03 | 6.04 / 7.04 / 5.62 |

- The app gains less than the headless race, and less under load.
- Timestamps inside the tasks (a temporary build, removed again) showed why. Under the game's
  load, the calling thread ran cars 0, 1, 2, … one after another. Only 2–3 pool workers ever
  joined, 0.2–0.6 ms after the scope started, while each car took 0.24 ms instead of 0.055 ms
  quiet. The headless race showed the same pattern in that period: the OS gave the
  background process only about 3–4 cores while the foreground game ran.
- A pool of the simulation's own instead of Bevy's (a `GRID_SIM_POOL` experiment with 4 or 8
  threads, removed) was no better.
- The quiet first A/B halved the vehicle stage; that is the expected case.

Cores used: the headless tools use every logical processor unless `GRID_THREADS` says
otherwise. grid-app uses Bevy's compute pool (12 threads here, 4 on an 8-core phone).

## Measurement noise

The PC ran other sessions' builds and tests, and for part of the time the owner's game, which
took about 60 % of the CPU. Single runs then varied by 2–3×. Under that load, parallel runs
also lose more than serial ones, because a car whose task lands on a busy core or an E-core
holds up the whole join. The tables use interleaved serial/parallel rounds in the quietest
period available; the loaded rounds are kept in `local/perf/measure/`.

## Phone (expectation, not measured)

- Bevy's default pools on an 8-core Snapdragon 8 Gen 2 (1 × X3, 2 × A715, 2 × A710,
  3 × A510): 2 I/O, 2 async-compute and 4 compute threads, plus the calling thread.
- Solo lap (F376: about 35 ms frames, `sim_ticks_per_s` 62): nothing to gain. One car is one
  task, and the frame was rendering-bound.
- 12-car race: at 30 FPS the fixed 62.5 Hz tick runs about twice per frame. On the desktop the
  parallel stages are about 80 % of the serial world tick. With 4–5 threads, some of them
  little cores, the phone's world tick might fall by 40–50 % [I]. This only shortens the frame
  if the main thread is the slowest of main, render thread and GPU in a race.
- To check: an `[osd] fps` / `frame_stats` race log with 11 opponents, before and after this
  commit. `GRID_PERF_TIMING` cannot be set on the phone yet (no environment variables for the
  activity).

## Loading a field (F382)

Before: `MultiRuntime::load_deforming_on` called `Runtime::prepare_body_geometry` for every
car, and each call parsed `objects.ens`, `objecttypes.pssg` and the barrier ornaments again
(`Props::read_route`). Every car of a field is on the same route, so all twelve parses gave the
same boxes.

Change: `load_deforming_on` parses the route props once and hands each car a clone through the
new `Runtime::prepare_body_geometry_with(root, table, props)`. `prepare_body_geometry` keeps
its old behaviour (it reads the props and calls the new function). `Props` already holds its
boxes as `Arc<Vec<PropBox>>` and every writer goes through `Arc::make_mut`
(`PropWorld::arm`, prop breaking), and `PropBox` has no interior mutability. So a car that
arms or breaks a box gets its own copy, as before; nothing per car is shared mutably. The
cars still load one after another; parallel loading was not needed.

Measured on the desktop (i7-12700, shared with other sessions' builds, so single runs vary),
release, interleaved rounds:

| | Before | After |
|---|---|---|
| `grid-opponent-race ROOT CD01 BP01 12 1 ai` (load + 1 tick), 3 rounds | 6.38 / 6.28 / 11.08 s | 1.09 / 1.07 / 1.24 s |
| `grid-app --drive --body-contacts CD01 --body-events BP01 --opponents 11 --render … --drive-ticks 1` (process start to exit), 4 rounds | 19.9 / 18.0 / 21.8 / 24.5 s | 12.4 / 13.2 / 13.0 / 12.5 s |
| The same app run without opponents, for reference | | 9.6 / 9.4 s |

So a 12-car race now starts about 7 s sooner in the app on this PC. The 12-car field costs
about 3 s more than a solo run. That and the rest of the app's start are probably Bevy, the
track and the car graphics [I, not profiled], not the car physics (the headless load of all 12
cars is about 1 s). On the phone
the saving should be several times larger [I, not measured].

Identity [V]: the 1-tick outputs are byte-identical; 12-car `… 12 11200 ai` sha256
54abbecb…2a2b and solo `… 1 11200 ai` aad9a07a…e41b, unchanged; a 12-car 6,000-tick race with
`GRID_DYNAMIC_PROPS=1` and DF01 (breakable props, deformation) is byte-identical to the
pre-change binary (sha256 3c932872…841e). `grid-soak quick` 20/20, all 20 digests equal to the
pre-change binary's.

## Per-frame mesh uploads (F392)

18 October 2026, Claude Code. The owner approved changing the graphics files for this
(through the coordinator; Codex was busy with other work). Evidence: this worktree's
`local/perf381/` (images, timing runs, trace summaries, test and clippy logs).

**What replaced meshes every frame.** A temporary counter of `AssetEvent::<Mesh>::Modified`
per entity name, run on a solo Baseline lap like the phone's (patch kept as
`local/perf381/mesh-census.patch`, not committed):

| Mesh | Replaced, frames 0–600 | Frames 600–1200 |
|---|---|---|
| Ground cover (`ground_detail::present`) | 585 | 110 (car slower) |
| 5 wheel-debris meshes (`wheel_debris::present`) | 585 each | 1,170–1,185 each |
| Tyre-smoke groups (`tyre_smoke::draw`, no `Name`) | — | 1,087 (sprites on screen) |
| Skid marks (only when dirty) | 2 | 40 |

Source trees are off in the Baseline preset (`--trees`, Enhanced only), so they did not show
in F380's profile. With `--graphics enhanced --trees source` every one of the 1,914 placed
billboard meshes was replaced every frame.

**Changes** (presentation only; no simulation code touched):

- New `mesh_stream.rs`. Systems pack new vertex bytes in the main world exactly as Bevy's
  `Mesh::write_packed_vertex_buffer_data` does: attributes interleaved per vertex in
  attribute-id order, little-endian f32. A unit test compares the bytes with Bevy's own
  packing. The bytes reach the render world at extraction; the main and render sides swap
  buffers, so nothing is allocated after the first frames. A render system right after
  `allocate_and_free_meshes` writes them with `RenderQueue::write_buffer` over the mesh's
  existing allocation (`MeshAllocator::mesh_vertex_slice`). The `Mesh` asset is left
  unchanged, so Bevy does not clone, free, reallocate or repack it. Writes whose size does
  not match the allocation are skipped, and a mesh not allocated yet draws its own data.
- `source_trees.rs`: `FacingSource` keeps the mesh's packed vertex buffer and layout.
  `placed()` writes the placement's radial (or world-up) normals once, because they do not
  depend on the camera. `present` writes only the faced positions each frame, with the same
  arithmetic as before. `append` is unchanged.
- `ground_detail.rs`: the cover mesh holds a capacity of quads (drawn ones first, then
  zero-area padding at the origin). It is replaced only when the cover outgrows it (+25 %
  headroom, AUTHORED); otherwise each rebuild streams its vertices (`mesh_stream::QuadMesh`).
  The rebuild threshold is unchanged.
- `wheel_debris.rs`: a slot's mesh is rebuilt while it has particles, and once more to empty
  it. Before, every fixed tick marked all 5 slots dirty, so all 5 meshes were replaced every
  frame even with no particles.
- `tyre_smoke.rs`: a texture group with no sprites leaves its already empty mesh alone.
  While sprites exist the groups are still rebuilt every frame (see "Not done").

**Picture.** Offscreen fixed-step captures (`--render-fixed-step`, 1280 × 800, Donington,
throttle 0.6), before (fde38ef) against after, are **byte-identical PNGs**: Baseline at 600,
1,300 (216 smoke particles on screen) and 3,000 ticks, Enhanced with `--trees source` at
1,300 and `--trees coverage` at 3,000. After the rebase onto main with Codex's graphics merge
(b43b575), the same comparison against that main is byte-identical again: Baseline 1,300 and
3,000, Enhanced `--trees source` 1,300, and `--trees coverage-shaded` 3,000 (Codex's sheet
colours, `saved_rgb=true`). Windowed or real-clock runs differ from run to run,
because frame timing changes what is drawn, so they cannot be compared.

**Render thread** (`GRID_PERF_TIMING=1 GRID_CPU_CLUSTERING=1`, offscreen 60 Hz, solo Lacetti,
3,000 ticks; steady state = frames 600 to the end), three interleaved rounds, ms per frame.
Other sessions kept the CPU at 42–96 % load throughout:

| | Round 1 | Round 2 | Round 3 | Median |
|---|---|---|---|---|
| Baseline, before | 9.07 | 7.32 | 5.79 | **7.32** |
| Baseline, after | 5.46 | 6.55 | 5.39 | **5.46** |
| Enhanced + source trees, before | 40.76 | 27.54 | 24.09 | **27.54** |
| Enhanced + source trees, after | 17.25 | 17.57 | 25.83 | **17.57** |

- Baseline (the phone's path): about 1.9 ms less render-thread time per frame on the median,
  0.4–3.6 ms per pair. The main thread fell slightly too (median 4.51 → 3.63 ms), because it
  no longer builds and moves new mesh assets.
- Enhanced with trees: about 10 ms less. The 1,914 tree meshes had made the render thread
  the bottleneck (p50 frame 28.8 → 17.8 ms in round 2, i.e. under 60 FPS before, at 60 after).
- Chrome traces (Bevy trace_chrome, same run with 900 benchmark frames, before fde38ef
  against the final tree; `local/perf381/trace/*-summary.txt`). Both runs were heavily
  disturbed (frame p50 44 and 37 ms instead of about 17), so the totals are inflated and
  only the mesh items compare: `allocate_and_free_meshes` 4.09 → 1.21 ms per frame, "mesh
  upload / allocation" 4.84 → 1.65 ms, the extract sub-app 16.0 → 7.8 ms. The load was, if
  anything, heavier in the after run (`drive_tick` 5.3 → 6.8 ms). What remains in
  `allocate_and_free_meshes` is mostly the smoke groups, which had sprites for most of this run.
- Phone [I, not measured]: F380 put the phone's render thread at 3–4× the desktop's. If
  that holds, 1.5–2 ms saved on the desktop is 5–8 ms of the phone's ~35 ms frame. The new
  phone log (F378 build: 2308 × 1080, still 34 FPS) supports a CPU limit. Check: the next
  `frame_stats` log.

**Not done here.** Smoke groups while sprites exist: streaming them with the same padded
capacity gave a slightly different picture at 1,300 and 3,000 ticks (2.6 % of pixels, at
most 25 levels, all in the smoke). Replacing the asset instead of streaming gives the same
difference, so the cause is the padding, not the streaming. Cause found and fixed in F394
(see "Tyre-smoke streaming"): Bevy sorts blended meshes by the mesh asset's bounding-box
centre, not the entity's bounding box.

**Merge with Codex's graphics work.** While this was under way, main merged
codex/grid-graphics, including e075319 (`append` gains `sheet_colours` and a COLOR
attribute on the tree sheets, `--trees coverage-shaded`) and a `tyre_smoke::draw` camera
query change. The rebase merged both without conflicts. For later merges of graphics
branches:

- Changed here in `source_trees.rs`: `FacingSource` (fields `packed` and `layout`, set in
  `from_mesh`), `FacingSource::placed` (writes normals into a packed copy), `Facing` (new
  `vertices` field), `install` (`init_resource::<MeshStream>`) and `present` (streams
  positions; no `Assets<Mesh>` writes). `append` is unchanged.
- The test `sheet_presentation_uses_the_final_replay_camera_in_the_same_frame` now reads
  the queued stream bytes instead of the mesh asset.
- New vertex attributes need no extra work. `from_mesh` packs every attribute the mesh has
  (`create_packed_vertex_buffer_data`) and takes the layout from the mesh, so Codex's sheet
  colours go into every frame's bytes unchanged.
- If a graphics branch changes `present`, keep the streaming and port only its orientation
  arithmetic into the closure. Do not write to the mesh asset there: that would bring back
  the per-frame reallocation.

## Tyre-smoke streaming (F394)

18 October 2026, Claude Code. Evidence: worktree objective-darwin-d70888 `local/f393/`
(img/, field/, time/, census, logs; the folder name predates the renumbering to F394).

**Why the padding changed the picture [V].** Bevy 0.19 sorts alpha-blended items
(`Transparent3d`) by `TransparentSortingInfo3d::Sorted::mesh_center`: the entity's
world-from-local transform applied to `RenderMesh::aabb_center`, the centre of the *mesh
asset's* bounding box (`Mesh::compute_aabb`, taken in `RenderMesh::prepare_asset`;
bevy_pbr `material.rs` `queue_material_meshes`, bevy_render `mesh/mod.rs`). The entity's
`Aabb` component and `NoFrustumCulling` play no part. All smoke groups have the identity
transform, so the centre of each group's sprites decides which group draws first. Padding
quads at the origin pull that centre towards the origin and reorder the groups. Streaming
has a second problem: the transparent phase is retained, and an entity is re-queued (with a
fresh centre) only when `AssetChanged<Mesh3d>` marks it dirty, so a streamed asset would
keep the centre of its last replacement.

**Change.**

- `mesh_stream.rs`: `MeshStream::sort_centre(entity, centre)` hands a world-space sort
  centre to the render world with the vertex bytes. A render system (`resort`, in
  `RenderSystems::PhaseSort` before `sort_phase_system::<Transparent3d>`) writes it into the
  entity's retained `Transparent3d` item, after the queue systems and before Bevy computes
  each item's distance from it. Items are keyed `(Entity::PLACEHOLDER, main entity)`, as
  `queue_material_meshes` adds them.
- `tyre_smoke.rs`: each texture group streams its sprites through `QuadMesh` (capacity +25 %,
  AUTHORED, zero-area padding), with the same vertex arithmetic as before. Each frame it
  passes the centre that the old unpadded mesh would have had: `Aabb::enclosing` over the
  drawn corners (Bevy's own function), or the origin for an empty group (formerly one
  degenerate quad at the origin). An empty group that is already empty is still skipped.

**Picture [V].** Offscreen `--render-fixed-step` captures, current main (dca690b, exe
sha256 03180c24…) against this change (d2af88bf…): **byte-identical PNGs** at Baseline 600,
1,300 (219 sprites) and 3,000 ticks, Enhanced `--trees source` 1,300, and a 12-car field
(`--opponents 11`) at 1,300 (224 sprites). Control: the same streaming without
`sort_centre` (sha256 2b8996e6…) differs from main at 1,300 (0.32 % of pixels, at most 80
levels) and 3,000 (2.63 %, at most 25, which is F392's difference).

**Meshes replaced [V]** (F392's temporary census patch, solo Baseline, 3,000 ticks): unnamed
entities, which include the smoke groups, went from 512 + 1,620 (frames 0–600 + 600–1,200)
on main to 510 + 620 with this change. About 1,000 smoke replacements in 600 frames are
gone. The remaining ~1 unnamed replacement per frame is also there before any sprite
appears; it is probably the HUD text meshes (`osd_scene::texts` replaces a text mesh
whenever its string changes, e.g. the lap timer) [I, not checked].

**Render thread [V]: no measurable change on the desktop.** `GRID_PERF_TIMING=1
GRID_CPU_CLUSTERING=1`, offscreen 60 Hz, 3,000 ticks, steady state from frame 600, ms per
frame:

| Run | Main | This change | Note |
|---|---|---|---|
| Solo Baseline, 6 interleaved rounds | 4.14, 3.99, 3.90, 4.85, 3.86, 3.82 (median 3.94) | 3.92, 3.94, 3.92, 4.05, 3.83, 3.85 (median 3.92) | median per-pair saving 0.04 ms; smoke in 45 of 51 samples, ~180 particles |
| 12 cars, rounds 1–3 (main first) | 4.07, 4.91, 5.00 | 4.22, 5.20, 6.06 | the main thread rose by about as much, so this was drifting load from other sessions, not the change |
| 12 cars, rounds 4–7 (ABBA, 55–83 % load) | 5.83, 6.34, 5.31, 5.15 | 5.44, 5.29, 4.98, 5.86 | |

Over all seven 12-car rounds, change minus main has a median of +0.16 ms with a spread of
±1 ms: noise. The smoke groups are a few meshes with about 200 sprites (≈ 1,000 vertices),
so replacing them costs little on this CPU. F392's trace put `allocate_and_free_meshes`,
mostly smoke, at 1.2 ms/frame, but that run was heavily disturbed. Phone [I, not measured]:
the saving is at most a fraction of a millisecond unless the phone's mesh allocation is
much slower than F380's 3–4× ratio suggests.

**Found in passing [I, not fixed here].** Ground cover and wheel debris (F392) are also
`AlphaMode::Blend` meshes streamed with padding, so since F392 they sort by the centre of
the padded asset at their last replacement, not by their current vertices. F392's captures
were byte-identical, so it did not show there. Passing their centres to `sort_centre` would
restore the order they had before F392. That could change today's picture compared with
main, so it needs its own before/after check.

## Phone bisection (FNNN)

Why: F392 cut the desktop render thread from 7.3 to 5.5 ms on the phone's preset, but the
owner's phone stayed at about 30 FPS [V, APK dca690b773e4, logs
`log-20261018-093253.txt` and `-093621.txt`, kept privately]:
- render 2308 × 1080, `display_hz` 120;
- mean 36 FPS (Fifo, then AutoVsync at 60/90 Hz) and 31 FPS (AutoNoVsync / Fifo at 120 Hz),
  ranging 22–65.

The front end with nothing simulating (`sim_ticks_per_s=0.0`) already sits at 30.5 FPS:
`avg_ms` 32.8 with `max_ms` exactly 33.33. A 33.33 ms ceiling is four refreshes at 120 Hz (or
two at 60 Hz), so the frame's work takes longer than 25 ms (or 16.7 ms) and less than
33.3 ms [I]. AutoNoVsync gave the same numbers, so the driver probably falls back to Fifo
[I]. The render height made no difference between 1280 × 800 and 2308 × 1080.
The desktop cannot show what the phone spends this time on, so the phone has to report it
itself. The owner only sideloads APKs and sends logs; there is no adb.

### Performance test screen (normal APK)

Race Options → **Performance test** (AUTHORED diagnostics; `crates/grid-app/src/perf_test.rs`).
Each row turns one part of the frame off, live, and is never saved, so the next start is the
normal game:

| Row | Off means |
|---|---|
| Shadows | every directional light stops rendering shadow maps |
| Foliage | camera-facing tree and bush billboards (`source_trees::Facing`) hidden |
| Ground cover | the `ground cover` mesh hidden |
| Smoke, debris, skid marks | tyre smoke groups, `wheel debris *` and `skid marks` hidden |
| HUD | race HUD parts and the damage panel hidden, the OSD scene's camera inactive (touch zones stay) |
| Sample logging | the periodic `drive_sample`/`drive_*_sample` "tick" reports skipped (events such as pause, reset and exit stay) |
| Whole scene | Off: every mesh except the player's car hidden (prop pieces that their own system shows again may stay) |
| Auto cycle | On: steps through normal → each part alone → all six off → car only → normal, 8 s each |

- Every `frame_stats` line ends with `perf_test=normal` or `perf_test=off:shadows+hud…` once
  the screen has been used (or `GRID_PERF_TEST` is set). Each change logs
  `perf_test version=1 event=apply switches=…`, and each Auto cycle step logs `event=step`.
  One run therefore gives a table per switch.
- The switches change only visibility, shadow maps, a camera's activity and log lines; the
  simulation is untouched.
- Desktop: `GRID_PERF_TEST=auto` or a list such as `GRID_PERF_TEST=shadows,foliage`.
  Checked on the desktop [V]: the auto cycle logs ten steps and labels every `frame_stats`
  line. Renders with everything off show no shadows, HUD or grass, and car-only shows the
  car alone.

### Per-system profile (separate profiling APK)

grid-app feature `profile` (`crates/grid-app/src/span_profile.rs`) turns on Bevy 0.19.1's
`trace` spans and adds a tracing layer through `LogPlugin::custom_layer`. The layer adds up
each span's inclusive and exclusive time per name and thread. Every 10 s
(`GRID_PROFILE_SECONDS`) the app logs:
- `profile_window` (frames, FPS, `perf_test` label);
- `profile_thread`: busy ms per frame of the 12 busiest threads (main, render, compute pool…);
- `profile_span`: the top 25 systems and work spans by exclusive ms per frame;
- `profile_schedule`: the top 12 schedules and wrappers by inclusive ms per frame (Main,
  Render, Extract…).

The tables are text in the normal log file, not a 1.5 GB chrome trace. Spans cost time, so
this lives only in a separate APK (`tools/build_android.ps1 -BundleData -Profile`, named
`…-bundled-PRIVATE-profile-debug.apk`); the normal APK has no spans.

### Log writes

On Android every log line used to be a separate unbuffered write to the shared-storage file,
besides the logcat call. That is about 220 lines/s while driving (`drive_sample`,
`drive_lap_sample`, `drive_handling_sample`, `drive_deformation_sample` about 49/s each, plus
`drive_input`), all on the `grid-logcat` thread that reads the stdout pipe. Now the file is
written through a 64 KiB buffer, flushed when a line finds the last flush at least 0.5 s old,
and at once for error lines (panics, fatal screens). On an abrupt kill up to 0.5 s of lines
can be lost. The game's main thread only writes to the pipe, unchanged.

## Follow-ups (not done here)

1. **Mesh re-uploads every frame.** Done in F392 for foliage, ground cover, wheel debris
   and empty smoke groups (see "Per-frame mesh uploads"), and in F394 for smoke groups with
   sprites. Still replaced: probably the HUD text meshes when their string changes [I].
   Ground cover and wheel debris sort by a stale centre (see F394).
2. **Loading 12 cars.** Done in F382: the route props are parsed once per field (see
   "Loading a field").
3. **GPU and render resolution on the phone.** 4× MSAA at the full surface, the render-height
   cap (F375) and the render graph are the likely limits of the solo lap.
