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
| `[drive]` | `enabled` | `on`/`off` | `--drive` |
| | `laps` | `0` (free practice) or 1..99 | `--time-trial-laps` |
| | `opponents` | `0` or 1..11 | `--opponents` |
| | `grid_position` | `back` (after the opponents), `0` (pole, default) or 1..opponents+1 | `--grid-position` |
| | `ai_characteristics` | private TSV; **used only when the file exists** | `--ai-characteristics` (must exist) |
| `[private]` | `body_contacts` | CD01 path or `off` | `--body-contacts` |
| | `body_events` | `derive`, a BP01 path, or `off` | `--body-events PATH\|derive` |
| | `deformation` | `derive`, a DF01 path, or `off` | `--deformation PATH\|derive` |
| `[assists]` | `tcs`, `stability`, `braking`, `gearbox`, `input` | as `--assists` | `--assists`, `--assists-file` |
| `[input]` | `device`, `*_deadzone`, `*_saturation` | as `--input` | `--input` |
| `[ai]` | `difficulty` (0..1 or `normal`), `catchup` (`rd_N_star`, `on` = `rd_3_star`, `off`), `choreographer`, `mistakes` (`on`/`off`) | 0.606, `rd_3_star`, on, on (AI_DRIVER_PORT.md, "Mistakes, failures and race pacing") | `--ai` |
| `[sound]` | `effects` | 0..100, percent (default 100): the pause menu's Effects Volume, engine and tyre sound | none |
| `[osd]` | `lap_timer`, `position` | `on` (default) / `off`: the pause menu's Screen Setup, HUD lap/times and race position blocks | none |
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
| | `texture_fallback` | `etc2`: private ETC2 cache; `rgba8` or `on`: CPU BC decode; `off`: automatic (BC, ETC2, RGBA8 by GPU support) (P8) | `GRID_TEXTURE_FALLBACK=1`, `rgba8` or `etc2` |

Notes:

- `--assists-file` and `--assists` still **replace** the aids (and `--assists-file` the input settings) wholesale, as before; they do not merge with the settings files.
- The `GRID_*` diagnostic variables are still read on desktop as the base that `[diagnostics]` overrides, so the research commands recorded in other docs keep working. No runtime code reads environment variables any more except through `settings::Diagnostics::from_env` (tests still use `GRID_GAME_ROOT` and the private-file variables).
- The run log records `settings_file path=…` for every file read, `data_dir path=…`, `game_root path=…` and `drive_private_parameters kind=… source=file|derived-from-grid.exe sha256=…`.

The pause menu (Escape/P, pad Start; [PAUSE_MENU.md](PAUSE_MENU.md)) starts from the `[sound]`, `[osd]` and `[input]` values and changes them for the session only; the original's profile autosave is not ported.

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

It holds `settings.ini`, the time-trial records (`playtest/time-trial-records.tsv`) and private files named by relative paths. `--render` output must be under it (it was `local/`). Run logs are still the launcher's redirect of stdout to `local\playtest\manual-*.log`; on Android stdout goes to logcat (P9).

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

CD01 is not derived: it needs the Steamless-unpacked image (P6). The profiles still name the prepared `local/body-contact-directions.bin`.

## 5. Launchers

`Play GRID Prototype.cmd [GAME_FOLDER] [PROFILE]` runs `grid-app --settings settings\PROFILE.ini` (default `practice`). The other launchers are thin wrappers that check for the private CD01 and pass a profile:

| Launcher | Profile | Contents |
|---|---|---|
| Play GRID Prototype.cmd | `settings/practice.ini` | free practice |
| Play GRID Body Contacts.cmd | `settings/body-contacts.ini` | CD01 + derived BP01/DF01 |
| Play GRID Time Trial.cmd | `settings/time-trial.ini` | as above, 3 laps |
| Play GRID Race.cmd | `settings/race.ini` | 3 laps, 11 opponents, back of the grid, AI characteristics when present |

Changes in behaviour: BP01 and DF01 are now always derived from `grid.exe` instead of being used only when `local\*.bin` existed (the bytes are equal on this PC). The game folder argument is optional. `local\assists.txt`, `GRID_PLAY_ASSISTS`, `GRID_PLAY_INPUT`, `GRID_PLAY_OPPONENTS` and `GRID_PLAY_GRID` still work; the launcher turns them into command-line flags. Equivalent without any variables: `target\release\grid-app.exe --settings settings\race.ini`.
