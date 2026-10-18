# Point-to-point routes: Haruna touge and the Yokohama freestyle arenas (F407)

Status: 2026-10-18, Claude Code. Follow-up of F399 ([TRACK_READERS.md](TRACK_READERS.md)
§5). Haruna's touge roads now load and drive from start to finish, solo and
two-car, headless and in the app. The four Yokohama freestyle arenas load and
run a time-limited Freestyle drift in the app. Every circuit is unchanged: 78 of
the 92 survey routes give byte-identical output; the other 14 failed to load
before.

Evidence (ignored, this worktree's `local/p2p/`):
- `progress/`: decoded progress files;
- `decomp/d1..d10`: Ghidra 12.1.4 headless, read-only, on the drift-ai project copy of `local/compatibility/grid-unpacked.exe`;
- `scan-23ec.txt`: Capstone 5.0.6 scan;
- `runs/`, `survey-after/`, `soak-haruna/`, `app/`.

Nothing was installed, and the original game was not started.

Tags: [V] observed (files, static analysis, runs); [I] interpretation;
AUTHORED: a port choice with no original counterpart found.

## 1. Data [V]

### Progress files

`progress_track.xml` has `track type="point_to_point"` on all ten Haruna routes and on Yokohama routes 4–6.

**Haruna.** Each route covers the whole mountain road (8.8–9.3 km) with two progress routes:
- `forwards`: `start`, two `time` splits, then one `finish` (two on route 0);
- `reverse`: the same gates with start and finish swapped.

Example, route 1: forwards start at gate 83, splits at 139 and 202, finish at 231; reverse start at 231, finish at 83. The last progress point lies exactly at `total_distance` (the end of the road).

**Yokohama arenas.** Routes 3–6 have placeholder progress with 3 gates. Route 3 is a `circuit` with a `start` split; on routes 4 and 6 the finish is gate 0 and the start is gate 2. Milan route 3 has a `speed` split.

### AI files

On Haruna, `ai_track.xml` and `ai_vehicle_track.xml` each hold two tracks, `default` and `reverse`:
- `default`: 450 gates, 449 links;
- `reverse`: 445 gates, 444 links.

Both are open chains, not loops. The vehicle tracks list only the classes `streetgt`, `promuscle`, `muscle` and `clubgt`. The track's own brake data includes lines with min, max and drift speed all 0.

The arenas' AI files have no racing line ("AI waypoint absent").

### Grids

`grids.pssg` names a group `grid_start_standing_subroute_<N>_<id>` per direction, with the slots `slot_00_<id>` and `slot_01_<id>` (two cars). The drift intro group `grid_drift_intro_*` holds `slot_00_dft` and `slot_01_dft`. Exceptions:
- route 0 has only subroute 0;
- route 9's subroute-0 group has no slots, so only reverse is drivable.

Forward grids stand 3.6 m past their start gate; reverse grids stand 9.3 m behind it. The arenas use plain `slot_00`..`slot_11`.

## 2. The original's selection [V unless marked]

- **Subroute.** At a race (re)start, 0x65B2C0:
  - selects `subroute_0` (string 0xCEA054) through 0x936B50;
  - then, unless `[race+0x23EC]+8 == 2`, selects `subroute_%d` (0xCEA048), with 1 when `[race+0x23EC]+4` is 1 or 2, else 0.
- **Grid groups.** 0x916BF0 parses a grid group's name: a start-type table, then `atoi` of the text after `subroute_` (0xD0E998) into +0x34.
- **Progress direction.** 0x915DE0 reads a progress route's `direction`: +0x19 is set unless it is `reverse` (0xCB00F8).
- **Touge state.** `[race+0x23EC]` is a 0xF0-byte object (vtable 0xCEA328, constructor 0x668930: `+4 = 0`, `+8 = 2`, two 0x50-byte records).
  - Its setter 0x631080 `set_state(s)` writes `+8 = s`. For `s == 1` it seeds the generator and sets `+4 = 3 - (rand(0..10) > 5)`, so 2 or 3; otherwise `+4 = 0`.
  - The race setup jump table at 0x5F86FE, indexed by format − 2, calls:
    - `set_state(0)` for format 5 (`pro_touge`);
    - `set_state(1)` for format 6 (`midnight_touge`);
    - `set_state(2)` (off) from 0x5ECE20.
- **Meaning of `+4`.** `+4` in {2, 3} selects the Haruna drift intro and results scenes (`haruna_drift_intro_reveal`, `drift_results_intro_haruna`, 0x5A2840/0x5A2978). With the subroute rule above:
  - a midnight touge runs in a random direction (2 → reverse, 3 → forwards) [I];
  - a pro touge starts forwards.
- **Freestyle.** For format 4 (`drift_freestyle`), case 0x5F8705 sets byte `[race+0x23E4]+0x1D0 = 1`; 0x5F84E0 clears it for every event.
  - The 0x23E4 object is the drift manager: 0x668720 constructs its settings at +8 with 0x6683D0, the drift-settings constructor with the rating table 500…10000.
  - So the byte is settings +0x1C8, `ignore_progress`. DISCIPLINES.md had this as [I]; it is now [V].
- **Brake data.** 0x792C80 takes the class brake data, or the brake line's own data at +0x10 when the class is absent. It does not treat zero speeds specially.
- **Not found.** Where a pro touge's second round sets `+4 = 1` (and so the reverse subroute) was not found. Pattern scans for stores to `[obj+4]` after loads of `[race+0x23EC]` found only the reset 0x63C710 (`+4 = 0` in state 0). F412 shows that `+4` indexes the per-run penalty counters (§7), so `+4` is the run and run 2 is the reverse subroute; the store itself is still not found.

## 3. Port

### Reader and lap timing

- `grid_formats::progress`:
  - `TrackKind::{Circuit, PointToPoint}`;
  - `Route::reverse`;
  - split kinds `Start` and `Speed`;
  - an open road's points may reach `total_distance`.
- `grid_world::lap_timing::Course::open`: gate 0 is the start and the last gate the finish; nothing wraps, and the finish faces away from the gate before it.
  - `State::advance` uses a separate path for open courses, so circuit timing is untouched.
  - AUTHORED: a car that starts past gate 0 (Haruna's forward grids) starts its run on its first forward movement. The original's standing-start timer was not traced. In the app this is the moment after GO (`start when=5.168` after the 5 s countdown).
- `grid_tools::route::Route::with_subroute(0|1)` and `ai_track()` (`default`/`reverse` [I]); `HARUNA` = route 1 (`haruna_ill_mid_bot`, 5.86 km).
- `lap_progress::load_route` builds the subroute's run (`run`):
  - the start becomes gate 0 and the first finish in the run's direction the last gate (AUTHORED where there are two finishes);
  - gates and points are put in travel order and measured from the start;
  - time splits become sectors.
  - Route 1: 149 gates, 5859.21 m, sectors at 56/119 (forwards) and 29/92 (reverse).

### AI and grid

- `ai_driver::Line`:
  - open chains (n − 1 links, a single head);
  - arc lengths beyond the ends continue along the end segments (AUTHORED look-ahead past a road's end);
  - projection never uses a closing segment.
- `AiData::load_route` / `load_named`, `Line::load_route`; `ai::track_named`, `class_brake_lines_on`, `class_names_on`.
- AUTHORED brake fallback: when the car's class is absent and the route's own brake data has a zero-speed line, the route's first class is used (Haruna: `streetgt`). Without it the uphill touring car braked to a stop at such a line. With `GRID_AI_CLASS=streetgt` the same run took 143.641 s, the same as the fallback. No circuit route meets the condition: the survey outputs are unchanged.
- `grid::read_slot(tree, slot, subroute)`: the slot itself when it exists (every circuit), else `slot_NN_<id>` of subroute N's standing group.

### Race, inspector bounds and app

- `race::Standings::new_run`: one run, with the target being the line distance from the start to the finish gate.
- Inspector bounds: `MAX_STATIC_SHAPES` 4096. The packed world, vehicle world and wheel contacts allowed 512 static shapes; Haruna has 532 road chunks. RUNTIME_LIMITS_AUDIT records the original's pools as 11,500 geoms and 10,000 fixed objects.
- Breakables whose pieces the port cannot break are kept static (unported) instead of failing the route. These are pieces with several shapes, static, sphericalised or fixed-mass pieces, and spheres; Haruna has them.
- grid-opponent-race: `--subroute`, the subroute's AI track (`GRID_AI_CLASS` too), run standings with one run, `GRID_TRACE_LAPS=1`.
- grid-app:
  - `[game] subroute` / `--subroute`;
  - a point-to-point race is one run;
  - run standings, AI data and reset line from the subroute;
  - entity draws without a diffuse binding (Haruna `flag_a_01`, 4 draws) are omitted on non-default routes; Donington stays strict.

### Freestyle (arenas)

- `discipline = drift_freestyle` (no opponents) and `[drive] time_limit` (default 90 s [D career]).
- `Route::ignoring_progress`: no lap timing (`drive_lap_timing enabled=false reason=progress-ignored`) and no reset line where the route has no AI line.
- `drift::Settings::ignore_progress` set [V].
- `time_trial::State::with_time_limit`: the event finishes at the limit, then banks and shows the drift post-race screens like a Drift GP.

## 4. Results [V]

Measured before rebasing onto F406 (99a63a2, the collision floor lift), except where marked "after F406" (`runs-f407/`).

### Solo AI

`grid-opponent-race --route tracks/japan/haruna/route_N [--subroute S] ROOT CD01 BP01 1 40000 ai`; `runs/haruna*-s*.txt`. Each run: 1 valid run of 1 (seconds). After F406, route 1: 151.751 / 143.641 s.

| Route | Forwards (subroute 0) | Reverse (subroute 1) |
|---|---|---|
| 0 | 98.42 | no reverse grid |
| 1 | 151.80 | 143.64 |
| 2 | 151.36 | 143.30 |
| 3 | 189.57 | 186.00 |
| 4 | 189.65 | 183.22 |
| 5 | 91.75 | 87.93 |
| 6 | 91.94 | 89.43 |
| 7 | 60.66 | 57.20 |
| 8 | 60.72 | 57.63 |
| 9 | no forward slots | 233.47 |

### Two cars (DF01 and the characteristics)

After F406 all four races classify both cars:

| Route | Forwards | Reverse |
|---|---|---|
| 1 | 2:32.272, +1.696 | 2:28.320, +10.080 |
| 4 | 3:05.232, +2.112 | 2:59.856, +8.864 |

Before F406, route 1 forwards ended with car 1 stopped after its first sector at (875.9, −803.4) and classified DNF at the tick limit. Both cars' runs had started (`Started` 0.16 s).

Cars that finish drive on past the end of the road; terminal damage there happens after their finish.

### App

- Windowed race (`GRID_AUTOPILOT=1 --route tracks/japan/haruna/route_1 --drive --time-trial-laps 1 --opponents 1 --drive-ticks 12000`; CD01/BP01/DF01 and the characteristics; `app/haruna1-race.log`):
  - lap timing 149 gates, 4 required;
  - 5 s countdown; start 5.168 s; sectors 54.523 / 67.790 / 31.428;
  - run valid 2:33.741;
  - classified: AI 1 2:32.544, player +1.357.
- Static start render (`app/haruna1-start.png`): the mountain road with concrete barriers and armco. The preview car sits low in the road, because the preview uses the marker height ("horizontal preview only").
- Freestyle, Yokohama route 3 (`app/freestyle3*.log`, 20 s limit, BMW): lap timing off, 2.0 s countdown, finish at the limit (tick 1438), banked, post-race stats. Routes 4, 5 and 6 load and run the same flow (5 s limit).
- **Not verified:** a non-zero freestyle score in the app. The scripted constant inputs ran into the arena barriers below drift speed. The scorer's ignore-progress path is unit-tested (`drift::tests::a_drift_without_race_progress_scores_only_when_progress_is_ignored`).

### Soak and survey

- `grid-soak quick --seeds 29..32` (Haruna, 1/2/1/2 cars): 3/4 pass.
  - Seed 29, solo chaos driver: the car left the road over an edge with no boundary wall and fell (no contacts), ending 50 m below the road at tick 1423.
  - [I] The original ships `route_N/resetlines.cqtc`, presumably its off-road reset lines; not read.
  - Seeds 29..32 stay outside the default set.
- Survey (`survey-after/`): 78 of 92 routes byte-identical to F399's. The 14 that differ are Haruna 0–9, Yokohama 3–6 and Milan 3 (which now laps, 101.93 / 99.60 s); all failed before.

## 5. Launchers and tests

### Launchers

- `Play GRID Touge Haruna.cmd` (`settings/touge-haruna.ini`): route 1 downhill, one AI opponent, one run.
- `Play GRID Drift Freestyle.cmd` (`settings/drift-freestyle.ini`): arena route 3, 90 s, BMW.

### Tests

- `lap_timing`:
  - `an_open_run_starts_at_gate_zero_and_ends_at_the_last_gate`;
  - `a_car_starting_past_the_start_gate_starts_on_its_first_move`;
  - `an_open_run_missing_its_sector_gate_is_not_finished`.
- `time_trial::time_limit_tests`.
- `drift::tests::a_drift_without_race_progress_scores_only_when_progress_is_ignored`.
- `route::tests::haruna_runs_both_ways_from_their_own_grids`: progress, open AI lines, two-car grids, and that the reverse run starts where the forward run ends.
- The soak seed test.

## 6. Open

- Pro Touge (F412, §7): the alongside fault geometry, the round sequence and decision state, the penalty HUD pills. Midnight Touge: traffic and the random direction (state 1) in the app.
- `resetlines.cqtc`: falls off Haruna's edges.
- Route 9 forward grid; AI behaviour at the road end.
- The app race and freestyle runs were measured before F406 and not rerun.
- Freestyle HUD and timer display, and a scored arena run in the app; the arenas have no player auto-reset (no AI line).
- `ai_pacing` catch-up still measures gaps on a loop of the open line's length [I: only with pacing on].

## 7. Pro Touge (F412)

Two runs against one AI car with collision penalties and a result by total time.

### Original [V unless marked]

Static analysis, read-only (Ghidra 12.1.4 headless, Capstone 5.0.6); evidence in this worktree's `local/touge/decomp/`.

- **Settings.** `gameconfig/touge.xml` (`TougeSettings`) is read by 0x630FD0 into the touge state at +0xB0:
  - `minorPenaltyTimePenalty` +0xB8 (2.0);
  - `majorPenaltyTimePenalty` +0xBC (10.0);
  - `ignoreCollisionTime` +0xC0 (2.0);
  - `alongsideLengthOffset` +0xC4 (0.05);
  - `alongsideThresholdTweakMax` +0xC8 (0.2).
- **Car records.** The touge state holds two 0x50-byte car records:
  - +0: the car;
  - +0x31: the car's applied-damage byte;
  - +0x38: a double, the penalty window;
  - +0x40: penalty pending;
  - +0x41: major;
  - +0x44: the other car's record;
  - +0x48: the touge state;
  - minor/major counters at +0x1C/+0x20 + 12 × run.
- **Contacts.** A per-car listener 0x644520 copies the car's +0xBA5 into record +0x31. +0xBA5 is the byte the impact handler 0x9EF360 stores from the damage dispatcher 0x9E6660 (BODY_IMPACT_HANDLER.md). The listener then runs 0x6441B0 when the contact is with the other touge car.
  - Past the alongside geometry gate (0x63C940 on both cars, using `alongsideLengthOffset` and `alongsideThresholdTweakMax`), the contact is major (0x631240) when either record's +0x31 is set, else minor (0x631210).
  - 0x631210: when no window is open, the window becomes `ignoreCollisionTime` and a minor penalty is pending.
  - 0x631240: the same with major, and it also upgrades a pending minor penalty in an open window.
- **Counting.** Each tick in a pro touge (state +8 == 0), 0x64E1E0 counts the window down. When it falls below zero, a pending penalty is counted once in the current run's minor or major counter, provided the car has not finished (+0x2E64 [I]). The minor/major HUD calls 0x6B3500/0x6B3630 show the seconds (`osd_minor`/`osd_major`).
- **Penalty time.** A run's penalty time is `minor × 2.0 + major × 10.0` (0x63C830, called for both cars by 0x644180).
- **Strings [D].** `touge_round_one_results_table_title` "Round 1", `touge_round_two_results_table_title` "Round 2", `touge_race_results_table_title` "Total Times", `post_race_stats_touge_round_1_info`/`_2_info` "Run 1/2"/"Run 2/2", `post_race_stats_touge_penalty_time` "Penalty Total: [AMOUNT]", `post_race_stats_touge_time_difference_title`.

### Port

- **Rules** (`grid_tools::touge`): `Settings::load`, `Penalties::{contact, tick, seconds}` as above, `classify` by total time.
- **AUTHORED:**
  - Who takes a penalty. The alongside gate is not ported: the car behind on race progress is penalised, both cars when closer than 4.5 m.
  - Run 2 swaps the grid slots [I].
  - Cars without both runs rank last.
- **Headless:** `touge::run_pro_touge` and the probe `grid-pro-touge ROOT CD01 BP01 [ROUTE] [CARS] [TICKS]`.
- **App:** `discipline = pro_touge` with one opponent (`settings/pro-touge.ini`, `Play GRID Pro Touge.cmd`).
  - The race block builds run 2 at start-up: the reverse subroute, the player in the other slot.
  - `touge_event` counts the penalties. After run 1 it waits for both cars, up to 30 s after the player's finish (AUTHORED), then swaps in run 2 with a new countdown.
  - The results screen opens only after run 2, ordered by total. Columns: Total Times in the time column, Round 1 and Round 2 (run time plus penalty) in the last two.
  - Restart Race restarts the current run (AUTHORED).

### Results [V]

**Headless,** `grid-pro-touge` with DF01 and the characteristics (`local/touge/runs/`):

| Route | Winner | Round 1 | Round 2 | Total | Other car |
|---|---|---|---|---|---|
| 1 | car 0 | 2:32.272 | 2:26.864 | 4:59.136 | car 1, 5:02.208 |
| 4 | car 1 | 3:07.344 | 2:59.888 | 6:07.232 | car 0, 6:30.288 (round 2 3:25.056) |

There was no car contact in either event, so no penalties.

**App,** windowed autopilot on route 1 with 1 AI (`local/touge/app/pro-touge.log`):
- Run 1: player 2:35.593, AI 2:33.440.
- `drive_touge event=run_end`, then `run_start run=2 subroute=1 slots=swapped` and a new countdown.
- Run 2: AI 2:25.664, player +1.495.
- The post-race stats opened only in segment 1 (after run 2).

**Not verified:** the penalty path in a real contact (unit-tested only), and the results table on screen (unit-tested view).

**F421:** the AI now loads the `pro_touge` section of `ai_driver_config.xml` [D] (`raGenerateRandomOffsets`, `avBufferZone` 5). Headless route 1 with it: car 0 5:12.064 (2:38.672 + 2:33.392), car 1 5:13.952; no contact.

