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
- The circle is drawn by the procedural shader `feov_ramp.fx`, not a texture. Its colours and ramp keys are data (§1.4); its mesh is not reachable, so DRIG's ring geometry is AUTHORED.

### 1.3 Not ported (yet)

- The 3D loading world's stat panel (`loading_stats`), its reflections and camera noise. The world itself is drawn since F784 (`fe_world.rs`, docs/FRONT_END_FLOW.md §2). `language_eng.lng` has no tip or hint strings (90 ids with load/tip/hint, all load/save or network messages), so `loading_stats` shows facts of the event, not tips [I].
- `fade_to_half_black_quick` (popups).
- The bars in the race: there the original slides them off only at the intro to in-game camera hand-over (`FUN_0058BAF0` posts `border_off`); DRIG has no race intro, so the race shows none. Scoped in F784: the bars belong to the intro camera (`FUN_0058BAF0` hands the intro camera to the in-game camera, then posts `border_off`) [D]; bars without that intro would only cover the start for 0.3 s with nothing to hand over, so they wait for an intro port.

### 1.4 The bars and the loading circle [D unless marked]

Read from `frontend/fePersistentDatabase.pssg` with `grid-fe-anim` (crates/grid-probes); evidence in the main checkout's `local/front-end-flow/loading/overlay-anim.txt` (private).

- **Bars.** `overlay` has a top and a bottom black quad, 200 units deep. The overlay camera looks at y 307.908 with a half-height of 475.861. The bars' Y keys: top (0, 852.123), (0.3333, 882.123), (0.6667, 1002.015); bottom (0, -26.122), (0.3333, -59.122), (0.6667, -172.521). As shares of the window height (top, bottom): frame 0 → (0.1383, 0.1490), frame 0.3333 → (0.1068, 0.1144), frame 0.6667 → (0, 0). `border_on` and `border_off` play 0.3 s between frame 0 and the off frame; `FUN_004964C0` posts `border_on` and `FUN_00496770` posts `border_off` to the overlay.
- The overlay's setup (`FUN_004A49E0`) posts `border_on`, so a front-end screen opens with the bars on. The button help (`helptext`) is a child of the bottom bar [I: the front-end screens show the bars].
- `FUN_004A4560`: when `border_off` finishes with the loading flag set, the loading text and circle come on. So the bars slide off under black, then "Loading" shows.
- **Circle.** `load_circle_anim` (`Anim!164`, 3.333 s, played by `circle_rotate`, looped [I]) animates the ring material's `RampMin` (0, 0), (1.6667, 0), (3.3333, 1.2) and `RampMax` (0, -0.2), (1.6667, 1.0); `RampSoftness` 0.2. The ring material (`feov_ramp.fx`) is grey 0.65 at alpha 0.5; the centre disk (`load_circle_disk`, `feov_colour.fx`) is yellow (0.9272, 0.8058, 0.0312) at alpha 0.5.
- [I] The ramp: alpha share = clamp((max - u)/soft + 1) × clamp((u - min)/soft + 1) along the ring coordinate u. The keys fit it exactly: empty at 0 and 3.333 s, full at 1.667 s. The shader itself is not decoded.
- The ring mesh sits in a component instance library the scene reader does not reach. DRIG's ring size (28 px, 5 px wide, 10 px disk) and its place (bottom right, beside the text) are AUTHORED.

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

The window is shown in `PostUpdate` (`transition::add_window_show`), never in `Last`. bevy_winit applies `Window` changes in `Last` (`changed_windows`), and on Windows it updates a visible window only on `RedrawRequested`, which a window that winit still keeps hidden never gets. If the `visible = true` lands after `changed_windows` in the same `Last`, the app never runs another update. Bevy believes the window is visible, winit keeps it hidden, and the main thread spins in Poll (`EnumDisplayMonitors` and `RedrawWindow`). Adding any system to `Last` could reorder the two. Two cases: an F773 draft that played front-end sounds from `Last` hung the menu screen in 3 of 3 runs (0 of 3 without it; `tools/stack_sample.ps1` sampled the main thread's stack), and career-ui found that `GRID_WINDOW_SHOTS` (`frame_pacing::window_shots`, also `Last`) stopped `--career-screen` and `--menu-screen` after the window was created. `tools/menu_loop_smoke.ps1` saves a window shot on every screen to keep this covered.

A child that exits without a marker (a crash, a close) returns its code to the supervisor as before. If the new child dies during the hand-off, the old one is killed and the code is passed on.

Android keeps `android::relaunch()` (one activity, its own markers); `exit_with` routes `Next::Code` there.

### 2.3 Launcher races return to the menu

A launcher race (`Play GRID Prototype.cmd`, the `race-*` launchers) used to close the app on results Continue. A windowed race that was not started from the menu or the career, and whose profile does not set `[front_end] event_menu = off`, now behaves like a menu race. The way out sends it back to the menu with the hand-off: the race has `transition::SelfHandoff`, so once black with "Loading" it starts `grid-app --menu` itself (`event_menu::spawn_menu`, the launcher's flags minus its `--settings`, `GRID_WINDOW_AT` and `GRID_HANDOFF_OUTER_READY`). The menu opens hidden at the race's window place and shows on its first frames; the race exits when the menu shows. A hand-off passes a process's marker on to the next one, because the leaving process waits for its own `GRID_HANDOFF_READY`: in the first F773 smoke the menu's first Go waited the 120 s limit while the race reported to another file (test `a_hand_off_passes_on_the_marker_the_leaving_screen_waits_for`). `main` waits for the menu process and returns its code, so a launcher that waits for the process still sees the whole session. A race that leaves without the fade still opens the menu in the same process afterwards (`event_menu::menu_after`). Scripted runs (`--drive-ticks`) keep closing unless `GRID_MENU_AUTO` is set, so captures and the build gate are unchanged.

Both front ends keep this contract (`transition::OuterReady`, `transition::window_at`):
- the one-window host (`front_end_host::run`, FRONT_END_FLOW.md, the default since F767): with `GRID_HANDOFF_OUTER_READY` set, its window opens hidden at `GRID_WINDOW_AT`, shows (and takes the focus) after the shown screen's third update (`transition::SHOW_FRAME`), and writes the marker on the next update (`front_end version=1 event=handoff_ready`). Showing happens in the outer app's `Update`, before bevy_winit applies window changes in `Last` (the F773 window-show hang). If the host ends before it shows, dropping `OuterReady` writes the marker, so the race does not wait the full 120 s. Before this, F773 made `spawn_menu` ask for the supervisor with `GRID_FRONT_END=processes`; that is no longer set, so the owner's launcher races now go back to the same one-window menu as the menu's own races;
- the supervisor (`GRID_FRONT_END=processes`, inherited from the environment): starts its first menu hidden with the race's ready marker (`transition::run_supervised_from`) and writes the marker itself if that menu dies before showing.

### 2.4 Career

Joined in F771 (backlog row 60, docs/CAREER_SCREEN.md "Processes"). `career_screen::supervise` runs its children through `run_supervised`; the career screen calls `transition::install(…, Start::Screen)` and `transition::window`, and leaves with `Fade::leave` (`career_go`, code 22; `career_quit`, Close). A career race hands back with 21 like a menu race and writes its result when that fade starts. `Fade::next()` (added for this) tells a screen which way it is leaving. `tools/career_loop_smoke.ps1` checks the loop screen → race → screen with both hand-offs.

### 2.5 Screen and widget sounds

A front-end screen app calls `hud_sound::install_front_end` after `transition::install`. It plays the original's plain `fe_fx` effects on the HUD group: `screen_whoosh_in` on the first frame, `screen_whoosh_out` once `Fade::leaving()` is true, and the menu's widget sounds (move, value step, dead end, activate, back). The pause menu and the pre-race screens play the race's `low_` set. The sounds hook only reads `Fade`; the transition code is unchanged. Details: AUDIO_RESEARCH.md §36.

The loading overlay's sounds (`hud_sound::loading_cues`) follow `Fade::loading`: when "Loading" comes up over the black, `loading_text_in` and `spinner` (−6.4 and −7.4 dB in `fe_fx`); when the screen starts to clear, `loading_text_out` (−9.0 dB). A race plays the `low_` text cues (−3.4 / −5.8 dB); its set has no spinner [D]. The state a screen opens with plays nothing, because a screen that opens on "Loading" after a hand-off continues the overlay the previous process showed. In the original the overlay's animation posts these sounds; the exact frame inside the fade is not traced [I].

### 2.6 The bars and the loading circle

- `transition::borders(app)` (after `transition::install`) gives a screen the overlay's bars (§1.4): two black nodes over the screen, under the fade, at the frame-0 shares. When the screen leaves, the fade goes black, then the bars slide off in 0.3 s (`screen_transition version=1 event=border_off seconds=0.30`), then "Loading" shows, as `FUN_004A4560` orders it. The event menu calls it; another front-end screen (the career screens) opts in with the same line.
- The race has no bars (§1.3).
- The circle: a 56 x 56 ring texture redrawn each loading frame (`transition::ring_texels`, anti-aliased edges), whose alpha follows the ramp of §1.4 (`transition::ramp_share`) clockwise from the top (direction and start AUTHORED: the ramp coordinate lives in the unread ring mesh), from empty when "Loading" shows, looped every 3.333 s, around the yellow disk.

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

- Unit tests in `transition.rs`: the curve, the durations, the frame cap, repeated leaves, marker parsing, the marker a hand-off passes on, the window shown before `Last`, the bars' shares, the ring filling and emptying, and "Loading" waiting for the bars.
- Unit tests in `event_menu.rs` and `settings.rs`: the Race Options defaults, order, Ninja lock, saved keys, the profile's AI ratio and grid value, and the 0x5B60F0 slots.
- `tools/menu_loop_smoke.ps1` drives the real path in interactive windows: launcher race → results → (hand-off) menu → Go → race → results → menu → quit, exit 0, with the menu's sound source and a window shot from every screen. `-FrontEnd host` (the default) checks the one-window host the owner plays: the hand-off into it (`event=handoff_ready`) and its `attach`/`exit` screen changes. `-FrontEnd processes` checks the per-screen hand-offs. `-CheckOnly` re-checks an earlier run's log.
- `hud_sound` unit test: the loading cues sound on the overlay's edges only.
- The build gate's `menu-default` smoke still covers the menu → race start.
