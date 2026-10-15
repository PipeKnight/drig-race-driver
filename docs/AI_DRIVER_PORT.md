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
- Stuck recovery (slow 2 s under throttle -> brake/reverse with opposite lock 1.5 s).
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
