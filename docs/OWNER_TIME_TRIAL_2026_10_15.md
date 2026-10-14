# Owner three-lap event and original-reference interruption

15 October2026. Owner reports completing the new time trial. Private log
local/playtest/manual-6944-21324.log (source0cb4405fc491-dirty) reports three
valid laps:90.599838176,82.241714465,79.950105520seconds. Total event
254.983957083seconds, zero invalid attempts, finish and normal exit at15937ticks.
The records-saved event is present; actual private time-trial-records.tsv stores
best lap79950105520ns/event254983957083ns, matching this run. A later owner
relaunch/display of those bests has not been observed.

Existing summarize_driving_log.py passes1550samples/no failures/normal exit;
local/playtest/owner-time-trial-20261015-summary01.json. It records92 geometry-
contact ticks/357contact records,315deformation records/334moved positions,
maximum displacement0.6803687; these are not counts of crashes. All three laps
finish with damage active and no terminal state. No build change was made.

The owner authorized using the still-open original game. Private process51496
and guarded AHK22096 were revalidated. Computer Use successfully restored its
pause menu. Fresh player ownership check succeeds; readonly DT02 coast trace
local/driving/native-reference-20261015-02/coast01.tsv captures1953observations,
all repeated controls exact, all throttle/brake/steering inputs zero. The guarded
F14 batch logs its normal two-second sequence and final Escape key release.

However this is NOT an accepted bounded two-second coast: physical pose keeps
advancing until the end of the30s capture, speed15.560398712->6.082081536m/s.
The final pause input evidently did not stop physics. Active stored world/job
values include0.04s; sequential observations cannot establish game-time cadence.
Two screenshot requests, including one refreshed-window recovery, fail with
"FrameArrived timed out: timed out waiting on channel". Further input stopped.

The owner then reports original GRID crashed and requests relaunch. A read-only
process query finds no GRID process. Application events saved in
local/driving/native-reference-20261015-02/crash-events01.json only show an OLDER
Steam-original crash at01:03:42local/PID0x8A40/access violation c0000005/offset
00801796. This does not explain the current privatePID51496 exit; do not
misattribute it. The crash cause remains unknown.

Before relaunch navigation, Computer Use explicitly reported that the user
stopped it with physical Escape. No further Computer Use actions or relaunch
were performed. Preserve the stopped state. Next, if the owner resumes desktop
work, inspect fresh exit/crash evidence and relaunch the private compatible copy
through the supported UI workflow, with a new process/session/helper identity.
Current playable and Steam files were not edited; private traces remain ignored.
