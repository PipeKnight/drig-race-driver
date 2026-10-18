# Yokohama Docks (sixth circuit)

Status: 2026-10-18, Claude Code (F399). Yokohama Docks `route_0` (`yoko_v1`,
circuit, 2.5 km [D: inventory], the Japan Street GT (Pro Tuned) and the first
round of the career `drift_b_champ` Drift Battle) is the sixth circuit and the
first Japanese track. It became loadable with the reader changes of
[TRACK_READERS.md](TRACK_READERS.md) (NaN half UVs in `objects.pssg`); it uses
the F304 route plumbing and the per-track start lights unchanged.

Evidence (ignored, this worktree's `local/track-readers/`): `survey-*/japan_yokohama_docks_*.txt`,
`yokohama0-12car.txt`, `yokohama0-drift8.txt`, `soak-yokohama/`,
`app/yokohama-start.png|.log`, `app/yokohama-race-app.log`.

## 1. Route data [V: asset tests]

| Piece | Yokohama route 0 | Test |
|---|---|---|
| Collision | `track.jpk` 200 chunks (≤ 1132 triangles, 657 vertices, 6 keys); keys mix to the car-body row except GRAS and SCRB, whose row equals GRAS's (friction 0.4, dynamicScale 4.0, dynamicThreshold 5.81) | `route::tests::circuit_barrier_keys_mix_to_the_car_row` |
| Lap timing | 57 gates over 2528.63 m; one forward route, finish gate 0, time splits at gates 21 and 35 | `route::tests::sixth_circuit_sources_load` |
| Start grid | `slot_00`..`slot_11` all build a Lacetti runtime with lap timing | same |
| AI line | `touring`/`lac` optimal line 2328.57 m | same |
| Props | 1503 route boxes; **no breakables ported**: barrels (`gg_barrel.max`, 77) stand on cylinders, crates, chevrons, direction barriers, armco and 21 close-drift flags need the joint rows the port lacks; 569 static placements | same |
| Start lights | `start_lights.xml`: object `startlights_lights`, **4 stages over 2.0 s** (textures 1, 2, 3, 7), so races count down 2.0 s | `start_lights::tests::circuits_ship_their_own_start_sequences` |
| Reset | a flipped player car is put back on the route's AI line | `player_reset::tests::a_flipped_player_car_is_reset_on_yokohama` |
| Loose wheel | a lost front-left wheel rolls off for the Lacetti and the BMW | `wheel_detach::a_lost_wheel_rolls_off_on_the_other_circuits` |

## 2. Headless [V]

- Solo AI (`grid-opponent-race --route tracks/japan/yokohama_docks/route_0 ROOT
  CD01 BP01 1 16000 ai`, no DF01): 2 valid laps, 72.320 / 71.185 s, state
  `a3495030888b83ea`, maximum speed 54.5 m/s, 9070 ticks.
- 12-car AI race (2 laps, 16000-tick budget, DF01 and the characteristics):
  11 cars 2 valid laps, winner car 0 2:29.360, last classified +19.024 s. Car 6
  retired at tick 3373 with a lost wheel after 445 contact records with car 7
  (a racing incident; not compared with the original).
- Drift Battle, 8 BMWs (`--cars bmw --discipline drift_battle`, 2 laps): 6 cars
  2 laps, car 6 DNF after 1 lap, car 3 wrecked (terminal damage, tick 3683);
  scores 0-1420, 1-34 spins per car. The AI drift quality gap of
  DISCIPLINES.md §4.5 applies here too.
- `grid-soak quick --seeds 25..28` (12, 1, 12 and 6 cars, 5000 ticks): 4/4
  pass. Seeds 25..28 stay outside the default set for now, like Jarama's
  21..24; a sixth of the long seeds move to Yokohama from their own stream.

## 3. App [V]

- Static render (`grid-app --route tracks/japan/yokohama_docks/route_0 --scene
  reference --view start --render …`, checked): the docks street between a
  warehouse and the quay, red/white concrete blocks, the start lights hung on a
  wire between two barrels, the Lacetti on the grid. Track scene 351 draws (42
  unresolved road batches omitted), objects 1984 placed draws, entities 569
  placements (1414 boxes, 128 cylinders), `dynamic_props breakables=0`.
- Windowed race (`GRID_AUTOPILOT=1 grid-app --route … --drive
  --time-trial-laps 1 --body-contacts CD01 --body-events BP01 --deformation
  DF01 --opponents 11 --ai-characteristics TSV`): `drive_lap_timing gates=57
  finish=0`; the 2.0 s countdown; autopilot lap 1 valid in 1:20.250 (sectors
  25.293 / 27.982 / 26.975); all 12 classified, winner AI 1 1:15.808, player
  sixth (+6.063). Ended by the timeout (the window stays open after the finish,
  as on the other circuits).
- Launchers: `Play GRID Race Yokohama.cmd` (`settings/race-yokohama.ini`, three
  laps, 11 opponents, back of the grid) and `Play GRID Drift Battle
  Yokohama.cmd` (`settings/drift-battle-yokohama.ini`, two laps, 7 AI BMWs).
- App tests: `pause_menu::race_tests::retire_from_race_shows_the_results_on_every_circuit`
  and `flashback::race_tests::a_flashback_resumes_the_race_tick_for_tick_on_every_circuit`
  (over `route::CIRCUITS`) include Yokohama.

## 4. Open

- Freestyle arenas (routes 3-6, `Free_Drift`): route 3's progress has a `start`
  split, routes 4-6 are `point_to_point`; neither is read (TRACK_READERS.md
  section 5). Freestyle itself ignores progress [I, DISCIPLINES.md §5].
- Breakable barrels, crates and chevrons (cylinder intacts, joint rows).
- Rendering: 42 unresolved road batches (flat grey road as on the other
  non-default circuits); NaN texture coordinates are passed through
  (TRACK_READERS.md section 3.1). Left for the graphics work.
- No live comparison with the original's AI on this track.
