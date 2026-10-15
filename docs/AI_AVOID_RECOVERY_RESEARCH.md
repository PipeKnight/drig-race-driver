# Original AI avoidance and recovery — research (F-AI-03)

15 October 2026, Claude Code. Static analysis only of the hash-identified private analysis
PE `local/compatibility/grid-unpacked.exe` (SHA-256 `aac46a64…4f34514`, base 0x400000),
continuing [AI_DRIVER_RESEARCH.md](AI_DRIVER_RESEARCH.md),
[AI_OVERTAKING_RESEARCH.md](AI_OVERTAKING_RESEARCH.md) and [AI_DRIVER_PORT.md](AI_DRIVER_PORT.md).
The running original game process was not touched. No game file, Steam install or Ghidra
project was modified: Ghidra 12.1.4 ran headless with `-readOnly`, and temporary functions
existed in memory only. Decompiler output, disassembly and helper scripts are private under
ignored `local/ai-avoid-recovery/`. This document contains only addresses, field offsets,
constants and structural descriptions.

Labels: **[V]** verified in disassembly/decompilation; **[D]** shipped data;
**[I]** inference, not verified.

Notation is the same as in AI_OVERTAKING_RESEARCH:
- `cfg+X` is the shared driver config value at offset X
  (`local/ai-research/driver-config-layout.tsv`). Getter 0x7A3840(i) returns
  `cfg+0x18+0x14·i`.
- `vcfg+X` is a vehicle config value.
- `map[k]` is the per-driver mapped value with the 1-based index of the table at 0xE4CE20.
- `req+X` is the request struct (behaviour context +4, reset every tick by 0x783490).
- `view+X` is the track view (context +0x18; test context +0x10).
- `own+X` is the own AI car state (context +0x24; test context +0x1C).
- `rec+X` is an observed record of another car.

## 0. Summary

- **avoid** (score 15) is for cars that are *not racing normally*: slower than 4 m/s, in
  record state 2, facing away from our heading, or (rarely) pointing within 45° of the wrong
  way. It does not trigger on ordinary traffic.
  - It fires when the time to reach the predicted path obstacle (distance / closing speed)
    is below 2 s.
  - Every 0.25 s it predicts the target's position at that time, picks the side that needs
    the least lateral movement, and sets the desired offset to the target's lateral position
    ± (buffer 2 m + both half widths + half the target's length).
  - If there is no room against the track limit on that side, it flips sides, or, with a car
    alongside, falls back to following the target at 3 m.
  - It switches the offset clamp to the track limits and merges at 5 m/s.
  - It only emits a desired speed (brake-to-speed) on decision ticks.
- **Recovery** has three parts, all [V]:
  - The `recovery` behaviour scores 11/16/17/19 depending on how many of four conditions
    hold: spun, off track, obstruction contact while off track, frontal obstruction contact.
  - On activation it classifies the car's orientation into octants. It drives forward, or
    brakes to a stop and then reverses at ≤ 5 m/s until aligned or perpendicular, then
    drives forward.
  - Obstruction contacts toggle reverse/forward for 2 s each.
  - `recover_tbone` (18) reverses straight for 1 s after the AI's front touches another car
    at a T-bone, head-on or slow nose-to-tail angle.
  - Stuck cars are retired by the retire test: 20 s without moving 5 m and no racing car
    within 100 m.
  - Reset after `rcRecoveryTimeOut` (10 s) is disabled by default (`rcResetAllowed` false).
- **lost_control** (21) is the execution of choreographed failures/mistakes (per-wheel grip
  cuts, crash PID, flip), not a reaction to real loss of control. One exception: when the
  car is spun and spinning faster than coSpinSpeed, it requests reverse [V].
- **Reverse path** [V]: req+0xAF negates the throttle (0x783180) and the steering
  (0x77AC50), and suppresses the |slip| > π/2 full brake. 0x7929D0 turns a negative
  throttle into |throttle| plus packet byte +0x1E = 1. 0x779F70/0x7FC030 carry the byte to
  the published control packet. The car-side consumer of that byte was not traced [I].

## 1. Corrections to earlier documents

- **Track view samples are swapped in AI_OVERTAKING_RESEARCH §1** [V].
  - 0x798E90 first walks from the car's link `view+0x18`/fraction `view+0x28` by the
    look-ahead distance `view+0x68` (0x788410).
  - It then builds the **+0x90 block** (line point +0xB0, lateral +0xC0, planned point
    **+0xD0**, planned offset **+0xE0**; 0x7878F0) **at the look-ahead link and fraction**.
  - It builds the **+0x150 block** (line point **+0x170**, lateral **+0x180**, planned point
    +0x190, planned offset +0x1A0; 0x779C40/0x788030) **at the car's own link and
    fraction**.
  - Both planned offsets move toward req+0x98 at req+0x9C.
  - Consequences:
    - view+0x64 (the heading error used by the spun test) is the signed angle from the car
      forward to `view+0xD0 − front`, the planned look-ahead target.
    - **0x40AE90(view)** = `dot(front(view+0x40) − view+0x170, view+0x180)` is the signed
      lateral position of the car's front relative to the racing line at the car (positive
      toward the right limit).
  - Geometric claims in the overtaking document that use both points (a line through +0xD0
    and +0x190) are unaffected. Only the labels "at the car" and "look-ahead" swap.
- **lost_control is not a recovery behaviour** (§4.4).
- **0x788A90** in the brake sum (AI_DRIVER_RESEARCH §4.4, "avoidance, not analysed") is the
  retire stopping brake [V].
  - It applies only when req+0x3A is set by the retire drive 0x79A090.
  - It returns brake 1 when `2 · D_brake(grip, v → 0) > req+0x3C` (distance to the retire
    point).
- The **request reset 0x783490 runs every tick** (common setup 0x7978D0) [V]. It clears:
  - the target point +0x60..+0x6C;
  - the flags +0xA4..+0xA7, +0xAC..+0xB6 (including reverse +0xAF and reset +0xB0);
  - the desired speeds +0x80/+0x84 (set to FLT_MAX);
  - the follow fields (+0x78 = 1).

  Every behaviour output therefore lasts one tick unless it is rewritten.

## 2. Avoid

### 2.1 Inputs from the path-obstacle scan 0x78B560 [V unless marked]

The scan runs before the tests every tick (0x78F690 clears the result, then calls 0x78B560).
It writes the path-obstacle object at test context +4:

| Field | Meaning |
|---|---|
| +0x00 | **relative velocity** `own.vel(+0xA0) − rec.vel(+0x90)`, y cleared |
| +0x10 | second vector (path point/direction, not used by avoid) |
| +0x20 | **record** of the nearest obstacle |
| +0x24 | **distance** to it along the path |
| +0x28 | time = distance / \|relative velocity\| (≥ 0); the obstacle with the smallest time is kept |
| +0x30.. | list of up to 6 obstacles |
| +0x160 / +0x164 / +0x168 | **contact list** (inline storage +0x170, capacity 6, count): entries (record, contact type), see §3.5 |

Own box margin for the scan: `buffer = clamp(speed / avBufferZoneBaseSpeed (10), 0, 1) ·
avBufferZone (2 m)`, added ahead of the front and behind the rear.

- The scan uses the rear direction when req+0xAF (reverse) was set [V].
- A car is considered only when `dot(rec − own, own heading) > 0` and the relative velocity
  points toward it [V structure; corridor details as in the overtaking document, I].

### 2.2 avoid test 0x79A960 (type 14, object 0x30 bytes) [V]

Outputs:

| Field | Meaning |
|---|---|
| +8 | fired |
| +0xC | target id |
| +0x10 | remembered target record |
| +0x14 | ETA |
| +0x18 | closing speed |
| +0x20 | relative velocity (4 floats) |

At the start of every update: +8 = 0, +0xC = 0, +0x14 = FLT_MAX, +0x18 = 0. On stop-firing
(vtable +8 = 0x776F30) +0x14 = FLT_MAX and +0xC/+0x18 = 0.

**Branch A — a path obstacle exists (record r = obstacle +0x20):**

1. **Racing-normally exclusion 0x79A530**
   `(own, r, view, avMinRacingSpeed cfg+0x964, avMaxRacingAngleFromTrack cfg+0x978)`
   returns 1 (excluded: return without firing, **record +0x10 kept**) only when **all** of
   these hold:
   - `r.speed (rec+0x10) ≥ avMinRacingSpeed`, and `r.state (rec+0xFC) ≠ 2`;
   - `dot(xz(own.fwd), xz(r.fwd)) ≥ 0` (same general heading);
   - 0x798C00(view, r.centre) finds r's link and its from-gate `g`, and
     `acos(dot(r.fwd, t) / (|r.fwd|·|t|)) > avMaxRacingAngleFromTrack`, where
     `t = (−g.normal.z, 0, g.normal.x)` (0x7A2EA0 rotation).
     - Gate +0x10 = position and gate +0x20 = normal are parsed by 0x7A0ED0 [V].
     - On Donington this `t` points **against** the driving direction at all 134 gates
       [D, private check of decoded ai_track.xml].
     - So the angle test excludes every car that is not pointing within 45° of the wrong
       way. For a car already heading our way this is always true.

   Net rule [V code + D data]: **racing normally ⇔ speed ≥ 4 m/s (XML), state ≠ 2, heading
   within 90° of ours, and not pointing within 45° of the wrong way.** The angle test reads
   like an inverted sign in the original [I]. Port the behaviour, not the intent.
   F306 numerical detail [V]:
   - the cosine is xz-only f32;
   - its lengths come from rsqrtps plus one Newton step, which biases |cos| upward;
   - |cos| > 1 makes acos return NaN, and NaN makes the function return 0 (not excluded).

   As a result, targets within about 5·10⁻⁴ rad of `t` are not excluded (see
   [AI_AVOID_RECOVERY_LIVE.md](AI_AVOID_RECOVERY_LIVE.md#the-036-leak-float-cosine-below-1-gives-acos-nan-f303)).
2. `v = |obstacle +0x00|` (closing speed), `d = obstacle +0x24`,
   `buf = map[32] braking_buffer_distance`.
   - If `v ≤ avMinSpeedNormal` (cfg+0x89C, XML 0.0): require `buf > d` and
     `v < avMinSpeedSlow` (cfg+0x8B0, 9), otherwise return.
   - With the shipped 0.0 this only applies to v = 0, where the ETA is infinite, so the
     branch is inert [V+D].
3. If `d / v < avETAReactionTime` (cfg+0x950, XML 2.0): **fire**. Store id = rec+0xC0,
   ETA = d/v, +0x10 = r, closing = v, relative velocity.
4. Otherwise +0x10 = 0.

**Branch B — no path obstacle, but a remembered record r (+0x10):**

1. `D = xz(r.centre − own.centre)`, `along = dot(D, own.heading(+0xB0))`.
2. Require `along > 0.5·|r.front − r.rear| + 0.5·|own.front − own.rear|` (fully ahead).
3. Require `cos(clamp(map[44]·π/180, 0, π)) < dot(own.fwd, normalize(D))` (view cone,
   driver_view_angle 50°).
4. `closing = |xz(own.vel − r.vel)|`, +0x18 = closing.
5. If `along < avETAReactionTime · closing`: fire with ETA = |along|/closing, the relative
   velocity and the id. The record stays remembered.
6. Otherwise +0x10 = 0.

Branch B has no racing-normally check. It keeps tracking a target that has left the
path-obstacle corridor.

### 2.3 avoid behaviour (vtable 0xCF902C) [V]

- **Score 0x788E30**: `priority[1]` (15) if the avoid test (test array +0x38) fired, else 0.
- **Object fields**:

  | Field | Meaning |
  |---|---|
  | +0x30 | target id |
  | +0x34 | target record |
  | +0x38 | side mode (0 follow, 1 negative/left, 2 positive/right) |
  | +0x3C | figure-of-eight flag |
  | +0x40 | decision timer |
  | +0x44 | target speed |
  | +0x48 | desired offset |
  | +0x4C | clearance |
- **Activate 0x77B610** clears +0x30..+0x50 and copies +0x3C = avFigureOfEightTrack
  (cfg+0xA40). Deactivate 0x776320 is empty.
- **Drive 0x79C600** (returns 0, so the default drive 0x797B50 runs afterwards):
  1. `+0x40 −= dt`.
  2. If the test object exists and its record (+0x10) is set: +0x34 = record. If
     `+0x40 < 0` or the id differs from +0x30 (**decision**):
     - +0x30 = id;
     - side choice 0x799B30;
     - offset 0x799DE0;
     - speed 0x77B5A0;
     - if +0x44 > 0: **req+0x84 = +0x44, req+0xA4 = 1** (brake to desired speed);
     - +0x40 = avDecisionTime (cfg+0x914, 0.25 s).
  3. While the test object exists: limit types (ctx+8 holder +0x48/+0x4C) = **3/4 (track
     limits)** instead of the racing limits 1/2 that 0x7978D0 sets every tick.
  4. Every tick: **req+0x98 = +0x48, req+0x9C = avOffsetMergeFactor** (cfg+0x888, XML 5).

Because the request is reset every tick, **the desired speed, brake-to-speed flag and mode-0
follow fields are only present on decision ticks** (one tick per 0.25 s) [V]. Their effect
through the slewed pedals is small [I]. The offset and merge persist.

**Side choice 0x799B30** (this = behaviour, args ctx, record r):
1. +0x38 = 0. **Clearance** +0x4C = `0.5·|r.rear − r.front| + avBufferZone (cfg+0x928, 2) +
   own.halfW (own+0x110) + r.halfW (rec+0xF0)`.
2. If `closing (test +0x18) > own.speed (own+0xD0)` → **mode 1** and return. No prediction
   and no room check (the target has a velocity component toward us [I]).
3. Predicted position `P = r.centre + ETA (test +0x14) · r.vel (rec+0x90)`.
4. Locate P: 0x798C00 (all links, nearest by 0x798920) or, when +0x3C is set, 0x78DCD0
   (first link containing P).
   - `lat` = 0x798D00(gateA, gateB, racing-line waypoint type 0, P) =
     `dot(P − racing-line waypoint at gate A, segment lateral direction)` (0x796CD0).
   - lat is 0 if a gate is missing.
5. `x` = 0x40AE90(view): own lateral position (§1).
6. **mode 2** (pass on the positive side) if `|lat + c − x| ≤ |lat − c − x|`, else **mode 1**:
   the side needing the smaller lateral move wins, and ties go positive.
7. **Room check 0x797720** (only after step 6):
   - `w = 2·own.halfW + r.halfW + 0.5`.
   - Check `dist²(P, track-limit segment of the chosen side) < w²`, where the segment is
     type 3 (left) for mode 1 and type 4 (right) for mode 2 (0x796B30). If so:
     - if avAvoidAlongside (cfg+0xA54, XML true) and the alongside test (test array +0) did
       not fire → **flip side**;
     - otherwise: mode 1 → 2 if `w < alongside gap on the positive side (+0x10)`; mode 2 →
       1 if `w < negative-side gap (+0xC)`; else **mode 0**.

**Offset 0x799DE0**:
- Mode 0: req+0x70 = record and req+0x74 = 3.0 (follow at 3 m). +0x48 is kept (0 after
  activation).
- Modes 1/2: recompute P and lat as above.
  - `desired = lat − c` (mode 1) or `lat + c` (mode 2).
  - Then mode 1 uses `min(desired, x)` and mode 2 uses `max(desired, x)`: **never move
    toward the obstacle from the current lateral position**.
  - +0x48 = desired; req+0x98 = desired; req+0x9C = avOffsetMergeFactor.

**Target speed 0x77B5A0** (+0x44 = 0 first):
- `r.speed < avStoppedSpeed` (cfg+0x8D8, built-in 1.0) → **avStoppedTargetSpeed**
  (cfg+0x8EC, XML 30).
- else `closing ≤ own.speed` → **avNormalTargetSpeed** (cfg+0x8C4, XML 200, i.e. no limit).
- else if the figure-of-eight flag is clear → **avWrongWayTargetSpeed** (cfg+0x900, XML 5).
- else 0 (no speed written).

**Planned-offset clamp**: the limit types 3/4 reach the request's left/right limit-line ids
+0x44/+0x48 through 0x794960 in the default drive. The next track-view update clamps the
planned offsets to the **track** limits [V].

**End**: avoid stops scoring when the test stops firing. The race behaviour then merges back
(raOffsetMergeFactor).

### 2.4 Shipped values used by avoid [D] (default section; built-in where the XML is silent)

| Parameter | Value |
|---|---|
| avETAReactionTime | 2.0 s |
| avDecisionTime | 0.25 s (built-in) |
| avBufferZone | 2.0 m |
| avBufferZoneBaseSpeed | 10 m/s |
| avOffsetMergeFactor | 5 m/s |
| avMinRacingSpeed | 4 m/s |
| avMaxRacingAngleFromTrack | 0.785 rad (built-in) |
| avMinSpeedNormal / avMinSpeedSlow | 0 / 9 m/s |
| avStoppedSpeed | 1.0 m/s (built-in) |
| avStoppedTargetSpeed | 30 m/s |
| avNormalTargetSpeed | 200 m/s |
| avWrongWayTargetSpeed | 5 m/s |
| avAvoidAlongside | true |
| avFigureOfEightTrack / avOncomingTraffic | false |
| braking_buffer_distance | lerp(lerp(5, 2, c), lerp(3, 1, c), d) = 1.39 m at d 0.606, c 1 |
| driver_view_angle | 50° |

- `single_demolition_race` sets avWrongWayTargetSpeed 25, avOncomingTraffic and
  avFigureOfEightTrack true, and avAvoidAlongside false.
- **avOncomingTraffic has no reader** in 0x760000–0x7C0000 apart from its constructor
  store [V].

## 3. Tests used by recovery

### 3.1 spun 0x77E5B0 (type 1) [V]

Inputs: `h = |view+0x64|` (heading error to the planned look-ahead target).

- **Fire** if `h > π/2`.
- Otherwise, if it fired last tick, it stays fired while any of these hold:
  - `|s'| > 0.1π` (.data 0xE4D0F0 = 0.1), where `s = |own+0xD4|` (slip angle [I, strongly
    supported]: used with π/2 in the brake and with crash angles) and
    `s' = π − s` when `dot(normalize(front − rear), own.vel) < 0` (moving backwards);
  - `h > π/3` (0xCF6F08);
  - `own.speed < 3 m/s` (.data 0xE4D0EC = 3).

  Otherwise it clears.
- **+0xC "spinning fast"** = fired and `|dot(own angular velocity +0xC0, own up +0x40)| >
  coSpinSpeed` (cfg+0xB94, XML 1.6 rad/s).
- Stop-firing vfunc 0x777430 is empty.

### 3.2 stop 0x77E7D0 (type 2) [V]

- Fired when the track view is invalid (`view+0x88 == 0`, look-ahead off the route).
  Certain game modes (string compares against globals 0xE4CD78/0xE4CCD8/…) force it on or
  off [I which modes].
- +0xC = `own.speed < minimumMovingSpeed` (cfg+0xC48, 1.0).
- Effect in common setup 0x7978D0: **req+0xA5 force brake = 1** for every behaviour, and
  **req+0xA6 no-steer** when +0xC is also set.
- controlled_stop (score 20 when fired and slow) holds the car: force brake, req+0x98 =
  view+0xE0, no-steer.
- The recovery score is 0 while stop fires.

### 3.3 collided_obstruction 0x776FE0 (type 10) [V]

- Fired when `own+0x120 > 0`.
- +0xC "frontal" when `acos(dot(normalize(front − rear), own+0x130)) > 2π/3` (120°, the
  contact normal opposes our nose).
- Source [V structure, I semantics]:
  - Contact callback 0x4022F0 accumulates `normal × magnitude` into own+0x160 (count
    +0x170) through 0x775FD0. Contacts are skipped when they are floor-like (normal y ≥ 0.1
    for one body class) or when the other body moves at ≥ 2 m/s. Static or slow
    obstructions remain: walls, barriers, stopped cars.
  - Once per frame, 0x77B1F0 (called from 0x401100) sets own+0x120 = |sum| and own+0x130 =
    normalized sum (left raw below 1e-5), then clears the accumulator.

### 3.4 on_track 0x79B250 (type 11) [V structure]

- For each wheel contact point (car vfunc +0x1C count, vfunc +4 points, stride 0x10), it
  tests the point against the left (type 3) and right (type 4) track-limit waypoints of the
  current link's gates (view+0x20/+0x24; 0x7983E0, side test 0x7771E0).
- +0xC = number of wheels inside. **Fired when more than half the wheels are inside the
  track limits.** Off track = not fired.

### 3.5 contacted_vehicle 0x784650 (type 17) and contact types [V]

- Fired when the path-obstacle object's contact list (+0x160, count +0x168) is non-empty.
- +0x10 = first entry's record, +0x14 = its **contact type**, +0x18 = count, +0x1C = own
  state.

The list is filled geometrically by the scan (0x783B20 → 0x7A45D0 [I that 0x7A45D0 pushes]):
- The own box comes from 0x77B400 and the other box from 0x77F9F0. Both are inflated by a
  margin and flattened to xz.
- Corner order: 0 front-right, 1 front-left, 2 rear-right, 3 rear-left (front/rear points
  ± right·(halfW + margin), front/rear ± forward·margin).
- Types:
  - **1–4**: own corner k is inside the other box;
  - otherwise one of the other car's corners is inside our box, classified by our nearest
    face: **5 front, 6 rear, 7 right, 8 left**;
  - two-corner cases are refined with 0.33/0.35/0.45 length-ratio thresholds.
- "Right" is the own +0x50 axis [I naming].

### 3.6 Others [V]

- **is_terminally_damaged 0x777600** (type 18): own+0xE3 == 1.
- **lost_control 0x77ED50** (type 19):
  - +0xC = own speed, +0x10 = rtRetirementSpeed (cfg+0x478, XML 15) + 5;
  - fired if any of the 4 bytes from car vfunc +0x18 is 1 (per-wheel failure flags [I],
    the same bytes the retire test counts).
- **retire 0x784560** (type 8):
  - +0x10 = number of failure flags;
  - +0xC = terminally damaged;
  - +0xD = damage (own+0x11C) > map[30] damage_retirement_ratio;
  - +0xE = damage ≥ 0.5 (both only when a flag at context +0x34 → +0x1D is set);
  - **stuck timer 0x77E3B0**: reference position +0x20. Reset (timer 0, +0x14 = 0) when the
    car is ≥ sqrt(rtStuckDistanceSq = 25) = 5 m from it, or in one game mode. Otherwise the
    timer +0x18 += dt. Past rtStuckTime (20 s), +0x14 = 1 and fired — **unless** a racing
    record (state 0) is within sqrt(rtPlayerDistanceSq = 10000) = 100 m.
  - fired = 0xC or 0xD or failures > 0 or stuck.

## 4. Behaviours

Priorities [D]:

| Behaviour | Score |
|---|---|
| race | 1 |
| race_damaged | 9 |
| retire | 10 / 23 |
| recovery | 11 / 16 / 17 / 19 |
| race_start | 14 |
| avoid | 15 |
| recover_tbone | 18 |
| controlled_stop | 20 |
| lost_control | 21 |

Recovery's values come from ai_behaviour_fire.xml (first, then second, third, fourth).

### 4.1 recovery (vtable 0xCF8F28) [V]

**Score 0x78AB20**: 0 if is_terminally_damaged or stop fired. Otherwise `n` counts:
- spun fired (+1);
- on_track exists and is **not** fired (+1);
- collided_obstruction fired: +1 if also off track, and +1 if frontal (and behaviour +0x34 =
  1).

Score = `priority[n]`: n = 1 → 11, 2 → 16, 3 → 17, 4 → 19.

Fields:

| Field | Meaning |
|---|---|
| +0x30 | mode |
| +0x35 | latched-after-stop |
| +0x36 | obstruction reverse toggle |
| +0x38 | elapsed time |
| +0x3C | obstruction timer |

- **Activate 0x776990**: +0x30 = 0, +0x35 = 0, +0x38 = 0, +0x3C = −1. +0x36 is *not*
  reset.
- **Deactivate 0x7769B0**: +0x38 = 0.

**Drive 0x78EE70** (returns 0; the default drive runs afterwards):
1. **Offset 0x78ABB0**: if on_track is **not** fired, req+0x98 = `view+0xE0 ∓ dt ·
   rcMergeFactor` (cfg+0x57C, 1.0): minus when own lateral 0x40AE90 ≤ 0, plus otherwise. The
   planned offset drifts toward the side the car is on at 1 m/s. req+0x9C =
   **avOffsetMergeFactor** (5). If on track, 0x7765B0 sets req+0x98 = own lateral (the
   planned line jumps to the car's position) and keeps the merge at tgOffsetMergeFactor.
2. **Octant 0x77C9B0** returns `n ∈ 0..8` and two angles in degrees:
   - `b` = view+0x64·57.2958 (heading error to the planned target);
   - `a` = signed angle between own forward and `xz(view+0xD0 − view+0x170)` (track direction
     from the racing line at the car toward the planned target);
   - both are negated when the own lateral is ≤ 0 (side mirror).

   | n | b range | a range |
   |---|---|---|
   | 1 | (0, 90) | (0, 90) |
   | 2 | (−90, 0) | (0, 90) |
   | 3 | (−90, 0) | (−90, 0) |
   | 4 | (−180, −90) | (−90, 0) |
   | 5 | (−180, −90) | (−180, −90) |
   | 6 | (90, 180) | (−180, −90) |
   | 7 | (90, 180) | (90, 180) |
   | 8 | (0, 90) | (90, 180) |

   n = 0 on boundaries or other combinations.
3. Mode selection:
   - While +0x35 = 0, **mode = 2 if n ≤ 3, else 1**.
   - **Mode 1**: req+0xB2 = 1 (full brake in 0x78E8A0) until `own.speed < 0.1`, then latch
     +0x35 = 1 and choose: n < 3 → 2, n 3..5 → **3**, n 6..8 → **5**.
4. Mode outputs:
   - **Mode 3**: if |a| < 5° or |b| < 5° → reverse off, mode 2, req+0x84 = FLT_MAX,
     req+0x7C = 1. Otherwise **req+0xAF = 1 (reverse), req+0x84 = 5 m/s, req+0x7C = 1**.
   - **Mode 5**: if 80° < |a| < 90° or |b| < 5° → reverse off, mode 2, req+0x84 = 0 (that
     tick), req+0x7C = 1. Otherwise reverse with req+0x84 = 5 m/s and req+0x7C = 1.
   - **Mode 2**: req+0xAF = 0 (drive forward; throttle cap stays 1.0, since recovery never
     sets throttle_confidence).
5. **Obstruction toggle**: if collided_obstruction fired and +0x3C < 0:
   - +0x3C = rcCollisionTimeOut (cfg+0x590, built-in 2 s);
   - **toggle +0x36**;
   - if own+0x120 ≠ 0: angle `θ = acos(clamp(dot(xz(normal), xz(right)), −1, 1))` in degrees.
     If θ < 45° (s = −5) or θ > 135° (s = +5), set the target point req+0x60 (0x7A31E0) =
     `front + 5·fwd + s·right` (forward) or `front − 5·fwd − s·right` (+0x36 set).
     **The default drive's 0x7892E0 overwrites req+0x60 later in the same tick** (it runs
     whenever the per-behaviour holder +0x50 is set, which 0x7978D0 does every tick), so
     this target has no effect [V].
   - req+0xAF = +0x36.

   While +0x3C > 0: req+0xAF = +0x36 and +0x3C −= dt. Net effect: each obstruction contact
   flips between reversing and driving forward for 2 s.
6. **Timeout**: +0x38 += dt. If rcResetAllowed (cfg+0x554; built-in false, not in the
   shipped XML) and +0x38 > rcRecoveryTimeOut (cfg+0x568, 10 s): req+0xB0 = 1 (reset, copied
   to packet +0x20) and +0x38 = 0. **Disabled by default** [D].

**End**: the behaviour stops when its score drops to 0 (back on track, not spun, no
obstruction contact) or something higher wins.

- Steering in recovery is the normal heading-error steering toward the planned target,
  **negated while reversing** (0x77AC50).
- The spun test, the octant and the brake-line logic all keep running.
- Note [I]: off-track-only recovery (n = 1) with a forward octant is ordinary driving toward
  the drifting planned line at throttle cap 1.0.

### 4.2 recover_tbone (vtable 0xCF8F4C) [V]

- **Score 0x78ACC0**: if contacted_vehicle fired with type ∈ {**1, 2, 5**} (front-right
  corner, front-left corner, front face).
  - Let `θ = acos(dot(own.fwd, r.fwd))` in degrees, `rel = |own.vel − r.vel|` and
    `vo = |r.vel|`.
  - The score is 18 and **+0x34 = 1.0 s** when any of these hold:
    - 70 < θ < 110 and rel < 5 (T-bone);
    - θ < 30 and rel < 5 and vo < 5 (slow nose-to-tail);
    - θ > 160 (head-on).
  - Without such a contact the score stays 18 while +0x34 > 0.
- **Activate 0x776A40**: +0x34 = −1, +0x31 = 0. Because activation runs after scoring, the
  first tick does nothing.
- **Drive 0x776A00**: while +0x34 > 0 → **req+0xAF = 1 (reverse), req+0xA6 = 1
  (no-steer)**, +0x34 −= dt. The car backs straight out until 1 s after the last qualifying
  contact.

### 4.3 controlled_stop, retire, race_damaged (brief) [V]

- **controlled_stop** (20): §3.2.
- **retire** (score 0x78B300; activate 0x77D570 sets +0x34 = rtRetirementResetTimer):
  - score is 10 for failures or damage, 23 for terminal damage or stuck (or behaviour
    +0x60);
  - drive 0x79C6E0: for terminal or stuck → force brake + no-steer; below rtStoppedSpeed
    (cfg+0x4F0, 0.5) → req+0x38 = 1 (removal request [I]);
  - for failures/damage → 0x79A090 drives to a retire point at req+0x84 = rtRetirementSpeed
    with the stopping brake 0x788A90.
- **race_damaged** (9, retire test +0xE): req+0x84 = 50, req+0x7C = 0.6, merge
  raOffsetMergeFactor (cfg+0x5CC), plus a lateral target from 0x797F00 [V structure].

### 4.4 lost_control (vtable 0xCF8F70) — choreographed failures [V]

- **Score 0x78B180**: 21 if the lost_control test fired with speed > rtRetirementSpeed + 5,
  or spun is fired and spinning fast.
- **Activate 0x78B1F0** copies the vcfg PID gains and picks one mode:
  - speed > crashMinSpeed (vcfg+0x630, 50) → crash mode +0x74;
  - else spun and fast → spin mode +0x75;
  - else per-wheel grip cut: one of the four grip multipliers = 0.1 according to the failure
    byte (0x776AB0).
- **Drive 0x776B80**:
  - crash mode → 0x77D200: desired speed 0, req+0xB2, crash PID gains,
    crashFront/RearGrip, spin/flip angles, req+0xD0 flip;
  - spin mode → grip multipliers 1.0 and **req+0xAF = 1** (reverse request while spinning);
  - otherwise 0x77CF10: desired speed rtRetirementSpeed with brake-to-speed, brake cap 0.15,
    speed-scheduled PID, random lateral wobble every 1 s, rear grip 0.3.
  - Then it writes the request grip multipliers +0xB8..+0xC4.

Not a recovery from real loss of control (apart from the spin-mode reverse request). Out
of scope for the port's recovery work.

## 5. From req+0xAF to the car [V unless marked]

1. Throttle 0x783180 computes the usual min of limiters, then max with the start minimum. If
   **req+0xAF**, the result is multiplied by −1.
2. Brake 0x78E8A0: req+0xB2 → 1. The |slip (own+0xD4)| > π/2 → 1 term is **skipped while
   req+0xAF**. req+0xA5 → 1, req+0xA4 brake-to-desired, follow and line brakes, capped by
   req+0xA0.
3. 0x7929D0: brake into packet[1]. If the throttle is < 0: packet[0] = −throttle and
   **packet byte +0x1E = 1**; else +0x1E = 0.
4. Steering 0x77AC50: 0 when req+0xA6, otherwise the normal law divided by the max steering
   angle, then **× −1 when req+0xAF**.
5. 0x794310 assembles the packet:
   - handbrake byte +0x1C = 0x77A110 (req+0xB3, or force brake below minimumMovingSpeed);
   - +0x20/+0x21 = req+0xB0/+0xB1 (reset request);
   - +0x1F = 0, +0x24 = 3.
6. 0x779F70 slews throttle, brake and steering and copies bytes +0x1C, +0x1E, +0x20, +0x21.
   0x7FC030 clamps (bytes to 0/1, [3] −1..6, [9] 1..4).
7. The only caller of 0x7FC030 is the AI (0x79443D). **The car-side consumer of byte +0x1E
   was not traced** [I]: it is presumably a reverse-gear request, consistent with the
   player's automatic gearbox, which selects reverse near rest when braking and then
   publishes the brake key as throttle (REFERENCE_DRIVING_INPUT.md).

## 6. Per-tick order relevant to the port [V]

1. Path scan (obstacles and contacts).
2. Tests 0..20.
3. Scores (§4 table).
4. Activate/deactivate.
5. 0x7978D0: request reset, stop → force brake, cruise speed, desiredSpeed cap,
   brake_for_vehicle follow, merge tgOffsetMergeFactor, limit types 1/2.
6. Behaviour drive.
7. Default drive 0x797B50: PID target into req+0x60, heading error req+0x88, brake-line
   speed, limit-line ids from the limit types.
8. Controls (§5).
9. Next tick's track view applies req+0x98/+0x9C and the limit ids to the planned offsets.

## 7. Port plan for `crates/grid-tools/src/ai_driver.rs`

### 7.1 Data and parameters

- `Parameters`, from the driver config (`d`, with the built-in fallbacks above):
  - avETAReactionTime, avDecisionTime (0.25), avBufferZone, avBufferZoneBaseSpeed,
    avOffsetMergeFactor;
  - avMinRacingSpeed, avMaxRacingAngleFromTrack (0.785), avMinSpeedNormal, avMinSpeedSlow;
  - avStoppedSpeed (1.0), avStoppedTargetSpeed, avNormalTargetSpeed, avWrongWayTargetSpeed;
  - avAvoidAlongside, avFigureOfEightTrack;
  - coSpinSpeed, rcMergeFactor, rcCollisionTimeOut (2), rcRecoveryTimeOut, rcResetAllowed
    (false);
  - rtStuckTime, rtStuckDistanceSq, rtPlayerDistanceSq;
  - minimumMovingSpeed, brTargetSpeedBig;
  - mappings 30 (damage_retirement_ratio), 32 (braking_buffer_distance) and 44 (already
    loaded);
  - the fire priorities for avoid (15), recovery (11/16/17/19) and recover_tbone (18).
- `Other` additions:
  - `speed` (already derivable);
  - `half_width`/`half_length` (current constants);
  - `state`: 0 racing; **2 when the velocity opposes the car's forward** [I mapping, as in
    the overtaking plan].
- Track data:
  - left/right **track** limits per segment (ai_track.xml types 3/4, already parsed for the
    racing limits?), for the avoid room check, the on_track test and the avoid clamp;
  - the gate normal for the racing-normally angle. Equivalently, the line direction at the
    other car's arc: since the original effectively compares with the *reverse* direction,
    use `t = −line_direction`.

### 7.2 Request additions

Extend the private `Request` with:

| Field | Original |
|---|---|
| `brake_to_speed: bool` | +0xA4 |
| `force_brake: bool` | +0xA5 |
| `no_steer: bool` | +0xA6 |
| `full_brake: bool` | +0xB2 |
| `reverse: bool` | +0xAF |
| `track_limits: bool` | limit types 3/4 |

`desired_speed`, `throttle_cap`, `follow`, `desired_offset` and `merge` exist already.

Apply them where the original does:
- brake = max(…, 1 if `full_brake`, 1 if `force_brake`, 1 if `brake_to_speed` and
  `speed − desired_speed > brTargetSpeedBig (2)`);
- the slip > π/2 brake is skipped when `reverse`;
- throttle = −throttle when `reverse`;
- steering = 0 when `no_steer`, × −1 when `reverse`;
- the planned-offset clamp uses the track limits instead of the racing limits when
  `track_limits` is set.

### 7.3 Reverse at the vehicle boundary

The original sends |throttle| plus a reverse flag. `source_vehicle::Controls` has no
reverse channel, and the port's automatic gearbox selects reverse when braking near rest and
then treats the brake pedal as reverse drive. Two options:

- **(a)** Add `reverse: bool` to `Controls` and route it to the gear selection.
  Recommended, faithful.
- **(b)** AUTHORED mapping: while `reverse`, publish `brake = |throttle|` and
  `throttle = brake command`.

Either way, delete the authored stuck recovery (`stuck`/`reversing`) once recovery and
recover_tbone are in.

### 7.4 Tests (per tick, before arbitration)

1. **Avoid test** (§2.2):
   - From the existing brake_for_vehicle path corridor, take the nearest obstacle by time
     `distance / |own.vel − r.vel|`, with the speed-scaled buffer
     `clamp(v/10, 0, 1)·2 m` ahead.
   - Apply the racing-normally exclusion (keep the remembered target when excluded).
   - Fire on ETA < 2 s.
   - Branch B on the remembered target (ahead beyond half lengths, 50° cone,
     `along < 2·closing`).
2. **spun**: `|heading_error| > π/2` (heading error to the look-ahead target, as already
   computed), with the hysteresis of §3.1 (0.1π, π/3, 3 m/s); `spinning_fast = |yaw| > 1.6`.
3. **on_track**: more than half of the four wheel contact points (or AUTHORED: the four box
   corners) inside the left/right track-limit polyline at the car's segment.
4. **collided_obstruction**:
   - From the physics: sum `normal × magnitude` over this frame's contacts of the car with
     static geometry (and slow bodies under 2 m/s), skipping floor normals.
   - fired = |sum| > 0; frontal = angle(normal, body forward) > 120°.
   - This needs a per-car contact feed from grid-world. Without it: AUTHORED
     (wall-proximity from the track limits plus speed collapse), labelled.
5. **contacted_vehicle**: geometric box overlap with margin (contactZone 0.1 [I that the
   margin is contactZone]) and the corner/face classification of §3.5 (types 1/2/5 matter).
6. **retire stuck** (optional): 20 s within 5 m and no racing car within 100 m → force
   brake + no-steer stop. Replaces the authored fallback for cars that cannot free
   themselves.

### 7.5 Arbitration

Order by score: recover_tbone 18 / recovery 11–19 (by n) / avoid 15 / alongside 8 /
overtake 5 / race 1.
- The first strictly greatest score wins; ties keep list order.
- Keep activate semantics: recovery activate resets mode, latch and timers but **not** the
  +0x36 toggle; recover_tbone activate sets its timer to −1 after scoring.
- Behaviours that set nothing fall through to the default drive (PID target, brake lines,
  governor), exactly as now.

### 7.6 Behaviour outputs composed with the existing request

| Behaviour | desired offset / merge | speed | throttle cap | follow | flags |
|---|---|---|---|---|---|
| avoid | +0x48 (lat ± c, never toward the target) / 5 m/s every tick | on decision ticks only: target speed + `brake_to_speed` | 1.0 | mode 0 on decision ticks: target at 3 m | `track_limits` |
| recovery | off track: planned ∓ dt·1 / 5 m/s; on track: own lateral / 2.5 | modes 3/5: 5 m/s | 1.0 | brake_for_vehicle only | mode 1: `full_brake`; modes 3/5 and obstruction toggle: `reverse` |
| recover_tbone | unchanged (setup default) | – | 1.0 | – | `reverse`, `no_steer` while timer > 0 |

Recommendation: port the decision-tick pulse of the avoid desired speed faithfully, but log
it. If the validation below shows avoid running into stopped cars, a held desired speed
would be an AUTHORED deviation to label.

### 7.7 Validation (headless `grid-opponent-race`)

1. A stopped car on the racing line (e.g. car 0 throttle 0 from tick N):
   - followers fire avoid with ETA < 2 s;
   - they pass on the side needing less movement with ≥ 2 m + half widths of clearance;
   - no contact records with the stopped car.
2. A car turned 180°:
   - avoid uses mode 1 when it closes faster than our speed;
   - wrong-way target speed 5 m/s.
3. A forced spin (yaw impulse):
   - spun → recovery (11);
   - octant n ≥ 4 → stop, reverse ≤ 5 m/s until aligned/perpendicular, then forward;
   - trace mode, n, a, b, `reverse` and speed.
4. Nose into a wall:
   - collided frontal → recovery 11;
   - 2 s reverse/forward toggles;
   - the car frees itself or retires after 20 s.
5. Two cars T-boned at low speed: recover_tbone reverses straight 1 s.
6. Regression:
   - solo laps bit-identical;
   - 4/8-car contact counts no worse than F297/F298.

## 8. Open questions

1. Car-side consumer of packet byte +0x1E (reverse request) and of +0x20 (reset); the role
   of packet [9] = 3.
2. Exact semantics of own+0xD4 (slip angle assumed) and own+0xC0 (angular velocity assumed).
3. Which body classes the obstruction callback 0x4022F0 accepts (0x958310 class check,
   0xA6A1F0, the 700 constant 0xCAFCD0), and whether other cars below 2 m/s count as
   obstructions.
4. Sign conventions of the octant angles `a`/`b` after the side mirror (the table is
   verified, the geometric meaning is [I]).
5. The racing-normally angle test compares with the reversed track direction. Confirm with a
   live trace whether the original ever fires avoid on a slowly moving, correctly oriented
   car (expected: only below 4 m/s).
6. Game modes forcing the stop test (string globals 0xE4CD78, 0xE4CCD8, 0xE4CDC8, 0xE4CDF0,
   0xE4CD50) and the retire stuck-reset mode 0xE4CDA0.
7. Corner order of the contact boxes relative to the left/right naming (types 1/2 are the
   two front corners either way).
8. 0x78B070 (a copy of the recovery offset helper without the on-track branch) has no
   direct caller found.

Live answers (F302, [AI_AVOID_RECOVERY_LIVE.md](AI_AVOID_RECOVERY_LIVE.md), read-only trace of
a 12-car race):
- **Q3**: callback thresholds read live: mass-like value > 700, speed < 2 m/s, floor y ≥ 0.1.
  A slow car counts statically. No live AI contact with a stopped car was observed.
- **Q4**: the sign convention is verified.
  - view+0x64 is positive toward view+0x180, and view+0x180 = −own+0x50.
  - The recovery mode choices match the octants.
- **Q5**:
  - The exclusion holds on 99.6% of traffic frames with ETA < 2 s.
  - 0.36% of those frames leak, and Branch B extends the leaks into short avoid episodes.
  - F306 identified the leak: when the target heads within about 5·10⁻⁴ rad of `t`, the f32
    cosine with rsqrtps-estimated lengths falls below −1. acos (0xBF4067) then returns NaN,
    and `ja` treats NaN as "not excluded". See [AI_AVOID_RECOVERY_LIVE.md](AI_AVOID_RECOVERY_LIVE.md#the-036-leak-float-cosine-below-1-gives-acos-nan-f303).
  - Avoid fires on every close frame for the stopped player car.
- Correction to §2.2: 0x798C00 searches only the track view's 32-link window (view+0x14) and
  keeps the nearest containing link.

## 9. Evidence (private)

- Decompilation (Ghidra 12.1.4 headless, `-readOnly`, runner
  `local/ai-avoid-recovery/scripts/decomp.ps1` reusing
  `local/ai-research/ghidra/GridAiDecomp.java`):
  - `local/ai-avoid-recovery/decomp/a1`: avoid score/activate, 0x79A530, 0x40AE90,
    0x797720, 0x798C00, 0x78DCD0, 0x798D00, 0x776F30, recovery/recover_tbone/lost_control/
    race_damaged/controlled_stop behaviours, and the spun, stop, collided_obstruction,
    on_track, contacted_vehicle, lost_control and is_terminally_damaged tests;
  - `a2`: activates, lost_control helpers, 0x77C9B0, 0x78ABB0, 0x7A31E0, 0x7A3250, track
    helpers, the car-state reset/update functions, retire, and 0x792ED0;
  - `a3`: 0x783B20, the box builders, 0x77E3B0, 0x79A090, and the locate helpers;
  - `a4`: 0x78B070, 0x77D5A0;
  - `a5`: collision accumulator callback 0x4022F0.
- Reused from earlier work: `local/ai-overtaking/decomp/b1..b3` (0x79A960, 0x79C600,
  0x799B30, 0x799DE0, 0x77B5A0, 0x78B560) and `local/ai-research/decomp/drv*` (0x7929D0,
  0x794310, 0x783180, 0x78E8A0, 0x7978D0, 0x79A400, 0x797B50, 0x7892E0, 0x77AC50,
  0x779F70, 0x7FC030, 0x788A90, 0x798E90).
- Disassembly: `local/ai-avoid-recovery/dis/` (0x79A530, 0x79A960, 0x79C600, 0x799B30,
  0x799DE0, 0x797720, 0x78EE70, 0x774E80, 0x775E80 regions).
- Helpers:
  - `local/ai-avoid-recovery/scripts/q.py`: regex over the cached linear sweep
    `local/ai-overtaking/sweep_760000.pkl`;
  - `local/ai-avoid-recovery/scripts/calls.py`: direct call-site search;
  - `local/ai-overtaking/scripts/pe.py`: hash-guarded dword/disassembly/reference queries,
    Capstone 5.0.6.
- Data: `local/binxml/captured-dependencies/decoded-009.xml` (driver config, `default` and
  `single_demolition_race`), `decoded-010.xml` (mappings), `decoded-011.xml` (vehicle
  config) and `decoded-007.xml` (Donington gates, used for the gate-normal orientation
  check).
