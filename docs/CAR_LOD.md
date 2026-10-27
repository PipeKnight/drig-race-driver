# Car LOD selection and car shadow casters

How the original picks a level of detail for each car and which cars draw into the shadow maps. Static analysis of the unpacked executable (`grid_re.py dis|xrefs|decompile`, master Ghidra project read-only) plus the shipped data. No runtime trace was taken. [O] means read in code or data, [I] means interpreted.

## Data

- [O] `system/render_persistent_data.xml` line 38 hands `/data/cars/carlodsettings.xml` to `VehicleManagerPlugin` with `userdata="lod settings"`. The exe has no `carlodsettings` string; the name lives only in that manifest.
- [O] `carlodsettings.xml` has 9 `lod_setting` blocks: `low medium high ultra replay_low replay_medium replay_high replay_ultra FE`. The parser reads `lod_cap_0..7` and `lod_distance_0..7` for each. The file gives caps 0..5, distances 0..6 for the race sets and 0..5 for the replay and FE sets.
- [O] Constructor defaults (0xA13994): every cap 0 [I: register holds 0], `distance[i] = (i + 1) * 5.0` (5, 10 ... 40 m). Entries missing from the file keep these values. So `lod_distance_7` is 40 for every set, and `lod_distance_6` is 35 for the replay and FE sets.
- [O] The graphics option `vehicles lodQuality` (0..3, `hardware_settings_config.xml` line 38, menu category `vehicleLOD`) picks the set: race set = quality, replay set = quality + 4, default 2/6 (0x5F48B0).

| set | caps 0..5 | distances 0..6 (m) |
|---|---|---|
| low | 1 1 1 1 6 99 | -10 20 30 50 70 150 5000 |
| medium | 1 1 2 2 7 99 | -10 20 30 50 70 150 5000 |
| high | 1 1 3 3 8 99 | -10 20 30 50 70 150 5000 |
| ultra | 1 1 4 4 8 99 | -10 10 40 60 80 150 5000 |
| replay_high | 1 1 3 3 10 99 | 5000 5001 30 50 200 5000 (35) |

## LOD meshes in the car files

[O] The loaders 0x8FF0A0 and 0x900560 put scene nodes named `xN_*` (or `*_xN`) under a group `LOD%d_` at model slot `+0xF0[N]` (16 slots). `+0x130` = coarsest slot present, `+0x134` = finest present (0x9006F3). Lacetti, from `grid-tools pssg-mesh` and node names:

| level | file | lac triangles | content |
|---|---|---|---|
| 0 | `_highLOD.pssg` | 22,688 | full car, with suspension and wishbones |
| 1 | `_highLOD.pssg` | 16,115 | without suspension and wishbones |
| 2 | `_lowLOD.pssg` | 7,613 | full outside, no interior |
| 3 | `_lowLOD.pssg` | 4,105 | no exhaust or brake pods |
| 4 | `_lowLOD.pssg` | 1,558 | body panels, glass, wheels |
| 5 | `_lowLOD.pssg` | 495 | `main_body` only (3 meshes) |

BMW and 911 low files have the same level 2..5 layout (level 5 is `main_body` only). `x1_exhaust_*_emmitter` dummies also sit in the low file. No level 6 or 7 meshes and no impostor exist. A request beyond the coarsest level hides the car (below).

[O] In the observed warm race load (docs/REFERENCE_DEPENDENCIES.md) the player car (lac) read `_highLOD`, `_lowLOD`, `_gen_high`, `_gen_low` and the high livery. The opponent BMW read only `_lowLOD`, `_gen_low` and low liveries. [I] Opponents normally own only levels 2..5, so their finest level is 2. The capture came from a warm process, so this is not proven for every mode (replays, FE).

## Selection, per frame (0x9F42A0)

The real work happens in the call 0x608940 -> 0x5FD310 -> 0xA04660 -> 0x9F42A0. 0x5FD310 passes the active CameraPlugin camera (FOV in radians at `+0x50`, converted to degrees) and the viewport aspect, then sets `+0x19D8 = 1.0`. [O] The other two callers (0xA04630, 0xA07FC0) pass an empty list, so they compute nothing. 0xA07FC0 instead applies the levels stored by the last selection pass (`+0x1A2C`, flag `+0x1A7C`).

```text
set  = active lod_setting (manager +0x1CC4)
for each car i:
    if box fully outside frustum plane 0, 2 or 3 (table 0xE500DC):
        m = max(far, 10000)                    # culled: sorts last, coarsest band
    else:
        d = |car_pos - camera_pos|             # 3D, metres; camera_pos at +0x19C0
        m = d * tan(fov/2) / tan(0.47996554)   # FOV scaling, reference FOV 55 deg
    band[i] = first k in 0..7 with m < distance[set][k], else 7     # 0xA14FD0
    sortkey[i] = m
    if offscreen_weight (+0x1CC8):              # 1 in race, set from camera in replay
        n = max corners outside plane 2 or 3
        sortkey[i] *= 2.5 if n >= 6 else 1.5 if n >= 4 else 1
order = cars sorted by sortkey ascending        # nearest first
cap = copy of caps[set][0..7]
for i in order:
    lo = model.finest (+0x134); hi = model.coarsest (+0x130)
    lod = hi if band[i] > hi else max(band[i], lo)
    if model.interior (+0x21F0) and (incar_cam (+0x1E7D) or interior.+0x42 or +0x1E7E):
        lod = lo                                # car with the cockpit forced to finest
    while cap[lod] <= 0 and lod < hi:           # cap full: push one level coarser
        lod += 1                                # (lod > 7 -> 0, unreachable with hi <= 5)
    if forced (+0x1A80 >= 0): lod = forced      # default -1, debug override [I]
    cap[lod] -= 1                               # the coarsest level consumes too (can go negative)
    level[i] = min(lod, hi)
apply (0x9CB0B0): focus car and not draw_player (+0x1CD4 == 0) -> hi + 1 (hidden)
model.set_lod (0x8F7480): empty slot -> next coarser present; none up to hi -> next finer;
                          lod > hi is stored as is and draws nothing [I]
if +0x19D8 == 0: every car gets hi + 1 (hidden)
```

Notes:

- [O] `lod_distance_N` is the upper bound of band N in FOV-scaled metres from the camera. With `lod_distance_0 = -10` the race sets never pick band 0 by distance. Band 1 covers 0..20 m (0..10 m on ultra). [I] In a race, chase-camera views draw the player at LOD1 unless the cockpit rule forces LOD0. The replay and FE sets (`distance_0 = 5000`) start everyone in band 0, and the caps give LOD0 to the single nearest car that owns it, then LOD1 to the next.
- [O] `lod_cap_N` is how many cars may use level N this frame. Cars are served nearest first (the sort key includes the off-screen penalty). A car whose level is full goes to the next coarser level. The clamp to the model's finest level comes before the cap check, so opponents that start at level 2 do not use up the LOD0 or LOD1 slots.
- [O] No hysteresis: nothing from the previous frame is read. [O] The player gets no cap exemption apart from the cockpit rule and the hide rule.
- [O] Set choice (0x607BC0): FE set 8 when `ctx+0x21C4 != 0` and 0x4CE080() (front end [I]). The replay set is used when `ctx+0x2254 -> +0x42` is set (replay playing [I]). Otherwise the race set is used. `+0x1CD4` (draw the focus car) is cleared for the `bumper`/`bumper_reverse` cameras (0x99C2F0 call in 0x607BC0). `+0x1E7D` = camera is `bonnet`/`dashboard`/`head-cam` and not 0x5ECC80.

## Shadows (0x9CB5B0, pass 0x401)

0x9CB5B0 submits the vehicles for every pass. Its last argument is the pass code: 0x400 colour pass at the current level, 0x401 shadow, 0x403 coarsest level within 200 m of the reference car (reflection pass [I]).

- [O] The CSM plugin (`CMR6PluginCSMShadows`, init 0x5CC6D0) creates "Shadow Cascade" passes 1 and 2. Their callback 0x5C02D0 (`cascade > 0`, and skipped when `+0xDD8 == 3`) submits that cascade's caster list from the VehicleManager with code 0x401 and only the non-focus targets. 0x5CCDC0 (`car_layer1`, `car_layer2`) submits two lists with code 0x401 and only the focus-car targets.
- [O] Per car in pass 0x401: skipped when hidden (current > coarsest), when `+0x3A34 != 0` (ghost or fade [I]) or when the car sits in the 3-slot table at manager `+0x3180` with `+0x3914 != 0`. Shadow mesh = the model's coarsest level (`+0x130`, LOD5 `main_body`, 495 tris on lac). Exception: the focus car (`+0x1EA8`) uses its current level when its coarsest level is above 2. A car with the cockpit shown (`interior.+0x42`) draws the coarsest slot + 1 plus the interior node.
- [I] So every visible opponent in a cascade's caster list casts a shadow with its coarsest body. The player car casts at its drawn level into its own car layers. The engine has no separate count cap and no blob shadow. Not traced: how each cascade's caster list is culled (cascade ranges `shadowRange0..2`, 0x5C04C0). The track `shadow_dist`/`shadow_obj_size` values (0x62E680, `route_overrides.xml`, e.g. Donington 150 m) belong to track-object culling and have no link to cars.
- [O] The option `shadows enabled` (off on low presets) toggles the whole system (`+0x18B`, 0x5CC6D0).

## Port

Before F600: `crates/grid-app/src/main.rs` `load` always read `_highLOD.pssg`, and `--lod` accepts only `LOD0_`/`LOD1_` (default `LOD0_`). `append_car` loads the player and each opponent model with the same `lod`. Everyone drew LOD0 (22.7k tris on lac), `_lowLOD.pssg` was never read, and every car part cast shadows unless marked `NotShadowCaster` (car_batch.rs).

F600 (shadows): `append_car` also reads the model's `_lowLOD.pssg` and merges its coarsest level (the highest of `LOD5_`..`LOD2_` present) into one position mesh in the parts' rest frame (`shadow_body`). Each opponent gets a `car_batch::ShadowBody` with that mesh on render layer 29; the sun's layers become 0 and 29, so the body casts while no camera draws it. car_batch poses the body with the car (the pose its batches use) and hides it when none of the car's parts is in place. That car's parts and batches get `NotShadowCaster`, also a part that leaves its batch (a loose panel casts no shadow of its own). The player keeps its parts' shadows. A model without a low file keeps its parts' shadows. With `GRID_CAR_BATCH=0` nothing changes (no bodies).

AUTHORED / not ported: the original submits opponents only to CSM cascades 1 and 2; Bevy draws a caster into every cascade whose frustum holds it. The original's focus-car `car_layer` maps are not modelled.

## Unknown

- Whether the replay and FE modes load `_highLOD` for opponents too.
- The exact meaning of `ctx+0x2254 +0x42`, `+0x1E7E`, the `+0x3180` table and `+0x3A34`.
- Whether 0xBF7112 is `tan` (it is the CRT math helper also used for `tan(pi/2 - fov/2)` in 0xA04660), and whether the FOV is vertical.
- Frustum planes 0/2/3 are probably near/left/right, not checked.
- How often 0x5FD310 runs per frame (one camera or several).

## Addresses

| address | role |
|---|---|
| 0x9CA3A0 | VehicleManager xml dispatch (`materials`, `damage spec`, `lod settings`) |
| 0x99C130 | `carlodsettings.xml` parser, 9 sets x 8 caps (`+0x1A84`) and distances (`+0x1BA4`), stride 0x20 |
| 0xA135C0 | VehicleManager constructor: default distances (i+1)*5, `+0x1CC4 = 2`, `+0x1A80 = -1`, `+0x19D8 = 1` |
| 0x5F48B0 | `vehicles lodQuality` -> race set, replay set |
| 0x607BC0 | per-frame set choice, `+0x1CC8`, `+0x1CD4`, `+0x1E7D`, `+0x1E7E` |
| 0x5FD310 -> 0xA04660 | camera FOV, aspect and projection -> selection |
| 0x9F42A0 | LOD selection: FOV-scaled distance, sort, caps |
| 0xA14FD0 | distance -> band |
| 0x9CB0B0 | apply levels; hide the focus car when `+0x1CD4 == 0` |
| 0x8F7480 | model set_lod with fallback over empty slots |
| 0x8FF0A0, 0x900560 | `xN_` / `_xN` node grouping into `LOD%d_` slots; `+0x130`/`+0x134` |
| 0xA07FC0 | applies the stored levels (`+0x1A2C`) |
| 0x9F4D30 | writes the `+0x1A80` override [I debug] |
| 0x9CB5B0 | vehicle pass submission; 0x401 = shadow at the coarsest level |
| 0x5C02D0 | CSM cascade 1/2 caster callback (vehicles, characters, trees, ornaments) |
| 0x5CCDC0 | focus-car shadow layers `car_layer1/2` |
| 0x5CC6D0 | CSM plugin init, `shadows enabled` |
| 0x848E50 | a different `lod settings` parser (6 caps, `default/low/.../replay/FE`), reached via 0x8F2A60 with `characterQuality` (driver characters [I]) |
