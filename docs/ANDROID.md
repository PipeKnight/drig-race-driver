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
- **Not measured yet:** frame rate (the counter below now shows it), thermal behaviour and memory on a phone (P12). Audio is not paused on suspend yet, so it may keep playing in the background [I].

## Frame rate

**Counter.** `[osd] fps` (default `on` on Android, `off` on the desktop, where F9 toggles it) shows `NN FPS  M ms  max W` at the top edge, between the II button and the race position panel: frames per second, the mean frame time and the worst frame of the last second. While it is on, the app logs once per second (logcat and `Download/GRID-Reassembly/log-*.txt`):

```
frame_stats fps=59.1 avg_ms=16.92 max_ms=18.49 frames=60 sim_ticks_per_s=63.1
```

`sim_ticks_per_s` is the simulation's successful fixed ticks per second (62.5 at full speed; lower means the simulation falls behind real time or is paused). The first line after start-up, `frame_setup`, records the requested present mode, frame latency, the window's physical and logical size, its scale factor and the refresh rate winit reports. The counter reads only real time and the tick counter, so simulation results are unchanged (solo identity, F374).

**Texture path.** At start-up `texture_upload.rs` logs `texture upload: gpu_bc=<bool> gpu_etc2=<bool> path=<...>` (and `texture cache: <dir>` for the ETC2 cache), which reaches the Download log file like every stdout line: BC native when the device has `TEXTURE_COMPRESSION_BC`, else the ETC2 cache, else RGBA8 CPU decode (P8). The Adreno 740 is expected to report BC through Vulkan [I: not seen in a phone log yet]; the line in the owner's next log settles it.

**What limits the frame rate in the current build** [V code, Bevy 0.19.1 / wgpu defaults; nothing measured on the phone yet]:

- **Present mode:** the window keeps Bevy 0.19's default `PresentMode::Fifo` (the enum's `#[default]`; a desktop run logs `frame_setup present_mode=Fifo`). Strict vsync: at most one frame per display refresh, and a frame that misses a refresh waits for the next. (F374 said AutoVsync; corrected in F375.)
- **Frame cap:** none in the app. `WinitSettings::game()`: continuous updates while focused; `desired_maximum_frame_latency` is Bevy's default (none set, wgpu's 2).
- **Display refresh:** the app requests none. No `ANativeWindow_setFrameRate`/`Surface.setFrameRate`, no `preferredDisplayModeId` or `preferredRefreshRate` in the manifest or activities; only `appCategory="game"`. So Android and MIUI/HyperOS choose the rate, and a phone's adaptive refresh often holds unknown apps at 60 Hz [I]. 120 Hz is therefore not available to the app, and a frame that misses a 16.7 ms vsync waits for the next one, so 60 → 30 FPS steps are expected with Fifo [I].
- **Render resolution:** since F375 capped by `[video] render_height` (below), default 1080 rows on Android. Before that, the full native surface: the requested `--resolution` (1280 × 800 default) is ignored on Android. The Xiaomi 13 Ultra's panel is 3200 × 1440 (WQHD+), 4.6 Mpixel, unless the phone's display resolution setting is FHD+ (2400 × 1080) [S]; `frame_setup` and `frame_stats` tell which.
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

**Video screen (F383, AUTHORED).** The settings below can be changed in the app: before the race, **Race Options → Video** (Up/Down to choose, Left/Right or the touch `<` `>` buttons to change, BACK to leave; leaving after a change saves to `settings.ini`, keeping the other keys and comments). Rows and when they take effect:

| Row | Values | Takes effect |
|---|---|---|
| FPS counter | On / Off (`[osd] fps`) | at once |
| Render height | 720 / 900 / 1080 / Native (`[video] render_height`; another value from the file shows as is and goes to 720 on the next change) | at once |
| Present mode | Fifo (vsync) / Auto vsync / Auto no vsync (`[video] present_mode`) | at once (Bevy reconfigures the swapchain) |
| Refresh rate | System / 60 / 90 / 120 Hz (`[video] refresh_hz`) | at once on Android (`ANativeWindow_setFrameRate`; System sends 0 = no preference); no effect on the desktop |
| Frame latency | Default (2) / 1 / 2 / 3 (`[video] frame_latency`) | next start (wgpu reads it only when the swapchain is created) |

Each row's help text says the same in plain words. A change is logged as `drive_video version=1 event=apply video=...`, the save as `drive_front_end ... event=save_video`.

**Render-height cap** (`[video] render_height = native | 240..4320`, F375). The 3D view renders into an image of at most that many rows with the window's aspect ratio, and a second camera stretches it over the window (bilinear). Android default 1080 (on a WQHD+ 3200 × 1440 screen: 2400 × 1080, 56 % of the pixels); desktop default `native`. A cap at or above the window height renders straight to the window, so nothing is ever upscaled beyond native. **The HUD, pause menu, touch zones and FPS counter are scaled with the scene**: they are UI on the 3D camera and are drawn into the same image; keeping them native would mean retargeting every HUD module to the window camera, which was not a small change. Their size on screen is unchanged (the UI scale follows the image), only slightly softer. The FPS counter's second line shows `render of screen` (e.g. `2400x1080 of 3200x1440`), and `frame_stats` ends with `render=WxH screen=WxH`; a `render_scale` line logs each change. Touch input uses window coordinates and is unaffected. Checked on the desktop [V]: a 1600 × 900 window with `render_height = 400` rendered 711 × 400 and showed it full-window, HUD and touch zones in place, `frame_stats ... render=711x400 screen=1600x900`.

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
