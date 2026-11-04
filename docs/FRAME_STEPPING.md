# Physics steps per rendered frame: original against the port

21 October 2026. Question from the coordinator: does the original run one physics step per rendered
frame with no catch-up? If so, port that, so that a slow frame no longer runs several catch-up
ticks. Background: the owner's phone (Adreno 740, Drift Battle at the Nürburgring `route_1`) fell
from 18–34 FPS to 3.4–5.9 FPS once about 220 prop pieces lay on the track, while
`sim_ticks_per_s` stayed at 53–62 (F540).

**Answer.** The original steps once per rendered frame, and that step is the real frame time. It is
not a fixed step. The port's physics and game logic assume the fixed 16 ms `DELTA`, so a faithful
port is a larger change, and the owner has to decide on it (section 4). Static analysis of the
private analysis image only. The original game was not launched.

**Done (F544).** The owner chose option 2: the interactive app runs at most 4 fixed ticks per
rendered frame (section 5). The fixed 16 ms tick stays, so this is not the original's rule.

Labels: [O] observed in the original (decompile or live read), [D] derived from [O] facts,
[I] inference.

## 1. The original: one step of the frame delta per frame

Call chain per rendered frame [O]:

- Physics task `5E7F30` is one of the per-frame tasks. Task objects have a three-slot vtable, and
  slot 1 is `run(double* delta)`; `5E7F30` sits in vtable `CE34EC`.
- `5E7F30` converts the double delta to f32. It passes it to the car manager `A02B70` and then,
  with the same value, to the world scheduler `A72930`. Neither call changes the value.
- `A72930(world, dt)` uses world `+8` maxfps, `+C` minfps and `+10` maxTimesToRun.
  WORLD_STEPPING_RESEARCH.md has the full rule; re-decompiled for this note with the same result:
  - `dt <= 1e-6`: no step;
  - `1/dt > maxfps`: one step of `1/maxfps`;
  - otherwise one step of `dt`, unless `1/dt < minfps`;
  - below minfps: `n = ceil(minfps*dt)`, at most maxTimesToRun, then n substeps of `dt/n`, with
    the step clamped back into `[1/maxfps, 1/minfps]`.
  - Intermediate substeps run `AD8D70`, `A727A0`, `A71C70`. The last runs `AD8E30`.
- Live values (F285, `tools/read_grid_world_settings.ps1`, five identical samples): maxfps 10000,
  minfps 0, maxTimesToRun 1. With these values [D]:
  - every frame longer than 0.1 ms gets exactly one world step, of length dt;
  - minfps 0 never triggers the substep branch, so this level has no upper bound on dt;
  - there is never more than one world step per frame. The world catches up by taking a longer
    step, never by taking more steps.
- Live confirmation [O]: world `+7C8` (the step) read 0.0060602077 s at the owner's 165 Hz
  display. The body delta ring held 0.00607639737–0.00608282071 s (LAUNCH_AND_GRID_START.md). The
  F419+ captures with vsync read about 16.67 ms.

Sub-steps inside the physics step [O]:

- The car job substeps the wheels and tyres with `count = max(1, trunc(dt * 999.99994))` and
  `sub_delta = dt / count`. That gives 6 substeps at 165 Hz, 16 at 60 Hz and 249 at dt 0.25 s
  (VEHICLE_UPDATE.md, "Embedded dynamics context and substeps"). The wheel/tyre part therefore
  adapts to any dt in about 1 ms pieces.
- The rigid-body/contact world takes the single step: quick-step with 5 iterations, then the
  first-order correction pass with 3 (F285 live solver block). It has no time-based substeps. ERP
  and CFM act per step, so contact stiffness depends on dt.

Not found [open]:

- The frame dispatcher that computes the double delta and calls the task list.
- Whether it clamps a long frame, for example after a loading hitch.
- Static facts:
  - task records are 0x28 bytes, queued by `87E2D0` (every frame) and `87E250` (fixed period);
  - the task scheduler is embedded at game object `+0x20` (registrations in `632B10` and
    `5EB6A0`);
  - QPC and GetTickCount wrappers: `B95CC0` (seconds since start, no direct callers) and `A6BBF0`
    (drift-corrected counter, 527 callers);
  - stopwatch callers of `A3F3B0`/`C457D0` are networking (`bdSocket`), and the QPC user
    `9C0F70` is the UDP motion-platform output.
- Settling this needs a live trace of a long frame, for example a breakpoint-free read of world
  `+7C8` while the window is dragged. That is a capture backlog item, not a blocker for the
  decision below.

## 2. The port: fixed 16 ms ticks with Bevy catch-up

- Interactive `grid-app --drive` (`crates/grid-app/src/main.rs`, the `Time::<Fixed>::from_seconds(DELTA)`
  insert next to `add_systems(FixedUpdate, drive_tick)`):
  - every simulation system runs in `FixedUpdate`, with `DELTA = 0.016`
    (`crates/grid-tools/src/source_vehicle.rs`);
  - nothing sets `Time<Virtual>::max_delta`, so Bevy 0.19.1's default of 250 ms applies
    (`bevy_time-0.19.1/src/virt.rs`, `DEFAULT_MAX_DELTA`);
  - `run_fixed_main_schedule` (`fixed.rs`) adds the virtual delta to the accumulator and runs
    `FixedUpdate` until the accumulator is spent: `floor(acc / 16 ms)` ticks per frame, up to about
    15.
- Headless (`--headless`), offscreen render captures with `render_fixed_step`, and the in-crate test
  apps use `TimeUpdateStrategy::ManualDuration(DELTA)`: exactly one tick per frame. grid-soak,
  grid-opponent-race and the identity runs call `Runtime::tick` directly without Bevy time. None of
  these see catch-up.
- `frame_stats ... sim_ticks_per_s` (`fps_counter.rs`) counts `runtime.ticks` per second of frames.

### Why the phone collapsed [D]

Let a frame cost `r` for rendering and other per-frame work, and `c` per tick.

- At steady state with `c < 16 ms`, `k = r / (16 ms - c)` ticks per frame.
- As `c` approaches 16 ms, `k` and the frame time grow without bound until the 250 ms clamp
  binds. From then on every frame runs about 15.6 ticks: frame time `r + 15.6 c`, game speed
  `250 ms / (r + 15.6 c)`.
- F540 matches the clamped state. Frames of 170–292 ms (`avg_ms`), 3.4 FPS, and 53–62 ticks/s:
  15.6 ticks in a 292 ms frame is 53 ticks/s, about 17–18 ms per tick with 220 resting pieces.
- The original under the same load would draw one frame of about `r + c(dt)` and step once. With
  `r` = 10 ms and `c` = 18 ms, that is about 36 FPS in real time instead of 3.4 FPS at 0.86 of
  real time. The wheel part of `c` grows with dt in the original; the world step does not.

| | Original | Port, interactive | Port, headless / identity / soak |
| --- | --- | --- | --- |
| World steps per frame | exactly 1 | `floor(acc/16 ms)`, up to ~15 | 1 |
| Step length | real frame dt (no clamp found) | 16 ms | 16 ms |
| Wheel substeps | `max(1, trunc(dt·1000))` of `dt/n` | 16 of 1 ms | 16 of 1 ms |
| Slow tick | longer step, FPS ≈ 1/(r + c) | more ticks, FPS collapses | n/a (as fast as CPU) |

## 3. Why the original's rule was not ported

The task's stop condition holds. The original's step is variable, and the port's physics assumes a
fixed dt.

- `source_vehicle::DELTA` is used in 40 source files.
  - It is the world solver parameter (`parameters[0]`), the deformation publication step, lap
    timing (`ticks * DELTA`) and the AI and event timers.
  - Many timers count ticks, not seconds.
- F285 measured the prototype at 16 ms against 16/3 ms with tripled controls. The drive itself
  differed: 37.5 against 28.8 m/s at the same record. Contact ERP/CFM per step also changes
  stiffness with dt. So a variable dt changes the handling, not only the timing.
- F286 (owner): the fixed step was kept for display-rate independence; 16 ms ≈ original at 62.5 fps.
- Bit-identical replays, `tools/identity.tsv`, flashback and `--headless` depend on fixed ticks.
  A variable step would apply to the interactive app only, so interactive play and every check
  would then run different physics.

## 4. Options for the owner

1. **Faithful: one step of the real frame time per frame** (the original's rule). Interactive only;
   headless, identity, soak and replays keep 16 ms.
   - Turn `DELTA` into a per-tick `dt` through the world solver, the wheel substep count
     (`trunc(dt·1000)`, already the original's rule), deformation, lap timing, and every
     tick-counted AI and event timer.
   - Record dt per tick in replays and flashback.
   - Large change across grid-world, grid-tools and grid-app. Handling then depends on the display
     rate, as in the original. Two physics regimes need testing.
2. **Keep fixed 16 ms ticks and cap the catch-up** (not the original's rule, but it removes the
   spiral).
   - One line in the interactive app: `Time<Virtual>::set_max_delta(N × 16 ms)`.
   - When a tick costs more than the frame allows, game time slows instead of FPS collapsing.
     Headless, identity, soak and replays are unchanged, because ManualDuration already gives one
     tick per frame.
   - Trade-off by N, from the model above, for `c` = 18 ms and `r` = 10 ms [D]:

     | N | FPS | game speed |
     | --- | --- | --- |
     | 1 | 36 | 0.57 |
     | 2 | 22 | 0.70 |
     | 4 | 12 | 0.78 |
     | 15.6 (today) | 3.4 | 0.86 |

   - Normal play keeps real time while it needs at most N ticks per frame. The phone today runs
     about 2 ticks per frame at 30–34 FPS, so N = 1 or 2 would slow ordinary play. N = 4 keeps
     real time down to 15.6 FPS.
3. **Make the tick cheaper** (independent of 1 and 2, and needed for 60 FPS either way). F541 found
   that half of a settled tick is whole-world body-array copies, plus an O(n²) piece-pair pass and
   car–piece hull tests.

Recommendation [I]: 2 with N = 4 now, as a small reversible change, together with 3; 1 only if the
owner wants the original's display-rate-dependent handling.

## 5. Ported: at most 4 ticks per frame (F544)

Owner's choice, 21 October 2026: option 2 with N = 4.

Code (`crates/grid-app`):
- `main.rs` `cap_catch_up`: `Time<Virtual>::set_max_delta(4 × Duration::from_secs_f64(DELTA))`.
  That is exactly four fixed timesteps, 64 ms. It logs
  `frame_catch_up max_ticks_per_frame=4 max_delta_ms=64.0`.
- It runs only when `interactive = !headless && output.is_none()`, which means a window on the
  real clock. Android starts through the same `main`, so the phone gets it too.
- Unchanged:
  - `--headless`, `--render` captures (with `render_fixed_step` or `render_fps`) and the in-crate
    test apps, all of which step a ManualDuration;
  - grid-soak, grid-opponent-race and the identity runs, which never use Bevy time.
- Diagnostics (docs/SETTINGS.md):
  - `max_ticks_per_frame` (`GRID_MAX_TICKS_PER_FRAME`) sets N; 0 restores Bevy's 250 ms clamp.
  - `tick_delay_ms` (`GRID_TICK_DELAY_MS`) sleeps in every vehicle tick, as a forced slow tick.

What else read the 250 ms default [O]: nothing in grid-app.
- Menus, HUD timers, the pause menu, the results screens, touch controls, engine sound and the
  frame counter use `Time<Real>`.
- Presentation interpolation uses `Time<Fixed>::overstep`.
- Pause is `Driving::paused`, not `Time<Virtual>::pause`.
- Only damage shards and the static orbit viewer read the generic (virtual) `Time` in `Update`.
  They now slow down with the game, as the simulation does.
- A long frame after a load, or an Android resume, now advances at most 4 ticks (64 ms of race
  time) instead of 15. The countdown runs in ticks, so it can skip less than before, not more.

### Desktop measurement

Release `grid-app.exe`, windowed 1280 × 800, autopilot (`GRID_AUTOPILOT=1`), muted. Cap 0 (Bevy's
250 ms clamp, the behaviour before F544) against cap 4. One binary for both, switched with
`GRID_MAX_TICKS_PER_FRAME`. The evidence (scripts, logs) is private, in the worktree's
`local/frame-cap/`.

"Identical" compares every `drive_sample` tick line except `wall_seconds`.

| Scenario | FPS cap 0 → 4 | ticks/s cap 0 → 4 | worst frame ms | drive samples |
| --- | --- | --- | --- | --- |
| Practice, Lacetti/Donington, 3000 ticks | 146.8 → 146.2 | 62.7 → 62.7 | 10.1 → 9.2 | 189/189 identical |
| Same, forced 12 ms tick | 69.3 → 70.3 | 62.5 → 62.5 | 121 → 30 | 95/95 identical |
| Same, forced 20 ms tick | 3.1 → 11.6 | 47.0 → 46.6 (game 0.75 → 0.74 of wall time) | 357 → 106 | 95/95 identical |
| Drift Battle, Nürburgring `route_1`, 8 BMWs, 8000 ticks, whole run | 112.5 → 105.5 median | 61.8 → 61.7 | 700 → 167 | 7608/7608 identical |
| Same, last 40 s (pile of about 470–490 pieces, about 30 ms per tick) | 1.8 → 7.8 | 28.2 → 31.1 (game 0.45 → 0.50) | | |
| Same with a forced 10 ms tick, 6000 ticks, last 40 s | 2.3 → 8.4 | 36.3 → 33.2 | 481 → 178 | 5698/5698 identical |

Reading [D]:
- Fast frames are unchanged: 62.5 ticks/s and the same simulation.
- Above 16 ms per tick, the cap turns the collapse into a slower game at a 3–4× higher frame rate.
  The game speed is about the same as before. With Bevy's clamp it was already below real time,
  because 15.6 ticks of 20 ms do not fit in 250 ms.
- This matches the model of section 2. For example, a 20 ms tick with r of about 6 ms gives a frame
  of about 90 ms, 11 FPS.
- The desktop reproduces the phone's pile collapse at the end of the 8000-tick drift battle. The
  piece counts are derived from the cumulative `GRID_PERF_TIMING` means: about 470–490 pieces
  over the last 800–1500 ticks.
- Option 3 (a cheaper tick) is still what brings back 60 FPS.

## 6. Drawing between ticks (backlog row 116, F729)

Owner's report, 4 November 2026: "игра показывает, словно я играю в 80-100 фпс, но картинка
рваная, словно я играю в 20-30 фпс". The FPS counter showed 80–100, but motion looked like
20–30 FPS.

Cause [O, code and trace]:
- The simulation advances in 16 ms ticks (62.5 Hz).
- Before the fix, a frame drew the pose of the last tick as it was: the player's car, the
  opponents, the cockpit, the driver, the parts and the chase camera (the camera's `Clock::Tick`
  also steps per tick).
- Above 62.5 FPS, some frames had no new tick. The world stood still on those frames and moved a
  whole tick on the next one.
- At 80–100 FPS, one frame in four to six repeats the last picture, which is a beat at about
  15–25 Hz. At 128 FPS every other frame stood still.
- The original does not have this problem: it steps its world once per drawn frame by the real
  frame time (section 1).

Fix (`crates/grid-app/src/main.rs`; the same path serves Windows and Android):
- Interpolation with one tick of latency.
  - `drive_tick` keeps every car's pose before the tick (`ShownPoses`).
  - After the fixed loop, `shown_alpha` reads `Time<Fixed>::overstep_fraction()`.
  - `Driving::shown_pose(car)` lerps translation and slerps rotation between the two poses.
- The users of the car poses draw `shown_pose`:
  - the player and opponent cars, and the headlights in `drive_present`;
  - the mirror (`cockpit.rs`);
  - the drivers (`exterior_driver.rs`);
  - the damage parts (`damage_parts.rs`).
- The camera's output shot is blended the same way between the shots of the last two ticks
  (`ShownShots`). With k ticks in one frame, the earlier shot is taken (k−1)/k of the way from
  the last frame's shot.
- These are drawn at once, not blended:
  - a move of more than `ornament_lod::CUT_METRES` (a reset);
  - a view change;
  - a stopped simulation (pause, the end of a scripted run).
- The simulation itself is unchanged: physics, AI and sound still read the tick state, and wheel
  spin already used the overstep.
- Only interactive windows interpolate. Captures, `--headless` runs and tests step whole ticks and
  draw the last tick exactly as before.
- Switches:
  - `GRID_TICK_INTERPOLATION=0` draws the last tick, for comparisons;
  - `GRID_PRESENT_TRACE=1` prints one `present_frame` line per drawn frame: the real frame time,
    the tick, alpha, the drawn car position and the eye.

Measurement: Washington, 11 opponents, autopilot, a real window at its default size (muted), the same exe (4958d9bebb25), the switch
alternated over two rounds. The logs and `steps.py`/`plot.py` are in the worktree's
`local/judder/`, with `steps-wa.svg`.
- Per drawn frame, r is the car's step divided by its expected step (its speed over the
  neighbouring 30 frames × the frame time), over frames above 5 m/s.
- The window ran at 128 FPS: mean frame 7.8 ms, p95 8.6 ms.

| Run | r mean | r sd | frames with r < 0.25 | frames with r > 1.75 | car–eye offset change per frame, sd |
| --- | --- | --- | --- | --- | --- |
| wa, off | 1.020 | 1.026 | 49.8 % | 1075 / 2293 | 1.45 cm |
| wa, on | 0.997 | 0.081 | 0.5 % | 2 / 2261 | 0.89 cm |
| wb, off | 1.022 | 1.008 | 48.6 % | 1019 / 2243 | 1.45 cm |
| wb, on | 0.997 | 0.081 | 0.5 % | 1 / 2288 | 0.87 cm |

The camera's eye gives the same figures as the car. The remaining 0.5 % are frames of the
1–2 frame-time spikes. An earlier pair under heavier load (about 55 FPS, `win-0`/`win-1`) gave
r sd 0.317 → 0.083.

How it should look:
- Before: the car and the scenery move in an even rhythm only when the frame rate is a multiple of
  62.5. Otherwise, the motion stutters in a regular beat, most visibly in the scenery passing at
  the sides and in the opponents.
- After: every drawn frame moves by its own frame time, so 80–100 FPS look like 80–100 FPS.
- The picture is one tick (16 ms) behind the simulation. The steering input is still read every
  tick.

Not changed:
- The present mode is the player's choice in the video menu (Fifo, AutoVsync, AutoNoVsync).
- AutoNoVsync can tear on a fixed-rate monitor; that is a different artefact from this judder.

## 7. Resolution, views, frame rates and Android (F730)

Follow-up of 4 November 2026 (coordinator, owner's words):
- "на моём ПК проблему видно при игре в максимальном разрешении, то есть в 2К в моём случае. В 4К
  будет ещё заметнее, вероятно. Ниже 2К не заметно фризов и рваной картинки".
- The owner asked for a thorough check: all views, several tracks, frame rates above and below
  62.5 Hz, desktop and Android.

### Why the resolution mattered

[V] The owner's playtest logs (`local/playtest/menu-*.log`, prototype e66e1c24963c) show the
setup:
- monitor 165 Hz, `present_mode = auto_no_vsync`, `render_height = native`;
- Baseline preset, MSAA ×4, DLAA not active;
- the window maximised to 2560 × 1369.

| Session | Window | Race seconds | FPS p10 / median / p90 | seconds at 50–75 FPS |
| --- | --- | --- | --- | --- |
| menu-32112-19856 | 2560 × 1369 | 175 | 56.9 / 69.3 / 97.6 | 55 % |
| menu-31016-11482 | 1280 × 800 | 266 | 132.7 / 154.5 / 162.5 | 2 % |
| menu-9456-17359 | 1280 × 800 | 115 | 87.9 / 100.8 / 136.9 | 1 % |
| menu-32762-29455 | 1280 × 800 | 555 | 60.0 / 85.9 / 149.6 | 47 % |

[D] How the frame rate decides what the tick-stepped drawing (before F729) looks like:
- At about 150 FPS, each tick stays on screen for 2 or 3 frames: 62.5 Hz motion with ±3 ms
  jitter, which reads as fairly even.
- Close to 62.5 FPS, most frames get one tick. But the frame times vary by ±20 %, so some frames
  get none (the picture repeats) and others get two (a double step). At a 69 FPS median that is
  several hitches a second.
- From 70 to 100 FPS, a tick stays for one frame or for two, irregularly.

At 2K the owner's frame rate sat in that band, at 1280 × 800 mostly above it. Nothing else in the
renderer depends on the window size:
- the render scale stayed native, with two `render_scale` events: the start and the maximise;
- there is no dynamic resolution on the desktop.

On this PC the research build reached about 100 FPS even at 3840 × 2160 (the main world is the
limit: 9–10 ms per frame at every size). So the resolution sweep below shows the same judder at
every size, and the frame-rate rows reproduce the owner's band. [I] The owner's 2K sessions ran
slower than these autopilot runs, probably from manual driving (crashes, debris) and other
sessions' builds loading the PC. The mechanism does not depend on why the frame rate drops.

What the FPS counter measures [V code, trace]:
- `frame_stats` averages the main world's `Time<Real>` frame times.
- The new `present_done` line marks each frame in the render world, after Bevy handed it to the
  swap chain (`RenderSystems::Cleanup`, after `render_system`'s present).
- In every run, the present intervals have the same mean, p99 and slow-frame share as the main
  world's frames (table below). So the counter is the presented rate: no frame is dropped and none
  comes in bursts at the swap chain.
- [N] Display-side timing (PresentMon or ETW) was not measured. It needs an elevated process or
  the Performance Log Users group, and this session has neither.
- In a window on a 165 Hz display, the compositor shows each frame for a whole number of refreshes
  (6.1 ms steps). That quantisation is the same with or without interpolation.

Queue and pacing:
- `frame_latency` is unset (wgpu's 2) and was not changed.
- Slow frames (over 1.5 × the median present interval) are about 1 % of the frames and come in
  clusters, with gaps of 1–2 frames (`spikes.py`).
- The autocorrelation of the present intervals falls smoothly with the lag, with no peak at a
  fixed period. So there is no periodic spike; the frame time drifts slowly with the scene's load.

### More of the frame drawn between ticks

Besides the F729 users:
- **Damage parts** (`damage_parts.rs`):
  - detached panels (their rigid body, or the AUTHORED flight without one);
  - lost wheels;
  - the swing angle of hinged loose parts.

  These stepped per tick and led the drawn car by one tick. `tick` now keeps each part's state
  from before every fixed step (`Part::before`, `LostWheel::before`), and `present` draws between
  that and the current state by alpha. Glass shards already spawned from the drawn pose and fly
  on the frame clock.
- **Minimap dots and the OSD map markers:** `race_hud::car_pose` returns the drawn pose.
- **The car reflection centre:** `car_reflections.rs`.
- **The live pose that replay uses to place the damage parts:** `replay.rs`.
- **The camera hold:** `Driving::frozen()` compared the player's pose with its pose a tick
  earlier, so a car standing still with the ticks running stopped the camera blend. The blend now
  holds only when a fixed step ran without a tick (`ShownPoses::steps`): pause, race over, a
  scripted end.

Left on the tick, on purpose:
- the HUD's wrong-way test, sound, force feedback and the simulation readers;
- the wheel's suspension travel and steer angle in the car's frame (one tick ahead of the body,
  sub-millimetre);
- the wheel spin phase, which already follows the overstep and is continuous from frame to frame.

Switches:
- `[diagnostics] tick_interpolation = off` (`GRID_TICK_INTERPOLATION=0`) draws the last tick.
- `[diagnostics] present_trace = on` (`GRID_PRESENT_TRACE=1`) prints `present_frame` and
  `present_done` (`frame_pacing.rs`). Both keys work on a phone through its settings file.
- `GRID_AUDIO=off` opens no audio device in a window, for silent research runs.

### Measurement matrix

Release exe 5705cb3e2224 (research build without `dlaa`/`paint-specular`), autopilot, 11
opponents, silent (`GRID_AUDIO=off`). Each run lasted 75 s and was analysed from 2 s after the
start. The view was set through `camera.ini`. Frame caps used `GRID_FRAME_CAP_HZ`.

Scripts: `local/judder/matrix.sh`, `plan.txt` and `pace.py`. Logs: `local/judder/m/`, summary
`pace.tsv`. Plots: `steps-1440.svg`, `steps-70.svg` and `steps-bumper.svg`.

Columns:
- car sd: the spread of each frame's car step against its expected step (speed × present
  interval);
- still: frames moving less than 25 % of the expected step;
- turn sd: the same as car sd for the camera's rotation, over turns faster than 0.5 rad/s;
- eye in car: the frame-to-frame change of the eye in the car's frame.

Off is `tick_interpolation = off`; on is the default.

| Run | Window, rate | Present mean / p99 ms | car sd off → on | still off → on | turn sd off → on |
| --- | --- | --- | --- | --- | --- |
| A, two rounds | 1280 × 800, uncapped | 9.1–9.4 / 13.3–14.4 | 0.873, 0.911 → 0.094, 0.100 | 40.7, 42.4 % → 0 | 0.853, 0.874 → 0.097, 0.107 |
| A | 1920 × 1080 | 8.9–9.8 / 13.4–14.4 | 0.874, 0.921 → 0.111, 0.097 | 40.4, 43.2 % → 0 | 0.837, 0.881 → 0.158, 0.137 |
| A (owner's window) | 2560 × 1369 | 9.1–9.6 / 14.0–14.3 | 0.845, 0.906 → 0.105, 0.087 | 38.8, 42.3 % → 0 | 0.784, 0.869 → 0.117, 0.101 |
| A | 3840 × 2160 | 9.6–10.0 / 14.3–14.7 | 0.817, 0.847 → 0.121, 0.120 | 37.6, 39.7 % → 0 | 0.791, 0.821 → 0.145, 0.123 |
| B | 2560 × 1369, Fifo (165 Hz) | 9.4–9.5 / 14.8 | 0.883 → 0.095 | 40.3 % → 0 | 0.824 → 0.111 |
| B | cap 144 | 9.2–9.3 / 14.3–15.0 | 0.905 → 0.106 | 41.8 % → 0 | 0.861 → 0.109 |
| B | cap 100 | 10.7 / 14.6–14.9 | 0.726 → 0.118 | 32.8 % → 0 | 0.725 → 0.132 |
| B | cap 70 | 14.6 / 16.5 | 0.352 → 0.112 | 8.8 % → 0 | 0.419 → 0.131 |
| B | cap 65 | 15.7 / 17.4 | 0.197 → 0.133 | 2.3 % → 0 | 0.206 → 0.098 |
| B | cap 60 (2 ticks on 6.5 % of frames) | 17.0 / 19.5 | 0.270 → 0.146 | 0.2 % → 0 | 0.330 → 0.239 |
| B, phone-like | 1920 × 1080, cap 45 | 22.6 / 25.5 | 0.374 → 0.136 | 0.1 % → 0 | 0.413 → 0.122 |
| B, phone-like | cap 30 | 33.7 / 37.3 | 0.198 → 0.104 | 0.1 % → 0 | 0.271 → 0.196 |

Tracks and views, 2560 × 1369, uncapped, interpolation on (off in brackets where measured):

| Track | View | car sd | still | turn sd | eye in car, sd |
| --- | --- | --- | --- | --- | --- |
| Washington | chase_close | 0.098 | 0 | 0.114 | 0.96 cm |
| Washington | head-cam | 0.085 (0.678) | 0 (29.3 %) | 0.247 (0.748) | 0.07 cm |
| Washington | bumper | 0.092 (0.963) | 0 (45.5 %) | 0.199 (0.964) | 0.05 cm |
| Spa | chase_close | 0.089 | 0 | 0.090 | 0.61 cm |
| Spa | head-cam | 0.068 (0.509) | 0 (18.9 %) | 0.110 (0.647) | 0.06 cm |
| Spa | bumper | 0.084 (0.784) | 0 (35.2 %) | 0.105 (0.876) | 0.05 cm |
| Shibuya | chase_close | 0.120 | 0 | 0.132 | 0.82 cm |
| Shibuya | head-cam | 0.102 | 0 | 0.121 | 0.05 cm |
| Shibuya | bumper | 0.088 | 0 | 0.106 | 0.05 cm |
| Le Mans | chase_close | 0.082 | 0 | 0.086 | 0.57 cm |
| Le Mans | head-cam | 0.083 | 0 | 0.193 | 0.06 cm |
| Le Mans | bumper | 0.081 | 0 | 0.112 | 0.04 cm |

Reading:
- With interpolation, every combination moves on every frame (no frame stands still), and the
  step spread is 0.07–0.15. The remainder is timing noise: the main world samples the clock at the
  start of a frame and the render world presents one frame later. The drawn simulation time per
  present interval has the same spread (`content sd` in `pace.tsv`).
- Without it, the spread is largest where frames often get no tick (up to 45 % standing still,
  worst in the bumper view, which shows the road closest). Below 62.5 FPS the failure is the
  opposite one, double steps. The phone's 40–50 FPS sits there.
- In the head-cam and bumper views, the eye moves 0.04–0.07 cm per frame in the car's frame. The
  camera and the drawn car or cockpit are blended consistently.
- The head-cam turn spread (0.11–0.25) is higher than the chase view's. It comes from the
  head-cam's own springs (head shake), not from ticks: with interpolation off it is 0.65–0.75.

### Android

[V code, phone log `local/android/phone-logs/20261022-0446`]:
- The phone starts through the same `main`, and its log has `frame_catch_up max_ticks_per_frame=4`.
  That line is printed only on the interactive branch, which also installs `shown_alpha` and the
  trace.
- The phone ran Fifo at 60 Hz (`display_hz=Some(60.000004)`) at 40–50 FPS, below the tick rate: the
  double-step case of the cap 45 row.

[N] Not measured on a phone: none was attached, and no Android emulator is installed. The APK is
arm64, so an x86 emulator would not run it representatively. A phone trace needs `[diagnostics]
present_trace = on` in the phone's `settings.ini`; `pace.py` reads the log file the app writes to
Downloads. The Android Clippy (`tools/build_android.ps1 -Clippy`) passes.

### The original

[V docs/BRAKING_REFERENCE.md, F419]:
- The shipped config has `vsync="0"` and `maxfps` 10000.
- The original steps its world once per drawn frame by the real frame time, and its unlocked
  captures on this PC ran at 57–164 Hz.

So it moves every frame at any rate, without vsync, like the port's interpolated drawing with the
owner's `auto_no_vsync`.

### How it should look

At 2K (or any size or rate):
- the cars, the scenery and the camera move by the same amount on each frame;
- the "20–30 FPS" stutter of ticks held for one or two frames is gone;
- in the head-cam and bumper views, the road and the scenery no longer shake;
- the debris of a crash flies smoothly beside its car instead of stepping ahead of it.

The picture stays 16 ms behind the simulation.

Not covered:
- display-side timing (PresentMon);
- variable refresh rate displays;
- a phone measurement;
- the prototype's `dlaa` build (DLAA was not active in the owner's session).

## Symbols

New or updated rows in `docs/re/symbols.tsv`: `5E7F30` (physics task run), `CE34EC` (its
vtable), `A72930` (world scheduler, re-read), `87E2D0`/`87E250` (task registration),
`632B10`/`5EB6A0` (task registrations on the game-object scheduler at `+0x20`), `B95CC0`/`A6BBF0`
(clocks), `9C0F70` (motion-platform UDP output, not the frame clock).
