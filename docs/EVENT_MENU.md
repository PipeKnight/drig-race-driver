# Event menu (Race Day)

18 October 2026, Claude Code (F414). Code: `crates/grid-app/src/event_menu.rs`. Launcher: `Play GRID.cmd`.

The owner picks the event inside the game instead of through one launcher per event. The menu is the original's free-race selection screen, then the race process with its Race Options front end ([DRIVER_AIDS.md](DRIVER_AIDS.md)) and Go To Race.

Labels: **[D]** read from shipped data; **[I]** interpretation; **AUTHORED** a choice of this port.

## Original structure and data

- Screen [D `system/states.bin`, decoded in the main checkout's `local/pause-menu/states.xml`]: `f_m_selectrace`, a `ScreenGarageFreeRaceSelect` with buttons `button_1`..`button_6`, glyphs discipline, car, track_select, track_type, laps, team. It leads to `f_m_event_options` (`ScreenPreEventOptions`, Race Options).
- Strings [D `language/language_eng.lng`, read at run time]: title `free_race_menu_screen_title` (Race Day); rows `free_race_button_discipline` (Discipline), `free_race_button_car` (Car), `free_race_button_track_select` (Location), `free_race_button_track_type` (Route), `free_race_button_laps` (Laps), `free_race_button_time` (Time); `raceday_buttontext_to_race` (Continue). Disciplines, venues and layouts are `db_<name_string_id>` (for example `db_touring` Touring Cars, `db_donington` Donington Park, `db_donington_nat` National Circuit, `db_illegal_touge` Midnight Touge, `db_1_minute`).
- Data [D `database/database.bin`]:
  - `free_race_discipline` gives the disciplines in `sort_order`;
  - `free_race_restrictions` gives each discipline's cars and its `free_race_track_pool`;
  - the pool's `track_model` rows give the layouts (route folder from `folder_string`/`file_string`/`route_string`, venue from `track`);
  - `free_race_times` gives the Freestyle Drift times (whole minutes).
  `grid_tools::series::Series::from_database` reads every discipline this way.
- Nothing of it is committed: the app reads the user's own files.

## What the menu offers

| Discipline | Source | Routes | Cars | Rows |
|---|---|---|---|---|
| Race | AUTHORED (the launchers' race) | `route::CIRCUITS` (6) | Lacetti, BMW 320si | Laps 1–10 (3), Opponents 1–11 (11) |
| Time Trial | AUTHORED (`main_menu_time_trial_button_title`) | `route::CIRCUITS` | every raceable car | Laps (3) |
| Pro Tuned, Touring Cars, GT1, GT2 | `free_race_discipline` | pool ∩ raced tracks (`Series::supported_layouts` rule), 21 each | `Series::raceable_cars` | Laps (3); the field is the original's grid fill |
| Le Mans Series | `le_mans_series` (F409) | pool ∩ raced tracks (4) | the series' raceable cars (7) | Laps (3); the original's 15–17-car field; experimental like its launcher (the desktop supervisor sets `GRID_SERIES_EXPERIMENTAL=1` for this race), help line says so |
| Drift Battle | `drift_battle` | pool ∩ raced tracks (9) | BMW, Lacetti (stand-ins) | Laps (2), Opponents 1–7 (7, pool `max_cars` − 1) |
| Drift GP | `drift_gp` | pool ∩ raced tracks (9) | BMW, Lacetti (stand-ins) | Laps (2) |
| Freestyle Drift | `Free_Drift` | the four Yokohama arenas | BMW, Lacetti (stand-ins) | Time (`free_race_times`; 2 minutes AUTHORED default) |
| Midnight Touge | `illegal_touge` | Haruna route 1 (`route::HARUNA`), the validated road | Lacetti, then the pool's raceable 350Z, Skyline, Supra | Direction Downhill/Uphill (subroute 0/1) |
| Pro Touge | `legal_touge` (F412) | Haruna route 1, as its launcher (the pool holds the legal `haruna_leg_*` layouts; AUTHORED) | Lacetti, then the pool's raceable cars | two runs, downhill then uphill (`discipline = pro_touge`) |
| Demolition Derby | `Demolition_D` (F421) | the Detroit stadium routes 8 and 9 (`route::DETROIT_STADIUM`, `_SHORT`) | the stock car `stk` | laps (6 [D]), opponents (11), `discipline = demolition_derby` (docs/DEMOLITION_DERBY.md) |

- The numbers in brackets are the counts with the owner's full installation (log line `drive_event_menu ... event=open types=...`).
- Not offered:
  - the 24 Hours of Le Mans, open wheel, muscle, club and prestige: not ported (docs/SERIES.md, docs/DISCIPLINES.md);
  - practice and the body-contact test drive: not events; their launchers stay.
- Stand-in cars: the original's drift cars (cor, gto, imp, mop, rx7, s15, soa) and most touge cars are not ported. The help line says so. AUTHORED.
- A route or car whose folder is missing is left out. On the phone's data subset that leaves Race and Time Trial at Donington with the Lacetti. That subset has no `database/`, so the series and drift disciplines are absent, and venue and layout names fall back to folder names (AUTHORED).
- AUTHORED rows: Opponents (the original fills the grid from the pool), Direction (the original's Midnight Touge picks it at random, docs/TOUGE.md) and the lap range 1–10 (the original's lap choices are not decoded). Livery (`team`) is not offered.
- Look: the pause menu's panel (AUTHORED, docs/PAUSE_MENU.md) on a plain background. The original's garage scene and glyphs are not ported.

## Controls

- Keyboard, pad and touch go through the input layer's Menu actions, like the pause menu: Up/Down choose a row, Left/Right change it, Select on a row moves down, Select on Continue starts the event. Back (Escape, pad B) closes the menu on the desktop.
- Held directions repeat with the original's delays (0.35 s, then every 0.15 s).
- Mouse (AUTHORED): a click selects a row; a second click on it changes the value, or starts the event on Continue.
- Phone: the P11 menu zones (UP, DOWN, `<`, `>`, OK, BACK). A connected pad hides them (`device=auto`, P11b).

## Saving

- On Continue the choice is saved in `<data dir>/settings.ini` `[event]` (`type`, `route`, `car`, `laps`, `opponents`, `time`, `direction`) with `settings::with_keys`, which keeps the other keys and comments. Only the menu reads `[event]`, so the launchers' profiles are unaffected. The next menu opens on that choice, with the cursor on Continue.
- The menu also writes `<data dir>/event-menu.ini`, a settings profile with the same keys as the launchers' `settings/*.ini` for that event. The race reads it like a launcher profile.
- The asset test `every_launcher_event_is_offered_and_written_identically` checks every launcher profile except practice and body contacts (22 profiles, including Le Mans Series, Pro Touge and Demolition Derby): the catalog offers its event, and the menu's profile for it parses to the same settings. The one exception is Freestyle Drift: the launcher's 90 s is not one of the original's whole-minute times.

## Processes and Back to menu

The menu does not run inside the race app, and the race app cannot return to the menu in place:
- the race is set up (track, cars, AI, collision) in `run()` before its Bevy app starts, about 1,200 lines that build the app's resources;
- winit allows one event loop per process, so a second Bevy app cannot start in the same process.

Moving the race set-up into the running app, and tearing it down for the menu, is a larger change than this task. So each step is a new process:

- **Desktop:** `grid-app --menu` is a small supervisor. It runs `grid-app --menu-screen` (exit code 20 = Continue, the profile is written), then `grid-app --settings <data dir>/event-menu.ini --from-menu`. When that race ends with exit code 21 (back to menu), it runs the menu again. Any other exit, such as closing a window or Back in the menu, ends the supervisor with that code. `--game-root`, `--data-dir` and `--settings` are passed on to both. `Play GRID.cmd` runs it.
- **Android:** no command line and no child processes. On start the app runs the menu, unless `<data dir>/event-menu.go` exists. Continue writes that marker and the profile, then calls `MainActivity.relaunch()`. That starts `RelaunchActivity` in its own process (`:relaunch`) and finishes the game activity, whose `onDestroy` ends the process. `RelaunchActivity` waits for the old process to end (at most 5 s), then opens `LauncherActivity`, which starts a new game process. The new process removes the marker and runs the race with `--from-menu`. Back to menu relaunches without the marker. `[front_end] event_menu = off` restores the old direct start with `settings.ini`'s own event.
- **Back to menu:** a race started from the menu leaves to the menu where it used to close the app: results Continue and Exit, Retire from the wreck screen, Retire from Race without results, and Finish Test Drive. This matches the original, whose results Continue (`end_race_finish_race`) returns to its front end (docs/RACE_HUD.md "Post-race"). A race started by a launcher closes the app as before.
- Log lines: `drive_event_menu version=1 event=open|go|quit|back_to_menu|go_failed`.

## Checks

- [V] Unit tests in `event_menu::tests`:
  - without a database only Race and Time Trial remain, and the phone subset leaves Donington and the Lacetti;
  - the menu opens on Continue and follows the original's button order;
  - Location and Route cycle; the Race profile equals `settings/race.ini`;
  - `[event]` round-trips through `with_keys` and keeps other text;
  - the asset test over all launcher profiles (above).
- [V] Desktop, owner's installation, release build, keystrokes and screenshots in the worktree's `local/event-menu/`:
  - the menu opens with 12 disciplines;
  - Time Trial with 1 lap → Continue → Race Options → Go To Race → pause → Retire from Race → Yes → results → Continue → the menu again, showing the saved 1-lap choice;
  - two mouse clicks changed Discipline to Pro Tuned;
  - Escape closed the menu and the supervisor;
  - a saved Le Mans Series choice → Continue (mouse) → the race process built the original's 15-car field (`drive_series ... opponents=14`) and opened Race Options; the experimental flag reached it;
  - with `[input] touch = on` and `device = keyboard` the P11 menu zones show; with `device=auto` the connected pad hides them, as P11b intends.
- [V] `cargo ndk … clippy --release --lib -p grid-app -- -D warnings` clean.
- [N] Not tried on a phone (the relaunch, the touch menu by finger). The Drift, other series and touge events were not started from the menu in a window. Their profiles equal the launchers' (asset test), and the launchers run (F408, F409, F412).
