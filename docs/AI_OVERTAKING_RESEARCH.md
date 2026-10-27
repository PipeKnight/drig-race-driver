# Original AI overtaking and side-by-side racing — research (F-AI-02)

15 October 2026, Claude Code. Static analysis only of the hash-identified private analysis
PE `local/compatibility/grid-unpacked.exe` (SHA-256 `aac46a64…4f34514`, base 0x400000),
continuing [AI_DRIVER_RESEARCH.md](AI_DRIVER_RESEARCH.md) (line following, speed, controls)
and [AI_DRIVER_PORT.md](AI_DRIVER_PORT.md) (Rust port). The running original game process
was not touched; no game file, Steam install or Ghidra project was modified (Ghidra 12.1.4
headless, `-readOnly`, temporary functions in memory only). Decompiler output, disassembly
dumps and helper scripts are private under ignored `local/ai-overtaking/`. This document
contains only addresses, field offsets, constants and structural descriptions.

Labels: **[V]** verified in disassembly/decompilation; **[D]** shipped data;
**[I]** inference, not verified.

Notation: `cfg+X` = shared driver config value at offset X (offsets as in
`local/ai-research/driver-config-layout.tsv`, which are value offsets [V]); the config getter
0x7A3840(i) returns `cfg+0x18+0x14·i` [V], so getter indices map to the same table
(e.g. 0x54 → 0x6A8 ovSpeedDifference). `vcfg+X` = vehicle config value. `map[k]` = per-driver
mapped value with the 1-based index of the .data table 0xE4CE20 (driver +0x3010, holder value
at +8); accessor 0x7A3820(out, i) reads `map[i+1]` (0-based) [V].

## 1. Corrections to earlier findings

- Waypoint type enumeration (lookup 0x778FB0, value/name pairs at 0xCF71B4/0xCF71B8) [V]:
  racing_line 0, left_racing_limit 1, right_racing_limit 2, left_track_limit 3,
  right_track_limit 4, left_driveable_limit 5, right_driveable_limit 6. The list in
  AI_DRIVER_RESEARCH §1.1 is shifted by one (`shortcut` belongs to the preceding table).
- Characteristic enumeration (0x775660, pairs at 0xCF6F34/0xCF6F38) [V]: aggression 0,
  control 1, reaction_speed 2, speed_in 3, speed_out 4, (6 entries). AI_DRIVER_RESEARCH §7
  lists them from 1.
- Track view sample 0 (view +0x90 block, offset at view **+0xE0**) is the *planned* lateral
  offset at the car's link, not a measurement: 0x7878F0 moves it toward request +0x98 by at
  most `dt·request+0x9C` per update (instant on the behaviour's first frame, flag +0xAE),
  stops it at a reached racing limit (flags view +0x238/+0x239), and re-projects the old
  planned point when the car changes link [V]. View +0xD0 = line point + lateral × that offset.
  Sample 2 (+0x150 block, offset +0x1A0) is the look-ahead equivalent (0x788030).
  This is the port's `offset` state. (F606: with the correction below, +0xE0 is the look-ahead
  offset, the port's `offset`; +0x1A0 is the car-link offset, the port's `offset_here`, which
  the lateral PID error and the room at the car read; FIRST_CORNER_LIVE §11.)
- **Correction (2026-10-26, drift-battle-ai):** the sample positions in this and the previous
  bullet are swapped, as AI_AVOID_RECOVERY_RESEARCH §1 already found for the blocks. 0x798E90
  builds the +0x90 block (+0xE0) at the look-ahead link and the +0x150 block (+0x1A0) at the
  car's link, and fills +0x218/+0x21C (track limits) and **+0x220/+0x224 (racing limits) from
  the +0x150 block, at the car**, then +0x228/+0x22C (track) and **+0x230/+0x234 (racing)
  from the +0x90 block, at the look-ahead** [V]. 0x797190 returns (planned offset of that
  block + line length) − left limit and right limit − that. So +0x230/+0x234 is the room at
  the look-ahead; the race start (0x78A740, 0x776740), the overtake side choice (0x77BD20),
  the yield (0x789780) and the alongside squeeze (0x799630) read it. The text below keeps the
  old wording.
- View +0x230/+0x234 [V] (0x797190 called from 0x798E90 with types 1/2 at the car sample):
  `abs = planned offset + racing-line length` (lengths from the left track edge);
  +0x230 = abs − left_racing_limit, +0x234 = right_racing_limit − abs, i.e. free lateral room
  from the planned path at the car to the left / right racing limit. Same at the look-ahead
  sample: +0x220/+0x224 (racing limits), +0x218/+0x21C (track limits types 3/4); car track
  limits +0x228/+0x22C.
- Sign conventions [V+I]: offsets grow toward the right limit (lengths are measured left→right
  along the gate normal). Curvature (0x798E90) is positive for a bend toward the positive-offset
  side (derived from the sign rule `dot(P_car − P_ahead, L_ahead)`; worked check with a circular
  arc) [I, strongly supported].
- No ±1000 desired offset is used by overtake or alongside (all stores to request +0x98 in
  0x770000–0x7A8000 listed; overtake pushes incrementally) [V]. The ±1000 convention noted in
  AI_DRIVER_RESEARCH §3 was not found in these behaviours.

## 2. Per-tick structure [V]

Per AI car (0x79EE30 → 0x79C360 track view → **0x79CAA0**):

1. **Tests** 0x792ED0: clears result slots, runs the path-obstacle scan 0x78B560, then calls
   update (vfunc +4) of each enabled test in type order 0..20 (test array = owner +0x1C4,
   slot = 4·type; a nonzero return ends the loop).
2. **Arbitration** 0x79A400: for each behaviour in the list (mgr +0x1D4..+0x1D8):
   `prev = +0x14 ← +0x10; +0x10 = score(ctx at mgr+0x1C0, dt)` (vfunc +0x14). Best = first
   strictly greater than −FLT_MAX (ties keep list order). If the previous active behaviour
   (mgr +0x320) differs: clear its +0x18 flag and call deactivate (+0x1C). If the best is not
   flagged active: set +0x18 and call activate (+0x18). Then common setup **0x7978D0**; unless
   that returns nonzero, the behaviour drive (+0x20) runs, and if it returns 0 the default race
   drive **0x797B50** (lateral PID, brake-line speed, heading error) runs after it. All drives
   used here return 0, so behaviours only edit request fields and the shared driving pipeline
   produces the controls.
3. Controls 0x794310 (unchanged, see AI_DRIVER_RESEARCH §4–6).

Common setup 0x7978D0 [V]: request reset 0x783490 (+0x70 follow vehicle 0, +0x74 follow
distance 0, **+0x78 follow-brake cap 1.0**, +0x7C throttle cap 1.0, +0x80/+0x84 desired speed
FLT_MAX, +0x98 desired offset 0, +0x9C merge 1.0, flags 0); first-frame flags +0xAD/+0xAE;
grip multipliers = vcfg racingGripMultiplier; cruise test → desired speed vcfg
endOfRaceCruiseSpeed; `desiredSpeed` config cap; **0x789700 follow from brake_for_vehicle**
(§5); merge **+0x9C = tgOffsetMergeFactor** (cfg+0x388, 2.5); limit-line types +0x48/+0x4C =
1/2 (racing limits).

Behaviour context (drive/score argument) [V]: +4 request, +8 per-behaviour limit-type holder,
+0x10 driver cfg, +0x14 observed-vehicle list (vfunc +0x10 get(i), +0x14 find(id),
+0x1C count), +0x18 track view, +0x24 own car state, +0x28 mapped values, +0x2C vehicle cfg,
+0x30 (+4 → test array), +0x40 dt (double). Test context: +4 path obstacle, +8 cfg, +0xC
list, +0x1C own car state, +0x28 mapped values, +0x2C vehicle cfg.

## 3. Behaviour arbitration [V]

Type enumeration (0x776190, value/name pairs at 0xCF6FA0/0xCF6FA4, 16 entries): overtake 0,
race 1, recovery 2, alongside 3, retire 4, draft 5, visibility_loss 6, block 7, avoid 8,
race_start 9, cruise 10, controlled_stop 11, recover_tbone 12, lost_control 13,
race_damaged 14, drift 15.

**Priorities**: parser 0x79B430 reads `ai_behaviour_fire.xml` child order; each recognised
child appends the running position (1-based float) to that type's list (map entry: +0 count,
+4.. values); the `index` attribute is not read. Types absent from the file fall back to a
default entry (key −1, four values 0.5). Base constructor 0x792F90 stores the entry pointer at
behaviour +4; score functions return its first value. From the shipped file [D]:

| Score | Behaviour (first value; later values) |
|---|---|
| 1 | race |
| 2 | drift (12, 13) |
| 3 | cruise |
| 4 | draft |
| 5 | **overtake** |
| 6 | visibility_loss |
| 7 | block |
| 8 | **alongside** |
| 9 | race_damaged |
| 10 | retire (23) |
| 11 | recovery (16, 17, 19) |
| 14 | race_start (22) |
| 15 | **avoid** |
| 18 | recover_tbone |
| 20 | controlled_stop |
| 21 | lost_control |

Behaviour object: +0 vtable, +4 priority entry, +8 owner arg, +0xC type, +0x10 score,
+0x14 previous score, +0x18 active flag, +0x1C sub-object (vtable 0xCF8DD0), +0x28 = 1.0.

| Type | Vtable | score +0x14 | activate +0x18 | deactivate +0x1C | drive +0x20 |
|---|---|---|---|---|---|
| overtake | 0xCF8E50 | 0x78A2C0 | 0x77BCF0 | 0x7766B0 | 0x78A330 |
| race | 0xCF8E74 | 0x78A490 | 0x7766D0 | 0x7766F0 | 0x78EDF0 |
| alongside | 0xCF9008 | 0x788E00 | 0x7762C0 (empty) | 0x7762D0 (empty) | 0x79C5A0 |
| avoid | 0xCF902C | 0x788E30 | 0x77B610 | 0x776320 | 0x79C600 |
| block | 0xCF8E08 | 0x788E70 | 0x77B700 | 0x77B720 | 0x792B90 |
| draft | 0xCF8E2C | 0x789D90 | 0x776660 | 0x776670 | 0x789DD0 |
| race_damaged | 0xCF8E98 | 0x78A690 | | | 0x799FB0 |
| race_start | 0xCF8EBC | 0x78A6D0 | 0x78A8F0 | 0x776730 | 0x78A740 |
| controlled_stop | 0xCF8EE0 | 0x78AA50 | | | 0x78AA90 |
| cruise | 0xCF8F04 | 0x78AAE0 | 0x7836E0 | | 0x783660 |
| recovery | 0xCF8F28 | 0x78AB20 | | | 0x78EE70 |
| recover_tbone | 0xCF8F4C | 0x78ACC0 | | | 0x776A00 |
| lost_control | 0xCF8F70 | 0x78B180 | 0x78B1F0 | | 0x776B80 |
| retire | 0xCF8F9C | 0x78B300 | 0x77D570 | | 0x79C6E0 |
| drift | 0xCF8FC0 | 0x78B360 | | | 0x78F490 |
| visibility_loss | 0xCF8FE4 | 0x78CBD0 | 0x7776F0 | 0x777710 | 0x78CC10 |

Score rules used here:
- race: test_race fired → 1 (test_race 0x77E380 sets fired from a string compare 0xA66720
  against a global name; its meaning is [I], in practice it fires during racing).
- **overtake** 0x78A2C0: 5 if brake_for_vehicle test exists and its *pass vehicle* (+0x14) is
  set; else 5 if test_attempt_overtake fired; else 5 while the behaviour's minimum-fire timer
  (+0x3C) > 0; else 0.
- **alongside** 0x788E00: 8 if test_alongside fired.
- **avoid** 0x788E30: 15 if test_avoid fired.
- block 0x788E70: 7 if test_block fired (then +0x34 = 1 s hold); if behaviour +0x30 ≠ 0 the
  score persists while the hold counts down by dt.

Default enabled tests (`ai_driver_config.xml` `default/behaviourTest`, flags cfg+0xCC4+type)
[D]: alongside, spun, stop, wait_for_start, cruise, race, attempt_overtake,
brake_for_vehicle, retire, collided_obstruction, contacted_vehicle, on_track,
visibility_loss, avoid, cornering, race_finished, is_terminally_damaged, lost_control.
**Disabled: draft, block, drift** — so draft and block never fire in a default race.

## 4. Tests [V]

Test object: +0 vtable, +4 type, +8 fired, +9 enabled; vtable = {+0 0x7157D0, +4 update,
+8 on-stop-firing}. Factories 0x7A6D50..0x7A7950 (allocation size, type):

| Type | Name | Vtable | Update | Size |
|---|---|---|---|---|
| 0 | alongside | 0xCF73C8 | 0x795C00 | 0x28 |
| 1 | spun | 0xCF7450 | 0x77E5B0 | 0x10 |
| 2 | stop | 0xCF745C | 0x77E7D0 | 0x10 |
| 3 | wait_for_start | 0xCF7474 | 0x77EC10 | 0x10 |
| 4 | cruise | 0xCF7414 | 0x77E060 | 0xC |
| 5 | race | 0xCF7438 | 0x77E380 | 0xC |
| 6 | attempt_overtake | 0xCF73E0 | 0x77DB90 | 0x10 |
| 7 | brake_for_vehicle | 0xCF73F8 | 0x78F6F0 | 0x1C |
| 8 | retire | 0xCF7444 | 0x784560 | 0x30 |
| 9 | draft | 0xCF7420 | 0x77E0F0 | 0x10 |
| 10 | collided_obstruction | 0xCF7404 | 0x776FE0 | 0x10 |
| 11 | on_track | 0xCF742C | 0x79B250 | 0x10 |
| 12 | visibility_loss | 0xCF7468 | 0x77E8A0 | 0x10 |
| 13 | block | 0xCF73D4 | 0x77D8D0 | 0x10 |
| 14 | avoid | 0xCF73EC | 0x79A960 | 0x30 |
| 15 | cornering | 0xCF748C | 0x77ED00 | 0x10 |
| 16 | race_finished | 0xCF7480 | 0x77EC80 | 0xC |
| 17 | contacted_vehicle | 0xCF7498 | 0x784650 | 0x20 |
| 18 | is_terminally_damaged | 0xCF74A4 | 0x777600 | 0xC |
| 19 | lost_control | 0xCF74B0 | 0x77ED50 | 0x14 |
| 20 | drift | 0xCF74BC | 0x78FE90 | 0x1C |

## 5. Perception

### 5.1 Records [V]

Each other car is seen through an *observed record* filled from that car's AI car state
(0x777940 + 0x777A80/0x77F970): rec +0x10 = speed (state +0xD0), +0x20 centre position
(+0x20), +0x30 forward (+0x30), +0x40 right (+0x50), +0x90 velocity (+0xA0), +0xA0 heading
(+0xB0), **+0xC0 id**, **+0xD0 front point** (+0xF0), **+0xE0 rear point** (+0x100),
**+0xF0 half width** (+0x110), +0xF4 (+0x114), +0xF8 (+0x140), **+0xFC motion state**
(+0x14C; constructor 0x777A20 sets 3). Front/rear naming is inferred from the yield test
(§7.3), where a larger respect fraction makes the AI yield more [I, strongly supported].
State 0 = racing normally (all side checks require it); state 2 = moving toward us
(attempt-overtake oncoming rule) [I].

Own car (context +0x1C/+0x24) uses the car-state layout directly: +0x20 centre, +0x30
forward, +0x50 right, +0xA0 velocity, +0xB0 heading, +0xD0 speed, +0xF0 front point,
+0x100 rear point, +0x110 half width, +0x144 grip.

### 5.2 Observable list

- maxObservableViewDistance (cfg+0xC0C, 200), maxObservableVehicles (+0xC20, 5),
  maxObservableVehiclesRear (+0xC34, 2) are copied by 0x79F550 to the observer manager
  (+0x54/+0x58/+0x5C) and by 0x790360 to every per-driver observer (+0x20 distance,
  +0x18 maximum, +0x1C = min(maximum, rear)) [V]. The selection routine that applies them
  was **not located** (open). The tests iterate whatever this list contains.
- All distances below are **Euclidean / projections on the car axes**, not along-track: the
  overtake, alongside and brake tests use bumper points and own forward/right vectors; only
  the side choice and limit room use the planned racing-line offset (§1) [V].
- `driver_view_angle` (map[44], 50) is used by brake_for_vehicle only: 0x7A2F30 tests whether
  any of the obstacle's four corners lies within `angle·π/180` (clamped 0..π) of own forward,
  seen from own rear point [V].
- Path obstacle 0x78B560 (start of every test pass): for each observed car (skipping cars
  where both are slower than a threshold) it builds the car's box and tests it against the
  rectangles of the AI's predicted path, using the centre for cars with a dissimilar heading
  (`|dot(headings)| ≤ |cos(avSimilarDirectionMaxAngle)|`) and the leading bumper otherwise;
  the nearest hit along the path is stored in the test context (+4: point, direction,
  +0x20 record) [V structure; details I].

## 6. brake_for_vehicle test and follow fields

### 6.1 Test 0x78F6F0 [V]

Outputs: +8 fired, **+0xC brake target id**, **+0x10 follow distance**, **+0x14 pass vehicle
id**, +0x18 pass side (1 = clear on the +right side). Uses the path obstacle record `r`.

1. Skip if there is no path obstacle or it fails the view cone (§5.2).
2. **Pass check** — if `dot(own heading, r heading) > |cos(avPassAngle)|` (0.15 rad):
   `lat = dot(r.centre − own.centre, own right)`, `W = r.halfW + own.halfW`,
   `gapR = lat − W`, `gapL = −lat − W`, `buf = vcfg brakeBufferZone` (1.0).
   If `gapR > buf or gapL > buf`: pass is allowed when r is not racing (state ≠ 0) or
   `r.speed < avPassSlowSpeed` (15); otherwise when
   `|dot(r.vel − own.vel, own right)| < remap(closing, 0..avPassMaxRelativeSpeed(20) →
   allowed..0)` with `allowed = remap(max(gapL,gapR) − buf, 0..avPassMaxSpareRoom(4) →
   avPassSideSpeedNear(2)..avPassSideSpeedFar(10))`. On pass: +0x14 = r id, +0x18 = gapR > buf.
3. **Brake check** (only when no pass vehicle):
   - same heading (`dot > |cos(avSimilarDirectionMaxAngle)|`, 0.5): `dist` = gap from own
     front to r's rear along own heading (0x7A2D30), `closing = own.speed − r.speed`;
     otherwise a path-relative distance from r's nearest corner (detail I).
   - `sep = remap(closing, 0..avSeparationSpeedDiff(5) → map[3] ssd_small(2.5)..map[2]
     ssd_large(10))`, `sep2 = remap(own.speed, 0..avSlowSpeed(40) → 0..sep)`,
     `avail = max(|dist| − sep2, 0)`.
   - `mult = remap(own.speed, 0..avSlowSpeed → 1 .. vcfg brakeExtraPrediction(0) +
     map[41] brake_for_vehicle_prediction_ratio(3))`.
   - 0x78CA20 decides it is still safe when `closing ≤ 0`, or when
     `(avail/closing)·v > (v/map[7] brake_on_speed · avPercentagePedalPushDistance(1.2) +
     D_brake(grip, v) − D_brake(grip, clamp(v − closing, 0, v)))·mult`
     (D = statistics braking-distance table, as the brake-line logic).
   - If not safe: fired, +0xC = r id, +0x10 = 100; if same heading and
     `closing < map[42] brake_for_vehicle_cruise_speed` then +0x10 = sep2.
4. On stop-firing (0x776F90) all outputs clear.

`braking_buffer_distance` (map[32]) was not seen in these paths (open).

### 6.2 Follow fields (request +0x70/+0x74/+0x78) [V]

Writers:
1. 0x789700 (common setup, every behaviour): if brake_for_vehicle fired and its target
   record exists and is racing (state 0) or is not the car flagged alongside on that side →
   +0x70 = record, +0x74 = test +0x10 (100 → "brake now", or sep2 → cruise behind).
2. Overtake state 4 (§7.4): +0x84 = target speed; if +0x70 empty, +0x70 = target,
   +0x74 = ovFollowDistance (3).
3. Alongside yield 0x789780 (§8.3): +0x70 = nearer yielded car, +0x74 = map[27]
   alongside_yield_follow_distance, **+0x78 = map[24] alongside_yield_max_brake**.

Consumers (already in the port): follow throttle 0x77A630 uses
`gap = |own.front − f.rear| − +0x74`; follow brake 0x77A1E0 applies when
`dot(f.rear − own.front, own fwd) − +0x74 < 0` and `own.speed > minimumMovingSpeed (1)`,
**amount = min(+0x78, brFollowSpeedBrake)** — i.e. 1.0 (full) unless the yield set a lower cap.
So a firing brake_for_vehicle (follow distance 100) means: throttle 0 and full brake until the
stopping check passes again. The port's closing-speed-proportional follow brake is authored and
should be replaced.

## 7. Overtake

### 7.1 attempt_overtake test 0x77DB90 [V]

Output +0xC target id; previous target kept for hysteresis. For every observed record `r`,
with own front `F`, own forward `f` (normalised xz), `d = r.rear − F`:
`along = dot(d, f)`, `lat = |dot(d, r.right)|`, `closing = own.speed − r.speed`.

- Candidate if `0 < along < best` (best starts at **ovRejectDistance 60**, then the selected
  `along`, so the nearest qualifying car wins) and `lat < ovSideRejectDistance 12`, and:
  - same id as the previous target: `closing > ovContinueSpeedDifference (−2)`;
  - new target: `along < ovAutoAcceptDistance (3)`; or `closing > ovSpeedDifference (0.1)`
    and `along/closing < ovTime (3 s)`; or (`closing ≤ 0.1`) `along < ovSlowRejectDistance
    (12)` and `r.speed < ovSlowSpeed (12)`.
- Oncoming rule (does not update best): `r.state == 2`, `0 < along < 4·ovRejectDistance`,
  `dot(own.vel, r.vel) < 0`.
- When it stops firing it calls +8 (clears +0xC).

### 7.2 Behaviour state [V]

Overtake object: +0x30 target id, +0x34 state, +0x38 decision timer, +0x3C minimum-fire
timer, +0x40 corner factor, +0x44 bend flag (κ < −ovMinCornerCurvature), +0x45 bend flag
(κ > ovMinCornerCurvature). Activate 0x77BCF0: +0x30/+0x34/+0x38 = 0,
**+0x3C = ovMinFireTime (3 s)**. Deactivate 0x7766B0: +0x3C = 0.

Drive 0x78A330 per tick:
1. Bend flags and `corner = clamp01((|κ| − ovMinCornerCurvature)/(ovMaxCornerCurvature −
   ovMinCornerCurvature))` (0.001/0.04 rad/m).
2. Target = brake_for_vehicle pass vehicle (+0x14) if set, else attempt_overtake +0xC.
3. No target → state 5. Else `+0x38 += dt`; if `+0x38 > ovDecisionTimeOut (0.25 s)` or the
   target changed: oncoming (state 2) → state 1; otherwise **side choice** 0x77BD20; reset
   +0x38.
4. Execute 0x77C180; `+0x3C −= dt`.

### 7.3 Side choice 0x77BD20 [V]

Planned path line through view +0xD0 (planned point at the car) and +0x190 (planned
look-ahead point). `dist` = perpendicular distance of the target centre from that line;
`side` = target lies on the positive-offset side (cross product with the line direction;
naming by consistency with the room terms below [I]). `w = r.halfW + own.halfW`.

- `dist > w + ovDesiredSeparationDistance (2.5)` → **state 3** (already clear: hold).
- `dist > w + ovSafeSeparationDistance (1.0)` → state 1 if target on + side, else 2
  (move away from it).
- Overlapping: `s = ±dist` (+ when target on + side), `need = w + ovSafeSeparation`,
  `roomL = view+0x230 + s`, `roomR = view+0x234 − s`.
  Keep the current state 1 while `roomL > need`, state 2 while `roomR > need`. Otherwise:
  both rooms → state 1 if `s > own.halfW`, state 2 if `s < −own.halfW`, else the side with
  more room (`roomL > roomR` → 1); only left room → 1; only right → 2; **none → state 4**.

States: 1 = move toward negative offset (left), 2 = toward positive (right), 3 = hold,
4 = no room, follow, 5 = no target.

### 7.4 Execute 0x77C180 [V]

- States 1/2: **request +0x98 = view+0xE0 ∓ dt·ovOffsetMergeFactor** (planned offset at
  the car pushed further each tick; minus for state 1). Merge **+0x9C = lerp(ovOffsetMerge
  Factor 4, ovOffsetMergeFactorCorner 0.5, c)** clamped between them, where `c = corner` only
  if `own.speed > ovSlowSpeed (12)` and the move goes toward the outside of the bend
  (state 1 with +0x45, state 2 with +0x44), else 0. Net effect: the planned offset ramps
  sideways at ≤ 4 m/s (≈0.5 m/s when widening in a bend) until the racing-limit clamp of
  0x7878F0/0x788030 stops it.
- States 3/5: +0x98 = view+0xE0 (hold the planned offset).
- State 4: +0x84 desired speed = target speed; follow target at ovFollowDistance if no
  follow vehicle is set.
- No change of throttle cap: the race drive is the only writer of +0x7C = throttle_confidence
  (0.9606 at Normal), so **overtake and alongside run with throttle cap 1.0** [V].
- Brake-line speeds keep running (default drive 0x797B50); the off-line factor
  tgOffsetSpeedFactor lowers corner speeds when far off line.

### 7.5 End [V]

Overtake stops when no test fires and the 3 s minimum-fire timer has run out, or when a
higher priority behaviour (alongside 8, avoid 15, …) wins (deactivate resets the timer).
During the timer tail without a target it holds the planned offset (state 5); afterwards the
race behaviour reactivates (activate 0x7766D0 sets its +0x30 = 0 and +0x34 = −1 [V]; that this
forces an immediate new random offset is [I]) and merges back at raOffsetMergeFactor
(1.0, cornering 0.5).

## 8. Alongside

### 8.1 Test 0x795C00 [V]

Outputs: +0xC gap on the negative (left) side, +0x10 gap on the positive side (FLT_MAX when
none), +0x14/+0x18 ids, +0x1C/+0x20 records, +0x24/+0x25 strict-overlap flags, +8 fired.
Hysteresis: the previous fired flag selects on- or off-distances
(map[17]/[18] front on/off 1.0/1.2, map[19]/[20] rear on/off 1.0/2.0).

For each record within **asRejectDistance (10 m, centre to centre)**:
- alongside if `dot(r.rear − (F + f·front), f) < 0` and `dot(r.front − (R − f·rear), f) > 0`
  (own front `F`, rear `R`, margins as above) → test fires;
- strict overlap flag: the same without margins;
- `lat` = signed lateral of r.rear relative to own front along own xz right;
  `gap = max(|lat| − (r.halfW + own.halfW), 0)`;
- slot = negative side if `lat ≤ 0`, else positive; keep the smallest gap, but once a slot
  holds a strict-overlap car only another strict-overlap car can replace it.
- Afterwards, an empty slot gets gap 0 when the own car's side edge
  (`centre ± right·halfW`) lies beyond that side's racing limit segment (types 1/2).

### 8.2 Behaviour 0x79C5A0 → 0x799630 [V]

Merge **+0x9C = asOffsetMergeFactor (4)**. `cur = view+0xE0`. Mapped minSpace = map[21]
(alongside_min_space_to_leave_distance), maxSpace = map[22].
- Car on exactly one side (strict flags, else slot ids): with `g` its gap:
  - `g < minSpace` → move away by `minSpace − g`;
  - else if the far-side room at the look-ahead is short
    (`view+0x234 − ((view+0x224 + asTargetBuffer(−2)) − g) < minSpace` for a car on the
    positive side, mirrored with +0x230/+0x220) → move away by minSpace and set
    "squeezed" (+0x30);
  - then if the result is still displaced toward that car's side and `g > maxSpace` →
    desired offset **0** (return to the racing line); else desired = result.
- Cars on both sides (or flags equal): desired offset = mean of two lateral values, one per
  car, obtained through 0x798C00/0x798D00 with the racing-line waypoint type (each car's offset
  from the racing line [I]); if either is invalid (≥ 2000) the fallback is own projection on the
  look-ahead line minus the left gap and plus the right gap [V structure]; 0 if a car record
  cannot be resolved.
- If `map[23] alongside_yield_distance > 0` → yield 0x78EC90/0x789780.
- asOffsetMultiplier (cfg+0x450) has no driver-config consumer in the traced code (open).

### 8.3 Yield 0x789780 [V]

Requires test_alongside fired and `own.speed > coYieldMinSpeed (16)`. For each slot car,
0x77B7D0 yields when: gap < map[23] yield_distance, `map[25] min_speed_diff <
r.speed − own.speed < map[26] max_speed_diff`, r racing (state 0), and the point
`r.rear + (r.front − r.rear)·map[28] extra_respect` is ahead of own front along own forward.
If only one side yields, it applies only if the other side is also tight
(`yield_distance > other gap`) or own room to the opposite racing limit
(`view+0x234` or `+0x230` minus own halfW) is below coYieldDistToLimit (1.0). Then: follow the
nearer yielding car (+0x70), +0x74 = map[27] yield_follow_distance, +0x78 = map[24]
yield_max_brake (follow-brake cap), flag +0xB2 when |own state +0xD4| > 0.1, behaviour +0xC = 1.

## 9. Avoid, block, others (brief)

- **avoid** (score 15): test 0x79A960 fires on a record whose time to contact
  (distance/closing) < avETAReactionTime (2 s), storing id/record/ETA/closing/relative
  velocity; cars that pass a "racing normally" check (avMinRacingSpeed, avMaxRacingAngleFrom
  Track) are excluded [V structure, I details]. Drive 0x79C600: every avDecisionTime (0.25 s)
  or target change, 0x799B30/0x799DE0 predict the target position at the ETA and pick a lateral
  offset with clearance avBufferZone + both half widths; 0x77B5A0 picks a target speed
  (avStoppedTargetSpeed if the target is slower than avStoppedSpeed, avNormalTargetSpeed if
  own speed ≥ test closing, else avWrongWayTargetSpeed unless figure-of-eight) written to +0x84
  with the brake-to-speed flag +0xA4; limit types switch to track limits (3/4); offset and
  merge avOffsetMergeFactor (5) every tick.
- **block** (score 7, test disabled by default): drive 0x792B90 mode (0x77B690): 3 if the next
  brake line is farther than blBrakeLineDistMin → mirror the blocked car's lateral position
  (0x788F10); 1/2 when a bend ahead exceeds blMinCornerAngle → push the offset by
  `dt·blOffsetMergeFactor` to cover the inside; merge blOffsetMergeFactor.
- draft (test disabled), visibility_loss (score 6), recovery and others are not needed for
  passing.

## 10. Values used at Normal [D + formula]

Bilinear mapping (difficulty d = 0.606, characteristic c = 1 as in the port):

| Mapping (index) | Value |
|---|---|
| alongside_front_projection_on/off (17/18) | 1.0 / 1.2 m |
| alongside_rear_projection_on/off (19/20) | 1.0 / 2.0 m |
| alongside_min/max_space_to_leave (21/22) | 2.5 / 5.0 m |
| alongside_yield_distance (23) | 4.0 + (1.0 − 4.0)·d = 2.18 m |
| alongside_yield_max_brake (24) | 0.9 − 0.5·d = 0.597 |
| alongside_yield_min/max_speed_diff (25/26) | −20 + 10·d = −13.9 / 10 − 5·d = 6.97 m/s |
| alongside_yield_follow_distance (27) | 2 − 2·d = 0.79 m |
| alongside_yield_extra_respect (28) | 1.9 − 1.4·d = 1.05 |
| safe_separation_distance_large/small (2/3) | 10 / 2.5 m |
| brake_for_vehicle_prediction_ratio (41) | 3.0 |
| brake_for_vehicle_cruise_speed (42) | 0.5 + 2.5·d = 2.02 m/s |
| driver_view_angle (44) | 50° |

Config (XML `default`, else built-in): ovOffsetMergeFactor 4, ovTime 3, ovRejectDistance 60,
ovSideRejectDistance 12, ovSpeedDifference 0.1, ovContinueSpeedDifference −2,
ovFollowDistance 3, ovDecisionTimeOut 0.25, ovSafeSeparation 1, ovDesiredSeparation 2.5,
ovMinFireTime 3, ovMin/MaxCornerCurvature 0.001/0.04, ovOffsetMergeFactorCorner 0.5,
ovSlowSpeed 12, ovSlowRejectDistance 12, ovAutoAcceptDistance 3; asRejectDistance 10,
asOffsetMergeFactor 4, asTargetBuffer −2; coYieldMinSpeed 16, coYieldDistToLimit 1;
avPassAngle 0.15, avSimilarDirectionMaxAngle 0.5, avPassSlowSpeed 15, avPassMaxSpareRoom 4,
avPassSideSpeedNear/Far 2/10, avPassMaxRelativeSpeed 20, avSeparationSpeedDiff 5,
avSlowSpeed 40, avPercentagePedalPushDistance 1.2, minimumMovingSpeed 1, brFollowSpeedBrake 1;
vehicle (touring = default): brakeBufferZone 1.0, brakeExtraPrediction 0.

## 11. Port plan for `crates/grid-tools/src/ai_driver.rs`

### Data

- Extend `Other` with `id`, `forward`, `right`, `speed`, `half_width`, `half_length` (front/rear
  points = centre ± forward·half_length) and `state` (0 racing; 2 when the velocity opposes
  the forward axis). Lacetti half sizes from car data (≈0.86/2.25 m [I]).
- `Parameters`: the ov*, as*, co*, av-pass/separation constants above, vehicle brakeBufferZone/
  brakeExtraPrediction, mapped indices 2, 3, 7, 17–28, 41, 42, 44 (all already loadable through
  the existing mapping evaluator), and the fire priorities from `ai_behaviour_fire.xml`.
- `AiDriver` state: active behaviour; overtake {target id, state, decision timer, min-fire
  timer}; attempt-overtake previous target; alongside previous fired flag.

### Per tick

1. Track view as now; compute room to the racing limits from the planned offset
   (`roomL = offset − left_limit`, `roomR = right_limit − offset` at the car and at the
   look-ahead).
2. Tests: brake_for_vehicle (start with "nearest car whose box overlaps the planned path
   corridor within the view cone", then pass/brake checks of §6.1), attempt_overtake (§7.1),
   alongside (§8.1), avoid (optional, later).
3. Arbitration with the scores race 1 / overtake 5 / alongside 8 / avoid 15; activate and
   deactivate hooks as §3 and §7.2.
4. Request reset (offset 0, merge tgOffsetMergeFactor 2.5, desired speed ∞, throttle cap 1,
   follow none, follow-brake cap 1), then brake_for_vehicle follow (§6.2 writer 1), then the
   active behaviour:
   - race: existing random offsets, merge, throttle cap = throttle_confidence;
   - overtake: §7.3/§7.4 (desired offset = planned offset ∓ dt·4, merge lerp 4→0.5, state 4
     follow);
   - alongside: §8.2/§8.3 (merge 4; yield sets follow + brake cap).
5. Planned offset `offset` approaches `request.desired_offset` at `request.merge` (replaces the
   race-only `desired_offset`/rate code), clamped to racing limits; everything after
   (look-ahead, PID, brake lines, controls) is unchanged.
6. Follow throttle/brake use `request.follow` and `request.follow_distance`; follow brake =
   `min(request.follow_brake_cap, brFollowSpeedBrake)` when the along-heading gap is below the
   follow distance and speed > 1 m/s (replaces the authored closing-speed brake and the authored
   follow-car choice). Desired speed feeds the existing governor target (`min`).

### Validation

Headless `grid-opponent-race` with a slowed leader (e.g. leader throttle cap 0.8): expect the
follower to fire overtake within 3 s of TTC < 3 s, move ≥ 1 m sideways, run alongside with
≥ 2.5 m gap, pass without contact records, and return to the line; trace behaviour, state,
offset, gaps and follow fields. Compare against a live read-only trace of the original when
the owner permits one.

## 12. Open questions

1. Observable-list selection (how 5/2/200 m are applied, sort key) — routine not located.
2. Exact corridor geometry of the path obstacle 0x78B560 and the different-heading branch of
   brake_for_vehicle; `braking_buffer_distance` consumer.
3. Record state values 1/3 and the 0x777A20 → racing transition; flag +0xB2.
4. Signs: right vector orientation of the car state (+0x50) relative to offset sign — the side
   rules are self-consistent but should be confirmed with a live sample.
5. asOffsetMultiplier consumer; avoid side selection details (0x799B30/0x799DE0).
6. Live behaviour: no runtime trace of an original overtake yet.

## 13. Evidence (private)

- Decompilation: `local/ai-overtaking/decomp/b1..b5` (Ghidra 12.1.4 headless read-only,
  runner `local/ai-overtaking/scripts/decomp.ps1` reusing
  `local/ai-research/ghidra/GridAiDecomp.java`).
- Disassembly: `local/ai-overtaking/dis_77db90.txt`, `dis_77bd20.txt`, `dis_77c180.txt`,
  `dis_78a2c0.txt`, `dis_78f7c0.txt`, `dis_789780.txt`, `dis_77b7d0.txt`.
- Helpers: `local/ai-overtaking/scripts/pe.py` (hash-guarded string/reference/disassembly
  queries, Capstone 5.0.6 from `local/tools/python`), `scripts/sweep.py` (linear sweep cache
  `sweep_760000.pkl`).
- Data: `local/binxml/captured-dependencies/decoded-009.xml` (driver config),
  `decoded-010.xml` (mappings), `decoded-011.xml` (vehicle config), `decoded-013.xml`
  (behaviour fire order).
