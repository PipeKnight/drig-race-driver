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
  the pause menu's "reset car". Over a 1 s fade the car's controller is off, at 0.5 s it is
  placed on the route's progress line by the **grid-spawn setter 0x9EF040** with zero speed,
  and it does not touch other cars until a 5 s tail has passed and no car overlaps it. There
  is no time penalty (3.2). The port (row 43, F762) follows this (section 6).
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
| Retired | drive 0x79C6E0: terminal damage or stuck → force brake + no-steer; below rtStoppedSpeed 0.5 m/s, req+0x38 = 1 | ported (force brake + no-steer; the removal request since F620, section 8) |

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

### 3.2 Request 0x60D0E0 and the job [V, row 43 re-read]

- Jobs are kept per vehicle in a map at manager+0xEC. An existing job of kind 3, or of
  kind 1 with a delay no longer than the new one, wins.
- The job copies the vehicle's car+0xB14 byte, and sets car+0xB14 = 0.
- Kind 0 (the player's reset, from the trigger and the menu): delay 0, a fade of
  **1.0 s** (vehicle+0x6C == 0), the placement when the fade passes its middle (0.5 s),
  and a tail of **5 s** (0xCB093C).
- Corrections to the F332 reading:
  - `0.9 − clamp(0.02·|v|, 0, 0.4)` (0xCB0DE8, 0xDB29E4, 0xCB22A4) belongs to kind 2
    (a damping of the car's speed), not to the player's fade.
  - The 3.2 s hold (0xCE6590) needs manager+0xD8, and only zero is ever written there:
    it is never armed. The 88 m advance (0xCE3970) behind the same flag is dead too.

The phase machine 0x5EBDA0 returns events that the update 0x612C90 handles:

| Event | Action |
|---|---|
| 0 (start) | 0x5EC0E0: choose the target trackpos, see below |
| 2 (fade-out starts) | **0x606990(veh, 0, …) disables the car's controller** (not a camera hold); vehicle+0x2C90 vfunc +0x38(1); vehicle+0x88 = 1, +0x8C = 15 |
| 4 (fade half way) | car+0xB14 restored; vehicle+0x20 = 1; **placement 0x9145D0**, retried while the car overlaps another (below); **0x5EBF90 puts every body of the car in collision layer 0xE** (no car-to-car contact); controller enabled again; vehicle+0x89 = 1; vfunc +0x38(0) |
| 5, 7 | 0x606990(veh, 1, …): controller enabled |
| 9 (tail over) | each frame: while the car overlaps another car (car+0xDD0 phantom) nothing happens; otherwise layer 0x12 is restored on all bodies (0xABC250 filter setter) and the job is removed |

Overlap at the placement [V]: after 0x9145D0, while the car's phantom (car+0xDCC, type 7)
overlaps another car and fewer than 20 tries were made, the trackpos is moved 1 m back
(0x90C0C0(1.0) → 0x90BEC0) and the car placed again. The last try stands; the layer
switch then keeps the cars apart until event 9.

The target trackpos (0x5EC0E0) [V]:
- It starts from the tracker record's current trackpos (+0x74 line, +0x78 distance).
- The record's route is rec+0x80 (0x912670 from vehicle+0x2E54); route+0x18 is the
  closed byte, route+0x19 the reversed byte.
- Closed route (a circuit): while the trackers' clock (trackers+0x120, int64 µs; [I] the
  race clock) is below 15 s (manager+0x224), the target moves ahead by manager+0x220 = 1.5
  times |rec+0x20 − rec+0x30|, the 3D distance from the car to its line point (0x90C0F0 →
  0x90BFC0: forward, wrapping).
- Open route (a point-to-point run): a target past the route's +0x44/+0x48 trackpos ([I]
  the finish) is clamped to it (0x90BDE0: "is this trackpos ahead of that one").
- The F332 reading of +0x44/+0x48 as a saved "last good" position was wrong: the car is
  reset at its own current trackpos.

The tracker line [V]: the route is the `progress_track.xml` route, **not the AI racing
line**. 0x915100 updates the record each frame: rec+0x20 = the vehicle position
(0x90CA00). 0x912980 places a trackpos: rec+0x30 = the lerp of the neighbouring points'
positions (point +0x10, distance +0x34); rec+0x40 = the lerp of their horizontal unit
directions to the next point (point +0x20, 0x909C00), negated on a reversed route.

No time penalty is applied anywhere on this path [V], and wrong way (0x5F7880) only shows
the `osd_wrong_way` warning; it never requests a reset. The still-and-tilted detector
(3.1) is the only automatic trigger: being off the track, below a kill plane or stuck
upright does not reset the player.

### 3.3 Placement 0x9145D0 [V]

The call is `0x9145D0(this = race+0x1040 trackers, vehicle, &trackpos, 1, 1, speed = 0.0)`
(disassembly 0x612F62..0x612F75):

1. Tracker record = 0x90D820: the entry of the list +0xAC..+0xB0 whose +0xA0 is the
   vehicle. 0x913CB0 moves it to the stored track position (0x912730 recomputes its line
   point).
2. Position p = record+0x30. Heading = (rec+0x48, rec+0x44, −rec+0x40): the record's +0x40
   direction turned 90° about Y (3.2: the line's horizontal direction). Argument 5 = 1
   keeps the sign.
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

- [V] From the placement (event 4) until the job ends, the car's bodies are in layer 0xE,
  which other cars do not collide with. The job ends at the first frame after the 5 s
  tail in which the car overlaps no other car; then layer 0x12 comes back. The F332 note
  "no separate no-collide period" was wrong.
- vehicle+0x2C90 vfunc +0x38 is called with 1 from the fade start until the placement,
  and with 0 after it [V]. Its target was not decompiled.

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

## 6. Player reset port (F341, row 43)

Code:
- `grid_world::reset_lines`: the detector, the limit, the drivetrain tail, `TrackerLine`
  (the 0x912980 points and directions), `Tracker`, `target` (0x5EC0E0) and `Job`
  (0x5EBDA0 / 0x612C90, kind 0).
- `spawn::align_below`, `Runtime::place_at_rest`, `MultiRuntime::place_at_rest`,
  `MultiRuntime::set_ghost` / `overlaps_other_car`.
- `source_vehicle::player_reset`: `ResetLine` (the route's progress line), `PlayerReset`
  (the job per tick) and `place` (the placement with the overlap retries). grid-app
  `drive_tick` and the soak `Session::step` use it.

Static facts from F341, still valid:
- [V] Constants: 0xDB29F0 = 0.8, 0xDB29EC = 0.17, 0xDB29E8 = 2.0, 0xCAFBE8 = 10000,
  0xCAFBE4 = 20000, 0xD17278 = −2000 (bit-exact nearest f32 values).
- [V] 0x9DE4E0's center and corner casts start at its argument 6 (the limit) and end at the
  fixed Y 0xD17278 = −2000. The grid start passes 10000. `spawn::align_below` takes the
  limit; `align_with_normalization` passes `GRID_LIMIT` = 10000, so the grid start is
  unchanged.
- [V] 0x9EF040's speed-0 tail: wheel +0x104 = 0 / radius (+0xE0) for +0x1620 wheels; the
  gear is the first of gears 0..9 with a non-zero ratio and the largest torque-curve value
  (+0x17AC, 31 nodes, index scale 0xD15028, top 0xD1502C = 1675.5) at the engine speed
  *(+0x18D8)+0x104 / ratio; +0x1894 = that speed (idle +0x17A8 if the ratio is 0); gear 0
  becomes 1. [I] +0x18D8 points to a wheel record, so the speed is 0 and the result is
  gear 1 with engine speed 0.
- [V] 0x5EC0E0's inline vector length: z² + (y² + x²), RSQRTPS seed s, root = s·sq,
  ((1 − root·s)·root)·0.5 + root, zero kept. `reset_lines::length` takes the seed from the
  F321 table (owner rule F320: no hardware rsqrtps).

What the port does (row 43):
- **Triggers.** The detector exactly as 3.1, after each successful player tick, with the
  frame time 0.016 s; a non-finite state restarts the stretch. The pause menu's Reset
  Vehicle (Button3 on the main pause screen, then Yes) calls `PlayerReset::request`; the
  job starts at the next tick. Both start the same job. A Demolition Derby car is
  disqualified by the detector instead (0x9E64E0(6, 0), F341). A retired player is never
  reset. There are no other triggers (3.2).
- **Line.** `ResetLine::load` takes the route's progress points as `lap_progress` prepares
  them: a circuit from its finish around the lap, closed; a point-to-point run in travel
  order from its start, open, with its finish distance. A run against the file's order
  (Haruna downhill) keeps `reverse`, so the directions are negated as in the original.
  `lap_progress::run` now keeps the route's `reverse` flag (it was forced to false; the
  gates and points were already in travel order, and no other reader used the flag).
- **Tracker.** AUTHORED: the car's trackpos is a nearest-point projection on the line,
  searched from 2 segments behind to 6 ahead of the last one, and on the whole line at
  first or when that window is more than 30 m away. The original's own tracker update
  (0x915100) is not ported (7).
- **Target.** `reset_lines::target` as 0x5EC0E0. The clock is the race clock (grid-app:
  the trial's race time; without a race trial and in the soak, AUTHORED, the segment's
  ticks × 0.016).
- **Job.** Kind 0 timings. Input is neutral from the request until the placement, for
  any driver (keyboard, pad, wheel, autopilot). The placement comes at the job tick that
  passes 0.5 s, on the line point with the heading of 3.3, through `place_at_rest`
  (limit cast, `align_below`, both velocities zero, the drivetrain tail; damage,
  deformation, suspension, lap state and body links kept). In a shared world the car is
  placed in slot 0 and tried again 1 m back while it overlaps another car's bodies, at
  most 20 tries. From the placement the car is a ghost: the shared world's pair loop skips
  every pair of it with another car (layer 0xE). After the 5 s tail the job ends at the
  first tick in which the car overlaps no other car; the ghost flag is cleared then.
  AUTHORED: a menu request while a job runs is refused, and the detector restarts at the
  job's start.
- **Screen and camera.** [I] `PlayerReset::darkness` rises linearly to 1 at the placement
  and falls back over the second half of the fade; grid-app draws it as a black
  full-screen overlay (`reset_fade.rs`, below the flashback whiteout and the menus).
  [I] The cameras restart their history at the placement, as at a new segment, so the
  chase camera does not swing across from the old position. Both are AUTHORED
  presentation: the readers of vehicle +0x88/+0x89/+0x8C and the 0x5F9AE0 / 0x5F18C0
  messages are not identified (7).
- **Penalty.** None, as in the original.
- **Logs.** grid-app logs `drive_reset version=2` with `event=start trigger=auto|menu
  target=`, `event=place point= heading= distance= retries=`, `event=end` and
  `event=disqualified`, and the menu's `drive_menu_reset version=2 accepted=`. A failed
  placement is logged with `error=`; the job is dropped and the car drives on. The soak
  Session counts started jobs and disqualifications in `auto_resets` and returns a
  placement error as a failed step. `R` (restart) clears the job; the flashback saves and
  restores the whole reset state with its checkpoints.

Route kinds: circuits and point-to-point runs use their progress lines. The Yokohama drift
freestyle arenas (routes 3–6) have only placeholder progress (2–3 gates); [I] it is the
only line the original's tracker has there, so the port uses it. Their gates are not valid
lap gates, so for a route that ignores progress `ResetLine::load` reads the file as it is
(`lap_progress::read_route`) instead of the lap preparation, which rejects it. The Detroit derby stadium
(route_8/9) disqualifies. Drift battles on circuits use the circuit line.

Checks (2026-11-09, release, the Lacetti from `slot_00`, flipped on its roof on the grid):
`reset_lines` 11 unit tests; `player_reset` unit test and 7 asset tests, all passing.
- Flip: 20 runs cover 11 circuits, Haruna routes 1/4/9 both ways and Yokohama arenas 4–6.
  In every run the auto trigger fires 125 ticks (2.0 s) after the car comes to rest tilted,
  places the car 32 ticks (0.5 s) later and ends 377 ticks (6.0 s) after the start. At
  the placement the car is upright (up_y ≥ 0.998), heads along the line (≥ 0.998), stands
  0.40–0.47 m above the ground, is at rest in first gear and keeps its damage. Throttle
  offered during the fade does not reach the car. With full throttle after the job it
  drives 50–59 m in 4 s on circuits and touge roads (top speed 32–34 m/s). In the arenas
  it drives 19–52 m before the arena wall, because the placeholder line points at the
  wall.
- Wedge: a reset onto another car steps back 5 × 1 m and is placed at 295 m.
- Derby: a flipped derby car is disqualified, not reset.
- Pause menu: the shared-world test and the menu reset run the same job. The grid-app
  test `pause_menu_reset_vehicle_runs_the_reset_job` drives the interactive pause menu
  (Donington). The request is accepted at tick 1500, the job starts at 1502, the car is
  placed at 1533 and the job ends at 1878. The darkest frame is 0.992, and the controls
  are neutral while the controller is off.

## 7. Not ported and open

1. The original tracker update 0x915100 (how the trackpos follows the car, its search
   window, shortcuts); the port projects on the nearest segment.
2. The readers of vehicle +0x88/+0x89/+0x8C and the 0x5F9AE0 / 0x5F18C0 messages sent
   at the fade's start and end: whether they drive a screen fade, a camera cut or a car
   visibility fade. The port's black fade and camera cut are [I].
3. The target of vehicle+0x2C90 vfunc +0x38; the reader of req+0x38 (still unknown after
   F620's look, section 8); and the `ResetCarToTrack`, `VehicleDisqualified` and
   `EnableAIControl` messages (handler 0x401380 → AI 0x79E240).
4. ~~What ResetLines' 0x20-byte elements (race+0x22AC) hold~~: answered in section 10.1 (F806). The reset path above does not
   read them.
5. The record state of a retired car in the retire-stuck 100 m test. The port treats a
   held car as racing (it is not moving backwards). So a follower pinned within 100 m of a
   held car never retires [I]. Not changed without evidence.
6. The car-side bytes +0xDE4/+0xDFC that vehicle_driver_prepare rejects (labelled
   "respawn" in docs/RUNTIME_LIMITS_AUDIT.md). The reset path above does not write them.
   Their meaning stays open.
7. The original's overlap test uses the car's phantom (car+0xDCC/+0xDD0); the port tests
   the car's world bodies against the other cars' bodies (`overlaps_other_car`) [I].

## 8. A car wedged against tyre stacks (Washington, F620)

Owner report (2026-10-27): in a 12-car Viper race at Washington (`GRID_AUTOPILOT=1`, 26 000
ticks) the AI-driven player car stopped at the hairpin tyre stacks and never drove off.
Evidence: the playtest log, race.ini and replay spool (private, `local/stuck/` and the
reporting session's `app-05/evidence`).

What happened [V from the log, the spool and a headless repro]:
- Tick 13 786: the car stops against the stacks the field broke at 13 308–13 310 (954, 956,
  959). Recovery reverses (gear 9, throttle 1, full lock) from 13 799. The car moves 3.4 m,
  breaks stack 957 (tick 14 482) and wedges its rear against the standing tyre columns at
  the barrier. From about 14 850 the position stays within millimetres.
- The block is physical. The contacts are car–piece only (20–40 per tick, normals pushing
  the car forward). The pieces are awake, not parked, 38–107 kg and do not move. There are
  no static body contacts.
- Damage is not the cause (scope item b): handling status 7 (running), power +3A4 and
  driveshaft +3A8 at 1.0, no terminal wheel, wheel levels at most 0.48. The rear wheels spin
  (slip 5.6 m/s). An undamaged Viper in the repro sticks the same way.
- 15 733: the retire test fires (20 s within 5 m, no racing car within 100 m). Force brake,
  no-steer, handbrake. 21 056–23 311: cars come within 100 m, retire drops, and recovery
  reverses again. Then retire again.

The original does the same up to this point [V]:
- 0x4022F0 counts a dynamic body only above 700 kg (body +0x3F0, A69370) and below 2 m/s.
  Tyre pieces never toggle recovery to forward.
- Mode 5 reverses until 80° < |a| < 90° or |b| < 5°, which a car that cannot move never
  reaches. `rcResetAllowed` is false, and the player's auto-reset (3.1) needs the car on its
  side or roof.
- The retire behaviour's vfuncs (vtable around 0xCF8FA8: score 0x78B300, activate 0x77D570,
  empty deactivate 0x776CA0, drive 0x79C6E0) never read the `rtRetirementResetTimer` value
  that activate stores at +0x34.

What the port lacked [V]: drive 0x79C6E0 sets the removal request req+0x38 in its
**terminal/stuck** branch (force brake, no-steer, speed below rtStoppedSpeed 0.5). The
port set the request only in the failure branch's retire point. Since F620
`Behaviour::Retire` sets it as well.

grid-app (AUTHORED, as before for opponents):
- An opponent's request retires it in the standings (`drive_ai_event … retired=removal`,
  formerly `retired=failure`, plus the notice).
- The autopilot's request now ends the player car's race as terminal damage does (retired,
  standings finalized, `car=0 retired=removal`).
- The car stays in the world. The reader of req+0x38 in the original is still unknown (§7
  item 3).

Repro: `multi::tests::a_car_wedged_against_tyre_stacks_retires_and_requests_removal`
(ignored, needs game data):
- breaks stacks 954/956/959;
- places the Viper at the playtest pose of tick 13 790;
- replays the recorded input to tick 14 700 (35 changes);
- hands the car to the AI for 1 800 ticks.

Recovery, then retire. The car moves 1.5 m and requests removal.

The headless run of the same race.ini (current main, and the evidence's own source rebuilt)
does not reach this state. Opponent contacts differ from tick 814 (a rebuilt binary, not
the playtest's), so the race diverges.

## 9. The retire path 1:1 (F755, backlog rows 45 and 46)

Static analysis of the hash-pinned image (`grid_re.py dis`; four of the functions are outside
the master analysis). Before F755 the port's retire test lacked the stuck latch, the racing-state
gate and the terminal input, and grid-app retired a terminally damaged AI on the spot.

### 9.1 AI racing states [V]

The AI driver keeps its racing state as a name string. Nine globals hold a pointer and a
0x20-byte buffer, filled at start-up by 0xC79C60–0xC79D40 through 0xA66940 (copy into a
0x20-byte buffer). The tests compare names with strcmp 0xA66720.

| state | string | global (pointer, buffer) |
|---|---|---|
| ai::RacingStateAtLights | 0xCF91FC | 0xE4CCB0, 0xE4CCB4 |
| ai::RacingStateRaceComplete | 0xCF9214 | 0xE4CCD8, 0xE4CCDC |
| ai::RacingStateCrashed | 0xCF9230 | 0xE4CD00, 0xE4CD04 (no code reference) |
| ai::RacingStateCruise | 0xCF9248 | 0xE4CD28 |
| ai::RacingStateInactive | 0xCF9260 | 0xE4CD50 |
| ai::RacingStatePreLights | 0xCF9278 | 0xE4CD78 |
| ai::RacingStateRacing | 0xCF9294 | 0xE4CDA0, 0xE4CDA4 |
| ai::RacingStateRetire | 0xCF92AC | 0xE4CDC8, 0xE4CDCC |
| ai::RacingStateStopping | 0xCF92C4 | 0xE4CDF0 |

The state objects' transition vfuncs (vtable slots 0xCF772C … 0xCF7804) read a status record
`p = [ebx+0x158]`, `status[p+4]`:

| vfunc | from (by the names it returns) [I] | transition |
|---|---|---|
| 0x7846A0 | AtLights | → Racing |
| 0x784870 | Racing | own car state +0xE3 == 1 or status byte 3 → Retire; byte 2 → RaceComplete; else stays |
| 0x784A80 | Cruise/Stopping [I] | byte 2 → RaceComplete; byte 3 → Retire |
| 0x784760 | RaceComplete | [+0x15C] == 1 → Cruise, else Stopping |
| 0x7849C0 | PreLights | → AtLights |
| 0x784990, 0x77F720 | Retire, Inactive [I] | none |

No transition leads out of Retire. Readers of the state in the behaviour tests:
- the retire stuck timer 0x77E3B0 runs only in Racing (0xE4CDA0);
- the cruise test 0x77E060 (type 4) fires in RaceComplete, Retire, Cruise and Stopping;
- the stop test 0x77E7D0 (type 2) fires in PreLights (with its +0x1C), RaceComplete, Retire,
  Stopping and Inactive; common setup 0x7978D0 then forces the brake for every behaviour.
  This closes docs/AI_AVOID_RECOVERY_RESEARCH.md open item 6.

### 9.2 Retire test, score and drive [V]

Retire test 0x784560 (type 8, 0x30 bytes), every tick:
- clears +8 (fired), +0xC, +0xD, +0xE and the count +0x10;
- +0xC = own car state +0xE3 == 1 (terminal damage);
- +0xD = damage > `damage_retirement_ratio` and +0xE = damage ≥ 0.5, both only behind the
  context flag ctx+0x34 → +0x1D (the engine-failure gate);
- +0x10 = count of the four failure bytes from car vfunc +0x18;
- fired if +0xC, +0xD or the count; then calls the stuck timer.

Stuck timer 0x77E3B0:
- not in Racing, or moved ≥ `rtStuckDistanceSq` (cfg+0x4C8, 25) from the reference +0x20:
  reference = position, timer +0x18 = 0, latch +0x14 = 0;
- else timer += dt; past `rtStuckTime` (cfg+0x4B4, 20 s) it walks the records. A racing
  record (+0xFC == 0) within 0x7A3840(0x3D) = `rtPlayerDistanceSq` (10 000) returns **without
  touching the latch or the fired flag**; otherwise latch = 1 and fired = 1.

So a latch set earlier survives a passing car, but the test does not fire while that car is
near; the score falls to 0 and recovery can take over until it is clear again.

Score 0x78B300: 0 if not fired; 10 if fired and neither +0xC, the latch nor behaviour +0x60
is set; otherwise 23.

Drive 0x79C6E0:
- +0xC or the latch: req+0xA5 force brake, req+0xA6 no-steer; below `rtStoppedSpeed`
  (cfg+0x4F0, 0.5 m/s) req+0x38 = 1;
- else +0xD or the count: 0x79A090, the drive to a retire point. Its +0x60 is set while a
  retire point is found (0x79A144, cleared at 0x79A14E); req+0x60 = the point. Below
  stoppedSpeed +0x40 = 1 and req+0x38 = req+0x39 = 1; with +0x40 set it forces the brake.

Activate 0x77D570 clears +0x30, +0x31, +0x38, +0x3C, +0x40 and +0x60 and stores
`rtRetirementResetTimer` at +0x34. The two drive branches belong to one behaviour, so moving
between them does not activate it again.

### 9.3 What happens to the entry [V]

- The race manager keeps a retired AI as an entry. Race over 0x90E1B0 (type 1) needs every
  entry to reach the event laps, so a stopped AI blocks it; the race ends when the player
  finishes and finalisation 0x911F30 / 0x90FCD0 estimates or retires the others (cars below
  5 mph are retired).
- The reader of req+0x38 is not traced (§7 item 3). The setter of the car-state byte +0xE3 is
  not traced either; the port maps it to handling status 3 or 4 (`terminal_wheel()`) [I].
- The physics side of terminal damage: status 3 sets +0x226C, which disables the controls,
  and the car rolls to rest (port: under 3 m/s within 4 s, the wheel-detach test).

### 9.4 Port (F755)

`AiDriver::retire_test` (ai_driver.rs):
- terminal damage (`View::terminal`) moves the driver to the Retire racing state (`retiring`);
  the stuck timer then stays reset, as outside RacingStateRacing;
- the stuck timer keeps the sticky latch and a per-tick fired flag;
- fired = terminal, the failure/damage branch, or the stuck timer;
- the score is 23 for terminal, latched, or once the failure drive holds its retire point
  (`Failure::point`, behaviour +0x60; the port's retire point, the nearer track limit, is
  always found [I]); else 10;
- `Behaviour::Retire` (stop on the spot) for terminal or latched, else
  `Behaviour::RetireFailure`; switching between them does not activate again.

So a terminally damaged AI brakes with no steer and, below 0.5 m/s, requests removal.

Standings (AUTHORED, rows 45 and 46): the removal request retires the AI in the standings
(grid-app `retired=removal` with the notice; grid-opponent-race `reason=terminal-damage`,
`lost-wheel` or `ai-failure`). The original keeps the entry until finalisation. The port
retires earlier so that an AI-only field (autopilot, headless) still ends by itself and the
HUD list drops cars that will never move again. A terminal AI is no longer retired on the
spot: it first stops. A pursuit driver (no retire drive) still retires on terminal damage.

Not ported: the RaceComplete, Cruise and Stopping states after an AI finishes (it keeps
racing at pace), the stop test's state list, and the status byte 3 that moves the Racing
state to Retire (writer not traced).

## 10. Reset lines: final boundaries (F806)

Static analysis of the master Ghidra project, 10 November 2026; no game run.

### 10.1 The object [V]

`ResetLines` (race+0x22AC) is a mesh test, not part of the player reset of section 3:

- **Construction.** Ctor 0x7E6160 (size 0x54), called at 0x67A507 in the race set-up
  (allocation tag "RESETLINES"). Vtable 0xCFB778 = [0x7E6FB0 dtor, 0x7E4DF0 update,
  0x5A42B0]; the secondary vtable 0xCFB76C holds the resource callback 0x7DD3C0.
- **Materials.** 0x7DBE30(race+0x2054) sets +0x48 and +0x44 = 0x7EF510(8, parser
  0x7DBD30, 0), a per-material attribute set built from `surface_materials.xml`.
- **RESET block (parser 0x7DBD30).** The parser reads each material's `RESET` block:
  - key `type`: "warning", "reset" and "unrecoverable" all give **type 3**,
    "finalboundary" gives **type 4**, anything else 0 (the disassembly compares the three
    strings in turn and jumps to one store of 3);
  - key `delay`, default 2.0.
  - No parser value gives type 2, so the update's ResetCar branch (0x7DD1C0) is
    unreachable from data.
- **Mesh.** 0x7DD3C0 accepts the "binary" resource and calls 0x7DD280, which builds a
  mesh shape (0xA73DC0, layer 0xC) into +0x28 and a static body (0xA74190, identity
  pose) into +0x2C. The route file is `resetlines.cqtc`. 0x7E1250 sets +0x50 and the
  loaded flag +0x41 (called from 0x669700 at 0x669ECD); 0x7E4D80 frees it (from 0x63E4E0).
- **Cars.** 0x5ED5A0 calls 0x7E6220 for every vehicle whose +0x6C is not 3 or 4. 0x7E6220
  needs vehicle+0x24 and its +0xDB0 body, skips a car already in the list, and adds a
  query shape (0xA73490, flags 0x40000000, filter 0xD) to that body (0xA69250 with
  ECX = (vehicle+0x24)+0xDB0, local pose zero). Each 0x20-byte element holds [0] the
  vehicle, [1] the query shape, and words 4..7 the last position, zero at registration.
  This answers section 7, item 4.

### 10.2 Update 0x7E4DF0 [V]

When the mesh is loaded (+0x28, +0x41) and enabled (+0x48, +0x4C), for each element:

1. Read the query shape's world position (0xA69630): the car body's origin.
2. d = now − last. If |d|² > 0.001:
   - if |d|² < 25, run the segment query 0xA71F40(last, now, filter 0xD) → hit normal and
     triangle sub-id;
   - then store now as last.

   A smaller move keeps the old position, so slow motion accumulates until it passes
   0.001. A jump of 5 m or more (a reset, a teleport) is not tested.
3. On a hit, 0x5E2730(sub-id, set +0x44) gives the material's {type, delay}.
   `against` = dot(normal, now − last) < 0.
4. Dispatch by type:
   - 2 → 0x7DD1C0 ResetCar;
   - 3 → 0x7DD430 UnrecoverableCar (handler 0x612BD0: a human with its conditions is
     disqualified, otherwise a reset is requested through 0x60D0E0; the Car Monitor 0x9D1010
     needs `against`);
   - 4 → 0x7E1110 FinalBoundary (handler 0x5EBC90: no direction test; vehicle+0xA0 = 1
     and 0x9E64E0(6, 0), the disqualify of `Runtime::disqualify`; for a human it also
     holds the camera through +0x338, 0xE6E000, 0x832570 and +0x2C90 vfunc +0x38(1));
   - otherwise → 0x7DD480.

### 10.3 Data [V]

Only Haruna routes 0–9 ship `resetlines.cqtc`, and every triangle's material is FBND
(final boundary). The other 92 routes have none. So in the shipped game the mechanism means
one thing: **a car that leaves Haruna's road over an edge crosses an FBND sheet and is
disqualified**. Below the world, nothing else happens (section 3.2): there is no
below-world reset.

### 10.4 Port (F806)

`grid_tools::reset_line_crossing`:
- reads a route's `resetlines.cqtc` and the RESET records of `surface_materials.xml`;
- `MultiRuntime` keeps one `Tracker` per car;
- after every tick it runs the update above on each car still in the world, from the body
  pose translation;
- a final-boundary crossing calls `Runtime::disqualify`.

The app already ends the player's race on a disqualification (`post_race`). Types 0 and 3
are reported (`MultiRuntime::reset_line_crossings`) and not acted on, because no shipped
data uses them.

[I]:
- the query shape's filter 0xD is taken to see only the reset mesh (layer 0xC);
- the normal's winding is the CQTC triangle order;
- the human camera hold is not ported.

A disqualified AI keeps its disabled controls and is not removed from the standings.

### 10.5 The tall fence plates are not raised boxes [V, O]

The coordinator's question was whether the raised-bottom fence boxes push cars down. On Yokohama
Docks route_1, seed 3 (F789 and F804 builds), car 6 drops through the road. A static look at the
box it hits (read_route box 2481, trace numbering 2513, `barrier_tall_fence_a`) shows a thin
plate, not a raised box: about 0.06 m thick, 3.9 m long, y 1.81–5.96 m, its bottom at the
fence's foot level.

The trace shows what happens [O]:
- car 6 hits the plate at 21 m/s (normal (−0.906, 0, 0.423), impulses 2000–3191) and slows to
  8.8 m/s;
- its centre then reaches the plate itself (x −215.3 → −212.5 at z ≈ 261.3, plate at x ≈ −213.2);
- from tick 2416 to 2435 the reported normal is downward (ny −0.99). With the car this deep, the
  smallest separating axis is about 1 m down against about 2.4 m along the plate's normal;
- at tick 2436 the car falls through the road (y 2.12 → 0.59).

[I] The contact code is the port of 0xAE8AD0, the same algorithm as the original: a GJK
witness, then 0xB7ED10's sampled penetration, oriented by the centres (0xAEE640). The shape
path is the original's too (0x7CB440, 0x7C1E60). So the downward push is what that algorithm
gives once the car has passed into a thin plate. The open question is how the car got that
deep. The prop trace logs only impact events, so the missing rows at ticks 2409–2415 do not
prove that the contacts vanished; a per-tick contact trace is needed. Section 10.3 holds here
as well: the original does not reset a car that falls below the world, so the port does not
add a reset.
