# AI choreographer and catch-up: original against the port (F439)

19 October 2026, Claude Code. Static analysis only: Ghidra 12.1.4 headless `-readOnly` on the
private analysis copy and Capstone 5.0.6 scans of the research copy; shipped data read with the
port's readers. No game launch.

This document compares how the original paces the AI field relative to the player with what the
port does today. The detailed research is in
[AI_MISTAKES_PACING_RESEARCH.md](AI_MISTAKES_PACING_RESEARCH.md) (§2.6, §4) and in the owner's
untracked research copy `docs/AI_CHOREOGRAPHER_CATCHUP_RESEARCH.md` (main checkout, not edited
here); the port is `crates/grid-tools/src/ai_pacing.rs`
([AI_DRIVER_PORT.md](AI_DRIVER_PORT.md), "Mistakes, failures and race pacing").

Labels: **[V]** verified in disassembly/decompilation; **[D]** shipped data; **[L]** live
observation; **[I]** inference.

## 1. Which catch-up config a race uses (new) [V/D/L]

The race manager selects one `catchup.xml` config by `user` name (0x5D5530). The name comes
from the game mode and the event descriptor [V, decompile `cu1/5D5530`; format strings at
0xCE205C/0xCE2054/0xCE2048/0xCE2040]:

| Game mode | Name |
|---|---|
| 0 (GRID World) | `gw_%d_star`, + `_h2h` for head-to-head |
| 5 (Race Day) | `rd_%d_star` |
| 1 | `network` (or `none`) |
| 6 | `first_race` |
| other | `none` |

`%d` is **event desc+0x68 + 1**, and `_touge` is appended when the competition format
(desc+0x4C) is 5 or 6 (pro_touge, midnight_touge; DISCIPLINES.md §1.1).

**desc+0x68 is the star level.**

- The same descriptor block desc+0x68..+0x88 is the event's `difficulty` row. Flashback
  setup (caller 0x58AC70 → 0x6AF7C0) copies it and reads desc+0x84 as `available_flashbacks`
  and desc+0x88 as `flashback_bonus` [V].
- The names run `rd_1_star`…`rd_6_star` [D], one per star level 0..5 (Basic…Ninja), so the
  field is the 0-based star level, not the row id (1..24) or the rank (1..4) [V structure].
- Race Day Normal is star level 1. The live Race Options screen (Difficulty NORMAL) showed
  "FLASHBACKS AVAILABLE: 4", the level-1 count [L, 18 October screenshots], and Normal's
  `ai_ratio` 0.606 is the level-1 row (FLASHBACK.md §2.1).

So **a Normal Race Day race uses `rd_2_star`**, and a Normal Pro Touge `rd_2_star_touge`. The
port had assumed `rd_3_star` (AUTHORED). The two configs differ in one value [D]:

| Config | min/max distance | power at min/max | reverse |
|---|---|---|---|
| `rd_1_star` | −30 / 100 m | 1.0 / 0.4 | 50 / 400 m → 1.0 / 1.2 |
| `rd_2_star` | 50 / 200 m | 1.0 / **0.45** | same |
| `rd_3_star` | 50 / 200 m | 1.0 / 0.5 | same |

All three: minSpeed 12 m/s, minRaceDistance 500 m, useLeadForMaxDistance on. An AI car more than
200 m ahead of the player now runs at 0.45 of its engine torque instead of 0.5.

**Port (F439):** `ai_pacing::race_day_catch_up(difficulty, touge)` builds the name from the star
level of the AI difficulty (`flashback::star_level`). `catchup = on` (and the default) and the
new `catchup = touge` resolve to it after the whole `[ai]` section is read. An explicit name
still selects that config. The settings files now say `catchup = on`; the two Haruna touge files
say `catchup = touge`, and so do the event menu's Midnight Touge and Pro Touge events. Catch-up only acts with a human reference car, so headless AI-only runs
are unchanged.

## 2. Original systems and port status

| System (original) | What it does | Port |
|---|---|---|
| Catch-up (catchup.xml, 0x90A000–0x917000, race manager 0x5C5000) | Engine torque factor per AI from its gap to the player, clamped 0.1–2.0, written to engine +0xE8 | Ported (`catch_up_power`, `torque_factor`); config name 1:1 since F439 |
| Choreographer events (ai_choreographer.xml, 0x401000–0x40C000) | Mistakes (brake_lock, brake_spin, powerout_spin), tyre blow-outs, engine failures on AI cars ahead of the player | Ported (triggers, windows, chances from the `Mistakes` characteristic) |
| Conveyor belt (0x407940, 0x403E20) | One front-third car slowed (A = 0.7), one back-third car sped up (A = 1.1) | Ported; AUTHORED: single-class fields only, 0-based places |
| Race-start factors (0x401AA0) | Engine × 1.05 ahead of the player in the first 100 m; 1.25 grip for the first 750 m | Ported (`grip_factor`) |
| Off-line grip assist (0x404C90) | Grip up to +25 % inside / +50 % outside when off the racing line in a bend | Ported (`grip_factor`) |
| Corner-speed A/B (driver+0x3154/+0x3158, 0x792C80) | S = max(A·corner_speed_in_ratio, B) | Ported (`Pace`, `corner_speed_ratio`) |
| Random brake-line mistakes (generateMistakes) | Gated off in shipped data | Inert in both |
| Difficulty scaling | `ai_driver_parameter_mappings.xml` by difficulty (Normal 0.606) | Ported (`Parameters::new`) |
| Lapping / team-mate yield, unsafe-retiree removal | Choreographer manager | Ported (yield); removal as the retire drive |

## 3. Remaining differences (not ported 1:1)

- **Race distance.** The original measures the field along the progress track; the port uses
  the AI racing line (AUTHORED). Gaps differ slightly on tracks whose AI line and progress line
  diverge.
- **Random numbers.** The original draws from the game's global RNG, whose stream depends on
  everything else that uses it; the port uses its own seeded MT19937. Event timing and car
  choice cannot match draw for draw.
- **Conveyor details.** The slot count and the place convention are AUTHORED; the threshold rule
  (own distance + U(0, max_sleep_distance), 0x4027B0) is ported as researched.
- **Choreographer lap length** (`+0x80→+0x70`) and participant status 4 are not traced.
- **Off-line assist side.** The original takes the inside assist when `lat·κ ≥ 0` (0x404C90,
  lat from 0x40AE90); the port's `grip_factor` takes it when `lateral·curvature < 0`. This
  agrees only if the port's lateral sign is the opposite of 0x40AE90's, which is not verified.
- **Redgate in traffic.** robustness's live Donington capture (FIRST_CORNER_LIVE.md §4): the
  original's back markers keep 25–28 m/s through Redgate, the port's slow to 18–22. The
  race-start grip factor matches the original, so the cause is elsewhere in the AI's braking
  in traffic (follow brake 0x77A1E0 is the first candidate); not traced here.
- **Redgate corner speed (F449 analysis, not ported).** The original's leader brakes ~18 m
  earlier with partial pressure (0.55–0.78), releases at 37 m/s and holds 37.0–37.6 m/s through
  Redgate with 0.45–0.6 throttle (live field02). The port's leader brakes later at 0.92–1.0, falls
  to ~35 and then sags to 33–34 m/s while its steering saws between −1 and +1. Causes found [V]:
  - The port's line-following curvature is `wrap(heading(ahead) − heading(here)) / look_ahead`
    from the racing-line **segment** directions, which are constant within a segment. In a
    hairpin it is exactly 0 for 10–20 ticks whenever the car and its look-ahead point are on one
    segment (1 309 of 6 000 solo ticks); the look-ahead then grows (0x779A20 reads |κ|), the
    target point jumps and the steering slams to ±1.
  - The original's 0x798E90 instead takes `acos(lateral_here · lateral_ahead)` (double, clamped
    to ±1) over the look-ahead, negated when `(point_ahead − point_here) · lateral_here < 0`
    (F454 correction: F449 had the two points' roles swapped).
    The two line-point records are at track view +0x90 (point +0xB0, lateral +0xC0, offset point
    +0xD0, offset +0xE0) and +0x150 (+0x170 / +0x180 / +0x190); layout from 0x779C40 (offset point
    = point + offset × lateral). The +0x144 braking-table grip is `0x9AD500(vehicle)` (wheel grip
    × surface × ¼, × an aero/mass term), but the Lacetti's two brake tables (grip 2.0 / 2.2) give
    the same 45.4 m from 57 to 39 m/s, so it does not explain the gap.
  - **Ported in F454** [V]. 0x7878F0 fills the look-ahead block from the line object's vfunc
    +0x14 (point and lateral at link/fraction) and projects the planned offset on that lateral, so
    the lateral is the line's own lateral; the port uses its interpolated gate normals. With the
    sign test the right way round κ is smooth and positive for a bend toward positive offsets
    (right), the original's convention. The F449 sign flips came from the swapped test. A few
    single-tick flips remain at bend ends (Redgate arc 505, 525) where the look-ahead point sits
    almost on the car's lateral; whether the original's laterals do the same is not checked [I].
  - Result, solo Lacetti, Redgate (arc 450–560): lowest speed 35.5 m/s (was 32.8), throttle no
    longer pinned at the corner lift's 0.2, κ no longer 0 inside the bend. Still below the
    original's 37.0–37.6, and the braking is unchanged (0.92 from x≈−478 against 0.55–0.78 from
    x≈−460): that part remains open.
  - **Braking, offline analysis (F459)** [V unless marked]. Evidence: main checkout
    `local/driving/redgate-20261019/` (`brk.txt`, `abs.txt`, `solo-f.tsv`, `solo-t.tsv`; port on
    F458 + temporary traces, not committed).
    - Original leader (field02 car 1): pedal 0 → 0.47 in 0.09 s from x −458.8, then 0.51 → 0.80
      over 1 s; 58.1 → 48.2 m/s in 0.84 s (≈ 11.8 m/s²); released at 37.3 m/s.
    - Port solo: activates at dist 63.3 m, trigger 63.4 (b 45.2, r 11.5, corner_brake_distance_factor
      1.118, target 39.7 from corner_speed_in_ratio 0.645, difficulty ≈ 0.61 with speed_in 1);
      ratio (dist − b)/(d − b) ≥ 1 from the first tick, so the raw line brake is 1; once the
      slewed pedal reaches 1 the reaction r becomes 0, d = b and the raw brake alternates 1/0 tick
      by tick, which the 5/s slew holds at 0.92–1.0; ≈ 18.7 m/s².
    - Same in both: the formula 0x7886C0 (decompile drv6), the Lacetti brake tables (grip 2.0 and
      2.2 both give b ≈ 45 m for 57.4 → 39.7 m/s ≈ 19 m/s²), and the previous brake: context
      +0x2C is the slewed published packet brake (0x794310 copies the packet that 0x779F70 slews
      into the local packet before 0x7929D0), as in the port.
    - **Not ABS**: 0x77A2E0 caps the AI's brake demand inside 0x78E8A0 (a pedal cap before the
      packet, not a wheel-torque control). In the port the slowest wheel stays within ~1–2 % of
      the car speed through the stop (one 0.2 cap tick at arc 420), so the cap does not engage,
      and 19 m/s² is real braking at 0.92–1.0. The original's 11.8 m/s² at 0.55–0.8 is
      consistent with a pedal-proportional brake on the same car.
    - Consequence [I]: with this formula a steady part pedal needs the slack `dist − b` to stay
      near 0.97·r (≈ 5 m at pedal 0.55) for a second while the car decelerates at 11.8 m/s².
      That needs either b shrinking at the original's real deceleration (a brake table or grip
      giving ≈ 12 m/s², i.e. b ≈ 80 m for 57 → 37 m/s), or a brake-line distance +0x28 about
      15 m shorter than the port's at the same place (it would also explain braking ~15 m
      earlier). The port's line progress projects the car centre; the original projects the
      front (≈ 2 m), which is too small alone.
    - Live values that decide it (queued capture, Donington Race Day as field02, leader, 60 Hz
      through Redgate): request +0x28 (distance) and +0x30 (target) against the port's at the same
      x; car state +0x144 (table grip) and speed +0xD0 (b recomputed from the table); the four
      wheel speeds (expected near car speed: ABS idle); the packet's raw and slewed brake.
      - +0x28 shorter by ~15 m → brake-line placement or distance;
      - +0x144 outside the tables' grip range with b ≈ 80 m → table lookup or extrapolation;
      - +0x30 ≈ 37.3 with the same b → driver mapping (difficulty/characteristics) only.
  - **Live captures (F469)**, 19 October 2026, owner-approved, research copy only, memory reads
    and menu macros; Race Day, Touring Cars, Lacetti, Donington GP, Normal, grid Back; two
    sessions (PID 36060, 43220), 4 × 60 s of all 11 AI drivers at ≈ 6–9 ms steps, world +7C8 per
    row; 0 of 19 user files changed. Tool: `tools/capture_grid_ai_brake.py` (driver discovery by
    the track-view vtable 0xCF7FAC at driver+0x2DD0; drivers at stride 0x4FE0). Evidence: main
    checkout `local/driving/redgate-live-20261019/` (`run1/`, `run2/`, decoders `*.py`).
    - [V] Every AI car brakes into Redgate the same way: the published brake (+E20) rises at the
      5/s slew (brake_on_speed) to 0.27–0.45, then creeps at ≈ 0.3–1/s to 0.6–0.75 and releases at
      the target (37–39 m/s). Row by row the creep is smooth (+0.003…+0.015 per step), so the raw
      demand sits just above the pedal; there is no 1/0 chatter.
    - [V] Live inputs of 0x7886C0: request +0x28 distance, +0x30 target, +0x20 modifier 1.0,
      +0x24 hold 1.5, line id +0x34; car-state speed +0xD0 and grip +0x144 = 2.18–2.20 (1.90 on a
      few frames); wheel speeds (car state +0x240, 4 floats; vfuncs +0x10/+0x1C) within 1 % of
      the car speed (ABS cap idle); request brake cap +0xA0 = 1.0 and flags +0xA4..+0xB6 = 0; no
      mistake pending or active (slot drv+0x3100 +0x30/+0x38 = −1).
    - [V] With those inputs and the decompiled formula (b from the Lacetti table, r =
      v/brake_on_speed·(1 − published brake), exponent vcfg+0xF4 = brakeCruiseExponent 24) the
      raw line brake is 1 throughout, and a simulation of formula + slew runs to 1 for any table
      (lac, default, bmw, …) and any trigger factor 1.1–1.2 (RMS 0.40–0.48 against the live
      pedal). Solving the creep backwards gives an implied `b ≈ 0.031·(v² − vt²)` (≈ 16 m/s²;
      the Lacetti table gives 0.027, "default" 0.030), but that alone does not reproduce the
      early slow-down at 0.27–0.45.
    - [V] The brake-line object is at **driver+0x4324** (found in 33 whole-driver dumps by its
      copies of target/modifier/hold): +4 active id, +0x16 braking flag. At part-braking onsets 8
      of 11 cars had the line active (+0x16 = 1); three (cars 7, 8, 9) braked 0.21–0.24 with
      **no active line** (one 146 m before it), so another brake source (follow brake 0x77A1E0)
      acts in traffic. The published (throttle, brake) pair is **not** inside the driver object:
      the slewed packet that 0x794310 writes (its `[ebp+8]` argument) lives in a separate
      controller object, and whether the controller (0x828670 getter +4) changes it before the
      setter 0x996240 writes +E20 is open.
    - **Third capture (F471)**, PID 7884, same setup, 2 × 60 s; evidence `run3/`.
      - [V] The AI's published control packet is at **driver−0x94** (updater header +0x04 points
        to it; +0 throttle, +4 brake, +0x24 gear). Its brake leads +E20 by one frame with the same
        shape, so the controller (0x828670) does not shape it.
      - [V] Through the creep the brake-line object (driver+0x4324) is active on the request's
        line with its braking flag +0x16 = 1. Whenever a step changes the packet by less than the
        5/s slew, the slew landed on the raw demand, so the raw brake is observable: 0.44–0.46
        early in Redgate, rising to 0.6–0.8; i.e. the line ratio is ≈ 0.97 (`ratio^24`).
      - [V, 6 554 such samples on lines 0, 3, 6–10] Solving `ratio = (dist − b)/r` per sample: with
        the port's reaction `r = v/5·(1 − prev)` the implied b is 1.15 × the Lacetti table (p10–p90
        1.07–1.25) and 1.05 × the "default" table (1.00–1.14); with the Lacetti table the implied
        reaction is about twice the port's (`v/c·(1 − prev)`, c median 2.36, per-bin 2.2–2.8).
        Joint fits `dist = k·b + ratio·v·(1 − prev)/c + e`: Lacetti k 1.16, c 2.75, e −2.6 m
        (RMS 1.0 m); default k 1.06, c 3.0, e −2.0 m (RMS 1.1 m).
      - Conclusion [I]: the original plans its line braking with a ≈ 5–15 % longer braking
        distance and a ≈ 1.7–2 × longer reaction distance than the port computes from the
        decompile, so it starts earlier and settles at a part pedal. Which term differs (table
        selection, the mapped brake_on value the formula reads, or the distance reference) is
        not identified; a fitted factor would not be 1:1, so nothing is ported.
      - Next (static): the values behind ctx+0x14 (mapped holders) and ctx+0x24 (statistics
        object) of 0x7886C0 in the driver update 0x7929D0/0x794310, against the slew's holder
        table (0x779F70 param_4).
    - **Resolved and ported (F476).** Static: 0x7886C0's two 0x7A6650 calls use different
      `this` objects, ctx+0x24 for the trigger (first call, `ecx = [esp+0x20]`) and ctx+0x28 for
      the ratio (second call, `ecx = [esp+0x28]`). Live read (fourth short run, owner-approved in
      this session, PID 1868, read-only, 0 of 19 user files changed; evidence `run4/`): both come
      from the vehicle AI data at updater header +0x28: **+4 = the car's brake tables, +8 = its
      cruise (coast-down) tables**, equal to `ai_vehicle_statistics.xml`. The field is the
      BMW/Lacetti mix (drivers 1, 2, 6, 7, 8 = grid slots 0, 1, 5, 6, 7 carry the `bmw` tables).
      - Formula [V]: `ratio = (dist − c)/(d − c)` clamped to 0..1 (0 when `c == d`), with `c` the
        coasting distance from v to the target on the cruise tables and `d` as before from the
        brake tables; brake = `ratio^24`. The coasting distance lies far beyond d, so the ratio sits
        just under 1 and the pedal is partial, rising as the car closes on the line.
      - Check [V]: with each car's own tables the formula reproduces the original's raw brake at
        **0.012 RMS over 6 699 samples** (every car 0.007–0.019, bias −0.001).
      - Port: `AiData::cruise_distance` (cruise tables, grip 2.0 clamps to the single 2.2 table);
        solo Lacetti at Redgate now brakes 0.32 → 0.5 → 0.75 from x −476 and releases at 39.4 m/s
        (low point 38.2), the original's shape; the remaining target difference (39.7 against
        37.3) is the driver's corner_speed_in_ratio. The table grip stays 2.0 (live 2.18–2.20;
        the Lacetti's 2.0/2.2 brake tables differ by < 0.1 %).
  - The κ sign consumers now follow the original literally: overtake state 1 widens with bend
    flag +0x45 (κ > ovMin), state 2 with +0x44 (κ < −ovMin); the off-line assist (0x404C90)
    takes max_assist_inside when `lat · κ ≥ 0`; drift state 3 takes the left room (+0x218) for
    κ ≥ 0 (unchanged). The drift next-corner side (0x792580) keeps its own heading-difference
    estimate (not traced).
- **GRID World names.** The career's `gw_%d_star` and `_h2h` names follow the same rule; the
  career session wires them when its races run.

## 4. Checks

`ai_pacing` unit test `settings_parse_and_print` covers the name rule (`rd_1_star` at 0.506,
`rd_3_star` at 0.706, `rd_6_star` at 1.0, the `touge` variant, item order and explicit names).
Full pre-push checks in PROJECT_LOG F439.

Evidence (private): `local/drift-ai/decomp/cu1/` (5D5530, 6AF7C0), main checkout.
