# Verified AutoHotkey GRID macros

Recorded: 6 October 2026, Europe/Kiev. Menu navigation, a bound hotkey, throttle, combined throttle/steering and pause now work in the private reference copy.

## Tool and authorization

The owner approved setting up the researched macro approach. Reused the earlier bounded inventory, which found no installed AutoHotkey. Downloaded the [official v2.0.29 portable ZIP](https://github.com/AutoHotkey/AutoHotkey/releases/tag/v2.0.29) to ignored `local/tools/`; SHA-256 `b2d0200724a6b6ad22c965c939c5e5a2c64a35d1ccb455a3ca3f8ce415c5a296` matches the published digest. Running runtime reports 2.0.29. No system installer, driver, file association or global input configuration was used.

Independently authored source: `tools/grid_macros.ahk`; guarded launcher: `tools/run_grid_macro.ps1`. This is the owner-requested alternate macro program; Computer Use continues to provide target selection, screenshots and bound-key triggers. No direct PowerShell UI Automation was added.

## Controls and scope

8 October extension: F2 performs the same bounded sequence as F5, with combined throttle/right instead of throttle: resume for150ms, hold both scan codes for2,000ms, release, pause for150ms. The common `SampleDrive` keeps physical-stop and exact foreground-process checks. The idle old handler was identity-checked before replacement; no duplicate handlers. AutoHotkey2.0.29 logs a2,000ms hold, key releases and completion; direct screenshots confirm movement into the Donington barrier and the resulting Pause Menu. F5 is also verified after this refactor. Evidence `local/reference-race/steering-20261008/macros/20261008T1416196427122Z-serve.tsv`; wall-clock macro timing remains distinct from game-tick timing. [Live steering comparison](HANDLING_DAMAGE.md#live-selection-and-mutable-outputs-f130).

All bindings apply only when the verified private GRID process is foreground. The launcher checks PID, start time, executable path and SHA-256; the AutoHotkey script checks foreground PID/path before and during a pulse. Runtime mode for the verified experiments is `Event`, with explicit scan-code down/up events. It releases held keys on normal completion, focus loss, stop request and ordinary exit. Physical Escape cancels non-Escape pulses; F12 cancels the current pulse. Requested holds are bounded to 30–2000 ms. Hard process termination cannot guarantee cleanup.

| Hotkey | Macro |
|---|---|
| F1 | From paused race / Continue: Escape 150 ms, throttle + left 2000 ms, Escape 150 ms |
| F2 | From paused race / Continue: Escape 150 ms, throttle + right 2000 ms, Escape 150 ms |
| F3 / F4 | Left / right menu pulse, requested 150 ms |
| F5 | From paused race / Continue: Escape 150 ms, throttle 2000 ms, Escape 150 ms |
| F6 | Down, requested 150 ms |
| F7 | Up, requested 150 ms |
| F8 | Enter, requested 150 ms |
| F9 | Up/throttle, requested 500 ms |
| F10 | Down/brake, requested 300 ms |
| F11 | Escape/back/pause, requested 150 ms |
| F12 | Cancel current macro and release its keys |

The menu and race interpret the same arrow differently. Observe the current state before triggering any macro. F12 cancels an action; it does not terminate the resident runner. `throttle_left` and `throttle_right` are also available as named command actions. Their duration holds both keys simultaneously. Native Rust input helper remains untested and was not needed for these successful trials.

Named command examples, from the workspace:

```powershell
./tools/run_grid_macro.ps1 -Action inspect
./tools/run_grid_macro.ps1 -Action serve
./tools/run_grid_macro.ps1 -Action throttle -HoldMs 500
./tools/run_grid_macro.ps1 -Action throttle_right -HoldMs 1000
```

The server launcher refuses a duplicate resident handler. Evidence is retained under `local/macros/`, including individual stdout/stderr, TSV events and resident session identity. The resident created during verification is PID 35864, start `2026-10-06T01:27:47.7542367Z`; recheck identity rather than blindly using this PID on later cleanup.

## Actual results

9 October extension: F1 uses the existing guarded `SampleDrive` with throttle/left. The old idle handler was checked against its saved PID/start/path/command before replacement; the new handler targets GRID36228, with records under `local/reference-race/impact-20261009-hardware/macros-left/`. Its first sample logs a normal2,000ms combined hold and full key release; screenshots show the car turning across the track and pausing. ScriptSHA256cf248c0c9df8b61a86707ce612e89ada74224a1fbf7f6167ee22a348f63f5d0f. The earlier statement that left steering was unverified is historical.

Nested debugger stops during the following F5 sample disturb real-time pacing: its final Escape did not leave the reference paused. A fresh screenshot detected this, and explicitF11 restored the observed Pause Menu. Always inspect the final state; macro wall-clock duration is not a simulation-tick guarantee.

7 October continuation: F3/F4 bind 150 ms left/right pulses for menu selection. Both directions are verified; four left pulses traverse Okutama → Shibuya → Yokohama → Le Mans → Donington. Current runner/session IDs must be taken from `PROJECT_LOG.md` and the latest private server/session JSON. Prior IDs below are historical. Direct short Right did not visibly change discipline. Macro source SHA after adding bindings: `a66af2285db919e82088dd8cbbad12e7de37951a8b755d030f219c2d9dba1b32`.

New runner setup initially produced a false outer failure from checking unset native `$LASTEXITCODE` after a PowerShell script. The resident was actually ready. A later exact-identity stop guard rejected JSON datetime/string comparison; using UTC ticks fixed it. The duplicate-runner guard correctly refused replacement while the old idle runner still existed. Source edits alone do not update a loaded resident script. Every replacement verified path/start identity and completed key-up before stopping the owned runner.

Continuation at 09:18 UTC: sandbox process inventory could not see the still-running old server. Escalated PID/path/start-time checks confirmed it; F12 cancelled pending input, and the guarded launcher replaced it. New runner PID 23492, start `2026-10-06T09:15:10.3306689Z`, targets unchanged GRID PID 19680. Evidence: `local/macros/pre-sample-server.json` and `local/macros/20261006T0915102081230Z-serve.*`.

Restart Race was selected and confirmed through observed menus. A post-load HUD showed 0 mph, gear 1, lap 1/3, timer 00:03.88. Tool gaps before pausing mean this is not the exact timer at sample start. F5 was invoked only after observing Pause Menu / Continue. Logs record Escape 156 ms, throttle 2000 ms, Escape 156 ms, key-up for every pulse and normal completion. An intermediate HUD showed 14 mph, gear 1, 12/12, timer 00:33.12; the subsequent capture confirms pause again. This verifies the bounded sequence, not acceleration from rest or simulation-tick timing. Cancellation or focus loss midway can leave GRID unpaused; inspect afterward.

The original view has pit wall left and grandstands right. The raw-slot static showcase faces the opposite way; car-front geometry and progress-gate direction support further spawn-orientation investigation.

- Inspection parses/runs without injecting input. The first wrapper falsely treated an unset PowerShell `$LASTEXITCODE` for a GUI executable as failure; switching to explicit process waiting/exit-code retrieval fixed reporting. No game event was sent by that inspection.
- Direct Event-mode Up, requested 150 ms, observed 157 ms, moves Race Options from Go To Race to Driver Assists.
- Injected F6 triggers the resident macro: a 156 ms Down returns selection to Go To Race. This verifies both the bound trigger and the game response.
- F7/F8 open Driver Assists; F11 returns without changing its values. F6/F8 then start the reference race.
- F9 requests 500 ms throttle; log reports 500 ms. HUD changes from 0 mph before the experiment to 6 mph afterward.
- Direct combined throttle/right, requested 1000 ms, log reports 1000 ms; car visibly turns right and the following HUD shows 30 mph.
- F11 successfully opens Pause Menu after the driving trial. No full lap or race completion is claimed.

These are qualitative acceptance checks, not isolated acceleration/steering measurements. Time passes between screenshots and tool calls, and assists are enabled. Logged wall-clock holds are not exact simulation-tick durations. Brake and left-steering response have not yet been separately verified.

## Reference conditions captured

Race Options: Normal difficulty, Random grid position, 4 Flashbacks. Driving Assists: traction control On, transmission Automatic, braking assist On, stability control On. In-race HUD: lap 1/3 and 12-car field. Car/circuit names are owner-confirmed Lacetti/Donington; the new trace confirms `cars/lac` and Donington `route_0` reads. Screenshots still return 800x600, while saved settings request a different fullscreen resolution; capture dimensions are not automatically the actual display size.

The macro-driven fresh load was captured separately with Process Monitor. See [reference race](REFERENCE_RACE.md) for trace results and limitations.

Final process inventory confirms GRID PID 19680 and the resident macro runner PID 35864 retain their recorded identities; no Procmon process remains. The runner is left idle for continued use. Completed pulses logged key-up. Rebind the session and restart the runner after restarting GRID; its current bindings target this specific game PID.
