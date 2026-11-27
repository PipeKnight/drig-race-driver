# DRIG: an open-source engine for Race Driver: GRID (2008)

[![CI](https://github.com/PipeKnight/drig-race-driver/actions/workflows/ci.yml/badge.svg)](https://github.com/PipeKnight/drig-race-driver/actions/workflows/ci.yml)

DRIG is a reimplementation of Codemasters' Race Driver: GRID (2008) in Rust with the [Bevy](https://bevyengine.org/) engine. The goal is a 1:1 recreation: the same handling, AI, damage, events and menus as the original game, running on a modern, open codebase.

The repository contains **no game assets**. The engine reads tracks, cars, textures, sounds and game data from your own installed copy of GRID, the same way OpenMW uses Morrowind's data files or OpenRCT2 uses RollerCoaster Tycoon 2's.

<img width="1280" height="720" alt="image" src="https://github.com/user-attachments/assets/f46aebc0-85d4-422a-9b75-f2b3b9c99744" />

Beyond the original look, I'm also experimenting with optional graphics improvements such as DLSS/DLAA and ray tracing (see the [roadmap](docs/ROADMAP.md)).

<img width="820" height="370" alt="moving-bias-native01" src="https://github.com/user-attachments/assets/bec7f111-4d29-4e80-be70-c2d5ed45c4f9" />

The physics engine and damage system have already been fully rebuilt from scratch:

<img width="520" height="292" alt="row117-crash(1)" src="https://github.com/user-attachments/assets/a272a9d6-3f7d-4d05-bc9f-66a2a8e554a3" />

(In this GIF, you can see some glitches with the shadows and trees. It was recorded on an early prototype, and these issues have since been resolved already)

## Download and play

Get the [latest release](https://github.com/PipeKnight/drig-race-driver/releases/latest) and download the file for your system:

| System | File |
| --- | --- |
| Windows | `DRIG-vX-windows-x86_64.exe`: one program file, nothing to unpack |
| Linux | `DRIG-vX-linux-x86_64.AppImage` (`chmod +x` it and run it; needs FUSE 2, or start it with `--appimage-extract-and-run`) or `DRIG-vX-linux-x86_64.tar.gz` (unpack it and run `drig`) |
| macOS | the DMG, added to the release by the Mac build |

No game data is included. You need your own installed copy of Race Driver: GRID (2008) for PC. DRIG looks for it in your Steam libraries. If your copy is somewhere else, start DRIG with `--game-root "D:\Games\GRID"` (any path), or set `root` under `[game]` in `DRIG-data/settings.ini`. `--help` lists every player option, and `--career` starts the career.

Put the program in a folder you can write to. DRIG keeps your settings, records, replays and the log in a `DRIG-data` folder next to it. To update, replace the program file; `DRIG-data` stays.

The builds are not signed by Microsoft or notarized by Apple, so the system may warn you the first time:

- Windows SmartScreen: click "More info", then "Run anyway".
- macOS Gatekeeper: if it refuses to open the app, open System Settings, Privacy & Security, and click "Open Anyway".

Each release's notes list what was tested on each system. `SHA256SUMS.txt` in the release lets you check your download.

## Status

Early and experimental, but you can already race. What works today:

- **Driving.** The vehicle physics, tyres, suspension, gearbox and driver aids (traction control, stability control, braking assist, automatic or manual gearbox) are ported from the original code.
- **Events.** Circuit races, Time Trial, Drift (Battle, GP, Freestyle), Touge, Demolition Derby, the Le Mans 24 Hours and the championship series (Touring, GT1, GT2, Pro Tuned, Open Wheel, Prestige, Le Mans Series and others). You choose them from the original Race Day menu.
- **AI opponents** that follow the original racing lines, overtaking and drift logic.
- **Damage.** Body contacts, collision damage, wheel detachment, mechanical and handling damage.
- **Audio.** Engine, gear changes, tyres and collisions.
- **Career.** GRID World with its events, fields, payouts, head-to-heads, driver offers, eBay Motors and the Le Mans selection events. Start it with `--career`; it is not in the main menu yet, and its window is fixed at 1280x800 for now.
- **Front end.** The Race Day menu, pause menu, HUD, results screens and the options for sound, screen and wheel controls. Every screen fades through the loading screen into the next.
- **Replays and Flashback.** Instant Replay with the race cameras and slow motion, and Flashback to rewind and jump back in. Some replay sounds are still missing.
- **Records.** Time Trial records are saved.
- **Graphics.** The original's lighting, shadows, post-processing, tyre marks, smoke and crowds are rebuilt, with differences that remain. Window or fullscreen, resolution, an automatic render scale that holds the frame rate, and shadow quality are settings and player options.
- **Input.** Keyboard, gamepad and steering wheel.
- **Portable releases.** One download with no installer; your data stays in `DRIG-data` next to the program.
- **Android.** An experimental build with tilt and touch steering and gamepad support.

Expect crashes, physics blow-ups, missing features and visual differences from the original. Fidelity work is tracked in [docs/FIDELITY_BACKLOG.md](docs/FIDELITY_BACKLOG.md). What comes next, in order: [docs/ROADMAP.md](docs/ROADMAP.md).

## What you need

- A legally purchased copy of **Race Driver: GRID** (2008) for PC, for example from Steam. The engine finds the Steam installation by itself.
- One of:
  - Windows 10 or 11 with a reasonably modern GPU;
  - Linux on x86_64 with glibc 2.35 or newer (Ubuntu 22.04, Debian 12, SteamOS 3 or newer) and a working Vulkan driver. So far it was tested only in WSLg on Windows 11, not on native Linux hardware;
  - macOS on Apple Silicon.

## Building and running

### Players

Download the release program and start it, as described in [Download and play](#download-and-play). You do not need Rust or any build tools.

### Developers: building from source

You need Rust 1.99.0 (pinned in `rust-toolchain.toml`). The first build compiles Bevy and takes a while.

```
cargo build --release -p grid-app
```

This developer build includes the diagnostic tools (the `dev-tools` feature): `--help-dev` lists the developer options, and the `[diagnostics]` settings section is read ([settings](docs/SETTINGS.md), "Player build and developer build"). The release program is the player build, without them:

```
cargo build --release -p grid-app --no-default-features
```

Then double-click **Play GRID.cmd** in the repository root and pick an event in the Race Day menu ([event menu](docs/EVENT_MENU.md)). The other `Play GRID *.cmd` launchers start one event directly, for example `Play GRID Le Mans 24 Hours.cmd` or `Play GRID Drift Battle.cmd`.

You can also run the executable directly:

```
target\release\grid-app.exe --settings settings\race.ini
```

If your game is not in a Steam library, set `[game] root` in `local\settings.ini` or pass `--game-root PATH` ([settings](docs/SETTINGS.md)).

### Controls

| Key | Action |
| --- | --- |
| Up / W | Throttle |
| Down / S | Brake; hold near rest to reverse |
| Left / A, Right / D | Steer |
| Space | Handbrake |
| X / Z | Gear up / down (manual gearbox) |
| Escape / P | Pause menu |
| R | Restart the event |

More detail, including gamepad, wheel and driver-aid settings: [docs/TRY_THE_PROTOTYPE.md](docs/TRY_THE_PROTOTYPE.md) and [docs/DRIVER_AIDS.md](docs/DRIVER_AIDS.md).

### Android

The Android build is experimental. It runs the same program on arm64 phones (Android 12+), using game files you import from your own copy. See [docs/ANDROID.md](docs/ANDROID.md).

### macOS

Native build and local `.app` packaging: `python3 -I tools/build_macos.py`.
Compilation needs no original game files; playing requires your own Windows GRID
2008 installation. See [docs/MACOS.md](docs/MACOS.md) for setup and validation limits.

## Reporting bugs and testing

Testers are welcome, on any system and any hardware. Play a few events and tell us what breaks, what feels different from the original and how fast it runs. Open an [issue](https://github.com/PipeKnight/drig-race-driver/issues) and attach:

- the run log: every start writes a new file `DRIG-data/playtest/drig-<UTC time>-<revision>.log` (the newest is the last run; the 10 newest are kept). On Android it is `Download/GRID-Reassembly/log-<UTC time>-<revision>.txt`. Builds up to v0.1.0 wrote `DRIG-data/drig.log` and `drig.previous.log` instead;
- your GPU, CPU, graphics driver version and resolution;
- the event, track and car, and what you did just before the problem.

For detailed diagnostics, build the developer version from source ([Developers](#developers-building-from-source)) and run it with `--help-dev` to see its options.

## How it is made

The original `grid.exe` is studied with Ghidra and other reverse-engineering tools. Its behaviour is then written again in Rust and checked against the original: recorded driving, AI and audio traces, plus bit-exact comparisons of individual routines. The port does not reuse original code or bundle original data.

Research notes for each subsystem live in [docs/](docs/). The current state is [PROJECT_LOG.md](PROJECT_LOG.md); the running record of findings, decisions and verification is one file per entry in [log/](log/INDEX.md).

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/grid-formats` | Readers for GRID's file formats (PSSG models and textures, databases, audio banks and more) |
| `crates/grid-world` | Simulation: physics, tyres, collisions, damage, AI |
| `crates/grid-app` | The game itself: Bevy rendering, menus, HUD, audio, input, Android |
| `crates/grid-tools` | Inspection and comparison command-line tools |
| `crates/grid-probes` | One-off research probes (built only on request) |
| `tools/` | Research and diagnostic scripts |
| `docs/` | Research notes and design documents |
| `local/` | Your machine's private files: extracted data, captures, logs. Ignored by Git and never published |

## Contributing

Forks, bug reports and pull requests are welcome. Most of the code is written by AI coding agents; their working rules are in [AGENTS.md](AGENTS.md). Never commit original game files, extracted assets or memory dumps.

After cloning, install the git hooks once: `py -I tools/hooks/install.py` (or `python3 tools/hooks/install.py`). On every commit they refuse game files, binaries, large files, conflict markers, broken Markdown links and scripts that do not parse ([docs/TOOLCHAIN.md](docs/TOOLCHAIN.md), "Git hooks").

## Fidelity backlog progress

The [Fidelity Backlog](docs/FIDELITY_BACKLOG.md) tracks known discrepancies and related tasks.
[GitHub Actions](https://github.com/PipeKnight/drig-race-driver/actions/workflows/fidelity-progress.yml)
reports open and closed items and publishes a JSON report for each run. The percentage is backlog
completion. Game parity and matching decompilation require separate measurements.

## Fixed gameplay scenarios

[GitHub Actions](https://github.com/PipeKnight/drig-race-driver/actions/workflows/scenario-progress.yml)
runs the [versioned scenario suite](tools/scenario_progress.py) and publishes a per-commit summary
and a JSON report. Suite v1 covers lap timing and race classification, flashbacks, drift scoring
and career rules, and reserves two headless driving scenarios that still need automated checks.
Its verified percentage is the share of the fixed scenario list that passed.
It measures headless behavior within that suite. Binary matching, graphics, audio and full races
in the interactive app are outside its scope.

CI runs the scenarios that need no original files. To include scenarios that use the original
career tables and drift settings, run locally with your copy of the game:

```sh
python3 -I tools/scenario_progress.py --game-root "/path/to/GRID" --json scenario-progress.json
```

The report keeps passed, failed and not-run scenarios in the denominator. Changing scenario
membership, inputs or success criteria requires a new suite version so results remain comparable.

## Featured in

1. [Awesome Decomp Games](https://github.com/SubmitGame/awesome-decomp-games)
2. [Game Decompilations](https://github.com/SamidyFR/Game-Decompilations)
3. [Greates Games Decompilations List](https://github.com/Carter54git/Greates-Games-Decompilations-List)
4. [Awesome Decomp Games RU](https://github.com/justxor/awesome-decomp-games-ru/blob/main/catalog/dekompilyacii.md)
5. [recomp.gg](https://recomp.gg/p/drig-race-driver)
6. [PlayPorted](https://playported.com/ports/2690-race-driver-grid/)

<details>
<summary>Pending catalog submissions</summary>

- [Open Source Game Clones](https://github.com/opengaming/osgameclones/pull/5201)
- [Awesome Game Remakes](https://github.com/radek-sprta/awesome-game-remakes/pull/96)
- [Awesome Game Decompilations](https://github.com/CharlotteCross1998/awesome-game-decompilations/pull/65)
- [Awesome Rebuilt Games](https://github.com/no7z/awesome-rebuilt-games/pull/1)
- [Awesome REA (yibie)](https://github.com/yibie/awesome-rea/pull/10)
- [Reverse Engineering Atlas](https://github.com/jammyfu/reverse-engineering-atlas/pull/8)
- [Awesome Open Source Games](https://github.com/djeada/awesome-open-source-games-list/pull/2)
- [Open Source Games List](https://github.com/Trilarion/opensourcegames/issues/517)
- [Awesome REA (kaixinbaba)](https://github.com/kaixinbaba/awesome-rea/issues/454)
- [RemakeList](https://github.com/CharlexH/awesome-remakelist/issues/1)
- [Recompendium](https://github.com/Nio03/unricopie/issues/104)

</details>

## Legal

DRIG is an independent project. It is not affiliated with, endorsed by or connected to Codemasters or Electronic Arts. "Race Driver: GRID" and related names are trademarks of their respective owners. You need your own copy of the original game to play.

New versions of DRIG are distributed under the [GNU General Public License, version 3 or any later version](LICENSE)
(`GPL-3.0-or-later`). Versions already published under Apache 2.0 remain available under those terms.

Third-party code and assets keep their original licenses. Upstream Bevy material is available under
the [MIT](LICENSES/Bevy-MIT.txt) or [Apache 2.0](LICENSES/Apache-2.0.txt) terms. Fira Mono is distributed
under the [SIL Open Font License 1.1](crates/grid-app/assets/fonts/FiraMono-LICENSE).
Original GRID game files are supplied separately by the player.

When publishing binaries under GPLv3, provide the corresponding source for that exact build, including
the scripts needed to build and install it, and retain the applicable third-party notices.

Each release ships `DRIG-vX-LICENSES.txt` beside the programs, with the GPL and the notices of the third-party libraries and the font DRIG includes.

## Disclaimer

This is pure 100% AI slop. I didn't made this by hand at all, it's all thanks to AI agents and I, pipeknight, take ZERO credit or responsibility of this port being unstable. It will run shitty, that's a guarantee. All I did is spent couple of sleepless night testing and pointing out the issues. Oh, yeah, and spent some pocket change on Claude sub ofc. Please be aware of it and feel free to fork this repo or use it as a reference in your future ports. Thank you and have a nice day!
