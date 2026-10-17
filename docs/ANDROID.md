# Android build (P9 skeleton)

Plan and status: [PLATFORMS_AND_INPUT_RESEARCH.md](PLATFORMS_AND_INPUT_RESEARCH.md) section 7 (P9), tools: [TOOLCHAIN.md](TOOLCHAIN.md) "Android toolchain".

## What is in the repository

- `crates/grid-app/src/lib.rs`: the `grid_android` library (`crate-type = ["cdylib"]`). On Android it `include!`s `main.rs`, so the phone runs the same program as the desktop binary. On every other target it is empty (`#![cfg(target_os = "android")]`), so the desktop executable, its tests and Clippy are unchanged.
- `main.rs`:
  - `#[bevy_main]` on `main`. On Android the attribute generates `android_main`, which stores the `AndroidApp` in `bevy::android::ANDROID_APP` and calls `main`. On the desktop the attribute changes nothing.
  - `mod android` (Android only), called first in `main`.
  - On Android the default game folder is `<data dir>/Grid`, not the Steam libraries.
  - `read` (main.rs) and `source_vehicle::read` (grid-tools) name the missing file in their open error, so a missing pushed file shows up in logcat.
- `crates/grid-app/src/android.rs`:
  - redirects stdout and stderr to logcat (tag `grid`);
  - `data_dir()` = `AndroidApp::external_data_path()`, which is `/sdcard/Android/data/org.reassembly.grid/files`. `settings::platform_data_dir` uses it (P5), so `settings.ini`, records, renders and the ETC2 texture cache (P8) live there.
- `settings::load` skips a UTF-8 byte-order mark at the start of a settings file. Windows PowerShell 5 writes one, and Windows editors may.
- `android/`: a Gradle project based on Bevy 0.19.1 `examples/mobile/android_example`:
  - AGP 9.4.1, `compileSdk`/`targetSdk` 36, `minSdk` 31, `arm64-v8a` only, `ndkVersion` 30.0.16248370;
  - `androidx.games:games-activity:4.4.0`, the version whose C sources android-activity 0.6.1 bundles (its README), plus appcompat 1.7.1;
  - `MainActivity extends GameActivity`: loads `libgrid_android.so`, keeps the screen on, hides the system bars; landscape (`sensorLandscape`);
  - the Rust library comes from `app/build/rust-jniLibs` (ignored output). No game data or asset folder is packaged.
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

## Installing the APK yourself (no adb)

1. Copy the APK to the phone: connect it by USB in "File transfer" mode and copy `grid-<revision>-debug.apk` (from `C:\Dev\Reassembly\local\android\apk\`) into `Download`. Sending it to yourself or using a cloud drive also works.
2. On the phone, open the file in Files (Xiaomi: File Manager) and tap it. Android asks you to allow that app to "install unknown apps"; allow it once.
3. If Play Protect warns about an unknown developer, choose "Install anyway". The APK is signed with this PC's debug key.
4. The app is called **GRID Reassembly**. To update it, install a newer APK over it. That works only while the signing key stays the same (the same PC). Otherwise uninstall first, which also deletes the pushed data.

Requirements: Android 12 (API 31) or newer, a 64-bit ARM phone (all three owner phones qualify).

### What the app needs, and what does not work yet

- **The game data is not in the APK.** The app reads it from its own folder: `Internal storage/Android/data/org.reassembly.grid/files/` (`settings.ini`, `private/`, `Grid/`, listed below).
- **Without the data the app closes right after start** [I: `main` exits on the error; not seen on a phone]. The cause is logged to logcat (`adb logcat -s grid`), but nothing is shown on screen; an error screen is P10 work.
- **Getting the data there without adb:** Android 11+ blocks phone file managers from `Android/data`. Copying from Windows Explorer over USB (MTP) into that folder still works on many phones, but Android does not guarantee it [S] (PLATFORMS_AND_INPUT_RESEARCH.md section 3). Not tested on the owner's phones. Steps:
  1. install and start the app once, so Android creates the folder;
  2. stage the data on the PC: `powershell -NoProfile -File tools\push_android_data.ps1 -Stage C:\Temp\grid-phone`;
  3. copy the **contents** of `C:\Temp\grid-phone` into `Android/data/org.reassembly.grid/files/`.
  If Explorer does not show `Android/data`, only adb (below) or P10 can place the data.
- **The reliable path is adb** (below). **P10** (folder picker: copy from `Documents/GRID` inside the app) removes the PC from the process.
- **No controls:** there is no touch, tilt or on-screen input yet (P11). The pushed `settings.ini` turns the autopilot on, so the car drives itself. A Bluetooth keyboard may work through GameActivity, but this is untested.
- **Not measured:** frame rate, tick time, thermal behaviour and memory on a phone (P12). Audio is not paused on suspend yet, so it may keep playing in the background [I].

## Game data on the phone (development path; P10 replaces it)

The phone needs a subset of the owner's own installation (about 250 MB) plus the private CD01 table. Nothing of it goes into Git or the APK.

```
powershell -NoProfile -File tools\push_android_data.ps1
```

It runs `adb push --sync` into `/sdcard/Android/data/org.reassembly.grid/files/`:

| Path on the phone | Source |
|---|---|
| `Grid/...` | the items listed in the script, from `C:\Program Files (x86)\Steam\steamapps\common\Grid` (read only) |
| `private/body-contact-directions.bin` | CD01 from the main checkout's `local/` |
| `settings.ini` | written by the script: drive on, CD01 path, BP01/DF01 `derive` (computed in memory from the pushed `grid.exe`, P5), `[diagnostics] autopilot = on` because there is no touch or tilt input yet (P11) |

The app must be installed first: Android creates the folder for the package, and `adb shell` may write to it. Uninstalling the app deletes it.

The list was found by running the desktop release binary on a staged copy (`-Stage local/android/stage`, `--data-dir` and `--game-root` pointing there) until a 600-tick autopilot drive rendered.
