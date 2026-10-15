# Original AI mistakes, failures, drafting and race pacing — research (F-AI-04)

15 October 2026, Claude Code. Static analysis of the hash-identified private analysis PE
`local/compatibility/grid-unpacked.exe` (SHA-256 `aac46a64…4f34514`, base 0x400000).
It continues [AI_DRIVER_RESEARCH.md](AI_DRIVER_RESEARCH.md),
[AI_OVERTAKING_RESEARCH.md](AI_OVERTAKING_RESEARCH.md),
[AI_AVOID_RECOVERY_RESEARCH.md](AI_AVOID_RECOVERY_RESEARCH.md) and
[AI_DRIVER_PORT.md](AI_DRIVER_PORT.md).

- The running original game (PID 7208) was **not** touched. The only live values quoted
  are re-decoded from the read-only driver dumps taken earlier for F290
  (`local/ai-research/live/driver00..11.bin`, `drivercfg.bin`).
- No game file, Steam install or Ghidra project was modified. Ghidra 12.1.4 ran headless
  with `-readOnly -noanalysis` on an idle project copy (`local/driving-assists/ghidra-project`)
  because the shared `local/phase3` project was locked by another session.
- Decompiler output, disassembly listings and helper scripts are private under ignored
  `local/ai-mistakes/` (and the earlier attempt's `local/ai-choreographer/`).
- This document holds only addresses, offsets, constants, shipped parameter values and
  structural descriptions.

Labels: **[V]** verified in disassembly/decompilation; **[D]** shipped data;
**[I]** interpretation, not verified. Notation as in the avoid/recovery document:

- `cfg+X` is the shared driver config; the getter 0x7A3840(i) returns `cfg+0x18+0x14·i`.
- `vcfg+X` is the vehicle config.
- `map[k]` is the driver's mapped value; the index table is at 0xE4CE20 (12-byte records).
- `req+X` is the request at driver+0x2CE0.
- `own+X` is the AI car state.
- `drv+X` is the AI driver object (stride 0x4FE0).

## 0. Answers

1. **Mistakes.**
   - There are two mistake sources. Both feed one per-driver mistake slot at drv+0x3100
     (§2):
     - A **random brake-line generator** (0x787620) driven by `mistake_probability` and the
       `mistake_brake_*` mappings. It is **switched off in every shipped mode**:
       `generateMistakes` (cfg+0xCAC) is built-in false and absent from the XML, confirmed
       false in the live config dump [V+D].
     - The **choreographer** (`ai_choreographer.xml`, game code 0x401000–0x40C000) [V].
       - It schedules scripted events relative to the human player's race distance. It
         picks an AI car 0–250 m **ahead of the player** (so the player sees it), weighted
         by `1 − Mistakes` characteristic.
       - Event types: `mistake` (`brake_lock`, `brake_spin`, `powerout_spin`), `blow_tyre`
         and `blow_engine`.
   - Mistake types (enum 0xCF6F64) [V]:

     | Value | Name |
     |---|---|
     | 0 | brake_heavy |
     | 1 | brake_light |
     | 2 | brake_late |
     | 3 | brake_early |
     | 4 | brake_spin |
     | 5 | powerout_spin |
     | 6 | brake_lock |

     Effects [V]:
     - brake-line trigger distance scaled (early/late);
     - brake pedal floor/cap (heavy/light);
     - rear grip cut on braking with excess speed (brake_spin, rear grip 0.2 for 3 s);
     - forced full throttle plus rear grip 0.4 on corner exit (powerout_spin, 3 s);
     - one wheel's grip 0.2 under braking (brake_lock, until the next brake line or 5 s).
   - **Failure bytes**: lost_control's four bytes are the car adapter's `+0x254` (vtable
     0xCAFC54 slot +0x18 = 0x40B4F0), filled through the virtual setter 0x40B640 (slot
     +0x34).
     - The `blow_tyre` event sets a random wheel's failure flag `wheel+0x7C = 1` (vehicle
       `+0x1990 + w·0x150`) [V].
     - That this flag is what reaches `+0x254` is [I]: the virtual caller of the setter was
       not found.
     - `blow_engine` raises engine/radiator damage and, 2 s later, sets drv+0x316D. That flag
       gates the damage-based `retire`/`race_damaged` tests [V].
2. **Drafting** is **not** an aerodynamic effect.
   - The vehicle's drag coefficient (+0xE10) and air density (+0x21C4) are written only at
     construction. No other code changes them [V, displacement scan].
   - The only slipstream code is the AI `draft` behaviour (score 4). It moves the desired
     lateral offset onto the car ahead at dfOffsetMergeFactor, with **no speed, throttle or
     grip bonus** [V].
   - Its test is **disabled in every shipped mode** (`test_draft false`) [D].
   - `slipStreamTime` is only a team-radio speech variable [V registration].
3. **Catch-up / rubber-banding** [V] acts on **engine torque** and **tyre grip**, and on
   corner speeds for a few cars. Per AI car per tick:
   - **Power**: engine `+0xE8` torque factor (vehicle +0x1888) = clamp(catch-up power ×
     choreographer power factor, 0.1, 2.0) (0x5C5B50 → 0x98F2D0 → read by engine torque
     0x9B1B90).
     - **Catch-up** (`catchup.xml`, 0x90CCE0): cars **ahead of the player** by more than
       minDistance lose power linearly down to powerAtMaxDistance (0.4–0.6). maxDistance can
       be extended to the leader's gap.
     - Cars **behind the player** by more than 50 m gain up to 1.2× at 400 m.
   - **Grip**: each wheel's grip scale (`wheel+0x80`) = request grip multiplier × manager
     grip factor (0x408710). The manager factor is:
     - `race_start_factor_2` 1.25 for the first 750 m of race distance;
     - afterwards `1 + off-line assist`: up to +0.25 inside / +0.5 outside when 4 m off the
       racing line in a corner.
   - **Corner speed**: `S = max(A · corner_speed_in_ratio, B)` with A = drv+0x3154 and
     B = drv+0x3158.
     - A is the **conveyor belt** multiplier: 1.1 while one designated car is pushed
       forward, 0.7 while it drops back.
     - B = 1 for AI cars ahead of the player in the first 100 m and for faster classes.
   - Leader/last-place logic exists only as:
     - the conveyor's target places (2–3 forward, last two backward);
     - `useLeadForMaxDistance`;
     - the choreographer's per-position chances.
   - Characteristics:
     - **Speed_In** drives corner_speed_in_ratio and corner_brake_distance_factor (ported).
     - **Speed_Out** has no effect: `corner_speed_out_ratio` has no reader [V], and the lift
       angles are constant in the data [D].
     - **Virtual_Performance** contributes 1 % (`0.01·VP + 0.99·x`) to a per-car value in
       the vehicle manager whose consumer was not traced [V/I]. It is not an AI pacing input.
4. **Remaining behaviours**: race_damaged (9), controlled_stop (20), block (7, test off),
   draft (4, test off), visibility_loss (6, **on**), lost_control (21), damage retire (10)
   and let-pass (part of race, not a behaviour). Table and sizes in §5.

## 1. Corrections to earlier documents

- **AI_DRIVER_RESEARCH §4.1** "ctx+0x38 (A,B) unresolved; candidate rubber band" [V]:
  - ctx+0x38 is drv+0x3150 (argument 11 of 0x79CAA0 from 0x79EE30).
  - A = drv+0x3154 (init 1.0) is written only by the conveyor belt (0x403E20).
  - B = drv+0x3158 (init 0) is written by the race-start/class rule (0x404C90) and reset
    every manager tick (0x404B50).
  - catch-up does **not** touch A/B; it changes engine torque.
- **AI_DRIVER_RESEARCH §4.2 / §7** "× mistake_brake_early/late factor during a
  choreographed mistake": the early/late factor applies while the **pending** mistake is
  early/late. Only the (disabled) random generator produces those types; choreographed
  events use only types 4–6. Heavy/light apply while **active** (§2.5).
- **AI_DRIVER_RESEARCH §7** catch-up "consumer not found": found, §4.1.
- **AI_AVOID_RECOVERY §3.6 / §4.4**: the lost_control failure bytes are car-adapter
  +0x254 (§2.8). They are not computed by the AI.
- **Record/own state +0x14C** (AI_OVERTAKING §5.1 "state 2 = moving toward us [I]") [V]:
  - 0x406F50 sets own+0x14C from the participant's status (`+0x6C`): 0 → 0, 4 → 2,
    anything else → 1.
  - F302 observed the player record 0 and racing AI cars 1 live.
  - So state 2 is "participant status 4" [I: retired/removed], not a motion state.
  - The overtaking side checks that require state 0 deserve a re-check (open question 1).
- **Mapping index order**: the 0xE4CE20 records are `(name, index, 0)`; indices 11–16 are
  corner_speed_in_ratio, corner_speed_out_ratio, corner_brake_distance_factor,
  throttle_corner_lift_start/end_angle and throttle_confidence (not the XML order). The
  mistake mappings are 33–39 (§2.3) [V].

## 2. Mistakes

### 2.1 Mistake slot drv+0x3100 [V]

Built in the driver constructor (0x79EB34..) and reset at 0x79EFD2.

| Offset | Meaning |
|---|---|
| +0x00 | pointer to the per-driver RNG at drv+8 |
| +0x04 | spin duration (3 s, .rdata 0xCB1790) |
| +0x08 | other duration (5 s, 0xCB093C) |
| +0x0C | probability (`map[33]`, set by 0x7758A0) |
| +0x10..+0x28 | 7 cumulative weights |
| +0x2C/+0x30 | **pending** type object / value (−1 = none) |
| +0x34/+0x38 | **active** type object / value |
| +0x3C | pending timeout |
| +0x40 | active timer |
| +0x44 | last brake-line id |
| +0x48 | brake-line counter |
| +0x4C | locked wheel index |

Small methods:

| Address | Method |
|---|---|
| 0x7758A0 | init (probability in [0,1]; weights each in [0,1], else error 0x16) |
| 0x7759E0 | set probability |
| 0x7759F0 / 0x775A20 | enum value ↔ name |
| 0x779990 | **SetPending(type, timeout)** |
| 0x7799B0 | **Activate** |

0x787620 (the per-tick update) is called from 0x79EE30 **after** arbitration and the behaviour
drive and **before** the controls (drv+0x9F8 vfunc +8 → 0x794310).

### 2.2 Weights and probability (0x779800, end of the mapping updater 0x793310) [V]

- probability = `map[33]` mistake_probability (holder drv+0x3094; 1.0 if absent);
- weights `w[k]` for type k = mapped `[heavy 36, light 37, late 35, early 34, brake_spin
  38, powerout_spin 39, brake_lock 49]`.
  - Index 49 is past the 48-entry table; the live holder there reads 0.0, so brake_lock is
    never drawn randomly. Default when a holder is null: 1/7.
- Live (Normal, d = 0.606) [V live]:
  - driver 0 = `p 0.1015, w [0.600, 0.956, 0.441, 0.120, 1.0, 1.0, 0.0]`;
  - the other drivers p 0.113–0.158, matching the bilinear mappings with their Control /
    Mistakes characteristics [D].
- Mapping limits [D] (difficulty 0/1 × characteristic 0/1; `value` is unused when limits
  exist):

  | Mapping | Characteristic | Limits |
  |---|---|---|
  | mistake_probability | Mistakes | 0.2 / 0.15 / 0.13 / 0.07 |
  | early | Control | 0.1 / 0.15 / 0.2 / 0.1 |
  | late | Control | 0.4 / 0.35 / 0.5 / 0.5 |
  | heavy | Control | 0.75 / 0.6 / 0.7 / 0.6 |
  | light | Control | 0.85 / 0.92 / 0.96 / 0.98 |
  | brake_spin / powerout_spin | Control | 1.0 |

### 2.3 Random brake-line generator 0x787620 (inactive in shipped data) [V]

On a change of the request's brake-line id (`*param3`, from the brake-line object at
drv+0x2E44 +4):

1. +0x44 = id. An **active** mistake of type 0, 1, 2, 3 or 6 is cleared. Spins (4, 5)
   survive.
2. Draw a mistake only if **all** of these hold:
   - `req+0xB4` (set each tick by 0x7978D0 = `generateMistakes` unless race_finished fired);
   - no pending mistake;
   - `++counter > mkNumBrakeLinesBeforeMistakes` (cfg+0xC84 = 3);
   - `rand01 ≤ p`;
   - id ≠ −1.
3. Then `r = rand01`; pending = the type with the **smallest weight strictly greater than
   r** (ties keep the lower index), with pending timeout 60 s (0xCAFEC4).
   - With the live weights: r < 0.12 → early; < 0.44 → late; < 0.60 → heavy;
     < 0.956 → light; otherwise brake_spin.

Every tick:
- Pending timeout −= dt and active timer −= dt. Expiry clears the slot.
- Then the grip override:
  - active 4: `req+0xB8/+0xBC` (rear wheels RL/RR) = mistakeEntrySpinRearGrip (vcfg+0x388,
    0.2) and the fronts = 1.0;
  - active 5: rears = mistakePowerSpinRearGrip (vcfg+0x360, 0.4);
  - active 6: all four wheels = 1.0 except wheel `+0x4C` = mistakeBrakeLockGrip (vcfg+0x374,
    0.2). Slot order is +0xC0 FL, +0xC4 FR, +0xB8 RL, +0xBC RR.
- This overrides the racingGripMultiplier that 0x7978D0 wrote, and any lost_control grips.

`generateMistakes` = false: no XML section sets it, the constructor default is false, and
live `drivercfg.bin` byte +0xCAC = 0. The live `req+0xB4` is 0 on all 12 drivers [V+D].

### 2.4 Activation 0x7799B0 and its four call sites [V]

`Activate`: if pending ∈ 0..6 and no active mistake:
- active = pending, pending = −1, timer = 5 s;
- spins (4, 5): timer = 3 s;
- brake_lock (6): wheel = RNG int in [0, 4).

| Site | Condition (pending type) |
|---|---|
| 0x7888AB brake-line trigger 0x7886C0 | 2 or 3 when the trigger distance is reached |
| 0x7889C7 same, while braking | 0, 1 or 6 once the brake-line brake is > ~0 |
| 0x77A152 handbrake byte 0x77A110 | 4 if brake-line braking is active this tick (brake-line object +0x16) and `speed − line target ∈ (8, 12) m/s` (.data 0xE4D0C0/0xE4D0BC) |
| 0x77A491 throttle limiter 0x77A450 | 5 if the governor's "past the brake line / in corner" flag (brake-line object +0x15, set while the held line target applies, cleared when no line and not cornering) is set, the previous throttle > 0.6 (0xE4D0C8) and speed > 10 m/s (0xE4D0C4) |

### 2.5 Effects [V]

- **Brake-line trigger** (0x7886C0): trigger distance × mistake_brake_early_factor while
  **pending** = 3, × mistake_brake_late_factor while pending = 2. Both shipped factors are
  < 1 (Normal, control 1: early 0.12, late 0.44), so **both make the braking point later**
  [D]. Port the data, not the names.
- **Brake output**: while **active** = 0, brake = max(brake, heavy factor) (0.6); while
  active = 1, brake = min(brake, light factor) (0.956).
- **powerout_spin**: 0x77A450 returns 1.0 while active = 5. 0x783180 takes the **max** with
  the min-of-limiters, so the throttle is full regardless of traction control and corner
  lift (unless braking). Rear grip 0.4 for 3 s.
- **brake_spin**: rear grip 0.2 for 3 s (no pedal change).
- **brake_lock**: one wheel's grip 0.2 until the next brake line or 5 s.
- **Cancellation**: 0x794310 clears the active mistake when speed < 10 m/s (0xCAFA58).
- Consequences are ordinary physics plus the existing tests. A spin trips `spun` →
  recovery (11+), or lost_control spin mode (21) when the yaw rate is > coSpinSpeed.

### 2.6 Choreographer (`ai_choreographer.xml`) [V structure, D values]

The manager (game object; tick at 0x4093E0..) runs, in this order:
1. reset 0x404B50;
2. conveyor slots 0x4041A0 → 0x403E20;
3. touge traffic;
4. triggers 0x408E50 [I name];
5. 0x4047E0;
6. 0x404860;
7. race-start/assist 0x404C90.

The XML `set` is chosen by discipline. `circuit`/`city`/`default` share the first set;
`first_race`, `le_mans` and `single_demolition_race` have their own; `network`, `garage`,
`test_drive` and `special` disable the manager.

**Reference car**: the participant with flag +0x28 (the human player, 0x405890). **Race
distance** = laps-completed fraction × lap length (0x4022A0: participant progress vfunc
+0x48 × `+0x80→+0x70`) [I: lap length = progress-track length]. With `use_time_limit` it is
race time.

**Candidate list** (0x405890, per trigger):
- every non-human participant whose status ≠ 4;
- class filter unless `affect_higher_classes`;
- base weight `b = clamp(1 − Mistakes, 0, 1)` (characteristic name at 0xCAF2A0), initial
  weight `0.5·b`.

**Trigger fields** (0x4067A0):

| Offset | Field (default) |
|---|---|
| +0x1DC | priority |
| +0x1E0 | min_distance |
| +0x1E4 | max_distance |
| +0x1E8 | min_race_distance |
| +0x1EC | min_event_separation |
| +0x1F0 | min_time |
| +0x1F4 | min_distance_ahead (1) |
| +0x1F8 | max_distance_ahead (100) |
| +0x1FC | select_closest |
| +0x200 | max_events (1) |
| +0x204 | chance (1) |
| +0x208 | affect_higher_classes |
| +0x209 | use_time_limit |

It also holds a `position` list (index, chance, rolled flag) and an `event` list.

**Arming** (0x4059F0, every tick while `max_events == 0` or the slot counter < max_events):
- `min_time` timer −= dt, and `last fire distance + min_event_separation − now` is
  recomputed. When both are < 0:
  - each candidate: `eligible = rand01 < weight·chance`, then
    `weight = clamp(weight + 0.1·b, 0, b)` (0x402AD0, 0.1 = 0xCAFCD4);
  - each `position` entry: `allowed = rand01 < chance`;
  - schedule the next window (0x402B50):
    - `start = max(min_race_distance·L, min_distance)`;
    - `end = L` or `max_distance` if 0 < it < L;
    - `slot = max((end − start)/max_events, min_event_separation)`;
    - window k (k = slot counter, then ++) = `[start + k·slot, start + (k+1)·slot]`,
      clamped to `end`;
  - pick the event uniformly from the trigger's list (0x778610 inclusive range).

**Selection** (0x406CF0, every tick):
- Once the reference distance is past the window start (re-arm if past its end), take the
  first eligible candidate, or the closest if `select_closest`. A candidate must have:
  - a position entry that is not disallowed (the entry index is matched against the
    participant's position vfunc +8 [I: race place]);
  - a track distance ahead of the reference in `(min_distance_ahead, max_distance_ahead)`
    (0x90BEA0, same route only).
- **Fire** (0x406EE0): event count++, the car's weight ×0.5, min_time timer reset, last
  fire distance = now.
- The event starts only if `speed ≥ event min_speed` (0x401630).

**Events** (vtables 0xCB0108 blow_engine, 0xCB0124 blow_tyre, 0xCB0140 mistake):
- **mistake** (parse 0x404360: `+0x7C` type by name, `+0x80` timeout 60):
  - start 0x4018A0 → drv+0x3100 SetPending(type, timeout);
  - update 0x401920: running while active == type;
  - stop 0x4018D0 clears pending.
  - The `tyre_smoke` flag sets participant `+4→+0x64` while running (0x401690).
- **blow_tyre** (0x404200): random wheel w ∈ [0, 3]:
  - vehicle `wheel+0x7C = 1`;
  - damage-state update 0x9EF020/0x9DE440/0x98A100 with wheel w flagged;
  - done after 2 s (0xCB8DA4).
- **blow_engine** (0x401730):
  - damage component 3 (engine, also own+0x11C via 0x401080) = max(current,
    engine_damage);
  - component 2 (radiator) = max(current, radiator_damage) (0x99A4D0/0x99A4E0);
  - after 2 s **drv+0x316D = 1** (§2.8).

Shipped `circuit` triggers [D] (chance is per-car per arming; `ahead` is metres ahead of the
player):

| Trigger | Events | min_distance / sep / min_time | max_events | chance | ahead | positions |
|---|---|---|---|---|---|---|
| mechanical_failures | blow_tyre_fast (≥ 60 m/s), blow_engine_slight (radiator 1, engine 0.25), blow_engine_total (1, 1) | 5000 m, from 25 % of the race / 5000 m / 120 s | 3 | 0.15 | 20–150 | index 1: 0.5 |
| major_errors | powerout_spin, brake_spin (≥ 30 m/s, timeout 60) | 500 m / 500 m / 20 s | 40 | 0.3 | 5–250 | index 0: 0.33, index 1: 0.66 |
| minor_errors | brake_lock (≥ 30 m/s) | 400 m / 200 m / 5 s | 100 | 0.75 | 0–200 | – |

Example: a 3-lap Donington race, L ≈ 12,012 m [I L]. Windows are 5000 m (failures, at most
two windows fit), 500 m (major) and 200 m (minor) of player race distance.

### 2.7 Worked consequence [I]

- A mistake needs a car **within the ahead range in front of the player** when the window
  opens. A player far behind or far ahead of the pack sees few events.
- With `1 − Mistakes` weights, the cars with Mistakes 0 are picked most.
- Without a human car (headless AI-only runs) there is no reference and no events.

### 2.8 Failure bytes and the damage chain [V unless marked]

- lost_control test 0x77ED50 reads the four bytes returned by `own` vfunc +0x18. The car
  adapter (vtable 0xCAFC54, built over the participant) implements +0x18 as `lea eax,
  [ecx+0x254]` (0x40B4F0).
  - The bytes are written only by the virtual setter 0x40B640 (slot +0x34, `n ≤ 4`) and
    cleared at 0x40B48F/0x40BFB4.
  - The setter's caller was not found in 0x401000–0x40C000 or 0x5B0000–0x5D0000.
  - Linking blow_tyre's `wheel+0x7C` to these bytes is therefore **[I]**, strongly
    suggested by the per-wheel layout and by retire counting failures.
- Then (AI_AVOID_RECOVERY §4.4): lost_control fires at speed > rtRetirementSpeed + 5
  (20 m/s).
  - Above crashMinSpeed (50) it runs the crash mode (crash PID, crashRearGrip 0.1, spin/flip
    angles, `req+0xD0` flip).
  - Otherwise it cuts the failed wheel's grip to 0.1 and wobbles at rtRetirementSpeed.
  - Retire (10) then drives to a retire point.
- 0x408710 also writes the vehicle inertia factor (+0x21C0) = crashInertiaFactor (vcfg+0x61C,
  0.1) while `req+0xD0` is set, else 1.0.
- **Damage gate**: retire test +0xD (damage > damage_retirement_ratio) and +0xE
  (damage ≥ 0.5 → race_damaged) are evaluated only when test context +0x34 → +0x1D, i.e.
  **drv+0x316D**, is set. Only `blow_engine` sets it.
  - own+0x11C damage = engine damage component (0x401080).
  - Contact damage alone therefore never sends the AI into race_damaged/retire [V].

## 3. Drafting

- **AI draft** (score 4 = `behaviour_draft` 4th in the fire list; vtable 0xCF8E2C):
  - **Test** 0x77E0F0 (type 9): the nearest record ahead, with `r.rear − own.front` along own
    forward > 0, `r.speed > dfMinDraftSpeed` (cfg+0x518, built-in 22 m/s) and angle
    `< dfMaxDraftAngle` (cfg+0x52C, 0.175 rad) → fired with the id. dfRejectDistance
    (cfg+0x504, XML 25) is not used by the test.
  - **Score** 0x789D90.
  - **Drive** 0x789DD0: `req+0x98` = the target's signed lateral offset from the racing line
    (0x7A2C80), `req+0x9C` = dfOffsetMergeFactor (cfg+0x540, 2.5).
  - Nothing else: no desired speed, throttle or grip change [V].
  - `test_draft` is false in default and every other set [D]. **Not active in GRID races.**
- **Vehicle**: aero (0x9E1290, ported as `vehicle_driver_prepare::aero`) uses the car's
  own `+0xE10` drag coefficient, `+0x21C4` density and `+0x2214` downforce terms.
  - The scan finds `+0x21C4` written nowhere in vehicle code and `+0xE10` only at
    0xA0E210 (construction).
  - **No slipstream** reduces drag behind another car [V within the scan; dynamic writes
    through computed pointers are not excluded].
- `slipStreamTime` (0xCB6D64) is registered at 0x45FABE with getter 0x418970 (`+0xB3FC`)
  among the commentary variables. No writer was found [I: unused speech variable].

## 4. Pacing and catch-up

### 4.1 Catch-up (`catchup.xml`) → engine torque [V]

**Config** (parser 0x90A590; object at game+0xC, config block +0x29C):

| Offset | Field |
|---|---|
| +4 | enabled |
| +8 | minDistance |
| +0xC | maxDistance |
| +0x10 | powerAtMinDistance |
| +0x14 | powerAtMaxDistance |
| +0x18 | minSpeed |
| +0x1C | minRaceDistance |
| +0x20 | render |
| +0x21 | renderOnCars |
| +0x22 | playerCatchup |
| +0x23 | higherClass |
| +0x24 | useLeadForMaxDistance |
| +0x2C | reverseEnabled |
| +0x30 / +0x34 | reverseMin/MaxDistance |
| +0x38 / +0x3C | reversePowerAtMin/MaxDistance |

The `config` is selected by a `user` name (0x90A790): `gw_N_star`, `rd_N_star`, `first_race`,
`*_h2h`, `*_touge`, `network` or `none`. Which name a Normal single race passes is [I].

**Position messages** (0x911690):
- leader distance → +0x2F0;
- reference distance → +0x2F4: the player's, or the leader's when `playerCatchup`;
- per-car entry `+0x10` (message +8; compared with minSpeed² [I speed²]) and `+0x14`
  race distance.

**Tick** (0x911590):
- `M = maxDistance`; with `useLeadForMaxDistance` and `leader − R ≥ M`, `M = leader − R`.
- Per car (0x90CCE0), with `R` the reference distance and `D` the car's distance:
  - `power = base (1)`.
  - `gap = D − R` (or `R − D` with playerCatchup).
  - **Ahead**: if `enabled`, `speed² > minSpeed²`, `gap > minDistance`,
    `D > minRaceDistance`, the class rule holds and the car is not the player: active,
    `power = clamp(lerp(pMin, pMax, (gap − minD)/(M − minD)))`.
  - **Behind**: otherwise, if `reverseEnabled` and `R − D > reverseMinDistance` (same speed
    and race-distance rules, not the player):
    `power = remap_clamped(R − D, revMin, revMax, revPMin, revPMax)` (0x40BB00).
- **Application** (0x5C5B50, while game +0x340): `f = 1`.
  - Then `f = catch-up power` if the car's entry is active (0x911630).
  - Then `f ×= choreographer power factor` (participant +0x20, getter 0x406190; 1.0 for the
    player).
  - `0x98F2D0(vehicle+0x17A0, f)` stores `clamp(f, 0.1, 2.0)` at engine +0xE8.
  - The engine torque 0x9B1B90 multiplies by it; already modelled in
    `grid-world::vehicle_propulsion` as `e + 0xe8`.

Shipped configs [D] (minDistance / maxDistance → power; reverse 50–400 m → 1.0–1.2 unless
noted):

| Users | Forward | Notes |
|---|---|---|
| gw/rd_1_star, first_race | −30 / 100 → 1.0–0.4 | useLead (gw/rd 1–4) |
| gw/rd_2_star | 50 / 200 → 1.0–0.45 | |
| gw/rd_3_star | 50 / 200 → 1.0–0.5 | |
| gw/rd_4_star | 100 / 300 → 1.0–0.6 | |
| gw/rd_5_star, gw/rd_6_star | disabled | the reverse boost still applies: the query and update run when `enabled` **or** `reverseEnabled` (+0x2A0 / +0x2C8), and 0x90CCE0's reverse branch tests only `reverseEnabled` [V] |
| *_h2h | −25…100 / 100…225 → 0.3–0.6 | reverse 50–200 |
| *_touge | | 5-star 100/250 → 0.75 |
| network | 100 / 500 → 1.0–1.2 | higherClass |

All use minSpeed 12 m/s and minRaceDistance 500 m.

### 4.2 Choreographer manager pacing [V]

**Manager fields** (0x401AA0, values from the `circuit` set [D]):

| Offset | Field | Value |
|---|---|---|
| +0xBDD | enable | |
| +0xBDE | remove_unsafe_retires | |
| +0xBE0 | removal_distance_crow | 125 |
| +0xBE4 | removal_distance_track | 350 |
| +0xBE8 | lapping_yield_distance | 50 |
| +0xBEC | team_mate_yield_distance | 25 |
| +0xBF0 | race_start_distance_1 | 100 |
| +0xBF4 | race_start_distance_2 | 750 |
| +0xBF8 | race_start_factor_1 | 1.05 |
| +0xBFC | race_start_factor_2 | 1.25 |
| +0xC00 | min_distance_offline | 0 |
| +0xC04 | max_distance_offline | 4 |
| +0xC08 | max_assist_inside | 0.25 |
| +0xC0C | max_assist_outside | 0.5 |
| +0xC10 | min_curve_offline | 0.001 |
| +0xC14 | fast_higher_classes | default true |

Per tick, after the reset (all non-player cars: power factor +0x20 = 1, grip factor +0x24 = 1,
B = 0, backwards flag drv+0x316C = 0):

- **Race start** (0x404C90), with `R` the player's race distance and `D` the car's:
  - If `R < D < race_start_distance_1`: **power factor = race_start_factor_1 (1.05)** and
    **B = 1** (corner speed ratio floor 1).
  - Otherwise B = 1 only for a faster class when `fast_higher_classes` is set.
  - If speed > 10 m/s:
    - `D < race_start_distance_2`: **grip factor = race_start_factor_2 (1.25)**;
    - else, if `|curvature| > min_curve_offline`: `lat` = own lateral offset from the
      racing line (0x40AE90); `assist` = max_assist_inside if `lat·κ ≥ 0` else
      max_assist_outside [I which is inside]; **grip factor =
      1 + clamp(assist·(|lat| − min_off)/(max_off − min_off), 0, assist)**.
  - At ≤ 10 m/s the grip factor is 1.
- **Grip application** (0x408710, every AI car except the player): for wheel w = 0..3,
  `vehicle wheel(w)+0x80 = req grip[w] (req+0xB8 + 4w, via 0x776680) × grip factor`.
  - The ported tyre code reads that field as `wheel_history::Wheel::grip_scale`.
  - Request grips are racingGripMultiplier (touring 1.0; drift classes 1.4 [D]) or the
    mistake/lost_control overrides.
- **Conveyor belt** (0x403E20, one call per conveyor slot; `circuit` data [D]):
  - Data: min_grid_size 6, forwards_multiple 1.1, backwards_multiple 0.7, max_sleep_distance
    1000, forward places 2–3, backward places 0–1. `min_sleep_distance` is in the XML but has
    **no parser string**, so it is ignored [V].
  - States:
    - 1/3 sleep until the reference distance passes the slot threshold.
    - **2 forward**: target place = rand[2, 3]. Until the car's place (participant vfunc
      +0x10) ≤ target: **A = power factor = 1.1**.
    - **4 backward**: target = `N − rand[0, 1] − 1`. Until place ≥ target: **A = power
      factor = 0.7** and **drv+0x316C = 1**.
    - 5: power 1.1 while place > 0.
    - Reaching a target calls 0x4027B0 (next threshold [I]).
  - Effects of A (§4.3): corner speeds ×A (capped by each brake line's max speed) and engine
    torque ×A.
  - Live [V]: one of the 12 drivers had A = 1.1 at the F290 sample time.
- **Let-pass** (race drive 0x78EDF0 → 0x77C2C0): while drv+0x316C is set (backward conveyor
  car), any record whose front is behind own rear and within coLetPassDistance (cfg+0xB6C,
  built-in 30 m):
  - throttle cap = throttle_confidence × coLetPassThrottleRatio (cfg+0xB80, XML 0.8);
  - a side flag (`dot(D, own right) < 0`) goes to the race offsets 0x78A4D0 [I use].
  - The **block** test refuses to fire for that car (§5).
- Removal of unsafe retired cars (0x401CB0): removal_distance_track/crow from the player.
  Lapping/team-mate yield distances are read elsewhere (not traced).

### 4.3 Corner speed with A and B (0x792C80) [V]

`S = max(A · map[11] corner_speed_in_ratio, B)`, then `target = clamp(min + (max − min)·f·S,
min, max)` as already ported (AI_DRIVER_PORT uses A = 1, B = 0).

- A = 1.1 raises S only up to the brake line's max speed.
- B = 1 means every brake line's max speed.

### 4.4 Characteristics [V/D]

| Characteristic | Effect in shipped data |
|---|---|
| Speed_In | corner_speed_in_ratio, corner_brake_distance_factor (ported) |
| Speed_Out | corner_speed_out_ratio (`map[12]`): no reader (holder-pattern sweep 0x760000–0x7C0000 and driver-relative +0x3040 scan); throttle_corner_lift_start/end_angle: identical limits 0.01/0.03 → **no effect** |
| Mistakes | `mistake_probability` (random generator, off); choreographer weight `1 − Mistakes` (active) |
| Control | steering/pedal speeds, throttle_confidence, mistake factors/weights |
| Virtual_Performance | 0x405C30 reads the `ai_driver` database field (1.0 for the human). 0x5F9xxx passes it per car to 0xA00960, which stores `0.01·VP + 0.99·x` (0xCB0DE4/0xCB0DC8) in vehicle-manager slot `+0x4C0 + i·0x50` (+4). Consumer not traced [open]. **Not part of AI driving or catch-up.** |

### 4.5 Leader / last place

- No grip/power scaling by place exists other than the conveyor's place targets and
  catch-up's leader-based max distance.
- Catch-up is player-relative.
- The choreographer prefers cars near the player and uses per-position chances.

## 5. Remaining behaviours

Scores from `ai_behaviour_fire.xml` [D]; test flags from `ai_driver_config.xml default` [D].
Sizes are rough Rust estimates for `ai_driver.rs` plus the boundary.

| Behaviour (score) | Test (default) | Score fn | Drive | Port size |
|---|---|---|---|---|
| race_damaged (9) | retire 0x784560 +0xE: damage ≥ 0.5, **only with drv+0x316D** (on) | 0x78A690 | 0x799FB0: req+0x84 = 50 m/s, +0x7C = 0.6, merge raOffsetMergeFactor. Every 1 s (or when 0x77C690 says so), 0x797F00 picks the nearer track limit (types 3/4, 0x796B30; random side without gates) → req+0x40 = that limit line, req+0x98 = ±(own halfW + 0.5) [I: offset relative to the limit line] | S, ~60 lines + limit-line reference; needs blow_engine |
| retire damage branch (10) | retire +0xD (damage > damage_retirement_ratio, gated as above) or failures | 0x78B300 | 0x79C6E0 → 0x79A090 retire point at rtRetirementSpeed 15, stopping brake 0x788A90 | S–M, ~80 |
| controlled_stop (20) | stop 0x77E7D0 (view invalid / mode names) and +0xC slow (on) | 0x78AA50 | 0x78AA90: force brake, req+0x98 = view+0xE0, no-steer when slow | XS, ~20 (the port has force brake / no-steer) |
| lost_control (21) | 0x77ED50 failure bytes, speed > 20 m/s; or spun and spinning fast (on) | 0x78B180 | 0x776B80: crash 0x77D200 / spin reverse / wobble 0x77CF10 (AI_AVOID_RECOVERY §4.4) | M–L, ~200; needs failure bytes and inertia factor |
| visibility_loss (6) | 0x77E8A0: a record ahead within vlViewHalfWidth (3 m) of our nose line and closer than speed·vlDropoffEndTime (1 s), whose `+0xF8` (own+0x140 from 0x645940, an effects value [I: tyre smoke]) > visibility_loss_tolerance_ratio (`map[40]`) (**on**) | 0x78CBD0 | 0x78CC10: offset = view+0xE0 ± dt·tgOffsetMergeFactor, merge tgOffsetMergeFactor; side re-chosen by 0x77EDB0 every 1 s or on target change (activate 0x7776F0) | M, ~90 + a smoke source (choreographer `tyre_smoke` [I]) |
| block (7) | 0x77D8D0 (**off**): not the backward conveyor car (drv+0x316C), \|κ\| < blMaxTrackCurvature (0.01), a car behind closing by < blMaxSpeedDifference, distance in (blMinDistance 1, min(blMaxDistance 20, blDistanceSpeedFactor·v)) | 0x788E70 (hold 1 s) | 0x792B90 modes 0x77B690 / mirror 0x788F10 (AI_OVERTAKING §9) | M, ~120; data-gated off |
| draft (4) | 0x77E0F0 (**off**) | 0x789D90 | 0x789DD0 (§3) | S, ~50; data-gated off |
| let-pass (race 1) | race drive 0x77C2C0 | – | throttle cap × 0.8 near a car behind (§4.2) | XS once the conveyor exists |

## 6. Live cross-check (earlier read-only dumps) [V live]

- `drivercfg.bin`: mkNumBrakeLinesBeforeMistakes 3, generateMistakes 0, trafficBehaviour 0,
  wheelLockedSpeed 1.0.
- Mistake slots: as in §2.2. No pending or active mistake in the snapshot. Spin duration 3,
  other duration 5.
- Pacing block: A = 1.0 for 11 drivers and **1.1 for one** (conveyor forward). B = 0. Backward
  flag 0. Damage gate 0. Request grips 1.0 (touring racingGripMultiplier). `req+0xB4` 0.

## 7. Port plan for `crates/grid-tools/src/ai_driver.rs`

Order by gameplay impact; every step keeps solo/no-player runs bit-identical (all factors 1,
no reference car → no events).

### 7.1 Data

- `grid_formats` readers, private-file driven like the existing AI data:
  - `catchup.xml` configs (§4.1 table; select by an explicit user name, default `rd_3_star`
    AUTHORED until the mode's name is traced);
  - `ai_choreographer.xml` set by discipline: manager, conveyer_belt, events, triggers with
    positions.
- `ai_vehicle_config`: mistakePowerSpinRearGrip, mistakeBrakeLockGrip,
  mistakeEntrySpinRearGrip, crash*, racingGripMultiplier.
- Driver config: generateMistakes (false), mkNumBrakeLinesBeforeMistakes (3),
  coLetPassDistance (30), coLetPassThrottleRatio, crMaxTrackCurvature, df*, bl*, vl*.
- Mappings 33–40 (mistake_* and visibility_loss_tolerance_ratio). The Mistakes
  characteristic is already loaded (F297).

### 7.2 Vehicle boundary (new, small)

Per AI car per tick, after the AI update and before the vehicle step:
- `wheel(w)+0x80 = request.grip[w] × manager.grip_factor` (wheel order RL, RR, FL, FR =
  req +0xB8..+0xC4);
- `engine+0xE8 (vehicle +0x1888) = clamp(catch-up × manager.power_factor, 0.1, 2.0)`;
- `+0x21C0 = crashInertiaFactor` while flipping, else 1.0.

These are fields the ported vehicle already consumes (`wheel_history` grip_scale,
`vehicle_propulsion` e+0xE8, suspension +0x21C0). The player car is never written.

### 7.3 Race manager (new `ai_pacing` module or `race.rs` extension)

- Reference = the player car (or a designated car in headless validation).
- Race distance from `race::Standings` (AUTHORED: AI-line distance instead of the
  progress-track distance; document the difference).
- Per tick, in the original order:
  1. reset;
  2. conveyor (only for grids ≥ min_grid_size; one slot [I count]);
  3. choreographer triggers;
  4. race-start/assist;
  5. then catch-up (§4.1).
- Expose to `AiDriver` per car: `a` (corner-speed scale), `b` (floor), `backwards` (let-pass
  and block suppression), `damage_gate`, and the pending-mistake commands.

### 7.4 Mistakes in `AiDriver`

- `MistakeSlot { pending, pending_timeout, active, timer, wheel, line_id, counter }` with the
  per-driver RNG (AUTHORED LCG as elsewhere, labelled).
- Update after the behaviour drive. On a new brake line: clear active 0–3/6. Random draw only
  if `generate_mistakes` (false by default).
- Activation hooks:
  - brake-line trigger (2/3) and brake-line braking (0/1/6) in the existing brake-line code;
  - brake_spin in the controls when brake-line braking with a speed excess in (8, 12);
  - powerout_spin in the throttle limiters when past the line, prev throttle > 0.6 and
    v > 10.
- Effects:
  - early/late trigger factors (pending);
  - heavy/light pedal floor/cap (active);
  - powerout forces throttle 1 as a max after the limiters;
  - grip overrides;
  - cancel below 10 m/s.

### 7.5 Behaviours

1. lost_control (crash/spin/wobble) with failure bytes from blow_tyre.
2. race_damaged + damage retire gated by blow_engine (engine damage stays a recorded value
   until engine damage is modelled; DAMAGE_BEHAVIOUR_CHECK notes damage does not yet cost
   power).
3. controlled_stop.
4. visibility_loss once a smoke value exists (AUTHORED: the choreographer's `tyre_smoke`
   flag of the car ahead).
5. block and draft behind their (false) test flags.

### 7.6 Validation (headless `grid-opponent-race`, plus unit tests)

1. **Regression**: solo laps bit-identical; 12-car run without a reference car identical to
   F301 except for catch-up/assist when explicitly enabled.
2. **Unit**: catch-up power tables:
   - `rd_3_star`: gap 50 → 1.0, 125 → 0.75, 200 → 0.5;
   - behind 50 → 1.0, 225 → 1.1, 400 → 1.2;
   - useLead extension;
   - clamp 0.1–2.0.
   Off-line assist: 2 m inside/outside at κ 0.01 → 1.125 / 1.25. Race-start factors by
   distance.
3. **Unit**: mistake activation windows; durations 3/5 s; 60 s pending timeout; brake_lock
   wheel range [0, 4); new-line clearing; cancel < 10 m/s; weights pick order with the live
   weights.
4. **Scenario**: player = car 11 at the back, 12 cars, 3 laps, fixed seed.
   - Trace per car A, B, grip factor, power factor, conveyor state/place and choreographer
     armings/windows/candidates/fired events.
   - Expect:
     - one car cycling forward to places 2–3 and back;
     - opening-lap grip 1.25;
     - major/minor errors firing on cars 0–250 m ahead of the player;
     - spins handled by recovery.
   - Compare contact counts and finishing spread with F300/F301.
5. **Scenario**: forced blow_tyre at 60 m/s → lost_control crash mode → retire; at 30 m/s →
   wobble at 15 m/s → retire.
6. **Live** (read-only, extend `tools/trace_grid_ai_avoid_recovery.ps1`): sample
   drv+0x3100..+0x3150 and +0x3150..+0x3170 and participant +0x20/+0x24 per frame over a
   race.
   - Confirm conveyor states and places, race-start factors, catch-up powers (engine +0xE8)
     and choreographed mistakes, including the catch-up user name.

## 8. Open questions

1. Overtaking side checks require record state 0. Since live AI cars are state 1, do they
   apply only to the player's car? Re-check AI_OVERTAKING §5–§7 against 0x406F50.
2. The caller of the car-adapter failure setter 0x40B640 and its link to `wheel+0x7C`.
3. Which catch-up `user` name a Normal single race passes, and game +0x340.
4. The conveyor slot count, slot setup, 0x4027B0 threshold rule and the place convention
   (0- or 1-based).
5. own+0x140 / 0x645940 (visibility value) and whether tyre smoke from events feeds it.
6. Virtual_Performance consumer 0x9C7E30 / slot `+0x4C0`.
7. Lap length used by the choreographer (`+0x80→+0x70`) and the meaning of participant
   status 4.
8. The race_damaged offset reference (`req+0x40` limit line).

## 9. Evidence (private)

- **Decompilation** (Ghidra 12.1.4 headless `-readOnly -noanalysis`, script
  `local/ai-choreographer/scripts/GridRangeDecomp.java`, runner
  `local/ai-mistakes/scripts/decomp.ps1` with `-Project` for the idle copy):
  - `local/ai-mistakes/decomp/m1`: 0x5C5B50, 0x77A450, 0x794310, 0x90A590, 0x911630,
    0x911690, 0x9134F0, 0x916730, 0x9169D0, 0x4061D0;
  - `m2`: 0x911530, 0x90DA20, 0x5C5B50;
  - `m3`: draft 0x77E0F0/0x789D90/0x789DD0; block 0x77D8D0/0x788E70/0x792B90/0x77B690/
    0x77B700/0x788F10; race_damaged 0x78A690/0x799FB0/0x797F00; controlled_stop
    0x78AA50/0x78AA90; visibility_loss 0x77E8A0/0x78CBD0/0x78CC10/0x7776F0; tests
    0x77EC80/0x77ED00; retire 0x78B300/0x79C6E0/0x79A090; 0x98DFE0.
- **Reused**:
  - `local/ai-choreographer/decomp/c1` (0x401000–0x40C000 and 0x909000–0x90C000: event,
    trigger, manager and conveyor functions);
  - `local/ai-research/decomp/drv1..drv8`, `map5`, `ra2`;
  - `local/ai-avoid-recovery/decomp/a1..a3`.
- **Disassembly**: `local/ai-mistakes/glue.dis` (0x401000–0x40C000), `cu.dis`
  (0x908000–0x91C000), `race5b.dis` (0x5B0000–0x5D0000).
- **Helpers** (`local/ai-mistakes/scripts/`):
  - `pe.sh` wraps the hash-guarded `pe.py`, `q.py`, `calls.py`, `fl.py`, `strs.py`;
  - `names.py` (index table);
  - `vt.py` (vtable search);
  - `substr.py` (string search);
  - `holder.py` (mapped-holder reads);
  - `live.py` / `live2.py` (re-decode the F290 dumps).
- **Data**: `local/ai-choreographer/xml/{ai_choreographer,catchup,ai_behaviour_fire}.decoded.xml`
  and `local/binxml/captured-dependencies/decoded-009/010/011.xml`.
