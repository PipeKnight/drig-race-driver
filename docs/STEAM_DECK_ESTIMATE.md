# Steam Deck estimate from stand-ins on the desktop PC

There is no Steam Deck and no other test device. This estimate runs DRIG on the parts of the desktop PC that
bound the Deck's CPU and GPU, measures the costs per thread and on the GPU, and scales them to the Deck.
- Tools: `tools/deck_estimate.ps1` (runs, under the build lock) and `tools/deck_estimate.py` (checks, table,
  scaling, VERDICT).
- New diagnostic: `[diagnostics] camera_presses` (docs/SETTINGS.md) replays the phone bench's camera presses on
  the PC.
- Log: F-entry "Steam Deck estimate".

[O] marks observations (measured on this PC, read from the logs). [I] marks interpretations and model choices.

## Result in short

[I] Deck with the current source (9406cb12 plus the `camera_presses` diagnostic), the phone bench's race
(Donington, 12 cars, 1280 x 800):

| preset | average frame | fps | bound by | 60 Hz | 30 Hz |
|---|---|---|---|---|---|
| desktop defaults (native 800 rows, high shadows) | 15.6-28.2 ms | 35-64 | GPU, at the slow end also the render thread | plausible, not assured | holds on average |
| mobile lean preset (480 rows, low shadows) | 15.1-24.4 ms | 41-66 | the CPU (render thread, then main thread) | plausible, not assured | holds on average |

- The two presets end close together because the Deck's 4 Zen 2 cores, not its GPU, bound the lean preset.
  The lean preset buys GPU headroom (GPU 5.6-10.2 ms against 13.6-22.8 ms), not a higher average.
- [I] Recommendation for a Deck profile: the lean preset, or the desktop preset at 480-600 rows, with a 30 Hz
  or 40 Hz cap on the LCD Deck. A 60 Hz target needs the CPU side to shrink first: render-thread prepare and mesh
  phases, and main-thread non-sim time.
- [O] On the four E-cores almost every one-second window has one 50-80 ms frame (p99 75-81 ms). Whether the Deck
  shares this is the largest open question for smoothness (see "Spikes").

## Hardware

| | Steam Deck | Stand-in on this PC | Ratio used, Deck / stand-in |
|---|---|---|---|
| CPU | Zen 2, 4 cores / 8 threads, 2.4-3.5 GHz, 15 W APU budget shared with the GPU | the 4 E-cores of the i7-12700 (Gracemont, up to 3.6 GHz, no SMT), logical CPUs 16-19 [O] | CPU time x 0.85-1.3 [I] |
| GPU | RDNA 2, 8 CU at 1.0-1.6 GHz: 1.0-1.6 TFLOPS | Intel UHD Graphics 770, 32 EU at up to 1.5 GHz: about 0.8 TFLOPS (the model uses 0.79) | compute 1.29-2.08 |
| Memory | 16 GB LPDDR5 at 88 GB/s (OLED 102 GB/s), shared CPU/GPU | 2 x 16 GB DDR4-3200 (Kingston KF3200C20S4/16G), one DIMM per controller: dual channel, 51.2 GB/s, shared with the iGPU [O] | bandwidth 1.72-1.99 |
| Screen | 1280 x 800 at 60 Hz (OLED 90 Hz) | a 1280 x 800 window on the RTX 4070's 2560 x 1440, 165 Hz monitor [O] | - |
| Driver / OS | SteamOS, RADV (Vulkan) natively or through Proton | Windows 11, Intel Vulkan driver 32.0.101.7092, NVIDIA 621.01 [O] | GPU time / 1.0-1.3 in the Deck's favour [I] |

- [O] Topology from `GetSystemCpuSetInformation`: 20 logical CPUs in two efficiency classes.
  - Class 1 = logical 0-15: 8 P-cores with two threads each, mask FFFF.
  - Class 0 = logical 16-19: 4 E-cores, mask F0000.
  - The script derives both masks from this, not from an assumption. Every run records its process affinity,
    and the analyser checks it against the mask.
- [O] Adapter per process: `WGPU_BACKEND=vulkan` and `WGPU_ADAPTER_NAME` ("rtx 4070" or "uhd graphics 770",
  Bevy's adapter-by-name). Every log's `frame_split ... backend=Vulkan adapter="..." timestamp_query=true` line
  names the intended GPU. The default (non-DLSS) build was used and the Intel adapter opened with it.
- No system setting was touched: no power plan, BIOS, driver panel or Windows graphics preference.

## Method

- Race: the phone bench's (tools/android_bench.ps1).
  - Donington, Lacetti, 10 laps, 11 AI opponents from grid position 4.
  - AI difficulty 0.606, catch-up, choreographer and mistakes on. Autopilot on, force feedback off.
  - `--drive-ticks 30000`: about 480 s of race at the measured 62 race ticks per second.
  - The bundle's settings.ini, plus a 1280 x 800 window without vsync (`present_mode = auto_no_vsync`), the fps
    counter on (for `frame_stats` and the GPU timestamps), sound volumes 0, and the run muted
    (`local/mute_grid.ps1`).
- Camera: `camera_presses = 30,5`, the phone bench's touch presses. Every 30 s Change View (held 150 ms) and
  Look Back (held 5 s) alternate, Change View first. 15 presses per run, each logged as `camera_press` [O].
- Presets:
  - desktop defaults: native 800 rows, high 2048 sun shadows;
  - lean preset: `GRID_MOBILE_PRESET=1` with 480 rows pinned, as the phone bench pins them (768 x 480 render,
    low 512 shadows, compute threads).
  - A ninth run, P-cores + UHD with desktop defaults at 480 rows, splits the GPU time into fixed and per-pixel
    parts.
- Each run is one process with `start /affinity <mask>`. All nine ran under one hold of the build lock, so no
  cargo build overlapped. Other sessions' non-cargo processes and Windows background work were not stopped.
- Windows: `frame_stats` 1 s windows with race ticks, the first 3 dropped. Medians over 447-469 windows per run.
- Deadline: the runs used a 480 s deadline. That cut each race 11-41 s before the 30000th tick; 457-466 race
  seconds remain [O]. The script's default deadline is now 600 s.

## Measurements [O]

Median of 1 s windows. Columns:
- main = main-thread ms per frame; non-sim = main without the fixed-step simulation;
- render = render-thread busy ms (render_ms without present and acquire);
- GPU = timestamp-query ms; p99 = median of the windows' p99, and the worst window;
- gap = frame - max(main, render, GPU).

| configuration | fps | frame ms | main | non-sim | render | GPU | p99 (worst) | gap | ticks/frame |
|---|---|---|---|---|---|---|---|---|---|
| P-cores + RTX, desktop | 146.8 | 6.8 | 3.7 | 3.2 | 5.9 | 2.2 | 8.3 (12.4) | 0.9 | 0.43 |
| P-cores + RTX, lean | 111.3 | 9.0 | 5.4 | 4.5 | 7.7 | 1.7 | 10.9 (25.9) | 1.2 | 0.56 |
| E-cores + RTX, desktop | 60.7 | 16.5 | 10.9 | 8.3 | 14.5 | 5.2 | 81.1 (279) | 2.0 | 1.00 |
| E-cores + RTX, lean | 57.6 | 17.4 | 12.1 | 9.3 | 15.4 | 4.6 | 75.7 (309) | 2.0 | 1.05 |
| P-cores + UHD, desktop | 26.1 | 38.3 | 9.9 | 5.0 | 33.1 | 35.7 | 40.8 (146) | 2.6 | 2.40 |
| P-cores + UHD, desktop at 480 rows | 45.1 | 22.2 | 7.8 | 5.4 | 19.3 | 19.4 | 24.2 (64) | 2.7 | 1.39 |
| P-cores + UHD, lean | 55.8 | 17.9 | 7.9 | 6.1 | 16.9 | 14.9 | 20.0 (56) | 1.0 | 1.12 |
| E-cores + UHD, desktop | 25.0 | 40.0 | 18.6 | 10.1 | 36.6 | 35.2 | 51.2 (232) | 3.4 | 2.46 |
| E-cores + UHD, lean | 39.7 | 25.2 | 16.2 | 11.6 | 22.9 | 14.5 | 64.9 (503) | 2.2 | 1.51 |

- P-cores + UHD lean: 22 of 462 windows were unfocused (5 %), below the 10 % limit.
- The first P-cores + RTX desktop run was invalid. Its window was unfocused in every window and held exactly
  60.0 fps with 10 ms of idle gap; the other runs were focused. The analyser now rejects a run unfocused in more
  than 10 % of its windows, and that configuration was rerun after the others (the row above; 477 windows,
  the whole race, 16 camera presses). The cap on an unfocused window comes from outside
  the game (frame_setup reports `frame_cap=none`). A background frame limit in the NVIDIA driver settings is the
  likely source [I]. It was not inspected or changed.
- Render-thread phases (medians, ms), lean preset:

  | | meshes | prepare | bind | encode | submit |
  |---|---|---|---|---|---|
  | P-cores + RTX | 1.3 | 1.0 | 0.6 | 3.9 | 0.5 |
  | P-cores + UHD | 2.5 | 4.7 | 0.7 | 3.4 | 5.8 |
  | E-cores + RTX | 3.0 | 1.9 | 2.0 | 6.3 | 0.7 |
  | E-cores + UHD | 5.5 | 6.5 | 2.2 | 6.0 | 1.3 |

  Desktop defaults, P-cores + UHD: submit 19.6 ms (RTX desktop runs: 0.7 ms).

  On the UHD the submit phase holds the wait for the GPU: it follows the GPU time. Prepare (3-5 x) and meshes
  (about 2 x) cost more with the UHD than with the RTX on the same cores, at every GPU load.

## Cross-adapter presentation

- [O] With the UHD rendering, the image is shown on the RTX's monitor, so Windows copies each frame across
  adapters.
  - Frame gap (frame - max of the three) on the P-cores: desktop defaults 2.6 ms with the UHD against 0.9 ms
    with the RTX; lean preset 1.0 ms against 1.2 ms. Both presets present a 1280 x 800 swapchain, so the copy is
    the same size in both.
  - The present and acquire phases total 0.2 ms with the UHD against 0.4-0.5 ms with the RTX.
- [I] The copy has no consistent CPU-side cost: at most the 1.7 ms of the desktop pair, and none in the lean
  pair. GPU timestamps do not cover it. The model takes its gap from the E-core + RTX runs, which have no copy,
  and the Deck has none either.

## Model [I]

All ranges are best..worst. `tools/deck_estimate.py` holds the constants.

1. GPU, desktop preset.
   - The UHD at 800 and 480 rows: 35.7 and 19.4 ms. That is 10.3 ms fixed plus 24.8 ms per megapixel; at
     480 rows the per-pixel part is 47 %.
   - Deck GPU = (fixed / compute ratio + per-pixel / bandwidth ratio) / driver factor.
   - Best: compute 2.08 (1.6 GHz), LPDDR5 bandwidth 1.99 (OLED), RADV 1.3 times faster than Intel's Windows
     driver.
   - Worst: compute 1.29 (1.0 GHz, the GPU clock under a shared 15 W budget), bandwidth 1.72, driver 1.0.
   - Result: 13.6-22.8 ms.
2. GPU, lean preset: the UHD's 14.9 ms, split with the desktop-480 share (47 % per pixel), scaled the same way:
   5.6-10.2 ms.
3. CPU.
   - Best: the E-core + RTX run's main non-sim, per-tick sim and render busy times x 0.85 (Zen 2's IPC above
     Gracemont, SMT for the worker threads).
   - Worst: the larger of x 1.3 (the Deck's CPU clocks down under GPU load) and the E-core + UHD run's own costs.
     The latter has its submit wait replaced by the RTX run's submit time, and includes Intel's higher prepare
     and mesh costs on a shared-memory iGPU.
   - The sim part follows the Deck's ticks per frame (60 Hz sim, at most 4 per frame), solved as a fixed point.
4. Frame = max(main, render, GPU) + the E-core + RTX run's gap (2.0 ms).
5. p99 = frame x the E-core + UHD run's p99/frame ratio (1.28 desktop, 2.58 lean).

| Deck | GPU | main | render | frame | fps | p99 |
|---|---|---|---|---|---|---|
| desktop preset | 13.6-22.8 | 9.2-16.4 | 12.3-26.3 | 15.6-28.2 | 64-35 | 19.9-36.1 |
| lean preset | 5.6-10.2 | 10.1-16.9 | 13.1-22.4 | 15.1-24.4 | 66-41 | 38.8-62.8 |

## Spikes

- [O] On the E-cores 87-97 % of the 1 s windows have a p99 above 33 ms, and 54-76 % above 50 ms. The median
  window's slowest frame is 51-81 ms, and the median p99 is 2.6-4.9 times the average frame with the lean preset
  or the RTX. On the P-cores the lean runs have 0-3 % of windows above 33 ms, a median slowest frame of 11-20 ms
  and a p99 of 1.1-1.2 times the average frame (the UHD desktop run is above 33 ms on average, so its share means nothing). The
  p99 column above is therefore one long frame per second, not a spread of slow frames.
- [I] Two readings are open:
  - With 4 threads and no SMT, a periodic job (streaming, AI planning, a compute task the main thread waits for)
    blocks a frame.
  - Windows schedules other processes' low-priority work onto the E-cores, and the game's threads share them.
    The affinity confines the game, not the others.

  The Deck has 8 hardware threads, and SteamOS's gaming mode has little background load. Its p99 may well sit
  far below the lean preset's worst 62.8 ms. A per-frame trace on the E-cores would separate the two readings
  (next action).

## Caveats

- No SMT on the stand-in. The Deck's 8 threads help the compute and render worker threads, not the main and
  render threads themselves.
- Driver: Intel's Windows Vulkan driver is weaker than RADV. The GPU range allows RADV up to 30 % faster and no
  more. The higher prepare and mesh costs on the UHD may be Intel-specific, or common to shared-memory iGPUs.
- Linux/RADV against Windows: thread scheduling, timer resolution and present modes differ. Proton adds a
  translation layer for a Windows build; a native Linux build avoids it.
- TDP: the Deck shares 15 W between CPU and GPU. The worst case assumes the GPU at 1.0 GHz and the CPU at
  1.3 x the E-core time, but not both at their best at once. Sustained throttling below that is possible in a
  warm case.
- One run per configuration; no repeat runs were made to measure run-to-run noise here. F919 measured up to
  3.6 ms between two runs of one revision on the same cores.
- Only Donington with 12 cars, the phone bench's race. Larger fields and tracks with more draws will cost more.
- The OLED Deck's 90 Hz is out of reach on either preset.

## Reproduce

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\deck_estimate.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File tools\deck_estimate.ps1 -CheckOnly -OutDir local\deck-estimate\matrix1
```

The first runs all nine configurations: about 90 minutes under the build lock, with the 600 s deadline per run.
`-Configs` takes a subset, and `-SelfTest` runs the analyser selftest and the topology read without a race.
