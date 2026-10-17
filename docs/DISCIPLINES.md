# Race disciplines of the original

17 October 2026, Claude Code. Inventory of every event type of Race Driver: GRID, the port of the
first one the engine did not have (Drift Battle, with the drift scoring shared by all drift
formats), and the order for the rest.

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
- **Engine: Drift Battle ported (this entry)**; Drift GP and freestyle share the scorer (section 4).

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
- Collision listener (registered through 0x996150): an impact above collisionThreshold (0x62FFE0)
  zeroes the collision clock; touching a `close_drift_flag` entity (0x64D7E0) zeroes the flag clock.
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
by event laps / progress. If its shown score is still 0, the original replaces it with its result
estimator × laps (estimator object at race +0xF2D8, value × 1e−6): **not ported**, like the race
estimator 0x67FFB0 (the port marks such rows with `*`).

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
- Not ported: **AI drift behaviour** (the original AI has `behaviour_drift`, `test_drift` and the
  `drift*` parameters; ours races the line, so AI cars score little); the result estimator for
  zero scores; smoke multiplier, wrong-way message; touching a flag (needs per-entity contacts; the
  flags are kept static props, F284); collisionPenalty use; the original OSD look and sounds;
  Drift GP rounds and intros; freestyle.

## 5. Order for the remaining disciplines

1. **Drift Battle completion**: AI drift behaviour (`behaviour_drift`, `drift_skill`,
   `drift_position`, `driftStartAngle`…, F-AI research style), the result estimator (shared with
   0x67FFB0), flag touches, the other two routes as launch options (nurburgring_spr, istanbul_int;
   reverse routes need their own check) and Jarama (F361: route_1 = jarama_r, route_0 = jarama_gp, both
   in the Drift Battle pool). Needs: no assets; AI research.
2. **Drift GP**: same scorer without the position multiplier; 2-car pools, rounds
   (`drift_gp_info.round`), `state_drift_gp_pre_results`, rival intros. Needs: the flow states.
3. **Circuit-race series on our tracks** (Pro Tuned, Touring, GT, Le Mans Series): only cars and the
   other route layouts (route_1–3 of our circuits, reverse routes). Needs: car generalization for
   more models (F338), random grid fill.
4. **Freestyle drift**: time-limited solo scoring with `ignore_progress` (+0x1C8 [I]); needs the
   Yokohama track (four arenas) — first Japanese track.
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
