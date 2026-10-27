# Settings file, install discovery and private parameters (P5)

Step P5 of [PLATFORMS_AND_INPUT_RESEARCH.md](PLATFORMS_AND_INPUT_RESEARCH.md) section 7: portability groundwork for the Android build. Code: `crates/grid-app/src/settings.rs`, `crates/grid-tools/src/install.rs`, `crates/grid-tools/src/private_parameters.rs`, `crates/grid-tools/src/bin/grid-private-parameters.rs`, `settings/*.ini`, the `Play GRID *.cmd` launchers.

## 1. Settings file

A plain text file in the style of the existing assists file. `grid-app` reads, in this order (later values win):

1. built-in defaults;
2. `<data dir>/settings.ini`, if present (the user's own preferences; in the owner's checkout `local/settings.ini`, ignored by git);
3. each `--settings FILE`, in command-line order (the launchers pass a profile from `settings/`);
4. every other command-line flag (`--game-root`, `--opponents`, `--assists`, …). The CLI stays a full desktop override.

Grammar:

- `[section]` headers; `key = value` lines; a `#` starts a comment at the start of a line or after whitespace (so `C:\Data#1\x.bin` keeps its `#`; a value cannot contain ` #`).
- Lines before any header belong to `[assists]`, so an old `local/assists.txt` is a valid settings file.
- `[assists]` and `[input]` use the existing `key=value` grammar (`Assists::parse`, `input::Settings::parse`); several files' sections are concatenated, so later keys win.
- Unknown sections and keys are refused with the line number. The file size limit is 64 KiB.
- Relative paths in `[drive]` and `[private]` resolve against the data dir, so a profile can say `body_contacts = body-contact-directions.bin`.

| Section | Key | Values | CLI equivalent |
|---|---|---|---|
| `[game]` | `root` | `auto` (default: search Steam) or a folder | `--game-root` |
| | `route` | `tracks/<region>/<track>/<route>` | `--route` |
| | `car` | `cars/<stem>` name, e.g. `lac` (default) or `bmw` | `--car` |
| | `opponent_cars` | `STEM[,STEM...]`, cycled over the opponents (default: the player's car) | `--opponent-cars` |
| | `series` | `pro_tuned`, `touring`, `gt1`, `gt2`, `le_mans_series`: the free-race series; the route must be in its pool, the opponents are its original free-race field (docs/SERIES.md; GRID_SERIES_SEED fixes the draw) | `--series` |
| | `series_seed` | the seed of the series' field and grid (career Le Mans races, F507); `GRID_SERIES_SEED` overrides it, else the clock | – |
| `[drive]` | `enabled` | `on`/`off` | `--drive` |
| | `laps` | `0` (free practice) or 1..99 | `--time-trial-laps` |
| | `opponents` | `0` or 1..11 | `--opponents` |
| | `grid_position` | `back` (after the opponents), `0` (pole, default) or 1..opponents+1 | `--grid-position` |
| | `ai_characteristics` | private TSV; **used only when the file exists** | `--ai-characteristics` (must exist) |
| | `discipline` | `race` (default) or `drift_battle` (needs `laps` and `opponents`; [DISCIPLINES.md](DISCIPLINES.md)) | `--discipline` |
| `[private]` | `body_contacts` | `derive`, a CD01 path, or `off` | `--body-contacts PATH\|derive` |
| | `body_events` | `derive`, a BP01 path, or `off` | `--body-events PATH\|derive` |
| | `deformation` | `derive`, a DF01 path, or `off` | `--deformation PATH\|derive` |
| `[assists]` | `tcs`, `stability`, `braking`, `gearbox`, `input` | as `--assists`; `tcs` and `braking` also `0`/`25`/`50`/`100` (25/50 % are AUTHORED graded aids, [DRIVER_AIDS.md](DRIVER_AIDS.md)). The front end's Driver Aids screen saves `tcs`, `stability`, `braking`, `gearbox` here in `<data dir>/settings.ini` | `--assists`, `--assists-file` |
| `[input]` | `device`, `*_deadzone`, `*_saturation`; phone controls `touch`, `tilt`, `tilt_max_angle`, `tilt_neutral`, `tilt_smoothing` ([ANDROID.md](ANDROID.md) "Controls"); force feedback `force_feedback`, `feedback_force`, `feedback_friction`, `feedback_effects` and the generic wheel preset's `wheel_*` keys ([FORCE_FEEDBACK.md](FORCE_FEEDBACK.md) §7.1) | as `--input` | `--input` |
| `[ai]` | `difficulty` (0..1 or `normal`), `catchup` (`on` = `rd_{star+1}_star` of the difficulty, `touge` = its `_touge` variant, an explicit `rd_N_star`, `off`), `choreographer`, `mistakes` (`on`/`off`) | 0.606, `on` (`rd_2_star`), on, on (AI_DRIVER_PORT.md, "Mistakes, failures and race pacing") | `--ai` |
| `[sound]` | `effects` | 0..100, percent (default 100): the pause menu's Effects Volume, engine and tyre sound | none |
| | `speech`, `music` | 0..100, percent (default 100): the pause menu's Speech and Music Volume, saved for later (the port plays no speech or music yet) | none |
| `[front_end]` | `show` | `on` (default) / `off`: the Race Options front end with Driver Aids before the race ([DRIVER_AIDS.md](DRIVER_AIDS.md)); never shown for scripted or autopilot runs | none |
| `[front_end]` | `event_menu` | `on` (default) / `off`: Android starts in the event menu ([EVENT_MENU.md](EVENT_MENU.md)); off = the old direct start. The desktop opens the menu with `--menu` only | none |
| `[event]` | `type`, `route`, `car`, `laps`, `opponents`, `time`, `direction` | the event menu's last choice, written by the menu and read only by it ([EVENT_MENU.md](EVENT_MENU.md)) | none |
| `[osd]` | `lap_timer`, `position`, `minimap`, `dials` | `on` (default) / `off`: the pause menu's Screen Setup, the HUD's lap group, race position, minimap and dial (docs/RACE_HUD.md) | none |
| `[osd]` | `fps` | `on` / `off`: frame-rate counter at the top (FPS, mean and worst frame time of the last second) and one `frame_stats` log line per second; default `on` on Android, `off` on the desktop, where F9 toggles it ([ANDROID.md](ANDROID.md) "Frame rate") | none |
| `[video]` | `render_height` | `auto`, `native` or rows 240..4320: the 3D view renders at most this many rows (window aspect) and is stretched over the window; HUD, menus and touch zones stay at the window's resolution (F556); never above native. `auto`: dynamic rows by the frame time (F556, F559, F571): 480-720 with the mobile preset; on the desktop from native down to half of native, rounded up to 24 rows, starting and staying at native while frames are on time (F573). Default `auto` on Android (mobile preset), `native` on the desktop ([ANDROID.md](ANDROID.md) "Frame rate") | none |
| `[video]` | `render_height_version` | written by the Video screen (2 since F559). With the mobile preset a `render_height` without it reads as `auto` (one-time migration) | none |
| `[video]` | `present_mode`, `refresh_hz`, `frame_latency` | `fifo` (default) / `auto_vsync` / `auto_no_vsync`; `system` (default) or 30..240 Hz (Android `ANativeWindow_setFrameRate` hint); 1..3 frames in flight (default wgpu's 2) ([ANDROID.md](ANDROID.md) "Frame rate") | none |
| | `speed_units` | `kph` (default) or `mph` | none |
| `[diagnostics]` | `free_look` | `yaw,pitch` | legacy `GRID_FREE_LOOK` |
| | `effects_camera` | `x,y,z,lx,ly,lz` | `GRID_EFFECTS_CAMERA` |
| | `ground_effects` | `on`/`off` | `GRID_GROUND_EFFECTS=0` |
| | `ground_trace` | `on`/`off` | `GRID_GROUND_TRACE` |
| | `autopilot` | `on`/`off` (scripted runs only) | `GRID_AUTOPILOT` |
| | `prop_script`, `prop_script_speed` | `TEMPLATE:INDEX`, m/s | `GRID_PROP_SCRIPT(_SPEED)` |
| | `look_back` | `on`/`off` | `GRID_LOOK_BACK` |
| | `damage_zones` | `on`/`off` | `GRID_DAMAGE_ZONES` |
| | `damage_camera` | `car,x,y,z` | `GRID_DAMAGE_CAMERA` |
| | `texture_fallback` | `etc2`: private ETC2 cache; `rgba8` or `on`: CPU BC decode; `off`: automatic (BC, ETC2, RGBA8 by GPU support) (P8). A forced value also leaves `TEXTURE_COMPRESSION_BC` off the device, so the desktop rejects any BC texture as a phone without BC does (F603) | `GRID_TEXTURE_FALLBACK=1`, `rgba8` or `etc2` |
| | `replay_capture` | seconds: after a scripted drive (`--drive-ticks`), play an Instant Replay and delay `--render` until it has run this long (docs/REPLAY.md) | `GRID_REPLAY_CAPTURE=S` |
| | `flashback_capture` | seconds: in that Instant Replay, Jump In (Flashback) once it has run this long; the drive continues from the jump-in frame (docs/FLASHBACK.md). Use a value below `replay_capture` | `GRID_FLASHBACK_CAPTURE=S` |
| | `max_ticks_per_frame` | interactive window only: at most this many 16 ms ticks per rendered frame, default 4; `0` restores Bevy's 250 ms clamp, for comparisons (docs/FRAME_STEPPING.md) | `GRID_MAX_TICKS_PER_FRAME=N` |
| | `tick_delay_ms` | milliseconds slept in every vehicle tick: a forced slow tick for frame-rate checks (docs/FRAME_STEPPING.md) | `GRID_TICK_DELAY_MS=MS` |
| | `camera_fps` | frames per second of race time: the in-race views step by the original's per-frame rules at this rate (head-cam springs, look smoothing, follow damp, spin hold), for comparisons with the original at 60 or 165 fps; unset, one step per 16 ms tick (docs/CAMERA_VIEWS.md §3 "Camera clock") | `GRID_CAMERA_FPS=F` |

Notes:

- `--assists-file` and `--assists` still **replace** the aids (and `--assists-file` the input settings) wholesale, as before; they do not merge with the settings files.
- The `GRID_*` diagnostic variables are still read on desktop as the base that `[diagnostics]` overrides, so the research commands recorded in other docs keep working. No runtime code reads environment variables any more except through `settings::Diagnostics::from_env` (tests still use `GRID_GAME_ROOT` and the private-file variables).
- The run log records `settings_file path=…` for every file read, `data_dir path=…`, `game_root path=…` and `drive_private_parameters kind=… source=file|derived-from-grid.exe sha256=…`.

The pause menu (Escape/P, pad Start; [PAUSE_MENU.md](PAUSE_MENU.md)) starts from the `[sound]`, `[osd]` and `[input]` values. Since F480 it saves them in `<data dir>/settings.ini` whenever a settings screen is left, like the original's profile autosave. It writes `[sound] effects` and the `[osd]` Screen Setup keys, plus the `[input]` dead zones and saturations after the Advanced screen. The pre-race Race Options **Video** screen (AUTHORED, F390) changes `[osd] fps` and the `[video]` keys and saves them in `<data dir>/settings.ini` when it is left, keeping the other keys and comments.

Example `local/settings.ini`:

```ini
[game]
root = D:\SteamLibrary\steamapps\common\Grid
[assists]
tcs=off gearbox=manual
[input]
device=pad
```

## 2. Data dir

`--data-dir DIR` sets it. Otherwise:

- the owner's checkout: `local/` when the current folder has one (the launchers start in the repository root), so records stay at `local/playtest/time-trial-records.tsv` and renders under `local/` as before;
- Windows: `%APPDATA%\GRID Reassembly`; macOS: `~/Library/Application Support/GRID Reassembly`; Linux: `$XDG_DATA_HOME/grid-reassembly` or `~/.local/share/grid-reassembly`;
- Android: none yet; P9 sets it from `AndroidApp::internal_data_path()` / `external_data_path()`.

It holds `settings.ini`, the time-trial records (`playtest/time-trial-records.tsv`), `camera.ini` (the last in-race view, `current_camera = IDENT`, written by the app on exit like the original's profile key; [CAMERA_VIEWS.md](CAMERA_VIEWS.md)) and private files named by relative paths. `--render` output must be under it (it was `local/`). Run logs are still the launcher's redirect of stdout to `local\playtest\manual-*.log`; on Android stdout goes to logcat (P9).

## 3. Install discovery and case-insensitive index

`grid_tools::install` [V unit tests, V on this PC]:

- `steam_roots()`: `%ProgramFiles(x86)%\Steam`, `%ProgramFiles%\Steam` on Windows; `~/.steam/steam`, `~/.local/share/Steam`, the Flatpak folder on Linux; `~/Library/Application Support/Steam` on macOS. The Windows registry (`SteamPath`) is not read.
- `library_folders(vdf)`: the libraries from `steamapps/libraryfolders.vdf`, both the current `"N" { "path" "…" }` form and the old `"N" "path"` form (KeyValues tokenizer with `\\` escapes and `//` comments).
- For each library, `steamapps/appmanifest_12750.acf` gives `installdir` (the owner's manifest: app 12750, name "GRID", installdir "Grid"; default "Grid"). A candidate is accepted when `cars/lac/lac.ned` exists in any casing.
- `FileIndex::build(root)` lists the install once (no symlink following, depth 32) and maps lower-case `/`-separated relative paths to the real names; `resolve()` tries the exact path first. `grid-app` uses it for `grid.exe` when deriving BP01/DF01.

`grid-app` without `--game-root` and without `[game] root` searches the libraries and logs `game_root path=…`. The asset test `install::tests::discovers_the_installed_game` checks that discovery finds `GRID_GAME_ROOT` on this PC.

**Not done:** the readers (about 88 `root.join(...)` sites in grid-app and grid-tools) still open exact paths. On Windows that is correct; routing them through `FileIndex::resolve` belongs to P7 (the Linux case-sensitivity run), which can measure which names actually differ.

## 4. BP01 and DF01 from the user's grid.exe

`grid_tools::private_parameters` is a port of `tools/extract_body_impact_parameters.py` and `tools/extract_deformation_parameters.py`:

- the same build check: SHA-256 of `grid.exe` must be `dd7b0933…e48f` (the Steam build); a small FIPS 180-4 SHA-256 is included instead of a dependency, checked against the standard vectors (empty, `abc`, the 448-bit message, one million `a`);
- the same PE header reading as `inspect_pe`, and the same section rules: BP01 takes 4 file-backed bytes from the first section where `0 <= rel <= raw_size - 4`; DF01 takes the first section whose mapped extent `max(virtual_size, raw_size)` holds the bytes and zero-fills past the file-backed part, as the loader does;
- the same addresses and layout (BP01 28 bytes, DF01 252 bytes). Data only; no code is executed or disassembled.

`settings::Private::Derive` (`body_events = derive`, `deformation = derive`, or `--body-events derive`) computes the bytes in memory at start-up; nothing is written. `grid-private-parameters GRID_EXE BP01_OUT DF01_OUT` writes the files (refuses to overwrite) for tools and tests.

Verified [V] on 2026-10-16 (evidence `local/p5-portability/` in the main checkout):

- Rust output vs the existing `local/body-impact-parameters.bin` and `local/deformation-parameters.bin`: byte-equal (`cmp`), SHA-256 `493380e1…946d` and `5d48e35d…3292`, the hashes the Python JSON records.
- A fresh Python 3.14.4 run of both extractors into `local/p5-portability/python/`: byte-equal to the Rust output.
- Asset test `private_parameters::tests::derived_parameters_equal_the_python_output` compares the derived bytes with `GRID_BODY_EVENT_PARAMETERS` / `GRID_DEFORMATION_PARAMETERS`.

CD01 is derived too since F359 (P6, docs/CONTACT_DIRECTIONS.md). `contact_directions` reads 119 `.rdata` words named by an address recipe that was traced once from the `C7DAE0` initializer. Its output is byte-identical to the prepared `local/body-contact-directions.bin`. The profiles say `body_contacts = derive`. `grid-private-parameters GRID_EXE BP01_OUT DF01_OUT [CD01_OUT]` writes all three files.

## 5. Launchers

`Play GRID.cmd [GAME_FOLDER]` runs `grid-app --menu`: the in-game event menu (Race Day, [EVENT_MENU.md](EVENT_MENU.md)); leaving a race returns to it. The launchers below start their event directly, as before.

`Play GRID Prototype.cmd [GAME_FOLDER] [PROFILE]` runs `grid-app --settings settings\PROFILE.ini` (default `practice`). The other launchers are thin wrappers that pass a profile; since F359 none needs a prepared private file:

| Launcher | Profile | Contents |
|---|---|---|
| Play GRID Prototype.cmd | `settings/practice.ini` | free practice |
| Play GRID Body Contacts.cmd | `settings/body-contacts.ini` | derived CD01/BP01/DF01 |
| Play GRID Time Trial.cmd | `settings/time-trial.ini` | as above, 3 laps |
| Play GRID Race.cmd | `settings/race.ini` | 3 laps, 11 opponents, back of the grid, AI characteristics when present |
| Play GRID Drift Battle.cmd | `settings/drift-battle.ini` | Drift Battle ([DISCIPLINES.md](DISCIPLINES.md)): Long Beach route_1, 2 laps, 7 BMW opponents |
| Play GRID Drift Battle Nurburgring.cmd | `settings/drift-battle-nurburgring.ini` | Drift Battle on Nürburgring route_1 (nurburgring_spr) |
| Play GRID Drift Battle Istanbul.cmd | `settings/drift-battle-istanbul.ini` | Drift Battle on Istanbul Park route_1 (istanbul_int) |
| Play GRID Drift Battle Jarama.cmd | `settings/drift-battle-jarama.ini` | Drift Battle on Jarama route_1 (jarama_r) |
| Play GRID Series Prototype.cmd | `settings/series-prototype.ini` | Prototype series, Istanbul Park GP (F457) |
| Play GRID Series Open Wheel.cmd | `settings/series-open-wheel.ini` | Open Wheel series, Donington GP (F457) |
| Play GRID Series GT Club.cmd | `settings/series-gt-club.ini` | GT Club series, Donington GP (F457) |
| Play GRID Series Prestige.cmd | `settings/series-prestige.ini` | Prestige series, Istanbul Park GP (F457) |
| Play GRID Drift Battle Jarama GP.cmd | `settings/drift-battle-jarama-gp.ini` | Drift Battle on Jarama route_0 (jarama_gp) |
| Play GRID Drift Battle Long Beach Reverse.cmd | `settings/drift-battle-long-beach-reverse.ini` | Drift Battle on Long Beach route_4 (longbeach_cl_r) |
| Play GRID Drift Battle Nurburgring Reverse.cmd | `settings/drift-battle-nurburgring-reverse.ini` | Drift Battle on Nürburgring route_3 (nurburgring_spr_r) |
| Play GRID Drift Battle Istanbul Reverse.cmd | `settings/drift-battle-istanbul-reverse.ini` | Drift Battle on Istanbul Park route_3 (istanbul_int_r) |

Changes in behaviour: BP01 and DF01 are now always derived from `grid.exe` instead of being used only when `local\*.bin` existed (the bytes are equal on this PC). The game folder argument is optional. `local\assists.txt`, `GRID_PLAY_ASSISTS`, `GRID_PLAY_INPUT`, `GRID_PLAY_OPPONENTS` and `GRID_PLAY_GRID` still work; the launcher turns them into command-line flags. Equivalent without any variables: `target\release\grid-app.exe --settings settings\race.ini`.
