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

Evidence: local/playtest/time-trial-20261015-release-pilot01.tsv. The pilot, captured debug run and current release repeat are byte-identical: all
647,736 formatted trace bytes and 136,228 complete final car/physical bytes.
The debug run began before the extra duplicate-start/empty-sector event guards;
those guards are checked separately by the current debug library tests. No
claim is made about every intermediate world byte. Final report
local/playtest/time-trial-20261015-final-validation01.json SHA256 043993184aecd20893ad97a2a35db546935e589ed32fb3c768151ffb26971b2a.
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

## Playable refresh and regression evidence

Build 0cb4405fc491-dirty / source 0cb4405fc4916a46d12478adc45168972a2fc50c
built 2026-10-14T22:16:17.5649546Z; executable SHA256
5aa01a5f727a0adbc75956869ac4331ab6b35044a1412ad116cc24a6684591df. Build manifest:
local/playtest/build-20261014T2215520884549Z/build.json.
All GRID app code was committed; separate diagnostic changes and unrelated
owner settings notes account for the dirty tag at build time. Later commits
only adjust diagnostics, documentation and storage tests, not playable behavior.
Eleven purchased-source driving checks and seven Bevy checks pass, including
existing body/deformation/terminal-damage recovery and the new finite event.
Ordinary tests: 178 world +15 tools pass; fmt and all-target Clippy pass.
The storage test additionally creates its own ignored parent in a fresh checkout
and verifies improving an already saved best.

Two bounded time-trial collision captures finish normally at 600 physics ticks.
The steering .6 stress run has 330 bulk calls /295 records /334 moved positions,
no terminal state/failure. It is a separate scenario, not the old .25 baseline.
The matching .25 run has all310 logged physical/deformation records exactly
equal to bc90482, excluding wall time. Nine mesh updates include the unchanged
startup; first changed tick165, last175. The image is visually inspected:
car/track, event time, lap target, practice splits and controls are readable.
UI differs intentionally; no full-image equality claim.

Private comparison local/playtest/time-trial-20261015-no-feedback01.json SHA256 91789baef32f2929235af9c7db08be2b717d9e512dffa51d796296a0dd48a208;
matched PNG SHA256 d00c51cf0c71eb2e5c7747548a429b9fc2d2447c8bad1e08c0f9411299c16f8e; log SHA256
6fa7afebc9e38d61d6b8615853fb7ffaa2ee76bf303e620cc98574b099747664.
Headless captures did not save owner PBs. All130 baseline Steam resources and
original executable remain unchanged at2026-10-14T22:17:49.231176UTC:
local/scene/preservation-20261015-time-trial.json.

The release is ready for an owner three-lap run. A full rendered owner event,
actual PB persistence across owner launches and an original-game input/braking
comparison remain to observe. No such observations are inferred from tests.
