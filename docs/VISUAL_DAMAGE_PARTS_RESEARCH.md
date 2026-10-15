# Visual damage: loose parts, broken glass and smashed lights

15 October 2026 (F307). Static research on how the purchased Lacetti package describes detachable
parts, glass and lights, and how the original game drives them, plus the presentation-only port
in `crates/grid-app/src/damage_parts.rs`.

Markers:

- **[V]** verified against the purchased files or by a run;
- **[D]** read from decompiled or disassembled original code (static, not executed);
- **[I]** interpretation or inference.

Evidence (private, untracked): `local/visual-damage/` in the main checkout. It holds:

- PSSG/NED dumps (`lac_highLOD.tree.txt`, `lac-ned.txt`);
- Ghidra 12.1.4 output under `ghidra/` (`loader`, `consumers`, `producers`, `callback`, `callers`,
  `glassinit3`, `cracked`, `disp-*.tsv`, `refs-window`);
- run logs and renders.

Analysis image: `local/compatibility/grid-unpacked.exe`, SHA-256 `aac46a64…4514` (hash-checked by the
runner). The shared Ghidra project was locked by another session part of the time, so later probes
ran on a private read-only copy of it.

## 1. Car package

### Model: one rigid joint per part [V]

`cars/lac/lac_highLOD.pssg` is a matrix-palette model. Bundle `LOD0_` has 67 joints (`x0_*`), each
with rigid draws (every vertex skinned to its joint). The damage-relevant joints are:

- body panels: `bonnet`, `boot`, `door_fl`, `door_fr`, `wing_fl`, `wing_fr`, `spoiler_b`;
- bumpers: `bumper_fl`, `bumper_fm`, `bumper_fr`, `bumper_bl`, `bumper_bm`, `bumper_br`;
- small parts: `wing_mirror_l`, `wing_mirror_r`, `exhaust_bl`, `wiper_fl`, `wiper_fr`, `aerial_fl`,
  `under_f`, `under_b`;
- glass: `window_f`, `window_fl`, `window_fr`, `window_b`, `window_bl`, `window_br`;
- lights: `light_pod_fl/fr`, `light_glass_fl/fr`, `light_pod_bl/br`, `light_brake_pod_bl/br`,
  `light_brake_glass_bl/br`, `light_rev_pod_bl/br`.

There are no separate damaged meshes and no hinge or attachment nodes in the PSSG. The Lacetti has no
rear doors, rear wings, canopies or `exhaust`/`exhaust_b`/`exhaust_br`.

`lac_anim_dummies.pssg` only holds driver/cockpit dummies (steering, pedals, hands).

### Damage textures [V presence, I use]

- `car_light_glass.fx` and `carglass.fx` declare a `TDiffuseCrackMap` sampler.
- The shader pack's glass pixel shaders also declare a `_Cracked` float, and the windscreen shaders a
  `crackAndScale` vector.
- The package ships `lac_glass_dm.tga` (livery packages, 1024×512) and `lac_lights_dm.tga`
  (`lac_gen_high.pssg`). These are the crack ("damage map") textures [I].
- The scratch map `lac_main_sc.tga` belongs to the paint shader's `TScratchMap`. It is driven by the
  FEMS `m_scratchMap*` values and is not handled here.

### `lac.ppts`: window shatter points [V format, I purpose]

The file layout:

- a little-endian u32 count;
- 40-byte entries: a 32-byte name, a u32 point count and a u32 offset;
- the points: six f32 each, a model-space position and an outward unit normal.

The Lacetti has 8 entries with 12–30 points each:

- `x0_window_f_0`, `x0_window_fl_0`, `x0_window_b_0`;
- `x0_window_bl_0` and `_1`;
- `x0_window_br_0` and `_1`;
- `x0_window_fr_0`.

The points lie on the window glass. They are read as glass-particle emitters [I].

### `lac.ned` DAMAGE sections [V]

The loader is `0x9EFB30` [D]. It zero-fills the 0x2720-byte description, then stores each present
attribute at a fixed offset. Flags are "value > 0.5".

**PARTS.** Lacetti values, with the description offset of each level:

| Part group | Swing level (offset) | Detach level (offset) | Flags |
| --- | --- | --- | --- |
| bonnet | 3.0 (+1C40) | 3.5 (+1C38) | swings 1, detaches 1 |
| boot | 2.5 (+1C58) | 3.0 (+1C5C) | swings 1 |
| front doors | 5.0 (+1C68) | 12 (+1C6C) | swing 1 |
| rear doors | 4.0 | 6.0 | swing 0 |
| front wings | 4.0 (+1C48) | 4.2 (+1C4C) | swing 1, detach 1 |
| rear wings | 3.0 | 5.0 | both 0 |
| wing mirrors | 3.0 (+1C80) | 5.0 (+1C84) | swing 1, detach 1 |
| front bumper sides | 2.0 (+1C8C) | 2.5 (+1C94) | swing 1, detach 1 |
| rear bumper sides | 2.5 (+1C90) | 2.5 (+1C98) | swing 1, detach 1 |
| exhaust | 5.0 (+1C78) | 6.0 (+1C7C) | swing 1, detach 1 |
| spoiler | — | 3.0 (+1C88) | detaches 1 |

Other PARTS values:

- whole bumpers: front detach 2.8 (+1C9C), rear detach 2.5 (+1CA0);
- `m_wheel_detach_level` 6 (+1C3C; the loader first seeds it with 2 × bonnet detach level);
- `m_overall_speed_required_scalar` 4 (+1C44, default 4);
- `m_boot_max_angle` 40, which the loader clamps to 5 (+1C60) [D];
- `m_smashLightCluster` 0.1 (+1C00, default 0.1);
- `m_disableLooseParts` 0 (+1C24);
- `m_wing_mirror_move_with_door` 1;
- `m_heirarchy_spoiler` 1.

**WINDOW.**

- `crackCosTheta` 0.999903 (+1BF8; about 0.8°), default cos 1°.
- `smashCosTheta` 0.999391 (+1BFC; 2°), default cos 2°.
- Per-window flags `frontWindow` … `rearWindow` are all 0. The window setup copies the description
  words +1C0C…+1C20 into each window record; non-zero means crack only [D]. Which name feeds which word
  is [I]; all are zero for the Lacetti.

**SIDE.** `strong_point_front` 0.384609 (+1BEC) and `strong_point_rear` −1.21562 (+1BF0).

The car body holds this description at car +D240. Car +EE2C/+EE30 are therefore these strong points,
car +ED70 is the description's `m_cs_left_x` and car +ED94 its `m_cs_roof_y`
([CAR_COLLISION.md](CAR_COLLISION.md)). Both are replaced by the collision mesh maxima: X 0.913279,
Y 1.420987 [V via the existing reader].

## 2. How the original drives parts

### Parts object and its event feed [D]

Constructor `0x9D7860` builds the parts object:

- 40 part records of 0x2E0 bytes at +8F0. The part id is at record +24C and the state at +58.
- 17 per-tick zone accumulators at +7F0…+830, with running totals at +834…+874.
- The description pointer at +878 (= car + D240) and the car at +884.

It then registers callback `0x9A7670` with itself as user in the car's notification slots
+37098/+370C4. These are the same eleven slots the ported body handler fills
([BODY_IMPACT_HANDLER.md](BODY_IMPACT_HANDLER.md)).

Callback `0x9A7670`, from aligned instructions [D]:

- It returns unless car bytes +B13 and +B14 are both non-zero.
- Otherwise it calls `0x98B430` with `this` = user, point = argument 4, normal = argument 5 and
  impact = argument 7. That is the notification packet's point, side-signed normal and solved
  contact +3C impact.
- `0x98B430` ends with `ret 0x10`. The timestep argument 8 is passed but unused.

### Zone classifier `0x98B430` [D]

The contact point and normal go into the car frame:

- local point = pose⁻¹·point − car +1630; local normal = pose⁻¹·normal. The pose is the physical
  body pose.
- The *roof* zone gains −nᵧ·m only when all three hold:
  - local y > `m_cs_roof_y` − 0.4;
  - nᵧ < −0.7071;
  - the pose's row 1 Y is below zero. This is component 1 of row 1 (`0x4B1A70`), i.e. the car is
    inverted.
- Lateral cell:
  - left (+X) if x > 0.5 × half-width;
  - right if x < −0.5 × half-width;
  - otherwise centre.
- Longitudinal cell: front if z > `strong_point_front`, rear if z < `strong_point_rear`, otherwise
  middle.

Accumulation per cell (m = impact; wheels in RL, RR, FL, FR order):

| Cell | Zones |
| --- | --- |
| rear, all | rear bumper, boot, exhaust += m |
| rear right | + rear-right corner, rear-right wing; RL += 0.2m, RR += m |
| rear centre | RL, RR += m |
| rear left | + rear-left wing, rear-left corner; RL += m, RR += 0.2m |
| middle right | only if nₓ > 0: right side += abs(nₓ·m); RR, FR += 0.5m |
| middle left | left side += abs(nₓ·m) if nₓ < 0; RL, FL += 0.5m always |
| middle centre | nothing |
| front right | front-right corner += m; FL += 0.2m, FR += m; bonnet += −n_z·m if n_z < 0 |
| front centre | bonnet += −n_z·m if n_z < 0; FL, FR += m |
| front left | front-left corner += m; FL += m, FR += 0.2m; bonnet as above |

### Decisions `0x9EC9B0` [D]

**Per tick.**

1. Totals += zones.
2. If `m_disableLooseParts` is set, clear the zones and stop.
3. Compute S = `FUN_00a69370()` × scalar. `0xA69370` reads car+DB0 → +8 → +3F0, the physical mass
   (1140 for the Lacetti), so S = mass × 4.
4. Apply the rules below, then clear the zones.

Part ids come from the name table at `0xE4EF20`: 10 bonnet, 11 boot, 12–17 bumper bl/br/bm/fl/fr/fm,
22–25 doors bl/br/fl/fr, 31/34/101/102 exhausts, 84–87 wings bl/br/fl/fr, 88/89 mirrors l/r, 96–100
canopies, 78–83 windows, 107–166 lights.

The setter is `0x9DC270`. State 2 = loose (swinging), 4 = detached, 0 = attached.

| Part | Rule |
| --- | --- |
| bonnet (10) | loose when bonnet zone > swing × S and `m_bonnet_swings` |
| boot (11) | loose when boot zone > swing × S and boot total > 2.5 × swing × S |
| exhausts | detach above detach × S; else loose above swing × S, also requiring car +1554 = 0 |
| front doors (24/25) | loose when level × S < scalar × side zone (i.e. zone > level × mass), or when a roof zone above 0.25 × S occurs with `m_front_doors_swing` |
| rear doors (22/23) | loose when side zone > level × S |
| wings (84–87) | detach when the corner zone > detach × S; else loose when the zone > swing level × mass |
| mirrors (88/89) | detach when the front corner zone > detach × S |
| whole front bumper | the two front corner zones together > front bumper detach × S (and `bumper_fm` exists) detaches 15, 16 and 17 |
| front bumper corners | while `bumper_fm` is attached: detach above side detach × S, else loose above side swing level × mass |
| whole rear bumper | rear zone > rear bumper detach × S detaches 12, 13 and 14; else above 0.6 × that, `bumper_bm` goes loose while a rear corner is still attached |
| rear bumper corners | as the front corners, gated on `bumper_bm` |
| wheels | zone > wheel detach × S sets the car flags +22E4…+22E7 |
| canopies | ×2.4 total rules (no Lacetti nodes) |

Only the loader reads the bonnet detach level and flag (displacement search over the whole program):
the bonnet never detaches through this path [D]. The spoiler detach level (+1C88) and the mirror
swing level (+1C80) are read by `0x9AE8F0`, `0x9E2AD0` and `0x9AEC60`, which were not analysed.

### Loose and detached motion [D partial, I]

`0x9DC270` handles the state change:

- It stores per-LOD joint matrices.
- Detached parts get their own record with a physical body pointer at record +64, so they become
  separate simulated objects [I].
- Setting state 2 on a detached part re-attaches it and swings it again. 9EC9B0 normally avoids this
  with explicit "not detached" checks.

`0x9DB0E0` builds one hinge per part through `0x9AA960(part, body, frame, axis, a5, lo, hi, …)`.
The argument meanings are [I]:

- doors: ±0.4;
- bonnet: (−0.5, 0) or (0, 0.5) by orientation;
- wings: ±0.05;
- boot: −`m_boot_max_angle` × π/180, i.e. −5° after the clamp;
- bumper corners: 0.12/0.14 with a5 = π/2;
- `bumper_bm`: axis (0, 0.707, 0.707), ±0.4;
- exhausts: 0.66 × `m_exhaust_swing_multiple`;
- mirrors: `0x9AA1D0(…, 0.4)`.

The hinge solver, debris physics and per-part masses were not ported.

### Windows `0x9A4A20` / `0x9D12B0` [D]

`0x9A4A20` creates one record (stride 0x90) per present window node, ids 0x4E…0x53. It builds six
anchors from the node's bounding box:

- minimum, maximum and centre;
- mixed corners chosen by the longer of the X/Z extents.

The exact mixing via `0x4B1B60` is [I]. Record +80 is the state, +84 a changed flag, +88 the
crack-only word, and +8C/+94 the smash delay timer and duration (initialised to 0).

`0x9D12B0` runs while deformation is active (car +A4AD):

1. Sample the six anchors through the deformation grid (`0x9A39D0`).
2. Build four triangles from anchors (0,1,2), (0,1,5), (4,3,2) and (4,3,5), at rest and deformed
   (`0x8B5710`).
3. Compare each rest normal with its deformed normal by abs(cos).

| Condition | New state |
| --- | --- |
| ≥ crack cos | unchanged (intact stays intact) |
| ≥ smash cos, or crack-only | intact → 1 (cracked) |
| otherwise | intact or cracked → 3 (smashed) |

States only rise. The window's state is the maximum over its triangles. A non-zero duration would
hold state 2 for that long before 3.

### Light clusters `0x9A5EC0` / `0x9D1A90` [D]

`0x9A5EC0` builds 12 light records:

- six front records, one per (pod, glass) pair of `light_pod/glass_fl/fr`, `light_2_*`, `light_3_*`;
- six rear records of eight nodes each: plain, brake, reverse and combi pod and glass, per side and
  variant.

Each record's anchor is the mean of its present nodes' centres (`0x9A5DF0`).

`0x9D1A90` samples each anchor through the deformation grid. A record is smashed (state 2) when the
anchor moved more than `m_smashLightCluster`, otherwise intact (1). `0x9D1820` then derives per-light
beam intensities.

For the Lacetti, four clusters are populated: FL, FR, BL, BR.

### Shader input [D lookup, I values]

`0x9FE020` resolves the `_Cracked`, `_DustAmount` and `DustColour` shader parameters for glass draws.
How states 1/2/3 map to `_Cracked` values was not traced.

## 3. Port: `crates/grid-app/src/damage_parts.rs`

Inputs, all read-only:

- The new `grid_tools::body_impacts::collect` extracts the same point, side-signed normal and +3C
  impact from the last tick's solved contact records.
- `Runtime::impacts()` covers the solo world. It uses the new `last_active_contacts`, which comes
  from the new `vehicle_world::Output::active_contacts`.
- `MultiRuntime::impacts(i)` covers the shared world, one call per car.
- Pose, physical mass, car +1630 and bytes +B13/+B14 come from the car's state.
- The deformation field is `grid_tools::body_deformation::Field`.

No physics or deformation result changes.

Recovered and ported:

- the NED reader with loader defaults and the boot-angle clamp;
- the zone classifier;
- all 9EC9B0 rules for Lacetti-relevant parts (wheels and canopies excluded);
- the window rule;
- the light rule;
- the light record layout;
- the window crack-only words.

They run per tick for the player and every opponent.

AUTHORED, labelled as such in the code:

- Hinge pivots on part bound edges, with the 9DB0E0 limits as angle ranges.
- A damped spring swing driven by the car's acceleration.
- Detached-part flight: launched outward and up, gravity, bounce on the detach-time ground height,
  then the part stays.
- Window anchors: four corners of the glass box plus its centre. These stand in for the six-anchor
  layout.
- Smashed windows are hidden and spawn short-lived shards from the `lac.ppts` points.
- Cracked windows and smashed light glass swap their diffuse texture to the `_dm` crack map.
- Side windows and mirrors ride on a loose front door.

Log lines:

- `drive_damage_parts` (configuration);
- `drive_damage_part car= tick= part= state= zone= threshold=`;
- `drive_damage_glass`;
- `drive_damage_light`;
- with `GRID_DAMAGE_ZONES=1`, `drive_damage_zones` per tick.

## 4. Validation [V]

Renders and logs are private, under `local/visual-damage/` in the main checkout. The `GRID_DAMAGE_CAMERA=car,x,y,z` diagnostic places the render camera at a car-local point.

- **Wheel contacts never feed the zones.** In the solo collision run (throttle 1, steering 0.25), all zones stay zero until the first barrier contact at tick 115.
  - Tick 116: front-left zone 8 995 against a wing swing threshold of 4 560 (4 × 1140) and a corner swing threshold of 2 280. Both parts go loose; nothing detaches.
- **Shallow crash** (steering 0.12, 1500 ticks):
  - corner loose 157, wing loose 438;
  - front light clusters smashed 656/711 (displacement 0.102/0.168 m);
  - windscreen cracked 713, then smashed 714;
  - bonnet loose 1090 (23 186 > 13 680), right corner 1091;
  - both side windows smashed and both rear clusters smashed at 1092.
  - Renders `w-s012-760.png` (windscreen gone, interior visible) and `w-s012-1100.png` (bonnet open, side glass gone, shards) checked.
- **12-car autopilot race** (`GRID_AUTOPILOT=1 --opponents 11 --time-trial-laps 1`, 3000 ticks):
  - opponent parts go loose and detach;
  - car 1 loses its front-right corner (2352) and its whole rear bumper (2557, 14 853 > 11 400);
  - car 6's bonnet goes loose (2557);
  - several cars crack or smash glass and lights.
  - The same events recur at the same ticks on replay.
  - `w-race-car1.png` checked: car 6's bonnet is flipped up, shards are in flight, and car 1's rear bumper is falling away.
- Glass and light states are evaluated once per rendered frame, when the car's deformation field changes. Their reported tick therefore follows frame pacing: a repeat run of the 1100-tick crash after rebasing logged the windscreen smash at 715 instead of 718. The part decisions run per simulation tick and were identical.
- These are checks of the port's behaviour only. Magnitudes and appearance are not compared with the original.

## 5. Open questions

- Whether +3C carries the same magnitude in the port and the original during heavy impacts. The
  thresholds are mass × level × 4 per tick, compared with summed solved +3C values, and need a live
  comparison.
- The 9AA960 hinge semantics and the original loose-part dynamics. The detached-part physics, its
  lifetime and its collision.
- `_Cracked` values; light beam/emissive changes (9D1820); spoiler and mirror-swing paths (9AE8F0,
  9E2AD0, 9AEC60); wiper detachment with the windscreen; light pods attached to bumpers
  (`m_attach_light_pod_*`, zero for the Lacetti).
- The car +1554 exhaust swing gate, read here as zero.
