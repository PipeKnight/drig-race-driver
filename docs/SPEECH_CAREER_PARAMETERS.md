# Race engineer speech: the career and profile parameters (research)

27 October 2026, Claude Code (car-series, speech part 3 step 3; research by a subagent of that
session, checked against the port in F624). Static only. The port and its summary are in
[SPEECH_AUDIO_RESEARCH.md](SPEECH_AUDIO_RESEARCH.md) §19.

Scope: where twelve career/profile parameters of the race engineer's speech get their values
in the original executable (`local/compatibility/grid-unpacked.exe`), so that they can be ported.
Method: `tools/re/grid_re.py` (dis, xrefs, containing, decompile through the master project;
pseudocode cached in the main checkout's `local/re-cache/`), plus small capstone scans. Nothing
was run; no capture. Marks: [O] observed in the static code, [I] inference.

## 0. Common facts

**Getter table [O].** The registration 0x45F590 (`speech_params_register`) repeats
`push getter; push name; mov ecx, esi; call 0x456B80`. A scan of 0x45F590–0x461800 finds 419
pairs (Key19Collision → 0x41B500 matches the known row). Every getter takes the collector pointer
as its argument. Byte flags return 1.0 (the float at 0xCAF888) or 0.0; ints are `fild`;
`numEventWins` and `racesCurTrack` are read as unsigned (+2^32 when negative).

**The race block is collector +0x60 [O].** The update 0x452810 calls the race interface's vf 0x40
(0x451A20, vtable 0xCB4CF8 slot 0x40 = 0xCB4D38) with `lea [esi+0x60]`. So the block fields of
0x451A20 are at collector offset = block offset + 0x60.

**When the block is refreshed [O].** 0x452810 (after the enable checks) increments the frame
counter +0xB410. While the in-race flag (+0xCC) is clear, it calls 0x451A20 every frame. While it is
set, it calls it only when counter % 5 ≠ 0 (four frames of five).

**Block clear 0x468B30 [O].** Called by the reset 0x452520 (every call: race set-up and Restart Race
with p = 1, flashback with p = 0) and by the load handler (0x43927E). It writes 0 to all fields
except: +0x14, +0x18 (the float at 0xCB0948), and +0x28, +0x34, +0x38, +0x3C, +0x44, +0x48 = −1.
In collector offsets: `bonusObjWinPos` +0x88, `teammateNumber` +0x94, +0x98, `numRaceWins` +0x9C,
`teamToBeatNum` +0xA4 and `wonChampionship` +0xA8 start at −1.

**The objects 0x451A20 reads [O, names I].**
- `game` = race interface +0x34.
- Game mode = `[game+0x23D0]+0x50`. `isNetRace` = mode 1, `inIntroRace` = mode 6,
  `isTestDrive` = mode 4 or event kind 2. Mode 0 is the career [O: 0x4EF9C0 and 0x60A960 gate
  career-only work on mode 0; CAREER_RESEARCH §3: the end-of-event dispatcher runs only in career].
- M = mode object (0x640020 → `[game+0x23D0]+0x34 + mode*4`). Its vtable for the career is
  0xCEA758: vf 0x80 = 0x63F320 (the current event), vf 0x8C = 0x6372D0 (track model id),
  vf 0x98 = 0x69B290 (Le Mans / multi-class, CAREER_RESEARCH §3).
- E = the event run (M vf 0x80). Vtable 0xCE9EC0 (interface at E+0x38: 0xCE9EA8). Fields used:
  +0x10/+0x14 the races (vf 0x24 = their count, 0x67DAC0), +0x30 active (vf 0x1C, 0x67CEF0),
  +0x34 current race index, +0x3C/+0x40 the database row of the event or offer, +0x44 kind
  (0 own event, 1 race driver offer, 2 test driver offer), +0x48 multi-class flag, +0x4C format,
  +0x50 the offer objective, +0x54 points table, +0x1C the field list.
- 0x451A20 dereferences E without a null test in several places, so every mode has an E [I].

## 1. Per parameter

### driverChampWinPos
- Getter 0x4185A0 → collector +0x14C = player car record (+0x10C) +0x40 [O].
- Writer: the car record refresh 0x438C10 (race interface vf 0x4C). Only while record +0x40 == −1
  it calls 0x670470 on E with the car's driver, stores the result, and turns −1 into 0 [O].
- **Never computed [O static]:** the collector constructor initialises the records with 0x468920
  and every clear uses 0x4689C0 (`car_record_clear`); both write +0x40 = 0, not −1. No other writer
  of record +0x40 was found (scan of 0x401000–0x480000 for `+0x14C]`/`+0x434]`, and of 0x438C10
  and 0x452810–0x453000 for `+0x40]`). So the guard is never true and the value is 0 in every
  race, career or not. [I] An original bug: the guard expected −1 as "unset".
- What 0x670470 would compute [O code, never reached]: only on the event's last race
  (E+0x34 == races − 1). Standings without the current race (Standing 0x66FF70 with flag 0; class
  standings when E+0x48). If the driver leads (dense rank 0): tied leaders → 0; else need =
  points[0] + (2nd's points − own points); if need > 0, the result is the largest 0-based place k
  (below the size of E+0x1C) with points[k] ≥ need, else −1. If the driver does not lead: 0 when
  own + points[0] ≥ leader + points[1], else −1. So the result is a 0-based place, while the speech
  rules test 1…8 as 1-based places [I: off by one as well].
- Messages: Strategy/gridPlaceInfo `DriverChampObj1st`…`8th`, `playerWinWin`;
  inRaceFinalLapFinalRace `mathChanceToWin`, `driverChampWinCur`, `driverChampWin1st`…`4th`;
  the group condition of inRaceTeamDec (== 0). With the value always 0, all of these are dead
  except that inRaceTeamDec's group condition always holds.
- Port: keep 0 (already 0 by default). Nothing to supply.

### teamDriverChampWinPos
- Getter 0x418800 → collector +0x434 = teammate record (+0x3F4) +0x40 [O].
- Same writer and the same guard as above (0x438C10 via 0x4529D6); always 0 [O static].
- Messages: inRaceTeamDec `teamNeeds1st`, `teamBehindNeedsToWin` (== 1): dead.
- Port: keep 0 (already set to 0 in publish).

### wonChampionship
- Getter 0x418170 → collector +0xA8 = block +0x48 (int) [O].
- Writer 0x451A20 at 0x451CE8/0x451D31 [O], on every block refresh when all hold:
  E exists, `isFinalRace` (block +0x6F) set, mode 0, the current race's result list is not empty
  (0x683E50 → E vf 0x38 = 0x81A500, the current leaf race; its +0x64/+0x68 vector), and not
  (E+0x4C == 5 and the touge state `[game+0x23EC]` exists with +8 ≠ 2 and +4 == 4).
- Value: 0x6706B0 on E: Standing(E races, multi-class = M vf 0x98(1), include the current race = 1),
  then the line whose driver has type +0x1C == 2 (the player [I]); `wonChampionship` = 1 when its
  dense rank is 0, else 0 [O].
- Side effect: when it is 1 and the event is the final head-to-head (`isFinalCareerEvent` test),
  block +0x71 (collector +0xD1) = 1. No getter reads +0xD1 [O: getter offset table].
- Outside the career, in driver offers, before the final race and before the current race has
  results: −1 (the block clear) [O]. [I] So during the final race it becomes 0/1 only once
  the race's result list fills, i.e. around the finish.
- Messages: raceEndOther `wonChampionshipNotFirst`, `wonChampionshipFirstTime`,
  `wonChampionshipAndRace` (== 1), `tooLowToWinEvent` (== 0).
- Port: today 0 (original −1 outside the career; no rule reads it outside the final race, so the
  difference is invisible [I]). Supply: the player's rank in `rules::standings` over
  `InProgress.results` including the race just finished, on the event's last race, own events only,
  and −1 otherwise. The port has all inputs (points table, results, entrants) [O in
  career_screen.rs / career/rules.rs]; the class standings for Le Mans are not ported.

### isFinalRace
- Getter 0x418530 → collector +0xCF = block +0x6F (byte) [O].
- Writer 0x451A20 at 0x451BE0 [O], once per reset (it runs only while block +0x34, the
  teammate number, is still −1; the same branch sets +0x34), and only when mode 0 and E kind is not
  1 or 2 (no driver offers). Value = E vf 0x7C = 0x640800.
- 0x640800 [O]: true when E is on its last race (0x7F7CE0: count > 0 and index +0x34 == count − 1)
  and the current race R is itself on its last part (R vf 0x34, or for a multi-part race
  (R vf 0x7C true) R vf 0x84 = 0x641D70, the same test recursively). [I] For a plain race this
  is "the event's last race"; whether a plain race has exactly one part was not checked.
- Outside the career and in driver offers: 0 [O].
- Messages: inRaceFinalLap (== 0, `lastLapNotFirst`, `lastLapFirst`), inRaceFinalLapFinalRace
  (== 1), the three `wonChampionship…` lines, `tooLowToWinEvent`.
- Port: today 0. Supply: own career event and the race about to be driven is the last of
  `InProgress.races` (`results.len() + 1 == races.len()`). Available in the port [O].

### isFinalCareerEvent
- Getter 0x418560 → collector +0xD2 = block +0x72 (byte) [O].
- Writer 0x451A20 at 0x451C3A [O], every block refresh: set to 1 (never cleared except by
  0x468B30) when E exists, mode 0, 0x637930(E) and 0x637B10(E).rank == 3.
  - 0x637930: for E kind other than 1/2, the event row's `head_to_head` [O].
  - 0x637B10: `event.tier_id` → `tier.career_rank_id` → `career_rank.name_string_id`, then
    0x62C280: its index in {rookie, pro, expert, international} (0xDB4C60) [O]. 3 = international.
- So it is 1 for the head-to-head of an international-rank tier (`i_h2h`, event 78, CAREER_RESEARCH
  §2.2) [O + D]. Outside the career: 0.
- Messages: raceEndFirst `finalEventWin` (== 1), raceEndOther `notAchievedWin_finalh2h` (== 1),
  `tooLowToWinEvent` (== 0).
- Port: today 0. Supply: event.head_to_head and the tier's rank is international. Catalogue has both
  (`Event.head_to_head`, tier rank) [O].

### 1_2Finish
- Getter 0x418D00 → collector +0xB71C (int) [O].
- Reset 0x452520 with p = 1 writes −1 (0x4526B1) [O]; the port does this already.
- Writer: the update at 0x452991 [O], once: while in race (+0xCC), `isFinalRace` (+0xCF), the
  player's record lap (+0x144 = record +0x38) == `currentTrackLaps` (+0x80), and +0xB71C < 0:
  +0xB71C = (0x4EF9C0 ≠ 0). The object is race interface vf 0x38 → +0x21C4 → +8 → +0x48 → +0x78.
- 0x4EF9C0 [O]: only in mode 0, E active (vf 0x1C), kind not 1/2, E vf 0x7C (final race). It builds
  the event standings (class standings via 0x661830 when Le Mans), counting the entries of each race
  whose result is final (+0x11) [I: so the current race adds little or nothing before the finish],
  dense ranks, then finds the ranks of the driver types 2 (player) and 1 (teammate). Returns 1 when
  both are found and rank(player) + rank(teammate) == 1, i.e. 1st and 2nd in the championship.
- So: −1 outside the career and before the last lap of the final race; then 0 or 1 [O].
- Message: inRaceFinalLapFinalRace `Team12Win` (== 1).
- Port: has the −1 reset only; missing the last-lap write. Supply: on the frame the player starts
  the last lap of a final race of an own career event: player and teammate (Entrant::TeamMate) are
  dense ranks 0 and 1 in `rules::standings` over the finished races. Inputs exist in the port.

### bonusObjWinPos
- Getter 0x418590 → collector +0x88 = block +0x28 (int) [O].
- Writer 0x451A20 at 0x451F6D–0x451F8C [O], every block refresh, all modes:
  - E kind == 1 (race driver offer): while −1, 0x62A8D0(E) + 1, or 0 when it returns −1.
  - otherwise 0.
- 0x62A8D0 [O]: kind 1 and the offer objective class (E+0x50 +4) is 0 or 1: the `driver_offer`
  row with `id` = (E+0x38) vf 8 [I: the offer id], its `objective_finishing_place` − 1; else −1.
  So the value is `objective_finishing_place` as stored (1-based).
- Objective classes (+4) from the constructors [O]: 0 0x62CB50 finishing place (race_offer_finishing_place,
  race_offer_finishing_place_le_mans, sponsor_offer_finishing_place), 1 0x62CB80 finishing place no
  damage (sponsor), 2 0x62CBF0 beat rival, 3 0x62CBB0 finish (sponsor), 4 0x62CBD0 no damage
  (race_offer_no_damage, sponsor_offer_finish_no_damage), 5 0x6654E0 speed test, 6 0x665520 time
  trial. E+0x50 is built only by 0x670C50, only for kinds 1 and 2, from
  `driver_offer.objective_type_id` (0x66E3A0) [O]. Sponsor objectives live elsewhere (0x69B120).
- Outside driver offers: 0 [O].
- Messages: Strategy/gridPlaceInfo `BonusObj1st`…`8th` (need `onStartingGrid`, which only a restart
  from a race in progress gives, SPEECH_AUDIO_RESEARCH §5); inRaceFinalLapFinalRace
  `FinalRaceNoBonus`, `FinalRaceLapBonus`. The latter two also need `isFinalRace` = 1, which is
  never set in driver offers, so they are dead [O static combination].
- Port: supply `objective_finishing_place` of the race offer when its objective is finishing place
  (including the Le Mans variant), else 0. The catalogue has the offer and objective
  (`OfferObjective`, data.rs) [O]. [I] For the Le Mans variant the original compares a class place
  with the overall `playerPos`.

### hasNoDamageObj
- Getter 0x418090 → collector +0xDC = block +0x7C (byte) [O].
- Writer 0x451A20 at 0x4522F5 [O], every refresh: set to 1 when E vf 0x1C (active), E+0x50 exists
  and its class is 1 or 4. Never cleared except by 0x468B30.
- Since E+0x50 exists only for driver offers, this means a race offer with `race_offer_no_damage`
  (class 4) [O + I: class 1 cannot come from `driver_offer_objective_type`].
- Outside: 0.
- Message: `objCarDamageLimit` (with `anyDamageIndOn` == 1).
- Port: supply 1 for a race offer whose objective is `OfferObjective::NoDamage`.

### anyDamageIndOn
- Getter 0x419010 → collector +0xE1 = block +0x81 (byte) [O].
- Writer 0x451A20 at 0x452329 [O], only inside the `hasNoDamageObj` branch: on the first pass it
  looks up the `dialinterface` HUD object (0x8EF250, name at 0xE4EB78) and stores it at
  `[game+0x23CC]+0xC` +0x3C; on later passes +0x81 = 0x8EF920 = `indicators` (0x8EEF60) for the
  HUD's car: any HUD damage icon above stage 0 (summed stages > 1e-4, docs/MECHANICAL_DAMAGE.md §4).
- Outside a no-damage race offer: 0 [O].
- Messages: `objCarDamageLimit` (== 1), `totalLightDamage` (== 0).
- Port: `grid_world::mechanical_damage::indicators` exists [O]; supply "any stage > 0" only in
  no-damage race offers, else 0.

### numRaceWins
- Getter 0x418DA0 → collector +0x9C = block +0x3C (int) [O].
- Writer 0x451A20 at 0x451D91 [O], all modes, while block +0x3C == −1 (so once per reset):
  `[game+0x22F0]+0x44`.
- `game+0x22F0` is the race history (`RACEHISTORY`, CAREER_RESEARCH §9.2) [I: 0x46EC20 on it says
  "race history items"]. +0x44 is incremented by 0x6105F0 (from the race-end recorder 0x610CB0 →
  player car) [O]: mode 0 or 6, an unidentified condition on `[game+0x31C]` (+8 == 2 or +4 in
  1..3), player status (vf 0x50) == 1 and finishing position (+0xC) == 0. Modes 1 and 5 count
  into +0x8C instead.
- So: career (and intro) race wins before this race, kept in the profile; read at the race start,
  not updated during the speech of that race [I: the block is not refreshed until the next reset].
- Message: raceEndFirst `firstWinEver` (== 0).
- Port: today 1 [AUTHORED]. `Career.races_won` exists but is counted at the event end over the
  event's races, own events and offers, career only [O state.rs:678]; not the same counter
  (intro race, time of counting). No profile race history in the port.

### numEventWins
- Getter 0x418DB0 → collector +0xB8 = block +0x58 (unsigned) [O].
- Writer 0x451A20 at 0x451D97, same once-per-reset branch: `[game+0x22F0]+0x30` [O].
- +0x30 is incremented by the event-end recorder 0x60A960 [O]: mode 0, E kind 0 (own events),
  the event row found, and the player's championship rank (class rank for multi-class) == 0.
  The same code counts events (+0x40).
- Message: raceEndOther `wonChampionshipFirstTime` (== 0).
- Port: no counter. `Career.event_results` (best place per event) could give a lower bound
  (distinct events won), not the original count of event wins with repeats.

### racesCurTrack
- Getter 0x418DD0 → collector +0xBC = block +0x5C (unsigned) [O].
- Writer 0x451A20 at 0x451DD4, same once-per-reset branch [O]: the track id = M vf 0x8C
  (track model) → `track_model.track_id` (−1 if none); 0x46EC20 on the race history finds or
  creates the item for that track id; value = item +8.
- Item +8 is incremented at the race end by 0x60F950 (all modes reached by 0x610CB0, when the race
  history is enabled, `[+0x288]+0xF0`) [O]; +0xC (wins at the track) by 0x6105F0.
- So: the races driven before on this `track` (not `track_model`), any mode [O/I].
- Messages: startingGridGen `gridNewbie` (== 0), `grid2` (== 2), `gridVet` (> 3). That group is
  dead in the original (SPEECH_AUDIO_RESEARCH §5).
- Port: `Career.layout_races` counts per `track_model` in the career only [O state.rs:97]; not the
  same key or scope. Since the group is dead, a port value has no audible effect.

### Adjacent (same branch, not asked)
- `terminalDamageCount` (0x418360, +0xA0 = block +0x40) = history +0x54 (sum of past races'
  `[game+0x280]+0x98`, 0x60F950) + the current race's `[game+0x2190]+0x98`, every refresh [O].
- `teamToBeatNum` (0x419040, +0xA4 = block +0x44): offers of kind 1/2 with objective class 2 (beat
  rival), the `team` row's `id`; else 0 [O, decompile only].

## 2. Summary of what can play [O static unless marked]
- Always dead: every line keyed on `driverChampWinPos` ≠ 0 or `teamDriverChampWinPos` == 1;
  `FinalRaceNoBonus` and `FinalRaceLapBonus`; the `racesCurTrack` lines (dead group).
- Career final race of an own event: `isFinalRace`, `wonChampionship`, `1_2Finish`.
- Final International head-to-head: `isFinalCareerEvent`.
- Race driver offers: `bonusObjWinPos` (grid lines only after a restart), `hasNoDamageObj`,
  `anyDamageIndOn`.
- Profile history: `numRaceWins`, `numEventWins`, `racesCurTrack`.

## 3. Not established
- The class of `game+0x31C` (the extra condition of the race-win counter) and the full list of
  game mode numbers (0 career, 1 net, 4 test drive, 6 intro observed; 2, 3, 5 not identified).
- Whether a plain race object has exactly one part, so that 0x640800 is true on the last race (I).
- Exactly when the current race's result list (+0x64) fills relative to the end-of-race speech.
- The meaning of the touge exclusion in the `wonChampionship` test (E+0x4C == 5, state +4 == 4).
- Constructor value of +0xB71C (irrelevant: the race set-up reset writes −1).

## 4. Proposed docs/re/symbols.tsv rows (address, name, note)

| Address | Name | Note |
|---|---|---|
| 0x00418090 | get_hasNoDamageObj | +0xDC |
| 0x00418170 | get_wonChampionship | +0xA8 (block +0x48) |
| 0x00418360 | get_terminalDamageCount | +0xA0 |
| 0x00418530 | get_isFinalRace | +0xCF |
| 0x00418560 | get_isFinalCareerEvent | +0xD2 |
| 0x00418590 | get_bonusObjWinPos | +0x88 |
| 0x004185A0 | get_driverChampWinPos | +0x14C (player record +0x40) |
| 0x00418800 | get_teamDriverChampWinPos | +0x434 (teammate record +0x40) |
| 0x00418D00 | get_1_2Finish | +0xB71C |
| 0x00418DA0 | get_numRaceWins | +0x9C |
| 0x00418DB0 | get_numEventWins | +0xB8, unsigned |
| 0x00418DD0 | get_racesCurTrack | +0xBC, unsigned |
| 0x00419010 | get_anyDamageIndOn | +0xE1 |
| 0x00419040 | get_teamToBeatNum | +0xA4 |
| 0x00468920 | car_record_init | record constructor, +0x40 = 0 |
| 0x00468B30 | race_block_clear | −1 at +0x28/+0x34/+0x38/+0x3C/+0x44/+0x48 |
| 0x004EF9C0 | champ_one_two | 1 when player and teammate are championship ranks 0 and 1 |
| 0x00670470 | champ_win_place | worst 0-based place that clinches the title (never reached) |
| 0x006706B0 | player_standing | the standings line of driver type 2 |
| 0x00637930 | event_is_head_to_head | `event.head_to_head` for own events |
| 0x00637B10 | event_tier_rank | tier's career rank index (0x62C280) |
| 0x0062C280 | career_rank_index | rookie/pro/expert/international → 0..3 |
| 0x0062A8D0 | offer_objective_place | `objective_finishing_place` − 1 or −1 |
| 0x00640800 | event_on_last_race | E vf 0x7C |
| 0x007F7CE0 | group_on_last_child | count > 0 and +0x34 == count − 1 |
| 0x00640020 | game_mode_object | mode index → mode object |
| 0x0063F320 | mode_current_event | mode vf 0x80 |
| 0x00670C50 | event_build_offer_objective | E+0x50 for kinds 1/2 |
| 0x0066E3A0 | objective_from_type | objective by type string |
| 0x0062CB80 | objective_place_no_damage | class 1 |
| 0x0062CBB0 | objective_finish | class 3 |
| 0x0062CBD0 | objective_no_damage | class 4 |
| 0x0062CBF0 | objective_beat_rival | class 2 |
| 0x006654E0 | objective_speed_test | class 5 |
| 0x00665520 | objective_time_trial | class 6 |
| 0x00640950 | standings_find | line of a driver |
| 0x006409E0 | standings_points_of_rank | points of the first line with a rank |
| 0x00640A10 | standings_count_rank | lines with a rank |
| 0x0046EC20 | race_history_track_item | find or add the item of a track id |
| 0x00610CB0 | race_history_on_race_end | records the player's race |
| 0x0060F950 | race_history_record_race | track count +8, terminal damage +0x54 |
| 0x006105F0 | race_history_record_result | race wins +0x44, track wins +0xC |
| 0x0060A960 | race_history_record_event | events +0x40, event wins +0x30 |
| 0x008EF920 | hud_any_damage_indicator | `indicators` for the HUD car |
| 0x00CE9EC0 | vtable: career event run (E) | interface at +0x38: 0xCE9EA8 |

Note: 0x0062CB50 is in symbols.tsv as `offer_objective_met`; it is the constructor of the class-0
(finishing place) objective (vtable 0xCE7C14, +4 = 0).
