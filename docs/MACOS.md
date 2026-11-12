# Native macOS build

Build the Rust engine on a Mac and package it as a code-only `DRIG.app`.
The optional private portable mode embeds your purchased game files and creates a DMG.
Original GRID files are not needed to compile or package the application.
They are needed to open the event menu and drive. This does not convert or run
the original Windows executable.

## Prerequisites and build

### One-click rebuild from fresh main

On the owner's Mac, double-click **Rebuild GRID.command** in the checkout root.
It fetches `origin/main`, freezes its exact revision in a temporary detached
clone, builds the native release and packages a new private portable DMG with
the resources from `/Applications/DRIG.app/Contents/Resources/Grid` and the
DRIG icon. It does not switch, reset or stash the current branch, install over
a running game, publish a release, or alter the original game folder.

Results appear in `local/macos-rebuilds/<time>-<revision>-<unique-id>/`:
the DMG, SHA-256 sidecar, `build.json` provenance/status and `build.log`.
Each run uses a new directory; previous successful builds remain available.
Temporary sources, app staging and compilation artifacts are removed after
success or failure. A failed run retains diagnostics and removes its partial
image. First compilation may take several minutes.

```sh
python3 -I tools/rebuild_macos.py --dry-run
python3 -I tools/rebuild_macos.py
python3 -I tools/rebuild_macos.py --code-only
python3 -I tools/rebuild_macos.py --game-root "/absolute/path/to/Grid"
python3 -I tools/rebuild_macos.py --keep-cache
```

`--dry-run` fetches and checks the current main/toolchain/resources/free space
without compiling. `--code-only` omits original game data. `--keep-cache` retains
`local/macos-build-cache/target` to speed later builds; omit it for automatic
cleanup. Both modes are local only. A lock prevents two instances of this
rebuild script from compiling at the same time. If main pins a Rust version not
installed on this Mac, the error prints the required rustup installation command.
Packaging uses the maintained local helper/icon and the frozen main revision's
engine, project license and font license; `build.json` records the engine source.

The installed app is used when it contains the required GRID files. The script
then falls back to `local/macos-portable/source/Grid.reference`,
`local/macos-portable/source/Grid`, and the folder recorded in the app's
`game-root.txt`. Use `--game-root` to override that search. If your private
`DRIG.app` is in another location, pass its `Contents/Resources/Grid` folder.

### Manual build

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
Fonts and shaders are embedded in the engine; by default no original assets are bundled.
Rebuild the bundle when its running instance has exited.
The packaging helper includes `crates/grid-app/assets/macos/DRIG.icns` as the
application icon when present; its generated source image is `DRIG.png` beside it.

For a public code-only app, the first ordinary launch asks for the user's own
Windows GRID 2008 installation folder. Its location is saved as `game-root.txt`
in the user data directory; `game-location.ini` supplies that location to the
menu and race. Remove `game-root.txt` to select a different folder next time.
Explicit command-line launches keep their arguments and do not open the picker.
No original game files are copied into a code-only app.

## Private portable app and DMG

To embed your purchased Windows GRID installation and create a compressed disk
image, use a fresh output directory under the repository's ignored `local/`:

```sh
python3 -I tools/build_macos.py \
  --game-root "/absolute/path/to/Grid" \
  --output-dir local/macos-private/stage \
  --dmg local/macos-private/GRID-macos.dmg
```

`--no-build` can reuse a release binary you have just built. The complete game
folder, including `grid.exe`, is copied into `Contents/Resources/Grid` using
APFS copy-on-write clones. The original executable is read for parameter tables,
not executed. Use an APFS workspace for this packaging path. Existing private
bundles and DMGs are not overwritten; choose a fresh destination for a new build.

The launcher discovers the embedded game relative to itself, so moving the app
does not depend on the source folder, Steam, Rust or this checkout.
Open the image and drag `DRIG.app` to Applications
or another local folder, then double-click it. This is a private build containing
proprietary game resources: do not upload the image or application to GitHub.

Settings, saves, caches and launch logs remain in
`~/Library/Application Support/GRID Reassembly/`; the app bundle stays read-only.
For isolated diagnostics, `GRID_DATA_DIR` overrides that directory. With no
arguments the launcher opens Race Day; explicit arguments allow bounded checks
such as `--headless --drive --drive-ticks 1000`. `build.json` inside Resources
records the source revision, architecture and whether game data is embedded.

The DMG contains the complete app and a shortcut to Applications. Its checksum
is verified after creation. Signing is local/ad-hoc; this is not an Apple-notarized
release and it is not a combined Intel/Apple Silicon build.

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
