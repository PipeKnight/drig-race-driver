# Race disciplines of the original

17 October 2026, Claude Code. Inventory of every event type of Race Driver: GRID, the port of the
first one the engine did not have (Drift Battle, with the drift scoring shared by all drift
formats), and the order for the rest. 18 October 2026: the result estimator (§4.2, F386), the road
contact question (§4.3, F387), Drift GP (§7, F388) and an AI drift quality measurement (§4.5, F389).

Labels:
- **[V]** verified in a decompilation or disassembly of the hash-pinned analysis image
  (`local/compatibility/grid-unpacked.exe`, the image pinned by `tools/run_ghidra_address_refs.ps1`);
- **[D]** read from shipped data;
- **[I]** interpretation;
- **AUTHORED** a choice of this port, not the original's.

## Sources and tools

- Shipped data, decoded privately with the independent decoders of docs/RACE_RULES_RESEARCH.md
  (`binxml.py`, `db.py`, `lng.py`; outputs in the main checkout's `local/race-rules/data/` and
  `local/race-rules/language/eng.tsv`):
  - `database/database.bin` + `schema.bin`: tables `competition_format`, `discipline`,
    `free_race_discipline`, `free_race_times`, `free_race_track_pool`, `event`,
    `event_track_pool`, `track`, `track_model`, `track_type`, `points`, `difficulty`;
  - `gameconfig/drift.xml`, `demolition_derby.xml`, `touge.xml`, `rules.xml`;
  - `system/flow.bin` and `system/states.bin` (decoded in `local/pause-menu/`): post-race flows and
    screens per discipline.
- New private inventory script `local/disciplines/inventory.py` → `local/disciplines/inventory.txt`
  (every format with its events, routes, laps, time limits, grid ranges and base results).
- grid.exe: Ghidra 12.1.4 headless, `-readOnly -noanalysis`, on a private copy of the race-rules
  project (`local/disciplines/ghidra-project`, wrapper `local/disciplines/ghidra/run.ps1`,
  pseudocode `local/disciplines/decomp/d1`…`d12`); Capstone 5.0.6 (`local/tools/python`) through
  `local/disciplines/scripts/dis.py`; `consts.py` reads float constants. Strings and references with
  the race-rules scripts `strings.py`, `xref.py`, `calls.py`. No tool was installed.
- Headless route checks: `grid-opponent-race` on the drift routes (`local/disciplines/runs/`).

## 1. Inventory

### 1.1 Competition formats

The rules of an event come from its **competition format**. `database.bin` `competition_format`
[D] and the format name table at 0xDB39CC [V] (index = the engine's format enum):

| Enum | DB id | Format | `time_based` | AI base difficulty |
|---|---|---|---|---|
| 0 | 1 | single_race | 1 | 0 |
| 1 | 2 | single_demolition_race | 1 | 0 |
| 2 | 3 | drift_battle | 0 | 0 |
| 3 | 4 | drift_grand_prix | 0 | 0 |
| 4 | 5 | drift_freestyle | 0 | 0 |
| 5 | 6 | pro_touge | 1 | 0.75 |
| 6 | 7 | midnight_touge | 1 | 0.75 |
| – | 8 | endurance | 1 | 0 |
| – | 9 | head_to_head | 0 | 0.75 |

The table continues with `test_offer_time_trial` and `test_offer_speed_test` (driver offers, below).
`time_based` = 1 for the formats classified by time, 0 for the score formats [I].

### 1.2 Disciplines are car series

`discipline` [D] (18 rows) names the series of an event and links it to a format: Touring_Car,
Pro_Tuned, open_wheel, GT1, GT2, Club_GT, Prestige, Pro_Muscle, Classic_Muscle, Prototype and
Endurance all use **single_race**; Stock_Car uses single_demolition_race; Drift_Battle, Drift_GP,
Downhill_Drift and Free_Drift the drift formats; Pro_Touge and Midnight_Touge the touge formats.

So **Touring, GT, Pro Tuned, Open Wheel, Muscle, Prototype and the Le Mans Series are circuit
races with different cars** — the engine's race already covers their rules.

**There is no Time Attack format** in GRID [D, V]: no table, string or format names one. The
closest are the driver-offer objectives `time_trial` and `top_speed` (`driver_offer_decision_maker`
in flow.bin, stats screens `post_race_stats_do_timetrial` / `_do_speedtest` [D]) and the test drive.

### 1.3 Track types

`track_type` [D]: circuit, point_to_point (Haruna touge roads), freestyle (four Yokohama drift
arenas, length 0), demolition (two Detroit stadium routes). `track_model.drift_off_track` marks the
Japanese and some Washington routes [D].

### 1.4 Per discipline

Our four circuits are Donington, Nürburgring, Istanbul Park and Long Beach (route_0 GP layouts so
far); our cars the Chevrolet Lacetti (FWD) and BMW 320si (RWD).

#### Circuit race (single_race): Touring, GT1/GT2, Club GT, Prestige, Pro Tuned, Open Wheel, Muscle, Prototype, Le Mans Series

- Rules [V/D, docs/RACE_RULES_RESEARCH.md]: laps (2–8 per `event_track_pool.num_laps`), standing
  start on the grid, start lights; player slot from `player_grid_position_base/range`; no false start,
  no blue flag, no track-limit penalty; flags green/yellow/chequered from rules.xml `single_race`.
- Scoring: finish order by time; career points tables `points` (eur_rookie 10-8-7-…, US_ARL_Pro
  160-140-…, Japan_GT_J1 50-45-…) [D].
- HUD: docs/RACE_HUD.md (dial, positions, lap group, minimap). Results `post_race_results_generic`.
- **Engine: supported** (F308, F336, F353; circuits F304/F350/F354; cars F338). Not yet: random
  grid fill, career points column, open-wheel/GT/prototype cars, other circuits.

#### Le Mans 24 Hours (endurance)

- Data [D]: career events gt2/gt1/lmp2/lmp1_leman24 are **single_race events with
  `time_limit_seconds = 720` and 8 laps** on le_mans/Route_0 (Circuit de la Sarthe, 13.7 km,
  `supports_time_of_day = 1`); free race `Le_Mans` offers 12/24/60 minutes and 2/4/6/12/24 hours
  (`free_race_times`); the `endurance` format exists but no event uses it; `lemans_restrictions`
  (453 rows) builds the multi-class field; `vehicle_class.lemans_progress_adjustment` and
  `lemans_sort_order`; results `post_race_results_lemans` with a Class column [D].
- Rules: race type 3 — the race is over when the time limit expires [V, RACE_RULES_RESEARCH.md §1];
  classification of time races by progress [V]; day/night cycle.
- **Engine: not supported** (no Le Mans track, no LMP/GT cars, no time-limited race, no class
  results, no time of day).

#### Demolition Derby (single_demolition_race)

- Data [D]: event Western_states_D (Michigan Demolition Derby), Stock_Car, Detroit stadium long/short
  (demolition track type), 6 laps, base results 25/30; rules.xml `single_demolition_race`: only
  race_start (green) and end_race (chequered), no serious-incident yellow flags;
  demolition_derby.xml: `flippedCountdown 5.0`, `flippedVelocityThreshold 1.0`,
  `wrongWayThreshold 50.0`, `warningThreshold 0.5`; OSD `osd_demolition_dis` "You will be
  disqualified!"; results `post_race_results_dd`.
- Rules [I]: a lap race in an arena where contact is the point; a car flipped for 5 s or driving the
  wrong way is warned and disqualified. Parser 0x630BC0/0x668846 [V] (not traced further).
- **Engine: not supported** (no stadium route, no stock car, no flipped/wrong-way disqualification).

#### Touge (pro_touge, midnight_touge)

- Data [D]: Haruna point-to-point routes (legal_touge/illegal_touge free race pools, 2 cars), base
  results 49–200 (seconds [I]); touge.xml `minorPenaltyTimePenalty 2.0`, `majorPenaltyTimePenalty
  10.0`, `ignoreCollisionTime 2.0`, `alongsideLengthOffset 0.05`, `alongsideThresholdTweakMax 0.2`;
  results Round 1, Round 2, Total Times; stats `touge_time_difference`, "Penalty Total", "Distance
  Diff"; `StateIngameTougeRoundDecisionMaker`; HUD penalty pills `osd_minor`/`osd_major`.
- Rules [I]: pro touge = two runs, total time with collision penalties; midnight touge = a single
  head-to-head run with traffic (`touge_traffic`).
- **Engine: not supported** (no Haruna, no point-to-point timing, no penalties, no traffic).

#### Drift (drift_battle, drift_grand_prix, drift_freestyle)

- Data [D]: drift.xml (section 2); Drift Battle and Drift GP free-race pools on the drift layouts of
  circuits, among them **Long Beach route_1 (longbeach_cl), Nürburgring route_1 (nurburgring_spr)
  and Istanbul Park route_1 (istanbul_int)** and their reverse routes — up to 8 cars in a battle, 2
  in most Drift GP pools; freestyle on four Yokohama arenas, 90 s (career) or 1–10 minutes (free
  race); career Drift Battle "Ecko Unltd US Drift Battle" on longbeach_cl/_r, 2 laps.
- Rules and scoring: section 2 [V].
- Post-race [D, flow.bin `f_post`]: stats `post_race_stats_drift_scores` (stat `drift_total`) →
  `post_race_drift_scoreboard` → `post_race_results_drift` ("Drift Scores"); Retire goes to the
  scoreboard; Drift GP adds `state_drift_gp_pre_results` and named rival intros.
- **Engine: Drift Battle ported (F362; AI drift behaviour, flag touches and more routes F367;
  result estimator F386; road contacts F387); Drift GP F388 (§7)**;
  Drift GP and freestyle share the scorer (section 4).

#### Head-to-head and driver offers

- Head-to-head [D]: `head_to_head` format and one h2h event per career region (2-car grids,
  `head_to_head_results` sequence, `Reward_Head_To_Head_Won_Final`).
- Driver offers [D]: objectives position, beat team, finish without damage, `time_trial`,
  `top_speed` (speed test) with their own stats screens.
- Engine: not supported (no career layer); a 1-opponent race is close to a head-to-head.

## 2. Drift scoring [V]

One drift manager (constructor 0x6683D0, `DriftSettings` parser 0x62FAE0, vtable 0xCEA030) owns an
entry per car (0x652E30): a detector, and at entry +0x60 a scorer (0x6300B0). Entries are created
only when the format enum is 2, 3 or 4 (0x5F4E60), with the scorer's battle flag = format 2: the
**position multiplier exists only in Drift Battle**. AI entries get their difficulty level, the
player −1 (bias 1). Per fixed step 0x65F250 updates every active entry; an entry whose car has
finished (race entry vf +0x50) is banked and deactivated.

### Settings

Constructor defaults [V] and `drift.xml` [D]:

| Field | Offset | Default | drift.xml |
|---|---|---|---|
| thresholdSlideAngle (°) | +0x2C | 5 | 15 |
| thresholdSlideTime (s) | +0x30 | 0.2 | 0.5 |
| comboTime (s) | +0x34 | 2 | 3.5 |
| thresholdSpeedEnter / Exit (mph) | +0x38/+0x3C | 40 / 20 | 50 / 30 |
| collisionPenalty | +0x40 | 500 | 500 |
| scoreUpdateInterval (s) | +0x44 | 0.1 | 0.1 |
| scoreDivisor | +0x48 | 100 | 100 |
| minimumComboDriftLength (s) | +0x4C | 0.5 | 1.0 |
| combo-fail cooldown (s), no attribute | +0x50 | 0.5 | – |
| smokeFactor / maxSmokeMultiplier | +0x54/+0x58 | 0.1 / 3 | 0.1 / 3 |
| maxCloseDriftBonus | +0x5C | 1000 | 20000 |
| closeDriftRadius / InnerRadius (m) | +0x60/+0x64 | 10 / 1 | 10 / 2 |
| wrongWayThreshold | +0x68 | 20 | 350 |
| driftScoreMainMultiplierIncrease | +0x6C | 0.1 | 0.1 |
| collisionThreshold | +0x70 | 150 | 400 |
| driftBattleFirst / LastMultiplier | +0x74/+0x78 | 3 / 1 | – |
| DriftType × 7 (name, time, bonus) | +0x7C, 0x2C each | Drift 0 s 0 … Drift Legend 7 s 10000 | osd_drift 0 s 0, great 1 s 500, fantastic 2 s 1000, incredible 3 s 1500, insane 4 s 2000, king 5 s 5000, legend 6 s 10000 |
| DifficultyBias level 0–5 | +0x1B0 | 1 | 0.1, 0.2, 0.5, 1.0, 1.8, 4.0 |

### Detector (0x65DD50, 0x62FF80, collision listener 0x64D840)

- Every tick: time since a collision and since a `close_drift_flag` touch (doubles, start 9999) grow
  by dt; speed = |v|; the slip angle is acos of the horizontal (X/Z lanes kept by the masks at
  0xCAF410/0xCAF430/0xCAF460) velocity direction · car forward, scaled by the speed below 1 m/s,
  negative on one side [I which side]; degrees = |angle| × 57.2957763671875 (0xCB8204); mph by
  0xA7DE00 (× 3600 × 0.001 × 0.62137121).
- Drifting (0x62FF80): while angle ≥ thresholdSlideAngle and mph ≥ the enter speed (the exit
  speed once drifting), a slide timer grows, else it is 0; drifting = timer > thresholdSlideTime.
- Collision listener (registered through 0x996150 in the car's 11 listener slots at +0x37098, called
  by the car body-contact handler 0x9EF360; 0x64D840 skips an object whose vf+4 type is 11, the
  packed track chunks (§4.3, F387), and a non-positive impact): an impact above collisionThreshold (0x62FFE0) zeroes the collision
  clock unless the other object's +8 → +0x230 byte is set or its callback (0xA6A270 → +0xC) is
  0xA01910, the car body-impact dispatcher [V]; **so contact with another car never fails a combo**
  [I: 0xA01910 serves car bodies]. Touching a `close_drift_flag` entity (0x64D7E0: the other object's
  entity in the manager's flag list) zeroes the flag clock (F367).
- Close drift: each of the 4 wheels against every `close_drift_flag` position (entities whose name
  contains `close_drift_flag`, collected by 0x668670), distance in X/Z, normalised (d − inner) /
  (radius − inner), clamped 0..1; "close" when the minimum is below 1.
- Race progress = laps + lap fraction; position multiplier = first − position/(cars − 1) × (first −
  last) with the race position (0x65A9C0 → race entry vf 2, 0 = leader).

### Scorer (0x643BC0, 0x630560, 0x63C370, 0x630490)

- A drift counts only while racing forwards: progress > 0 and ≥ the furthest progress (a jump of more
  than half a lap restarts the furthest).
- Every scoreUpdateInterval while drifting: points += trunc(angle° × mph / scoreDivisor × m × bias ×
  main multiplier), at least 1; then the main multiplier += 0.1. m = the position multiplier in a
  battle, else 1; bias = DifficultyBias of the AI level. **The main multiplier grows through a drift,
  so long drifts score quadratically.**
- Ratings: when the drift lasts longer than the next DriftType time, + trunc(bonus × m × bias).
- Combo: the first drift opens it (×1); a later drift raises it (up to 99) once it lasts longer than
  minimumComboDriftLength. A drift that ends adds its points to the combo; comboTime without a new
  drift banks the combo: total += combo points × combo multiplier.
- Close drift: entering a flag's radius while drifting arms it; leaving it while still drifting pays
  trunc(m × maxCloseDriftBonus × trunc((1 − closest) × 100) × 0.01 × bias). Touching the flag
  disarms it.
- A collision above collisionThreshold during a combo (and outside the 0.5 s cooldown) loses the
  current drift and the combo. The failed drift cannot raise a combo again; its later points join the
  next combo. collisionPenalty (500) is parsed but not read by this path.
- Shown score = (combo points + current drift) × combo + total.
- Statistics for the scoreboard: `numSingleDrifts` (+0x104; the combo bank also counts one), longest
  drift, largest angle, fastest drift speed, best combo, running averages of speed, distance and
  angle, points and count per rating.
- The drift-count stat of `post_race_stats_drift_scores` is read through 0x4D1890 (0x51E2C0), not
  traced; the port shows the banked rating counts, the drifts that scored [I].

### Race end (0x64D940)

Every active entry is banked. A car still racing that moves faster than 5 mph has its total scaled
by event laps / progress. If its shown score is still 0, the original replaces the banked total and
the shown score with its virtual race's estimate × the event laps (§4.2, ported in F386).

## 3. Choice: Drift Battle

Least new physics and assets on our tracks and cars:

| Discipline | New physics | New assets | Rules work |
|---|---|---|---|
| Pro Tuned / Touring / GT | none | cars only | none (already a race) |
| **Drift Battle** | none (the BMW slides; F338 handling) | none: the original runs it on route_1 of three of our circuits | scorer, HUD, results |
| Drift GP | none | none | as battle + rounds, rival intros |
| Freestyle drift | none | Yokohama arenas | time limit |
| Demolition Derby | heavier contact use | Detroit stadium, stock car | DQ rules |
| Touge | none | Haruna, its cars, traffic | runs, penalties |
| Le Mans | time of day | Le Sarthe, LMP/GT cars | time limit, classes |

Pro Tuned needs nothing because it is not a new discipline. Drift Battle is the first event type
with its own rules that runs on our data as the original runs it: longbeach_cl, nurburgring_spr and
istanbul_int are original Drift Battle layouts, and all three race headless with the AI field and
lap timing (`local/disciplines/runs/`: 4 cars, 9000 ticks, laps 85–97 s, no failure).

## 4. Port (Drift Battle)

- `grid_tools::drift`: `Settings` (constructor defaults, `drift.xml` through the binary-XML reader),
  `close_drift_flags` (route `objects.ens`), `Detector`, `Scorer`, `Entry`, `finish_race`,
  `classification`.
- grid-app `drift_event.rs`: `DriftEvent` with one entry per car, updated after every fixed tick
  from the shared world (pose, velocity, wheel anchors, `MultiRuntime::impacts` largest solved
  contact impact, `Standings` progress and position); banks a car when it finishes or drops out and
  the rest at the race end; HUD panel (top centre) and the scoreboard data.
- `post_race.rs`: drift flow stats (`drift_total`) → scoreboard → drift results, Retire → scoreboard;
  results in score order with a Score column.
- `[drive] discipline = drift_battle` / `--discipline drift_battle` (needs laps and opponents);
  `settings/drift-battle.ini`, launcher `Play GRID Drift Battle.cmd`: longbeach_cl, 2 laps [D: the
  career event], 7 AI cars [D: free-race battle maximum 8], BMW field (AUTHORED), grid at the back
  (AUTHORED; the original draws a random slot), AI drift level from the AI difficulty (0.606 → level
  1, bias 0.2) [I].
- Interpretations and authored parts: impact = the largest solved contact impact (+0x3C), the scalar
  the body-impact listener receives [I]; the detector permission (+0x41) is always granted [I]; the
  results order by score with ties in race order [I]; a car that drops out is banked like a finisher
  [I]; the slip-angle sign side [I]; the HUD is text in the original's places only roughly.
- Not ported: smoke multiplier, wrong-way message; collisionPenalty use; the original OSD look and
  sounds; Drift GP rounds and intros; freestyle. AI drift behaviour and flag touches followed in
  F367 (§4.1, §4.3), the result estimator in F386 (§4.2), Drift GP in F388 (§7).

### 4.1 AI drift behaviour (F367) [V unless marked]

18 October 2026, Claude Code. Sources: decoded `ai/*.xml` (`local/drift-ai/data/`), the route
`ai_track.xml` files (`local/drift-ai/tracks/`), Ghidra 12.1.4 headless read-only pseudocode on a
private copy of the disciplines project (`local/drift-ai/ghidra-project`, wrapper
`local/drift-ai/ghidra/run.ps1`, outputs `local/drift-ai/decomp/a1..a5, e1..e3, f1, f2, l1, l2`),
Capstone 5.0.6 dumps and scans (`local/drift-ai/scan.py`, `decomp/78fe90.dis`), constants through
`local/disciplines/scripts/consts.py`. Nothing was installed.

**Data [D].** `ai_driver_config.xml` has a section per competition format; `drift_battle` overrides
laDistMin 15, laDistMax 45, thBrakeLineHoldTimeDrift 0.2, rsEndSpeed1/2 15, coSpinSpeed 3, switches
on `behaviour_drift` and `test_drift`, and off race_damaged, draft, block and controlled_stop.
`ai_driver_parameter_mappings.xml` `drift_battle` overrides follow, alongside-yield, separation,
braking-buffer, prediction-ratio and deviation-large mappings; `default` maps drift_skill (control,
always 1), drift_position (aggression, 0..0.5) and drift_position_attempts (0..5).
`ai_vehicle_config.xml` `default` holds the `drift*` parameters (touring has no overrides); the
`drift`, `drifts15`, `driftrx7`, `driftgto`, `driftimp`, `driftmop`, `driftsoa` and `driftcor`
classes override them (angle range 40–55°, front grip 1, rear 0.2–2, racingGripMultiplier 1.4).
Every drift layout of our circuits ships a `drift` racing line on every gate besides `optimal`
(longbeach_cl 169 gates, its reverse 172, nurburgring_spr 145 / reverse 141, istanbul_int 130 /
reverse 125, Jarama route_0 150, route_1 157); the class brake data carry `drift_speed`.
`ai_behaviour_fire.xml` gives drift the scores 2, 12, 13. `ai_choreographer.xml` has a
`drift_battle` set: manager on, no conveyor, events or race-start factors.

**Config overlay [I, strongly supported].** The format section overrides `default`: the names are
the competition format names, `single_race` is empty (the F290 live race showed `default` values),
and without the overlay `behaviour_drift` would be off and the drift code below unreachable.

**Behaviour flags.** Behaviour switches sit at driver cfg +0xCB4 + type (drift = type 15 →
+0xCC3), test switches at +0xCC4 + type (drift test = type 20).

**Line choice 0x794870** (from common setup 0x7978D0, every behaviour): racing-line kinds
(.rdata 0xCF7128, value → name: retirement 0, optimal 1, overtake 2, left 3, right 4, traffic 5,
drift 6, start 7; AI_DRIVER_RESEARCH's "retirement 1 … drift 7" was off by one): optimal; traffic
with trafficBehaviour (cfg+0xC98); **drift with behaviour_drift (cfg+0xCC3)**; start before the start
line (context progress < 0); a link without the wanted kind falls back to optimal (0x793720(…, 1)).

**Drift test 0x78FE90** (object 0x1C bytes from 0x7A7950: +8 fired, +0x10 place at the last fire,
+0x14 timer = −1, +0x18 before-start flag, +0x19 "always" flag, never set). f = place/(cars − 1)
(0 = leader); random draws 0xA683B0 U(a, b) on the RNG at context +0x38:
- progress ≤ 0: not fired, before-start set; a negative timer draws U(clamp(10 − 8f, 2, 10),
  clamp(20 − 12f, 8, 20)) s;
- racing, timer −= dt; not fired: fires when the timer is negative, or at once on an **even place**
  once the before-start flag is clear (the compiler's `place % 2 == 0`); on firing the timer draws
  U(48 − 32f, 24 − 16f) s and the place is kept; fired: when the timer is ≤ 0, the place has not
  improved and the car does not lead, it stops and the timer draws a pause U(15 + 15f, 5 + 10f) s;
- a fired test runs 0x78CB30: with a valid brake line in the request and speed above its drift speed
  (+0x1C), a braking distance 0x7A6650 above the request's line distance (+0x28) refuses the drift
  (not fired, timer 3 s).

**Score 0x78B360**: drift test fired → 2; fired and the cornering test (0x77ED00: |κ| >
crMaxTrackCurvature) fired → 12; behaviour state (+0x30) ≠ 1 → 13 (so a started drift holds against
overtake 5 and alongside 8, not against avoid 15 or recovery 16+). The object (0x795AC0) starts in
state 0. No activate/deactivate; drive 0x78F490 = 0x78B3C0 then 0x783A10, returning 0, so the
shared race drive 0x797B50 runs after it.

**State update 0x78B3C0**: request +0xCC = driftLookAhead; 0 → 1; speed < driftSpeedMin → 1;
1 → 2 when cornering and speed < 50 m/s (0xCB1104); 2 → 3 when |slip| > driftStartAngle; 3 → 4 when
|slip| < driftEndAngle or the cornering test is not fired; 4 → 1.

**Drive 0x783A10** (grips in request order +0xB8/+0xBC rear, +0xC0/+0xC4 front): merge +0x9C = 0;
- 1 approach: drift mode +0xC8 = 1; desired offset ±drift_position, negative when the next corner's
  curvature (track view +0x84 from 0x792580: heading change from the next brake line's link over
  ≥ 20 m of links, signed) is negative; merge driftMergeFactor; all grips driftRacingGrip;
- 2 initiate: drift mode; unless context +0x3C +4 > 0 [I: the published brake], rear driftMinGrip,
  front driftInitiateGrip;
- 3 drifting: drift mode; angle = |slip| × 57.29578; top = driftAngleRangeMax, lowered to
  asin((room − w)/(length + w)) in degrees (0xBF8C93 = asin, VEHICLE_STEP.md; 90 when it fits) with
  room = the look-ahead's free room to the **outside** track limit (view +0x218 left for κ ≥ 0, else
  +0x21C), w = half width (+0x110), length (+0x114); bottom = drift_skill × driftAngleRangeMin; rear =
  driftMinGrip below the bottom (the decompiled lerp clamps to it), 1 inside, lerp 1 →
  driftMaxGrip from the top to driftAngleMax above; front driftGripFront;
- 4 end: no drift mode, grips racingGripMultiplier.

**Drift mode in the shared pipeline** (every reader of +0xC8 in 0x770000–0x7B0000 scanned):
steering 0x77AC50 damps the yaw rate with driftStAngVelFactor (vcfg+0x3B0) instead of stAngVelFactor;
corner speed 0x792C80 counts the next brake line driftBrakeLineAdvance (vcfg+0x4F0, built-in 8 m)
earlier, returns early (no brake line this tick) within it, and targets the line's drift_speed
(max_speed when it is 0); governor 0x77A4C0 holds a passed line for thBrakeLineHoldTimeDrift
(cfg+0x2E8) instead of its hold time; look-ahead 0x79C360 would use +0xCC only with +0xC9, which no
drift code sets. On a non-optimal line 0x792C80 uses tgNonOptimalSpeedFactor (0.8) instead of the
off-line factor.

Constants: 0xCB3D64 90, 0xCB8200 π/180, 0xCB8204 180/π, 0xCB1104 50, 0xCB4804 −8, 0xCAFA58 10,
0xCB8DA4 2, 0xCB47E4 −12, 0xCAFBD8 20, 0xCB0C44 8, 0xCB2B74 48, 0xCB0F3C 16, 0xCB0C48 24, 0xCB3BA8
15, 0xCAFDB8 30, 0xCB093C 5, 0xCB1790 3, 0xCB0948 −1. Vehicle built-ins without XML values:
driftInitiateGrip 1.2, driftRacingGrip 1.1, driftBrakeLineAdvance 8, driftLookAhead 15
(`local/ai-research/vehicle-config-layout.tsv`).

**Port** (`grid_tools::ai_driver`, `grid_formats::ai::track_on`):
- `AiData::load_for(…, Some("drift_battle"))` overlays the format section and mappings, reads the
  switches and builds a second `AiData` on the `drift` line; `Parameters::drift` (`Drift`) when
  behaviour_drift and test_drift are on; race_damaged and controlled_stop follow their switches;
- `Behaviour::Drift`, `DriftState::test` (0x78FE90), `Drift::next_state` (0x78B3C0),
  `Drift::drifting_grip` (state 3), `AiDriver::drive_drift` (0x783A10) and the drift-mode branches of
  the shared pipeline; `AiDriver::race` (`RaceView`: 0-based place, field size, progress) set by the
  race before each update; the line switches to the drift line past the start line, shifting the
  planned offset by the lines' distance at the car [I];
- `ai_pacing::Data::load_for(…, "drift_battle")` (the choreographer set);
- grid-app: the Drift Battle loads the drift_battle AI data, choreographer set and race place;
  `grid-opponent-race --discipline drift_battle` (headless; applies the request grips itself without
  a race manager; `GRID_AI_CLASS` for class research, `GRID_TRACE_DRIFT_CONTACTS`; drift trace rows
  `#drift`); the normal race is unchanged (identity checks, §6).
- AUTHORED / [I]: the draws use the driver's authored LCG (the original's RNG instance is not
  identified); the previous update's request stands in for the test's request read; the next-corner
  curvature is measured over a continuous 20 m instead of whole links; the state-2 brake gate; the
  line-change offset shift. drift_position_attempts has no reader in these paths (not searched
  elsewhere).

**AI vehicle class.** Our BMW 320si is a touring car ([D] `ai_vehicle_type_track` = touring), so it
keeps the `default` drift values; the original's Drift Battle grids use drift cars with the `drift*`
classes. Solo two-lap runs of every class on longbeach_cl (`GRID_AI_CLASS`,
`local/drift-ai/runs/class-*.txt`) [V]: touring completes 2 laps (106 s) but scores 1 point (front
grip ×10 snaps the car around); drift, drifts15, driftcor, driftgto, driftmop, driftimp hit
barriers and stop before one lap; driftrx7 (252 points), driftsoa (674) and streetgt finish one lap.
The port keeps the car's own class (no authored choice); the field results below follow from it.

**Results [V]** (headless, BMW field, 7 + 1 AI cars, 16000 ticks, `local/drift-ai/runs/field-*`):
- longbeach_cl: scores 1–625, 3 cars banked; 5 classified, 3 DNF; 18223 contact records;
- nurburgring_spr: 1019–17703 points; 3 classified, 1 wrecked, 4 DNF;
- istanbul_int: 29–1555 points; 5 classified, 3 DNF.
Before F367 the AI raced the line and scored little (F362). AI cars now follow the drift line,
initiate and hold slides of 25–50°, and score; they also spin (largest angles near 180°), touch
barriers and each other, and some stall in recovery, so the fields are much less tidy than a race.
Whether the original's AI shows the same with a touring car cannot be checked (the original never
runs one in a drift event; no live drift trace was taken).

### 4.2 Result estimator: the virtual race (F386)

18 October 2026, Claude Code. Sources: F367's decompilation (`local/drift-ai/decomp/e1..e3`) and new
Ghidra 12.1.4 headless read-only pseudocode on a private copy of the drift-ai project
(`local/drift-est/ghidra-project`, deleted after use; wrapper `local/drift-est/ghidra/run.ps1`,
outputs `local/drift-est/decomp/g1..g10`), Capstone 5.0.6 dumps through `local/disciplines/scripts/dis.py`
(`local/drift-est/dis/`), constants through `consts.py`, strings through
`local/drift-est/scripts/str.py`. Nothing was installed.

**Observations [V].**
- The object at race world +0x21A0 → +0xF2D8 (vtable 0xCE87FC, constructor 0x67D590) is the
  original's *virtual race*: its error strings are "Virtual race not initialised", "Competitor not
  in virtual race", "Not enough competitor slots" and `virtualRace::updateVirtualLapBaseResult::
  eventTrackPoolId %d not found` (0xCE8F6C..0xCE9020).
- Set-up for a free race (0x646320) and a career event (0x646A90): +0x10, +0x14, +0x18 are copied
  from a difficulty object's +0x14, +0x18, +0x1C, +0xABC points at that object, +0xABA = 1, and
  0x680CE0 loads the base results; when it succeeds, +0xAB9 = 1.
- The difficulty object (vtable 0xCC0A64, constructor 0x5F1B10) is filled by the `difficulty` row
  reader 0x60A7B0: +4 `star_level`, +0xC `ai_ratio` (clamped 0..1), +0x10 `rep_bonus_factor`,
  +0x14 `base_time_factor`, +0x18 `top_time_factor`, +0x1C `performance_variance`, +0x20
  `available_flashbacks`, +0x24 `flashback_bonus`. So the virtual race holds +0x10
  base_time_factor, +0x14 top_time_factor, +0x18 performance_variance.
- 0x680CE0 reads `event_track_pool` (career) or `free_race_track_pool` (free race)
  `base_result_list`: each value is a float in seconds (0x826740), converted to 64-bit
  microseconds (0x908450: × 1e6 in 0xD0E390, truncated by 0xBF7786) into +0x28 + 8·i, summed into
  +0x20, count +0x48; `competition_format.time_based` goes to +0xAB8.
- 0x64D940 at the race end, for a car whose shown score (+0x130) is still 0: add the car with
  0x62B170's skill (0x67FF20 clamps it to 0..1), call +0x10 = 0x6850F0 into a local result, remove
  the car (0x67D660), and store result × 1e−6 (0x908820, an unsigned 64-bit conversion) × the event
  laps (fimul) into both the banked total (+0xC) and the shown score (+0x130).
- 0x62B170: a driver entity with a driver row returns `ai_driver.Virtual_Performance` (through
  `driver.ai_driver_id`); without one (or for entity type 2) it returns 1.0.
- 0x6850F0: result 0 unless the car is in the race and +0xAB9 is set; else s = 0x67D6C0(car) and
  0x680B20(+0x20 base, s).
- 0x67D6C0: s = U(clamp(1 − (skill + var/2), 0, 1), clamp(1 − (skill − var/2), 0, 1)) with the
  global generator 0xA683B0 (a + u·(b − a), u = 32 bits × 2^−32; seeded with the clock × 1000 in
  the constructor, 0xA68440).
- 0x680B20 with +0xAB8 = 0 (score formats; `time_based` is 0 for all drift formats [D]): result =
  (1 − s) · clamp(base/top_time_factor − base/base_time_factor, 0, DBL_MAX) + base/base_time_factor,
  in seconds, back to microseconds. The time branch (+0xAB8 = 1: base × a factor between 1 +
  0.05·(8 − n) and 1.15 + 0.0625·(8 − n) clamped to 1..1.4 and 1.15..1.65, n from two counts of the
  difficulty object) is overwritten by this branch in score formats and is not ported.
- vtable 0xCE87FC = {+0 0x6808C0, +4 0x67FF20 add, +8 0x67D650 clear, +0xC 0x67D660 remove, +0x10
  0x6850F0 result of one car, +0x14 0x685190, +0x18 0x67FFB0 (a lap estimate of the time races,
  used by 0x63C830), +0x1C 0x67D3A0 reset}.

**Data [D]** (`database.bin`, `local/race-rules/data/database.json`): career Drift Battle events
carry `100000,0,0,0`, career Drift GPs 650,000–2,000,000; the free-race pools carry per route
values, e.g. Drift Battle longbeach_cl 5,300,000, nurburgring_spr 4,900,000, istanbul_int
3,210,000, jarama 3,420,000; Drift GP longbeach_cl 1,890,000, nurburgring_spr 1,250,000,
istanbul_int 1,750,000, jarama 1,900,000 (53 drift rows in all, `virtual_race::DRIFT_POOLS`,
cross-checked against the decoded table by a script). `difficulty` rank 2 star 1 (ai_ratio 0.606,
the port's Normal): base_time_factor 43, top_time_factor 14, performance_variance 0.13.

**Interpretation [I].** A zero-score car gets a score between laps × base/43 (skill 0) and laps ×
base/14 (skill 1): on longbeach_cl, 2 laps, 246,500 to 757,100 points. The free-race base results
are the score scale the original expects of a drift event.

**Port** (`grid_tools::virtual_race`, F386): `DIFFICULTY` (24 rows), `DRIFT_POOLS`, `VirtualRace`
(`result`, `drift_score`), `micros`/`seconds`; `drift::Entry::finish_race(settings, (finished,
speed, laps), race, performance)` replaces a zero score and sets `estimated`. The results no longer
mark a `*`. grid-app and `grid-opponent-race` build the virtual race from the route's free-race pool
row of the event's format and the career rank 2 row of the AI difficulty's star level.
AUTHORED / [I]: the free-race pool (not the career event's 100,000) because the port's launch
options are free-race style; career rank 2 (the rank `flashback::AI_RATIOS` uses); a seeded LCG
(seed 0x5EED) in place of the clock-seeded global generator; the skill is the characteristics
table's `virtual_performance` when the private table is set, else 1.0 (0x62B170 without a driver
row), and the player 1.0. Routes without a pool row (Donington GP) estimate 0, as the original
without base results.

### 4.3 Flag touches and the collision filter (F367)

- `MultiRuntime::contact_impacts` returns each solved body record with what it touched
  (`Contacted::Prop(box)`, `Car(i)`, `Static`); the body-to-prop slots are recorded every tick.
- `drift::split_contacts`: a `close_drift_flag` box is a flag touch only (`Detector::touch_flag`:
  disarms the close-drift bonus) [I: the original's flags are jointed entities, the port keeps them
  as static boxes, F284]; a car is no collision ([V] 0xA01910 guard); the rest gives the collision
  impact. grid-app and the headless tool use it.
- Road contact (F387) [V unless marked]: the listener 0x64D840 returns before the collision clock
  when 0x958310(other) is true, i.e. the other object's virtual +4 returns 11. The class whose +4
  returns 11 (0xAD9950) has vtable 0xD3A778, built by 0xAD9720 from the body factory 0xA73FD0
  (via 0xA75E40). The static loader 0x5BB180 calls that factory once per packed `qt_*.vcqtc`
  collision chunk (docs/GRID_POPULATION.md: the 245 Donington chunks). Two other listeners treat
  type 11 the same way: 0x5B5E90 skips it, 0x4022F0 skips its contacts with a normal Y ≥ 0.1. So a
  car body touching a packed track chunk (road, kerbs, verges) never fails a combo. The boundary
  sheet `boundarylines.cqtc` is shape kind 10 (docs/CQTC_BOUNDARY_CONTACTS.md) and keeps counting
  [I: its wrapper's type was not traced]; props and barrier boxes count.
- Port: `Contacted::Track` for records whose other token is a packed chunk (`Track::is_chunk_token`,
  chunk i = shape i + 1); `split_contacts` ignores it like a car; the solo world uses
  `Runtime::impacts_off_track`. Car physics is unchanged (the headless field state hashes are equal
  before and after); only the drift scores change. Headless 8-car BMW battles, 16,000 ticks
  (`local/drift-est/runs/base-*` against `road-*`): summed scores Istanbul 5,911 → 44,980,
  Nürburgring 47,297 → 64,381, Long Beach 1,875 → 2,566.

### 4.4 Routes (F367)

Launch options (`settings/drift-battle-*.ini`, `Play GRID Drift Battle *.cmd`, 2 laps, 7 AI BMWs as
`drift-battle.ini`): nurburgring_spr (route_1), istanbul_int (route_1), jarama_r (route_1),
jarama_gp (route_0). Headless 4-car drift battles on all of them and on the reverse layouts
longbeach_cl_r (Long Beach route_4), nurburgring_spr_r (Nürburgring route_3) and istanbul_int_r
(Istanbul route_3) load, lap and score (`local/drift-ai/runs/route-*.txt`, 16000 ticks): Istanbul
route_3 every car 1–2 valid laps (104–141 s), Nürburgring route_3 two cars 2 laps (113–122 s), one
stopped with wheel terminal damage; Long Beach route_4 three cars 1 lap (118–245 s); Jarama route_0
three cars 2 laps (96–106 s), route_1 three cars 2 laps, one with terminal damage at 0.13 laps.
Close-drift flags: 17, 11, 10, 16, 16. The reverse layouts are not launch options yet.

### 4.5 AI drift quality (F389)

18 October 2026, Claude Code. Measure (AUTHORED, `grid-opponent-race` `drift_quality` lines): per
car, *spins* (body slip above 90° until it falls below 45°), *barrier hits* (a boundary or prop
contact above collisionThreshold with a mostly horizontal normal, a new hit after 1 s without one;
packed road chunks excluded) and *stalled seconds* (below 1 m/s while racing), counted from the
start line on. Headless 8-car BMW Drift Battles, 16,000 ticks, `local/drift-est/runs/road-*`
(totals over the field):

| Route | laps | spins/lap | hits/lap | stalled s | score sum | best |
|---|---|---|---|---|---|---|
| longbeach_cl | 10.5 | 25.1 | 5.6 | 544 | 2,566 | 1,156 |
| nurburgring_spr | 13.8 | 11.1 | 1.6 | 343 | 64,381 | 17,887 |
| istanbul_int | 14.6 | 12.0 | 2.0 | 316 | 44,980 | 28,835 |

Against the virtual race's scale (one Normal AI car on longbeach_cl ≈ 250,000–750,000 points for 2
laps, §4.2) the port's AI drift scores are one to two orders of magnitude low.

Checks against the original, none of which showed a port difference that a 1:1 change could fix:
- drive 0x783A10 and states 0x78B3C0 re-read: state grips, the state-3 angle window, the room
  limit and the state transitions match `drive_drift`/`next_state`.
- The state-2 gate "context +0x3C → +4 > 0" (port: the published brake [I]): the behaviour context
  built at 0x79CAA0 stores the driver's `this+4` at +0x3C, consistent with the published controls;
  not proven further.
- Steering 0x77AC50 [V]: steer = |slip|·stSlideFactor + factor·(−ω·up) + G·heading error, the yaw
  term negated through the sign mask 0xCAF470; the port uses +ω·up (AUTHORED, docs/AI_DRIVER_PORT.md:
  "the yaw term sign (damping)").
  A private experiment with the sign flipped in drift and race alike made the fields far worse
  (spins/lap 12 → 52 at Istanbul, 25 → 67 at Long Beach, `local/drift-est/runs/yawflip-*`), so the
  port's angular velocity sign convention is probably opposite to the original's car-state copy and
  the effective law matches; the convention itself is not established. No change.
  **Settled in F391 [V]** (docs/AI_DRIVER_PORT.md, "Steering sign convention"): ω and up are the
  same body values in both; the original's heading error has the opposite sign and its publisher
  0x996240 writes +E30 = −steer. So the port's +ω·up equals the original, and the flip was
  anti-damping. The chaotic fields have another cause.
- Stalled cars: a spun car on the track scores recovery 11 against drift 12/13, so Drift wins; when
  `on_track` flickers at the limit Recovery re-activates (and resets, [V] 0x776990) every few ticks
  (car5 at Long Beach: 1,047 Recovery activations, 0.05 m/s for 170 s). Scores and activation
  semantics are the original's; the flicker comes from the shared on_track test.
- The touring BMW uses the `default` drift values (driftGripFront 10); the original's drift grids
  use the `drift*` classes (§4.1).
The fields stay chaotic. Next candidates: a live drift trace of the original (not allowed in this
session: no input to the original), the car-state angular velocity sign, the drift classes.

## 5. Order for the remaining disciplines

1. **Drift Battle completion**: AI drift behaviour, flag touches and the routes done in F367
   (§4.1, §4.3, §4.4); the result estimator (F386, §4.2) and road contacts (F387, §4.3). Left: the
   reverse layouts as launch options; a drift-class car or a live original trace to judge the AI
   (§4.5).
2. **Drift GP**: done as a free-race event in F388 (§7). Left: career rounds
   (`drift_gp_info.round`), the pre-results scene and rival intros.
3. **Circuit-race series on our tracks** (Pro Tuned, Touring, GT, Le Mans Series): only cars and the
   other route layouts (route_1–3 of our circuits, reverse routes). Needs: car generalization for
   more models (F338), random grid fill.
4. **Freestyle drift**: time-limited solo scoring with `ignore_progress` (+0x1C8 [I]); needs the
   Yokohama track (four arenas) — first Japanese track. Not cheap: every `Free_Drift` pool row is a
   Yokohama freestyle arena (route_3..6, 90 s career, 1,148,000 base result [D]); none of our tracks
   has one.
5. **Touge**: Haruna point-to-point routes, two-run timing, touge.xml penalties and the alongside
   rules, midnight traffic; needs Haruna and Japanese cars, point-to-point lap timing.
6. **Demolition Derby**: Detroit stadium routes, stock car, flipped/wrong-way disqualification
   (demolition_derby.xml, 0x630BC0), DD results screen; needs Detroit and a stock car, robust heavy
   contact (soak).
7. **Le Mans**: Circuit de la Sarthe, LMP/GT field (`lemans_restrictions`), time-limited race type 3,
   class results, time of day and headlights; the largest asset and rendering step.
8. Head-to-head and driver offers come with a career/front-end layer.

## 6. Verification

- Unit tests (`grid_tools::drift`, 11 plus 2 asset tests): detector thresholds, close distance,
  single drift steps/ratings/bank, combos and short drifts, collision fail and cooldown (with the
  original's failed-drift carry-over), progress rules, close drift bonus and flag touch, battle
  multiplier and AI bias, race end banking/scaling/estimate flag, constructor defaults; asset tests:
  the shipped drift.xml equals the decoded values, the three drift routes have close-drift flags and
  Donington GP has none.
- grid-app: `drift_event` unit tests (HUD text, levels, grouping), post_race drift flow, and two
  release asset tests in `pause_menu::race_tests`: a one-lap autopiloted Drift Battle on longbeach_cl
  (BMW field) banks the player at the finish, goes stats → scoreboard → results, and classifies
  every car by score; a scripted handbrake turn in the BMW drifts (27°, 104 mph peak) and scores.
- Offscreen app runs with `settings/drift-battle.ini` (`local/disciplines/drift-battle-app.log`,
  `drift-battle-hud.png`): 17 close-drift flags, 8 cars, HUD panel in place.
- Pre-push checks (F362): 565 release workspace tests passed, 0 failed, 0 ignored; Clippy clean;
  solo identity aad9a07a…e41b; the 12-car headless race byte-identical to origin/main before the
  change; grid-soak quick 20/20.

## 7. Drift GP (F388)

18 October 2026, Claude Code.

**Observations.**
- [V] The drift entries exist for format enum 2, 3, 4 (0x5F4E60); the scorer's battle flag is
  format 2 only, so a Drift GP scores without the position multiplier.
- [D] `ai_driver_config.xml` `drift_grand_prix` (and `drift_freestyle`): `<behaviour
  default="false">` with only `behaviour_controlled_stop` on, every other behaviour off.
  `ai_choreographer.xml` puts drift_grand_prix in a set with drift_freestyle and pro_touge (manager
  on, no conveyor or race-start factors).
- [D] Free-race Drift GP pools hold 2 cars on every route except Okutama (8), with their own base
  results (§4.2). Career Drift GPs (world_drift, Lng_Beach_battle, yokohama_drift, drift_gp_inter,
  downhill_drift) run on Yokohama, Washington, Okutama, Shibuya and Haruna only.
- [D flow.bin] Pre-race `c_pre_drift_gp_place_vehicles` (`state_drift_gp_pre_results`), the
  results through `end_race_prepare_retire_event` → `state_drift_gp_pre_results` →
  `drift_results_sequence` (a StateRaceIntroDrift scene with two drivers' wheel locks) and a
  `c_full_drift_gp_next_round` → `next_race` path for the rounds; stats → scoreboard → results as
  the other drift formats. `drift_gp_info.round/track_type/track_name` (0x5A0CC0) and the rival
  intros `DriftGPIntro_Kazuhiro/Nobushige/Madoka/Youichi` are front-end/audio.
- [V] 0x64D940 gives every car still unfinished with a zero shown score the virtual race's
  estimate.

**Interpretation [I, supported by the AI config and the race end].** In a Drift GP the player
drifts alone; the rivals do not drive (every behaviour off) and their results are the virtual
race's estimates at the race end. Where the original keeps the rivals' cars (parked for the
pre-results scene, or on the grid) was not traced.

**Port.**
- `[drive] discipline = drift_grand_prix` / `--discipline drift_grand_prix` (laps, no opponents):
  the solo run with the time-trial event (3 s countdown, AUTHORED: no start lights) and a
  `DriftEvent` of one car on the track plus the pool's other cars as virtual rivals (Long Beach:
  1); the player's race progress comes from a one-car `Standings` on the reset line; impacts from
  `Runtime::impacts_off_track` (no flag touches in the solo world, AUTHORED); at the finish the
  player is banked and every rival estimated (skill 1.0 without the private table).
- Results: one row per rival ("AI k", the player's car label), ordered by score; heading
  "drift gp"; HUD without the battle multiplier.
- `settings/drift-gp.ini` and `Play GRID Drift GP.cmd`: longbeach_cl, 2 laps (career Drift GPs
  run 1–3 laps [D]).
- `grid-opponent-race --discipline drift_grand_prix` runs one car (an AI drift driver with the
  drift_battle config stands in for the player, AUTHORED) and estimates the rivals.
- Not ported: rounds, the pre-results scene and intros, parked rival cars.

**Checks.** Unit test `a_drift_gp_estimates_its_virtual_rivals_and_ranks_them`; headless runs
`local/drift-est/runs/gp-*` (longbeach_cl stand-in 1 point against the rival's 262,259; istanbul_int
11,760 against 242,832; nurburgring_spr 8,588 against 173,452; jarama_gp 43 against 263,647); an
offscreen app run of `settings/drift-gp.ini` (`local/drift-est/gp-app.log`, `gp-1500.png`: heading
"drift gp", HUD "Total 0", `drive_drift_event version=2 discipline=drift_grand_prix ... rivals=1
... base_result=Some(1890000.0)`). A full app Drift GP to the results screen was not run (the solo
run has no autopilot).
