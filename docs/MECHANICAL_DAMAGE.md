# Mechanical damage — F333

16 October 2026. Owner goal: crashes should cost performance the way they do in the original.
Static evidence: hash-pinned `local/compatibility/grid-unpacked.exe`
(SHA-256 `aac46a64…4514`), private outputs in `local/mechanical-damage/`.
Tags: [D] read from the code, [V] checked by a run or test, [I] interpretation, AUTHORED port choice.

## 1. Summary

The mechanical damage chain was already ported before F333, and it was already active in the
playable build. This corrects the F281 note in [DAMAGE_BEHAVIOUR_CHECK.md](DAMAGE_BEHAVIOUR_CHECK.md),
which said that a crash could not yet cost power.

| Stage | Original | Port | Status |
| --- | --- | --- | --- |
| Collision → component levels | body handler 9EF360 → dispatcher 9E6660 → region helper 999C00 | `body_impact`, `collision_damage`, `damage_region` (F241/F243) | active in races since body events were enabled |
| Landing / wheel impacts → levels | 9F3E80 wheel loop → 9E6660 (wheel mode) | `vehicle_handling::calculate_owned` | active |
| Wear, engine temperature, gearbox history | 9F3E80, 9C4B70, 9C4EB0, 9C5250 | `vehicle_handling` (F191) | active |
| Levels → effect words | 9C5B90 | `vehicle_handling::effects` | active |
| Effect words → driving | copier 9BD7F0 → force pipeline | `vehicle_sync` COPIES → `vehicle_propulsion`, `vehicle_controls`, `vehicle_driver_prepare` | active |
| Damage option `realistic_damage` | FUN_00654C20 | **new** `mechanical_damage::Damage`, `Assists::damage` | F333 |
| Damage off (mode 0) | B14 = 0 → 9F3E80 returns at once | **new** early return in `vehicle_handling` | F333 |
| HUD damage icons | FUN_008EEF60 / FUN_008EEB80, `object/dial_damage` | **new** `mechanical_damage::indicators`, grid-app `damage_hud` | F333 |
| Detached-part queries (exhaust, rear spoiler) | 98BFE0 records from the parts object | not wired (`None`) | open |

The owner's playtest logs already show it: `local/playtest/manual-19590-14358.log` ends with radiator
level 0.958, and `manual-14216-20708.log` shows engine power factor 0.993 and cut-out factor 0.024 [V].

## 2. Components and what they do (Standard.hdm, Lacetti)

Field names are the HDM attributes (offsets in `grid_formats::handling_damage_fields`). Effects are the
9C5B90 words at mutable `+3A0..`; `ramp(level, t) = 0 if level < t, else (level − t)/(1 − t)` clamped.

| Component (level) | Collision gain (`damageScale`) | Effect | Consumer | Lacetti |
| --- | --- | --- | --- | --- |
| Radiator (+8) | 0.1792 | cap +3A0 = 0.8 × ramp(radiator, 0) | engine temperature target | **active** |
| Engine temperature (+C) | none (rises toward the cap at 0.007/s × gain) | power +3A4 = 1 − 0.30 × ramp(temp, 0.3) − exhaust − turbo; cut-out +3B8 = 0.8208 × ramp(temp, 0.2948) | +22C8 drive force (propulsion), +22C4 random cut-outs | **active** |
| Gearbox (+14) | 0.4046; plus over-revving 0.03/s × +18C4; per change 0 | +3C0 = 1.0 × ramp(gearbox, 1.0) | +2280 gear change (vehicle_controls) | only at level 1 |
| Driveshaft (+0) | 0 | +3A8 = 1 − 0 × … | +2264 wheel torque | none |
| Turbo (+4), exhaust (+10) | 0 | power terms | — | none |
| Brakes (+38.., per wheel) | wear from brake force > 999 999 | +3D8 = 1 − 0 × … | +2284.. brake force | none (max loss 0) |
| Suspension (+28.., per wheel) | 0.2312 | +3C8 = 1 − 0.4046 × ramp(level, 1.0) "spring loss" | **no reader** [D] | HUD only |
| Wheels (+18.., visual) | from deformation (F131) | wobble, rotational resistance, steering caps | prepared frame, steering | **active** (F281) |
| Rear spoiler | detached part query 0x45/0x46 | +3BC = 1 − 0.1688 | +2268 aerodynamic force | not wired |
| Front spoiler | query present | +3A0 += 0.1948 × 0 | — | none [D: multiplied by zero] |

So on the Lacetti a crash costs **engine power and causes cut-outs through the radiator**, damages the
gearbox (felt only when it reaches 1.0), and bends wheels (wobble, drag, steering). It does **not** cost
brakes, driveshaft or turbo power: Standard.hdm sets those to zero in the original data too.

Thresholds: a body contact damages only when the solved impulse exceeds mass × `minCollisionAccToCauseDamage`
(1140 × 2.5); the normalized impact is impulse / (mass × `maxCollisionAccToCauseDamage` = 22), capped at 1.
Wheel/landing impacts use `min/maxLandAccToCauseDamage` (0.8 / 8.5). Each component region (NED boxes,
`impactSphereRadius` falloff) draws one RNG sample for minor (×0.75) or major (×1.25) damage
(LOCALIZED_COLLISION_DAMAGE.md, COLLISION_DAMAGE_DISPATCH.md).

The suspension "spring loss" word (+3C8 + 0x18·wheel) is written by 9C5B90 but has no reader: a sweep of
every instruction for car offsets D1B8/D1D0/D1E8/D200 finds none, the verified copier 9BD7F0 does not copy
it, and the mutable-relative `+3C8` hits are in other objects [D, linear sweep `scan-01.json`]. The
per-wheel wobble angle +3D0 (car D1C0 + 0x18·wheel) is read by 9FBCB3 in the wheel presentation.

## 3. The damage option (FUN_00654C20) [D]

The options applier also handles the driver aids (DRIVING_ASSISTS_RESEARCH.md). For the local player car
only (`FUN_009EE380(0)`), with `v` the profile value of `realistic_damage`:

- mode = 1 if v = 0, 2 if v = 1, else 0; stored at car +155C;
- mode 0: car byte +B14 = 0; mode 1 or 2: +B14 = 1 and mutable +430 = 1.0 (CAF888) or 2.0 (CB8DA4).

The profile defaults builder FUN_005F1100 stores `realistic_damage` = 1, so a fresh profile races with
gain 2.0. The language file names the option "Realistic Damage" with the help text that switching it on
adds a reputation reward; on and off are the only single-player values [I: the toggle stores 0/1].
There is no "cosmetic" level.

Mutable +430 multiplies every damage gain: collision channels use (+438 × +430) × +434, and wear,
engine temperature and gearbox use the same product. +434/+438 are 1.0 from the outer constructor
and no other writer was found. AI cars never pass through FUN_00654C20 and keep gain 1.0 and B14 = 1
from the constructor (A0F2D3 stores word +B13 = 0x0101).

Mode 0 (B14 = 0):

- 9F3E80 checks +B13 and +B14 first and returns (9F3EAC..9F3EB7): no wear, no temperature, no effect update;
- the body handler 9EF360 skips 9E6660 (9EF400 loop);
- 9BD7F0 stores +C7AF = 1, which turns the bulk deformation off (deformation_callback_state);
- the loose-part callback 9A7670 returns.

So mode 0 is a fully undamageable car. Single player cannot select it. The network session override
FUN_006572D0 does: when the session's damage flag (+1B4) is 0 it sets every car to B14 = 1, gain 2.0
and the body-impact scale global E4FC30 to 0.75 (CAFE64); otherwise every car gets B14 = 0 and C7AF = 1.
The body handler multiplies each solved impulse by E4FC30 before 9E6660 (9EF415), which is 1.0 offline.

Port: `grid_world::mechanical_damage::Damage {Off, Normal, Realistic}` (`apply` writes +155C, +B14, +430)
and `Assists::damage`, parsed from `damage=realistic|normal|off` (also `realistic_damage=on`) in
`--assists`/`--assists-file`. Default `Realistic`, the original profile default. `vehicle_handling`
now returns its input unchanged when +B13 or +B14 is 0, as 9F3E80 does. Old app logs without
`damage=` replay with `Normal`, the gain those builds used (soak log parser).

## 4. HUD damage icons [D]

`Osd/sceneconfig.xml` attaches `object/dial_damage=damage` with animated nodes engine, gears,
suspension and steering; `Osd/dial2.pssg` also has `damage_wheels` states. FUN_008EEF60 (and the
change-tracking twin FUN_008EEB80) computes eight inputs from the mutable levels:

| # | Icon | Value |
| --- | --- | --- |
| 0 | engine | engine temperature level +C |
| 1 | gears | gearbox level +14 |
| 2 | steering | (0.2 × max front suspension + 0.8 × max front wheel level)², fronts = wheel record +8 > 0 |
| 3 | suspension | mean of the four suspension levels (summed last wheel first) |
| 4..7 | wheels | wheel levels +18.. (RL, RR, FL, FR) |

Each value becomes a stage: 0 up to the low threshold, 0.2 up to the middle, 0.7 up to the high, else 1.0.
Components use 0.05 / 0.4 / 0.75 (CAFE60, CB22A4, CAFE64); wheels use the globals E4FD4C/E4FD50/E4FD54 =
0.3 / 0.65 / 0.8 (image defaults; no writer found). The function returns whether the summed stages exceed
1e-4 (the script variable `anyDamageIndOn`). The pow call is CRT pow(x, 2.0) (CB9B78), exact as x·x.

FUN_0050A200 is a related per-component "health" getter, 1 − round(level × 100)/100, for eight types
(engine temperature, gearbox, driveshaft, exhaust, radiator, suspension mean, wheels+brakes mean, turbo);
no reference to it was found. The other level readers are car presentation: engine steam and smoke
(453C50, engine ≥ threshold), gear-damage clunk on shifts (43BED0), turbo misfire (44D840), brake
glow (453A70), a total-damage percentage (424650) and the network sync packer (91D4B0).

Port: `mechanical_damage::indicators` (unit-tested) and grid-app `damage_hud`. AUTHORED presentation:
a panel with the eight labels coloured by stage (yellow, orange, red), hidden while no icon is lit;
the original dial geometry and animation are not reproduced.

## 5. Parts that change handling (not wired)

9F3E80/9C5B90 query the parts object (98BFE0) by part id, status 4 or 5 meaning detached. The ids come
from the name table at E4EF20 [D]: 0x1F = `exhaust` (forces exhaust level to 1; Standard exhaust loss is 0,
so no effect on the Lacetti), 0x45 = `spoiler_b_2` and 0x46 = `spoiler_b` (rear spoiler gone: +3BC =
1 − `rearGripPercentLoss` 0.1688, applied to the aerodynamic force through +2268). The port passes no query
object, so a detached rear spoiler does not yet cost downforce. Doing this needs the part decisions of
9EC9B0 (today presentation-only in grid-app `damage_parts`) in the simulation, and the spoiler detach rule
(9AE8F0/9E2AD0/9AEC60), which is not analysed.

## 6. Validation

Unit tests [V]:

- `mechanical_damage::tests`: option stores of FUN_00654C20 for all three modes (including the
  short body copy), icon thresholds with boundary values and NaN, and the 8EEF60 inputs (rear
  wheels excluded from steering, last-wheel-first suspension sum, exact square).
- `vehicle_handling::tests::inactive_damage_returns_unchanged`: with +B14 = 0 an above-threshold
  wheel impact and a gear change leave car, physical body and damage RNG unchanged; with +B14 = 1
  the same input changes the car.
- `assists::tests::default_parse_display_and_bytes`: `damage=` parsing, display round trip and bytes.

Scripted crash [V] (`grid-opponent-race` release, `ROOT CD01 BP01 1 11200 ai`, solo Donington AI;
`GRID_CRASH_CAR0=1500:25080:6` = six full frontal hits through 9E6660 before tick 1500, impulse
25080 = mass × maxCollisionAcc; exe SHA-256 `9dbd31e6…6410`; outputs `local/mechanical-damage/runs/`):

| Run | Radiator | Engine temp at 5000 / cap | Power factor | Cut-out factor | Lap 1 | Lap 2 |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| no crash | 0 | 0 | 1 | 0 | 88.4203 | 86.2823 |
| `GRID_DAMAGE_CAR0=off` | 0.665 (see below) | 0 / — | 1 | 0 | 88.4203 | 86.2823 |
| normal (gain 1) | 0.665 | 0.392 / 0.532 | 0.901 | 0.276 | 88.7038 | 88.1740 |
| realistic (gain 2) | 1.000 | 0.784 / 0.800 | 0.786 | 0.588 | 90.3894 | not finished by tick 11200 |

The temperature rises by 0.007 × gain per second, exactly as the HDM rate says (0.112 per 500 ticks at
gain 2). The frontal hits also reach the front suspension regions (0.22 / 0.44) but, as in the original,
that has no driving effect. Off mode: the hook calls 9E6660 directly, so it still writes the levels (the
body handler 9EF360 would skip the call with +B14 = 0), but 9F3E80 never runs, so the temperature stays 0,
the effects keep their reset values and the laps and final position equal the clean run.

Clean laps [V]: the default `grid-opponent-race` solo run is byte-identical to F324's
`solo-after.txt` (SHA-256 `124acff5…2d9a`): laps 88.42029812185638 / 86.28228501935392. No collision
means no damage; the AI car keeps gain 1.0 because the option is applied to the player only.

Regression [V]: full workspace tests 380 passed, 0 failed after rebasing on dc2d8ff; Clippy
`--workspace --all-targets -D warnings` clean; `grid-soak quick` shows only the known findings 1 and 4
(all five Donington scenarios, now with damage=realistic for the player, pass).

## 7. Not done / open

- Detached rear spoiler → rear downforce loss (section 5) and exhaust detachment queries.
- The real dial graphics (`Osd/dial2.pssg` icon animation) and the OSD "Damage Icons" option.
- Effects at a threshold of exactly 1.0 (gearbox, suspension, brake in Standard.hdm) reach 0/0 when a
  level caps at 1.0; the original also stores NaN there (COLLISION_DAMAGE_DISPATCH.md), while the port
  rejects nonfinite effects. This is the likely cause of soak finding (2) and is left to the soak fix.
- No live comparison with the original game (no input was sent to it in F333).
- The playable was not rebuilt. Its player now defaults to Realistic Damage (gain 2.0), the original
  default; `--assists damage=normal` restores the earlier gain 1.0.
