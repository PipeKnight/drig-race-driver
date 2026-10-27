# Race engineer speech: triggers, choice and queue (research)

27 October 2026, Claude Code (car-series, owner task: speech part 1 — race start, position calls,
finish). Static only: `grid_re.py` decompile/dis of `local/compatibility/grid-unpacked.exe`
(cache `local/re-cache`), the purchased `audio/eventSpeech.bin` and `audio/audioEventSpeech.bin`
decoded with the binary XML reader (private XML under `local/speech/`). GRID was not launched.
Sizes and folders: docs/AUDIO_RESEARCH.md §28.

The original's speech is data-driven. `audio_event_speech_manager.cpp` (vtable 0xCB786C) loads two
binary XML files and evaluates their rules every frame against about 400 named parameters that
`audio_game_info_collector.cpp` (vtable 0xCB7804, object at audio system +0x29450) fills from the
race. The port therefore needs three things: the file formats, the rule engine, and the
parameter values. This document gives all three for the Position and EndOfRace events.

## 1. Files [O]

**`audio/audioEventSpeech.bin`** (loader callback 0x45C5C0): `Speeches` →
`Category path=FOLDER` → `Speech name=NAME [long=FILE] [isPA=true]` → `Variation filename=FILE`.
The line is `speech/<lang>/<FOLDER>/<FILE>.raw`: headerless PCM16 mono.
- 18 categories and 907 speeches.
- No speech name is duplicated.

**`audio/eventSpeech.bin`** (parser 0x462FF0). The root `EventSpeech` holds the manager settings:

| Attribute | Value | Field |
|---|---|---|
| minimumSilence | 7.0 | +0xA4 |
| maxMsgInQueue | 5 | +0xA8 |
| reductionPercentage | 5 | +0xAC |
| delay | 0.0 | +0x610 |
| previousQueue | 5 (at least 1) | +0x614 |
| enabled | true | +0x60D |

Below the root:
- **Event:** `type`, `priority` (+0x2C), optional `isEventTrigger` (+0x30), `isExternalTriggered`
  (+0x31) and `delay` (+0x34); `Conditions`; then its groups.
- **Group:** `name`, optional `delay` (+0x2C); `Conditions`; then its messages.
- **Message** (record built by 0x45F080). Fields:

| Field | Offset | Notes |
|---|---|---|
| event, group | +4, +8 | |
| `msg` name | +0x10 | |
| speech | +0x34 | `speech` attribute looked up by name; null if absent |
| priority | +0x38 | unsigned; `priority=""` parses as 0 |
| queueValue | +0x3C | |
| repetitionThreshold | +0x40 | |
| long-played flag | +0x48 | set at load when the speech has no `long` |
| queued-at time | +0x4C | |
| last-played time | +0x50 | |
| nameToSay | +0x54 | neutral 0, positive 1, negative 2, namedAI 3, none 4 (default) |
| isEvent | +0x58 | |
| play counts (u16) | +0x5A, +0x5C, +0x62 | total, this race, this event |
| maxPlayCount, maxPlayCountInRace, maxPlayCountInEvent (u16) | +0x60, +0x5E, +0x64 | |
| linkedPhrasePercentage | +0x68 | |
| exclusive | +0x6C | |
| delay | +0x70 | |
| keepInQueueTime | +0x74 | when it exceeds queueValue, queueValue becomes keepInQueueTime + 1 |
| userData | +0x78 | |
| ignoreQueue, ignoreMinimumSilence, isFinalMessage | +0x7C, +0x7D, +0x7E | |
| conditions | +0x84 | |

- **Condition:** `param` (index), operator, `secondParam` (index), `value` (float).
  - Parameter names resolve through 0x44C510. An unknown name gives index 0 (`inRace`).
  - A missing `value` is −10000 (0xC61C3C00).
  - The operator names are parsed by 0x417090.
  - The comparisons are in the table 0xE5A3B0, filled by 0x45F080. Each is called as
    f(a = param, v = value, c = second param); for operators 0–5 the second argument is −10000:

| # | Name | True when | # | Name | True when |
|---|---|---|---|---|---|
| 0 | moreThan | a > v | 10 | moreThanParamDivValue | a ≥ c / v |
| 1 | lessThan | a < v | 11 | equalsParamPlusValue | a = c + v |
| 2 | equals | a = v | 12 | equalsParamMinusValue | a = c − v |
| 3 | notEquals | a ≠ v (NaN: true) | 13 | equalsWith | a = c |
| 4 | lessThanOrEqual | a ≤ v | 14 | notEqualsWith | a ≠ c |
| 5 | moreThanOrEqual | a ≥ v | 15 | moreThanParam | a > c |
| 6 | moreThanParamPlusValue | a ≥ c + v | 16 | lessThanParam | a < c |
| 7 | lessThanParamMinusValue | a ≤ c − v | 17 | moreThanOrEqualParam | a ≥ c |
| 8 | moreThanParamMinusValue | a ≥ c − v | 18 | lessThanOrEqualParam | a ≤ c |
| 9 | lessThanParamPlusValue | a ≤ c + v | | | |

The "Plus/Minus" comparisons are inclusive (`comiss` with `jb`). Ordered comparisons are false
when either side is NaN.

In the shipped data:
- Position and EndOfRace have 158 messages and 434 line files.
- 17 of those files are absent. Every `offPaceEnd` variation (8th–15th place) names
  `07_performance/notAchievedWin_2|3`, which exists only in `11_endofrace`. `h2h_lose` names
  `PU_Pit_Chief_14`.
- `pos4thGap3` plays `pos4thGap2`.
- `keyDriver17Ahead3s` tests key driver 16.

## 2. Evaluation (0x45EC00, from the manager task 0x45F050) [O]

The task waits on its event, then runs 0x45EC00 and the queue expiry 0x444FE0 [I: once per audio
frame; the signalling was not traced].

For each event that is not external (+0x31 = 0) whose conditions hold (0x4385C0), for each group
whose conditions hold (0x4384D0), for each message:
- events flagged `isEventTrigger` (+0x30) go to a separate external queue (+0x110). Only
  ReactiveCrowd has this flag; that queue is not part of this slice;
- otherwise the message is a candidate when the manager is enabled (+0x14) and:
  - (A) priority = 0, **or** nothing has been played yet (last +0x62C = 0), **or** all of:
    - it is not the last played message;
    - repetition: its race count = 0, or repetitionThreshold ≤ 0, or
      now − last-played ≥ repetitionThreshold;
    - `ignoreQueue`, or it is not among the last `previousQueue` played messages (0x4386B0,
      ring +0xD8);
    - the last played message is not `exclusive` from this same group;
    - no `exclusive` message of this group is loading (+0x634) or in that ring (0x438740);
  - and (B) the counts are under their limits: maxPlayCount (total), maxPlayCountInEvent,
    maxPlayCountInRace (0 = no limit).
- It is queued if:
  - `range(0, 100)` (0xA683E0, [0, 100)) ≤ the play percentage (0x417000); the percentage is 100
    for priority < 2, else max(100 − total count × reductionPercentage × priority,
    trunc(100 / priority));
  - it is not already queued (0x42FA30);
  - its own conditions hold (0x4383E0).
- When queued:
  - insertion 0x45EAE0. Priority 0 goes to the front. Otherwise the message goes before the
    first entry of a higher event priority number; within its own event, before the first later
    entry with a higher message priority number; else at the end. A duplicate (0x437E00) is
    not inserted;
  - queued-at = now;
  - evaluation stops when the queue holds `maxMsgInQueue` entries.

**Expiry 0x444FE0:** a queued message is dropped once now − queued-at > max(queueValue,
keepInQueueTime).

Lower numbers come first in both orders. In the data, EndOfRace (1) is ahead of Position (2).

## 3. Starting a line (0x44BE90 each frame, 0x445170, 0x437F70) [O]

Let F be the front of the queue, C the message being loaded (+0x634) and L the last played
message (+0x62C).
1. Nothing starts once L is an `isFinalMessage`. After the finish message the speech stays
   silent until the next reset.
2. **Preemption.** This applies while C is loaded (+0x60B), C ≠ F, C's priority ≠ 0, there is no
   load error (+0x1D5), and L exists and is not F.
   - If F's event priority number is greater than C's and F's priority ≠ 0: go to step 4.
   - Otherwise, if F's priority < C's, C is stopped (0x430350(0)) and removed from the queue.
3. **Urgent start.** If F's priority = 0, and C exists and is not F, and L exists and is not F:
   try F at once (step 5).
4. **Normal start.** Nothing may be pending (+0x60C) or loading (C). Unless F has
   `ignoreMinimumSilence` or `isFinalMessage`, now > silence base (+0x604) + minimumSilence is
   required. Then now > F's queued-at + keepInQueueTime is required.
5. **Try F (0x445170).**
   - A priority ≠ 0 message waits while the speech voice (+0x120) is playing.
   - Unless F is `isEvent`, the event, group and message conditions are checked again. If they
     fail, F is removed and its queued-at is reset to 0.
   - 0x437F70 then loads the line:
     - no speech: F is accepted silently;
     - the file is the speech's `long` file while the long-played flag (+0x48) is clear, else
       `variations[int(0, n)]` (0x46D930, 0xA68410, uniform);
     - if that path equals the one already in the line stream (the previous line), the load
       fails and F is dropped. The same file never plays twice in a row;
     - after a successful load, `int(0, 100)` ≤ linkedPhrasePercentage arms the player-name
       phrase of the message's nameToSay. The phrase is `Names_Player/man/Man_<name>`, chosen in
       the profile; 0x465690 skips the names when the profile has none.
   - A started `exclusive` message drops every other queued message of its group (0x445070).

## 4. Playing it (0x45E870 each frame, 0x45C510, 0x42FFC0) [O]

- The manager clock +0x600 advances by the frame dt.
- **Load result.**
  - Load error (+0x1D5): the message is stopped and removed.
  - When the stream is ready (+0x1D4), the message is committed (0x45C510) if it is `isEvent`
    or its conditions still hold; otherwise it is stopped and removed.
  - A message without speech commits at once.
- **Commit (0x45C510):**
  - a priority-0 message stops a playing line;
  - the message enters the played ring (`previousQueue` long);
  - L = it; pending (+0x60C) = 1;
  - its long-played flag is set;
  - its three counts increase;
  - last-played = now;
  - it leaves the queue.
- **Playback (0x42FFC0)** waits for now > L's queued-at + event delay + group delay + message
  delay + global delay, and for the voice to be free.
  - It plays the armed name phrase first, if any, on the next call.
  - It then plays the line on the voice (0x421C90 with 2 and 14, PA lines 15 [I: group 14 Codriver and 15 3DSpeech]) and sets
    silence base = now + 2.5 (0xCB092C; 2.0 after a named-AI phrase).
  - It switches the mix to **Talking** (mode 1) through 0x411D40, except for PA lines, a mode-5
    (Intro) state and a set +0x638.
  - 0x411D40 refuses the switch while the mode is Paused, PausedReplay or Transition. It
    remembers the mode it leaves, and 0x427ED0 sets that mode again when the voice stops.
- **Reset per race (re)start (0x454870 → 0x444E30(1) → 0x438840):**
  - the queues, the played ring, L, C, pending and the silence base are cleared;
  - the line stream name is cleared, so the first line may repeat the last one;
  - each message's race count and last-played time are reset. A message whose speech has a
    `long` file gets the long version again.
  - Total and event counts are kept. A new race world builds a new manager with every count at 0
    [I: the audio system is built with the race world at 0x67A281].

## 5. Parameters used by Position and EndOfRace [O unless marked]

Collector update 0x452810 (each audio frame).
- The race block (+0x60) is refreshed by the game interface's vf 0x40 (0x451A20). Once racing,
  it is skipped on frames whose counter is a multiple of 5.
- The player's record (+0x10C) is refreshed by vf 0x54 → 0x438C10 every frame.
- One more record is refreshed per frame, round robin on the frame counter mod 4:
  - 0: the car one place behind (+0x188);
  - 1: the car one place ahead (+0x204);
  - 2: the leader (+0x280);
  - 3: the cars ahead and behind on the track.

Race entry getters are those of docs/RACE_RULES_RESEARCH.md §1 (vtable 0xD0E898).

**Car record 0x438C10** (0x7C bytes, ints where marked):

| Offset | Value |
|---|---|
| +0 | time = race time (vf 12) / race progress (vf 18; 0 → 1e-6, RAY_TOLERANCE 0xCB22E4). An average lap time, not a gap |
| +4 | race progress (laps + lap fraction) |
| +0xC | speed, mph |
| +0x18 | lap fraction (vf 19) |
| +0x2C | time in current position (collector) |
| +0x38 | laps completed + 1 (int) |
| +0x50 | grid slot (car +0x2E4C) + 1 (int) |
| +0x54 | position + 1 (int) |
| +0x58 | previous position: the old +0x54, written only when the position changes |
| +0x70 | finished (status = 1) |

**Player and race parameters:**

| Parameter | Source |
|---|---|
| playerPos | player +0x54 |
| playerPrevPosition | player +0x58 |
| plBestPosInRace | minimum position this race (+0xB728, starts at 20) |
| playerTime | player +0 |
| carPosPlus1Time | +0 of the car one place behind |
| carPosMinus1Time | +0 of the car one place ahead |
| playerCarSpeed | mph |
| playerLapDist | lap fraction |
| playerLap | laps completed + 1 |
| playerFinished | status finished |
| plTimeInCurrentPos | + dt while the player's position equals the previous frame's, else 0 |
| plus1TimeInCurrentPos, minus1TimeInCurrentPos, leadTimeInCurrentPos | + dt while the same car holds that place, else 0 |
| raceTime | race clock (manager +0x28, starts at the green light) |
| inRace | race manager +0x10 (set at the green light, 0x90DE60), unless race over (+0x11), and the player has not finished |
| endRace | race over (+0x11) |
| firstCarFinished | leader record finished |
| numCarsInRace | entries − entries with controller type 3 |
| playerStartPosPercentage | (grid slot + 1) / entries × 100, see below |
| totalDam | 0x424650: 100 × mean of the six component levels (docs/MECHANICAL_DAMAGE.md) + the largest of four body-side percentages. Updated for the player only on collisions (0x424780) |
| playerCollision | set by a player collision whose impulse is at least half the physical mass (0x430F50: car +DB0 → +3F0 × 0.5, 0xCAF88C); cleared when an update changes totalDam by less than 1 |
| noCollisionTime | + dt while playerCollision is clear, else 0 |
| isNormalRace | not drift, Le Mans, touge or Demolition Derby |
| isNetRace, isTestDrive, isHeadToHead, inIntroRace, isTouge, isLeMans, isDriftBattle, isDestructionDerby | race kind flags (0x451A20) |
| isFinalRace, isFinalCareerEvent, wonChampionship, numRaceWins, numEventWins, racesCurTrack | career and profile values (0x451A20) |
| carPosMinus1KeyDriver, leadingCarKeyDriver, team parameters | named rival drivers and teams |
| carAhead*, carBehind* | the cars ahead and behind on the track (lapping lines) |

**Integer division [O].** At 0x452A42 the start percentage is computed as
`div dword [+0xAC]` on the integer grid slot + 1 before `fild` and × 100. It is therefore 0 for
every grid slot but the last, which gives 100. Of the `startingGridPos` messages only
`playerGridPosLast` can ever play (`playerStartPosPercentage` = 100). The `hardcharger` and
`goodstart` tests (> 75, > 25) also reduce to "started last".

**Grid lines [O, static].** `onStartingGrid` (+0xCE) is written in only three places:
- 0x468A60 and 0x451A20 clear it (the latter while racing);
- 0x452520(p) sets it to p only while the collector's in-race flag is set.

0x452520(1) runs from every race set-up 0x65B2C0 → 0x454870, the first one and each restart. The
same call sets `startAfterRestart` (+0xB6FC) to 1. Besides the constructor, only the collector's
load and unload handlers (0x4391E0, 0x455900, §8) clear it; a restart does not reload the audio
subsystems [I], so after a restart from a race in progress the flag stays 1. So:
- `startingGridPos` (`playerGridPosLast`) plays only after a restart from a race in progress;
- `startingGridGen` (`playerGridNewbie`, `playerGrid2`, `playerGridvet`) needs `onStartingGrid` = 1
  with `startAfterRestart` = 0, which no path produces. It is unreachable.

## 6. What the three moments need

- **Race start:** `startingGridPos` (`playerGridPosLast`) on a restart from the last grid slot.
  `startingGridGen` is dead (§5).
- **Position calls (Position event):**
  - `inRacePos1st`–`inRacePos4th`: in a place, with gaps of average lap time;
  - `inRacePos`: a new best position: 4th–11th, last;
  - `inRacePlus1`: a car about to be lapped, a lapped car behind;
  - `inRaceCarAheadLess3s` and `inRaceCarFirst`: named rivals, which need key drivers.
- **Finish (EndOfRace event):**
  - `head2head`; `raceEndFirst` (`firstWinEver`, `finalEventWin`, `win`, `winNoDamage`);
  - `raceEndOther`: 2nd–15th, championship and margin lines;
  - `endRaceWinningTeam` and `endRaceTeamCongrat`: teams.
  The lines are `isFinalMessage`, so the first one played ends the speech.

## 7. The port (part 1)

`grid_tools::speech` holds the files, the manager and the collector; grid-app's `speech_sound`
plays the lines.
- Only the Position and EndOfRace events are evaluated (`SLICE`, the scope of this part); the
  others are parsed and skipped. Their messages' parameters all resolve (`unknown_params` is
  empty for the purchased file).
- The manager runs once per vehicle tick (16 ms) [I: the original once per audio frame].
- Parameters the port does not have keep 0, except `numRaceWins` = 1 [AUTHORED], so
  `firstWinEver` does not play at every win. Key drivers, teams, the cars ahead and behind on the
  track, career and profile values are 0. So `inRaceCarAheadLess3s`, `inRaceCarFirst`, the team
  lines and the lapping lines (`inRacePlus1`) do not play.
- The generator is MT19937 (`series::Mt`) with an authored seed (5489); the line choice uses
  int(0, n) as 0xA68410, and the name-phrase roll is drawn but nothing is armed (no profile name).
- The line stream is ready at once and a missing file is a load error [I: the original loads
  asynchronously]. The voice counts as busy for the line's length at 32 kHz from its start
  [I: the device's voice state].
- The voice plays on Codriver (14), PA lines on 3DSpeech (15), with the Speech Volume trim
  (slider kind 2). While a line plays the mix is in Talking (`ModeMix::set_tied`, 0x411D40);
  when it ends the remembered mode returns (`release`, 427ED0's +C28 check).
- Pause (the port's pause menu) is the Paused case of 0x45E870: the queue is cleared and the
  line stops.
- Restart Race calls `Collector::restart` (0x452520(1)) and `Manager::reset_race`
  (0x444E30(1)).
- Speech runs only in races of the shared world (with opponents) [AUTHORED: solo drives are
  test drives and time trials].
- `grid-speech-render` (crates/grid-probes) renders an authored 12-car race through the
  purchased rules and lines to a private WAV.

## 8. Part 2: how the parameters are refreshed [O]

The collector (vtable 0xCB7804) has three interfaces:
- +0: vf1 0x452810, the update with the frame time (§5, §9);
- +4 (vtable 0xCB77F0): the audio subsystems' load and unload calls (base 0xCB0DF4):
  - 0x4391E0, load. On phase 2 it registers the collision callback 0x430F50 on every car
    (0x996150) and the part callback 0x417D60 on cars with controller type 0 (the player). It also
    sets registered (+0xB41C) and enabled (+0x18).
  - Then, for every phase, it sets the frame counter +0xB410 = 0, +0xB6E4 = 1 and
    `startAfterRestart` = 0, and clears the race block (0x468B30).
  - 0x455900, unload. On phase 2 it removes the callbacks. For every phase it runs 0x452520(1) and
    clears `aiSpin`, `aiRoll`, `startAfterRestart`, +0xB6FD and `usedFlashback`.
- +0x1C (vtable 0xCB77D8): the task 0x44C640, once per task frame unless disabled (+0xB740):
  - 0x44C3C0 calls every registered getter (0x45F590) and stores its value in the value array
    that the rule conditions read (§2);
  - 0x417ED0 then clears the one-shot event flags: `carToCarCollision`, `hitAnotherCar`,
    `playerCollideWithTeammate`, +0xB43E, +0xB43F, `playerCollWithKeyPlayer` (= −1), +0xB700,
    `coolFinish`, `newPersonalBest`, `newWorldRecord`, `carsPlayerOverTook`,
    `playerMovedTrackPos`, `aiSpin` and `aiRoll`.

So an event flag set by a callback is seen by exactly one refresh. The conditions read the stored
array, so a getter runs once per refresh. One getter has a side effect:
- `playerCrossedSFLine` (0x418460). If the last seen lap (+0xB3F4, 1 after 0x452520(1)) is below
  `playerLap`, it stores the lap and returns 1. It also moves the damage taken this lap (+0xB450)
  to `damageTakenInPrevLap` (+0xB44C) and clears +0xB450. Otherwise it returns 0.

The getters live in a std::map keyed by the name string's address (0x456B80 inserts, 0x7B0CF0
compares the keys unsigned), and 0x44C3C0 walks it in order: ascending string address.
`damageTakenInPrevLap` (string 0xCB6FCC) is therefore read before `playerCrossedSFLine`
(0xCB7004) moves the lap's damage. At a crossing the refresh still reports the lap before; the
value of the lap just completed appears on the next refresh, when `playerCrossedSFLine` is 0
again. The lines that test both on the same refresh (`cleanLap`, `notCleanLapOffPace`,
`offPace3s`, `aheadPace2s`) judge the lap before the one just completed [O static].

While the collector is disabled (+0x18 clear, terminal damage, §9) the update stops, so the
records and timers freeze (`terminalDam` stays 1). On its first disabled frame it also runs
0x44C470, which writes −10000 into every value; the task's refresh is not gated by +0x18 and
overwrites them [I: the task runs after the update in a frame]. In the Paused or Intro mix modes, or without a player car, the update runs
0x417ED0 and 0x417E40 (which clears the records) once and returns.

## 9. Part 2: player parameters [O unless marked]

**The update 0x452810**, every frame once racing unless noted:
- `carsPlayerOverTook` (+0xB418). Out of the race, the reference place +0xB40C is the player's
  place. In the race, when the place is better than the reference, the count is the difference.
  The reference moves to the place only once the player has gained two or more. Every 60th frame
  (0x4692B0: counter % 60 == 0) the reference is set to the place.
- `playerMovedTrackPos` (+0xB424) is set when record +0x34 (vf 0x80 of the car) exceeds the
  reference +0xB414. The reference follows every 60th frame.
- `satOnTrackTime` (+0xB3F0): + dt while racing and the race progress grew by less than 1e-5
  this frame (0xCB46B4), else 0.
- `slipStreamTime` (+0xB3FC): + dt while the vehicle's air density (+0x21C4, 0x98DFE0) is at most
  0.6 (0xCB0B74), else 0. The density is written only at construction
  (docs/AI_MISTAKES_PACING_RESEARCH.md §2), so this timer never runs and the slipstream line is
  dead.
- `wrongWayTime` (+0xB400): + dt while the race interface's wrong-way flag (+0x18) is set.
- `noCollisionTime` (+0xB454): + dt while `playerCollision` is clear (part 1).
- `timeOffTrack` (+0xB430): + dt while `droveOffTrack` is set, else 0.
- `tmTimeInCurrentPos` (+0x420): the player's timer, for the teammate's place.
- Every second frame the collector copies the player's damage object (vehicle +0x22D0 → own
  +0x470, 0x9EF020), so `engDam` and the other damage values lag by up to one frame.
- On terminal damage (record +0x74) the collector disables itself (+0x18 = 0).

**The car record (0x438C10)** adds these fields to part 1's:
- +8 `playerPrevLapTime`: the last completed lap's time (vf 0x5C), or the record time while
  fewer than two laps are counted;
- +0x10 `maxPlayerCarSpeed`: the highest mph so far;
- +0x3C the driver's `audio_key_driver_number` (database `driver`), cached per entry (+0x50/+0x54);
- +0x40 a championship value (0x670470) [I]; +0x44 the team id (`team.id`);
- +0x71 `droveOffTrack`, player car only: the result of the car's AI driver test of type 11
  (`on_track`, driver +0x2C64 → +0x2C) is 0;
- +0x74 `terminalDam`: the damage status (+0x444) is not 7 (0x99A720);
- +0x75 racing (status 0).

**The race block (0x451A20)** adds:
- +0x68 `currentPaceTime`: the mean of race time / race progress over cars that:
  - have a controller type other than 0;
  - are not terminally damaged;
  - have progress > 0;
  - are faster than 10 mph;
- +0x80 `currentTrackLaps`: the race's lap count;
- +0x84 `numRunningCars`: the cars of `currentPaceTime` without the speed and progress
  conditions, plus 1;
- career and mode flags [O, career]: `isTestDrive`, `isNetRace`, `inIntroRace`, `isHeadToHead`,
  `isFinalRace`, and the objectives `bonusObjWinPos` (+0x88), `hasNoDamageObj` (+0xDC) and
  `anyDamageIndOn` (+0xE1).

**Damage getters** read the copied damage object (component levels at +0, wheels at +0x18):
- `engDam` = component 3 × 100, `radiatorDam` = component 2 × 100, `gearboxDam` = component 5 × 100.
- `frontRightTyreDam`, `frontLeftTyreDam`, `rearRightTyreDam` and `rearLeftTyreDam` = wheel 3, 2,
  1 and 0 × 100. `frontTyreDamage` = (wheel 3 + wheel 2) × 50, `rearTyreDam` = (wheel 1 + wheel 0)
  × 50, and `tyreDamage` = the sum of the four × 25.
- `frontDam`, `rearDam`, `rightDam` and `leftDam` (+0xB70C…+0xB718) are the four body-side terms
  of 0x424650 (part 1) × 100. `bodyDam` (+0xB708) is the largest of them. 0x424780 calls 0x424650
  on every frame of the player's slot, so all five follow the body on those frames, collision or
  not; `totalDam` follows only after a player collision (part 1).
- `damLastAccident` (+0xB440) = |ΔtotalDam| of that update. The same amount is added to the lap's
  damage (+0xB450, §8).
- +0xB42C is set when the car's up axis points down at that update [I: 0xA69630 transform].

**Part losses (0x417D60).** The vehicle's detach callback (+0x7BFC) is called from the timed
detach 0x9FF4F0 with the part id. It sets:

| Part id | Flag |
|---|---|
| 10 | `looseBonnet` |
| 12–17 | `lossBumper` |
| 22–25 | `lossDoor` |
| 84 | `lossRearWing` |
| 85 | `lossRightWing` |
| 86 | `lossLeftWing` |
| 87 | `lossFrontWing` |

The flags stay set until 0x452520.

**Messages** (0x44C670 subscribes the collector):
- `BroadcastMessage_NewPersonalBestMessage` and `…NewWorldRecordMessage` set `newPersonalBest`
  and `newWorldRecord` (one-shot, §8).
- `RaceResults` (0x41BC70 → 0x417C10): a car whose result status is not 7 gets its AI terminal
  flag. For a key driver it also sets the key driver's flag (+0xB53F + n) and counts
  `keyDriversOut` (+0xB6E8) up, or down when the status is 7.
- `TerminalDamageMessage` (0x4457F0 → 0x417CB0): §10.

**Other values.**
- `playerGoodStart` = 1 when the vehicle's +0x2204 byte is 0 [I: the low-speed flag of
  docs/SUSPENSION_GRIP_PIPELINE.md].
- `playerCarRevRatio` = revs (+0x1894) / limiter (+0x17A4).
- `isPlayerCarOpenCockpit` (+0xB72C), `playerVehicleID` (+0xB720 = the player's entry + 1) and
  `teammateVehicleID` are taken before the race.
- `AITermUserImpact` (+0xB448) is only ever cleared (0x4310EB, 0x465DB6), so it is always 0.
- `noRestartOptionEnabled` is the race interface's vf 0x64. `terminalDamageCount` (+0xA0) is a
  profile count [I].
- `usedFlashback` (+0xB72D) is set by a flashback (0x454910, 0x453000).

## 10. Part 2: collisions, AI slots and key drivers [O]

**Slots.** `AIn…` parameters index the race entry n − 1 (vf 0x1C order). `playerVehicleID` is the
player's entry + 1, so each `AI_0n_…` message excludes the player's own slot. Key drivers are
numbered by `driver.audio_key_driver_number` (1–24); `KeynX` reads key driver n.

**The collision callback 0x430F50** is registered per car and gets the car hit, the other object
and the impulse.
- It returns unless the impulse is at least 0.5 × the car's physical mass, the collector is
  enabled and a player record exists.
- `carToCarCollision` = the other object is a car.
- If the other object is a car with a key driver n: +0xB6EC = n and `KeynCollision` = 1.
- If the car hit is the player's:
  - `playerCollision` = 1;
  - against a car, `hitAnotherCar` = 1;
  - against the teammate (not terminally damaged), `playerCollideWithTeammate` = 1;
  - against a key driver, `playerCollWithKeyPlayer` = n.
- If the car hit is the teammate's:
  - `teammateCollision` = 1;
  - against a car, +0xB43E = 1;
  - against the player, +0xB43F = 1.
- Otherwise (an AI car): its own key driver number sets +0xB6EC and `KeynCollision`, and +0xB730
  = the other car's driver id.
- Finally, if the car hit's entry index i is above 0, `AI(i+1)Collision` is set.

**Per-car update 0x431200.** It handles one entry per frame (counter % entries) and skips entries
with controller type 3:
- `AInDistance` (+0xB55C + 4i) = the entry's lap fraction (vf 0x3C → vf 0x4C);
- `AInTermDam` (+0xB660 + i) = its damage status is not 7. It also keeps the count of newly
  wrecked cars (+0xB458) and the last wrecked driver (+0xB734);
- `AInForcedRetire` (+0xB6C4 + i) = 1 when the entry's state (+0x2E64) is 2;
- in a Demolition Derby, the pile-up count (+0xB6DC), not needed here.

**Damage of AI cars (0x424780, on the same slot).** For an entry whose `AInCollision` is set:
- total damage = 0x99EBB0 × 100 (the RMS over the whole lattice × 5/3, clamped to 1) + the mean of
  the six components × 100;
- `AInCollisionDam` (+0xB610 + 4i) = |Δ| since its last update. Below 1 the collision flag clears;
- for a key driver, `KeynCollisionDam` (+0xB4DC + 4n) is computed the same way, and
  `KeynCollision` clears.
- For the teammate, while `teammateCollision` is set, the update writes `teammateTotDam` (+0xB3E4)
  and `teamDamFromCollision` (+0xB444). Below 1 the flag clears.

**Terminal status messages.** A car's damage status (+0x444, 7 = running) changes through
0x9E64E0. Each change away from running broadcasts `TerminalDamageMessage` with the new status as
its type, the car and a flag.
- Statuses: 3 terminal wheel damage (9F2F80), 4 a detached wheel (9E65B0), 5 the engine (A03220
  `publish`), 6 disqualified (derby, out of bounds; 60D590, 612BD0, 630CF0, 5EBCA0).
- 0x417CB0 maps the type to a slot: 1 → `TyreBlown`, 2 → `EngBlown`, 3 and 4 → `Spun` (and
  `aiSpin`), 5 → `Roll` (and `aiRoll`), 6 → `ForcedRetire`.
- For the teammate the flags are +0xB434 + slot (`teammateEngBlown`, `teammateTyreBlown`,
  `teammateSpun`, `teammateRoll`). For other cars they are +0xB674 + 0x14·slot + i (`AInEngBlown`,
  `AInTyreBlown`, `AInSpun`, `AInRoll`, `AInForcedRetire`).
- So the original's names do not match the causes. A terminal wheel plays the "spun" lines and
  the engine the "roll" lines. No shipped path sends types 1 or 2 [O static].

## 11. Part 2: teammate [O]

- The race interface's vf 0x58 tells whether the player has a teammate. vf 0x5C (0x430AA0)
  returns the entry whose driver is the profile's team-mate. The collector keeps it (+0xE8) and
  refreshes its record (+0x3F4, the layout of 0x438C10) every frame.
- Fields: `teammatePos` +0x448, `teammateFinished` +0x464, `teammateTermDam` +0x468, `teammateLap`
  +0x42C, `teamDriverChampWinPos` +0x434, `tmTimeInCurrentPos` +0x420 (§9).
- Collisions and terminal messages: §10.

## 12. Part 2: what can play [O static]

- Dead in the original:
  - the `AITermUserImpact` lines;
  - `slipStream2s`;
  - lines needing message types 1 or 2 (`AInTyreBlown`, `AInEngBlown`, `teammateTyreBlown`,
    `teammateEngBlown`).
- Career-only inputs (objectives, championship values, final race, `1_2Finish`, team to beat)
  are 0 outside the career.
- Lines that need the player's profile statistics (`terminalDamageCount`, `numRaceWins`, …) use
  the port's defaults (§7).

## 13. The port, part 2 step 2: the player's lines

`SLICE` adds CarDamage, Performance, Strategy and Rules. Every parameter those events read is
registered, including the ones that stay constant. An unregistered name would read `inRace`
(0x44C510), which turns tests such as `isNetRace equals 0` false.

**Reset 0x452520 [O].** The port's reset follows the original:
- Every call clears:
  - the one-shot flags and the overtake count;
  - `noCollisionTime`, `timeOffTrack` and `wrongWayTime`;
  - the part-loss flags;
  - the race block (0x468B30: in-race and kind flags, lap count, pace; targets −1);
  - every car record (0x417E40), including the player's. Part 1 kept the player's record over a
    restart; `playerPrevPosition` and the time in position now start from 0 as in the original.
- `playerCollision` (+0xB43A) is not cleared by the reset. Part 1 cleared it.
- With p = 1 (race set-up and Restart Race) it also clears:
  - the damage values (`totalDam`, `damLastAccident`, the lap damages, the sides);
  - the best position and `usedFlashback`;
  - it sets the seen lap to 1, `startAfterRestart` to 1 and `1_2Finish` to −1.
- With p = 0 (a flashback, 0x453000/0x454910, followed by `usedFlashback` = 1) it re-enables the
  collector.

**Getters [O].**
- The damage getters read the damage object copied on even frames (0x452C1E).
- `playerCarRevRatio` and `playerGoodStart` read the live vehicle.
- The refresh reads `damageTakenInPrevLap` before `playerCrossedSFLine` (§8).
- Part ids come from the name table 0xE4EF20: 10 `bonnet`, 12–17 `bumper_bl/br/bm/fl/fr/fm`,
  22–25 `door_bl/br/fl/fr`, 84–87 `wing_bl/br/fl/fr`. 0x417D60 calls them rear, right, left and
  front wing. 0x9FF4F0 calls the part callback when a detached part's body is made, so the port
  reports the parts that became detached in the tick.

**Inputs that are the port's own [I].**
- The wrong-way flag (race interface +0x18, set by the WrongWay message 0x431470 → 0x429640): the
  HUD's input test, moving against the racing line faster than 3 m/s.
- `droveOffTrack`: the player has no driver object in the port. The AI's `on_track` test is
  applied to the player's wheel anchors on the racing line, without a segment hint.
- Contacts: the port's solved contact impulses for every car, against half that car's physical
  mass.
- A flashback is taken when the flashback manager counts one used.
- Restart Race re-enables a collector that terminal damage disabled. Whether the original's load
  handler runs on a restart is not established (§5); without it, speech would stay silent after
  restarting a wrecked car.
- `isPlayerCarOpenCockpit` (car description +0xD4) is not traced to a port field and reads 0.

**Not available outside the career or without a profile.**
- `bonusObjWinPos`, `driverChampWinPos`, `1_2Finish`, `hasNoDamageObj`, `anyDamageIndOn` and the
  test-drive targets.
- `terminalDamageCount` (profile) is 0, so `terminalDamRestart` and `terminalDamNoRestart`
  (count > 1) do not play.
- `newPersonalBest` and `newWorldRecord` (broadcast messages) have no sender in the port.
