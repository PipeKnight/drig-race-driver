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
| `uclamp` | `sched_setattr` minimum utilisation clamp 512 of 1024 on the main and render threads. On the phone the call fails with errno 1 (EPERM: `cpu_place ... event=uclamp ... result=1`), so the word changes nothing there (batch 1, F972) |

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
  `timed_shadow_pass apply` 1.09, `main_opaque_pass_3d apply` 0.69. Switch: `GRID_BEVY_PARALLEL_FINISH=1`, the mobile default since F965 (`=0` off)
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
  phone-only by design, but off by default until the phone pair shows a gain (the body no longer leans, which the owner can see; F944). The opponents' figures were already baked static meshes.
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
| 1 | 30 FPS cap, static or by the governor | static 30 cap (measured); `GRID_THERMAL_GOV=cap` measured equal or slightly worse (pair 9) | 32–38 → 30 (capped) | 25.9 → 28.6 (15 min race average) | measured, static cap: after minute 2 the share of seconds at 29+ FPS rises from 6 % to 66 %, and frames ≥ 50 ms fall from 7.3 % to 1.2 %. It does not remove every dip (the heaviest minute: GPU 29.5, main 33.1 ms). Governor A/B against the static cap pending |
| 2 | Single-threaded schedules | `main_single`, `render_single`; mobile default since F983; `main_multi`, `render_multi` turn them off | 39 → 42 (median; p10 32–33 → 38), with parallel finish on | 29.7 → 34.5 (15 min race average; seconds at 29+ FPS 32 → 85 %) | measured, made the mobile default; the throttled soak confirms it (queue 2): main 22.3 → 20.6 ms, render 21.9 → 19.1 ms, both on-runs beat both off-runs. CPU time per frame falls from about 67.7 to 55.1 ms over main, render and the pool, so less heat is likely |
| 3 | Parallel encoder finish | mobile default since F965; `GRID_BEVY_PARALLEL_FINISH=0` turns it off | 35 → 40 (median; p10 33 → 37) | 30.2 → 31.2 (15 min race average) | measured, made the mobile default: render thread 25.8 → 21.6 ms, `r_encode` 10.4 → 4.6 ms, p99 38.1 → 34.9 ms, both on-runs beat both off-runs. Warm soak: a small edge, the same heat (pair 6w) |
| – | Render ceiling 0.8 when hot | `GRID_THERMAL_GOV=scale` | not measured | 30.4–30.5 in both arms (15-min soaks, `render_height = auto`, 30 cap) | measured no-op (pair 10a, F987): the governor applies `Scale`, the ceiling drops 720 → 576, but the dynamic height already sits at its 480-row floor in both arms, so there is nothing to cut on this phone. Off |
| – | ADPF session (`adpf.rs`) | `adpf`, `GRID_ADPF_TARGET_MS` | not measured | 31/30 → 31/30 at a 30 cap | measured (pair 11, F987): neutral to slightly worse, main 22.4 → 23.1 ms, frames ≥ 50 ms 0.28 → 0.40 %. Off |
| – | Thread pinning, uclamp | `pin_*`, `uclamp` | neutral (pairs 1, 2, 3 repeated, 4) | not measured | measured: no pin set beats the run-to-run spread; `uclamp` fails with EPERM on the phone, so it never applied. Off; closed |
| – | Bind-group cache (F893) | default on; `GRID_BEVY_BIND_GROUP_CACHE=0` turns it off | 38/33 → 40/32 (median/p10, off → on); render 22.8 → 21.4 ms | not measured | measured (pair 7): both on-runs at or above both off-runs; stays on |
| – | Compute pool threads | `GRID_COMPUTE_THREADS=4`, `=8` | 39/33.5 → 39/33.5 (4), 40/34.5 (8) | not measured | measured (pair 8): neutral, the `=8` edge is one run; no change |
| – | Lazy Transit command buffer (F943) | `GRID_WGPU_LAZY_TRANSIT=1` | 44/40 → 45/40; avg 23.0 → 22.6 ms | not measured | measured (b1, F987, one run per arm): within the same-arm spread. Off |
| – | Coalesced mesh-stream upload (F943) | `GRID_MESH_STREAM_COPY=1` | 44/40 → 44/41 | not measured | measured (b2, F987, one run per arm): within the spread, no catch-up spiral; frames ≥ 50 ms 0.13 → 0.21 %. Off |
| – | Static driver figures (F943, owner decision) | `GRID_STATIC_DRIVERS=1` (off by default, F944) | 43/40 → 44/40 | not measured | measured (b3, F987, one run per arm): within the spread, so no gain that would justify the static figures. Stays off |

Ranks in brackets are expectations before phone numbers (interpretation, not measurement).

State after queue 3 (F987, F989): every candidate has a phone measurement. The measured wins are ranks 1–3 plus the bind-group cache; ranks 2 and 3 and the cache are mobile defaults; this series did not change the 30 cap (rank 1), a frame-cap setting that the governor did not beat (pairs 9, 10, 10a). Everything marked – stays off. On APK 2fde6a80, plain `lean` runs at 43–44 FPS (p10 40) from starts at ≤ 38 °C, against 38–40 in F972 and F984; that rise combines F983 and the GPU side's F967 and is not an A/B.

## 6. Phone A/B pairs

**Static 30 cap against 60, 15-minute soak, lean** (phone executor via coordinator, 2026-11-26; battery 37.7 → 43.3 °C in the 30-cap run):
- Race average: 28.6 FPS at cap 30, against 25.9 at cap 60.
- After minute 2: 66 % of seconds at 29+ FPS, against 6 %.
- Frames ≥ 50 ms: 1.2 %, against 7.3 %.
- Throttled physics tick: 4.9–5.0 ms, against 5.3–5.4.
- Both runs end at the same battery temperature.
- The heaviest minute still reaches GPU 29.5 ms and main 33.1 ms, so those frames exceed the 33.3 ms budget.

Reading: under heat, a heavy view still needs about 15–20 % less main and render work to hold 30. Battery temperature reacts too slowly to be a trigger, so the governor keys on the thermal headroom forecast.

**Pair 6: parallel finish off → on** (phone executor, 2026-11-27; APK c2de04b7, lean, cap 60 at 60 Hz, cool ABAB, 200 s per run, every start at 36.9 °C; means of race seconds 20+; evidence `local/android-bench/cpu1/p6-parfinish`):
- The log confirms `env=[("GRID_BEVY_PARALLEL_FINISH", "1")]` in the on-runs. Both on-runs beat both off-runs: FPS 35/35 off, 41/39 on; render thread 25.9/25.7 off, 21.0/22.1 on.
- FPS median 35.0 → 40.0, p10 33 → 37; frame p99 38.1 → 34.9 ms; frames ≥ 50 ms 0.06 → 0.13 % (both rare).
- Frame average 29.0 → 25.4 ms; main 23.0 → 22.0; render 25.8 → 21.6; `r_encode` 10.4 → 4.6; GPU 19.4 → 17.8 ms.
- CPU run/wait ms per frame: main 4.94/1.62 → 4.71/1.12; render 13.8/0.53 → 9.7/0.65; compute pool 50.8/12.3 → 52.0/12.8.
- `thermal_status` is 3 (SEVERE) in both arms already at 36.9 °C, with headroom about 1.0; the start line gives no thresholds (`thresholds=[]`). So this phone reports itself hot from the start, and the governor (section 4) counts it hot at once (pair 9).
- Reading: the switch moves about 4 ms of encoder finishes off the render thread, as the proxy predicted (F935), and the frame follows because the cool phone was render-bound. Main (22 ms) is now the limit, so main-thread pairs come next. Made the mobile default (F965); the warm repeat at the end of the batch checks the throttled state.
**Pair 9: static 30 cap against `GRID_THERMAL_GOV=cap`, 15-minute soak AB** (phone executor, 2026-11-27; APK c2de04b7, lean, 60000 ticks, battery 36.8 → 40.8 °C on both; evidence `local/android-bench/cpu1/p9-gov-cap`):
- The governor went to level 1 (cap) at 13.4 s, before the race (`reason=headroom headroom=0.931 status=0`), and never stepped down. This phone's headroom is 0.93–0.95 when cool, 1.01 by minute 2 and 1.09 at the end, and `thermal_status` is 3 from minute 1. With hot 0.8 and cool 0.6 the governor cannot release here, so on this phone it is a static 30 cap.
- Static cap vs governor: race average 28.1 vs 27.2 FPS; seconds at 29+ FPS after minute 2 52 % vs 38 %; frames ≥ 50 ms 2.57 vs 5.61 %; FPS median/p10 30/23 vs 28/22; frame 34.2 vs 36.4 ms; main 27.1 vs 29.1; render 30.3 vs 32.3; GPU 23.9 vs 24.2 ms.
- Both runs dip in the same minutes (8 and 13; minute FPS 24.2/22.5 and 21.6/20.6). The about 1 FPS gap is one soak per side, so run-to-run drift rather than a governor cost (interpretation). Main rises from 24 to 33–37 ms by minute 13: the throttled state is main-bound.
- Verdict: no gain over the static cap on this phone, so the governor stays off by default. Thresholds that release on this phone would need its own range; the system gives none (`thresholds=[]`), and a cool start already reads near SEVERE, so a 60 cap "while cool" has little room here. Row 1 stays the static cap. Pair 10 (`scale,cap`) pending.
**Pair 10: static 30 cap against `GRID_THERMAL_GOV=scale,cap`, 15-minute soak AB** (phone executor, 2026-11-27; APK c2de04b7, lean, 60000 ticks, 36.7–36.8 → 40.9 °C on both; evidence `local/android-bench/cpu1/p10-gov-scalecap`):
- Governor: scale at 13.4 s (headroom 0.925), cap at 19.5 s (0.933), never released; `thermal_h` 0.94 → 1.09, status 3 from minute 1.
- Static cap vs `scale,cap`: race average 27.1 vs 27.9 FPS; seconds at 29+ FPS after minute 2 36 % vs 50 %; frames ≥ 50 ms 5.18 vs 3.34 %; FPS median/p10 28/22 vs 30/23; p99 47.6 vs 44.4 ms; main 29.3 vs 27.2; render 32.6 vs 30.6; GPU 24.3 vs 23.8 ms. Both decline the same way late (main 24 → 34–39 ms by minutes 11–14, 21 FPS in minute 13).
- The scale step did nothing in this bench: its `bench.ini` sets `render_height = 480`, a fixed height, and `render_scale::adjust_height` returns at once unless the height is `auto`, so the thermal ceiling is never applied (no `render_dynamic` line in either log). Pair 10 therefore compares two 30-cap runs.
- The static cap's own spread between soaks (pair 9: 28.1 FPS / 52 % / 2.57 %; pair 10: 27.1 / 36 % / 5.18 %) is as large as the pair-10 gap, so the gap is noise.
- Verdict: no governor default. The `scale` step needs a test with `render_height = auto` (the player's mobile default), or a thermal ceiling that also applies to a fixed height; either is a follow-up, not a measured gain.
**Batch 1 on APK 4ea02bca** (phone executor, 2026-11-27, F972; parallel finish on by default; lean, cap 60, cool ABAB, 200 s and 30000 ticks per run, starts at 36.7–37.0 °C unless noted; evidence `local/android-bench/cpu1/p6w-parfinish-soak`, `p1` … `p5`):
- Pair 6w, parallel finish off → on, one 15-minute soak each: race average 30.2 → 31.2 FPS; seconds at 29+ FPS after minute 2 42 → 48 %; frames ≥ 50 ms 1.62 → 0.94 %; p99 45.9 → 43.8 ms; main 31.5 → 31.0, render 29.2 → 29.0, `r_encode` 6.4 → 6.2 ms; battery 36.9 → 41.2 against 37.0 → 41.4 °C. A small edge while throttled at no heat cost; one soak per side.
- Pair 1, `pin_pool_noprime`: neutral. FPS median/p10 39.5/35.5 → 40/36; main 22.1 → 21.9; GPU 17.9 → 17.7 ms; inside the run-to-run spread.
- Pair 2, `pin_main_big,pin_render_big`: neutral. 38.75/35.5 → 39/34; main 22.8 → 22.3 ms.
- Pair 3, `pin_main_prime,pin_render_big,pin_pool_noprime`: inconclusive. One on-run started at 32.1 °C (`thermal_status` 0) and is heat-inflated (42/40); the other, from 36.9 °C, gave 40/31 against base 39/36 and 38/36. Mean main 22.6 → 21.4 ms. To be repeated.
- Pair 4, `uclamp`: neutral to slightly worse (39.5/34.5 → 39/34.5, p99 34.7 → 36.5 ms), but the log shows `event=uclamp ... util_min=512 result=1`: `sched_setattr` returned EPERM (`cpu_place::clamp` logs the errno), so the clamp never applied and the pair measured two equal arms. The pins log `result=0`.
- Pair 5, `main_single,render_single`: win. Both on-runs beat both off-runs: FPS median/p10 39/32 and 39/33 → 42/38 and 42/38; frame average 25.9 → 24.0 ms; p99 35.1 → 34.1 ms; main 22.3 → 20.6, render 21.9 → 19.1, `r_encode` 4.8 → 3.9, GPU 17.9 → 17.0 ms. CPU run time per frame moves from the pool to the two threads: main 4.7 → 19.4, render 9.8 → 19.0, pool 53.2 → 16.7 ms, about 67.7 → 55.1 in total.
- Reading (interpretation): on this phone the multi-threaded executors cost more in task hand-off and pool wake-ups than the parallel systems gain; one thread per world runs the same systems with less overhead. This reverses F927, which ran on the GPU-bound APK 1a741c53, and agrees with section 1's note that the CPU-bound lean default could differ. Made the mobile default (F983): `Switches::MOBILE` = `lean` + both executor switches; the desktop keeps Bevy's executors. The 15-minute soak AB of p5 checks the throttled state.

**Queue 2 on APK 4ea02bca** (phone executor, 2026-11-27, F984; same setup as batch 1, starts at 36.8–37.0 °C; in this APK `lean` is still the multi-threaded arm; evidence `local/android-bench/cpu1/p7…`, `p8…`, `p5w…`, `p3r…`):
- Pair 5w, `main_single,render_single` against multi-threaded lean, one 15-minute soak each: race average 29.7 → 34.5 FPS; seconds at 29+ FPS after minute 2 32 → 85 %; frames ≥ 50 ms 1.76 → 0.39 %; p99 46.6 → 40.5 ms; main 32.5 → 23.0, render 29.8 → 25.3, GPU 23.4 → 20.5 ms; minute 14 25.7 → 32.0 FPS. CPU run time in per cent of a core: main 6.6 → 21.5, render 13.0 → 25.2, pool 75.7 → 23.4, total about 95 → 70. Battery 36.9 → 41.8 against 36.9 → 42.2 °C. The default of F983 holds while throttled, and the throttled state is no longer main-bound at 30.
- Pair 7, bind-group cache off → on (the env reached the app): FPS median/p10 38/33 → 40/32; frame average 26.5 → 25.2 ms; main 22.6 → 21.7, render 22.8 → 21.4, `r_encode` 5.0 → 4.6 ms. Both on-runs (41, 39) at or above both off-runs (38, 38). The F893 cache stays on.
- Pair 8, `GRID_COMPUTE_THREADS`: default 39/33.5, `=4` 39/33.5, `=8` 40/34.5; the `=8` edge comes from one run (41 against 39). Neutral; no change.
- Pair 3 repeat: 38.75/32.5 → 38/32, main 22.4 → 22.9 ms; inside the spread. Pinning is closed as neutral.

**Queue 3 on APK 2fde6a80** (phone executor, 2026-11-27, F987; plain `lean` now includes the single-threaded schedules; owner-approved shortcuts: starts at ≤ 38 °C, batch 2 as AB with one run per arm; evidence `local/android-bench/cpu1/p11-adpf`, `b1`…`b3`, `p10a-scale-auto`):
- Pair 11, ADPF at a 30 cap: 31/30 FPS in both arms; main 22.4 → 23.1 ms; frames ≥ 50 ms 0.28 → 0.40 %. Neutral to slightly worse; not promoted.
- Batch 2: lazy Transit 44/40 → 45/40 (avg 23.0 → 22.6 ms); mesh-stream copy 44/40 → 44/41, no catch-up spiral; static drivers 43/40 → 44/40. All three inside the same-arm spread, so no repeats; all stay off.
- Pair 10a, `scale,cap` with `render_height = auto`, 15-minute soaks: the governor applies `steps=[Scale, Cap]` and the dynamic ceiling drops 720 → 576, but the height sits at its 480-row floor most of the time in both arms; 30.4–30.5 FPS and 98 % of seconds at 29+ in both. A measured no-op on this phone.

**E-core proxy, parallel finish off → on** (F935; a cheap first read, not a phone result):
- Predicted render thread 27.4 → 22.6 ms; `r_encode` 6.07 → 2.88 ms.
- Frame unchanged: the proxy is main-bound (main 29.9 → 30.8 ms).
- GPU counter 2.15 → 3.91 ms with a different scene sample (`vis` 1413 → 1541); unexplained, so check it on the phone.

**E-core proxy, batch 2** (F943; 3 repeats per arm unless noted, mobile preset, which then turned the static figures on, so they are on in the base; since F944 they are off by default):
- The PC's background load changed between two sets of runs: the same base predicted 43.5 ms in the first set and 37.8 ms in the second (another session's capture ran beside the first). Only pairs from the same set are compared.
- Set 1: lazy Transit −1.0 ms (noise). Mesh copy +24.4 ms, but two of its three repeats sat at the 4-tick catch-up limit with `r_prepare` 11 ms. The second set did not reproduce this, so it is put down to the outside load (interpretation). The base lost its third repeat to a stall in loading.
- Set 2: mesh copy 37.8 → 35.8 ms (−2.0, noise limit 2.1), with `r_prepare` and `mesh_stream_kb_s` unchanged; figures skinned (`GRID_STATIC_DRIVERS=0`) 37.8 → 38.7 ms (+0.9, noise).
- Transform probe (`GRID_TRANSFORM_BURST_MS=0`, one race each, figures skinned → static): median changed `GlobalTransform`s 2262 → 2150, changed `Transform`s 1408 → 1298, unchanged rewrites (`same`) 26 → 6, propagation 2.01 → 1.94 ms (p90 2.58 → 2.23). The head-cam figure keeps 40 of its 75 skin joints in the proxy car and 1720 of 3225 over all 43 cockpits (the fingers are keyed by the steering clip and stay), and the player's exterior figure loses all its joint entities.
- Reading: on this proxy, every batch-2 switch lies within or near the noise limit. The figures are about 5 % of the propagation work, so a large phone gain is not expected. The phone's hot heavy view decides.

Pending: `GRID_THERMAL_GOV=cap` and `scale,cap` against a static 30 cap, both 15-minute soaks from the same start temperature, and the cool A/B pairs of every switch.
