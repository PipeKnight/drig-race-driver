# Android CPU cost (P12, phone)

The CPU side of the Android push (owner, 2026-11-26: stable 30+ FPS without dips, ideally 40+). The GPU side
is docs/ANDROID_GPU.md. Every change here is a runtime switch, off by default until the phone shows a gain
beyond noise, and general for all tracks and cars.

## 1. Where the frame goes (lean default, APK b008ab / F928 bench)

Observed on the Xiaomi 13 Ultra (Snapdragon 8 Gen 2: 3 little, 4 big, 1 prime core), lean profile, Donington
bench, `local/android-bench/f928-lean2` in the strange-chatelet worktree:

| Part | ms per frame |
|---|---|
| Frame (33.6 FPS) | 29.8 |
| GPU | 21.5–23.5 |
| Render thread (`render_ms`) | 26.1, of which encode 10.0, meshes 4.8, submit 3.6, prepare 2.8, bind 2.1 |
| Main thread (`main_ms`) | 23.8, of which fixed step 7.6, outside 5.5 |
| Extract | 2.1 |
| Fixed ticks per frame | 1.6–2.0 calm, 2.5–3.1 in dips |
| `drive_tick` | 4–11, of which the shared world step 3.5–9.5 |
| Transform propagation | 3.4–5.5 (2,265 GlobalTransforms change per frame) |
| Bounds/visibility window | 6–10 (includes parallel PostUpdate work) |

Interpretation: the lean profile is CPU-bound. The frame is about the render thread's 26 ms plus extract; the
main thread is close behind at 24 ms, and the GPU (22 ms) has headroom. A cut to only one of the two threads
moves the frame by at most the gap to the other. The single-threaded executor switches of F920 changed nothing
on the GPU-bound APK 1a741c53 (F927); that result does not carry over to the CPU-bound lean default.

## 2. Thread placement switches (`cpu_place.rs`)

`perf_test` words (diagnostics setting, `tools/android_bench.ps1 -PerfTest`), all off by default:

| Word | Effect |
|---|---|
| `pin_main_big`, `pin_main_prime` | `sched_setaffinity` of the main thread to the big (incl. prime) or the prime core |
| `pin_render_big`, `pin_render_prime` | the same for the render thread |
| `pin_pool_big`, `pin_pool_noprime` | the compute pool threads to the big cores, or to every core but the prime |
| `uclamp` | `sched_setattr` minimum utilisation clamp 512 of 1024 on the main and render threads |

ADPF hints are the `adpf` word of `adpf.rs` (docs/ANDROID_GPU.md section 10, F934). Its target is 16.67 ms; `GRID_ADPF_TARGET_MS` overrides it (F935), e.g. 33.3 under a 30 cap.

Core classes come from `/sys/devices/system/cpu/cpuN/cpu_capacity` (fallback `cpuinfo_max_freq`): little = the
lowest capacity, big = all faster cores, prime = the highest. The start log prints `cpu_topology`, each applied
switch a `cpu_place ... result=<0 or errno>` line.

New counters in every `frame_stats` line on Android and Linux (always on): `cpu_<main|render|pool>_run` and
`_wait`, ms per frame of run time and runqueue wait from `/proc/self/task/<tid>/schedstat` (pool = all compute
threads summed), and `main_little`, `main_prime`, `render_little`, `render_prime`, the share of frames whose
thread began on such a core. A high `_wait` or `_little` share is the scheduler cost the switches target.

## 3. Render thread (the CPU limit on the lean default)

Dev APK 0173ba35 (lean, 37.4 FPS): main schedule 21.3 ms, render schedule 25.7 ms, so the render thread sets the
frame. Physics is about 4–5 ms (1.66–1.83 fixed ticks per frame at about 4 ms each); capping the catch-up is a
minor lever there. APK 50f84fe0 render breakdown (ms per frame): `r_encode` 9.78, `r_meshes` 4.04, `r_submit`
3.45, `r_prepare` 2.98, `r_bind` 2.81; `queue_submit` 2.95 for 27 command buffers, `prepare_preprocess_bind_groups`
1.83, `prepared_mesh_producer` 1.78, `mesh_stream::upload` 1.67.

- **Serial encoder finish.** The 2D camera's first encode segment (4.24 ms) is not 2D work: it is the apply of
  the 3D camera's render systems, where Bevy's `RenderContextState` finishes each encoder one after another. In
  wgpu 29 `CommandEncoder::finish` does the actual Vulkan recording (`encode_commands`). Profile spans agree:
  `timed_shadow_pass apply` 1.09, `main_opaque_pass_3d apply` 0.69. Switch: `GRID_BEVY_PARALLEL_FINISH=1`
  (vendored bevy_render, F935) keeps the encoders unfinished until `PendingCommandBuffers::take` and finishes
  them on the compute pool, in push order.
- **Mesh-stream uploads.** `frame_split` prints `mesh_stream_writes` (write_buffer calls per frame; each
  stages its own copy). Many small writes would make coalescing worth a try.
- **Bind groups.** The F893 bind-group cache can be switched off with `GRID_BEVY_BIND_GROUP_CACHE=0` to measure
  its phone value.

### Batch 2 (F943)

- **Lazy Transit command buffer.** For every submitted command buffer, wgpu-core opens an internal "(wgpu
  internal) Transit" command buffer for the barriers and memory inits it needs, even when there are none. With
  about 27 buffers per submit this is the larger part of `r_submit`'s `queue_submit` (2.95 ms). Switch:
  `GRID_WGPU_LAZY_TRANSIT=1` (vendored wgpu-core, third_party/README.md) opens it only when a barrier or an
  init is recorded. Test: `crates/grid-app/tests/lazy_transit.rs` (zero-init of a buffer and a texture, a copy
  chain across command buffers, a render-pass clear read by the next buffer, two rounds).
- **Coalesced mesh-stream upload.** `mesh_stream_writes` on the proxy is about 90 `write_buffer` calls per frame
  (23–150), each with its own staging buffer. Switch: `GRID_MESH_STREAM_COPY=1` writes the frame's aligned
  ranges into one scratch buffer (one `write_buffer`) and copies them into the slabs with one command buffer
  pushed to Bevy's `PendingCommandBuffers`. Queue writes run before the command buffers of the same submit, so
  Bevy's own writes to those slabs this frame still land first; unaligned ranges keep `write_buffer`.
- **Static driver figures** (owner decision, 2026-11-26). Switch: `GRID_STATIC_DRIVERS` (`mobile::static_drivers`),
  on with the mobile preset, never on the desktop. The opponents' figures were already baked static meshes.
  The player's figure outside the head-cam is now baked too, in its s = 0 pose, with no joint entities. The
  head-cam figure keeps joint entities only for the arms' moving joints. Every other joint's skin matrix is a
  fixed frame in its part or in the arm joint that carries it, folded into the inverse bind matrices. The body
  holds `exterior_pose(0)` and no longer leans; the arms and fingers still reach the wheel, gear and handbrake. Test:
  `driver::tests::static_figure_keeps_only_moving_arm_joints` (every cockpit). A figure under a moving car still
  costs one `GlobalTransform` per remaining entity, so the gain is the removed entities only.
- **Transform probe.** `GRID_TRANSFORM_BURST_MS=0` prints `transform_burst` every frame, now with `same`: the
  number of changed `Transform`s whose value equals the last one seen (written without a change).

Ornament re-bounding (55–62 entities per frame, mostly `ornament/*`) is not a bug: it is the LOD fade, where
`respecialize_ranged` marks an entity changed when its `VisibilityRange` is added or removed, plus the shadow
proxies, about 0.2 ms. The 6.1 ms bounds window is a wall-clock window over parallel PostUpdate work.

## 4. Heat: the throttled steady state (`thermal.rs`)

Coordinator's phone soak, 15 minutes, lean, 60 cap: the phone throttles after about 2 minutes. FPS falls from
32 to about 25 and holds there. GPU time rises from 23 to 27–30 ms, `main_ms` from 27 to 35, `r_encode` from 10.8
to 13–16, and CPU per fixed tick from 4.6 to 5.4 ms. So the budget that counts is the hot phone, and every
candidate is rated twice: by cool frame time, and by work per second, which sets the throttled steady state.
A cut that only moves work between threads (thread placement, parallel finish) can help the cool frame and
cost heat. A cut that removes work (fewer frames, fewer pixels, less CPU per frame) helps both.

Always on under Android: a `grid-thermal` thread reads `AThermal_getThermalHeadroom` (NDK API 31) once a
second with a 10 s forecast. It also registers a thermal status listener (API 30). Every `frame_stats` line
gains `thermal_h` (forecast headroom; 1.0 = the SEVERE status), `thermal_status` (0 none … 2 moderate,
3 severe) and `thermal_level`. The start line `thermal version=1 event=start` lists the system's headroom
threshold for each status where the system has them (API 35); the governor's thresholds should come from
those numbers.

Governor, off by default: `GRID_THERMAL_GOV` lists its steps in order, and each level adds the next one.
- `scale`: the dynamic render height's ceiling drops to 0.8 of itself (720 → 576 rows on the phone).
- `cap`: the frame cap drops to 30 FPS.
A sample is hot at headroom ≥ `GRID_THERMAL_HOT` (0.8) or status ≥ MODERATE. Hysteresis: at most one raise
per 5 s, and one step down after 30 s of uninterrupted cool samples (headroom ≤ `GRID_THERMAL_COOL` (0.6), status
below MODERATE). It acts before the throttle when the forecast headroom crosses the threshold, and is general:
it reads only the system's headroom, not the track, car or phone model.

Interaction to watch: the dynamic render height judges frames against the 60 Hz period. Under a 30 cap, a
smaller height cannot make frames faster, so its probes come back ineffective and it returns to the ceiling.
It does not drift to the floor, but it keeps probing.

The phone executor is testing a static 30 cap and `sustainedPerformanceMode` in parallel; the governor is the
dynamic counterpart (60 cap while cool, 30 when hot).

## 5. Ranked candidates

Filled from the phone's A/B pairs (section 6); rank = measured phone gain.

| Rank | Candidate | Switch | Cool FPS (base → switch) | Throttled FPS (base → switch) | Verdict |
|---|---|---|---|---|---|
| 1 | 30 FPS cap, static or by the governor | static 30 cap (measured); `GRID_THERMAL_GOV=cap` pending | 32–38 → 30 (capped) | 25.9 → 28.6 (15 min race average) | measured, static cap: after minute 2 the share of seconds at 29+ FPS rises from 6 % to 66 %, and frames ≥ 50 ms fall from 7.3 % to 1.2 %. It does not remove every dip (the heaviest minute: GPU 29.5, main 33.1 ms). Governor A/B against the static cap pending |
| (2) | Parallel encoder finish | `GRID_BEVY_PARALLEL_FINISH=1` | pending | pending | expected: render thread −2 to −3 ms when cool. Same work, spread over more cores, so about neutral for heat |
| (3) | Render ceiling 0.8 when hot | `GRID_THERMAL_GOV=scale` | pending | pending | expected: little when cool (CPU-bound), but the hot GPU (27–30 ms) becomes a limit, so it helps the throttled state |
| (4) | ADPF session (`adpf.rs`) | `adpf`, `GRID_ADPF_TARGET_MS` | pending | pending | expected: the system picks the clocks for the target. With a 16.67 ms target and 26 ms actual it asks for a boost, so more heat. A 30 cap needs a 33.3 ms target |
| (5) | Thread pinning, uclamp | `pin_*`, `uclamp` | pending | pending | expected: may help when cool. `uclamp` raises clocks, so it likely costs heat |
| (6) | Bind-group cache off (control) | `GRID_BEVY_BIND_GROUP_CACHE=0` | pending | pending | measures the F893 cache's phone value |
| (7) | Lazy Transit command buffer (F943) | `GRID_WGPU_LAZY_TRANSIT=1` | pending | pending | expected: `queue_submit` (2.95 ms) shrinks by the empty Transit buffers. Proxy: no frame change outside noise (main-bound or GPU-bound proxy) |
| (8) | Coalesced mesh-stream upload (F943) | `GRID_MESH_STREAM_COPY=1` | pending | pending | expected: fewer staging buffers on the render thread. Proxy: −2.0 ms in the quiet pair (noise limit 2.1 ms); one earlier set spiralled, not reproduced |
| (9) | Static driver figures (F943, owner decision) | on with the mobile preset; control `GRID_STATIC_DRIVERS=0` | pending | pending | on by default. Proxy: −112 changed `GlobalTransform`s per frame (−5 %), propagation 2.01 → 1.94 ms median; frame change within noise |

Ranks in brackets are expectations before phone numbers (interpretation, not measurement).

## 6. Phone A/B pairs

**Static 30 cap against 60, 15-minute soak, lean** (phone executor via coordinator, 2026-11-26; battery 37.7 → 43.3 °C in the 30-cap run):
- Race average: 28.6 FPS at cap 30, against 25.9 at cap 60.
- After minute 2: 66 % of seconds at 29+ FPS, against 6 %.
- Frames ≥ 50 ms: 1.2 %, against 7.3 %.
- Throttled physics tick: 4.9–5.0 ms, against 5.3–5.4.
- Both runs end at the same battery temperature.
- The heaviest minute still reaches GPU 29.5 ms and main 33.1 ms, so those frames exceed the 33.3 ms budget.

Reading: under heat, a heavy view still needs about 15–20 % less main and render work to hold 30. Battery temperature reacts too slowly to be a trigger, so the governor keys on the thermal headroom forecast.

**E-core proxy, parallel finish off → on** (F935; a cheap first read, not a phone result):
- Predicted render thread 27.4 → 22.6 ms; `r_encode` 6.07 → 2.88 ms.
- Frame unchanged: the proxy is main-bound (main 29.9 → 30.8 ms).
- GPU counter 2.15 → 3.91 ms with a different scene sample (`vis` 1413 → 1541); unexplained, so check it on the phone.

**E-core proxy, batch 2** (F943; 3 repeats per arm unless noted, mobile preset, so the static figures are on in the base):
- The PC's background load changed between two sets of runs: the same base predicted 43.5 ms in the first set and 37.8 ms in the second (another session's capture ran beside the first). Only pairs from the same set are compared.
- Set 1: lazy Transit −1.0 ms (noise). Mesh copy +24.4 ms, but two of its three repeats sat at the 4-tick catch-up limit with `r_prepare` 11 ms. The second set did not reproduce this, so it is put down to the outside load (interpretation). The base lost its third repeat to a stall in loading.
- Set 2: mesh copy 37.8 → 35.8 ms (−2.0, noise limit 2.1), with `r_prepare` and `mesh_stream_kb_s` unchanged; figures skinned (`GRID_STATIC_DRIVERS=0`) 37.8 → 38.7 ms (+0.9, noise).
- Transform probe (`GRID_TRANSFORM_BURST_MS=0`, one race each, figures skinned → static): median changed `GlobalTransform`s 2262 → 2150, changed `Transform`s 1408 → 1298, unchanged rewrites (`same`) 26 → 6, propagation 2.01 → 1.94 ms (p90 2.58 → 2.23). The head-cam figure keeps 40 of its 75 skin joints in the proxy car and 1720 of 3225 over all 43 cockpits (the fingers are keyed by the steering clip and stay), and the player's exterior figure loses all its joint entities.
- Reading: on this proxy, every batch-2 switch lies within or near the noise limit. The figures are about 5 % of the propagation work, so a large phone gain is not expected. The phone's hot heavy view decides.

Pending: `GRID_THERMAL_GOV=cap` and `scale,cap` against a static 30 cap, both 15-minute soaks from the same start temperature, and the cool A/B pairs of every switch.
