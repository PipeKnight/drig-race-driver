# Android GPU cost (P12, phone)

25 November 2026, Claude Code. Goal: 40+ FPS on the owner's phone (Xiaomi, Adreno 740,
2308 × 1080) without dips, through general fixes only and with no visible change to the
default look (owner, 2026-11-24). This page holds the phone measurements, the research on
tiled GPUs, and the Performance test rows that split the GPU cost. The CPU side and the earlier
phone work are in [PERFORMANCE.md](PERFORMANCE.md) ("Phone bisection", F555, F915, F925).

## 1. What the phone does now (APK 1a741c53, 2026-11-25)

The data comes from the owner's logs `log-20261125T143932Z-1a741c53e6dc.txt` (normal race) and
`log-20261125T144250Z-1a741c53e6dc.txt` (switches turned off one by one). Both are in the main
checkout under `local/android-logs/2026-11-25-xiaomi-b/` and are not in the repository. The
run was Donington, Lacetti, 11 AI. The table gives medians of the `frame_stats` lines. "both
single" means `main_single` + `render_single`.

| Run | FPS | avg ms | gpu ms | g_opaque ms | opaque draws |
|---|---:|---:|---:|---:|---:|
| normal | 24.3 | 41.2 | 38.0 | 29.2 | 59 |
| both single | 24.7 | 40.4 | 37.3 | 29.2 | 58 |
| shadows, foliage, ground, effects and HUD off | 27.4 | 36.5 | 33.4 | 27.1 | 58 |
| + only the cars (`empty_scene`), both single | 32.5 | 30.7 | 21.1 | 15.4 | 11 |
| pre-race menu (for comparison) | | | 8 | 0.55 | 232 |

Observed:

- **The render settings.**
  - The view is 1026 × 480: the dynamic height sits at its floor of 480 rows (F556), and the image is upscaled to the screen.
  - MSAA is 4x: `graphics_display … msaa=Some(Sample4)`.
  - HDR is off: Baseline preset, `hdr=false`, so the target is 8-bit, not Rgba16F.
  - One 3D view (`views3d=1.0`): no mirror camera in the chase view.
  - The car reflection cube is the static map, drawn once at load ([CAR_REFLECTIONS.md](CAR_REFLECTIONS.md)), so only the overlays cost per frame.
- **The display runs at 60 Hz.** `android_frame_rate requested_hz=60 result=0 display_hz=Some(60.000004)` and `android_frame_cap hz=Some(60.0)` (F636).
- **The GPU is the bottleneck.** The GPU time (38 ms) is nearly the whole frame (41 ms), and single-threaded execution changes nothing.
- **The opaque pass is most of the GPU time:** 29 ms in normal play, and 15–17 ms with only the cars drawn (11–16 draws).

Interpretation [I]:

- **Pacing.** At 60 Hz a frame lasts 2 or 3 vsyncs (33.3 or 50 ms). A GPU time of 38 ms therefore averages out near 41 ms, which is 24 FPS. A steady 40 FPS is not a 60 Hz divisor: it needs a 120 Hz mode presented every third vsync (section 5). 30 FPS needs the GPU under 33 ms with a margin.
- **MSAA stores cannot explain 29 ms.** Storing the 4x samples between the two passes moves about 1026 × 480 × 4 × (4 + 4) bytes ≈ 16 MB per store. With the load in the transparent pass, that is about 60 MB a frame, roughly 1 ms at the phone's bandwidth. Most of the opaque time is therefore fragment or vertex work, and the new rows (section 7) are there to split it.
- **The same phone ran faster a month ago.** F555 (2026-10-22, Nürburgring, 720 rows, MSAA 4x) measured 52–60 FPS while driving and 60 FPS with only the car drawn. Today Donington with only the cars drawn runs at 32 FPS on fewer pixels. The track and the scene differ, so this suggests a shader regression but does not prove one. The candidate commits since then are in section 8.

## 2. Bevy 0.19 render passes on a tiled GPU

Checked in the Bevy 0.19.1 and wgpu 29.0.4 sources (cargo registry):

- **The opaque and transparent phases are separate render passes.**
  - `main_opaque_pass_3d_node.rs:65` and `main_transparent_pass_3d_node.rs:82` both take the depth attachment with `StoreOp::Store`.
  - `texture_attachment.rs:40–80` always uses `StoreOp::Store` for colour, with `LoadOp::Clear` on the first pass and `LoadOp::Load` after it.
  - With MSAA, all multisampled colour and depth are written to memory after the opaque pass and read back for the transparent pass.
  - Bevy uses discard (`StoreOp::Discard`) only in its deferred, meshlet and cluster paths.
- **wgpu 29 can do better.** It has `LoadOp::DontCare` and `TextureUsages::TRANSIENT`. Changing the passes means patching Bevy's core pipeline, which is step 4 of the plan.

Sources (from the coordinator's research; the pages were not re-read for this entry):

- Qualcomm, Adreno best practices: use CLEAR or DONT_CARE loads, merge passes, and MSAA 2x is "practically free" on a tiler: <https://docs.qualcomm.com/bundle/publicresource/80-78185-2/topics/mobile_best_practices.md>
- Meta, never store all MSAA samples: <https://developers.meta.com/horizon/blog/loads-stores-passes-and-advanced-gpu-pipelines/>
- Arm's Mali best-practice guidance (no URL recorded): splitting a frame into passes can cost up to +400 % framebuffer bandwidth.

## 3. PBR shading on mobile GPUs

- An unlit material took an Adreno 618 from 11 to about 40 FPS: <https://github.com/bevyengine/bevy/issues/11213>. Direct light alone was about 30 % of the fragment time: <https://github.com/bevyengine/bevy/issues/10338>.
- Qualcomm (same page as above): fp16 / mediump arithmetic gives up to 2x.
- **Adreno LRZ.** LRZ is Adreno's low-resolution depth test. It turns off after any blended or colour-masked draw until the next depth clear, and also for draws that use discard or alpha-to-coverage. So alpha-tested foliage and the crowd should be drawn after the opaque geometry.
- Our own fragment shaders call `apply_pbr_lighting` without checking the `unlit` flag, as `track_surface.wgsl`, `paint_environment.wgsl` and `window_response.wgsl` do. So a material-level `unlit` switch would not cover them. The `lighting` row therefore removes the lights and environment maps instead, which every shader sees.

## 4. Pre-rotation

- `wgpu-hal-29.0.4/src/vulkan/swapchain/native.rs:202` always sets `pre_transform(IDENTITY)`. On a landscape phone, SurfaceFlinger then rotates every frame we present.
- Google measured 1–3 ms of extra work and a 40 % higher GPU clock for this: <https://developer.android.com/games/optimize/vulkan-prerotation>.
- **Fix (step 2).** This needs a wgpu-hal patch, since wgpu-core is already vendored under `third_party/`:
  - set `preTransform = currentTransform`;
  - give the swapchain the identity (portrait) size;
  - apply the rotation in our final upscale blit.

  It goes in behind a switch for an A/B run, and becomes the default only after a phone log shows a gain and no visible change.
- **Switch (F927).** `[diagnostics] pre_rotation = on` (or `GRID_PRE_ROTATION=1`), off by default; `tools/android_bench.ps1 -PreRotation` sets it. The log prints `render_pre_rotation on`. The patch is in `third_party/wgpu-hal`, `third_party/bevy_render` (surface size) and `third_party/bevy_core_pipeline` (rotated upscale blit and scissor); third_party/README.md lists the changed lines. Only the last pass, which writes the swapchain, rotates; the camera views, the dynamic resolution and the UI stay landscape.
- The rotation direction follows the Vulkan definition (`ROTATE_90`: the image content is rotated 90 degrees clockwise). If the phone shows the picture upside down or sideways with the switch on, the shader mapping in `blit.wgsl` is the place to check.

## 5. Frame pacing

- F636 asks for 60 Hz, and the phone grants it (section 1).
- **40 FPS** needs a 120 Hz display with every frame shown for exactly 3 vsyncs, which means a GPU time under 25 ms. That can be done with Swappy or with `ANativeWindow_setFrameRate` plus our own present pacing: <https://developer.android.com/games/sdk/frame-pacing>.
- **30 FPS** at 60 Hz is the fallback.
- Pacing is step 3, once the GPU time is near 25 ms. Pacing alone cannot make 38 ms fit.

## 6. Other findings

- **GPU preprocessing and indirect draws are on for the Adreno 740.**
  - Bevy blocks Adreno 730 and older, except the 720 (`gpu_preprocessing.rs:1328–1333`).
  - The adapter offers `MULTI_DRAW_INDIRECT_COUNT`. Reports on whether multi-draw-indirect-count is slow on Adreno disagree, so the `no_indirect` row measures it.
- BC textures are native on Adreno 7xx.
- **Thermal throttling.** Android's ADPF thermal headroom API reports it, and Google recommends runs of 15+ minutes. The owner's runs are minutes long, so the numbers here are early-run numbers.
- **Profiling without root.** Android Performance Analyzer or AGI (Android GPU Inspector) work on a debuggable APK, and the profile APK is debuggable.

## 7. Performance test rows for the GPU split (F-entry of 2026-11-25)

New test-only switches (`perf_test.rs`). Each one changes only the running view and nothing is saved:

The 2x MSAA row (`mobile:msaa2`, `Msaa::Sample2`) first panicked on the phone: the track's alpha-coverage
materials (`foundation_backing.rs` source coverage, used in every normal race) routed only 1x and 4x and called
`expect` on any other sample count ("source coverage refused unsupported pass", F927). Since F927 every multisampled
main pass (2x or 4x) takes the same hardware alpha-to-coverage variant, and an unknown variant keeps Bevy's stock
descriptor with one `source_coverage unsupported_variant` log line instead of a panic, so no MSAA setting can crash
the race.

| Row (screen) | Label | What it changes | What it isolates |
|---|---|---|---|
| Anti-aliasing 4x → Off | `mobile:msaa2` | 3D camera `Msaa::Sample2` | MSAA 4x vs 2x on a tiler |
| Anti-aliasing → Off | `mobile:msaa_off` (existed) | `Msaa::Off`; wins over `msaa2` | all MSAA cost |
| GPU-built draws → Off | `mobile:no_indirect` | `NoIndirectDrawing` on the 3D camera | GPU preprocessing / multi-draw-indirect cost |
| Car reflections → Off | `off:reflections` | the cars' reflection overlays hidden; the dynamic cube (if a dynamic level is set) draws no faces | the overlay draws |
| Lighting → Off | `off:lighting` | every directional, point and spot light hidden; the 3D camera's `EnvironmentMapLight` and `GeneratedEnvironmentMapLight` removed (restored on Off) | the light loops, shadow lookups and cube samples of every PBR fragment, including our own shaders |

Words without a screen row (`GRID_PERF_TEST`, `[diagnostics] perf_test`, `android_bench.ps1 -PerfTest`):

| Word | Label | What it changes |
|---|---|---|
| `hard_shadows` | `mobile:hard_shadows` | 3D cameras filter shadows with one hardware 2x2 compare (`ShadowFilteringMethod::Hardware2x2`) |
| `tiny_shadows` | `mobile:tiny_shadows` | sun shadow map 256 px, cascades 4/25 m (`light_shadows` is 1024 px, 6/60 m) |
| `local_lights` | `off:local_lights` | point and spot lights hidden; the sun and sky light stay |
| `env_maps` | `off:env_maps` | the 3D camera's environment maps removed (the `lighting` part without hiding lights) |
| `lean` | all of: `msaa_off`, `light_shadows`, `small_casters`, `far_props`, `reflections`, `hard_shadows`, `local_lights`, `tiny_shadows` | the ceiling of the mobile cuts; adds to the other words in any order |
| `no_reflections` | `reflections` + `env_maps` | every reflection path off |

Not added:

- **HDR off:** the phone already renders without HDR (section 1).
- **A separate mirror camera row:** the existing `mirror` switch covers the only mirror, the head-cam one, and the chase view has none.

### Auto cycle

`[diagnostics] perf_test = auto` (any build with dev-tools, including the phone APK) or `GRID_PERF_TEST=auto` (desktop)
runs 16 steps of 20 s, 5 min 20 s in all (owner, 2026-11-25: one pick, about 20 s a step, normal at both ends, under
6 minutes, only rows that can change GPU time):

1. normal;
2. each part alone: shadows, foliage, ground cover, effects, HUD;
3. all six parts off, then only the cars;
4. 720 rows;
5. MSAA off, `msaa2`, `no_indirect`, `reflections`, `lighting`;
6. only the cars without lighting;
7. normal again: the change against step 1 is the heat drift.

The cycle waits for the first race tick and holds while the race is paused, so loading and the menu do not eat steps.
Samples, the mirror and the two executor rows stay screen-only (no GPU change, or F925: no gain). The phone has no Auto
row (F559); the bench below sets the setting instead. Every `frame_stats` line carries `perf_test=<label>`, and
`py -I tools/phone_profile_summary.py --table LOG` prints one row per step (medians of FPS, frame time, GPU passes and
draws, p10 FPS, the change against the first normal step) and the drift line.

**Camera views** (owner, 2026-11-25: press every 30 s, in turn, Change View and Look Back; this also keeps the screen
awake). `tools/android_bench.ps1` does it from the PC: every `-PressSeconds` (30) it reads `dumpsys window` and, only
when `org.reassembly.grid` has the focus, holds the race's on-screen VIEW zone for 150 ms or the LOOK zone for
`-LookSeconds` (5) with `adb shell input swipe` on one point (`touch_controls.rs` zones; a plain tap ends inside one
frame and the game would miss it). Without the focus it sends nothing. Each press, sent or skipped, goes to
`presses.txt` with its seconds since `am start` (the game log's clock starts within about 0.1 s of it).
`phone_profile_summary.py --table --presses presses.txt` labels each 1 s window with its camera: the view of the
game's last `drive_camera event=change view=` line, or `back` inside a Look Back hold. It then adds the medians per step
and camera and an all-steps line per camera (FPS, frame ms, GPU ms, `g_opaque`, opaque draws), dropping the window of
each change. The presses run on their own 30 s clock, so the steps see different cameras; compare a step's cameras
with the all-steps lines, not steps with each other. (An in-game timed camera schedule was tried first and dropped
for this.)

### USB bench with no touch (`tools/android_bench.ps1`)

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\android_bench.ps1
```

1. Checks for exactly one authorised phone.
2. `adb install -r` of the newest profile APK (keeps the app's data folder).
3. Pushes `bench.ini` and `bench-args.txt` into the app's files folder. `bench.ini` holds the owner's usual race
   (Donington, Lacetti, 11 AI, Normal, as the event menu writes it), `force_feedback = off` (owner, 2026-11-25: no
   vibration, the phone lies on a table), `autopilot = on`, `perf_test = auto` and `[video] render_height = 480`
   (`-RenderHeight`, '' keeps the phone's own). Fixed rows make the steps comparable: in the first bench the dynamic
   rows moved between 1180x552 and 1026x480 from step to step, and the GPU time follows the pixel count.
4. Waits until the battery is at or below 40 °C (`dumpsys battery`, at most 15 min), then `am force-stop`, `am start`
   of `LauncherActivity`, waits 450 s, `am force-stop`. The temperature before and after goes into `run.txt`.
5. Removes its two files, pulls the new `Download/GRID-Reassembly` log into `local/android-bench/<time>/`, saves the
   game's logcat lines and prints the table and a verdict. The verdict fails when the log lacks the `android_bench`
   line or `drive_force_feedback off`.

The game reads `bench-args.txt` once at start and deletes it (`event_menu::android_args`, dev-tools builds only); its
lines are the arguments `--settings …/bench.ini --drive-ticks 30000`. A scripted drive with `autopilot = on` skips the
Race Options screen and drives itself (`drive_autopilot enabled=true scope=scripted-validation`). The owner's settings.ini
keeps its other keys (controls, video); bench.ini is read last and wins only on the keys it sets.

The phone has no shell environment. `-SetEnv NAME=VALUE` (repeatable) writes `env NAME=VALUE` lines into
`bench-args.txt`, and the game sets the `GRID_`-prefixed ones as environment variables before the app is built
(`event_menu::bench_split`). An example is `-SetEnv GRID_PERF_TIMING=1` for the per-stage physics lines
(`perf_timing`, `perf_rebounded`); the start line `android_bench args=[…] env=[…]` shows them. Any other `env` line
stays an ordinary argument. `-PerfTest none` writes no `perf_test` key, so the run uses the app's own default (lean
on the phone since F928).

Thermal soaks: `-Sustained` writes `[diagnostics] sustained_performance = on`. At start-up the app then calls
`Window.setSustainedPerformanceMode(true)` and, on Android 13+, `GameManager.setGameState` with gameplay
(`MainActivity.sustainedPerformance`); the log line `android_sustained_performance on supported=… game_mode=…
game_state=…` records `PowerManager.isSustainedPerformanceModeSupported()` and the game mode the user or system chose
(0 unsupported, 1 standard, 2 performance, 3 battery, 4 custom). An app cannot set its own game mode. The verdict fails
when the line is missing.

The script uses only `adb install -r`, `push`, `pull`, `shell am start` / `force-stop`, `logcat` and `rm` of its own two
files. It never uninstalls, never changes a phone setting and never touches the rest of the data folder, which holds the
owner's GRID copy and settings.ini.

What the owner does on the phone: turn on USB debugging (Developer options), connect the cable, accept this PC's
"Allow USB debugging" prompt (with "Always allow"), keep the phone unlocked and on mains or charged, and leave it
lying flat with the screen on. The game keeps the screen on by itself.

### Owner run by hand (no USB)

1. Install the profile APK, start the Donington event and drive. Normal look, at least 30 s.
2. Pause → Race Options → **Performance test**. Turn one row Off, resume and drive about 20 s. Turn it back On before the next row. Do this for each row in turn: **Anti-aliasing 4x**, **Anti-aliasing**, **GPU-built draws**, **Car reflections**, **Lighting**.
3. Finally turn **Lighting** and **Whole scene** both Off, and drive 20 s.
4. Send the log. The whole run takes 3–4 minutes.

The pairs of rows to compare:

| Compare | Shows |
|---|---|
| Anti-aliasing 4x vs normal | MSAA 4x vs 2x |
| Anti-aliasing vs normal | all MSAA |
| GPU-built draws vs normal | indirect drawing |
| Car reflections vs normal | the reflection overlays |
| Lighting vs normal | shading |
| Lighting + Whole scene vs F925's `empty_scene` | shading in the car shaders |

### Phone results (2026-11-25, F927)

Phone without its case, 1026x480 (`-RenderHeight 480`), Baseline preset (no HDR, static car cube, no sky IBL), the
owner's race (Donington, Lacetti, 11 AI), camera presses every 30 s, medians of 250 s of race:

| Run | Battery °C | FPS | GPU ms | g_opaque | g_transp | opaque draws |
|---|---|---|---|---|---|---|
| normal | 37.8→41.3 | 23.2 | 35.6 | 27.7 | 3.4 | 70 |
| `off:reflections` | 37.8→41.4 | 23.4 | 34.4 | 28.0 | 1.7 | 70 |
| `mobile:no_indirect` | 39.6→41.8 | 23.5 | 34.1 | 26.5 | 3.1 | 524 |
| `lean` | 37.0→40.6 | 28.3 | 25.1 | 21.0 | 1.1 | 72 |

GPU ms per camera:

| Camera | normal | reflections off | no_indirect | lean |
|---|---|---|---|---|
| chase_close | 38.3 | 36.0 | 36.7 | 28.6 |
| chase_far | 37.8 | 36.0 | 35.1 | 23.6 |
| back | 37.1 | 35.2 | 35.5 | 26.4 |
| bumper | 31.6 | 31.0 | 30.9 | 24.0 |
| bonnet | 33.7 | 32.2 | 32.2 | 24.2 |
| head-cam | 42.6 | 42.0 | 42.0 | 34.7 |

- **Reflections** cost about 1.3–2.3 ms on the phone: the overlays' second, transparent draw. The phone draws no cube
  faces per frame.
- **GPU-built draws** cost nothing measurable on the GPU: 7x the draws, same GPU ms.
- **`lean`** removes about 10 ms (30 %), but still leaves about 21 ms in the opaque pass. Above 16.7 ms a 60 Hz frame
  lands on 2 vsyncs at best.
- **The rest** is shading of the opaque pass plus a fixed part. With nearly nothing drawn, the Auto cycle still showed
  about 22 ms: all parts off 23.5 ms, only the cars (11 draws) 21–27 ms. The next tool for it is a GPU frame capture.
- The Auto cycle's steps differ in camera and track position as well. Compare matched runs of a whole lap, as above,
  or the per-camera lines.

### AGI frame capture (tried 2026-11-25, failed)

AGI 3.3.3 (`gapit`) can trace the debuggable profile APK. What the attempts taught:

- gapis finds adb only through `ANDROID_HOME` or `PATH`.
- Under Git Bash, `MSYS_NO_PATHCONV=1` keeps the `/sdcard/...` paths intact.
- `-uri` is a regex on `action:package/full.Activity`. Use `org.reassembly.grid/org.reassembly.grid.LauncherActivity`.
- `gapit trace` sets the four `gpu_debug_*` global settings itself and removes them when it exits. It installs the
  helper `com.google.android.gapid.arm64v8a`, which has to be uninstalled afterwards.
- With `-start-at-frame`, the trace layer (GraphicsSpy) loaded in our process, and gapii waited for its connection.
  Meanwhile our main thread blocked for 5.8 s in `GameActivity.onSurfaceCreatedNative`, and gapis gave up as
  "Traced application is unresponsive". Try `-start-defer` next, or let the connection come before surface creation.

## 8. Candidate shader changes since F555 (2026-10-22)

From `git log --since=2026-10-22 -- '*.wgsl'`. Bisect them once the rows show whether the time is in lighting or in the rest:

- car reflection overlay and dynamic cube (F670–F688, F865);
- F813 world cube;
- F775 night and headlight terms;
- `track_surface.wgsl` (8 commits);
- F815 car sun terms;
- F705 water and light shafts.

## 9. Plan

1. Profile APK with the rows above, and one owner run (this entry).
2. wgpu pre-rotation behind a switch, as an A/B.
3. Pacing for 40 FPS at 120 Hz.
4. Later:
   - no MSAA or depth store and load between the passes (discard, don't-care, or merged passes where Bevy allows);
   - fp16 in our own shaders;
   - a fix for any regressed shader the bisection finds, with no visible desktop change.
