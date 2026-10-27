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
  - It plays an armed player-name phrase first, if any, on the next call. A `namedAI` name plays
    after the line instead (§16).
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
| carAhead*, carBehind* | the cars ahead and behind on the track (lapping lines, §17) |

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
- Correction (F611): 0x44C670 registers each handler *before* the channel string it then
  subscribes. Each registration helper builds a prototype of its message type, so the pairs are
  fixed by the type name of the prototype's vtable [O]:
  - `RaceResults` (helper 0x46CCE0, vtable 0xD0E77C) → 0x41BC60, which only returns 1;
  - `TerminalDamageMessage` (helper 0x61FD50, type name at 0xCB14FC) → 0x41BC70 → 0x417C10;
  - `AIChoreographerEvent` (helper 0x46CD90, vtable 0xCAFE10) → 0x4457F0 → 0x417CB0 (and
    0x43D8A0).
- `TerminalDamageMessage` → 0x417C10, with the message's +4 = the entry and +8 = its new status:
  - `AInTermDam` = status ≠ 7;
  - the first time an entry is seen (+0x50 clear), its record (vf 0x4C) caches the key driver
    number (+0x54) and `keyTeamInRace` (+0xB6EC) becomes the record's team id (+0x44);
  - for a key driver n: `KeynTermDam` (+0xB53F + n) = status ≠ 7, and `keyDriversOut` (+0xB6E8)
    counts up when the status is not 7, down when it is.
- `AIChoreographerEvent` → 0x417CB0: §10.

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
0x9E64E0. Each change away from running broadcasts `TerminalDamageMessage` with the car and the
new status. Statuses: 3 terminal wheel damage (9F2F80), 4 a detached wheel (9E65B0), 5 the engine
(A03220 `publish`), 6 disqualified (derby, out of bounds; 60D590, 612BD0, 630CF0, 5EBCA0). The
collector handles it in 0x417C10 (§9): terminal and key-driver flags only.

**Choreographer events (corrected in F611).** An earlier draft of this section had 0x417CB0
handling the terminal messages. It handles `AIChoreographerEvent` (§9):
- The choreographer runner 0x408E50 sends it. +4 is the event's `common.message_type`
  (0x402830 parses it through the name table 0xDA83B0: 1 `blow_tyre`, 2 `blow_engine`,
  3 `brake_spin`, 4 `power_spin`, 5 `roll`, 6 `retired`, 7 `brake_lock`), +8 the car's entry and
  +0xC on/off. The runner sends it "on" once while the event is active (state 3, flag +0xBDC),
  and "off" when the event ends (state 2 or 4). An event without a type (0) sends nothing.
- Shipped events [D]: `powerout_spin` 4, `brake_spin` 3, `blow_tyre_slow`/`_fast` 1,
  `blow_engine_slight`/`_total` 2, `brake_lock` 7.
- 0x417CB0 maps the type to a slot: 1 → `TyreBlown`, 2 → `EngBlown`, 3 and 4 → `Spun` (and
  `aiSpin`), 5 → `Roll` (and `aiRoll`), 6 → `ForcedRetire`; 7 is outside the jump table and
  ignored. The slot's flag takes the message's on/off. `aiSpin`/`aiRoll` are set for "off" too.
- For the teammate the flags are +0xB434 + slot (`teammateEngBlown`, `teammateTyreBlown`,
  `teammateSpun`, `teammateRoll`). For other cars they are +0xB674 + 0x14·slot + i (`AInEngBlown`,
  `AInTyreBlown`, `AInSpun`, `AInRoll`, `AInForcedRetire`).
- So the tyre, engine and spin lines follow the choreographer's failures and mistakes. No shipped
  event has type 5 or 6, so the Roll and ForcedRetire messages are not sent by it.

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
  - the Roll lines (no shipped choreographer event has type 5).
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

## 14. The port, part 2 step 3: accidents and AI events

`SLICE` adds CrashesAccidents and AIEvents.

**Registration [O].** The numbered getter families are `AI1…AI20` and `Key1…Key24`:
- `AIn`: Collision, CollisionDam, Distance, TermDam, TyreBlown, EngBlown, Spun, Roll and
  ForcedRetire;
- `Keyn`: Collision, CollisionDam and TermDam.

`AIn` is race entry n − 1 (getters +0xB5AC, +0xB610, +0xB55C, +0xB660 and +0xB674 + 0x14·k).

**Collisions [O].**
- 0x430F50 sets `AI(i+1)Collision` only for an entry index i above 0 (`jle` at 0x4311E7), so
  `AI1Collision` is never set.
- The player is entry 0 in the port, as `playerVehicleID` = 1 implies for the original's single
  player.
- 0x424780 runs on entry `counter % entries`, after 0x431200, in this order:
  - the player's entry while `playerCollision` is set: the player branch (§9), and nothing else;
  - otherwise the teammate branch;
  - otherwise the AI branch, which also covers the player's own slot when it has no collision. It
    runs while the slot's collision flag is set: `CollisionDam` = |Δ total|, and the flag clears
    below 1.
- The total is 0x424650's: for cars other than the player's, 0x99EBB0 × 100 plus the mean
  component × 100. 0x99EBB0 is the RMS displacement over the whole lattice × 5/3, clamped to 1,
  and 0 while lattice byte +0xA4AD is clear; the four side terms have no such gate.

**0x431200 [O].** For entry `counter % entries` (skipping controller type 3) it sets:
- `AInDistance` = the entry's lap fraction (vf 0x3C → 0x417860);
- `AInTermDam` = its status is not 7.

`AInForcedRetire` from race state +0x2E64 == 2 is not ported [I: no writer of state 2 is traced].

**Messages [O] (§9, §10).**
- `TerminalDamageMessage`: the app sends one when a car's status word (+0xD234) leaves 7. The
  collector (0x417C10) sets that entry's `AInTermDam`; the key-driver part needs identities
  (below).
- `AIChoreographerEvent`: `grid_formats::ai::Event::message_type` holds the parsed type, and the
  runner (`ai_pacing::Field`, 0x408E50) records an "on" message once in state 3 and an "off"
  message when the event ends (state 2 or 4) in `Field::messages`. The app keeps the messages of
  the last 60 ticks with their tick, so that the speech frame, which runs once per rendered frame,
  sees every one. The collector maps them through 0x417CB0's jump table (0x417D44): 1 → TyreBlown,
  2 → EngBlown, 3 and 4 → Spun (+ `aiSpin`), 5 → Roll (+ `aiRoll`), 6 → ForcedRetire.
- [I] When the port's runner replaces an event that has not started (state 1) by another, it sends
  no message; the original's equivalent path is not traced.
- The AI tyre, engine and spin lines therefore play with the choreographer on
  (`--ai choreographer=on`, the career profiles' default). No shipped event has type 5 or 6.

**Reset [O].** 0x452520 always clears the slots' collision, terminal, collision damage and event
flags, and with p = 1 also their distance and last total.

**Not available in the port's races (step 3).** Driver identities: step 4 (§15) adds them.

## 15. The port, part 2 step 4: drivers, key drivers and the teammate

`SLICE` adds Teammates (261 messages; 881 lines in all).

**Where the drivers come from.**
- Career races: the career screen writes `[game] opponent_drivers` (each opponent's `driver.id`
  in car order, 0 for none) and `[game] teammate` (the hired team mate's opponent number, 0 for
  none) into the race profile. Profiles without them load as before and have no drivers.
- Free races of a series: `series::build_field` keeps the `free_race_drivers` rows that the
  removal draws leave (0x664630). The draws were already ported; the port now removes the drawn
  row instead of only counting it, so the RNG stream is unchanged. Entry i gets driver i.
- Le Mans Series fields: each `lemans_restrictions` row's `driver_id` (0x663860).
- Other races (no series, no pool, no career field) have no drivers: every key is 0.
- [I] An entry's team is its driver's `driver.team_id`. The original reads the race entry's team
  object (0x62AC70); the port has no team objects.
- The player's entry has no driver in the port (key 0, team 0) [I].

**The car record (0x438C10, 0x4689C0) [O].** A cleared record has key (+0x3C) and team (+0x44)
−1. The first refresh takes the entry's key (cached on the entry at +0x50/+0x54) and team, and
keeps them until the record is cleared again. So:
- `leadingCarKeyDriver` (getter 0x418130, +0x2BC) and `leadingCarTeam` (0x418140, +0x2C4) read the
  leader's record (+0x280);
- `raceWinnerTeam` (0x418160) reads the same +0x2C4;
- `carPosMinus1KeyDriver` (0x418BA0, +0x240) reads the record of the car one place ahead (+0x204);
- all four read −1 while their record is cleared.

**Key drivers [O].**
- The entry's key is taken once (entry +0x50) by a record refresh, a terminal message or a
  collision that involves the entry.
- 0x430F50, for each accepted contact:
  - when the other object is a car with key n > 0: `keyTeamInRace` = n (the key number, not a team)
    and `KeynCollision` = 1;
  - `carToCarCollision` is assigned (not or-ed): the last contact of a frame decides it;
  - the player hit by a car with key n > 0: `playerCollWithKeyPlayer` = n. 0x417ED0 sets it back to
    −1 after every refresh, so it lasts one frame;
  - an AI car hit (not the player, not the teammate): its own key n > 0 sets `keyTeamInRace` = n
    and `KeynCollision`.
- 0x424780, an AI slot with its collision flag set and a key n > 0 taken: `KeynCollisionDam` =
  |total − the last total of key n| (+0xB47C + 4n), the last total is updated, and
  `KeynCollision` is cleared, whatever the damage.
- 0x417C10 (`TerminalDamageMessage`): if the entry's key is not yet taken, `keyTeamInRace` = the
  entry's team, then the key is taken. For a key n > 0, `KeynTermDam` = status ≠ 7 and
  `keyDriversOut` counts +1 or −1.
- Getters: `KeynCollision` +0xB465 + n, `KeynCollisionDam` +0xB4DC + 4n, `KeynTermDam` +0xB53F + n.
  `Key19Collision` (0x41B500) reads +0xB479, key 20's flag, as `Key20Collision` does; key 19's own
  flag (+0xB478) is never read. The port keeps this.
- 0x452520 clears every key's flags and damage, `keyDriversOut`, `keyTeamInRace` and
  `playerCollWithKeyPlayer` (−1); with p = 1 also the last totals.

**The teammate [O].**
- 0x452997: while +0xE8 is 0 and the race interface has a teammate (vf 0x58), +0xE8 = vf 0x5C.
  0x417E40 clears +0xE8 with the records, so a restart or a pause takes it again.
- Its record (+0x3F4) is refreshed every frame. Getters: `teammatePos` +0x448, `teammateFinished`
  +0x464, `teammateTermDam` +0x468, `teammateLap` +0x42C, `tmTimeInCurrentPos` +0x420,
  `teamDriverChampWinPos` +0x434 (record +0x40, a championship value; the port keeps 0 [I], as
  `driverChampWinPos`).
- `tmTimeInCurrentPos` (0x452DAD) grows by dt while the teammate's position equals +0xB408 (its
  position at the last frame), else 0. +0xB408 is set only here and by the constructor.
- `teammateVehicleID` (+0xB724) = the teammate's entry + 1, taken before the race while a
  teammate is known; it keeps its value otherwise.
- `playerHasTeammate` (0x418980) = +0xE8 ≠ 0.
- Collisions (0x430F50): the player hit by the teammate, while `teammateTermDam` is 0, sets
  `playerCollideWithTeammate` (one frame: 0x417ED0 clears it). The teammate hit sets
  `teammateCollision`; 0x424780 then writes `teammateTotDam` and `teamDamFromCollision`
  (|Δ total|) on the teammate's frame and clears the flag below 1.
- Choreographer messages for the teammate's entry set +0xB434 + slot (`teammateEngBlown`,
  `teammateTyreBlown`, `teammateSpun`, `teammateRoll`) instead of its AI slot.
- `teammateNumber` (race block +0x34, 0x451A20): cleared to −1 with the block. In the career mode
  the block computes it once: 0 without a hired team mate, otherwise 0x430D60 for the team mate's driver:
  - the `nationality_voice` rows of the driver's `nationality_id`;
  - skip (picks for that nationality so far) % (rows) rows, count the pick, and cache the number
    for that driver (+0xC0/+0xC4);
  - 0x4244B0 maps the voice: `us_1`–`us_3` 1–3, `british_1`–`british_3` 4–6, `french_1` 7,
    `italian_1` 8, `german_1` 9, `spanish_1` 10, `japanese_1`/`_2` 11/12, `scan_1` 13, `irish_1`
    14; anything else 0 (the database's `japan_1`, nationality 37).
  - [I] The port starts every race with no picks counted, which is the original's result for the
    first team mate of a game session. The original keeps the counts and the last driver's number
    for the whole session.
  - [I] 0x451A20 also excludes two event kinds (`local_78[0x11]` 1 and 2) that are not
    identified.
- The Teammates lines are numbered by `teammateNumber` (`teammate4_spin`, `team4Plus1Less3s`, …),
  so the voice decides which recorded name is said.

**Checks.**
- Unit tests: the teammate's record, collisions, events and restart; key drivers through records,
  collisions (including the Key19 quirk), damage and terminal messages; the voice table; the
  free-race drivers and the Le Mans driver ids; the career profile's drivers.
- Offline render (`grid-speech-render`, local `render-05`): the authored race with a career field
  (drivers 4, 5, 7, 10, 11, 12, 8, 13, 14, 15, 16; teammate driver 8, British, number 4) plays
  key-driver and key-team position lines, `team4Plus1Less3s`, `team4Minus1Less3s`,
  `teammate4_spin` and `playerWonTeam4`.

## 16. The port, part 2 step 5: the `namedAI` name phrase [O]

**Arming (0x437F70).** After a line loads, `int(0, 100)` ≤ `linkedPhrasePercentage` arms a name:
- nameToSay 0–2 arm the player-name phrases only while the profile has names (+0x609); ported
  in §18;
- nameToSay 3 (`namedAI`): n = (int) `userData` (+0x78). It requires n > 0, n − 1 > 0 and
  n − 1 < the entry count (game interface vf 0x10). So the `AI_01_…` messages (entry 0, the
  player's slot) never name anyone. The game interface's vf 0xC gives entry n − 1's driver, and
  the named-AI stream (+0x3E8) loads `speech/<lang>/Names_AI/<pit_speech_filename>.raw`; +0x60E
  is set.
- [I] An entry without a driver in the port arms nothing. The original always has drivers.

**Playing (0x42FFC0, called while +0x60C is pending and the voice is free).**
1. The line plays. If the last message is `namedAI` and +0x60E is set, pending stays set and the
   silence base is not moved; otherwise pending clears and the silence base is now + 2.5.
2. On a later call, once the voice is free again: if the name loaded, it plays, pending clears,
   the silence base is now + 2.0 and +0x60A clears. If the name failed to load ("Error loading
   NAMED_AI"), pending clears and the silence base stays. Either way +0x60E clears.

**Stops and resets.**
- 0x430350(p) drops an armed name and the pending playback. With p = 1 it also empties a line
  stream that is not loading (pause, a load error); preemption calls it with p = 0.
- The reset 0x444E30 clears +0x60E, +0x60A and both streams.
- The port now tracks the line stream's ready flag (+0x1D4) separately from the file name: the
  repeat test still compares the last file, and a line is played once.

**Checks.**
- Unit test: the name follows the line once the voice is free, with the 2.0 s silence base; entry
  0 and a driverless entry are never named; a missing name file ends the playback.
- Render (`local/speech-render/render-06`): AI_3_AISpin is followed by `Names_AI/Chief_AI_457`
  (entry 2, driver 5), AI_05_accidentTerm by `Chief_AI_443` (entry 4, driver 10) and
  AI_02_tyreBlow by `Chief_AI_467` (entry 1, driver 4).

## 17. The port, part 3 step 1: the lapping lines (cars ahead and behind on the track) [O]

**The track order.** The race keeps a list of its entries sorted by position on the track, not by
race position (race +0x21A0 → +0xCC; elements of 12 bytes: entry, line descriptor, key).
- `track_order_update` (0x917190) runs every frame. For each element it takes the race car's
  line descriptor (+0x74) and its arc along the racing line (+0x78, distance through 0x909340)
  as the key.
- `track_order_sort` (0x91B240, through 0x91BA70) is an insertion sort, ascending by the key (the
  descriptor at DAT_00E4F564 has byte +1 = 1). The list survives from frame to frame.
  `track_order_add` (0x91BCB0) adds an entry; `track_order_find` (0x46E8D0) finds an entry's node.
- `track_order_next` (0x46D260) and `track_order_prev` (0x46D320) step to the next (ahead) and
  previous (behind) element. They are called with 0: no wrap at the ends of the list.
- Game interface vf 0x80 (`track_order_index`, 0x445770) gives an entry's index in the list;
  vf 0x48 (`cars_ahead_behind_on_track`, 0x4456F0) writes the next and the previous entry
  (0 when none).

**The collector (0x452810, case 3 of `counter % 4`, 0x452B8C).** It calls vf 0x48 for the player
with both outputs preset to 0. For each output: if the car changed, the record (+0x2FC ahead,
+0x378 behind) is cleared (0x4689C0); a present car's record is refreshed through vf 0x4C. The
player record's +0x34 is the player's index (vf 0x80).

**`playerMovedTrackPos` (+0xB424).** Set when the player's index is above the reference +0xB414
(signed). The reference takes the index every 60th frame (0x4692B0), in and out of the race; it
is 0 after construction and never reset. The flag is one-shot: 0x417ED0 clears it after the
publish.

**Getters.**

| Parameter | Getter | Value |
| --- | --- | --- |
| carAheadLap, carBehindLap | 0x4187A0, 0x4187F0 | record +0x38 (lap) |
| carAheadDistance, carBehindDistance | 0x418A00, 0x418A10 | record +0x18 (lap fraction) |
| carAheadTerminal, carBehindTerminal | 0x418A60, 0x418AA0 | 0 if the car is not terminally damaged and racing (+0x74 clear, +0x75 set), else 1; a cleared record reads 1 |
| carAheadSpeed, carBehindSpeed | 0x418AE0, 0x418AF0 | record +0xC (mph) |
| playerMovedTrackPos | | +0xB424 |

**The lines.**
- `dontWorryCarAhead` (Position, `inRacePlus1`): the car ahead on the track is a lap or more down
  (`playerLap` > `carAheadLap` > 1), racing, 0.05 to 0.1 laps ahead and above 40 mph. The
  Position event also needs `playerLapDist` < 0.85.
- `lappedDriver`: the player has just moved up the track order (`playerMovedTrackPos`) past a
  racing car one lap down, now 0.05 to 0.1 laps behind it and above 40 mph, with the player past
  0.1 of the lap.
- LeMans `leMansCarBehind` reads the same parameters (Le Mans is not in the port's slice).

**The port.**
- `Entry::arc` and `Entry::racing`; the collector keeps the order, sorts it the same way every
  frame and fills the ahead/behind records in case 3.
- [I] grid-app's arc is the nearest racing-line point (`ai_driver::Line::arc`, a global search)
  for every car, finished cars included, not the original route tracker's +0x78. `racing` is
  `Standings` status `Racing`.
- The render probe's entries use the lap fraction × 1000 as the arc. Its scenario cannot stage a
  lapping line: the player leads only from 164 s of lap 3, the Position event stops at a lap
  fraction of 0.85 (171 s), and `greatOverTaking` takes that slot with the 7 s minimum silence.

**Checks.**
- Unit test `the_track_order_names_the_cars_ahead_and_behind`: the sort, the ahead/behind
  records, the terminal getter of a cleared record and the 60th-frame reference.
- Purchased-rules test `purchased_rules_play_the_lapping_lines`: the leading player on lap 3
  with a lapped car 0.07 ahead hears `dontWorryCarAhead`; passing it every 10 s gives
  `lappedDriver`.
- Render `local/speech-render/render-07`: the lines of render-06; only the end-of-race variation
  picks differ, as the rolls now see the lapping parameters.

## 18. The port, part 3 step 2: the player-name phrases [O]

**The audio name.** The profile keeps an audio name of two words, read through the speech
manager's +0xC object at +0x28: the kind (0 male, 1 female, 2 nickname) and a number.
`audio_name_format` (0x470880) writes it as `%c_%02d` (`M_01`, `F_55`, `N_42`; with a prefix
`%s_%c_%02d`). The front end offers it as "Audio Name" (language ids
`commentary_name_M_01` Aaron … `commentary_name_M_100` William, `commentary_name_F_01` Abigail …
`F_55` Victoria, `commentary_name_N_01` Ace … `N_42` Wingnut) and as "Disable Audio Name" with a
warning popup. Number 0 is no name.

**Loading (0x465690, the manager's vtable entry at 0xCB78A0).**
- Mode 1 builds `speech/<lang>/Names_Player/man/Man_` + `WD_`/`Hi_`/`Hello_` + the name; mode 2
  (a race) builds `speech/<lang>/Names_Player/pit/Chief_` + `WD_`/`BL_`/`OK_` + the name, each with
  `.raw` (strings at 0xCB3364 `BL_`, 0xCB336C `Hi_`, 0xCB3370 `WD_`, 0xCB0F74 `.raw`).
- The three streams: +0x1D8 (`WD_`), +0x288 (`Hi_`/`BL_`) and +0x338 (`Hello_`/`OK_`). +0x608 marks
  the paths built, +0x609 the streams loaded (all three ready, or one failed).
- Number 0 sets +0x608 and +0x609 without building or loading anything.
- [V] The 2026-10-06 race-load capture (docs/REFERENCE_DEPENDENCIES.md) read
  `Names_Player/pit/Chief_BL_M_01`, `Chief_OK_M_01` and `Chief_WD_M_01`: a race uses mode 2, and
  the owner's profile had `M_01`.

**Arming (0x437F70).** After a line loads and `int(0, 100)` ≤ `linkedPhrasePercentage`: with +0x609
set, nameToSay 0 (neutral) sets +0x3E4 (`OK_`), 1 (positive) +0x284 (`WD_`) and 2 (negative)
+0x334 (`BL_`), and the load returns; otherwise `namedAI` (§16).

**Playing (0x42FFC0).** An armed flag (+0x284 first, then +0x334, then +0x3E4) and no line load
error (+0x1D5): the name stream plays on group 14 and its flag clears. Pending and the silence base
do not change, so the line plays on a later call, once the voice is free. Every stop (0x430350)
clears the three flags.

**The port.**
- `Manager::set_player_name` takes the audio name and builds the three `Chief_` lines
  (`speech::player_name_lines`); `try_start` arms the phrase, `playback` plays it before the line.
- `[game] audio_name` (`M_nn`, `F_nn`, `N_nn` or `off`) in grid-app's settings. Default `M_01`
  [I: the owner's profile in the capture; the port has no profile screen]. grid-app prints it in
  `speech_audio … event=drivers … audio_name=`.
- [I] A missing name file plays nothing, and the line follows at once. With number 0 the original
  still arms a flag on an empty stream; what that plays is not established. The port's `off` arms
  nothing (the roll is still drawn).
- `grid-app --speech-files` lists the three `Chief_` files of the profile's audio name, so the APK
  bundle lists carry `Chief_{OK,WD,BL}_M_01`.

## 19. The port, part 3 step 3: the championship and career values [O static]

The full analysis is in [SPEECH_CAREER_PARAMETERS.md](SPEECH_CAREER_PARAMETERS.md).

**Where the values live.** The registration 0x45F590 (`speech_params_register`) pairs 419
parameter names with getters; each getter reads a collector field. The race block of 0x451A20 is
collector +0x60. It is refreshed every frame before the race and on four frames of five during it
(§8). The block clear `race_block_clear` (0x468B30, every reset and the load handler) writes 0
except −1 at block +0x28, +0x34, +0x38, +0x3C, +0x44 and +0x48.

| Parameter | Getter | Field | Value |
| --- | --- | --- | --- |
| driverChampWinPos | 0x4185A0 | player record +0x40 | always 0 (below) |
| teamDriverChampWinPos | 0x418800 | teammate record +0x40 | always 0 |
| isFinalRace | 0x418530 | block +0x6F | own career event (not a driver offer) on its last race: E vf 0x7C (`event_on_last_race`, 0x640800), once per reset |
| isFinalCareerEvent | 0x418560 | block +0x72 | the event is a head-to-head (0x637930) of a tier whose career rank is international (0x637B10 → 0x62C280: rookie, pro, expert, international = 0…3) |
| wonChampionship | 0x418170 | block +0x48 | −1; on the final race, once its result list is not empty: 1 when the player's dense rank is 0 in the standings including this race (0x6706B0), else 0 |
| 1_2Finish | 0x418D00 | +0xB71C | −1 (race set-up); once, on the final race's last lap (0x452991): 1 when the player and the teammate are ranks 0 and 1 (`champ_one_two`, 0x4EF9C0) |
| bonusObjWinPos | 0x418590 | block +0x28 | a race driver offer with a finishing-place objective: its `objective_finishing_place` (0x62A8D0 + 1); else 0 |
| hasNoDamageObj | 0x418090 | block +0x7C | a race driver offer with the no-damage objective (class 4) |
| anyDamageIndOn | 0x419010 | block +0x81 | only then: any HUD damage icon lit (0x8EF920 → `indicators`) |
| numRaceWins | 0x418DA0 | block +0x3C | race history (game +0x22F0) +0x44: career race wins before this race, read once per reset |
| numEventWins | 0x418DB0 | block +0x58 | race history +0x30: own career events won |
| racesCurTrack | 0x418DD0 | block +0x5C | race history item of the track: races driven there |

- **`driverChampWinPos` and `teamDriverChampWinPos` are never computed.** The car record refresh
  0x438C10 calls 0x670470 only while record +0x40 is −1. The record constructor 0x468920 and the
  clear 0x4689C0 both write 0, and no other writer was found. The lines that need a non-zero value
  (`DriverChampObj1st`…, `driverChampWin1st`…, `teamNeeds1st`, `teamBehindNeedsToWin`) are dead
  in the original. [I] The result would also be a 0-based place against the rules' 1-based tests.
- `FinalRaceNoBonus` and `FinalRaceLapBonus` need `bonusObjWinPos` and `isFinalRace` together,
  which never happens (offers are not own events): dead.
- The race history counters (`numRaceWins`, `numEventWins`, `racesCurTrack`; writers 0x6105F0,
  0x60A960, 0x60F950) have no port counterpart. `Career.races_won` and `layout_races` count
  differently. They keep the port's defaults: `numRaceWins` 1 [AUTHORED, §7], the others 0.
- Not established:
  - the extra race-win condition on game +0x31C;
  - game modes 2, 3 and 5;
  - whether every plain race has one part (0x640800);
  - when the current race's result list fills;
  - the touge exclusion in the `wonChampionship` test.

**The port.**
- `RaceView::career` (`CareerRace`) and `damage_indicator`. The collector publishes the values
  above. `wonChampionship` follows the block clear (−1) outside the career.
- [I] The original's result list is approximated by the current race order once a car has
  finished. Each car's total is its points before the race plus the points of its current place,
  against the best points of the event's entrants who are not in the race.
- The career screen (`career_screen::speech_lines`) writes `[game] speech_final_race`,
  `speech_final_event`, `speech_standing`, `speech_standing_absent`, `speech_one_two`,
  `speech_bonus_place` and `speech_no_damage`. grid-app reads them, with the points table, from
  `[game] points`.
- [I] 0x4EF9C0 counts only results already final, so the port computes `1_2Finish` from the
  races before this one.
- [I] The standings are written for career-field races of own events with a points table.
  Head-to-heads, the AUTHORED fields and the Le Mans selection races (class standings, not
  ported) get the flags without standings, so `wonChampionship` stays −1 there.

**Checks.**
- Unit test `the_final_race_settles_the_championship`.
- Career screen tests: `bmw_si_tour_is_offered_planned_and_paid` (race 2's lines after a win)
  and `the_international_head_to_head_is_the_final_career_event` (78 on, 73 off).
- Render `local/speech-render/render-10` (`career`): `wonChampionshipFirstTime` at the finish
  instead of `playerWonTeam4` and `win`.
