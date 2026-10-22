# Dynamic and breakable trackside props (research)

15 October 2026, Claude Code. Static research only: purchased Donington data
(`route_0/objects.ens`, `objecttypes.pssg`, `raceload.jpk#physparams.xml`) and
read-only Ghidra 12.1.4 decompilation of the hash-pinned analysis image
(SHA-256 `aac46a64…f34514`) from a private copy of the shared Ghidra project
(`local/dynamic-props/ghidra-project`; the shared one was locked by another session).
The original game was not run. Evidence: `local/dynamic-props/` (decompilation
`decomp/a1..a14`, schema table, `classification.txt`, `objecttypes-tree.txt`,
`objects-ens-tree.txt`, helper scripts).

Labels: **[V]** read directly from purchased data or verified in the port;
**[D]** read from decompiled original code (static, not executed);
**[I]** interpretation.

## Summary

Every route entity starts **static** (infinite mass). A shape may carry a
*behaviour* with an impulse threshold; contacts against that shape are capped at
the threshold, and when a contact saturates the cap the entity manager swaps the
entity's components after the physics step: the intact static part is deleted and
dynamic pieces (finite mass, rbDefault body properties) are created in place.
Nothing despawns during a session; the entity manager resets on restart.

| Template (route_0 count) | Intact shape (static) | Threshold | Broken state | Constraints |
| --- | --- | ---: | --- | --- |
| `gg_brake_marker_01_i_*` (34) | box 0.94 x 0.96 x 0.16, sCardboard | bPhantom 50 | two dynamic cardboard boards 0.94 x 0.47/0.46 x 0.16 (4.9 / 4.8 kg) stacked | none |
| `tyrestack_black` (8) | box 1.83 x 0.94 x 0.60, sRubber | bRubber 50 | 12 dynamic tyres (cylinders r 0.28-0.29, h 0.14-0.30, 24-56 kg) in 3 columns of 4 | none |
| `cushion_blue` (148) | box 2.17 x 0.89 x 0.74, sPlastic | bLight 250 | one dynamic box of the same size (996 kg) | none |
| `tyre_wall` (588) | box 4.01 x 1.13 x 0.99, sDeformableRubber | bDeformableRubber 1000, explosion 0.35 | 6 dynamic blocks 1.41 x 0.94 x 0.46 (428 kg) + 16 loose tyres (17-19 kg) | 7 spring joints between the blocks |
| `armco_white_main` / `armco_blue_main` (123 / 120) | box 3.96 x 0.74/0.96 x 0.09, sArmco | bArmco 1000 | 4 rail sections (134 / 174 kg) + 1 post (13 / 17 kg) | 6 joints incl. post-to-world (breaks at 10) and rail override bodies; neighbouring armco linked through 238 ENS constraint links |
| concrete walls/blocks, `chnfen*`, `bridg_sponsor_*`, `stlight_a`, `stone_sink_a`, `superchips_a`, `gg_ad_brd_minor_fence_*`, `clock_st_a` | static boxes / cylinders | none (no shape behaviour) | never | - |
| `flag_a` / `flag_b` (24) | cloth only | - | - | - |

[V] from `objecttypes.pssg` + `physparams.xml` (`classification.txt`). Masses are
density x volume (below).

## Data model

- [V] Each `TEMPLATEENTITY` has an initial `TEMPLATEBEHAVIOUR` (`impulse =
  physparams.xml#bDefault`, 1e7) whose `typeuri` names a `TEMPLATEBEHAVIOURTYPE`
  with ordered `TEMPLATECOMPONENTCHANGE add/delete` records. Applied to an empty
  set this gives the initial components (already used by the renderer,
  `ENTITY_COMPONENTS.md`).
- [V] The intact subentity's rigid body has `infMass = 1`. Its collision shape
  carries a nested `TEMPLATEBEHAVIOUR` with `impulse = physparams.xml#bXxx` (the
  threshold) and `typeuri = #..._damage_behaviour`. The damage behaviour type deletes
  the intact subentity and adds the damaged subentities (and constraints).
- [V] Damaged subentities have `infMass = 0` and `props = physparams.xml#rbDefault`;
  their shapes reference `TEMPLATESHAPEPROPS` (material + `massSource`).
- [D] `0x7C0F20` validates every template: an initial behaviour must exist, must have
  properties, and every component it adds must be infinite-mass ("initial behaviour
  for %s has a non infinite mass default state"); shape behaviours must reference valid
  properties. So the initial state is always static.
- [D] Property schema (`.data` 0xE4D2xx-0xE4D7xx: name, attribute id, type):
  behaviour props `impulse` (+0x2C) and `explosion` (+0x30) (`0x7B9C40`); shape props
  `massSource` (+0x2C, +0x58 = "d" density flag, `d+inf` = FLT_MAX), `collisionDetection`
  (+0x30), `softMaxImpulse` (+0x34), `material` (+0x44), `corematerial` (+0x4E),
  `bodyOverride` (+0x5C) (`0x7D24D0`); rigid-body props (`0x7D0F40`) +0x30..0x38
  maxAngVel, +0x40 angDamping, +0x50 forwardAirRes, +0x60 backwardAirRes, +0x70
  sphericalise, +0x74 sphericalmass, +0x78 sphericalradius, +0x7C linearStationary,
  +0x80 angularStationary, +0x84 wakeUpForce, +0x88 wakeUpTorque, +0x8C turnOffTime,
  +0x90 maxLinSpeed, +0x94 linDamping, +0x98 gravityScale, +0xA0 uprootProps.

## Break mechanism

1. **Threshold into the shape.** [D] Runtime shape construction `0x7C6350` takes the
   threshold from the shape behaviour: behaviour +0x3C if >= 0, else its props
   `impulse` (+0x2C), else 1000; shapes without a behaviour get FLT_MAX. It is written
   to shape descriptor +0x6C. [V] The ported shape constructor maps descriptor +0x6C to
   shape +0x224 (`collision_mesh.rs`), and the verified original mesh-contact
   generator writes the contact limit, `min(shapeA+0x224, shapeB+0x224)` when one side
   has byte 0x228 or the 0x17/0x18 flag pairing, into contact **+0x3C**
   (`MESH_CONTACT_GENERATION.md`, `tools/validate_mesh_contact.py`). The solver caps a
   contact's normal impulse at `dt x contact+0x3C` (`contact_rows.rs`), and after the
   solve stores the solved impulse divided by dt back in +0x3C.
   [D] (F557) Which pairs get the threshold. `B0EE80` takes `min(+0x224)` only when
   `A.0x228 || B.0x228 || (A.0x17 && B.0x18) || (B.0x17 && A.0x18)`, else FLT_MAX.
   Entity shapes (`0x7C6350`): `0x469A80` zeroes descriptor bytes +4/+5/+6 (shape
   0x17/0x18/0x216), `0x7B9390` then sets +5 (0x18) = 1 unless props byte +0x5D, sets
   +6 for static or heavy bodies and clears dword +0x60 (shape 0x228). So every prop
   and piece shape has 0x17 = 0, 0x18 = 1, 0x228 = 0. The car's shapes (`car_box`
   0xA01960, e.g. 0xA026BD / 0xA02816 / 0xA01C94) set descriptor byte +4 = 1 (0x17)
   before `0xA72350`. A car-versus-prop contact therefore carries the threshold; a
   piece-versus-prop or prop-versus-prop contact carries FLT_MAX.
2. **Callback.** [D] Shapes with a behaviour register `0x7C46A0` as their contact
   callback (`0x7C6350` tail, registration +0xC/+0x10). The event is built by
   `B0CFC0` (event+0x20 = contact normal, event+0x40 = contact +0x3C, event0/4 = side
   registrations). `0x7C46A0` first requires world byte +0xBC20 and the same flag
   pairing as `B0EE80` on the two sides' shapes (`0xA6A3D0` = shape 0x228, `0xA6A3F0`
   = 0x17, `0xA6A410` = 0x18; F557), so a piece never breaks an intact prop. Then,
   for each side whose shape has a behaviour, `0x7BDFD0` fires when
   `threshold <= 1.01 x event+0x40` (constant 0x00CBA8F0 = 1.01). It records the
   behaviour and the **other body's velocity projected on the contact normal**
   (`(v . n) n`, body +0x220), then queues the entity (+0xEA flag, list manager+0xBC08).
3. **Deferred swap.** [D] The entity manager update `0x7D64B0` (after physics) walks
   the queued entities and calls `0x7D5180` (apply behaviour type) with
   `explosion x (v . n) n` (behaviour props +0x30). `0x7D4C00` applies the component
   changes (`0x7D3C40` add/delete subentity); `0x7D4940` first ensures pool capacity
   (`maxSubEntityInstances`, `maxConstraintInstances`).
4. **New dynamic bodies.** [D] `0x7CDCD0` creates each added rigid body: infMass bodies
   use the static constructor; others build a descriptor from the rigid-body props
   (`0x7C1DD0` -> `0x7B8F40`, mapped field by field onto the ported B11F40 constructor
   settings: +0x90/94 wake force/torque, +0x98/9C linear/angular stationary, +0xA0
   turnOffTime, +0xA4..AC maxAngVel, +0xB0 maxLinSpeed, +0xB4..BC angDamping,
   +0xC0..D4 air resistance, +0xD8 linDamping, +0xDC gravityScale, +0xE0..E8 spherical,
   +0xEC 50) with descriptor mass 0 and identity inertia, so mass comes from the
   shapes (`0x7B9390` passes shape props mass value and its density flag). Shapes are
   attached by `0x7C6350`. If the replaced body was moving its motion is copied
   (`0x7CDCD0`, `param_5`). If the explosion velocity is non-zero, the new body gets
   `v_old + e + (r - (r . e^) e^) |e| x 0.25`, with `r` a pseudo-random unit vector
   seeded by the fractional part of `|x|+|y|+|z|+|e|` of the body position
   (constants 0x00CFAA48..60, 2 pi, moduli 0x1948/0x14F7/0xC54).
5. **Links.** [D] `0x7D5540` breaks the intact sections linked to a section a car broke
   (one level) and joins broken neighbours' rails to its post (ported F501,
   docs/JOINTS_RESEARCH.md "Phase 3b").

[I] Consequence: in the break step the car only feels the capped impulse
(`threshold x dt`: 0.8 N s for a board at 60 Hz, 16.7 N s for a tyre wall), then
meets the new pieces as ordinary dynamic bodies.

## Body properties used by the pieces (rbDefault)

[V] `sphericalise 0`, `linDamping 1`, `angDamping 1 1 1`, air resistance 0,
`linearStationary 0.3`, `angularStationary 0.3`, `turnOffTime 1`, wake force/torque 0,
`gravityScale 1`, max velocities `+inf`. Shape props: sCardboard `PLAS d70`,
sRubber/sPlastic/sDeformableRubber `PLAS d700`, sArmco `SIGN d2610`.
[I] `d` = density in kg/m^3. [D, F549] The body's mass is density x the volume of the
shape grown by its shell, with that shape's solid inertia (B11520 over the shape vtables;
BOX_CONTACTS.md "Cylinder pairs and piece mass"). Before F549 the port used the bare
template volume (boards 4.8-4.9 kg, tyres 17-56 kg, cushion 996 kg, tyre-wall block
428 kg); with the shell a board weighs 7.0 kg and a Nürburgring tyre 26.7 kg.

## Lifetime

[D] No timer or distance despawn was found for pieces. The per-entity timer in
`0x7D64B0` (`+0x15`/+0x10, `0x7BB800`) switches render state of the subentities
[I], not removal.
Pieces stay until the pool needs space (`0x7D4940` recycles, below) or the entity
manager is reset (`ResetEntityManager`, restart). [I] Resetting on restart.

## Pools and world bounds (F317)

15 October 2026, Claude Code. Static, Ghidra 12.1.4 headless `-readOnly -noanalysis`
on a private copy of the dynamic-props project (`local/prop-pool/ghidra-project`),
same hash-pinned image; pseudocode `local/prop-pool/decomp/b1..b7`. Prompted by the
owner's playtest of 7f22c32: a side-swiped board in a 12-car race stopped physics
("tick 2188: invalid contact-group counts or selector",
`local/playtest/manual-23606-10155.log`).

**Physical world.** [D] `0xA74F00` reads a `PHYSICSENGINE` node over the defaults set
by `0x6195F0`; `0xA749A0` builds the world from them. Defaults:
`maxRigidBodies` 1000 (pool world+0x24), `maxFixedObjects` 1000 (+0x20), `maxGeoms` 1000
(+0x28), `maxConstraints` 1000 (+0x2C), `maxBodiesInIsland` 1000,
`maxConstraintsInIsland` 1000, `maxDOFConstraintsInIsland` 6000, `maxContacts` 10000,
`maxExternalContacts` 1000, `maxContactsPerCollision` 20, `maxBroadPhaseCollisions`
10000, `numQuickStepIterations` 20. Each pool is a free list with capacity +0xC and
used count +0x10 (`0xAD9220` take, `0xAD9260` give back); a full pool returns no
object. [V] No file in the Steam install contains `PHYSICSENGINE` or `maxRigidBodies`
(only `grid.exe`), and the optional `physicsOverride.xml` named at `0x5C704B` is not
shipped. [I] The defaults apply: one world holds up to 1000 rigid bodies, an island
(contact group) up to 1000.

**Entity manager.** [D] `0x7BA850` reads an `ENTITYMANAGER` node with defaults
`maxShapeInstances`, `maxRigidBodyInstances`, `maxTransformableInstances`,
`maxRenderableInstances`, `maxSubEntityInstances`, `maxConstraintInstances` 1000 each and
`maxClothInstances` 100 (`0x7BA970` repeats them when no file loads). Pools sit at
manager +0xBC24..+0xBC38; `0x7B5A70` returns their free counts plus the world's four
(`0xA72470`). [I] Whether a route overrides these is open (Donington's objects.ens has
more than 1000 entities in total).

**Pool full on a break.** [D] `0x7D5180` (apply behaviour type) swaps components only
if `0x7D4940` returns true. `0x7D4940` compares the free counts with the entity's
needs (`0x7B6F40`/`0x7CF2A0` count the subentities, bodies, shapes and constraints to
add; `0x7D8520` requires every difference to stay non-negative). If short, `0x7D4770`
collects up to 1000 other entities (not the breaking one, not flag +0xEB) keyed by
squared distance to the manager's focus point +0xBC80 (`0x7B5FC0`), sorts them with
CRT `qsort` (`0xBF7BD0`) and comparator `0x7B6080` (farthest first), and restores the
first candidate whose reset frees enough (`0x7D6760(1,1)` -> `0x7D5180` with the
initial flag, i.e. back to the intact static state). It repeats while that succeeds;
if nothing frees enough the break is skipped and the entity stays intact. `0x5BB5C0`
writes +0xBC80 from the translation of a game-object matrix (+0x72C -> +0x24) every
frame [I: the viewed car]. There is still no timer despawn.

**Port bounds before F317.** [V] The port's world guards capped one world at 16 bodies
(`contact_groups` `1..=16`, `contact_rows`, `impact_frame`, `impact_step`,
`physical_tick`) and the solver at 64 (`main_contact_response`, `physical_response`,
`impact_correction`), with 256 contacts and 513 shapes. They were the validated domain
of the native comparisons (CONTACT_GROUPS.md, GROUPED_PHYSICAL_FRAME.md), not original
values. Audit: the algorithms use `Vec`s and one-based `u32` tokens; no bitmask, fixed
array, `u16` field or byte index depends on the body count (the other 16s are manifold
capacity, deformation grid dimensions and matrix sizes). 12 cars plus one board's two
halves (14) passed; the first tyre stack or a second board in a full field failed.
Solo play (1 car) never reached the cap, so F312's solo validation missed it.

**Port (F317).**
- `grid_world::MAX_WORLD_BODIES` = 1000 (maxRigidBodies; maxBodiesInIsland is the same,
  so the per-group solver guard uses it too), `MAX_WORLD_CONTACTS` = 10000 (maxContacts),
  `MAX_WORLD_SHAPES` = 513 + 1000 (the port's static table plus one shape per dynamic
  body; maxGeoms is 1000). Car contact pools keep their sizes (24 per car up to 256,
  AUTHORED as before); pieces still add 12 slots each, now beyond 256.
- `PropWorld::apply` refuses a break whose pieces would push the world past 1000 bodies
  (the entity stays intact and keeps its threshold). Deviation: the farthest-first
  restore of other broken entities is not ported. The 190 ported breakables yield at
  most 312 pieces, so with 12 (or 20) cars the bound cannot be reached.
- Per-group solve: `impact_frame::calculate_owned` / `impact_step::calculate_owned` move
  the shared row buffer instead of copying it 4 times per group; piece-piece pairs farther
  apart than sqrt(3) x (summed hull reach + margin) skip the face-axis test (a box or
  12-gon prism has an orthonormal triple of face normals, so that test returns nothing
  there); each piece builds its world hull for the car pair queries once. Results are
  identical (state hash below).

**Validation (F317).** `twelve_car_world_keeps_stepping_after_props_break_into_many_pieces`
(Donington, 12 cars at throttle 0.6, every tyre stack and two boards broken = 100
pieces, 400 ticks): before the fix it failed at tick 2 with the playtest's error; after,
12 -> 112 bodies, every piece within 1.5 m of its spawn height, final state FNV
`6e19b665dbf510b1` with and without the pair skip; 62 ms/tick at first, 38 ms with
cached piece hulls, 9.2 ms with the moved row buffer.

- `grid-opponent-race` (private `local/prop-pool/races/`): 12 cars, 12000 ticks without
  props: output byte-identical to the same tool built from cf0e72e (11,726 race ticks,
  140 car contacts). With `GRID_DYNAMIC_PROPS=1` the AI field breaks nothing in 12000
  ticks. New `GRID_PROP_TARGET=TEMPLATE:INDEX` drives car 0 with the authored
  `PropPursuit` while the 11 AI cars race: board `gg_brake_marker:0` breaks at tick 5314
  (force 50.0, as in the solo probe), also with every car deforming; `tyrestack:0`
  run, 20000 ticks: two stacks and a board break (26 pieces, 38 bodies), no failure.
- Solo `1 car 11200 ticks`: laps 88.42029812185638 / 86.28228501935392 s, identical with
  and without `GRID_DYNAMIC_PROPS`, 0 breaks.
- App, scripted window run with 11 AI opponents, deformation and body events
  (`GRID_PROP_SCRIPT`, `--drive-ticks`, renders in `local/prop-pool/windowed/`). The app now
  logs `dynamic_props event=break` per break; the 7f22c32 playtest log has no such line,
  so which breaks preceded its failure is not recorded. Run 1 (`gg_brake_marker_01_i_right_100m:0`,
  7000 ticks): the player breaks board #77 at tick 1548 (14 bodies), an AI car (the player is at rest) breaks the
  same-type board #79 at tick 6628 (16 bodies); one more board would have exceeded the old
  cap. Run 2 (`tyrestack:0` at 25 m/s, 9000 ticks): two tyre stacks break at ticks
  8138/8140 (24 pieces, 36 bodies); tyres settle on the road and grass, no pause.
  [I] Playtest: the failing tick's player position is 1.9 m from board #81
  (`gg_brake_marker_01_i_right_100m`); 12 cars + 2 halves stay within 16, so at least one
  earlier break (as AI cars do in run 1) must have been live.

## Port (F312)

Code: `grid_formats::physparams` (raceload.jpk#physparams.xml reader),
`grid_formats::entities` (shape behaviours with the damage behaviour type's deleted
and added subentities and constraint count), `grid_tools::dynamic_props`
(catalog, pieces, contacts, break detection/apply), `Runtime::enable_dynamic_props`
/ `MultiRuntime::enable_dynamic_props`, `grid-app` `dynamic_props.rs`.

- **Which props.** All 190 constraint-free breakables: 34 brake marker boards,
  8 tyre stacks, 148 cushions. Since F484 also the 588 tyre walls, whose six blocks are
  joined by seven springs (docs/JOINTS_RESEARCH.md "Phase 3a"), and since F501 the 243
  armco sections: rails hinged to the post, the world and the neighbouring section's post,
  with the override bodies (docs/JOINTS_RESEARCH.md "Phase 3b"). Everything without a
  shape behaviour stays static. [V data, D mechanism]
- **Contact limit.** An armed intact box writes its threshold into contact +0x3C
  (`BodyContact::limit`); the ported solver caps the row at `dt x limit` and writes the
  solved force back. Unarmed boxes keep FLT_MAX, so nothing changes until a breakable
  is touched. [D/V port]
- **Break.** After the step, a recorded prop contact whose solved +0x3C reaches
  `threshold / 1.01` queues the entity; the intact box(es) stop colliding in every
  car's track and each added subentity becomes a body appended to the world's active
  chain (the world double-buffers its active heads/tails by selector world[5]) with
  its own dynamic shape token. Explosion velocity `e (v.n) n` with the 0x7CDCD0 scatter
  is ported but all ported classes have explosion 0, so pieces start at rest. [D]
- **Bodies.** Ported B11F40 constructor with the mass and solid inertia of the shape grown
  by its shell (F549, [D] B3D480/B40880, B46460/B46C90, B4A5F0/B4AAB0), rbDefault
  damping/drag/gravity scale/stationary values in the 0x7B8F40 field order. turnOffTime
  1 s since F541 (FLT_MAX before; BODY_SLEEPING.md).
- **Contacts.** Piece vs track, boundary and enabled props through the same cold body
  query as the car (piece margin = shape shell 0.02); car vs piece through the
  car-to-car hull path; a cylinder piece against a cylinder or box piece through AE8AD0
  and AE2FA0's cylinder handlers (F549, BOX_CONTACTS.md "Cylinder pairs and piece mass");
  box against box through an authored separating-axis generator (face axes of both hulls;
  the triangle-soup pair query lost stacked pieces once they overlapped beyond the
  margin). Authored budgets: 8
  track and 4 pair contacts per piece, pool +12 slots per piece up to 256. [I]
- **Single car.** `vehicle_world` now admits further dynamic bodies after the car
  (shapes after the statics, deformation registration slot None, body events skip
  piece-only contacts); with no pieces every guard is as before.
- **Reset.** Restart restores the intact world (pieces removed, boxes re-armed).

### Validation

Private evidence `local/dynamic-props/` (probe TSVs, logs, renders).

- `grid-prop-probe` (follow the centreline, aim at the prop, brake 0.5 s after the
  break; deformation and body events on):
  - board `gg_brake_marker_01_i_left_50m` #0: breaks at tick 5314, 15.07 m/s, solved
    force exactly 50.0 (the cap); the two 4.9 / 4.8 kg halves ride the bonnet, then lie
    flat ~24 m on (centre 0.09 m above the queried ground, thickness 0.16 m). Single and
    shared-world (`PROBE_MULTI=1`) runs are bit-identical, before and after rebasing onto
    F311.
  - tyre stack #6: breaks at tick 18511 (12.05 m/s, force 50.0); 12 tyres scatter up to
    6 m and settle flat or on edge at the expected heights (no interpenetration after
    the separating-axis pair contacts).
  - cushion #0: breaks at 250.0 (tick 15945, 15.1 m/s); the 996 kg block takes 3 m/s
    against the barrier behind it and stops; the car drops to 7.8 m/s.
- App renders (`GRID_PROP_SCRIPT=TEMPLATE:INDEX` with `--drive-ticks`, authored
  validation drive): `marker-5290.png` intact "50" board, `marker-break.png` halves on
  the bonnet, `marker-5520.png` both halves flat on the grass; `stack-18490.png`
  intact stacks, `stack-18560.png` / `stack-18800.png` scattered tyres, neighbouring
  stacks intact.
- Car-only regression: `grid-opponent-race 1 car 11200 ticks` with and without
  `GRID_DYNAMIC_PROPS=1`: laps 88.420 / 86.282 s bit-identical, 0 breaks.
  `dynamic_props_leave_an_untouched_drive_identical`: 240 throttle ticks with props
  armed equal the plain run word for word.

## Soak fixes (F342)

16 October 2026, Claude Code. Found by `grid-soak` (F326, docs/SOAK_TESTING.md).
Static evidence: capstone on the hash-pinned analysis image with the private
`local/ai-overtaking/scripts/pe.py` and the existing decompilations
`local/dynamic-props/decomp/a4`, `a8`, `a11`, `a12`; new helper scripts and
dumps in the main checkout's `local/soak-fixes/`.

### Scaled entity placements (Nuerburgring tyre stacks)

- [V] Nuerburgring `route_0` has 6 `tyrestack_white`/`tyrestack_red` breakables
  (72 pieces) whose `objects.ens` placement rows all have length 0.9718 (a
  uniform scale). The port refused them ("scaled prop piece placement"), so no
  Nuerburgring race with body contacts could start. Other scaled entities there
  (`armco_square_*`, `tyrewall_white`, scale 1.004-1.46, partly non-uniform) are
  joint-state breakables the port keeps static, and static boxes already apply
  the full placement (`entity_geometry.rs`).
- [D] `0x7D3C40` (add subentity) reads the entity world matrix at +0x40 (or the
  replaced body's current matrix through `0xA69630` / `0xA7D3C0`) and passes it
  **unnormalised** to `0x7CE490` -> `0x7CDCD0`, which copies it into the body
  descriptor (`0xA742E0` -> B11F40 for dynamic bodies, `0xA74190` static). Only
  the render-component path of `0x7D3C40` normalises each row (RSQRTPS + Newton,
  `0x4B4440` stores the three lengths) and hands the lengths to `0x7C4FF0` /
  `0xAD1040` as a separate scale.
- [I] So a piece body keeps the scaled basis; shapes and contact hulls scale
  with it, the mass stays density x unscaled template volume, and nothing
  re-orthonormalises it (only the world's special body, B13E10). The port now
  does the same: `Catalog::read` keeps the composed pose and rejects only a
  degenerate (non-positive or non-finite determinant) basis. The face-axis
  shortcut in the piece pair query assumes an orthogonal triple, which a uniform
  scale keeps. Test: `nurburgring_scaled_tyre_stacks_keep_their_placement_scale`.

### Track contact budget per shape pair

- [V] A piece (or car) leaning on a static prop box got all its track contacts
  from the box: `Track::body_contacts` queried props, then the boundary, then
  the road chunks against one shared capacity (8 per piece, 32 per race car),
  so the road chunk got nothing and the body slid down through the road (soak
  quick seed 9, Donington; the 16-contact prop pair plus the 16-contact
  boundary already filled a race car's 32).
- [D]/[V] The original generator fills one manifold per shape pair per call
  (AEA1F0 capacity argument; 20 per pair in the original, 16 in the port, F318
  A32); only the world pool bounds the sum. The port now gives every source
  (props, boundary, each road chunk) its own pair capacity, bounded by the pool
  share (`body_contacts(.., pair_capacity, pool, ..)`); since F317 the solver
  bounds are world-wide (10 000 contacts, 40 000 rows), so the old per-body
  totals were only an authored budget. The solo runtime pool grows from 36
  (4 wheels + props + boundary) to 64 (+ a road pair). Test:
  `a_hull_against_a_prop_box_keeps_its_road_contacts` (557 Donington prop
  boxes whose pair fills up; with one shared budget the road gets 0, with
  per-pair budgets 556 keep road contacts).

### Lost pieces (F318 R06)

A piece that has tunnelled keeps falling; about 150 s later |pose| > 1e5 makes
`Mesh::world_bounds` fail every tick and the whole world pauses (a race freeze
the soak never reaches, because it stops at the first fall). The original has
no coordinate domain there, so its lost piece just falls, out of sight.
`PropWorld::hold_lost` (both runtimes, after each committed step) holds a piece
once it is `LOST_DEPTH` (100 m, AUTHORED) below the lowest road chunk: zero
velocity, fixed pose, until the world is reset. Test:
`a_piece_lost_below_the_world_is_held_until_reset`.

### Pieces pushed into the ground (mostly orientation)

[V] Most Donington board falls (F326 #4) were the contact orientation flaw of
CONTACT_REORIENTATION_RESEARCH.md "Degenerate displacement": at |x| 400-550 a
triangle witness a few ulps from its anchor vertex took the normalised branch
and flipped a resting or sliding board's ground normal. With the coordinate
tolerance, 12 of 13 board falls of the pre-fix 37-minute Donington long run,
16 of 17 Nuerburgring failures of its long run, and long-1 seeds 1044, 1066
and 1386 pass on replay.

Still open (seeds 1091 and 1304 of long-1, 1067 of the pre-fix Donington long
run, 1065 of the Nuerburgring one):
- seed 1091: the lower brake-marker board (0.94 x 0.47 x 0.16, 4.9 kg) spawns
  1-2 cm above the grass (inside the 0.02 shell, so no ground contact that
  tick) while the car hits the stack at 16 m/s; in one tick it moves 0.31 m
  down, turns about 60 degrees and gets -4.4 m/s, 7 rad/s, and lying flat it is
  embedded deeper than its 0.08 m half thickness;
- seed 1067: a car at 51 m/s breaks a prop and the piece spawns inside the car;
  the separation drives it below the ground in its first step (vy -6 m/s).
[I] The original's AEA1F0 has a backface correction for an oriented normal
against an up-facing triangle (face y > 0.707 at D11548, vertical ray through
the chunk B225E0), but it is gated by B1AB30 on shape +0x294..+0x2A0, which only
the constructor writes (FLT_MAX, 0xB1F938..0xB1F976; no other writer found), so
it never runs. Whether the original avoids hit tunnelling another way (contact
scheduling, speculative pairs) is not established. Such pieces are now held
when lost (above).

[V] F351 found how the original avoids it ([BOX_CONTACTS.md](BOX_CONTACTS.md)).
Road triangles carry their chunk's 0.02 margin: every chunk read from the running
original has +0x220 = 0.02, and B212D0 copies it onto AEA1F0's temporary triangle.
The port used 0. The hot-spot board (the 150 m marker at (-406.7, -16.0), 2.14 cm
above the grass) therefore had no ground contact in its first step, and a car still
inside it pushed it under. With `ROAD_MARGIN` it rests on the grass. For example,
main's seed-1005 inputs are identical up to the spawn at tick 8822; the board then
falls to y -50.6 on main and stops at y -15.15 with the margin. The car contacts
still push it down (the original's sampled penetration takes the same vertical
axis); the ground contact now holds it. The car-at-51-m/s case (seed 1067) is not
re-examined. Static route prop boxes are now queried as box primitives (AE8AD0),
pieces against them included. Car against piece keeps the hull-triangle path: the
original's AE8AD0 pairing for it was tried and lost more boards at the port's fixed
16 ms step (BOX_CONTACTS.md, "Tried and not adopted").

## Drawn pieces and road depth (F564)

Owner's report of 23 October 2026 (PC, Detroit demolition derby, after F561): after a stack
breaks, tyres jump unnaturally, partly sink into the road and pop back out, and often end up
asleep in the air. Two separate causes.

**Presentation [V].** grid-app drew each piece from `Renderables::draws(anchor)`. That composes
transforms only from the anchor node down; the transforms of the damage renderable's nodes above
the anchor were missing. Every Detroit and Nürburgring tyre was therefore drawn about (-0.61,
-0.54, 0) m off its collision cylinder, in the piece frame. Lying flat, a tyre was drawn half a
metre under its body (into the road). Standing on its edge or tilted against a wall, it was drawn
beside or above its body (in the air). This matches the owner's screenshots. The new
`Renderables::draws_in(renderable, anchor)` composes from the anchor up to the damage renderable.
The test `round_piece_meshes_sit_on_their_bodies` (since F566 part of `breakable_pieces_draw_on_their_bodies`) checks every round piece of the first breakable
per template on Detroit route_9 and Nürburgring route_1: mesh centre within 5 cm of the hull
centre, mesh half-extents within 6 cm of the cylinder. With `draws` it fails at tyrestack_black
piece 0 (centre (-0.609, -0.539, 0.006)).

Some pieces had no draw at all: Nürburgring's `tyrewall_black_yok` pieces 6-21 and
`close_drift_flag`, among others. F566 resolves them (next section).

**Physics: sinking under load [V, AUTHORED].** A car pressing a tyre (sitting on it, or driving
into a wall of them) pushes the hull below the road for a tick or two. The solver then recovers
the penetration (before this change: piece 139, Detroit route_9, tick 1061, 0.30 m under the road,
popped 0.33 m up the next tick). The original solves contacts the same way. Its recovery from
such loads is not established, and it was not captured. The owner asked that pieces never sink,
partly or fully. So `PropWorld::lift_sunk` runs after each committed step, after `hold_lost`. It
moves every live piece or wheel whose hull bottom is more than `SINK_TOLERANCE` (0.01 m) below
the road under its centre back to that tolerance, through `continuous::apply` (the B14550 move:
+0x1C0, the corrected and published poses, the +0x360 correction), and clears any downward
linear velocity. `PropWorld::road_depths` measures the depth: the cylinder's or sphere's lowest
point, or the mesh hull's world bounds, against a track cast from 5 cm above the centre. A start
3 m higher hit wall ledges. The test `pieces_pressed_by_a_car_stay_above_the_road` (Le Mans wall
969, the car at full throttle for 600 ticks) sank body 22 by 0.099 m at tick 397 before the
change. After it, the deepest of 22091 piece-ticks is 0.0100 m.

Detroit demolition derby, 12 stk cars, 12000 ticks, `GRID_SINK_CHECK=0.02 GRID_HANG_CHECK=300`
(grid-opponent-race):

| route | build | ticks with a piece > 2 cm under the road | deepest | unsupported (max per check) | breaks / pieces |
|---|---|---|---|---|---|
| route_8 | before | 5 | 0.231 m | 0 | 3 / 3 |
| route_8 | after | 0 | - | 0 | 4 / 4 |
| route_9 | before | 41 | 0.302 m | 0 | 32 / 285 |
| route_9 | after | 0 | - | 2 | 39 / 394 |

**Asleep "unsupported" pieces after the change [O].** The route_9 run after the change shows
pairs of unsupported pieces at some checks: 112 and 120 at ticks 1200-1500, 272 and 273 at ticks
6000-6600. These are not in the air. Bodies 112, 120, 121 lean as a heap: 112 on 120, 120 on
121, 121 on 122, 123 and a static route prop box. At tick 900 the heap still crept about
0.05-0.13 m/s, below the rbDefault stationary threshold of 0.3 m/s. So after turnOffTime (1 s)
the island slept at tick 901, the original's rule [D, BODY_SLEEPING.md section 2]. The diagnostic
wakes every piece at once with parking off, and the heap then slides on another 0.3-0.5 m. It
counts as unsupported for that reason, though every body has a supporting contact. No
correction: the original freezes a slowly creeping heap the same way.

## Pieces without draws (F566)

A test checks every piece of every breakable template on all routes of the phone tracks
(Nürburgring route_0-3, Detroit route_0-9, Le Mans route_0-1, Donington route_0-3). Each piece
must get a draw from its anchor in its damage renderable (`Renderables::draws_in`, as grid-app
places it), and that draw must sit on its collision shape. Before this change, 159 of 357 pieces
had no draw. Every piece that did draw sat on its body. Three causes [O, objects.pssg]:

1. **BUNDLENODE roots.** The damage renderable of every tyre wall, Armco section and the flag is a
   `BUNDLENODE`, not a `NODE`. The scene index only had NODE ids.
2. **Skinned damage meshes.** Tyre-wall panels (`*_dam_fr/fc/fl/bl/bc/br`), Armco rail sections
   (`*_dam_obj01-04`) and the flag keep their broken geometry in one `SKINNODE` per renderable,
   not in mesh nodes under each piece:
   - its `MODIFIERNETWORKINSTANCE` (network `skinTransformLocal`) draws a source of
     `SkinnableVertex`/`SkinnableNormal` float3, `SkinWeights` float4 and `SkinIndices` uchar4;
   - its `SKINJOINT`s name one `JOINTNODE` under each piece's anchor;
   - its `SKELETON` holds one `INVERSEBINDMATRIX` per joint.

   There are 18 such sources on the four tracks. `read_static_meshes` skipped them, and a piece's
   anchor has no render instance below it. The flag is rigid (weight 1). Tyre-wall panels blend
   about 10% of their ~290 vertices (main weight at least 0.7). Armco rails blend most of theirs
   (main weight down to 0.39), so the rail bends.
3. **Suffixed names.** Le Mans' `tyrewall_blue` and `tyrewall_yellow` name their tyres
   `tyre01_obj_01!0` and so on: the exporter suffixes a repeated id. The entity data names
   `tyre01_obj_01`.

Port:
- `Renderables` indexes BUNDLENODE ids as `draws_in` roots, BUNDLENODE composes as a transform
  ancestor, and an anchor matches its `!N`-suffixed NODE under the root.
- `mesh::read_object_meshes` (now grid-app's object reader) adds the skinned sources, with each
  vertex's main joint in `skin_indices`.
- When the anchor has no render instance, `draws_in` falls back to the root's skins. It makes one
  draw per joint at or under the anchor: the triangles whose vertices' main joint (two of three,
  else the first) is that joint, with transform inverse bind x joint-to-root chain. Inverse bind
  matrices with float noise in the last column (tyre walls, about 1e-8) are cleaned.
- AUTHORED approximation: the original blends up to four joints per vertex. Here a triangle
  follows one joint. This is exact while the pieces are at their bind pose, and leaves seams
  where an Armco rail bends or pieces separate. Proper blend skinning in grid-app (joint matrices
  from the piece bodies) is not done.

After the change, all 353 drawable pieces draw on their bodies: round pieces are centred and fill
their cylinder or sphere, box pieces' mesh centres lie in their box, and every draw's material
has a diffuse binding. The other 4 pieces are the Armco main sections' `section_dam_link1` posts
(Nürburgring grey, red and white, Le Mans `armco_02`). Their anchors carry no geometry and no
joint, so there is nothing to draw in the original either; grid-app counts them in
`pieces_without_draws`. Donington's Armco link post mesh is 1.01 m tall on a 0.76 m collision
cylinder, centred 0.12 m higher. That is data, not an offset, and the test allows a post's mesh
up to 20% of the cylinder's height beyond it.

## Not yet established

- Armco end pieces (`link0`, a rail of three shapes) and scaled jointed entities stay
  static (the close-drift flags break since F509); uproot (rail bodies switching to `rbDefault`) is not
  ported, its trigger is not found. Armco and tyre-wall joints are ported since F501 and
  F484 (docs/JOINTS_RESEARCH.md).
- Sleeping: ported in F541 (BODY_SLEEPING.md); parking (status 2) is not.
- Whether routes override the ENTITYMANAGER defaults; the farthest-first restore of
  broken entities when a pool is full (recovered, not ported, unreachable with the
  ported catalog).
- Live confirmation of any of the above.
