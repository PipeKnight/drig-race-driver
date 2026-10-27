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
    - `LauncherActivity` (the launcher entry): copies bundled game data on the first start (below), then starts `MainActivity`. Without bundled data (the code-only APK) it hands over to `ImportFlow`, which imports the user's own GRID files ("Importing the game data", P10).
    - `MainActivity extends GameActivity` (not exported): loads `libgrid_android.so`, keeps the screen on, hides the system bars; landscape (`sensorLandscape`). `openLogFile()` and `showFatal(String)` are called from Rust. When it finishes it ends its process, because winit and Bevy cannot start twice in one process.
    - `ErrorActivity`, in its own process `:error`, so it stays on screen when the game process exits or aborts: the message, the log file path and a Close button.
    - `BundleExtractor`: the copy logic, plain Java; `CrashLog`: log files through MediaStore.
    - P10 import: `ImportFlow` (screens, picker, worker thread), `GridImporter` (find, check, resumable copy; plain Java), `SafTree` (the picked folder through the Storage Access Framework), `NativeCheck` (JNI into `android_import.rs`).
  - `app/src/main/assets/grid-import/`: `import.txt` (the import manifest: size, SHA-256 and path of each needed file, no contents; written by `tools/make_import_manifest.ps1`) and `settings.ini` (the phone's drive profile, also used by `push_android_data.ps1`). The bundled build replaces the asset folder, so these are only in the code-only APK.
  - desktop unit tests: `app/src/test/.../BundleExtractorTest.java` and `GridImporterTest.java` (JUnit 4.13.2), run with `gradle -p android testDebugUnitTest` (set `GRID_GAME_ROOT` to include the import of the real installation);
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

Before step 1 the script adds the seed's Android artifacts to this worktree's build dir (`tools/seed_build_dir.ps1 -Android`; `-NoSeed` skips it), so Bevy for aarch64 is not compiled per worktree. Run the Android Clippy as `tools\build_android.ps1 -Clippy` (add `-Profile` for that variant), not as a bare `cargo ndk … clippy`: jni-sys's build script reruns whenever JAVA_HOME differs, and that rerun makes every Android crate built before it stale (docs/TOOLCHAIN.md "Build speed — 19 October 2026").

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

### Events in the bundled APK (F418)

`build_android.ps1 -BundleData -Event <name>` (and `push_android_data.ps1 -Event <name>`) chooses which game data the private APK carries and which event the event menu (F414, [EVENT_MENU.md](EVENT_MENU.md)) opens on:

| Event | Profile | Extra data | APK name |
|---|---|---|---|
| `donington` (default) | `android/events/donington.ini`: no starting choice (the menu's default) | — (the base list has `database/schema.bin` and `database/database.bin` since F539) | `grid-<rev>-bundled-PRIVATE-debug.apk` |
| `drift-nurburgring` | `android/events/drift-nurburgring.ini`: Drift Battle, Nürburgring sprint (`route_1`), 2 laps, BMW 320si against 7 AI BMWs from the back of the grid, as `settings/drift-battle-nurburgring.ini` | `cars/bmw`, `audio/eng_bmw.WIP`, `audio/mec_bmw.WIP`, `tracks/europe/nurburgring`, `database/dlc-database-a0.bin` and `-p0.bin` (with the base list's two files the whole `database` folder, as before F539), `gameconfig`, and the private `local/ai-driver-characteristics.tsv` (`-Characteristics PATH`; skipped if missing) | `grid-<rev>-bundled-PRIVATE-drift-nurburgring-debug.apk` |

- The profile is written as **`event.ini`** next to `settings.ini` and replaced on every install. `settings::load` reads `<data dir>/event.ini` first, then `settings.ini` (kept across installs, written by the menu, Driver Aids and Video screens), so the owner's choices win. The drift profile holds only the menu's starting choice (`[event] type = drift_battle`, Nürburgring `route_1`, BMW, 2 laps, 7 opponents), so the menu opens on the Drift Battle until the owner picks something else; a Race picked in the menu gets no drift keys. With `[front_end] event_menu = off` the app starts `settings.ini`'s own event (Donington practice) as before.
- What the menu offers with the drift bundle [V desktop, `--menu-screen` with `[game] root` on the staged folder]: `race:2x2, time_trial:2x2, touring:8x2, drift_battle:2x2, drift_gp:2x2` (Donington and the Nürburgring layouts, Lacetti and BMW). A Touring Cars race there fills its field with the bundled cars only (`field=bmw_…,lac_…`, ran 300 ticks headless). The Donington bundle has the database since F539 but no BMW, so its menu still offers Race and Time Trial at Donington only: a series is left out when one of its raceable cars is missing (Touring Cars draws BMWs into the field and failed with `cars/bmw/bmw.csv` before that rule).
- Why the Nürburgring: of the Drift Battle routes, the 8-car AI battles there scored most and spun and hit barriers least (F367: up to 17,703 points; F389: 11.1 spins and 1.6 barrier hits per lap, against 25.1 / 5.6 at Long Beach and 12.0 / 2.0 at Istanbul). Yokohama Docks (F399) was not measured in drift battles.
- [V desktop] The release binary on the staged drift folder (`--data-dir` and `--game-root` there, autopilot, 3000 ticks, 914 × 411 render) gives a log identical to the same run on the full installation apart from time stamps: `drive_drift_event discipline=drift_battle route=tracks/europe/nurburgring/route_1 cars=8`. The first try failed with "os error 3" until `database` and `gameconfig/drift.xml` were added.
- **Races on half-copied data (F451):** the owner's logs of 19.10 02:19–02:23 (`local/android/phone-logs-20261019-0520/`) show menus offering only Donington and the Lacetti (`race:1x1`) at 02:19–02:20 and the full drift set (`race:2x2 … drift_battle:2x2`) at 02:22, and two Donington races failing with `No such file or directory (os error 2)` between body events and deformation, while the same race ran at 02:20:45: game processes ran while the bundled data was still being copied [I from the logs]. Two ways in: Android can recreate `MainActivity` straight from Recents or the task after an update, skipping the copy screen; and each event-menu relaunch is a new process whose `LauncherActivity` could start a second copy, which replaces the same files under a running race. Fixes: `MainActivity.dataReady()`, called by the native code first, opens the copy screen instead of the game unless the version marker matches; `BundleExtractor.extract` holds a file lock (`.bundle-lock`) across processes and skips a copy another process just finished. The prop and entity readers now name the missing file. **F539: the real cause of these failures** was most likely not the copy but the Donington data subset itself: it had no `database/`, and `Route::additional_ground_clearance` (read by `prepare_body_geometry`, between `drive_body_events` and `drive_deformation`) opens `database/schema.bin` and `database/database.bin`. Every race on the Donington bundle or the code-only import failed there with a bare `os error 2`; the race that ran at 02:20:45 was on the drift data, which has `database`. The owner's profile APK (a11cddf7de27, Donington bundle) failed the same way three times on 21.10. The base list now has those two files and `grid_tools::read_database` names a missing one.
- **Menu start on the bundle's event (F451):** `settings.ini`'s saved `[event]` (the last Continue) overrode the drift bundle's `event.ini`, so after an earlier APK the menu opened on the Donington Race. The menu now starts on `event.ini`'s `[event]` when that file is newer than `event-menu.ini` (the last menu profile), that is once after each install; after a Continue the owner's choice is remembered again (unit test `a_newer_bundled_event_ini_sets_the_starting_choice`).
- **Black screen after Continue (F445):** the race's set-up before its window draws took minutes. Building the Nürburgring's 961 breakable props (11,962 pieces) asked `scene::read_renderable_draws` once per piece, and each call scanned the whole object library (262 s on the desktop i7, in a run with other builds; far longer on the phone, so it looked like a hang). `scene::Renderables` builds the tables once; the set-up of the 8-car drift battle now takes about 25 s on the desktop [V: timestamped run on the bundle's data, same `dynamic_props breakables=961 drawn_pieces=11962` line; asset test `renderables_match_one_scan_per_id_on_the_nurburgring_objects` compares every node id with the old scan]. The screen is still black during set-up (no loading screen); on the phone expect up to about a minute [I].
- Phone layout [V desktop render, 914 × 411]: the Drift Battle score panel (`Total … ×multiplier`) sits at the top centre over the II and CENTRE zones (the zones are drawn under the HUD; both still respond to a tap there). The FPS counter moved to the right edge under the race position so it no longer covers the score.
- Drifting by touch: hold GAS with the right thumb, steer by tilting, and pull the car sideways with a tap or hold of HAND (left thumb, next to BRAKE); BRAKE shifts weight forward before a corner. HAND is the original Hand Brake action (record handbrake > 0.5), so it acts exactly like Space or the pad's B. The manual gearbox's GEAR ± zones appear next to GAS if Driver Aids sets it.

### Bundle file list (F608)

A bundled APK carries every GRID file its event reads; nobody copies the game onto the phone. A friend's Mali phone (F603 log) showed that the bundles had always lacked `audio/car_genmec.WIP`, `car_collisions.WIP`, `audiohud.WIP`, `GroupModeVolumes.bin` and `ImpactVarieties.bin` (mechanical, collision, HUD, mix and tyre audio), plus the speech and force-feedback files. The owner's own phones had shown the same warnings since at least 22.10.

- **`android/events/<event>.files`** (committed, names only) lists every file a desktop run of the event read, then every speech line the ported speech events can play. `tools/push_android_data.ps1` bundles the hand list above, plus the Donington list for every bundle (its menu offers the Donington races), plus the event's own list. With `-Stage` it fails when a listed file is not in the stage, so `build_android.ps1 -BundleData` and `make_import_manifest.ps1` stop there.
- **`tools/trace_bundle_files.ps1 -Event <event>`** writes a list. It runs the event as the phone does: the bundle's `settings.ini` and `event.ini`, the menu's Continue (`GRID_MENU_AUTO`), autopilot, a 3000-tick race, muted, in a window. The game root is a scratch robocopy mirror of the installation (about 10 GiB, under the ignored `local/android/trace/`, deleted unless `-KeepMirror`) whose last-access times are first set to 2000-01-01. NTFS on this PC updates a last-access time when a file is read and the stored time is over an hour old (`fsutil behavior query disablelastaccess` = 2, enabled), not on a metadata query, so the files whose time moved were read. Procmon needs elevation, which the sessions do not have. Speech streams one `.raw` per spoken line, so `grid-app --speech-files` (all long versions and variations of the `speech::SLICE` events' messages that exist, 643 files, 110.6 MB) is appended.
- **`-Stage DIR`** checks a staged bundle the same way: every file the run read must be in the lists, and the log must have no file errors (exit code 1 otherwise).
- **Regenerate** the lists after a change that reads new game files (a new reader, a new audio bank), then rebuild the import manifest. A file read only in situations the scripted run does not reach (a pause-menu action, a crash type it never has) can still be missed. Such files go into the hand list.

### Profile APK (F539)

`tools/release_playable.ps1` builds the per-system profile APK ([PERFORMANCE.md](PERFORMANCE.md) "Per-system profile") with `-ProfileEvent drift-nurburgring` by default, that is `build_android.ps1 -BundleData -Profile -Event drift-nurburgring`, renamed to `grid-<rev>-bundled-PRIVATE-profile-debug.apk`. Reasons: the owner's main phone test is the drift bundle and the frame-rate collapse to profile (broken tyre stacks and armco in the Drift Battle) happens there; and the bundle version depends only on the staged data, not on the `profile` feature, so the profile APK and the drift APK of one revision have the same bundle version and switching between them copies nothing (about 520 MB otherwise). Releases before F539 used the Donington bundle, whose races could not start (above). `-ProfileEvent donington` still builds that variant.

### First start: copying the data

`LauncherActivity` reads `assets/grid-data/bundle.txt`. If `Android/data/org.reassembly.grid/files/.bundle-version` holds the same version, it starts the game at once. Otherwise it shows "First start: copying the game data" with a progress bar (MB done of total), copies every file to `files/` (each through `name.part`, renamed when its size matches), then writes the marker and starts the game. The marker is deleted before copying, so an interrupted copy (app killed, storage full) starts again on the next launch. Files of an older bundle that the new one no longer lists are not deleted. A failed copy shows the error on screen and writes `Download/GRID-Reassembly/extract-<time>.txt`.

All loaders still read from `files/` (the same layout as the adb push), so the bundled and adb paths can be mixed. Uninstalling the app deletes `files/`; reinstalling the bundled APK copies again.

Unit tests (desktop JVM, `gradle -p android testDebugUnitTest`, 7 tests [V]): first copy and marker; same version skipped; new version replaces game files but keeps an edited `settings.ini`; a deleted `settings.ini` is written again; an interrupted copy leaves no marker; a size mismatch fails without replacing the old file; `bundle.txt` paths that would leave the data folder and malformed lines are rejected.

## Errors and the log file

- **Log file:** every start creates `Download/GRID-Reassembly/log-<yyyyMMdd-HHmmss>.txt` in the phone's shared Downloads folder (Files / File Manager → Downloads → GRID-Reassembly). It is written through MediaStore, which needs no permission on Android 10+. It gets everything the game prints (stdout and stderr), errors marked `E`, each line with seconds since start. The ten newest files are kept; older ones are deleted. Logcat (`adb logcat -s grid`) still gets the same lines.
- **Empty logs (F445):** the P12 profiling change (20f4c5d) buffered the file (64 KiB, flushed at most every 0.5 s from the calling thread). From then on the owner's phone listed every log as 0 B (nine files on 19.10, both the drift and the profile APK). The exact mechanism is not proven without the phone [I]; the file is now written by a writer thread, one write() per line as soon as it arrives (as before P12, but off the game threads), so a hard kill keeps every line already handed over. A failed write is reported to logcat (`log file: write failed: …`).
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
- **Gamepads (P11b):** code since F404, see "Gamepads" below; not yet tried on a phone. A Bluetooth keyboard may work through winit, but this is untested.
- **Data without the bundled APK:** since F405 the code-only APK imports the user's own GRID folder ("Importing the game data" below); adb still works. Not yet tried on a phone.
- **Not measured yet:** frame rate (the counter below now shows it), thermal behaviour and memory on a phone (P12). Audio is not paused on suspend yet, so it may keep playing in the background [I].

## Frame rate

**Counter.** `[osd] fps` (default `on` on Android, `off` on the desktop, where F9 toggles it) shows `NN FPS  M ms  max W` at the right edge under the race position (since F418; it was at the top centre, where the Drift Battle score panel now sits): frames per second, the mean frame time and the worst frame of the last second. While it is on, the app logs once per second (logcat and `Download/GRID-Reassembly/log-*.txt`):

```
frame_stats fps=59.1 avg_ms=16.92 max_ms=18.49 frames=60 sim_ticks_per_s=63.1
```

`sim_ticks_per_s` is the simulation's successful fixed ticks per second (62.5 at full speed; lower means the simulation falls behind real time or is paused). The first line after start-up, `frame_setup`, records the requested present mode, frame latency, the window's physical and logical size, its scale factor and the refresh rate winit reports. The counter reads only real time and the tick counter, so simulation results are unchanged (solo identity, F374).

**Texture path.** At start-up `texture_upload.rs` logs `texture upload: gpu_bc=<bool> gpu_etc2=<bool> path=<...>` (and `texture cache: <dir>` for the ETC2 cache), which reaches the Download log file like every stdout line: BC native when the device has `TEXTURE_COMPRESSION_BC`, else the ETC2 cache, else RGBA8 CPU decode (P8). The Adreno 740 is expected to report BC through Vulkan [I: not seen in a phone log yet]; the line in the owner's next log settles it.

**GPUs without BC (F603).** A friend's Galaxy Note20 (Mali-G77, Exynos 990, Android 13; log `local/android/f-nobc/` of the android-profile-apk worktree) chose `path=etc2-cache (no BC support)` correctly, but the renderer was lost 2 s after the window opened: 48 `Bc3RgbaUnormSrgb` textures were created on a device without `TEXTURE_COMPRESSION_BC`, every `OsdMaterial` bind group failed, then `DeviceLost`. The device, and so the path, exists only once the app runs (`configure` is a PreStartup system), but `osd_scene::install` builds the HUD's images while the app is assembled, so they kept the BC default. Now `convert_existing` (PreStartup, after `configure`) converts every BC image already made, and `convert_added` (Last, before extraction) every BC image added later by any loader; the log line `texture upload: late_converted=N failed=F when=built|added` and `late_converted` in the summary count them. The owner's Adreno 740 has BC, so it never hit this.

Desktop check without a phone: `[diagnostics] texture_fallback = etc2` or `rgba8` now also leaves BC off the device (`texture_upload::render_creation`), so a desktop GPU fails exactly like the Mali when a BC texture slips through. Before the fix that run ended with the same 48 errors and `renderer exited with an error`.

**What limits the frame rate in the current build** [V code, Bevy 0.19.1 / wgpu defaults; nothing measured on the phone yet]:

- **Present mode:** the window keeps Bevy 0.19's default `PresentMode::Fifo` (the enum's `#[default]`; a desktop run logs `frame_setup present_mode=Fifo`). Strict vsync: at most one frame per display refresh, and a frame that misses a refresh waits for the next. (F374 said AutoVsync; corrected in F375.)
- **Frame cap:** none in the app. `WinitSettings::game()`: continuous updates while focused; `desired_maximum_frame_latency` is Bevy's default (none set, wgpu's 2).
- **Display refresh:** the app requests none. No `ANativeWindow_setFrameRate`/`Surface.setFrameRate`, no `preferredDisplayModeId` or `preferredRefreshRate` in the manifest or activities; only `appCategory="game"`. So Android and MIUI/HyperOS choose the rate, and a phone's adaptive refresh often holds unknown apps at 60 Hz [I]. 120 Hz is therefore not available to the app, and a frame that misses a 16.7 ms vsync waits for the next one, so 60 → 30 FPS steps are expected with Fifo [I].
- **Render resolution:** since F375 capped by `[video] render_height` (below), default 720 rows on Android since F549 (mobile preset, [PERFORMANCE.md](PERFORMANCE.md) "Mobile preset"; 1080 from F375). Before that, the full native surface: the requested `--resolution` (1280 × 800 default) is ignored on Android. The Xiaomi 13 Ultra's panel is 3200 × 1440 (WQHD+), 4.6 Mpixel, unless the phone's display resolution setting is FHD+ (2400 × 1080) [S]; `frame_setup` and `frame_stats` tell which.
- **Graphics preset:** `baseline`, the desktop default (log `graphics preset=Baseline shadow_size=2048 anisotropy=1 sky_ibl=false ssao=false taa=false tonemapping=Bevy-default`): none of the enhanced preset's HDR, bloom, SSAO, SMAA/TAA, atmosphere or 4096 shadow map. But the camera keeps Bevy's default **4× MSAA** (`Msaa::Sample4` is a required camera component; only the enhanced preset sets `Msaa::Off`), at the full native resolution.
- Tuning any of these is P12, after the phone numbers.

**First phone measurement** [V, owner's Xiaomi 13 Ultra, APK 7b3122337846 (F374, before the render cap), log `log-20261018-030256.txt`, kept privately]: 112 `frame_stats` lines over a 104 s solo lap attempt at Donington.
- While driving (106 s): mean 35.7 FPS; per-second FPS p10 26.6, median 32.6, p90 48.4; worst single frame 72 ms after start-up (744 ms in the loading second). Long stretches sit at 28 FPS with 35.6 ms frames (max 36.8 ms), i.e. not on a 60 Hz vsync multiple (33.3 or 50 ms): the GPU or CPU frame takes about 35 ms [I].
- `sim_ticks_per_s` 61.7–63.0 throughout: the simulation keeps real time, so the low rate is rendering, not physics.
- Renderer: Adreno (TM) 740, Vulkan, Qualcomm driver 06/04/23; `texture upload: gpu_bc=true gpu_etc2=true path=bc` (BC native, no CPU decode or ETC2 cache). `frame_setup` printed the requested 1280 × 800 before Android had sized the window, so the real surface size is still unknown; since F376 the line repeats when the size changes, and the render cap's `render_scale` line logs it too. `monitor_hz=[unknown]`: winit reports no refresh rate on Android.
- Also in the log: `Tyre audio unavailable ... No such file or directory`: `tyre_audio` also reads `audio/audioVarieties.bin`, `CollisionInfos.bin` and `audioMaterials.bin` (124 KB together), which the data subset lacked; added to `push_android_data.ps1` in F376 (the bundle version changes, so the phone copies once more). The gilrs "does not support current platform" error is expected.

**Window size on Android (F378).** Bevy's `Window` starts with the requested 1280 × 800 at scale factor 1, and winit on Android ignores the request; the first phone log printed `frame_setup physical=1280x800 logical=1280x800 scale=1`. Whether the window stayed there is not known (that build logged the size only once, before Android sized it), but nothing in our code made it follow the screen: a stale size would give a 16:10 camera aspect on a ~20:9 panel and a swapchain below the panel's resolution. `android_display.rs` now sets the window every frame it differs to the native surface (`ANativeWindow` width × height) and the density scale (`AConfiguration` dpi / 160), logging `android_surface native=WxH density_scale=S window_before=WxH scale_before=S` when it changes anything. `render_height` then caps the 3D scene from that size.

**Display rate options (F378, all default off / unchanged):**
- `[video] refresh_hz = system | 30..240`: sends `ANativeWindow_setFrameRate(hz, DEFAULT)` (API 30) to the surface; a hint the system may ignore. Logged as `android_frame_rate requested_hz=... result=... display_hz=...`. `frame_setup` on Android now includes `display_hz` from `Activity.getDisplay().getRefreshRate()` (JNI), since winit's `monitor_hz` is always unknown there.
- `[video] present_mode = fifo | auto_vsync | auto_no_vsync`: `fifo` is the current default (strict vsync). `auto_vsync` asks for FifoRelaxed (a late frame is shown at once, with tearing, instead of waiting a whole refresh), `auto_no_vsync` for Immediate → Mailbox → Fifo. Only modes with a fallback chain are offered, so an unsupported mode cannot fail at start.
- `[video] frame_latency = 1..3`: frames in flight (wgpu default 2); 1 lowers input lag, 3 can smooth drops.
- Reading of the phone log: the driving stretches at 28 FPS have 35.6 ms frames with a 36.8 ms maximum. Under strict Fifo at 60 Hz, frame intervals could only be 33.3 or 50 ms, so either the panel runs at a higher or variable rate (the 13 Ultra has an LTPO 1–120 Hz panel, from memory of its specification [I]) or the measured CPU loop time is not the present interval [I]. The pre-race stretch at 32.4 ms / max 33.4 looks like 30 FPS on a 60 Hz vsync. The next log, with `display_hz` and `android_surface`, should settle it before changing any default.

**Video screen (F390, AUTHORED).** The settings below can be changed in the app: before the race, **Race Options → Video** (Up/Down to choose, Left/Right or the touch `<` `>` buttons to change, BACK to leave; leaving after a change saves to `settings.ini`, keeping the other keys and comments). Rows and when they take effect:

| Row | Values | Takes effect |
|---|---|---|
| FPS counter | On / Off (`[osd] fps`) | at once |
| Render height | Auto / 720 / 900 / 1080 / Native (`[video] render_height`; Auto is dynamic 480-720 rows and shows the current rows, e.g. `Auto (624)`, F559; another value from the file shows as is and goes to Auto on the next change) | at once |
| Present mode | Fifo (vsync) / Auto vsync / Auto no vsync (`[video] present_mode`) | at once (Bevy reconfigures the swapchain) |
| Refresh rate | System / 60 / 90 / 120 Hz (`[video] refresh_hz`) | at once on Android (`ANativeWindow_setFrameRate`; System sends 0 = no preference); no effect on the desktop |
| Frame latency | Default (2) / 1 / 2 / 3 (`[video] frame_latency`) | next start (wgpu reads it only when the swapchain is created) |

Each row's help text says the same in plain words. A change is logged as `drive_video version=1 event=apply video=...`, the save as `drive_front_end ... event=save_video`.

**Render-height cap** (`[video] render_height = native | 240..4320`, F375). The 3D view renders into an image of at most that many rows with the window's aspect ratio, and a second camera stretches it over the window (bilinear). Android default 720 since F549 (mobile preset; 1080 before, on a WQHD+ 3200 × 1440 screen: 2400 × 1080, 56 % of the pixels); desktop default `native`. A cap at or above the window height renders straight to the window, so nothing is ever upscaled beyond native. Since F556 the HUD, menus, touch zones, FPS counter and OSD are drawn by the window camera at the screen's full resolution: UI roots aimed at the 3D camera are moved to it while the scaled image is shown (`render_scale::ui_on_window`, logged as `render_scale ui_roots`), and the OSD image follows the window size. Before F556 they were drawn into the scaled image. `render_height = auto` (the Android default since F559) is dynamic: between 480 and 720 rows, lowered by 48 rows at most every 0.5 s while frames run later than 18 ms and the simulation is not the reason, raised by 24 rows after 3 s on time, and never back to a height that ran late in the last 15 s; since F571 each step down must speed the frame up by at least half of what the pixel count alone would give one second later, or the height returns to 720 and holds for 60 s (`reason=ineffective`); each change logs a `render_dynamic` line, and `frame_stats` shows the current `render=`. A row count is a fixed height; `native` turns the scaling off; `GRID_DYNAMIC_RES=0` keeps Auto at 720. Heights saved before F559 read as Auto once (F556 used a saved row count as the ceiling, and its CPU check most likely blocked every drop on the phone). [PERFORMANCE.md](PERFORMANCE.md) "Dynamic render height (F556, F559)". The FPS counter's second line shows `render of screen` (e.g. `2400x1080 of 3200x1440`), and `frame_stats` ends with `render=WxH screen=WxH`; a `render_scale` line logs each change. Touch input uses window coordinates and is unaffected. Checked on the desktop [V]: a 1600 × 900 window with `render_height = 400` rendered 711 × 400 and showed it full-window, HUD and touch zones in place, `frame_stats ... render=711x400 screen=1600x900`.

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

## Gamepads (P11b, F404)

Bluetooth and USB pads (Xbox, DualSense, other HID pads Android classifies as gamepads) drive through the same original XInput pad preset as on the desktop (`ActionMap/actionMapPreset4.xml`, input.rs): steering on the left stick, triggers for throttle and brake, the preset's buttons for gears, view, pause and the menus. Pause, Race Options (with Video, Driver Aids and Performance test), the post-race screens and replays all read the same Menu/Replay actions, so they work with the pad without screen-specific code.

**Why a bridge.** Bevy's gamepads come from gilrs, and gilrs-core 0.6.8 has no Android backend: the log line `bevy_gilrs: Failed to start Gilrs. Gilrs does not support current platform` is expected and harmless. winit 0.30.13 drains GameActivity's input itself (`platform_impl/android/mod.rs`): it turns a joystick's ACTION_MOVE into a fake `Touch` at the stick's AXIS_X/Y (near the top-left corner), drops every other axis, and maps pad buttons to unidentified keys [V code].

**Path.**
1. `PadBridge.java` (owned by `MainActivity`) overrides `dispatchKeyEvent` and `dispatchGenericMotionEvent`, which run before GameActivity's own handlers. Events from a device whose sources include `SOURCE_GAMEPAD` or `SOURCE_JOYSTICK` (and that is not virtual) are taken and never reach GameActivity, winit or Android's synthetic DPAD/BACK fallbacks. So a pad's B or Back button does not close the game.
2. Connections come from `InputManager.InputDeviceListener` and the device list at `onStart`, for devices that also have an A button or an X/Y stick (AUTHORED guard, so a built-in device that only lists a gamepad source cannot hide the touch zones); any other pad device is connected on its first pad event; `onStop` releases every pad (no input arrives in the background), `onStart` announces them again.
3. Axes sent per sample: left stick AXIS_X/Y; right stick AXIS_Z/RZ; triggers max(LTRIGGER, BRAKE) and max(RTRIGGER, GAS); hat X/Y. A pad with RX/RY but no trigger axes (no Android key layout file, Linux XInput layout) gets right stick RX/RY and triggers Z/RZ mapped from -1..1 to 0..1 (AUTHORED heuristic).
4. `android_pad.rs` (JNI natives `Java_org_reassembly_grid_PadBridge_*`) queues the events; a `PreUpdate` system before Bevy's `InputSystems` turns them into `GamepadConnectionEvent` and raw button/axis events, like bevy_gilrs. Keys: A/B/X/Y → South/East/West/North, L1/R1 → LeftTrigger/RightTrigger, L2/R2 → LeftTrigger2/RightTrigger2 (OR'ed with the analogue triggers), THUMBL/R, START, SELECT and a pad's BACK → Select, MODE, DPAD keys (OR'ed with the hat). Stick Y is negated to Bevy's up-positive convention; input.rs flips it back for the preset as on the desktop.
5. Entities get `input::ClassifiedPad`: with `device=auto` Android's own classification replaces the desktop name test ("8BitDo Pro 2" would fail "pad"/"controller"), keeping only the wheel guard (wheel makers' vendor ids, "wheel"/"racing"/"pedal"/... in the name).

**Pad takes over from touch.** With `[input] device = auto` (the default) and the phone controls on, a connected accepted pad hides every touch zone and the tilt bar, ignores touches, and silences tilt (`Mobile::tilt_raw` is `None`). On disconnect they come back; fingers down at either change are ignored until lifted. `device = pad` or `keyboard` keeps the zones (no takeover). Log lines: `drive_input_pad android_device=ID entity=... name=... vendor=... product=...` on connect, `drive_input_pad connected=... preset4=used|ignored(...)` from input.rs, `drive_input_pad takeover=on|off`, `drive_input_pad android_device=ID disconnected`.

**Rumble:** the original's XInput motor levels go to the pad's vibrators, or to the phone's vibrator when no pad is connected ([FORCE_FEEDBACK.md](FORCE_FEEDBACK.md) §7.2; `[input] force_feedback`, `feedback_effects`). **Not covered:** pad-specific glyphs in help texts, more than one player, the DualSense touchpad and motion sensors (separate input devices, not pads). Desktop gamepads are unchanged (gilrs; `ClassifiedPad` is never set there).

**Checks [V]:** mapping unit tests (`android_pad::tests`, desktop), wheel guard test, tilt silenced while a pad holds; `cargo ndk … clippy --release --lib -p grid-app -- -D warnings` clean; Java compiles in the APK build. [N] Not tried with a real pad on a phone.

## Event menu (F414)

The phone starts in the event menu (Race Day, [EVENT_MENU.md](EVENT_MENU.md)); its Continue and a race's Back to menu restart the game process through `RelaunchActivity` (own process `:relaunch`), since winit and Bevy cannot start twice in one process. With the bundled or imported data subset only Race and Time Trial at Donington with the Lacetti are offered (no other track or car in the subset; a series needs all its raceable cars, F539). `[front_end] event_menu = off` in `settings.ini` restores the old direct start. Touch uses the P11 menu zones; a pad navigates as in the pause menu. Not tried on a phone yet.

## Importing the game data (P10, F405)

The code-only APK (`tools\build_android.ps1` without `-BundleData`) carries no game data. On a start without data it explains what is needed and lets the user pick their own copy of the GRID folder. This is the path for anyone who owns the game: no PC tools, nothing proprietary in the APK.

**What the user does.** Copy the GRID folder of the installation (Steam: `steamapps\common\Grid`) to the phone, for example over USB into `Documents/GRID`, start the app, tap **Choose GRID folder** and pick that folder (the one with `grid.exe`, or the folder above it). The whole folder (several GB) works; only the 394 needed files (248 MB) are read. Afterwards the copy in Documents can be deleted. Android's picker cannot pick the storage root, `Download` itself or `Android/data` (Android 11+), so a sub-folder such as `Documents/GRID` is needed.

**Flow** (`ImportFlow.java`, worker thread; the screens are plain Android views):
1. **Start screen** with the steps, the needed and the free space, and **Choose GRID folder** / **Close**. The system picker (`ACTION_OPEN_DOCUMENT_TREE`, starting in Documents) returns a tree URI; the app takes a persistable read permission and remembers it, so an interrupted import can continue after a restart.
2. **Find:** the picked folder itself when it holds `grid.exe`, else a folder directly inside it (a folder named Grid first). Names are matched ignoring case (a copy from Windows may differ in case); the copies get the manifest's spelling, which the game expects on Android's case-sensitive storage.
3. **Check** against `import.txt` (committed: 394 lines of size, SHA-256 and path, version c32bec7e…; the same subset `push_android_data.ps1` stages and the bundled APK carries): missing files and files of another size are listed on screen (up to 12, then "… and N more"), with **Choose another folder**. Then free space: the bytes still to copy + 64 MB.
4. **Copy** with a progress bar (MB done of total, current file) into `files/Grid/`, each file through `name.part`, hashed on the way. A file whose SHA-256 differs from the manifest is not copied; at the end they are listed with **Choose another folder** and **Use these files anyway** (copies them unverified; the game may then fail to start).
5. **Resume:** each verified file is appended to `files/.import-progress` (`sha256<TAB>path`). A later run skips a file that is in the journal with the manifest's hash and has the right size. After a kill or a crash the next start shows **The last import did not finish** (copied X of Y MB) with **Continue** (same folder, while the permission is held) and **Choose GRID folder**.
6. **Native check:** `NativeCheck.deriveTables` loads `libgrid_android.so` and derives CD01, BP01 and DF01 from the imported `grid.exe` exactly as the game does at start (`android_import.rs`, `grid_tools::private_parameters`), logging `import_derive kind=… sha256=…`. A failure is shown on screen.
7. **Finish:** writes `settings.ini` when missing (the template from the assets; an edited file stays) and the marker `files/.import-version` (the manifest version), then starts the game. Later starts with that marker start the game at once.

**Texture cache:** not run on the import screen. Which path applies (BC native, ETC2 cache or CPU decode, P8) depends on the GPU, which only the game's renderer knows; on the owner's Adreno 740 it is BC native and nothing is encoded. On an ETC2-only phone the game builds the cache on its first start, as before ("import on first launch", P8).

**Other data paths stay:** the bundled PRIVATE APK has no `grid-import` assets and runs its first-start copy unchanged (the push script now reads the same `settings.ini` template; the staged bytes are identical, sha256 888bfe9e…, so the bundle version is unchanged). Data placed by adb, or left by an earlier bundled APK, starts the game at once (a `Grid/grid.exe` without an interrupted import).

**Controls on the import screens:** touch; a pad moves the focus with the D-pad or the stick (Android's own focus navigation), **A** presses the focused button (also handled explicitly, since not every pad maps it to DPAD_CENTER) and **B** or Back goes back (ignored while copying). Tilt has nothing to do there. The system folder picker is Android's own screen: touch works; pad support there is the system's.

**Errors:** on screen, and in `Download/GRID-Reassembly/import-<time>.txt`; logcat tag `grid`, lines `import: …`.

**Updating the manifest:** after changing the file list in `push_android_data.ps1`, run `powershell -NoProfile -File tools\make_import_manifest.ps1` (stages into the ignored `local/android/import-stage`, hashes, deletes the stage) and commit `import.txt`. A new manifest version makes phones import again; only changed or new files are copied, since the journal keeps the hashes.

**Checks [V]:** `GridImporterTest` (desktop JVM): manifest parsing rejects bad lines and paths; the GRID folder is found directly or inside the picked folder, any case; a copy keeps the manifest spelling and finishes with the marker and `settings.ini`; missing and wrong-size files are reported; an interrupted import resumes without copying finished files and copies a deleted one again; other contents are reported and copied only when accepted; an edited `settings.ini` stays; the committed manifest parses; with `GRID_GAME_ROOT` the owner's installation imports with every hash equal (about 3 s on the desktop). `cargo ndk … clippy -D warnings` clean. [N] Not tried on a phone: the picker, SAF read speed, the screens and pad focus.

## Game data on the phone (development path)

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
