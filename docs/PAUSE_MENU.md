# In-race pause menu

17 October 2026, Claude Code. The original's in-race menu in the playable app
(`crates/grid-app/src/pause_menu.rs`). Static research only: no live process, no input sent to
the original.

Markers: **[D]** read directly from shipped data, **[V]** verified by a test or run, **[I]**
interpretation, **AUTHORED** the port's own choice.

## 1. Evidence

Private evidence in the main checkout: `local/pause-menu/` (`states.xml` and `flow.xml` decoded
with `tools/validate_binxml.py`'s decoder, string scans made with `tools/scan_strings.py`).

- The menus are not in the front-end `.pssg` databases. `fePersistentDatabase.pssg` has only the
  generic `in_game_N_button` glyphs and `pause_root` [D].
- The screens are defined in **`system/states.bin`** (GRID binary XML). Classes include
  `ScreenPauseMainMenu`, `ScreenPauseBase`, `ScreenPauseOptionsSound`, `ScreenOptionsBasePause`,
  `ScreenOptionsAdvancedControlsPause` and `ScreenYesNo`. Each screen has an `itemflow` (order), items with
  `IBSelectableSimple link=…` and `IBTextStatic string=…`, and an `overlay` of button help [D].
- **`system/flow.bin`** is the state graph. It says which screen each event type uses and where each
  link goes [D].
- Labels are language ids in `language/language_*.lng` [D]. `grid_formats::language` reads that
  table: big-endian `LNGT`, then `HSHS`/`HSHT`, then `SIDA` (count, then id/text offset pairs), then
  the `SIDB`/`LNGB` blobs. The app shows the user's own English texts at run time.
- Menu input: `ActionMap/actionMapPreset0.xml` (keyboard) and `actionMapPreset4.xml` (XInput) have
  a `<!-- Menu Actions -->` block. `repeatSettings.xml` gives the held-direction repeat: delay
  0.35 s, then every 0.15 s [D].

## 2. Which menu

`flow.bin` has one pause branch per front-end flow. Each picks its main menu through
`current_event_type_decision` [D]:

| Flow | Main menu | Buttons (itemflow order) |
|---|---|---|
| `f_` Race Day free race | `pause_mainmenu_free_race` | Continue, Instant Replay, Restart Race, Options, Retire from Race |
| `t_` test drive | `pause_mainmenu_test_drive` | Continue, Instant Replay, Restart Test Drive, Options, Finish Test Drive, Ghost Car |
| `c_` career race | `pause_mainmenu_career_race` | Continue, Instant Replay, Restart Race, Options, Retire, Quit to Garage, Sponsor Objectives |
| intro / driver offers | `pause_mainmenu_intro_race`, `_driver_offers` | as career, without Sponsor Objectives |

The app starts single races from a launcher, like Race Day. So timed events (races with or
without opponents) use **free race**. Free practice has no closer original event than the
**test drive** [I]. Both screens are reproduced exactly. The asset test
`menus_match_the_original_states_file` compares links, strings and order with `states.bin` [V].

All main menus have the same overlay [D]: select `default_buttontext_select`, back
`pause_return_to_race` ("Back"), button3 `pause_reset_to_track` ("Reset Vehicle").

Links in the free-race flow [D]:

- `BACK` (Continue, or the back button): blur off, OSD on, back to the race.
- `restart` → `restart_race_confirm` → `restart_darken` → `restart_overlay_hide` → `closed_shutters_restart` (`StateRestartRace`).
- `retire_race` → `retire_race_confirm` → `post_race_retire_race_entry`, i.e. the post-race screens.
- `settings` → `st_pause_options_entry` → `pause_settings_standard`, `_wheel_no_ffb` or `_ffb`, by controller.
- `reset_car` (button3) → `reset_car_confirm` → fade → `reset_car` (`StateIngameResetToTrack`).
- `instant_replay` → the 15-second instant replay.

Confirmations are `ScreenYesNo`: No (`overlay_popup_generic_decline`), then Yes
(`overlay_popup_generic_accept`). `active_default="0.library.button_1"`, so **Yes is the default**
[D].

Options screens [D]:

| Screen | Rows |
|---|---|
| `pause_settings_standard` (pad, keyboard) | Sound Setup, Screen Setup |
| `pause_settings_wheel_no_ffb` | Sound Setup, Screen Setup, Advanced |
| `pause_settings_ffb` | Sound Setup, Screen Setup, Force Feedback, Advanced |
| Sound (`*_pause_st_sound_optionsscreen`) | Effects Volume, Speech Volume, Music Volume (sliders, %); button3 Undo |
| Screen Setup (`*_pause_st_osd_optionsscreen`) | Lap & Times, Race Position, Mini Map, Dials (on/off), Speed Unit (km/h, mph) |
| Advanced (`*_pause_st_adv_optionsscreen`) | Steering Deadzone, Steering Saturation, Steering Linearity, Acceleration Pedal Deadzone/Saturation, Brake Pedal Deadzone/Saturation, 900 degree wheel |

Leaving a settings screen autosaves the profile (`*_st_*_autosave`) [D].

## 3. The port

- **Pause** (keyboard Escape or P, pad Start) opens the main menu.
  - Keyboard Escape is the original preset 0's Pause key. P is kept from the prototype. Escape no
    longer closes the app.
  - While the menu is open, `Driving::paused` holds every fixed tick. Nothing in the simulation,
    AI or race manager runs. The engine and tyre sound are muted, as they already were on pause.
- **Navigation:**
  - keyboard: arrows or numpad 8/2/4/6, Enter, Escape (back), Ctrl (button3);
  - pad: D-pad or left stick, A, B, X (button3);
  - held directions repeat after 0.35 s, then every 0.15 s;
  - lists wrap (`wrapV="true"`); left/right toggle combo boxes, which wrap too.
- **Continue / Back** resumes. The race continues exactly where it stopped [V].
- **Restart Race / Restart Test Drive**: after Yes, the whole event restarts, the same as `R`. That
  covers:
  - every car (state, damage, deformation, lost wheels);
  - the shared world and its dynamic props;
  - the AI drivers, the race manager (catch-up, choreographer) and the standings;
  - lap timing and the countdown.

  The restarted race replays a fresh race bit for bit [V].
- **Retire from Race**: after Yes, the player is retired (`Out::Retired`), the race is classified,
  `drive_race_result` lines are logged and the app closes, back to the launcher.
  - The original goes on to its post-race screens, and from there "Exit" to the front end. The
    results screens are a separate later task.
  - Decision: closing the app is the nearest equivalent of returning to where races start [I].
- **Finish Test Drive**: after Yes, the app closes (practice).
- **Reset Vehicle** (button3, with its confirmation): the player's car is placed at rest on the
  racing line, with the same placement as the auto-reset of F341
  (`player_reset::place`, `c_pause_reset_car` [I]). It is offered only when the route has a reset
  line.
- **Options**: the standard screen, or the wheel screen for `device=wheel`. There is no force
  feedback in the prototype, so the `_ffb` screen is not used.
  - Effects Volume scales the engine and tyre sound (`AudioSink` volume).
  - Lap & Times and Race Position hide those HUD blocks. Speed Unit switches the HUD between km/h
    and mph.
  - Advanced recalibrates every steering, accelerate and brake binding live
    (`ActionMap::calibrate`, the same as `[input] *_deadzone`/`*_saturation`). The dead zone stays
    below the saturation.
  - Changes apply at once. Button3 (Undo) restores the values the screen was entered with.
  - Starting values come from `[sound] effects` and `[osd] lap_timer`/`position`/`speed_units`
    (docs/SETTINGS.md) and from the live input calibration.
- **Shown but not selectable** (dimmed), because the prototype has no counterpart yet:
  - Instant Replay, Ghost Car;
  - Speech and Music Volume, Mini Map, Dials;
  - Steering Linearity, 900 degree wheel.
- **Gear safety:** pad A is both Menu Select and Gear Up. Every menu action clears the gear queue,
  so a menu press never shifts the car.
- **Pad stick Y:** the original flips XInput Y thumbs (1 − v) when its device flag +0x94 is set
  [D], and binds Menu Up and Look Up to `biDirectionalLower` [D]. So the port now reads both Y
  thumbs flipped (stick up reads low), which makes Menu Up go up [I]. Look Up/Down has no consumer
  in the app yet.
- **Log lines:**
  - `drive_menu version=1 … action=open|resume|restart|reset-car|retire-race|finish-test-drive`;
  - `drive_menu version=1 action=apply …` and `action=unavailable button=…`;
  - `drive_menu_reset …`;
  - the existing `drive_sample event=pause|resume|reset|retire`.

AUTHORED presentation:

- a centred plain panel: title, subtitle (confirmations), up to eight rows with the selected row
  highlighted and unavailable rows dimmed, and a button-help line;
- slider steps of 10 % (volume) and 5 % (controls);
- none of the original's animated glyphs, pause blur, paused-world camera or audio ducking.

Not persisted: the original autosaves the profile when a settings screen is left. The port keeps
changes for the session only; put lasting values in `local/settings.ini`.

## 4. Tests

- `pause_menu::tests` (unit) cover:
  - order and links;
  - wrap;
  - Escape as Pause plus Back counting as one step;
  - No/Yes and the default answer;
  - unavailable entries;
  - Reset Vehicle with and without a line;
  - options limits and Undo;
  - the repeat timing.
- `pause_menu::tests::menus_match_the_original_states_file` (asset) compares, against `states.bin`:
  - both main menus;
  - both options screens;
  - the sound, screen and advanced rows of the `f_` and `t_` flows;
  - the four confirmations.

  It also checks that every label exists in `language_eng.lng`.
- `pause_menu::race_tests` (asset and private CD01/BP01/DF01; 3 AI opponents, 2 laps, keyboard
  through the input layer):
  - `pause_menu_freezes_the_race_and_resumes_bit_identically`:
    - drive to tick 300 and open the menu;
    - browse Options › Sound, change and undo the volume, and idle for 50 frames;
    - back out and Continue, then drive to tick 600.
    - The state is unchanged during the pause, and at tick 600 it equals an unpaused run. The
      state digest is the FNV over every car's vehicle and physical words, plus lap, event clock,
      standings, props, contacts, retirements, race-manager log and AI throttles.
  - `restart_race_resets_cars_damage_ai_timing_and_props`:
    - drive 1800 ticks into the field and the barriers, then Restart Race from the menu (Yes by
      default);
    - the state after the restart frame equals a fresh race after one frame;
    - the restarted race driven 900 ticks equals a fresh race given the same input [V].
    - The first attempt left real damage behind: player damage words changed and 115 car-contact
      ticks. It broke no prop.
- `source_vehicle::multi::tests::reset_restores_intact_props_and_replays_a_fresh_world`
  (grid-tools, asset and private):
  - break two tyre stacks and drive 200 ticks, then reset;
  - no events or pieces remain, the body count and the world/vehicle hash are back to the start;
  - 200 more ticks equal a fresh world [V].
- `language::tests` (grid-formats): synthetic tables and the shipped English table.
- `settings::tests::pause_menu_sound_and_screen_options`.
