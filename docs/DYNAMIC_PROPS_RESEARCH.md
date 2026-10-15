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
   [I] For car-versus-prop pairs the flag pairing applies (otherwise the 1.01 tolerance
   below would never be reached); the exact flag provenance of shape bytes 0x17/0x18 for
   entity shapes (`0x7B9390` descriptor bytes +5/+6) is not traced further.
2. **Callback.** [D] Shapes with a behaviour register `0x7C46A0` as their contact
   callback (`0x7C6350` tail, registration +0xC/+0x10). The event is built by
   `B0CFC0` (event+0x20 = contact normal, event+0x40 = contact +0x3C, event0/4 = side
   registrations). For each side whose shape has a behaviour, `0x7BDFD0` fires when
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
5. **Links.** [D] `0x7D5540` propagates a break along ENS entity links (armco chains).

[I] Consequence: in the break step the car only feels the capped impulse
(`threshold x dt`: 0.8 N s for a board at 60 Hz, 16.7 N s for a tyre wall), then
meets the new pieces as ordinary dynamic bodies.

## Body properties used by the pieces (rbDefault)

[V] `sphericalise 0`, `linDamping 1`, `angDamping 1 1 1`, air resistance 0,
`linearStationary 0.3`, `angularStationary 0.3`, `turnOffTime 1`, wake force/torque 0,
`gravityScale 1`, max velocities `+inf`. Shape props: sCardboard `PLAS d70`,
sRubber/sPlastic/sDeformableRubber `PLAS d700`, sArmco `SIGN d2610`.
[I] `d` = density in kg/m^3; mass = density x shape volume, inertia of the solid
box/cylinder. Boards 4.8-4.9 kg, tyres 17-56 kg, cushion 996 kg, tyre-wall block 428 kg.

## Lifetime

[D] No timer or distance despawn was found for pieces. The per-entity timer in
`0x7D64B0` (`+0x15`/+0x10, `0x7BB800`) switches render state of the subentities
[I], not removal.
Pieces stay until the pool needs space (`0x7D4940` recycles) or the entity manager is
reset (`ResetEntityManager`, restart). [I] Resetting on restart.

## Port (F312)

Code: `grid_formats::physparams` (raceload.jpk#physparams.xml reader),
`grid_formats::entities` (shape behaviours with the damage behaviour type's deleted
and added subentities and constraint count), `grid_tools::dynamic_props`
(catalog, pieces, contacts, break detection/apply), `Runtime::enable_dynamic_props`
/ `MultiRuntime::enable_dynamic_props`, `grid-app` `dynamic_props.rs`.

- **Which props.** All 190 constraint-free breakables: 34 brake marker boards,
  8 tyre stacks, 148 cushions. Tyre walls (588) and armco (243) need the joint rows
  the port lacks and stay static (`Catalog::unported`). Everything without a shape
  behaviour stays static. [V data, D mechanism]
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
- **Bodies.** Ported B11F40 constructor with mass = density x volume, solid
  box/cylinder inertia, rbDefault damping/drag/gravity scale/stationary values in the
  0x7B8F40 field order. Deviation: turnOffTime is FLT_MAX (the ported grouped frame
  only admits never-sleeping bodies). [I mass formula, deviation recorded]
- **Contacts.** Piece vs track, boundary and enabled props through the same cold body
  query as the car (piece margin = shape shell 0.02); car vs piece through the
  car-to-car hull path; piece vs piece through an authored separating-axis generator
  (face axes of both hulls; the triangle-soup pair query lost stacked tyres once they
  overlapped beyond the margin). Cylinders are 12-sided prisms. Authored budgets: 8
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

## Not yet established

- Exact constraint (joint) solver, so tyre walls and armco stay static: tyre-wall springs (`cDeformableRubber*`,
  `dontCollideWithAny`), armco hinges with `bodyOverride` rail bodies (gravityScale 0,
  sphericalmass 7000, damping 14), post-to-world joint breaking at 10, ENS link
  propagation. The port has no joint rows.
- Sleeping: the ported grouped frame only admits `turnOffTime = MAX`.
- The pool limits (`maxSubEntityInstances`, `maxConstraintInstances`) values.
- Live confirmation of any of the above.
