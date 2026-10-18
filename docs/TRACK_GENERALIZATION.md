# Track generalization (second circuit)

Status: 2026-10-15, Claude Code. Third circuit (Istanbul Park, 2026-10-17):
[ISTANBUL_PARK.md](ISTANBUL_PARK.md), which also replaces the start-light open
item below with per-track sequences. Fourth circuit (Long Beach) and routes whose
finish is at a later gate: [LONG_BEACH.md](LONG_BEACH.md). Goal: remove the hard-coded Donington
assumptions so another purchased circuit loads and races end to end, while the
default (Donington `route_0`) stays bit-identical.

## Route parameter

`grid_tools::route::Route` names one purchased route folder
`tracks/<region>/<track>/<route>` (lower-case ASCII, digits, `_`; exactly four
components). Its parent is the track folder. `Route::default()` is
`tracks/europe/donington/route_0`.

| Layer | Default-preserving entry | Route-aware entry |
|---|---|---|
| `source_vehicle::Runtime` | `lacetti(root, slot, seed)` | `lacetti_on(root, &route, slot, seed)`; `route()` getter; body geometry and lap timing use the runtime's route |
| `source_vehicle::multi::MultiRuntime` | `lacetti`, `lacetti_deforming` | `lacetti_on`, `lacetti_deforming_on` |
| `lap_progress` | `load(root)` | `load_route(root, &route)` |
| `entity_geometry::Props` | `read(root)` | `read_route(root, &route)` |
| `ai_driver::AiData::load` | already took the route folder | callers pass `route.path()` (no ai_driver edits) |
| `grid-opponent-race` | positional arguments unchanged | `--route ROUTE` anywhere in the arguments; prints `route=` |
| `grid-app` | unchanged arguments | `--route ROUTE` (scene, collision scene, grid slot, driving, opponents, AI, standings, HUD) |

The existing entries delegate to the route-aware ones with `Route::default()`,
so every existing caller and test is unchanged.

## Inventory of Donington assumptions (before this change)

Observed by `grep` over `crates/` at 32e747e.

| Location | Assumption | Now |
|---|---|---|
| `source_vehicle.rs` `Runtime::lacetti` | `route_0/track.jpk`, `route_0/grids.pssg` | route |
| `source_vehicle.rs` `prepare_body_geometry` | `route_0/boundarylines.cqtc`, `Props::read` | route |
| `source_vehicle.rs` `enable_lap_timing` | `lap_progress::load` (Donington) | route |
| `source_vehicle.rs` solver settings | captured live in a Donington race (world +788 block) | unchanged; treated as global world settings [I] |
| `source_vehicle.rs` CAR_ material comment | Donington barrier keys (CONC, PLAS, SIGN, SHTC, ROCK, FENC, WALL, TARM) all mix to the CAR_ row | unchanged; other tracks' keys not re-checked (open) |
| `source_vehicle/multi.rs` | `Runtime::lacetti` per car | route |
| `lap_progress.rs` | `route_0/progress_track.xml`; one route, finish gate 0 | route; the one-route/finish-0 check still applies |
| `entity_geometry.rs` `Props::read` | `route_0/objects.ens`, `objecttypes.pssg` | route |
| `entity_geometry.rs` `ornament_barriers` | `objects.pssg`, `route_0/ornaments.bin`; family list includes the Donington-only `don_ad_brd_` | route; family list unchanged (authored stand-in, see below) |
| `ai_driver.rs` | doc example and two asset tests use Donington | unchanged (tests are Donington-specific by design) |
| `time_trial.rs` | record header `donington-route0` | unchanged; the app saves bests only on the default route |
| `bin/grid-opponent-race.rs` | AI data and lap progress from Donington | `--route` |
| `bin/grid-collision`, `grid-entities`, `grid-ground`, `grid-ornament-reach`, `grid-placement-track`, `grid-spawn` | Donington paths | unchanged diagnostics (Donington evidence tools) |
| `bin/grid-practice-drive`, `grid-wall-probe`, `grid-lap-replay`, `grid-multi-drive` | `lap_progress::load` / `Runtime::lacetti` defaults | unchanged (Donington) |
| `grid-app` `load` | `donington/tracksplit.pssg` | route track folder |
| `grid-app` `append_objects` | `objects.pssg`, `route_0/ornaments.bin`, `patchup_ot.pssg`, `lemans_sponsors.pssg` | route; `track_sponsors/tag_heuer` and `bridgestone` packages unchanged (event sponsors, not track data) |
| `grid-app` `append_entities` | `route_0/objects.ens`, `objecttypes.pssg`; two Donington empty renderables mapped to closest-LOD prototypes, any other empty renderable is an error | route; other routes use the `<stem>_default_NN` -> `<stem> Root` fallback or omit (see validation) |
| `grid-app` `load_collision`, `grid_slot` | `route_0/track.jpk`, `route_0/grids.pssg` | route |
| `grid-app` driving/opponents | `Runtime::lacetti`, `lacetti_deforming`, AI data, `lap_progress::load` | route |
| `grid-app` HUD | "Lacetti / Donington ..." | track folder name, title-cased (`Donington`, `Nurburgring`, `Istanbul Park`) |
| `grid-app` start lights | Donington `start_lights.xml` constants (6 stages, 4.0 s, textures 1..5,7, material `objects/start_lights`) | unchanged; see open items |
| `grid-app` time trial | private bests keyed `donington-route0` | bests persist only when `route.is_default()` |
| `grid-app` `--scene donington` | name | kept; now means "the selected route's track scene" |

## Reader gaps found

- **AI racing-line kinds** (`grid_formats::ai::track`): Jarama ships a second
  `racing_line` waypoint per gate with `racing_line type="drift"` (150 optimal +
  150 drift). The reader rejected every non-`optimal` kind. The executable's
  racing-line enumeration (.rdata 0xCF7120 [V], AI_DRIVER_RESEARCH 1.1) has
  retirement 1, optimal 2, overtake 3, left 4, right 5, traffic 6, drift 7 and
  `start`; the race line is `optimal`. The reader now skips racing-line
  waypoints whose kinds are all from that list but not `optimal`, and still
  fails on mixed, unknown or duplicate optimal entries. Donington (only
  `optimal`) parses identically.
- **Spa-Francorchamps** `route_0/track.jpk`: `VCQTC packed count limit
  exceeded` (authored sanity limits in `grid_formats::collision`: vertices >
  1024, materials > 8 or triangles > 32768 per chunk). F399: one chunk has 9
  keys; the material field is four bits, so 16 are now allowed and Spa loads
  ([TRACK_READERS.md](TRACK_READERS.md) section 3.4).

## Candidate circuits (headless, 1 car, ported AI, `touring`/`lac`)

Evidence: `local/track-generalization/<track>-solo.txt` (main checkout).

| Route | Loads | Solo result |
|---|---|---|
| `donington/route_0` | yes | 88.420 / 86.282 s (bit-identical to F301) |
| `nurburgring/route_0` | yes | **113.857 / 110.669 s, 2 valid laps, no recovery** |
| `istanbul_park/route_0` | yes | 110.442 / 108.945 s valid, but two off-track recoveries (ticks 3381, 10191) at ~56 m/s |
| `jarama/route_0` | yes after the reader fix | off track at ~61 m/s at tick ~3130 (arc 2190 m), stuck, retired; no lap |
| `spa_francorchamps/route_0` | no (VCQTC limit) | — |

Selected second circuit: **Nürburgring GP `tracks/europe/nurburgring/route_0`**.

Jarama analysis (trace `local/track-generalization/jarama-solo-trace.tsv`):
touring brake lines 4 (gate 85, 47.6/79.3 m/s) and 5 (gate 88, 21.6/36.0 m/s)
are ~73 m apart on the racing line. The port (like 0x792C80 [V]) looks only at
the next brake line, so line 5 becomes visible 67.7 m before the corner at
62 m/s; the braking distance to 28.1 m/s is longer, so dist < b and the
brake-line path outputs 0 (the late case documented as [I] in
AI_DRIVER_RESEARCH 4.2). The car was also ~6 m off line (off-line factor 0.7).
Whether the original survives this corner (unresolved A/B corner-speed scale,
late-case brake behaviour) needs a live Jarama AI trace; not attempted.

## Validation on Nürburgring

- 4-car race, all AI, private characteristics, 2 laps
  (`grid-opponent-race --route tracks/europe/nurburgring/route_0 ROOT CD01 BP01 4 40000 ai`):
  every car 2 valid laps (car1 113.659/110.596, car3 115.101/111.682,
  car2 116.646/111.803, car0 118.315/111.414 s); classification car1, car3
  +3.120, car2 +4.512, car0 +5.120; 219 car-contact records; 14465 ticks.
- 12 grid slots load (`slot_00..slot_11`, 20-tick smoke).
- Static render `grid-app --route tracks/europe/nurburgring/route_0 --scene
  reference --view start --render local/track-generalization/nurburgring-start.png`
  (checked): start/finish straight, Lacetti on the slot_00 box, both start
  gantries, pit wall, grandstand and BMW banner. Track scene 400 draws (55
  unresolved batch draws omitted, so the road shows flat grey), objects 2040
  placed draws, `objects/start_lights` material present.
- App race: `GRID_AUTOPILOT=1 grid-app --route ... --drive --time-trial-laps 1
  --body-contacts CD01 --body-events BP01 --opponents 3 --ai-characteristics
  TSV --drive-ticks 8400` (windowed, log
  `local/track-generalization/nurburgring-race-app.txt`): lap 1 valid
  1:54.763 (sectors 42.675 / 37.822 / 34.266), no failure. The simulation
  stops at the player's finish (tick 7529), so a larger tick budget never
  exits; a `--render` run of the same race hit the fixed 120 s capture
  wall-clock limit at tick 7429 (Nürburgring lap > 2 min of wall time).
- Entity preview: the app mapped only Donington's two empty startup
  renderables to closest-LOD prototypes. On other routes an empty
  `<stem>_default_NN` renderable now previews `<stem> Root` when it exists
  (Nürburgring: armco_square_grey_end_right 6, tyrestack_red 387,
  tyrestack_white 429 instances) and is otherwise omitted with a count (827
  instances); Donington keeps the explicit pairs and the error.

## Open items

- ~~Start lights per track~~ (done 2026-10-17, ISTANBUL_PARK.md section 3): Nürburgring/Jarama `start_lights.xml` differ
  (object `gg_startlights_lights_horiz`, 7 stages over 5.0 s, textures 1..7);
  the app still uses the Donington sequence constants on every route.
- ~~Spa VCQTC count limits~~ (F399); Spa's solo AI retires; Jarama route 0 late-braking corner (route 1, the other
  direction, is the fifth circuit: JARAMA.md); Istanbul off-track excursions at ~56 m/s.
- Barrier ornament stand-in families: generic `concrete_`, `chnfen`,
  `gg_ad_brd_` match on other tracks; armco/tyre stacks stay non-solid as on
  Donington; per-track barrier keys of the CAR_ material mix not re-checked.
  Since F357 a stand-in leaves out mounted parts that lean out across the
  barrier (Long Beach catch fences; LONG_BEACH.md section 5.1).
- Diagnostic binaries listed above remain Donington-only.
