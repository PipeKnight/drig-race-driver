# Box primitive contacts (AE8AD0) and the soak residuals

17 October 2026, Claude Code. The F342 soak follow-ups: thin props driven into the
ground and players falling out of the world after a hard hit off the track
([SOAK_TESTING.md](SOAK_TESTING.md), "Residuals (F351)"). Static evidence from the
hash-pinned analysis image (`aac46a64…4514`): Ghidra 12.1.4 headless `-readOnly
-noanalysis` on a private copy of the dynamic-props project and capstone 5.0.6
through the private `pe.py`. Pseudocode, scripts and traces are in the main
checkout's private `local/soak-residuals/` (`decomp/b1..b6`, `scripts/`, the trace
logs and the binaries named below). Grades: [V] verified by running, [D] read from
the disassembly or decompilation, [I] inferred.

## The original's pair dispatch

[D] The world constructor B18E70 fills a 12 x 12 matrix of 16-byte entries
(collide, two callbacks, swap flag) at `(kindA * 12 + kindB) * 16`, as
PACKED_MESH_CONTACTS.md found for two entries. The whole matrix:

| kinds | function | note |
|---|---|---|
| any two of {0, 3, 4, 5, 6, 7} | AE8AD0 | generic convex pair; (b, a) entries carry the swap flag |
| {0, 3, 4, 5, 6, 7} with 1 | AE5010 | |
| {0, 3, 4, 5, 6} with 2, and 7/8 | AE8DB0 | |
| {0, 3, 4, 5, 6, 7} with 9 | AE6860 (3: AE7150, 2: AE73B0) | |
| {0, 3, 4, 5, 6, 7} with 10 | AE95B0 | |
| {0, 3, 4, 5, 6, 7} with 11 (packed road chunks) | AEA1F0 | per road triangle |
| 9/9, 9/10 | AEE210, AE7940 | |

[D] Support functions per kind (table E534A0) identify the convex kinds: 0 box
(B41750, `sign(d) * half`, half extents at +0x240/+0x244/+0x248), 3 sphere,
4 capsule, 5 cylinder (B47A70, radius +0x240, half height +0x244), 6 a rounded
segment/disc shape, 7 the collision mesh (car hulls; B45610). The margin is at
+0x220 in every support. Kind 13 is the triangle used inside AEA1F0.

So in the original:
- a car hull against a route prop box (TEMPLATESHAPEBOX, kind 0) is one AE8AD0 query
  over the whole box;
- a car hull against a broken piece (a box or a cylinder) is AE8AD0 too, and piece
  against piece;
- only the packed road chunks are queried triangle by triangle (AEA1F0).

[V] The port did not follow this for boxes. Route prop boxes were twelve triangles
in a one-leaf packed tree, queried through the packed path. Car against piece used
the car-to-car triangle-soup path, and piece against piece an authored
separating-axis test.

## AE8AD0

[D] AE8AD0(A, B): B7F740(A, B, poses, supports, pair direction) → witnesses wA, wB;
`n = (wA - wB) / |wA - wB|`, depth `|wA - wB|`, nothing at or below 1e-3 (CB2294);
AEE640 orientation with the two pose origins; AE2FA0 manifold
(B, A, feature function of B, of A, wB, n, depth).

- **B7F740** expresses B in A's frame. An early test along the cached pair direction
  (support of A, batch depth of B) needs `mA + mB + depth >= -0.01` (D426B0). Then
  B7F520, and the witnesses go back to world space.
- **B7F520** runs B7EA60 (the distance loop the port has in `convex_distance`) with
  margin² = (mA + mB)². If the shapes are within their margins, the witnesses are
  moved by `-mA·n̂` and `+mB·n̂`. If they intersect, B7ED10 runs.
- **B7ED10** is generic penetration. Samples: the 44 template directions (CD01, in
  A's frame), then ±search direction and ±each axis of B (relative). A kind-13
  triangle A also adds ±its normal. The minimum batch depth is found, then the same
  two shifted distance queries refine it, as the port's triangle path already does
  (`Context::penetration`).
- **AE2FA0** calls A's feature function with `-n` and B's with `+n`, then dispatches
  on the two feature *types* (table D3AD70). A box face (B3D890) and a mesh face
  (B45310) are both type-1 polygons, which gives the polygon/polygon clipper the port
  already has (`contact_manifold::assemble`).
- **B3D890** (box features) always returns the face along the dominant local axis of
  the direction: x on ties, then y. It has four vertices, the shape margin, and is
  oriented (a clipping reference) when that component exceeds 0.99 (CB0DC8).

## Port (F351)

- `convex_distance`: the distance loop, batch depth and penetration take a convex
  point set instead of a triangle (`points_support`, first maximum). The triangle
  path keeps every operation. `Context::contact_points` is B7F740/B7F520 with a
  margin for A and B7ED10's extra samples (`Extra::Triangle` = mesh axes and triangle
  normal, `Extra::Axes` = B's axes).
- `contact_manifold`: `mesh_feature` (B45310) is split out. `assemble_faces` clips
  polygons of any vertex count; for triangles it is the old code.
- `box_contacts` (new): AE8AD0 for a moving mesh against a static box. The box takes
  the packed path's static role (query shape with the point set, second manifold
  feature), so the records keep the packed convention (the normal pushes the mesh).
  The original runs AE8AD0 with the mesh as A. The two assignments differ only in
  signs and witness order, which AEE640 and AE2FA0 treat symmetrically [I]. The 44
  directions are rotated from the mesh frame into the world.
- `entity_geometry`: every route prop box is a `BoxShape` (axes and half extents
  from its eight corners, margin = the shape's `shell`, 0 for the authored ornament
  barrier stand-ins) and goes through `box_contacts`. The packed triangle data of the
  boxes is gone. Pieces against static boxes take the same path, because they share
  `Track::body_contacts`.
- Unchanged: car against pieces (the car-to-car hull-triangle path), piece against
  piece (the authored separating axes, margin = the larger shell), and car against
  car (the packed decoded path). The original uses AE8AD0 for all of them. The next
  section explains why the first two were tried and not adopted.

[I] Deviations, recorded: no per-pair direction cache (every query starts along
+x, as the port's other cold queries do); the vertex order of a box face matches
the one decompiled branch, and the others are assumed to follow it; the box face
normal carries the face's sign (the clipper aligns it with the contact normal
either way).

## Players falling out of the world (seed 1087)

[V] Reproduced on the F342 build 4ffec8b (a scratch worktree, since removed; its trace changes are `local/soak-residuals/wip/f342-trace.patch`;
`grid-soak replay local/soak/final-long-*/seed-1087.inputs.tsv`): the same tick and
failure on both routes. Current main no longer reaches them: F345 changed the
field's paths.

- Nürburgring: the player leaves the track at 39 m/s and hits
  `tyrewall_black_yok` entity box 1508 at (-283, 607, 28) (1.0 x 1.13 x 4.0 m,
  standing on the road). Ticks 2236-2241: the car stops in 6 ticks while the box's
  per-triangle contacts include normals (0.61, -0.79, 0), (0.33, -0.92, -0.22) and
  similar, up to 0.9 m deep. The correction pass moves the car 0.5 m down per tick.
  At tick 2242 its centre is 0.76 m below the flat road (ground 606.53 by a
  vertical ray). Every road contact then points down and the car falls.
- Donington: the same thing at 45 m/s against a `tyre_wall.max` box at
  (762, -6, 46), with a `chnfen_long` ornament stand-in next to it.

Root cause: a box queried as twelve separate triangles. Each triangle is a plate,
and the penetration direction for a plate can be its own normal even when that is
the box's bottom or inner face. As one convex query (AE8AD0), the minimum overlap
over the whole box cannot point down into the road for a box that stands on it.

[V] With the port, both replays run through their recorded ticks (Nürburgring 2404,
Donington 6549). At the recorded Nürburgring pose, wall 1508 gives contacts with
normal (0.0004, 1.0, -0.004), 1.08 m deep: the car is lifted out over the wall's top.
Test `entity_geometry::a_car_deep_in_a_tyre_wall_is_never_pushed_down` uses the
recorded hull matrices from both routes. The old triangle path gives downward
normals there; the new path gives none.

## Thin props driven into the ground (seed 12): the road triangles' margin

[V] Reproduced on main 307fe9b: `grid-soak replay` of
`local/car-extents/soak/replay-seed12/seed-12.inputs.tsv` fails at tick 3003, digest
3a013f64a39c511d. The lower half of a Donington brake-marker board
(breakable 116; 0.94 x 0.47 x 0.16 m, 4.9 kg) spawns at tick 2830, when the player
breaks the marker at 33.6 m/s while spinning (yaw rate 2.6 rad/s). The same board
at (-406.7, -16.0) is the hot spot of the class: 11 of the 24 piece falls of F342's
long Donington run, and seeds 1012, 1025 and 1029 of the runs below.

- The break comes after the step in which the intact box's contact was capped at
  `threshold · dt`, so the car is already 0.4-0.5 m inside the new piece. The
  original has the same order (DYNAMIC_PROPS_RESEARCH.md, "Break").
- The car contacts (eight, 0.38-0.40 m deep) all point straight up. [V] Over the 44
  CD01 directions and their negatives, the smallest overlap between the car hull and
  the board is straight up: 0.48 m, against at least that in every other direction.
  B7ED10 takes this minimum (plus ±search direction and the board's axes), so the
  original's AE8AD0 also pushes the board down [I].
- What differs is the ground. The board stands 2.14 cm above the grass. In the port it
  had no ground contact in its first step: its own 2 cm shell plus a road-triangle
  margin of 0. One correction pass then moved it 0.38 m down, its centre ended 0.13 m
  below the ground, the road contacts pointed down and it sank.
- [V] Live original: all eight road chunk shapes (kind 11) in the read-only captures
  `local/behavior/runtime-collision-20261006T144*.json` have +0x220 = 0.02
  (0x3CA3D70A).
- [D] AEA1F0 gathers candidates through B212D0. B212D0 copies the packed shape's
  +0x220 onto the cache's temporary kind-13 triangle (`triangle+0x220 = chunk+0x220`),
  and the other branch does the same at 0xAEA2AA..0xAEA2B8 (B21306..B2130C in B212D0). B212D0 also widens the body's bounds
  by that margin. The triangle then enters B7F740 as shape A with margin 0.02.
- [V] The port had zero triangle margin, the scope of the F221 packed proof ("zero
  triangle margin"). With 0.02 + 0.02 the board is inside its margins at spawn and
  has road contacts from the first step. Test
  `dynamic_props::every_brake_marker_board_touches_the_ground_when_it_spawns`: all 34
  marker boards at their spawn pose. With a zero road margin it fails on breakable
  116, the hot spot.
- [D] The other remedy the original has for a sunk piece, AEA1F0's backface
  correction for an up-facing road triangle, never runs. B1AB30 gates it on the
  chunk's rectangle +0x294..+0x2A0. The constructor fills all four from the global
  E51D40 (FLT_MAX, read-only use). The only other write of +0x294 in the physics range,
  the one-field setter B0AD10, is referenced by no code or data (the chunk vtable
  D3D468 does not hold it). The other writes of +0x298..+0x2A0 found there are integer
  stores in other functions (B0AD..B0B1, the world constructor B19494); none was traced
  to the chunk class. B1AB30 tests `min <= x <= max`, which FLT_MAX..FLT_MAX never
  satisfies (as F342 found).

Port (F351): `packed_contacts::Settings::shape_margin` is the packed shape's margin.
`Context::contact_triangle` gives it to the triangle, and the manifold uses it as the
triangle feature's margin. The candidate bounds widen by it. `Track` passes
`ROAD_MARGIN` (0.02) for road chunks. Kept at 0: the boundary sheet (its shape margin
is not established), the authored car-to-car path and the diagnostics.

## Tried and not adopted: AE8AD0 for car against box pieces, summed piece margins

The original also uses AE8AD0 for a car hull against a broken box piece, and its pair
query sums the two shape margins (B7F520: mA + mB). Both were implemented and
measured. The patches are kept in the main checkout as
`local/soak-residuals/wip/v3-commit.patch` and `v4-uncommitted.patch`.

- [D] AE2FA0's clipping reference for two bodies: if only one feature is oriented,
  that one. If both are and one shape has no body (`+0xD8`), the bodiless shape's
  feature. Between two bodies, the feature more aligned with the normal.
- [V] What it fixed: two board losses of the margin build. Seed 1056 of its long
  Donington run is a board of breakable 105 at (-11, -331). The other is main's
  seed-1062 inputs replayed with the margin. Both boards held at spawn. Then the car's
  triangle-soup contacts mixed up- and down-pointing normals (up to 0.57 m deep) in
  one tick and spun the board at 37 rad/s, so its edge rotated 0.3 m into the road.
  With one direction per pair, both replays pass.
- [V] What it broke: targeted `--cars 1 --chaos prop --ticks 8000`, seeds 1..80 on
  Donington:

  | Build | Fails |
  |---|---|
  | main 307fe9b | 1 |
  | margin (this push) | 0 |
  | + car-against-box-piece AE8AD0 | 2 (seeds 17, 71) |
  | + summed piece margins | 4 (seeds 19, 22, 54, 71) |

  The margin build passes seed 1040 of the long run, a 29 m/s hit on the 50 m
  marker's two boards. The car-against-box-piece build loses it: the car overlaps the
  upper board by 0.84 m, the minimum-overlap direction is vertical, and three
  correction iterations cannot push it back through the lower board to the ground.
  The run is identical to the margin build up to that break.
- [I] Why: the original's single direction per pair takes the smallest overlap. Its
  live world steps once per rendered frame (6 ms on the owner's 165 Hz display;
  WORLD_STEPPING_RESEARCH.md), so a car at 30-50 m/s is 0.2-0.3 m into a new piece
  and the smallest overlap is usually along the car's travel: the board flies off. At
  the port's fixed 16 ms step (F320) it is 0.5-0.85 m in, and the smallest overlap
  is often vertical: the board is pressed down. The triangle soup pushes it off at
  odd angles, which happens to lose fewer boards at this step.
- Decision (made overnight; the owner's to revisit): keep the hull-triangle path and
  the larger-shell piece margin until the stepping question is decided. Adopting the
  original's pairing without its frame-rate step loses more pieces.

## Race results and the solo lap (F351)

[V] Release, cargo/rustc 1.99.0; outputs in the main checkout's `local/soak-residuals/runs/identity/`:

- Solo `grid-opponent-race ROOT CD01 BP01 1 11200 ai`: byte-identical to main
  (sha256 aad9a07a…e41b, laps 88.42029812185638 / 86.28228501935392), with both
  changes. The Lacetti's body never comes within the margins of a prop box or the
  road on the AI line.
- 12-car Donington (`12 12000 ai`, DF01): the box change alone is byte-identical
  (a8241241…6341). The road margin changes it. The first divergence is car 9 at
  tick 2749: its floor comes within 2 cm of the road near (677, -166), and those
  light contacts (at most 4 mm inside the margin) did not exist with margin 0. Car
  contacts go from 1604 to 1211, places 8/9 and 10/11 swap, and the winner's time
  goes from 2:57.968 to 2:58.016.
- 12-car Nürburgring (14000 ticks): the box change alone first diverges at tick
  3163. Car 10 drives over a flat `close_drift_flag.max` entity box and meets its
  2 cm shell (contacts at most 3 mm inside it). With the road margin as well, the
  race already differs at tick 1500. That earlier divergence was not traced; the
  same floor contacts as on Donington are the likely cause [I].

The first divergences come from temporary per-tick pose prints and a margin toggle
(GRID_POSE_TRACE, GRID_ROAD_MARGIN_ZERO). They were removed before the commit.

## Cylinders and spheres (FIDELITY_BACKLOG row 20, F462)

19 October 2026, Claude Code. Static route cylinders (kind 5: cones, foam posts,
poles, drift flags, Yokohama's barrels and pipes) and spheres (kind 3: the Milan
balloons) were not route collision; cars drove through them. Pseudocode in this
worktree's private `local/robust/decomp/r20a..r20m`.

### The original [D]

- Construction: 0x7B96D0 fills the cylinder descriptor with the template radius
  times the smaller horizontal scale and the template height times scale.y times
  0.5, so +0x244 is the half height; B46180 stores them at +0x240/+0x244.
- Support B47A70: the direction in the shape frame (0x770AB0); if its horizontal
  length is at most 1e-6, `(0, ±h, 0)`, else `(r dx/l, ±h, r dz/l)` (the sign of h
  follows dy, -0 - h below), back to the world, plus the shell (B41BC0). The
  cylinder is exactly round in the distance query. Sphere support B4B0A0: the
  centre pushed by radius plus shell.
- Feature table 0xE5223C: cylinder B47590, sphere the default ADB8F0 (returns 0).
  B47590 returns a typed feature from `c = axis . direction`:
  - `|c| > 0.707` (D11548): type 2, the cap disk facing the direction (centre ±
    h·axis, radius r, the shell as margin), a clipping reference when `|c| > 0.99`;
  - `0.044 < |c| <= 0.707` (D19778): type 1 with two vertices, the generator nearest
    the direction (margin = shell);
  - otherwise type 3, the side: radial normal, axis, bottom centre, radius plus
    shell, height 2h, always a reference.
- AE2FA0 calls the feature functions (A with -n, B with n). A feature function
  returning 0 sends the pair straight to the single fallback contact. A type-3
  feature becomes its generator segment (AE2F10) when the other feature is a
  reference. Flags then decide the common normal (the reference's, signed along
  n); both features' vertices move by their margins; the pair table 0xD3AD70
  (`typeB + 4 typeA`, function and swap flag) dispatches: polygon/polygon AEB630,
  polygon/disk AEB8D0, polygon/side AE2710, disk/disk AEBC00, point types 0 with
  AE2C90/AE2E80. After the handler the fallback contact joins at most two
  contacts; when one feature is a two-vertex segment, only an empty result.
- AE2710 (polygon edges against the side): each edge is cut by the finite cylinder
  (AE16A0: caps at y = ±h with codes ±2, the side quadratic with code 1, the
  parameter interval clamped to 0..1). The cut's middle becomes a contact whose
  depth is probed along 2(r + shell)·n through a cylinder of twice the height;
  each end becomes a contact with depth 0 when it is a true side crossing, else
  probed the same way. The end probe starts at the edge start, not at the end
  (0xAE2B8F passes the edge cut's stack slot); ported as found.
- AEB8D0 (disk first): a reference disk clips the polygon to its circle (AE1BD0:
  vertices inside, edge crossings) and then to the cap plane (AE2000, the port's
  `clip`), depth below the cap. Fewer than two points, or a disk that is not a
  reference: the disk becomes a rim triangle (AE2210, rotation 2.0944 rad about
  the normal, first vertex towards the direction in the cap plane, or the shape's
  x axis) and goes to AEB630.
- `cylinderFacets` (world config +0x84, 10): copied into the world construction
  struct by A749A0 (index 0x21), consumer still untraced. Neither the support nor
  any feature or pair handler above reads it. The kind-5 contact geometry is
  round, not a 10-gon.

### Port

- `grid_world::round_contacts`: `CylinderShape`, `SphereShape`, `contacts()` (AE8AD0
  as for boxes, B7ED10 sampling the shape's axes). `convex_distance` takes a
  `Static` shape (a point set or the cylinder support) in the distance loop, batch
  depth and penetration; point sets run the same operations as before.
  `contact_manifold::assemble_rule` adds the segment fallback rule.
- The mesh face is a type-1 polygon, so the ported pairs are polygon/side,
  polygon/segment, polygon/disk (both branches) and the sphere's fallback.
  Floating-point order follows the decompilation where it was read
  (0x770AB0, B47A70); the AE2210 rotation is Rodrigues' formula (the original
  builds the matrix in 0x772670, order not compared).
- `entity_geometry::Props::read_route` adds every solid cylinder and sphere with
  its entity index (normalised axes, radius x min horizontal scale, half height x
  scale.y; a sphere takes its smallest scale [I: the sphere scale rule was not
  read]). `positions` hold the local bounding box corners (bounds, the overlap
  diagnostic).
- Breakables: an intact cylinder now counts as the intact route shape, so
  Yokohama's 77 `gg_barrel.max` break. Sphere pieces stay unported. Pieces keep
  their 12-sided prism hulls (the original's piece cylinders are the same round
  kind 5; a round piece needs the body pair paths, not a facet count).

### Counts [V]

Route shapes (boxes plus round shapes) after the change, `route` asset tests:
Nürburgring 2911 (71 round), Jarama 2251 (16), Yokohama 1631 (128), Spa 1552
(37), Milan 2340 (104: 65 cones, 39 balloon spheres), Shibuya 1212 (164),
San Francisco 1309 (1), Le Mans 2510 (97), Haruna route 1 2034 (524). Donington,
Istanbul, Long Beach and the Detroit stadium have none. Breakables change only on
Yokohama (0 to 77).

### Round pieces (F465)

[D] The pieces' cylinders and spheres are the same kinds 5 and 3, so the moving side of
every query can be round too. 0x7DB060 gives a sphere template's volume as r³ x 4.18879
(CFAAF4) x density; the cylinder's (0x7DAFC0) is r² h π x density.

Port:
- `convex_distance::Hull` is the moving body: a collision mesh, a cylinder (axis = local y,
  B47A70's local support) or a sphere (its centre; the radius joins the body margin, B4B0A0).
  Every mesh path takes it (`packed_contacts`, `box_contacts`, `round_contacts`,
  `continuous::sweep_packed`, `Track::body_contacts`/`sweep_road`, `Props`, the boundary).
  Mesh bodies run the same operations as before.
- The manifold for a round body is the cylinder dispatcher above with the roles swapped:
  B47590 gets -n (A's direction), the polygon (a packed triangle or box face) is feature B,
  and when both are references the static polygon keeps its flag. Records of a moving
  cylinder are not flipped. A sphere gives the fallback contact.
- A car against a round piece is AE8AD0 with the piece as the static side
  (`BodyGeometry::car_contacts_round`, `round_contacts::contacts`). Box pieces keep the
  hull-triangle path ("Tried and not adopted" above).
- Not ported: two round shapes (a cylinder piece on a cone, or piece against piece) give
  AE2FA0's fallback contact only (AEBC00 disk/disk and the segment pairs are not ported);
  piece against piece stays the AUTHORED face-axis test over the 12-sided prism or, for a
  sphere, an inscribed icosahedron. The continuous stage's radius for a round body is its
  smaller half extent [I]. The lost wheel (9A8BB0) keeps its prism (FIDELITY_BACKLOG row 24).
- `dynamic_props`: sphere pieces are accepted (`Primitive::Sphere`, mass from 0x7DB060,
  inertia 0.4 m r² [I]). No route's catalog changes: the templates with sphere or cylinder
  pieces that stay static (Haruna's poles and signs, cones, foam posts, drift flags) have more
  than one shape behaviour per entity, which `Catalog::read` refuses (row 16 work).
