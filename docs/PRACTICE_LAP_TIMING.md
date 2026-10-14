# Practice lap and sector timing

F271 adds an independent practice timer to the source Lacetti/Donington driving prototype. The app displays current lap time, sector number, last lap and best lap, plus clear paused and terminal-damage states. The timer starts when the car crosses the start/finish line in the forward direction. It does not require desktop control or original-game telemetry.

## Course and policy

The purchased `tracks/europe/donington/route_0/progress_track.xml` has 71 ordered progress gates, 389 centerline points and source total distance 4003.84. Its forward route explicitly identifies finish gate **0** and timing splits **22/45**. Resource SHA-256 `61f3c443615b7593b625979bc107cb99249fd327930bf0476a2dd0286e29a9b4`.

The practice policy requires **0 -> 22 -> 45 -> 0**, in order. Gate planes, width and slope come from the purchased endpoints; forward orientation follows the next gate center. The car root must cross within the gate width and within 3 metres of its interpolated height. More than 50 metres of movement in one successful tick invalidates the active lap and cannot credit a crossing. These tolerances and the checkpoint policy are independently authored. They do not claim the original game's race rules, penalties, restart behavior or clock.

An earlier draft required all 71 progress markers. Replaying the owner's collision-heavy full lap revealed that gate 17 was crossed 4.708 metres above its source height. The source distinguishes progress markers from the timing splits, so the usable practice policy requires the explicit splits. A retained strict all-gate control still rejects that sampled lap at gate 17. This is a policy correction, not evidence that the owner's lap was invalid in GRID.

Crossing time is interpolated within the successful physics interval. The clock uses the existing stored-f32 0.016-second timestep, accumulated from successful tick count in f64. Wall-clock delays and P pause add no time. Reverse crossing and recrossing the finish cannot create extra laps. Returning to the finish before completing the splits records an invalid attempt and begins a new lap. Invalid attempts cannot improve the best time. **R resets the car, current timer, last lap and best lap together.**

## Transaction and presentation

`grid-world::lap_timing` returns staged state and ordered events. `source_vehicle::Runtime` runs it after the complete staged physical update and before committing the world, query cache, external collision geometry, deformation registration and counters. Late timing errors therefore preserve the entire tick's authoritative ownership boundary. Timing is optional for diagnostic callers; both playable launchers enable it.

The driving app reports `drive_lap_sample` and explicit start/checkpoint/sector/finish events in private logs. `tools/summarize_driving_log.py` reads those events rather than inferring laps from travelled distance. It also reads handling-effect/terminal-state telemetry. The on-screen practice display and existing title read state without feeding values back into vehicle physics. Bevy 0.19.1's cached `ui_bevy_render` feature provides the display and existing bundled font; no new tool or package download was needed.

## Verification

Five ordinary course tests cover ordered timing, fractional finish, all sectors, reverse/endpoint crossings, missed width/height, position jumps, invalid intervals, late counter overflow, and explicit required splits versus optional markers. The purchased-source test traverses the source centerline kinematically and repeats exactly; reverse traversal earns no lap. This is route validation, not a car driving test.

Paired actual source-car runs of 300 ticks retain exactly equal complete car/physical state, contact records and solver rows with timing enabled versus disabled. The timed run crosses the start once. A late invalid timing state preserves world, clock and cache. The complete physical-deformation test injects a timing failure at tick 166, after tick 165 made collision geometry dirty: owner, physical state, contacts, solver rows, query cache, deformation RNG/counters and actual queried geometry all stay unchanged. Repair then completes the existing 600-tick collision path and reset/replay exactly.

The Bevy keyboard tests cover timer retention across several paused updates and failure recovery, reset back to the waiting state, terminal-damage physics continuation and reset, in baseline/body-contact/deformation modes. The first expanded pause test mistakenly retained its synthetic just-pressed P across multiple updates because its minimal app has no input plugin. Clearing that synthetic input fixed the fixture; it was not a playable pause failure. Four nested-if Clippy findings were corrected without changing the gate arithmetic. All 206 ordinary library tests, five source-backed Bevy checks, formatting and warnings-denied all-target Clippy pass.

## Earlier owner lap as a sampled-path check

Private owner log `local/playtest/manual-15536-24160.log`, SHA `64bdf2377dfff3e1fd68af200506ac943c5b6c26d93d4438240c19aa5f289449`, contains 693 successful pose observations plus exit at tick 7424. The replay linearly interpolates between observations; the maximum gap is 16 ticks (about 0.256 seconds).

Debug, release and repeated runs all produce the same events and recognize one **105.43776314342081-second** practice lap, displayed as **1:45.438**. Sectors are **33.28723301342384 / 37.289685852220536 / 34.86084427777644 seconds**. Start crosses at tick 238, then splits at 2319/4649, finish at 6828. A separate Python decoder/plane oracle agrees on gate order and fractional crossing times within 2e-12 seconds. The replay audit reconstructs the interpolated input, checks complete output equality and current source/executable identities. Twenty malformed-frame/nonfinite/order/existing-output controls reject without overwriting output.

This validates the independent timer on the sampled earlier prototype path. It does **not** reproduce exact unlogged motion, resimulate that earlier car build, establish original GRID timing fidelity, or constitute a fresh owner lap in the new build.

Private evidence:

- `local/behavior/owner-lap-replay-{debug,release}-20261014-f271-final-02/validation.json`.
- `local/behavior/owner-lap-crossings-20261014-f271-final-01.json` and `owner-lap-replay-audit-20261014-f271-final-02.json`.
- `local/behavior/owner-lap-replay-all-gates-20261014-f271-control-02/validation.json`.
- `local/behavior/lap-replay-guards-20261014-f271-final-01/validation.json`.
- `local/behavior/lap-progress-source-release-20261014-f271-final-01.txt`, `lap-deformation-atomic-release-20261014-f271-final-01.txt`, library/Clippy and Bevy logs with the F271 prefix.

Next validate the clean playable display and replay the healthy-contact render to confirm no driving changes. Then exercise a complete physical lap with an independently authored diagnostic driver, retaining failures and uncertainties. Original audio, broader handling calibration, opponent cars and progression remain separate work.
