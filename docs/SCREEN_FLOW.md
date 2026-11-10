# Screen flow: transitions, loading overlay and the menu-race loop

How DRIG moves between the front-end screens and the race the way the original does: the overlay fade, the loading overlay, the process hand-off, and the launcher race's return to the event menu. Code: `crates/grid-app/src/transition.rs` (shared framework), `event_menu.rs` (supervisor, menu), `main.rs` (race window), `pause_menu.rs` and `post_race.rs` (leaving the event). Smoke: `tools/menu_loop_smoke.ps1`. Backlog rows 63, 62 and 69 (docs/FIDELITY_BACKLOG.md).

Tags: [D] read from the original's data or code (static only; grid.exe was not run), [V] verified in DRIG, [I] inference, AUTHORED our own choice.

## 1. The original

### 1.1 One black layer over every screen [D]

`fePersistentDatabase.pssg` (the front end's persistent scene, 236 roots) holds root `overlay` (AnimSet!141), which hosts the help bar, popups, the `load_overlay` component (`loading`), the chat and the `overlay_fade` component. `overlay_fade` (AnimSet!148) is a single RENDERNODE `{1}null=black_fade`: one full-screen black quad.

Its alpha channel is `Anim!169`, a BEZIERFLOAT with keys frame 0 → 0 and frame 0.6667 → 1 and control points at one and two thirds: a smoothstep. The FEANIMDATA of the fade (`defaultFrame=0.6667`) names the events:

| Event | Target frame | Duration |
|---|---|---|
| `fade_to_black` | 0.8333 (black held) | 1.0 s |
| `fade_to_clear` | 0 | 1.0 s |
| `fade_to_black_quick`, `_quickest` | 0.8333 | 0.5 s |
| `fade_to_clear_quick`, `_quickest` | 0 | 0.5 s |
| `fade_to_black_instant`, `fade_to_clear_instant` | as above | 0 |
| `fade_to_half_black_quick` | (half) | 0.5 s |

### 1.2 Who plays it [D]

- `FUN_004C6DB0` is the front end's overlay-fade state. Its `moveto`, `to_world` and `to_black` entries post the overlay_fade events to the glyph; `states.xml` names the speeds (`anim_suffix`).
- Going into the race: the screen goes to black with `fade_to_black_quickest` (0.5 s) on Go; the race starts black and plays `overlay_fade_off` (fade to clear, 1.0 s) when the world is ready.
- Leaving the race (pause Exit/Retire, results Continue): `overlay_fade_on_hh`, fade to black in 1.0 s, then the front end loads.
- `FUN_004A4560` handles the overlay's event-finished messages. When `border_off` finishes with the loading flag set, it plays the loading component's `fade_on` and `circle_rotate`: "Loading, please wait" (language id `loading`) with a turning circle, on black, bottom right.
- The circle is drawn by the procedural shader `feov_ramp.fx`, not a texture. DRIG's ring is therefore AUTHORED in look (orange arc, 0.9 s per turn).

### 1.3 Not ported (yet)

- The 3D loading world (`loading` root, track preview and `loading_stats` tips/facts), and the top and bottom bar shutters of `overlay`.
- Front-end sounds on transitions.
- `fade_to_half_black_quick` (popups).

## 2. DRIG

### 2.1 The fade resource

`transition::Fade` is one resource per app: from/to frame, duration, elapsed time, the loading flag, and what to do when black (`Next::Close` or `Next::Code(exit code)`). `transition::install(app, loading_text, Start)` adds the overlay (black node with `GlobalZIndex(i32::MAX)`; the pipeline warm-up text sits at `i32::MAX - 1`, under it) and its systems:

- `Start::Race`: the race opens black with "Loading" and fades to clear (1.0 s, `overlay_fade_off`) once the pipeline warm-up reports ready.
- `Start::Screen`: a menu opens black and fades to clear (1.0 s).
- `Fade::leave(next, speed, reason)`: fade to black, then show "Loading", then exit with `next`. Repeated calls are ignored, and screens stop reading input once leaving.
- `transition::Leave` (a SystemParam) is the race's way out: pause Exit/Retire and results Continue call `leave.event()`. A race from a menu exits with BACK_TO_MENU (21); otherwise the app closes. Without a `Fade` (headless, captures) it leaves at once as before.
- The frame step is capped at 0.05 s, so a load stall does not skip the fade.
- A window that fades (the menu, a windowed race) opens hidden and shows at its third frame, already black (`transition::window`). Before, the first frames showed the system's blank white window for about a second (matched captures in the F756 entry).

Every transition logs `screen_transition version=1 event=… seconds=… reason=…` (`race_start`, `screen_open`, `menu_go`, `menu_quit`, `leave_event`), and then `loading_on`, `handoff code=…`, `handoff_ready` and `exit next=…`.

### 2.2 The hand-off between processes

winit allows one event loop per process, so each screen is its own process (event_menu.rs). The supervisor `transition::run_supervised` removes the desktop gap between them:

1. Every child gets `GRID_HANDOFF_GO` and `GRID_HANDOFF_READY`, two marker paths in the temp folder named after the supervisor's PID.
2. A leaving child, once black with "Loading", writes the go marker: its exit code and window position (`20 x y`). It then waits, still black, for the ready marker (at most 120 s).
3. The supervisor starts the next child with `GRID_HANDOFF_SHOW` and `GRID_WINDOW_AT=x,y`. Its window opens hidden at the same place (`transition::window`), shows and takes focus at frame 3, and writes the ready marker at frame 4. The first frame shown is black because the overlay starts black.
4. The old child exits; the new one fades in. The leaving process calls `AllowSetForegroundWindow` so the new window may take focus.

A child that exits without a marker (a crash, a close) returns its code to the supervisor as before. If the new child dies during the hand-off, the old one is killed and the code is passed on.

Android keeps `android::relaunch()` (one activity, its own markers); `exit_with` routes `Next::Code` there.

### 2.3 Launcher races return to the menu

A launcher race (`Play GRID Prototype.cmd`, the `race-*` launchers) used to close the app on results Continue. A windowed race that was not started from the menu or the career, and whose profile does not set `[front_end] event_menu = off`, now behaves like a menu race. The way out sends it back to the menu: `run` drops the race app, then `main` opens `event_menu::supervise` in the same process with the launcher's flags minus its `--settings`. This first step has no hand-off (the race window closes before the menu opens), so a short gap shows. Scripted runs (`--drive-ticks`) keep closing unless `GRID_MENU_AUTO` is set, so captures and the build gate are unchanged.

### 2.4 Career

Joined in F771 (backlog row 60, docs/CAREER_SCREEN.md "Processes"). `career_screen::supervise` runs its children through `run_supervised`; the career screen calls `transition::install(…, Start::Screen)` and `transition::window`, and leaves with `Fade::leave` (`career_go`, code 22; `career_quit`, Close). A career race hands back with 21 like a menu race and writes its result when that fade starts. `Fade::next()` (added for this) tells a screen which way it is leaving. `tools/career_loop_smoke.ps1` checks the loop screen → race → screen with both hand-offs.

## 3. Race Options: Difficulty and Grid Position

### 3.1 The original [D]

`states.xml` `f_m_event_options` (`ScreenPreEventOptions`) has the itemflow difficulty, gridpos, realisticdam, promode, headcam, driveraids, race. realisticdam, promode and headcam carry `IBVisibilityControl visible=false`, so the player sees four items:

| Item | Values | Default |
|---|---|---|
| Difficulty (`event_options_difficulty_setting_title`) | `difficulty_1..5`: Basic, Normal, Serious, Savage, Extreme; `difficulty_1..6` (adds Ninja) when `unlockedDifficulty` | profile |
| Grid Position (`event_options_grid_position_title`, `IBGridPositionSelect`) | Front, Middle, Back, Random, pushed in that order at 0x5035D0 | `default_index=3`: Random |
| Driver Assists | the Driver Aids screen | |
| Go To Race | | |

The free race reads the grid option at 0x5D7630 → 0x5B60F0 ([V], docs/SERIES.md "Grid"): option 1 puts the player at slot n − 1, 2 at n / 2, 3 at `rng(0, n) % n`, anything else at 0; the other cars fill the free slots in order. The Le Mans grid (0x5BDF10, `series::lemans_grid`) uses the same values within the player's class group. The menu index → option value mapping by name (Front 0, Middle 2, Back 1, Random 3) is [I]: the list order is front, middle, back, random, while the option values name back = 1 and middle = 2.

The difficulty's star level sets the AI's ratio `flashback::AI_RATIOS` (0.506 … 1.0) and the flashback count.

### 3.2 DRIG

- The event menu shows Difficulty and Grid Position after the AUTHORED rows, before Continue (`event_menu.rs`). AUTHORED placement: the original asks on the Race Options screen after the event screen, but DRIG builds the grid before the race process opens its front end, so the choice is made one screen earlier. Labels come from the language file.
- Difficulty is offered for every event with AI (not Time Trial), levels Basic … Extreme. Ninja stays locked: its unlock (`unlockedDifficulty`) is not ported. The profile writes `[ai] difficulty = AI_RATIOS[level]`; Normal (0.606) is the launchers' value.
- Grid Position is offered for the events with a grid of the player's choice: race, series, drift battle, Le Mans and demolition derby. A touge pair keeps the player behind (`grid_position = back`).
- `[drive] grid_position` takes `front`, `middle`, `back`, `random` or a slot number (`settings::GridOption`). The race places the option with the 0x5B60F0 rule once the field is known; Random draws from the series seed (`GRID_SERIES_SEED`, `[game] series_seed`, else the clock). The Le Mans grid gets the same option value.
- The choice is saved in `[event] difficulty` and `[event] grid` and carries over between event types.
- In the race, Race Options keeps DRIG's items: Performance test (diagnostics), Driver Assists, Video and Go To Race (docs/DRIVER_AIDS.md).

## 4. Checks

- Unit tests in `transition.rs`: the curve, the durations, the frame cap, repeated leaves and marker parsing.
- Unit tests in `event_menu.rs` and `settings.rs`: the Race Options defaults, order, Ninja lock, saved keys, the profile's AI ratio and grid value, and the 0x5B60F0 slots.
- `tools/menu_loop_smoke.ps1` drives the real path in interactive windows: launcher race → results → menu → Go (hand-off) → race → results → menu (hand-off) → quit, exit 0.
- The build gate's `menu-default` smoke still covers the menu → race start.
