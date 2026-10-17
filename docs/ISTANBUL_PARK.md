# Istanbul Park (third circuit)

Status: 2026-10-17, Claude Code (night run). Istanbul Park GP,
`tracks/europe/istanbul_park/route_0`, is the third circuit after Donington
(default) and Nürburgring ([TRACK_GENERALIZATION.md](TRACK_GENERALIZATION.md),
F304). It drives solo, races with the AI field in the playable app, and runs in
`grid-soak`. It uses the route plumbing of F304 unchanged; the new code is the
per-track start-light sequence, the soak route draw and tests.

Evidence (ignored, this worktree's `local/third-track/`, copied to the main
checkout's `local/third-track/`): `solo/*.txt` (route survey), `solo-trace.tsv`,
`race12-ai.txt`, `soak-quick-forced/`, `istanbul-start.png`/`.log`,
`istanbul-race-app.log`, `regress/`, `soak-quick-final/`.

## 1. Choice

Solo AI survey of every shipped route with the current loaders (`grid-opponent-race
--route R ROOT CD01 BP01 1 16000 ai`, before any change of this session; files
`local/third-track/solo/`):

| Track | Routes | Result |
|---|---|---|
| donington | 0 | 88.420 / 86.282 s (identity) |
| donington | 1-3 | lap timing refuses the progress data (not one forward route with finish gate 0) |
| nurburgring | 0, 1, 2 | 2 valid laps each; route 3 refused like Donington 1-3 |
| **istanbul_park** | **0, 1, 2, 3** | **2 valid laps on every route** (route 0: 110.442 / 108.945 s) |
| jarama | 0 | loads; the AI leaves the track at ~61 m/s (F304 late-braking pair), no lap |
| jarama, le_mans | 1; 0-1 | lap timing refuses the progress data |
| usa/long_beach | 0-5 | loads and laps (street circuit; routes 1 and 5 one valid lap of two) |
| spa_francorchamps | 0-3 | `VCQTC packed count limit exceeded` |
| milan, usa/detroit | all | `unsupported collision primitive` (detroit 8-9: mesh stream) |
| japan/haruna | all | `collision archive entry count unsupported` |
| japan/okutama | all | `expected one typed entity child` |
| japan/shibuya, usa/san_fran, usa/washington | all | `unsupported mesh stream kind/format` |
| japan/yokohama_docks | all | `nonfinite half UV` |

Chosen: **Istanbul Park route 0** (the full GP layout, 5.34 km). Reasons: of the
European circuits it is the only one besides the two working ones that loads and
laps with today's readers on every route; it is a touring-car GP circuit like the
other two (same Lacetti/BMW field, same `touring` AI data); Long Beach also works
but is a street circuit with different barrier and scene content, so it is the
better fourth candidate once a street layout is wanted. Spa and Jarama need reader
or AI research first (open items below).

The parked draft `claude/second-circuit-03a7ea` (7ecd58f) is a parallel, unbuilt
version of the F304 route plumbing (`route.rs`, `lap_progress::load_route`,
`Props::read(root, route)`, `MultiRuntime::on_route`). F304 on main already does
all of it; the only extra is a stricter `route_<n>` name check, not needed. Nothing
was taken over; the branch was not pushed.

## 2. Route data [V: asset tests]

| Piece | Source | Istanbul Park route 0 | Test |
|---|---|---|---|
| Collision | `route_0/track.jpk`, `boundarylines.cqtc` | loads (VCQTC chunks within the reader limits); keys CONC, GRAS, HGDR, RMBL, ROCK, RTDR, SHTC, STDR | `route::tests::circuit_barrier_keys_mix_to_the_car_row` |
| Car-body material | `surface_materials.xml` CAR_ row | every key except GRAS mixes (contact_rows `mixed`) to the CAR_ row, exactly as on Donington and Nürburgring, so the single CAR_ registration (source_vehicle.rs) stays exact; GRAS is the same existing deviation on all three | same |
| AI racing line | `route_0/ai_track.xml`, `ai_vehicle_track.xml` (`touring`/`lac`) | optimal line 5226.9 m | `third_circuit_sources_load` |
| Start grid | `route_0/grids.pssg` | slots `slot_00`..`slot_11` all build a Lacetti runtime with lap timing | same |
| Lap / sector timing | `route_0/progress_track.xml` | 109 gates over 5336.79 m; one forward route, finish gate 0, time splits at gates 33 and 74 (three sectors) | same |
| Dynamic props | `route_0/objects.ens`, `objecttypes.pssg`, physparams | 1689 prop boxes; breakables: 6 brake-board templates (`gg_brake_marker_01_i_{left,right}_{50,100,150}m`, 18 boards) and 222 `tyrewall_black_double`; `armco_main_section` (802) and `tyrewall_black` (221) need the joint rows the port lacks and stay static, as tyre walls and armco do on Donington ([DYNAMIC_PROPS_RESEARCH.md](DYNAMIC_PROPS_RESEARCH.md)) | same |
| Reset lines (F341) | the route's AI racing line (`Line::reset_target`) | a car flipped on its roof on `slot_00` is put back upright on the line after 2 s (fired at tick 237, 125 ticks after it became still and tilted), drives off at 33.3 m/s after 4 s | `player_reset::tests::a_flipped_player_car_is_reset_on_istanbul_park` |
| Start lights | `tracks/europe/istanbul_park/start_lights.xml` | object `gg_startlights_lights_horiz`, 7 stages over 5.0 s, textures `start_lights_1_e` .. `7_e` (identical to Nürburgring) | `start_lights::tests::circuits_ship_their_own_start_sequences` |

## 3. Start lights per track (new)

Before this change the app and the soak used Donington's sequence (6 stages over
4.0 s) on every route. Now:

- `grid_formats::start_lights::read` parses `start_lights.xml` (root
  `startLightSequence`; siblings `lights object=` and `sequence numStages= time=`;
  `stage id= texture=` in id order). Unit tests in `grid-formats/tests/start_lights.rs`.
- `grid_tools::start_lights::load(root, route)` reads the route's track file.
- grid-app: a `StartSequence` resource (idle texture + the shipped stages, and the
  time) drives the board textures, the race countdown (`Trial` state) and the
  AI grid revving threshold (half the countdown). The stage mapping generalises the
  F296 [I] rule: stages 1..n-1 across the countdown, the last stage (green row) for
  3 s from GO. Donington is unchanged (stages 1..5 then texture 7, 4.0 s); unit test
  `start_light_stages_follow_the_shipped_sequence`.
- grid-soak `Session` holds a race for the track's sequence time
  (`Session::start_sequence`).

Consequence, recorded on purpose: **Nürburgring races now count down 5.0 s
instead of 4.0 s** in the app and in the soak (the shipped value). That the race
countdown equals the light sequence time is the existing [I] of RACE_MODE.md;
applying it per track is the choice closest to the original data. Headless
`grid-opponent-race` has no countdown and is unchanged. Soak replays of
Nürburgring races recorded before this change start 1 s later and are not
comparable tick for tick.

## 4. Behaviour checks

- Solo AI (`grid-opponent-race`, 1 car, 14000 ticks): 110.442 / 108.945 s, state
  `5a2312df2df7bb3b`. Trace `solo-trace.tsv`: two `Recovery` episodes (ticks
  3382-3469 and 10192-10280), both in the fast left-hander at racing-line
  distance ~2290-2400 m (turn 8): full throttle (0.96), steering saturated at 1.0,
  the car drifts wide to 8.7 m off the line at 56-57 m/s, the off-track recovery
  runs for ~1.4 s and the car rejoins; both laps stay valid. This is the same
  pattern as the "n = 1 off-track moments at speed" on Donington
  ([AI_AVOID_RECOVERY_RESEARCH.md](AI_AVOID_RECOVERY_RESEARCH.md)); whether the
  original AI takes turn 8 flat or lifts needs a live Istanbul AI trace (open).
- 12-car headless AI race (2 laps, 24000-tick budget, `race12-ai.txt`): every car
  2 valid laps; winner 3:42.432, last +10.944 s; 3682 car-contact records, no
  retirement.
- App, static render (`--scene reference --view start`, `istanbul-start.png`,
  checked): pit straight, pit building and grandstand, start gantry with the
  light board, Lacetti on the grid. Track scene 574 draws (50 unresolved batch
  draws omitted, so the road shows flat grey as on Nürburgring), objects 2037
  placed draws, 1898 entity placements (23 unique renderables);
  `start_lights enabled=true stages=7 sequence_seconds=5`.
- App, windowed race (`GRID_AUTOPILOT=1 grid-app --route ... --drive
  --time-trial-laps 1 --body-contacts CD01 --body-events BP01 --deformation DF01
  --opponents 11 --ai-characteristics TSV --drive-ticks 8200`,
  `istanbul-race-app.log`): countdown 4..1, GO; autopilot lap 1 valid 1:53.376
  (sectors 39.745 / 36.997 / 36.633); classification of all 12 (winner AI 1
  1:54.816, player +1.130, last +7.568); no failure, retirement or auto-reset.
  The capture keeps the window open after the finish (as on Nürburgring, F304) and
  was ended by the 420 s timeout.
- Launcher: `Play GRID Race Istanbul.cmd` runs the race profile
  `settings/race-istanbul.ini` (= `race.ini` with `[game] route =
  tracks/europe/istanbul_park/route_0`). The prototype launcher's banner still
  says "Lacetti at Donington".

## 5. Soak

- Forced quick set (`grid-soak quick --route tracks/europe/istanbul_park/route_0`,
  seeds 1..12 before the generator change, `soak-quick-forced/`): **12/12 pass**
  (1, 2, 4, 6, 9 and 12 cars), breaks on seeds 3 and 10, at most 46 bodies.
- Generator: quick seeds 1..12 keep their F326 routes and fields; the quick
  default is now seeds 1..16, and 13..16 (2, 12, 9, 4 cars) run Istanbul Park.
  Long runs move about a third of the seeds to Istanbul Park, drawn from a
  separate stream (`seed ^ 0x157a...`), so every other field of a seed keeps its
  draw. Scenario files carry the route, so replays do not depend on the generator.
  Unit test `istanbul_park_joins_without_moving_the_earlier_seeds`.
- Quick run on this build (`soak-quick-final/`): 15/16. Istanbul seeds 13..16
  pass (2, 12, 9, 4 cars). The one failure is the known Donington seed-12 board
  fall (tick 3003, prop piece through the road at x -406), as on F346/F347.
  Against the 307fe9b baseline (`soak-quick-baseline/`, 11/12, the same seed 12):
  every Donington seed and the solo Nürburgring seeds give the same result line;
  the Nürburgring races 6, 7 and 11 differ in breaks and body counts because they
  now hold the grid 5.0 s (all pass).

## 6. Open

- Turn 8 off-track recoveries of the solo AI (above): live comparison.
- Spa (VCQTC count limits), Jarama (late brake-line pair), Long Beach as the
  first street circuit; Donington/Nürburgring/Jarama/Le Mans routes refused by
  lap timing (more than one route or a finish gate other than 0).
- The 50 unresolved track batch draws (flat grey road) and the entity preview
  fallbacks are the same F304 renderer gaps as on Nürburgring.
- Enhanced graphics stay Donington-only; time-trial personal bests are kept only
  for Donington (`route.is_default()`).
