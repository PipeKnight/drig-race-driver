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

Phase 1 (F473): the joint itself is ported and verified. Phase 2 (F477): the solver runs
world joints (islands merged through them, rows beside the contact rows, impulse sums,
breaking, the correction pass). Phase 3a (F484): a breakable whose damage state joins its
own pieces (tyre walls, Milan's green cushions) breaks into pieces with their joints.
Constraints to the world or to a linked entity, body overrides and uproots (armco), and the
close-drift flags are phase 3b; those entities stay static (`Catalog::unported`).

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

### Phase 2: island pools, merging, breaking (F477)

Original path [D] (decompiles `local/robust/decomp/r16e`, `r16f`; asm `local/r20/*.asm`):

- Grouping, AD6C40: before the contacts, every active body pushes its unbroken joints
  (byte +0x48 clear) from its A list (+0x320, newest first, B825E0 prepends) onto a stack;
  a joint enters from B's list (+0x324) only while A is inactive. The stack is grouped with
  AD6250, the contacts' union routine: the joint joins its bodies' groups (a fixed B, +0xC,
  does not) and goes to the head of the group's list. B24C90 then sets each item's solver
  indices (B82850: +0x1C = A's +0x404, +0x20 = B's or -1 when B is fixed or absent) and,
  for a joint (byte +0xF clear), clears +0xE (AEEC80).
- B7B850 sorts the group list into four pools by +0xF and by bodies: two-body contacts
  +0xA0, two-body joints +0xA4, one-body contacts +0x98, one-body joints +0x9C, each in list
  order.
- B7ACC0: AF4890 prepares every joint while counting its rows into the normal spans
  (joints have +0x14 = +0x18: no friction rows), so the normal pools are two-body contacts,
  two-body joints, one-body contacts, one-body joints; AF71C0/AF9430 continue the contacts'
  batches; B7ACC0 closes each pool's last batch. Context: +4 inverse time, +8 error
  reduction (manager parameter 6), +0x14 compliance scale (parameter 3), +0x18 impulse
  scale (parameter 4), +0x1C default softness.
- After the solve, AFB280 (two-body) / AFB800 (one-body) walk the joint's rows in the order
  they were written: friction rows ask for a re-seat (byte +0x32D) when |impulse| reaches
  0.95 x +0x258 x context +0x14; every limit and spring row (AF47C0) and the extra rotation row add
  impulse x (rcp(n) J refined once) into the sums +0x140 (linear A), +0x150 (angular A),
  +0x160/+0x170 (B) and break the joint (+0x48) when 1.01 x |impulse x scale / inverse
  time| passes the row's limit (`breakAngularImpulse` +0x24C for angular rows,
  `breakLinearImpulse` +0x248 for linear ones); finally the sums are scaled by scale /
  inverse time and the joint breaks when a sum's squared length (four lanes,
  (x0^2 + x1^2) + (x2^2 + x3^2)) passes the limit squared (FLT_MAX, 0xE51D40, means none).
  A point joint only skips its rows.
- Correction, B7B5E0 (manager correction iterations > 0): every normal row is reset (bias 0,
  the correction softness) and the pools get their position biases in order; AEF930 /
  AEFBB0: a broken or disabled joint keeps no response (words 16..31, one body 16..23); the
  angular rows (+0x54) lose theirs; each linear degree of freedom with its limit flag
  (state +2) biases the next row by (error -/+ threshold) x scale, zeroed when it points
  the wrong way, and asks for the pass; the point row uses distance - (rest + 0.1). The
  one-body frame joint always asks for the pass. The limit loop advances one row per
  flagged degree of freedom, so a degree of freedom with spring rows shifts the later
  biases by a row: ported as the original does it.

Port: `joint_rows::write_back` (AFB280/AFB800 with AF47C0, AFCD90/AFCC80), `joint_rows::correct`
(AEF930/AEFBB0); `contact_groups::WorldJoint` in `Input`/`Output.joints` with
`Output.joint_groups` (each root's joints in list order); `grouped_physical_frame` hands each
island's joints to `impact_step::Input.joints` (`Joints`: the island's body records in local
order and the joints) and writes the solved joints back; `impact_step` prepares, counts,
writes and writes back the joint rows; `impact_frame`/`impact_correction` add the joint rows
to the correction pass.

Verification:

- [V] Probe modes 3 (write-back after supplied row impulses and denominators) and 4
  (B7B5E0's row reset, then the correction rows) against the original, 8192 cases each,
  0 mismatched words; mode 3 breaks 5556 joints and re-seats 2947, mode 4 activates the pass
  in 1316 cases (most others are broken by mode 3). Modes 0..2 still pass.
- Unit tests: `impact_step` a joint alone (no contacts) holds a leaving body and breaks
  with a small `breakLinearImpulse`; `grouped_physical_frame` two bodies with no contact
  form one island through a joint, and two without it.
- Not verified against the original: the whole island solve with joints (B7ACC0 end to end)
  and the grouping order with several joints and contacts; those follow from the verified
  parts and the decompiled order. [I] The correction context gives contacts +0xC (island +0xE8) and
  joints +0x10 (island +0xEC); the port uses one threshold for both (both 0.02 in the captured race,
  `source_vehicle`).
- Limits: the manager's row capacity is not grown for joint rows yet (a full pool gives
  status 1 as before); a fixed B (static object) is accepted but nothing builds one yet.

### Phase 3a: joints from the entity templates (F484)

Original path [D] (decompiles `local/robust/decomp/r16g`, `r16h`; asm `local/r20/*.asm`):

- Props, 0x7B7FC0 / 0x7D00F0, schema 0xE4D390.. (names read from the table): +0x30/+0x40
  `linearLimitAABBMin/Max`, +0x50/+0x60 `linearSpringAABBMin/Max` (the reader writes lane 3
  as zero), +0x70 `linearSpringK`, +0x7C `linearSpringS`, +0x88 `rotationType` ("cone" 2,
  "plane" 1, else 0; default 2), +0x8C..+0x9C limit angle (default 0.5236), spring angle, K,
  S, tangent S, +0xA0 `twistType` ("circle" 1, else 0; default 1), +0xA4..+0xB0 twist,
  +0xB4/+0xB8 `breakLinearImpulse`/`breakAngularImpulse` (0x7D8600: "+inf" is FLT_MAX,
  "-inf" -FLT_MAX, else atof; default FLT_MAX), +0xBC `softRotationalFriction`, +0xC0
  `hardRotationalFriction`, +0xC4 `softImpulseFactor`, +0xC8 `breakIfAngExtentHit`, +0xCC /
  +0xDC / +0xEC `linearRotation` / `angularRotation0/1` (quaternions; default all zero),
  +0xFC `uprootProps` (other constraint props, 0x7B7F80 picks them for an uprooted joint),
  bytes +0x100 `dontCollideWithAny`, +0x101 `dontCollideWithFixedObjects`, +0x102
  `specificPostCollision`, +0x104 `bodyOverride`, +0x108 `bodyOverridePriority` (default -1).
- Template, 0x7CF4B0: +0x2C/+0x30 `body1uri`/`body2uri`, +0x140 `display`, +0x148 the props,
  +0x100..+0x13F the `TEMPLATETRANSFORM` with rows 0 and 1 normalised (RSQRTPS and one
  Newton step over all four lanes) and row 2 the normalised cross product 0x67C200 of the
  new rows. 0x7C7C00 then keeps three frames: +0x40, +0x80, +0xC0 = the transform times the
  rotation matrices of the three quaternions (0x7C06B0, 0x7C08E0, 0x7C0B10, one routine:
  rows (C0, B1, D2), (D0, C1, B2), (B0, D1, C2) with C the diagonal (1 - 2y y) - 2z z..., B
  2y w + z 2x..., D y 2x - 2z w...).
- Component, 0x7D3AF0 (once per constraint of the entity) resolves the bodies (A the
  template body; B the template's `body2uri` body, or for `link1` the linked entity's body
  through the entity's link list +0xD8) and calls 0x7CC1E0:
  - B absent: the world's fixed object (0x958200), joint byte +0xC set by 0xA74670.
  - `dontCollideWithAny` (when the component has no explicit pair lists +0xB4..+0xC0): every
    shape of A and B gets the pair filter 0x7B62C0 (shape +0x188). The narrow phase's pair
    test 0xB38D80 skips a pair whose filter returns non-zero, and a pair an unbroken joint
    joins directly (each body's joint list +0x320/+0x324). 0x7B62C0 returns non-zero for two
    shapes of the same entity whose bodies are joined through unbroken joints (0xAD68F0,
    depth-first over the joint lists with the world +0x7D0 visit stamp).
  - `dontCollideWithFixedObjects`: A's and a dynamic B's body byte +0x416 cleared
    (0xA69560); B18300 / B15B70 schedule a pair with static geometry (the road included)
    only while it is set.
  - `specificPostCollision` (phantom posts) and a soft rotational friction different from
    the hard one (shape callback 0x7C5210) have their own paths.
  - The request (0x7D8C40 defaults; +0x170 point flag clear): 0x7C7B10 copies the three
    frames to +0x70/+0xB0/+0xF0 and 0x7C04D0 the props: limit and spring boxes, spring K
    and S times the entity scale, the rotation and twist words raw, the break impulses
    times the scale unless FLT_MAX (+0x15C/+0x160), `hardRotationalFriction` (+0x164), the
    callback 0x7B81A0 for `breakIfAngExtentHit` (+0x168); +0xC = `display`.
  - Each frame's position row is multiplied lane by lane by the entity's scale vector
    (+0x80, 0x4B1BD0), then the frame by the entity matrix (the inlined SSE row product).
  - 0xA74670 maps the request to AEFEB0's descriptor: frames to +0x10/+0x50/+0x90, boxes to
    +0xD0.., K and S to +0x110.., rotation type (0, 1, else 2) to +0x128, the rotation words
    to +0x12C.., twist type != 0 to +0x140, twist words to +0x144.., break impulses to
    +0x154/+0x158, friction to +0x15C, callback to +0x160, point flag to +0x164; byte +0xC
    fixed B, +0xD `display`.
  - After the joint, a break callback is registered (0x7C45C0) when either break impulse is
    finite.

Port: `grid_formats::entities::Constraint` (typed `TEMPLATECONSTRAINT`s of a damage state),
`grid_formats::physparams::Constraint` (typed props with the constructor's defaults),
`grid_world::joint_template` (`template_transform`, `quaternion_matrix`, `template_frames`,
`request`, `descriptor`), and `grid_tools::dynamic_props`:

- `Catalog::read` keeps a breakable whose constraints join two of its own pieces
  (`JointSpec`: the pieces, the request at unit scale, the two collision flags). Still
  static: a constraint to the world or `link1`, a body override, uproot props, a phantom
  post filter, soft friction different from the hard friction, or a placement with a
  non-unit scale (0x7CC1E0's lane choice for the props' scale is not ported).
- `PropWorld::apply` builds each joint after the entity's pieces, in the damage state's
  order: the descriptor from the request and the placement, `joint::construct` on the
  pieces' spawn poses, appended to the world's joint list (B825E0), with body byte +0x416
  cleared for `dontCollideWithFixedObjects`. Each committed step it reads back which joints
  broke.
- `PropWorld::contacts` skips piece pairs as 0xB38D80 and 0x7B62C0 do, and track contacts
  (and `continuous` the road sweep) of a body without byte +0x416.
- `reserve` grows the solver rows by 16 per live joint (authored bound).

Verification:

- [V] Template stage: `tools/probe_joint_template.c` runs 0x7C7C00 and 0x7C7B10 (with
  0x7C04D0) in-process on random props and template transforms (unit and non-unit
  quaternions, scales 0.5..2, FLT_MAX and finite break impulses); `grid-joint-template`
  runs the port; `tools/validate_joint_template.py`: 8192 cases, 0 mismatched words (the
  callback compared as an image address).
- [D] The reader's renormalisation and 0x7CC1E0's frame product use the joint's verified
  normalise, cross product and row product in the same operation order (asm); not run
  natively.
- Unit test: a Donington tyre wall breaks into 22 pieces and 7 joints; after five seconds
  the springs keep the blocks within 0.03 m of their rest distances, no joint breaks and
  every piece is at rest (`a_broken_tyre_wall_stays_joined_and_settles`).
- [V data] Breakables with joints: Donington 588 `tyre_wall` (7 joints each, all
  `cDeformableRubber*`, `dontCollideWithAny`, unbreakable), Istanbul Park 221
  `tyrewall_black`, Le Mans 208 tyre walls, Spa 55 `tyrewall_b_*`, Milan 113
  `green_cushion_a`, San Francisco 171 `tyrewall_black`. Still static: armco everywhere
  and Spa's 709 `tyrewall_a_*` (a piece with a static or phantom shape).
- Not verified: the whole break against a live capture of the original (needs the
  coordinator's go), the narrow phase's pair filter in a live world.

### Open for phase 3b [D, partly read]

- Armco (`armco_*_main*`): post to world (`body2uri="none"`, `cArmcoPost`, breaks at 10,
  `bodyOverride` `_rbArmcoPost` priority 1), rail 1 to post and rail 4 to `link1`
  (`cArmcoEnd`), three rail-to-rail (`cArmcoMiddle`), all `bodyOverride` `_rbArmcoMiddle`
  priority 0 (sphericalised 7000 kg, damping 14, gravity 0, `uprootProps` `rbDefault` only
  without gravity). The port's body constructor has no sphericalised bodies yet.
- B absent: 0x958200 returns the world's fixed object, 0xA74670 hands 0xA7D330's body with
  joint byte +0xC; the port's `IslandJoint`/`WorldJoint` take its body record (`fixed`), not
  yet built from the world object.
- `link1`: 0x7D3AF0 finds B through the entity's link list (+0xD8, next +0x38): the link whose
  +0x2C is this template constraint and +0x34 the linked entity, then 0x7B6D10 its body.
- 0x7D5540 (entity, other, propagate): over the entity's outgoing (+0xBC, next +0x40) and
  incoming (+0xC0, next +0x44) ENS links. When both linked entities are broken it creates the
  cross-entity constraint (0x7D3AF0 with both sides' bodies, after moving them to the
  entity's collision group 0xA696F0); otherwise, with `propagate` and the other entity's
  flag +0xEB, it breaks the other entity too (0x7D5180) and recurses without propagation.
  With byte +0x2C of `other` set it instead re-runs the incoming links' joint (0x7D2D90).
- `breakIfAngExtentHit`: callback 0x7B81A0 (joint +0x254), called when the angular limit is
  hit; posts (`c*PostBase`).
- Uproot: 0x7B7F80 picks the constraint props' `uprootProps` (+0xFC) for an uprooted
  component (+0x14); the rigid-body side switches to its `uprootProps`.
- Close-drift flags (drift routes) are jointed entities of the same kind.

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
2. ~~The joint pools in the island solve (0xB7ACC0, 0xB7B850), islands merged through joints,
   the correction solvers 0xAEF930/0xAEFBB0, and breaking~~: F477.
3. ~~Breaking (break impulses)~~: F477. Joints between a breakable's own pieces: F484.
   Still open: `breakIfAngExtentHit` (0x7B81A0), body overrides and uproot.
4. Entities: constraint components in `dynamic_props`, `link1` across entities, and the ENS
   link propagation 0x7D5540; the close-drift flags as the original builds them. Props to
   descriptor ([D]): the reader 0x7D00F0 and 0x7C04D0/0x7C7B10/0xA74670 map
   `hardRotationalFriction` to joint +0x250, `breakLinearImpulse`/`breakAngularImpulse`
   to +0x248/+0x24C, `breakIfAngExtentHit` to a callback (0x7B81A0).
