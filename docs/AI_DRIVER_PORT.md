# AI driver port (F290)

15 October 2026, Claude Code. Implementation of the original race driver recovered in
[AI_DRIVER_RESEARCH.md](AI_DRIVER_RESEARCH.md). Code: `grid_formats::ai` (data) and
`grid_tools::ai_driver` (driver). Opponents in `grid-app --opponents N` and the headless
`grid-opponent-race ROOT CD01 BP01 CARS TICKS [ai|pursuit] [local/TRACE.tsv]` use it.

## Data (shipped files, read at start)

- `tracks/europe/donington/route_0/ai_track.xml`: 134 gates, racing line =
  position + normal × racing_line length (closed polyline 3944.03 m in link order),
  racing limits, 11 brake lines.
- `route_0/ai_vehicle_track.xml`: per-class brake data (`touring`), falling back to the
  track defaults when a class is absent.
- `ai/ai_vehicle_statistics.xml`: Lacetti (`lac`) brake speed->distance tables.
- `ai/ai_driver_config.xml` (`default`), `ai/ai_vehicle_config.xml` (`default` plus class
  overrides), `ai/ai_driver_parameter_mappings.xml` (`default`) with the bilinear
  difficulty × characteristic evaluation. The C-style literal `3.0f` is accepted.

Asset test `shipped_ai_data_reproduces_live_normal_values` checks the evaluated live
Normal values: throttle_confidence 0.9606, corner_speed_in_ratio 0.6454,
corner_brake_distance_factor 1.1182, steering_speed 15, steering gain 2.7, Redgate
touring 28.8/46 m/s, 132.48 m from 70.28 m/s at grip 2.0.

## Per-tick pipeline (ported)

1. Track view: projection on the racing line (windowed search), signed lateral error.
2. Race offsets: alone (no car within 70 m, off at 60 m) -> ±deviation_small every 5 s,
   merge 0.2 m/s; in traffic ±deviation_large, merge 1.0 (0.5 above crMaxTrackCurvature);
   below 3 m/s the offset follows the car. Clamped to the racing limits.
3. Look-ahead 0x779A20 (exact formula, rate limit, low-speed blend); curvature 0x798E90 =
   ±acos(lateral(car) · lateral(look-ahead)) / look-ahead, positive when the look-ahead point
   lies on the positive-offset side of the car's lateral (F454).
4. Lateral PID 0x7892E0: target shifted by PID(error from the desired offset) with the
   shipped gains (P -1, I -0.5, D -0.3) and limits (2/1/3, output 15).
5. Corner speed 0x792C80 for the next brake line: min + (max − min)·f·S, S =
   corner_speed_in_ratio (A=1, B=0), f = remap(off-line 0..4 m -> 1..0.7).
6. Brake-line braking 0x7886C0: b from the speed->distance table, reaction
   v/brake_on_speed·(1 − previous brake), d = r + b or 2b, × distance_modifier, trigger
   × corner_brake_distance_factor, brake = ((dist − b)/(d − b))^24, cancel below 0.8 vt,
   passed-line hold.
7. Brake 0x78E8A0: line brake, follow brake, spin (> π/2) -> 1.
8. Throttle 0x783180: previous brake -> 0; otherwise min of governor 0x77A4C0 (target
   vt − 1 while a line is active, held vt for hold_time after passing; 1/0.4/hold/0.1/0
   bands; 4/s approach), follow 0x77A630, slide lift 0x77A880, corner lift 0x77A940,
   throttle_confidence.
9. Steering 0x77AC50: (2.7·heading error + stAngVelFactor·yaw rate) / steering lock
   (vehicle +E54, 0.576 rad for the Lacetti), clamped ±1. Signs: the port works in the
   vehicle's +E30 convention (positive = left); the original computes the negated command
   and its controller publisher negates it into +E30, so both terms match [V] (see
   "Steering sign convention (F391)").
10. Output slew 0x779F70 (steering 15/s, pedals by mapped on/off rates).

## Authored or not yet recovered

- Corner-speed scale A/B (A=1, B=0 used); lateral PID internals, prediction and corner
  factor. (The yaw term sign is settled [V] in F391, below.)
- Followed-car choice (nearest car ahead within ±2.2 m of our heading) and follow-brake
  amount (closing speed / thFollowSpeedMedium); the recovered follow throttle uses the
  shipped thFollow* bands.
- ~~Stuck recovery (slow 2 s under throttle -> brake/reverse with opposite lock 1.5 s).~~
  Replaced in F301 by the recovered avoid, recovery, recover_tbone and retire-stuck paths
  (see "Avoid and recovery").
- Random offsets use an authored LCG; characteristic values are 1.0 for every car
  (the live player-slot set); difficulty is the live Normal 0.606; table grip is 2.0.
- Not ported: alongside, overtake, block, avoidance (traction control and the AI ABS cap: F444),
  start-light reaction, choreographed mistakes, catch-up, drafting.

## Traction control and ABS cap (F444)

19 October 2026, Claude Code. Static, read-only (Ghidra 12.1.4 headless, worktree
fervent-napier-50814b `local/robust/decomp/h1`, `h2`, `h5`, `h6`; Capstone 5.0.6 for the car-state
vtable 0xCAFC54 and the stack layout of 0x402D20). Found by the live first-corner comparison
(docs/FIRST_CORNER_LIVE.md).

- [D] Car-state wheel inputs. The AI car-state update 0x402D20 stores, per wheel w
  (vehicle +0x1990 + i·0x150):
  - the rolling speed w+0xE0 × w+0x104 (0x990FB0 × 0x990DB0) in car state +0x240, via vfunc
    +0x2C; vfunc +0x10 returns it;
  - the driven flag (0x990EC0: drive share w+0x10C ≠ 0.0, NaN counts) in +0x250, via +0x30;
    vfunc +0x14 returns it. Vfunc +0x1C returns 4.
  - The w+0x7C bytes (0x9910A0) go to +0x254 (vfunc +0x34). They are not the driven flags,
    and only the reset 0x996BE0 writes w+0x7C.
- [D] Traction control 0x77AA10 is one more term of the throttle minimum in 0x783180, after the
  +0x7C cap.
  - Between tcMinSpeed and tcMaxSpeed (car state +0xD0), with f = the fastest driven wheel's
    rolling speed and f > speed:
    - level = lerp(tcMinLevel, tcMaxLevel, (v − min)/(max − min));
    - limit = 1 + (level − 1) · (f/v − 1)/(tcMaxWheelSpin − 1), clamped between level and 1.
  - Defaults (vehicle config ctor 0x781AB0) 1, 20, 0, 0.5, 2.
- [D] ABS cap 0x77A2E0. 0x78E8A0 applies it to the maximum of every brake source:
  to-desired, follow, forced, brake line, spin and avoidance 0x788A90 (not ported).
  - Then the full-brake flag +0xB2, which returns 1 before the cap, then the req+0xA0 cap.
  - Above absMinSpeed, with s = the slowest wheel's rolling speed (all wheels) and s < v:
    cap = 1 + (absMinBrake − 1) · (s/v − absMinWheelLock)/(absMaxWheelLock − absMinWheelLock),
    clamped between absMinBrake and 1.
  - Defaults 5, 0.7, 0.3, 0.2.
- Port: `View::wheel_speeds` and `View::driven` (`Runtime::wheel_rolling_speeds`,
  `driven_wheels`), `traction_control`, `abs_cap`. The port's brake order now follows 0x78E8A0:
  forced and to-desired inside the cap, full brake outside it.
  Unit tests `traction_control_limits_the_throttle_by_driven_wheel_spin` and
  `abs_cap_limits_the_brake_by_the_slowest_wheel`.
- [V] Effect on the launch. With the field held as in the live capture (`GRID_GRID_HOLD=3.7,3.04`):
  - Speed 1 s after the go: 8.4–8.5 → 13.8–14.0 m/s (original 15.4–17.1); Redgate 0.4 s
    earlier (9.30 s, original 8.60).
  - Before the change the front wheels rolled at about 19.5 m/s while the car did 0.2–6 m/s.
  - The throttle now dips to about 0.54 after the go and ramps up, as the original's does.
- [V] The ABS cap changes the first corner little. At Redgate the leader, alone, still takes it
  at 32.6 m/s against the original's 37.0: a base corner speed difference, not traffic. The
  follow brake 0x77A1E0 matches the port 1:1.

## Steering sign convention (F391)

18 October 2026, Claude Code. Static trace (Capstone 5.0.6 and Ghidra 12.1.4 headless,
read-only, on `local/compatibility/grid-unpacked.exe`; evidence `local/yaw-sign/`). Question:
the original 0x77AC50 uses −ω·up (sign mask 0xCAF470 = 0x80000000 in all lanes), the port
+ω·up.

**Inputs are the same values [V].**
- The AI car state (class vtable 0xCAFC90, constructor 0x775EC0) is filled each tick by
  0x402D20 from the car's physics body (vehicle +0xDB0, whose +8 is the body record):
  - +0xC0 = body +0x210 copied unchanged (getter 0xA69820, a plain `movaps`, store at
    0x403280);
  - +0xA0 = body +0x220 (getter 0xA69800);
  - +0x30 / +0x40 / +0x50 = normalised rows 2 / 1 / 0 of the body matrix +0x1D0
    (getter 0xA69630; +0x40 stored at 0x402F7C);
  - +0x118 = 0x9959A0 (steering lock).
- The port's `View::angular` is physical +0x210 and its up is row 1 of physical +0x1D0
  (`source_vehicle::Runtime::angular_velocity`, `pose`). So ω·up is the same number in both.
  The port's ω has no opposite sign convention, contrary to the F389 guess.

**The heading error has the opposite sign [V].** 0x797B50 stores request +0x88. T is the
target (route point +0x60) minus the car-state position +0xF0, and F is car-state forward
+0x30. Both are flattened (mask 0xCAF410 clears y) and normalised. Then:
- a = acos(F·T) (0xBF4067);
- c = F.z·T.x − F.x·T.z;
- +0x88 = a when c < 0, otherwise −0.0 − a (0xCAFE50 = −0.0).

A target on the row-0 side (+X, the car's left) gives c > 0, so the original's heading
error is negative. The port's `heading_error = atan2(T·row0, T·row2)` is positive there.
Hence h_orig = −h_port.

**The publisher negates the command [V].** The controller loop 0x828670 handles the
controller types `"ai"` (0xCF6F30) and `"human"` (0xCE7994). It reads the controller's
packet and writes the vehicle through the setter 0x996240 (`vehicle + 0xE18 + 4·slot`).
That setter multiplies slot 6 = +E30 by −1.0 (0xCB0948) for every caller. Slot 6 is
packet +0x10, the steer word clamped by 0x7FC030. The same loop also consumes packet
+0x1E: a value of 1 pushes gear 9 at 0x8289EB. That is the reverse byte from 0x7929D0 and
answers the open "car-side consumer" question in AI_AVOID_RECOVERY_RESEARCH.md as a reverse
request [V for the branch; its gearbox effect not traced]. The human record has the same
layout (+0 throttle, +4 brake, +0x10 steer, +0x1C handbrake → slot 8).

**Result.** The vehicle receives
+E30 = −(|slip|·stSlideFactor + f·(−ω·up) + G·h_orig) = G·h_port + f·ω·up − |slip|·stSlideFactor.
The port's law is G·h_port + f·ω·up written straight to +E30. Several things commute with
the final negation: the slew (equal 15/s rates both ways), the ±1 clamp and the reverse
negation. So the yaw and heading terms are the original's, signs included, and the yaw term
damps (f = stAngVelFactor < 0; ω·up > 0 is a left yaw in this frame). F389's private flip
made the yaw term anti-damping, which explains why it made the fields worse. Normal races
need no change.

Still differs (not sign questions):
- The slide term −|slip|·stSlideFactor is omitted. It is inert at the recorded value 0
  (AI_DRIVER_RESEARCH.md §5).
- The car-state +0xE2 == 0 branch (velocity-direction blend by stProportionRabbitWhenInAir)
  is not ported. 0x402D20 counts the wheels whose contact word wheel +0xC0 is zero
  (0x990FC0) and sets +0xE2 = (count < 4). So the branch runs only while all four wheels
  are off the ground [V]. It does not affect ground driving or drift.

## Brake lines a class does not brake for (F425)

18 October 2026, Claude Code. Found while making Milan route 0 raceable (docs/STREET_CIRCUITS.md).
Static, read-only. Pseudocode is in the worktree fervent-napier-50814b
`local/robust/decomp/e1`, `e2`; the earlier `local/ai-research/decomp/drv3/7923f0.c.txt`,
`792c80.c.txt` and `drv6/7886c0.c.txt` were reread.

- [V] The shipped `ai_vehicle_track.xml` gives some class/line pairs `min_speed = max_speed = 0`.
  - Milan route 0: touring lines 1 and 3, formula3 also line 7, and others.
  - Every Milan route has some.
  - None for touring on the other circuits the port races.
- [D] The brake-data class (vtable 0xCF7244: loader 0x775100) has a validity method at slot +0xC,
  0x7A3160: `max_speed > 0 && min_speed > 0 && distance_modifier > 0`.
- [D] The track view's next-line search 0x7923F0 walks the links ahead. For each link it asks
  0x78E820, which returns the link's line only when the class's data passes 0x7A3160, so an
  invalid line is passed over and the next valid one is the car's next brake line.
- [D] 0x7886C0 also calls the same check before it touches the line's state, and skips braking
  when it fails.
- Port: `next_brake_line` skips lines whose data is invalid (`brake_data_valid`), and the
  line-braking block requires it as well.
  - Before: the touring AI braked toward 0 m/s at Milan's lines 1 and 3. Solo it rolled past them
    (121.7 / 118.7 s laps). In traffic a car that slowed early stopped short of the line, the
    assisted gearbox selected reverse, and the authored reverse-gear pedal mapping dropped the
    brake that the throttle logic still saw. Every 12-car race deadlocked at the first chicane.
  - After: solo 100.8 / 98.8 s (101.9 / 99.9 s on top of F417's engine braking); the 12-car race
    goes past the chicane (docs/STREET_CIRCUITS.md section 2).
- The solo identity and the 12-car Donington races are byte-identical, because no touring line on
  the existing circuits has invalid data [V]. On main 57b406d (F417): solo 1ee81525…7267 and
  `12 11200 ai` 012d4c0d…68f3, as F417 recorded.

## Results (headless, Donington, Lacetti, Normal)

- One car: 2 valid laps, 88.42 s (standing) and 86.28 s, no contacts, bit-identical
  repeat; minimum corner speeds 1–2 m/s under the brake-line targets
  (e.g. 28.6 vs 30.7, 22.3 vs 24.3 m/s). Before the PID and the correct steering
  lock, the car drifted 3.5–4 m off line in the fast curve and hit the wall.
- Four cars: all 2 valid laps, flying laps 87.15–87.80 s, 0 car-contact records.
- Eight cars: all 2 valid laps, flying laps 87.15–88.23 s, 2,240 car-contact records
  (start), ~5 ms per shared tick.
- The owner's manual time trial laps were 90.6/82.2/80.0 s. The original AI's Normal
  lap time has not been captured yet; a live read-only trace of one AI car through a
  braking zone remains the next validation step.

## Opponent body dents

With `--deformation`, each opponent's body parts own their mesh copies and are re-projected
from that car's own deformation field when it changes. Before this fix the opponents reused
the player's mesh handles and would have shown the player's dents. Car-to-car impacts still
produce no dents (several-car contact groups solve without deformation phases).

## Traffic behaviours (F294)

Ported from [AI_OVERTAKING_RESEARCH.md](AI_OVERTAKING_RESEARCH.md). Each tick:

- **Tests:**
  - brake_for_vehicle: pass vehicle (clear sideways by brakeBufferZone 1 m and same heading),
    or a stopping check with the speed-scaled separation, the braking-distance table and
    prediction ratio 3 → follow distance 100 or sep2;
  - attempt_overtake: nearest car ahead within 60 m and 12 m sideways; auto-accept 3 m,
    closing 0.1 m/s with time under 3 s, or a slow car; target kept while closing > −2 m/s;
  - alongside: overlap within 10 m with on/off margins 1.0/1.2 front and 1.0/2.0 rear;
    nearest gap per side.
- **Arbitration:** alongside 8 > overtake 5 (3 s minimum fire) > race 1.
- **Request:** desired offset, merge rate, desired speed, throttle cap and follow fields.
  - The race behaviour keeps the random offsets and the throttle_confidence cap.
  - Overtake and alongside run with cap 1.0.
- **Overtake drive:** side choice every 0.25 s or on a new target, relative to our planned
  line and the room to the racing limits. States: 1/2 push the planned offset at 4 m/s
  (0.5 m/s toward the outside of a bend above 12 m/s), 3/5 hold, 4 follow the target at
  3 m and its speed.
- **Alongside drive:** merge 4 m/s; keep 2.5 m from a car on one side and return to the
  line once the gap is beyond 5 m.
- **Follow:** follow throttle (0x77A630) and follow brake (0x77A1E0) use the request's follow
  car and distance. The follow brake is the cap min(+0x78 = 1.0, brFollowSpeedBrake), i.e.
  full braking. This replaces the earlier authored follow-car choice and closing-speed brake.

AUTHORED or omitted:
- the path-obstacle scan is a straight corridor (own heading, combined widths
  + 1.5 m, 50° view cone from the rear point);
- the far-side squeeze is omitted;
- cars on both sides hold the planned offset;
- avoid, block and draft are not ported (block and draft are disabled by default anyway);
- the bend-direction sign is [I];
- car geometry used Lacetti half width 0.9 m and half length 2.24 m; since F345 each car has its own extents, taken from its hull, body offset and tyres as the original does (docs/CAR_GENERALIZATION.md, "AI car extents").

Checks (headless `grid-opponent-race`; `GRID_SLOW_CAR0=f` scales car 0's throttle,
`GRID_TRACE_BEHAVIOUR=1` prints behaviour changes and contact pairs):
- solo laps unchanged, bit-identical (88.42 / 86.28 s);
- slowed leader (`GRID_SLOW_CAR0=0.6`, 2 cars): passed side by side (alongside ticks
  81–177) with **0 car contacts**, then lapped the same way;
- 4 equal cars, 2 laps:
  - positions change (car 1 wins 2:57.808, gaps 1.39/2.74/4.38 s);
  - 192 car-contact records in four episodes: three while the pack bunches into the first
    corners (ticks 663–856), one brief touch at tick ~2608;
- 8 cars: all finish 2 laps, 521 records;
- app autopilot race with damage, 1 lap, 3 opponents: You 1:31.018, AI 1 +1.238,
  AI 3 +2.933, AI 2 +4.161 (est.).

Alongside yield (0x789780 / 0x77B7D0) ported afterwards:
- it requires own speed above 16 m/s and a car beside within 2.18 m, inside the speed window
  (−13.9 … 6.97 m/s), whose respect point (rear + 1.05 × length) is ahead of our front;
- a single yielding side applies only when the other side is also tight or there is under 1 m of
  room toward the opposite limit;
- the car then follows that car at 0.79 m with the follow brake capped at 0.597.

Results: solo unchanged; slowed leader 0 contacts; 8 cars 193 contact records (521 before the
yield); 4 cars 207.

## Per-driver characteristics (F297)

Read-only capture from the running original (PID 7208, the existing bounded VM_READ dump tool):

- Each AI driver object holds, at `+0x30F4`, a vector of 8 pointers to 0x30-byte characteristic
  records. The value is at `+8`, and `+0xC` points to a shared type descriptor whose name is at
  `+0x14`.
- The names, in order: Aggression, Control, Mistakes, Reaction, Variance, Speed_In, Speed_Out,
  Virtual_Performance.
- Driver 0 (the player slot) is all 1.0. The other 11 drivers vary between 0 and 1 (e.g.
  driver 01: 0.7, 0.8, 0.8, 0.9, 0.5, 0.9, 0.8, 0.8).
- The table is kept **private** in `local/ai-driver-characteristics.tsv` (game database values).
  `grid-app --ai-characteristics PATH` (passed by the launcher when the file exists) and
  `GRID_AI_CHARACTERISTICS` for `grid-opponent-race` feed them into the bilinear mappings.
  Opponent k uses driver row k.

Effect (headless, 2 laps):
- 4 cars: flying laps 86.98 / 88.08 / 88.60 / 88.47 s; classification gaps +1.776 / +3.520 /
  +4.384 s; 102 contact records.
- 8 cars: gaps to +8.000 s; 243 contact records.

Without the table every car uses 1.0 as before.

## Race start (F298)

- `max_lights_reaction_time` is 0.0 at every difficulty and characteristic in the shipped mappings,
  so the original AI launches at GO without delay; the port already did.
- Grid revving 0x775BF0 while held:
  - throttle 1.0 in random on phases and 0 in off phases;
  - start state 3: on 0.20–0.48 s, off 0.01–0.70 s; state 2: on 0.08–0.18 s, off 0.60–1.15 s
    (.rdata constants read from the analysis image); other states give 0.
- AUTHORED/[I]: state 2 in the first half of the countdown and state 3 in the second; authored
  random stream.
- Opponents rev on the handbrake and stay on their slots (look-back render at tick 240).
- Unit test: pulse lengths stay within the shipped ranges.

## Avoid and recovery (F301)

Port of docs/AI_AVOID_RECOVERY_RESEARCH.md §7. The authored stuck recovery is deleted.

### Inputs

- `View::of(runtime, obstruction)` adds:
  - the four wheel positions in world space (on_track);
  - the gearbox reverse state (vehicle +196C = 9);
  - the obstruction sum from `MultiRuntime::obstruction(i)`.
- The obstruction sum (collided_obstruction feed, 0x4022F0/0x77B1F0) holds the last tick's car
  body contacts with static geometry, plus car-to-car contacts with a car slower than 2 m/s.
  - Each contact adds its normal, oriented to push the car (away from the contact point).
  - Floor-like normals (y ≥ 0.1) are skipped.
  - Unit weights stand in for the callback magnitude [I]; the test reads only direction and
    non-zero length.
  - With no shared world (autopilot without opponents) the feed is zero.
- `Line` keeps the ai_track **track** limits (types 3/4) beside the racing limits.

### Tests, in the original's order before arbitration

| Test | Port |
|---|---|
| spun 0x77E5B0 | \|heading error to the planned look-ahead point\| > π/2. Held while slip > 0.1π (π − slip when moving backwards), heading error > π/3, or speed < 3 m/s. |
| on_track 0x79B250 | More than half of the wheels inside the track limits at their projected arc. |
| collided_obstruction 0x776FE0 | Sum non-zero. Frontal when the angle to our nose exceeds 120°. |
| contacted_vehicle 0x784650 | Box overlap with contactZone 0.1. Types 1–4 for our corner inside the other box; 5–8 for its corner inside ours, by nearest face. The two-corner length-ratio refinement is not ported. |
| retire stuck 0x77E3B0 | 20 s within 5 m and no non-reversing car within 100 m. |
| path obstacle 0x78B560 | AUTHORED corridor, as brake_for_vehicle. Keeps the closing car ahead with the smallest distance / closing speed. Our box is extended by clamp(v/10)·2 m. |
| avoid 0x79A960 | Branch A: racing-normally exclusion (speed ≥ 4, not reversing, heading within 90°, not within 45° of the *reversed* line direction, as the original). Inert-branch handling is unchanged. Fires on ETA < 2 s. Branch B: the remembered target, ahead beyond the half lengths, inside the 50° cone, `along < 2·closing`; otherwise forgotten. |

The racing-normally state 2 ("not racing") is mapped to a car moving against its nose [I].
Live, racing AI records are in state 1 and the stationary player car is in state 0; state 2
was not observed.

Live check (F302, [AI_AVOID_RECOVERY_LIVE.md](AI_AVOID_RECOVERY_LIVE.md)):
- The original's exclusion holds on 99.6% of race-traffic frames with ETA < 2 s.
- 0.36% of those frames leak. Branch B then extends a leak into avoid episodes of up to 3.6 s,
  so traffic avoid takes about 0.4% of AI frames.
- F306 cause: the original's f32 cosine, with rsqrtps-estimated lengths, drops below −1 when
  the target heads within about 5·10⁻⁴ rad of the from-gate direction. acos then returns NaN,
  and NaN fails the `ja` compare, so the car counts as "not racing".
- The port keeps the documented rule; it clamps the cosine and uses the line direction. A
  proposal to reproduce the leak (not implemented) is in
  [AI_AVOID_RECOVERY_LIVE.md](AI_AVOID_RECOVERY_LIVE.md#port-proposal-not-implemented).
- Stopped-car avoid matches: target speed 30, clearance 6.05–6.08 m.

### Arbitration

- Scores: retire 23, recover_tbone 18, recovery 11/16/17/19 (n = spun, off track,
  obstruction while off track, frontal), avoid 15, alongside 8, overtake 5, race 1.
- The first strictly greatest score wins.
- Activation semantics:
  - avoid clears its object but keeps the test's remembered record;
  - recovery resets mode, latch and timers, but not the reverse toggle;
  - recover_tbone sets its timer to −1 after scoring, so it needs the contact on two ticks.
- Common setup now uses tgOffsetMergeFactor (2.5) as the default merge, as 0x7978D0 does.
  Before, it used raOffsetMergeFactor. This affects only overtake states 3–5.

### Drives

- **avoid**: decides every 0.25 s or on a new target.
  - Clearance = the target's half length + 2 m + both half widths (Lacetti pair 6.066 m since F345; 6.04 m with the old constants).
  - Mode 1 when closing > own speed.
  - Otherwise the side needing the smaller move from our front's lateral position.
    - The room check uses the track limit at the predicted position, with w = 3.2 m.
    - With no room it flips side (avAvoidAlongside, alongside not fired), or falls back to the
      alongside gaps, or follows at 3 m (mode 0).
  - The offset is never moved toward the target.
  - Desired speed with brake-to-speed is set on decision ticks only: 30 (target stopped), 200 or
    5 (wrong way).
  - Track-limit clamp, merge 5 m/s.
- **recovery**:
  - Offset: off track it drifts 1 m/s toward our side at merge 5; on track it goes to our own
    lateral position at 2.5.
  - Octant from the side-mirrored angles `b` (heading error) and `a` (track direction at the
    planned point), with clockwise-positive angles. The sign convention is verified live
    against the original's recovery mode choices (F302,
    [AI_AVOID_RECOVERY_LIVE.md](AI_AVOID_RECOVERY_LIVE.md)); the unit test checks that positive
    lateral offsets point right.
  - Mode 2 forward for octants 0–3.
  - Otherwise full brake until under 0.1 m/s, then latch:
    - 3–5: reverse at 5 m/s until |a| or |b| < 5°;
    - 6–8: reverse until 80° < |a| < 90° or |b| < 5°;
    - then forward.
  - An obstruction contact toggles reverse/forward for 2 s.
  - Throttle cap is 1.0 (no throttle_confidence).
- **recover_tbone**: reverse with no steering while the 1 s timer runs. It is armed by a front
  contact (types 1, 2, 5) at a T-bone (70–110°, relative < 5 m/s), a slow nose-to-tail (< 30°,
  both slow) or head-on (> 160°).
- **retire (stuck)**: force brake and no steering.
- **Request flags**:
  - full/force brake → 1;
  - brake-to-speed → 1 when speed exceeds the desired speed by > 2 m/s;
  - the slip > π/2 brake is skipped while reversing;
  - steering is 0 with no-steer and negated while reversing;
  - the throttle is negated, then published as |throttle| with a reverse flag (0x7929D0).

### AUTHORED reverse boundary

The car-side consumer of the original reverse byte is not traced. The port drives the assisted
automatic gearbox through the pedals:

| Gear | Request | Throttle slot | Brake slot |
|---|---|---|---|
| forward | forward | throttle | brake |
| forward | reverse | 0 | max(throttle, brake), to select reverse near rest |
| reverse | reverse | brake | throttle |
| reverse | forward, rolling back | max(throttle, brake) | 0 |
| reverse | forward, at rest | throttle | 0 |

Observed: the gearbox selects reverse (id 9) about 10 ticks after the stop, and drops out of
it on its own when both pedals are released.

### Results (headless)

| Run | Result |
|---|---|
| Solo | 88.420 / 86.282 s, bit-identical to before; no new behaviour fired. |
| Slowed leader (2 cars, ×0.6) | 0 contacts. One 0.25 s off-track recovery (n = 1, forward octant). |
| Car 0 brakes to a stop at tick 900 or 420 (4 or 3 cars) | Followers pass through overtake: avoid only fires below 4 m/s, and they are already offset more than 4.3 m. No contact after the stop. |
| Car 0 stalled on pole (4 cars) | Car 2, starting 8 m directly behind, fires avoid (ETA 1.92 s, side 1) but side-swipes car 0 at about 8 m/s (15 records). It cannot gain 6 m of clearance in 8 m. |
| Forced spin (car 0, full lock and handbrake 2.5 s) | spun → octant 4 → full brake to 0.06 m/s → octant 6 → reverse in gear 9 at ≤ 7.7 m/s; heading error 2.33 → 0.15 rad; forward; back on track and racing 6 s later. |
| Off-track wall scrape (car 3, stalled-pole run) | Non-frontal obstruction → reverse 2 s → forward → rejoined at tick 1200. |
| 12 cars, 2 laps, characteristics | All finish within +10.2 s; 579 car-contact records, against 1124 and two cars losing 23.8/47.2 s with the authored recovery (F300). 32 recovery events, almost all n = 1 off-track moments at speed; one wall scrape at 55 m/s reversed for 2 s (faithful toggle). 98.6 s wall for 11,765 ticks. |

Open:
- avoid never fired in race traffic (overtake/brake_for_vehicle act earlier); a live trace of
  the original is needed to compare;
- the octant sign convention;
- the obstruction body classes and magnitude;
- ~~lost_control and controlled_stop are not ported.~~ Ported with the race manager; see "Mistakes, failures and race pacing" below.

## Mistakes, failures and race pacing (F336)

16 October 2026, Claude Code. Port of the plan in
[AI_MISTAKES_PACING_RESEARCH.md](AI_MISTAKES_PACING_RESEARCH.md) §7. The choreographer and
catch-up details also use the owner's untracked research copy
`docs/AI_CHOREOGRAPHER_CATCHUP_RESEARCH.md` (main checkout, not committed). Where the two
documents differ, this port follows the second one for:
- the conveyor start draws (front third → drop back, back third → push forward);
- the mistake event states 1/2/3/4;
- the one-event-at-a-time runner;
- the brake_lock wheel order.

Not independently re-verified here: no new disassembly was read. Both documents agree on every
constant used below. The shipped values were re-read from the purchased XML by the asset test
`shipped_pacing_data`.

Code:
- `grid_formats::ai`:
  - `catch_up(doc, user)`: catchup.xml with the reset defaults;
  - `choreographer(doc, discipline)`: set selection with the `default` fallback, manager,
    conveyor, events and priority-sorted triggers with the built-in defaults, including
    `min_distance_ahead` 1, `max_distance_ahead` 100 and `max_events` 1;
  - quirks kept: `rev_length` is 10 and `min_sleep_distance` is ignored.
- `grid_tools::ai_pacing` (new): the race manager `Field`, `Settings`, MT19937, and the pure
  `catch_up_power`, `catch_up_max_distance`, `grip_factor` and `torque_factor`.
- `grid_tools::ai_driver`: `Pace` inputs, the mistake slot, the failure behaviours and
  let-pass, in a separate `impl` block.
- `Runtime::set_ai_pacing(torque, grip)` writes engine +0xE8 (vehicle +1888) and wheel +0x80
  for AI cars. The ported vehicle already reads both fields; they start at 1.0.

### Race manager, per tick before the AI update

The manager needs a reference car (the human). Without one nothing is computed or written.

1. Race distances along the AI racing line from the start-finish gate (AUTHORED; the original
   uses the progress track). Places are 0-based by distance.
2. Choreographer, while the reference races:
   - reset: power and grip factor 1, B 0, backwards off; A persists;
   - conveyor (fields of 6 or more): one front-third car sleeps U(0, 1000) m, then has power
     and A at 0.7 and the backwards flag until it is in the last two places; one back-third
     car has power and A at 1.1 until it reaches place 2–3; then they swap roles;
   - events (with `mistakes`);
   - lapping yield (backwards flag when a car is within 50 m of lapping it);
   - race start: power 1.05 and B 1 for cars ahead of the player in the first 100 m; grip 1.25
     below 750 m; after that the off-line assist `1 + clamp(t·m, 0, m)`, t = |lat|/4 m,
     m = 0.25 inside or 0.5 outside. Which side the original calls inside is [I].
3. Catch-up (the Race Day config of the difficulty, `rd_2_star` at Normal, F439):
   - ahead of the reference by more than 50 m: power 1.0 → 0.5 at 200 m;
   - useLead stretches the maximum to the leader's gap;
   - behind by more than 50 m: power 1.0 → 1.2 at 400 m;
   - never below 12 m/s or within the first 500 m.
4. Application:
   - engine factor `clamp(catch-up × power, 0.1, 2.0)`;
   - wheel grip = the driver's request grip of its last update × grip factor;
   - the player car is never written.

### Choreographer events

Shipped `circuit` set:
- `mechanical_failures`: blow_tyre_fast, blow_engine_slight/total; from 5000 m and 25 % of
  the race; 20–150 m ahead;
- `major_errors`: powerout_spin, brake_spin; windows of 500 m, 20 s apart; 5–250 m ahead;
  position filters for places 0 and 1;
- `minor_errors`: brake_lock; windows of 200 m, 5 s apart; 0–200 m ahead.

Mechanics:
- Candidates are the AI cars with weight `max = 1 − mistakes`, starting at 0.5·max:
  - an arming draws each candidate `U < cur·chance` and grows `cur` by 0.1·max;
  - success halves `cur`.
- With the characteristic default 1.0 (no private table) no car is ever chosen, as in the
  original.
- Only one event runs in the field at a time:
  - a different selection aborts an event still waiting in state 1;
  - an executing event (state 3) blocks;
  - the timeout is 30 s.
- Events:
  - mistake: arms the driver's slot with a 60 s timeout;
  - blow_tyre: fails a random wheel;
  - blow_engine: raises the engine damage, and 2 s later sets the damage gate.

### Driver side

- Corner speed `S = max(A·corner_speed_in_ratio, B)`.
- Mistake slot (0x787620/0x7799B0):
  - the armed type activates at its own condition:
    - late/early: trigger distance × factor, then on reaching it;
    - heavy/light/brake_lock: once braking for a line;
    - brake_spin: braking for a line with 8–12 m/s excess;
    - powerout_spin: in the post-line hold, previous throttle > 0.6 and above 10 m/s;
  - effects for 3 s (spins) or 5 s:
    - brake_spin: rear grip 0.2;
    - powerout_spin: rear grip 0.4 and full throttle;
    - brake_lock: one wheel 0.2, cleared by the next brake line;
    - heavy/light: brake floor/cap;
  - any active mistake ends at or below 10 m/s;
  - the random generator is ported but off (`generateMistakes` false in the data).
- lost_control (21): a failed wheel above rtRetirementSpeed + 5 (20 m/s).
  - Above crashMinSpeed (50): crash mode, full brake, rear grip 0.1.
  - Otherwise a wobble at 15 m/s with brake cap 0.15 and rear grip 0.3.
- Retire failure branch (10): failures, or gated damage above damage_retirement_ratio.
  - It drives to the nearer track limit at 15 m/s, stops there and requests removal; the
    race then retires the car (`Out::Wrecked`).
- race_damaged (9): gated damage ≥ 0.5; 50 m/s, throttle cap 0.6, near a track limit.
- controlled_stop (20): force brake and hold the offset on an external request. Its original
  triggers (view invalid, AI state names) are not resolved; nothing requests it yet.
- Let-pass: while the backwards flag is set, throttle cap × 0.8 with a car within 30 m
  behind.

AUTHORED where not recovered:
- crash PID, spin/flip angles and the inertia factor +0x21C0 (normal steering, no flip);
- the wobble amplitude (deviation_from_racing_line_large);
- the retire point (the nearer track limit);
- a failed wheel keeps grip 0.1 (stands in for the vehicle's failed-wheel parameter, which
  the port does not model);
- engine and radiator damage only drive the AI gate; they do not change the engine. The
  mechanical-damage work owns that;
- the spun-and-fast lost_control branch is not ported: the built-in coSpinSpeed is not
  recovered;
- visibility_loss is not ported: no smoke value exists in the simulation;
- block and draft stay off as in the data;
- the brake_lock wheel and wobble draws use the driver's authored LCG; the choreographer
  uses its own seeded MT19937.

### Settings

An `[ai]` section in a settings file ([SETTINGS.md](SETTINGS.md); `settings/race.ini` lists
the defaults), or grid-app `--ai difficulty=0.606,catchup=on|touge|off|rd_N_star,choreographer=on|off,mistakes=on|off`.
The difficulty feeds the AI parameter mappings. The log line is `drive_ai_pacing`; events
are logged as `drive_ai_event`.

grid-soak uses the default settings, like the app. Replays of app logs made with other
`--ai` settings are not exact.

Headless (`grid-opponent-race`):
- `GRID_REFERENCE_CAR=k`: car k stands in for the player; it is AI-driven and never paced;
- `GRID_AI_SETTINGS=SPEC`;
- `GRID_RACE_LAPS=n`;
- `GRID_PACING_TRACE=local/X.tsv`;
- `GRID_BLOW_TYRE=car:tick[:wheel]`.

### Validation

Evidence is in the main checkout's `local/ai-pacing/runs/`.

| Check | Result |
|---|---|
| Solo `1 11200 ai` | 88.42029812185638 / 86.28228501935392 s; output byte-identical to F324 `solo-after.txt` |
| 12 cars, deformation, `12 12000 ai`, no reference | byte-identical to F324 `race12-after.txt`, including all 12 state hashes |
| 3 laps, 12 cars, characteristics: no reference vs reference car 11 with `catchup=off,choreographer=off` | state hashes and classification identical |
| Same with the defaults, reference car 11 at the back (scenario 7.6.4) | see RACE_MODE.md "AI pacing and mistakes": field held together; mistakes fire on cars 0–250 m ahead; spins go through recovery |
| Forced blow_tyre, car 0, 59.5 m/s (tick 1995) | lost_control crash mode (rear grip 0.1, full brake), spin, recovery, RetireFailure; spun again on the failed wheel, recovered; stopped and retired at tick 2349 (`Wrecked`) |
| Forced blow_tyre, car 0, 35.5 m/s (tick 610) | lost_control wobble mode, spin, recovery, RetireFailure, retired at tick 868. Both modes spun the car: with the failed rear wheel at 0.1 and the other rear at 0.3 the wobble is not a controlled slow-down [I compare live] |
| Unit tests | catch-up tables (rd_3_star 125 m → 0.75, 200 m → 0.5; behind 225 m → 1.1, 400 m → 1.2; useLead; 5–6 star reverse only; clamp 0.1–2.0), off-line assist 2 m inside/outside at κ 0.01 → 1.125/1.25 and race-start 1.25, MT19937 reference stream, settings; asset tests `mistake_slot_windows_and_effects` (activation only for the armed type, 3/5 s, 60 s arm timeout, brake_lock wheel spread and new-line clearing, grips 0.2/0.4/0.2, failed wheel 0.1, A/B, failure scores and the damage gate), `random_mistake_pick_follows_the_live_weights`, `shipped_pacing_data` |
| `grid-soak quick` | the 6 Donington scenarios pass; the 6 Nürburgring ones fail at set-up with the known "scaled prop piece placement" (F326 finding 1) |

Open:
- A live trace of the original (§7.6.6) is needed to confirm the conveyor, race-start factors,
  catch-up powers and choreographed mistakes, and the catch-up config name.
- About half of the armed mistakes are aborted before they activate (another trigger's
  selection replaces them). This follows the documented runner rule, but the original's
  behaviour in that case is not observed.
- Contact records in a paced 12-car race are about 4.7 times the unpaced count.
