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

Settings (P5, docs/SETTINGS.md): `[game] car = bmw` and
`[game] opponent_cars = lac,bmw` set the same values; the flags override them.
`opponent_cars` without opponents is ignored. The launchers are unchanged
(all-Lacetti unless the settings file names a car).

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
| `ai_driver.rs` `HALF_WIDTH`/`HALF_LENGTH` | Lacetti extents 0.9 / 2.24 m for traffic geometry | per car, the original's rule (F345, "AI car extents" below) |
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

Data observations made by the new tests [D, V]:

- Both CSVs give the **same mass and inverse inertia** (manager properties
  equal; docs/BODY_PARAMETERS.md already noted that mass/inertia alone cannot
  tell the two apart). The cars differ in wheel geometry, tuning and drivetrain.
- The BMW wheel meshes in `bmw_highLOD.pssg` sit a uniform **~11 mm inboard**
  of the `.ned` wheel positions (|x| 0.784 vs 0.796 m; height and axle within
  0.1 mm); the Lacetti's are within 3 mm. The render keeps each mesh's bind
  offset around the physics pivot, as for the Lacetti, so this is shipped
  authoring, not a placement error.

Checks after rebasing on ae14337 (P5 settings, F331/F332 reset path, F333 damage option, F335 ETC2 cache, F336 AI mistakes and pacing):
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --release --lib --test '*' --bin grid-app --no-fail-fast -- --include-ignored`
  with absolute GRID_GAME_ROOT, GRID_CONTACT_DIRECTIONS, GRID_BODY_EVENT_PARAMETERS and
  GRID_DEFORMATION_PARAMETERS: **402 passed, 0 failed** (the `cargo t` alias of F330 fails
  in a worktree, see the log).
- Lacetti solo: same lap times and state hash `a4a35f899a490c36`; the output differs
  from `solo-after.txt` only by the `laps_covered=2.000` field added upstream (F331/F332).
  BMW solo and the mixed 4-car race: values identical to the first runs.
- `grid-soak quick` (all-Lacetti): 6 of 12 fail, all the known F326 Nürburgring set-up
  "scaled prop piece placement". Before F336 the same run also had Donington seed 5
  (prop piece out of bounds at tick 1362), exactly as on the F325 and `quick-f331` runs;
  it passes now because grid-soak mirrors the F336 default AI settings. No new failure.

Not checked: windowed play with sound (one engine bank per model is loaded;
not listened to), BMW damage-part thresholds in a crash, BMW glass shards.

## AI car extents (F345, 2026-10-17)

Before F345 every car's AI traffic geometry used `ai_driver::HALF_WIDTH` 0.9 m
and `HALF_LENGTH` 2.24 m, with bumper points at centre ± forward · 2.24. These
were the port's rounded Lacetti body-mesh values, not values taken from the
original. They are now per car (`ai_driver::Extents`, `Runtime::ai_extents()`,
`Other::of`, `View::extents`).

### Where the original takes them [V static]

Read-only capstone disassembly of the hash-pinned unpacked image
(`local/ai-overtaking/scripts/pe.py`; listings in the worktree's private
`local/car-extents/static/`):

- **0x402D20** is the AI car-state update. It also writes the centre +0x20,
  axes, velocity +0xA0, speed +0xD0 and grip +0x144.
  1. It calls the body dimension getter **0x9B8750**(vehicle, &front, &rear,
     &half width).
  2. It stores **+0x110 = half width** and **+0x114 = front − rear** (the full
     length).
  3. It grows the extents to cover the tyres (below).
  4. It writes the **front point +0xF0 = centre + forward · front** and the
     **rear point +0x100 = centre + forward · rear**.
- **0x9B8750** builds two points:
  - front = (body +0xED64, +0xED68, **+0xED6C**), rear = (+0xED64, +0xED68,
    **+0xED88**);
  - only when body byte +0xC7AD is set, it moves both through the deformation
    lattice (0x9A39D0);
  - it adds the body offset vector +0x1630 to both;
  - it returns their Z values and the half width **+0xED70**.
  - [docs/CAR_COLLISION.md](CAR_COLLISION.md) already identifies these fields
    as the `.ned` `collision_main` description extents: +0xED6C maximum Z,
    +0xED70 maximum X, +0xED88 minimum Z.
- **Tyre cover** (0x4033CE..0x4034B5). The wheel words are written by 0xA0EC80
  from the description: +0x14E0/+0x14F0/+0x1500/+0x1510 are the wheel centres
  RL/RR/FL/FR, +0x1520.. the radii, +0x1540.. the tyre widths.
  - If front-left Z + radius > front, then front = that value.
  - Otherwise, if rear-right Z − radius < rear, then rear = that value. The two
    tests are exclusive: the rear test is the else branch of the front one.
  - If front-left X + width/2 > half width, then half width = that value.
  - Otherwise, if |rear-right X − width/2| > half width, then half width = that
    value.
- **Consumers**:
  - Observed records copy the fields (0x777A80): record +0xD0/+0xE0 bumpers,
    +0xF0 half width, +0xF4 length.
  - The overtake side choice (0x77BF4C..) and brake_for_vehicle (0x78F928) use
    own +0x110 + record +0xF0.
  - The avoid clearance (0x799B30) is 0.5·|front − rear| + avBufferZone + own
    half width + other half width.
  - The avoid room check (0x797720) is own half width × 2.0 (0xCB8DA4) + other
    half width + 0.5.

Not ported: the lattice transform of the bumper points of a deformed car. The
extents stay those of the undamaged hull.

### Values [V asset test `ai_extents_are_each_cars_hull_and_body_offset`]

| Car | Half width | Front | Rear | Body offset Z (+0x1638) |
|---|---|---|---|---|
| Port before F345 (all cars) | 0.9 | +2.24 | −2.24 | — |
| Lacetti | 0.913279 | +2.095238 (2.241661 − 0.146423) | −2.384299 (−2.237876 − 0.146423) | −0.14642298 |
| BMW 320si | 0.947784 | +2.076094 (2.258862 − 0.182768) | −2.420644 (−2.237876 − 0.182768) | −0.1827681 |

No tyre reaches past either hull, so the tyre cover changes nothing for these
two cars. It is ported and unit-tested anyway. The body origin sits about
0.15 m ahead of the hull centre, so the AI box is asymmetric: the nose is
shorter and the tail longer than the old ±2.24.

### Port changes

- `Other` carries `extents`. `front()`/`rear()`, `corners`, `contains` and the
  `contact_type` faces use each car's own front, rear and half width.
- The pass and corridor width, the alongside gap, the overtake side-choice
  width, the avoid clearance and the avoid room check use own + other half
  widths, as the original does.
- The path-obstacle distance and the remembered avoid target (both AUTHORED
  corridors) use the own front and each car's half length.
- The failure retire point and the yield room use the own half width.

### Results change on purpose

The original's rule reproduces neither old constant, so every multi-car race
changes, Lacetti-only fields included. This is intended (coordinator decision,
2026-10-17). The solo AI lap has no traffic and stays byte-identical.

Release `grid-opponent-race`, Donington, baseline = origin/main c09ca82 built
in a separate worktree, private characteristics. Outputs are in the worktree's
`local/car-extents/runs/`.

| Run | Before (c09ca82) | After (F345) |
|---|---|---|
| Solo `1 11200 ai` | 88.42029812185638 / 86.28228501935392 | **byte-identical output** |
| Lacetti × 4 | 102 car contacts; car3 only 1 valid lap | 106 contacts; every car 2 valid laps |
| Lacetti × 12 | 1021 contacts, 11747 ticks | 868 contacts, 11769 ticks; all 2 valid laps |
| Lacetti × 12 + DF01 | 787 contacts; car8 **Wrecked, 0 laps** | 1604 contacts; **no wreck**, all 12 cars 2 valid laps |
| `--cars lac,bmw` × 4 | 19 contacts; winner car2 3:00.560 (as F338) | 134 contacts; winner car3 3:02.416 |
| `--cars lac,bmw` × 12 + DF01 | 1825 contacts | 3356 contacts; all 12 cars 2 valid laps |

The contact counts count every contact tick of every pair. They are chaotic
over a 2-lap race and move both ways (787→1604, 1021→868), so they are not a
measure of AI quality on their own.

## Open items

- Free-race field composition and grid order of the original: not traced.
- AI bumper points of a deformed car: the original moves them through the
  deformation lattice (0x9A39D0 when body +0xC7AD is set); the port keeps the
  undamaged hull extents (F345).
- The launchers do not pass a car (the settings file can).
- Diagnostic binaries and `engine_audio::EngineBank` remain Lacetti-only.
