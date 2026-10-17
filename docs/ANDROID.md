# Android build (P9 skeleton, bundled data, error screen)

Plan and status: [PLATFORMS_AND_INPUT_RESEARCH.md](PLATFORMS_AND_INPUT_RESEARCH.md) section 7 (P9), tools: [TOOLCHAIN.md](TOOLCHAIN.md) "Android toolchain".

## What is in the repository

- `crates/grid-app/src/lib.rs`: the `grid_android` library (`crate-type = ["cdylib"]`). On Android it `include!`s `main.rs`, so the phone runs the same program as the desktop binary. On every other target it is empty (`#![cfg(target_os = "android")]`), so the desktop executable, its tests and Clippy are unchanged.
- `main.rs`:
  - `#[bevy_main]` on `main`. On Android the attribute generates `android_main`, which stores the `AndroidApp` in `bevy::android::ANDROID_APP` and calls `main`. On the desktop the attribute changes nothing.
  - `mod android` (Android only), called first in `main`.
  - On Android the default game folder is `<data dir>/Grid`, not the Steam libraries.
  - `read` (main.rs) and `source_vehicle::read` (grid-tools) name the missing file in their open error, so a missing pushed file shows up in logcat.
- `crates/grid-app/src/android.rs`:
  - redirects stdout and stderr to logcat (tag `grid`) and to a log file in the shared Downloads folder (below);
  - installs a panic hook; a start-up error (`run()` returning `Err`) or a panic is shown on screen through `MainActivity.showFatal` (JNI, `jni` 0.21, the version cpal already uses);
  - `data_dir()` = `AndroidApp::external_data_path()`, which is `/sdcard/Android/data/org.reassembly.grid/files`. `settings::platform_data_dir` uses it (P5), so `settings.ini`, records, renders and the ETC2 texture cache (P8) live there.
- `settings::load` skips a UTF-8 byte-order mark at the start of a settings file. Windows PowerShell 5 writes one, and Windows editors may.
- `android/`: a Gradle project based on Bevy 0.19.1 `examples/mobile/android_example`:
  - AGP 9.4.1, `compileSdk`/`targetSdk` 36, `minSdk` 31, `arm64-v8a` only, `ndkVersion` 30.0.16248370;
  - `androidx.games:games-activity:4.4.0`, the version whose C sources android-activity 0.6.1 bundles (its README), plus appcompat 1.7.1;
  - Java classes in `org.reassembly.grid`:
    - `LauncherActivity` (the launcher entry): copies bundled game data on the first start (below), then starts `MainActivity`. Without bundled data it starts the game at once.
    - `MainActivity extends GameActivity` (not exported): loads `libgrid_android.so`, keeps the screen on, hides the system bars; landscape (`sensorLandscape`). `openLogFile()` and `showFatal(String)` are called from Rust. When it finishes it ends its process, because winit and Bevy cannot start twice in one process.
    - `ErrorActivity`, in its own process `:error`, so it stays on screen when the game process exits or aborts: the message, the log file path and a Close button.
    - `BundleExtractor`: the copy logic, plain Java; `CrashLog`: log files through MediaStore.
  - desktop unit tests: `app/src/test/.../BundleExtractorTest.java` (JUnit 4.13.2), run with `gradle -p android testDebugUnitTest`;
  - the Rust library comes from `app/build/rust-jniLibs` (ignored output). Game data is packaged only with `-PgridBundleAssets` (the private bundled build); `ignoreAssetsPattern` is `!.svn:!.git`, so files starting with `.` and folders starting with `_` are not dropped.
- `tools/build_android.ps1`: builds the APK (below).
- `tools/push_android_data.ps1`: copies the data to the phone, or with `-Stage DIR` to a desktop folder for checking the file list.

## Build

```
powershell -NoProfile -File tools\build_android.ps1
```

1. `cargo ndk -t arm64-v8a -P 31 -o android/app/build/rust-jniLibs build --release --lib -p grid-app`. The release profile: a debug Bevy is too slow to drive. The `GRID_BUILD_REVISION` is set as in `tools/update_prototype.ps1`.
2. `gradle -p android --no-daemon assembleDebug` with the pinned Gradle, JDK and SDK. `GRADLE_USER_HOME` is `C:\Users\debros\Tools\gradle-home`.
3. The APK is copied to `local/android/apk/grid-<revision>-debug.apk`, with a `.json` manifest: revision, time, SHA-256 of the APK and of `libgrid_android.so`, rustc version and tool paths.

"Debug" means signed with this PC's Android debug key (`%USERPROFILE%\.android\debug.keystore`, created by AGP on first use). The key differs per machine, so APK hashes differ between machines; the `.so` hash does not depend on it.

### Private bundled APK (`-BundleData`)

> **For the owner's own phone only. Never share, upload or commit this APK.** It contains about 245 MB of proprietary files from the owner's purchased GRID installation. AGENTS.md forbids publishing original assets; this file is the owner's private copy, like the Steam folder itself.

```
powershell -NoProfile -File tools\build_android.ps1 -BundleData
```

1. `tools/push_android_data.ps1 -Stage … -NoAutopilot` copies the same subset as the adb path (table below) into `local/android/bundle/assets/grid-data/` (ignored; the script refuses a folder git does not ignore). The original installation is only read. `-NoAutopilot` leaves out `[diagnostics] autopilot = on`, so the bundled APK uses whatever controls profile the code selects by default (P11 makes tilt/touch the default on a touch screen). Desktop check [V]: with that `settings.ini` the release binary starts with `drive_input device=auto` and no autopilot.
2. It writes `grid-data/bundle.txt`: a header `grid-bundle 1`, `version <sha256>` (over every file's SHA-256, size, mode and path), then one `size<TAB>mode<TAB>path` line per file. `settings.ini` has mode `keep` (written only when missing, so the owner's edits survive an update); everything under `Grid/` is `replace`.
3. Gradle gets `-PgridBundleAssets=<that folder>`, so the files go into the APK under `assets/grid-data/`.
4. The APK is `local/android/apk/grid-<revision>-bundled-PRIVATE-debug.apk`; the `.json` manifest adds `bundled_data` (version, file count, bytes).

The default build (no switch) is unchanged and packages no game data. Gradle keeps a merged copy of the assets in `android/app/build/intermediates` (about 245 MB); delete `android/app/build` and `local/android/bundle` when disk space matters.

**Measured (F365):** 382 files, 256,504,419 bytes of data (245 MiB), stored compressed as 143 MB; the APK is about 233 MB (223 MiB), of which the stripped `libgrid_android.so` is 82.8 MB. Exact size and hashes of the delivered APK: PROJECT_LOG.md F365. Trimming Donington's other routes and Le Mans layout (about 23 MB, under 10 %) was not done: the subset is already one car and one circuit, and a trim would need a new desktop check.

**Desktop checks [V]:** the APK was unpacked with the app's own `BundleExtractor` (a desktop Java runner reading the APK as a zip): 382 files byte-identical to the staged folder, 2.2 s. The desktop release binary on that unpacked folder (`--data-dir`, `--game-root`, 600 autopilot ticks, render): log identical to a run on the full installation apart from timestamps and paths, and identical drive samples.

### First start: copying the data

`LauncherActivity` reads `assets/grid-data/bundle.txt`. If `Android/data/org.reassembly.grid/files/.bundle-version` holds the same version, it starts the game at once. Otherwise it shows "First start: copying the game data" with a progress bar (MB done of total), copies every file to `files/` (each through `name.part`, renamed when its size matches), then writes the marker and starts the game. The marker is deleted before copying, so an interrupted copy (app killed, storage full) starts again on the next launch. Files of an older bundle that the new one no longer lists are not deleted. A failed copy shows the error on screen and writes `Download/GRID-Reassembly/extract-<time>.txt`.

All loaders still read from `files/` (the same layout as the adb push), so the bundled and adb paths can be mixed. Uninstalling the app deletes `files/`; reinstalling the bundled APK copies again.

Unit tests (desktop JVM, `gradle -p android testDebugUnitTest`, 7 tests [V]): first copy and marker; same version skipped; new version replaces game files but keeps an edited `settings.ini`; a deleted `settings.ini` is written again; an interrupted copy leaves no marker; a size mismatch fails without replacing the old file; `bundle.txt` paths that would leave the data folder and malformed lines are rejected.

## Errors and the log file

- **Log file:** every start creates `Download/GRID-Reassembly/log-<yyyyMMdd-HHmmss>.txt` in the phone's shared Downloads folder (Files / File Manager → Downloads → GRID-Reassembly). It is written through MediaStore, which needs no permission on Android 10+. It gets everything the game prints (stdout and stderr), errors marked `E`, each line with seconds since start. The ten newest files are kept; older ones are deleted. Logcat (`adb logcat -s grid`) still gets the same lines.
- **On-screen error:** a start-up error (for example missing game data) or a Rust panic opens a "GRID Reassembly stopped" screen with the message and the log file path. It runs in its own process, so it stays when the game process exits. "Close" ends both processes.
- Without game data the message is "no game data in …/files/Grid: this APK carries none; …" (an Android-only check in `run()`; on the desktop the first loader's error is unchanged).
- Not covered: a crash inside Java or the system before the Rust code starts (for example the library failing to load) appears only in logcat.
- [V] Java compiles, both APK variants build, the extraction tests pass, `cargo ndk … clippy -D warnings` is clean. [N] The error screen, the log file and the copy have not been seen on a phone; no device or ARM emulator was available.

## Installing the APK yourself (no adb)

1. Copy the APK to the phone: connect it by USB in "File transfer" mode and copy the APK (from `C:\Dev\Reassembly\local\android\apk\`) into `Download`. A cloud drive also works, but only your own private one: the bundled APK must not be shared.
2. On the phone, open the file in Files (Xiaomi: File Manager) and tap it. Android asks you to allow that app to "install unknown apps"; allow it once.
3. If Play Protect warns about an unknown developer, choose "Install anyway". The APK is signed with this PC's debug key.
4. The app is called **GRID Reassembly**. To update it, install a newer APK over it. That works while the signing key stays the same (APKs built on this PC). If Android says "App not installed" or reports a conflicting package, uninstall the old app first (this deletes its data folder; the bundled APK copies it again).

Requirements: Android 12 (API 31) or newer and a 64-bit ARM phone; all three owner phones qualify. The code-only APK is about 90 MB; the bundled one about 233 MB and needs about 245 MB more free space for the copied data.

### What does not work yet

- **Controls (P11):** tilt steering with on-screen pedals and buttons; see "Controls" below. Not yet tried on a phone. A Bluetooth keyboard may also work through GameActivity, but this is untested.
- **Gamepads:** not available on Android. Bevy's gamepad input comes from gilrs, and gilrs-core 0.6.8 has no Android backend (`src/platform` has linux, macos, wasm and windows only; other targets get a dummy). Android delivers pad buttons and sticks as KeyEvent/MotionEvent through GameActivity, but winit 0.30.13 maps pad buttons to `Key::Unidentified` ("Gamepad events should be exposed through a separate API", `platform_impl/android/keycodes.rs`) and does not forward joystick axes. A pad needs a bridge from android-activity's input events into the P1 input layer, which is input code (P11's "optional Android pad bridge"), so it was not done here. A Bluetooth keyboard may work through winit, but this is untested.
- **Data without the bundled APK:** the code-only APK needs the data placed by adb (below). Android 11+ blocks phone file managers from `Android/data`; copying from Windows Explorer over USB (MTP) into it works on many phones but is not guaranteed [S] (PLATFORMS_AND_INPUT_RESEARCH.md section 3). **P10** (folder picker) would remove the PC step without bundling.
- **Not measured:** frame rate, tick time, thermal behaviour and memory on a phone (P12). Audio is not paused on suspend yet, so it may keep playing in the background [I].

## Controls (P11)

Tilt and touch are not original GRID inputs, so the layout and the defaults are AUTHORED. They feed the original action layer (F325) like keyboard keys and a pad stick, so the human record 827D10, assisted steering (+E6C = 1, the keyboard/pad class) and the driver aids are the same as on the desktop. Design: [PLATFORMS_AND_INPUT_RESEARCH.md](PLATFORMS_AND_INPUT_RESEARCH.md) 6.2 and 7.7.

**Holding the phone.** Landscape, screen towards you, like a steering wheel. Turn it clockwise to steer right. Leaning it back or forward does not steer. The neutral angle is taken from the first sensor reading, and again whenever you tap **CENTRE**; hold the phone the way you want "straight" and tap it.

**Screen zones** (fractions of the screen; the race HUD draws on top of them):

| Zone | Where | Action |
|---|---|---|
| GAS | bottom right, 17 % × 50 % | Accelerate, held (0 or 1) |
| BRAKE | bottom left, 17 % × 50 % | Brake, held (0 or 1) |
| HAND | right of BRAKE | Hand Brake, held |
| GEAR + / GEAR − | left of GAS, only with the manual gearbox | Gear Up / Gear Down, one shift per tap |
| II | top centre | Pause (opens the pause menu; in the menu it goes back one step) |
| CENTRE | left of II, only with tilt | re-centre the tilt neutral |
| VIEW / LOOK | bottom centre | Change View; Look Back while held |

A thin bar above VIEW/LOOK shows the tilt value. While the pause menu, the post-race screens or a replay own the input, the driving zones are hidden and the menu zones appear at the side edges: UP/DOWN and `<`/`>` on the left, OK and BACK on the right (BACK also leaves a replay). A finger can slide from one zone to another; the zone under it counts each frame. A finger that is down when the layer changes (OK resumes the race, II pauses it) is ignored until lifted, so OK does not turn into GAS. At most 8 touches count (Bevy issue #16798).

**Settings** (`[input]` section of `settings.ini`, docs/SETTINGS.md):

| Key | Values | Default |
|---|---|---|
| `touch` | `auto`, `on`, `off` | `auto`: on when Android reports a finger touch screen; off on the desktop |
| `tilt` | `on`, `off` | `on` (with touch). Off: no steering source on the phone yet |
| `tilt_max_angle` | degrees 5..90 | 30: full lock at 30° from neutral |
| `tilt_neutral` | degrees −90..90, or `auto` | `auto` (captured at start and by CENTRE) |
| `tilt_smoothing` | seconds 0..1 | 0.05 (one-pole low-pass; 0 = off) |

The tilt dead zone and saturation are those of the steering bindings: biLower/biUpper dz 0.05, sat 1.0, the values the original's 4CC630 gives steering on controller types 1–3. The Advanced Controls `steering_deadzone`/`steering_saturation` (settings file or pause menu) override them, as for every steering binding.

**Sensor.** `TYPE_GAME_ROTATION_VECTOR` at 100 Hz through the NDK (`ndk-sys` 0.6, the version android-activity uses), on its own thread; the accelerometer if the phone has no rotation vector. It is switched off while the app is suspended. Logcat shows `drive_tilt sensor=Rotation period_us=10000`, and `drive_tilt recentre neutral_deg=...` on each CENTRE tap. Which landscape the phone is in (top edge left or right) is inferred from gravity, because the NDK has no display-rotation API (a JNI call to `Display.getRotation` would be the alternative now that `jni` is a dependency, F365).

**UI scale.** With touch on, the UI scale becomes window height / 720 (at most 1), so the desktop-sized race HUD and pause menu fit a phone's small logical window. Not used on the desktop.

**Older settings.ini.** The bundled APK copies `settings.ini` only when it is missing, so a phone that installed the F365 bundle keeps its file (with `autopilot = on` and no `[input]`). The controls still start, because `touch = auto` and `tilt = on` are the defaults when the keys are absent; the autopilot line does nothing without `--drive-ticks`.

**Desktop.** `[input] touch = on` shows the zones on the desktop too (Bevy touch events, e.g. a touch monitor); there is no tilt sensor there. With the default `touch = auto` nothing changes on the desktop.

**Not tested on a phone** (no device was connected): the sensor sign and the landscape inference, the zone sizes under real thumbs, the UI scale, multi-touch throttle + brake, the pause menu by touch, and whether GameActivity delivers touches to Bevy as expected.

## Game data on the phone (development path; P10 replaces it)

The phone needs a subset of the owner's own installation, about 245 MB. No private table is needed: CD01 (F359), BP01 and DF01 are derived in memory from the pushed `grid.exe`. Nothing of it goes into Git; only the private bundled APK (above) packs it.

```
powershell -NoProfile -File tools\push_android_data.ps1
```

It runs `adb push --sync` into `/sdcard/Android/data/org.reassembly.grid/files/`:

| Path on the phone | Source |
|---|---|
| `Grid/...` | the items listed in the script, from `C:\Program Files (x86)\Steam\steamapps\common\Grid` (read only) |
| `settings.ini` | written by the script: drive on, CD01/BP01/DF01 `derive` (computed in memory from the pushed `grid.exe`), `[input] touch = auto`, `tilt = on`, `tilt_max_angle = 30` (P11). Until P11 it set `[diagnostics] autopilot = on`, but that switch only acts with `--drive-ticks`, which the phone never passes, so the P9 car would have stood still [V code: `main.rs` builds the autopilot only when `drive_ticks.is_some()`] |

The app must be installed first: Android creates the folder for the package, and `adb shell` may write to it. Uninstalling the app deletes it.

The list was found by running the desktop release binary on a staged copy (`-Stage local/android/stage`, `--data-dir` and `--game-root` pointing there) until a 600-tick autopilot drive rendered.
