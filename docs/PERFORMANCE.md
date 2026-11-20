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
| `GRID_PERF_TIMING=1` (new) | this commit | Wall time per stage, printed by grid-app every 600 frames and at exit, and by grid-opponent-race at exit (stderr). Since F538 the line also gives the mean world size per shared-world tick: `world_bodies` (cars and pieces), `world_pieces` (pieces and lost wheels), and active contacts by kind: `contacts_wheel` (wheel contacts in the shared pool), `contacts_body` (car hulls against the track and props), `contacts_car` (car pairs) and `contacts_piece` (piece contacts); since F546 also `world_asleep` (bodies out of the active list) and `world_parked` (of those, parked). grid-soak `run` prints the same line on stderr. |
| Bevy `trace_chrome` | Bevy 0.19.1, tracing-chrome 0.7.2, tracing 0.1.44, tracing-subscriber 0.3.23 | Per-system spans. grid-app feature `trace` (off by default). |
| `tools/chrome_trace_summary.py` (new) | this commit, bundled Python 3.12.14 | Streams a trace (1.5 GB for 33 s) and reports busy time per thread and the largest spans per frame. |
| `GRID_CPU_CLUSTERING=1` (new, F380) | grid-app | Bevy's CPU light clustering (the Android path) instead of GPU clustering; rendering only. |
| `GRID_ASSET_COUNTS=1` (new, F482) | grid-app | Every 600 frames: spawned entities and Mesh/StandardMaterial/Image counts of the main world; entities, prepared meshes and images of the render world; wgpu's live buffers, textures, views, bind groups, samplers and pipelines and its GPU allocator total. Flat counts with a growing process rule out assets, entities and GPU objects (see "Memory growth"). |
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

## Phone bisection (F395)

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
normal game. Until F553 the panel had only 5 row texts, so the last three rows (Sample logging, Whole scene, Auto cycle) were never drawn. The Auto cycle row (the first row since F553) was removed in F559 at the owner's request; `GRID_PERF_TEST=auto` still runs the cycle on the desktop, and any switch on the screen ends it:

| Row | Off means |
|---|---|
| Shadows | every directional light stops rendering shadow maps |
| Foliage | camera-facing tree and bush billboards (`source_trees::Facing`) hidden |
| Ground cover | the `ground cover` mesh hidden |
| Smoke, debris, skid marks | tyre smoke groups, `wheel debris *` and `skid marks` hidden |
| HUD | race HUD parts and the damage panel hidden, the OSD scene's camera inactive (touch zones stay) |
| Sample logging | the periodic `drive_sample`/`drive_*_sample` "tick" reports skipped (events such as pause, reset and exit stay) |
| Whole scene | Off: every mesh except the player's car hidden (prop pieces that their own system shows again may stay) |

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

## Sort centres and the HUD timer (F398)

18 October 2026, Claude Code, coordinator's follow-ups to F394. Evidence: this worktree's
`local/f398/` (img/, time/, census, check logs).

**Sort centre.** F394 found that Bevy sorts blended meshes by the centre of the mesh asset's
bounding box. It copies that centre into the retained `Transparent3d` item only when the
entity is queued, so a streamed or skipped asset can keep a stale centre.

- Ground cover: blended in Enhanced (soft alpha, the default). Its padded asset is replaced
  only on growth, so since F392 it sorted by the padded centre. `ground_detail::present` now
  passes the centre of the drawn quads on every rebuild, as the asset replaced on every
  rebuild had before F392; an empty cover is the origin, as the old three degenerate
  vertices.
- Wheel debris needs nothing. A slot with particles is still a replaced asset. An empty
  slot left alone holds the same mesh as before F392 (three degenerate vertices at the
  origin, independent of the camera), so its centre is the same.
- `mesh_stream::resort` now takes a local centre and applies Bevy's own world-from-local
  transform (`get_mesh_instance_world_from_local(...).transform_point3`, as
  `queue_material_meshes`). That matters for HUD nodes, which move. It also keeps, per
  entity, the world centre it last wrote. When Bevy re-queues the entity from the stale
  asset (visible again, material changed), the item's centre no longer matches, so the
  current centre is written again. Otherwise an item keeps its old centre, as a retained
  item of an unchanged asset does. So the order changes exactly when it changed with
  replaced assets. Superseded by F698 (next section): every item's centre is now
  written each frame.

## World sort centres of moving meshes (F698)

2 November 2026, Claude Code, car-reflections session; the owner's report of the windscreen and
its frame showing sharply through the Viper's rear window, at some camera angles only.
Evidence: this worktree's `local/reflections/crops/gl*`, `gd*` and `local/reflections/out/`.

**Cause.** Bevy 0.19.1 `queue_material_meshes` (bevy_pbr `material.rs`) adds a blended mesh
with `add_retained` and stores its world centre: the entity's world-from-local at that moment
applied to the asset's bounds centre. Nothing updates it when the entity moves; the item is
re-queued only when it is respecialised, becomes visible again or its asset or material
changes. `sort_phase_system` then recomputes only the view distance from that stored point.
So a car's glass and lamps sort by where the car was when they were queued, with the panes'
offsets in the car's orientation of that moment. Car glass writes no depth, so whichever
pane draws last covers the other. Once the car has turned, its windscreen can sort after
its rear window, and the windscreen's black frame and cracks are then drawn over the rear
window. The same held for every moving transparent mesh: opponents' glass and lamps, the
reflection overlays and the sky dome, which follows the camera.

Window-phase audits (`--window-phase-audit`, 1280 × 800 Baseline, reflections off,
build 85e1c69c):

| Case | Stored centre error | Order of the four Viper panes |
|---|---|---|
| Washington, standstill after spawn | 0 m | windscreen 313 first, rear window 310 last (correct) |
| Washington, 500 ticks straight | 142.7 m | correct (same orientation) |
| Donington, 1,250 ticks circling, three camera offsets | 15–18 m | **rear window 310 first, windscreen 313 last**, the cracked windscreen visible through the rear window |

Codex's G237/G238 found the stale centres on the Lacetti; G313 refreshed them only for the
player's windows, in ordinary Donington Lacetti driving (`window_response::install_current_sort`).

**Fix.** `mesh_stream::resort` (before `sort_phase_system::<Transparent3d>`) now writes every
mesh item's centre each frame, in every view: the entity's current world-from-local (as Bevy's
queue takes it) applied to the asset's bounds centre, or to the latest centre streamed through
`MeshStream::sort_centre`. The original also sorts its instances every frame: `carglass`
saves `instancesRequireSorting` 1 (docs/TYRE_SMOKE_RESEARCH.md 7.6), and each pane is its
own render instance (the Viper's `MPJRI310`–`313`, as the Lacetti's). The Lacetti-only
`--window-sort current` adapter is now redundant, and `--window-sort off` no longer gives
stale centres.

**Not ported.** The original's `carglass` and `car_light_glass` also write depth (LEQUAL,
writes on; docs/TYRE_SMOKE_RESEARCH.md 7.1) and draw two-sided, as ours (`cull_mode` none).
With the panes in back-to-front order the depth writes do not change how one pane shows
through another; they would hide later transparent draws behind the glass, which the
smoke compositing here does not reproduce. Ours still writes none.

**Cost.** A hash lookup and one point transform per transparent item per view per frame
(about 70 items in the main view of a solo race).

**HUD timer.** A temporary counter in `osd_scene::texts` (not committed) on a solo Baseline
run at 1,300 ticks: one 8-character text (the running time) was rebuilt in 892 of about 900
text updates; every other text changed once. `texts` already skipped unchanged strings. Now
a new string whose glyph mesh has the same vertex count and triangles as the asset (the
next frame's time) is streamed: `create_packed_vertex_buffer_data` of the new mesh is
written over the old allocation, and its centre goes to `sort_centre`. Other changes still
replace the asset. The asset test with the purchased time font checks that two consecutive
times have equal vertex counts and indices.

**Picture.** `--render-fixed-step` PNGs, 1280 × 800, Donington solo, throttle 0.6. Pre-F392
= 7912ee7, main = 2393a9e (F397), new = this change:

| Case | new = pre-F392 | new = main | main = pre-F392 |
|---|---|---|---|
| Baseline 1,300 ticks | identical | identical | identical |
| Baseline 3,000 ticks | **identical** | differs | differs (2 pixels, 1 level) |
| Enhanced `--trees source` 1,300 | identical | identical | identical |
| Enhanced `--trees source` 3,000 | identical | identical | identical |

At Baseline 3,000 main differs from pre-F392 and the change matches pre-F392 again: the
intended fix. The other cases never showed the stale order.

**Meshes replaced** (temporary counter, solo Baseline, fixed step, 1,300 ticks): unnamed
meshes (HUD texts, smoke) 496 → 9 in frames 0–600 and 1,119 → 32 in frames 0–1,200. Ground
cover 1, skid marks 44 and wheel debris (only while a slot has particles) are unchanged.

**Render thread** (`GRID_PERF_TIMING=1 GRID_CPU_CLUSTERING=1`, offscreen 60 Hz, solo, 3,000
ticks, steady state from frame 600; 4 ABBA rounds of main 2393a9e against the change, 3–97 %
load from other sessions), ms per frame:

| | Runs, sorted | Median |
|---|---|---|
| Baseline, main | 5.19, 5.24, 5.56, 5.76 | 5.40 |
| Baseline, change | 4.31, 5.25, 5.59, 8.14 | 5.42 |
| Enhanced + trees, main | 12.71, 12.88, 14.83, 25.53 | 13.86 |
| Enhanced + trees, change | 12.63, 12.81, 12.85, 15.98 | 12.83 |

No measurable change: the per-round differences scatter around zero within ±1.5 ms (one
outlier each way under load). One small mesh per frame is below the desktop noise. Whether
the phone shows it is not measured; it now has no per-frame mesh replacement on a solo lap
apart from smoke-free debris and real HUD changes.

## Memory growth (F482)

Reported by the career session (19 October 2026): a plain release race on Nürburgring
route_1 with `--drive-ticks` and the autopilot grew by ~9.5–10 MB/s with 20 cars and
~5.7 MB/s with 12, and kept growing after `drive_script_complete`. An unattended 20-car
run reached 64 GiB; a guarded one ended in wgpu `DeviceLost` and a `bevy_pbr` cluster
`PoisonError` panic once memory ran out.

**Cause [V]: audio, not rendering.** The opponent engines, every car's tyre voices and the
HUD sounds were spawned with `PlaybackSettings::LOOP`. Their decoders synthesise sound
forever (`Iterator::next` never returns `None`), so the loop never repeats anything, but
Bevy plays a looped source as rodio 0.22's `repeat_infinite()`. That wraps the decoder in
`Buffered` (`rodio/src/source/repeat.rs`) and keeps a clone of the first span to replay
from. Every span decoded since the start then stays reachable: one `Vec<f32>` of 32 768
samples (131 072 bytes) per source every 0.68 s. With 48 kHz mono f32 that is 192 kB/s per
source: 19 opponent engines + 20 tyre sets + the stereo HUD ≈ 7.9 MB/s for 20 cars,
≈ 4.8 MB/s for 12 (Rust heap; the working set adds allocator overhead). It grows with audio
time, not frames or ticks, so it went on in a finished race. The player's engine was already
played once and did not grow.

The collision-impact source added in F479 (one mono 48 kHz source, `collision_sound.rs`)
had the same LOOP and would have added 192 kB/s; it landed after the measurements below
and is fixed the same way (by inspection: its decoder never ends either).

**Fix.** These sources are played with `PlaybackSettings::ONCE`, as the player's engine:
the decoder is appended to the sink as it is, nothing is buffered, and since it never ends
the sink never empties. No cap or purge. Test
`tyre_sound::tests::endless_tyre_voices_play_once` (ignored; needs the purchased bank).

**How it was found** (evidence: worktree trusting-driscoll-f0fd01 `local/leak/`):
1. Guarded 20-car runs (`local/leak/guard2.sh`: own PID by snapshot, working set and private
   bytes every 10 s, killed above the limit or after the time cap).
2. `GRID_ASSET_COUNTS=1`: all asset, entity and wgpu counts and the GPU allocator total
   were flat (e.g. 15 819 meshes, 648 materials, 476 images, 666 GPU allocations) while the
   process grew, so neither Bevy assets nor GPU resources were leaking.
3. A temporary counting global allocator showed the Rust heap itself growing
   ~55 KB per frame (`local/leak/runs/base.out`).
4. A temporary sampling heap profiler (every 97th allocation backtraced, dropped when
   freed, live sites printed; source kept as `local/leak/asset_counts_sampler.rs`, not
   committed) put all growing bytes under
   `rodio::source::buffered::extract::<engine_sound::Decoder / tyre_sound::Decoder /
   hud_sound::Decoder>` on the cpal output thread (`local/leak/runs/sample1.out`).
   An earlier subsystem bisection had mis-read the audio-off run as still growing (its rate
   window included loading); its samples were in fact flat after loading.

**Android** shares this path (same `bevy_audio` feature and the same `engine_sound`,
`tyre_sound` and `hud_sound` installs), so phones grew by the same ~0.19 MB/s per endless
source while racing, with far less memory. Not measured on a device; no APK built here.

## Follow-ups (not done here)

1. **Mesh re-uploads every frame.** Done in F392 for foliage, ground cover, wheel debris
   and empty smoke groups (see "Per-frame mesh uploads"), and in F394 for smoke groups with
   sprites, and in F398 for the HUD timer; the stale ground-cover sort centre is fixed in
   F398 too (see "Sort centres and the HUD timer").
2. **Loading 12 cars.** Done in F382: the route props are parsed once per field (see
   "Loading a field").
3. **GPU and render resolution on the phone.** 4× MSAA at the full surface, the render-height
   cap (F375) and the render graph are the likely limits of the solo lap.

## Le Mans field (F538)

Where the tick goes in the 24 Hours career race (17 cars, the player on the
autopilot; `local/repro/lemans-manifold-40552`), current main, `grid-app
--headless` with `GRID_PERF_TIMING=1`, 600-tick windows from the cumulative
reports (`local/r26/perf-lm-full`). Milliseconds per tick; pieces and piece
contacts are the window means.

| tick | drive_tick | world advance | piece contacts | vehicles (17 cars) | commit | AI | frame | pieces | piece contacts |
|---|---|---|---|---|---|---|---|---|---|
| 2399 | 5.6 | 1.9 | 1.1 | 1.0 | 0.5 | 0.17 | 8.1 | 26 | 121 |
| 4199 | 22.9 | 14.4 | 4.8 | 1.0 | 0.9 | 0.16 | 25.2 | 154 | 679 |
| 5999 | 27.4 | 16.7 | 7.0 | 1.0 | 0.9 | 0.16 | 29.5 | 223 | 1035 |
| 7799 | 37.8 | 23.1 | 10.7 | 1.0 | 1.0 | 0.15 | 39.9 | 335 | 1676 |
| 9599 | 44.0 | 27.0 | 12.6 | 1.0 | 1.1 | 0.15 | 46.2 | 403 | 2079 |
| 11399 | 54.9 | 31.7 | 16.9 | 2.3 | 1.2 | 0.17 | 57.3 | 408 | 2115 |

The cars cost 1-2 ms per tick throughout; the pieces' advance and contacts
take 85-90 % once a few hundred pieces lie around. Until F541 the pieces never
slept, so the cost only grew: by tick 40489 of the F513 run 872 pieces were alive.
Milan seed 41, 12 cars, grid-soak to tick 3000: world tick 16.5 ms, of which
advance 9.6, piece contacts 3.7, vehicles 1.2; 72 pieces, 349 piece contacts.

## Piece sleeping (F541)

Pieces at rest now sleep after turnOffTime (1 s) and leave the active list
([BODY_SLEEPING.md](BODY_SLEEPING.md)). Wall times on the shared PC varied up to
4× between runs (the same Milan seed measured 16.5 ms in F538 and 6.9 ms here),
so the cost is compared in one process: a broken Nürburgring pile is cloned,
one copy sleeps, the other is forced awake, and they tick alternately
(`sleeping_cost_on_a_broken_nurburgring_field`).

| Pile | Pieces | Asleep | Sleeping ms/tick | All awake ms/tick |
|---|---:|---:|---:|---:|
| tyre stacks, settled 3000 ticks | 228 | 161 | 2.87 | 5.53 |
| armco and tyre stacks | 755 | 746 | 7.58 | 18.00 |
| armco and tyre stacks | 987 | 856 | 12.18 | 24.85 |

Piece contacts per tick in the Le Mans race fall from about 2,100 (main, 400
pieces) to 300-500 (500 pieces). An asleep piece still costs about 12 µs per
tick: a sample profile puts about half of the tick in whole-world memory copies
(the 0x424-byte body array cloned several times per tick), plus the O(n²)
piece-pair distance pass and car–piece hull tests in `PropWorld::contacts`.
Those are the next levers.

## Catch-up cap (F544)

The interactive app runs at most 4 fixed ticks per rendered frame (`Time<Virtual>` max delta
64 ms instead of Bevy's 250 ms; [FRAME_STEPPING.md](FRAME_STEPPING.md) section 5). While a tick
costs under 16 ms nothing changes: 62.5 ticks/s, identical drive samples. Above that, the game time
slows instead of the frame rate collapsing.

Desktop measurements (cap 0 → cap 4):
- forced 20 ms tick: 3.1 → 11.6 FPS at the same 0.75 game speed;
- the end of an 8000-tick Nürburgring drift battle, about 480 pieces at about 30 ms per tick:
  1.8 → 7.8 FPS, at 0.45 → 0.50 game speed.

`GRID_MAX_TICKS_PER_FRAME=0` restores the old clamp for comparisons, and `GRID_TICK_DELAY_MS`
forces a slow tick.

## Tick cost and parking (F546)

F546 removes per-tick work without changing a result: the identity runs, the grid-soak digests
(Milan seeds 41 and 53, Nürburgring `--chaos prop` seeds 1 and 2), the headless Le Mans career and
Nürburgring drift-battle drive samples and the paired probe's world hashes are bit-identical before
and after each step (evidence `local/r28/`, `signatures.txt` in each run folder). Parking
(F546, [BODY_SLEEPING.md](BODY_SLEEPING.md) section 8) changes the races once a pile parks.

- **Row buffer copies.** The solver's shared row buffer (`Manager::rows`, 4 × 192 bytes per
  contact slot of the pool, megabytes once pieces have grown the pool) was cloned for every car's
  island on every tick: the deformation frames called `impact_frame` by reference
  (`calculate_with_callbacks`). It now moves through them as through the plain frame
  (`calculate_with_callbacks_rows`, `deformation_world::calculate_group(…, rows)`). This was most
  of the Le Mans and Nürburgring world advance (3.5 of 3.9 ms at 164 pieces).
- **Whole-world copies.** The world moves through the tick by value
  (`physical_world::advance_with_*_in`, `grouped_physical_frame::tick`, `contact_groups::build`):
  bodies, contacts and joints are moved, not cloned, and the rows are moved out of the rollback
  copy. One rollback copy of the bodies per tick remains (`MultiRuntime::tick`), so a failed tick
  leaves the previous world; that world has no rows and must be reset (every caller already stops
  or restarts).
- **Piece pairs and car–piece tests** (`PropWorld::contacts`): an asleep piece only pairs with the
  awake pieces after it; a conservative reach bound skips far pairs before any hull is built; hulls
  are built lazily; each car's pose and bounds are computed once per call and boxed against the
  piece before the hull test.
- **Road chunks** (`Track::body_contacts`, `Track::sweep_road`): `packed_contacts::MissTest`
  evaluates the body's terms of `Known::misses` once per body instead of once per chunk, the
  704-byte chunk settings are built once, and the continuous sweep reuses each chunk's load-time
  mesh check instead of re-validating every vertex per piece and chunk.

**Paired probe** (`sleeping_cost_on_a_broken_nurburgring_field`, same pile, one process; the
sleeping copy keeps sleeping, the other has every piece forced awake; base = F543 main c664a9c1,
new = this work; parking off unless stated, i.e. the pile is near the player). ms per tick:

| Pile | Pieces | Asleep (mean) | Base sleeping | New sleeping | New, parked | Base all awake | New all awake |
|---|---:|---:|---:|---:|---:|---:|---:|
| tyre stacks, settled 3000 ticks | 60 | 52 | 0.50 | 0.32 | 0.18 | 1.38 | 1.09 |
| tyre stacks, settled 3000 ticks | 120 | 120 | 1.05 | 0.20 | 0.21 | 3.10 | 1.96 |
| tyre stacks, settled 3000 ticks | 228 | 161 | 2.87 | 1.39 | 0.26 | 5.54 | 3.72 |
| tyre stacks and armco, 300 ticks | 996 | 574 | 17.47 | 9.61 | 1.01 (859 pieces) | 24.62 | 15.87 |
| tyre stacks and armco, 300 ticks | 995 | 491 | 19.25 | 11.24 | 1.31 | 25.14 | 16.44 |

"New, parked": parking on, the two cars on the grid far from the pile, every piece parked (the
755-break pile ends with fewer pieces because parked pieces break no neighbours). An asleep piece
now costs under 2 µs per tick (120 settled tyres: 0.20 ms), down from 9 µs; a parked pile of 1,000
pieces 1.3 µs per piece. An awake piece costs about 16 µs, down from 25 µs; what remains is the
real work (road and piece contact queries, the solver) plus the one rollback copy (about 6 % of an
all-awake tick).

**Races** (headless, autopilot, back to back on the same PC; base F543 → this work with parking):

| Race, window ending at tick | Pieces (base / new) | Asleep / parked (new) | World tick ms, base | World tick ms, new | World advance ms, base → new |
|---|---|---|---:|---:|---|
| Le Mans career, 599 | 0 / 0 | 0 / 0 | 1.13 | 0.96 | 0.38 → 0.27 |
| Le Mans career, 2999 | 32 / 25 | 22 / 17 | 3.77 | 1.69 | 2.14 → 0.48 |
| Le Mans career, 5399 | 180 / 207 | 203 / 187 | 13.99 | 3.23 | 9.47 → 0.73 |
| Le Mans career, 6000 | 236 / 304 | 305 / 288 | 16.57 | 4.01 | 11.11 → 0.72 |
| Nürburgring drift battle, 5399 | 4 / 4 | 4 / 0 | 0.70 | 0.61 | 0.23 → 0.15 |
| Nürburgring drift battle, 7799 | 64 / 63 | 39 / 33 | 3.83 | 2.01 | 2.00 → 0.57 |
| Nürburgring drift battle, 10199 | 161 / 141 | 106 / 79 | 8.91 | 2.41 | 4.75 → 0.67 |
| Nürburgring drift battle, 12000 | 161 / 155 | 138 / 98 | 7.80 | 1.41 | 4.33 → 0.36 |

Windows of 2,400 ticks (`GRID_PERF_TIMING` lines, means over the window); whole runs 67 → 27 s
(Le Mans, 6,000 ticks) and 67 → 33 s (Nürburgring, 12,000 ticks). The Nürburgring battle is the
phone-like case the owner named: the player laps away from the crash sites, so at the end 98 of
155 pieces are parked and 40 more asleep. The races diverge once the first island parks, so the
piece counts differ; base (which already sleeps, F541) has no asleep or parked counters. The world
advance at 0 pieces also falls (0.19 → 0.12 ms on the Nürburgring, 8 cars), from the row move.

## Mobile preset (F549)

Owner, 2026-10-22: a steady 60 FPS on phones, including weaker ones. The phone picture may drop to
PS3-era quality, and the desktop picture stays as it is. `crates/grid-app/src/mobile.rs` is the
Android default. It is off on the desktop unless `GRID_MOBILE_PRESET=1`; `GRID_MOBILE_PRESET=0`
turns it off on Android for comparisons. It changes presentation only.

**What the phone logs showed** (owner's profile APK 6087fbcafa30, Adreno 740, 2308 × 1080, display
60/120 Hz, Fifo; `log-20261022-004650.txt` and its siblings):

- **Front end over the 3D grid, simulation off:** 55 ms per frame, steady for 30 s.
  - `present_frames` waits 21.6 ms per frame: the GPU is the limit.
  - `drive_tick` takes 10.7 ms per frame at 3.3 calls with no ticks running.
  - [I] The first profile window's `allocate_and_free_meshes` (4.6 ms) and
    `prepare_assets<GpuImage>` (3.1 ms) were startup uploads averaged over that window, not
    steady churn. The second window does not have them, and the new `asset_churn` line reads 0
    after the first 10 s on the desktop.
- **Smooth driving:** 25 ms per frame.
  - `present_frames` waits 1.8 ms: CPU-bound.
  - The main app takes 19 ms, of which `drive_tick` is 6 ms.
  - The render thread does 10.4 ms of its own work. The rest of its 23 ms is waiting for
    visibility tasks on the 4 compute threads, each busy about 7.5 ms.
  - Core3d runs twice per frame: the main view and the OSD camera.
- **Entity counts** (desktop, Nürburgring drift battle): 17,785 mesh entities, 15,748 of them
  small static ones (a bounding radius under 4 m), most of them the 11,962 breakable prop
  pieces. Every view and shadow cascade visits them.

**The F549 preset** (F555 keeps only the render rows and the compute threads; see below):

| Change | Value | Why |
|---|---|---|
| 3D view rows (`render_scale`) | 720, stretched over the screen | the GPU-bound phases; this phone rendered its native 1080 rows before. `[video] render_height` still overrides. |
| Sun shadows | 1 cascade, 1024 map, to 45 m | one shadow view instead of four (visibility, binning, the shadow pass) |
| Small static meshes | no shadow; not drawn beyond 110 m (`VisibilityRange`) | most entities are small props; culled ones skip extraction, queueing and batching |
| Ground cover | 0.4 × the authored LOD distances | about 0.16 × the sprites |
| Compute threads | cores − 2, at least 4 (6 on an 8-core phone) | Bevy gives 4 of 8 cores to compute and leaves IO and async threads idle |

**Measurement switches.** `perf_test` keeps the per-part switches. The Auto cycle (15 steps of 8 s since F625's `off:mirror`, the head-cam's rear-view mirror)
now ends with `mobile:render1080` (full resolution), `mobile:render540` and `mobile:msaa_off`; under
the preset, the "normal" step is the preset itself. The Auto cycle also runs in the pre-race menu.
The cycle labels `light_shadows`, `no_small_casters` and `far_props` are for runs without the preset
(`GRID_PERF_TEST`).

The new `asset_churn` line, every 10 s, reports Image and Mesh assets added, modified and removed
per frame, with the most changed meshes named by an entity that draws them.

**Checks:**
- The desktop default is unchanged: every preset change is behind `mobile::enabled()`. The
  `asset_churn` line is the only new desktop output.
- Desktop with `GRID_MOBILE_PRESET=1 GRID_PERF_TEST=auto`: the whole cycle ran without a panic,
  and `render_scale` logged 1152 × 720, then 1280 × 800 native, then 864 × 540.
- [Not measured] The phone gain. The owner's next run of the Auto cycle (in the menu and while
  driving) gives the table.

**Next, by the data:**
- Draw intact breakable props as one mesh each instead of their pieces (about 12,000 entities on
  the Nürburgring).
- Render the OSD only when it changes.
- A lower preset for weaker phones.

### Phone measurement and the reduced preset (F555)

The owner ran the Auto cycle on APK 9ac27f9a958a (F549 preset + F553), Adreno 740, 2308 × 1080. The log is `log-20261022-044620.txt` (main checkout `local/android/phone-logs/20261022-0446/`). The table gives the mean of the `frame_stats` FPS per 8 s step. "normal" is the F549 preset: 720 rows plus the other cuts.

| Step | Pre-race menu, 1st cycle | menu, 2nd cycle | Driving, no debris |
|---|---:|---:|---:|
| normal | 42.1 | 42.5 | 52.8–59.1 |
| shadows off | 43.8 | 44.7 | 56.0 |
| foliage / ground / effects / HUD / samples off (each) | 42.0–42.9 | 42.5–43.0 | 57.1–59.9 |
| all six off | 45.3 | 45.9 | 59.9 |
| only the player's car | 59.5 | 59.9 | 60.1 |
| render1080 (native) | 31.6 | 33.1 | 58.0 |
| render540 | 48.4 | 49.1 | 58.7 |
| MSAA off (720 rows) | 55.5 | 54.8 | (pile-up began) |

Interpretation [I]:

- **Driving** is at or near the 60 FPS cap at any resolution. Even 1080 rows gave 58.
- **The pre-race menu is the slow case.** Its frame time falls only part-way with fewer pixels: 31.6 ms at 2.49 MP, 23.5 ms at 1.11 MP, 20.7 ms at 0.62 MP. That fits about 17.6 ms + 5.6 ms per megapixel. So resolution alone cannot bring the menu under 16.7 ms: even 0 pixels would leave about 17.6 ms.
- **MSAA 4× costs about 5.8 ms at 720 rows** (23.6 → 18.0 ms), more than halving the pixels saves.
- **The rest is the scene behind the menu.** With only the player's car drawn the menu reaches 60 FPS, and the six parts the switches remove (shadows, foliage, grass, effects, HUD, logging) together give only about 3 FPS. So the remaining cost is the other cars and the track and prop meshes in view. With 17,785 mesh entities (Nürburgring), that is most likely per-draw GPU and driver work at 4 MSAA samples. No profile build ran this time, so the split between CPU and GPU is not confirmed.

**F555 preset** (owner, 2026-10-22: "картинка стала очень плохой … обойтись только понижением разрешения"):
- Only the 720-row render and the compute threads remain.
- Shadows, small-prop shadows and the draw distance, and the grass distance are as on the desktop again.

Expected [I, not measured]:
- Driving: about 55–60 FPS. The F549 cuts gave about 0–4 FPS while driving.
- Pre-race menu: about 40 FPS (F549 normal 42 minus their roughly 2–3 FPS).
- With MSAA off, the menu would reach about 52–55 FPS. That is the owner's call: off by default only with the owner's say.
- Dynamic resolution would not reach 60 FPS in the menu either (see the fixed part above).

### Dynamic render height (F556, F559, F571)

**Stuck at 480 rows (F571).** Owner, APKs 3cdbf523a7f0: never below 30 FPS, but the render height
never rose above 480. Phone log `log-20261026-125712.txt` (profile APK, Donington, 12 bmw; worktree
`local/playtest/phone-20261026/`) [O]:
- `render_dynamic` stepped 720 -> 480 in 2.5 s from 14.7 s on. The frame went 41.3 -> 36.0 ms while
  the fixed-step work read 1.3-1.8 ms, so every frame counted as GPU bound.
- From 40 s on, the profile windows show a CPU-bound frame: 23-27 ms with Main 18-24 ms and Render
  21-25 ms. The six compute threads work about 6 ms each per frame. The render thread is inside spans
  for 23-26 ms but works 8.5-10.7 ms. `present_frames` takes 1-3 ms.
- A height is raised only after 3 s under 17.2 ms, which this race never reached, so it stayed at 480.

Fix: every step down is a probe. One second (`PROBE_SECONDS`) later, the smoothed frame time must have
gained at least half (`GPU_SHARE`) of what a frame bound only by the pixel count would gain:
`before x (1 - (to/from)^2)`, 5.4 ms from 720 to 672 rows at 41 ms. Otherwise the height returns to
the 720 ceiling at once and holds there for 60 s (`HOLD_SECONDS`), then probes again.
- `render_dynamic version=2` logs `reason=ineffective` with `probe_gain_ms` and `pixel_gain_ms`.
- Down steps now come once per probe second, so 720 -> 480 takes 5 s when every step helps.
- The fixed-step check (F559) stays.
- [I] On the race above, a step gains about 0.8 ms (F555 fit: 5.6 ms per megapixel, 0.14 MP per
  step) against 5.4 ms, so the height returns to 720. It probes once a minute, at 672 rows for one
  second.
- [I] The pre-race menu (F555: 17.6 ms + 5.6 ms per MP, 24 ms at 720 rows) is also mostly not
  pixel bound (about 26 %), so it now stays at 720 instead of dropping to 480 for about +6 FPS.
- Checked by the unit tests `dynamic_height_returns_when_a_step_does_not_help` (a constant 37 ms
  frame, and a frame of which a quarter scales with the pixels) and
  `dynamic_height_drops_fast_when_late_and_rises_slowly` (pixel-bound frames still go to 480). Not
  verified on the phone yet.

The owner chose a dynamic resolution of 480 to 720 rows (2026-10-22). Since F559 it is the Video
screen's Render height value **Auto** (`[video] render_height = auto`), the Android default; the
range is fixed at 480-720 rows. A row count is a fixed height and `native` turns the scaling off.
The Video screen shows the rows rendered now beside Auto, for example `Auto (624)`.

**What went wrong in F556 (owner, APK 94e06d136784: "no dynamic resolution, the fixed Render Height
is used").** From the code [V, reading]: the controller ran only while the setting was a row count,
that count replaced the 720 ceiling, and the Video screen showed only the saved number, never the
height rendered. The controller treated a frame as main-thread bound when the whole main schedule
(`First` to `Last`) took over 80 % of the frame or 14 ms. On the phone that span is long: the F549
profile (APK 6087fbcafa30) measured the main app at 19 ms of a 25 ms driving frame, and 10.7 ms of
`drive_tick` alone per pre-race frame; the F555 fit (17.6 ms + 5.6 ms per megapixel) also has a large
resolution-independent part. [I] So late frames counted as CPU-bound and the height never dropped.
No phone log of APK 94e06d136784 was available to confirm it (no `render_dynamic` lines to read).

**F559 changes:**
- The work measure is only the fixed-step loop (`RunFixedMainLoop`, the driving ticks), which a
  lower resolution cannot speed up; the hold thresholds are 50 % of the frame or 16 ms (a whole
  60 FPS frame; the paused pre-race ticks' 10.7 ms stays below it).
- Saved heights from before F559 (no `[video] render_height_version = 2`) read as Auto once with
  the mobile preset; the Video screen writes the version with every save, so a fixed height chosen
  after F559 stays fixed. The desktop reads the saved height as before.
- While late frames are held as simulation bound, `render_dynamic … reason=cpu_bound` is logged
  every 10 s (rows unchanged), so a phone log shows why the height does not drop.

The rest of this section is the F556 design, with the work measure as above.

**Per frame**, the controller keeps exponential averages (factor 0.1) of two values:
- the frame time (`Time<Real>`);
- the simulation time: the fixed-step loop (F559; F556 used the main schedule from `First` to
  `Last`).

**The rules:**
- **Late:** the average frame time is above 18 ms (below about 55 FPS).
- **Simulation bound:** the simulation is above 50 % of the frame time, or above 16 ms (F556:
  the main schedule above 80 % or 14 ms). For example a tyre pile; a lower resolution would not
  help, so the height holds.
- **Down:** when late and not main-thread bound, 48 rows lower, at most every 0.5 s, to the floor
  of 480. The height it left is blocked for 15 s.
- **Up:** after 3 s on time (below 17.2 ms), 24 rows higher, but not to a blocked height. From 480
  back to 720 this takes about 30 s.
- **Ignored:** frames above 100 ms (loading, pauses) and the first 3 s.

Render-thread CPU limits are not told apart from GPU limits. They would lower the height for little
gain, but never below 480.

**Logging:**
- `render_dynamic version=1 event=install` gives the parameters.
- Each change logs `render_dynamic … rows=… from=… reason=late|on_time|ceiling frame_ms=… work_ms=…`;
  `reason=cpu_bound` (F559) is a hold report with rows unchanged.
  Changes are at least 0.5 s apart, so the line is rate-limited.
- `frame_stats` shows the current `render=WxH`.

The Performance test's `render…` steps hold the dynamic height (`RenderScale::hold`), so they
compare fixed heights.

**Implementation:**
- Each change allocates a new target image at the new size. Changes are at most 2 per second.
- The UI (HUD, menus, touch zones, FPS counter) is drawn by the window camera whenever the scaled
  image is shown: `ui_on_window` moves the UI roots, and the OSD image follows the window size. So
  the UI keeps the screen's resolution at any render height.
- The desktop is unchanged: it renders native by default, so no image and no move.

**Expected [I, from the F555 table]:**
- Driving: 720 rows, about 60 FPS.
- Pre-race menu: down to 480 rows (1,026 × 480, 0.49 MP), about 17.6 + 2.8 ms, so about 46–49 FPS
  instead of about 40 at 720.
- 60 FPS in the menu still needs MSAA off (about 55 at 720) or a cheaper scene.

**Checks:** unit tests `dynamic_height_drops_fast_when_late_and_rises_slowly` and
`dynamic_height_ignores_a_main_thread_bound_frame`. On the desktop with `GRID_MOBILE_PRESET=1
GRID_PERF_TEST=auto`: 8 UI roots moved to the window at 720 rows and back at native, with no panic.
The desktop never runs late, so no dynamic change was exercised live; the phone log will show it.

**Desktop Auto (F573):**
- The Video screen offers `Auto` on the desktop too. The desktop default stays `native`.
- Range: from native down to half of native, rounded up to the 24-row step (2560 × 1369: 696–1369).
  It starts at native, and while frames are on time it stays there (native means no scaled image).
  A resized window starts again at its new native height (`render_dynamic version=2 event=range`).
- Controller changes for both platforms, found by the desktop test (a jump from 7 ms to 80 ms
  frames): the probe stepped down while the smoothed time was still rising, and judged the step
  ineffective.
  - A step down waits for 20 consecutive late frames (`LATE_FRAMES`).
  - The probe compares a frame time smoothed by 0.25 per frame instead of 0.1. One probe second at
    80 ms is 13 frames; with 0.1 the average would still be settling. Late and on-time decisions
    keep the slow average.
- Test `desktop_auto_range_starts_native_and_stays_there_on_time`.

**High-DPI windows and larger steps (F642):** the owner's MacBook log (Apple M4, 3024 × 1890,
Detroit Derby, F640) showed three faults with dense tyre smoke:
- Native and the `Auto` ceiling were 1890 rows (5.7 MP). On a window with a scale factor above 1
  the desktop `Auto` ceiling is now at most `HIDPI_CEILING_ROWS` = 1440 (2304 × 1440 there, 58 % of
  the pixels), the floor half of that. macOS defaults to `auto`, since a Mac's built-in display is
  high-DPI; other desktops keep `native`. A Windows display scaled above 100 % gets the same cap
  under `Auto`.
- A step was 48 rows, 2.5 % of the pixels at 1890 rows: a 68 ms frame cannot show that gain above
  its noise. A step now goes to the height where a pixel-bound frame would take 16.7 ms
  (rows × √(16.7 / frame ms)), in whole 24-row units, at least 48 rows and at most a quarter of the
  height (`MAX_DOWN_SHARE`). From 1440 rows at 68 ms that is 1080, then 816, then the 720 floor.
- The probe from 1818 to 1770 rows measured −46 ms (smoke thickening) against 2.1 ms expected,
  was called `ineffective` and returned to native for 60 s. A frame slower by more than the
  expected gain is now inconclusive: the lower height stays and later steps can continue. A frame
  gaining little but not slower (CPU bound, F571) still returns to the ceiling.
- Frames up to 250 ms count (was 100 ms): the smoke held 87–102 ms frames for seconds.
- Logs are `render_dynamic version=3`; `event=range` adds `scale_factor`.
- Not verified on a Mac: unit tests only (`dynamic_height_hidpi_range`,
  `dynamic_height_steps_by_the_frame_time`, `dynamic_height_keeps_a_step_when_the_scene_gets_slower`).

### Compute threads on the desktop (F573)

`mobile::compute_threads` (cores − 2, at least 4) against Bevy's default split (compute gets 12 of
20 threads on the measuring PC, an i7-12700 with 20 logical cores). Switch for measurement: `GRID_COMPUTE_SPLIT=preset`.

Runs from 14:54 to 15:04 on 26 Oct 2026 (after 14:07, under the build lock, so no other cargo
build ran). Offscreen 2560 × 1369, autopilot, 3,600 benchmark frames, alternating:

| Run | Compute threads | DriveTick ms | Main ms | Render ms | p95 ms |
|---|---|---|---|---|---|
| race 1 | 12 | 1.51 | 5.02 | 9.47 | 20.04 |
| race 1 | 18 | 1.24 | 4.56 | 7.24 | 18.21 |
| race 2 | 12 | 1.22 | 4.40 | 7.09 | 18.13 |
| race 2 | 18 | 1.20 | 4.43 | 7.03 | 18.07 |
| derby 1 | 12 | 1.76 | 3.88 | 5.60 | 18.74 |
| derby 1 | 18 | 1.67 | 3.79 | 5.48 | 18.68 |
| derby 2 | 12 | 1.71 | 3.80 | 5.60 | 18.75 |
| derby 2 | 18 | 1.69 | 3.77 | 5.44 | 18.63 |

(race: Donington, 12 cars; derby: `settings/demolition-derby.ini`. Mean frame time is 17.0 ms in
every run: the benchmark is paced at 60 Hz.)

- The first race pair is a warm-up outlier (the first run after the build). From the repeats, the
  gain is 0–3 %, which is within run-to-run noise.
- The simulation is unchanged: every derby run logged the same 6 breaks at the same ticks. The
  parallel paths are index-ordered (F568).
- Decision (F573; F860 later capped both splits at 8 threads): Bevy's split stays the desktop default. The preset split stays on Android (and with
  `GRID_MOBILE_PRESET=1`, which now also applies it on the desktop).

## Dormant pieces (F574)

Every breakable's pieces are spawned with the track and stay hidden until the breakable breaks:
14,463 drawn pieces on Donington and 11,962 piece entities on the Nürburgring. Hidden, they still
cost CPU every frame in each pass that walks the mesh entities. On the phone (log
`log-20261026-125712.txt`) these passes were among the top spans:
- view visibility: 2.65 + 1.27 ms;
- the directional light's casters: 1.9–2.6 ms;
- changed-mesh checks: 1.3 ms;
- Aabb: 0.76 ms.

`lean_pieces.rs` (every build) gives a hidden piece of an unbroken breakable Bevy's `Disabled`, which
the default query filter skips. Timing:
- from frame 30 on, then again every 60 frames (after a restart clears the breaks);
- the piece wakes, losing `Disabled`, before `dynamic_props::present`, in the frame its breakable
  breaks.
Such a piece is never drawn, so the picture does not change. `GRID_LAZY_PIECES=0` turns it off. The
log shows `lean_pieces version=1 event=rest frame= disabled= dormant=`.

**Checks (26 Oct 2026, 14:28–14:36, after 14:07):**
- Image A/B on the Detroit derby at tick 2,700 with `--render-fixed-step`, 1600 × 900, 6 breaks
  before the capture:
  - a repeated baseline gives 0 changed pixels;
  - dormant on against off also gives 0 changed pixels, with broken debris in the picture.
- Phone view on the desktop (`GRID_MOBILE_PRESET=1`, 4 threads, Donington, 12 cars, 1539 × 720,
  profile build, 3,600 frames, 2 runs each). Another session's cargo test held the build lock
  during these runs, so the absolute values carry load noise.

| | Main ms | Render ms | DriveTick ms |
|---|---|---|---|
| off (`GRID_LAZY_PIECES=0`) | 6.05 / 6.16 | 9.75 / 9.46 | 1.47–1.54 |
| on | 5.81 / 5.64 | 9.07 / 8.87 | 1.47–1.54 |

- Entities (`GRID_ASSET_COUNTS=1`):
  - main world 21,589, of which 14,706 pieces are disabled (disabled entities still count);
  - 18,320 mesh assets.
- In the span tables, the Aabb pass (0.77 ms) and the changed-mesh pass (0.74 ms) drop out of the
  top entries, and view visibility goes from 0.675 to 0.472 ms.
- Phone estimate [I]: the disabled passes cost about 6–7 ms of compute-pool work per frame on the
  Adreno phone, which is maybe 2–3 ms on the critical path. Not measured on the phone yet.

`GRID_ASSET_COUNTS` now also prints `mesh_entities` with all/disabled/drawn counts by kind (track,
intact, piece, foliage, player, opponent, other).

## Static world batching (F576)

`static_batch.rs` (every build) merges the track's static parts into one mesh per material and
64 m ground cell, at `PostStartup` after the track surfaces are attached.
- Vertices are moved into the cell's frame as the vertex shader would place them:
  - positions by the part's matrix;
  - normals by its inverse transpose, normalized as the shader does;
  - tangents with the determinant's sign;
  - triangle order and winding kept.
- Opaque and Mask materials only. Blended parts, foliage that faces the camera, car parts, pieces,
  parts with children, render layers, `NotShadowCaster` or `VisibilityRange` keep their own entities.
- A batch holds at most 65,536 vertices (16-bit indices). A cell with a single part keeps it as is.
- The original static entities are despawned, which frees the meshes only they used.
- Intact breakable props get batches of their own:
  - their entities stay, without a mesh, and keep the visibility that `dynamic_props::present`
    and the replay set;
  - a batch drops a prop's triangles while the prop is hidden (broken) and restores them when it
    shows again (a restart);
  - a batch with nothing left is hidden.
- `GRID_STATIC_BATCH=0` keeps every part separate. `GRID_STATIC_BATCH_CELL=<metres>` changes the
  cell size (for measurements).
- Log: `static_batch version=1 material= parts= batches= intact_parts= intact_batches= vertices=`.

**Checks (26 Oct 2026, 15:49–16:18, after 14:07; timing runs under the build lock):**
- Images, offscreen 1600 × 900 with `--render-fixed-step`, batching off against on:
  - Donington, 12 cars, tick 600: 1 pixel differs, by 1 level;
  - Detroit derby, tick 2,700, with breaks before the capture: 3 pixels differ, by 1 level.
- Donington batches: 1,937 static parts became 569 batches; 1,609 intact props became 207 batches;
  16 surface-material parts became 7 batches.
- Drawn mesh entities at frame 3,600 (`mesh_entities`, phone view):
  - track and intact: 2,467 + 1,199 = 3,666 without batching;
  - 927 + 618 batches = 1,545 at 64 m;
  - 746 + 481 = 1,227 at 128 m.
- GPU memory (`wgpu_memory`): Donington 472 → 532 MB allocated, because instanced props are copied
  into their batches; derby 399 → 407 MB. Reserved memory unchanged.

| Phone view (desktop, mobile preset, 4 sim threads), 2 runs each | off | 64 m | 128 m |
|---|---|---|---|
| Main ms (release) | 4.27 / 4.39 | 4.39 / 4.32 | 4.37 / 4.26 |
| Render ms (release) | 7.17 / 7.32 | 7.07 / 7.15 | 7.13 / 7.04 |
| Compute-pool work ms/frame (profile build) | 9.18 / 9.24 | 8.84 / 8.84 | — |

- In the profile build, the main visibility pass goes 0.36 → 0.23 ms per frame and the Aabb pass
  0.085 → 0. Other per-entity spans shrink by 0.01–0.04 ms. The render thread is flat (3.49 / 3.45
  → 3.45 / 3.43 ms).
- Desktop, 2560 × 1369, no preset, 2 runs each: Main 4.24 / 4.30 → 4.27 / 4.30 ms, Render 6.88 /
  6.92 → 6.84 / 6.79 ms. That is within noise; the desktop's per-entity passes are cheap.
- Phone estimate [I]: the saved compute work scales with the phone's slower cores (about 3–4×), so
  roughly 1–1.5 ms of compute-pool work per frame. On the critical path that is likely under 1 ms.
  Not measured on the phone.
- Unit test `parts_keep_their_shader_placement_and_order`.
- Not verified: a restart restoring broken props in a batch (the code path is shared with replay
  visibility); tracks other than Donington and Detroit.

## Per-frame mesh work: ground cover and skid marks (F580)

**Ground cover.** Before, `ground_detail::present` rebuilt and streamed every visible billboard
whenever the camera moved or turned, which is every frame while driving. Now:
- a material extension (`ground_cover.wgsl`, vertex, prepass and deferred stages) faces each
  billboard to the main camera and fades it with distance, using the CPU path's formulas;
- the camera arrives in a uniform, so every pass faces the main camera, as the CPU path did;
- the vertex buffer holds each billboard's centre four times, the corner offsets (sprite corner ×
  width, height) in UV1 and its draw distance in the colour's alpha. A billboard past its distance
  becomes a degenerate quad;
- the CPU gathers every instance within its distance plus 6 m (`MARGIN`), in the old bucket order.
  The gather stays valid for 6 m. The next one starts on the async compute pool after 3 m, and its
  bytes are written over the mesh's allocation (mesh_stream) when ready. Blended cover (Enhanced
  soft alpha) also gathers again at once in a new 16 m bucket, because the bucket order sets its
  draw order;
- blended cover still gets the exact sort centre of its drawn quads each frame. Masked cover skips
  it, since it is not sorted;
- motion vectors use the current corner for the previous position, as Bevy did with the CPU-written
  vertices.

**Skid marks.** Before, every change replaced the whole mesh asset, so Bevy reallocated and repacked
it about once per frame in a race. Now:
- the mesh holds every strip slot at full length (240 × 33 points);
- changes are written in place;
- points past a strip's end repeat its last point, so their quads have no area. The triangles with
  area, their order, the packed bytes and the sort centre equal those of the old mesh (unit test).

Tyre smoke was already streamed in place (F392/F394), and source foliage is opt-in
(`source_trees = false` by default). Neither changed here.

**Checks (26 Oct 2026, 17:00–17:50, after 14:07; timing under the build lock):**
- Images, 1600 × 900 with `--render-fixed-step`, F576 build against this one:
  - Donington tick 600, baseline (alpha to coverage, grass in view): 0 changed pixels;
  - Enhanced preset (blended grass, TAA): 7 pixels differ, by 1 level;
  - Detroit derby tick 2,700: 0 changed pixels.
- No capture showed skid marks; the unit test covers their equivalence.

| Phone view (desktop, mobile preset, 4 sim threads), 2 runs each | F576 | F580 |
|---|---|---|
| Grass CPU ms/frame (`GRID_GROUND_TRACE`) | 0.760 / 0.755 | 0.058 / 0.060 |
| Skid CPU ms/frame | 0.004 / 0.004 | 0.013 / 0.012 |
| Main ms | 4.31 / 4.31 | 3.21 / 3.27 |
| Render ms | 7.11 / 7.04 | 6.50 / 6.52 |
| p95 / p99 frame interval ms | 18.03, 20.68 / 18.01, 20.54 | 18.24, 20.09 / 18.18, 21.33 |

- Mesh assets modified per frame (`asset_churn`) drop by about one (the skid marks).
- A first version gathered on the main thread: about 1.5 ms every ~6 frames (17,000 billboards).
  That raised p95 to 19.0 ms and put about 225 frames in 18.5–19.5 ms (against about 25). With
  the async gather that bin holds 65–71.
- The remaining spread comes from the 4.7 MB vertex upload of each new gather (21,000 × 4 vertices ×
  56 bytes); before, about 2.3 MB went up every frame.
- Desktop, 2560 × 1369, no preset, 2 runs each (17:42–17:47): Main 4.30 / 4.26 → 3.13 / 3.32 ms,
  Render 6.83 / 6.79 → 6.29 / 6.31 ms, p95 18.08 / 18.14 → 18.30 / 18.23 ms.
- Phone estimate [I]: the grass work moves off the main thread; on the phone it was 1.6–2.0 ms per
  frame (`ground_detail::present`, log `log-20261026-125712.txt`). Main thread about 1.5–2.5 ms
  faster, render thread about 1–1.5 ms. Not measured on the phone.

## Dedicated graphics rendering work (G242, 23 October 2026)

The graphics worktree now has a dedicated grass/rendering-performance agent, with a separate static-environment audit. Physics/tick/AI and generic mesh streaming remain outside this task. Current mesh/material sharing and F392 buffer reuse are already implemented; they must not be presented as new savings. Entity/source draw-record counts are not measured GPU draw calls.

First candidate: [conservative grass bucket rejection](GRASS_RENDERING.md#g242-conservative-cpu-bucket-evaluation-23-october-2026), preserving the existing quality profile. Build/test and six paired timing runs completed: the candidate increased median CPU grass cost in all three pairs and was rejected. Production grass source is restored exactly; no FPS improvement is claimed. A second unapplied candidate shares immutable tree source arrays to reduce duplicate allocation, with no promised frame-time gain. Source forest sheets, shadow range/resolution, grass density and reflection update cadence are retained. More ambitious LOD/batching/per-view changes require separate source/visibility/temporal evidence and measured bottlenecks.

## Bounded tree-facing dependency census (G265, 24 October 2026)

An opt-in `GRID_FACING_CENSUS=timing:N|keys:N` samples 1..3600 calls of tree-facing presentation. It retains every existing vertex expansion, stream copy and upload; no position cache, visibility reduction or quality change is implemented. With the variable absent, the census is disabled. Invalid values print an explicit disabled message. An exhausted budget is not evidence of a usable scene sample.

Timing records an inclusive wall CPU span from the camera query through the final stream write, including diagnostic work and any stream-lock wait. It is neither GPU time nor display FPS. Key mode adds a bounded 16,384-entity metadata map. Spherical keys use the exact six float32 words of camera right/up; cylindrical keys use eye X/Z. Signed zero is distinct. Nonfinite inputs, reentry, changed components and replaced mesh IDs cannot count as equal-key samples. Counts include presented objects that may later be culled, rather than visible GPU draws.

Reports at call240, multiples of600 and the final budget retain cumulative counts and time. A budget360 with benchmark120 covers its 240 warmup calls and following120 calls; subtract cumulative boundaries for the later interval. A short run without the final report is incomplete. Equal keys indicate potential reuse eligibility, not a cache hit or measured saving. Camera jitter can eliminate practical repeats, so a static result alone cannot justify a driving optimization.

The default path gains a Local lookup and query metadata; diagnostic key bookkeeping also costs CPU. Compare uninstrumented parent, instrumented Off, Timing and Keys with repeat controls before attributing timings. Existing shadow/reflection geometry must still be uploaded even when future keys repeat; first allocation and replacement cannot use unexpanded source positions. The current ordinary main-camera adaptation is also not original per-pass billboard parity. Initial compilation and rendered controls are recorded separately in G265; no performance improvement is claimed here.

G265 validation passes 323 app tests and Clippy, followed by a fresh baseline-feature build. Eight common-profile static controls compare parent / Off / Timing / Keys / Keys / Timing / Off / parent. The shared fixture explicitly removes the old optional paint, lamp and glass pilots and uses unfiltered1280x800 with the same source lighting, covered trees, static probe and authored camera. It is a CPU fixture, not a replacement DLAA quality profile.

All eight final decoded images match exactly. Actual later120-call counts give 577 spherical and three cylindrical instances per call, with100% potential equal-key eligibility and no invalid samples. Timing-only inclusive CPU work averages0.978/0.981ms; Keys averages1.044/1.038ms. Main wall interval means are16.961..17.008ms, not GPU timestamps or display FPS. These are observations of the unchanged work, not savings. Driving key reuse and an actual cache remain unverified.

A preceding identical cohort also passed image neutrality, but its timing was rejected: a private PowerShell7 build-lock wrapper consumed CPU while mishandling a parsed DateTime queue record. Its original results remain intact. The confirmed loop was stopped, the standalone job completed using documented Windows PowerShell5, and the eight controls above were repeated afterwards. No shared build-lock tool or engine behavior was changed to repair that private setup. Exact source/executable pins, both cohorts and separate root admissions are retained privately in `local/graphics/facing-census01/`.

## Moving tree-facing key control (G268)

Two controls use the pinned published G265 executable and immutable source objects, not the later working source. Both use the same new1280x800 unfiltered driving fixture, explicit autopilot and source lighting, with census absent versus Keys360. Both processes finish successfully, actual sampled simulation/ground traces and request metadata agree, and all60 motion PNGs plus the final decoded image are exactly equal. Root reviewed the native first/final views. This is not native DLAA quality acceptance.

Reports240 and360 are bracketed by actual ticks497..498 and747..748. The sampled car moves147.212m between endpoints, with speeds33.128..57.298m/s and no disabled/terminal state. These are log tick brackets, not screenshot Extract/readback timestamps. In the later120 calls, all69,240 spherical and360 cylindrical instance visits change their exact keys. None repeat; there are no nonfinite, cold, changed-component/mesh or untracked samples. The observed stream work represents15,020,640 spherical and701,280 cylindrical vertices, not visible GPU draw counts.

The stationary100% potential reuse from G265 therefore does not justify an exact-key cache for this moving interval. No cache or performance improvement is implemented. Future work should reduce expansion/upload work while preserving geometry, rather than quantizing keys or reducing quality without evidence. The controls are not a timing benchmark.

The first Off renderer also completed successfully, but its private parser incorrectly expected the adjacent float32 reciprocal rather than the actual Duration-to-Bevy serialized timestep. That failure remains intact. A one-line exact-token correction passes23 CPU/read-only controls before the fresh pair above; no tolerance was introduced. Private provenance and both attempts remain in `local/graphics/facing-driving01/`.

G270 separately checks a possible exact-input remap against the four actual Donington spherical sources. All844 complete centre.xyz/UV1.xy raw-float-bit tuples are distinct. Weighted577 placements require125,172 expansions per call, agreeing with the census. Centres repeat, but offsets distinguish the full inputs; placements also have different inverse transforms/scales. This remap would eliminate no expansion in this fixture, so it is not implemented. The existing survey/stream readers are reused and game bytes stay private. No new render, performance measurement or all-track claim follows.

### Current all-view eligibility audit (G273)

The existing tree producer runs in `Update`, before Bevy0.19.1 computes current
visibility. Reading `ViewVisibility` there could use a previous-frame or initial
hidden bit and miss geometry when it returns to view. A main-camera frustum alone
also omits reflection views and shadow-only casters. Trees keep their expanded
bounds and cast shadows in the current app.

A current all-view signal is available later in `PostUpdate`, after
`VisibilitySystems::MarkNewlyHiddenEntitiesInvisible`: camera checks and deferred
light/shadow marking precede that final step. Per-camera `VisibleEntities`,
`CascadesVisibleEntities` and point/spot visibility lists can explain membership.
This is conservative render eligibility, not measured draws or fragment coverage;
`NoCpuCulling` can make it broader still. Moving the producer or skipping its
writes has not been implemented or validated.

The scene-reflection probe retains six active layer0 cameras in moving mode.
After initial readiness, its six-face copy batch is submitted each render
invocation; the first/every120 log frequency is not update cadence. Initial
60-frame/pipeline/image copy guards do not pause the cameras. Static mode removes
capture cameras after the first copied cube; sky-only mode does not install them.
Active cameras indicate scheduling eligibility, not proof of completed views or
draw counts. Their broad union may leave little tree work to exclude.

The next useful check would be a bounded observer-only census after final
visibility, retaining every existing expansion/write and counting main, probe,
shadow-only, hidden and newly visible trees. First-visible/replaced meshes must
still receive current full bytes through F392 streaming; an absent GPU allocation
cannot be treated as initialized. No runtime visibility census, safe skip
population or performance gain is established. Cached source LOD distances exist,
but original shadow/environment LOD or exclusion policy remains unproven; see
[per-view foliage](PER_VIEW_FOLIAGE.md). No quality or probe-cadence reduction is
proposed.

## Current all-view foliage observer (G275, 24 October 2026)

`GRID_FACING_VISIBILITY=1..360` enables a bounded observer after final current-frame
camera and shadow visibility. The producer retains every original expansion and
stream write. It joins entity, mesh handle, component change ticks, asset-event
epoch, producer/frame/camera state and current main/probe/other/shadow membership.
Missing, contradictory, changed or incomplete observations are explicitly unknown.
A main-world asset-event epoch does not establish GPU allocation or upload success.
The metadata allocations and visibility-list scans cost diagnostic CPU work; the
feature is disabled when the variable is absent or invalid and removes its resource
at the final budget. No culling, caching, camera or quality change is implemented.

The required app gate passes365tests and Clippy; a fresh baseline build succeeds.
In a same-executable moving Off/On pair, both render processes complete successfully
and actual simulation/ground traces, environment, assists, camera state, source-tree
inventory and150request-state rows agree.150of151decodedimage pairs match exactly,
including the final image; frame0016 differs at eight distant pit-building pixels
(maximum channel difference7, alphaexact). Native first/0016/final views were reviewed.
Strict image-neutrality remains unproven; the raw nonzero validator result is retained.

At reports240/360, actual active main/probe/other/nocpu counts are[1,6,1,0]. The private
validator had incorrectly required other0 and therefore refuses admission. The extra
camera is consistent with the enabled source OSD's Camera3d, separate target and render
layer23, corroborated by its load log and visible HUD; this identifies a source-backed
explanation, not a new live camera-entity stamp. Other-camera foliage membership is0.

The later120calls contain69240spherical and360cylindrical instance visits, with no
unknown/invalid joins and zero hidden visits in the observed current CPU union. All
visits belong to probe visibility. Main membership is2010spherical/127cylindrical,
and shadow membership44184/360; overlapping counts must not be summed as distinct
objects. The stream represents15020640+701280expanded vertices, not completed GPU
views/draws/uploads. Reports bracket actual driving ticks497..748,147.212m endpoint
displacement and33.128..57.298m/s sampled speed. Generic motion request ticks are not
pixel Extract/readback timestamps. No timing benchmark or performance gain is claimed.
A policy that skips every non-main-visible tree would discard observed probe/shadow
members in this fixture; reducing probe cadence or quality remains a separate decision.

The first attempt completed its Off renderer but stopped before On when the owner's
main game appeared; it was left running. After the owner closed it and actual process
absence was checked, the pair above used a fresh output directory. Both attempts and
all raw failures remain in ignored `local/graphics/visibility-census01/`.

The completed Off from the interrupted first attempt also has identical canonical
arguments, source/executable/input pins and trace/request/runtime metadata. All151of
its decoded images equal the second Off exactly; comparing it against On reproduces
the same eight pixels. Existing repeat evidence therefore does not establish noise.
The diagnostic remains default-off with unresolved strict image-neutrality, and the
original failed admission is preserved. No additional renderer or tolerance was used.
## Car part batching (F584)

Before, every car part was its own entity: at Donington 107 player parts and 1,177 opponent parts,
all drawn, each going through visibility, the shadow cascade, mesh collection and the draw queues.
`car_batch` (common build; `GRID_CAR_BATCH=0` keeps the parts separate) now merges each car's parts
per material into one mesh in the car's frame:
- members: no wheel (it turns on the car), an opaque or masked material, an indexed triangle list
  with the static batch's attributes, and no per-entity render setting (render layers, shadow or
  range settings, parents, facing foliage, the sidewall filter). At Donington: 888 parts in 108
  groups of 12 cars, one batch each;
- the vertices are baked with the part's rest matrix (`static_batch::build`); the batch's transform
  is the car's pose, taken from its largest part;
- a batched part keeps its entity and every component the presentation writes (transform,
  visibility, material, deformed mesh) and moves to render layer 30, which no camera or light
  draws;
- each frame (PostUpdate, after the asset events, before transform propagation) a part stays in
  its batch while it is at its rest place on the car (1 mm, 1e-4 on the axes), not hidden, and
  keeps its material and mesh. Otherwise it leaves and is drawn on its own: a loose or detached
  panel, smashed glass, cracked-glass material. A part back in place for 30 frames (a restart)
  rejoins. When most of a car's parts disagree with its anchor, the car's batches hold for that
  frame and the next part anchors;
- a deformed member mesh (body damage) rebuilds its batch the same frame, so the dents show as
  before.

Entity counts, phone view, frame 3,600 (drawn): opponent parts 961 → 375 (wheels, glass and other
blended parts) plus 108 car batches; player parts 107 → 33. Rebuilds in that race: 1,132, of which
827 for deformation, 83 ms in all (about 0.02 ms per frame).

**Picture.** Not bit-identical. Fixed-frame renders, 1600 × 900, Donington, on against off with the
same executable: tick 300 0.085 % of pixels differ (largest channel difference 20), tick 2,400
0.21 % (29), tick 6,000 after contact 0.17 % (24). The changed pixels are single MSAA samples on
coplanar stickers (number band, NGK, KW, LUK) and their edges, where body and sticker depths
z-fight; the baked vertex positions round differently from the per-part model matrices. Zoomed
crops look the same, and the same differences remain with shadows off. Two renders of one path are
identical. Motion check (the coordinator's condition for default on): 120 consecutive frames at
virtual 60 fps behind the player's car. Pixels that change by more than 24 levels and back within
three frames: off 640,383, on 640,337; in the car's region: off 111,620, on 111,573. Batching adds
no flicker, so it is on by default; `GRID_CAR_BATCH=0` turns it off for comparison.

**Cost** (desktop, phone view 1539 × 720, mobile preset, 4 compute threads, profile build, two
alternating rounds, 26 October 23:25–23:37), off → on:
- `prepared_mesh_producer` 1.12-1.17 → 0.51 ms, `collect_meshes_for_gpu_building` 0.39-0.41 →
  0.28-0.31 ms, render schedule 20.0-20.8 → 19.0-20.0 ms, extract 4.8-5.0 → 4.1-4.8 ms;
- main thread about even: `car_batch::refresh` 0.16-0.18 ms, the visibility passes and
  `mark_dirty_trees` save about as much; PostUpdate 4.31-4.54 → 4.25-4.50 ms;
- frame mean 22.8-23.6 → 21.9-22.9 ms (the PC was slower in this period than at 22:12, when a
  first pair gave 17.0 ms for both, near the 60 Hz pacing).

Phone [I], not measured: about 3x the desktop CPU times, so about 2 ms less on the render thread
and no change on the main thread, which the phone log shows as the critical path.

## PostUpdate: shadow and bounds passes (F588)

What PostUpdate spends in the phone view (desktop, profile build, after F584), per frame:
- the directional light's cascade visibility (`check_dir_light_mesh_visibility` and its parallel
  loops) about 0.55 ms;
- camera visibility about 0.35 ms, the visibility resets about 0.17 ms, bounds (`calculate_bounds`)
  0.14 ms, `update_mesh_previous_global_transforms` 0.16 ms, `mark_dirty_trees` 0.14 ms,
  `car_batch::refresh` 0.17 ms;
- several Bevy systems show far more inclusive than exclusive time (for example
  `check_entities_needing_specialization` 0.23-0.29 ms inclusive, 0.02-0.08 ms exclusive): they wait
  for their parallel loops on the compute pool [I].

Mesh asset changes are not the cause: a probe (not committed) saw a mesh change in 117 of 600
frames (wheel debris, and one car's parts and its batch during a deformation burst), so the
`AssetChanged` filters cost nothing on most frames.

Changes, picture bit-identical (fixed-frame renders at ticks 300, 2,400 and 6,000: 0 pixels differ):
- batched car parts also carry `NotShadowCaster` while they are batched (removed when they leave),
  so the cascade passes skip them: 163 → 124 parallel batches per frame;
- car batches, static batches and single static track parts carry `NoAutoAabb` with their bounds
  computed once. Car parts keep the bounds the presentation sets on deformation. Bevy's per-frame
  bounds pass now checks only the other meshes.

Result, F584 → F588 executables alternating, two rounds (27 October 01:30-01:37): bounds 0.14 →
0.07 ms, cascade loops 0.35 → 0.32 ms, about 0.1 ms in all; PostUpdate 4.41-4.64 → 4.57-4.61 ms,
inside the noise. Phone [I]: about 0.3 ms.

## Shadow view preparation: single static parts (F591)

Round 2, step 3. Per cascade, Bevy checks every shadow-casting mesh entity, then specializes,
queues and batches the visible ones. A batch-set probe (not committed) counted the work per view at
Donington: 11, 19, 31 and 15 batch sets in the four cascades and 80 in the opaque pass, over three
vertex slabs. `write_binned_instance_buffers<Shadow>` (about 0.65 ms) writes two small buffers per
batch set and view, so it follows the batch sets, not the entities. The entity passes follow the
entities, and the largest group left after F584 was about 1,000 track parts that the static batch
kept because each was alone in its material and 64 m cell.

Now such a part gathers with the other single parts of its material in a 256 m cell (4 × 4 cells)
and is merged there; a part still alone keeps its entity. Donington: 282 more parts merged into
206 more batches (StandardMaterial 1,937 → 2,219 parts, road surface 16 → 209); 569 parts stay
single; track entities 1,044 → 569, static batches 783 → 989, 269 fewer mesh entities in all.

Picture, fixed-frame renders 1600 × 900 against origin/main F588: tick 300 460 pixels differ by at
most 2 levels, tick 2,400 none, tick 6,000 4 pixels by 1 level. (A first comparison against an
executable built before F585 showed a different race: F585 changed the AI, which the autopilot
uses.)

Cost (desktop phone view, profile build, origin/main against F591 alternating, two rounds,
27 October 04:31-04:38): PostUpdate 4.56-4.63 → 4.31-4.46 ms, cascade visibility 0.17 → 0.15 ms,
camera visibility 0.16-0.17 → 0.15 ms, `collect_meshes_for_gpu_building` 0.29-0.31 → 0.27-0.28 ms,
`queue_submit` 2.05 → 1.76-1.88 ms, frame mean 23.1-23.5 → 22.1-23.4 ms. The shadow instance
buffer writes stayed at 0.64-0.68 ms. Phone [I]: about 0.6 ms less on the main thread.

## Render-thread chain: streamed mesh bytes (F592)

Round 2, step 4. The render thread's largest items in the phone view (desktop, profile build,
after F591): `queue_submit` 1.7-2.0 ms, `prepare_preprocess_bind_groups` about 1.0 ms,
`grid_app::mesh_stream::upload` 0.6-0.8 ms, `prepare_clusters_for_gpu_clustering` 0.7 ms, the
shadow instance buffer writes 0.65 ms, `prepared_mesh_producer` 0.5 ms (1.1 ms before F584).

A byte probe on `mesh_stream::upload` (not committed), frames 2,400-3,000 at Donington:
- skid marks: the whole mesh, 760 KB, in 548 of 600 frames (694 KB per frame), although a frame
  changes only the strips being laid or faded;
- ground cover: 4.7 MB per regather, 150 regathers (1.18 MB per frame), plus three smaller cover
  and smoke streams of 28-110 KB.

Change: `MeshStream::write_at` writes part of a mesh's vertex buffer at a vertex offset; the rest
keeps the bytes written before (Bevy's slab allocator copies a slab's contents when it grows it).
`skid_marks` keeps a copy of the strips as last written, and packs and writes only the runs of
strips that differ. A unit test checks that the partial writes over the previous bytes equal the
whole packed buffer through laying, chaining, ending, fading and clearing.

Picture: fixed-frame renders at ticks 2,400 and 6,000, F591 against F592: 0 pixels differ.
Cost (F591 and F592 alternating, two rounds, 27 October 05:02-05:09): `skid_marks::present` on the
main thread 0.25 → 0.09 ms; `mesh_stream::upload` 0.78 → 0.59-0.77 ms, which the ground cover's
regathers dominate. Phone [I]: about 0.4 ms less on the main thread.

Left: ground cover writes all its billboards on each regather. Stable slots for the billboards
would let it write only the ones that enter or leave; done in F846 ("Ground cover: stable
billboard slots (F846)").

## Grid start: GPU passes and opponent shadow bodies (F600)

Owner report, APK 3c7a3516: 30-50 FPS in the race, but below 30 FPS at the very start, where the
render height drops. Phone logs (local/phone-start/, Adreno 740) [O]:
- Nürburgring, cars waiting on the grid (16-38 s): a steady 35.4 ms frame (max 37). The render
  thread waits 11-13 ms per frame in `present_frames`, and both threads are under 21 ms, so the
  GPU sets the frame.
- `render_dynamic` drops to 672 rows, gains about 2 ms (probe 2.12 ms against 4.56 ms if the
  frame scaled with pixels) and returns to 720 rows as "ineffective" a second later: that is the
  dip the owner sees. About half the GPU frame does not scale with pixels.
- After the start the frame falls to 30-33 ms, and to 22-25 ms as the field spreads.

Tool: `gpu_passes` (on in the profile build, or `GRID_GPU_PASSES=1`) logs per render pass the GPU
time from timestamp queries, with primitives and shader invocations where the device has pipeline
statistics (the Adreno 740 has both), and the triangles each view submits by owner from the
visibility lists (`view_triangles`). Bevy does not time its shadow passes; their triangles show
their size. Since F863 each line also carries `cpu_ms`, the render thread's time inside the
pass span (Bevy's `elapsed_cpu`), which is the pass's share of `r_encode`.

PC, desktop phone view (mobile preset, 1539x720), Donington, 12 cars, frozen at tick 1 [O]:

| view | before | F600 |
|---|---|---|
| camera | 806,139 (opponent cars 268,699) | 806,139 (268,699) |
| cascades 0-3 | 124,843 + 248,016 + 442,119 + 645,647 = 1,460,625 (opponent cars 747,022) | 81,778 + 114,497 + 193,398 + 396,926 = 786,599 (opponent shadow bodies 988 + 2,964 + 5,434 + 5,434 = 14,820) |
| total | 2,266,764 | 1,592,738 (-30 %) |

During the first 66 s of the race (3-s windows, player at the back) the camera draws 0.35-1.4 M
triangles and the cascades 0.86-1.39 M, opponent cars 0.25-0.27 M and 0.47-0.78 M of those. The
grid is the worst case because all eleven opponents stand close together inside the first
cascades.

The original draws opponents into cascades 1 and 2 with the model's coarsest level only (LOD5
`main_body`, 494 triangles on the BMW; docs/CAR_LOD.md "Shadows"). Change: each opponent gets
that mesh as a shadow body on a layer only the sun takes casters from, posed by car_batch; the
opponent's parts and batches cast no shadow. Picture, fixed step, desktop 1600x900, against
`GRID_CAR_BATCH=0` (no bodies; it also unbatches, which alone flips 0.09-0.21 % of samples, F584):
ticks 300 / 2400 / 6000 differ in 0.20 / 0.24 / 0.17 % of pixels: the outline of the opponents'
ground shadows, no rear-wing shadow on their boot lid, and the player car's batching speckle. The
grid frame in the phone view against main at F595 with the timer: 0.25 %, the same places
(local/gridstart/crop-grid-s1*.png). Timing was not taken: the owner was playing on the PC.
Phone [I]: the shadow passes lose about 0.67 M of 2.27 M triangles at the start; with about half
the 35 ms GPU frame not scaling with pixels, a few ms less.

Mesh allocation at the start: `allocate_and_free_meshes` 3.46 ms per frame in the first window
(242 frames) is about 840 ms in total, the 0.8-1.1 s first two frames after loading (22,488 mesh
entities uploaded); it does not appear in later windows. Not changed here.

## Grid start: opponent car levels (F609)

The original draws opponents with the low file's levels 2..5, chosen per frame by FOV-scaled
distance under per-level caps (docs/CAR_LOD.md "Selection", "Port"). Change: car_batch selects a
level per opponent each frame and shows that level's batches and loose parts; the others sit on a
hidden layer. Hidden batches and loose parts are not posed (`car_batch::Unposed`); a level's loose
parts are posed one frame before the level is shown.

PC, desktop phone view (mobile preset, 1539x720), Donington, 12 cars, frozen at tick 1 [O]
(local/gridstart/grid-s2.log):

| view | F600 | F609 |
|---|---|---|
| camera | 806,139 (opponent cars 268,699) | 591,256 (opponent cars 53,816) |
| cascades 0-3 | 786,599 | 786,599 (shadow bodies as in F600) |
| total | 1,592,738 | 1,377,855 (-39 % against 2,266,764 before F600) |

Over a 3,600-frame race in the phone view the opponents spend about 80 % of car-frames at levels
2-5 (`level_car_frames=[17409,0,4421,5988,7469,29513]`); 108 level changes.

CPU, the same race, release build on the PC, three alternating rounds, `perf_timing` mean ms per
frame (wall time is capped at 60 Hz) [O]:

| round | `GRID_CAR_LOD=0` Main / Render | LOD on Main / Render |
|---|---|---|
| 1 | 5.94 / 12.00 | 5.99 / 10.91 |
| 2 | 4.79 / 9.73 | 5.29 / 9.52 |
| 3 | 5.51 / 9.84 | 4.73 / 8.53 |

The main schedule is within run-to-run noise; the render schedule is 0.2-1.3 ms lower with LOD on.
The level selection and visibility switch add about 0.15 ms per frame (`refresh_ms`) and the extra
posing about 0.07 ms per frame.

Picture, fixed step, desktop 1600x900, against `GRID_CAR_LOD=0`: ticks 300 / 2400 / 6000 differ in
0.83 / 0.12 / 0.03 % of pixels (coarser silhouettes of the cars further down the grid); the grid
frame in the phone view against F600 differs in 1.53 %. Liveries and numbers stay on the low
levels (local/gridstart/crop-grid-s2*.png, crop-2400-s2.png).

Phone [I]: the camera loses about 0.21 M triangles more at the start, so the grid frame loses
about 1.39 M of the original 2.27 M triangles in total. A profile APK shows it directly:
`gpu_pass` lines (main opaque pass GPU ms and primitives) and `view_triangles` per view in the
first windows, compared with the same lines in 3c7a3516's logs.

The start mesh-allocation hitch (F600) is unchanged.

## Start hitch: shared prop-piece meshes, OSD camera cascades (F616)

Phone (APK 3c7a3516, Nürburgring) [O]: the first two frames after loading take 787 and 1,109 ms
(`frame_stats frames=2`); `allocate_and_free_meshes` averages 3.46 ms per frame over the first
`profile_window` (242 frames), about 840 ms in all. The cars then wait on the grid for about 20 s,
so the hitch is the moment the grid first appears.

Cause, PC, phone view (Donington, 12 cars, `GRID_ASSET_COUNTS=1`) [O]: 19,888 mesh assets for 22,898
entities. 14,706 of the entities are the hidden pieces of the 1,021 breakable props
(`dynamic_props`), and 14,706 of the meshes were theirs: each piece draw got its own mesh holding
its source's whole vertex arrays. Bevy uploads every mesh asset, drawn or not, in the first frames.

Change: `setup` gives pieces with the same source mesh, draw, material and index list one mesh asset
(`dynamic_props::piece_draw`; the piece name now carries the source index). `draw_mesh` builds a
piece's mesh from its source and indices only, so equal keys give equal meshes. `piece_meshes
parts=14706 draws=63 meshes=63`: 63 meshes for all pieces; 5,245 mesh assets in all. The
`mesh_entities` line of `GRID_ASSET_COUNTS` now also gives distinct meshes per kind.

| first frames after loading, PC profile build | before | after |
|---|---|---|
| `allocate_and_free_meshes` | 570 + 570 + 82 = 1,222 ms | 321 ms |
| `extract_render_asset<RenderMesh>` | 253 + 5 = 258 ms | 69 ms |
| `prepare_assets<RenderMesh>` | 114 + 7 ms | below the list |
| mesh assets | 19,888 | 5,245 |

`setup` is unchanged (147 against 140 ms). Remaining mesh assets: 3,080 opponent meshes (deforming
parts own a mesh per car), 990 static batches, 383 car batches, 376 track parts, 258 others.

OSD camera: Bevy builds the sun's four cascades for every active camera, also for the OSD dial
camera (`osd_scene`, order -20). The render world never draws the sun for that view (its layer is not
the sun's), so those cascades only cost main-world frusta and caster visibility. `osd_scene` now
drops them after Bevy builds them. `check_dir_light_mesh_visibility` 0.38 ms per frame (mean of 42
two-second windows) → about 0.07-0.11 ms (below the profile list in all but one window).

Picture: fixed step, desktop 1600x900, against the build before: Donington ticks 300 and 6000 (one
prop broken) and the demolition derby at tick 2,700: 0 pixels differ.

Phone [I]: the first frames lose roughly three quarters of their mesh work, about 0.6 s of the 1.9 s
the two frames take, and the main thread about 1 ms per frame of caster visibility if the phone is
3-4x slower than the PC. Check in the owner's log: the first `frame_stats frames=2` line (avg_ms),
`allocate_and_free_meshes` and `check_dir_light_mesh_visibility` in the first and later
`profile_window`s, and `asset_counts ... meshes=`.

Cascades 1 and 2 (the coordinator's item): the original's caster callback skips cascade 0 for every
world caster, not only opponents (docs/CAR_LOD.md "Cascades (F616)"), so the port's rule is already
the equivalent one. Not changed.

## Track scenery levels of detail (F633)

The owner saw frame drops at Washington and asked whether its tall buildings are simplified with
distance. They were not: grid-app drew every placed ornament at level 0 at any distance. The
original selects each ornament's level per frame from the route's `ornament_attributes.xml`
distances and culls it beyond the last one (docs/TRACK_LOD.md). `ornament_lod.rs` ports that rule;
`GRID_ORNAMENT_LOD=0` gives the old picture.

- Level entities: every level of every placed ornament draw is an entity; a per-frame system shows
  one level per instance and hides the rest (no index rewrites, no uploads when a level changes).
  They are not merged by `static_batch`. Washington: 7,008 track entities instead of 1,381 plus
  740 fewer static batches (`mesh_entities` at frame 3,600); the drawn track entities stay near 100.
- Shadows: every ornament casts with its level 1, as in the original (pass 0x401).

Washington Viper race, 12 cars, PC, `--benchmark-frames 3600` after 240 warm-up frames, two runs
each, under the build lock (no build alongside), means over the 5 s GPU windows [O]:

| | phone view 1539x720, mobile preset | desktop 1920x1080 |
|---|---|---|
| camera triangles, off / on | 558k, 546k / 330k, 332k | 526k, 511k / 319k, 316k |
| triangles in the 4 shadow cascades | 494k, 479k / 361k, 363k | 450k, 448k / 340k, 339k |
| `main_opaque_pass_3d` GPU ms | 0.42, 0.53 / 0.37, 0.43 | 0.59, 0.62 / 0.65, 0.57 |
| frame interval mean / p95 / p99 ms | 17.8, 18.2 / 17.8, 17.6; p95 23.6, 27.1 / 23.7, 22.4; p99 40.3, 39.7 / 39.9, 38.4 | 17.8, 18.4 / 17.8, 18.4 |
| MainSchedule / RenderSchedule ms | 8.4, 9.4 / 8.8, 8.0; 14.8, 14.3 / 14.2, 13.5 | 8.0, 9.4 / 8.4, 9.1; 13.0, 14.4 / 13.8, 14.5 |

On the PC the frame is CPU-bound and the GPU is far from busy, so the frame rate does not move; the
CPU cost of the extra hidden entities is within the run-to-run noise. Shadows were measured
separately on the Donington phone view (a GPU span around both shadow passes, PC): 2.0 ms GPU
against 1.3 ms for the main opaque pass, the largest pass. That span hung the app at exit with GPU
timing on and was not kept.

Picture: fixed step, 1600x900, off against on: Washington tick 300 7.1 % and tick 3,000 6.5 % of
pixels differ, Donington tick 300 5.6 % and 6,000 1.1 %. Almost all of it is shadow detail (level 1
casters: e.g. less self-shadow under overhangs); near geometry is the same. Distant ornaments draw
their coarser levels and vanish past `lod_distance_02`. The original cross-fades scenery over 20 m;
the port switches in the middle of that overlap. The owner approved the change for desktop and
phone (28 October: "everywhere, as in the original"), an exception to the identical-desktop rule.

Washington spikes: the frames over 45 ms fall at the same benchmark frames with the levels off and
on (about frames 480 and 550, then single frames), so scenery does not cause them. The 10 s window
that holds the first ones has 36.9 mesh modifications per second (`asset_churn`: `car_batch` and
opponent LOD meshes, the pack's contacts after the start) against 1.5-6.5 in later windows [I: the
car-batch rebuilds and deformation uploads after the first contacts are the cause; not traced
further]. Backlog.

Phone [I]: about 40 % fewer camera triangles and 25 % fewer shadow-cascade triangles at Washington.
The phone's frame was GPU-bound at the grid (F555), so some of that should show as GPU time. Check
in the owner's log: `view_triangles view=camera0` total and the cascade lines, `gpu_pass ...
main_opaque_pass_3d`, and `perf_timing` MainSchedule / RenderSchedule (the hidden level entities
are main-world work).

Not done (backlog after the slow-down of 27 October): per-route-block `world_cull_dist`, the
instance caps, the scenery cross-fade; shadow caster culling and static shadow caching; the car
mesh rebuilds behind the Washington spikes; ground cover partial writes (F592); per-island
`WorldAdvance` (F568); the car-group contact solve and asleep pieces (F579); the vehicle stage
(round 2).

## Opponent dents: parallel projection, batched parts in the main world (F654)

Follow-up of F633's Washington frame drops. What a dent costs [O, profile build, Washington, 1 s
windows]: in the slow windows `drive_present` took 25-30 ms and `allocate_and_free_meshes` 14-42 ms
per frame, next to physics. `drive_present` re-projects every deforming part of an opponent whose
dent field changed (`body_damage::project`: two lattice samples per vertex and smooth normals), on
the main thread, and replaces each mesh; each replaced mesh is cloned for extraction and uploaded,
and `car_batch` rebuilds and uploads every batch holding one, although a batched part is never
drawn on its own.

A bit-exact skip of parts no moved lattice node reaches does not help: from one update to the next
329 of a Viper's 364 nodes move (the whole field shifts by small amounts), so nearly every part
really changes. Measured with a temporary diagnostic and dropped.

Change (`main.rs` `drive_present`, `car_batch.rs`, `static_batch.rs`; `GRID_DENT_FAST=0` restores
the old path for comparisons; since F852 the player's car also projects on the pool):

- The dirty parts are projected on the compute pool (`project_dent`), the same bits as one by one,
  and written on the main thread.
- A projection equal to the part's current mesh (positions, normals, indices) is not written: no
  upload, no batch rebuild (about 3.5 % of the writes at Washington: parts outside every dent).
- A batched part's projected mesh is written for the main world only
  (`RenderAssetUsages::MAIN_WORLD`): `car_batch` builds the batch from the main-world copy, and
  Bevy neither clones nor uploads the part's own mesh. `car_batch::Demoted` records each such part,
  and `car_batch::restore`, right after `refresh`, gives the mesh its render copy back in the frame
  the part leaves the hidden layer (a loose panel, a level shown on its own), before extraction.
  `static_batch::build` now always gives a batch render usage, whatever its first part has.

Picture [O]: fixed step, 1600x900, `GRID_DENT_FAST=0` against 1: Washington ticks 3,000 and 6,000
and the demolition derby at tick 2,700: 0 pixels differ in all three.

Washington Viper race, PC, `--benchmark-frames 3600` after 240 warm-up frames, two rounds under the
build lock (physics 1.03-1.07 ms per tick in every run, so no contention) [O]:

| | old path | new path |
|---|---|---|
| dent work on the main thread per update | 6.1, 6.3 ms | 1.8, 1.9, 2.1, 2.1 ms |
| meshes written per 128 updates | 49,490 | 46,795-47,775 |
| phone view p99 frame interval | 24.6, 24.5 ms | 20.1, 21.6 ms |
| desktop 1920x1080 p99 | 24.8, 23.9 ms | 22.8, 21.5 ms |
| MainSchedule mean | 3.76-3.91 ms | 3.64-3.77 ms |

The PC is held at 60 Hz, so mean frame times do not move (17.0 ms in all runs); the frames over
33 ms (3-8 per 3,600) and the maxima vary run to run in both modes. Fixed-step renders of 128
updates: main-thread dent time 2,009 ms (old) → 649 ms (parallel only) → 330 ms (parallel and
main-world parts) at tick 3,000.

Phone [I]: a dent update cost about 6 ms of main thread on the PC, roughly 20-25 ms on a phone 3-4x
slower: one dropped frame per update when the pack touches. It is now about 2 ms (6-8 ms on the
phone, spread over its cores), and the upload of each batched part's own mesh is gone. Check in the
owner's log: the `opponent_dents` line (`total_ms` over `updates`), `perf_timing` MainSchedule and,
in the profile APK, `drive_present` and `allocate_and_free_meshes`.

Left: the batch rebuild itself still re-uploads the whole merged mesh of a dented car and material;
writing only the changed member's vertex range needs partial mesh uploads, which Bevy does not
offer. The physics rise in the pack that the profile build suggested (`drive_tick` 5.5 → 9.5 ms
per tick) was span overhead and machine load, not contact work: F655 below.

## Washington pack tick profile (F655)

Coordinator's question after F654: where does the contact tick spend its time in the pack, and do
F653's basic-entity collision boxes add per-tick cost? Profile only; nothing changed.

Setup [O]: headless grid-app on main ca565a2a (`--settings local/p12/washington.ini --headless
--drive-ticks 9000`, `GRID_AUTOPILOT=1 GRID_THREADS=4 GRID_PERF_TIMING=1`), per 600-tick window
from the cumulative `perf_timing` lines; every run under the build lock. F653's boxes were switched
off through a temporary local environment toggle in `Props::read_route` (reverted, never
committed). Two rounds each; the rounds give identical contact counts, and the timings of the two
"on" rounds differ by up to 1.5x across the whole tick (machine state), so the comparisons are
within a run.

Main, quieter round, ms per world tick:

| window (ticks) | WorldTick | FramePlain | PieceContacts | LiftSunk | FrameGroup | BodyContacts | car contacts/tick | pieces (asleep) | piece contacts/tick |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| quiet (1800-5400) | 0.62-0.85 | 0 | 0.02-0.03 | 0.003 | 0-0.03 | 0.07-0.10 | 0-1.2 | 9 (9) | 0 |
| car contacts (600-1200, 1800-2400) | 0.69-0.84 | 0-0.01 | 0.02 | 0.003-0.009 | 0.08-0.09 | 0.07-0.09 | 2.6-5.3 | 5-9 (3-9) | 0-8 |
| props broken (5400-7200) | 1.29-1.44 | 0.29-0.37 | 0.17-0.20 | 0.07-0.08 | 0.01-0.04 | 0.08-0.09 | 0-1 | 65-82 (21-35) | 224-294 |

- [V] The doubling of the tick comes from broken prop pieces: islands without a car
  (`FramePlain`, piece piles) about 0.3-0.4 ms, piece contacts about 0.2 ms, `lift_sunk` about
  0.08 ms, while about 45 pieces are awake with 220-290 piece contacts per tick. Once all 82 sleep
  (from 7,200) the tick is back to 0.65-0.77 ms.
- [V] Car-to-car contact groups cost little: `FrameGroup` 0.08-0.09 ms per tick in the
  contact windows, `CarPairs` 0.06-0.08 ms throughout.
- [V] F653's basic boxes add no visible per-tick cost: `BodyContacts` 0.07-0.10 ms with them and
  0.07-0.13 without (a different race: the boxes change the course of events, so the off runs
  break props at other times). The body query uses the 16 m grid `BoxIndex`.
- [I] The "drive_tick 5.5 → 9.5 ms per tick" in F654 came from the span-instrumented profile build
  (`local/derby/spk-1.log`, run soon after the 14:38 reboot while builds ran): its cost per tick grew
  with the ticks per frame (3.5 ms at 1.9 ticks per frame, 9.7 ms at 13.6). That is span overhead
  and load, not contact work; the release build peaks at 1.6 ms per drive tick on this PC.
- Phone [I]: at 3-4x, about 2.5-3 ms per tick normally and 4-6 ms while pieces are awake.

Candidates (not done, each must keep every identity bit-exact):
1. Islands without a registered car on the compute pool. They do not use the shared deformation
   seed, but `grouped_physical_frame::calculate_impl` passes solver state from one island to the
   next (`manager.rows` contents copied into the row builder, `ends`, `cached_pair` and
   `cached_material`, `workspace` prefix, `properties` by local index). Each needs a proof that it
   cannot change a result (or an ordered replay of it), then a serial-against-parallel twin test.
2. Piece contacts per piece on the compute pool, merged in order like the body contacts.
3. Smaller: `lift_sunk` already has a parallel path; its 0.08 ms is the casts of awake pieces.

**Follow-up: the two candidates (F658).** The coordinator chose candidate 2 first, then candidate 1
only with a proof.

- Candidate 2 is already in place: since F568 `PropWorld::contacts` generates each piece's contacts
  on the compute pool and joins them in piece order (falling back to the serial pass near pool
  exhaustion). Same race at `GRID_THREADS` 1, 4 and 8, two rounds each, under the build lock
  (`local/pack/threads.sh`; identical race in all six, final `crt_seed` cae1df84), ms per tick in
  the broken-props windows [O]:

  | threads | PieceContacts | FramePlain | LiftSunk |
  |---|---:|---:|---:|
  | 1 | 0.73-1.03 | 0.35-0.55 | 0.09-0.12 |
  | 4 | 0.22-0.32 | 0.35-0.55 | 0.08-0.11 |
  | 8 | 0.22-0.31 | 0.37-0.52 | 0.08-0.10 |

  3.3-3.8x at 4 threads and no further gain at 8: little left to win there. (The machine ran about
  1.5x slower than in the F655 rounds; compare within the table.)
- Candidate 1 stopped [V]. In the expensive windows there is about one island without a car per
  tick (1.0-1.2 per tick, 0.31-0.36 ms each: the one awake pile), so islands in parallel have
  nothing to overlap; splitting one island's solve would reorder an order-dependent iterative
  solver. The carried solver state is also not a pure cache: `contact_rows::mixed` skips material
  fields 6-10 for frictionless pairs and keeps the previous pair's values there, and the row
  builder keeps words of the previous island's rows (the tests
  `restitution_group_rows_negative_zero_and_opaque_response_survive` and
  `body_change_flushes_previous_batch_only` show words 16-31 and 45 surviving). A parallel
  scheme would need a read-set check per island against the serial carry, for no gain here.
- What remains of the pile's cost is the single island's frame (`FramePlain` 0.35-0.55 ms in this
  table) and its contacts. A result-preserving cut would have to come from copies inside that
  frame (as F579 found for car groups), not from threads; not profiled yet.

## Demolition derby frame rate and parallel debris (F568)

Owner (24 October 2026, playable 7360a4865f54 with F563-F566, Detroit route_8 derby, 12 stk, 2560 x 1369,
RTX 4070, i7-12700): "FPS стало заметно меньше. Мы распределяем операции между ядрами процессора?"

**The owner's two derby logs [O]:**
- `local/r33/manual-24447-19896.log` (main checkout; F561-era build, logged `revision=unversioned`):
  about 145-155 FPS in the race. `present_mode=Fifo`, and it did not load `local\settings.ini`.
- `local/playtest/manual-30625-23044.log` (7360a4865f54-dirty): mean 112 FPS, 95-130 with dips to
  70. It loaded `local\settings.ini`: `present_mode = auto_no_vsync` and the FPS counter on.
- The lower rate starts at the green light, before the first prop break (tick 2295). Both runs
  logged `dynamic_props breakables=91 drawn_pieces=322`.

**Matched measurement, this PC, offscreen** (`--resolution 2560x1369 --render ... --drive-ticks 3600
--throttle 0.6 --steering 0 --benchmark-frames 2400`, `GRID_PERF_TIMING=1`). F561 = 99fef6aca274;
current = c52097d3 plus the timers below. Three alternating runs each, taken while no other cargo
build held the build lock:

| ms | F561 | current |
|---|---|---|
| `DriveTick` per tick | 1.46-1.57 | 1.84-1.94 (+0.38, +25 %) |
| `MainSchedule` per frame | 3.63-3.88 | 4.09-4.24 |
| `RenderSchedule` per frame | 6.12-6.41 | 6.17-6.32 |
| `DamageParts` (new timer) | - | 0.055 |
| `PropPresent` (new timer) | - | 0.003 |

The offscreen run is paced at about 60 Hz (mean frame 17 ms either way), so it gives stage times,
not a frame rate. The scripted run breaks no prop; the current build has 14 detached panels as bodies.

- Only the simulation got more expensive: the F565 panels as bodies, F564's `lift_sunk` and the
  piece contacts they bring. Presentation (`damage_parts`, piece transforms) and the render schedule
  did not change.
- [I] At the owner's 0.55 ticks per frame, +0.38 ms per tick is about +0.2 ms per frame. That is too
  small for 145 -> 112 FPS (6.9 -> 8.9 ms). The render schedule (6.2 ms) is longer than the main
  schedule (4 ms) here.
- [I] Two other differences between the runs are larger candidates.
  - The present mode: Fifo -> AutoNoVsync, from the settings file that only the second run loaded.
  - Load from other processes. While I measured, other sessions' cargo builds made the same run
    3x slower (`DriveTick` 5.5 instead of 1.85 ms), and builds were running when the owner played.
- The FPS counter costs nothing measurable (offscreen A/B).
- Not measured: the owner's windowed run with each present mode on an idle PC. That needs a window,
  which this session must not open.

**Headless derby with breakable props** (`GRID_DYNAMIC_PROPS=1 grid-opponent-race ... --route
tracks/usa/detroit/route_8 --demolition --cars stk 12 3000 ai`, serial). The F561 race breaks
nothing in 3000 ticks. The current race breaks 8 stacks at tick 1697; F565 makes the race diverge.
- F561: world tick 3.2-3.8 ms, 12 bodies.
- Current: 4.8-4.9 ms, 48.7 bodies and 71.6 piece contacts on average.
  - `PieceContacts` 0.37 ms.
  - `LiftSunk` 0.16 ms (one cast per piece and wheel every tick).
  - `PanelSettle` 0.008 ms.
  - `LooseParts` 0.01 ms.

**New timers** (grid-tools `perf::Stage`, `GRID_PERF_TIMING` only):
- `PropCommit`: the prop world's clone, detection, breaks and posts.
- `LiftSunk`.
- `PanelSettle`.
- `LooseParts`: `detach_wheels`, `Parts::step` and the new loose bodies.
- grid-app: `DamageParts` and `PropPresent`.

**Parallel debris (F568):**
- `PropWorld::contacts`:
  - Each piece's contacts (track, later pieces, car hulls) are generated on the compute pool
    (`grid_tools::parallel::map`) and joined in piece order.
  - Hulls and car placements are built once per call behind `OnceLock`s.
  - The parallel lists equal the serial ones when the pool keeps 16 spare slots after them
    (`POOL_SLACK`, the largest capacity of one call), so no room-limited capacity applied.
    Otherwise, or on any error, the serial pass runs.
  - It applies from 8 live pieces (`PARALLEL_PIECES`).
- `PropWorld::lift_sunk`: the road depths are computed in one run of pieces per thread. Each cast is
  vertical, so the rectangle-cull cache's plane order cannot change a result.
- `MultiRuntime::parallel` switches both (the serial twin turns it off). The solo `Runtime` passes
  `true`.
- Not done: per-island solving. `WorldAdvance` is 0.5 ms per tick with 110 pieces, for all bodies
  together.

**Identity [V]:**
- `derby-df` (no dynamic props) still gives `166c2d18…` (tools/identity.tsv).
- `GRID_SERIAL_TWIN=1` (new, grid-opponent-race) runs every tick also on a serial clone. It compares
  every body word, each car's vehicle, physical and frame records, and the break count.
  - Derby with props, 12000 ticks requested (the race ends at 8174): `serial_twin ticks=8174
    max_bodies=183 threads=20`, 16 breaks, 137 pieces, no difference.
- The current and parallel builds print identical outputs: derby `e9011d21…`, drift `bb1661c7…`.

**Effect** (headless with dynamic props, two alternating runs each):

| | threads | current | F568 |
|---|---|---|---|
| Detroit derby, 8174 ticks, 110 pieces | 20 | world tick 2.00 ms, `PieceContacts` 0.36, `LiftSunk` 0.225 | 1.61-1.74 ms, 0.14, 0.12 |
| same | 4 (phone) | 1.98-2.00 ms | 1.56-1.63 ms |
| Nürburgring drift battle, 8 bmw, 9000 ticks, 48 pieces | 4 (phone) | 1.15-1.22 ms, `PieceContacts` 0.25 | 0.95-1.02 ms, 0.08-0.09 |

- [I] The phone runs the same work about 3x slower than this PC. That makes about 1.2 ms less per
  tick in a derby full of debris, and 0.6 ms less in the drift battle.
- In the scripted grid-app run (14 pieces, no break) the change is within noise: `DriveTick`
  1.79-1.81 against 1.81-1.82 ms.

## Physics cost growth over a race (F579)

Coordinator's task (26 October 2026): the phone's physics cost grows over a race from about 4 to
9 ms per frame; find why and cut it with bit-identical results.

**The phone curve [O]** (owner's profile APK log `log-20261026-125712.txt`, Xiaomi 13 Ultra, 4
compute threads, Donington route_0, 12 BMWs; `drive_tick` exclusive ms per frame divided by ticks
per frame): about 2.8 ms per tick for the first 80 s, then 3.5, 4.5, 5.1, 5.3 ms per tick from 90 to
140 s, and 3.1 ms after the finish. The replay of that log on this PC (`grid-soak replay-log`)
diverges at tick 958 (ARM against x86), so it does not reproduce the phone's pile.

**Reproduction off the phone.** `grid-app --headless` with `GRID_AUTOPILOT=1 GRID_THREADS=4
GRID_PERF_TIMING=1` on the same event (`--settings donington-12.ini`: route_0, bmw against 11 bmw,
15,000 ticks) and on the Nürburgring drift battle (`settings/drift-battle-nurburgring.ini`, 12,000
ticks). All timed runs below ran under the build lock (no cargo build overlapped them), between
14:25 and 17:02 on 26 October, after the owner's game had closed at 14:07. Earlier runs that day
were discarded.

New diagnostic timers (`GRID_PERF_TIMING` only; they never feed the simulation):
- grid-tools `perf::Stage`: `WorldClone`, `Continuous`, `PropClone`, `PropDetect`, `PropApply`,
  `BodyEvents`.
- grid-world `stage_timing` (reported on the same line): `Predict`, `Islands`, `IslandPack`,
  `IslandFrame`, `IslandStore`, and the frame by island kind: `FramePlain` (no car), `FrameCar` (a
  car with static contacts only), `FrameGroup` (a car with other bodies), `GroupPrepare` and
  `GroupSolved` (its two deformation phases), `GroupContacts` (contacts per tick in car groups).
- grid-soak prints the cumulative line every 600 ticks.

**Cause [V].** The growth follows the piece count. Pieces that cars keep touching never sleep:
sleeping is per island, cars never sleep, so a car touching a pile keeps the pile in its island
(BODY_SLEEPING.md, as the original). Such an island is a car group (`FrameGroup`), and the car
group's per-contact work copied whole records:
- `response_consumption::validate` compared every contact identity and owner with every other
  (O(n²)), twice per car and tick, over all contacts of the group (about 1,200 per tick late in
  the drift battle);
- `deformation_contact_limit::calculate_side` copied the car's 42 KB deformation descriptor and
  re-checked its 400 grid nodes for every contact;
- `body_impact::calculate` copied the car's whole state (the 135 KB vehicle record) two or three
  times for every contact of the car, in the body events after the solve;
- `impact_frame::run` cloned the callback context (every owner's vehicle record) once per frame.

Drift battle, ticks 9000-12000 (268 pieces, 54 asleep, 1,271 piece contacts per tick), timer build
before → fixed build, ms per tick:

| Stage | before | after |
|---|---:|---:|
| `WorldTick` | 5.76 | 3.93 |
| `FrameGroup` | 3.20 | 1.93-1.94 |
| `GroupSolved` | 1.40 | 0.33 |
| `GroupPrepare` | 0.14 | 0.03 |
| `BodyEvents` | 0.57 | 0.10 |
| `FramePlain` | 0.15 | 0.14 |
| `PieceContacts` | 0.66 | 0.61-0.64 |
| `LiftSunk` | 0.23 | 0.23 |

What is left in `FrameGroup` is the solve of those contacts. On Donington, where the pile sleeps
(56 pieces, 56 asleep late), the growth is spread over many small per-body costs; the largest one
removable without changing a result was `lift_sunk`, which cast a 100 m ray under every piece every
tick, asleep or not.

**Changes [V], all result-preserving:**
- `response_consumption::validate`: identities sorted once, duplicates and pending lookups by
  neighbour and binary search, owners sorted by (identity, registration). The test
  `sorted_validation_matches_the_pairwise_checks` compares it with the pairwise version on 20,000
  random inputs (accepted and rejected, same messages).
- `deformation_contact_limit::calculate_side_in`: the descriptor is updated in place (the window
  slot and +A490..+A498, the only words written; each read sees what it saw in the copy), and the
  grid check (`validate_grid`, words the limit never writes) runs once per descriptor
  (`validate_cached`). Used by `deforming_group_frame` and `deformation_preparation`;
  `calculate_side` keeps its copying form.
- `collision_damage` split into `validate` and `apply`; `calculate_in_place` writes the owner's
  mutable block only after every region succeeded, as before. `body_impact::calculate_in_place`
  uses it and the validation-only `check` (the first damage call's result was discarded). The
  multi-car body events and the solo `vehicle_world` path use the in-place forms; an error
  discards the staged car either way.
- `impact_frame::calculate_with_callbacks_rows` takes the callbacks by value, and the group
  deformation phase takes each owner's record instead of copying its descriptor.
- `PropWorld::depths`: a memo of asleep pieces' road casts, by world body, used only when the
  (breakable, piece) and the 16 pose words match bit for bit, and only for the road chunks it was
  made with (`Track::road`). The cast reads only the static road with a fresh cull cache, so a hit
  equals the cast. `pieces_at_rest_sleep_and_a_car_wakes_them` compares memo and fresh casts.

**Identity [V]:**
- Headless drive samples (every `drive_*` line without its wall-clock field): drift battle
  `62b70b73…` and Donington `9d232e98…`, the same for the base build (origin/main 96a9008d) and
  every step, two rounds each.
- grid-opponent-race with dynamic props, 4 threads: Detroit derby (`--route tracks/usa/detroit/
  route_8 --demolition --cars stk 12 12000 ai`) `e9011d21…` and Nürburgring (`--route tracks/
  europe/nurburgring/route_1 --cars bmw 8 9000 ai`) `0eaebdd8…`, base = fixed; with
  `GRID_SERIAL_TWIN=1` 8174 and 9000 ticks without a difference.
- The identity runs (`tools/identity.tsv`) in the full prepush tier.

**Before and after at 4 threads** (base build without timers against the fixed build, alternating,
two rounds; ms per world tick, early = ticks 600-3000):

| Event | Window | base | fixed |
|---|---|---:|---:|
| Drift battle | early (0 pieces) | 0.55 | 0.50-0.59 |
| Drift battle | late, 9000-12000 (268 pieces) | 5.46-5.55 | 3.92-4.10 |
| Donington, 12 bmw | early (0 pieces) | 0.73-0.76 | 0.71-0.74 |
| Donington, 12 bmw | late, 12000-15000 (56 pieces) | 1.14-1.18 | 1.02-1.06 |

- Donington late: `LiftSunk` 0.080-0.084 → 0.012-0.013 ms; `Commit` (which holds it and the body
  events) 0.29-0.30 → 0.20-0.21 ms.
- [I] The phone runs this work about 3-4x slower (2.8 against 0.75 ms early in the same race).
  That suggests about 0.4 ms per tick less late in a Donington race and several ms in an awake
  pile like the drift battle's. Not measured on the phone.
- Not done: the solve of a car group's contacts itself, and the per-body work of asleep pieces
  (piece pairs, the world's rollback copy, the vehicle stage's growth from damage). They are the
  rest of the growth.

**Profiling** (`tools/sample_profile.py --busy`, new: it samples only threads that ran since their
last sample, so Bevy's idle pool threads no longer dilute a headless race). The release build's
names are the nearest public symbols, so call paths are approximate; the leaf
`response_consumption::validate` (8 % of busy samples in the drift race) led to the first fix.
Evidence (machine-local): worktree `local/tick-growth/` (`runs.txt` lists every run with its start
time and the lock state).

### G300 integration ownership guard (27 October 2026)

F584's reported measurements remain historical evidence for its exact build. During graphics integration on pinned F586, player source entities carrying car0/ and no OpponentPart are conservatively excluded from new car batching: existing main-view bumper suppression and probe ownership use those source roots. Opponent batching remains enabled, including the same model prefix on explicitly marked opponents. Layered/probe/glass exclusions already protect several paths, so this is a precise ownership guard rather than a claim that every batch broke rendering. The actual ECS fixture passes with retained player originals and constructed opponent geometry; the full1137-test gate, four incoming identities,24quick-soak cases and fresh native BMW/Istanbul smoke controls also pass (G300). Actual bumper/Manual/general rendering remains outside that bounded GPU proof. No new FPS gain is claimed for the adjusted graphics build.

## Early-race tick cost: whole-record copies (round 2)

Coordinator's task (26 October 2026): after F579 the phone still pays about 2.8 ms per tick early
in a race at 4 threads (Donington, 12 cars; owner's log `log-20261026-125712.txt`). Profile the
early-race tick and cut its largest costs with bit-identical results.

**Profile [O]** (`tools/sample_profile.py --busy` on a line-tables build of `grid-opponent-race`
with deformation, AI and props, 12 cars, 4 threads; new option `--path-frames N` prints longer call
paths). The leaf `_NLG_Return2` (vcruntime's memcpy) was 31.6 % of all samples. About 26 points of
that fell inside `MultiRuntime::tick` (69 % of samples; the rest is loading), so about 40 % of the
tick's CPU was copying. Each car's 132 KB vehicle record was copied about eight times per tick:
- in the serial body events, the mirror copy into the deformation registration (9.3 % inclusive
  at that one line);
- in the serial world advance, the deforming frame's `owner.to_vec()` (5.9 %) and a
  validation-only `deformation_preparation::calculate` call that copied the 42 KB descriptor and
  the contacts and threw them away (about 3.4 %). The descriptor's 400-node grid check also ran
  twice per frame (3.1 %);
- in the parallel vehicle stage, the staged registration's owner clone, the sync slice (53 KB:
  `to_vec`, a clone inside `vehicle_sync::calculate`, and the copy back), the deformation
  publication's whole-state clone (`update_without_rebuild`), and the handling update's clone of
  the publication's fresh output.

**Changes [V], all result-preserving:**
- The vehicle stage moves the car's record into its staged registration (`mem::take`); nothing
  reads the staged state's record before the body events take it back.
- `deformation_world::Registration::owner` is documented as a slot that is valid only during a
  world tick. Every caller fills it from the car first: the solo `vehicle_world` path copies it,
  and the multi-car runtime moves it in. No reader looks at its content between ticks (checked:
  grid-app, grid-tools, the probes and the tests read only `bulk_calls`, `records`, `random_seed`,
  `owner_toggle`, or the owner's length for the flashback memory estimate). So the multi-car body
  events no longer copy the car's record back into it; the committed registration keeps an empty
  owner.
- The body events run on the compute pool, one run of cars per thread, joined in car order (each
  car touches only its own staged state, registration and damage RNG).
- `deforming_physical_frame` is split into `prepare` (every check, no owner) and `finish` (takes
  the owner record by value). `deformation_world::calculate_group` moves the registration's owner
  in; a failure inside the frame drops it, and both callers discard the registration with the
  failed tick (the solo path works on a clone, the multi-car path on per-tick staging).
- `deformation_preparation::validate` (the checks alone) replaces the validation-only call, and
  `calculate_in` runs the prepare phase on the context's own descriptor and contacts. The grid
  verdict from the validation is kept for the prepare phase, which sees the same descriptor word
  for word, and the limit never writes the grid.
- `vehicle_sync::calculate_in` updates the record and the physical body in place. Every word it
  reads after its first write is one it never writes. `vehicle_job::calculate_staged` uses it on
  its staged copy.
- `vehicle_job::Publication` takes the owner state by value.
  `deformation_publication::calculate_owned` and `after_geometry_rebuild_owned` update in place
  (`vehicle_publication::update_in`). The words the update writes (+C780..+C798, +15CC, +15DC,
  physical +210/+220) are not among those read after it, and each later write is read, if at all,
  before it. `BodyGeometry::deformation_publication_owned` serves the multi-car stage.
- `vehicle_handling::calculate_with_empty_notification_receiver_owned` takes the publication's
  output by value. The damage dispatches' inputs (wheel impacts, collision points, the physical
  record) are read before the state becomes the output, as the copying version read them from its
  input.
- Kept: `Runtime::staged_car`'s copy, which is the car's rollback state when the world tick fails.

**Identity [V]** (base = origin/main 12aa5b00 built in this worktree; fixed = the change):
- grid-opponent-race with props, 4 threads: derby `e9011d21…` and Nürburgring `0eaebdd8…`, base =
  fixed, and the same with `GRID_SERIAL_TWIN=1`.
- Headless drive samples without wall-clock fields (`GRID_AUTOPILOT=1 GRID_THREADS=4`, no timers):
  drift battle `d01bde2a…`, Donington 12 bmw `5be6f0c1…`, base = fixed.
- Solo grid-opponent-race (Donington route_0, `1 5600 ai`) with `GRID_ENGINE_AUDIO_TRACE`, with and
  without `GRID_DEFORMATION_PARAMETERS`: race output and trace base = fixed in both. The
  deformation parameters alone change this lap (86.792 s without, 86.785 s with, from tick 1518).
  That explains a reported lap difference against an F578-era trace, which ran without them.
- `tools/identity.tsv` in the full prepush tier.

**Before and after at 4 threads** (headless `GRID_PERF_TIMING=1`, base against fixed, alternating,
three rounds, all under the build lock, 22:42-22:49 on 26 October, after the owner had finished
playing; an earlier set at 22:04 was disturbed by outside load and dropped). ms per world tick;
early = ticks 600-3000, late = the last 3000:

| Event | Window | base | fixed |
|---|---|---:|---:|
| Donington, 12 bmw | early | 0.70-0.74 | 0.46-0.50 |
| Donington, 12 bmw | late (56 pieces, asleep) | 1.00-1.08 | 0.61-0.67 |
| Drift battle | early | 0.51-0.53 | 0.37-0.38 |
| Drift battle | late (268 pieces) | 3.86-3.94 | 3.63-3.71 |

Donington early by stage, base → fixed: `WorldAdvance` 0.18-0.19 → 0.08; `VehicleStage`
0.27-0.29 → 0.18-0.21; `BodyEvents` 0.08-0.09 → 0.05; `Commit` (which holds the body events)
0.13 → 0.08-0.09.

- [I] Early in a Donington race the PC's tick falls by about a third. At the phone's 2.8 ms that
  would be about 1.9 ms per tick (2.8 × 0.48 / 0.72), assuming the phone's cost scales like the
  PC's. Memory-bound copies may gain more on the phone; this is not measured there.
- Left: the vehicle stage itself (wheels, suspension, contact updates; 0.18-0.21 ms on 4 threads),
  `staged_car`'s rollback copy, and in an awake pile the solve of a car group's contacts.

G302 integrates pinned main F587-F590 after the G300 ownership guard. The true three-way car_batch merge preserves player SourcePart car0/no-Opponent exclusion, opponent batching, incoming NoAutoAabb/NotShadowCaster handling and (!shadowless || ours) admission. Exact player ownership ECS test, full1137tests/Clippy/four unchanged identities/24soak, combined feature build and fresh native BMW smoke pair pass. This establishes bounded integration safety, not a new measured FPS gain or actual bumper view. See G302 for source/artifact/capture hashes; incoming F587/F588 measurements keep their original scope.

G304 integrates pinned F591/F592 into the graphics branch, retaining player ownership and native ground-skid identity. Full1139tests/Clippy/four unchanged identities/24soak and a fresh combined-feature native Istanbul pair pass. Root sees actual laid curves in Off and their attenuation in Late; no additional FPS gain or exhaustive strip-delivery claim. Last-queued snapshots still precede upload acknowledgment, so the conditional missing-allocator cold-drop limit remains explicitly open. Upstream measurements retain their scope. Source/artifact/visual and cleanup evidence is in G304.

## Bounded CPU submission observer (G343)

`GRID_OBJECT_SUBMISSIONS=120` enables sampled diagnostic tables for at most 120
main/render invocations (accepted budget 1..360). The default path installs no
observer. This switch alone does not enable GPU timestamp queries or profile
mode. Samples are first, every thirtieth and last invocation; startup may not yet
have prepared geometry, so empty early samples are not a claim of zero draws.

The observer preserves existing opaque, alpha-mask and shadow draw-function IDs
and command tuples, replacing only the terminal mesh command with a delegate
that calls the original exactly once. It reports successful direct/indirect CPU
submission calls, indirect capacity and terminal skips/failures. A material label
identifies the representative phase entity, not every member of an indirect draw.
Render phase numbers are 0 opaque, 1 alpha-mask and 2 shadow.

Queue keys, CPU visibility memberships and issued API calls are different metrics.
Main-world visibility IDs and render-world IDs are intentionally not joined by
matching invocation numbers. GPU executed command counts, indirect live members,
per-material GPU work and ornament ownership remain unknown. These tables alone
do not establish a bottleneck, culling defect, FPS gain or measured GPU cost.

Tables have explicit storage/view/membership caps, but every sampled terminal
call still incurs a lookup and lock; these caps do not bound all CPU work. After
the budget ends, installed wrappers retain an atomic flag check per terminal draw.
Use a fully disabled control for performance measurements. A same-artifact
Off/On runtime comparison and actual output checks are required before accepting
this diagnostic; compilation alone does not prove draw-order or pixel neutrality.

The actual 600-tick twelve-STK Detroit baseline Off/On pair passes with exact
final RGBA and all four ordered player-record families. Five bounded sample
footers complete; the four prepared samples report Raw 2 indirect CPU calls with
capacity2, versus Standard 2/capacity117 in the main opaque view. Four shadow
views each report Raw2/capacity2 and Standard6, with capacities80/94/95/103.
Those samples occur during the held starting grid, not a controlled moving-view
comparison. There are no reported terminal failures or dropped updates. This
supports the observer's operation and sampled neutrality, not all-context or
GPU-execution equivalence, a measured speedup or absence of other bottlenecks.
Receipts and the independent counter audit are private under
`local/graphics/object-submissions01/runtime01/`.

## Sun shadow cost (F652)

Task: the shadow passes looked like the largest GPU pass (F633: a span "around both shadow passes",
2.0 ms against 1.3 ms for `main_opaque_pass_3d`, Donington phone view on the PC), and the phone was
GPU-bound at the grid (F555). The original's cascades: docs/SUN_SHADOWS.md (three maps of the
option's size, 1024 on medium and 2048 on high; world casters only in cascades 1 and 2).

**Measuring.** F633's span opened a timestamp span in one system and closed it in another. Bevy's
recorder closes the span opened on the calling thread, so the close found none and panicked the
renderer (`bevy_render diagnostic/internal.rs:404`); that is the hang at exit. On the render thread
(both systems `NonSendMarker`) the device was lost instead, most likely because the closing timestamp
was submitted after the frame's query resolve, which waits for it [I]. `gpu_passes` now replaces
Bevy's early shadow pass system with the same pass plus one pass span per cascade
(`gpu_pass ... name=shadow_cam0_c0..3`), as Bevy times its main passes; only with GPU timing on
(`GRID_GPU_PASSES=1`, profile build). Runs exit normally.

**Result: the shadow passes are not the PC's largest pass.** All four cascades take 0.13-0.34 ms
GPU at the RTX 4070's normal clocks, against 0.39-0.70 ms for the main opaque pass. With the GPU in
a lower clock state (after the PC's reboot of 30 October, every run held the 60 Hz cap) all passes
take about three times as long: shadows 0.3-1.2 ms, opaque 0.4-2.0 ms. F633's 2.0 ms span covered
more than the shadow passes, in a profile build.

**Change: casters culled per cascade by receiver** (`crates/grid-app/src/shadow_cull.rs`,
`GRID_SHADOW_CULL=0` for the old lists). Bevy keeps a caster in a cascade when its box meets the
cascade's light-space square, which is fitted to the slice's diagonal and so also takes casters beside
and behind the camera. Cascade `i` is sampled only for fragments between `0.8 * far(i-1)` and
`far(i)`, so a caster whose box lies wholly outside one face of that camera slice (a face whose normal
does not point along the light; margin 10 cascade texels) cannot shade a sampled fragment and is
dropped from that cascade's list after Bevy's caster visibility. The rule is exact apart from the
margin, so the picture is bit-identical: fixed-step renders 1600x900, Donington ticks 300, 2,400 and
6,000, Washington ticks 300 and 3,000, and Donington 2,400 / Washington 3,000 with the mobile preset,
`base` (origin/main 4b450407) against culling on: 0 pixels differ in all seven, the PNG files are
byte-identical.

Triangles in the four cascades, PC, `--benchmark-frames 3600` after 240 warm-up frames, means over the
5 s windows, three alternating rounds (release build a8f338051339 with the timed shadow pass) [O]:

| view | off | on | cascades 0 / 1 / 2 / 3, on against off |
|---|---|---|---|
| Donington phone view 1539x720 (mobile preset) | 430k, 411k, 411k | 298k, 291k, 290k (-30 %) | -3 / -28 / -32 / -36 % |
| Donington desktop 1920x1080 | 370k, 372k, 376k | 276k, 270k, 274k (-27 %) | -3 / -28 / -31 / -32 % |
| Washington phone view | 346k, 360k, 361k | 248k, 256k, 256k (-29 %) | -3 / -29 / -33 / -37 % |
| Washington desktop | 326k, 340k, 339k | 240k, 245k, 244k (-28 %) | -2 / -29 / -32 / -36 % |

GPU ms per frame, shadow cascades / main opaque, in the same three rounds (round 1 normal clocks,
rounds 2 and 3 the lower clock state; compare within a round) [O]:

| view | round 1 off / on | round 2 off / on | round 3 off / on |
|---|---|---|---|
| Donington phone view | 0.19 / 0.42, 0.13 / 0.39 | 0.31 / 0.44, 0.13 / 0.31 | 0.34 / 0.43, 0.32 / 0.58 |
| Donington desktop | 0.18 / 0.59, 0.15 / 0.61 | 0.16 / 0.47, 0.24 / 1.15 | 0.31 / 1.06, 0.35 / 1.25 |
| Washington phone view | 0.34 / 0.49, 0.30 / 0.43 | 0.80 / 0.99, 0.98 / 1.49 | 1.21 / 1.49, 0.99 / 1.51 |
| Washington desktop | 0.29 / 0.70, 0.31 / 0.64 | 1.12 / 1.92, 0.96 / 1.97 | 1.12 / 1.96, 0.97 / 1.99 |

Where the opaque pass shows the same clock state (Washington rounds 2 and 3, Donington phone view
round 1) the shadow passes lose 13-32 %; the other pairs ran at different clocks. On this GPU the
cascades seem to cost more in fill and clear (four 2048x2048 depth maps) than per triangle [I].

Frame interval mean / p50, ms: round 1 (the owner at the PC) off/on Donington phone view 21.9/21.3,
20.2/19.6; Donington desktop 17.5/17.2, 18.7/18.3; Washington phone view 21.6/20.6, 21.3/20.1;
Washington desktop 22.2/21.0, 21.5/20.2. Rounds 2 and 3: 17.0-17.9 mean, 17.0 p50 in every run (the
60 Hz cap), so the wall frame shows no difference.

CPU, profile build (feature `profile`), ms per frame off / on. Round 2, the PC quiet (all runs at
the 60 Hz cap): render app Washington phone view 8.29 / 8.09, desktop 8.03 / 7.96; Donington phone
view 7.62 / 7.64, desktop 7.37 / 7.25. Within it at Washington the shadow instance buffers
(`write_binned_instance_buffers<Shadow>`) 0.40 / 0.32 and 0.37 / 0.31, `unpack_bins` 0.29 / 0.26 and
0.28 / 0.26, `prepare_preprocess_bind_groups` 0.49 / 0.45 and 0.47 / 0.44, the shadow pass encoding
0.45 / 0.44 and 0.47 / 0.45: about 0.15-0.2 ms less. Round 1 ran while the PC was busy (the owner
and other sessions): every figure about 2.7 times higher (render app 22.6 / 21.4 ms at Washington),
the same items about 0.7 ms less. The cull system itself is below the table (top 60 spans, under
about 0.1 ms).

Phone [I]: about 30 % fewer shadow triangles in cascades 1-3 and fewer shadow draws. On a tiled GPU
the depth-only cascades cost bandwidth per map more than per triangle, so the gain is likely smaller
than 30 % of the shadow time; the render thread loses the same shadow batching work as on the PC.
Check in the owner's profile log: `gpu_pass ... name=shadow_cam0_c*` (now timed),
`view_triangles view=camera0_cascade*` and `perf_timing`.

Not done: static shadow caching. Bevy re-fits each cascade every frame (texel-snapped, but the centre
and the depth range move with the camera), so cached static casters would have to be scrolled and
depth-remapped per frame, and a racing camera moves the near cascades completely within a fraction of
a second. With the passes at 0.1-0.3 ms on the PC and fill-bound, not pursued. The faithful layout
(two world cascades and the car's own map, docs/SUN_SHADOWS.md section 3) would roughly halve the
passes on every device but changes the picture; it needs the owner's decision.

## Light clustering and the OSD view (F661)

Task: profile RenderSchedule and MainSchedule and cut the top render-thread costs without changing the
picture. F652's profile (Washington phone view, quiet PC) showed the render app at 8.0 ms, about 1 ms
of it Bevy's light clustering for the main and the OSD dial camera, and 1.5-1.7 ms more for the OSD
camera as a whole (clustering 0.45 ms, two extra command buffers 0.27 ms, its transparent pass 0.28 ms).

**What clusters.** Bevy clusters point and spot lights, light probes and clustered decals per 3D camera
every frame. grid-app has no point or spot light, light probes only with `--reflections probe`
(`scene_probe`), and clustered decals not at all; the sun is not clustered. On the desktop Bevy
clusters on the GPU, and that path rebuilds its buffers, bind groups and two compute passes for every
camera even with nothing to cluster. Android always clusters on the CPU.

**Change** (`crates/grid-app/src/light_clusters.rs`, `GRID_LIGHT_CLUSTERS=bevy` for Bevy's): without
light probes the app clusters on the CPU (`GlobalClusterSettings::gpu_clustering = None`, as on
Android), and while nothing clusterable exists every 3D camera's `ClusterConfig` is `None`, which skips
the CPU assignment and uploads an empty grid. A light, probe or decal appearing restores Bevy's default
config the same frame. With probes the GPU path stays.

**Picture.** Fixed-step renders 1600x900, same executable, `GRID_LIGHT_CLUSTERS=bevy` against the
port's: Donington ticks 300 and 2,400, the enhanced profile (Lacetti, Donington 2,400) and probe
reflections (enhanced, `--ibl source`) are bit-identical. Washington tick 3,000 differed in 115 pixels
of the minimap: the player's arrow and the opponents' dots blend at one saved depth, so where they
overlap their order was a sort tie, and the change flipped it (the dark outline of one dot over
another does not blend commutatively). `race_osd::minimap` now gives each car its own depth toward
the OSD camera (player in front, then the opponents in car order; 0.002 scene units per car at about
3,300 units' distance), so the order no longer depends on unrelated changes; with it Washington and
Donington are bit-identical between the two clusterings. That is a small HUD
change against the old frames where markers overlap (Donington 2,400: 199 pixels, at most 14 per
channel).

**Cost** (PC, `--benchmark-frames 3600`, GPU timing on; other sessions' builds ran throughout, so
whole-frame and render-app totals moved by about ±1 ms between runs and are not compared):

- GPU: the `clustering` pass, 0.17-0.20 ms per frame with Bevy's clustering (main camera, two
  rounds, all four views), is gone.
- Render thread, profile build: Bevy's GPU-clustering systems (`prepare_clusters_for_gpu_clustering`,
  `cluster_on_gpu` encoding, `prepare_clustering_bind_groups`) took 1.0-1.8 ms per frame in this busy
  round (Washington phone view 1.79, desktop 1.17; Donington phone view 1.01, desktop 1.81); with the
  change none run and `prepare_clusters_for_cpu_clustering` takes at most 0.23 ms. `queue_submit` sends
  19 command buffers instead of 21.
- The phone already clustered on the CPU; `ClusterConfig::None` there skips the assignment for both
  cameras (F652's phone log: CPU clustering about 0.6 ms) [I until the owner's log].

## Frame split (F836)

17 November 2026, Claude Code (session mac-perf). Goal: one owner log from the Mac must say
where a frame goes, so the optimisations for the Mac can be chosen by measured cost.
`crates/grid-app/src/frame_split.rs` appends the split to the periodic `frame_stats` line
(the fps counter's one-second window, written while the counter is shown: Video → FPS
counter, or F9).

### Fields

Main world (the app thread, means per frame over the window):

- `main_ms`: `First` to `Last` of the main schedule, the whole update.
- `fixed_ms`: the fixed-step loop inside it (simulation ticks).
- `fixed_max_ms`: the slowest single fixed tick of the window (F851, "Worst fixed tick").
- `outside_ms`: the rest of the frame: waiting for the render thread, extract, window events.
  `main_ms + outside_ms` equals `avg_ms`.
- `extract_ms`: the extract step (runs on the main thread while the render world waits).

Render world (the render thread, means per render frame, they sum to `render_ms`):

| field | stage |
|---|---|
| `r_meshes` | extract commands, asset preparation (mesh and texture uploads to the GPU), view management, specialisation |
| `r_views` | view preparation (`PrepareViews`) without the acquire |
| `acquire` | swap-chain image acquire (`prepare_windows`); high values mean a GPU or vsync wait |
| `r_queue` | queue (phase items) |
| `r_sort` | phase sort |
| `r_prepare` | prepare resources: batching, instance and uniform buffers (up to the resource flush) |
| `r_bind` | bind groups and the rest of `Prepare` |
| `r_encode` | render graph: command encoding of all views and passes |
| `r_submit` | queue submit and the graph's finish |
| `r_present` | the rest of the render set (present) |
| `r_cleanup` | cleanup |

`render_frames` counts render frames in the window. `rworst` is the window's slowest render
frame: its `render_ms`, then its eleven stages in the table's order, in ms, separated by `/`
(F853), so a single hitch shows its stage even when the means hide it.

GPU (timestamp queries, means per frame; needs `TIMESTAMP_QUERY`):

| field | interval |
|---|---|
| `gpu_ms` | first to last marker of the frame |
| `g_pre` | 3D view start to main pass, without the shadows (prepass, clears, culling) |
| `g_shadow` | the sun cascades' shadow passes (per cascade in `frame_gpu_views ... shadow_cN=`) |
| `g_opaque` | opaque and alpha-mask passes |
| `g_transp` | transparent pass (cars' glass, smoke, particles) |
| `g_post` | post process to the end of the 3D view; with `render_height = native` also the UI and the window copy |
| `g_ui` | the display camera's UI pass (scaled image shown, `mode=upscaled-image`) |
| `g_upscale` | the display camera's copy to the window |
| `g_gap` | `gpu_ms` minus the sum of the segments (between views, idle) |

`g_frames` counts frames with readable timestamps, `g_bad` frames with a negative or
missing interval (dropped). Every tenth window adds `frame_gpu_views` lines per camera
(`cam=` order, `kind=3d|2d`).

Counts (means per render frame): `d_opaque`, `d_alpha`, `d_shadow`, `d_transp`, `d_ui` are
the phase items (draw groups after batching) per pass; `shadow_views` the shadow views;
`vis` visible meshes of the 3D cameras, `casters` shadow casters of the cascades,
`views3d` 3D views. Per second: `mesh_up_s` and `mesh_kb_s` mesh asset changes that reach
the render world (each one is prepared and uploaded again; a mesh kept for the main world only
is not counted), `mesh_stream_kb_s` vertex bytes written in place over existing GPU buffers
(`mesh_stream`, no preparation; F837), `img_up_s` and `img_kb_s` the same for images.

### Ordering with Automatic smoke

The shadow timing wrapper installs once per render world at the start of RenderGraph. Its
early shadow replacement builds Core3d. The native transparency transaction must inspect
the unbuilt Core3d graph, so its dedicated pre-Render schedule runs first. The timed shadow
pass and its dependency edges are available to the first camera. [G417](../log/G0417.md)
verifies both the Automatic smoke path and actual shadow profiling on Vulkan.

### GPU markers and Metal

The start line `frame_split version=1 backend=... adapter=... timestamp_query=...
inside_encoders=... inside_passes=... gpu_markers=on|off reason=...` names what the adapter
supports. A marker is an empty compute pass with a beginning timestamp, so it needs only
`TIMESTAMP_QUERY`, which Metal on Apple GPUs has; the shadow cascades use the render pass
descriptor's timestamps. Without the feature the line says `gpu_markers=off reason=...` and
`frame_stats` says `gpu=off`; the CPU split is unchanged. `GRID_FRAME_GPU=0` turns the
markers off. They run only while the counter is shown (`gpu=hidden` otherwise).

Bevy's `RenderDiagnosticsPlugin` measures passes with `TIMESTAMP_QUERY_INSIDE_PASSES`,
which Metal lacks, and on macOS Bevy skips encoder timestamps near present
(bevy#22257, flicker), so on the Mac it gives no GPU pass times. The frame split avoids
both: no timestamps inside passes, and on macOS no marker after the last pass of the frame
(`near_present_marker=false`), so there `g_upscale` and the tail of `gpu_ms` are not measured.

### Cost

The CPU split reads `Instant::now()` about 20 times per frame and takes an uncontended mutex
per point. While the counter is shown, the markers add one empty compute pass per marker
(two per frame, four or five per 3D view, three per display view), the cascades' pass
timestamps, and one query resolve with a readback of at most 2 KiB per frame (a ring of three).

### Windows check (RTX 4070, Vulkan, 1280x800 window, Fifo at 165 Hz)

`tools/frame_split_smoke.ps1` (interactive window, autopilot), means over the driving windows:

| run | fps | avg | main (fixed) | outside | render | r_meshes | r_encode | GPU | mesh uploads/s |
|---|---|---|---|---|---|---|---|---|---|
| Detroit derby route_8, 12 cars, 720 rows | 61.2 | 17.37 | 11.44 (4.41) | 5.93 | 13.78 | 3.12 | 5.87 | 3.16 | 5369 |
| Donington route_0, 12 cars, native | 60.0 | 16.86 | 9.42 (2.46) | 7.43 | 15.65 | 5.37 | 5.83 | 2.82 | 4793 |

The parts sum to the frame as checked by the smoke. On this PC the frame is set by the CPU
(render thread and main update); the GPU is busy under 3.2 ms per frame.

## Mesh uploads per frame (F837)

17 November 2026, Claude Code (session mac-perf). The frame split showed about 5,000 mesh
asset uploads per second in a race (5,369 in the derby, 4,793 at Donington). Every mesh asset
that is `Added` or `Modified` in a frame is prepared again in the render world and its whole
vertex and index buffers are uploaded again. `GRID_MESH_UPLOADS=1`
(`crates/grid-app/src/mesh_upload_probe.rs`) prints every five seconds which meshes change,
how often and who draws them (`mesh_uploads`, `mesh_upload`, `mesh_upload_group`), and the
bytes written in place by `mesh_stream` per owner (`mesh_stream_group`).

### Sources and fixes

| source | before | fix |
|---|---|---|
| crowd card cells (`crowd_billboards::update_cards`) | every cell in range replaced its positions and normals at 15 Hz or whenever the eye moved 0.5 m | same cards: the bytes go over the mesh's GPU allocation (`mesh_stream::write`); a new asset only when the cards change |
| car batches (`car_batch`) | a dirty group rebuilt and replaced every batch mesh, also for detail levels not drawn | a group is rebuilt only while its level is shown, posed or fading out; a batch with the same layout, vertex counts and indices gets only its changed vertex runs written in place (`mesh_stream::write_at`) |
| owned part meshes (`main.rs`, `drive_present`) | every staged part mesh was assigned again, also when its shape had not changed | skipped when `body_damage::same_shape` holds |

What remains is the camera flashes, unnamed custom-material meshes (probably OSD texts whose
glyph count changes) and meshes that really change shape (the damaged car bodies).

### Statistics added

`frame_stats` now also gives `p99_ms` (nearest rank), `over10` and `over16` (frames slower
than 10 ms and 1/60 s) and, last on the line, `ft_hist=edge:count,...`: a sparse histogram of
frame times with half-millisecond buckets from 5 to 20 ms and edges at 0, 25, 33.3 and 50 ms.
`tools/frame_split_smoke.ps1` sums the histograms over the driving windows and prints the 1 %
low (the bucket that holds the slowest 1 % of frames), the share of slow frames, the slowest
window's split and whether the machine was idle at the start and the end.

### A/B on Windows (RTX 4070, i7-12700, Vulkan, 1280x800 window, Fifo at 165 Hz)

`before.exe` is this branch with the three fixes reverted (probe and statistics kept);
`after.exe` the branch. Five alternating runs of `frame_split_smoke.ps1` per scene and
variant, `GRID_MESH_UPLOADS=0`, each started only when no cargo, rustc, linker, Gradle or other
grid process ran (26 of 30 runs idle at start and end; in 4 a build started near the end).
Pooled driving windows (the first two skipped):

| scene | variant | fps | per run | 1 % low | worst window | p10 window | >1/60 s | main | outside | render | r_meshes | r_encode | GPU | uploads/s | upload KB/s | stream KB/s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Detroit derby route_8, 720 rows | before | 95.9 | 97.7/108.7/104.5/77.9/90.7 | 40-50 | 18.9 | 66.6 | 5.1 % | 7.34 | 3.94 | 10.10 | 2.23 | 4.27 | 2.49 | 4,941 | 33,636 | 142,797 |
| | after | 97.9 | 92.0/92.3/104.5/97.1/103.3 | 40-50 | 17.0 | 75.7 | 3.0 % | 7.13 | 3.76 | 9.80 | 1.75 | 4.30 | 2.52 | 324 | 4,576 | 165,624 |
| Detroit derby route_8, native | before | 102.3 | 91.4/105.6/111.8/97.1/105.7 | 40-50 | 51.4 | 72.5 | 3.4 % | 6.87 | 3.45 | 9.20 | 2.16 | 3.75 | 2.59 | 5,001 | 34,081 | 151,678 |
| | after | 107.9 | 112.7/100.9/116.4/105.1/104.6 | 50-57 | 57.5 | 88.7 | 1.0 % | 6.40 | 3.15 | 8.51 | 1.64 | 3.64 | 2.55 | 348 | 4,903 | 179,788 |
| Donington route_0, native | before | 69.8 | 75.4/67.4/73.0/66.2/66.9 | 40-50 | 46.2 | 60.3 | 8.6 % | 7.85 | 6.64 | 13.48 | 4.66 | 5.02 | 2.34 | 5,073 | 15,506 | 396,551 |
| | after | 69.0 | 60.8/69.5/74.5/69.4/71.0 | 40-50 | 45.8 | 59.9 | 13.1 % | 7.64 | 7.01 | 12.97 | 4.12 | 5.04 | 2.43 | 129 | 328 | 404,289 |

ms are means per frame; `1 % low` is the histogram bucket (fps) that holds the slowest 1 %.

- Uploads fall by 93-97 % and their bytes by 86-98 %. `r_meshes` falls by 0.5 ms and the
  render thread by 0.3-0.7 ms per frame.
- The mean fps moves by +2 %, +5 % and -1 %. Runs of one binary differ by up to ±10 %, so the
  fps effect is inside the noise; the per-stage times are the measured effect.
- The GPU needs about 2.5 ms per frame: every scene is CPU-bound. Donington is bound by the
  render thread (13 ms: `r_meshes` 4.1 ms with few uploads, `r_encode` 5 ms with about 360
  transparent, 85 shadow and 55 opaque draw groups); the main thread takes 7-8 ms.
- Single windows fall to 17-19 fps in both variants: one frame of 0.5-0.8 s with `r_prepare`
  25-44 ms near the start of the drive (pipeline or resource creation). Derby dip windows show
  `main_ms` 14-16 ms with `fixed_ms` 5-7 ms (simulation and collision bursts).
- In-place vertex writes (`mesh_stream`) carry 140-180 MB/s in the derby and about 400 MB/s at
  Donington (about 4-6 MB per frame) in both variants; the crowd cards moved about 25 MB/s from
  uploads to it. `mesh_stream_group` (one probe run per scene, not idle) names the writers:

  | owner | derby native, MB/s | Donington, MB/s |
  |---|---|---|
  | trees, facing and sheet billboards (`source_trees::present`) | 91 | 228 |
  | ground cover (`ground_detail`, gathers) | - | 43 |
  | skid marks | 19 | - |
  | tyre smoke | 20 | 1 |
  | car batches | 14 | - |
  | crowd cards | 8 | 9 |

  The trees face every billboard of every shown level toward the camera on the CPU each frame
  and write the whole vertex buffer again (F392). Facing them in the vertex shader, as the grass
  does since F580, would remove these writes and the main-thread facing work.

### Image

Offscreen fixed-step captures (`--render-fixed-step --render ... --resolution 1280x720
--drive-ticks 600|1500`, autopilot) from `before.exe` and `after.exe`, plus a second `after.exe`
run as the noise floor, compared pixel by pixel:

| scene, tick | after vs after | before vs after |
|---|---|---|
| Detroit derby, 600 | 0 px | 2 px, max delta 1 |
| Detroit derby, 1500 | 1 px, max delta 1 | 2 px, max delta 1 |
| Donington, 600 | 1 px, max delta 1 | 1 px, max delta 1 |
| Donington, 1500 | 0 px | 3 px, max delta 1 |

The image is unchanged (at most 3 of 921,600 pixels differ by one level, as between two runs of
one binary). Without `--render-fixed-step` two runs of one binary differ in 62 % of the pixels
(the frames fall on other ticks), so only fixed-step captures can be matched. Crops (centre and
lower 320x180, before | after) are in `local/ab/crops/` (private).

### Android

The same sources run on the phone, so the same uploads and their buffer reallocations go
away there. Not measured on a phone.

### Where the rest of the frame goes, and the plan to 100+ fps up to 2560x1440

Owner target (2026-11-17): 100+ fps under any conditions at up to 2560x1440 on the RTX 4070 /
i7-12700. The GPU needs 2.3-2.6 ms at 1280x800 (about 6 ms at 2560x1440 if it scales with the
pixels), so the target is a CPU problem: both threads must stay under 10 ms, also in the worst
frames.

Profile build (`--features profile`, `GRID_PROFILE_SECONDS=10`, `frame_split_smoke.ps1
-RenderHeight native`, idle machine; the trace spans slow the build to about 65 fps, so the
absolute numbers are inflated and only the ranking counts). Exclusive ms per frame:

| span (thread) | derby native | Donington |
|---|---|---|
| `mesh_stream::upload` (render: `queue.write_buffer` of the in-place writes) | 0.93 | 3.26 |
| `source_trees::present` (main: CPU facing of every tree billboard) | 0.50 | 1.48 |
| `drive_tick` (main: simulation step) | 2.90 | 1.73 |
| `frame_split::gpu_begin` (render: `device.poll` for the GPU markers, only while the counter is shown) | 1.10 | 1.17 |
| transparent pass encode (render) | 0.51 | 1.27 |
| `prepare_preprocess_bind_groups`, binned instance buffers (render) | 2.2 | 1.2 |
| visibility `par_for_each` and AABB updates (main, task pool) | 0.9 | 1.3 |
| `smoke_boundary::live::extract` | 0.26 | 0.52 |
| `mesh_stream::resort` | 0.15 | 0.36 |
| `skid_marks::present`, `car_batch::refresh`, `crowd_billboards::update_cards`, `drive_present` | 0.3-0.6 each | 0.2-0.3 each |

Ranked plan (expected gain on the bound thread; all general, image-neutral, with matched crops):

1. **Done in F838** (a compute pass instead of the vertex shader; see "Tree billboards faced on the
   GPU (F838)" below). **Tree billboards faced in the vertex shader** (`source_trees.rs` `present`/`expanded`/
   `sheet_expanded`, `tree_cards.rs` `CardMaterial`, the facing `StandardMaterial` parts via an
   extended material, shadow twins faced to the sun by a view uniform). Removes 90-230 MB/s of
   in-place writes: about 3 ms of render thread and 1.5 ms of main thread at Donington, 1.5 ms
   in the derby. Grass did the same in F580. Biggest single item.
2. **Measured in F847, nothing merged** (see "Car transparent draws (F847)" below).
   **Render-thread encode at Donington** (5-6 ms, about 360 transparent draw groups): merge the
   blended static track groups that share material and pipeline into fewer draws, check
   specialisation and bind-group churn in the transparent phase, sort once per frame.
3. **Measured in F840, not kept**: without the poll the same maintenance runs inside
   `queue.submit` (see "Frame-split poll (F840)" below). **Frame-split overhead**: `gpu_begin` polls the device every frame while the counter is shown
   (about 1.1 ms in the profile build). Poll every few frames or read back on the submit
   callback; measure first whether wgpu only moves the same work elsewhere.
4. **Done in F846** (see "Ground cover: stable billboard slots (F846)" below).
   **Ground-cover gathers** (`ground_detail`, 43 MB/s): blended cover is gathered on each 16 m
   bucket change synchronously and the whole capacity buffer is written; stream only the
   changed range or keep static chunks.
5. **Main thread** (7-8 ms in release): `drive_tick` (derby collisions, 5-7 ms bursts of
   `fixed_ms` in the dip windows), visibility and AABB recomputation for meshes changed by the
   streams, `smoke_boundary`, `mesh_stream::resort`. This fits a second session: files
   `main.rs` (`drive_tick`, `drive_present`), `smoke_boundary/live.rs`, `mesh_stream.rs`
   (`resort`), `damage_parts.rs`, `car_batch.rs`, `skid_marks.rs`; it does not touch the
   render-thread or shader work of items 1-4.
6. **Dips**: the start hitch (one 0.5-0.8 s frame, `r_prepare` 25-44 ms: pipelines and bind
   groups created on first sight; pre-warm them behind the loading screen) and the derby
   `fixed_ms` bursts (item 5).
7. **Remaining uploads**: player and opponent body meshes (about 400/s in the derby: stream the
   deformed vertices with `write_at` as the car batches do), unnamed custom-material meshes
   (probably OSD texts whose glyph count changes, `osd_scene::texts`; 54-81/s), camera flashes.

## Tree billboards faced on the GPU (F838)

Item 1 of the plan above. The source vegetation (docs/SOURCE_VEGETATION.md) faces its sphere,
sheet and card billboards toward the camera every frame. Until F838 `source_trees::present` did
this on the main thread and wrote every shown level's vertices again through `mesh_stream`
(90-230 MB/s, about 3 ms of render thread at Donington).

### Design

- A compute pass (`source_trees/gpu_facing.rs`, `tree_facing.wgsl`) at the start of the render
  graph (`RenderGraphSystems::Begin`) writes the faced POSITION, NORMAL and card UV0/UV1 straight
  into the mesh's vertex slab. The vertex slabs get `STORAGE` usage through
  `MeshAllocatorSettings::extra_buffer_usages`, set before the allocator is created.
- The main world registers a placement once (`Changed<Facing>`: world and inverse transform,
  kind, vertex layout, the unfaced source vertices shared per source mesh) and sends per frame
  only the list of shown placements and the view (right, up, eye, blend flag). One workgroup of
  64 lanes per placement; jobs are grouped per slab buffer into bindings aligned to
  `min_storage_buffer_offset_alignment` and within `max_storage_buffer_binding_size`.
- The shader mirrors the CPU math, including the guarded normalisation, so the image is the
  same.
- Why not the vertex shader, as the grass does (F580): the facing materials are shared with
  shadow twins, ornament proxies and the card material, and are swapped by the level-of-detail
  and fade code; a compute pass keeps every material and the shadow path unchanged.
- CPU fallback, per frame and per mesh: `GRID_TREE_FACING=cpu`; no compute shaders or fewer than
  4 storage buffers per stage (the install returns early, which is the old path exactly); the
  pipeline not compiled yet; a mesh in a slab without `STORAGE` (large-object slabs). Shadow
  twins are still faced to the sun on the CPU.

### A/B on Windows (RTX 4070, i7-12700, Vulkan, 1280x800 window, Fifo at 165 Hz)

One binary; `cpu` = `GRID_TREE_FACING=cpu`; `tools/frame_split_smoke.ps1 -RenderHeight native`;
5 alternating runs per scene and variant, idle-gated (no build or game process at any start);
pooled.

| scene | variant | fps (runs) | 1 % low | worst window | over 10 ms | main ms | render ms | `r_meshes` | `r_encode` | GPU ms | in-place writes kB/s |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Detroit derby route_8 | cpu | 99.0 (105.9/116.2/110.4/84.6/78.0) | 40-50 | 29.0 | 38.3 % | 7.24 | 9.52 | 1.75 | 4.07 | 3.79 | 166,369 |
| | gpu | 112.9 (129.1/125.0/122.9/80.7/107.3) | 40-50 | 48.2 | 19.9 % | 6.42 | 8.44 | 1.00 | 3.85 | 3.14 | 53,605 |
| Donington route_0 | cpu | 67.2 (68.2/78.5/74.3/61.9/53.6) | 40-50 | 32.7 | 100 % | 8.53 | 14.28 | 4.37 | 5.58 | 3.63 | 395,530 |
| | gpu | 79.1 (81.6/90.4/95.6/60.5/67.4) | 40-50 | 46.7 | 85.1 % | 7.04 | 11.53 | 1.36 | 5.70 | 3.85 | 67,624 |

- In-place writes fall by 68 % (derby) and 83 % (Donington); `r_meshes` by 0.75 and 3.0 ms.
- Render thread -1.1 and -2.75 ms per frame, main thread -0.8 and -1.5 ms.
- Mean fps +14 % and +18 %, and in each scene four of the five gpu runs beat the cpu run of the same round. The
  worst 1 s window rises from 29 to 48 fps and from 33 to 47 fps. The 1 % low bucket is unchanged.
- Runs 4 and 5 are slower in both variants (the machine, not the variant).
- GPU time is unchanged within the noise (3.1-3.9 ms in both variants); the pass was not timed on its own.

### Image and probe

- Fixed-step offscreen captures (1280x720, ticks 600 and 1500, derby and Donington), cpu vs gpu:
  0 px differ in three of the four pairs, 2 px by one level at derby tick 1500, where two gpu
  runs differ in 1 px. Unchanged. Crops in `local/ab/crops-trees/` (private).
- `GRID_MESH_UPLOADS=1` (Donington, 900 ticks): with `cpu` the tree groups write about 25, 23 and
  12 MB/s (facing), 3.1 and 2.5 MB/s (sheets) and 0.6 MB/s (cards); with `gpu` no tree group
  writes. What is left: ground cover (up to 49 MB/s), crowd cells, skid marks, tyre smoke.

### Android

The same code; phones with compute shaders and enough storage buffers take the GPU path, others
the CPU path. Not measured on a phone.

F841 makes the choice testable and covers the limits: `gpu_path_supported(flags, limits)` needs
`DownlevelFlags::COMPUTE_SHADERS`, `max_storage_buffers_per_shader_stage >= 4` and 64-wide
workgroups (tests: wgpu desktop defaults and GLES 3.1-class `downlevel_defaults` take the GPU
path; WebGL2 limits, no compute, or 0-3 storage buffers take the CPU path). A mesh larger than
one storage binding goes to the CPU path instead of being skipped, and placement or source
tables larger than one binding turn the GPU path off from the next frame. Metal on the Mac meets
the limits and takes the GPU path (not run on a Mac yet).

### Next

Donington is still render-thread bound (11.5 ms: `r_encode` 5.7 ms with about 360 transparent
draw groups), the derby main-thread bound in its dips. Items 2-4, 6 and 7 of the plan above
remain; item 5 (main thread) is planned as a separate session.

## Main thread: collision bursts and per-frame damage reads (F839)

17 November 2026, Claude Code (session main-thread). Item 5 of the plan above: the derby's
`fixed_ms` bursts and the main-thread work around them.

### Diagnostics added (`GRID_PERF_TIMING=1`)

- `perf_timing` gains `ticks_over_2ms/4ms/8ms` and `worst_tick` (shared-world ticks), the top
  eight stages of the slowest tick (`worst.<Stage>`, grid-world stages included) and the
  slowest single call of every stage over 0.5 ms (`max.<Stage>`). The means hide the bursts;
  these name them.
- New stages: `HullRebuild` (a deformed car's collision hull, `BodyGeometry`), `PropDisable`
  (broken prop boxes switched off in every car's track), `Checkpoint` (grid-app
  `flashback::capture`, every 4 s).
- grid-app prints `perf_rebounded` every 600 frames: the entities whose `Aabb` Bevy's
  `calculate_bounds` recomputes (mesh asset or handle changed), by name, with mean count and
  vertices per frame. In the derby it is about 5 entities and 2,000 vertices per frame (unnamed
  meshes and a few ornaments), so AABB recomputation is not a main-thread cost worth a change.

### What the bursts are

Timed derby runs (`frame_split_smoke.ps1`, Detroit route_8, native; props on as in the game):

- The slowest shared-world ticks take 5-7 ms. Their largest stages are the continuous stage of
  fast prop pieces and lost wheels (`Continuous`, 1.6-2.9 ms in the worst tick), the solver
  (`WorldAdvance`, `IslandFrame`, `FrameGroup`) and, twice per race, `PropDisable` (1.2-1.7 ms
  per call, up to 3.3 ms: every car's track copy switches the broken boxes off). One run had a
  13.7 ms `VehicleStage` tick (single occurrence, machine busy).
- `Checkpoint` costs 1.1 ms (up to 1.5 ms) every 4 s; minor.
- Sharing the disabled-box list between the cars' tracks instead of the per-car loop gave no
  measurable gain (both 1.2-1.4 ms per call), so it was not kept.
- Dip windows show `fixed_ms` 6-10 ms per frame while the slowest single tick is 5-7 ms: a dip
  is several expensive ticks in the frames after a slow one, not one pathological tick
  (interpretation).

### Fixes

| change | where | effect |
|---|---|---|
| The continuous stage sweeps every fast body at once on the compute pool when each part has its own body, then applies the results in part order. A sweep reads only its body and the static track; an early sweep is used only when the record cap (`MAX_RECORDS`) could not have cut it (every query keeps the hits it found before its capacity, so with fewer hits than the room left it is the same sweep), otherwise it is swept again in order. | `PropWorld::continuous` (`dynamic_props.rs`), callers in `source_vehicle.rs` and `multi.rs` | identical results; see below |
| Damage presentation skips a car whose deformation words equal the last presented key, checked in place (`body_deformation::unchanged`) before `Field::read` copies up to 800 nodes and builds a new key | `drive_present` (player, opponents), `damage_parts::present` (live only; replays keep their path) | about 0.03-0.05 ms per frame less in `DamageParts` alone |

### Identity

`grid-opponent-race` with `GRID_THREADS=1` (sequential) against the default pool: 12-car,
12-car-df and derby-df match `tools/identity.tsv`; with `GRID_DYNAMIC_PROPS=1` the 11,200-tick
derby (4 breaks, 48 pieces) gives the same hash sequential and parallel (`4818bd38…`), as do
the other three runs. Unit test `unchanged_matches_read_key` checks that `unchanged` agrees
with `Field::read`'s key, also for words outside the key and refused activity bytes.

### A/B

Headless continuous stage, derby 11,200 ticks with props (48 pieces), pool on, sequential
(temporary switch, removed) against parallel, three alternating pairs, identical hashes:

| pair | `Continuous` mean ms, seq / par | `max.Continuous` ms, seq / par |
|---|---|---|
| 1 | 0.107 / 0.071 | 1.29 / 0.68 |
| 2 | 0.128 / 0.090 | 1.70 / 0.77 |
| 3 | 0.151 / 0.103 | 2.27 / 0.92 |

In-app derby (native, 1280x800 window, `GRID_PERF_TIMING=1`, `before.exe` = F837 with the
timers, `after.exe` this change; two alternating pairs, other sessions' cargo builds ran during
3 of the 4 runs):

| variant | `Continuous` ms/call | `DamageParts` ms/call | `DriveTick` ms | ticks > 4 ms |
|---|---|---|---|---|
| before | 0.45 / 0.36 | 0.103 / 0.086 | 3.02 / 2.54 | 217 / 41 |
| after | 0.17 / 0.18 | 0.070 / 0.067 | 2.36 / 2.33 | 4 / 27 |

Three further untimed pairs (busy machine in 2 of 6 runs) gave 87.2 / 84.3 / 78.0 fps before and
75.5 / 83.1 / 85.4 fps after: the whole-frame effect is inside the run-to-run spread and is not
claimed. The stage times are the measured effect.

### Image

Fixed-step offscreen captures (1280x720, ticks 600 and 1500, derby and Donington, autopilot):
before against after differ in 0, 0, 2 and 1 pixels by one level, two runs of after in 0, 0, 1
and 4. The image is unchanged.

## Frame-split poll (F840)

Item 3 of the F837 plan. `frame_split::gpu_begin` polls the device (`PollType::Poll`) on the
render thread every frame while the GPU markers exist. wgpu-core 29 `Queue::submit` already ends
with the same non-blocking `device.maintain`, so the poll looked redundant.

Experiment: `gpu_begin` without the poll, against the pushed F838 binary, 5 alternating
idle-gated rounds per scene (log/F0840.md has the full table). The marker readbacks still
complete (99.9 % of frames, no bad frames). Paired per round, `r_submit` grows by 0.66 ms
(derby) and 0.90 ms (Donington), in all 10 pairs, while `r_encode` (which holds the graph
begin) falls by 1.3 ms in the derby and not at Donington. The render thread moves by -1.2 ms
(derby, median -0.4) and +1.8 ms (Donington, worse in 4 of 5 pairs) in a noisy session.

So the cost is the maintenance itself (submission triage, freeing resources dropped this frame,
callbacks), not the extra call; dropping the poll only moves it. The change was not kept. A real
gain needs less to maintain: fewer buffers and bind groups created and dropped per frame
(item 7) and a profile of `Device::maintain`.

## Main thread: prop breaks copy switches, not boxes (F842)

17 November 2026, Claude Code (session main-thread). Item 5 of the plan above, the worst
frames: a prop break cost up to 12 ms on the main thread in one tick.

### What the spike was

`Props.boxes` (`entity_geometry.rs`) is an `Arc<Vec<PropBox>>` shared copy-on-write by the
twelve cars' track copies and grid-app's flashback checkpoints. A break switched its boxes
off with `Arc::make_mut`, so the first break after a checkpoint copied the whole box list
(each box holds a template name, eight corner positions and a shape) once per car. Temporary
counters in `PropWorld::disable` measured 12 copies of 3,621 boxes = 8.1 ms at Long Beach and
12 copies of 1,664 boxes = 3.3 ms in the Detroit derby; a second break before the next
checkpoint changed the now unshared lists in place (1 us). The headless `grid-opponent-race`
never shares the lists, so it never showed the copy. F839's shared disabled-box list changed
the loop around the copy, not the copy, and gave nothing.

### Fix

| change | where | effect |
|---|---|---|
| The collision switch moves out of `PropBox` into its own shared list, `Props.enabled: Arc<Vec<bool>>`. `Props::new` turns every box on except the damage states' static ones, `Props::disable` turns a break's intact boxes off and its damage boxes on, the queries read `enabled[index]`. The box list stays shared; a break copies one byte per box for each car. | `entity_geometry.rs`, `PropWorld::arm` / `disable` (`dynamic_props.rs`), three probe bins | the copying call 6.1-12.1 ms -> under 0.5 ms |

A draft that copied once for all cars per break (`disable_all`) only went from 4.2 to 2.9 ms on
noisy runs and was dropped.

### Identity

`grid-opponent-race`, `GRID_THREADS=1` against the pool: 12-car, 12-car-df and derby-df match
`tools/identity.tsv`; with `GRID_DYNAMIC_PROPS=1` the four runs, including the 11,200-tick derby
(4 breaks, 48 pieces, `4818bd38…`), hash the same sequential and parallel and as in F839. Unit
test `disable_copies_the_switches_and_shares_the_boxes` checks that a car's switch copy leaves
the shared switches and boxes alone and that an unshared list changes in place.

### A/B

In-app, `frame_split_smoke.ps1` (native 1280x800 window, `GRID_PERF_TIMING=1`), before = F839
with the uncommitted drafts and slow-frame attribution, after = the same plus this change; two
alternating pairs per track, idle machine (no cargo, rustc or other grid process at the start
and end of every run). Long Beach (`race-long-beach.ini`, two breaks at ticks 1295-1296):

| pair | `PropDisable` slowest call ms | `max.Commit` ms | `worst_tick` ms | `max.DriveTick` ms | max frame in the break's window ms |
|---|---|---|---|---|---|
| 1 before / after | 6.10 / < 0.5 | 6.40 / 1.42 | 7.67 / 3.01 | 8.66 / 4.21 | 18.5 / 14.3 |
| 2 before / after | 12.06 / < 0.5 | 12.43 / 1.20 | 13.87 / 3.84 | 15.19 / 5.34 | 29.2 / 25.5 |

`PropDisable` per call (mean of the two) 3.05 / 6.03 ms before, 0.008 / 0.010 ms after; in the
derby 0.004-0.006 ms after (its breaks fall after the last timing print of the before runs).
Four of the eight runs, of both variants, ran locked near 60 fps for the whole run (16.7 ms
frames, `outside_ms` 9-10 ms), the others at 84-127 fps, so whole-run fps and frame counts are
not compared here.

### Image

No render code reads the switches and the simulation is identical, so the image cannot change;
no captures were made.

## Main thread: skid-mark and smoke-boundary caches, slow-frame split (F843)

### Skid marks

`present` (skid_marks.rs) rebuilt every strip's vertices to find the sort
centre each frame. It now keeps each strip's bounds (`bounds`, set from the
runs `changes()` reports) and takes the centre from them; a quad's vertices
come from `quad_vertices`. The unit test checks that the cached centre equals
the full scan for random edits.

### Smoke boundary

The render-world extract (smoke_boundary/live.rs) searched every mesh entity
for the five smoke-boundary owners each frame. It now keeps their entities and
mesh ids (`SmokeOwners`), checks them with one `get` each, and scans again
only when a cached entity loses its mesh or a changed `Mesh3d` names a cached
mesh on another entity.

### Slow-frame split in frame_stats

frame_split's window report gains, for frames over 10 ms and 14 ms
(`slow10_*`, `slow14_*`): their count and mean frame, main, fixed (FixedMain
time inside the frame), outside, render, fixed ticks per slow frame, and how
many ran more than one tick. perf_timing gains `RaceUpdate` (the race
update after the runtime tick in drive_tick).

Since F863 the slow-frame fields also carry `slow10_laps` and `slow14_laps`: the main laps (`dt dp
dpo dpn gd gdg tp cb out mt`, the order of `lap_*`) averaged over those frames, slash-separated. The
window also splits all its frames by fixed ticks: `tick0_*` holds frames without a FixedMain tick
and `tick1_*` frames with one or more, with the same fields as `slow10_*`.

### A/B

`local/mt/buildsk.ps1` (under the build lock; release tests of the changed
modules pass, 150 passed): A = `local/mt/base842.exe` (origin/main F842),
B = `local/mt/new.exe` (this change), `local/mt/cost.ps1`: offscreen
1280x720, `--render-fixed-step`, the Detroit derby, 1,500 drive ticks,
`GRID_PERF_TIMING=1`, three alternating pairs. Idle machine: no cargo or
rustc process before the pairs and no busy process during any run
(`busy_procs=0`). Raw: `local/mt/buildsk.txt`.

| ms per frame | A1 | B1 | A2 | B2 | A3 | B3 |
|---|---|---|---|---|---|---|
| main | 6.112 | 5.860 | 6.017 | 5.626 | 5.814 | 5.546 |
| MainSchedule | 5.715 | 5.610 | 5.702 | 5.287 | 5.424 | 5.204 |
| skid present | 0.411 | 0.123 | 0.407 | 0.117 | 0.400 | 0.118 |
| extract | 0.392 | 0.388 | 0.381 | 0.359 | 0.356 | 0.356 |
| render | 7.321 | 7.429 | 7.213 | 7.132 | 7.008 | 7.013 |

Main thread −0.25 to −0.39 ms per frame in every pair, most of it the skid
present (−0.29 ms). The derby has little smoke here, so the extract gains
less than in the earlier loaded batch (`local/mt/cost-sk.txt`, F839 base:
extract 1.14–1.31 -> 0.96–1.10 ms). The simulation is not touched (the new
`RaceUpdate` timer only reads the clock); the skid vertices and the smoke
mesh are the same, so the image is unchanged by construction.

Two runs (A2 and B1, one per variant) had one tick of 175 and 189 ms, spent
in different stages (`CarPairs`, `BodyEvents`). Interpretation: the thread
was stalled by the system (the PC runs near its commit limit), not by
either variant.

### What slow frames are made of

Six earlier windowed derby runs of the draft (`local/mt/ab/sk-t-B*`,
`skw-B*`, machine not idle, so only the proportions count) had 1,650–2,275
frames over 10 ms each. Those frames averaged 7.5–12.7 ms main, 2.9–5.2 ms
fixed, 9.1–14.6 ms render and 5.3–10.1 ms outside, with 0.98–1.18 fixed
ticks per slow frame; 1–14 % of them ran more than one tick. Interpretation:
on this machine the slow frames are mostly render and presentation, and
catch-up ticks are a minor cause.

### Stdout cost depends on the launcher

RaceUpdate measured 0.42–0.44 ms per tick in Start-Process runs; a temporary
probe put 0.42 ms of it in the report lines a tick writes to stdout
(`local/mt/buildsp.txt`, `RPROBE`). A stand-alone writer
(`local/mt/wbench.rs`: four report-sized lines per tick through `writeln!`;
raw `local/mt/wbench/`) gives the per-tick cost by launcher:

| Launch | µs per tick |
|---|---|
| cmd `> file 2>&1` (the owner's `Play GRID Prototype.cmd`) | 21.3–22.4 |
| PowerShell `Start-Process -RedirectStandardOutput` | 245.6–253.2 |
| same, one buffered write per tick | 73.7 |

Formatting is 10–12 µs of it. A WriteFile costs about 5 µs to a file cmd
opened and about 60 µs to the one Start-Process opened. The owner's launcher therefore pays about 0.02 ms per
tick; harness scripts that use Start-Process (frame_split_smoke,
career_loop_smoke, front_end_loops, menu_loop_smoke, run_grid_macro, …) add
about 0.2 ms per tick that players do not pay. Their absolute DriveTick and
fixed numbers are high by that much; A/B pairs from the same harness stay
valid.

### Present mode in the harness

frame_split_smoke uses the default present mode (AutoVsync, which resolves to
Fifo). Some runs locked at exactly 60.0 fps on the 165 Hz monitor, with about
9.5 ms outside the frame. The owner plays with `auto_no_vsync`; interactive
A/B runs pass `-Extra "present_mode = auto_no_vsync"`.

## Main thread: flashback checkpoints from the live words, packed in parallel (F844)

Every 250 ticks (4 s) `flashback::capture` stores a checkpoint. Before, it
cloned the whole state, including every car's vehicle words (33,792 words), its
deformation owner and its solver rows (12,288 words), and then compared the
copies with the previous base to keep only the runs that differ.

Now (`crates/grid-app/src/flashback.rs`):
- `Checkpoint::take_live` moves those three word blocks out of the live runtimes,
  copies the rest, packs the words straight from the live blocks and moves them
  back. `capture` reaches `Driving` through `bypass_change_detection`, because
  the state ends up unchanged. The pause-menu path (`watch`) keeps the
  copy-then-pack route.
- `Packed::pack` takes a borrowed slice. It skips equal words 64 at a time with
  a slice compare, and its runs are the same as before. A unit test covers
  changes at block edges.
- `pack_words` packs each car against its own bases on Bevy's compute pool, in
  one scope. The scope returns the cars in spawn order. Without a pool (tests)
  it packs them one after another.

A probe (not committed) measured the serial pack per car:
- vehicle words: 20–25 µs when equal, 45–70 µs once about 4,600 words differ in
  about 735 runs;
- solver rows: 5–10 µs;
- owner: 17–25 µs, for the one car that has one.

Both are memory-bound. With 12 cars that adds up to 0.4–0.8 ms. Copying the
rest of the state costs 0.2–0.4 ms; the opponents' multi-car state is the
largest part.

**A/B.** Offscreen 1280×720 fixed-step Detroit derby, 1,500 ticks (7
checkpoints), alternating pairs. Every pair started with no cargo or rustc
running (`busy_procs=0`). A is F843 (`local/mt/base843.exe`).

| Variant | Checkpoint mean, ms | Checkpoint max, ms |
|---|---|---|
| A: F843 | 0.97 / 1.38 / 1.04 | 1.72 / 1.80 / 1.44 |
| B: serial pack from the live words | 0.69 / 0.75 / 0.71 | 0.86 / 0.88 / 0.98 |
| B: same, cars packed in parallel | 0.48 / 0.38 / 0.50 | 0.66 / under 0.5 / 0.64 |

- The serial row comes from an earlier batch of three pairs (`local/mt/buildck.txt`).
  Its A runs measured 1.06 / 0.83 / 0.78 mean and 1.54 / 0.93 / 0.92 max.
- The parallel row's batch is in `local/mt/buildpar.txt`.
- perf_timing lists a stage's maximum only when it is over 0.5 ms. B2 of the
  parallel batch lists none for Checkpoint, so its maximum was under 0.5 ms.
- The rest of the frame differed between runs as much as between variants:
  main 5.7–8.6 ms in both. Only the checkpoint stage is compared here.
- The simulation is unchanged: the packed runs and the restored state are the
  same, and the identity check is in prepush.

## Main thread: skid marks write only the changed quads (F845)

### What was left

After F843, skid present still cost 0.29–0.40 ms per frame late in the derby,
although the mesh stream wrote only about 2 KB per frame. An instrumented build
split a frame with changes like this:

| Part | Cost |
|---|---|
| `changes()` | 190–200 µs |
| packing the changed strips | 85–109 µs |
| sort centre | 37–48 µs |
| stream lock wait | about 1 µs |

About 225–233 of the 240 strips were in use, and 28–37 changed per frame.
`changes()` compared every strip with its written copy, point by point. A
changed strip was then repacked whole (32 quads), although an advancing strip
changes one or two quads and a fading strip two.

### Fix

- **Touched slots.** The strip slots are a `Pool`. It has no `DerefMut`, and `IndexMut` marks the slot touched, so every mutation goes through the tracking. `changes()` compares only the touched slots. In test builds it asserts that an untouched slot still equals its written copy.
- **Changed quads.** Of a changed strip, `changes()` reports the quads from the first to the last that differ (`changed_quads`, over `quad_inputs`), or all 32 when the bank or `linked` changed. Runs are global quad indices: `pack_quads` packs them, and `present` writes them at `run.start * 4` vertices.
- **Bounds.** Strip bounds use `quad_corners` (positions only) instead of the full vertices.

The unit test `changed_strip_runs_rebuild_the_whole_buffer` covers advancing, ending, a checkpoint restore and a pool filled by 1,500 short strips (steals, retiring, fading). After every write it checks that the streamed bytes equal a whole repack and that the cached centre equals the full scan.

### A/B

The A/B used `local/mt/cost.ps1`: offscreen 1280x720, fixed step, the Detroit derby, 3,000 ticks, three alternating pairs.

- A = F844, B = this change.
- The pairs ran under the build lock on an idle machine: no cargo, rustc or grid-app before them, and `busy_procs=0` in every run.
- Raw output: `local/mt/skq2.txt`.

| ms per frame | A1 | B1 | A2 | B2 | A3 | B3 |
|---|---|---|---|---|---|---|
| skid present | 0.291 | 0.094 | 0.299 | 0.106 | 0.288 | 0.091 |
| main | 5.255 | 5.857 | 6.305 | 5.956 | 5.667 | 5.814 |
| MainSchedule | 5.172 | 5.715 | 6.089 | 5.816 | 5.577 | 5.659 |

Skid present fell by 0.19–0.20 ms per frame in every pair. The main-thread mean varied between runs by more than that, so only the skid stage is compared.

### Image

The mesh is unchanged: after every write, the streamed bytes equal a whole repack.

The final screenshots of these runs cannot check this. One executable gives two or three different final frames across runs. For example, hash 4a63f5bd… came from A in two runs and from B in one.

## Car transparent draws (F847)

Item 2 of the F837 plan: the Donington render thread spends 5-6 ms in `r_encode`, mostly in the
Transparent3d phase. Goal: fewer draws or switches there with no visible change.

Census (a diagnostic module, not committed), Donington race, native, 420 frames: 243.6
transparent items and 238.6 draws per frame, 7.1 pipelines, 41.8 materials, 124 pipeline switches
and 194 material switches per frame. Almost all of them are car parts:

| kind | draws per frame |
|---|---|
| reflection overlays of opponent glass | 53.2 |
| opponent glass panes | 53.2 |
| overlays of player opaque parts | 40.0 |
| overlays of batched opponent parts | 27.4 |
| overlays of opponent wheels | 23.9 |
| other blended parts | 10.0 |
| player glass, and its overlays | 10.0 + 10.0 |
| overlays of player wheels | 8.0 |
| unclassified | 3.0 |

Only 39.9 draws per frame share pipeline, material and mesh slab with their neighbour. No merge
keeps the image:
- each glass overlay is drawn right after its pane (sort bias 0.01). A merged draw of a car's
  panes followed by a merged draw of their overlays draws pane 1, pane 2, overlay 1, overlay 2,
  so a rear pane's reflection lands over the front pane;
- opponent wheels rotate on their own transforms;
- the opaque-part overlays of batched opponents are already merged with the batch;
- the player car is never batched (G300).

Upper bound: the same binary with `GRID_CAR_REFLECTIONS=static` against `off` (all of the
~160 overlay draws gone), baseline MSAA, 4 idle-gated alternating pairs:

| variant | fps | 1 % low | worst window | render ms | `r_encode` ms | GPU ms | main ms |
|---|---|---|---|---|---|---|---|
| static | 79.7 | 44.4 | 58.9 | 10.46 | 5.26 | 2.54 | 6.42 |
| off | 79.8 | 44.4 | 59.0 | 9.03 | 4.08 | 2.40 | 6.47 |

Paired (off minus static): `r_encode` -1.18 ms (every pair -0.9 to -1.6), render -1.43 ms, GPU
-0.14 ms. So removing every overlay saves about 1.2 ms of encode and does not move the frame
rate at this scene. A material sort of the ~99 opaque-part overlays (bias -1e6) would win part
of that and needs `mesh_stream::resort` (item 5); the coordinator chose to skip it. Nothing was
changed.

F865 later merged neighbouring overlays of different parts without reordering them: a bindless
overlay material shares one bind group, so the batcher no longer needs the same material (see
"Transparent draws: bindless car reflection overlay (F865)").

## Ground cover: stable billboard slots (F846)

Item 4 of the F837 plan. Before, each regather of the ground cover (`ground_detail`, every 16 m
bucket change) packed every gathered billboard and wrote the whole capacity buffer, 4.7 MB at
Donington (F592), 43-49 MB/s in a race (F837, F838).

Change (`ground_detail.rs`): a slot table keeps which cover instance each billboard slot of the
padded asset holds. A regather returns only the instance indices (on the compute pool as before);
`apply` updates the table and builds, packs and writes (`MeshStream::write_at`) only the runs of
slots that changed (runs closer than 8 slots are joined). Free slots get the padding billboard
(limit 0, never drawn), as the asset's padding always had.
- Masked and A2C cover (draw order does not matter): an instance that stays keeps its slot; a new
  one takes the lowest free slot.
- Blended cover (the default in Enhanced and with `anti_aliasing = 3`) keeps the gather's fixed
  global order (bucket x, bucket z, index), which is its draw order. The new placement keeps every
  instance that still sits after its predecessor in place and puts new instances in the slots
  before the next kept one; a slot is written only when its instance changes. When the new order
  does not fit, the gather is placed again centred in the capacity and written whole (also on
  the first gather and when the asset grows).

Unit tests: the unordered table writes only changed slots; the ordered one keeps the drawn
order equal to the gather order through a 24-step diagonal drive over 7x7 buckets, rebuilds at
most 8 times and writes 1,707 slots where whole writes would be 4,392 (39 %): each step east
appends a column after the last one, which the slack of a centred layout holds about once.

A/B: Donington race, native, 1280x800 window, `GRID_MESH_UPLOADS=0`, idle-gated alternating
runs (0 busy starts); before = `cen.exe` (F841 c12f3e5a plus the census module, which only counts),
after = the same plus this change. Baseline is the default MSAA profile (A2C cover); Enhanced is
`--graphics enhanced` (blended cover). DLAA needs the `dlaa` feature build with the NVIDIA SDK
and was not measured; it uses the same blended cover path as Enhanced, so the Enhanced rows stand
for it.

| mode | variant | runs | fps | 1 % low | worst window | main ms | render ms | GPU ms | `mesh_stream` KB/s |
|---|---|---|---|---|---|---|---|---|---|
| baseline (MSAA, A2C) | before | 7 | 94.1 | 70.2 | 64.9 | 5.72 | 9.84 | 2.24 | 69,858 |
| baseline (MSAA, A2C) | after | 7 | 96.3 | 65.6 | 41.5 | 5.52 | 9.70 | 2.38 | 17,180 |
| Enhanced (blended) | before | 6 | 76.6 | 44.4 | 14.2 | 6.48 | 12.54 | 2.16 | 73,393 |
| Enhanced (blended) | after | 6 | 74.8 | 44.4 | 8.7 | 6.72 | 13.05 | 2.07 | 40,788 |

Paired per run (after minus before): baseline render -0.14 ms (+1.1 to -1.5), main -0.20, GPU
+0.14; Enhanced render +0.51 ms (+2.0 to -0.6), `r_prepare` +0.24, main +0.25. The streamed bytes
fall by 75 % (baseline) and 44 % (Enhanced; the blended order forces more rewrites); the frame
times move inside the run-to-run spread, so no frame-rate gain is claimed. The lower worst
windows of the after runs are not the cover: in baseline run 1 every timer was slower (fixed
3.7 ms, interference), and baseline run 2's 109 ms frame came at tick 128 during the countdown
with the cars stopped (the start hitch, item 6). In Enhanced, the first pair (the first runs of
the session) had one 1.5 s (before) and 1.9 s (after) frame in the 6th and 8th window, with
`r_prepare` 50 and 92 ms in that window: pipelines created on first sight, in both variants (item
6). The worst windows of the other five pairs are 61-69 fps before and 61-71 after.

Image: fixed-step offscreen captures (1280x720, autopilot, ticks 900 and 1800, Donington and
Spa, baseline and Enhanced), before against after: 0 pixels differ in all 8 pairs; two runs of
the after binary differ in 0-2 pixels (noise). Unchanged. Crops in `local/ab/crops-cover/` (private).

## GO spike: single FixedUpdate tick not reproduced (F849)

Report: about 1 in 10 Donington starts (`auto_no_vsync`) showed one 180-290 ms FixedUpdate tick
just after GO. Task: name the stage with `perf_timing worst_tick` and fix the cause.

Why `worst_tick` cannot name it: `perf.rs` measures only the WorldTick stage
(`add(Stage::WorldTick)` to `tick_end`), not the whole FixedUpdate schedule, so a stall in another
fixed system does not appear in it.

Evidence [V]:
- mac-perf's hitch runs (100 runs with `frame_stats`): 38 one-second windows after tick 150 had a
  max frame of 100 ms or more. 37 were render-led (the first-sight compiles of the start, which
  are mac-perf's area). One was fixed-led: `hw-base-b-race-native-2` (05:16 on 2026-11-18),
  window max 289.9 ms, `slow14_fixed` 35.7 ms over 8 slow frames (286 ms in all), `slow14_ticks`
  0.88, `slow14_multi` 0, render 10.1 ms, 50.3 ticks/s. The drive samples for ticks 240 to 251
  are 462 ms apart instead of 176 ms. So one tick held the main thread for about 280 ms at GO
  while the render thread was not stalled.
- A local probe build (branch `spike-probe`, never pushed: per-system wall and thread-CPU timers
  for the 23 FixedUpdate systems, page faults, working set, and a timer over the whole
  `RunFixedMainLoop`) logged every tick of 15 ms or more. In 40 Donington starts on a quiet PC
  (10:46-11:26, after the owner's game session), each 400 ticks past GO, no FixedUpdate tick
  reached 15 ms. The largest frame after the cover lifted was 74-86 ms, and it was render-led.
  A heartbeat line showed the probe was armed in every run.
- 16 earlier starts ran while another game used the PC. Their durations are not used.
- The race checkpoint written at GO took 0.6-1.9 ms in every run.

Interpretation [I]: on a quiet PC the rate is below about 1 in 19 (0 of 56, rule of three at
95 %). A 1-in-10 rate would give 0 of 40 with a probability of about 0.3 %. The one observed case
ran during mac-perf's series, whose idle gate is checked only at the start of each run, so other
sessions' builds or captures may have been running at that moment. The likeliest causes are
contention outside the game (paging or disk I/O while code and data are touched for the first
time at GO) rather than a fixed system with a slow path. This is not proven: without per-system
times from a spiking run, no stage can be named, and nothing was changed.

Next: either a permanent, cheap record of the worst FixedUpdate tick (its tick number, the time
of each fixed stage, the page-fault delta) in the normal `frame_split`/`perf_timing` output, so
the next occurrence names its stage; or an overnight series of the probe build while other
sessions build, to test the contention explanation.

## 1440p render breakdown (F850)

Donington race, 2560x1440 native window, `present_mode = auto_no_vsync`, F848 executable, idle
PC. The render thread sets the frame; the GPU does not:

| mode, resolution | fps | main ms | render ms | `r_encode` | `r_prepare` | `r_meshes` | `r_submit` | GPU ms |
|---|---|---|---|---|---|---|---|---|
| baseline 1440p | 87.6 | 6.08 | 11.23 | 5.21 | 1.65 | 1.20 | 0.91 | 4.51 |
| baseline 800p | 85.0 | 6.54 | 11.26 | 5.59 | 1.70 | 1.24 | 0.75 | 2.59 |
| Enhanced 1440p | 62.6 | 8.07 | 15.81 | 7.65 | 2.22 | 1.54 | 1.54 | 4.74 |
| Enhanced 800p | 68.1 | 7.57 | 14.25 | 7.39 | 2.10 | 1.45 | 1.01 | 2.22 |

Two 3D views render every frame: the main camera and the OSD camera (`osd_scene.rs`, the
dashboard and dial HUD drawn into a window-sized MSAA-4 image, 82 blended draws). The upper bound
of the HUD view, `GRID_PERF_TEST=hud` (OSD camera off, HUD hidden), 4 runs per variant in both
orders, play after the cover:

| variant | fps | 1 % low | worst window | main ms | render ms | `r_encode` | GPU ms |
|---|---|---|---|---|---|---|---|
| baseline, HUD on | 87.8 | 58.0 | 58.7 | 5.88 | 10.98 | 5.24 | 4.54 |
| baseline, HUD off | 99.5 | 65.6 | 67.9 | 5.64 | 9.62 | 4.27 | 4.35 |
| Enhanced, HUD on | 67.3 | 44.4 | 46.0 | 7.13 | 14.38 | 7.09 | 4.33 |
| Enhanced, HUD off | 80.1 | 58.0 | 59.1 | 6.15 | 11.98 | 5.38 | 4.31 |

The HUD view costs 1.4-2.4 ms of render thread, but in many small pieces (profile build, span
diff): 13 extra command buffers per frame; transparent pass encode about 0.34 ms; then Bevy's
per-camera nodes at 0.02-0.07 ms each (empty opaque pass, upscaling, five GPU-preprocess and
indirect-parameter nodes); our five GPU-marker nodes while the counter is shown; 0.12 ms more
submit; 0.15 ms of main-thread visibility. Bevy 0.19 runs the indirect-parameter clear and build
nodes once per camera although they cover all batches. No cut of that size keeps the image: light
clustering off for the OSD camera saves about 0.03 ms; drawing the HUD in the main view or less
often changes what is shown. Nothing was changed.

Noise: two identical Enhanced configurations interleaved differ by up to 1 ms render and 5 fps,
so Enhanced A/Bs here need both orders.

## Worst fixed tick (F851)

Follow-up of F849: the single slow FixedUpdate tick after GO could not be attributed because
`perf_timing worst_tick` covers only the WorldTick stage. `crates/grid-app/src/fixed_ticks.rs`
now times every system of the fixed step, always, so the next slow tick names itself.

How: when app setup ends (`Plugin::cleanup`, before any schedule is built), every system of
`FixedMain` and of each schedule in `FixedMainScheduleOrder` (FixedFirst, FixedPreUpdate,
FixedUpdate, FixedPostUpdate, FixedLast and any added later to that order) is replaced in the
schedule graph by a wrapper. The wrapper delegates everything to the original system (name,
type, flags, access, change ticks, sets, so ordering, run conditions and ambiguity checks are
unchanged) and adds the wall time of its run and of its deferred commands to its slot. Bevy's
`FixedMain` system (`run_fixed_main`, one call per tick) bounds the tick: it clears the slots,
reads the process page-fault count before and after (Windows `K32GetProcessMemoryInfo`, Unix
`getrusage`), and reads the race tick when the tick ends. Cost: two `Instant::now` and one
atomic add per system run.

Output:
- At start: `fixed_ticks wrapped=N rev=<revision> systems=FixedMain:1,FixedFirst:n,...`, the
  build revision (`GRID_BUILD_REVISION`, set by the release scripts; `unversioned` in ad-hoc
  builds) and the system count of each fixed schedule.
- `frame_stats`: `fixed_max_ms`, the slowest single tick of the window.
- `fixed_tick_slow ms= rev= run= tick= faults= <schedule>/#<index>=ms ...` for every tick of
  40 ms or more (at most 100 lines per run), with the 8 slowest systems. `run` counts fixed ticks
  since start, `tick` is the race tick after the tick (`-` before a race), `faults` the page
  faults during the tick (soft and hard). Systems of one schedule run in parallel, so their
  times can overlap; time spent outside any wrapped system (Bevy's executor, systems added after
  setup) is the difference between `ms` and the systems.
- `perf_timing` (`GRID_PERF_TIMING`): `fixed_worst_ms`, `fixed_worst_rev`, `fixed_worst_run`,
  `fixed_worst_tick`, `fixed_worst_faults` and `fixed_worst.<schedule>/#<index>=ms`, the run's
  slowest tick.

Names: systems are printed by their index in the schedule (the order they were added), because
release builds have no system names: Bevy keeps them only with its `debug` feature, which
grid-app enables only under `--features profile`. Turning it on for normal builds would
recompile every Bevy crate and need a new shared seed, for a diagnostic alone (coordinator,
2026-11-18). To name an index, build the same revision with the profile feature and run it with
the same arguments and `GRID_FIXED_TICK_NAMES=1`:

```
cargo build --release -p grid-app --features profile
GRID_FIXED_TICK_NAMES=1 target/release/grid-app <same arguments>
```

It prints the start line, then `fixed_tick_name <schedule>/#<index> <system path>` for every
wrapped system, and exits before the window opens. Use the table only when its `systems=`
counts equal those of the run being read: indices follow the order systems are added, so a
different revision, or arguments that add or skip fixed systems, shift them. A build without
the names prints `fixed_tick_name unavailable: build with --features profile` instead.

Reading a spike: many faults with no slow system means paging or I/O outside the game's code;
one system with most of `ms` names the stage.

## drive_present, solver stages and visibility (F852)

Where the main thread's driving presentation goes, measured with temporary sub-laps in
`drive_present` and `body_damage::project` (not committed; the probe diff is kept locally).
Release build, fixed step, `auto_no_vsync`, idle machine.

Donington, windowed, per frame: deform check 10 µs, own-car parts 17.5, opponent cars 9.6,
opponent dirty check 55.5, opponent parts 91.6, dents 25.8, camera 24.7, HUD text 23.4, window
title 6.2. Nothing there is worth a change (the title, suspected earlier, is 6 µs).

Demolition derby (12 cars, 1200 frames), where dents dominate:
- Opponent dents: about 0.92–0.99 ms of wall time per frame on the compute pool, writes about
  0.09 ms. The built-in counter: 256 updates, 60,792 parts projected, 59,296 meshes written.
- Own car: 59 updates of 52 meshes, about 4 ms each, serial on the main thread (about 0.2 ms
  per frame on average, but all of it in contact frames).
- Per part (CPU): lattice displacements 2.34 s, mesh clone 0.64 s, smooth normals 1.0 s for
  13.3 M vertices, about 300 ns per vertex in the game against 57 ns per point for the
  displacements alone in an uncontended micro-benchmark: the pool is shared with Bevy's own
  tasks.
- One task per part instead of one chunk per thread: about 10 % less wall time (parts differ
  widely in size, so the chunks were unbalanced).

Tried and dropped: sampling the current and rest lattices together (they share the layout, so
the cell of a point can be found once). Bit-identical, but 58.0 ns per point against 57.1: the
interpolation, not the lookup, is the cost.

Change (`main.rs` `drive_present`, `on_compute_pool`): the player's car projects its dents on the
compute pool like the opponents, one task per part; both use the same helper, which keeps the
part order, so the first refusal still refuses the field and the meshes have the bits of the
serial path. Meshes are moved into the asset instead of cloned. `GRID_DENT_FAST=0` now also makes
the own car serial. `drive_body_projection event=update` gains `ms=`, the wall time of the
update on the main thread.

Measured, same executable, derby, `GRID_DENT_FAST=0` against the default (two runs each, sum
over the 59 updates; the worst single update in brackets):

| own-car projection | serial | pool |
| --- | --- | --- |
| total ms | 250, 295 | 111, 114, 151 |
| per update ms | 4.2–5.0 (8.4, 16.8) | 1.9–2.6 (5.9–8.6) |

Opponent dents with one task per part: 1194–1492 ms against 1285–1541 ms for the HEAD build in
the same pairs (alternating, within noise of each other; mean `main_ms` 17.95 against 18.17,
also noise: the saving is about 0.1 ms per frame averaged, concentrated in contact frames).

The derby's final frame differs between runs of one executable (base and fix alike, three
different hashes in three base runs), so it cannot show identity here; the projection is the
same pure function on the same inputs in the same order.

Solver stages (perf_timing, mean per tick): VehicleStage 0.57 ms, CarPairs 0.14, BodyContacts
0.135, Commit 0.18. No single stage is large enough to repay a restructuring now.

Visibility: mesh assets change in 599 of 600 frames (Donington; 1199 of 1200 in the derby), so
Bevy's `calculate_bounds` never takes its unchanged fast path over 19,338 (9,484) `Mesh3d`
entities. Marking static meshes `NoAutoAabb` could skip that, but which meshes are truly static
needs an audit and matched images; deferred. F854 found the cause and removed it.

## Post-lift worst frame and the OSD camera's own schedule (F853)

### What the post-lift worst frame is

F849 reported 74-86 ms "after the cover" frames, 37 of 38 render-led. `frame_split` now prints
`rworst=`, the stages of each window's slowest render frame (see the field table above). In the
window that holds the cover lift, those frames carry a 20 ms `r_cleanup`: the
`pipeline_warmup` yield, which runs only while the black cover is up and a pipeline compiles
(F848), plus 12-34 ms of first-use `r_encode`. So they are drawn under the cover; they mixed into
the first post-lift window only because the one-second window straddled the lift.

`fps_counter` now closes its window at the lift (the first frame `Warmup::ready()` holds) and
prints that partial window with ` cover=tail`; every later window is play. Measured with
`local/ab/rw.py` (Donington, `auto_no_vsync`, the PC shared with other sessions' builds):

| series | runs | visible post-lift max per run, ms |
|---|---|---|
| before, native, lift window excluded | 8 | 27/30/37/23/36/25/33/24 |
| after (cover tail split off), native | 8 | 24/46/33/46/46/29/27/31 |
| before, 2560x1440 forced, build running beside | 8 | 17/48/50/510/35/30/43/199 |
| after, 2560x1440 forced, build running beside | 8 | 27/39/41/38/36/43/22/66 |

The covered tail windows themselves hold the 48-87 ms frames. No visible window reached 50 ms
at native resolution; the slow visible frames' top stages are
`present` or `encode` and vary by run, so there is no first-sight work left to move into loading.
With a build running beside it, the two large visible stalls were outside our work: a
469 ms swapchain `acquire` and a 187 ms `submit`; the 66 ms window was `meshes`/`bind`/`encode`
in an Enhanced run while the other 7 runs stayed under 45 ms.

### OSD camera schedule

The HUD view (F850: up to 1.4-2.4 ms of render thread) ran Bevy's whole `Core3d` schedule. It
now runs `osd_scene::graph::OsdCore3d`: the view's GPU mesh preprocessing, one pass that clears
the target and draws the transparent phase, and the upscaling blit into the OSD image. The
camera draws directly (`NoIndirectDrawing`). Dropped: the prepass, deferred, opaque and
alpha-mask passes (empty), the indirect-parameter clear, unpack and build nodes, transmission,
tonemapping and the other post-processing nodes, and the `frame_split` GPU markers for this view.
The pass warns once if the OSD ever gets opaque or alpha-mask items, which it would not draw.

Image: fixed-step captures (1280x720, autopilot), before (b) against after (a), and a repeat of
after (a2) for noise:

| case | after vs before | noise |
|---|---|---|
| Donington, baseline and Enhanced, ticks 600 and 1500 | 0 px (4 cases) | 0 px, except 3 px (delta 1) in baseline tick 1500 |
| Spa, baseline and Enhanced, ticks 600 and 1500 | 0 px (4 cases) | 0-2 px |
| Haruna touge, baseline and Enhanced, tick 900 | 0 px (2 cases) | 0 px |

Drift battle captures are not repeatable (two identical runs differ in 99 % of the pixels), so
they prove nothing either way; the drift OSD uses the same camera and material. The menus draw
through `bevy_ui` on the viewer or display camera, whose schedule is unchanged; the OSD camera
exists only in a race and renders only into its own image, which the crops above compare.

Cost, Donington 2560x1440, both orders:

| measurement | before | after |
|---|---|---|
| command buffers per submit (profile build) | 34 | 24 |
| profile build: render ms / `r_encode` / fps (3 runs each, interleaved) | 12.31 / 6.11 / 77.6 | 11.77 / 5.52 / 81.8 |
| release, baseline, A/B 1: render / `r_encode` / fps | 11.19 / 5.53 / 85.7 | 11.02 / 5.24 / 90.4 |
| release, Enhanced, A/B 1 | 13.83 / 7.15 / 70.3 | 12.99 / 6.61 / 76.0 |
| release, baseline, A/B 2 (build running beside) | 12.08 / 5.78 / 80.0 | 12.07 / 5.56 / 79.2 |
| release, Enhanced, A/B 2 (build running beside) | 15.97 / 7.77 / 61.8 | 16.48 / 7.68 / 61.0 |

`r_encode` fell by 0.1-0.6 ms in every comparison; total render time fell by 0-0.8 ms, inside
this PC's noise (F850: 1 ms, 5 fps between identical runs). The schedule removes about a quarter
of the HUD view's cost. The rest is per view and stays with any separate camera: the 82 blended
draws' encode (about 0.34 ms), view preparation, specialization, batching and bind groups, and
main-thread visibility. A 2D camera would still pay those. Cutting them needs a change to the
picture or its timing (HUD drawn in the main view, or redrawn less often), which is the desktop
look owner's decision.

## Mesh changes and visibility each frame (F854)

Question: what do Bevy's `calculate_bounds` and the visibility systems cost per frame, and why
do meshes count as changed in every frame (F852)? Windowed 1280x800, native render height,
`auto_no_vsync`, idle PC, autopilot; Donington (`settings/race.ini`, 19,275 `Mesh3d`) and the
derby (`settings/demolition-derby.ini`, 9,484). A temporary probe (not committed) timed the
PostUpdate systems and named the mesh assets that sent `AssetEvent::Modified` each frame.

Cost before (mean per frame, two runs each):

| system | Donington | derby |
|---|---|---|
| `mark_3d_meshes_as_changed_if_their_assets_changed` | 0.26 ms | 0.13-0.15 ms |
| `calculate_bounds` | 0.12-0.13 ms | 0.10-0.11 ms |
| `check_visibility` (CPU) | 0.23-0.25 ms | 0.18-0.19 ms |
| directional light visibility | 0.26-0.27 ms | 0.18-0.20 ms |
| PostUpdate wall | 4.09-4.12 ms | 3.42-3.72 ms |

This is over the 0.3 ms threshold. A `Modified` event on any one mesh in a frame makes Bevy
revisit every `Mesh3d` entity: the mark system and `calculate_bounds` (`AssetChanged<Mesh3d>`)
scan all of them, and so do the per-material specialization checks. Every frame was dirty
(900 of 900 Donington, 2100 of 2100 derby). Sources, by mesh label:

- the minimap ribbon: replaced its asset (`*mesh = ...`) every frame;
- the crowd camera flashes: a new view-facing mesh per active card per frame;
- the crowd cells: `get_mut` when the set of omitted card rows changed, because the index
  count followed the visible card count;
- opponent dents and tyre smoke: real geometry changes (kept).

Freezing only the ribbon (probe switch) left 722 of 1200 Donington frames dirty
(PostUpdate 3.77 ms dirty against 3.42 ms clean), mostly flashes (578) and cells (213).

Fix, in the shared streaming path (`mesh_stream.rs`), with no list of tracks:

- `SoupMesh`, for a mesh rebuilt every frame: it de-indexes the triangles
  (`duplicate_vertices`), pads them to a capacity with zero-area triangles and writes the
  vertices through `MeshStream` over the GPU allocation, so the asset never changes. It
  replaces the asset only when the capacity grows (+25 %) or the attributes change. It queues
  the sort centre Bevy took from the replaced asset (`compute_aabb` of the unpadded mesh). The
  entity's bounds do not follow, so the users carry `NoFrustumCulling` (the minimap ribbon
  and the flash cards already did).
- The minimap ribbon and the crowd flash cards use it.
- Crowd cells pad their batch to the cell's full card count with zero quads, so the index
  buffer never changes and the vertices stream; they carry `NoAutoAabb` with their spawn
  bounds, which hold for every card direction.

After (fix build with the probe):

- Dirty frames: Donington 64 of 1800 and 63 of 1200 (opponent dents and tyre smoke); derby
  289 of 1200 and 302 of 1800 (opponent dents).
- Mark system 9-19 us Donington, 28-34 us derby; `calculate_bounds` 64-85 us and 80-82 us.

Alternating A/B, 3 pairs, same probe in both builds:

| pair | Donington PostUpdate before / after | derby PostUpdate before / after | Donington main_ms | derby main_ms |
|---|---|---|---|---|
| 1 | 4.57 / 3.39 ms | 3.21 / 2.63 ms | 9.33 / 6.86 | 6.73 / 5.80 |
| 2 | 3.58 / 3.12 ms | 2.63 / 2.53 ms | 6.73 / 6.14 | 5.23 / 5.21 |
| 3 | 3.39 / 3.16 ms | 2.53 / 2.43 ms | 6.36 / 6.42 | 5.14 / 4.95 |

PostUpdate was lower in every pair: Donington by 0.23-1.18 ms (median 0.46), derby by
0.10-0.58 ms (median 0.10). main_ms moves with it in five of six pairs; pair 1 includes
warm-up noise. `check_visibility` and light visibility still run every frame; they do not
depend on mesh changes.

Images: matched offscreen frames (1280x720, `--render-fixed-step`, autopilot) of the race and
the derby at ticks 600 and 1500, HEAD against the fix, with a second fix run as noise. Three
pairs are identical; derby 1500 differs in 1 pixel by 1 level, as does its noise pair. A
bottom-left crop shows the minimap ribbon drawn the same in both (local/mt/vis/img).

## HUD as an overlay pass; no OSD camera (F856)

F853 cut the OSD camera's schedule to the passes it used, which left the per-view cost of a
separate camera: view preparation, specialization, batching, bind groups, main-thread visibility
and the encode of about 80 blended draws. The coordinator chose to draw the HUD as an overlay
in the main frame instead of with a second camera, keeping the image unchanged.

Design (`osd_scene::overlay`):

- No OSD camera. The scene camera's view and projection live in the `Osd` resource
  (`Osd::view()`, `Osd::projection()`); the name tags and the minimap read them from there.
- The OSD's mesh nodes carry `OsdDraw` (mesh, material, optional sort centre) instead of
  `Mesh3d`/`MeshMaterial3d`/`RenderLayers`, so Bevy's visibility, extraction, specialization
  and batching never see them.
- One render-graph system (`RenderGraph` schedule, `RenderGraphSystems::Render`, before
  `camera_driver`) clears the transparent OSD image and draws the visible nodes with the
  `osd.wgsl` pipeline: 4x MSAA resolved into the sRGB image (1 sample on Android), alpha
  blending, no depth. It is one render pass with its own small bind groups prepared in
  `RenderSystems::PrepareBindGroups`.
- Draw order copies the former camera's: Bevy 0.19's retained `Transparent3d` order (items that
  left are swap-removed in Bevy's `Entity` order, new items are appended in that order, then a stable
  sort by view-space depth of the bounds centre). Ties stay in the same order as before (test
  `retained_order_keeps_ties_and_swap_removes`). Streamed meshes sort by their unpadded centre,
  as `mesh_stream` does for Bevy's phase: the texts (F398) and the minimap ribbon (F854).
- The viewer camera's UI composites the image after tonemapping, post-processing and
  upscaling, under every other UI node, as before. Bloom, anti-aliasing and grading never
  touch the HUD. The image stays as an intermediate target, because the UI tree draws over it
  (pause menu, post-race screens, touch zones).

Image: the F853 10 fixed-step cases (1280x720, autopilot), after against current main, with a
second after run as noise:

| case | after vs main | noise |
|---|---|---|
| Donington, baseline and Enhanced, ticks 600 and 1500 | 0 px (4 cases) | 0 px, except 3 px (delta 1) in baseline tick 1500 |
| Spa, baseline tick 600 | 1 px in the first run (delta 1, at (1177, 63)); 0 px in 3 repeats | 3 px, delta 1 |
| Spa, baseline tick 1500 and Enhanced ticks 600 and 1500 | 0 px (3 cases) | 0 px |
| Haruna touge, baseline and Enhanced, tick 900 | 0 px (2 cases) | 0 px |

The one Spa pixel is run-to-run noise. Three more runs of each build matched the first main run exactly, and the first
overlay run differs from them by that pixel alone. Two runs of the same build also differed by delta-1 pixels in two cases
(Donington and Spa, baseline). Crops: `local/ab/crops-ov2/base-spa-600-*`, repeats in `local/ab/crops-ov2/spa600/`.

Cost: Donington, `present_mode = auto_no_vsync`, both orders x2 runs per cell, play windows
after the cover lift, against the F853 release (both builds without F854):

| resolution, mode | build | fps | 1 % low | worst window | main ms | render ms | `r_encode` | GPU ms |
|---|---|---|---|---|---|---|---|---|
| 2560x1440 baseline | F853 | 92.9 | 67.8 | 72.1 | 5.87 | 10.24 | 4.59 | 4.63 |
| 2560x1440 baseline | overlay | 90.0 | 56.3 | 60.3 | 6.33 | 10.64 | 4.78 | 4.94 |
| 2560x1440 Enhanced | F853 | 76.7 | 51.9 | 56.9 | 6.32 | 12.57 | 5.82 | 4.26 |
| 2560x1440 Enhanced | overlay | 78.2 | 59.7 | 63.5 | 6.35 | 12.33 | 5.64 | 4.41 |
| 1280x800 baseline | F853 | 89.7 | 44.4 | 55.8 | 6.49 | 10.87 | 5.13 | 2.58 |
| 1280x800 baseline | overlay | 95.7 | 58.0 | 64.1 | 6.10 | 10.11 | 4.65 | 2.30 |
| 1280x800 Enhanced | F853 | 69.8 | 44.4 | 43.9 | 7.76 | 14.10 | 6.97 | 2.19 |
| 1280x800 Enhanced | overlay | 72.5 | 44.4 | 47.8 | 7.42 | 13.41 | 6.70 | 2.08 |

Single runs spread widely (2560x1440 baseline, overlay: 80.7 to 102.1 fps), and the frames-over-16-ms share
fell in three cells (4.1 to 1.3 %, 4.3 to 1.4 %, 19.7 to 12.6 %) and rose in one (0.3 to 1.7 %).
Render time fell by 0.2-0.7 ms in three cells and rose by 0.4 ms in 2560x1440 baseline. That is inside this PC's noise
(F850: about 1 ms, 5 fps between identical runs), so the gain is not proven by these runs; what
is certain is the removed work (a whole camera's view, specialization and batching).

### Post-lift windows with a slow frame, by stage

`frame_split` prints the stages of each window's slowest render frame (`rworst=`), so this
counts windows, not frames. Each window is attributed to the stage furthest above its window
mean (`local/ab/spikestage.py`). F853 runs, 8 per mode:

| set | mode | windows | encode | meshes | present/acquire | prepare/bind | submit |
|---|---|---|---|---|---|---|---|
| native 1280x800, slowest frame >= 20 ms | baseline | 41 | 61 % | 20 % | 12 % | 7 % | - |
| native 1280x800, slowest frame >= 20 ms | Enhanced | 60 | 87 % | 7 % | - | 7 % | - |
| 2560x1440, slowest frame >= 30 ms | baseline | 9 | 44 % | - | 33 % | 22 % | - |
| 2560x1440, slowest frame >= 30 ms | Enhanced | 20 | 60 % | - | 10 % | 20 % | 5 % |

The slow windows spread over the whole run (windows 0-17), not over one track section. Encode
dominates in both modes, so frame pacing settings (present mode, frame latency) can at most
address the present/acquire minority; the next step is to find which pass or phase grows in
those frames.

## Slow render frames by span (F857)

F856 bucketed the post-lift frames of 20 ms or more by their dominant `rworst` stage, and encode came first
(61 % baseline, 87 % Enhanced at 1280x800). To find which pass grows, `span_profile` (feature `profile`) now
logs every render-app update of `GRID_PROFILE_SLOW_MS` (default 20) or more. It logs that update's spans by
exclusive time, from every thread while the update ran, beside the window's mean per render frame:

```text
profile_slow render_ms=21.80 rank=1 excl_ms=8.8 mean_ms=5.9 name=schedule: name=Render
profile_render_mean frames=58 rank=1 excl_ms=5.9 name=schedule: name=Render
```

Main-world spans that overlap the render update (pipelined rendering) are included and recognised by name.

Donington, native 1280x800, `auto_no_vsync`, 3000 ticks, 1 s windows, 2 runs per mode, windows after the
cover lift:

| mode | windows | render updates of 20 ms or more | what grew |
|---|---|---|---|
| baseline | 96 | 1 (51 ms) | `present_frames` 37.7 ms (mean 0.9) |
| Enhanced | 94 | 11 (20.1-22.6 ms) | Render schedule wait outside any span +1.7 to +11.6 ms (8 of 11); main-world peaks in the same instant (7 of 11): `drive_tick` +2.2 to +4.2 ms, `drive_present` +2.7 to +2.9, `ground_detail::present` +2.8, once `propagate_parent_transforms` +12.4 (mean 0.2); present +7.9 once; render-graph passes at most +1.4 ms |

In the F856 release runs at 1280x800, the windows with a slow render frame are busier on the main thread for
the whole second:

| mode | windows | main ms | fixed ms | render mean ms |
|---|---|---|---|---|
| baseline, slow render frame | 6 | 8.1 | 2.4 | 13.3 |
| baseline, other | 73 | 5.5 | 1.2 | 9.5 |
| Enhanced, slow render frame | 25 | 8.0 | 2.3 | 14.8 |
| Enhanced, other | 50 | 6.4 | 1.6 | 12.2 |

Verdict: no render pass or phase grows. The render thread waits for its tasks while the main world's work
(fixed-step driving, presentation, transform propagation) peaks in the same instant. `encode` is wall-clock
time over render-graph systems that run on Bevy's task pools, shared with the main world, so contention for
those threads shows up there. This is an interpretation from 11 profiled updates and the window correlation.
The profile build makes render spikes rarer (11 per 94 windows against 25 per 75 in the release Enhanced runs),
and its own report adds main-thread frame spikes. Baseline's rare render spikes include the driver's present.
Levers: smaller main-world peaks, or a task pool split so that render-graph tasks do not queue behind
main-world `par_iter` work (an A/B, not yet run).

## Per-frame mesh uploads during a race (F859)

Question (mac-perf item 7): which senders still upload mesh or buffer data during a race, how
many bytes per frame, and what render-thread prepare cost (`r_meshes`, `r_prepare`) they cause.
Windowed 1280x800, native render height, `auto_no_vsync`, idle PC, autopilot; Donington
(`settings/race.ini`) and the derby (`settings/demolition-derby.ini`). The upload probe is
kept in `frame_split.rs`: `main_end` hands the frame's prepared meshes, their bytes and the
streamed vertex bytes to the render frame that draws it, which splits `r_meshes`/`r_prepare`
between frames with and without prepared meshes and reports `up_max_kb` and `stream_max_kb`;
`mesh_upload_group` and `mesh_stream_group` lines name the senders by mesh label.

Senders before (probe build, per second of race):

| sender | path | Donington | derby |
|---|---|---|---|
| opponent dents (`opponent#_MPJRI`) | whole-mesh re-upload | 4 uploads, few KB | 78 uploads, 0.94 MB |
| car batches (`car_batch/#/`, batch rebuilt when a member left) | whole-mesh re-upload | 0.2, 0.09 MB | 12, 2.0 MB |
| largest single frame | | 500 KB | 1.3 MB |
| crowd cells | stream (F854) | 9.1 MB | 6.9 MB |
| tyre smoke (`Tyre_smoke_*`) | stream | 1.4 MB | 7.5-23 MB (varies with contact) |
| skid marks | stream | 0.02 MB | 3.6 MB |
| ground cover | stream | 1.3 MB | - |
| car batch member moves | stream | 0.05 MB | 6.4 MB |

The streamed senders write only changed vertex ranges through `MeshStream` (one
`write_buffer` per run into the existing allocation) and cost nothing measurable in
`r_meshes`; they stay as they are. The whole-mesh re-uploads did not: a frame that prepared
meshes cost about 0.3 ms more `r_meshes` than one that did not, and in the derby these landed
on single frames of up to 1.3 MB.

Fix, through shared paths (no track or car lists):

- `mesh_stream::replace(meshes, handle, new, stream, centre)`: when the new mesh has the
  drawn mesh's structure (usage, attributes, vertex count, indices) and that mesh is
  uploaded, it compares runs of 64 vertices and writes only the runs that differ over the
  GPU buffer, then updates the asset with `get_mut_untracked` (no `Modified` event, so no
  re-extraction, no repack and no F854 rescan). Otherwise it replaces the asset as before.
  Either way it queues the sort centre of the new bounds, so a later stream never sorts by an
  older one.
- Dents (`write_dent`, the player's car and the opponents alike) use it; a batched part keeps
  the existing path (main-world copy, its event dirties the batch, which streams).
- `car_batch` rebuilds keep every member's vertex range and collapse members that left the
  batch to zero-area triangles (a chunk with none left is hidden), so a member leaving no
  longer changes the vertex count and the rebuild streams through the shared
  `mesh_stream::stream_changed` instead of re-uploading the batch.
- `car_reflection_overlay`: `follow_bounds` copies a dented part's `Aabb` to its overlay
  (no change event refits it any more), and `follow` now also handles a removed
  `RenderLayers`. Before, an overlay left the hidden car-batch layer only when its part's
  mesh or material changed; a part leaving its batch (loose debris) removes the part's
  layers, which is not a change, so the overlay stayed hidden until the next whole-mesh dent
  write re-synced it by accident. Streaming removed that accident and showed the bug: derby
  tick 1500 differed in 240 px (max 72) on loose debris. Diagnosis by marking `Mesh3d`
  changed on chosen drawers: all drawers 2 px, auto-bounds drawers only 239 px, `NoAutoAabb`
  drawers only 1 px; refitting or touching `Aabb` on all drawers 239-240 px; streaming the
  whole mesh 239 px; full writes with the sort centre 0 px. Test
  `an_overlay_leaves_the_hidden_layer_with_its_part`.

After (fix build with the probe): whole-mesh uploads Donington 6.1 per s (18.8 KB/s, largest
frame 37 KB), derby 11.9 per s (160 KB/s, largest frame 142 KB). What remains are
opponent meshes whose structure changed (2-4 per s, the fallback of `replace`) and smoke
buffers growing.

Alternating A/B, 2 pairs, same probe in both builds (`frame_stats`, first 3 windows skipped):

| run | scene | main_ms | render_ms | r_meshes | r_prepare | uploads/s | upload KB/s | largest frame KB |
|---|---|---|---|---|---|---|---|---|
| before 1 | Donington | 5.79 | 10.06 | 1.15 | 1.62 | 27.9 | 51.9 | 500 |
| after 1 | Donington | 5.41 | 9.24 | 1.06 | 1.56 | 6.1 | 18.8 | 37 |
| before 1 | derby | 6.57 | 8.62 | 1.11 | 1.39 | 264.7 | 4480 | 1300 |
| after 1 | derby | 6.05 | 8.04 | 0.97 | 1.31 | 11.9 | 160.5 | 142 |
| before 2 | Donington | 5.57 | 8.89 | 1.05 | 1.46 | 24.1 | - | 500 |
| after 2 | Donington | 4.59 | 8.39 | 0.91 | 1.34 | 5.8 | - | 37 |
| before 2 | derby | 6.01 | 7.91 | 1.02 | 1.29 | 266.4 | 4586 | 1300 |
| after 2 | derby | 4.37 | 6.37 | 0.69 | 1.04 | 11.9 | 160.4 | 142 |

Streamed vertex bytes are unchanged in kind: 12-15 MB/s on Donington and 44-55 MB/s in the
derby, largest frame 1.5-4 MB, all as ranged writes into existing buffers.

Warm-up: the first run of a new executable path spends 180-200 frames compiling pipelines
(the driver's shader cache is keyed per path); later runs 53-79. A/B runs therefore copy each
executable once and discard the first run.

Images: offscreen frames (1280x720, `--render-fixed-step`) of the race and the derby at
ticks 600 and 1500, the pre-change build against the fix: race 600 identical, race 1500 and
derby 600 2 px by 1 level, derby 1500 293 px by up to 30 on one damaged opponent at the wall
(local/mt/up/img/cmp-bf4-at0.png). A control build of HEAD with only the overlay change
gives the fix's image there (1 px by 1 level), and both builds repeat themselves (0 and
2 px): the difference is the overlay change drawing that car's reflection on parts that had
left their batch, which HEAD left on the hidden layer. The streaming changes alone match the
pre-change frames except where they removed the accidental re-sync (the 240 px on debris
above, gone with the overlay change).

## Compute pool size and slow render frames (F860)

Task (coordinator, after F857): test whether the render executor waits for compute-pool threads busy
with main-world work. Release build local/ab/tp.exe caca6a1e9fe18505 (main after F857, plus the switches
below), Donington native, auto_no_vsync, 3000 ticks, 1 s `frame_split` windows after the cover lift,
baseline (b) and Enhanced (e), both variant orders × 2. New window field `rslow20`: render frames of
20 ms or more. Drivers: local/ab/tp.ps1, tq.ps1 and tprerun.ps1; summaries by local/ab/tp.py (per variant)
and tppair.py (paired against variant a in the same order block).

Variants, one switch each:

- a: current main (Bevy's split; 12 compute threads on this 20-thread PC);
- b: `GRID_DENT_POOL=async` (our `on_compute_pool` jobs on the async compute pool; switch removed again);
- c4, c6, c8, c10, c18: `GRID_COMPUTE_THREADS=N`;
- d: `GRID_DENT_FAST=0`.

The PC: i7-12700, 8 P-cores with HT plus 4 E-cores.

### Clean runs

These ran under the build lock or in windows with no other timing or build. Each delta is the mean of
4 pairs, as fps against a, with the sign of every pair.

| res | mode | c4 | c6 | c8 | c10 | c18 | b | d |
|---|---|---|---|---|---|---|---|---|
| 1280x800 | b | +2.1 (3 of 4 +) | +4.5 (4 +) | +2.8 (4 +) | +1.3 (4 +) | | | |
| 1280x800 | e | +1.5 (4 +) | +2.0 (4 +) | +1.4 (4 +) | +0.4 (3 of 3 ≥ 0) | | | |
| 2560x1440 | b | | | +1.8 (4 +) | | −3.1 (4 −) | −0.3 | −1.1 |
| 2560x1440 | e | | | +2.0 (4 +) | | −1.0 (3 −) | +0.2 | −0.1 |

- Absolute fps for a: 116.1 baseline and 91.4 Enhanced at 1280; 114.2 and 87.7 at 2560.
- With c8, main and render time both fall, by 0.26 to 0.32 ms and 0.12 to 0.24 ms per window.
- Slow render frames are rare in clean runs for every variant: 0 to 0.022 per window. One exception:
  18 threads raise them at 2560 Enhanced, 0.097 per window against 0.011.
- 1 % low (histogram buckets) is equal or better with c6 and c8.
- One run (1280 r1, c10, Enhanced) ended in a wgpu `DeviceLost` at 04:05. It is the only one in 488
  `frame-split-smoke` runs and is left out of the table.

### Runs under background load

The 1280 tp blocks ran from 23:59 to 02:10. They took 25 to 50 minutes per block of 10 runs, against
about 10 minutes for a clean block, and variant a's fps was 15 % lower (98.5 and 76.0), so other
sessions' builds ran beside them. There, c8 gave:

- baseline: +7.2 fps and fewer slow render frames, 0.031 per window against 0.164;
- Enhanced: +6.2 fps, with slow render frames unchanged (0.63 and 0.61 per window).

The F856 release runs had 25 slow render frames in 75 Enhanced windows, so they may also have run
under such load. Contaminated blocks of the other A/Bs (02:15–02:23 and 02:52–03:10) were rerun under
the build lock; the tables above use the reruns.

### Decision

`mobile::MAX_COMPUTE_THREADS = 8` caps the compute pool on every platform, for Bevy's split and for the
preset split alike (`mobile::pool_threads`).

- On this PC the pool drops from 12 threads to 8.
- Machines with 16 or fewer logical cores keep their count: phones under the preset (8 cores give 6) and
  Macs up to M4 Max. Larger machines also get 8.
- c6 was a little faster at 1280 but was not measured at 2560, so the cap is the count measured at both
  resolutions.
- Bevy's IO and async pools keep their own counts.
- `parallel::map` is index-ordered, so the simulation is unchanged.
- Image check: 10 matched fixed-step cases, 8 threads against the old default (local/ab/crops-tp). 9 are identical; one has a single pixel off by 1 of 255.

The F857 hypothesis is only partly confirmed:

- Fewer compute threads cut main and render time.
- In clean runs slow render frames are too rare to show a cut. Under background load the cut showed in
  baseline only.
- Moving our own jobs to the async pool (b) and the dent fast path (d) are within noise.

Follow-up, c6 against c8 on F861 (acc2ba15), 4 alternating pairs per cell under the build lock. Paired fps, c6
minus c8, was +4.6, +0.4, +0.5 and +0.5 for 1280 baseline and Enhanced and 2560 baseline and Enhanced. Each
mean rests on one outlier pair, and the other pairs change sign. The 1 % low is worse with c6 at 1280 Enhanced
(72.7 against 75.5). The cap stays at 8.

## Main-world peaks (F861)

F857 found main-world peaks in the same instant as slow render frames: `drive_tick` +2-4 ms,
`drive_present` about +2.8 ms, `ground_detail::present` about +2.8 ms and once
`propagate_parent_transforms` +12.4 ms. To find what makes them, `frame_split.rs` now times
named main-world work per main frame (laps; `Lap::new(lap::X)` charges the time until it drops)
and reports them per window in `frame_stats`:

- `lap_<name>=mean/max/over2ms`: mean ms per main frame, the largest, frames over 2 ms;
- `rslow_laps=n:...`: the largest laps of the main frames beside the window's render frames of
  20 ms or more (the main frames after the one drawn, up to the running one);
- `transform_burst`: with a transform propagation of 3 ms or more, the changed `Transform` and
  `GlobalTransform` counts by hierarchy root.

Laps: `dt` drive_tick, `dp` drive_present, `dpo`/`dpn` own and opponent dents inside it, `gd`
`ground_detail::present`, `gdg` its gathers, `tp` transform propagation, `cb` visibility bounds,
`out` stdout lines from the drive paths, `mt` the opponents' `multi.tick` inside drive_tick.

### What makes each peak

| peak | cause | change |
|---|---|---|
| drive_tick | (1) about five stdout lines per simulation tick (`drive_sample` and its siblings), each one synchronous file write on the main thread, because the launchers and the smoke tools redirect stdout to a file: 0.3-0.8 ms per frame on average, single writes up to 7 ms. The 42-110 ms single stalls of the first probe batch were inside drive_tick at those writes (interpretation; they did not recur, and that batch may have overlapped other timing runs). (2) The opponents' simulation (`multi.tick`, already scope-parallel): 0.6-0.8 ms mean in the derby on an idle PC, up to 4 ms. | (1) fixed; (2) real simulation work, unchanged (identity) |
| drive_present | contact dents (`dpn`/`dpo`, 3-4 ms in single frames): the deformation solve runs only when a contact changes a part and is already on the compute pool | F864 (section below): the projection keeps each part's fixed lattice data, up to 2.5 ms |
| ground_detail::present | Enhanced only: blended (soft alpha) cover gathered again at once on the main thread on every 16 m bucket crossing (`gdg` up to 6 ms), because F580 kept its draw order by bucket | fixed |
| propagate_parent_transforms | 1,400-2,350 changed `GlobalTransform`s per frame, almost all from real motion (cars, wheels, driver joints, camera). The peaks of 15-49 ms (both builds, all scenes, beside most remaining slow render frames) changed 1,440-2,245, the same as ordinary frames, and `calculate_bounds` stretches with them: the PostUpdate transform/bounds work waits for task-pool threads, not a hierarchy dirtied in full (interpretation; the task-pool split is mac-perf's A/B) | no fix; the interior driver poses its 76 joints while hidden, a small extra left in place (the view switch reads `shown` after the pose) |

### Fixes

- `log_out.rs`: `println!`, `log_line!` and `eprintln!` in grid-app append to one queue in call
  order; a background thread named `stdout` writes it in one write per wake-up. `eprintln!`
  writes the queue first, so a merged log keeps its order; `log_out::exit` (every
  `process::exit` in grid-app), the end of `main` and the panic hook write the rest; past 4 MiB
  queued the caller writes it itself. Lines from other crates and Bevy's log are written directly
  and may land a few ms earlier relative to queued lines. Tests:
  `every_line_reaches_the_sink_in_order`, `a_full_queue_is_written_by_the_caller`.
- `ground_detail.rs`: blended cover takes the async gather path of masked cover. The gather walks
  the buckets in one global order (x, then z) whatever the eye, `Slots::update_ordered` keeps it,
  and the shader collapses billboards beyond their distance, so a gather made up to `MARGIN`
  (6 m) away draws the same quads in the same order (and gives the same sort centre) as one made
  at once in the new bucket. Test `an_older_gather_draws_what_a_new_one_draws_in_the_same_order`
  (400 eyes, 3 steps each, over 100 bucket crossings).

### Before and after

Donington (`settings/race.ini`) and the derby (`settings/demolition-derby.ini`), windowed 1280x800,
native render height, `auto_no_vsync`, 3000 ticks, baseline (b) and Enhanced (e); the probe build
(`p`, the lap probe on F859) and the fix build (`f`, the same probe) alternate per run, 2 runs each,
under the build lock with no other game or build running (`local/ab/run.ps1`, `mw.py ab3`). Windows
after the cover lift; spike rate is render frames of 20 ms or more per window; 1 % low from the
0.5 ms frame-time histogram.

| scene | mode | build | windows | spikes per window | fps | 1 % low | worst frames ms |
|---|---|---|---|---|---|---|---|
| Donington | b | before | 95 | 0.032 | 108.9 | 72.7 | 32/28/26/20/18 |
| Donington | b | after | 95 | 0.010 | 109.4 | 75.5 | 26/17/17/17/17 |
| Donington | e | before | 93 | 0.032 | 94.3 | 78.4 | 38/33/21/16/15 |
| Donington | e | after | 93 | 0.043 | 95.2 | 81.6 | 41/41/39/36/32 |
| derby | b | before | 96 | 0.042 | 144.1 | 93.0 | 35/32/29/21/16 |
| derby | b | after | 96 | 0.031 | 145.6 | 102.6 | 32/26/22/15/15 |
| derby | e | before | 94 | 0.043 | 108.7 | 78.4 | 30/26/23/21/18 |
| derby | e | after | 94 | 0.011 | 108.8 | 78.4 | 27/23/19/17/15 |

Main-world laps, mean ms per frame / largest / main frames over 2 ms (same runs):

| scene | mode | build | drive_tick | stdout | ground gather | transform propagation |
|---|---|---|---|---|---|---|
| Donington | b | before | 1.04/5.6/1832 | 0.42/4.4/10 | 0.02/0.6/0 | 1.11/3.3/128 |
| Donington | b | after | 0.61/3.0/29 | 0.01/0.2/0 | 0.02/0.5/0 | 1.15/15.2/145 |
| Donington | e | before | 1.09/5.8/319 | 0.45/4.3/10 | 0.11/4.0/27 | 1.08/27.9/35 |
| Donington | e | after | 0.63/2.9/7 | 0.02/0.1/0 | 0.09/2.2/2 | 1.11/30.5/49 |
| derby | b | before | 0.89/6.4/3590 | 0.26/4.8/13 | - | 0.94/27.0/310 |
| derby | b | after | 0.64/4.2/1173 | 0.01/0.1/0 | - | 0.94/3.8/297 |
| derby | e | before | 1.20/6.1/3506 | 0.34/4.8/10 | - | 1.02/20.1/323 |
| derby | e | after | 0.88/4.0/1113 | 0.01/0.1/0 | - | 1.02/17.7/318 |

Over all eight pairs the spikes went from 14 to 9 in 378 windows each; single counts per row are
noise (one spike more or less). The worst frames of the Enhanced Donington fix runs and most of the
remaining spikes are the transform/bounds waits above, which both builds have (beside the four
spikes of that row: `tp` 20.2 ms, `cb` 28.9 ms). The derby's remaining drive_tick frames over 2 ms
are the opponents' `multi.tick` (0.6-0.8 ms mean, up to 4 ms). Contact dents (`dpn`, up to 3-4 ms)
are unchanged here (F864, section below).

Images: offscreen frames (1280x720, `--render-fixed-step`), the probe build against the fix, race and
derby, baseline and Enhanced, ticks 600 and 1500: Enhanced (blended cover) 0 px in all four; baseline
race 600 1 px, race 1500 3 px, derby 1500 2 px, all by 1 level; derby 600 0 px.

Measurement notes: two earlier batches overlapped another session's timing runs (its executable has
another name, so the wait for other game processes missed it) and are discarded; timing batches now
run under the build lock. In those batches the probe build also showed single render stalls of 85 and
499 ms with no main-world lap peak.

These batches ran before the compute pool cap of F860 (section above), which addresses the task-pool
contention behind the propagation and bounds peaks.

## Status matrix against the 100 fps goal (F862)

The owner's goal is 100 fps or more in every mode up to 2560x1440. This matrix compares the start
of the series (A = 7dfed11e, before F837, `local/ab/mA.exe` 086e8281b9e45c8a) with the current code
(B = acc2ba15, identical code to origin/main e01185a3, `local/ab/mB.exe` 36a366fcd480361c).

Setup:

- tracks and resolutions: the derby (`settings/demolition-derby.ini`, 12 cars) and Donington
  (`settings/race.ini`), windowed 1280x800 and 2560x1440, native render height;
- modes: baseline and Enhanced, `auto_no_vsync`, 3000 ticks;
- runs: 2 alternating pairs per cell (pair 1 A then B, pair 2 B then A), 32 runs in all, under the
  build lock (`local/ab/mxall.ps1`, `mx.ps1`, `mx.py`);
- windows: the 1-s `frame_stats` windows after the loading cover, dropping the first one after the
  lift and those after the simulation stops;
- phases: lights out is the countdown's `remaining=GO` event (tick 304 in the derby, 240 at
  Donington). "Start" holds the windows ending in the 15 s after it (ticks GO to GO+900). Countdown
  windows are dropped. "Mid" holds the rest, about 30 s. At these resolutions the cover lifts at
  ticks 64 to 192, before GO, so the whole start is visible.

A lacks `ft_hist` and the render stage fields; F837 and later entries added them. A's 1 % low and
slow-frame rate are therefore n/a. The worst 1-s window and the worst frame (`max_ms`) compare
both builds. Main, render and GPU times are frame-weighted means.

| Cell | Phase | fps A → B | 1 % low B | worst window fps A → B | worst frame ms A → B | main ms A → B | render ms A → B | GPU ms A → B | ≥20 ms % B |
|---|---|---|---|---|---|---|---|---|---|
| derby 1280 baseline | start | 87.1 → 136.7 | 65.6 | 43.6 → 84.3 | 43.6 → 21.9 | 8.04 → 4.64 | 10.08 → 6.48 | 2.33 → 2.08 | 0.15 |
| derby 1280 baseline | mid | 88.5 → 122.1 | 67.8 | 50.7 → 82.4 | 36.9 → 23.6 | 6.89 → 4.38 | 10.49 → 7.64 | 2.55 → 2.29 | 0.01 |
| derby 1280 Enhanced | start | 74.1 → 103.5 | 63.5 | 54.5 → 76.8 | 27.8 → 19.5 | 8.45 → 5.41 | 12.44 → 9.02 | 2.16 → 2.06 | 0.00 |
| derby 1280 Enhanced | mid | 66.5 → **85.6** | 44.4 | 38.6 → 51.4 | 47.7 → 43.5 | 7.79 → 5.06 | 14.26 → 11.14 | 2.43 → 2.31 | 1.69 |
| derby 2560 baseline | start | 93.6 → 129.2 | 61.5 | 47.1 → 78.5 | 42.3 → 30.3 | 7.35 → 4.82 | 9.47 → 6.94 | 5.03 → 4.75 | 0.17 |
| derby 2560 baseline | mid | 93.9 → 117.4 | 63.5 | 54.6 → 69.3 | 65.8 → 50.1 | 6.45 → 4.54 | 9.87 → 7.96 | 5.82 → 5.59 | 0.14 |
| derby 2560 Enhanced | start | 81.8 → 113.6 | 75.5 | 54.9 → 82.4 | 44.3 → 17.1 | 7.15 → 4.54 | 11.35 → 8.28 | 4.67 → 4.53 | 0.00 |
| derby 2560 Enhanced | mid | 73.5 → **91.7** | 56.3 | 49.8 → 61.8 | 45.2 → 44.9 | 6.75 → 4.44 | 12.92 → 10.41 | 5.16 → 5.07 | 0.69 |
| Donington 1280 baseline | start | 66.7 → **99.8** | 61.5 | 59.0 → 81.4 | 58.0 → 34.3 | 8.18 → 4.95 | 13.98 → 9.44 | 2.10 → 2.30 | 0.17 |
| Donington 1280 baseline | mid | 70.1 → 112.6 | 70.2 | 55.0 → 74.1 | 45.9 → 27.3 | 7.49 → 4.29 | 13.34 → 8.37 | 2.36 → 2.40 | 0.10 |
| Donington 1280 Enhanced | start | 54.4 → **84.2** | 54.8 | 42.3 → 72.4 | 49.7 → 25.8 | 9.65 → 5.35 | 17.28 → 11.33 | 2.05 → 1.88 | 0.67 |
| Donington 1280 Enhanced | mid | 58.5 → **87.6** | 58.0 | 51.8 → 72.4 | 49.6 → 47.9 | 8.88 → 5.26 | 16.12 → 10.89 | 2.10 → 1.92 | 0.32 |
| Donington 2560 baseline | start | 71.2 → 110.1 | 88.9 | 63.8 → 99.0 | 45.4 → 13.3 | 7.35 → 4.23 | 13.11 → 8.58 | 4.13 → 4.10 | 0.00 |
| Donington 2560 baseline | mid | 68.5 → 111.8 | 93.0 | 60.2 → 95.9 | 32.7 → 17.3 | 7.60 → 4.13 | 13.64 → 8.46 | 4.23 → 4.23 | 0.00 |
| Donington 2560 Enhanced | start | 61.1 → **88.6** | 72.7 | 52.1 → 84.1 | 49.6 → 17.2 | 8.07 → 4.63 | 15.42 → 10.80 | 4.19 → 4.12 | 0.00 |
| Donington 2560 Enhanced | mid | 60.1 → **89.0** | 78.4 | 54.4 → 82.0 | 49.9 → 14.4 | 8.40 → 4.77 | 15.69 → 10.76 | 4.27 → 4.23 | 0.00 |

B's fps per run (start | mid):

| Cell | baseline | Enhanced |
|---|---|---|
| derby 1280 | 126.9/145.7 \| 116.0/128.4 | 91.7/108.6 \| 87.0/84.1 |
| derby 2560 | 136.8/121.5 \| 113.2/121.5 | 117.4/109.8 \| 92.6/90.8 |
| Donington 1280 | 95.9/104.0 \| 108.4/116.8 | 84.0/84.5 \| 83.5/91.7 |
| Donington 2560 | 107.0/113.2 \| 110.4/113.1 | 88.9/88.3 \| 89.5/88.4 |

Observations:

- A to B: fps rose by 25 to 63 % in every cell. Main time fell by 30 to 46 % and render time by 19 to
  38 %. The GPU time did not change (within 0.25 ms).
- The derby varies from run to run by up to 19 fps, because the crashes differ. Donington varies
  by up to 9 fps.
- Below 100 fps (bold): every Enhanced cell except the derby's start, and Donington 1280 baseline
  at the start (99.8).
- 1 % low below 80: 14 of 16 cells, all except Donington 2560 baseline (88.9 and 93.0). In the
  derby the 1 % low sits at 61 to 68 even at 117 to 137 fps. Its slowest 1 % of frames take 14.5 to
  16.5 ms. Frames of 20 ms or more stay at 0.17 % or fewer in baseline. In Enhanced they reach 0.67 %
  (Donington 1280 start), 0.69 % (derby 2560 mid) and 1.69 % (derby 1280 mid).
- Only Donington 2560 baseline meets both limits.
- At the same settings, Donington 1280 baseline renders slower than 2560 (render 9.44 against
  8.58 ms at the start). F860 saw the same direction, but it was not investigated here.

Bottleneck of the cells below a limit: render-CPU in every one. The render thread's busy time minus
surface acquire and present is the largest of the three times (5.7 to 10.8 ms). Main runs 4.3 to
5.4 ms and the GPU 1.9 to 5.6 ms.

- Render top stages: `r_encode` first everywhere (2.5 to 4.4 ms baseline, 3.3 to 5.7 ms Enhanced).
  Next come `r_prepare` (0.9 to 2.0 ms), then `r_meshes` (about 1 ms), `r_submit` (1.4 to 1.6 ms at
  2560 Enhanced) or `r_bind` (1.1 ms, derby 1280 Enhanced mid).
- GPU top passes: `g_opaque` (1.4 ms at 1280, 3.5 to 4.0 ms at 2560 in the derby), `g_pre` in
  Enhanced (0.9 to 1.1 ms at 1280, 1.5 to 1.8 ms at 2560), `g_post` at 2560 Enhanced (0.7 ms).
- Main laps (F861), from the top: `cb` visibility bounds 1.6 to 2.1 ms, `tp` transform propagation
  1.0 to 1.3 ms, `dt` drive_tick 0.6 to 1.3 ms, of which `mt`, the opponents' tick, takes 0.5 to
  1.2 ms.

So the remaining average gap is render-thread encoding and preparation, in Enhanced most of all
(about 4 to 5 ms of `r_encode` against a 10 ms budget). The 1 % low gap is a tail of frames of
14.5 to 16.5 ms, plus derby Enhanced frames of 20 ms or more. This matrix does not say which part of the
frame makes that tail.

### The 1 % low tail was background load (F863)

Task: find what the derby's slowest 1 % of frames (14.5 to 16.5 ms in the matrix above) wait on.

Instrumented runs: release exe `local/ab/tl.exe` 279d33041788cb8a (f06cdeb2 plus the tick split
and slow-frame laps), derby 1280 baseline x2 and Donington 1280 baseline, 3000 ticks, under the
build lock (`local/ab/tl.ps1`, `tl.py`). Means over the windows after GO:

| Run | fps | frames | share | frame ms | main | fixed | outside | render | top laps (ms) |
|---|---|---|---|---|---|---|---|---|---|
| derby 1 | 149.3 | no tick | 58 % | 5.39 | 2.72 | 0 | 2.68 | 6.21 | cb 1.36, tp 0.79 |
| | | tick | 42 % | 8.51 | 4.67 | 1.73 | 3.84 | 6.18 | dt 1.47 (mt 1.33), cb 1.50, tp 1.00 |
| | | over 10 ms | 4.6 % | 11.59 | 7.64 | 2.55 | 3.95 | 6.68 | cb 2.48, dt 2.28 (mt 2.11), tp 1.86, dp 1.32 (dpn 1.09) |
| | | over 14 ms | 0.2 % | 15.18 | 10.38 | 2.92 | 4.81 | 7.98 | cb 3.63, tp 2.91, dt 2.66, dp 2.41 (dpn 2.19) |
| Donington | 124.9 | no tick | 50 % | 7.07 | 3.21 | 0 | 3.85 | 7.68 | cb 1.70, tp 0.87 |
| | | tick | 50 % | 8.94 | 4.38 | 1.16 | 4.56 | 7.42 | cb 1.72, tp 1.06, dt 0.90 |
| | | over 14 ms | 0.1 % | 16.21 | 8.26 | 1.85 | 7.96 | 9.57 | cb 2.85, dp 2.05 (dpn 1.95), tp 1.80 |

Derby run 2 matches run 1 within 0.05 ms in the tick rows.

Observations:

- On this run the derby 1280 baseline 1 % low was about 103 fps (bucket 9.5 ms), not the matrix's
  65.6 and 67.8. Frames of 14 ms or more were 0.2 to 0.3 %.
- A re-run of the matrix's own B binary (`local/ab/mB.exe` 36a366fcd480361c), 3 pairs alternating
  with tl.exe (`local/ab/tr.ps1`): mB 1 % low about 103 fps in all 3 runs, tl 103 to 108. The
  instrumentation costs nothing visible.
- The slow frames are main-thread frames with a fixed tick. All main laps inflate together (cb, tp,
  dt, dp roughly double), which points to a slower CPU in those frames rather than one stage. The
  one stage that grows more than the rest is `dpn`, the opponents' dent update in drive_present
  (0.13 ms on average, 1.1 ms over 10 ms, 2.2 ms over 14 ms). The render thread stays at 6.7 to
  8.0 ms in those frames. No surface acquire, present, pipeline compile or periodic job shows in them.

Idle re-measure of the B matrix: the same binary mB.exe, 8 cells x 2 runs, under the build lock,
logging the CPU seconds of every other process (`local/ab/mi.ps1`). A run where a process other than
Explorer used more than 5 s of CPU was rerun once. `local/ab/mi.py` compares it with the matrix
(`local/ab/mi.out`):

| Cell | Phase | fps matrix → idle | 1 % low matrix → idle | ≥20 ms % matrix → idle |
|---|---|---|---|---|
| derby 1280 baseline | start | 136.7 → 153.9 | 65.6 → 93.0 | 0.15 → 0.00 |
| derby 1280 baseline | mid | 122.1 → 145.0 | 67.8 → 108.1 | 0.01 → 0.00 |
| derby 1280 Enhanced | start | 103.5 → 126.5 | 63.5 → 85.1 | 0.00 → 0.00 |
| derby 1280 Enhanced | mid | 85.6 → 101.7 | 44.4 → 70.2 | 1.69 → 0.02 |
| derby 2560 baseline | start | 129.2 → 135.5 | 61.5 → 67.8 | 0.17 → 0.43 |
| derby 2560 baseline | mid | 117.4 → 133.2 | 63.5 → 78.4 | 0.14 → 0.00 |
| derby 2560 Enhanced | start | 113.6 → 117.5 | 75.5 → 72.7 | 0.00 → 0.00 |
| derby 2560 Enhanced | mid | 91.7 → **97.0** | 56.3 → 70.2 | 0.69 → 0.02 |
| Donington 1280 baseline | start | 99.8 → 124.6 | 61.5 → 102.6 | 0.17 → 0.03 |
| Donington 1280 baseline | mid | 112.6 → 125.0 | 70.2 → 102.6 | 0.10 → 0.01 |
| Donington 1280 Enhanced | start | 84.2 → **98.3** | 54.8 → 85.1 | 0.67 → 0.00 |
| Donington 1280 Enhanced | mid | 87.6 → **98.5** | 58.0 → 85.1 | 0.32 → 0.00 |
| Donington 2560 baseline | start | 110.1 → 107.9 | 88.9 → 75.5 | 0.00 → 0.03 |
| Donington 2560 baseline | mid | 111.8 → 111.6 | 93.0 → 93.0 | 0.00 → 0.00 |
| Donington 2560 Enhanced | start | 88.6 → **88.0** | 72.7 → 72.7 | 0.00 → 0.00 |
| Donington 2560 Enhanced | mid | 89.0 → **87.2** | 78.4 → 67.8 | 0.00 → 0.04 |

- The matrix ran 10:57 to 11:29 under the build lock, but the lock covers only cargo and timing
  scripts. In one rejected idle run the ChatGPT and Codex desktop apps used 8 to 16 s of CPU, and in
  derby 2560 baseline run 1 Claude processes used about 10 s (that run gave 119.7 fps against 152.5
  and the 137.8 ms worst frame). The exact load during the matrix is unknown.
- Cells that ran fastest gained most: 1280 cells gained 12 to 25 fps and 30 to 44 points of 1 % low,
  while Donington 2560 did not change. Interpretation: high-fps cells leave more idle CPU per frame,
  and another process takes it.
- Still below 100 fps on the idle PC: Donington Enhanced at both resolutions (87 to 99 fps) and
  derby 2560 Enhanced mid (97.0). Still below 80 for the 1 % low: the derby's 2560 cells and
  1280 Enhanced mid (68 to 78), Donington 2560 baseline start (75.5) and Donington 2560 Enhanced (68
  and 73). Per run, the 2560 lows vary from 66 to 103.
- All of those cells are render-CPU bound (render minus acquire and present 7.5 to 10.3 ms, against
  main 3.9 to 4.9 ms). `r_encode` is the top stage (2.4 to 5.0 ms), then `r_prepare` (0.9 to 1.8 ms)
  and `r_submit` (1.0 to 1.6 ms at 2560 Enhanced).
- Donington 1280 baseline against 2560 baseline on the idle PC: render 7.57 against 8.76 ms at the
  start. The matrix's "1280 renders slower" was part of the background load.

So the tail was not code. The remaining 1 % low gap at 2560 and in Enhanced is the render thread's
average time (`r_encode`, `r_prepare`); the derby's tick frames with opponent dents add a smaller
main-thread tail (`dpn`, drive_present).

Timing rule from this: before a timing batch, check that no desktop app outside the build lock is
busy, and log the other processes' CPU seconds per run (as `mi.ps1` does) so that a loaded run can
be found and rerun.

## Contact dents (F864)

`dpn` (the opponents' dents) and `dpo` (the player's) project each body part of a car through its
deformation lattice when a contact changes the lattice. Both already ran on the compute pool and the
main thread waits for all parts, so the cost was the parts' work itself: 3-4 ms in single frames,
mostly in the derby.

What a field update changes: in the ignored test `deformation_dents_have_the_bits_of_whole_projections`
(Lacetti, 1500 ticks against the barriers, 81 parts, 24 field updates) every update moved 334 of the
360 lattice nodes (median, p90 and maximum alike). A dent spreads over the whole lattice, so
re-projecting only the vertices near moved nodes saves nothing; a first version that did this cost
2.83 ms per update against 2.22 ms for whole projections and was dropped.

What it does now (`body_damage::Dents`, one per part):

- Where a vertex lies in the lattice (clamped point, cell, fraction) depends only on the layout
  (dimensions, spacing, minimum), and its rest-lattice sample also on the rest nodes; both are fixed
  per car. `body_deformation::Field::probes` keeps them once per part
  (`deformation_grid::Sampler::locate`), so each field samples only the current lattice
  (`Sampler::sample_at`, the second half of the former `sample`, same arithmetic). A field with
  another layout rebuilds them.
- Normals are summed as Bevy 0.19.1's `compute_smooth_normals` sums them (per triangle in index
  order: angle weights, the triangle normal, then the three sums; `try_normalize`), on the part's
  positions, without the mesh copy and attribute round trip of the old path.
- A part whose drawn mesh already shows the projection gets no new mesh (the old path built one and
  then compared).
- Meshes this does not cover (not an indexed triangle list, or precomputed bounds) and refused layouts
  take the whole projection, which is unchanged.

`GRID_DENT_FAST=0` still selects the old serial path, for A/B comparisons in the same executable.

Exactness: the unit tests `probed_displacements_have_the_bits_of_sampled_ones` (grid-tools) and
`dents_have_the_bits_of_a_whole_projection` (grid-app: a bent sheet with degenerate triangles, 60
fields with resets, an inactive field and a refused one) compare positions, normals, indices, bounds
and the moved flag bit for bit with `project`. The real-car test does the same for all 81 parts at all
24 updates: all exact. It prints per update: whole projections 2.29 ms mean (max 2.79), the new path
1.15 ms mean (max 2.07), both serial on one thread.

Images: offscreen frames (1280x720, `--render-fixed-step`, baseline), `GRID_DENT_FAST=0` against the
default in one build: race 600 and 1500 and derby 600 0 px; derby 1500 2 px by 1 level.

Timing (one run each, 3000 ticks, 1280x800, no vsync, under the build lock; laps
mean/max/frames over 2 ms):

| scene | build | dp | dpn | dpo | fps | 1% low |
|---|---|---|---|---|---|---|
| derby | F861 default (2 runs) | 0.17/3.1/97 | 0.05/3.0/53 | 0.01/1.1/0 | - | - |
| derby | F864, `GRID_DENT_FAST=0` (serial) | 0.38/16.4/275 | 0.25/16.3/257 | 0.02/2.4/56 | 143.0 | 54.8 |
| derby | F864 | 0.16/2.6/17 | 0.04/2.4/8 | 0.01/0.9/0 | 149.4 | 108.1 |
| race | F861 default (2 runs) | 0.14/3.3/25 | 0.01/3.2/21 | - | - | - |
| race | F864, `GRID_DENT_FAST=0` (serial) | 0.15/14.8/21 | 0.03/14.6/21 | - | 124.8 | 102.6 |
| race | F864 | 0.12/2.7/5 | 0.01/2.5/5 | - | 124.0 | 102.6 |

Against the F861 default (pool, whole projections; another build and two runs, so a rough
comparison) the largest dent frame went from 3.0 to 2.4 ms (derby) and 3.2 to 2.5 ms (race), and dent
frames over 2 ms from 53 to 8 and 21 to 5. The spike is smaller, not gone: a large contact still
projects every part in the frame it happens.

The opponents' dents (`dpn`, the main tail that F863 found in frames over 14 ms on an idle PC: 2.2 ms
there) take this path too. A tail batch after the rebase onto F863 (derby 1280 baseline, alternating,
2 runs each) gave the fix `dpn` 1.67 and 1.82 ms in its frames over 14 ms (23 and 22 frames), and frames
with `dpn` over 2 ms 58/51 -> 12/15. The PC was loaded (other processes 85-110 CPU s per run; the fix's
largest `dpn` frames reached 9-10 ms, against 2.4 in the quieter batch above), and the base executable
ran at 98-104 fps with a quarter of its frames over 14 ms waiting outside the main world, so this batch
has no valid tail reference. An idle-PC before/after remains to be done.

Not done: projecting on the pool in the background and applying the result a frame later would remove
the wait, but it shows each dent one frame late and breaks the matched fixed-tick image check; spreading
a contact over frames has the same cost. Both remain options if the remaining 2.5 ms matters.

## Transparent draws: bindless car reflection overlay (F865)

Part 2 of the F862 follow-up: `r_encode` (2.5 to 5.7 ms) is the top render-thread stage in every
cell below the goal. Goal: lower its cost per frame through a shared path, with no visible change.

### What `r_encode` holds

`frame_split` now splits `r_encode` by camera schedule and counts the sorted transparent phase
after batching (fields in `frame_stats`, all render thread, Donington 1280 baseline, one run):

- `enc=pos:3d:order:runs:lead/prepass+shadows/opaque/transparent/post` (ms): `0.83/0.20/0.08/0.19/0.05`.
  The markers are the times at which each camera schedule reaches a pass, not the encode itself.
- `enc_tail` 2.31 ms: from the last marker to the submit. `enc_cbs` 21: command buffers pending
  at the submit. In wgpu 29.0.4 a render pass only records commands; the real encoding of every
  pass (per-draw validation, tracker merges, the Vulkan command recording) runs in
  `CommandEncoder::finish` (wgpu-core `command/mod.rs`, `encode_commands`). That is outside
  Bevy's pass spans (`elapsed_cpu`, F863) but inside `r_encode`, so the tail holds the per-draw
  cost of every pass. In earlier runs of this work it grew with sorted and shadow draws, while
  the opaque multi-draws stayed cheap.
- `t_draws`, `t_items`, `t_pipe_sw`, `t_mat_sw`, `t_pipes`, `t_mats`: draws, phase items,
  pipeline and material switches between consecutive draws, distinct pipelines and materials.
  Every item is queued with `batch_range` 0..1; a batch set's first item gets the set's whole
  instance range and `render_range` skips that many items. A draw is an item at that walk's
  position (an earlier count of "items with a non-empty range" counted items, not draws).

Observations:
- `r_encode` does not depend on resolution: Donington 1280 and 2560 baseline both 3.84 ms on the
  idle PC. The matrix's "Donington encodes slower at 1280" was background load (F863). No LOD or
  culling choice depends on the resolution here.
- Before this change the transparent phase drew 97 draws per frame in the derby and 232 at
  Donington (1280 baseline), almost one draw per item: neighbouring overlays had different
  materials (each part has its own base texture and mask), so the batcher split them.
- A grouping probe (`GRID_OVERLAY_GROUP`, not committed, it reorders the overlays and so changes
  the image) cut the derby to 38 draws (`r_encode` -0.11 ms) and Donington to 175 (-0.15 ms):
  about 2 to 3 µs of encode per removed draw.

### The change

`CarReflectionOverlay` is a bindless material (`#[bindless]`). With binding arrays, all overlay
materials share one bind group, and a draw finds its textures through the slot in the mesh
uniform (`material_and_lightmap_bind_group_slot`). Bevy's batch-set merge needs the same pipeline,
draw function, material bind group, mesh slabs and lightmap slab, but no longer the same material
asset, so consecutive overlays of different parts merge into one multi-draw. The phase order is
unchanged, so the image is unchanged.

Pitfall found on the way: the constants must be `#[data(0, OverlayUniform, binding_array(10))]`.
In bindless mode `#[uniform(.., binding_array(..))]` binds an array of uniform buffers
(`binding_array<T>`), while the shader read one storage array; the first build drew wrong
overlays on 8 to 28 % of the pixels. `data` is one storage buffer `array<T>`. Without bindless
support both forms bind one uniform, and the shader takes its `#else` path with the six separate
texture and sampler bindings.

`GRID_NO_BINDLESS=1` (texture_upload.rs `render_creation`) disables the binding-array features at
device creation, so the fallback path can be checked on a PC whose GPU has them.

### Image check

Fixed-step frames, 1280x720, the HEAD binary against the change, plus a second run of the change
as the noise baseline (`local/ab/crops-bl.ps1`), Donington 600, derby 1500, baseline and
Enhanced, touge 900:

| case | bindless vs HEAD | fallback (`GRID_NO_BINDLESS=1`) vs HEAD | noise |
|---|---|---|---|
| base-don-600 | 0 px | 0 px | 0 px |
| enh-don-600 | 0 px | 0 px | 0 px |
| base-derby-1500 | 1 px, max delta 1 | 0 px | 2 px, max delta 1 |
| enh-derby-1500 | 0 px | 0 px | 0 px |
| base-touge-900 | 0 px | 29.8 % | 0 px |

The touge fallback difference is not the overlay: without binding arrays Bevy's clustered decals
are unusable, so `night_lights` reports `cookie_decals=false` and the headlight cookies change.
That already happens on any GPU without binding arrays; the overlay's own fallback is identical
in the other four cases.

### Timing

`local/ab/ab-bl.ps1`, 3 alternating pairs of HEAD (og) and the change (bl), 1280 baseline, under
the build lock, means of the runs:

| cell | fps | render ms | `r_encode` ms | `r_prepare` ms | `t_draws` | `t_pipe_sw` |
|---|---|---|---|---|---|---|
| derby, og → bl | 134.9 → 136.5 | 6.92 → 6.80 | 2.95 → 2.78 | 1.19 → 1.21 | 97.1 → 11.2 | 9.2 → 9.3 |
| Donington, og → bl | 109.1 → 112.0 | 8.67 → 8.43 | 4.05 → 3.91 | 1.46 → 1.46 | 231.6 → 145.8 | 143.7 → 143.6 |

Per pair, `r_encode` -0.16/-0.17/-0.18 ms (derby) and -0.19/-0.11/-0.12 ms (Donington), fps
+1.4 to +4.7; every pair has the same sign. About 2 µs per removed draw, as the probe predicted.

### What remains

- Donington's 146 draws are mostly glass pane and its overlay, alternating by design (sort bias
  0.01, F847): two pipelines, 143 pipeline switches per frame. A merge would reorder them.
- `enc_tail` (the `finish` encoding) is most of `r_encode`. Next: split it by pass (shadow views,
  prepass, transparent), for example by draw counts per phase, before changing more draws.
- Every run, including HEAD, logs two `bevy_render::slab_allocator` "Use-after-free" errors. They
  predate this change and are not explained yet.

## Encode: indirect-call validation only where DX12 can be chosen (F868)

Part of the F865 follow-up on `enc_tail`, the `CommandEncoder::finish` encoding of every pass.

### Split of the tail by pass

`frame_split` now also counts the prepass phases (`d_pre`, `d_pre_alpha`) and the bindless
material slabs (`bl_slabs`, `bl_res`: slabs and resources of all bindless material allocators).
Donington and derby 1280 baseline, one run each:

| scene | `d_opaque` | `d_alpha` | `d_shadow` | `d_transp` | `d_pre` | `shadow_views` | `bl_slabs` | `bl_res` |
|---|---|---|---|---|---|---|---|---|
| Donington | 37 | 6 | 72 | 307 | 0 | 3 | 2 | 565 |
| derby | 164 | 4 | 49 | 145 | 0 | 3 | 2 | 315 |

The prepass draws nothing in either scene, so its share of the tail is zero. The shadow views hold
50 to 80 draws.

A sampling profile of the encode (`tools/sample_profile.py --busy --filter wgpu_core`, Donington,
inclusive shares of wgpu_core samples) shows where the tail goes: `encode_render_pass` 31.8 %,
`flush_bindings_helper` 16.7 %, `register_init_action` 14.2 % (self 9.0 %, with
`TextureInitTracker::check_action` 4.8 %), `queue_write_buffer` 17 %,
`TextureUsageScope::merge_single` self 3.1 %. In wgpu-core 29.0.4, `set_bind_group` merges the
group's whole resource set into the pass scope. The next draw's flush then re-registers every
texture of every group from the changed index up. A switch into a bind group with many textures
(a bindless slab) therefore costs time in proportion to its texture count. This is the next lead,
not part of this change.

### The change

Bevy 0.19.1 (`bevy_render` settings.rs, `WgpuSettings::default`) keeps wgpu's
`InstanceFlags::VALIDATION_INDIRECT_CALL` in release builds whenever DX12 is among the backends.
The D3D12 backend needs it for `num_workgroups`, `first_vertex` and `first_instance` of indirect
draws. `Backends::all()` always contains DX12, also on Android and Linux. With the flag, wgpu adds
a validation compute pass and tracks every indirect draw in `finish`; Bevy draws with indirect
multi-draws.

`texture_upload.rs` `drop_unreachable_dx12` (called from `render_creation`) removes DX12 and the
flag when wgpu would never pick a DX12 adapter:
- Off Windows DX12 does not exist, so it is removed without a probe.
- On Windows, wgpu-core's `request_adapter` lists the Vulkan adapters before the DX12 ones and
  stable-sorts all of them by device type for the power preference. A Vulkan adapter of the
  top-ranked type therefore wins whatever DX12 offers: discrete for `HighPerformance` (Bevy's
  default), integrated for `LowPower`, any for `None`. The probe enumerates only Vulkan; a full
  Vulkan and DX12 probe took 665 ms, most of it the DX12 enumeration.
- If no such Vulkan adapter exists (no Vulkan driver, or Vulkan only on the weaker GPU), DX12 and
  the flag stay, and the log says so.
- Debug builds, `WGPU_BACKEND`, `WGPU_ADAPTER_NAME` and `WGPU_VALIDATION_INDIRECT_CALL` keep
  Bevy's behaviour.

The log line is `render backends: DX12 left out (top-ranked Vulkan adapter), probe N ms`. On this
PC the chosen adapter is the same RTX 4070 on Vulkan as before.

### Image check

Fixed-step frames, 1280x720, `local/ab/crops-bl.ps1`, HEAD-equivalent (`local/ab/iv.exe`) against
the change (`local/ab/dx.exe`), plus a second run of the change as the noise baseline: Donington
600 and derby 1500 in baseline and Enhanced, and touge 900. Every case differs by 0 or 1 px at max
delta 1, the same as the noise runs (`local/ab/crops-dx`).

### Timing

Three alternating pairs, 1280 baseline, under the build lock (`local/ab/ab-iv.ps1` with the
environment variable, `local/ab/ab-dx.ps1` with the build; evidence
`local/frame-split-smoke/ab-iv-*` and `ab-dx-*`):

| run | scene | `r_encode` ms | pairs | `enc_tail` ms |
|---|---|---|---|---|
| flag on/off by env | derby | 2.56 → 2.52 | -0.03/-0.04/-0.07 | 1.26 → 1.21 |
| flag on/off by env | Donington | 3.72 → 3.58 | -0.23/-0.07/-0.10 | 2.28 → 2.19 |
| HEAD vs build | derby | 2.78 → 2.77 | +0.04/-0.10/+0.04 | 1.34 → 1.32 |
| HEAD vs build | Donington | 3.89 → 3.87 | +0.16/-0.17/-0.05 | 2.35 → 2.30 |

The gain on this desktop is small: about 0.03 to 0.1 ms of `r_encode`, near the run-to-run noise.
The build runs were noisier than the environment runs: one Donington pair of each kind ran under
background load. Phones lose the same per-draw work on slower cores.

Startup gains more. Bevy's renderer creation (first log line to `AdapterInfo`) took 520 to 840 ms
with DX12 enumerated and takes 55 to 90 ms without it. With the 170 to 330 ms probe, startup is
300 to 450 ms faster on this PC.

## r_prepare by system (F869)

19 November 2026, Claude Code, coordinator task after F862/F863: render CPU is the limiter in
every failing cell, and `r_prepare` costs 0.9-2.0 ms per frame. Split it by system, then fix the
largest shared-path costs, outside mac-perf's encode and pass-recording code. Evidence: this
worktree's `local/prep/` (scripts, stdout logs, image diffs).

### Split

A temporary probe (not committed) wrapped the render world's `Prepare` sets in timers. Release
build, 1280x800 Enhanced, `auto_no_vsync`, autopilot, 45 s, means over the run (ms per frame):

| cell | `write` (instance and uniform buffers) | `bind` (bind groups) | `flush` | `resources` | `r_prepare` | fps | other CPU s |
|---|---|---|---|---|---|---|---|
| derby | 1.13 | 0.89 | 0.19 | 0.24 | 1.62 | 106 | 14.5 |
| Donington | 0.80 | 0.74 | 0.44 | 0.24 | 1.59 | 93.5 | 15.8 |

The sets overlap in time (Bevy runs them on several threads), so they add up to more than
`r_prepare`. Phase census (`views`, mesh draws per phase): derby opaque 21, alpha 9, shadow 48
(3 views); Donington opaque 71, alpha 12, shadow 87.

### By system

Profile build (`--features profile`, recipe below), last 15 s window, exclusive ms per frame:

| system | derby | Donington |
|---|---|---|
| `prepare_preprocess_bind_groups` (Bevy) | 0.69 | 0.57 |
| `write_binned_instance_buffers` Opaque3d + prepass + Shadow (Bevy) | 0.89 | 0.68 |
| `write_batched_instance_buffers` (Bevy) | 0.10 | 0.29 |
| `prepare_generated_environment_map_bind_groups` (Bevy) | 0.28 | 0.27 |
| `mesh_stream::upload` (grid-app) | 0.23 | 0.41 |
| `mesh_stream::resort` (grid-app) | 0.08 | 0.26 |
| `osd_scene::overlay::prepare` (grid-app, mac-perf) | 0.13 | 0.14 |
| `prepare_mesh_bind_groups` (Bevy) | 0.11 | 0.10 |

Spans add their own cost, so these are relative sizes, not release times. `car_batch::refresh`
(0.18-0.24 ms) also shows in the window but runs in the main world's `PostUpdate`, not in
`r_prepare`.

The largest items are inside Bevy 0.19.1. `prepare_preprocess_bind_groups` rebuilds the GPU
preprocessing bind groups for every view and phase, plus one for unpacking each batch set,
every frame. The `write_*_instance_buffers` systems write the instance buffers per phase. Both
grow with views x phases x batch sets. Reducing them needs a Bevy patch (caching the bind
groups while the buffers are unchanged) or fewer batch sets (bindless materials, which is
mac-perf's area). Left for the coordinator to decide.

### Streamed mesh writes

`mesh_stream::upload` is the grid-app item. Writers: crowd billboards, ground detail, skid marks,
dents, source trees, OSD scene. Per frame (500-frame means, temporary statistics):

| cell | writes | KB | target buffers | contiguous runs |
|---|---|---|---|---|
| derby | 56-65 | 369-616 | 2.7-2.9 | 52-59 |
| Donington | 49-77 | 333-585 | 1.5-2.5 | 47-75 |

Contiguous runs are about as many as writes, so merging adjacent writes would not help. The cost
is per call: in wgpu-core 29.0.4 every `Queue::write_buffer` creates its own staging buffer
(`StagingBuffer::new` -> `create_buffer`), maps it and records a copy.

### Tried: one staging buffer per frame (not kept)

`upload` wrote every mesh range into one host-visible `MAP_WRITE | COPY_SRC` buffer created
mapped, then recorded one `copy_buffer_to_buffer` per range into an encoder pushed to Bevy's
`PendingCommandBuffers`, which is submitted before the frame's passes. Frames with unaligned or
overlapping ranges fell back to queue writes. `GRID_STREAM_STAGING=0` kept the queue writes for
A/B on one exe (sha256 prefix 8DDDDD38085E649F, `local/prep/stage.ps1`).

Images, offscreen 1280x720 Enhanced, queue writes against staging: race 600/1500 and derby 600
0 px, derby 1500 2 px by 1 level (the known derby noise, F864/F865).

Timing, 2 alternating pairs per cell, 45 s, `auto_no_vsync`, under the build lock (means, ms):

| cell | `r_prepare` queue | `r_prepare` staged | fps queue | fps staged | other CPU s |
|---|---|---|---|---|---|
| derby 1280 | 1.617 | 1.621 | 105.8 | 88.3 | 8-60 |
| Donington 1280 | 1.650 | 1.673 | 74.2 | 72.4 | 14-63 |
| derby 2560 | 1.633 | 1.665 | 101.0 | 101.4 | 18-25 |
| Donington 2560 | 1.711 | 1.669 | 86.5 | 73.7 | 19-25 |

`r_prepare` moves by -0.04 to +0.03 ms, both signs, within run-to-run noise. The fps columns are
not usable: single runs fell to 60-74 fps on both sides (pacing or background load: other
processes used up to 63 CPU s per run), and the quieter 2560 derby pairs agree within 1 fps.
In a release build the per-call staging cost is too small to show in `r_prepare`; the profile
build's 0.23-0.41 ms for `upload` was mostly span overhead and the copies themselves. The change
was dropped; `mesh_stream` is unchanged.

### What remains

All of the large `r_prepare` items are Bevy's own per-view, per-phase and per-batch-set work
(preprocess bind groups, instance buffer writes, the generated environment map bind groups).
Options, for the coordinator:
- patch Bevy 0.19.1 to keep the preprocess and environment-map bind groups while their buffers
  and views are unchanged (a `[patch]` of bevy_pbr; a seed refresh);
- fewer batch sets through bindless materials, which is mac-perf's area (F865 did this for the
  car reflection overlay).

### Profiling build recipe

The profile feature turns on Bevy's `debug` and `trace` features, so it rebuilds every Bevy crate.
To keep the normal release cache warm, build into a separate build dir seeded from the shared
seed, and into a separate target dir:

```
robocopy C:\cargo-build\seed\release C:\cargo-build\prof-<name>\release /E /MT:8
$env:CARGO_BUILD_BUILD_DIR = 'C:/cargo-build/prof-<name>'
tools\build_lock.ps1 cargo build --release -p grid-app --features profile --target-dir local\prep\tgtp
$env:GRID_PROFILE_SECONDS = '15'   # profile_span tables every 15 s
```

Here it took 714 s at the default job count, and the build dir grew to about 3.4 GB. Delete the
build dir afterwards (docs/DISK_USAGE.md). To read spans for the render world, take the
`profile_span` lines of the last window (they follow `rank=1`).


## Bevy bind-group cache for preprocess and environment maps (F873)

19 November 2026, Claude Code, coordinator decision after F869: patch Bevy 0.19.1 so that
`prepare_preprocess_bind_groups` and `prepare_generated_environment_map_bind_groups` (0.69 /
0.57 and 0.28 / 0.27 ms per frame in the F869 profile, derby / Donington) keep their bind groups
while every input is unchanged. Result: the cache never hits, the frame gets slower, and the patch
is **not** merged. Evidence: this worktree's `local/prep/` (`bgc.ps1`, `bg-*`, `rate-*`,
`img-bgc-*`, and the full patch as `f870-bevy-patch.diff`).

### What was tried

- `bevy_pbr` 0.19.1 vendored under `third_party/bevy_pbr` (crate sha256 `244ae7d6...a23d6`,
  git `b56fc29d`) through `[patch.crates-io]`; only `bevy_pbr` and its dependents rebuild.
- A small cache in `bevy_pbr::render`: the key is the layout id plus, per entry, the buffer id with
  offset and size, the sampler id or the texture-view id. A bind group is reused only when every
  entry is identical, so a reallocated buffer, a resized target or a new view gives a new bind
  group. Entries not used for two frames are dropped (two generations). The environment-map path
  keys on the probe entity and its input texture views. `GRID_BEVY_BIND_GROUP_CACHE=0` turns it off.
- Tests (GPU, `#[ignore]`d like the other adapter tests): one checks that identical inputs give
  the same bind group and that a new texture view, a reallocated buffer or a different size gives a
  new one; one switches the environment map of a camera twice (as between tracks) and checks the
  generator bind groups change each time and stay put between switches. With the cache off the
  second test fails at the "unchanged inputs" step, as designed.
- Images: derby and race at 600 and 1500 ticks, cache off against on and parent (F869) against on:
  0 px different in all 8 diffs.

### Timing

Release, Enhanced, `auto_no_vsync`, autopilot, 45 s, 2 alternating pairs per cell under the build
lock, other processes 6-10 CPU s per run (the PC was not idle). Means, ms per frame, cache off ->
on; exe `610FC32F796F1025`:

| cell | `r_prepare` | `bind` set | `r_submit` | render_ms | fps |
|---|---|---|---|---|---|
| derby 1280 | 1.496 -> 1.515 | 0.88 -> 1.21 | 0.974 -> 0.767 | 8.51 -> 8.67 | 114.8 -> 113.0 |
| Donington 1280 | 1.551 -> 1.556 | 0.80 -> 1.05 | 0.619 -> 0.542 | 9.66 -> 9.82 | 99.0 -> 97.4 |
| derby 2560 | 1.508 -> 1.517 | 0.89 -> 1.19 | 1.339 -> 1.190 | 8.67 -> 8.82 | 112.4 -> 111.0 |
| Donington 2560 | 1.586 -> 1.573 | 0.80 -> 1.05 | 1.354 -> 1.204 | 10.12 -> 10.29 | 94.7 -> 93.2 |

`r_prepare` is unchanged, the `bind` set costs 0.25-0.33 ms more, and render_ms is about 0.15 ms
worse; fps is lower in every pair. Below the 0.3 ms gain threshold in every cell, so dropped.

### Why it cannot hit

A temporary counter in the cache (30 s, 1280): derby 0 hits in 417k bind groups created by frame
2400; Donington 0 hits in 300k by frame 1200. Bevy builds about 160-260 of these bind groups per
frame, and every frame at least one input is a new `wgpu::Buffer`. Miss reasons over 1200 derby
frames (166k misses):

| bind group, changed entries | misses |
|---|---|
| bin unpacking, b2 (work-item buffer): new buffer, same size | 94.6k |
| bin unpacking, b0 offset + b2 new buffer, same size | 16.5k |
| bin unpacking, b2 new buffer, resized | 15.9k |
| bin unpacking, b0 offset + b2 resized | 7.6k |
| frustum culling, b5-b8 (indirect parameters) resized | 7.2k |
| reset indirect batch sets, b0 resized | 6.0k |
| build indirect parameters, b1-b4 resized | 6.0k |

So the inputs themselves are reallocated each frame, mostly at the same size: the work-item and
indirect-parameter buffers in `bevy_render::batching::gpu_preprocessing`. An input-keyed cache in
`bevy_pbr` only adds the key cost and keeps two generations of bind groups alive.

### Lead

A real gain needs `bevy_render` to keep those buffers across frames (reuse at the same capacity
instead of a new `wgpu::Buffer`); after that the existing Bevy code may already reuse much less
work, and a bind-group cache would start to hit. Unconfirmed: which call reallocates. Candidates
read in `gpu_preprocessing.rs`: `clear_indirect_parameters_buffers` (clears the phase buffers each
frame), `write_indirect_parameters_buffers` (`write_buffer` may reallocate), and
`RawBufferVec::reserve`, which reallocates when `changed` is set (for example by `set_label` with a
different label) even at the same capacity. That is a `bevy_render` patch, outside this task's
approved scope.


## Bevy bind-group cache over two frames (F874)

19 November 2026, Claude Code, coordinator follow-up to F873: find which `bevy_render` call
reallocates the preprocessing buffers every frame, keep them across frames, and measure retention
alone and retention plus the F873 bind-group cache. Result: nothing reallocates; the cache now
hits, but `r_prepare` does not get shorter, so the Bevy-patch route is closed. Evidence: this
worktree's `local/prep/` (`bgc2.ps1`, `b2-*`, `img-bgc2-*`, `rate-*`, and the patch as
`f874-bevy-patch.diff`).

### Probe: no reallocation, two buffer sets

None of the three suspects reallocates at a steady size. `RawBufferVec`, `UninitBufferVec` and
`PartialBufferVec` only create a buffer when the capacity grows (or the label changes), and
`clear_indirect_parameters_buffers` only clears the CPU side. The new buffer each frame comes from
`collect_buffers_for_phase`: it swaps the per-phase `PhaseBatchedInstanceBuffers` and
`PhaseIndirectParametersBuffers` with the global tables every frame, so two retained sets of
work-item and indirect-parameter buffers take turns. Bevy already retains them, so a retention
patch has nothing to add: "retention alone" is the upstream path.

The F873 cache dropped entries unused for one frame, so it evicted set A just before set A came
back. With a three-frame window (probe, 30 s, 1280): derby 255k hits and 100k created by frame
1800 (72 %), Donington 131k hits and 39k created by frame 600 (77 %). The largest remaining miss
is the bin-unpacking bind group's binding 0 (the `bin unpacking metadata` uniform), whose offset
changes with the batch-set order: 60.7k of the derby misses (25.6k on Donington). Not chased
further. The shipped variant keeps entries for two frames, the least that covers the swap; a unit
test checks a bind group that returns every other frame still hits.

### Correctness and images

GPU tests passed (`bind_group_cache` with the swap case, `environment_map_switch`). Cache off
against on, same exe (`45A06BC6A5492E91`), race and derby at 600 and 1500 ticks: 0 px in all four.
The comparison against F869's parent captures is no longer valid (14-20 % of pixels differ for
both sides, because origin/main changed the look after F869), so it is not counted.

### Timing

Release, Enhanced, `auto_no_vsync`, autopilot, 45 s, 2 alternating pairs per cell under the build
lock. The PC was busy: other processes used 29-130 CPU s per run (mac-perf's A/B ran before and
other sessions were active), against 6-10 in F873. Means, ms per frame, cache off -> on:

| cell | `r_prepare` | `r_bind` | render_ms | fps |
|---|---|---|---|---|
| derby 1280 | 1.776 -> 1.741 | 1.08 -> 0.83 | 10.11 -> 9.68 | 96.3 -> 82.7 |
| Donington 1280 | 2.003 -> 2.092 | 1.06 -> 0.97 | 12.49 -> 12.81 | 75.4 -> 76.2 |
| derby 2560 | 1.686 -> 1.697 | 0.99 -> 0.76 | 9.59 -> 9.29 | 102.1 -> 104.7 |
| Donington 2560 | 1.779 -> 1.796 | 0.92 -> 0.78 | 11.18 -> 11.19 | 85.6 -> 85.5 |

(One derby 1280 "on" run dropped to 69 fps under load; its render_ms was still lower.) The cache
saves 0.10-0.25 ms of `r_bind` in every cell, but `r_prepare` moves -0.04 to +0.09 ms: the bind
systems run in parallel with the write systems, so the frame waits for the writes
(`write_binned_instance_buffers`, 0.7-0.9 ms in F869) either way. render_ms and fps follow the
load, not the variant. Below the 0.3 ms threshold in both cells, so not merged, and the
Bevy-patch route for bind groups is closed.

### Next

The `r_prepare` critical path is the `write` set. `write_binned_instance_buffers` (opaque,
prepass and shadow phases) is the next item; its cost scales with batch sets and draws, which
mac-perf's car-glass material change may cut.

## Bin buffers: upload only changed batch sets (F875)

20 November 2026, Claude Code, coordinator follow-up to F874: split `write_binned_instance_buffers`
(0.7-0.9 ms of `r_prepare` in F869) by what it scales with, name the general lever, and measure it.
Result: almost all of it is per-batch-set upload overhead for data that did not change. A patched
`bevy_render` that uploads only changed batch sets cuts `r_prepare` by 0.60-1.23 ms in every cell
with identical images. Evidence: this worktree's `local/prep/` (`wbib-derby/`, `wbib-don/` probe
logs, `wbib.ps1`, `w-*`, `img-wbib-*`, `wbib-test/`).

### Probe: what the cost scales with

`write_binned_instance_buffers` (`bevy_render` `batching/gpu_preprocessing.rs`) walks every view
and every binned phase, and for every multidrawable batch set calls `RawBufferVec::write_buffer` on
two buffers: the batch set's instance list (`render_binned_mesh_instance_buffer`) and its bin to
indirect-parameters map (`bin_index_to_indirect_parameters_offset_buffer`). Bevy keeps the bins
across frames, so these buffers only change when an entity enters or leaves a bin or a bin is
created or removed. A temporary probe (release, Enhanced, 1280, autopilot, 30 s) counted per frame
the batch sets, their instances and bytes, how many changed since the last upload, and the time:

| phase (views) | derby sets | changed | upload µs | Donington sets | changed | upload µs |
|---|---|---|---|---|---|---|
| Opaque3d (1) | 160.9 | 2.10 | 525 | 53.0 | 0.87 | 191 |
| Opaque3dPrepass (1) | 160.9 | 2.10 | 495 | 53.0 | 0.87 | 172 |
| Shadow (3 cascades) | 60.9 | 2.10 | 202 | 75.7 | 2.68 | 240 |
| AlphaMask3d (1) | 10.1 | 1.10 | 38 | 11.6 | 2.19 | 45 |
| AlphaMask3dPrepass (1) | 10.1 | 1.10 | 37 | 11.6 | 2.19 | 43 |
| total | 403 | 8.5 | 1296 | 205 | 8.8 | 690 |

Deferred phases are empty. Each upload costs 1.5-1.9 µs whatever its size (4-12 KB per phase in
total; instance count and record size do not matter at this scale), so the cost is the number of
batch sets times views times phases. 98 % of the derby uploads and 96 % of the Donington ones
carried the same bytes as the frame before (alpha-mask sets change more often: 81-89 % unchanged).
The opaque and prepass phases upload the same batch sets twice, and each shadow cascade once more.

The general lever is therefore "skip unchanged batch sets", not a smaller record or fewer shadow
views: it removes the opaque, prepass and shadow repeats together and works for every track and car.

### Change: a dirty flag per batch set (vendored bevy_render)

`bevy_render` 0.19.1 is vendored in `third_party/bevy_render` and used through `[patch.crates-io]`
(`third_party/README.md` lists the upstream checksum and commit, and the upgrade rule). Every
changed line carries a `DRIG patch (F875)` marker.

- `RenderMultidrawableBatchSetGpuBuffers` gets `needs_upload`, true when the batch set is created.
  The only four places that change its two buffers set it: `insert` and `remove` (an entity enters
  or leaves a bin), `allocate_indirect_parameters` (a new bin) and `remove_bin_if_empty` (a bin goes
  and the last bin moves into its slot).
- `write_binned_instance_buffers` uploads a batch set only while the flag is set or a buffer has no
  GPU allocation yet, then clears it. A GPU buffer is created or grown only inside that upload, so
  growth is always uploaded; the CPU side grows only in `insert` and `allocate_indirect_parameters`,
  which set the flag. The shaders only read both buffers (`unpack_bins.wgsl`, `var<storage>`), so
  the GPU copy stays valid between frames.
- No double-buffer gap: `collect_buffers_for_phase` (F874) swaps only `PhaseBatchedInstanceBuffers`
  and the phase indirect-parameter buffers. The batch-set buffers live in the per-view
  `BinnedRenderPhase`, one GPU buffer each, so an upload in frame N is still there in frame N+1.
- Views: a view that appears (resize, shadow cascade or light added) gets a new phase whose batch
  sets start with the flag set; a removed view drops its phase. An emptied batch set is removed and
  comes back as a new one, flagged.
- `GRID_WBIB_ALWAYS_UPLOAD=1` restores the upstream behaviour (upload every batch set each frame).
  It stays in the build as a fallback if a stale bin is ever suspected, and for A/B checks.

### Correctness

- Test `crates/grid-app/tests/bin_buffer_upload.rs` (GPU, ignored by default, runs in prepush's full
  tier): a headless Bevy app renders lit, shadowed meshes in several bins and batch sets, then
  despawns entities (a bin empties), spawns one with a new mesh (a new bin), swaps materials (an
  entity moves to another batch set, as mac-perf's car-glass proxy materials do), hides and shows
  entities, swaps a mesh, and switches the shadow maps off and on (the shadow views go and come
  back). Each change lands on one frame; the two following frames and a later one must equal a
  fresh app's render of the same scene (0 px differing), and the change must be visible.
  Two steps only insert, with nothing else in the batch set changing: a spawn into an existing bin,
  and showing a hidden entity. Materials can share one bindless batch set, so a step that also
  removes something sets the flag anyway and would hide a missing `insert` flag. Result: passes
  (release, RTX 4070, Vulkan). Sensitivity check: with the flag removed from `insert`, the test
  fails at "show a hidden entity, change frame + 1: 583 pixels differ" (the spawn step still passed
  there, so a spawn also reaches another flagged path; not chased). Logs: `local/prep/wbib-test/`.
- Images, same exe (`D6AFB7F25FCD70F3`), switch on against off, race and derby at ticks 600 and
  1500: 0 px differing, max delta 0 in all four. Against clean main (`ecca9792`): also 0 px in all
  four.

### Timing

Release, Enhanced, `auto_no_vsync`, autopilot, 45 s, 2 alternating pairs per cell on one exe
(`GRID_WBIB_ALWAYS_UPLOAD=1` as "off"), under the build lock. Means, ms per frame, off -> on:

| cell | `r_prepare` | render_ms |
|---|---|---|
| derby 1280x800 | 1.494 -> 0.657 (-0.84) | 8.48 -> 7.49 |
| Donington 1280x800 | 1.572 -> 0.977 (-0.60) | 9.90 -> 9.67 |
| derby 2560x1440 | 1.908 -> 0.678 (-1.23) | 11.11 -> 8.18 |
| Donington 2560x1440 | 1.947 -> 1.201 (-0.75) | 12.71 -> 11.69 |

Every run of a cell is on the same side of its pair (derby 1280: 1.491-1.497 off, 0.654-0.661 on).
At 1280 the gain is close to the probe's upload time on Donington (0.69 ms) and below it on derby
(1.30 ms), because `r_prepare` measures the critical path of the prepare systems, not their CPU
sum. fps is not comparable at 2560: the PC was busy (other processes 33-130 CPU s per
run), and both derby "on" runs sat at exactly 60.0 fps. `r_prepare` is the criterion; it passes the
0.3 ms bar in every cell.

### Next

`r_prepare` is now 0.66 ms on derby and 0.98-1.20 ms on Donington. The F874 bind-group cache
(`local/prep/f874-bevy-patch.diff`) saved 0.10-0.25 ms of `r_bind`, which ran in parallel with the
write set; with the write set this much shorter, the bind systems may now be the critical path, so
it is worth re-timing on top of this change. The shared build seed must be refreshed after this
push (`tools/build_seed.ps1`), because the patched `bevy_render` replaces the registry crate.
