# Collision gaps: barrier sinking and non-colliding props (research)

15 October 2026, Claude Code research session. Read-only investigation of two owner
reports from **Play GRID Time Trial.cmd** / **Play GRID Body Contacts.cmd**
(Lacetti, Donington):

1. The car sometimes visibly sinks into a barrier before the collision response.
2. Distance boards and tyre stacks near the chicane do not collide at all.

No crate source, launcher, Steam file or setting was changed. Original GRID was not
launched. Measurements use a private copy of committed HEAD `a016fff` under
`local/collision-gaps/src/` (crates exported with `git archive`, never edited in
place under `crates/`), with diagnostic-only instrumentation and three private probe
binaries. Proprietary outputs (for example the extracted `physparams.xml`) stay under
ignored `local/`.

Labels used below: **[verified]** = measured in the prototype or read directly from
purchased data/executable; **[static]** = read from decompiled/disassembled original code,
not executed; **[inference]** = reasoned, not proven.

## Summary

| # | Cause | Effect on the owner report | Status |
| --- | --- | --- | --- |
| B1 | Prototype collision world = 245 packed `track.jpk` chunks + one `boundarylines.cqtc` sheet. The 1,021 entity collision boxes in `route_0/objects.ens` are rendered but never added. | Distance boards and tyre stacks have no collision at all. | [verified] |
| A1 | Tyre walls, armco and cushions are entity boxes that sit **in front of** the invisible CQTC boundary sheet (tyre walls: median front face 2.66 m in front; armco: about 0.57 m). | The car drives into the visible barrier until it reaches the invisible sheet behind it. | [verified] geometry |
| A2 | With `--deformation` (both launchers), the ported pre-solver deformation contact limit caps the contact impulse in proportion to penetration. | At about 18 m/s into a wall the response takes 2 to 4 ticks; the deformed hull overlaps the sheet by 0.35 to 0.65 m. | [verified] in prototype |
| A3 | Fixed 16 ms world step, where the original world steps at the frame delta (about 6 ms on the owner's 165 Hz PC). | Bigger first-tick overlap and a softer effective cap (the cap is per step). | [verified] prototype; [static]+live delta for original |
| A4 | Solver/material parameters are authored, not the original's: threshold 0.125 m, restitution 0.3, soft K/S. | Residual 5 to 12 cm rest penetration, and a bounce. Little effect on peak sink. | [verified] differences; [static] original defaults |
| A5 | Each tick builds a cold contact context per pair. Some deep overlaps of the 100 m-tall sheet triangles return no witnesses. | The occasional tick with zero contacts at 0.4 to 0.5 m overlap. | [verified] |
| A6 | Car shape margin is 0. Original shapes use 0.02 (spawn shape descriptor, all entity shells). | Small effect. | [static]/[verified] small |

Render alignment is **not** a cause. The visible Lacetti and the collision hull agree
within 3 cm at the front, rear and corners. Body offset `0x1564` is 0, so the visual
and collision Y offsets are identical. Only the mirror/wheel-arch region is wider (0.10 m
per side) and the roof/aerial taller (0.2 m) than the 51-point hull. [verified,
`grid-visual-extent`]

## A. Penetration measurements

Probe `grid-collision-gaps`
(`local/collision-gaps/src/crates/grid-tools/src/bin/grid-collision-gaps.rs`) drives the
same setup as `grid-practice-drive` / the time trial: deforming body geometry, BP01
events, DF01 callbacks, pure pursuit to a course distance, then straight steering into the
barrier. It records every tick: body contacts (count, max penetration, normal) from an
instrumented `BodyGeometry::contacts`, the post-solve normal velocity at the contact
point, and an **independent** overlap depth. That depth is the minimum separating
translation along the face normal of the **current (deformed)** hull against each
boundary sheet triangle.

Four impact sites (course distances 150/400/700/1000 m, speed target 30 m/s):

| Site | Approach normal speed | Deformation on (launcher config): max overlap / hull crush / contact-point vn after ticks 1..3 | Deformation off: max overlap |
| --- | --- | --- | --- |
| d150 (-575.5,-52.0) | 8.5 m/s | 0.122 m / 0.06 m / +0.7, +2.2, 0 | 0.135 m (first tick only) |
| d400 (-595.4,-146.3) | 17.9 m/s | **0.649 m** / 0.67 m / **-17.4, -10.3**, 0 | 0.057 m |
| d700 (110.3,-342.3) | 18.8 m/s | **0.350 m** / 0.63 m / **-18.4, -9.7, -2.1** | 0.196 m (first tick only) |
| d1000 (69.4,-364.8) | 7.5 m/s | 0.157 m / 0.26 m / -4.5, +1.4, +1.1 | 0.053 m |

[verified] Without deformation the first solve reverses the contact-point normal
velocity (+1.4 to +2.2 m/s, from restitution 0.3). The only overlap left is the
first-tick v·dt fraction. With deformation the first solve removes only about 3% of an
18 m/s approach, so the hull keeps moving into the wall for 2 to 3 ticks. The mechanism:

- `deformation_preparation` (9A0C90 port in `grid-world/src/deformation_contact_limit.rs`)
  returns `cap = multiplier * (contact penetration + projected)`. The cap is written to
  contact `+0x3c`.
- `contact_rows` then bounds the row impulse with `upper = time * previous_impulse`.
- A fresh contact with 0.046 m penetration therefore gets a tiny cap. The cap grows only as
  penetration grows, which makes this a penetration-proportional spring, applied **once
  per world step**.

This is the verified original contract for the given inputs. It is not a porting bug in
the function itself.

Controls, each at the same impact ticks:

- Original-default solver parameters, original material and 20 iterations
  (`PROBE_SOLVER=original`, `PROBE_MATERIAL=original`, hard material): peak overlap is
  unchanged within ±0.01 m. The first-tick vn stays at -17.4 m/s. Iterations and softness
  are not the limiting factor.
- 6 ms world step (`DELTA = 0.006`, private build only): d700 peak drops 0.350 → 0.164 m;
  d400 0.649 → 0.553 m (that site also hits the cold-query gap A5); d150 0.122 → 0.075 m.
- Car margin 0.02 (`PROBE_MARGIN`): mixed, ±0.1 m.

The deformed collision hull crushes by 0.42 to 0.72 m in the high-speed hits. Whether the
**visual** dent matches that crush has not been verified. The visual mesh uses the
independent policy in `crates/grid-tools/src/body_deformation.rs` (sampled grid delta).
The collision hull uses `collision_mesh::build_from_deformed_source`. [open]

### A1. Visible barriers in front of the invisible sheet

Probe `grid-entity-collision` places each entity box with the renderer's composition order
(shape transform, then entity transform, row-vector, full-extent box centred on the shape
origin). The visual tyre-wall meshes coincide with the boxes, with a median centre
offset of 0.01 m. Each box is compared with its nearest `boundarylines.cqtc` triangle. The
positive side of that triangle (`cross(t1-t0, t2-t0)`) is the side the car was on at
every measured impact.

| Template (route_0) | Count | Within 6 m of sheet | Front face in front of sheet (p10 / median / p90) | Back face |
| --- | ---: | ---: | --- | --- |
| tyre_wall | 588 | 567 | 1.53 / **2.66** / 4.65 m | median 1.54 m in front |
| armco_blue_main / armco_white_main | 120 / 123 | all | 0.56 / **0.59** / 0.62 m | about 0.45 m in front |
| cushion_blue | 148 | 145 | box 0.46 / 1.89 / 3.40 m | — |
| tyrestack_black | 8 | 8 | 4 protrude 1.07–2.28 m; 4 are behind the sheet | — |

At every measured impact site (d150, d700, d1000) the nearest prop is a tyre wall
1.0–2.4 m away that lies entirely 1.3–2.8 m in front of the sheet. [verified] The car
therefore drives through the visible tyre wall (or 0.5 m into the armco rail) before
any contact exists. In the original these boxes are collidable. Evidence:
`infMass = 1`, `shell = 0.02` and physical shape props. This is the larger part of the
"sinks into the barrier" report at tyre-wall and armco sections. [inference for the
original's runtime use; [verified] for the data]

### Original configuration evidence ([static])

- **Physics world defaults.** Constructor `6195F0` and loader `A74F00` read element
  `PHYSICSENGINE`, then `A749A0` and manager constructor `B7A9E0` (config `+0x0C..+0x34` →
  manager `+0xCC..+0xF4`):

  | Setting | Default | Manager offset |
  | --- | --- | --- |
  | globalCFM | 1e-5 | +CC |
  | globalERP | 0.2 | +D0 |
  | minPenetrationDepth | **0.001** | +D4 |
  | quickStepW | **1.3** | +D8 |
  | numQuickStepIterations | **20** | +DC |
  | firstOrderCFM | 0 | +E0 |
  | firstOrderERP | 1.0 | +E4 |
  | firstOrderMinPenetrationDepth | 0.02 | +E8 |
  | firstOrderMinConstraintError | 0.02 | +EC |
  | firstOrderQuickStepW | 1.0 | +F0 |
  | firstOrderNumQuickStepIterations | 5 | +F4 |

  The scheduler defaults are maxfps 60, minfps 30 and maxTimesToRun 1. Scheduler `A72930`
  uses `+8/+C/+10`.
- **Prototype values.** The prototype uses `[.. 0.01, 0.2, 0.125, 0.8 ..]` with 4
  iterations, and correction `{4, 0.01, 1, 0, 0.8}`
  (`crates/grid-tools/src/source_vehicle.rs` lines 316–343). The mapping of the
  prototype's `parameters[5..8]` to `+CC..+D8` follows the documented field order and
  `IMPACT_STEP.md`. [inference]
- **No override file found.** `PHYSICSENGINE` exists only as an exe string. No installed
  file (byte scan) contains `maxTimesToRun`/`minPenetrationDepth`. The defaults cannot be
  the whole story: with maxfps 60 the 165 Hz world would step 1/60 s per frame. The live
  world delta 0.00606 s (`LAUNCH_AND_GRID_START.md`) shows that something overrides at
  least maxfps. Treat the defaults as a baseline and confirm `manager+CC..+F4` and
  `scheduler+8..+10` by read-only live capture. [open]
- **Physical materials** (`surface_materials.xml`). SHTC (sheet), CONC, PLAS, SIGN and CAR_
  all use friction 0.4 and otherwise loader defaults: restitution 0, cfm 0.01,
  correctionVel 1, minBounceVel 0, no softness, dynamicThreshold 10. The prototype's shared
  material `[0, 0.3, 0.4, 0.01, 0.5, 0.2, 2, 0.5, 3, 0.75, 20, 0, 0.125]` is authored.
  [verified]
- **Shape margins.** The car placement shape built in `612C90` passes descriptor
  `+0x68` (→ shape `+0x220` margin) = `DAT_00CB0934` = **0.02**. All 1,021 Donington
  entity shapes have `shell = 0.02`. The main car shape path (`9B8A50` → `A72EF0`,
  descriptor from its caller) was not resolved. [static] The prototype uses margin 0
  (`body_geometry.rs` `BodyGeometry::read`).

### A5. Cold contact queries

At d400 tick 1375 the deformed hull overlaps CQTC triangles 48 and 49 by 0.41 m. Both are
accepted candidates, yet `convex_distance::Context::contact` from a cold context returns
direction 0 and no witnesses. The tick therefore has zero body contacts (also tick 1385).
Seeding the query with the face normal recovers witnesses in some of these cases but not
all. The original retains per-pair state (triangle collision caches; see
`RETAINED_TRIANGLE_MESH_DISTANCE.md`). `Track::body_contacts` creates a fresh context per
pair per tick. [verified prototype; inference about original frequency]

## B. Missing props

[verified] `Runtime::prepare_body_geometry` (`source_vehicle.rs:429`) attaches only
`boundarylines.cqtc`. `Track::shape_count` = 245 + 1. `Track::body_contacts` queries only
the boundary, then the packed chunks. The scene loader `append_entities` (`grid-app`
`main.rs` about line 580) only **renders** entities and counts boxes. Static ornaments
(1,985) carry no collision data (`ornament_attributes.xml` holds LOD/instancing only).
Correction (BARRIER_COLLISION.md): 836 of them are drawn copies of `objects.ens` basic
entities (`TEMPLATEBASICENTITYINSTANCE`), whose templates do have infinite-mass collision
shapes that the original builds at level setup (0x7CB440).

Donington `route_0` entity inventory (`grid-entities` report, all `infMass = 1`, all box
shapes with `shell = 0.02`):

| Template | Count | Box (w × h × l, m) | Shape props → material / density |
| --- | ---: | --- | --- |
| tyre_wall | 588 | 4.01 × 1.13 × 0.985 | sDeformableRubber → PLAS, d700 |
| cushion_blue | 148 | 2.165 × 0.893 × 0.736 | sPlastic → PLAS, d700 |
| armco_white_main / armco_blue_main | 123 / 120 | 3.96 × 0.74 or 0.96 × 0.091 | sArmco → SIGN, d2610 |
| tyrestack_black | 8 | 1.826 × 0.943 × 0.599 | sRubber → PLAS, d700 |
| gg_brake_marker_01_i_{left,right}_{50,100,150,200}m | 34 | 0.94 × 0.96 × 0.16 | sCardboard → PLAS, d70 |
| flag_a / flag_b | 24 | none (cloth) | — |

- **Distance boards.** All 34 boards stand 9–12 m from the course centreline and 7–52 m
  inside the boundary sheet. Only one touches a non-ground packed triangle (RMBL curb).
  They are unreachable by any current collision shape. [verified]
- **Tyre stacks.** The 8 stacks are at (-107,145), (-94,88) and (-74,84), 18–23 m from the
  centreline. Four protrude 1.07–2.28 m in front of the sheet; four are behind it.
  [verified]
- **Break behaviour data.** `raceload.jpk` contains `physparams.xml` (private copy:
  `local/collision-gaps/physparams.xml`). It defines `TEMPLATESHAPEPROPS`
  (material + `massSource` density), `TEMPLATERIGIDBODYPROPS` and
  `TEMPLATEBEHAVIOURPROPS`. The behaviour thresholds are impulse 50 (bRubber, bPhantom),
  250 (bLight), 1000 (bArmco, bMedium, bDeformableTyreWall with explosion 0.35) and
  2000 (bHeavy). [verified data]
- **Break behaviour interpretation.** The exe schema includes `uprootProps`,
  `breakLinearImpulse` and `dontCollideWithFixedObjects`. Together with the data above,
  props likely start fixed (`infMass`) and switch component state or become dynamic when
  the contact impulse exceeds the behaviour threshold. Boards would be knocked over and
  tyre walls deform. [inference; consumer code not traced, string refs are table-based]

## Prioritized implementation plan

Each item lists files/functions for the implementer and the check that proves it.

1. **Static entity boxes in the body-contact world.** This fixes B, and most of A at
   tyre walls and armco.
   - Add an adapter, for example `crates/grid-tools/src/entity_geometry.rs`. It reads
     `route_0/objects.ens` + `objecttypes.pssg` via `grid_formats::entities::read` and
     composes `entity.transform ∘ shape.transform` (full-extent box centred on the shape
     origin; this matches the visual mesh).
   - Emit 12 outward-wound triangles per box as a one-leaf `packed_query::Mesh`
     (`nodes [0x8000]`, one leaf list, as in the `packed_contacts` unit test). Query each
     box with `grid_world::packed_contacts::calculate_decoded` and `sheet_mask = 0`
     (witness normals, not face normals).
   - Cull with box AABBs before the query; a uniform XZ grid is enough for 1,021 boxes.
   - Register the box shapes in `Runtime::prepare_body_geometry`. Push the extra
     `contact_groups::Shape` entries after the boundary one so the 245 wheel shape ids
     stay fixed.
   - Schedule boxes in `Track::body_contacts` before the packed chunks.
   - Raise the 20-slot reservation (`reserve_geometry_pool`, up to 64) so the boundary and
     boxes do not starve the packed road.
   - Static first: all startup bodies are `infMass`.
   - Material: map shape props through `physparams.xml` (in `raceload.jpk`, private
     read) to the `surface_materials.xml` key, then use `Surfaces::physical`.
   - Validate with the probe (`grid-entity-collision`, `grid-collision-gaps`). The car
     must stop at the tyre-wall/armco face, not the sheet, with no change on clear road
     (existing 240-tick straight test).
2. **Run the collision world at the original cadence** (A2/A3). Substep the body/contact
   world (`vehicle_world::calculate_with_*` / `grouped_physical_frame::calculate_tick`)
   with A72930 semantics: substeps such that the step delta ≤ 1/maxfps. Keep the 1 ms
   wheel substeps. The penetration-proportional cap stiffens as 1/dt (measured d700:
   0.35 → 0.16 m at 6 ms). This needs `maxfps/minfps/maxTimesToRun` from a live
   read-only capture of `scheduler+8..+10`.
3. **Verify the deformation inputs and the visual crush** (A2). Compare DF01-derived
   `deformation_contact_limit::Parameters` and descriptor markers/levels with a live
   capture of a real barrier hit.
   - Visual check: confirm the visual dent (`body_deformation.rs`) moves the nose by the
     same amount as `collision_mesh::build_from_deformed_source`. The hull crushed by
     0.6–0.7 m in the measured hits.
   - If the original cap behaves the same at its own cadence, the remaining sink is
     authentic crumple and should appear as a dent, not as intersection.
4. **Replace the authored solver and material values** (A4).
   - In `source_vehicle.rs` `Runtime::lacetti` manager setup (316–343), use per-material
     coefficients via `Surfaces::physical` instead of the shared material. This removes
     restitution 0.3 and the K/S softness.
   - Use the original defaults (`+CC..+F4` above) until the live capture confirms or
     overrides them.
   - Measured effect: residual rest overlap falls (threshold 0.125 → 0.001) and there is
     no rebound. Peak sink is unchanged.
5. **Retained per-pair contact contexts** (A5). Keep a `convex_distance::Context` per
   (shape, triangle) across ticks in `Track::body_contacts` /
   `packed_contacts::calculate_owned`, using the already-verified retained-query
   behaviour. This removes zero-contact ticks at deep sheet overlaps.
6. **Car shape margin 0.02** (A6). Change `BodyGeometry::read` `ContactSettings.margin`
   and `ShapeSettings.scalar220` only after resolving the caller of `9B8A50` that fills
   descriptor `+0x68`. The measured effect is small.
7. **Dynamic props** (phase 2 of B). Trace the behaviour/uproot consumer (component state
   switch at impulse threshold) and add dynamic rigid bodies. Inputs are box mass from
   `massSource` density × volume and `rbDefault` damping. This builds on the multi-body
   contact groups (`MULTI_CAR_INTEGRATION.md`). Until then, static boards are closer to
   the original than no collision.

## Reproduction (private)

- Snapshot: `local/collision-gaps/src` (HEAD `a016fff`; workspace without `grid-app`).
- Build: `cargo build --release --offline -p grid-tools --bin grid-collision-gaps`
  (also `grid-visual-extent`, `grid-entity-collision`) inside that directory. It uses its
  own `target/` and never touches the shared one.
- Run: `grid-collision-gaps ROOT CD01 BP01 DF01 OUT D_SWITCH SPEED STEER MAX_TICKS [MARGIN|-1] [AFTER]`.
- Environment switches:

  | Switch | Effect |
  | --- | --- |
  | `PROBE_NODEFORM` | Deformation off |
  | `PROBE_NOEVENTS` | Body events off |
  | `PROBE_SOLVER=original` | Original-default solver parameters |
  | `PROBE_MATERIAL=original` | Original material |
  | `PROBE_MATERIAL_CUSTOM=13 floats` | Custom material |
  | `PROBE_SOLVER_CUSTOM=threshold,iterations,relaxation` | Custom solver values |
  | `PROBE_DEBUG_TICKS=a,b` | Hull, triangle and query dumps for the listed ticks |

  The 6 ms variant is a separate private build: `grid-collision-gaps-dt006.exe`.
- Outputs:
  - `local/collision-gaps/runs/{m-,n-,v-,x-,y-,z-,dbg*}*.tsv`
  - `local/collision-gaps/entity-collision.tsv`, `entities.tsv`
  - `local/collision-gaps/{surface,car}_materials.tsv`, `physparams.xml`
- Ghidra surveys (`local/behavior/`):
  - `20261015T0243105025986Z` (A72EF0 refs)
  - `20261015T0243462845068Z` (9B8A50, 612C90)
  - `20261015T0250527722551Z` (config key refs)
  - `20261015T0251041064187Z` (A74F00)
  - `20261015T0251349954896Z` (6195F0, A749A0)
  - existing `20261010T1115122015337Z` (B7A9E0) and `20261011T0920042177720Z` (A72930)
- Tools: Ghidra 12.1.4, Rust/Cargo 1.99.0 and Python 3.12.14 reused. No installs.
- Failures retained:
  - The first overlap measure used the undeformed hull and overstated the overlap and the
    zero-contact count. It was replaced by the current-hull measure; all deformation
    numbers above use the corrected measure.
  - A raw ripgrep scan skipped binary files. A byte scan replaced it.
