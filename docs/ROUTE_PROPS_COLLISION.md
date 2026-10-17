# Solid route barriers and props — F283

15 October 2026. Owner report: the car visibly sinks into barriers before the impact,
and marker boards and chicane tyre stacks are driven through. Research:
[COLLISION_GAPS_RESEARCH.md](COLLISION_GAPS_RESEARCH.md).

## Cause

The collision world held only the 245 packed road chunks and the invisible
`boundarylines.cqtc` sheet. The 1,021 entity collision boxes of `route_0/objects.ens` +
`objecttypes.pssg` were rendered but never collided. Tyre walls stand 1.5–4.6 m in front
of the invisible sheet (median 2.66 m), armco about 0.57 m, so the car drove into the
visible barrier before any contact existed; marker boards sit 7–52 m inside the sheet and
could never be touched.

## Change

- `grid_tools::entity_geometry::Props` reads all non-phantom entity boxes (shape transform
  then entity transform, full extents centred on the shape origin): 588 `tyre_wall`,
  243 armco, 148 `cushion_blue`, 8 `tyrestack_black`, 34 brake markers (50/100/150/200 m).
  Each box is twelve triangles in a one-leaf decoded tree with its own bounds.
- `Track::body_contacts` queries props first (box AABB cull, witness normals, <=16
  contacts), then the boundary, then road chunks. Props are one registered static shape
  after the boundary (`objects.ens`, track shape 247), so road and boundary identities are
  unchanged. The geometric pool grows from 20 to 36 slots (4 wheel + 16 props + 16 boundary).
- All props are static: data flags them infinite-mass; `physparams.xml` break/topple
  thresholds and prop materials are not yet used (shared solver material as before).
  F312 update: brake boards, tyre stacks and cushions now break into dynamic pieces at
  their `physparams.xml` thresholds when dynamic props are enabled; tyre walls and armco
  stay static pending joints. See [DYNAMIC_PROPS_RESEARCH.md](DYNAMIC_PROPS_RESEARCH.md).

## Verification

- `grid-wall-probe` (follow centreline to a switch distance, then constant steering):
  d400 steer 0.6 first touches tyre walls at 16 m/s with contact depth ~0 and peak
  0.025 m, sliding along the wall; d2000 steer -0.5 peak prop depth 0.076 m. Scenarios
  without props in the way meet the boundary (peak <=0.085 m).
- Autonomous three-lap event unchanged: 43561 ticks, 3 valid laps, 0 contacts, lap times
  identical to the previous build (props are off the racing line).
- Tests: new `donington_route_props_are_solid_boxes` (1,021 boxes, valid bounds); boundary
  test updated for shape 247 and 36 slots; 255 default + 20 asset tests, Clippy pass.

## F284: barrier ornaments (owner playtest of 9998458)

The owner's screenshots (practice log `manual-14843-11973.log`, ticks 1768/3890/6050)
showed the car resting partly inside white/blue concrete blocks with `BODY 0`. Those
blocks are not `objects.ens` entities but `ornaments.bin` instances of objects.pssg
prototypes (`concrete_block_long` 16.0x1.05x0.54 m, `concrete_block_blue/white`,
`concrete_wall_*_long`), which had no collision at all. Their back face sits on the
boundary sheet and their front face 0.41–0.49 m in front of it, so the car sank ~0.45 m
before the sheet stopped it (the third screenshot was a reversing touch with the tail).
New diagnostic `grid-ornament-reach` lists every ornament prototype protruding in front
of the boundary: concrete blocks/walls, chain-link fences `chnfen*` and trackside ad
hoardings `gg_ad_brd_*`/`don_ad_brd_*`.

Change: those four families (`concrete_`, `chnfen`, `gg_ad_brd_`, `don_ad_brd_`) get a
solid oriented box from each prototype's mesh extent per placement, added to the same
props shape (1,045 boxes; 2,066 total with the entities). This is an authored stand-in:
the original's collision for ornament instances is not recovered. Poles, lights and
other ornaments stay non-solid.

F357 (17 October): the stand-in no longer covers parts mounted on the base that lean out across
it. Long Beach's `concrete_fence_*` catch fences lean 0.63 m out over the track at the top, so
their whole-mesh box stood 0.63 m in front of the concrete at road level and the tyre stacks'
pieces spawned inside it (LONG_BEACH.md section 5.1). The boxes of every other barrier
prototype on Donington, Nürburgring and Istanbul Park are unchanged.

Replay of the owner's own inputs (`grid-replay-contacts`, DR01 from the log) is exact
for 726 ticks, then diverges at the first barrier impact (27 m/s). With the new boxes
contacts appear at 0.085 m true hull overlap, the deepest hull penetration into the
concrete block is 0.254 m for one tick before the car is pushed out, and it then slides
along the face with 0.01–0.07 m overlap. In the owner's session the same impact left the
footprint corners about 1 m inside the block region, coming to rest inside it.
Autonomous three laps still have 0 contacts; 255 default + 20 asset tests and Clippy pass.

## F286: original solver settings and contact material

- Solver: live-captured original values (WORLD_STEPPING_RESEARCH.md) replace the authored
  ones: CFM 0, min penetration 0.003, W 1.0, 5 iterations; correction CFM 0, threshold
  0.02, W 1.0, 3 iterations.
- Material: the authored restitution-0.3 soft material is replaced by the purchased
  `surface_materials.xml` row for `CAR_`. New `grid-surface-rows` shows CAR_, CONC, PLAS,
  SIGN, SHTC, ROCK, FENC, WALL and TARM all share restitution 0, friction 0.4, cfm 0.01,
  correction velocity 1, minimum bounce 0, no softness and dynamic threshold 10, so every
  car/barrier pair mixes to this row. `physparams.xml` names the prop shape materials
  (armco/metal SIGN, tyre walls/rubber PLAS, stone ROCK, wood FENC).
- Stepping decision (owner, 15 October): the prototype keeps a fixed step so behaviour is
  identical at any display rate; the original steps once per rendered frame, and its
  reference behaviour is taken at 30–60 fps. The current 16 ms step corresponds to the
  original at ~62.5 fps. An experiment with 16/3 ms steps changed the drive itself (37.5 vs
  28.8 m/s at the same record), so the prototype's step-size sensitivity remains to be
  explained before any change of the fixed step.
- Owner impact replay: no bounce now; peak hull overlap 0.251 m held for two ticks and
  removed by ERP 0.2, then sliding contact 0.004–0.036 m. 275 tests, Clippy and the
  autonomous three laps pass.

## Remaining (from the research)

The ~0.25 m transient at 27 m/s comes from the 16 ms step (0.43 m of travel per tick)
and deformation's softened impact response; the original steps at the frame delta
(about 6 ms on the owner's 165 Hz display).

Deformation's pre-solve contact limit lets the hull crumple 0.35–0.65 m into a wall at
18 m/s; the prototype steps at 16 ms against the original's frame delta; solver defaults
(iterations 20, CFM 1e-5, min penetration 0.001, W 1.3) and per-material coefficients
differ; cold queries occasionally miss deep overlaps; props are not breakable/dynamic.
Each needs live confirmation before changing.
