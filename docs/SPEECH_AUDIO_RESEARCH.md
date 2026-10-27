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
| +0 | time = race time (vf 12) / race progress (vf 18; 0 → 1e-4). An average lap time, not a gap |
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
| playerCollision | set by a player collision; cleared when an update changes totalDam by less than 1 |
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

0x452520(1) runs from the race (re)start 0x65B2C0 → 0x454870. The same call sets
`startAfterRestart` (+0xB6FC) to 1, and only the constructor clears it. So:
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
