# Jarama (fifth circuit)

Status: 2026-10-17, Claude Code. Jarama, `tracks/europe/jarama/route_1`, is the fifth circuit
after Donington, Nürburgring ([TRACK_GENERALIZATION.md](TRACK_GENERALIZATION.md)), Istanbul Park
([ISTANBUL_PARK.md](ISTANBUL_PARK.md)) and Long Beach ([LONG_BEACH.md](LONG_BEACH.md)). It drives
solo and races with the 11-car AI field in the app, with a launcher. It is the first circuit in
the app and the soak whose progress file finishes at a later gate (gate 67). The engine needed no
new format or runtime code for it: the work is the route constant, the race profile and launcher,
the soak draw and tests. Grades: [V] verified by running, [D] read from the data, [I] inferred.

Evidence (ignored): this worktree's `local/fifth-track/`, copied to the main checkout's
`local/fifth-track/` (route surveys, solo and 12-car runs, app logs and the start render,
soak runs and replays, regression outputs, the temporary probe sources `*.probe.rs`).

## 1. Choice

The two open candidates of F350/F354, checked on fb32d54:

| Track | What blocks it | Format work |
|---|---|---|
| Spa-Francorchamps | three reader limits (LONG_BEACH.md section 1): a `track.jpk` chunk with 9 materials over the VCQTC cap of 8, a prop mesh with a non-UV half-float stream, and the packed body-contact material bound | all three, each needing static research first |
| Jarama route 0 | none in the formats; the AI leaves the track at ~61 m/s and wrecks at tick 3286 (the F304 late brake-line pair) | none, but AI braking research that needs a live Jarama trace |
| **Jarama route 1** | none: it loads, and since F354 lap timing re-indexes its later finish gate; the solo AI laps twice validly | **none** |

Chosen: **Jarama route 1**, because it needs no format work at all. Spa needs three reader
changes. Route 1 also covers the later-gate finish that the brief asks for.

What route 1 is [D]: it shares `track.jpk`, `boundarylines.cqtc` and the scene with route 0.
Its own files are the progress file, AI lines, grid, entities and ornaments. Its progress
gates run the other way round the circuit: route 0's gate 0 is at (345.7, 111.8) heading +z,
route 1's at (248.6, -74.0) heading -z. Both have 68 gates over 3903 m and the racing lines are
3830.7 / 3829.4 m. So route 1 is Jarama run in the opposite direction. The app's HUD names it
"Jarama", from the track folder. The original's display name for the layout was not looked up.

## 2. Route data [V: asset tests]

| Piece | Jarama route 1 | Test |
|---|---|---|
| Collision | `track.jpk` within the reader limits; every key except GRAS mixes to the CAR_ row, as on the other circuits | `route::tests::circuit_barrier_keys_mix_to_the_car_row` (now over `route::CIRCUITS`) |
| AI racing line | optimal line 3829.4 m (`touring`/`lac`); the drift lines are skipped (F304 reader rule) | `route::tests::fifth_circuit_sources_load` |
| Start grid | `slot_00`..`slot_11` all build a Lacetti runtime with lap timing | same |
| Lap / sector timing | 68 gates over 3903.32 m. The file has the finish at gate 67 and time splits at 23 and 45; re-indexed (`lap_progress::start_at_finish`, F354), the finish is gate 0 and the splits are at 24 and 46 | same |
| Dynamic props | 2235 prop boxes; 665 breakables: 8 brake-board templates (25 boards) and 640 tyre stacks (`tyrestack_{blue,white,yellow}`, `_double` variants); 576 entities need the joint rows the port lacks and stay static (armco sections and ends, 16 `close_drift_flag`) | same |
| Reset lines (F341) | a car flipped on `slot_00` is put back upright on the route's AI line (fired at tick 196, 125 ticks after it became still and tilted) | `player_reset::tests::a_flipped_player_car_is_reset_on_jarama` |
| Start lights | `tracks/europe/jarama/start_lights.xml`: object `gg_startlights_lights_horiz`, 7 stages over 5.0 s (as Nürburgring and Istanbul), so races count down 5.0 s | `start_lights::tests::circuits_ship_their_own_start_sequences` |
| Barrier stand-ins | no barrier prototype loses a part to the F357 rule; at their spawn poses the 10460 breakable pieces have 487 contacts with stand-in boxes, deepest 0.155 m, none downward | `entity_geometry::tests::only_long_beach_fences_lose_their_leaning_catch_fence`, `dynamic_props::tests::no_piece_spawns_pressed_down_by_a_barrier_stand_in` |
| Loose wheel (F355) | a lost front-left wheel rolls off as a body for the Lacetti and the BMW | `wheel_detach::a_lost_wheel_rolls_off_on_the_other_circuits` |

## 3. App [V]

- Static render (`grid-app --route tracks/europe/jarama/route_1 --scene reference --view start
  --render local/fifth-track/jarama-start.png`, checked): the pit straight with the pit
  building, the pit wall and the Lacetti on the grid. Track scene 397 draws (46 unresolved batch
  draws omitted, so the road is flat grey, as on the other non-default circuits). Objects:
  1277 placed draws. Entities: 1455 placements (24 unique renderables; 185 tyre stacks use the
  provisional intact LOD). `start_lights enabled=true stages=7 sequence_seconds=5`. Two
  brake-board library prototypes (`gg_brake_marker_01_i_right_200m`/`300m Root`) are reported
  unplaced by the renderer; their collision boxes are in the route.
- Windowed race (`GRID_AUTOPILOT=1 grid-app --route tracks/europe/jarama/route_1 --drive
  --time-trial-laps 1 --body-contacts CD01 --body-events BP01 --deformation DF01 --opponents 11
  --ai-characteristics TSV --drive-ticks 7600`, `jarama-race-app.log`): lap timing
  `gates=68 finish=0`. 5.0 s countdown 4..1, GO. Autopilot lap 1 valid in 1:31.163 (sectors
  31.958 / 26.165 / 33.041, checkpoints at gates 24 and 46). All 12 classified: winner AI 1 in
  1:31.792, player +0.989, last +10.064. No failure, retirement or auto-reset. As on the other
  circuits, the window stays open after the finish; the run was ended by the 480 s timeout.
- Launcher `Play GRID Race Jarama.cmd` runs `settings/race-jarama.ini` (race.ini with
  `[game] route = tracks/europe/jarama/route_1`; three laps, 11 opponents, back of the grid).
- App tests: `pause_menu::race_tests::a_race_finishing_at_a_later_gate_is_classified` (one
  autopilot lap at Jarama with 3 opponents, through the stats to the classified results;
  finished at tick 6078, P3, best 90.512 s). `retire_from_race_shows_the_results_on_every_circuit`
  now includes Jarama.

## 4. Headless behaviour [V]

- Solo AI (`grid-opponent-race --route tracks/europe/jarama/route_1 ROOT CD01 BP01 1 16000 ai`,
  no DF01): 2 valid laps, 89.557 / 86.612 s, state `bf7582e60abdabf1`, maximum speed 70.2 m/s.
  The same as the F354 run.
- 12-car AI race (2 laps, 24000-tick budget, DF01 and the characteristics, `race12-ai.txt`):
  8 cars 2 valid laps (winner car 1 2:58.832, last classified +29.360 s); **cars 5, 6, 7 and 10
  wrecked by terminal damage at ticks 790-919**. They came off at the first corner after the
  start, against the armco on the outside (`armco_ax5` boxes around (172, -327)); 24293
  car-contact records in total. Cars 8 and 9 lost ~25 s there. The chain-fence stand-in at
  that corner stands behind the armco, so it is not on the track [V: probe of the box corners].
  This is a mid-pack pile-up of the AI field on the run to the first braking zone. The windowed
  app race (player on the front row, AI pacing on) had no retirement. Whether the original's
  field gets through this corner cleanly needs a live 12-car Jarama trace; not done.

## 5. Soak [V]

- Generator: quick seeds 21..24 (2, 12, 9, 4 cars) run Jarama route 1. A fifth of the long
  seeds move to Jarama from their own stream (`seed ^ 0x7a2a...`), after the Istanbul and Long
  Beach draws, so every other field of a seed keeps its draw. Seeds 1..20 are unchanged.
  Unit test `soak::tests::new_circuits_join_without_moving_the_earlier_seeds`.
- Forced quick set (`grid-soak quick --route tracks/europe/jarama/route_1 --seeds 1..12`,
  `soak-quick-forced/`): **12/12 pass** (1, 2, 4, 6, 9 and 12 cars; up to 14 breaks and 139
  bodies).
- Jarama's own seeds (`grid-soak quick --seeds 21..24`, `soak-quick-jarama/`): **2/4**. Seeds
  21 and 23 pass. The two failures follow, both from existing families. Seeds 21..24 therefore
  stay **out of the default quick set** (`QUICK_SEEDS` stays 1..20), as Long Beach's 17..20 did
  until F357; run them with `--seeds 21..24`.

### 5.1 Seed 24: a tyre spawned below a sloped verge

Replay with `GRID_SOAK_TRACE_BODY=90` (`replay-24.txt`): the player hits the stacks by the
armco at (155-167, -60..-67). At tick 2624 body 90, the bottom tyre of `tyrestack_yell_double`
(entity 1297), spawns upright at (156.21, -6.219, -65.48). The road there is at -6.19 (ray
probe), so the piece's centre is 3 cm below the road surface. The verge slopes about 0.2 m per
metre across the stack; the stack is placed level, so its uphill tyres sit partly in the
ground [D]. In the next step the piece turns ~30° and moves 0.17 m down. From then on it has no
road contacts and falls.

A survey of every breakable piece's spawn pose on the five circuits (temporary test,
`dynamic_props.probe.rs`, output `spawn-road-survey.txt`):

| Circuit | Pieces | Centre below the road | With downward road contacts (deepest) |
|---|---|---|---|
| Donington | 312 | 0 | 0 |
| Nürburgring | 11578 | 28 | 28 (0.223 m) |
| Istanbul Park | 7584 | 0 | 0 |
| Long Beach | 1632 | 0 | 0 |
| Jarama route 1 | 10460 | 47 | 45 (0.348 m) |

So a piece whose centre starts below the road plane gets road contacts with the normal pointing
down, 0.2-0.35 m deep, and is pushed through. This is the road-orientation family of F342
(SOAK_TESTING.md, and LONG_BEACH.md section 5.2, where a tumbling tyre ends with its centre below
the road). The same 28 Nürburgring pieces may explain the untraced Nürburgring tyre-stack loss
of long seed 1085 [I]. Not fixed: the ported contact orientation is shared by every circuit,
and how the original treats a piece whose centre starts below the triangle is not known
(no live trace).

### 5.2 Seed 22: a loose wheel spawned under an armco rail

Replay with `GRID_SOAK_TRACE_BODY=12` and the cars (`replay-22.txt`): in the 12-car start,
car 3 hits the first-corner armco at about 54 m/s (its z velocity goes from -41.4 to -18.3 m/s
between ticks 1109 and 1110) and loses a wheel. The wheel body (F355) spawns at tick 1110 at
(170.56, -0.40, -331.53). The road is at about -0.72 there, so the tyre (radius about 0.3 [I]) about touches it.
The wheel is inside armco box 58 (`armco_ax5`, bottom edge -0.32..-0.55, above the road: the
rails stand on posts). One step later it is at -0.76 and the next at -1.01, below the road.
After that it falls with no contacts. So the static rail box pushed the wheel out downward,
under the rail and into the road, as the F357 stand-in did to tyre pieces on Long Beach. This
time the box is a real entity box. Not fixed: it needs a rule for bodies that spawn inside
static boxes, with the same cross-circuit care as F357.

## 6. Regression on the other circuits [V]

Release, cargo/rustc 1.99.0, against a baseline build of fb32d54 (temporary worktree):

- Solo identity `grid-opponent-race ROOT CD01 BP01 1 11200 ai` (no DF01): sha256
  aad9a07a…e41b, unchanged.
- 12-car races (12000 ticks, DF01 and the characteristics) on Donington, Nürburgring, Istanbul
  Park and Long Beach: byte-identical to the baseline (`regress/`).
- The engine code is unchanged; the diff is the route constant, the soak generator (seeds 21..24
  and the long-run draw), the race profile and launcher, and tests.

## 7. Open

- Seeds 21..24 into the default quick set once 5.1 and 5.2 are fixed.
- 12-car first-corner pile-up (section 4): live comparison.
- Jarama route 0 (the other direction): AI late braking at the gate 85/88 brake-line pair (F304).
- Spa: the three reader blockers of LONG_BEACH.md section 1.
- The flat grey road (46 unresolved track batches) and the two unplaced brake-board renderables.
