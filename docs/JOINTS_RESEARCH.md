# Joints, armco hinges and ENS links (FIDELITY_BACKLOG row 16, research)

19 October 2026, Claude Code. Static research of the hash-pinned analysis image
(`aac46a64…4514`) with Ghidra 12.1.4 headless `-readOnly -noanalysis` on the private copy of
the dynamic-props project and capstone 5.0.6 (`pe.py`). Pseudocode in this worktree's private
`local/robust/decomp/r16a..`; the purchased `raceload.jpk#physparams.xml` constraint props in
`local/r20/physparams-constraints.txt`. Grades: [V] read from purchased data or run, [D] read
from the decompilation, [I] inferred. The original game was not run.

## What breaks into joined pieces

[V] `objecttypes.pssg` gives a damage behaviour's added components: the pieces
(`TEMPLATESUBENTITY` with a rigid body) and `TEMPLATECONSTRAINT`s between their bodies:

- armco (`armco_*_main*`): 4 rail sections and a post; six constraints: post to world
  (`body2uri="none"`, `cArmcoPost`), post to rail 1 and rail 4 to the neighbour's post
  (`body2uri="link1"`, `cArmcoEnd`), three rail-to-rail (`cArmcoMiddle`). Each constraint has a
  `TEMPLATETRANSFORM` (the joint frame in the entity).
- tyre walls (`tyrewall_*`, `tyre_wall`): six blocks and loose tyres; seven `NeonConstraint_*`
  (`cDeformableRubberLateral`, `Centre`, `Ends`).
- The entity's `TEMPLATEBEHAVIOUR` deletes the intact subentity and adds the pieces and
  constraints (`TEMPLATECOMPONENTCHANGE`).

[V] `physparams.xml` `TEMPLATECONSTRAINTPROPS`: a six-degree-of-freedom joint.
`linearLimitAABBMin/Max` (per-axis linear travel), `linearSpringAABBMin/Max`, `linearSpringK/S`
(per axis), `linearRotation`, `angularRotation0/1` (quaternions), `rotationType`
(`cone`/`plane`/`free`) with `rotationLimitAngle`, `rotationSpringAngle/K/S`,
`rotationTanSpringS`, `twistType` (`circle`/`free`) with `twistLimitAngle`, `twistSpringAngle/K/S`,
`breakLinearImpulse`, `breakAngularImpulse`, optional `hardRotationalFriction`,
`softRotationalFriction`, `softImpulseFactor`, `breakIfAngExtentHit`, `uprootProps`,
`dontCollideWithAny`, `dontCollideWithFixedObjects`, `bodyOverride` and `bodyOverridePriority`.
Armco middle and end joints override the rail bodies with `_rbArmcoMiddle` (sphericalise 1,
spherical mass 7000, damping 14, gravity scale 0, uproot to `rbDefault` only without gravity);
the post joint breaks at impulse 10 and overrides the post with `_rbArmcoPost` (gravity 1).

## Code path [D]

- Property schema: `.data` 0xE4D378.. (name, attribute-id slot, type), the serializer 0x7CF9C0
  and reader 0x7D00F0. Props layout (from the serializer): +0x30 linear limit min, +0x40 max,
  +0x50/+0x60 linear spring box, +0x70 spring K, +0x7C spring S, +0x88 rotation type (1 plane,
  2 cone, else free), +0x8C limit angle, +0x90 spring angle, +0x94 K, +0x98 S, +0x9C tangent
  S, +0xA0 twist type (1 circle, else free), +0xA4.. twist, +0xCC linear rotation, +0xDC/+0xEC
  angular rotations 0/1.
- Template class ids (static initialisers 0xC7A0xx): `TEMPLATECONSTRAINT` 0xE7A6B0,
  `TEMPLATESUBENTITY` 0xE7A770, `TEMPLATECLOTH` 0xE7A9B0.
- Component add 0x7D3C40: a constraint component goes to 0x7D3AF0 (once per entity; resolves
  the two bodies, `link1` through the entity's link list +0xD8), then 0x7CC1E0 builds the
  world joint: 0x7C7B10 fills a descriptor from the props and the transform, and 0xA74670
  copies it into the physics world's create call (0xA75EA0 allocates 0x330 bytes and runs
  the joint constructor 0xAEFEB0, 1569 lines).
- The joint class: vtable 0xD3ADFC (destructor 0xAFCC50, debug draw 0xAF2240, base reset
  0xB827C0); base constructor 0xB825E0 links it into the world's constraint list (world
  +0x1C), each body's list (+0x320 / +0x324) and merges the bodies' islands (0xB24E70).
  The image names it "NeMouse" (its error strings).
- The solver: an island has joint pools beside the contact pools (island +0x9C/+0xB0 and
  +0xA4/+0xB8). The main response 0xB7ACC0 builds each joint's rows with 0xAF4890
  (1997 lines) and counts them into +0xBC/+0xC4; the optional correction 0xB7B5E0 solves
  them with 0xAEF930 and 0xAEFBB0 beside the contact rows (0xB0CE10/0xB0CEC0).
- Links: 0x7D5540 propagates a break along ENS entity links (DYNAMIC_PROPS_RESEARCH.md).

## The port today

The port's solver refuses bodies with constraint lists (`contact_groups`: body +0x320..
must be zero) and has no joint pools; `dynamic_props::Catalog::read` keeps every template
with constraints, or with more than one shape behaviour, static.

## Size [D]

Decompiled lines (Ghidra, `local/robust/decomp/r16a..r16c`): joint constructor 0xAEFEB0 1569;
prepare 0xAF4890 1997; two-body row initialisation 0xAF71C0 1834; one-body 0xAF9430 1523;
correction solvers 0xAEF930 151 and 0xAEFBB0 136; ENS link propagation 0x7D5540 489; props
reader 0x7D00F0 334; descriptor 0x7C7B10 and header 0x7C04D0 170. The main response 0xB7ACC0
handles the joint pools beside the contact pools: AF4890 counts each joint's rows into the
island totals (+0xBC/+0xC4), AF71C0/AF9430 write them after the contact rows, and the shared
row solver runs them all. The port's frame pipeline has no joint pools, so the solver
integration (contact groups, grouped and impact frames) is part of the work, as are islands
merged through joints, breaking, body overrides and the entity side.

Verification: the native probes (`tools/probe_*.c`) run original functions in-process on
supplied state; a joint probe (constructor, prepare, row initialisation) gives the same
bit-for-bit comparison the impact pools have.

## Plan

1. Joint descriptor and constructor (0x7C7B10, 0xAEFEB0) as data: frames, limits, springs.
2. Row builder 0xAF4890 and the joint pools in the island solve (0xB7ACC0, 0xB7B850),
   then the correction solvers 0xAEF930/0xAEFBB0.
3. Breaking (break impulses, `breakIfAngExtentHit`), body overrides and uproot.
4. Entities: constraint components in `dynamic_props`, `link1` across entities, and the ENS
   link propagation 0x7D5540; the close-drift flags as the original builds them.
