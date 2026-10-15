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
3. Look-ahead 0x779A20 (exact formula, rate limit, low-speed blend); curvature = heading
   change line(car) -> line(look-ahead) / look-ahead.
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
   (vehicle +E54, 0.576 rad for the Lacetti), clamped ±1.
10. Output slew 0x779F70 (steering 15/s, pedals by mapped on/off rates).

## Authored or not yet recovered

- Corner-speed scale A/B (A=1, B=0 used); lateral PID internals, prediction and corner
  factor; the yaw term sign (damping).
- Followed-car choice (nearest car ahead within ±2.2 m of our heading) and follow-brake
  amount (closing speed / thFollowSpeedMedium); the recovered follow throttle uses the
  shipped thFollow* bands.
- ~~Stuck recovery (slow 2 s under throttle -> brake/reverse with opposite lock 1.5 s).~~
  Replaced in F301 by the recovered avoid, recovery, recover_tbone and retire-stuck paths
  (see "Avoid and recovery").
- Random offsets use an authored LCG; characteristic values are 1.0 for every car
  (the live player-slot set); difficulty is the live Normal 0.606; table grip is 2.0.
- Not ported: alongside, overtake, block, avoidance, traction control and AI ABS cap,
  start-light reaction, choreographed mistakes, catch-up, drafting.

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
- car geometry uses Lacetti half width 0.9 m and half length 2.24 m.

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
- 0.36% of those frames leak, for an unexplained reason. Branch B then extends a leak into
  avoid episodes of up to 3.6 s, so traffic avoid takes about 0.4% of AI frames.
- The port keeps the documented rule.
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
  - Clearance = half length + 2 m + both half widths (6.04 m).
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
- lost_control and controlled_stop are not ported.
