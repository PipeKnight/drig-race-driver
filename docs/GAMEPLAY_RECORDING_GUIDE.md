# Gameplay recording guide for the owner

Prepared 9 October 2026, Europe/Kiev. Start with **Session 1 only**. We will review the short test clip before you spend time on longer recordings. The complete sequence below is a plan, not a claim that any recording or recorder compatibility has been verified.

## 1. Tools: what you need

You do not need to install development tools, debuggers, controller drivers or a keyboard-overlay plugin. Our existing research tools handle diagnostic captures; I will prepare those when required.

Checked on this PC on 9 October:

| Tool/hardware | Inventory result | What this means |
|---|---|---|
| Xbox Game Bar | Installed, `7.326.10010.0` | First option for a short test with no installation; GRID recording still untested |
| NVIDIA App | Installed, `11.0.9.251` | Another existing recording option, but overlay availability and GRID capture not tested |
| OBS Studio | Not found in PATH, standard system/user install locations, uninstall entries or the inspected research-tool directories | Optional recommended recorder for consistent settings; a portable copy elsewhere could still exist |
| NVIDIA GeForce RTX 4070 | Detected; driver `32.0.16.1692` | Use NVIDIA hardware H.264 encoding if OBS offers it |
| AutoHotkey | Existing portable `2.0.29`, verified GRID macros | No reinstall needed; leave macro triggers alone while driving manually |

**Recommended approach:** try the existing Game Bar for one test. If its video is usable, it is sufficient for initial visual references. Use OBS if Game Bar fails or if you want the consistent recording setup below. No new software was installed for this guide.

### No-install test with Game Bar

1. Open our working GRID copy: `C:\Users\debros\Tools\grid-research\grid-compat\grid.exe`. Use arrows and Enter in the menus.
2. With GRID active, press **Win+Alt+R** to start recording. A second press stops it. **Win+G** opens Game Bar; its Capture/Gallery controls can locate the recording. These controls are documented by [Microsoft](https://support.microsoft.com/en-gb/accessibility/windows/use-a-screen-reader-to-record-your-screen-with-xbox-game-bar).
3. Record about 30 seconds: show a menu, enter a loaded race, drive briefly, then pause. If the race is not loaded, a short menu clip first is fine.
4. Stop recording, open the saved video and check that the image moves, the HUD is readable and the engine is audible. Copy it into the session folder described below. Keep the original if copying fails.
5. If recording is unavailable, black, silent or stutters badly, use the OBS setup. Installed Game Bar is not proof that it captures this old game correctly.

## 2. Optional OBS setup

1. Download Windows OBS Studio from [the official OBS page](https://obsproject.com/download). It listed **32.2.2** when checked for this guide. Record the version you actually install in the session notes; later releases may differ. No extra plugins are required.
2. Open OBS. If the automatic setup wizard appears, select the option for recording rather than streaming. The wizard can also be opened from **Tools → Auto-Configuration Wizard**. See [OBS Quick Start](https://obsproject.com/kb/quick-start-guide).
3. Create a scene named `GRID reference`. Launch the private GRID copy above and return to OBS.
4. Under **Sources**, click **+ → Game Capture**. Name it `GRID`. Set **Mode → Capture specific window**, then select the `grid.exe` window. Leave the remaining source properties at their defaults initially. This is the supported OBS game-source workflow, not yet a proven GRID setup. [OBS Game Capture](https://obsproject.com/kb/game-capture-source).
5. Open **Settings → Video**. Match the canvas and output to the game's actual image dimensions. Our last verified game setting was **1280×1024**; recheck rather than assuming it is still current. Choose **60 FPS** initially. Keep the full image and original aspect ratio; do not stretch a 1280×1024 image into 1920×1080. Matching resolution and the FPS controls are described in [OBS Overview](https://obsproject.com/kb/obs-studio-overview).
6. Open **Settings → Output**, choose **Simple** mode and use these project starting settings:

| Setting | Starting choice |
|---|---|
| Recording Path | The session folder from section 3 |
| Recording Quality | High Quality, Medium File Size |
| Recording Format | MKV |
| Encoder | Hardware NVIDIA/NVENC H.264, if offered |

Hardware encoding reduces CPU encoding work; H.264 is our compatibility choice for these files. [OBS hardware encoding guidance](https://obsproject.com/kb/hardware-encoding). MKV is recommended for resilience to interrupted recordings; OBS can later make an MP4 through **File → Remux Recordings**. Keep the MKV original. [OBS recording output guidance](https://obsproject.com/kb/standard-recording-output-guide).

7. In the audio mixer, check that **Desktop Audio** responds to GRID's engine sound. Mute **Mic/Aux** initially. If Desktop Audio is silent, select the playback device used by the game in **Settings → Audio**. Do not add a second capture of the same sound. A microphone is optional for later commentary; written notes are sufficient. [OBS Quick Start](https://obsproject.com/kb/quick-start-guide).
8. Under **Settings → Hotkeys**, assign **Start Recording = Ctrl+Shift+9** and **Stop Recording = Ctrl+Shift+0**, provided those combinations are unused on your PC. Try them during the test. Avoid **F1–F12**, because our resident GRID macro runner uses them. OBS supports configurable recording hotkeys. [OBS Overview](https://obsproject.com/kb/obs-studio-overview).
9. Make the same 30-second test as above. Play the actual file, rather than relying on the preview. Fullscreen games can stop rendering when minimized, so an OBS preview while GRID is inactive can be misleading. [OBS troubleshooting](https://obsproject.com/kb/game-capture-troubleshooting).
10. If capture is black, confirm the selected window and repeat the short test with GRID active. If that fails, try **Window Capture** with the game windowed, or **Display Capture** as a fallback; show only the game during the clip. Record this change in notes. If OBS causes a crash, stop and report it; do not change the game executable or install compatibility patches for the recording test. These fallback methods follow [OBS troubleshooting](https://obsproject.com/kb/game-capture-troubleshooting), but GRID success remains to be tested.
11. If the recording causes stutter, try 30 FPS and note the change. Do not silently alter game graphics, assists or speed limits between comparison runs. A 60-FPS video does not prove a 60-Hz simulation.

## 3. Where to save recordings and notes

An initial private session folder has been prepared:

```text
C:\Users\debros\OneDrive\Документы\Reassembly\local\recordings\2026-10-09-session-01
```

Keep game video and diagnostic evidence under ignored `local/`, outside the public Git repository. No public upload is needed. For later sessions, make a new date/session folder; do not overwrite previous attempts. Preserve original clips without cutting, speed changes or recompression.

Use these names after stopping the recorder:

```text
00_capture_test.mkv
01_setup.mkv
02_baseline_lap_attempt01.mkv
03_accel_brake_attempt01.mkv
04_steer_release_attempt01.mkv
05_frontal_contact_attempt01.mkv
06_glancing_contact_attempt01.mkv
notes.md
```

Use `.mp4` if that is what your recorder produces. Only rename a file after recording has stopped. Repeat attempts use `attempt02`, `attempt03`, etc. The [notes template](templates/GAMEPLAY_SESSION_NOTES.md) is also copied into the initial folder. Unknown values can stay `unknown`; do not guess them.

## 4. Session 1 — test capture and document the setup

Allow roughly 10–15 minutes for setup; stop after this session until the video has been reviewed.

1. Complete the short recorder test and save it as `00_capture_test`.
2. Start a second clip. In GRID, show the car/track selection and options screens long enough to read each page, about five seconds each.
3. Use **Race Day → Touring Cars → Chevrolet Lacetti → Donington Park → Grand Prix Circuit** where those labels are available. This car/track/layout was observed in the earlier reference setup. Do not substitute a different layout silently; record the label you see.
4. Show/record the lap count, opponent count, difficulty, grid selection, damage options and Flashback count where available. Our earlier reference had three laps, twelve cars, Normal difficulty, Random grid and four Flashbacks. Values visible now take precedence over historical notes.
5. Show **Driver Assists**, transmission, keyboard/control bindings and graphics/display settings. Earlier assists were traction control On, braking assist On, stability control On and Automatic transmission. **Keep the current settings for the baseline** and record them; do not assume they match the old session. Show speed units, resolution, fullscreen/windowed state and VSync where available.
6. Load the race, select the normal chase camera, and pause. Stop the clip and name it `01_setup`. If changing the camera requires a binding you do not know, show Controls; do not try F-keys because they can invoke our macros.
7. In `notes.md`, record the recorder/version, video FPS/resolution, selected car/layout, input device and settings. Note any capture stutter or black frames.
8. Tell me the folder path and which files are ready. I will inspect the local files and decide whether to proceed. No need to upload them or drive further yet.

## 5. Session 2 — a visual baseline lap

Do this after the test video has been accepted. This records visual/audio behavior and your observations; it does not by itself measure precise input or simulation state.

1. Keep Session 1 settings and the same car/layout. Use the same input device throughout.
2. Start recording while paused. Show the initial condition, then restart the race through the normal menu. Keep the loading/countdown and HUD in the video.
3. Drive one complete lap at a comfortable pace with the normal chase camera and HUD visible. Aim to avoid contacts, but do not chase a fast time. There is no need to finish the entire race.
4. After crossing the line, pause and leave the timing visible for about five seconds. Stop recording and save as `02_baseline_lap_attempt01`.
5. Note any collision, Flashback, reset, AI obstruction, missed corner or unusual behavior, with the approximate **video timestamp**. Keep imperfect attempts; they can still be useful if labeled accurately.
6. Record a second lap attempt only if requested for comparison. Do not record every car or track yet.

## 6. Session 3 — coordinated short handling tests

Wait for my capture-ready message before these tests if we need numerical evidence. I will provide the exact starting location, reset procedure and capture identifier. I will not send macros or desktop input while you drive. Short sequences are easier to diagnose than a full lap.

For our verified keyboard setup, **Up = throttle, Down = brake, Left/Right = steering, Esc = pause**, and arrows/Enter navigate menus. Confirm these in your Controls page before testing; if you changed them, use and record your actual bindings.

For each attempt: start video while paused; show the starting state; perform one maneuver; pause; stop video; record the result in notes. Restart/reset between attempts as specified. Do not use Flashback to create a supposedly identical starting state.

### Test 3A: acceleration and braking

1. Position the undamaged Lacetti on the agreed clear straight, fully stopped and pointed straight. Use the same visible road marker each time.
2. Record the HUD at rest. On the agreed start, hold throttle until approximately **40 mph (64 km/h)**, then release throttle and brake to a stop. If traffic or the end of the straight interferes, stop the attempt and label it obstructed.
3. Pause and finish the clip `03_accel_brake_attempt01`. Note approximate throttle/brake transition timestamps and anything unexpected.
4. Repeat from the agreed reset twice more only if capture is working and the maneuver is unobstructed. Speed targets and human transitions are approximate; diagnostic inputs/state must supply precise evidence.

### Test 3B: steering and release

1. Start from the same agreed clear, paved area with fresh state. Reach approximately **20 mph (32 km/h)** going straight.
2. Release throttle, briefly hold the agreed steering direction for roughly one second, then release steering and allow the car to coast for roughly two seconds. Brake to a stop and pause. If space is insufficient, stop instead of forcing the maneuver.
3. Save `04_steer_release_attempt01`. Note steering direction, whether the car continued turning after release, and any correction you made. Human timing is approximate.
4. Repeat only on request; left/right comparisons need matched initial conditions.

Keep assists unchanged within a series. If we need an unassisted comparison, I will explicitly request a separate series with traction control, braking assist and stability control Off, preserving Automatic transmission initially. Record the changed options first and restore your baseline afterward. Do not mix assisted and unassisted results.

## 7. Session 4 — coordinated contact tests

These tests answer contact/damage questions. **Wait until I confirm the relevant diagnostic capture is armed.** Video of a crash alone cannot reveal internal contacts or validate the response routines.

### Test 4A: low-speed frontal contact

1. Use the agreed solid barrier and a fresh, undamaged car. I will specify the barrier with a visible landmark or reference image; do not select a different barrier between attempts.
2. Position the car approximately perpendicular to it, with room for a short approach. Begin video while paused and wait for capture-ready confirmation.
3. Approach at approximately **20 mph (32 km/h)**, releasing throttle just before contact. Allow one frontal hit; do not keep accelerating into the wall.
4. After about two seconds of game time, pause. Save `05_frontal_contact_attempt01` and note whether the contact was frontal/off-center, whether the car bounced/rotated and whether damage was visible.
5. Wait for capture validation before repeating. Higher-speed impacts are a separate later request, not needed initially.

### Test 4B: glancing contact

1. Reset to fresh state and use the same barrier unless I specify otherwise.
2. Aim for a shallow approach, roughly **20–30 degrees along the wall**, at approximately **20 mph (32 km/h)**. The angle is a driving target, not a measured value.
3. Make one glancing hit, release the controls, then pause after about two seconds. Save `06_glancing_contact_attempt01`.
4. Note which side hit, whether the car scraped or bounced, any spinning and visible damage. Wait for validation before another attempt.

If the debugger freezes/slows the game, release the controls and tell me; do not keep pressing inputs through the pause. I will handle the capture/recovery. Do not launch diagnostic scripts yourself or replace our private working executable.

## 8. What to report after each session

Send the local folder path, completed clip names and a brief note such as: “setup and baseline complete; one AI contact at 01:24 in the lap clip.” Mention problems and failed attempts as well as successful ones. I can hash and inspect files locally, correlate prepared diagnostic evidence, and update the research documents.

The most useful handling feedback is concrete: “it keeps turning briefly after I release Left,” “the rear breaks away under braking,” or “the steering response feels delayed.” Later, the same observations in our Rust build will guide original-versus-replacement comparisons. A perfect lap, professional equipment and public video sharing are unnecessary.
