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
| Detached-part queries (exhaust, rear spoiler) | 98BFE0 records from the parts object | `None`; the spoiler never detaches in single player (F339) | equal to the original |

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
| Rear spoiler | detached part query 0x45/0x46 | +3BC = 1 − 0.1688 | +2268 aerodynamic force | unreachable in single player (F339) |
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

## 5. Detached parts and driving (F339)

Static research, 16 October 2026 (Ghidra 12.1.4 headless `-readOnly -noanalysis` on `local/phase3/ghidra-project`,
JAVA_HOME = the Songs of Syx Demo JRE as in `tools/run_ghidra_probe.ps1`; capstone sweeps with
`tools/scan_code_displacements.py`). Private evidence: `local/detached-parts/` (scan-01..06, dis-01/02,
ghidra-01..06, `part-names.txt` = the full E4EF20 id table).

**Every reader of a part state.** The lookup 98BFE0 (linear search of the 0x2E0-byte records at parts +8F0
for record +24C = id; 0 when the part never had a record) has these callers [D, literal sweep + aligned
disassembly + decompilation]:

| Caller | Ids | Use | Driving? |
| --- | --- | --- | --- |
| 9C5B90 effects | 0x1F exhaust; 0x46/0x45 rear spoiler | exhaust drag term; +3BC = 1 − `rearGripPercentLoss` | yes (ported, no query object) |
| 9C50C0 (exhaust normalise, 9F3E80 chain) and 9F3E80 | 0x1F | exhaust level +10 = 1 | yes (ported) |
| 9EC9B0 decisions, 9FF4F0 per-tick update | all | the part rules themselves | — |
| 9DB0E0 hinge builder | 0x0C/0x0D and others | hinge set-up; may queue rear corner 0x0C detach | no |
| 9E99B0 | 0x58/0x59 mirrors, 0x51 window_f | mirror view flags | no (presentation) |
| A18460 → +3A38 → 9CB5B0 | 0x0E bumper_bm, state 2 | scene-node lookup of the dangling bumper | no (presentation [I]) |
| 9F9BA0 | node loop | joint matrices of attached parts | no (rendering) |
| 920D30/920E90/920F00/920F70/921080, 928680 | stored id | position of a part's debris body (+64) | no (camera/effects [I]) |

No handling, aerodynamic, drag, mass or collision-shape code queries bumpers (12–17), bonnet, boot, wings
or doors. A detached part becomes its own debris body (record +64); nothing found removes mass from the car
body or edits its collision hull. **Bumper detachment has no driving cost in the original** [D within this
caller list; a hidden reader through a copied state would be outside it].

**The rear spoiler never detaches in single player** [D]:

- `m_spoiler_detach_level` (+1C88) is read only by the loader (9F11F8 `lea`); the F307 note that 9AE8F0,
  9E2AD0 and 9AEC60 read it was a displacement collision (9AE8F0 is a wheel-load split writing car
  +19E4/+1B34/+1C84/+1DD4 and divides by another object's +1C8C).
- No damage rule sets 0x45/0x46: the only immediates are in 9C5B90 and a developer bound dump 9A61D0;
  9EC9B0 has no spoiler rule; `m_spoiler_moves_with_boot`/`m_heirarchy_spoiler` (+1B84/+1B88) are node
  hierarchy flags.
- The generic setters cannot reach it: the 9FF4F0 loose timer needs state 2, which the spoiler never gets;
  the resets 9FF170/9FF3E0 set 4 and then 0 or the old state in the same call; 9D77B0/92E440 apply
  network state pairs.
- So the +3BC grip loss and the `SpoilerSeparation`/`SpoilerDangling` sounds (car audio 43CF10 +C4/+C8) are
  reachable only through network state or the state-5 list below.

**Statuses** [D setter 9DC270, I names]: 0 attached (record freed), 2 loose (hinged), 4 detached (debris body
created; mirrors on a door with `m_wing_mirror_move_with_door` follow), 5 detached without a transition.
State 5 is written only by 9EC950, which applies the id list at parts +890 (u32 ids, count at +A0). That
list is the constructor's third argument, the vehicle wrapper's +14 block, a 0xCC90-byte copy made in
A100A0 only when a source block is supplied; 9FF170 (full repair) and 9FF3E0 (re-attach) re-apply it.
[I] it is a restored vehicle snapshot (network or saved state), not race damage. A request queue (9AB790,
40 entries at +7C10, drained by 9DCFA0 before 9EC9B0) carries network, repair (5EC370) and hinge requests.

**The part that does change driving is the wheel.** 9EC9B0 sets car bytes +22E4..+22E7 when a wheel zone
exceeds `m_wheel_detach_level` × S (Lacetti 6 × 4560 = 27360 per tick); 996F60 is a second setter and
5EC370 clears them. The flags are already consumed by the port (`wheel_contacts` skips the wheel,
`vehicle_sync`/`vehicle_handling`/`vehicle_manager` as in 9BD7F0/9F3E80), and 9E65B0 turns a detached
wheel into terminal damage (9E64E0(4, wheel)). The producer is not ported: grid-app `damage_parts`
excludes wheels and is presentation-only, so no port car ever loses a wheel. This is the open follow-up.

**Wheel detachment port scope (F340).** The owner chose to record the scope now and port it after the
weekly usage reset on 19 October. No code was changed.

The producer is small:

- the 98B430 zone classifier, today in grid-app `damage_parts::classify`, moves to the simulation;
- the 9EC9B0 wheel rule: when zone +824..+830 (RL, RR, FL, FR) exceeds +1C3C `m_wheel_detach_level` × S,
  with S = physical mass × `m_overall_speed_required_scalar`, it sets car byte +22E4 + i once;
- the rule is gated by `m_disableLooseParts` = 0 (9EC9B0 jumps past every rule), parts +7BF1 = 0 and car
  B13/B14 non-zero (9A7670 forwards notifications only then);
- the loader seeds +1C3C with 2 × the bonnet detach level before reading the attribute.

What is not needed:

- 996F60 is a trivial setter with no direct caller;
- +7BF1 is set only by game-mode code (70558E, 70574E, 70D316) and cleared by repair 9FF170, so it stays 0.

The producer cannot land first. A set flag today pauses the run, because the port refuses a detached
wheel as an unsupported branch ([RUNTIME_LIMITS_AUDIT.md](RUNTIME_LIMITS_AUDIT.md) B11). The
detached-wheel branches of these original functions must be recovered and ported first, in this order:

1. `suspension_vehicle` (9F1E10): `suspension_vehicle.rs:78` refuses +2278 ≠ 0.
2. `vehicle_driver_prepare` (A03670): `vehicle_driver_prepare.rs:63` refuses +2278 ≠ 0.
3. `vehicle_handling` (9F3E80): `vehicle_handling.rs:266` refuses +22E4 ≠ 0. Also the terminal check
   9E65B0, called at 9F4246: with status +444 = 7, the first flagged wheel calls 9E64E0(4, wheel). The
   port only has the 9F2F80 transition to status 3.
4. The wheel update 9FA6E0: with +22E4 + i set it skips the wheel's normal processing and sets the
   wheel/tyre parts to state 4. Its port counterpart has not been identified yet.
5. Check the remaining readers 9F6ED0, 9BAB10 (constraint producer, CAR_MANAGER.md) and 7E9D60.

Already handled in the port: `wheel_contacts.rs:146` skips the wheel, `vehicle_sync.rs:200-203` sets +2270
and copies +2278, `vehicle_controls.rs:133` gates the brakes, and `vehicle_manager.rs:45` gates the
constraint check. These cover 9BD7F0 and the wheel contacts.

After the port, grid-app `damage_parts` should read the simulation's flag instead of a rule of its own. The
validation planned for the port:

- unit tests of the zones and the rule;
- a scripted heavy corner hit through `grid-opponent-race` that loses a wheel;
- the solo laps, `grid-soak quick`, Clippy and the full tests.

Evidence: `local/detached-parts/` (ghidra-02..06, scan-01..07).

Other notes: the exhaust query uses id 0x1F only (`exhaust`); the Lacetti's `exhaust_bl` is 0x65 and its
exhaust loss is 0, so no Lacetti effect. 9FF4F0 also detaches a loose part after a time while car +1648 is
above a threshold (constants CB093C/CB51B0/CB0D94; bumper_bm and mirrors use their own pair), and on
car +C8A0 (windscreen smashed) detaches the wipers 0x5C/0x5F; neither is in grid-app `damage_parts` yet.

Port decision: nothing changed. The handling query stays `None`, which equals the original for every
single-player car whose `exhaust` (0x1F) part does not detach.

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

- Wheel detachment (+22E4 flags from 9EC9B0) is not produced in the simulation; the port order is in section 5 (F340); the exhaust (0x1F) query for cars that have that part.
- The real dial graphics (`Osd/dial2.pssg` icon animation) and the OSD "Damage Icons" option.
- Effects at a threshold of exactly 1.0 (gearbox, suspension, brake in Standard.hdm) reach 0/0 when a
  level caps at 1.0; the original also stores NaN there (COLLISION_DAMAGE_DISPATCH.md), while the port
  rejects nonfinite effects. This is the likely cause of soak finding (2) and is left to the soak fix.
- No live comparison with the original game (no input was sent to it in F333).
- The playable was not rebuilt. Its player now defaults to Realistic Damage (gain 2.0), the original
  default; `--assists damage=normal` restores the earlier gain 1.0.
