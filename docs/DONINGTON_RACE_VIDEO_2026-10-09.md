# Owner's three-lap Lacetti/Donington recording — 9 October 2026

## Assessment

The recording is suitable as a visual reference for the first playable engine milestone. It shows launch/menu flow, the selected car/layout, a standing start, three laps, traffic, off-track recovery, a barrier contact, a first-place finish and the fastest-lap presentation. **Lap two is the best available free-track baseline**, while laps one and three provide useful contact and recovery examples. There is no need to repeat this full race just to obtain a baseline video.

This is video analysis, not a synchronized vehicle telemetry capture or a Rust-versus-original physics comparison. The engine completion estimate is unchanged.

## File, tools and inspection method

- Owner file: `C:\Users\debros\Videos\NVIDIA\Race Driver Grid\Race Driver Grid 2026.10.09 - 22.31.31.10.DVR.mp4`.
- Size: **391,962,669 bytes**. SHA-256: `a2052492c8234e4ecc51022c0d30a4c44e4d4bb5b4452e88871f67c2d5b2c3d9`.
- Container video duration: **300.350656 seconds (5:00.351)**; **1280×1024**, AV1, approximately **60 FPS**; AAC stereo at **48 kHz**.
- Embedded creation time: `2026-10-09T19:31:32Z`, or **22:31:32 Europe/Kiev**. This is container metadata, not independently verified capture timing.
- Full decode succeeds for **18,021 video frames**. Presentation timestamps strictly increase, with median interval **16.711 ms**, minimum **14.022 ms**, maximum **19.411 ms**, and no interval above 50 ms. This does not prove unique rendered images, absence of game stutter or a 60-Hz simulation.
- Audio decodes with nonzero signal: normalized RMS **0.217807**, peak **1.020506**. The peak can include codec overshoot; these statistics do not establish clipping, perceived audio quality or the identity of sounds. No auditory review is claimed.
- Visual review uses a five-second overview across the entire file plus focused setup, corner/contact and finish samples: **170 frame exports including repeat timestamps**, not continuous visual inspection of every decoded frame.
- Existing inspected tool locations had no FFmpeg/ffprobe, PyAV or OpenCV decoder. Installed **PyAV 19.0.0** only under ignored `local/tools/video-python`, using bundled Python **3.12.14**. The official Windows wheel SHA-256 matches PyPI: `96e4c0981ec21cadfef4c0b840d04a5e3c3be821894aa6a7d7a8ee870ba6b727`. Sources: [official package metadata](https://pypi.org/pypi/av/19.0.0/json), [PyAV installation documentation](https://pyav.basswood.io/docs/stable/overview/installation.html).
- Runtime libraries: libavcodec **63.1.102**, libavformat **63.1.102**, libavutil **61.1.102**, libswscale **10.1.102**, libswresample **7.1.102**; the full list and wheel provenance are retained privately.

Independently authored inspection script: `tools/inspect_gameplay_video.py`. It hashes the source, decodes timestamped samples, generates contact sheets and optionally audits video timestamps/audio signal. It refuses output outside repository `local/` and refuses an existing output directory. The original video is never modified; four separate inspection runs produce the same source hash.

Validation: Python AST parses; actual-video sample/audit runs succeed; three negative checks reject public-directory output, an existing output directory and a zero sampling interval. Rejected checks create neither requested new directory. Relative report links resolve and Git whitespace checks pass. Engine tests were not rerun for this independent inspection/documentation change.

Private evidence under `local/recordings/donington-race-20261009/`:

- `overview/inspection.json`, source-size frames and `sheet-00.jpg` through `sheet-05.jpg`: 61 samples and full decode/audio audit.
- `setup-finish/inspection.json`: 26 samples.
- `events/inspection.json`: 69 samples around traffic, corners and recovery.
- `impact-finish-detail/inspection.json`: 14 finer samples.
- `settings-snapshot/`: current settings/profile copies, `snapshot.json` and `profile-hints.json`.

Wheel source/hash metadata: `local/tools/video-decoder/source.json`. Game imagery, video, settings and profile data remain private; none is committed to the public source repository.

## Confirmed setup and remaining uncertainty

| Item | Evidence / status |
|---|---|
| Mode, car | Menu at 00:37–00:39: Race Day, Touring Cars, Chevrolet Lacetti |
| Track/layout | Same menu: Donington Park, Grand Prix Circuit |
| Livery and distance | Nevada Autosport, three laps |
| Difficulty/grid | Options at 00:40: Normal, Random grid |
| Flashbacks available | Options show four; this is availability, not proof of use/non-use |
| Field/start | HUD shows twelve cars, starting position 5/12 and 0 mph |
| Presentation | Chase camera, lap/race timers, position, minimap, speed in mph and gear visible |
| Input | Owner confirms driving with keyboard only; controller remained connected and vibrated. This is owner-reported input/feedback, not a captured input stream |
| Assists/transmission | Owner subsequently confirms all assists On and Automatic transmission: traction control On, braking assist On, stability control On. This is owner confirmation, not a decoded profile or inspected assists-page result |
| Executable identity | No process path/hash is embedded in the inspected video metadata. Prior private-copy provenance remains separate; this recording alone does not identify the loaded binary |

### Saved settings checked after the owner's question

Read the actual Documents path `C:\Users\debros\OneDrive\Документы\Codemasters\GRID`. Current `hardwaresettings/hardware_settings_config.xml`, version **39**, specifies **1280×1024 fullscreen**, refresh request **165 Hz**, **VSync 0**, **medium** graphics, **2x MSAA**, software audio, postprocess blur enabled, vehicle/environment damage true, `input device_type="auto"`, and motion output disabled. Its last-write timestamp is **19:26:19.8361232Z**, before the container's creation timestamp. The output video independently confirms its own dimensions, not the game's actual refresh/presentation cadence or every graphics setting.

**No traction-control, braking-assist or stability-control values are present in that readable XML.** Fifteen `Autosave0` files were checked for plain ASCII assist/transmission labels without matches; no profile option layout has been decoded by this check. A string search does not establish where the options are stored or their values. All 19 current GRID user files copied for the private snapshot match their source hashes before/after copying; this establishes per-file copy stability, not an atomic snapshot or recording-time settings.

The connected controller and `device_type="auto"` make the owner's vibration report plausible, but do not reveal which device supplied internal commands. For a later controlled keyboard experiment, temporarily disconnecting the controller would remove an input-device variable. It is not necessary to redo this recording. A brief Driver Assists/Controls screen capture before the next prepared test is simpler than guessing binary profile fields.

The owner's subsequent confirmation resolves the recording's assist/transmission uncertainty: **all assists On, Automatic transmission**. Treat the footage as an assisted keyboard-driving baseline. Visible braking, stability and speed changes can include assistance effects; they do not isolate manual input or unassisted vehicle behavior. Match these settings for a later original-versus-Rust comparison, and keep any unassisted experiment as a separately labeled series.

## Race timing and result

All timestamps below refer to **elapsed video time**, not the yellow in-game race clock.

| Milestone | Video time | Observed result |
|---|---|---|
| Standing grid | 00:49–00:51 | 5/12, 0 mph, lap 1/3 |
| Race begins | About 00:51.4 | Race timer starts; first movement follows |
| Lead established | By 01:16 | HUD 1/12; nearby rival remains visible briefly |
| Lap one completed | About 02:16 | Lap time displayed **1:24.61** |
| Lap two completed | About 03:34 | Lap time and best displayed **1:17.89**; cumulative split **2:42.51** |
| Last visible racing HUD | 04:56.349 actual sample time | Race **4:04.90**, lap three **1:22.39**, position 1/12 |
| Finish presentation | 04:56.365 actual sample time | **FINISHED 1ST** |
| Fastest-lap presentation | 04:58.5–04:59.5 | **1:17.89**, with the game's “World Record” label |
| Replay | 05:00 | Replay camera and controls visible at the end |

Lap three is approximately **1:22.4**, and total race time approximately **4:04.9**, based on the last racing HUD; a complete post-race timing table is not shown. Lap one is **6.72 seconds** slower than lap two, but includes the standing start and traffic. Lap three's last displayed time is **4.50 seconds** slower than lap two; this is not a measurement of the wall impact's isolated cost. The “World Record” text is an observed game message, not a verified online/global record.

Selected racing clock readings track elapsed video time approximately one-for-one, with an offset around 51.4 seconds. This is useful for navigation, not a synchronized simulation-timestep measurement. Sampled review cannot rule out every brief Flashback or edit between reviewed frames.

## Reference events and their use

| Video window | Observation | Use for the replacement |
|---|---|---|
| 00:49–01:05 | Standing start, visible gear progression and first corner; 37 mph/gear 1 at 00:53, 81 mph/gear 3 at 00:55, 136 mph/gear 5 at 01:00 | HUD/gearing/launch presentation and rough sequence reference; not a calibrated acceleration test |
| 01:08–01:16 | Overtaking/close side-by-side traffic; cars overlap in projection near the left side at 01:10 and 01:16 | Candidate car-contact/AI-spacing examples; image overlap alone cannot prove exact contact geometry or impulse |
| 01:27–01:31 | Car leaves the paved line, crosses gravel/curb and rejoins; 116 mph at 01:27, 61 at 01:29, 57 at 01:30, 75 at 01:31 | Surface-transition visuals, dust/smoke and recovery; throttle/brake/assist contributions unknown |
| 02:08–02:13 | Large visible yaw and smoke at the final tight corner; 124 mph at 02:08, 54 at 02:10, 85 at 02:13 | Low-speed corner/body presentation; smoke does not by itself identify a tyre model or prove oversteer cause |
| 02:16–03:34 | Lap two: mostly clear track ahead in inspected frames, with kerb use and tight-corner smoke | Best broad scene/route/chase-camera baseline; not a certified incident-free or unassisted lap |
| 02:51–02:58 | Speed/gear sequence around a corner: 116 mph/4, 124/5, 129/5, then 107/4, 101/4, 100/4, recovering to 115/4 | Qualitative braking/turning/recovery benchmark with visible road landmarks |
| 03:05 | HUD **153 mph, gear 6** on the straight | At least this speed is observed; not an exhaustive maximum-speed measurement |
| 03:27–03:32 | Lap-two final tight corner: 78 mph/3 → 50/2 → 56/2 → 72/2 → 88/3 → 100/4 | Useful compact free-track corner sequence for later matched tests |
| 04:36–04:38.4 | Lap-three car runs beyond the corner onto the grass and reaches the blue barrier | Good visible approach/surface/contact reference |
| 04:38.200–04:38.400 | HUD **40 mph/gear 1**, then **2 mph/gear 1**, with car at the barrier and dust appearing | Strongest single wall-contact example in this recording; exact impact time/normal, inputs and physical deceleration unmeasured |
| 04:39–04:44 | Recovery: displayed 7, 13, 25, 52, 71 and 86 mph at successive one-second samples | Rejoining/drivability after impact; not an isolated drivetrain test |
| 04:56–05:00 | Last HUD, finished-first cinematic, visibly cracked windscreen, fastest-lap banner and replay | Finish/presentation/damage reference; do not attribute all final damage to one impact |

Manually read HUD values are rounded game displays at sampled timestamps. They are not a continuous telemetry trace. No friction coefficient, suspension constant, steering command, restitution value or AI rule is inferred from these numbers.

## What this changes in the work plan

1. Keep this recording as the accepted broad visual reference. NVIDIA-produced AV1/MP4 is usable with the local decoder; OBS is not required to replace it.
2. Use lap two for scene orientation, landmark order, chase-camera composition and later matched driving sequences. Keep the start/finish material as presentation references.
3. Use 04:38 as the concrete wall-contact scenario to reproduce in a prepared diagnostic session, capturing contact inputs/body state and downstream changes. The existing video cannot retroactively supply memory data.
4. Before the next numerical experiment, record Driver Assists/Controls briefly, establish initial/reset state and remove the connected-controller variable if testing keyboard input. Prepare a short maneuver/capture rather than asking for another whole race.
5. Continue the first integrated Rust driving loop. This video enriches the reference; it does not establish any new Rust physics equivalence or remove the timing/contact-production dependencies.

No additional owner driving is required for this analysis. Remaining open questions are original input commands, exact recording-time process identity, synchronized update timing and contact geometry. Assist/transmission values are resolved by owner confirmation; their storage in the profile remains undecoded.
