# Reference race checkpoint

Recorded: 6 October 2026, Europe/Kiev.

7 October update: menu inspection directly identifies Donington Park's selected layout label **Grand Prix Circuit**, with Touring Cars / Chevrolet Lacetti / three laps. This supplements the earlier `route_0` load trace; this setup has not yet produced a new captured load. Current game/macro identities and debugger attempts are recorded in `PROJECT_LOG.md`; old process IDs below are historical. All reference-derived files remain private under `local/`.

Race loading, physical-key driving and agent-controlled timed macros are **verified**. The owner confirms Chevrolet Lacetti at Donington Park; a fresh load trace confirms reads from `cars/lac` and `tracks/europe/donington/route_0`. Earlier attempts below are historical checkpoints; the latest milestone is recorded at the end.

## Attempt and interruption

- Started the hash-verified legacy-codepage executable in `C:\Users\debros\Tools\grid-research\grid-compat`, PID 28952, at `2026-10-05T22:16:58Z` (01:16:58 local). Original Steam files untouched.
- Backed up the two hardware-settings XML files to `local/phase2/settings-before`, then changed only fullscreen to false for interactive research.
- Prepared Process Monitor with `/NoConnect`, so no race capture was started. No race dependency list is claimed.
- Applied the Computer Use skill and read its guidance, API and confirmation rules. The GRID capture initially reported a minimized window; activation/rebinding subsequently reported `no screenshot targets found`. These are automation failures, not a new diagnosed GRID compatibility defect.
- During the recovery attempt, Computer Use reported that the owner stopped it using the physical Escape key. Stopped app input immediately; no car/track menu navigation performed.
- Cleanup closes only the reference game/tracer processes owned by this attempt and restores backed-up settings. Exact restoration results are in `local/phase2/settings-restored.json`.

## Next reference experiment

After UI control is authorized again, verify a working game-window capture or use owner-guided selection. Record exact car, circuit/route, mode, assists, controller and display settings; capture relevant files while loading the race. Keep menu, race-loading, driving and restart as separate verified milestones.

## Resumed attempt

Owner authorized resumption and then explicitly handed GRID navigation to the agent. New private-copy PID 7700, start `2026-10-06T00:25:12.8096652Z`, executable SHA-256 `3aac37346531229daaec760d00ed0b6189c935be2356b97f039a680a795b9cda`. No temporary settings edit was made; snapshots and session identity are under `local/phase2-resume/`.

Screenshots work at 1280x720. The resolution-confirmation prompt was followed by Create Profile (PLAYER / ONE / REST OF WORLD). No profile/race completion is claimed. Injected Down and Return left the highlight on First Name; a click positioned the cursor over Confirm without selecting it. Earlier input-guard errors required refreshed observations. The owner was asked for a physical-key test/profile confirmation; game input failure is not established from automation results alone.

Process Monitor remains capture-disabled; its higher-integrity window has limited accessibility, and the injected filter shortcut showed no dialog. No race-load dependency map exists yet. The resource readers were developed from file inspection independently of this pending UI experiment; see [format specification](RESOURCE_FORMATS.md).

A later screenshot in the same session shows the Main Menu with GRID World selected; the cause of progression past Create Profile was not observed. This provides direct screenshot verification of the menu, without assuming that the injected keys caused the transition. An injected Down on the main menu again produced no immediate visible selection change. Race setup/loading/driving remains pending.

Checkpoint: owned GRID PID 7700 is left open for the pending physical-key check; Process Monitor PID 27464 remains capture-disabled. Existing XML settings were byte-identical to the backup at `2026-10-06T00:44:00Z`, so no restoration write was made. On later cleanup, match the saved process start time before closing an owned process, preserve any newly created user profile, and restore settings only after the game has closed if temporary edits require it.

## Persisted profile and loaded race

The private reference now uses manifest `uk-UA`, SHA-256 `b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6`. Save creation and restart are documented in [autosave investigation](AUTOSAVE_INPUT.md), with full private user backups. The owner confirms that the existing profile loaded after the guarded restart, then opened Race Day and launched the proposed configuration: Touring Cars / Chevrolet Lacetti / Donington Park / single race. This reports following the recipe; exact selected UI labels, route, laps, opponents, assists and difficulty have not yet been independently recorded.

Fresh Computer Use selection and activation now succeed. A direct 800x600 screenshot shows the Pause Menu, with Continue selected, over a rendered race scene containing the player's car, opponents, start straight and spectator stands. This verifies scene loading, without claiming driving response, race completion or exact route from scenery alone. Current process identity remains PID 19680, created `2026-10-06T01:04:50.0562006Z`, with the expected private executable path.

Requested a brief physical-key driving test and exact car/circuit confirmation from the owner. No injected driving action was attempted. Process Monitor remains capture-disabled; the race was already loaded before this observation, so there is no load-time dependency trace for this run. A fresh scene load is required for that capture.

Owner subsequently confirms: "Yes, it works good, I can drive the car" and confirms Chevrolet Lacetti at Donington Park exactly. Another captured frame shows the running scene with HUD speed 86 mph, third gear and elapsed timer 00:04.41. That frame arrived while targeting the occluded higher-integrity Process Monitor window; its GRID pixels are useful scene evidence, but do not represent the tracer's own UI or verify tracer state. No injected vehicle input or full lap was performed by the agent.

The owner was asked to retire and prepare the same race, stopping before launch for a fresh dependency capture, and to report any circuit layout label. Updated `tools/summarize_procmon.py` supports explicit source/output/PID/game-root arguments, refuses evidence replacement, retains only requested GRID PIDs, distinguishes opens from reads and hashes accessed game files. Python compilation and a synthetic five-event check verify PID filtering, read/open counts, hashes and overwrite rejection. This is preparation, not a completed trace.

Ended the sole owned capture-disabled tracer PID 27464 after checking its start time; native `/Terminate` returned 0 and subsequent process inventory contains no Procmon instance. This prepares a fresh bounded capture; no existing trace was deleted. Current metadata and settings snapshot are under `local/reference-race/`. Saved settings now request 1280x1024 fullscreen / 165 Hz / vsync off / medium detail / 2x MSAA / software audio / workerMap4Core. Actual presentation size and race-specific options remain separate observations; no settings were manually changed by the agent this turn.

Prepared `tools/start_race_capture.ps1`, validated with the PowerShell parser. Once the owner is stopped before race launch, run it with `local/autosave/persistence-restart/session.json`, a fresh evidence directory under `local/reference-race/`, and a 90-second bound. Check actual backing-file creation before telling the owner to launch. After the capture has ended, export its PML using the installed native Procmon CLI, then summarize with PID 19680 and the private game root. If the process changes, refresh verified identity first. The bounded raw trace includes other processes and must remain private. No capture has started at this checkpoint.

The owner then requested a program providing agent-controlled button durations. Built the [native timed-input experiment](TIMED_INPUT.md) with the existing Rust compiler. Before testing, Computer Use reported a physical Escape stop during window activation. All app input stopped; the native controller was not run and no capture started. The existing reference remains open; do not restore older user files over the confirmed profile.

## Macro-driven load and dependency capture

7 October fresh control: menus independently show Touring Cars / Chevrolet Lacetti / Donington Park / **Grand Prix Circuit**, three laps, Nevada Autosport livery. Race Options show Normal / Random grid / four Flashbacks. Session `local/reference-race/resume-20261007e/session.json` loads successfully without a debugger; HUD shows lap 1/3, twelve cars and 0 mph, followed by verified pause. No new file trace was taken, and the earlier route_0 trace remains a separate observation. Current dimensions are 1280x1024; prior dimensions below are historical. Assists were not separately rechecked in this run.

After the owner resumed and approved AutoHotkey setup, verified menu navigation, a bound trigger, throttle, combined throttle/right and pause. [Macro setup](MACRO_SETUP.md) records the input implementation and actual timing logs. This uses the same PID 19680 and Cyrillic-codepage executable; the native Rust input experiment remains untested.

Observed Race Options: Normal difficulty, Random grid position, 4 Flashbacks. Observed assists: traction control On, braking assist On, stability control On, Automatic transmission. The loaded HUD shows lap 1/3 and a 12-car field. The owner-selected Race Day / Touring Cars / Chevrolet Lacetti / Donington Park recipe is now backed by actual resource reads, including Lacetti high/low LOD resources and Donington route_0. Opponent BMW resources are also read. A specific human-readable circuit-layout label has not been observed. Do not translate route_0 into a named layout by assumption.

Started a fresh bounded Procmon trace at `2026-10-06T01:28:45.9899092Z`, with a 90-second runtime request. The agent triggered Go To Race at `01:29:18Z`, and observed the loaded scene afterward. Final PML: 1,373,278,476 bytes; its last write is `01:30:22.8622718Z`, so finalization extends beyond the requested interval. Procmon exited. Native export completed with exit 0 at `01:31:27.9344626Z`; CSV: 531,027,398 bytes.

Streaming analysis found 2,878,420 total events and 107,756 events for the verified GRID PID. All 130 reported installation resources have successful ReadFile events; opens and reads are counted separately. The [dependency map](REFERENCE_DEPENDENCIES.md) lists all 130 paths and counts. All 130 resource hashes match the corresponding original Steam files; the original executable also retains its baseline hash. Verification time: `01:35:14.981935Z`.

Evidence under ignored `local/reference-race/load-20261006-0128/`: capture request and settings snapshot, `race-load.private.pml`, `race-load-all.private.csv`, export request/result, `summary/procmon-summary.json`, `summary/grid-events.private.csv`, and `steam-resource-verification.json`. Raw all-process evidence remains private. Confirm exact filenames from the request metadata when replaying commands.

This is a fresh race load in a warm game process, following an earlier loaded race. Cached startup/shared resources may be absent. Container reads do not identify which internal entries were consumed. Read multiplicity does not prove a particular vehicle/object assignment. The trace is an observed dependency set, not a complete dependency graph or a decoding specification. No full lap, fidelity comparison or tick-accurate input test has been completed. GRID was paused after the driving trial; the macro runner is idle.
