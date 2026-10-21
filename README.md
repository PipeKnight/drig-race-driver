# DRIG: an open-source engine for Race Driver: GRID (2008)

DRIG is a reimplementation of Codemasters' Race Driver: GRID (2008) in Rust with the [Bevy](https://bevyengine.org/) engine. The goal is a 1:1 recreation: the same handling, AI, damage, events and menus as the original game, running on a modern, open codebase.

The repository contains **no game assets**. The engine reads tracks, cars, textures, sounds and game data from your own installed copy of GRID, the same way OpenMW uses Morrowind's data files or OpenRCT2 uses RollerCoaster Tycoon 2's.

<img width="1280" height="720" alt="image" src="https://github.com/user-attachments/assets/f46aebc0-85d4-422a-9b75-f2b3b9c99744" />

I also work on graphics improvement, testing technologies like DLSS/DLAA, RT etc

<img width="820" height="370" alt="moving-bias-native01" src="https://github.com/user-attachments/assets/bec7f111-4d29-4e80-be70-c2d5ed45c4f9" />

## Status

Early and experimental, but you can already race. What works today:

- **Driving.** The vehicle physics, tyres, suspension, gearbox and driver aids (traction control, stability control, braking assist, automatic or manual gearbox) are ported from the original code.
- **Events.** Circuit races, Time Trial, Drift (Battle, GP, Freestyle), Touge, Demolition Derby, the Le Mans 24 Hours and the championship series (Touring, GT1, GT2, Pro Tuned, Open Wheel, Prestige, Le Mans Series and others). You choose them from the original Race Day menu.
- **AI opponents** that follow the original racing lines, overtaking and drift logic.
- **Damage.** Body contacts, collision damage, wheel detachment, mechanical and handling damage.
- **Audio.** Engine, gear changes, tyres and collisions.
- **Career.** Its research and a first port are in progress.
- **Front end.** The pause menu, HUD, results screens and the options for sound, screen and wheel controls.
- **Input.** Keyboard, gamepad and steering wheel.
- **Android.** An experimental build with tilt and touch steering and gamepad support.

Expect crashes, physics blow-ups, missing features and visual differences from the original. Fidelity work is tracked in [docs/FIDELITY_BACKLOG.md](docs/FIDELITY_BACKLOG.md).

## What you need

- A legally purchased copy of **Race Driver: GRID** (2008) for PC, for example from Steam. The engine finds the Steam installation by itself.
- Windows 10 or 11 with a reasonably modern GPU.
- To build from source, Rust 1.99.0 (pinned in `rust-toolchain.toml`). The first build compiles Bevy and takes a while.

## Building and running

```
cargo build --release -p grid-app
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

## Legal

DRIG is an independent project. It is not affiliated with, endorsed by or connected to Codemasters or Electronic Arts. "Race Driver: GRID" and related names are trademarks of their respective owners. You need your own copy of the original game to play.

The source code is licensed under the [Apache License 2.0](LICENSE).

## Disclaimer

This is pure 100% AI slop. I didn't made this by hand at all, it's all thanks to Claude and I, pipeknight, take ZERO credit or responsibility of this port being unstable. It will run shitty, that's a guarantee. All I did is spent couple of sleepless night testing and pointing out the issues. Oh, yeah, and spent some pocket change on Claude sub ofc. Please be aware of it and feel free to fork this repo or use it as a reference in your future ports. Thank you and have a nice day!
