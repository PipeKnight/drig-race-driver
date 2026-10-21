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

## Symbols

New or updated rows in `docs/re/symbols.tsv`: `5E7F30` (physics task run), `CE34EC` (its
vtable), `A72930` (world scheduler, re-read), `87E2D0`/`87E250` (task registration),
`632B10`/`5EB6A0` (task registrations on the game-object scheduler at `+0x20`), `B95CC0`/`A6BBF0`
(clocks), `9C0F70` (motion-platform UDP output, not the frame clock).
