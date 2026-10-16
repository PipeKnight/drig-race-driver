# Car generalization (second car, mixed-model fields)

Status: 2026-10-16, Claude Code. Goal: remove the hard-coded Chevrolet Lacetti
(`cars/lac`) assumptions so another purchased car loads, renders, drives,
deforms and races, and AI opponents can drive different models, as GRID's
touring races do. The default (all-Lacetti) runs stay bit-identical. Same
approach as the route parameter in [TRACK_GENERALIZATION.md](TRACK_GENERALIZATION.md).

Labels: **[D]** read from shipped data; **[V]** verified by a run or test;
**[I]** interpretation; **AUTHORED** a port choice with no recovered original.

## Car parameter

`grid_tools::car::Car` names one purchased car folder `cars/<stem>` (2..8
lower-case ASCII letters or digits). `Car::default()` is `lac`. Helpers:
`file(".ned")` = `cars/<stem>/<stem>.ned`, `livery(n)` =
`cars/<stem>/livery_NN/textures_high/<stem>_tex_high_NN.pssg`,
`texture("main")` = `<stem>_main.tga`, `label()` (HUD name), `Car::list("bmw,lac")`.

| Layer | Default-preserving entry | Car-aware entry |
|---|---|---|
| `source_vehicle::Runtime` | `lacetti`, `lacetti_on` | `load(root, &route, &car, slot, seed)`; `car()` getter; body geometry (`.ned` hull) and deformation (`.ned` + `_highLOD.pssg`) use the runtime's car |
| `source_vehicle::multi::MultiRuntime` | `lacetti*` | `load_deforming_on(root, &route, &[(slot, &car)], table, events, deformation)`; car `i` keeps seed 200679 + i |
| `engine_audio_continuous::Bank` | `lacetti(root)` | `load(root, &car)`: `audio/carsetup.bin` row `<stem>`, bank `eng_<stem>.WIP`, RPM bounds from `<stem>.csv` |
| `ai_driver::AiData::load` | already took the car stem (`ai_vehicle_statistics.xml` brake tables) | callers pass each car's stem |
| `grid-opponent-race` | positional arguments unchanged | `--cars STEM[,STEM...]` anywhere, cycled over the grid slots (`lac,bmw` alternates); prints `cars=` only for a non-default field, so default output is byte-identical |
| `grid-app` | unchanged arguments | `--car STEM` (player car and the `car` scene; `--scene lacetti` stays as an alias), `--opponent-cars STEM[,STEM...]` (cycled over the opponents; default = the player's car) |

Settings: the P5 settings file runs in a parallel session, so only the CLI is
added here; a `[race] car=`/`opponent_cars=` settings key can map onto these
two arguments later. The launchers are unchanged (all-Lacetti).

## Inventory of Lacetti assumptions (before this change)

Observed by `grep` over `crates/` at 0eb216e.

| Location | Assumption | Now |
|---|---|---|
| `source_vehicle.rs` `Runtime::lacetti_on` | `cars/lac/lac.csv`, `cars/lac/lac.ned` (tuning row, wheel geometry and widths, damage regions, collision hull, front extent for grid alignment) | per car |
| `source_vehicle.rs` `prepare_body_geometry` | `cars/lac/lac.ned` hull | per car |
| `source_vehicle.rs` `enable_deformation` | `source_deformation::load(root, "lac")` | per car (the loader already accepted `lac` and `bmw`) |
| `source_vehicle.rs` solver settings / CAR_ material | captured in a Lacetti race; `surface_materials.xml` CAR_ row | unchanged: world/material settings, not car data [I] |
| `source_vehicle.rs` `raceload.jpk` `standard.hdm` | handling-damage configuration | unchanged: one shared Standard HDM file [D] |
| `multi.rs` | `Runtime::lacetti_on` per car | per entry |
| `engine_audio_continuous.rs` | `carsetup.bin` row `lac`, `eng_lac.WIP`, `lac.csv` | per car |
| `engine_audio.rs` `EngineBank::lacetti` | `eng_lac`, `lacengine.ini` (older diagnostic layout) | unchanged (diagnostic tools only) |
| `ai_driver.rs` `HALF_WIDTH`/`HALF_LENGTH` | Lacetti extents 0.9 / 2.24 m for traffic geometry | **unchanged, open** (see below) |
| `ai_driver.rs`, `soak.rs`, `route.rs`, `tyre_*`, diagnostics `bin/*` | `"lac"` / `Runtime::lacetti` | unchanged (Lacetti evidence tools and tests) |
| `soak.rs` | all-Lacetti session | unchanged; `replay-log` now refuses an app log whose `drive_replay` names a non-Lacetti field instead of replaying it as Lacetti |
| `grid-app` `load` / `append_car` | `lac_highLOD.pssg`, `livery_00`, `lac_gen_high.pssg`, materials and textures keyed `lacetti/` | per model, keyed `car<m>/` (m = 0 the player's model, then each new opponent model) |
| `grid-app` `diffuse` | `lac_main.tga` etc. | `<stem>_main.tga` etc. (the shipped names are parallel [D]) |
| `grid-app` liveries | `cars/lac/livery_{k % 10}` | `livery_{k % count}` of the opponent's model; count is the shipped `livery_NN` packages (Lacetti 10, so unchanged; BMW 13) |
| `grid-app` setup | one part set drawn on every car, opponents use the player's model offset | each model's parts on the cars that drive it, at that model's NED offset (`model_offset`) |
| `grid-app` HUD | `Lacetti / <track>` | `<car label> / <track>` |
| `grid-app` engine sound | one Lacetti bank for every car | one bank per field model; each opponent uses its own |
| `grid-app` damage parts | `lac.ned` PARTS/WINDOW/SIDE, `lac.ppts`, `lac_glass_dm`/`lac_lights_dm`, layout from the player's parts | per model; a model's layout comes from the player (model 0) or its first opponent |
| `grid-app` time trial | private bests keyed `donington-route0` | bests persist only for the default route **and** car |
| `grid-app` skid marks | `skidmarks/road_tm.xml` (Lacetti `vehicle_skid_type_id=4`) | unchanged: BMW also has `vehicle_skid_type_id=4` [D] |
| `grid-app` tyre audio, tyre smoke, wheel debris | surface/tyre data, not car files | unchanged |

### Per-car private tables (static answer)

None of the three private tables is per car:

- **BP01** (28 bytes) is six file-backed initial scalar words of the body-impact
  handler, read from `grid.exe` (docs/BODY_IMPACT_RUNTIME.md). Executable
  globals, not car data [V by its extraction provenance].
- **DF01** (252 bytes) is the initial mapped deformation contact-limit and
  bulk-callback parameters from `grid.exe` (docs/DEFORMATION_DRIVING_INTEGRATION.md).
  Executable globals; `source_deformation` builds the per-car grid from each
  car's own `.ned` and `_highLOD.pssg` [V: 12-car mixed race below deforms
  BMW cars].
- **CD01** (712 bytes) is 44 unit sampling directions
  (`body_geometry::BodyGeometry::read` checks |d| = 1, w = 0). They are applied
  to the per-car collision points from `car_collision::read(<stem>.ned)`; the
  table holds no car geometry [V: code]. The same table serves both models.

So no owner action is needed for the second car.

## Second car: BMW 320si (`cars/bmw`)

`database.bin` (decoded privately, docs/RACE_RULES_RESEARCH.md) [D]:

- `vehicle_class` 10 `Touring_cars` holds exactly `bmw` (vehicle ids 9, 55,
  58), `lac` (10) and the traffic car `tra` (57, AI config `traffic`).
- Lacetti and BMW both have `ai_vehicle_type_config` and
  `ai_vehicle_type_track` = `touring`, statistics `lac` / `bmw`,
  `vehicle_skid_type_id` 4 (road). Lacetti is `fwd`, BMW `rwd`.
- Events with both models: `euro_touring` (event 36) and `inter_touringcar`
  (event 60) list 6 Lacetti + 6 BMW drivers each (`event_driver` -> livery ->
  vehicle). `goodridge_trophy` (34) and `DC_challenge` (20) are all-Lacetti
  (20 and 12 drivers), `bmw_si_tour` (7) all-BMW.
- The free-race `touring` discipline (9) offers BMW and Lacetti; its track
  pools have `max_cars` 12 (8 at one pool).
- The owner's reference Race Day (docs/LIVE_SPAWN_CAPTURE.md) had 12 cars:
  the player's Lacetti, 6 more Lacettis and 5 BMWs [V, live capture].

How the original picks a free-race field's models and grid order is not
traced (the reference race used a random grid). The port's AUTHORED policy is a
cycled list; `--opponent-cars bmw,lac` gives the 6+6 ratio of the two mixed
events in a 12-car field.

### BMW files [D]

Parallel to the Lacetti: `bmw.csv` (Standard tuning, one row), `bmw.ned`,
`bmw.ppts`, `bmw_highLOD.pssg`/`bmw_lowLOD.pssg`, `bmw_gen_high.pssg`,
`livery_00..12` + `livery_hero`, audio `eng_bmw.WIP` and `bmwengine.ini`.
Texture ids are `bmw_main.tga`, `bmw_glass.tga`, ... (same suffixes as
`lac_*`). No reader change was needed: the Standard initializer, wheel
geometry, suspension, damage regions, handling and RWD drivetrain all come
from the car's own CSV/NED.

## Validation

Evidence: main checkout `local/car-generalization/` (private). Release
`grid-opponent-race ROOT CD01 BP01 CARS TICKS ai`, Donington unless noted,
cargo/rustc 1.99.0.

| Run | Result |
|---|---|
| Lacetti solo, default arguments (`1 11200 ai`) | laps 88.42029812185638 / 86.28228501935392; whole output byte-identical to F324/F325 `solo-after.txt` (SHA-256 `124acff5…2d9a`) [V] |
| BMW solo, `--cars bmw 1 11200 ai` | 2 valid laps **88.95528864026515 / 86.89961494252937**, no failure, max 67.10 m/s. With `GRID_TRACE_BEHAVIOUR=1`: one change at tick 1560 (arc ~1181 m, 49.9 m/s, line error -2.7 m): `on_track=false` for 4 ticks, i.e. only 2 of 4 wheels inside the track limits (0x79B250 test needs more than half), AI `Recovery` mode 2, back to `Race` at tick 1564. The Lacetti has no behaviour change there (line error -2.1 m). A kerb run, not a departure; both laps stay valid [V] |
| BMW solo, Nürburgring `--route tracks/europe/nurburgring/route_0 --cars bmw 1 15000 ai` | 2 valid laps **115.06284042385033 / 112.07460729934344**, **no behaviour change** (never off track), max 69.71 m/s [V] (Lacetti F304: 113.857 / 110.669) |
| Mixed 4 cars, `--cars lac,bmw 4 20000 ai`, private characteristics | every car 2 valid laps; classification car2 (lac) 3:00.560, car0 (lac) +1.216, car1 (bmw) +6.544, car3 (bmw) +18.160; 19 car contacts; 12420 ticks [V] |
| Mixed 12 cars, `--cars lac,bmw 12 16000 ai`, DF01 + characteristics | race over at tick 12363, **every car 2 valid laps**, 1659 car contacts, every car deforms (BMW car 9: 155 bulk calls / 944 records); winner car0 3:01.584, last car7 (bmw) +16.224 [V] |
| Asset test `mixed_field_bmw_equals_its_single_runtime_until_contact` | Lacetti slot_00 + BMW slot_01 in one world: models, wheel anchors and mass properties differ; BMW wheel records rear L/R, front L/R; BMW drives 200 ticks bit-equal (pose/velocity words) to a single BMW runtime [V] |

App renders (RTX 4070, Vulkan; `grid-app` release):

- `--car bmw --scene reference --view start --render local/car-generalization/bmw-start.png`:
  BMW 320si with livery 00 on the slot_00 box (107 draws, 23105 triangles,
  19 textures; Lacetti 98 draws) [V, checked].
- `GRID_AUTOPILOT=1 --drive --car bmw --opponents 3 --opponent-cars lac,bmw
  --body-contacts CD01 --body-events BP01 --deformation DF01
  --ai-characteristics TSV --time-trial-laps 1 --drive-ticks 420 --render
  bmw-race-mixed.png`: HUD "BMW 320si / Donington race", 104 km/h at
  tick 420, Lacetti opponent beside; log `models=bmw,lac`, damage parts read
  for both models (BMW half width 0.9478 m, roof 1.3497 m; Lacetti 0.9133 /
  1.4210) [V].
- `GRID_DAMAGE_CAMERA=1,-3,2.5,-7 --drive --opponents 3 --opponent-cars bmw,lac
  ... --drive-ticks 60 --render lac-grid-bmw-opponent.png`: an opponent BMW
  (livery 01) on the grid next to the Lacetti player, wheels on the road at
  its own model offset [V, checked].

Not checked: windowed play with sound (one engine bank per model is loaded;
not listened to), BMW damage-part thresholds in a crash, BMW glass shards.

## Open items

- **AI car extents**: `ai_driver::HALF_WIDTH`/`HALF_LENGTH` (0.9 / 2.24 m)
  are Lacetti body extents and are used for every car's traffic geometry
  (bumper points, contact type, pass width). Other sessions are changing
  `ai_driver.rs`; making them per-car fields is left to a follow-up.
- Free-race field composition and grid order of the original: not traced.
- `grid-soak` builds an all-Lacetti field; mixed fields are not soaked.
- Launchers and the P5 settings file do not expose the car yet.
- Diagnostic binaries and `engine_audio::EngineBank` remain Lacetti-only.
