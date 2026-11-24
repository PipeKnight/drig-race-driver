# Native Linux build

Builds the Rust engine for Linux x86_64 and packages it as a code-only `.tar.gz` and AppImage
(backlog row 135). The packages contain only our code and licences, never GRID files. To drive,
the player points the engine at a GRID installation they own (Steam on Linux runs the game through
Proton, or a copy from Windows). This does not run or convert the original Windows executable.

## Packages

| File | Contents |
|---|---|
| `DRIG-v<ver>-linux-x86_64.tar.gz` | folder `DRIG-v<ver>-linux-x86_64/` with `drig`, `LICENSE`, `LICENSES/`, `README.txt`, `build.json` |
| `DRIG-v<ver>-linux-x86_64.AppImage` | the same files in a type 2 AppImage (squashfs); start with `chmod +x` and run |
| `SHA256SUMS.txt` | one line per package |

The tarball also suits launchers inside a Flatpak sandbox, where an AppImage cannot mount itself.

### Data folder

Both packages are portable (row 129, `settings::portable_anchor`). The tarball keeps
settings, records, replays and logs in a folder next to `drig`. The AppImage keeps them next to
the `.AppImage` file: its runtime sets `$APPIMAGE` to that file and the engine anchors the data
folder there, never inside the read-only mounted image. `--data-dir DIR` overrides both.

### Game files

`--game-root` defaults to discovery (`grid_tools::install::steam_roots`/`discover`). It looks in
`~/.steam/steam`, `~/.local/share/Steam` and the Flatpak Steam
(`~/.var/app/com.valvesoftware.Steam/.local/share/Steam`), plus every library their
`steamapps/libraryfolders.vdf` lists. It also checks the Windows Steam client in a Wine prefix
(`$WINEPREFIX` or `~/.wine`, `drive_c/Program Files (x86)/Steam`). Steam with Proton installs GRID
in the ordinary library (`steamapps/common/Grid`); the Proton prefix `compatdata/12750` holds only
the original's saves and is not needed.

Case: GRID's files have Windows spellings (`Osd`, `ActionMap`, ...), and Linux file systems are
case-sensitive. The engine opens data through manifest spellings and the case-insensitive
`FileIndex`; a Donington race from a case-sensitive ext4 copy had no case-only misses (see "What
was tested").

## Requirements

- x86_64 and glibc 2.35 or newer (`min_glibc` in `build.json`; Ubuntu 22.04, Debian 12, SteamOS 3
  and newer). Derived from the binary's symbol versions: the newest non-weak one is GLIBC_2.35
  (`hypot`); Rust's std references `pidfd_spawnp@GLIBC_2.39` weakly and falls back without it.
  Not run on an older distribution.
- A Vulkan driver (Mesa RADV/ANV/NVK/lavapipe or a vendor driver).
- X11 or Wayland.
- ALSA: PulseAudio and PipeWire provide it.
- The AppImage needs FUSE 2 (`libfuse2`), or run it as `./DRIG-....AppImage --appimage-extract-and-run`.

## Building

### Build host (WSL2 on the build PC)

Ubuntu 24.04 in WSL2. Build on the ext4 filesystem (`~/drig-build`), never on `/mnt/c`.

```sh
sudo apt-get install build-essential pkg-config git curl ca-certificates file xz-utils \
  libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libx11-dev libxcursor-dev \
  libxi-dev libxrandr-dev libvulkan1 mesa-vulkan-drivers vulkan-tools libfuse2t64 desktop-file-utils
curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain none
rustup toolchain install 1.99.0 -c rustfmt -c clippy   # rust-toolchain.toml selects it
```

Versions recorded in the F-entry that added this document.

### Packaging

From Windows, one commit, under the build lock:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\package_linux.ps1 -Commit <sha>
```

This checks out exactly that commit in `~/drig-build` and runs `tools/package_linux.sh`. The
packages go to the main checkout's `local/release/v<ver>/`. On any Linux machine, from a clean
checkout:

```sh
tools/package_linux.sh --version 0.1.0 --out OUT --tools TOOLCACHE
```

The script builds the player build, `cargo build --release --locked -p grid-app
--no-default-features` (no developer tooling, row 132; `--cargo-args` replaces the feature
arguments), with `GRID_BUILD_REVISION` set so `drig --version` prints
`DRIG <ver> (revision <12 hex>)`, plus `-dirty` for a test package from a changed checkout. It fetches the pinned
appimagetool 1.9.1 and AppImage type2-runtime 20251108 from their official GitHub releases into
the tool cache, and checks their SHA-256 on every use. Tar entries are sorted with fixed owner and
mtime (the commit time), and gzip carries no name or time. Ends with a `VERDICT` line.

## What was tested

Only WSL2 with WSLg on the build PC (Windows 11, WSL 3.0.5.0, WSLg 1.0.79, kernel
6.18.58.1-microsoft-standard-WSL2, Ubuntu 24.04.5). Its only Vulkan device is Mesa's CPU renderer:
`renderer adapter="llvmpipe (LLVM 20.1.2, 256 bits)" backend=Vulkan` (Mesa 25.2.8), about
3 frames per second. **No real GPU driver was tested**; this says nothing about AMD, Intel or
NVIDIA drivers on a real Linux machine. The F-entry that added this document has the exact runs.

| Check | Result |
|---|---|
| `cargo build --release -p grid-app` (Rust 1.99.0) | builds unchanged, 0 warnings |
| `--help` (plain binary, AppImage with FUSE, AppImage `--appimage-extract-and-run`) | rc 0 |
| Windowed Donington race, scripted 300 ticks, game root on `/mnt/c` (the Windows install, read-only) | `drive_script_complete ticks=300` |
| The same from a case-sensitive ext4 copy of the install, under `strace` | 300 ticks, same final pose; 4 missing paths, all absent in any casing (optional files), 0 case-only misses |
| The same race from the AppImage (FUSE) | 300 ticks |
| Packages | tar listing and `build.json` as specified, `min_glibc` 2.35 |
| v0.1.0 candidate files (player build of e4e5b0c9): `--version`, `--help` (tarball, AppImage with FUSE, AppImage extract-and-run) | `DRIG 0.1.0 (revision e4e5b0c9b99e)`, rc 0, no developer flags |
| v0.1.0 candidate files: the race above without `--data-dir` (tarball and AppImage with FUSE) | 300 ticks; `DRIG-data` created next to `drig` and next to the `.AppImage` |
| v0.1.0 release files (player build of 1ed1b050): `--version`, `--help`, menu start with only `--game-root PATH`, `--list-tracks --game-root PATH`, no Steam library present; the race above from the tarball | `DRIG 0.1.0 (revision 1ed1b05002e4)`; `built screen=Menu ok=true`; 77 routes, rc 0; 300 ticks (F908) |

Not tested:
- real GPUs and their drivers, frame rate, X11 versus Wayland differences on real desktops
  (WSLg offers both; winit chose its default);
- audio: WSL has no ALSA card (`cannot find card '0'`), the engine runs without sound;
- gamepads, wheels and force feedback;
- glibc older than 2.39 at run time, other distributions, Steam Deck;
- other tracks and cars on case-sensitive storage (Android has long read the same data through
  the same lookups).

The game folder can also be set as `[game] root = PATH` in the data folder's `settings.ini`
(docs/SETTINGS.md); `--game-root` overrides it. The player build ignores the `GRID_GAME_ROOT`
variable, like every `GRID_*` variable (F906).
