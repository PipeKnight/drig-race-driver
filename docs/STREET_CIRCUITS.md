# Street circuits: Milan, Shibuya, San Francisco (eighth to tenth circuits)

Status: 2026-10-18, Claude Code (F425). The route_0 circuit of each street track is raceable:

| Constant | Route | Layout (Pro Tuned / Touring pools) |
|---|---|---|
| `route::MILAN` | `tracks/europe/milan/route_0` | `milan_gp`, 4.9 km |
| `route::SHIBUYA` | `tracks/japan/shibuya/route_0` | `shib_drift_cir`, 2.1 km (Shibuya has no `_gp` layout) |
| `route::SAN_FRANCISCO` | `tracks/usa/san_fran/route_0` | `san_francisco_gp`, 5.3 km |

F399 made them load ([TRACK_READERS.md](TRACK_READERS.md)). They use the F304 route plumbing and
the per-track start lights. Milan also needed an AI fix: brake lines a class does not brake for
([AI_DRIVER_PORT.md](AI_DRIVER_PORT.md), "Brake lines a class does not brake for").

Evidence (ignored, this worktree's `local/circuits/f418/`): `{milan,shibuya,san_fran}-{solo,12car}.txt`,
`f423/quick-37-52/`, `f423/sf-seed49-body12.txt`, `{milan,shibuya,san_fran}-{forced,chaos}/`, `milan-12-car7.txt` (behaviour trace).
Pre-F417 runs and the Milan deadlock traces (`milan-12*.txt`, `sf-12-*.txt`) are in `local/circuits/`.

## 1. Route data [V: asset test `route::tests::street_circuit_sources_load`]

| | Milan | Shibuya | San Francisco |
|---|---|---|---|
| Gates / length | 59 / 4876.56 m | 28 / 2064.05 m | 61 / 5292.38 m |
| Time splits | gates 23, 48 | 7, 16 | 31, 46 |
| Grid slots (all build a Lacetti with lap timing) | 16 | 12 | 16 |
| AI line (touring, lac) | 4821.81 m | 1865.68 m | 5178.88 m |
| Route boxes | 2236 | 1048 | 1308 |
| Breakable entities | 251 tyre stacks | none | 68 `tyrestack_black_single` |
| Unported (static) placements | 1212 (armco, cushions, cones) | 382 (cushions, foam posts, railings, drift flags, cones) | 172 (tyre walls, a cone) |
| Ground clearance | 0.05 | 0.0 | 0.05 |
| Start lights | `gg_startlights_lights_horiz`, 7 stages, 5.0 s | `startlights_lights_horizontal`, 7 stages, 5.0 s | as Shibuya |

Other tests updated for the three routes:
- the barrier keys mix to the car row except grass;
- the start lights;
- `player_reset::tests::a_flipped_player_car_is_reset_on_the_street_circuits`;
- the loose wheel on every circuit;
- the series pools now support 25 layouts.

## 2. Headless [V], release, on main 57b406d (F417, engine braking) + this change

| | Milan | Shibuya | San Francisco |
|---|---|---|---|
| Solo AI, 2 laps (s) | 101.868 / 99.920 | 67.735 / 67.484 | 119.966 / 122.927 |
| 12-car race, 2 laps (DF01, characteristics) | 6 classified, 5 wrecked, car 7 stuck (2.3); tick limit | 11 classified, car 8 wrecked; winner 2:19.456 | 11 classified, car 2 wrecked; winner 4:03.984 |
| `grid-soak quick` own seeds (on main 309b73b, F422) | 41..44: 4/4 | 45..48: 4/4 | 49..52: 3/4 (2.4) |
| Forced `--seeds 1..24` | 24/24 | 24/24 | 24/24 (a first run was cut off by a process-start failure, 0xC0000142, while other sessions were building; rerun alone) |
| Prop chaos `--cars 1 --chaos prop --ticks 8000 --seeds 1..20` | 20/20 | 20/20 | 20/20 |

Before F417 (main 9bafbfb + this change): solo 100.824 / 98.821, 65.939 / 65.362 and
118.988 / 117.293 s; Milan's 12-car race 8 classified (winner 3:28.688), Shibuya's all 12
(2:14.896), San Francisco's 11 with car 1 stranded (2.2); forced and chaos all passed.

The quick seeds 41..52 stay outside the default set for now, like Yokohama's and Spa's (F421's
derby took 33..36, so Spa has 37..40). An
eighth, a ninth and a tenth of the long seeds move to Milan, Shibuya and San Francisco from their
own streams.

### 2.1 Milan: the first-chicane deadlock (fixed)

[V] Before the fix, every Milan 12-car race stopped at the first chicane, (140–165, 365–385), at
0.16 laps:
- with 12 cars, nobody finished 2 laps in 24,000 ticks (215,191 car-contact records);
- with 4, 6 or 8 cars, at most one car finished;
- with 3 cars, one car stopped there with **no contact at all**.

The cause:
- **Brake target.** Milan's touring brake data gives lines 1 (arc 790) and 3 (arc 1123) 0/0 m/s,
  and the port braked toward 0.
- **Reverse gear.** A car that slowed early (Avoid/Alongside) came to rest short of the line, and
  the assisted gearbox selected reverse.
- **Dropped brake.** In reverse gear, the authored pedal mapping dropped the brake request, while
  "previous brake > 0" kept the throttle at 0, so the car coasted to a stop forever.

The original skips such lines (0x7A3160/0x78E820/0x7923F0 [D]). With that ported, the deadlock is
gone. The ornament stand-ins near the chicane were ruled out: the race deadlocks the same way
without them.

### 2.2 San Francisco: a stranded car (before F417)

[V] Before F417, in the 12-car race car 1 is hit by car 4 at tick 9019. It spins off at about (975, 217) and
then loops Recovery, reverse and Race off the track for thousands of ticks, ending parked at
(1051, 290). The stuck-retire path never fires, so the race only ends at the tick limit; the other
11 cars finish. This is the AI recovery weakness also seen on Detroit (TRACK_READERS.md), not a
route fault. With engine braking the race takes another course and every car but one wrecked car
finishes.

### 2.3 Milan: a car stuck behind wrecks (open)

[V] With engine braking, Milan's 12-car race reaches the tick limit. Cars 4, 8, 1, 9 and 3
wreck between ticks 3240 and 10866, four of them near (270, 320) at 0.44 laps. From tick 10712
car 7 alternates RecoverTbone forward and reverse against wrecked car 8 (5021 car-contact records
with it) until tick 24000 (`milan-12-car7.txt`). The stuck-retire path does not fire. Six cars
finish 3 timed laps; car 0 wins in 3:25.888. This is the same recovery weakness as 2.2.

### 2.4 San Francisco quick seed 49: a lost wheel driven through the road (open)

[V] Seed 49 (12 cars, player in slot 6) fails at tick 984. The player car loses a wheel at tick
825, near (909, 1.3, 160): the wheel spawns spinning at 86–112 rad/s, is driven down 0.47 m in the
next step (y 1.30 → 0.84, centre below the road) and then falls freely (`f423/sf-seed49-body12.txt`).
This is the thin-body family "driven down in one 16 ms step by a hard hit" of
[ROBUSTNESS_RESEARCH.md](ROBUSTNESS_RESEARCH.md) sections 3 and 7, here a lost wheel under its own
car; not a route fault. The soak does not count it as pressed down at spawn.

## 3. App

- Launchers: `Play GRID Race Milan.cmd`, `Play GRID Race Shibuya.cmd` and
  `Play GRID Race San Francisco.cmd` run `settings/race-{milan,shibuya,san-francisco}.ini`: three
  laps, 11 opponents, back of the grid.
- App tests:
  - `pause_menu::race_tests::retire_from_race_shows_the_results_on_every_circuit` includes the
    three (Milan lac, Shibuya bmw, San Francisco lac);
  - the flashback test runs over `route::CIRCUITS`.
- Not checked here: a static render or a windowed race.

## 4. Open

- AI recovery against wrecks and off the track (2.2, 2.3), and the stuck-retire path that never
  fires there.
- Milan's 12-car race wrecks 4–5 cars, at the start chicane and near (270, 320): no live
  comparison yet (a capture of the original's field on Milan was requested).
- San Francisco quick seed 49 (2.4): a lost wheel driven through the road.
- Breakable cushions, armco and tyre walls (joint rows), as on every circuit.
- Rendering: unresolved road batches and NaN UVs, left to the graphics work.
