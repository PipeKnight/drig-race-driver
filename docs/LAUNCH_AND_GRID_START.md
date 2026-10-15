# Standing start, idle engine and grid hold — F280

15 October 2026. Owner report: original GRID readily produces a burnout at the race
start, the prototype did not. This records the cause, the fix and its limits.

## Frame delta (F279 follow-up)

Original physics task `5E7F30` receives a pointer to a double frame delta and passes it,
converted to f32, to the car manager (`A02B70`) and world scheduler (`A72930`). Paused
native world/job deltas (0.00605..0.00609 s) and the five-entry body delta ring
(0.00607639737..0.00608282071 s, reciprocals 164.4..164.6) match this PC's 165 Hz
display: the original uses a **variable per-frame delta**, then substeps the car at
`trunc(delta*1000)` (VEHICLE_UPDATE.md). The prototype's fixed 0.016 s step therefore
corresponds to the original at about 62.5 fps; both give ~1 ms vehicle substeps.
Per-frame stages (assists, delta ring, world solver) run at a different cadence, so
quantitative comparisons should match the original frame rate or use 60 Hz captures.

Static notes: task registration `87E2D0` queues 0x28-byte records (period 0 = every
frame); `87E250` registers a fixed-period variant, used once with 1/48 s (`CE7868`) for
owner slot +0x1050, phase 8. QPC helpers: `A3F3B0` ticks, `A3F3D0` ticks->seconds,
stopwatch `C45770/C457D0`, message stopwatch `A4C350/A4BEA0`. The frame dispatcher that
supplies the double to `5E7F30` was not located; a survey of `87EA50` reached OS version
detection, not the scheduler.

## Cause

`grid-launch-slip` (new, source-only, private TSV) records forward speed and per-wheel
surface speed `spin(+104) * radius(+E0)` from a standing start.

| Run | Observation |
| --- | --- |
| no input, zero start | engine speed stays **0** for the whole run |
| full throttle, zero start | engine climbs linearly ~219 rad/s² (starter path `low*delta`, clutch byte 1860) for ~1 s; 1.75 m/s at 0.96 s, 7.2 m/s at 1.6 s; front slip then peaks ~9 m/s |
| native stationary reference (F275 stationary-idle01, HUD 0 mph, no input) | engine angular speed **232.6573..232.6873** |
| native standing start (DONINGTON_RACE_VIDEO, GO ~00:51.4) | HUD 37 mph (16.5 m/s) in gear 1 at 00:53 |

Reset `A0C9F0` zeroes 1894/1F00. The reconstructed post-force idle governor holds
`idle(17A8)*1.01` = 232.687 once reached, but a zero engine only cranks when throttle
>0.5 sets clutch byte 1860. So the prototype sat on the grid with a dead engine and
spent the first second of every launch cranking it. The original idles on the grid.

## Fix

- `Runtime::lacetti` starts 1894/1F00 at `idle*1.01` (bits `3F8147AE`), the governor
  target. No-input runs now hold **232.687** throughout, the native maximum.
  Full throttle from rest: 8.2 m/s at 0.96 s, **16.58 m/s at 1.6 s**, front slip peak
  11.6 m/s near 1 s, gear 2 at ~2.3 s. This matches the video's 37 mph at ~1.6 s
  within its reading precision; not a calibrated trace comparison.
- Time trial countdown now ticks the car instead of freezing it. The car is held with
  the handbrake; reconstructed controls already engage the clutch while the handbrake is
  held (`1640 -> E34=1`), so throttle revs the engine freely (limiter ~908 rad/s within
  1 s) and releasing at GO dumps the clutch: front slip 18 m/s immediately, 16.7 m/s
  forward at 1.6 s. Hold displacement over the 3 s countdown: 0.000 m with and without
  throttle. Event clock and lap start times count from GO (`Trial` offsets runtime ticks).
- Engine sound is no longer muted during the countdown (it idles/revs); still muted
  after the finish.

**Authored, not recovered**: the handbrake hold is our grid-hold policy. The original
race-state field D234 (`!= 7` sets 226C "controls disabled") forces throttle 0 and full
brakes in reconstructed controls and zeroes sub-idle RPM, so it cannot be what lets the
player rev on the original grid; how the original holds cars during its countdown is
still unknown (D234 is written through a sub-object pointer; no direct store found).
Holding with the foot brake is wrong here: with zero throttle the keyboard preference
1972 selects reverse at rest (24 m reverse in 3 s).

## Verification

- Default tests 254 pass; asset-dependent ignored tests: grid-app 7/7, grid-tools 11/11
  (with GRID_GAME_ROOT and private CD01/BP01/DF01). fmt and all-target `-D warnings`
  Clippy pass.
- Test updates reflect the faster launch: handbrake-stop test powers 72 ticks (about the
  former ~10 m/s); keyboard test allows a bounded (<=300 tick) handbrake stop; the
  deformation-callback atomicity check moves from fixed tick 165 to the first tick >=165
  with pending geometry (now 217); countdown test expects held ticks, pose within 1 cm
  and GO-relative event time; engine sound test expects idle audibility after reset.
- Autonomous three-lap source-physics event (18 m/s cap): 43561 ticks, 3 valid laps
  231.947/231.752/231.747 s, 0 contacts, no failure; start line now crossed at tick 96
  (formerly 139).
- Offscreen release time trial, throttle held through countdown, 290 ticks: handbrake
  held through tick 188, GO, 62 km/h and event 0:01.632 at the end; image inspected.

Private evidence: `local/launch/` (TSVs, test logs, render and log).
