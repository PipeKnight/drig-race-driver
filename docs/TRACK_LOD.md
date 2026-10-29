# Track scenery LOD and culling

How the original picks the level of detail of track scenery (placed ornaments, Washington's buildings, world geometry) and which distances cull it, and what the port does (`crates/grid-app/src/ornament_lod.rs`). Static analysis of the unpacked executable (`grid_re.py lookup|xrefs|dis|decompile`, master Ghidra project read-only) plus the shipped data, 27 October 2026. No runtime trace of the original was taken. Labels: [O] observed in the original (data or code), [I] inferred, [?] open. Game-derived tables stay in private `local/track-lod/`.

## 0. Short answer

- **Track surface (`tracksplit.pssg`), land, route pieces:** no geometric LOD [O]. Each 2D cell
  `ROOT_x_y` holds `NONLOD_x_y` (per-material draws) and `NONLODBATCH_x_y` (the same triangles
  merged into one `batchmaterial` draw), plus `DECAL`/`DECALBATCH`. Washington: 68 cells, NONLOD
  523 draws / 104,976 tris, NONLODBATCH 68 draws / 104,969 tris, decals 79 draws / 18,626 tris.
- **Ornaments (all placed objects, including every Washington building):** up to 3 levels per
  prototype in `objects.pssg` (`LODVISIBLERENDERNODE` direct instances = level 0, then
  `LODRENDERINSTANCES` with 1–2 `LODRENDERINSTANCELIST`s = levels 1, 2) [O]. The runtime picks the
  level from the route's `ornament_attributes.xml` distances (`lod_distance_00/01/02`), not from
  the PSSG `lod` values [O].
- **Selection rule** (0x96FBC0 -> 0x965790 -> 0x96EC80, ranges from 0x95D2A0) [O]:
  `d` = distance from the camera to the instance's AABB measured along the camera-to-centre
  direction to the nearest box corner; `k = tan(27.5 deg) / tan(fov/2)`;
  `d' = d / ((1 - d/D)(k - 1) + 1)`; `v = 1 - d'/D` (D = `lod_distance_02`), times a global scale
  (default 1.0). `v <= 0` -> culled. Level i draws while `r0_i < v <= r1_i`, i.e. level 0 for
  `d' in [0, d0)`, level 1 for `[d0, d1)`, level 2 for `[d1, D)` (with 3 levels; 2 levels: `[0,d0)`,
  `[d0,D)`). "scenery" prototypes extend each non-last level by 20 m and cross-fade
  (`xFadeWeight`) in that overlap. No hysteresis [O: nothing from the last frame is read].
- **lod_overrides.xml / route_overrides.xml** do not pick ornament levels. They set per-pass
  distances: `world_cull_dist` / `world_clip_dist` go to the main scene render task (+0x48 cull
  far, +0x44 projection far); `track_cull_dist` goes to the ornament/track system (+0x3EF0) where
  the `SURFACELOD` blend function uses it as an outer cut-off; `refmap_dist` is the reflection
  pass cull far; `*_obj_size` are minimum-size thresholds handed to each pass's visibility query.
  Route values override the track file per **route block** (start/end in metres along the route),
  blended over `fade` metres at block edges [O, 0x62EBF0].
- **Shadows:** ornament shadow casters use **level 1** (the middle level) in both CSM cascades
  (0x96EC80 pass 0x401) [O]. The XML `shadow_dist` is parsed but the per-frame updater ignores it
  and always outputs 225 m (constant 0xDB4C88) [O]; `shadow_obj_size` is applied [O]. Where the
  225 m output is consumed is not traced [?].
- **Washington route 0, ornament totals** (3,449 instances of 214 prototypes; every instance at
  the given level): L0 4,443 draws / 1,346,625 tris; L1 4,349 / 734,229; L2 3,850 / 365,297.
  The 157 building ("scenery") instances: L0 637 / 285,656; L1 621 / 147,588; L2 175 / 18,152.
  Before the port (F633) grid-app drew L0 for all of them at every distance.

## 1. Levels present in the files

### 1.1 Track surface and land [O]

`tracksplit.pssg` (Washington, 29 MB): `Scene Root > surface > ROOT_x_y` (68 cells), each with
RENDERNODEs `NONLOD_x_y`, `NONLODBATCH_x_y` and on 30 cells `DECAL_x_y`, `DECALBATCH_x_y`. The
batch nodes carry the same triangles as their non-batch twins in one draw (difference 7 tris over
the track). The names say "no LOD". `route_0/routesplit.pssg`: 24 cells, NONLOD 180 draws /
33,847 tris (+ batch twin), decals 5 / 416. `land.pssg`: `landscape` 4 draws / 2,694 tris,
`nonlod_distant_wall` 172 tris, `sea_mesh` 229 tris. No `LOD%d_` groups in any of these. (The
`LOD%d_` string 0xD0DE88 belongs to the car loaders 0x8FF0A0/0x900560, docs/CAR_LOD.md.)

[I] What the batch twins are for is not traced (a merged draw for shadow/reflection passes, or a
far-cell draw). `grid-app` loads the non-batch draws by default (`--include-batches` adds them).

### 1.2 Ornament prototypes (`objects.pssg`) [O]

Each placed prototype is a `ROOTNODE "<name> Root"` with:
- `LODVISIBLERENDERNODE "lod!N"`: its direct `RENDERSTREAMINSTANCE` children are level 0;
- a child `LODRENDERINSTANCES` (`lodCount` = 1 or 2) with `LODRENDERINSTANCELIST`s, attribute
  `lod` = 0.6667 and 0.3333 (3 levels) or 0.5 (2 levels): levels 1 and 2;
- `NODE "default!N" > NODE "rigidbody!N" > RENDERNODE "<name>_physics"`: a copy of level 0
  (collision/physics representation; `grid-app` already skips it).

Washington `objects.pssg`: 229 prototypes; 154 with 3 levels (`lod` 0.6667/0.3333), 20 with 2
levels (0.5: e.g. `building_distant_a..c`), 55 with 1 level (all `horizon_*`, all
`overhead_object_*`, some barriers). Donington: 120 / 6 / 7. The runtime limit is 3 levels
("Too many LODs found", 0x97CE80) and level count = `lodCount + 1` [O].

[I] The PSSG `lod` values match the runtime's normalised space `v = 1 - d/D` (0.667 = one third of
D), i.e. they are authoring defaults; GRID replaces them with the attribute distances
(0x95D2A0 never reads them) [O for "never reads them" within the traced path].

Washington big buildings (draws, tris per level L0 / L1 / L2; `ornament_attributes` distances
d0/d1/D; instances in route 0):

| Prototype | L0 | L1 | L2 | d0 / d1 / D (m) | Inst. |
|---|---|---|---|---|---|
| building_department_a_vara | 5 / 4,256 | 5 / 2,278 | 4 / 862 | 100 / 200 / 700 | 1 |
| building_features_05_a_vara | 4 / 6,652 | 4 / 2,473 | 1 / 120 | 100 / 310 / 700 | 2 |
| building_features_05_a_varb | 3 / 3,407 | 3 / 1,095 | 1 / 42 | 150 / 300 / 750 | 5 |
| building_02_01_b | 5 / 2,515 | 5 / 1,480 | 1 / 138 | 100 / 315 / 900 | 7 |
| building_05_01_c | 3 / 1,826 | 3 / 890 | 1 / 106 | 100 / 320 / 400 | 9 |
| building_01_02_c | 2 / 2,820 | 2 / 1,386 | 1 / 94 | 100 / 300 / 750 | 5 |
| building_03_01_a | 7 / 1,610 | 7 / 939 | 1 / 72 | 100 / 300 / 780 | 8 |
| building_church_a | 5 / 2,532 | 5 / 1,263 | 2 / 150 | (see attributes) | |
| building_art_a | 2 / 3,030 | 2 / 1,583 | 1 / 149 | | |
| gg_building_grandstand_01_j | 4 / 2,352 | 4 / 1,828 | 4 / 1,465 | (non-scenery) | 17 |
| horizon_06_a | 1 / 35 | – | – | 3000 / 3000 / 5000 | 15 |

Full per-prototype list: `local/track-lod/wash-objects-all.txt` (all), `wash-objects-levels.txt`
(buildings/horizon/overhead). Typical Washington building: L1 ~ half of L0, L2 ~ 5 % of L0 and a
single draw.

Whole route 0 (instances from `route_0/ornaments.bin` prototype record +28, prototype levels from
`objects.pssg`; a prototype with fewer levels counts its coarsest for the missing ones):

| | draws | triangles |
|---|---|---|
| all 3,449 ornament instances at L0 | 4,443 | 1,346,625 |
| all at L1 | 4,349 | 734,229 |
| all at L2 | 3,850 | 365,297 |
| 157 building ("scenery") instances at L0 / L1 / L2 | 637 / 621 / 175 | 285,656 / 147,588 / 18,152 |
| track surface (NONLOD) + decals + routesplit | 523 + 79 + 185 | 104,976 + 18,626 + 34,263 |

Biggest L0 contributors are not buildings: `tyrestack_white_double` (67 x 1,598), `armco_main_section`
(959 x 104), `tyrestack_white` (118 x 744), `concrete_block_set_a` (139 x 418), `street_lamp_b`.
`chnfen_e_group2` is one 19,988-tri L0 (L1 3,846, L2 152).

### 1.3 Ornament categories in `ornaments.bin` [O]

The prototype record words at +44/+48 (docs/ORNAMENT_PLACEMENTS.md calls them opaque) are: +44 an
offset to a category string or 0xFFFFFFFF; +48 a 0/1 flag. The only category string is `scenery`.
Washington route 0: 58 scenery prototypes (all `building_*`, 157 instances), 156 without;
Donington 7 / 124. 0x980830 compares the string with `scenery` (0xA66720) and passes the result
to the type builder 0x97D9D0 -> 0x97CE80 (`+0x15C = 1` for scenery). The +48 flag is passed as the
type builder's last argument and links the type into a list at system +0x2C [I: purpose unknown].

## 2. XML files and fields

### 2.1 `tracks/<region>/<track>/lod_overrides.xml` (binary XML) [O]

Root `xml`, one element `LOD`. Parsed by 0x64C310 (data package `overrides`, dispatcher 0x64C560)
into the route-track plugin object (`this`, vtable 0xCE8D40):

| Attribute | Field | Washington | Donington | Default (0x64C070) |
|---|---|---|---|---|
| crowd_max_distance | +0x59C | 110 | 110 | 165 |
| crowd_max_shadow | +0x5A0 | 2 | 2 | 2 |
| track_cull_dist | +0x5D0 | 460 | 1500 | 400 |
| world_cull_dist | +0x5C8 | 900 | 1500 | 800 |
| world_clip_dist | +0x5CC | 1800 | 1800 | 10000 |
| shadow_dist | +0x5D4 | – | – | 225 |
| refmap_dist | +0x5D8 | – | – | 2500 |
| main_obj_size | +0x5DC | – | – | 0.004 |
| shadow_obj_size | +0x5E0 | – | – | 0.02 |
| refmap_obj_size | +0x5E4 | – | – | 0.2 |

A child `Systems` element may hold `shadows` / `reflections` booleans (+0x5A8 default 1, +0x5A9
default 0), forwarded to the OrnamentsPlugin (0x9521E0: system +0xD0/+0xD1) and to the main scene
task. Neither track file has it. The crowd values go to the OrganismPlugin (0x64C7E0).

### 2.2 `route_N/route_overrides.xml` (binary XML) [O]

Root `route`; parsed by 0x62E680 (data package `route_override`). Elements:
- `default`: track_cull_dist, world_cull_dist, shadow_dist, refmap_dist, fade (+0x610, default
  25), main/shadow/refmap_obj_size -> +0x5AC..+0x5C4 (0 = not set).
- `block0` .. `block31` (stops at the first missing index): `start`, `end` (swapped if reversed),
  `fade` (default = the route default fade), the same seven values. Stored as 10 floats (stride
  0x28) at +0x7C, count at +0x57C. A block whose start is < 1 m gets `start -= fade` (no fade-in at
  the route origin). Clamps: track/world cull <= 3000, refmap <= 500, shadow <= 250.
- `replay`: the same seven values for replays (+0x5E8.., flag +0x604); without it the replay set
  copies the default.
- Running maxima of track/world/shadow/refmap over all sources at +0x580..+0x58C [I: used to size
  something; consumer not traced].

Washington route 0: default track 180, world 600, shadow 180, fade 20; 12 blocks covering
0–5265.5 m with track_cull 170–500 and world_cull 345–850 (e.g. block6 1890–1965 m: track 320,
world 345; block11 4495–5265.5 m: track 500, world 850). Donington route 0: default track 2000,
world 2000, shadow 150, fade 30, refmap_obj_size 0.5, plus crowd values (not read by 0x62E680)
and no blocks. Decoded tables: `local/track-lod/washington-route0.tsv`, `donington-route0.tsv`
(hex), readable via `py -I <scratch>/dec.py`.

### 2.3 `route_N/ornament_attributes.xml` (binary XML) [O]

`ornament_attribute_list > ornament_attributes*`, parsed by 0x9522F0 into 0x40-byte records
(name, `instance_buffer_max_00..02` at +0x24.. (default 100), `lod_distance_00..02` at +0x30..
(default 30/60/100), `key_feature` at +0x3D). 0x97D9D0 copies the record into the type entry
(stride 0xC0, record at +4, so d0/d1/D at +0x34/+0x38/+0x3C, key_feature +0x41) and clamps each
buffer max to the prototype's instance count. Washington: 262 records, 110 key features. Buildings
use d0 80–380, d1 200–490, D 400–1500 m (`building_03_02_b` 380/490/900); horizons 3000/3000/5000
(single level, so only D matters). Full table: `local/track-lod/wash-orn.txt`.
A debug writer exists (`ornament_attributes_%d_out.xml`, 0x95CD90).

### 2.4 `tracks/tracklodsettings.xml` (text) [O]

`<SURFACELOD loddistance="30" blend="10" />`, read by 0x952B30 (via the dispatcher's default
branch) into OrnamentSystem +0x3EE8 / +0x3EEC (defaults 30 / 0). See 3.4.

### 2.5 `tracks/ornament_mip_streaming.xml` [O]

Texture mip streaming spheres around the camera (far r=30 m at +20 m ahead, near r=5.5 m...).
Texture streaming only; not geometry LOD.

## 3. Runtime

### 3.1 Ornament level selection (the part that matters for buildings)

Call chain per pass: pass callback (main scene 0x5B6E00, CSM cascades 0x5C02D0, reflection and
other passes 0x5BCEA0, 0x5EA6A0, 0x5EC880) -> 0x979430 (static instances) / 0x979590 (second
list, not decoded) -> 0x96FBC0 per instance -> 0x965790 (metric) -> 0x96EC80 (level dispatch) ->
0x95D880 (append to the level's instance buffer).

0x979430 [O]: `k = (1/tan(fov/2)) / (1/tan(0.47996554))`, fov = camera +0xF4 in degrees
(0.47996554 rad = 27.5 deg, the same reference as cars; same caveat that 0xBF7112 is taken to be
`tan`). Iterates all instances (`+0x288[0..+0x290]`) when the pass has no visibility list,
otherwise the pass's visible-instance list (u16 indices, stride 8) [O]; the list comes from the
pass's visibility result (`+0x340` lists at +0x1C/+0x20 and +0xC4/+0xC8 in 0x5B6E00) [I: built by
the vis query with the pass's cull far and min size, 3.3].

0x96FBC0 [O], per instance (type = instance +0xA0):
- pass 0x401 (shadow): skip if instance +0xC4 set, or cascade 2 and instance +0xC5 set;
- pass 0x402: only key features; pass 0x404: non-key features use D = 70 m instead of their own;
- `v = metric(instance AABB at +0x20, camera position, D = type lod_distance_02, k) * system +0x84`
  (+0x84 = 1.0 from the constructor 0x97D400; no other writer found [I: constant]);
- `v > 0` -> 0x96EC80 and 0x9529A0.

0x965790, the metric [O]:
- `dir = normalise(centre - camera)`; `d = dot(nearest_corner - camera, dir)` where
  `nearest_corner` takes, per axis, the box min if `dir` is positive on that axis, else the max.
  So d is the distance to the near side of the box along the line to its centre, not the centre
  distance; it is <= 0 when the camera is inside the box.
- `d <= 0` -> 1.0; `d >= D` -> 0.0; else `1 - (d / ((1 - d/D)(k - 1) + 1)) / D`.
- With k = 1 (55 deg FOV) this is `1 - d/D`. A narrower FOV (k > 1) shrinks the effective
  distance near the camera but leaves the cut-off at D unchanged.

0x95D2A0, per-level ranges in `v` space (type +0x154, stride 0x180) [O]. With n levels,
`ext = 20` for scenery types (else 0):
- level i < n-1: `r0 = clamp01(1 - (d_i + ext)/D)`, `r1 = clamp01(1 - d_{i-1}/D)` (1.0 for i = 0),
  `r2 = clamp01((1 - d_i/D) - r0)` (= ext/D, the cross-fade width);
- last level: `r0 = 0`, `r1 = clamp01(1 - d_{n-2}/D)`, `r2 = 0`;
- one level: `(0, 1, 0)`.
In distance terms (k = 1): L0 [0, d0 + ext), L1 [d0, d1 + ext), L2 [d1, D). With 2 levels
`lod_distance_01` is unused.

0x96EC80, dispatch by pass code [O]:
- 0x400 (colour): every level with `r0 < v <= r1` gets the instance (two levels inside a
  scenery overlap);
- 0x401 (shadow, cascades 1 and 2 only): level 1 if the type has >= 2 levels, else level 0;
- 0x402 / 0x403 / 0x404: the coarsest level.

0x95D880, append [O]:
- falls back to the next finer level when the type has no buffer for that (pass, level)
  (table below);
- per (level, pass) instance counter vs a limit derived from `instance_buffer_max_NN`: when full,
  the instance is not added at that level [O: the add is skipped]. [I] Order is the list order,
  not distance, so which instance is dropped is arbitrary. 0x97CE80 splits each buffer max into
  two numbers (>= 3: instanced buffer of that size; < 3: that many individual slots) [I: meaning
  of the two numbers];
- scenery types in pass 0x400 inside the overlap: shader parameter `xFadeWeight = clamp01((v -
  r0)/r2)` on the fade variant [O]; the counters reset each frame (0x95D200, 0x951C40) [O].

Per-pass level table at 0xE4F91C (11 passes x {level0, level1, level2 flags, float}); built per
type by 0x951E40, which drops passes 1/2 when shadows are off, pass 10 when reflections are off,
and passes 3..8 unless the type is a key feature [O]:

| Pass | Levels | Float | Gate |
|---|---|---|---|
| 0 | 0, 1, 2 | 1.0 | – (main colour) |
| 1 | 1 | 1.0 | shadows |
| 2 | 1 | 0.2 | shadows |
| 3, 4, 6, 7 | 2 | 0.2 | key feature |
| 5 | 2 | 1.0 | key feature |
| 8 | 2 | 0.6 | key feature |
| 9 | 1, 2 | 0.6 | – |
| 10 | 2 | 1.0 | reflections |

[I] Passes 3..8 are six faces of a dynamic cube map (only key features reflect, coarsest level);
pass 10 the track reflection; pass 9 a mirror. The float's use is not traced.

### 3.2 Route-block distance updater 0x62EBF0 (route-track plugin vtable slot) [O]

Per frame, if the system flag 0xDB0201 is set:
1. Start from the `lod_overrides` values (+0x5C8.. ). Overlay route `default` values that are
   set (cull/refmap distances > 1.0, sizes > 0). Shadow distance is not part of this set.
2. If +0x60C <= 0 (route position not valid [I]): use the `replay` set instead when +0x604 is
   set, then output.
3. Else, if the external override struct (+0x78, byte 0 set) is active, copy its values into both
   the lod_overrides and the route default fields and output those.
4. Else find the block with `start <= pos < end`, pos = +0x608 (metres along the route [I]).
   Fade width `w = min((end - start)/2, fade)`.
   - In the first `w` metres: `t = (pos - start)/w`; base = average of the previous and this block
     (per field, when both are set) if the previous block ends exactly at this start **and the
     block index is >= 2** (`cmp idx,1; jle`, so block 1 never blends with block 0 [O, likely an
     original slip]); otherwise base = the defaults from step 1.
   - In the last `w` metres: `t = (end - pos)/w`, base = average with the next block if it starts
     exactly at this end (and this is not the last block).
   - Otherwise t = 1. Each set field = `base + (block - base) * t`.
   No block contains pos -> step 1 values.
5. Output to +0x620.. : track cull, world cull, world clip, refmap dist, **shadow dist = 225
   constant (0xDB4C88)**, main/shadow/refmap obj size (refmap size clamped to <= 0.1 when +0x615).
6. Push: main scene task interface (+0x5A4, slot 1 = 0x5B71A0) gets (world cull, world clip);
   OrnamentSystem (+0x590) +0x3EF0 gets track cull.

0x64C7E0 does the same push once after loading (with the lod_overrides values) plus crowd values
and the shadows/reflections switches.

### 3.3 Where the values go [O unless marked]

- **world_cull_dist / world_clip_dist** -> main scene render task ("vis:main scene", 0x5C8080,
  vtable 0xCE088C) +0x48 / +0x44. The base render task (0x64BCD0) keeps near +0x40, far clip +0x44,
  cull far +0x48, min object size +0x4C; the generic setter 0x5B51A0 writes (+0x40, +0x44, +0x48).
  The main scene camera setup 0x5C7F90 passes +0x44 to the camera matrix builder 0x63A370 as the
  projection far plane. In the cube-face pass 0x5BCC40 the visibility frustum uses +0x48 (x1.5 on
  some faces) and the render projection +0x44. [I] So world_cull_dist is the far plane of the
  visibility/cull frustum and world_clip_dist the depth far plane; an object (ornament, track cell)
  beyond world_cull is not in the visible list at all. Not traced: the vis query's use of +0x48.
- **track_cull_dist** -> OrnamentSystem +0x3EF0, used by 0x952CB0 (3.4). [?] Whether it also
  culls track-surface cells directly.
- **refmap_dist** -> reflection pass cull far (0x5FAD10: setter with near 0.1, far 1000, cull =
  refmap_dist).
- **main_obj_size** -> main scene +0x4C; another pass gets 15 x main_obj_size;
  **refmap_obj_size** -> reflection passes +0x4C; **shadow_obj_size** -> game render +0xF04;
  **shadow_dist (225)** -> game render +0xCC8 (0x5FAD10). +0x4C is copied into the vis query input
  next to the frustum (0x5BCC40 -> +0x150) [I: minimum projected size, fraction of the screen].

### 3.4 SURFACELOD weights 0x952CB0 (OrnamentSystem vtable 0xD13C64 slot 1) [O]

`x = a / 0.959931 * b` (0.959931 rad = 55 deg: distance scaled by FOV / 55 deg, linear in the
angle). With L = loddistance (30), B = blend (10), C = track cull:
`x < L - B` -> (near 1, far 0); `x > C` -> (0, 0); `x > L + B` -> (0, 1);
`L - B <= x < L` -> (1, (x - L + B)/B); `L <= x <= L + B` -> (1 - (x - L)/B, 1).
[I] Two cross-faded weights for near/far detail of the track surface (e.g. detail map vs
plain), beyond track_cull nothing. Caller and the meaning of the two weights not traced.

## 4. Shadows

- Ornament casters: level 1 in cascades 1 and 2 [O, 0x96EC80/0x96FBC0]; instance flags +0xC4
  (never casts) and +0xC5 (not in cascade 2). With shadows off (`Systems shadows`, or the option)
  passes 1/2 have no buffers [O].
- Cars cast their coarsest level (docs/CAR_LOD.md); scenery casts its middle level. So shadows of
  buildings use a coarser mesh than the colour pass only inside d0.
- `shadow_dist` from both XMLs is parsed and clamped (<= 250) but not used by the updater; the
  output is always 225 m [O]. `shadow_obj_size` (Washington: not set -> 0.02) is applied [O].
  What the game render object does with +0xCC8/+0xF04 (caster range / min caster size per cascade)
  is not traced [?]; 0x5C04C0 (cascade setup) was checked and does not read these.

## 5. Port (F633)

`crates/grid-app/src/ornament_lod.rs`, common build; `GRID_ORNAMENT_LOD=0` keeps the old picture (every ornament at level 0, merged by `static_batch`).

- Loading: `scene::read_object_level_draws` returns every level with its number (level 0 = direct children of `LODVISIBLERENDERNODE`, then each `LODRENDERINSTANCELIST` in file order). `main.rs::ornament_types` reads `ornament_attributes.xml` (missing record: 30/60/100 m), the `scenery` category from `ornaments.bin` +44, and each placement's world box from the prototype bounds (+4..+28) through the placement matrix. Washington route 0: 214 types, 1,970 instances with a stored transform in `ornaments.bin` (the other 1,479 of the 3,449 counted have none; [I] they are entity placements such as breakable props, which keep their own path and stay at level 0), 58 scenery types, 4 types without a record.
- Each placed level draw is one entity with `OrnamentLevel`. Every frame `select` computes `v` per instance (0x965790, the camera's vertical FOV, global scale 1.0) and shows the level whose 0x95D2A0 range holds it; nothing beyond D. No hysteresis, as in the original.
- Scenery overlap: the original draws both levels for 20 m and cross-fades with `xFadeWeight`; the port switches at the middle of the overlap (fade weight 0.5) [difference].
- Shadows: the type's level 1 (level 0 for single-level types) casts at every distance inside D (0x96EC80 pass 0x401). While it is not the colour level it sits on `car_batch::shadow_layer()`, which only the sun takes. The other levels are `NotShadowCaster`.
- Level entities are not merged by `static_batch` (a merged mesh would be rebuilt whenever one of its instances changes level, a full re-upload).
- Not ported: per-route-block `world_cull_dist` / `world_clip_dist` (whether world_cull culls per object is inferred, and the camera far plane belongs to camera.rs), `track_cull_dist` and the SURFACELOD weights, the instance caps (`instance_buffer_max`, overflow rule uncertain), the key-feature passes (cube map, mirror).

## 6. Open

- Who consumes world_cull (+0x48) in the visibility query, and whether it culls per object or only
  bounds the PVS/frustum; whether track cells are culled by track_cull_dist.
- 0x979590 (the second ornament list, probably animated/dynamic instances) not decoded.
- The per-pass float in 0xE4F91C and the two numbers from `instance_buffer_max`.
- What +0x608 (route position) and +0x60C (gate) are exactly; who writes them.
- Consumers of shadow_dist output (+0xCC8) and shadow_obj_size (+0xF04); the running maxima
  +0x580..+0x58C.
- The caller of 0x952CB0 and what the SURFACELOD weights blend.
- Whether the NONLODBATCH twins are used for a pass (shadow/reflection) or distance.
- `+0x84` scale: only the constructor value 1.0 seen.
- Runtime verification against the original (e.g. a capture at a Washington position counting
  drawn ornament levels) has not been done; everything above is static.

## 7. Addresses (for docs/re/symbols.tsv)

| Address | Role |
|---|---|
| 0x0062E680 | route_overrides.xml parser (default, block%d, replay) |
| 0x0062EBF0 | route-track per-frame distance updater (block blend, outputs +0x620..) |
| 0x0063B2A0 | route-track plugin constructor (vtable 0xCE8D40) |
| 0x0064C070 | route-track distance defaults/reset (0xDB4C7C..0xDB4C9C) |
| 0x0064C310 | lod_overrides.xml parser (`LOD`, `Systems`) |
| 0x0064C560 | route-track data-package dispatcher (`overrides`, `route_override`, else SURFACELOD) |
| 0x0064C7E0 | route-track post-load push (crowd, shadows/reflections, cull distances) |
| 0x00652750 | route-track init (creates OrnamentSystem +0x590 via 0x985050) |
| 0x005B51A0 | render task setter (near +0x40, clip +0x44, cull +0x48) |
| 0x005B71A0 | main scene interface slot 1: world cull -> +0x48, world clip -> +0x44 |
| 0x005C8080 | main scene render task constructor ("vis:main scene"; defaults cull 1000, clip 10000) |
| 0x005C7F90 | main scene camera setup (far = +0x44 into 0x63A370) |
| 0x005FAD10 | per-frame pass setup from route-track outputs (refmap, obj sizes, shadow 225) |
| 0x0064BCD0 | base render task constructor (+0x40 1, +0x44 100, +0x48 100) |
| 0x009522F0 | ornament_attributes.xml parser (0x40-byte records) |
| 0x00952B30 | tracklodsettings SURFACELOD parser |
| 0x00952CB0 | SURFACELOD near/far weights with track cull cut-off |
| 0x00951E40 | per-type pass/level buffer table from 0xE4F91C |
| 0x0095D2A0 | per-level v ranges from lod_distance_00..02 (+20 m for scenery) |
| 0x0095D880 | append instance to (level, pass) buffer; cap; xFadeWeight |
| 0x00965790 | ornament distance metric (near-corner distance, FOV-warped, normalised by D) |
| 0x0096EC80 | level dispatch by pass code (0x400 range test, 0x401 level 1, 0x402-0x404 coarsest) |
| 0x0096FBC0 | per-instance LOD evaluation (shadow flags, key-feature gates) |
| 0x00979430 | static ornament pass submit (k = tan 27.5 / tan(fov/2)) |
| 0x0097CE80 | ornament type render setup (levels = lodCount + 1, <= 3) |
| 0x0097D9D0 | ornament type creation (attributes copied, buffer max clamped) |
| 0x00980830 | ornaments.bin instance list loader (scenery category) |
| 0x00985050 | OrnamentSystem constructor (0x3F30 bytes, vtable 0xD13C64) |
| 0x00DB4C88 | constant 225.0: shadow_dist default and fixed output |
| 0x00E4F91C | ornament pass table (11 x {3 level flags, float}) |

## 8. Evidence and reproduction (research)

- Tools used (all existing): `C:\Dev\Reassembly\target\release\grid-binxml.exe` (binary XML ->
  hex TSV), `tools/survey_pssg_payloads.py` (PSSG node survey), `tools/re/grid_re.py`
  (lookup/xrefs/dis/decompile, master project read-only). Scratch scripts (not committed) in the
  session scratchpad: `count.py` (draws/tris per LOD per prototype), `orninst.py` (instance
  weighting), `scan.py`/`fscan.py`/`bytes.py` (displacement searches).
- Private outputs: `local/track-lod/` in this worktree (`*-lod.tsv`, `*-route0.tsv`,
  `wash-orn.txt`, `wash-objects-all.txt`, `wash-objects-levels.txt`, `wash-scenery.txt`).
- New decompiles cached in `C:\Dev\Reassembly\local\re-cache\` this session: 0064C070, 009522F0,
  0095CD90, 00961640, 009637C0, 00952B30, 009521E0, 0064ADB0, 0064B100, 0064C7E0, 00898EB0,
  0062F420, 00952C10, 00985050, 00966B40, 00966080, 00982830, 00980830, 0097D9D0, 0097CE80,
  00619AD0, 00951E40, 0095D2A0, 00951C40, 0095D880, 0095D5D0, 0095D200, 00951CB0, 009514F0,
  00951740, 00951940, 0096EC80, 0096FBC0, 00965790, 00979430, 00979590, 005C8080, 005C88D0,
  005C85D0, 005C8570, 005C8A10, 005C8370, 005BCC40, 0064BCD0, 00643BC0, 005B6E00, 005C04C0.
  0x62EBF0 is not a function in the master analysis (decompile miss); read from disassembly.

## Static reference visibility lifecycle (G333)

Static track references create hidden ornament levels and their Lods resource just as driving does. The selector must therefore install in shared application setup, not only when Driving exists. Registration now occurs once for all scenes; the existing resource guard safely skips scenes without Lods. Level distances, shadow levels and selection math are unchanged. An ECS lifecycle regression covers absent resources and real near/far camera changes. Initial548tests and post-main551tests, combined-feature Clippy/release and both pre/post-main three-run windowless GPU controls pass. Opposite-face static references now show the fence/stands with default LOD; an ordinary twelve-car600tick control retains all ordered player/input/handling/deformation records and the exact final PNG from G331. G331's earlier LOD0-isolated fence proof remains a separate experiment. Private evidence: local/graphics/fence-sidedness01/gpu05-default-lod01/. All-track/device/performance parity remains unverified.
