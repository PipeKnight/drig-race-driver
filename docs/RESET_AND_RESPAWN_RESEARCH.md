# Reset and respawn: what the original does with stuck and retired cars

Static research, 16 October 2026 (F332). Ghidra 12.1.4 headless, read-only, on the
analysis image `local/compatibility/grid-unpacked.exe` (sha256 aac46a64…4514). No input
was sent to the game. Private decompilation: `local/ai-avoid-recovery/decomp/r1`…`r13`.
Labels: [V] read from code or data, [I] interpretation, AUTHORED = port decision.

## 1. Summary

- **AI cars are never reset or respawned** [V]. The only AI reset request is recovery's
  timeout, gated by `rcResetAllowed`, which is false by default and absent from the
  shipped XML (docs/AI_AVOID_RECOVERY_RESEARCH.md 4.1). So the AI packet's reset byte
  +0x20 always stays 0.
- **An AI car that cannot free itself is retired** after 20 s within 5 m with no racing car
  within 100 m. It brakes to a stop with the wheels straight and **stays in the world**.
  No code that removes, hides or ghosts it was found [V for the drive, I for "stays"].
- **A car blocked behind a stopped car gets free with its own behaviours**: avoid plans
  around it, and an obstruction contact with a car slower than 2 m/s starts recovery's
  reverse/forward toggles. The port already has both (F301), and with them a 12-car field
  gets past a held car (section 5).
- **The reset path that does exist is for the local player** [V]. A `ResetLinesManager`
  resets the player's car when it is nearly still and on its side or roof for 2 s, and on
  the pause menu's "reset car". The car is faded out, placed on the track by the
  **grid-spawn setter 0x9EF040** with zero speed, and faded back in.
- The F323 observation, "car 7 parked behind held car 5 moved 0.0 m", came from a test
  that drives every car with fixed throttle 0.6 and no AI. It is not evidence of a
  missing respawn.

## 2. AI cars: stuck, retire, never reset

From docs/AI_AVOID_RECOVERY_RESEARCH.md (F301), re-read for this question:

| Step | Original | Port |
|---|---|---|
| Stopped car ahead | avoid (15) plans around it; ETA < 2 s | ported (F301) |
| Nose against a slow car or wall | collided_obstruction (callback 0x4022F0: mass > 700, speed < 2 m/s, floor y ≥ 0.1) → recovery 11..19, 2 s reverse/forward toggles | ported; `MultiRuntime::obstruction` counts cars below 2 m/s, so held cars count |
| T-bone or head-on contact | recover_tbone (18): 1 s straight reverse | ported |
| Recovery timeout | `rcRecoveryTimeOut` 10 s → req+0xB0 reset **only if rcResetAllowed** (false) | not needed |
| Still stuck | retire stuck timer 0x77E3B0: 20 s within 5 m, no racing record (state 0) within 100 m | ported |
| Retired | drive 0x79C6E0: terminal damage or stuck → force brake + no-steer; below rtStoppedSpeed 0.5 m/s, req+0x38 = 1 | ported (force brake + no-steer) |

New details [V]:
- req+0x38 belongs to the request's first 0x3C bytes. They form a message object: vtable
  0xCF7218, clone 0x7794A0/0x774E20, setter 0x7A3130, getter 0x7A3150. Its vfunc +0xC,
  0x792260, calls 0x78EB10 when +0x38 is set. 0x78EB10 copies a 0x2C-byte record looked up
  by key +0x34. The reader of the flag outside the AI was not found. Nothing that removes
  a car from the world was found on this path [I: the stopped car stays as an obstacle].
- Retire activate 0x77D570 loads `rtRetirementResetTimer` (cfg+0x48C; XML 12.0) into the
  behaviour's +0x34. Its consumer was not traced.
- Failures and damage (retire score 10) drive to a retire point at `rtRetirementSpeed` 15
  with the stopping brake 0x788A90. Only terminal damage and stuck (score 23) stop on the
  spot.

## 3. The player's reset: ResetLinesManager

Construction (race set-up 0x6797F0) [V]:
- race+0x22AC = `ResetLines`: ctor 0x7E6160, vtable 0xCFB778, size 0x54, a vector of
  0x20-byte elements reserved for 32 (0x7E78A0).
- race+0x22B0 = `ResetLinesManager`: ctor 0x612870, vtable 0xCE6584, size 0x33C.
  - Its +0x50 is race+0x1040, the per-vehicle track tracker set.
  - It loads the OSD strings `osd_warning_reset` and `osd_wrong_way`.
  - Per-frame update 0x612C90.

### 3.1 Auto-reset trigger 0x60D590 [V]

The update runs it when manager+0x218 is set. It applies only to the local player
(0x9EE380(0)), and not when vehicle+0x2E5C == 1.

```
s = |linear velocity| + |angular velocity|      (0xA69800, 0xA69820; f32, rsqrtps)
u = pose row 1 . y                              (0xA69630; the car's up axis has y = u)
if s > 0.8 (0xDB29F0) or u > 0.17 (0xDB29EC):  timer(+0x214) = 0
elif timer < 2.0 (0xDB29E8):                    timer += dt
elif timer < 3.0:                               timer += 2.0   (fires once per stretch)
    if vehicle+0x2E5C == 2: 0x9E64E0(6, 0)     [I: another mode's handling]
    else: request reset 0x60D0E0(player, delay 0, …, kind 0)
```

In words: the car is almost still (speed plus spin at most 0.8) and tilted past about 80°
(on its side or roof) for 2 s. A port must take the lengths from the F321 tables, not
hardware rsqrtps (owner rule F320, F324).

Other requesters [V]:
- The pause menu's `reset_car` entry (state 0x507520, next to `retire_race`, `restart`
  and `instant_replay`).
- A debug-key path in 0x60DB40 that resets every car (not gameplay).
- A delayed-request queue at manager+0x104. Its entries count down +0x10 and then call
  0x60D0E0 [I: delayed resets].

### 3.2 Request 0x60D0E0 and the sequence [V structure, I roles]

- Jobs are kept per vehicle in a map at manager+0xEC. An existing job of kind 3, or of
  kind 1 with a delay no longer than the new one, wins.
- The job copies the vehicle's car+0xB14 byte, and sets car+0xB14 = 0.
- Timings: delay = argument; a time field of 5 (0xCB093C); a hold of 3.2 (0xCE6590) when
  manager+0xD8 is set; and `0.9 − clamp(0.02·|v|, 0, 0.4)` (0xCB0DE8, 0xDB29E4, 0xCB22A4)
  [I: a fade length that is shorter at speed].

The phase machine 0x5EBDA0 returns events that the update 0x612C90 handles:

| Event | Action |
|---|---|
| 0 (start) | 0x5EC0E0: choose the target track position, see below |
| 2 (fade-out starts) | camera hold 0x606990(veh, 0, …); **vehicle+0x2C90 vfunc +0x38(1)**; vehicle+0x88 = 1, +0x8C = 15 |
| 4 (screen black) | car+0xB14 restored; vehicle+0x20 = 1; **placement 0x9145D0**; 0x5EBF90; camera back; vehicle+0x89 = 1; **vfunc +0x38(0)** |
| 5, 7 | camera 0x606990(veh, 1, …) |
| 9 | job removed |

The target track position (0x5EC0E0) is a (line, distance) pair:
- it starts from the tracker's current position (record +0x74/+0x78);
- a saved position (tracker sub-object +0x44/+0x48) replaces it when 0x90BDE0 accepts it
  (a distance comparison in the line's direction);
- 0x90C0F0 offsets it along the line (manager+0x220 × a length, or the constant at
  0xCE3970).

[I] In effect, the car goes back to its last good place on the line. The exact choice is
not established.

### 3.3 Placement 0x9145D0 [V]

The call is `0x9145D0(this = race+0x1040 trackers, vehicle, &trackpos, 1, 1, speed = 0.0)`
(disassembly 0x612F62..0x612F75):

1. Tracker record = 0x90D820: the entry of the list +0xAC..+0xB0 whose +0xA0 is the
   vehicle. 0x913CB0 moves it to the stored track position (0x912730 recomputes its line
   point).
2. Position p = record+0x30. Heading = (rec+0x48, rec+0x44, −rec+0x40): the record's +0x40
   vector turned 90° about Y [I: +0x40 is the line's lateral vector]. Argument 5 = 1 keeps
   the sign.
3. Height limit: a vertical cast 0x96B7F0 from p.y + 1 − 10000 to p.y + 1 + 10000. On a
   hit at fraction f, h = f·20000 + p.y + 1 − 10000 + 1. The limit is max(p.y + 1, h).
4. **0x9EF040(car, x, z, &heading, speed 0, limit)**. This is the setter the grid start
   uses (docs/PROGRESS_AND_SPAWN.md, GROUND_PLACEMENT.md, LIVE_SPAWN_CAPTURE.md):
   - identity pose;
   - linear and angular velocity zero;
   - ground placement 0x9DE4E0 with the wheel geometry, below the limit (the grid start
     uses 10000);
   - pose set again, then velocity = speed × forward = 0;
   - every wheel's spin = speed / radius = 0, and the gear chosen again for that speed.

### 3.4 Collision during and after a reset

- vehicle+0x2C90 vfunc +0x38 is called with 1 from the start of the fade-out until the
  placement, and with 0 after it [V]. The target was not decompiled [I: physics or
  collision off while the car is invisible].
- No separate no-collide period after the placement was found.

## 4. What the port does (F332)

- **No AI respawn**. This matches the original: AI cars never reset.
- **A car retired by F323** (its own pipeline failed: a port-only state) stays held at its
  last pose with zero velocity, still colliding.
  - This matches how the original leaves a retired or wrecked car standing in the world.
  - AUTHORED: the hold is instant. The original's retiring car first brakes (or, for
    failures, drives to a retire point at 15 m/s). The port cannot run a failed car's
    physics.
- **Followers are freed by the ported avoid and recovery behaviours.** No new mechanism
  is needed. The regression `twelve_car_ai_field_passes_a_car_held_on_the_grid`
  (multi.rs) locks this in.
- `grid-opponent-race` has a new option `GRID_RETIRE_CAR=car:tick`: that car's controls are
  NaN at that tick, so the shared world retires and holds it. The tool also prints
  `retire` lines and per-car `laps_covered`.

## 5. Evidence (12-car Donington AI races, release, DF01 deformation)

Outputs are in `local/reset-respawn/`. The command was `grid-opponent-race ROOT CD01 BP01
12 6000 ai` with `GRID_RETIRE_CAR` set:

| Retired | Situation | Result |
|---|---|---|
| car 5 at tick 1 (on the grid) | car 7 starts right behind it | car 7 avoids it at tick 41 (ETA 1.94 s) and passes. **0 contacts** with car 5. All 11 cars cover 0.97..1.07 laps; car 5 is classified "Retired, 0 laps" |
| car 3 at tick 1500, 60 m/s in traffic | frozen on the racing line | car 4 hits it (182 contact records), goes off the track, runs recovery from tick 1618 and races again at tick 2200 (0.924 laps at the end). Cars 6/7/8/9/10 touch it (25..141 records) and go on (0.96..0.98 laps) |
| car 1 at tick 2000 | in traffic | cars 7 and 10 touch it (63, 85) and go on; every other car covers ≥ 0.969 laps |

## 6. Player auto-reset port (F341)

Code: `grid_world::reset_lines` (detector, limit, drivetrain tail), `spawn::align_below`,
`Runtime::place_at_rest`, `MultiRuntime::place_at_rest`, `ai_driver::Line::reset_target`
and `source_vehicle::player_reset::PlayerReset` (used by grid-app `drive_tick` and the
soak `Session::step`).

New static facts (read-only, analysis image constants and r9/r10/r12 decompilation):
- [V] Constants: 0xDB29F0 = 0.8, 0xDB29EC = 0.17, 0xDB29E8 = 2.0, 0xCAFBE8 = 10000,
  0xCAFBE4 = 20000, 0xD17278 = −2000 (bit-exact nearest f32 values).
- [V] 0x9DE4E0's center and corner casts start at its argument 6 (the limit) and end at the
  fixed Y 0xD17278 = −2000. The grid start passes 10000, which gave the earlier fixed
  10000/−2000 pair. `spawn::align_below` takes the limit; `align_with_normalization`
  passes `GRID_LIMIT` = 10000, so the grid start is unchanged.
- [V] 0x9145D0's limit cast runs upward, from p.y + 1 − 10000 to p.y + 1 + 10000; a hit at
  fraction f gives f·20000 + (p.y + 1 − 10000) + 1, kept if above p.y + 1.
- [V] 0x9EF040's speed-0 tail: wheel +0x104 = 0 / radius (+0xE0) for +0x1620 wheels; the
  gear is the first of gears 0..9 with a non-zero ratio and the largest torque-curve value
  (+0x17AC, 31 nodes, index scale 0xD15028, top 0xD1502C = 1675.5) at the engine speed
  *(+0x18D8)+0x104 / ratio; +0x1894 = that speed (idle +0x17A8 if the ratio is 0); gear 0
  becomes 1. [I] +0x18D8 points to a wheel record, so the speed is 0 and the result is
  gear 1 with engine speed 0.
- [V] 0x5EC0E0's inline vector length: z² + (y² + x²), RSQRTPS seed s, root = s·sq,
  ((1 − root·s)·root)·0.5 + root, zero kept. [I] 0x60D590 uses the same inline helper
  (it was not re-decompiled); `reset_lines::length` takes the seed from the F321 table.
- [V, structure only] 0x5EC0E0's target: tracker +0x74/+0x78 (current line position);
  with the job byte set it is moved by 0x90C0F0 by the constant 0xCE3970, otherwise, when
  tracker-sub +0x18 is set, by manager+0x220 × |record +0x20 − record +0x30|. When +0x18
  is clear, a saved position (sub +0x44/+0x48) replaces it if 0x90BDE0 finds it behind.

Port decisions:
- Detector exactly as 3.1 with the frame time 0.016 s; a non-finite state restarts the
  stretch. It runs after each successful player tick; a retired player (F323) is skipped.
- [I] Target: the AI racing line point at the car's projection and the line direction
  there. The tracker's own line, the saved point and the offsets are not ported.
- Placement: limit cast and `align_below` with ReferenceSse normalization and the current
  wheel displacements; pose through the verified bridge (B12710 path, F062/F160 bridge
  code), both velocities zero in every lane, then the drivetrain tail. Damage, deformation,
  suspension, lap state and the body links in the shared world are kept.
- AUTHORED: the car is placed on the tick the detector fires. The fade-out, camera hold,
  vfunc +0x38 collision switch and the pause menu's `reset_car` are not ported.
- grid-app logs `drive_auto_reset` (point and heading, or the placement error, after
  which the car drives on). The soak Session counts `auto_resets` and returns a placement
  error as a failed step. `R` clears the detector.

Checks: `player_reset` asset tests flip car 0 on its roof on the Donington grid, solo and in
a two-car shared world. Both fire at tick 260, 125 ticks after the car became still and
tilted (tick 135). Placed: up Y 0.99981, 0 m from the line point, heading · line 0.99981,
speed 0, gear 1, wheel spins 0, damage words unchanged. After 2 more seconds: up Y 0.99982,
drift 0.0023 m, speed 1.4e-5 m/s, road 0.39 m below the body origin. With full throttle the
solo car reaches 32.9 m/s in 4 s.

## 7. Not ported and open

1. The tracker's own line and target choice (3.2, section 6), and what ResetLines'
   0x20-byte elements hold. The fade timing and the collision switch during it.
3. The target of vehicle+0x2C90 vfunc +0x38; the reader of req+0x38; the handler
   0x9E64E0(6, 0); and the `ResetCarToTrack`, `VehicleDisqualified` and `EnableAIControl`
   messages (handler 0x401380 → AI 0x79E240).
4. The record state of a retired car in the retire-stuck 100 m test. The port treats a
   held car as racing (it is not moving backwards). So a follower pinned within 100 m of a
   held car never retires [I]. Not changed without evidence.
5. The car-side bytes +0xDE4/+0xDFC that vehicle_driver_prepare rejects (labelled
   "respawn" in docs/RUNTIME_LIMITS_AUDIT.md). The reset path above does not write them.
   Their meaning stays open.
