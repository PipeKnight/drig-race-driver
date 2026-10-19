# Spa-Francorchamps (seventh circuit)

Status: 2026-10-18, Claude Code (F425). Spa-Francorchamps `route_0`, layout `spa_gp`
(the 7.0 km Grand Prix circuit, in the Pro Tuned and Touring pools), is the seventh
circuit.

F399 made the track load ([TRACK_READERS.md](TRACK_READERS.md)), but the solo AI
"retired" at tick 1. The cause was a reader limit, not the AI: a road chunk with nine
material keys (`qt_11_11_10_01_11`). The packed contact generator still checked
material indices below 8, although F399 had established the 4-bit material field
(0xAEAFE5, `1 << (b & 15)`). `packed_contacts::materials_valid` now accepts indices
below 16. That changes only chunks with more than eight keys, so every other circuit is
unaffected.

The route uses the F304 route plumbing and the per-track start lights unchanged.

Evidence (ignored, this worktree's `local/circuits/f418/` and `f423/`): `spa_francorchamps-{solo,12car}.txt`,
`spa{1,2,3}-solo.txt`, `f423/quick-37-52/`, `spa_francorchamps-{forced,chaos}/`,
`spa-seed10-body3.txt`. The pre-F417 runs are in `local/circuits/` (`spa*`, `final/`).

## 1. Route data [V: asset tests]

| Piece | Spa route 0 | Test |
|---|---|---|
| Lap timing | 150 gates over 6995.89 m; one forward route, finish gate 0, time splits at gates 41 and 112 | `route::tests::seventh_circuit_sources_load` |
| Start grid | `slot_00`..`slot_19` all build a Lacetti runtime with lap timing (20 slots; the app's fields use 12) | same |
| AI line | `touring`/`lac` optimal line 6909.03 m | same |
| Props | 1515 route boxes; 22 breakable brake-marker boards; 1136 static placements (armco, 764 tyre-wall blocks, 37 cones) | same |
| Collision keys | mix to the car-body row except GRAS and SCRB, whose row equals GRAS's (as on Yokohama) | `route::tests::circuit_barrier_keys_mix_to_the_car_row` |
| Ground clearance | `additional_ground_clearance` 0.0 | `route::tests::additional_ground_clearance_comes_from_the_track_row` |
| Start lights | `gg_startlights_lights_horiz`, 7 stages over 5.0 s (as Nürburgring, Istanbul Park and Jarama) | `start_lights::tests::circuits_ship_their_own_start_sequences` |
| Reset | a flipped player car is put back on the route's AI line | `player_reset::tests::a_flipped_player_car_is_reset_on_spa` |
| Loose wheel | a lost front-left wheel rolls off for the Lacetti and the BMW | `wheel_detach::a_lost_wheel_rolls_off_on_the_other_circuits` |
| Series | `spa_gp` is a supported layout of Pro Tuned and Touring | `series::tests::series_cars_and_pools_come_from_the_database` |

## 2. Headless [V], release, on main 57b406d (F417, engine braking) + this change

- **Solo AI** (`grid-opponent-race --route tracks/europe/spa_francorchamps/route_0 ROOT CD01
  BP01 1 20000 ai`, no DF01): 2 valid laps, 136.609 / 134.735 s (135.330 / 133.558 s before F417).
- **The other routes:**
  - route 1 (`spa_sport`): 138.854 / 136.012 s;
  - route 2 (`spa_gp_r`): 139.372 / 137.330 s;
  - route 3 (`spa_sport_r`) wrecks at 0.95 laps (terminal damage, tick 8363; open).
- **12-car AI race** (2 laps, 24,000-tick budget, DF01 and the characteristics):
  - 11 cars classified with 2 valid laps, winner car 0 4:37.088; car 6 wrecked on lap 2;
  - 1053 car-contact records; 17,827 ticks.
  - Before F417: all 12 classified, winner 4:32.272, 3190 records.
- **Soak:**
  - `grid-soak quick --seeds 37..40` (2, 12, 9 and 4 cars, 5000 ticks; F421's derby took
    33..36): 4/4. Spa's seeds stay outside the default set for now, like Yokohama's 25..28. A seventh of the long seeds move
    to Spa from their own stream.
  - Forced `--route … --seeds 1..24`: 24/24.
  - Prop chaos `--cars 1 --chaos prop --ticks 8000 --seeds 1..20`: 17/20 (19/20 before F417,
    seed 13). The three failures and seed 13 are one spot: a piece of the brake board near
    (576, 40, 965) falls through the grass beside the road (below).

### 2.1 Brake-board pieces through the grass at (576, 40, 965) (open)

[V] Seeds 1, 10 and 18 (and seed 13 before F417) lose a piece of the same brake board. In seed
10 (`grid-soak replay`, `GRID_SOAK_TRACE_BODY=3:0`):
- body 3 rests at (575.83, 40.11, 965.33) until the car hits it at tick 3088 (v 12 m/s, spin
  18 rad/s);
- it then falls freely, with no contact at all, from y 40.1 to 5.6 by tick 3243
  (−25 m/s, the soak's fall limit).

A vertical segment query (`Track::query`) finds the ground there: chunk 158, GRAS, y 39.85, normal
up (0.999); the board's other piece lands on STDR at y 40.3 three metres away. The soak does not
count the piece as pressed down at spawn. So the road is there and the piece gets no contact
from it. Not explained yet: a contact-level trace of body 3 at ticks 3088–3093 is the next step.
It is the only Spa chaos failure.

[V] F465: quick seed 38 loses the upper half of the same board, and there the cause is a hole in
the packed road collision: no triangle of any chunk covers xz (579.0..580.0, 967.6..970.0), about
4 m from the board, where the piece slides off and falls (SOAK_TESTING.md, "Known failure on
main"). The chaos seeds' fall point (575.83, 965.33) does have ground, so they may be a different
cause.

## 3. App

- Launcher `Play GRID Race Spa.cmd` runs `settings/race-spa.ini`: three laps, 11 opponents,
  back of the grid.
- App tests:
  - `pause_menu::race_tests::retire_from_race_shows_the_results_on_every_circuit` includes
    Spa;
  - `flashback::race_tests::a_flashback_resumes_the_race_tick_for_tick_on_every_circuit`
    runs over `route::CIRCUITS`.
- Not checked here: a static render or a windowed race.

## 4. Open

- Route 3 (`spa_sport_r`) AI wreck.
- The brake-board pieces through the grass (2.1).
- Breakable tyre walls and armco (joint rows), as on every circuit.
- Rendering: unresolved road batches and NaN UVs, left to the graphics work.
- No live comparison with the original's AI on this track.
