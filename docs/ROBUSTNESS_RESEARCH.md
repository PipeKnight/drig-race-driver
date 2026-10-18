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

## 7. Open

- The 16 ms step against the original's per-frame step (F320; seeds 22 and 59, the pile-up depth).
- The original's continuous collision (`maxContinuousCollisions`, config +0x3C): consumer not
  traced (seed 59).
- Kind-5 cylinders and `cylinderFacets` (10) against the port's 12-sided prisms.
- First-corner pile-up: the exact 0x795C00 segment test, then a live comparison.
