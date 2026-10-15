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

## Verification

- `grid-wall-probe` (follow centreline to a switch distance, then constant steering):
  d400 steer 0.6 first touches tyre walls at 16 m/s with contact depth ~0 and peak
  0.025 m, sliding along the wall; d2000 steer -0.5 peak prop depth 0.076 m. Scenarios
  without props in the way meet the boundary (peak <=0.085 m).
- Autonomous three-lap event unchanged: 43561 ticks, 3 valid laps, 0 contacts, lap times
  identical to the previous build (props are off the racing line).
- Tests: new `donington_route_props_are_solid_boxes` (1,021 boxes, valid bounds); boundary
  test updated for shape 247 and 36 slots; 255 default + 20 asset tests, Clippy pass.

## Remaining (from the research)

Deformation's pre-solve contact limit lets the hull crumple 0.35–0.65 m into a wall at
18 m/s; the prototype steps at 16 ms against the original's frame delta; solver defaults
(iterations 20, CFM 1e-5, min penetration 0.001, W 1.3) and per-material coefficients
differ; cold queries occasionally miss deep overlaps; props are not breakable/dynamic.
Each needs live confirmation before changing.
