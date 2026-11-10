# GRID World 3D scene (the garage) and the livery 3D preview

10 November 2026, Claude Code (F809). This is static analysis only, from the master Ghidra project and the game files; there was no live run. Code: none yet. The port's career screen draws text lists (CAREER_SCREEN.md).

This doc covers what draws behind the GRID World menus and the livery designer, and what the port needs to draw it. Tags: [V] verified in code, [O] observed in the files, [I] interpretation.

## The scene is a front-end track

- [O] There are three garage worlds, `tracks/frontend/garage_01`, `garage_02` and `garage_03`. Each is a track folder with the usual track files: land.pssg, objects.pssg, objectstextures.pssg, objecttypes.pssg, sky.pssg, trees.pssg, tracksplit.pssg, patchup_ot.pssg, lighting.xml, intro_sequence.xml, effects.xml, cloth.xml and the others.
- [O] Their routes have no race data. There is no progress_track.xml, no ai_track.xml / ai_vehicle_track.xml, no routesplit.pssg, no boundarylines.cqtc and no replay_camera_config.xml. Each route has grids.pssg, objects.ens, ornaments.bin, ornament_attributes.xml, crowd_standing.bin and clothFile.bin.
  - garage_02/03 route_0 and garage_01 route_1 also have track.jpk, track.vis, route.clm and trees.bin.
  - garage_01 route_0 has neither track.jpk nor track.vis.
- [O] Each grids.pssg has a single grid node, `grid_start_standing_0`. [I] The player's car stands there.
- [O] Each intro_sequence.xml has a `point2point` "approach" timeline. It uses `approachCam`, 2.5 s with a 1.5 s select, a linear transition in direction −x, FOV 60 and a bezier path. In garage_01 the path ends near (801.4, 463.2, −321.8).
- [V] The world for garage N loads in 0x4CD100 (it switches on the "garage on" byte +0x6D):
  - It formats `garage_%02d` from the message's garage number and passes it with `"garage"` to 0x4A7B70.
  - It runs 0x4CC8E0.
  - It sets the front-end mode +0xD4→+4 to 1 and the +0x1C8→+0xDD8 mode to 2.
  - It turns the MipVis cameras "Main Cam Near" and "Main Cam Far" off and "Main Cam FE" on.
  - Off reverses all of this (`garage`, mode 2/0, Near/Far on, FE off).
  - [I] 0x4A7B70 opens a named "_open" stream or sound bank for the garage. It searches a list at +0x84 and is not the mesh load.
- [V] 0x4CCD80 loads the garage bundle `frontend/b_garage_%02d.pssg` once, when its flags allow it (+4/+5/+6 clear, +7 set), and then clears +7.
  - [O] The three bundles are the same size (999,645 bytes). Each holds `ref_garage_sharp.dds` and `ref_garage_soft.dds`.
  - [O] The generic `frontend/b_garage.pssg` is 28.9 MB; its contents are not read yet.
- [V] 0x4CC8E0 fills the `dynamic_screen_list` with an item for each `targetzone_01..99` found that has `magnets`. The garage named items are matched through 0x477940("garage", name).
  - [I] These are the garage zones that the zone cameras move between (`garage_zone_manager`; StateGarageZone).
- [V] The reflection cube follows a mode in 0x4DCFA0 (called from 0x4E30C0 and the message handler 0x4ED780):
  - 1: the garage's `ref_garage_sharp.dds` and `ref_garage_soft.dds`.
  - 2: one texture from +0x1CC→+0x138.
  - 3: the TrackManagerPlugin's environment map (+0x164 → +0x3F20).
  - any other value: the default (+0x1E4 → +0xC).
  - Both reflection slots +0x1DC and +0x1E0 are set by 0x487680.
- [O] How many garages are open: 1 at first, 2 after the first licence, 3 after the fourth (0x4DA640, `career::state::garages`, already in the port).
- [O] The flow.xml loader states are garage_prepare, garage_load_coordinator, garage_load_audio, garage_load_cars, garage_load_track, garage_load_patch_up and garage_wait_for_load (FRONT_END_FLOW.md).
- [O] fe_paramcontrol.xml has a lighting group for each garage (`garage_01` from line 2, `garage_02` from 226, `garage_03` from 449). hud's fe3d `Lights::read` reads these groups.

## Code references not followed yet

[O] code references to strings; their roles are not analysed yet:

| String | String VA | Referenced at |
|---|---|---|
| `GarageLocationManager` | 0xCC34F8 | 0xC6E720 |
| `garage_zone_manager` | 0xCC1350 | 0x4CE086, 0x5105C6, 0x51063A, 0x5106E8, 0x580D39, 0x580E58, 0x58158B, 0x5882F7, 0x58ECCE, 0x58EE83, 0x59FB3F, 0x5AD4D0 |
| `StateEditGarageCarLivery` | 0xCDD720 | 0xC734B0, 0xC734D0 (registrars) |
| `StateGarageZone` | 0xCDD9D4 | 0xC73BD0, 0xC73BF0 (registrars) |
| `pregarage` | 0xCC1D0C | 0x4D75E0 |

- [V] `RenderGarageWorld` (0xCD7A1C) is a message class, not a renderer:
  - 0x5A30E0 returns its name;
  - its constructor 0x5A3020 sets vtable 0xCD7970 and copies four 16-byte rows from its argument (+0x10..+0x4F).
  - The pre-race info screens send it: 0x58C150, 0x5986F0, 0x5991C0 and 0x59C550 (`C.pre_race_info_%d`, the drift GP page).
  - At 0x5991C0 the first row is a position scaled by 100 with w = 1.
  - [I] It places something in the garage world (the event's car or a marker) for a 2D panel; its receiver is not found yet.

## The livery designer's 3D preview

- [O] `livery_editor_root` in `frontend/feNonPersistentDatabase.pssg` has no car. It holds `rotation` → `pitch` → `component/livery_carousel_pills` and `component/livery_carousel_main`:
  - the pills are `colour_pills`, `pattern_pills` and `number_pills`, with t = (0, −14.441, 0);
  - main is `main`.
  - The garage_01/02/03_root nodes in the same database are empty (bounds 0).
- [O] The pills' materials are fe3d effects (hud's `fe3d` module: GENERIC, GD, GPD, I, GMPD).
- [I] The car that shows the livery is the garage world's car at `grid_start_standing_0`. It is seen through the garage zone camera of StateEditGarageCarLivery, not through a model in the front-end database.

## What the port needs

These steps are shared by all three garages and every car:

1. A scene-only route. The race pipeline opens race files unconditionally: routesplit.pssg is already optional (main.rs), while progress_track.xml, the AI files and track.jpk are not.
   - Observed: `grid-app --route tracks/frontend/garage_02/route_0 --render …` stops with "The system cannot find the file specified" (local/livery-3d/garage-probe/run2.log; the same for garage_01).
   - Needed: a loader for land/objects/sky/lighting with the player's car at `grid_start_standing_0` and no race systems.
2. The garage cameras: the intro approach spline, and the zone cameras from `garage_zone_manager` / `targetzone_NN` (the code references above).
3. The front-end look: fe_paramcontrol `garage_0N` lights and the `ref_garage_*` cubes (mode 1) through hud's fe3d module. Do not make a second copy of its shaders.
4. The career screen draws the menus over that scene instead of over its plain background. The livery pages then also show the car with the edited livery (the livery textures are F799's).

None of this is implemented yet. Each step needs its own verified F entry with a matched before/after image.
