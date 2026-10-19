# Original AI opponent driver — research (F-AI-01)

15 October 2026. Static analysis of the hash-identified private analysis PE
(`local/compatibility/grid-unpacked.exe`, SHA-256 `aac46a64…4f34514`, base
0x400000) plus one bounded read-only snapshot of the paused original race
(research copy `grid-compat\grid.exe` SHA-256 `b1da1b2d…aece34a6`, PID 7208,
started 2026-10-15 11:58:42 local; Touring/Lacetti/Donington GP, Normal
difficulty). No game file, Steam install, process memory or Ghidra project was
modified; Ghidra 12.1.4 ran headless with `-readOnly` (temporary functions were
created in memory only). Decompiler output, dumps and live captures stay under
ignored `local/ai-research/`. This document contains only addresses, field
names, numeric constants and structural descriptions.

Labels used below: **[V]** verified from disassembly/decompilation (and, where
noted, from live memory); **[D]** directly observed in data files; **[I]**
inference, not yet verified.

## 1. Data files

All AI XML files are sectioned binary XML (see [BINARY_XML.md](BINARY_XML.md));
readable private derivatives already existed in
`local/binxml/captured-dependencies/decoded-007..015.xml`.

### 1.1 `tracks/<track>/route_0/ai_track.xml` (root `ai_track_data` v3.0.2) [D]

- `track name="default"` → `gates num_gates=134` (Donington). Each gate:
  `position` float3, `normal` float3 (horizontal unit vector), `waypoints`
  (5 entries) and `edges num_edges=0`.
- Waypoint `type`/`length`: `racing_line` (child `racing_line type="optimal"`),
  `left_track_limit` (always 0.00), `right_track_limit` (16.0–19.76),
  `left_racing_limit`, `right_racing_limit`. `length` is metres along `normal`.
  **Point = position + normal × length** [I, strongly supported]: `normal`
  equals the left→right direction of the matching progress-track gates
  (e.g. 0.1755/−0.9845 vs progress gate 0 left→right 0.18/−0.98), so
  `position` is the left track edge and the track width is
  `right_track_limit`. The racing-line polyline built this way is 3944.0 m
  (progress track 4003.84 m); gate spacing 3.7–78.9 m (mean 29.4 m).
- Executable enumerations [V] (.rdata 0xCF71A0 / 0xCF7120): waypoint types
  shortcut 0, racing_line 1, left_racing_limit 2, right_racing_limit 3,
  left_track_limit 4, right_track_limit 5; racing-line kinds retirement 1,
  optimal 2, overtake 3, left 4, right 5, traffic 6, drift 7 (+`start`).
  Correction (F367, DISCIPLINES.md §4.1): the table at 0xCF7128 pairs value then name, so the
  racing-line kinds are retirement 0, optimal 1, overtake 2, left 3, right 4, traffic 5, drift 6,
  start 7.
  Donington only ships `optimal`; Jarama ships `optimal` and `drift` on every
  gate (the reader keeps `optimal`, TRACK_GENERALIZATION.md).
- `links num_links=134`: `link from_gate→to_gate` (closed loop 133→0).
- `brake_lines num=11`: `brake_line gate_id` + `brake_data min_speed max_speed
  drift_speed distance_modifier` (track defaults; min = 0.75 × max).
- `retire_lines` (7, gate + side + position), `fork_sets` (one empty "race").
- Brake-line gates sit at/just before racing-line curvature peaks (gate 9 → 61 m
  before Redgate peak 0.0142 rad/m; gate 100 → 39 m before the 0.0436 rad/m
  hairpin) [D, private `line.py` analysis].

### 1.2 `ai_vehicle_track.xml` (root `vehicle_type_track_data` v3.0.0) [D]

Per vehicle class (13 classes incl. `touring`) the same 11 brake lines with
`min_speed max_speed drift_speed distance_modifier hold_time` (hold 1.50 s,
modifier 1.00 everywhere). Touring Donington min/max (m/s):
28.8/46.0, 33.0/53.0, 48.4/78.6, 37.2/60.0, 43.7/70.8, 47.6/77.3, 34.2/55.0,
30.0/48.0, 22.8/36.0, 18.0/28.0, 18.0/28.0. Index = brake_line id → gate_id
from ai_track.xml.

brake_data parser 0x775100 [V]: struct +4 max_speed, +8 min_speed, +0xC
drift_speed (default max−5), +0x10 distance_modifier, +0x14 hold_time
(default −1). Units are m/s (they are compared against speed and fed into the
speed→distance tables).

### 1.3 Global `ai/` folder (inventory) [D]

| File | Root | Content |
|---|---|---|
| ai_driver_config.xml (13,164 B) | ai_driver_config | Parameter overrides per game mode (`default`, `single_race` (empty), demolition, drift, touge, network, test_drive, garage) incl. enabled `behaviour_*` and `test_*` flags |
| ai_driver_parameter_mappings.xml (16,796 B) | driver_parameter_mappings | 46 `ai_variable` mappings: base `value` plus 4 `limit`s keyed by (difficulty 0/1, characteristic_value 0/1) |
| ai_vehicle_config.xml (10,548 B) | ai_vehicle_config | Steering/throttle/brake/TC/ABS/PID/drift/crash parameters per vehicle class (`default`; touring has no overrides) |
| ai_vehicle_statistics.xml (228,684 B) | ai_vehicle_data | Per car (46 incl. `lac`): brake/cruise/accelerate `grip_performance` tables of speed→distance |
| ai_behaviour_fire.xml | ai_behaviour_fire | Behaviour priority/index list |
| ai_choreographer.xml | ai_choreographer | Scripted mistakes/failures (triggers, chances), manager/start factors, conveyor belt |
| catchup.xml | catch_up | Rubber-band configs per difficulty/user (distance→power) |

Statistics tables (`lac`): brake grip 2.0 and 2.2, 14 points; e.g. 70.28 m/s →
132.48 m to stop (≈18.6 m/s² mean). Cruise (coast) and accelerate tables also
exist. Parser 0x777CF0 [V] loads them into three sub-objects (+4 brake,
+8 cruise, +0xC accelerate). Lookup 0x7A64C0 [V]: picks the two grip tables
around the requested grip (map keyed by grip), interpolates distance(speed)
in each, then linearly between grips. 0x7A6650(grip,v1,grip,v2) = D(v1)−D(v2)
(braking distance from v1 down to v2) [V].

### 1.4 `route.clm` [D]

Header: f32 origin (−632.99, −403.58), f32 cell 1.35065 × 1.35065, u32 2048,
10, 128, 0xFEFF, then zeros; 350,816 bytes. No `.clm` string exists in the
executable, and no AI consumer was found. **Not part of the line-following
pipeline** as far as traced; leave for a later surface/occupancy study.

### 1.5 Relation to progress_track.xml [D/I]

Independent data sets: AI gates (134, racing-line based, ~29 m spacing) vs
progress gates (71 + fine points, lap distance 4003.84). AI gate 0 is ~100 m
before progress gate 0. Same left→right orientation. The AI tracks its own
progress through AI gates/links (0x793F60 advances a per-car link index by
gate-plane crossing tests 0x778B40) [V-structure]; lap/position use the
progress track.

## 2. Code architecture (per AI car)

Addresses are function entries in the analysis image.

| Function | Role |
|---|---|
| 0x79F040 → 0x79EE30 | Per-car AI update (AI driver object, stride 0x4FE0 live, 12 instances incl. player slot) |
| 0x79C360 | "Track view" update: copies car pos/forward, advances gate/link progress (0x793F60), updates look-ahead distance (0x779A20), builds target point (0x798E90) |
| 0x79CAA0 → 0x79A400 | Behaviour arbitration: every behaviour returns a score (vtable +0x14); highest wins; activate/deactivate (+0x18/+0x1C); common setup 0x7978D0; behaviour-specific vfunc +0x20 or default drive 0x797B50 |
| 0x7978D0 | Per-frame request init (0x783490 reset), desired speed caps, grip multipliers, mistakes flag |
| 0x797B50 | Default "race" drive: lateral PID 0x7892E0, brake-line speed 0x792C80, heading error → request+0x88 |
| 0x78EDF0 / 0x78A4D0 / 0x78A610 | Race behaviour: "alone" test, random lateral offset, offset merge rate, throttle cap = throttle_confidence |
| 0x794310 → 0x7929D0 | Controls: brake 0x78E8A0, throttle 0x783180, steering 0x783330→0x77AC50, then slew 0x779F70, clamp 0x7FC030 |

### 2.1 Request (intent) struct (behaviour ctx +4; reset 0x783490) [V]

+0x10 brake-line provider (vtable; +0xC “valid”), +0x14 max_speed, +0x18
min_speed, +0x1C drift_speed, +0x20 distance_modifier, +0x24 hold_time,
+0x28 distance to next brake line, +0x2C speed factor, +0x30 brake-line target
speed, +0x34 brake-line id, +0x44/+0x48 left/right limit line ids, +0x60..+0x6C
steering target point, +0x70 followed vehicle, +0x74 follow distance, +0x78
follow brake, +0x7C throttle cap (default 1), +0x80/+0x84 desired speed
(default FLT_MAX), +0x88 signed heading error to target (rad), +0x8C
curvature ahead, +0x98 desired lateral offset (m), +0x9C offset merge rate
(m/s), +0xA0 brake cap (1), +0xA4.. flags (brake-to-desired, force brake,
no-steer, instant controls 0xA7, reverse 0xAF, …), +0xB8..+0xC4 grip
multipliers (= racingGripMultiplier), +0xC8 drift mode.

### 2.2 Parameter objects [V + live]

Driver config ctor 0x785660 (vtable 0xCF8C08) and vehicle config ctor
0x781AB0 (vtable 0xCF7EF8) build tables of 20-byte entries
`[type, name*, value, built-in default, -]` (type 1 float, 2 int, 3 bool,
0 section). Complete name→offset tables are private:
`local/ai-research/driver-config-layout.tsv`, `vehicle-config-layout.tsv`.
Live snapshot: **one** shared driver config (XML `default` set) and **one**
vehicle config (`default`, touring has no overrides) — values equal the XML,
e.g. laDistMin 10 (built-in 3), stAngleFactorMin 2.7, brakeCruiseExponent 24.

Per-driver mapped values: driver object +0x3010 is a table; entry
`+0x3010 + 4·idx` → holder (vtable 0xCAFC00, value at +8). Index order =
.data table 0xE4CE20 (follow_distance 1 … driver_view_angle 44; also
position_line_offset_ratio 29 (not in XML), drift_* 45–47).
Mapping evaluation 0x7A7A00 [V]: **bilinear** — interpolate on characteristic
within the difficulty-0 and difficulty-1 limit pairs, then interpolate by
difficulty (each clamped); no limits → `value`. Updater 0x793310 gets the
difficulty from a provider vfunc (+0x1C) and characteristic values from the
driver's characteristic list (+0x30F4).

## 3. Line following [V unless marked]

1. **Look-ahead distance** 0x779A20 (state +0x68, curvature input +0x6C):
   - speed ≥ laSpeedMin: `a = clamp01((|κ| − laAngleMax)/(laAngleMin − laAngleMax))`,
     `s = clamp01((v − laSpeedMin)/(laSpeedMax − laSpeedMin))`,
     `target = laDistMin + s·a²·(laDistMax − laDistMin)` (clamped).
   - speed < laSpeedMin: lerp laDistExtraMin (at laSpeedExtraMin) → laDistMin
     (at laSpeedMin).
   - Rate limit: step = clamp(|target − cur|·laStepProportion, 0, laStepMax)
     per update; final clamp [min(laDistMin, laDistExtraMin), laDistMax].
   - Live constants: 10/45 m, κ 0.002/0.035 rad/m, v 10/60 m/s, step 0.5,
     proportion 0.1, extra 5 m at 0 m/s.
2. **Curvature κ** (0x798E90): angle between the racing-line **laterals** at
   the car (+0x180) and at the look-ahead point (+0xC0), `acos(clamp(dot, −1, 1))`,
   negated when `(P_ahead − P_car) · lateral_car < 0`, divided by the look-ahead
   distance (rad/m) when it is positive → +0x6C, copied to request+0x8C (F454).
3. **Look-ahead point**: 0x788410 walks the link list from the car's current
   link/fraction by the look-ahead distance; link length = `L0 + L1·offset`
   (lateral-offset dependent) — i.e. distance **along the line**, not
   Euclidean.
4. **Lateral offset** 0x788030: current offset (+0x1A0) moves toward
   request+0x98 at `dt × request+0x9C` m/s; point = line point (+0x170) +
   lateral dir (+0x180) × offset; clamped by the selected limit lines
   (tgClampOffsetLeft/Right → racing limits) or tgOffsetMaxNoLimits.
5. **Race behaviour offsets** 0x78A4D0/0x78A610/0x77C570: "alone" = no car
   within raAloneDistanceOn 70 m (off 60 m). Every raOffsetGenerateTime 5 s
   (if raGenerateRandomOffsets) pick uniform random offset in
   ±deviation_from_racing_line_small (alone) or ±_large (in traffic); merge
   rate raOffsetMergeFactorAlone 0.2 m/s alone, raOffsetMergeFactor 1.0 /
   raOffsetMergeFactorCornering 0.5 otherwise; below vehicle slowSpeed 3 m/s
   offset = current and merge instant. Overtake etc. set ±1000 (= to limit).
6. **Lateral PID** 0x7892E0 (optional, behaviour flag): shifts the target
   point laterally by `(prediction·pidPredictionMultiplier(v) + PID(lateral
   error))·cornerFactor`, gains from vehicle config (P −1, I −0.5, D −0.3,
   limits 2/1/3/15, prediction −1 @10 m/s → 0.8 @20 m/s). Exact gating
   flag and the decompiler-lost factor are **[I]**.
7. **Heading error** (0x797B50): signed angle between (target − car
   position) and car forward → request+0x88.

## 4. Speed model [V]

### 4.1 Brake-line target speed (0x792C80)

- Uses the **next brake line** ahead (distance +0x28, minus
  driftBrakeLineAdvance 8 m in drift mode) and the vehicle class brake_data.
- `S = max(A · corner_speed_in_ratio, B)` with A/B from a context object
  (ctx+0x38 +4/+8) **[A/B source unresolved; candidate rubber-band/catch-up
  scale — I]**.
- `f = 1` normally; if not on the optimal line: tgNonOptimalSpeedFactor 0.8;
  off-line penalty `remap(max(|offset|, |dist from line|), 0..tgOffsetMaxForSpeed 4 m → 1..tgOffsetSpeedFactor 0.7)`.
- `target = clamp(min + (max − min)·f·S, min, max)`; drift mode uses drift_speed.
- Example (Normal, speed_in 1): S = 0.6454 → Redgate touring
  28.8 + 17.2·0.645 ≈ 39.9 m/s; hairpin 18 + 10·0.645 ≈ 24.5 m/s.

### 4.2 Brake-line braking (0x7886C0)

With v = speed, vt = target, b = D_brake(grip,v) − D_brake(grip,vt) from the
car's **brake** statistics table (grip = car state +0x144):

- `r = v/brake_on_speed · (1 − previous brake)` (pedal-ramp distance).
- `d = r + b` if b > r else `2b`; `d *= distance_modifier`.
- trigger = `d · corner_brake_distance_factor` (× mistake_brake_early/late
  factor during a choreographed mistake). Line activates when distance ≤ trigger.
- While active and v > vt: `brake = clamp01((dist − b)/(d − b))^brakeCruiseExponent`
  (pow at 0x788994, exponent 24; ratio forced 0 when d == b). Mistake types
  then apply heavy (max) / light (min) brake factors.
- v ≤ vt → 0; v < vt·brLineCancelFactor 0.8 → cancel line.
- **[I] behaviour:** at trigger the ratio is ≥1 (full brake); while braking
  fully r→0 so the threshold collapses toward b and full braking continues
  until v ≤ vt; if the car is late (dist < b) this path outputs 0. Validate
  against a live AI brake trace before relying on the late-case behaviour.

### 4.3 Throttle (0x783180 = min of limiters, then max with start-line minimum)

- previous brake > 0 → throttle 0; request flag 0xAC → 1.
- **Speed governor 0x77A4C0**: target = min(request desired speed,
  active-line target − thBrakeLineSpeedDrop 1.0, held last-line target for
  hold_time 1.5 s after passing it). Error e = v − target:
  e < −thTargetSpeedBig(1) → 1.0; e < −thTargetSpeedSmall(0.5) → goal
  thTargetMedium 0.4; |e| ≤ 0.5 → hold; 0.5 < e ≤ 1 → thTargetSmall 0.1;
  e > 1 → 0. Approach `cur + clamp(dt·thTargetMoveSpeed(4)·(goal−cur), ±|goal−cur|)`.
- Follow 0x77A630 (thFollow*), slide lift 0x77A880 (|slip| 0.2→0.6 rad →
  1→0.4 above 5 m/s), corner lift 0x77A940 (|κ| 0.01→0.03 rad/m → 1→
  thCornerLiftMin 0.2), request cap +0x7C = throttle_confidence (live 0.9606),
  traction control 0x77AA10 (tc* vehicle params), start stall/rev 0x775BF0.

### 4.4 Brake (0x78E8A0 = max of sources, capped by request+0xA0)

brake-to-desired (v − desired > brTargetSpeedBig 2 → 1), follow brake
0x77A1E0, forced flag, brake-line 0x7886C0, |slip| > π/2 → 1, 0x788A90
(avoidance, not analysed), then ABS cap 0x77A2E0 (min wheel/car speed ratio
0.7→0.3 → brake 1→absMinBrake 0.2 above 5 m/s).

## 5. Steering [V]

0x77AC50 (car state p6, vehicle config p7):

`steer = |slip|·stSlideFactor(0) + stAngVelFactor(−0.4)·(−ω·up) + G·headingError`

Signs (F391, [AI_DRIVER_PORT.md](AI_DRIVER_PORT.md) "Steering sign convention"): ω = body
+0x210 and up = body matrix row 1, unchanged. headingError is negative toward the car's
left (+X). The publisher 0x828670 → 0x996240 writes +E30 = −steer.

- **G is constant stAngleFactorMin (2.7)**: the code interpolates between
  cfg+0x40 and itself (cfg+0x54 Max is never loaded; checked in assembly at
  0x77AC84..0x77ACC7), so speed scheduling is inert in this build.
- If car state +0xE2 == 0 (all four wheels without contact, [V] F391) blend toward velocity
  direction by stProportionRabbitWhenInAir (0 → steer = slip angle).
- Divide by car max steering angle (state +0x118) → normalized; negate when
  reversing; clamp ±1 (0x783330).

## 6. Output smoothing and publication [V/I]

0x779F70 slews outputs toward the new commands at mapped rates (per second):
steering_speed 15, brake_on/off 5, throttle_on/off 5 (instant when request
0xA7). 0x7FC030 clamps a generic control packet: [0] throttle 0..1, [1] brake
0..1, [2],[2+1] bools, [3] gear request −1..6, [4] steer −1..1, [5] −1..1,
[6] 0..1. **[I]** This packet uses the same path as player input into car
+E1C/+E20/+E3C/+E40 ([REFERENCE_DRIVING_INPUT.md](REFERENCE_DRIVING_INPUT.md));
no AI code writes those fields directly (assembly scan 0x760000–0x7D0000).

## 7. Difficulty, characteristics, randomness, rubber band

- Live (Normal): all 12 driver objects evaluate with **difficulty 0.606**
  (from throttle_confidence 0.9 + 0.1·d = 0.9606) [V live+formula].
  Inverted characteristic values per car (speed_in/speed_out/aggression/
  mistakes/control/variance): car 0 all 1.0; others 0.1–1.0 (private table).
  Resulting Normal values: corner_speed_in_ratio 0.614–0.645, corner brake
  distance factor 1.118–1.218, deviation small 0.74–1.58 m, mistake
  probability 0.10–0.16, throttle_confidence 0.9606, steering_speed 15.
- Characteristic name table 0xCF6F38: aggression 1, control 2,
  reaction_speed 3, speed_in 4, speed_out 5. XML also uses `variance`,
  `reaction` and `mistakes`; how unmatched names are resolved is **[I]**.
- Source of difficulty 0.606 and per-driver characteristics (likely
  database/driver profiles) not traced.
- Randomness: random lateral offsets (above), start reaction/stall timers
  (0x775BF0 uses RNG 0xA683B0), choreographer mistakes/failures
  (ai_choreographer.xml triggers; mistake types feed brake factors).
- Catch-up (catchup.xml) parser 0x90A590 [V]: struct +4 enabled, +8/+0xC
  min/max distance, +0x10/+0x14 power at min/max, +0x18 minSpeed, +0x1C
  minRaceDistance, flags, +0x2C reverseEnabled, +0x30/+0x34 reverse distances,
  +0x38/+0x3C reverse powers. **Consumer not found**; ctx+0x38 (A,B) in §4.1 is
  the prime candidate [I].

## 8. Awareness/avoidance (overview only)

Behaviours (ai_driver_config `behaviour_*`/`test_*`): race, race_damaged,
recovery, recover_tbone, alongside, avoid, retire, draft, visibility_loss,
block, overtake, race_start, cruise, controlled_stop, lost_control, drift.
Arbitration by score (0x79A400). Relevant parameters: ov* (overtake: reject
60 m, side 12 m, speed difference 0.1, time 3 s), as* (alongside), av*
(avoidance ETA 2 s, buffer 2 m), bl* (block), follow distances
(mapping follow_distance 10, safe separation 10/2.5), maxObservableVehicles
5 (2 rear) within 200 m. Not analysed beyond parameter wiring.

## 9. Evidence (private)

- Decompilation: `local/ai-research/decomp/{cfg1,drv1..drv8,map1..map6,cu1,ra1..ra3}`;
  helper script `local/ai-research/ghidra/GridAiDecomp.java` + `run.ps1`
  (read-only project open).
- Layout tables: `local/ai-research/driver-config-layout.tsv`, `vehicle-config-layout.tsv`.
- Live (PID 7208): `local/ai-research/live/mappings-04.json` (SHA-256
  03dd732c…), `ptr-drivercfg-01.json` (14a673ad…), `drivercfg.bin`
  (1afa96f6…), `vehcfg.bin` (eea61c7d…), `driver00..11.bin`
  (e.g. 00: 1ab3a44f…), decoded TSVs.
- Tool: [tools/read_grid_ai_mappings.ps1](../tools/read_grid_ai_mappings.ps1)
  (VM_READ|QUERY_LIMITED_INFORMATION only; identity/hash and live-code prefix
  checks; 64 KiB windows with 4 KiB fallback). The first scan without page
  fallback missed most objects (17 vs 46 mappings); corrected.

## 10. Open questions / next steps

1. Live AI pedal/steer trace through one braking zone (read-only sampler of an
   AI car's control packet and request struct) to validate §4.2 behaviour.
2. Identify ctx+0x38 (A,B) and the catch-up consumer.
3. Trace the target-point builder internals (0x787870/0x7878F0/0x797190) and
   the PID gating flag; confirm sign conventions of heading error and yaw term.
4. corner_speed_out_ratio consumer; avoidance 0x788A90; overtake/alongside.
5. Difficulty provider and driver characteristic source.

## Corrections (from the overtaking research, F294)

- The waypoint type numbers are zero-based: racing_line 0, left_racing_limit 1,
  right_racing_limit 2, left_track_limit 3, right_track_limit 4.
- The characteristic numbers start at 0 (aggression 0, …).
- AI view +0xE0 is the planned (smoothed) offset at the car, not a measured one.
