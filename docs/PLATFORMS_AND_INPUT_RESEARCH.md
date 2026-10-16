# Platforms and input: research and plan

16 October 2026, Claude Code. The owner asked two things: how hard it is to build the port for other platforms (Android first), and how to add more control methods (gamepad, steering wheel with force feedback, phone gyroscope). The fun target is an Android build that steers by tilting the phone, with on-screen throttle and brake.

This document is research and a plan. The only code change is the proof of concept in section 1.2, which removes the hard ARM blocker in the physics and leaves the x86 build unchanged.

- **Revision studied:** `1c32c09` (main).
- **Method:**
  - Static reading of the workspace.
  - `cargo check`/`clippy` for `aarch64-linux-android`.
  - An exhaustive hardware measurement of RCPPS/RSQRTPS on the owner's CPU, with regression runs of the race tool.
  - Static reverse engineering of the original input and force-feedback code. Ghidra 12.1.4 headless `-readOnly -noanalysis` ran on a private project copy, with Capstone 5.0.6 on the hash-pinned `local/compatibility/grid-unpacked.exe` (SHA-256 `aac46a64…4514`) and the shipped `ActionMap/` and `forcefeedback/` XML. No live process was involved, and no input was sent to the original game.
  - Web research on official sources; the links are in section 9.
- **Evidence:** private, in the main checkout's `local/platforms/` and `local/platforms-input/`:
  - check logs;
  - the SSE probe sources and run outputs in `sse-approx/`;
  - disassembly, decompilation and the input report `ORIGINAL_INPUT_DEVICES.md`.
- **Tags:**
  - **[V]** verified here (run, test, or a cited official source);
  - **[D]** read directly from decompiled code or shipped data;
  - **[S]** secondary source;
  - **[I]** inference;
  - **ORIGINAL** behaviour taken from GRID;
  - **AUTHORED** a port choice with no original counterpart.

## 0. Short answer

- **Android is feasible, but needs about 8–11 focused sessions before a playable tilt-steering build.** It also needs a test phone. The blockers, in order:
  1. The physics used x86-only approximate SSE instructions. **Solved in this session** (1.2): the reference arithmetic now runs on ARM with bit-identical results.
  2. The renderer requires BC (DXT) texture compression. About 63 % of Android Vulkan devices lack it, including all Mali and Xclipse GPUs. A CPU decode or transcode path is needed (1.5).
  3. The app is configured through command-line flags, environment variables, `.cmd` launchers and a hard-coded Windows install path. Android has none of these (1.4).
  4. The game data must come from the user's own purchased copy. The design is in section 3. One private parameter table (CD01) currently needs a Steamless-unpacked executable, which an ordinary user does not have (1.4).
  5. There is no Android project yet: NDK, Gradle/GameActivity wrapper, landscape lock, sensor and touch input.
- **Linux and Steam Deck are the cheapest new platforms**, about 1–2 sessions each. **macOS** is moderate. **Consoles** are not realistic: Bevy has no public console support and the SDKs are under NDA.
- **Input:** today the port has only digital keyboard input. Bevy already compiles in gilrs (gamepads). Wheels and force feedback need SDL3 or DirectInput/evdev. Phone tilt is not an original GRID input, so tilt is AUTHORED. Section 6 gives one abstraction that lets every device feed the original per-device filtering and the same simulation path. The assists keep working in any combination.

## 1. What the current stack does on other platforms

### 1.1 Compile check for Android ARM64 [V]

- Installed via rustup 1.29.1: Rust std target `aarch64-linux-android` for toolchain 1.99.0. Nothing else was installed: no NDK, SDK or Gradle.
- Simulation crates:
  - `cargo check -p grid-formats -p grid-world -p grid-tools --lib --bins --target aarch64-linux-android` passes in 24 s.
  - `cargo clippy … -- -D warnings` for the same target passes, after 1.2 as well.
  - Their only external dependency is `quick-xml`.
  - Logs: `local/platforms/check-sim-android.txt`, `clippy-sim-android.txt`.
- `grid-app` (Bevy 0.19.1) stops at two blockers, as expected:
  1. android-activity 0.6.1 `compile_error!`: "Either game-activity or native-activity must be enabled". Bevy's `android-game-activity` feature is not enabled (`check-app-android.txt`).
  2. With `--features bevy/android-game-activity`, the build scripts of `blake3 1.8.7` (C) and android-activity's GameActivity glue (C++) need the NDK's `aarch64-linux-android-clang` (`check-app-android-gameactivity.txt`).
- No `cfg(windows)`, `winapi` or `windows-sys` code exists in our crates. `grid-app` is a binary only; Android needs a `cdylib` with `#[bevy_main]`.
- Bevy's `3d` feature already pulls in `default_platform`: `bevy_gilrs`, `bevy_winit`, `x11`, `wayland`, `webgl2`, `multi_threaded`. Gamepads, X11 and Wayland are therefore compiled today; Android activity features are not.

### 1.2 Approximate SSE arithmetic: the hard ARM blocker, solved [V]

**Problem.** The reference arithmetic matches GRID bit for bit, and part of it is the SSE *approximate* reciprocal (RCPPS) and reciprocal square root (RSQRTPS).

- `grid_world::spawn::reference_seed` (`crates/grid-world/src/spawn.rs:60`) feeds 34 files and about 40 call sites: collision mesh, contact manifold/rows/ray/preparation, packed shapes and contacts, poses, suspension context, vehicle manager/chassis/step/sync/post-force/driver-prepare, wheel coupling/damage, surface drag, assisted steering, deformation and impact. It also feeds spawn alignment (`Normalization::ReferenceSse`).
- `grid_tools::engine_audio_publication::root` uses RSQRTPS for the engine gain.
- On any non-x86 CPU all of these returned `Err("… requires x86 SSE")`, so the first tick of any car would fail.
- `contact_material::byte_noise` used SQRTSS and failed the same way.

These instructions are vendor-specific approximations. ARM's FRSQRTE/FRECPE give different bits, and so do AMD CPUs. Physics results, and therefore lap times, would drift.

**Measurement.** Done on the owner's validation CPU, a 12th Gen Intel Core i7-12700 (family 6, model 151, stepping 2), with the default MXCSR. Probe source: `local/platforms/sse-approx/src/`.

- RCPPS depends only on the top **11** mantissa bits. RSQRTPS depends on the exponent parity and the top **10** mantissa bits.
- Both return 12 significant bits; the low 11 mantissa bits are always 0. Each has exactly 2048 distinct mantissas.
- Other exponents only move the result exponent.
- A pure table emulation (2 × 2048 `u32`, 16 KB, FNV-1a-64 `f8d5343dbccd59f9`) matched both instructions on **all 2³² inputs with zero mismatches**. This includes ±0, denormals (read as zero, giving signed infinity), infinities, NaNs (quieted) and results that underflow or overflow.

**Port** (this session):

- New module `grid_world::sse_approx` with `rcp`/`rsqrt` and the measured tables.
- `reference_seed` and `engine_audio_publication::root` use it on non-x86 targets. The x86 path is unchanged and still executes the real instruction.
- `byte_noise` on non-x86 uses `f32::sqrt`. SQRTSS is the correctly rounded IEEE square root, so the result is the same.
- A validation-only cargo feature `soft-sse` (in grid-world; grid-tools forwards it) forces the table path on x86, so the path ARM takes can be checked on this machine.

**Checks** [V]:

- Unit test `sse_approx::tests::tables_match_this_cpu` compares against the hardware at every table cell × 10 exponents × both signs, plus a 1-in-4093 stride over all bit patterns.
- `grid-opponent-race`, release build, comparing the hardware path (`target/release`) with the `--features soft-sse` path. Runs in `local/platforms/sse-approx/runs/`:
  - **Solo Donington AI, 1 car, 11,200 ticks:** laps 88.42029812185638 / 86.28228501935392 s. Output files are byte-identical (SHA-256 `124acff5…2d9a`), and equal to the reference values in docs/DRIVING_ASSISTS_RESEARCH.md.
  - **12 cars with deformation (DF01), 12,000-tick limit:** the race ended at tick 11,800 with 827 car contacts. Standings, classification and all 12 per-car FNV state hashes are byte-identical (SHA-256 `ad7986bf…d008`).
- fmt check of the touched files; Clippy `-D warnings` with and without `soft-sse` (host, all targets) and for `aarch64-linux-android`.

**Scope and caveats:**

- The tables describe the owner's Intel CPU. The original game run on an AMD CPU would itself produce different bits. "Faithful" therefore means equal to the reference machine that validated the port.
- The emulation was verified on x86 against the hardware. It has **not** been executed on an ARM device yet; there is no device here, and no emulator was installed. The code is plain integer and bit manipulation, so no ARM-specific behaviour is expected [I].
- **Recommendation (owner rule F320: no CPU-dependent results):** the x86 build still executes the hardware instruction, so the port's physics is CPU-dependent today. On an AMD CPU, the same race can give different bits than on the reference Intel. Making the table path the default on every CPU removes that dependence. It is already proven bit-identical to the reference machine (solo and 12-car runs above), so the reference results do not move. This session leaves the desktop default unchanged; the switch is a one-line change of the `cfg` (follow-up P1b in section 7).
- Approximate RCPPS/RSQRTPS also occur in the original's other paths, such as particles. Future ports of those should call `sse_approx` rather than intrinsics.

### 1.3 Remaining cross-platform determinism risk: libm [V call sites, I risk]

`f32`/`f64` `+ − × ÷ sqrt` are IEEE-exact on x86-64 and ARM64. Rust never contracts to FMA, and the code has no `mul_add`.

The physics also calls the platform math library, through f64 and then cast to f32, so results can differ across MSVC UCRT, glibc, bionic and Apple libm:

- `sin`/`cos`: `steering_input.rs:31`, `steering_coefficients.rs:41`, `wheel_history.rs:199`;
- `tan`: `steering_coefficients.rs:126`, `suspension_context.rs:31`, `vehicle_properties.rs`;
- `atan`: `steering_feedback.rs:59`, `vehicle_post_force.rs:92`, `assists.rs:254`;
- `asin`: `vehicle_step.rs:90`;
- `acos`: `assisted_steering.rs:136`, `wheel_deformation.rs:147`;
- `atan2`: `deformation_volume.rs:152`;
- `powf`: `assisted_steering.rs:96`, `vehicle_grip.rs:174`, `assists.rs:276`.

These libraries are accurate to about 1 ulp in f64, so the f32 result differs only when the f64 value lands within an ulp of an f32 rounding boundary. That is rare but possible.

Consequence: each platform is deterministic for itself, but an Android lap is **not guaranteed** to be bit-identical to the Windows lap, and ghosts or replays could diverge. Fix, if cross-platform identity is wanted: route these calls through one pure-Rust implementation (the `libm` crate, or our own x87-faithful kernels). Then re-run the existing probe validations, because the Windows results could move too. This is a separate, optional session (P13 in section 7).

### 1.4 Desktop-only assumptions [V]

| Area | Where | Breaks on | Replacement |
|---|---|---|---|
| Install root hard-coded to `C:\Program Files (x86)\Steam\steamapps\common\Grid` | `grid-app/src/main.rs:1004`, `--game-root` at `:1086`; `tools/update_prototype.ps1:1`; the extractor scripts | everything except this PC | settings file plus discovery (Steam `libraryfolders.vdf` on Windows/Linux/Deck; the app data dir on Android) |
| CLI flags (`--drive`, `--opponents`, `--route`, `--assists`, `--grid-position`, …) parsed by hand from `std::env::args()` | `main.rs:1028-1170`; `process::exit(1)` at `:862` | Android/iOS (no argv); awkward on the Deck | `Settings` struct loaded from a file (and a future menu); CLI stays as a desktop override |
| Environment switches `GRID_FREE_LOOK`, `GRID_EFFECTS_CAMERA`, `GRID_GROUND_EFFECTS`, `GRID_GROUND_TRACE`, `GRID_AUTOPILOT`, `GRID_PROP_SCRIPT(_SPEED)`, `GRID_LOOK_BACK`, `GRID_DAMAGE_ZONES`, `GRID_DAMAGE_CAMERA` | `free_look.rs:46`, `ground_effect_assets.rs:63,78,99`, `main.rs:1342,1547,1560,2522`, `damage_parts.rs:842,846` | Android (no shell) | diagnostics fields in the same settings file |
| Launchers `Play GRID *.cmd` (cmd.exe; `GRID_PLAY_*` variables, `local\assists.txt`, logs in `local\playtest\`) | repo root | all non-Windows | shell scripts on Linux/macOS; the settings file on Android |
| CWD-relative writes: `local/playtest/time-trial-records.tsv`; renders forced under `local/` | `time_trial.rs:22`, `main.rs:1387` | Android (read-only CWD) | per-platform data dir (`AndroidApp::internal_data_path()`, XDG, `%APPDATA%`) |
| Private parameter files CD01 (712 B), BP01 (28 B), DF01 (252 B) for body contacts, opponents/race and damage | `main.rs:1238-1255`; produced by `tools/extract_*.py` (Python + Capstone) | any user other than the owner | BP01 and DF01 read plain data from the purchased `grid.exe` (SHA-256 `dd7b0933…`): port the readers to Rust and run them at import. **CD01** interprets an initializer in the *Steamless-unpacked* image; the Steam `grid.exe` code section is SteamStub 2.1 encrypted (docs/COMPATIBILITY_MEMORY_PATCHES.md). Needs its own research: compute the table without the unpacked image, or require a documented desktop preparation step |
| Whole-file `std::fs` reads from a real directory tree | `main.rs:350-363`, `source_vehicle.rs:91`, `dynamic_props.rs:250` | fine on Android **inside the app's own directories**; not for SAF `content://` URIs | import into app-specific storage first (section 3) |
| File-name case | literals verified to match the install's casing; names derived from data (`effects/particle_systems/{name}.xml` from XML, the engine ini path from `carsetup.bin`) are unchecked | Linux/Android/macOS (case-sensitive filesystems) | case-insensitive resolver built once from a directory listing (one index per install) |

The runtime itself never calls Python or anything in `tools/`. Those scripts are research and preparation only.

### 1.5 Renderer [V code, S device data]

- The app asserts the GPU feature `TEXTURE_COMPRESSION_BC` (`main.rs:1661-1666`).
- Every texture is uploaded as BC1/BC3 DDS, or BGRA8 for a few named ones (`grid-formats/src/texture.rs`; `main.rs:1669`, `:1842`; `ground_effect_assets.rs:42`; `tyre_smoke.rs:77`; `damage_parts.rs:1144`).
- Bevy's DDS loader does **not** decompress on the CPU; it fails with "Format not supported by this GPU".
- Android support for BC:
  - Vulkan `textureCompressionBC` is reported by about 37.5 % of Android devices, against about 99.9 % for ETC2/ASTC (vulkan.gpuinfo.org).
  - Adreno 7xx/8xx usually has it; Mali, Xclipse and PowerVR usually do not.
- Options:
  - (a) Decode BC1/BC3 to RGBA8 on the CPU at load (`bcdec_rs`, `texture2ddecoder`). Simple, but 4× (BC3) to 8× (BC1) the GPU memory.
  - (b) Transcode once, at import, to ASTC 6×6/8×8 or ETC2 in a private cache (KTX2). The cache is a derivative of the user's own data and stays on their device.
  - Recommended: (a) as a fallback everywhere; (b) for Android memory.
- No custom shaders, storage buffers or compute exist; everything is `StandardMaterial`. So the GLES fallback is plausible but unmeasured.
- Heavy items for a phone:
  - one directional light with default shadow cascades;
  - grass: about 531k (Donington) and 540k (Nürburgring) CPU-rebuilt billboard instances, with `AlphaToCoverage`, which needs MSAA;
  - CPU-rebuilt smoke, particle, skid and debris meshes every frame;
  - CPU re-projection of body damage meshes.
- Settings needed: grass density, shadow quality, MSAA, effects budget. Some already exist as `GRID_GROUND_EFFECTS=0`.
- Bevy open Android issues to watch:
  - #23754 PowerVR shader crash (Pixel 10);
  - #24926 Adreno skinning corruption (GRID cars are not skinned) [S/I];
  - #16798 crash with more than 8 touches;
  - #9591 `load_folder`.

### 1.6 Memory and asset size [V sizes, I estimates]

- The installation is **10,319 MB in 22,375 files**:

  | Directory | Size |
  |---|---|
  | `cars` | 3,431 MB |
  | `audio` | 3,420 MB (speech 2,581, music 312, car audio 1.5) |
  | `tracks` | 2,321 MB |
  | `video` | 602 MB |
  | `interiors` | 333 MB |
  | `frontend` | 50 MB |
  | `ghostlaps` | 46 MB |

- What one race reads today: Lacetti `cars/lac` (105 MB on disk, of which the race reads the meshes plus one 6.1 MB livery per car), `tracks/europe/donington` (95 MB; Nürburgring 104 MB), audio WIPs (19 MB), effects and skid marks (about 3 MB). The raw read is roughly 100–130 MB per race.
- A phone needs only the subset for the routes and cars the port supports. That is a few hundred MB today, not 10 GB.
- No RSS measurement exists. Known duplications:
  - textures kept in both the main and render worlds (`RenderAssetUsages::MAIN_WORLD | RENDER_WORLD`);
  - the `Car.textures` DDS byte map kept for the app's lifetime (`main.rs:50`);
  - the grass instance array.
- A memory pass belongs to the Android performance session (P12).
- CPU: 1.3–1.7 ms per 12-car tick (2.2–3.0 ms with deformation) on the i7-12700 P-core, at 62.5 Hz (docs/TICK_PERFORMANCE.md).
- Geekbench 6 single-core ratios [S]:
  - Snapdragon 8 Gen 3 ≈ 0.78× a desktop i7-13700K;
  - Snapdragon 8 Elite Gen 5 ≈ 1.3×;
  - mid-range (Snapdragon 7s Gen 3, Dimensity 7300) ≈ 0.36–0.41×.
- The estimate is about 2–4 ms per tick on a flagship and 4–8 ms on a mid-range phone, plus thermal throttling, against the 16 ms tick. Feasible if the physics gets its own big core. Rendering, especially grass and shadows, is the bigger risk [I].

## 2. Platforms

| Platform | Difficulty | Blockers | Effort (sessions) | Notes |
|---|---|---|---|---|
| **Linux (x86-64)** | Low | install discovery and case-insensitive lookup (1.4); `.cmd` launchers; untested Wayland/X11 and ALSA builds | 1–2 | Same x86 SSE path, so physics is bit-identical apart from libm (1.3). Bevy deps: `libx11-dev libasound2-dev libudev-dev libxkbcommon-x11-0`, plus Wayland. BC textures are supported on desktop GPUs |
| **Steam Deck** | Low after Linux | controller input (P2); the 1280×800 window already matches; settings without a shell | +1 | Native build inside Steam Linux Runtime 3.0, or the Windows exe under Proton (untested). For "Verified": a default controller config and an on-screen keyboard. Steam Input can map the Deck controls to a virtual pad that gilrs sees |
| **macOS (Apple Silicon)** | Medium | ARM (solved in 1.2); BC textures are supported on Apple Silicon Metal [I]; signing and notarization ($99/yr) | 1–2 | wgpu/Metal. libm differences apply |
| **Android (ARM64)** | Medium–high | BC textures (1.5); settings, paths and launchers (1.4); asset import (3); CD01 (1.4); NDK/Gradle/GameActivity wrapper; touch and tilt input; performance and thermal | 8–11 | Section 7 plan. gilrs does not support Android, so pads there need an android-activity input bridge |
| **iOS** | High | as Android, plus Apple distribution: no sideloading without a developer account, and store review of a game that needs user-supplied data | — | CoreMotion via `objc2-core-motion`. Not recommended |
| **Consoles** | Not realistic | NDA SDKs, no public Bevy support (bevy#8161 open), certification | — | — |

**Recommended order:** input abstraction and gamepad first (they benefit the desktop now). Then the portability groundwork, which is shared by every platform. Then Linux and the Deck as a cheap check of that groundwork. Then Android.

## 3. Assets on the device: a legal flow

**Rule:** no original file, extracted asset, private parameter table or transcoded derivative is ever put into the repository, an APK or any download. The APK contains only our code. Everything else comes from the user's own installation, on their own device.

### Recommended flow (AUTHORED design; untested)

1. **The user copies their own installation** to the phone over USB/MTP: either the whole `Grid` folder, or the subset that a manifest lists (section 1.6). The destination is a shared-storage folder such as `Documents/GRID`.
   - From target API 30, the system folder picker cannot select `Android/data` or `Download` roots, but `Documents/GRID` is allowed.
2. **First run:**
   - The app opens the system folder picker (`ACTION_OPEN_DOCUMENT_TREE`) and takes a persistable read grant.
   - It copies the needed files through `ContentResolver`, via JNI, into **app-specific external storage** (`getExternalFilesDir()`, `AndroidApp::external_data_path()`). That needs no permission, and `std::fs` works there.
3. **The importer verifies** each file against a manifest of SHA-256 hashes of the supported Steam build. The manifest contains hashes only, never content. The importer also builds the case-insensitive index.
4. **Optionally, the importer derives** private caches in the same app directory:
   - transcoded textures (ASTC/ETC2);
   - BP01 and DF01 from the user's `grid.exe` (ported to Rust);
   - later CD01 (open, see 1.4).
5. **The game reads** only from app-specific storage. Uninstalling the app removes the copy (Android deletes the app-specific directory).

### Alternatives

- **Direct MTP copy into `Android/data/<package>/files/`:** fast, and the "advanced" path. MTP from a PC still reaches `Android/data` on Android 13/14 in reports, but this is not officially documented [S]. Phone file managers cannot reach it.
- **`adb push` / `run-as`:** for development only.
- **`MANAGE_EXTERNAL_STORAGE`:** not acceptable; Google Play allows it only for file managers and similar apps.

### Desktop

The same importer logic applies to Linux, the Deck and macOS:

- find the Steam library (`libraryfolders.vdf`) or let the user pick the folder;
- verify the files against the manifest;
- read the files in place, with no copy.

## 4. What the original GRID does per input device

Static research, all **[D]** unless marked. Full addresses and evidence: `local/platforms-input/ORIGINAL_INPUT_DEVICES.md` (decompilation rounds `decomp/r1..r9`, `human-update.asm`, `poll.asm`, `ffb-output.asm`). This extends docs/DRIVING_ASSISTS_RESEARCH.md §2.3–2.5 and docs/STEERING.md F116/F121.

### 4.1 Back end and action maps

- **Input APIs:** DirectInput 8 (keyboard, mouse, joysticks and wheels) plus XInput 1.3 (`XInputGetState`/`SetState`/`GetCapabilities` by ordinal).
  - A WMI `IsXInputDevice` test removes Xbox 360 devices from DirectInput.
  - There is no Raw Input; window messages are used only for the message pump and text.
- **DirectInput axes:** range set to 0..65535 with no driver dead zone, normalised raw/65536.
- **XInput:** trigger = b/256; thumb = (s + 32768)/65536. Disconnected pads are polled round-robin, one slot every 4 polls.
- **Actions** (table `0xE4DEE8`):

  | Id | Action |
  |---|---|
  | 0 | Accelerate |
  | 1 | Brake |
  | 2 | Handbrake |
  | 3 | SteerL |
  | 4 | SteerR |
  | 5 | View |
  | 6 | GearUp |
  | 7 | GearDown |
  | 8–0xC | Look left / right / up / down / back |
  | 0xD | Pause |
  | 0xE | Boost |
  | 0xF | Instant Replay |
  | 0x10–0x15 | Gear 1–6 |
  | 0x16 | Reverse |

- **Presets** `ActionMap/actionMapPreset0..7.xml`:

  | Preset | Device | Steering / pedals |
  |---|---|---|
  | 0 | Keyboard | arrows; steer dead zone 0.2, which has no effect on 0/1 keys; Gear Up A, Gear Down Z; Space handbrake; End look back; C view; Esc pause |
  | 1 | Logitech G25 | X axis split into SteerL/SteerR halves, dead zone 0; pedals Y/Rz uniNegative plus Slider0 split for combined pedals; H-shifter gears 1–6 on buttons 8–13, reverse on 14 |
  | 2 | MOMO | like G25 |
  | 3 | Driving Force Pro | like G25 |
  | 4 | XInput pad | left stick X, dead zone 0.2; triggers |
  | 5 | RGT FFB Pro | — |
  | 6 | XInput wheel | — |
  | 7 | Logitech RumblePad 2 | X axis, dead zone 0.2; one Rz axis split into accelerate/brake, dead zone 0.2 |

- **Preset choice** (`83A130`):
  - The keyboard is first.
  - A DirectInput device needs its **exact product name** to equal a preset's `deviceType`. There is no generic DirectInput fallback, so an unknown wheel gets no binding.
  - An XInput device gets the wheel scheme when its SubType is WHEEL, otherwise the pad scheme.
- **Calibration** (`AD9AD0`/`AD9A80`), with raw ∈ [0, 1] and per-binding dead zone dz and saturation sat:

  | Type | Range [lo, hi] | Inverted |
  |---|---|---|
  | uniPos | [dz, sat] | no |
  | uniNeg | [1 − sat, 1 − dz] | yes |
  | biUpper | [0.5 + 0.5·dz, 0.5 + 0.5·sat] | no |
  | biLower | [0.5·(1 − sat), 0.5·(1 − dz)] | yes |

  The value is v = (clamp(raw, lo, hi) − lo)/(hi − lo), or 1 − v when inverted. An action's value is the **maximum** over its bindings.

### 4.2 Human record and per-device filtering (`827D10`, `828670`)

- **Record values:**
  - accel = clamp(action 0, 0, 1);
  - brake = clamp(action 1, 0, 1);
  - handbrake = action 2 > 0.5;
  - **steer s = SteerR − SteerL**;
  - gear up/down = pressed edges of actions 6/7;
  - direct gear = last pressed of 0x10..0x16.
- **Keyboard: no ramp.** There is no ramp, rate limit or smoothing for digital steering, throttle or brake at the input layer.
  - Keys give 0/1 and s ∈ {−1, 0, +1} instantly.
  - All softening for keyboard and pad comes later, in the vehicle's *assisted* steering: `98F790`, then `9B3110` with +E6C = 1 (FDC dead zone × 0.99, power response, speed-interpolated maximum lock, E70 speed reduction, slew `+1F2C·Δt·5`). That is already ported.
  - **The port's digital keyboard path is therefore ORIGINAL as it is.**
- **Assisted flag** (record +0x33 → vehicle +E6C):
  - Keyboard, XInput pad and RumblePad 2 are assisted (1).
  - DirectInput wheels and XInput wheels (SubType 2) are unassisted (0).
  - The hardware setting `input/device_type` = `pad`/`wheel` overrides the choice.
- **Wheel only** (+E6C = 0):
  1. Latency extrapolation. A 4-entry ring of (dt, s) gives `s += mean(3 slopes)·dt`. A global 8-sample history then adds `mean(7 slopes)·0.05`. The extrapolated value is written back, so later slopes include it.
  2. Command shaping `828670`:
     - ratio 1.5 (1.0 when car +50C > 50);
     - power response k = 1 + 0.5·resp (resp ≥ 0), else 1/((1 − resp)·0.5);
     - resp = (wheelLinearity − 0.5)·2.
     - Ported as `assists::wheel_command`; the extrapolation is not ported.
- **Advanced Controls options** (`persistentdataupdate.xml` `<adv_controls>`, integer sliders read as value/max):

  | Option | Default | Effect |
  |---|---|---|
  | `wheelLinearity` | 14/20 = 0.7 | resp 0.4, k 1.2. Range: resp −1..+1, k 0.25..1.5 |
  | `steeringSaturation` / `steeringDeadzone` | 20/20, 0/20 | overrides sat/dz of **every** binding of actions 3/4, via `654C20` at race setup and on options apply |
  | `accelerateSaturation` / `accelerateDeadzone` | 20/20, 0/20 | the same for action 0 |
  | `breakSaturation` / `breakDeadzone` (sic) | 20/20, 0/20 | the same for action 1 |
  | `feedback_force` | 5/10 = 0.5 | force-feedback force gain |
  | `feedback_friction` | 5/10 = 0.5 | force-feedback friction gain |
  | `feedback_effects` | 5/10 = 0.5 | force-feedback effects gain |
  | profile `forcefeedback` | 1 | FFB on |

  - `4CC630` sets steering dz 0.05 and sat 1.0 for controller types 1–3.
  - Open: whether a fresh profile's pad dead zone is the preset's 0.2 or the slider's 0 (section 8).

### 4.3 Force feedback and rumble

- **Data:** `forcefeedback/forceFeedbackPaths.xml` loads three files through processor `GameFFB`:
  - `devicesetup.xml` — per device name, with fallback `default`: scaleForce, scaleFriction, baseFriction, maxFriction, scaleEffects, delay;
  - `effectsetup.xml` — FFBCaster, FFBlateral, FFBSurface, FFBGearChange, FFBWheelDamage, FFBSkid, FFBSuspension, FFBCollision and FFBSample (wave/envelope digit strings);
  - `carsetup.xml` — per-car gains.
- **Thread:** `7ED380`, "FFB updateTask", runs on the hardware-settings core with `Sleep(10)`, so about 100 Hz or less. It is asynchronous to physics and consumes per-car snapshots queued by the game thread.
- **Caster force** (`7E8580`), per front wheel:
  1. tq = −trail·Fx + toe·Fz, plus a camber term;
  2. c = mean(tqL, tqR);
  3. g = pow((|c| + preload)/cornerPoint, exponent)·scale·cornerPoint/(|c| + preload);
  4. force = clamp(g·c, ±scale), times a speed fade (shipped: 2.5 → 6 m/s).
  - Shipped values: scale 2, trail 1, cornerPoint 0.075, exponent 0.35.
- **Friction:**
  - lateral friction 25 below 2.5 m/s, 5 above 6 m/s;
  - steering-to-grip angle friction 17.5 → 6.5 between 6.5° and 25°;
  - the sum is clamped to the device's [baseFriction, maxFriction] (G25 max 45).
- **Vibration samples:** collision, surface/kerb RUMBLESTRIP, gear change, skid, suspension, wheel damage.
  - Wave digit d → (d − 5)/4; envelope digit e → e/9.
  - The result is added to the wheel force, or sent to pad motors.
- **DirectInput device:**
  - `DIPROP_AUTOCENTER` off.
  - **One infinite GUID_ConstantForce effect**, the only effect GUID in the executable. There is no spring, damper or periodic effect.
  - Damping is computed in software from the wheel position:
    1. vel = ((vel + Δpos)/dt)·0.005;
    2. damp = vel·friction;
    3. damp is averaged with the previous value on a sign flip;
    4. magnitude = trunc((force + effects + damp)·10000).
- **XInput pads:** `XInputSetState(|ch0|·65535, |ch1|·65535)` from the vibration samples only.
- **Telemetry:** the `motion` block in `hardware_settings_config.xml` (UDP 127.0.0.1:20777) exists and is disabled. Not analysed.

### 4.4 Consequences for the port

- Keyboard: ORIGINAL already. No ramp should be added.
- Pad: ORIGINAL = assisted steering, plus the preset calibration (left-stick biUpper/biLower dz 0.2, triggers uniPos) and the Advanced Controls overrides.
- Wheel: ORIGINAL = unassisted steering, plus extrapolation, `828670` shaping and calibration.
- FFB: ORIGINAL = a single constant-force channel plus software damping. That is the easiest effect to reproduce on every FFB API: SDL3 `SDL_HAPTIC_CONSTANT`, Linux `FF_CONSTANT`, DirectInput, and Windows.Gaming.Input `ConstantForceEffect`. The missing piece is the snapshot builder: which physics values fill the per-car FFB snapshot (section 8).
- **Phone tilt and touch are not original inputs.** They are AUTHORED device classes that must map onto one of the original classes, keyboard/pad or wheel (section 6).

## 5. Input in the port today [V]

- **Bevy resources read:**
  - `ButtonInput<KeyCode>` only: `drive_tick` (`main.rs:2042`), `drive_actions` (`:2330`), `drive_present` (`:2419`), `assists.rs:21`, `engine_sound.rs:167`.
  - Mouse for the free-look camera (`free_look.rs:56`).
  - Nothing reads `Gamepad` or `Touches`.
  - The Windows gilrs log line `0x80070057` seen in earlier sessions comes from the compiled-in but unused `bevy_gilrs`.
- **Keys:**

  | Action | Keys | Where |
  |---|---|---|
  | Throttle | Up / W | `main.rs:2140` |
  | Brake | Down / S | `main.rs:2141` |
  | Steer | Left/A, Right/D | `main.rs:2142` |
  | Handbrake | Space | `main.rs:2144` |
  | Manual shift up / down | X / Z, edge-triggered | `assists.rs:22` |
  | Reset | R | |
  | Pause | P | |
  | Finish race | F | |
  | Quit | Esc | |
  | Look back | C | |
  | Free look | right mouse | |
  | Mute engine | M | |

- **Values:**
  - Steering is a digital −1/0/+1 (positive = left, `source_vehicle.rs:73`); pedals are 0/1.
  - There is no app-side ramp: the car's own assisted steering (`vehicle_controls.rs:75-108`, original 9B3110 with +E6C = 1) supplies the slew and dead zone from car data.
  - Whether the original's keyboard path adds a ramp *before* the vehicle is answered in section 4.
- **Hand-off to the simulation:**
  - `Controls { throttle, brake, steering, handbrake }` (`source_vehicle.rs:69`) → `Runtime::tick…` or `MultiRuntime::tick(&[Controls])`.
  - `staged_car` (`source_vehicle.rs:587`) range-checks the values, then `stage_driver` writes +E1C/+E3C, +E20/+E40, +E30 and +E38.
- **Device-dependent part:**
  - `Assists.input` is `Keyboard` or `Wheel`. `pad` and `gamepad` parse as Keyboard (`assists.rs:132`).
  - `Wheel` sets +E6C = 0 (unassisted steering) and shapes the command with `wheel_command` (original 828670: ratio 1.5 or 1.0, front-axle heading blend, power response).
  - The wheel-axis latency extrapolation is not ported.
- **Timing:**
  - Input is read inside `FixedUpdate` at 62.5 Hz (`DELTA = 0.016`, `main.rs:1590`), so all fixed ticks of one frame see the same keys.
  - [I] Bug risk: `shift_keys` uses `just_pressed` inside `FixedUpdate`. A frame with two ticks requests the shift twice; a frame with zero ticks drops it. Section 6 fixes this with an edge queue.

## 6. Input architecture (design)

**Goal:** every device feeds the same simulation call, through the same original per-device filtering. The assists (TC/SC/BA/gearbox) stay independent of the device and work in any combination.

```
 device back ends              action layer (ORIGINAL)          human record (ORIGINAL)        simulation
 ─────────────────            ───────────────────────          ───────────────────────        ──────────
 keyboard  (bevy ButtonInput)  ┐
 pad       (bevy gilrs; Android ├─► bindings + calibration ──► 827D10 record:            ──► Controls + Shift
            input bridge)       │   (uniPos/uniNeg/biUpper/     s = R − L, accel, brake,      → Runtime / MultiRuntime
 wheel     (SDL3 joystick)      │    biLower, dz, sat,          handbrake > 0.5, gear edges,    (unchanged API)
 touch HUD (bevy Touches)  AUTH │    max over bindings)         device class → +E6C,
 tilt      (Android sensor) AUTH┘                               wheel: extrapolation + 828670
                                                                        │
 FFB/rumble output ◄── GameFFB port (ORIGINAL, ~100 Hz thread) ◄── per-car FFB snapshot (tyre forces, speed, surface, impacts)
```

### 6.1 Layers

1. **Back ends** produce *raw sources*:
   - `Source { device, axis_or_button, raw ∈ [0, 1] }`, with axes centred at 0.5 like DirectInput's 0..65535 / 65536;
   - XInput-style triggers b/256 and thumbs (s + 32768)/65536 when the device reports integers.
   - Each back end is a Bevy plugin behind a cargo feature or a `cfg` (`input-sdl3`, Android-only modules), so the desktop build is unaffected until enabled.
2. **Action layer** (ORIGINAL):
   - The 23 original actions, each with a list of bindings `{source, calibration type, dz, sat, threshold}`.
   - An action's value is the maximum over its bindings.
   - Presets are read **from the user's own `ActionMap/actionMapPreset*.xml`** at runtime, so they are never copied into the repo.
   - Our own layer: generic presets for devices the original does not know, marked AUTHORED, chosen when no exact product-name match exists (the original has no fallback). It also holds the Advanced Controls overrides (steering/accelerate/brake dz/sat, wheelLinearity, feedback_*) in the settings file.
3. **Human record** (ORIGINAL, a port of `827D10` + `828670`), once per fixed tick:
   - s = SteerR − SteerL, accel, brake, handbrake > 0.5;
   - gear up/down and direct gear from an **edge queue** filled in `PreUpdate` and drained one event per tick. This fixes the `just_pressed`-in-`FixedUpdate` double/lost shift (section 5).
   - the device class sets `Assists.input`, i.e. +E6C;
   - the wheel class runs the latency extrapolation (to port) and `wheel_command` (ported).
   - The output is the existing `Controls` plus `request_shift`. Nothing below this line changes.
4. **Settings:** `assists` keeps traction/stability/braking/gearbox. `input` becomes *derived* from the active device class (keyboard/pad → assisted, wheel → unassisted). An explicit override keeps the original's `input/device_type` = pad/wheel semantics.

**Device classes:**

| Class | Assisted (+E6C) | Extrapolation + 828670 | Source |
|---|---|---|---|
| keyboard | 1 | no | ORIGINAL |
| pad (XInput, RumblePad 2) | 1 | no | ORIGINAL |
| wheel (DirectInput / XInput wheel) | 0 | yes | ORIGINAL |
| **touch HUD** | 1 (as keyboard: digital buttons) | no | AUTHORED |
| **tilt** | 1 by default (as a pad stick); 0 optional ("wheel-like" mode) | only in wheel-like mode | AUTHORED |

### 6.2 Phone tilt and touch (AUTHORED)

- **Sensor:** `TYPE_GAME_ROTATION_VECTOR` through `ndk-sys` `ASensorManager_*` (API 26+) at 60–100 Hz. It is gyro + accelerometer without the magnetometer, which Google recommends for games. The 200 Hz cap needs no extra permission.
- **Steering value:**
  - roll = rotation of the device about its screen-normal axis in landscape, relative to a **calibrated neutral** (captured at race start and from a "re-centre" button);
  - tilt = clamp(roll/maxAngle, −1, 1), with maxAngle 30° AUTHORED and adjustable;
  - the result is fed as a *bi-axis* source raw = 0.5 + 0.5·tilt into SteerL/SteerR with biLower/biUpper calibration. The pad dead zone and saturation logic is therefore reused, and the original vehicle-side assisted steering does the rest.
  - No extra smoothing beyond the sensor's own filter at first; add a one-pole low-pass only if testing shows jitter. Any such filter is AUTHORED.
- **Touch HUD:** Bevy `Touches`.
  - Bottom-right accelerate, bottom-left brake, handbrake, and shift up/down shown in Manual.
  - Pause, look back and re-centre.
  - Digital 0/1, like keyboard keys, which is ORIGINAL for digital pedals.
  - Optional AUTHORED analog throttle: vertical position within the pedal.
  - Keep fewer than 8 simultaneous touches (Bevy issue #16798).
- **Screen:** landscape lock (`android:screenOrientation="sensorLandscape"`, `android:appCategory="game"` so Android 16 large-screen rules keep it) and `KEEP_SCREEN_ON` through `AndroidApp::set_window_flags`.

### 6.3 Gamepad (ORIGINAL pad semantics)

- **Desktop:** Bevy `Gamepad` entities (gilrs 0.11: Windows.Gaming.Input/XInput on Windows, evdev on Linux).
  - Set Bevy's `GamepadSettings` dead zones to 0, so the original calibration alone applies. Bevy's default is ±0.05.
  - Map Bevy's −1..1 stick to raw = 0.5 + 0.5·x; triggers 0..1 map directly.
- **Rumble:** Bevy `GamepadRumbleRequest` from the GameFFB vibration channels:
  - ch0 → strong (left) motor, ch1 → weak (right) motor, as `XInputSetState`;
  - duration one FFB tick, renewed.
- **Android:** gilrs has no Android back end. Read joystick axes from android-activity input events (`Axis::X/Y/Gas/Brake/…`). Whether GameActivity forwards them needs a device test.

### 6.4 Wheel and force feedback

- **Back end: SDL3**, joystick and haptic subsystems only, no SDL window; the `sdl3` crate 0.20.
  - DirectInput on Windows, evdev on Linux, IOKit on macOS.
  - SDL 3.4 adds a HIDAPI driver for Logitech G25/G27/G29/DFGT.
  - The alternative per-OS route is `windows` crate DirectInput8 COM plus the `evdev` crate. That is more code for the same coverage.
  - Risks: duplicate enumeration with gilrs (disable gilrs for devices SDL claims) and conflicts with Logitech G HUB. Record SDL's version and hash when it is installed.
- **Device matching:**
  - Exact product-name match against the user's presets (G25, MOMO, DFP, RGT FFB Pro), as the original does.
  - Otherwise an AUTHORED generic wheel preset: X axis split, separate pedals with uniNeg calibration, user calibration screen.
- **FFB:** port GameFFB as one module.
  - Inputs: the per-car snapshot (needs the snapshot-writer research, section 8) and the user's `forcefeedback/*.xml`.
  - Its own ~100 Hz thread, like the original.
  - One `SDL_HAPTIC_CONSTANT` effect with infinite length, plus the software damping. This equals the original's single DirectInput ConstantForce, so the force path is ORIGINAL on any SDL-supported wheel.

### 6.5 Timing

The original samples input once per rendered frame. Its order against the physics substeps is not yet traced (section 8). The port samples per frame into a latched snapshot that every fixed tick of that frame reads, plus the edge queue for one-shot actions. This is the current behaviour, minus the edge bug.

## 7. Plan: independent sessions

Effort is in focused agent sessions. Sessions in the same row group can run in parallel; arrows show dependencies.

| Id | Session | Depends on | Effort | Done when |
|---|---|---|---|---|
| **P0** | ✔ This session: research, plan, SSE table emulation (1.2) | — | done | sim crates build and lint for `aarch64-linux-android`; table path bit-identical on x86 |
| **P1** | **Input abstraction refactor**, desktop: action layer, calibration, human record, edge queue, device class → `Assists.input`, settings file section `[input]` | — | 1 | keyboard path bit-identical (solo laps 88.42029812/86.28228502; assists matrix unchanged); unit tests for the four calibration types and the max-over-bindings rule; double/lost-shift test |
| **P1b** | **CPU-independent physics**: make `sse_approx` the default on x86 too (drop the hardware branch or keep it behind a feature), and re-run the solo, 12-car and assists-matrix identity checks | — | <1 | identical results on the reference CPU; no hardware RCPPS/RSQRTPS left in the runtime |
| **P2** | **Gamepad**: gilrs source, original pad preset from the user's `actionMapPreset4.xml`, Bevy dead zones 0, Advanced Controls dz/sat, rumble from vibration samples (stub until P4) | P1 | 1 | an Xbox-type pad drives a lap; the owner checks feel; the fresh-profile dz question (section 8) recorded |
| **P3** | **Wheel input**: SDL3 joystick source (tool install recorded), exact-name preset match + AUTHORED generic preset, wheel latency extrapolation port (ring + global history) with unit tests, wheelLinearity option | P1 | 1–2 | wheel steering with unassisted steering and 828670 shaping; extrapolation matches a decompiled-formula unit test |
| **P4** | **Force feedback**: snapshot-writer research (fields +0x80.., +0x1BC.., +0x1D8..), GameFFB port (caster, lateral, surface/kerb, gear, skid, suspension, wheel damage, collision samples), SDL3 constant force + software damping, pad rumble | P3 (P2 for rumble) | 2 | the owner feels centring, kerbs and impacts on their wheel; numeric unit tests of the caster/friction/sample formulas against the decompiled code |
| **P5** | **Portability groundwork**: `Settings` file replacing env switches and launcher variables (CLI stays as an override), install discovery (Steam `libraryfolders.vdf`) and case-insensitive file index, per-platform data dir for records/logs, Rust port of the BP01/DF01 readers from the user's `grid.exe` | — | 1–2 | the Windows playable runs from the settings file with no env vars; the BP01/DF01 bytes are equal to the Python output |
| **P6** | **CD01 without the unpacked image** (research): find whether the 44-direction table can be computed from data, or define a documented one-time desktop preparation step | — | 1 | a decision recorded; ideally CD01 bytes reproduced from the Steam `grid.exe` alone |
| **P7** | **Linux / Steam Deck**: native build, Bevy deps, case-sensitivity run (Donington and Nürburgring), shell launchers; Deck controller via P2 | P5 (P2 for the Deck) | 1–2 | a Linux lap equal to Windows apart from the libm caveat (1.3) |
| **P8** | **Texture fallback**: remove the BC assert; CPU BC1/BC3 decode when the GPU lacks BC (`bcdec_rs` or our own decoder); optional import-time ASTC/ETC2 cache | — | 1 | the app renders with BC disabled on desktop (forced fallback flag), with a recorded memory delta |
| **P9** | **Android skeleton**: install and record SDK cmdline-tools, NDK r30 LTS, JDK, Gradle/AGP, cargo-ndk 4.1.2; `grid-app` as `cdylib` with `#[bevy_main]`; Gradle project from Bevy's mobile example (GameActivity 4.4.x matching android-activity 0.6.1, minSdk 31 or 26, target 36); landscape, keep-screen-on; lifecycle (suspend/resume) | P5, P8 | 1–2 | an APK under `local/` that starts on the owner's phone and shows the car on Donington from data placed with `adb push` |
| **P10** | **Android data import**: folder picker (`ACTION_OPEN_DOCUMENT_TREE`) via JNI, copy to app-specific storage, hash manifest, BP01/DF01 import (P5), progress UI | P9 | 1–2 | data copied from `Documents/GRID` on the phone; the race starts with no adb |
| **P11** | **Android controls**: tilt source (`ndk-sys` sensor, calibration, maxAngle setting), touch HUD, optional Android pad bridge | P1, P9 | 1 | **the fun target**: a lap steering by tilt with on-screen throttle and brake |
| **P12** | **Android performance and memory**: on-device profiling, tick time on a big core, grass density/shadows/MSAA settings, texture memory, thermal (ADPF) | P9–P11 | 1–2 | a stable 62.5 Hz tick and an acceptable frame rate on the owner's phone, with measurements recorded |
| P13 | optional **cross-platform determinism**: one libm for all platforms, then re-run the probe validations | — | 1 | Windows/Linux/Android solo laps bit-identical |

- **Recommended order:** P1b (cheap, removes CPU dependence now) → P1 → P2 (immediate desktop value) ∥ P5 ∥ P8 ∥ P6 → P9 → P10 ∥ P11 → P12. P3/P4 follow when the owner wants the wheel; they need the owner's wheel for live checks. P7 any time after P5.
- **Shortest path to the Android tilt build:** P1, P5, P8, P9, P11 (using `adb push` instead of P10), P12. That is about 6–9 sessions.

## 8. Open questions and tests that need hardware

1. **Android device:** the tables of 1.2 have not been executed on ARM. P9 should run the unit tests (or a small `grid-world` self-test) on the phone.
2. **Original pad dead zone on a fresh profile:** 0.2 (preset) or 0 (Advanced Controls default). Settle it with a live read of the Advanced Controls screen with a pad attached, or a dump of the binding calibration blocks. Needs the owner's approval to run the original.
3. **Input sampling against physics substeps** in the original: trace the order of `AFDB50`/`827D10`/`828670` against `A03670`.
4. **FFB snapshot writer:** which physics values fill the per-car snapshot. This is static research and can be done in P4.
5. **Missing pedal axis behaviour** (uniNegative with no source reads 1.0 [I]), the SetCooperativeLevel flags, and whether the native-friction effect is ever created.
6. **GameActivity joystick forwarding** and MTP access to `Android/data` on the owner's phone.
7. **Vulkan vs GLES stability** on the owner's phone GPU (Mali/Adreno/Xclipse), and whether it supports BC.
8. **SDL3 alongside gilrs and Logitech G HUB** on Windows.
9. **libm differences** between platforms (1.3): measure in P7 or P13 before promising identical ghosts or replays across platforms.

## 9. References

Official sources unless marked [S]. Full annotated web notes: the session's shared notes file `platforms-input-web-research.md` (outside the repo).

- Bevy:
  - 0.19: https://bevy.org/news/bevy-0-19/
  - 0.20 (released 2026-10-08): https://bevy.org/news/bevy-0-20/
  - mobile example: https://github.com/bevyengine/bevy/tree/v0.19.1/examples/mobile
  - Android asset reader: https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_asset/src/io/android.rs
  - DDS loader: https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_image/src/dds.rs
  - gamepad: https://docs.rs/bevy/0.19.1/bevy/input/gamepad/index.html
  - touches: https://docs.rs/bevy/0.19.1/bevy/input/touch/struct.Touches.html
  - Android issues: https://github.com/bevyengine/bevy/issues?q=is%3Aissue%20state%3Aopen%20label%3AO-Android
  - console issue: https://github.com/bevyengine/bevy/issues/8161
- android-activity:
  - `AndroidApp`: https://docs.rs/android-activity/latest/android_activity/struct.AndroidApp.html
  - releases: https://github.com/rust-mobile/android-activity/releases
  - input axes: https://docs.rs/android-activity/latest/android_activity/input/enum.Axis.html
- Android NDK and SDK: NDK downloads https://developer.android.com/ndk/downloads; 16 KB pages https://developer.android.com/guide/practices/page-sizes; target SDK https://developer.android.com/google/play/requirements/target-sdk
- Android sensors: overview https://developer.android.com/develop/sensors-and-location/sensors/sensors_overview; NDK sensor API https://developer.android.com/ndk/reference/group/sensor; `ndk-sys` https://docs.rs/ndk-sys/latest/ndk_sys/fn.ASensorManager_getInstanceForPackage.html
- Android storage:
  - Android 11 storage: https://developer.android.com/about/versions/11/privacy/storage
  - app-specific storage: https://developer.android.com/training/data-storage/app-specific
  - documents and the folder picker: https://developer.android.com/training/data-storage/shared/documents-files
  - all-files access policy: https://support.google.com/googleplay/android-developer/answer/10467955
  - [S] MTP to `Android/data`: https://esper.io/blog/android-dessert-bites-28-file-manager-loophole-closed-73891524
- Android behaviour and performance: Android 16 orientation changes https://developer.android.com/about/versions/16/behavior-changes-16; ADPF thermal https://developer.android.com/games/optimize/adpf/thermal
- Texture compression: [S] https://vulkan.gpuinfo.org/listfeaturescore10.php?platform=android ; [S] https://web3dsurvey.com/webgpu/features/texture-compression-bc/platform/Android ; Mali BCn https://docs.mesa3d.org/drivers/panfrost/texcomp.html
- gilrs force feedback: https://docs.rs/gilrs/latest/gilrs/ff/enum.BaseEffectType.html
- SDL3:
  - haptics: https://wiki.libsdl.org/SDL3/CategoryHaptic
  - Logitech wheel HIDAPI driver: https://wiki.libsdl.org/SDL3/SDL_HINT_JOYSTICK_HIDAPI_LG4FF
- Windows wheels:
  - Windows.Gaming.Input RacingWheel: https://learn.microsoft.com/en-us/uwp/api/Windows.Gaming.Input.RacingWheel
  - DirectInput in the `windows` crate: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Devices/HumanInterfaceDevice/struct.IDirectInput8W.html
- Linux: `evdev` crate https://docs.rs/evdev/latest/evdev/ ; [S] universal PIDFF driver https://www.phoronix.com/news/Linux-hid-universal-pidff
- Steam Deck: FAQ https://partner.steamgames.com/doc/steamdeck/faq ; recommendations https://partner.steamgames.com/doc/steamdeck/recommendations
- Apple notarization: https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution
- [S] CPU benchmarks: https://www.cpu-monkey.com/en/benchmark-intel_core_i7_13700k-geekbench_6_single_core ; https://wccftech.com/galaxy-s24-ultra-sporting-snapdragon-8-gen-3-stops-by-geekbench-6/
