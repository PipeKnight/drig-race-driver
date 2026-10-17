# Long Beach (fourth circuit) and later finish gates

Status: 2026-10-17, Claude Code (night run). Long Beach, `tracks/usa/long_beach/route_0`,
is the fourth circuit after Donington, Nürburgring ([TRACK_GENERALIZATION.md](TRACK_GENERALIZATION.md))
and Istanbul Park ([ISTANBUL_PARK.md](ISTANBUL_PARK.md)). It is the first street circuit. It
drives solo and races with the 11-car AI field in the app. The same session
also lets lap timing accept routes whose finish is at a later gate (section 6). Its tyre-stack
falls were fixed later the same day (F357, section 5.1), and its quick seeds 17..20 are now in
the default pre-push set.

Evidence (ignored): this worktree's `local/fourth-track/`, copied to the main checkout's
`local/fourth-track/`.

## 1. Choice

Candidates from the F350 open list, checked on 8b8a48b:

| Track | What blocks it | Work needed |
|---|---|---|
| Spa-Francorchamps | One `track.jpk` chunk (of 173) has 9 materials; the VCQTC reader caps them at 8 (authored; the 4-bit triangle field allows 16). With the cap at 16, a prop mesh fails `nonfinite half UV` (a half-float stream with values like 636.5 and NaN, so not a UV stream); past that, the packed body-contact path refuses the chunk (`packed body material count exceeds bound`, `packed_contacts::materials_valid` requires indices < 8). | reader cap, a mesh stream layout and the packed-contact material bound, each needing static research |
| Jarama route 0 | The AI leaves the track at ~61 m/s (late brake-line pair, F304) | AI braking research; needs a live Jarama trace |
| **Long Beach** | nothing: every reader, lap timing and the AI already work; only the app needed an optional sponsor package | none in the formats |

Chosen: **Long Beach route 0**, because it needs no new format work. Spa's three blockers are
recorded above (evidence `local/fourth-track/spa-*.txt`); none of them was changed in the code.

## 2. Route data [V: asset tests]

| Piece | Long Beach route 0 | Test |
|---|---|---|
| Collision | 101 `track.jpk` chunks within the reader limits; keys CONC, GRAS, RMBL, SHTC, STDR; every key except GRAS mixes to the CAR_ row, as on the other circuits | `route::tests::circuit_barrier_keys_mix_to_the_car_row` |
| AI racing line | optimal line 3580.5 m (`touring`/`lac`) | `route::tests::fourth_circuit_sources_load` |
| Start grid | `slot_00`..`slot_11` all build a Lacetti runtime with lap timing | same |
| Lap / sector timing | 101 gates over 3655.78 m; finish gate 0, splits at gates 33 and 69 | same |
| Dynamic props | 1325 prop boxes; 124 breakable tyre stacks (single and double, blue/red/white); 275 tyre walls (`tyrewall_*`) need the joint rows the port lacks and stay static, as on Donington | same |
| Reset lines (F341) | a car flipped on `slot_00` is put back upright on the line (fired at tick 351, 125 ticks after it became still), drives off at 32.9 m/s | `player_reset::tests::a_flipped_player_car_is_reset_on_long_beach` |
| Start lights | `tracks/usa/long_beach/start_lights.xml`: Donington's board object `startlights_lights_horizontal`, but 7 stages over 5.0 s (textures 1..7), so races count down 5.0 s | `start_lights::tests::circuits_ship_their_own_start_sequences` |

The street walls are static prop boxes (concrete fences) and track geometry; none of the tyre
stacks has `track.jpk` geometry within 2 m sideways.

## 3. App

- Long Beach ships no `lemans_sponsors.pssg` (the three European tracks do). The app now skips
  that package when the track lacks it; any object binding that needs it still fails as
  "object texture unresolved", and none did.
- Static render (`--scene reference --view start`, `long-beach-start.png`, checked): the start
  straight between concrete walls with catch fences, the gantry and its light board, grandstand
  and city skyline. Track scene 518 draws (70 unresolved batch draws omitted, so the road is
  flat grey, as on Nürburgring and Istanbul), objects 2692 placed draws, 399 entity placements
  (210 empty-renderable instances omitted, the F304 fallback), `start_lights enabled=true
  stages=7 sequence_seconds=5`.
- Windowed race (`GRID_AUTOPILOT=1 grid-app --route tracks/usa/long_beach/route_0 --drive
  --time-trial-laps 1 --body-contacts CD01 --body-events BP01 --deformation DF01 --opponents 11
  --ai-characteristics TSV --drive-ticks 6000`, `long-beach-race-app.log`): 5.0 s countdown;
  autopilot lap 1 valid 1:22.763 (28.746 / 28.600 / 25.417); all 12 classified (winner AI 1
  1:23.024, player +1.369, last +7.936); no failure, retirement or auto-reset.
- Launcher `Play GRID Race Long Beach.cmd` with `settings/race-long-beach.ini` (race.ini with
  `[game] route = tracks/usa/long_beach/route_0`).

## 4. Headless behaviour

- Solo AI (1 car): 81.342 / 78.816 s, state `43e27d38e72618f8`. Trace `solo-trace.tsv`: two
  short off-track `Recovery` episodes at racing-line distance ~2664-2700 m (ticks 3748-3811
  and 8711-8736, ~36.5 m/s); the largest line error is 8.2 m at ~2571 m. Both laps valid.
- 12-car AI race (2 laps, `race12-ai.txt`): 11 cars 2 valid laps (winner 2:43.312, last
  classified +15.024 s); car 8 wrecked by terminal damage at tick 1643 (a first-lap crash on a
  narrow street circuit; not investigated further).

## 5. Soak

- Generator: quick seeds 17..20 (12, 1, 12, 6 cars) run Long Beach; a quarter of the long
  seeds move to Long Beach from their own stream (`seed ^ 0x10b3...`), after the Istanbul draw,
  so the other fields of a seed are unchanged. F354 kept the default quick set at 1..16; since
  F357 (section 5.1) it is 1..20.
- Forced quick set (`grid-soak quick --route tracks/usa/long_beach/route_0 --seeds 1..12`,
  `soak-quick-forced/`): **5/12 pass, 7 fail**, every failure `out-of-bounds: prop piece falling
  through the world`, mostly at the stacks around (-512, -86), also (-390, 110) and (-369, 42).
- Diagnosis (replay of solo seed 10 with `GRID_SOAK_TRACE_BODY`): the stack pieces spawn 5-10 mm
  above the road (checked for every stack near the failures), upright, with ordinary placements
  (det 1). At tick 3844 the player drives over the stack at 34 m/s (208 car contacts the next
  tick). In that one tick the bottom tyres (cylinders 0.148 m tall, ~17 kg) are turned about 90°
  and pushed 0.4-0.7 m into the road under the car's floor, below the road surface (2.12), and
  then sink and free-fall. So it is the existing "thin piece crushed under a car" weakness of
  the prop contact solving (the F342/F351 residual family), not a loading error; street-circuit
  stacks sit at the edge of the racing surface, so cars run over them far more often than on
  Nürburgring. Not fixed here: it needs solver work in the soak-residuals area. Seeds 17..20
  join the default set once it is fixed.
  **Superseded by 5.1:** the car was not what pushed the tyres down. The F354 trace showed body
  positions only; the contact trace of 5.1 shows that the downward contacts came from a static
  barrier box.

### 5.1 Cause and fix (F357)

17 October 2026, Claude Code. Evidence (ignored): the F357 worktree's `local/lb-tyres/`, copied
to the main checkout's `local/lb-tyres/` (replays, soak runs, regress outputs, the temporary trace
build's source `dynamic_props.survey.rs`). Grades as elsewhere: [V] verified by running, [D] read
from the binary or data, [I] inferred.

**Contacts at the failure [V].** The seed-10 replay with a temporary per-contact trace (since
removed): at tick 3844 the two bottom tyres of the stack at (-506.4, -83.7) had just spawned.
Neither had any car contact. Each had three contacts with route prop box 587, normal
(-0.015, -0.9999, -0.009), 0.33 and 0.50 m deep. One step later the upper tyre was turned about
90° and both were below the road. Box 587 is not an entity box: it is the authored F284 stand-in
of a `concrete_fence_white_vara` ornament (ROUTE_PROPS_COLLISION.md, F284). So the solver pushed
the pieces out of a barrier box they spawned inside, and the shortest way out was down.

**Why the stand-in reached the stacks [V].** Long Beach lines its streets with `concrete_fence_*`
ornaments (8 prototypes in `objects.pssg`, 7 of them placed 787 times). Each mesh is a 1.0 m concrete wall, 0.42 m thick
(local z -0.22..0.20), plus a catch fence on top. The fence's posts and panels start on the wall
at 0.98-1.14 m, run up at z 0.05 and lean out at the top (3.4-4.1 m) to z 0.83, out over the
track. The F284 stand-in is the oriented box of the whole mesh, so at road level it stood 0.63 m
in front of the visible concrete face. The boundary sheet runs along the wall's back face
(the sampled placements have it within 0.15 m of local z -0.22), as it does behind Donington's concrete blocks.
The tyre stacks stand against the concrete face. A survey of every breakable piece's spawn pose
against the prop boxes (temporary test, since removed):

| Circuit | Pieces | Pairs inside a barrier stand-in | Deepest | Downward normals |
|---|---|---|---|---|
| Donington | 312 | 22 | 0.078 m | 0 |
| Nürburgring | 11578 | 6 | 0.013 m | 0 |
| Istanbul Park | 7584 | 1 | 0.274 m | 0 |
| Long Beach, before | 1632 | 1330 | 0.757 m | 977 |
| Long Beach, after | 1632 | 16 | 0.121 m | 0 |

**What the original does [D/I].** Static ornaments carry no collision data in the original:
`ornaments.bin` holds placements and `ornament_attributes.xml` only LOD and instancing
(COLLISION_GAPS_RESEARCH.md, B). The fence prototypes do have a `default/rigidbody/<name>_physics`
render node in `objects.pssg`, which points at the same index source as the closest LOD. But
neither "rigidbody" nor a `_physics` suffix appears among the strings of the analysis image, so
there is no sign that the game reads it [I]. The original's barrier along these walls is the
boundary sheet. The stand-in boxes are an authored F284 addition that keeps cars out of the
visible concrete. A stand-in that pushes pieces out from under a catch fence matches neither
the original nor what the player sees. In the original these stacks stand and fall in front of
the wall.

**Fix (authored, in the F284 stand-in).** `entity_geometry::barrier_extent` builds each barrier
prototype's box from its connected mesh parts (welded at 1 mm). It leaves out the parts that are
mounted on top of the base (they start within 5 cm of the top of the parts on the ground) and
reach more than 1 cm past the base across the barrier (along the shorter horizontal axis). For
the eight Long Beach fences this removes the leaning posts and panels: the box is the concrete
wall (z -0.22..0.198) up to 3.13 m, the straight part of the fence. No barrier prototype of
Donington, Nürburgring or Istanbul Park loses a part, so their boxes are bit-identical. The first
version also tested the barrier's length axis. It cut a 9 mm sliver off the top of Donington's
`concrete_wall_*_long` (whose ground parts cover only the middle 8 m) and changed Donington's
quick seeds 4, 5, 9 and 12 (12 then failed), so it was dropped.

**Results [V]** (release, cargo/rustc 1.99.0; outputs under `local/lb-tyres/`):

- Replays of the seven F354 failures (seeds 1, 3, 6, 7, 9, 10, 11): all pass through their
  recorded ticks.
- `grid-soak quick --route tracks/usa/long_beach/route_0 --seeds 1..12`: 12/12 (F354: 5/12).
  `--seeds 17..20`: 4/4 (F354: 2/4).
- Default quick 1..16: every result line equal to a baseline build of 33292e7 (16/16).
- Targeted `--cars 1 --chaos prop --ticks 8000 --seeds 1..19` on Long Beach: baseline 19/19
  fail (18 of them a piece at (-247, 224) near tick 1630, seed 1 the car falling), fix 18/19
  (seed 13 at tick 6829, not traced).
- More Long Beach seeds, forced route `--seeds 21..80`: 59/60. Seed 59 is a different mechanism
  (section 5.2).
- Solo identity: `grid-opponent-race ROOT CD01 BP01 1 11200 ai` (no DF01) sha256
  aad9a07a…e41b, unchanged. Long Beach solo (1 car, 11200 ticks, with its car-0 trace):
  byte-identical to the baseline.
- 12-car races, 12000 ticks with DF01 and the characteristics, against the baseline:
  Donington, Nürburgring, Istanbul Park and a Donington `--cars lac,bmw` field byte-identical.
  Long Beach changes. With a temporary per-tick pose print (since removed), the first divergence
  is car 3 at tick 962 at (-64.5, 1.89, 153.1). The only prop box within 3 m is
  `concrete_fence_white_varb` box 718: the car centre is 2.26 m from its new concrete face
  and 1.63 m from the old box's face, within a car's half-diagonal. So the car touched the old
  box [I: the contact itself was not printed]. The result: car 8's lap-1 wreck is gone; car 6
  retires with a lost wheel at tick 4451; winner 2:44.416 (before 2:43.312).
- Tests: `entity_geometry::tests::a_catch_fence_leaning_out_is_not_part_of_the_stand_in`
  (synthetic), `only_long_beach_fences_lose_their_leaning_catch_fence` (asset, all four
  circuits) and `dynamic_props::tests::no_piece_spawns_pressed_down_by_a_barrier_stand_in`
  (asset, all four circuits). All three fail with the old stand-in.

### 5.2 Residual: a tumbling piece landing through the road (seed 59)

[V] `local/lb-tyres/soak/forced-21-80/seed-59.inputs.tsv` (12 cars, 15 breaks, 192 bodies)
fails at tick 4053: body 32, a tyre from a stack hit at about tick 3800, flies to 6.9 m and
lands near the centreline at (-295, 201). At tick 3921 it meets the road at 10 m/s while
tumbling at 26 rad/s: three road contacts, normal up, 0.10 m deep. At tick 3922 the same road
triangle (shape 38) gives three contacts with the normal pointing down, 0.20-0.23 m deep, and
the body moves 0.18 m down with its centre below the road plane. From tick 3923 on it has no
road contacts and falls. No barrier box is involved. This is the road-triangle orientation
weakness of a fast-turning thin piece (the F342 AEE640 family; SOAK_TESTING.md), not the
stand-in problem. Recorded, not fixed.

## 6. Finish gates other than 0 (multi-route progress item)

The refused "multi-route" progress files are not multi-route: Donington routes 1-3, Nürburgring
route 3, Jarama route 1 and Le Mans 0-1 each have one forward route whose finish split is at a
later gate (read: Donington route 1 at gate 54, Nürburgring route 3 at 72, Jarama route 1
at 67, Le Mans route 0 at 157). `lap_progress::load_route` now re-indexes such a
circuit so the finish gate is gate 0: gates and points are rotated and their distances measured
from the finish, and the splits follow (`start_at_finish`). A finish already at gate 0 is left
exactly as read, so Donington, Nürburgring, Istanbul Park and Long Beach route 0 are unchanged.
Tests `lap_progress::tests::a_later_finish_gate_becomes_gate_zero` (synthetic) and
`routes_finishing_at_a_later_gate_load` (asset).

Solo AI with the re-indexing (`finish-*.txt`, 1 car, 24000 ticks):

| Route | Line | Result |
|---|---|---|
| donington/route_1 | 3156.5 m | 2 valid laps, 64.517 / 62.314 s |
| donington/route_2 | 3943.8 m | 2 valid laps, 87.945 / 86.251 s |
| donington/route_3 | 3152.0 m | 2 valid laps, 64.325 / 62.877 s |
| nurburgring/route_3 | 3549.8 m | 2 valid laps, 87.963 / 85.240 s |
| jarama/route_1 | 3829.4 m | 2 valid laps, 89.557 / 86.612 s |
| le_mans/route_0, route_1 | 13519 / 13472 m | no lap: the AI crashes and loses a wheel near the end of lap 1 (ticks 14320 / 13580) |

These routes are not in grid-soak or the launchers yet; they can be selected with `--route`.

## 7. Open

- ~~Tyre-stack falls under cars~~: fixed in F357 (section 5.1); seeds 17..20 are in the default quick set.
- Seed 59 of the forced Long Beach set: a tumbling tyre lands through the road (section 5.2).
- The barrier stand-ins remain authored; the catch fence above 3.13 m and its leaning top are not solid.
- Spa: the three blockers of section 1.
- Le Mans: the lap-1 crash; Jarama route 0: late braking (route 1 is the fifth circuit since
  F361, [JARAMA.md](JARAMA.md)).
- Istanbul turn-8 and Long Beach ~2.7 km recoveries: a comparison with the running original
  needs a live AI trace on those tracks, which needs input to the game (not done in a night run).
- The flat grey road (unresolved track batches) and the prototype launcher's Donington banner.
