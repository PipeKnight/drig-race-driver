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

Phase 1 (F473): the joint itself is ported and verified, not yet wired into the solver.
The port's solver still refuses bodies with constraint lists (`contact_groups`: body
+0x320.. must be zero) and has no joint pools; `dynamic_props::Catalog::read` keeps every
template with constraints, or with more than one shape behaviour, static.

### Phase 1: constructor, prepare, rows (F473)

- `grid_world::joint`: the joint kept as the original's 0x330-byte object (pointer words
  left out). `construct` = AEFEB0 with B825E0/B827F0's fields; `prepare` = AF4890 with
  AEED10 (per-degree-of-freedom limit and spring state), AF1F10 (swing axis) and the
  verified `pose::rotation` (0x772670).
- `grid_world::joint_rows`: `write` = AF71C0 (two dynamic bodies) and AF9430 (one), with
  AEF780 (limit and spring rows of a degree of freedom), B82550 (compliance) and AFCBF0
  (impulse bounds), into the contact row pools and B7ACC0's batch bookkeeping
  (`Context`).
- [D] The constructor's two branches: a frame joint (descriptor byte 0x164 clear) puts
  three descriptor frames into body space (+0x70 A's frame, +0x180 A's second frame, +0x1C0
  B's frame; copied to +0x200, the target the friction rows use) and re-orthonormalises
  them; a point joint keeps two anchors relative to the bodies' mass centres (+0x230) and
  their rest distance (+0x240).
- [D] Prepare: the three linear degrees of freedom along A's world frame (state at +0x264,
  stride 0x28, errors written back into the offset by 0x4B4440), the rotation limit
  (+0x25C: 1 plane with an acos and an axis-angle test, 2 cone with a 0.999 near-axis
  fallback), the twist (+0x260, AF1F10), the rotational friction factor (+0x250
  `hardRotationalFriction`, +0x258 = mass-weighted anchor lengths times it) and the
  re-seat of B's target frame when byte 0x32D is set (0.8 of the limit). Row counts in
  +0x14/+0x18, the angular ones in +0x54.
- [D] Rows: friction rows (2 for a plane joint, 3 otherwise; bounds +-step x +0x258,
  bias reduction x inverse step x angle), the rotation-limit rows (AEF780 on +0x2DC, plus
  one row about +0x120), the twist rows (+0x304) and the linear rows along A's frame with
  lever arms from the mass centres; a point joint has one row along the anchor separation
  with bounds 0..FLT_MAX.
- [D] Arithmetic order comes from the disassembly, not Ghidra's chains: a row times a
  matrix is (x R0 + z R2) + (w R3 + y R1) and a dot product (p0 + p1) + (p2 + p3);
  normalisation is RSQRTPS with one Newton step (`sse_approx::rsqrt`), lengths
  ((1 - p r) p) 0.5 + p; acos is the CRT's (0xBF4067 and the dispatcher 0xBF33B0) as f64
  rounded to f32. `-0.0 - x` keeps a NaN operand as SUBSS does (Rust may fold it to a sign
  flip). An unordered compare in the point branch counts as below (COMISS/JB).

Verification [V]: `tools/probe_joint.c` runs the original functions in-process on the
hash-pinned analysis image (code pages unchanged, four lazy-data pages writable),
`grid-joint` (crates/grid-probes) runs the port on the same JNT1 cases and
`tools/validate_joint.py` compares them byte for byte: modes 0 (constructor), 1
(+ prepare with moved bodies and the re-seat flag) and 2 (+ rows into pre-initialised
pools with or without a current batch), 8192 random cases each, 0 mismatches. Cases cover
frame and point joints, one and two bodies, rotation types 0..2, twist, friction, limit
and spring rows, re-seats and aligned rest frames. Evidence: this worktree's
`local/joint/` (cases, original and Rust outputs).

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

1. ~~Constructor, prepare and rows (0xAEFEB0, 0xAF4890, 0xAF71C0/0xAF9430)~~: F473.
2. The joint pools in the island solve (0xB7ACC0, 0xB7B850), islands merged through joints,
   the correction solvers 0xAEF930/0xAEFBB0, and breaking.
3. Breaking (break impulses, `breakIfAngExtentHit`), body overrides and uproot.
4. Entities: constraint components in `dynamic_props`, `link1` across entities, and the ENS
   link propagation 0x7D5540; the close-drift flags as the original builds them. Props to
   descriptor ([D]): the reader 0x7D00F0 and 0x7C04D0/0x7C7B10/0xA74670 map
   `hardRotationalFriction` to joint +0x250, `breakLinearImpulse`/`breakAngularImpulse`
   to +0x248/+0x24C, `breakIfAngExtentHit` to a callback (0x7B81A0).
