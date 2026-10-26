# Native macOS build

Build the Rust engine on a Mac and package it as a code-only `DRIG.app`.
Original GRID files are not needed to compile or package the application.
They are needed to open the event menu and drive. This does not convert or run
the original Windows executable.

## Prerequisites and build

- Xcode Command Line Tools (`xcode-select --install` if absent).
- Rust 1.99.0, as pinned in `rust-toolchain.toml`; Python 3 for packaging.
- A native Rust compiler: `aarch64-apple-darwin` for Apple Silicon or
  `x86_64-apple-darwin` for Intel. Leave Cargo's `build.target` and
  `CARGO_BUILD_TARGET` unset for this helper.

From the repository root:

```sh
python3 -I tools/build_macos.py
```

The helper runs `cargo build --release --locked -p grid-app --bin grid-app`,
checks that the executable contains the host architecture, and creates
`target/release/DRIG.app` (or under Cargo's configured target directory).
`--no-build` packages an existing native release binary. The first build
downloads and compiles Bevy and can take a while. Build only one Cargo job
at a time. The Windows seed, build lock and pre-push PowerShell scripts refer
to the Windows machine's build directories; they are not macOS prerequisites.
The Windows-only linker setting in `.cargo/config.toml` does not affect macOS.

The bundle is ad-hoc signed for local use; no paid Apple developer account is
needed. It is not a notarized distribution build or a universal Intel/ARM app.
Fonts and shaders are embedded in the engine; no original assets are bundled.
Rebuild the bundle when its running instance has exited.

## Checks before downloading the game

```sh
target/release/grid-app --help
```

This checks executable startup and argument parsing without opening a window.
Compilation and this check do not verify Metal rendering, audio, gamepad input,
physics parity, or performance. Real menu and race checks require the game data.

## After copying the game

Copy the complete Windows Steam installation of GRID 2008, preserving its
directory structure, including `grid.exe` (private parameter tables are read
from it; it is not executed). Keep the files outside Git.

For a first run, supply an absolute folder path:

```sh
target/release/grid-app --menu --game-root "/absolute/path/to/Grid"
```

For double-click launching, create
`~/Library/Application Support/GRID Reassembly/settings.ini` containing:

```ini
[game]
root = /absolute/path/to/Grid
```

Then open `target/release/DRIG.app`. The bundle opens Race Day and keeps settings,
records and diagnostics in `~/Library/Application Support/GRID Reassembly/`.
Each launch writes a separate `playtest/macos.*` log there. Its launcher fixes
the working directory and passes an explicit data directory, so Finder and the
menu's child processes use the same settings. The Steam installation finder
also already searches `~/Library/Application Support/Steam` on macOS.

Until actual game files are available, a missing-installation error is expected;
it is not evidence that a race or a graphics backend has been tested.

## Validation on 26 October 2026

On Apple Silicon, macOS 15.7.7 (24G720), Rust/Cargo 1.99.0 and Python 3.12.5:

- The unmodified engine at `5deae45dca29` built with
  `cargo build --release --locked -p grid-app` in 4 minutes 13 seconds.
- The result is a native arm64 Mach-O executable. Its dynamic dependencies
  resolve to system libraries/frameworks, including Metal; no Homebrew dylib
  is linked. `grid-app --help` exited successfully.
- Both packaging modes passed; `codesign --verify --deep --strict` accepted
  the bundle. The launcher passed `sh -n`, and its plist points to that launcher.
- The bundled binary's `--menu` supervisor started its child, which exited
  with the expected missing-Steam-installation error. No window or race was
  opened. Finder double-click launching is still unverified.

The engine needed no Windows/ARM source changes for this compilation.
Intel macOS builds, actual Metal rendering, audio, input, numerical fidelity
and frame rate remain untested.
