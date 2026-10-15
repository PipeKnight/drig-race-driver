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
- The estimator 0x67FFB0:
  - [V] looks the car up in a per-vehicle table (+0x4C, 8-byte records) and uses the RNG 0xA683B0;
  - [V] has a speed branch (≤ the constant at 0xCBA1F0);
  - [I] the lap reference is an AI lap-time model;
  - the estimator itself is not ported.

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
  yellow_flag.
  - [I] The thresholds describe a spun or slow car; incident detection was not decompiled further.
- **corner_cutting** [V]: every method is a stub (load 0x949790 and update 0x9497C0 return 0; the
  enable setter 0x94B930 only stores a byte), and the rule is absent from rules.xml [D].
  - **There is no corner-cutting or track-limit penalty in a single race.**
- [D] Penalties exist only in other formats:
  - touge.xml: `minorPenaltyTimePenalty 2.0`, `majorPenaltyTimePenalty 10.0`;
  - the HUD `penalty_pill` (0x6B3380);
  - drift.xml `collisionPenalty 500`;
  - language entries `osd_minor`/`osd_major` and the touge penalty totals.

## 4. Start and false starts

- [D] No false-start or jump-start text exists in the binary strings or the English language table.
- [V] The only start rule is the green flag above.
- [D] The choreographer's `race_start_distance_1/2 = 100/750` and `race_start_factor_1/2 = 1.05/1.25`
  are AI start parameters.
- [I] GRID has no false-start rule; cars are released at GO. The port keeps every car on the handbrake
  until GO (AUTHORED, docs/RACE_MODE.md).

## 5. Grid order

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
- **Ported** as `race::grid_slot`. The app's `--grid-position N` keeps fixed slots; a random option is
  not wired yet.

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
- [I] Terminal damage therefore means DNF / retired. The port retires an AI car with terminal damage
  immediately (AUTHORED shortcut for the retire behaviour).

## Port (F308)

`grid_tools::race::Standings`:
- status Racing / Finished / Estimated / Out(Stopped | Wrecked | Retired);
- order: finished by time, then estimated, then everyone else by distance;
- `finalize(speeds, time, humans)` with the 5 mph rule and the unfinished-human rule;
- `over()`: finalised, or no car still racing;
- `leader_flag()` (chequered);
- `grid_slot`.

The race progress measure (distance along the AI line) and the remaining-distance estimate (the car's
mean race speed so far) are AUTHORED. The app and headless race use it as described in
docs/RACE_MODE.md.

## Open

- Estimator 0x67FFB0 (per-vehicle reference table, RNG).
- Meaning of entry status 3 and of the vehicle race-state enum.
- Results cell formatting.
- Free-race front/middle/back mapping.
- serious_incident detection 0x94A6E0.
- What makes the human's race type 1 end when an AI car has retired: the frontend finalisation
  0x583B00 [I].
