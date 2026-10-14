# Finite Lacetti / Donington time trial — F276

This is an independently authored single-car event over the existing purchased
Donington route_0 start line and ordered sector gates. It does not reconstruct
original GRID race management, standings, penalties or career rules. Further
sound work is deferred at the owner's request; audio decoding and mixing remain
unchanged except silence while the event car is frozen.

## Playing

After the checked release refresh, use **Play GRID Time Trial.cmd** from the
workspace. It selects three laps with the locally prepared body contacts, body
impact events and deformation data. The two existing launchers retain free
practice. All launchers use target/release/grid-app.exe and private logs under
local/playtest/. The time-trial launcher requires the existing private CD01,
BP01 and DF01 files; no proprietary inputs are published in the repository.

Arrows/WASD drive, Space applies the handbrake, P pauses, R restarts the event,
M mutes engine sound and Escape exits. Watch the countdown and wait for GO.
After three valid laps, the car freezes and results show each lap, total event
time and saved personal bests. There are no opponents or original race results.

The CLI enables this optional mode with --drive --time-trial-laps N (1..99).
The launcher fixes N=3; ordinary --drive stays free practice.

## Rules and clocks

- A three-second countdown uses 188 unpaused fixed callbacks at the existing
  f32 0.016-second physics interval (3.008 seconds). No vehicle, contact, lap or
  event clock advances during it; held throttle becomes effective after GO.
- The event clock starts at GO. It includes the short drive to the first
  start-line crossing, as well as time spent on any invalid attempt. Individual
  laps retain the existing fractional crossing interpolation and ordered sector
  policy. Invalid attempts never satisfy the requested valid-lap count.
- P freezes both countdown and running time. R resets the authoritative vehicle,
  lap and event state and restores the countdown, preserving saved bests.
- Only successful physics ticks advance event time. A physics failure leaves the
  event clock unchanged. Event validation stages its own state before commit;
  if that validation fails after a successful physics tick, the app stops with
  R recovery. This is not a rollback of the already successful vehicle tick.
- At the final valid crossing, total time uses the fractional crossing time,
  rather than rounding up to the physics tick. Car presentation freezes at that
  tick's pose, potentially fractionally past the finish. P cannot resume a
  finished event; R starts a new one.
- Terminal damage retains the existing disabled-controls policy; R restarts.
  Damage never counts as a finish. No new damage behavior is introduced here.

## Local best records

local/playtest/time-trial-records.tsv is ignored and machine private. It contains
a strict versioned header for Lacetti, Donington route_0, assisted automatic
controls and authored rules1. Keys separate target lap count and wheel-only,
body-contact, impact-event or deformation mode. Minimum lap and event times are
stored in integer nanoseconds; the build tag describes the best event.

The file is bounded to 64 KiB / 396 keys. Save reloads and merges minima, writes
a new unique sibling, syncs it and replaces the previous file. Windows file
replacement/reload is exercised by the test. Malformed files are preserved;
load/save failures display best-times unavailable and do not pause driving.
This supports a single app writer, not concurrent sessions or cloud sync.
Future rule/handling changes may require a new namespace before comparing PBs.
Headless --render runs never read or save owner bests.

## Validation and limits

Pure state checks cover countdown, invalid attempt exclusion, fractional finish,
reset, missing/duplicate ticks and late malformed-event rollback. Storage checks
cover creation, Windows replacement/reload, minima, mode separation and preserving
malformed bytes. The first storage test used the system temporary directory and
hit sandbox PermissionDenied; it now uses an authored unique workspace-local
file and removes only that file. No security setting was changed.

A minimal Bevy app uses real purchased-source vehicle physics to check held-input
blocking, 188-callback countdown, pause, successful tick clocks, failed physics,
R recovery and frozen completion. Its short completion events are explicitly
authored; they do not establish a full physical lap. The separate diagnostic
below exercises real crossings during full source-physics laps.

The existing independent pure-pursuit diagnostic accepts an optional trial lap
count (1..9), keeps ordinary source physics/contact/damage/timing and stops only
at the requested valid finish. It never teleports the car or replays poses.
The three-lap release pilot completed 35,764 ticks, all three valid laps,
zero invalid attempts, zero body contacts, no terminal damage or failure and
maximum lateral route error 3.629247944 m. Times:

| Result | Seconds |
| --- | ---: |
| Lap 1 | 191.056381556 |
| Lap 2 | 189.487657193 |
| Lap 3 | 189.446805080 |
| Event, GO to final crossing | 572.213027077 |

Evidence: local/playtest/time-trial-20261015-release-pilot01.tsv. Debug comparison,
final release repeat and playable render are still pending at this draft stage.
This is an authored conservative diagnostic driver, not original GRID AI and
not calibrated handling equivalence. Existing collision/damage/reset checks are
required in the release update; the three-lap clean run alone tests no impacts.

The original private GRID copy launched through the observed Explorer entry
with Computer Use and reached the title/attract screen. The first sandboxed
process query could not see the external process; an approved identity-checked
query found PID51496/start2026-10-14T22:05:30.3394574Z, unchanged private executable
b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6.
Guarded AHK2.0.29 server22096 logged two normal 150ms confirm pulses, but the
observed screen did not advance to the profile/menu. No original in-race input
or braking capture is accepted from this attempt. Do not repeat failed driving
input or tune handling from it. Session/settings backup/macro evidence stays
under local/driving/native-reference-20261015-01/.

Reused Rust/Cargo1.99.0, Bevy0.19.1 and Python3.12.14; no installation, original
Steam modification or audio retuning. Full original/prototype input-and-braking
comparison remains open independently of the new event implementation.
