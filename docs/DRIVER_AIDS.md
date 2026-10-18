# Driver Aids screen and graded aids

18 October 2026, Claude Code. The front-end Driver Aids options screen of the playable app and the
port's graded Traction Control and Braking Assist. Code: `crates/grid-app/src/front_end.rs`,
`crates/grid-app/src/settings.rs` (`with_driver_aids`, `save_driver_aids`),
`crates/grid-world/src/driver_aids.rs`, `crates/grid-tools/src/source_vehicle/assists.rs`
(`Level`). The aids themselves: [DRIVING_ASSISTS_RESEARCH.md](DRIVING_ASSISTS_RESEARCH.md).

Markers: **[D]** read from shipped data, **[V]** verified by a test or run, **[I]**
interpretation, **AUTHORED** the port's own choice.

## 1. The original's screens

From the decoded `system/states.bin` and `system/flow.bin` (main checkout `local/pause-menu/`) and
`language_eng.lng` [D]:

- `ScreenOptionsDriverAids` (`driveraids_optionsscreen`, glyph `options_driver_aids`; the event
  variants `do_…` and `h2h_…` are the same screen):
  - items in this order, `itemflow wrapV="true"`: `traction`, `gearbox`, `brake`, `steering`
    (`surface` is listed but has no item, docs/DRIVING_ASSISTS_RESEARCH.md 1);
  - each an `IBComboTextSimple wrap="true"`: `on_off_off`/`on_off_on` for `driveraid_traction`,
    `driveraid_brake`, `driveraid_steering`; `transmission_automatic`/`transmission_manual` for
    `transmission`;
  - titles `driver_aids_traction_control_toggle_title` (Traction Control),
    `driver_aids_transmission_control_toggle_title` (Transmission),
    `driver_aids_braking_assist_toggle_title` (Braking Assist),
    `driver_aids_steering_assist_toggle_title` (Stability Control); help texts
    (`IBHelpInfo`) for all but the gearbox; title `driver_aids_screen_title`;
  - overlay: Back (`default_buttontext_back`) and Button3 = Undo (`default_buttontext_undo`).
- Reached from the front end only:
  - Options → Driving Options (`optionsdriving_standard`: Driver Aids, Controller Setup);
  - the pre-race Race Options (`ScreenPreEventOptions event_options`, items difficulty, gridpos,
    realisticdam, promode, headcam, driveraids, race; `active_default="race"`): Driver Assists
    (`event_options_driver_aids_button_title`) and Go To Race
    (`event_options_exit_to_race_button_title`).
- The in-race pause menu's Options are Sound, Screen Setup and Advanced only
  (docs/PAUSE_MENU.md), so the aids cannot change in a race.
- The choice is applied to the player's car at race set-up (FUN_00654C20, gearbox mode 2 or 4 by
  FUN_00606E80, docs/DRIVING_ASSISTS_RESEARCH.md 2).

## 2. The port's front end

The app had no front end: it started the race at once. Now, before the race:

- **Race Options** (AUTHORED subset of `event_options`): Driver Assists and Go To Race, Go To Race
  selected first; Up/Down wrap; Back does nothing (there is no screen before it). The other items
  (difficulty, grid, damage, Pro Mode, head camera) are set by the settings profile and are not
  shown.
- **Driver Aids**: the original's four items, order, wrapping combos, help texts, Back and
  Button3 = Undo (values on entering the screen). Left/Right change the selected item.
- Leaving Driver Aids with a changed choice saves it to `<data dir>/settings.ini`: the
  traction, stability, braking and gearbox items are removed from its `[assists]` lines and one
  line `tcs=… stability=… braking=… gearbox=…` ends the section; other keys, sections and comments
  stay (`settings::with_driver_aids`). The written file is read back before it replaces the old
  one. A `drive_front_end … event=save` line is logged.
- Go To Race applies the aids to the player's car before its first tick (`Runtime::set_assists`)
  and starts the race; the AI cars keep every aid on, as in the original.
- While the front end is open the race is held (`Driving::paused`), so nothing ticks; the pause
  menu, Instant Replay, R and F are ignored. It never opens again in that run, so the aids cannot
  change in the race.
- Look: the pause menu's panel (AUTHORED, plain text in the original's strings), values shown as
  `< value >`.
- Navigation: keyboard (arrows, Enter, Escape, Ctrl), pad (D-pad/stick, A, B, X), and on a phone
  the P11 touch menu zones (UP/DOWN, < >, OK/BACK; docs/ANDROID.md "Controls"), which show
  because the race is held. The touch layout has no Button3, so Undo is keyboard/pad only.
- Shown unless the run is scripted (`--drive-ticks`), `[diagnostics] autopilot = on`, or
  `[front_end] show = off`.
- Precedence: the starting values are the run's aids after every settings file and flag. A
  profile's `[assists]` (none of the shipped ones has one), `--assists`/`--assists-file` or the
  launchers' `GRID_PLAY_ASSISTS`/`local\assists.txt` still override the saved choice at the next
  start.

- **Video** (F383, AUTHORED, not an original GRID screen; the PC version's graphics options are
  not ported): a third Race Options item between Driver Assists and Go To Race, so the port's
  `[osd] fps` and `[video]` settings can be changed on a phone. Same controls as Driver Aids
  (Up/Down, Left/Right cycle, Ctrl/X undo, Back saves when changed; touch UP/DOWN, < >, OK, BACK).
  Details: [ANDROID.md](ANDROID.md) "Frame rate".

## 3. Graded Traction Control and Braking Assist (AUTHORED)

The original has on and off only. The port adds 25 % and 50 %; 0 % is the original's off and 100 %
its on.

Encoding (`grid_world::driver_aids`): the original's aid bytes (+1559 braking, +155A traction) are 0
for on and 1 for off, and every read site only tests them for non-zero. The port keeps those two
values and their code paths unchanged and writes `0x80 | p` for a graded level, p = the percentage
of the "off" behaviour (75 for a 25 % aid, 50 for 50 %). The original never writes such values.

With f = p/100, at each original "off" site:

| Aid | Original off | Graded |
|---|---|---|
| Traction | throttle blend × 1/0.9, clamped 0..1 (`vehicle_propulsion`) | × (1 + f (1/0.9 − 1)), clamped |
| Traction | driven-wheel grip × (1 − k) at throttle > 0.9 (`vehicle_grip`) | × (1 − f k) |
| Braking | brake command × 1/0.9, clamped (`vehicle_controls`) | × (1 + f (1/0.9 − 1)), clamped |
| Braking | lock-up timer +E74 runs (`vehicle_post_force`) | runs |
| Braking | lock-up (grip × (1 − +116C), wheel spin 0) when +E74 > +1168 (`vehicle_grip`) | when +E74 > +1168 / f |

So a partial Traction Control scales the penalties linearly; a partial Braking Assist scales the
pedal penalty and lets the wheels lock only after the brake is held 1/f times as long (0.4 s at
25 %, 0.6 s at 50 % for the Lacetti's 0.3 s). Stability Control stays on/off.

Settings: `tcs` and `braking` take `on`/`off`, `0`, `25`, `50`, `100` (with or without `%`);
`Display` writes `on`/`off` for 100/0 and `25%`/`50%`.

## 4. Tests and results

- Unit [V]: `grid_world::driver_aids` (bytes and scale), `assists::graded_levels_parse_display_and_bytes`
  (parse, display, bytes, the wrapping step order Off → 25 → 50 → On),
  `settings::driver_aids_are_saved_in_the_assists_section`, `front_end` screen and navigation tests.
- Asset `assists::source_driving_assists_matrix` (Donington, Lacetti) [V]:
  - the original matrix values are unchanged: launch slip TC on 1.418, off 2.837; Braking off:
    lock-up timer 2.11 s, 17 locked ticks, stop at tick 139 against 209 with it on;
  - graded: launch slip 2.445 at 25 % and 2.065 at 50 %, between on and off; braking stops at tick
    141 at 25 % and 145 at 50 % (between off 139 and on 209; the lock-up comes later, the locked
    tick count stays 17..19);
  - 0 % is bit-identical to the original's off (bytes written as the original) and 100 % to on, over
    700 ticks of throttle, steering and full brake (vehicle and physical words compared each tick).
- Asset `front_end::the_front_end_holds_the_race_saves_and_applies_the_aids`: no tick while open,
  Pause and R ignored, the choice saved to a temporary settings file with its `[osd]` section kept,
  Go To Race applies TC 25 %, Braking 50 %, Manual at tick 0, and 500 ticks later the race digest
  equals a race set up with those aids directly.
- Solo identity and the 12-car race: see the F entry in PROJECT_LOG.md.

## 5. Open items

- The original's look (panels, icons, animations) and the other Race Options items are not
  ported; no main menu or career front end exists yet.
- Controller Setup, the surface assist and the reward bonuses for racing without aids are not
  ported.
- Whether real GRID players would prefer the braking lock-up to scale differently is a design
  choice, not evidence.
