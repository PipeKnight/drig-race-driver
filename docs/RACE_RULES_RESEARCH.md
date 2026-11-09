# Race-event rules of the original (single race)

15 October 2026, Claude Code. Static analysis only. The live original (PID 7208) was not attached.

Labels:
- **[V]** verified in decompiled code or in a disassembly of the hash-pinned analysis image;
- **[D]** read from shipped data;
- **[I]** interpretation.

## Sources and tools

- Analysis image `local/compatibility/grid-unpacked.exe` (SHA-256 `aac46a64…4514`, the image pinned by
  `tools/run_ghidra_address_refs.ps1`).
- Ghidra 12.1.4 headless, read-only, no auto-analysis. The shared `local/phase3/ghidra-project` was locked
  by another session, so the image was imported into a private project, `local/race-rules/ghidra-project`.
  - Functions are created on demand by `local/race-rules/ghidra/GridAiDecomp.java`, a copy of the AI
    research script.
  - Wrapper: `local/race-rules/ghidra/run.ps1`.
  - Pseudocode: `local/race-rules/decomp/r1`…`r14`.
- Private helper scripts in `local/race-rules/scripts/`:
  - `strings.py`, `xref.py` (raw immediate references), `calls.py` (E8/E9 callers), `disp.py` (field
    displacement uses), `dis.py` (Capstone 5.0.6), `rd.py` (vtables and constants).
- Shipped data, decoded privately:
  - `gameconfig/rules.xml` (SHA-256 `034ac330…`) via `binxml.py`, an independent decoder of the
    documented binary-XML format (docs/BINARY_XML.md): `local/race-rules/data/rules.decoded.txt`.
  - `database/schema.bin` (binary XML) and `database/database.bin` (SHA-256 `fb9943bc…`), decoded by
    `db.py` into `local/race-rules/data/schema.txt` and `database.json`:
    - each table is a byte index plus `LBT`, then a u32 row count;
    - each row is `ITM` plus the table index, then the fields in schema order;
    - field sizes: int/float/bool 4 bytes, `string size=N` padded to (N+5)&~3 bytes;
    - all 74 tables parse to exactly the end of the file [D].
  - `language/language_eng.lng` via `lng.py` (big-endian `SIDA` (id offset, text offset) pairs into the
    `SIDB` id and `LNGB` text blobs, 4,907 entries): `local/race-rules/language/eng.tsv`.
  - `ai/ai_choreographer.xml`: `local/race-rules/data/ai_choreographer.decoded.txt`.

Race-state values in the original are XOR-obfuscated with a per-run random key (0xE5AF88, created
lazily by 0xA6BBF0). The pseudocode below shows the decoded values.

## 1. Race manager and classification

The race manager sits at +0xF8 of the 0xF370-byte object built at 0x6799EC (constructor 0x911C90).
- It holds up to 20 entries of 0xC08 bytes at +0x68 (count +0x60, vtable 0xD0E898) [V].
- Entry fields, read through vtable getters:

| Getter | Field | Meaning |
|---|---|---|
| vf 2 (0x90EE50), set by vf 21 | +0x8 | position, 0 = leader [V] |
| vf 5 (0x90EF10) | +0x18 | laps completed [V] |
| vf 12 (0x9091A0) | +0x30/+0x34 | 64-bit race/finish time [V] |
| vf 18 (0x90D6E0) | int laps + float +0x70 | race progress: laps plus lap fraction [V] |
| vf 20 (0x90F750) | +0x74 | status: 0 racing, 1 finished [V]; 3 is excluded from gate processing by 0x913BE0 [V], meaning not traced |

The event lap count is stored XOR-obfuscated at manager +0x14 [V].

**Finishing [V]** (gate handler 0x912170):
- a line crossing is processed only while the car's laps < event laps;
- when the crossing makes laps == event laps (and the race type is not 3), 0x90EDB0 stores the finish
  time and sets status 1, and the entry is appended to the finish list (+0xF10C, count +0xF108);
- this holds for every car, lapped cars included.
- There is no rule that ends a car's race when the leader finishes [V].

**Race over [V]** (0x90E1B0, called every frame from the manager update 0x913AF0; when it returns true,
finalisation 0x911F30 runs). The race type at +0xF1B0 is set at 0x669B0D:

| Type | Used for | Race over when |
|---|---|---|
| 1 | single-player lap race | every entry has completed the event laps |
| 2 | not set at 0x669B0D | a car with controller type +0x6C == 0 (human [I]) has completed them |
| 3 | time-based formats | the time limit expires |
| 0 | network races, and game mode 4 | never by this test |

**Classification order [V]** (0x908CF0, rebuilt for all entries by 0x90B5B0, which then sets each
entry's position and per-class position for classes 0–5):
- finished entries come first, ordered by finish time;
- every other entry follows, ordered by race progress (laps + fraction), larger first;
- in a type-3 race finished entries are also ordered by progress.

**Finalisation [V]** (0x911F30):
- it runs once, on race over or from the frontend (0x583B00 and 0x585E00, which first sets the player
  vehicle's +0x2E64 = 3; 0x63E4E0 on leaving; 0x7028E0/0x70FC80 in network code);
- if a human car has not finished (status ≠ 1), a flag is set;
- then each entry's vf 24 (0x90FCD0) is called with the event laps and that flag.

0x90FCD0, for an entry still racing:
- human car (+0x6C == 0): the vehicle race state is set to 3, and nothing else happens [V];
- other cars, when speed > 5 mph or the flag is set:
  - speed is |v| from 0x9974D0, compared with 0xA7DDB0(5.0) = 5 × 1.609344 × 1000 / 3600 = 2.2352 m/s;
  - each remaining lap is predicted by the estimator's vf 6 (0x67FFB0), and the car's best lap and lap
    count are updated;
  - its time becomes the sum of its lap times [V].
- otherwise the vehicle race state is set to 3 [V].
- The estimator is described in "Lap estimator" below; **ported** (row 44, 9 November 2026).

### Lap estimator (0x67FFB0) [V unless marked]

The estimator is a "virtual race" object (constructor 0x67D590, vtable 0xCE87FC; the drift
VirtualRace of docs/DRIFT.md is the same class). 0x90FCD0 calls its vf 6 = vtable +0x18 = 0x67FFB0.

Layout:
- +4 vehicle count; +8 period = 1;
- +0x10/+0x14/+0x18 = base_time_factor, top_time_factor, performance_variance of the career
  difficulty row;
- +0x20 base lap time (i64 µs); +0x28 + 8i the sector times; +0x48 sector count = 4;
- +0x4C records {vehicle id, skill}; the race manager adds every car with skill 1.0 (0x916FE0);
- +0xAB9 loaded, +0xABA initialised.

The circuit base lap comes from 0x67D890: the first `world_record` row whose `track_model_id` is
the route's track model (seconds; no grade filter [I]), default −1.0 (0xCB0948) when no row exists.
Each sector is base × 0.25. Of 99 track models, 76 have a row [D]; none for haruna, detroit
Route_8/9, yokohama_docks Route_3–6 and the frontend models.

Finalisation 0x90FCD0 calls the estimator for the lap the car is on (record L with its time so far,
the previous lap, the lap fraction +0x70 and the "human unfinished" flag), then once per remaining
lap with fraction 0 and the lap just estimated as the previous one. The car's time is the sum of its
lap times; estimated laps count for the best lap.

0x67FFB0(L, prev, frac, flag), with fixed-point helpers 0x908470 (i64 × double, truncated),
0x908360 (add), 0x9083B0 (subtract) and 0x9082B0 (seconds → µs):
1. One RNG draw (0xA683B0, MT19937), discarded. C = sum of L's existing splits, k = their count.
2. Projected lap time T:
   - frac > 1e-4 (0xCBA1F0): T = L.total × (1/frac);
   - else with a previous lap: T = prev.total;
   - else: T = base + base × U(0, performance_variance).
3. With the flag set, T is replaced as in step 2's later branches when T < base/2, or when
   T > 2·base and frac < 0.1.
4. Splits:
   - if k ≥ n − 1, frac > 0.5 and T > C: push T − C;
   - else for i in k..n: S = prev.split(i); if S > 2·sector then S = 2·sector × U(0.5, 1); if S = 0
     then S = T / n; one discarded draw when i % period = 0; at i = k, S = max(S, L.total − C).
   - The last split becomes max(2 s, (T − Σ splits) + split[n − 1]).
5. L.total = T; L is marked valid.

Port (`grid_tools::virtual_race::LapEstimator`, `race::Standings::with_estimator`):
- AUTHORED: the original seeds the RNG from the clock; the port seeds it with 0x5eed per race, so
  results never depend on CPU or frame timing.
- AUTHORED: the difficulty row is career rank 2 at the AI star level (`flashback::star_level`).
- [I] The fraction is the share of the line length covered since the last line crossing (0 before
  the first), and the current lap's time is the time since that crossing.
- AUTHORED: the port keeps no sector splits of driven laps, so k = 0 and the previous driven lap has
  no splits. Lap totals do not depend on splits; only the splits and the RNG use do.
- On a route with no world_record row the base is −1 s, as in the original, so a fresh-lap estimate
  is negative. A car only gets a fresh lap when it is ended before its first line crossing with
  fraction ≤ 1e-4.
- Without an attached estimator (unit tests, point-to-point, touge tools) the AUTHORED mean-speed
  estimate remains. Le Mans `finalize_timed` is unchanged.
- Observation (F759, `grid-opponent-race ROOT CD01 BP01 12 6000 ai`, cut at 96 s in lap 2):
  every car was estimated, at 2:37.2–2:44.3, in a different order from the track. The leader
  (1.112 laps) came 10th: projected from 8.5 s over 11 % of the lap, its lap was 76 s; the car
  at 1.082 laps, from 5.5 s over 8 %, got 67 s. The standings' line crossings solved from the
  output (87.5 s and 90.5 s) reproduce both totals, so this is the original's T = time / fraction
  on a short fraction, not a port error [I: the original behaves the same; no capture].

**State 3 [I]** is "retired / did not finish".
- The retire path 0x585E00 writes the same value to the player vehicle.
- The network state names 0x6EC5A0 are stateFinished, stateTimedOut, stateTerminalDamage, stateRacing,
  stateRetired, stateDisconnected and stateDisqualified, but they are indexed differently, so the
  mapping is not one to one.

## 2. Lapped cars and blue flags

- [V] There is no blue flag. The flag system knows green_flag, yellow_flag, white_flag and
  chequered_flag (0xCE97B4–0xCE97D0, 0xD10A30).
- [D] rules.xml loads only green, yellow and chequered for single_race.
- [D] Lapping is handled by the AI choreographer:
  - `ai_choreographer.xml` `<manager … lapping_yield_distance="50" team_mate_yield_distance="25" …>`
    (parser 0x401AA0, strings 0xCAFB70/0xCAFB54);
  - AI behaviour belongs to the AI research sessions and is not ported here.
- [V] Lapped cars race on to the full distance (section 1).

## 3. rules.xml: marshal flags, not penalties

[D] `single_race` defines these rules:
- `race_start distance_to_line=20`;
- `serious_incident` (min angle 90, max speed 40, slow speed 3, 8 s between, 0.5 s to register, 10 s
  minimum race time);
- `end_race distance_to_line=250`;
- flags green (duration 30), yellow (10) and chequered (600).

[V] The parser (dispatch around 0x64D296) maps rule names to constructors:

| Rule | Constructor | Vtable |
|---|---|---|
| race_start | 0x949D30 | 0xD1091C |
| corner_cutting | 0x949D00 | 0xD10900 |
| serious_incident | 0x94A120 | 0xD109E8 |
| last_lap | 0x949D70 | 0xD10938 |
| end_race | 0x949DA0 | 0xD10954 |

Each rule's update emits a flag event to the flag system:
- **race_start** 0x94A1E0 [V]: once, after the race has started, emits green_flag when the leader
  (0x908F20(0) = the car whose position is 0) is within 20 m of the line.
- **last_lap** 0x94A930 [V]: white_flag when the leader has 2 laps left (event laps − completed) and is
  within distance_to_line of the line; default 250, from 0xCC3224.
  - [D] The rule is not in the shipped rules.xml, so it never fires in a single race.
- **end_race** 0x94AA70 [V]: chequered_flag once, when the leader has 1 lap left and is within 250 m of
  the line. **Ported** as `Standings::leader_flag`.
- **serious_incident** 0x94AC60 [V]: an accumulating timer gates 0x94A6E0/0x94A7D0, which emit
  yellow_flag. Described in "Flag system" below; **ported** (row 47, 9 November 2026).
- **corner_cutting** [V]: every method is a stub (load 0x949790 and update 0x9497C0 return 0; the
  enable setter 0x94B930 only stores a byte), and the rule is absent from rules.xml [D].
  - **There is no corner-cutting or track-limit penalty in a single race.**
- [D] Penalties exist only in other formats:
  - touge.xml: `minorPenaltyTimePenalty 2.0`, `majorPenaltyTimePenalty 10.0`;
  - the HUD `penalty_pill` (0x6B3380);
  - drift.xml `collisionPenalty 500`;
  - language entries `osd_minor`/`osd_major` and the touge penalty totals.

### Flag system [V unless marked]

Static RE of 9 November 2026 (row 47). Several functions are not function entries in the master
analysis and were read with `grid_re.py dis`.

**Rules system** (vtable 0xD109DC, update 0x94A4A0): +0x20 sums dt; when it reaches
`rule_reaction_time` (0.3 s) every enabled rule's update runs with the summed dt and the sum resets.

**serious_incident** (ctor 0x94A120, load 0x949A70). Defaults before the attributes are read: angle 45,
maximum_speed 10, slow_speed 1, time_between 30, time_to_register 1, race_time 5. rules.xml sets
90 / 40 / 3 / 8 / 0.5 / 10 (the attribute names are spelt `minumum_*`). The angle is in degrees and
converted at use; no other conversion.
- Incident test 0x949B40: `(angle > minimum_angle AND speed < maximum_speed) OR speed < slow_speed`.
  - angle 0x909F70: acos of the flattened, normalised car axis against the line vector of the car's
    tracker record (+0x40), 0..π [I: the heading against the line direction].
  - speed 0x90A250: the body velocity along the line, negative going backwards, so reversing is
    always an incident.
- Update 0x94AC60: only while race state +0x10 = 1 (started) and +0x11 = 0 [I: not ended]; +0x24 sums
  dt and the rule continues only past `minumum_race_time` (10 s).
- 0x94A6E0, per tracked car (up to 20 entries of 0x20 bytes): skip a car out of the race (entry
  vf+0x50 [I: finished or retired]) or without a tracker record; if the test holds, the entry's
  register time += dt and its incident position = the car's track position; otherwise the register
  time is 0.
- 0x94A7D0, when any car is in an incident: for each entry with register ≥ 0.5 s and at least 8 s since
  that car's last yellow (system clock 0x4B1E20; the first always passes), raise a global
  `yellow_flag` at the incident position. Several cars can each raise one.

**race_start** 0x94A1E0: once, after the start, when the leader's forward distance to the
start-finish line is ≤ 20 m, raise a global `green_flag` at the line. **end_race** 0x94AA70 raises
`chequered_flag` the same way at 250 m on the last lap.

**Flag system** (vtable 0xD10D6C: raiseFlag 0x94B440, registerVehicle 0x94AFA0):
- raiseFlag finds the type by name, takes a free pool instance (0x94A370) and activates it (0x949960:
  remaining = duration), then applyFlag 0x94B2A0 puts a global flag in every registered vehicle's list
  (0x94B130: no duplicates, at most `max_flags_per_vehicle` 12).
- `max_vehicles` is 1 [D]: one vehicle holds flags [I: the local player].
- Zone 0x90CB10: from `track_distance_before` behind the flag's point to `track_distance_after` past it;
  the whole line if their sum exceeds the line length. Zone test 0x94BA10: the zone length exceeds the
  distance from the car forward to the zone end.
- Instance tick 0x949E60: remaining −= dt; below 0 the instance is freed. 0x94B990 compares by
  priority; 0x94AD60 returns the highest in-zone flag (debug view 0x64CDA0).

| Flag | Priority | Duration (s) | Before (m) | After (m) |
|---|---|---|---|---|
| green_flag | 2 | 30 | 400 | 100 |
| yellow_flag | 3 | 10 | 100 | 20 |
| chequered_flag | 4 | 600 | 400 | 100 |

`single_demolition_race` has green and chequered only [D].

**What the player sees.**
- HUD 0x6BA460: the minimap node `object/flag=flag` (flag.tga) plays "in" while the viewed car is
  inside an active yellow_flag's zone and "out" otherwise. Green and chequered have no HUD element.
- 0x64BA60 posts game-broadcaster codes for the local car's flags (green 1/4, yellow 2/5, chequered
  3/6, on/off). Their receiver (speech or crowd) was not found, so they are not ported.
- No AI reaction to flags was found [I: the AI code was not searched for flag reads].

**Port** (`grid_tools::race_flags`): `RuleSet::load` reads rules.xml; `RaceFlags::step` runs the rules
every 0.3 s on the fixed simulation tick. Decisions:
- AUTHORED: the "time between" clock is the flag system's own summed simulation time instead of the
  wall clock, so results never depend on frame timing.
- AUTHORED: within a rule tick, active flags count down before new ones are raised.
- [I] Every race car is tracked by serious_incident (the caller of vf+8 was not traced); cars not
  racing in `Standings` are skipped; the player is the registered vehicle.
- [I] Format: `single_demolition_race` for a derby, `single_race` for every other lap race;
  point-to-point runs get no flags.
- On an open line the zone is a plain interval [I].
- The minimap flag before this port played for the chequered flag; it now follows the yellow zone.
- grid-app logs `drive_race_flag`; grid-opponent-race prints `flag NAME tick= car= position=`.

Open: the caller registering cars with serious_incident; flag.tga content; the zone sign check of
0x90BFC0/0x90BEC0; race state +0x11; vehicle byte +0x2E68 (0x98D620); 0x94AC50; receivers of the
broadcaster codes.

## 4. Start and false starts

- [D] No false-start or jump-start text exists in the binary strings or the English language table.
- [V] The only start rule is the green flag above.
- [D] The choreographer's `race_start_distance_1/2 = 100/750` and `race_start_factor_1/2 = 1.05/1.25`
  are AI start parameters.
- [I] GRID has no false-start rule; cars are released at GO. The port keeps every car on the handbrake
  until GO (AUTHORED, docs/RACE_MODE.md).

## 5. Grid order

F401 correction (docs/SERIES.md "Grid fill"): 0x5BE470 and 0x5C2AC0 below belong to the career grid
(0x5C7D40). The free race places the player by its grid option (0x5B60F0) and every other car in
entry order; `grid_order` is not read.

- [D] `grid_order` has a single row, `random`.
- [D] Free-race options are `event_options_grid_position_front/middle/back/random`. The owner's reference
  race used Random, 12 cars, start 5/12 (docs/REFERENCE_RACE.md, docs/DONINGTON_RACE_VIDEO_2026-10-09.md).
- [V] Player slot (0x5BE470):
  - b = min(player_grid_position_base, field) − 1;
  - r = min(player_grid_position_range, field − b);
  - the slot is drawn from [b, b + r).
- [V] Named drivers use the same formula with `event_driver_grid_position.grid_position_base/range`;
  the draw is 0xA683E0(0, r) and the driver is stored at that slot (0x5C2AC0).
  - [I] The draw's upper bound is exclusive.
- [D] Shipped event values (271 track pools):
  - most common player placements are (7, 19), (8, 19), (1, 19), (6, 19), (9, 19) and (7, 3);
  - 31 named-driver rows, mostly (1, 0–4).
- [I] The remaining cars fill the free slots randomly, per `grid_order random`; the fill was not traced.
- **Ported** as `race::grid_slot`. The app's `--grid-position N` keeps fixed slots. The free race's
  Front/Middle/Back/Random option (0x5B60F0) is `[drive] grid_position = front|middle|back|random`,
  chosen in the event menu (docs/SCREEN_FLOW.md §3).

## 6. Results screen fields

- [D] `post_race_positions_table_*` columns: Pos, Name, Team, Vehicle, Class (Le Mans), Time, Laps,
  Distance, Points / Penalty, Score.
- [V] The column layout is built in 0x52E2C0 and 0x51C930. How each cell is formatted (gap or absolute
  time, laps down) is data-driven and not traced.
- [D] Status words:
  - online results: `net_race_results_terminal_damage` "Wrecked", `terminal_retired` "Retired",
    `timed_out` "DNF", `disqualified` "Disqualified";
  - `RETIRED` "Retired" and `net_dnf` "DNF".
- [I] Laps down: since every finisher covers the full distance (section 1), no finisher is a lap down.
  Cars out of the race are classified by distance (section 1), and the port shows their completed laps.

## 7. Damage and retirement

- [V] Terminal damage is logged by 0x5EDA60 as "Human/Non human vehicle %d terminal damage due to %s".
  Causes are wheel (%d), "engine failure" and "gear box failure". It is broadcast as
  `TerminalDamageMessage` (0x44C670, 0x6006E0, 0x6EBDF0, 0x70B030).
- [D] For the player, the result is a "Race Over" / "WRECKED" screen ("Your car is severely damaged and
  cannot complete the race") with the options Restart Race, Retire, Instant Replay, Spectate and Continue
  to Results.
- [V] Retiring (0x585E00) sets the player state 3 and finalises the race (section 1), so the unfinished
  human makes every AI car still racing be estimated.
- [V, docs/AI_AVOID_RECOVERY_RESEARCH.md] AI cars retire through the AI `retire` behaviour:
  - damage above `damage_retirement_ratio`, or stuck for 20 s;
  - if they stand still when the race is finalised, they get state 3 (section 1).
- [V, docs/RESET_AND_RESPAWN_RESEARCH.md §9] Terminal damage (AI car state +0xE3) moves the AI to
  RacingStateRetire; its retire behaviour (score 23) brakes with no steer and requests removal below
  0.5 m/s. The entry stays in the race until finalisation.
- Port (F755): the AI stops the same way; its removal request then retires it in the standings
  (AUTHORED: earlier than the original, so a field without a human ends by itself). Before F755 the
  port retired a terminally damaged AI on the spot.

## Port (F308)

`grid_tools::race::Standings`:
- status Racing / Finished / Estimated / Out(Stopped | Wrecked | Retired);
- order: finished by time, then estimated, then everyone else by distance;
- `finalize(speeds, time, humans)` with the 5 mph rule and the unfinished-human rule;
- `over()`: finalised, or no car still racing;
- `leader_flag()` (chequered);
- marshal flags in `race_flags` (section 3);
- `grid_slot`.

Finishing (F671): every car with lap timing finishes on the lap-timing line crossing that completes
the event laps (`Standings::line_crossing`, as 0x912170), the player through its trial; the
racing-line distance finishes only a car without lap timing. Before F671 AI cars finished on that
distance, measured from gate 0's midpoint: on Okutama route 2 (gate 0 53 m wide and slanted) it ran
out 0.47 m before the car reached the gate, so the race ended with the second lap unfinished.

The race progress measure (distance along the AI line) and the remaining-distance estimate (the car's
mean race speed so far) are AUTHORED. The app and headless race use it as described in
docs/RACE_MODE.md.

## Open

- Meaning of entry status 3 and of the vehicle race-state enum.
- Results cell formatting.
- Free-race front/middle/back mapping.
- serious_incident: which vehicles are registered; receivers of the flag broadcaster codes.
- What makes the human's race type 1 end when an AI car has retired: the frontend finalisation
  0x583B00 [I].
