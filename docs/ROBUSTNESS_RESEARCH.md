# Open robustness failures: Jarama, Long Beach and the first-corner pile-up

18 October 2026, Claude Code. This follows up the soak failures left open by F354, F357 and F361:
Jarama quick seeds 22 and 24, forced Long Beach seed 59, and the 12-car Jarama first-corner pile-up.

Static evidence comes from the hash-pinned analysis image (`aac46a64…4514`), read-only:
- Ghidra 12.1.4 headless (`-readOnly -noanalysis`) on the dynamic-props project;
- Capstone 5.0.6 through the private `pe.py`.

Pseudocode, scripts, traces and runs are in this worktree's private `local/robust/`.

Grades:
- [V] verified by running;
- [D] read from the disassembly or decompilation;
- [I] inferred.

## 1. Jarama seed 24: tyre stacks authored into the verge

[V] The seed-24 replay still fails on main 2355c47 (`local/robust/base-21-24/`).
- Body 90 is piece 14 of `tyrestack_yell_double` entity 1297, the bottom tyre of the stack's
  fifth column (cylinder r 0.280 m, h 0.453 m).
- It spawns at y −6.219. The road under its centre is at −6.186.
- At its spawn pose the road gives three contacts with the normal pointing down, 0.21–0.29 m deep
  (probe `local/robust/probe-sunk-jarama.txt`).

[V] Most of the stacks involved are authored below the ground. Ground under each breakable's
placement origin minus the origin height (`local/robust/probe-sink-all.txt`):

| Circuit | Placements | Sunk > 0.15 m | 0.05–0.15 m | Within ±0.05 m |
|---|---|---|---|---|
| Donington | 190 | 0 | 24 | 166 |
| Nürburgring | 930 | 10 (up to 0.31 m) | 13 | 885 |
| Istanbul Park | 239 | 6 (tyre walls) | 41 | 192 |
| Long Beach | 124 | 0 | 1 | 123 |
| Jarama route 1 | 665 | 15 (up to 0.39 m) | 67 | 576 |

- Every Jarama stack in the table's top rows is a `tyrestack_*_double`. Its bottom pieces are
  0.45–0.46 m cylinders resting on the placement origin.
- Where the verge rises across a sunk stack, a bottom piece's centre ends up below the road
  surface. Seventeen Jarama breakables have such a piece.

[D] The original spawns a piece at the raw placement. No ground snap exists in that path:
- 0x7D3C40 reads the entity world matrix at +0x40 and passes it unnormalised to 0x7CE490 →
  0x7CDCD0 (DYNAMIC_PROPS_RESEARCH.md, F317).
- No call from the entity code (0x7B0000–0x7E0000) reaches a world ray query on that path.
  The callers into the physics query range are:
  - 0x7C98D0, a four-ray ground probe (centre plus three offsets) that stores a height offset
    and a ground normal. It is called only from 0x7CA070. That function is called from 0x7CA240
    (activate pooled objects inside a box) and from the replay code 0x9335F0. It is not a prop
    break.
  - 0x96B7E0, called twice in the runtime shape builder 0x7C6350: an empty function (`ret`).
  - 0x9AD030: a vector-size getter.

[D] The road contact is the original's. AEA1F0 treats a non-sheet road triangle as a thin plate:
- The normal is the witness difference.
- AEE640 orients that normal by the moving shape's transform origin.
- A piece whose centre starts below a triangle's plane therefore gets a downward normal. This is
  the minimum separation of a thin plate from a cylinder centred below it.

The port's packed generator matches the original over 2,642 cases (PACKED_GENERATOR_COMPLETE.md).
Only SHTC (the boundary sheet) is a sheet material, so the road triangles here are not sheets.

[I] So the original loses these tyres too when the stack breaks. This is an authored-data
property of the five or so sunk Jarama and Nürburgring stacks, not a port gap.

Two port differences remain and are not the cause:
- The original's cylinder is a kind-5 primitive (B47A70), the port's a twelve-sided prism. The
  original's world config also has `cylinderFacets` = 10 (config +0x84; default set by 0x6195F0,
  read by 0xA74F00). Its consumer was not traced.
- The original steps at the frame rate (F320 open question).

## 2. Jarama seed 22: a wheel spawned inside an armco rail

[V] It still fails at tick 1296. Car 3 hits the first-corner armco at 54 m/s and loses a wheel
(F355). The wheel spawns at the hub, inside armco box 58, whose bottom edge is 0.32–0.55 m above
the road. The minimum-overlap exit from the box is downward.

The original's box query (AE8AD0, ported in F351) behaves the same way [I]. The deeper overlap
comes from the port's 16 ms step: the car is 0.86 m further into the rail per tick than with the
original's per-frame step.

## 3. Long Beach forced seed 59: a thin tyre landing through the road

[V] It still fails, at tick 4053 (`local/robust/lb59/`).
- The tyre is 0.15 m thick.
- It lands at about 10 m/s while tumbling, so it moves about 0.16 m per 16 ms step.
- Its centre crosses the triangle plane within one step. The next step's road contacts point down.

This is the step-size family. The original's world config has `maxContinuousCollisions` (config
+0x3C, default 1000), so the original world has a continuous-collision stage. The port has not
researched it. Next: trace the consumer of world config +0x3C in the world constructor (0xA75530).

## 4. First-corner pile-up (12 cars, Jarama route 1)

[V] Reproduced on main (`local/robust/pileup/`): cars 5, 6, 7 and 10 retire with terminal damage at
ticks 790–919, against the outside armco of the fast left-hander at (172, −327).

[V] Per-car trace (temporary instrumentation, removed):
- The grid rows run side by side in the Alongside behaviour from about tick 680. Pairs (3,4),
  (5,6) and (7,8) are then in continuous contact from tick 700–730 on.
- The left car of a row is pinned at the left racing limit. That limit narrows from −7.9 m to
  −0.5 m on the run to the corner. The car is physically 2.5 m outside it, so its lateral
  controller pulls it right into its neighbour.
- The right car merges right at the alongside rate of 4 m/s.
- Once in contact, both cars hold full steering lock from tick 720 at about 48 m/s (car 1, alone
  on the line, needs 0.2–0.6). They run 15–18 m wide into the armco.
- The leader takes the corner flat at 51–52 m/s, so the corner itself is not the problem.

[D] Three parts of the original's alongside code are missing in the port. All three were ported
and measured; the change is **not adopted** (section 6). The patch is kept in the private
`local/robust/alongside-port.patch`.
1. 0x795C00 tail. An empty side slot gets gap 0 when the own side edge (centre ± right row ×
   half width, +0x110) lies beyond that side's racing limit segment. The test is the sign of the
   cross product with the limit segment between the view's two links, from points 0x793790
   types 1/2. The port tests the edge's lateral position against the racing limit at the car [I].
   The gap feeds the yield's "other side tight" test and the avoid side choice.
2. 0x799630 squeeze, one side only, gap ≥ min space. If
   `room_at_car − ((room_at_look_ahead + asTargetBuffer) − gap) < min space`, the offset moves
   away by the whole min space and the squeezed flag +0x30 is set (its consumer is not traced).
3. 0x799630 with cars on both sides, or with equal strict flags: the desired offset is the mean of
   the two cars' racing-line offsets (0x798C00/0x798D00), and 0 when a car record is missing.

In the patch, the side flags also follow the original's order: the strict-overlap flags first,
then the slot cars.

[V] Effect on the Jarama 12-car race (1,400 ticks): retirements go from 4 (cars 5, 6, 7, 10) to 3
(cars 5, 7, 6), and car-contact records from 526 to 420. Items 2 and 3 do not fire in this race.
The pile-up is therefore not explained by a missing alongside branch.

Whether the original's field gets through this corner needs a live 12-car trace (not allowed in
this task).

## 5. grid-soak: expected losses (option A, coordinator's decision)

Physics is unchanged. Option B, an authored deviation that would keep such pieces on the ground,
needs the owner's approval and was not done.

- `dynamic_props::PropWorld::pressed_down_at_spawn(body, …)` returns whether a piece or lost
  wheel gets a contact with a downward normal (y < −0.5) at its spawn pose, and from what:
  - `Pressed::Road`: a road chunk or the boundary sheet;
  - `Pressed::EntityBox`: a route entity box other than the piece's own intact boxes.
- Authored ornament stand-ins (boxes without an entity, F284/F357) do not count, because they are
  not the original's geometry. Without that exclusion the rule would have hidden the F357 stand-in
  bug.
- `soak::Session::check` calls it only for a prop body that is about to fail as out of bounds or
  falling through the world. Such a body is recorded in `expected_losses` (segment, tick, body,
  position, `pressed`) and is not a failure.
- grid-soak prints one `soak_expected_loss … pressed_by=Road|EntityBox` line per loss. The result
  and `soak_done` lines gain `expected_losses=N`. `summary.md` lists the seeds that have expected
  losses.
- Asset test `dynamic_props::tests::only_sunk_tyre_stacks_spawn_pieces_pressed_down_by_the_road`
  [V]: Jarama 45 pieces (all `tyrestack_*`, including entity 1297 piece 14), Nürburgring 28, and
  none on Donington, Istanbul Park, Long Beach or Yokohama. These match the F361 survey.

[V] Results:
- Jarama `--seeds 21..24`: 4/4 pass (was 2/4).
  - Seed 22: 1 expected loss, the wheel (`EntityBox`, armco box 58).
  - Seed 24: 14 expected losses, all `Road`, all bottom tyres of the sunk stacks along the verge at
    x 148–157, z −65.
- `QUICK_SEEDS` is now 1..24.
- Long Beach forced seed 59 still fails, as it should: the piece was not pressed down at spawn
  (section 3).

## 6. Alongside port across circuits: not adopted

[V] 12-car races, 12,000 ticks, DF01 and the characteristics; main 2355c47 against main plus the
patch (`local/robust/race12/`, binaries `local/robust/bin/`):

| Circuit | Retirements (base / patch) | Car-contact records | Cars with 2 valid laps |
|---|---|---|---|
| Donington | 0 / 0 | 1,211 / 1,615 | 12 / 11 |
| Nürburgring | 0 / 0 | 8,517 / 2,361 | 0 / 0 (budget) |
| Istanbul Park | 0 / 0 | 4,367 / 3,380 | 0 / 0 (budget) |
| Long Beach | 1 / 1 | 3,214 / 2,592 | 11 / 11 |
| Jarama | 4 / 4 | 24,245 / 84,363 | 6 / 2 |

Every 12-car result changes:
- Jarama gets worse: one more car loses a wheel at tick 6,943, and cars stay in contact far longer.
- Nürburgring and Istanbul get fewer contacts.

The edge test is an approximation [I] (lateral against the racing limit at the car, not the
original's segment between the view's two links). The field is not clearly better. Under the rule
to keep race baselines unless a change is a proven 1:1 fix, the patch stays out.

Next for the pile-up:
- recover the view links +0x20/+0x24 that 0x795C00 uses, and port the exact segment test;
- a live 12-car trace of the original at Jarama, if the owner allows it.

## 7. The original's continuous collision (F411)

18 October 2026, Claude Code, the follow-up to section 3. Static, read-only. Pseudocode and
dumps are in this worktree's private `local/robust/decomp/d1..d13`. Nothing was run in the
original game.

### Where the setting goes [D]

- `maxContinuousCollisions` is config index 0xF (+0x3C, default 1000).
  - 0xA749A0 puts it at world-struct +0x54.
  - The collision world B18E70 takes its config at world-struct +0x3C, so the value lands in its
    +0x958.
- 0xB18A00 divides it by the worker count and passes it to each narrow-phase worker (0xB3AC80).
  The worker allocates that many 0x30-byte records at +0x18 (count +0x20, capacity +0x24).
- The world step 0xAD8A40 runs in phases:
  1. AD8070;
  2. **B18E20**;
  3. B18300, the ordinary narrow phase;
  4. B184F0;
  5. AD7110;
  6. AD8130.
- B18E20 refreshes the broadphase for the moved bodies and calls **B15B70** when world byte +0x84
  is set. The constructor AD8BA0 sets it to 1, and the existing read-only capture
  `local/world-settings/paused01.json` shows +0x84 = 1 in the running original [V].

### Which bodies [D]

B15B70 works per body (list +0x300):
- d = predicted centre (+0x334 → +0x1C0) − step start (+0x2A0).
- The body is swept when, for one of its shapes, (r · 0.75)² < |d|² (0xCAFE64 = 0.75, x87).
- r is the shape's vtable +0x18. For a mesh shape, B41F50 returns mesh data +0x5C: the least
  absolute coordinate among the six extrema, which the port already writes (B42540).
- Pairs with another body qualify. So do pairs with static geometry, such as the road, if body
  byte +0x416 is set; B11F40 sets it to 1 for every body.
- Joined or same-body pairs (B38D80) and the layer mask (B15540) are excluded, as in the ordinary
  narrow phase.

### The swept test [D]

B3A700 calls the pair matrix's third entry (+8):
- AE4700 for a convex shape against a packed road chunk;
- AE34B0 for convex pairs;
- AE3910, AE3F20 and ADDFD0 for the other kinds.

Each call gets the shape's end pose and its start pose, which is the end pose with the
displacement subtracted (end rotation, translation only).

AE4700, per candidate triangle (B212D0 with the motion):
- **The mesh already touches the triangle at the start pose.** B7F740 finds witnesses, and the
  record is kept only if their distance exceeds 1e-3 (CB2294):
  - normal: the witness difference, or the face normal on a sheet;
  - AEE640 orientation;
  - time 0.
- **Otherwise, B80CE0 → B801C0, a GJK ray cast:**
  - shape 1 is the triangle (first-maximum support), shape 2 the mesh rotated about the origin;
  - both margins are 0;
  - the ray runs from the start translation to the end translation;
  - the simplex is B81620/B81700/B81E30/B817C0, using the port's existing B7D570/B7DE90 features;
  - it stops at |v|² ≤ 1e-6 × the simplex's largest |w|², or after 100 iterations (scratch
    +0x200, B7E440);
  - a hit at λ > 0 gives a record with the ray normal (a sheet's face normal is turned to agree
    with it).

### The response [D]

B3A700, for each record:
- k = −(d_rel · n) / ((w_A + w_B) |n|²), with w the inverse mass (+0x3F4).
- If k ≤ 0 the record is dropped.
- Otherwise the body moves to prev + t′·d + (1 − t′)(d + k w n), with t′ = min(1.01 t, 1). After
  the time of impact, the approach along n is removed.

B18220 then applies the records:
- B10680 sums each body's records.
- B14550:
  - averages them (sum × refined RCPPS of the count);
  - stores the correction in +0x360;
  - sets the centre;
  - republishes the pose from the unchanged rotation (B0F730).

Velocity is unchanged. Phase 3 then generates the ordinary contacts from the corrected pose.

### Port [V]

- `grid_world::continuous`:
  - `radius`, `is_fast`, `step`;
  - the ray simplex and `ray_cast`;
  - `sweep_packed` (AE4700);
  - `response` (B3A700), `mean` and `apply` (B14550).
- `Track::sweep_road`: every road chunk the swept bounds reach.
- `PropWorld::continuous` for the pieces and the lost wheels, called at the start of the geometry
  provider (between prediction and contacts, as phase 2 precedes phase 3). The provider now gets
  `&mut [Body]`.

Not ported, recorded:
- cars;
- the boundary sheet (AE95B0);
- route boxes;
- piece and car pairs (AE34B0).

Inferences [I]:
- candidate gathering uses the union of the start and end bounds;
- each query starts from direction +x;
- records are applied in body and chunk order;
- the live `maxContinuousCollisions` was not read (default 1000).

Native byte-exact proof is not done.

### Results [V], release

First on main 78b1837 (F401), before F406 (`local/robust/ccd/`, binaries `soak-base`/`soak-ccd`):

| Run | main 78b1837 | with continuous collision |
|---|---|---|
| solo identity | aad9a07a…e41b | same |
| 12-car Donington, 12,000 ticks | 1c66a317… | same |
| grid-soak quick 1..24 | 24/24 | 24/24 |
| Long Beach forced 21..80 | 59/60 (seed 59) | **60/60** |
| Long Beach `--cars 1 --chaos prop --ticks 8000 --seeds 1..19` | 1 fail (13) | 2 fail (2, 13) |
| Donington `--cars 1 --chaos prop --ticks 8000 --seeds 1..80` | 3 fail (car falls 50 and 66, piece 54) | 1 fail (54, same tick) |

Then on main 9050690 (F408, with F406's collision-floor lift; `local/robust/ccd/checks3/`, binaries
`soak-main408`/`soak-ccd408`, `race-main408`):

| Run | main 9050690 | with continuous collision |
|---|---|---|
| solo identity | aad9a07a…e41b | same |
| 12-car Donington, 12,000 ticks (DF01, characteristics) | 85363097… | **e42ba1dd…** |
| grid-soak quick 1..24 | 23/24 (seed 14, tick 1531) | **24/24** |
| Long Beach forced 21..80 | 57/60 (37, 41, 48) | 57/60 (37, 61, 74) |

- **Seed 59** (the 0.15 m tyre landing at 10 m/s) no longer tunnels.
- **Seed 14** (F406's residual, a prop piece at Istanbul nudged and sinking through the ground)
  passes. It fails on main at tick 1531; both runs have 2 breaks.
- **The 12-car race change is the 1:1 port.** Since F406, car 6 loses wheels 2 and 3 at tick 6,640.
  The lost wheel is a body of the shared world, so it is swept:
  - wheel 3 ends at (−526.43, −173.43) instead of (−526.52, −173.56);
  - cars 9–11 then differ in late contacts;
  - finishing order, lap counts and retirements are unchanged.
  - A race without a lost wheel or broken prop is unchanged, because cars are not swept.
- **Long Beach chaos seed 2** is a different trajectory from main's: 22 breaks against 15, and
  main never creates body 231.
  - At tick 2121 a car hit drives body 231 down 0.38 m in the ordinary contact solve. The swept
    stage found no hit at that step.
  - From then on the piece starts its steps below the road, and the swept stage correctly adds
    nothing.
  - This is the known family of thin pieces driven down in one 16 ms step by a hard hit (F351).
- **Long Beach forced 61 and 74** (new on the F408 base, while 41 and 48 pass) are trajectory changes:
  - Seed 74's piece gets small upward corrections at ticks 3302–3303 (+1 to +13 mm, the approach
    along up-pointing road normals removed).
  - At tick 3304 it drops 0.1 m in the ordinary solve with no swept hit, then sinks: the
    thin-piece family of section 3.
- **Seed 22** is not settled by this. Its wheel spawns inside the armco box, and the sweep only
  shortens motion toward the obstacle; it is still an expected loss (EntityBox).
- **The F406 GT2 residual** (a car on its nose sinking through the road) is not covered: cars are
  not swept yet.

## 9. Quick seeds 17 and 23 after F444

F444 (AI traction control and ABS cap) changed the AI's trajectories, so different props are hit.
`grid-soak quick` went to 22/24. Evidence: worktree fervent-napier-50814b
`local/firstcorner/checks2/quick/seed-{17,23}.*`, traces `local/firstcorner/checks/seed{17,23}-body*.txt`,
`local/firstcorner/props/s17-tmp.txt` (per-tick road contacts of the piece, temporary
instrumentation, removed).

- **Seed 23, Jarama** [V]:
  - Piece 22 rests on GRAS at (211, −0.9, −312). A car hits it at 29 m/s, and it flies about
    80 m to (246, −33, −396).
  - A vertical segment query (`Track::query`) finds no road chunk from about x 221, z −337 on:
    the piece has left the collision ground.
  - The original uses the same chunks [I: so it would lose the piece too, unless it culls pieces
    in a way not found].
- **Seed 17, Long Beach** [V]:
  - Piece 15 of a tyre stack rests on GRAS (y 1.10, shape 50 `qt_01_11_01_01`). A car hits it at
    21 m/s into the neighbouring intact stack, entity box 144 (`tyrestack_white`).
  - The box contact's minimum-penetration exit is the box's bottom face: normal y −1.0, depth
    0.38 m. In the next steps the piece is driven to y 0.80, the grass contacts come back with
    downward normals (−0.42, then −1.0), and it falls.
  - Original geometry and the ported contact code. The only candidate difference is the 16 ms
    step against the original's per-frame step (F320; the F351 thin-piece family).
- Neither has a 1:1 physics fix. The coordinator approved two rules in the manner of section 5
  (F448; harness only, physics unchanged), checked in this order after the spawn rule:
  - **(i) Left the ground** (`Pressed::NoGround`, an expected loss):
    - a vertical segment (±5000 m) at the lost body's xz finds no road chunk;
    - [I] the original has the same chunks; a culling of such bodies in the original was not
      looked for.
  - **(ii) Driven in by an entity box** (`Pressed::EntityBoxInRun`):
    - a separate counter `box_losses`, not an expected loss in the counts, and debt in
      FIDELITY_BACKLOG row 88;
    - `soak::Session` records, each tick, for every prop body moving faster than 0.5 m/s,
      `PropWorld::pressing_now` at its current pose: a downward route entity box contact
      (normal y < −0.5, not its own boxes, not a stand-in) and the first road contact deeper
      than 0.1 m (AUTHORED threshold; resting pieces show about 0.03 m);
    - the body counts only when that first deep contact comes in the same step as a box
      contact or the step after one.
  - Counts [V], on 8f02760 + F444 + this rule: quick 1..56: (i) 3 bodies (seed 23), (ii) 1 body
    (seed 17); long 1001..1100 (100 scenarios): (i) 3 bodies (seeds 1023, 1089 Le Mans, 1092 Spa), (ii) none,
    and 37 option A losses (Jarama 1010, 1013). Quick 1..24 passes 24/24 again; Haruna seeds 29 (known since
    F412) and 32 (a car off an unwalled edge, new with F444's trajectories) fail outside the default set.

## 10. Sarthe: "invalid contact material inputs" (F458)

le-mans saw it once on 592430d: the 9-car Sarthe quick scenario of the then seed 39 (stock cars)
retired car 5 at tick 2419. Replay: main checkout `local/lemans/soak/quick-37-40/seed-39.inputs.tsv`.

- [V] It does not reproduce on current main: F444 and F447 changed the trajectories.
- [V] Reproduced on 592430d in a temporary worktree, with a print of the rejected input:
  - car 5 lies upside down (body up y −0.98);
  - wheel 2's ray hits the road at fraction −8.5e−8, a hit at the ray start;
  - `contact_material::resolve` rejects any fraction outside 0..1, and the car retires.
- [V] The port's ray test `triangle::query` keeps the original's f32 tolerances. It accepts
  fractions from −1e−6 to just above 1 and keeps them, not clamped. The leaf query's running
  fraction is the product of such hits.
- So the original passes −8.5e−8 on to the wheel contact. Only the port's guard was stricter.
- Fix: the guard accepts the ray test's own range (`triangle::FRACTION_LOW..=FRACTION_HIGH`);
  anything outside it still fails.
  - [V] With that guard the same replay on 592430d passes 2419/2419 ticks.
  - Unit test in `contact_material::tests`.

## 8. Open

- The 16 ms step against the original's per-frame step (F320; seeds 22 and 59, the pile-up depth).
- Continuous collision (section 7): cars, the boundary sheet, route boxes and piece/car pairs are
  not swept yet.
- Kind-5 cylinders and `cylinderFacets` (10) against the port's 12-sided prisms.
  F462: static route cylinders use the round kind-5 support and features; the
  contact path never reads `cylinderFacets` (BOX_CONTACTS.md "Cylinders and
  spheres"). Pieces keep the prisms.
- First-corner pile-up: the exact 0x795C00 segment test. The live comparison on Donington
  (F433, [FIRST_CORNER_LIVE.md](FIRST_CORNER_LIVE.md)): the original's field also gets through
  Redgate; the main difference is its much harder launch, not the alongside logic.
  F444 ported the AI traction control and ABS cap (the launch); the Jarama route 1 12-car race
  now has no retirement.
