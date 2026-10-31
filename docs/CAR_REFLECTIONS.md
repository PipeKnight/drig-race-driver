# Car reflections: the original's environment map and the port plan

Backlog row 100 (owner playtest F657). Research of 31 October 2026, static analysis of the unpacked executable through the master Ghidra project (read-only). The pseudocode is private, under `local/re-cache/`.

Notation: [O] observed in code or data, [I] inferred, [?] open.

The owner's bar: reflections should match the scene geometry. Paint, metal and glass should reflect differently. Cracked or smashed glass should stop reflecting normally. A dented panel should reflect according to its deformed shape.

## 1. The original

### 1.1 Passes

`renderer_create_race_passes` (0x602400) creates two cube render targets, `dynamic_env_map` and `dynamic_env_map_far` [O]:

- **`env map`**: a pass object constructed by 0x5C1880, with vtable 0xCE0778. The renderer keeps it at +0xF20, and the global 0xE5E83C points to it.
- **`far map`**: the same class, stored in the near pass at +0x1940. The near pass also keeps the far cube at +0x19E0.

The cube format code is 0xC in both cases; it is not decoded here [?].

The generic range setter 0x5B51A0 (near, far clip, cull far) is called with:

| Map | Near (m) | Far clip (m) | Cull far (m) |
|---|---|---|---|
| near | 0.1 | 300 | 15 |
| far | 0.1 | 1000 | 600 |

`renderer_frame_render` (0x5FAD10) later overrides one cull distance with the track's `refmap_dist` (docs/TRACK_LOD.md 3.3). `refmap_obj_size` is the reflection passes' minimum object size, at +0x4C, which is copied into the visibility query at +0x150 [O]. That is the only object selection seen: there is no separate "reflection" object list.

[I] Two cube maps with these ranges mean:
- the near map holds the objects within 15 m of the centre, the other cars and nearby barriers;
- the far map holds the scenery out to 600 m;
- for the face states that read the far cube (2 and 3, in 0x5BD1C0 via 0x5B58F0), the far map is composited behind the near one.

The exact composition and the face-state values are not traced [?].

### 1.2 Settings

`system/hardware_settings_options.xml`, category `reflections` [O]:

| Option | faces | size | forceBilinear |
|---|---|---|---|
| low | 0 | 256 | true |
| medium | 6 | 256 | true |
| high | 6 | 256 | true |
| ultra | 6 | 512 | false |

`faces` is clamped to 0..6 and stored at +0x140; with 0 the pass is off [O, 0x602400].

`_TEnvironmentMap` (0x602FE0) binds the dynamic cube when faces > 0. Otherwise it binds the track's static environment map (scene +0x3F20) [O]. That map is the "low" fallback.

### 1.3 Update rate

The draw method 0x5BCA80 runs once per frame [O]:
- it increments a frame counter at +0x13C;
- it builds a 6-bit mask with the lowest `faces` bits cleared;
- it draws face `i` when bit `i` of `(mask << (counter·faces mod 6)) mod 63` is clear.

So it draws `faces` faces per frame, round robin [O]. With 6 (every shipped option above low) all six faces are redrawn every frame.

Face 3 (D3D order: −Y, looking down) gets no visibility query [O, 0x5BCC40 and 0x5BD0A0]. It is drawn, but with no objects [I]: only what the face is cleared to or copied from the far map.

The visibility frustum of a face is the pass's cull distance, ×1.5 on faces flagged at +0x208 when the pass flag +0x160 is set [O]. The 90° projection uses the pass's far clip [O, 0x5BCC40].

### 1.4 Centre

`renderer_frame_cameras` (0x5FBC10) centres the cube on the focused vehicle, the one the camera follows [O]:

- **Usually:** the vehicle position plus 0.5 m up.
- **When the renderer flag +0xF50 is set and the vehicle has locator 10:** that locator's world position plus 1.5 m up. The same flag also enables the ×1.5 cull on the near map. Which camera sets the flag is not traced [?]; [I] it is a cockpit or bumper camera.

There is one cube per frame [O]. All cars bind the same `_TEnvironmentMap` (0x602FE0). [I] So opponents reflect the scene around the focused car, with no parallax correction; none was seen.

### 1.5 Shaders

Constants are per material instance, saved in each car's PSSG (docs/LACETTI_GRAPHICS_MATERIALS.md, docs/CAR_SHADER_MASKS.md). Body paint, rims and the metal groups all use `carpaint_metallic`.

- **Paint** [O, Lacetti `Car Body Paint`]:
  - environment term `EnvironmentColour` ≈ 0.537;
  - Fresnel power 1, min 0.3, max 0.6;
  - specular 300 / 64.
  - The pixel program reads the cube, flips Z, and divides RGB by alpha (an RGBD-style encoding). Scratch alpha and the packed map's alpha mask the reflection.
  - The loader's fallback record (0x99C300) has Fresnel 2 / 0.1 / 0.9.
- **Glass** (`carglass`) [O, Lacetti window]: environment ≈ 0.486, Fresnel 0.3–0.7.
- **Cracked glass** uses the `_Cracked` variant with `TDiffuseCrackMap` [O].
- **Smashed glass**: the pane is removed and shards are spawned [O, port damage_parts.rs].
- **Dents**: [I] the deformed vertices and normals feed the same shaders. Nothing separate was seen.

## 2. The port before this work

- **Default playable:** reflects the source sky only (image-based lighting).
- **`scene_probe.rs`** (Codex), launcher "Play GRID Graphics Reflections":
  - six Bevy `Camera3d` views at 256, 512 or 1024 render the whole layer-0 scene: every LOD the main camera would pick, a 10 km far plane, foliage and opponents;
  - only the player car is excluded, through layer 1;
  - each frame the faces are copied into a cube that Bevy's `GeneratedEnvironmentMapLight` filters;
  - the light probe covers a 60 m box with parallax correction;
  - it is gated to enhanced graphics with source IBL at Donington with the Lacetti, or the validated BMW route.
- **`paint_environment`:** a paint layer with linear Fresnel, gain 0.5 approved for that launcher. It needs the `paint-specular` build feature.
- **Glass pilot (G224–G228):** opt-in.
- **Dents:** `body_damage::project` recomputes smooth normals after deformation.

### Cost of Codex's probe

Codex's probe cost could not be isolated (31 October 2026; RTX 4070, Vulkan, 3840x2160, 1800 ticks, private `local/reflections/runs/`):
- Bevy's per-pass GPU diagnostics are keyed by pass name, so the six probe cameras' passes collide with the main camera's, and `gpu_pass` shows one view's value.
- The 30-FPS-paced run reports the same frame count for off, 256 and 1024.

What is known from the code [O, bevy_light 0.19.1 `build_directional_light_cascades`; bevy_pbr `prepare_lights`]:
- Every active camera gets its own sun shadow cascades, so the probe adds six cascade sets.
- A view only gets the directional lights whose render layers intersect its own.

The port's probe therefore draws on its own render layer and is timed by its own GPU timestamps (section 3).

## 3. Plan

Approved by the coordinator on 31 October 2026; step 4 waits for the owner. One push and one log entry per step.

1. **Research** (this doc).
2. **Probe in the common build.**
   - One cube per frame centred like the original: the focused car plus 0.5 m.
   - Faces: all except −Y, which takes the sky.
   - Contents: a dedicated probe render layer with the track at the original ranges (near 0.1, far 300/1000 m), opponents and the sky.
   - Sun: a shadowless sun on the probe layer, so the probe has no cascades of its own.
   - Levels are a setting, like shadows:

     | Level | Size | Detail |
     |---|---|---|
     | off | – | sky only, as now |
     | original | 256 | as the original |
     | high | 512 | as the original |
     | ultra | 1024 | finer LOD |

   - Parallax correction only if it clearly helps.
   - The phone stays at 256 unless the measured cost forces 128; report the number first.
   - Off by default until the owner has seen crops.
3. **Per-material response.**
   - Read the saved `EnvironmentColour` / `FresnelValues` of each car material instance, for every car.
   - Paint, rims and metal (`carpaint_metallic`) and glass (`carglass`) each take their own constants. This replaces the authored linear paint layer in the common path.
   - Cracked glass takes the crack map's response; smashed panes stay removed; dents use the recomputed normals.
   - Image-compare every surface that does not reflect.
4. **Owner crops** at 1080p and 4K, levels side by side: paint, glass intact and cracked, a dented panel. The owner then picks the desktop default.
5. **Later:** wet asphalt, mirrors.

## 4. The port (F670)

Common build. Off unless `--car-reflections original|high|ultra` or `GRID_CAR_REFLECTIONS` names a level; step 4 (the owner's comparison) decides the default.

### 4.1 The cube (`car_reflections.rs`)

- **Cameras:** six Bevy cameras, one per face. Each is HDR (Rgba16Float), unshaded by tonemapping, has no MSAA, and uses the main camera's exposure, sky lighting and fog.
  - Placement: 0.5 m above the player's car origin, as in 0x5FBC10. In a replay, the shown car.
  - Range: near 0.1 m, far 1000 m (the far map).
  - Size by level: 256 (original), 512 (high), 1024 (ultra).
- **Copy:** after each frame, the faces are copied into one cube texture. The main view samples it the next frame.
- **Layer 27:** the cameras draw only render layer 27.
  - Every mesh on the default layer is also put on layer 27, except the player car's parts and batches, and the overlays.
  - A mesh that leaves the default layer leaves layer 27 too: a batched part, an opponent's hidden detail level.
- **Lighting:** a copy of the brightest sun, without shadow maps, lights layer 27. The main sun is not on that layer, so the probe has no shadow cascades: Bevy gives a view the cascades of every shadowing sun its layers see. Point and spot lights stay out of the probe.
- **Detail:** the probe draws the main camera's choice of track and car levels; finer levels for ultra are not separate yet.
- **Dump:** `GRID_CAR_REFLECTIONS_DUMP=N` writes the six faces as one PNG after N copies, to `<data dir>/graphics/`, for checking orientation and contents.
- **Pass timing:** `gpu_passes` sums every camera's pass of one name (it read only the newest measurement before), so the probe's cost shows in the `gpu_pass` lines.

### 4.2 Material response (`car_reflection_overlay.rs/.wgsl`)

`grid_formats::material::read_constants` reads every car's saved shader constants.

Survey of all 44 `*_highLOD.pssg` files (private `local/reflections/constants.json`):

| Shader group | Instances | Materials | EnvironmentColour | Fresnel (min / max / power) |
|---|---|---|---|---|
| `carpaint_metallic` | 168 | paint, wheel, bare and shiny metal | 0.24–1 | e.g. 0.3 / 0.6 / 1 (Lacetti paint), 0.1 / 0.7 / 4 |
| `carglass` | 42 | windows, grill glass | 0.486 / 0.490 | 0.3 / 0.7 / 1, 0.3 / 0.8 / 4 |
| `car_light_glass` | 83 | lamp glass | 0.459 | 0.3 / 0.6 / 1 |
| `car_light_pod` | 175 | lamp pods, discs, grills | 0.62–1 | — |
| `car` | 227 | matt, cabin, tyres, suspension | mostly 0: no reflection | — |

**Every material reflects with its own saved values. Nothing is authored per car.**

**Fresnel**, from the original paint program's arithmetic [O], with c = n·v:
- `t = 1 / (1 + FresnelPower^(20·c − 10))`;
- `weight = mix(FresnelMin, FresnelMax, t)`. Power 1 gives the midpoint at every angle.

**Mix:** the program mixes the lit colour toward the environment colour times EnvironmentColour. [?] How the CPU packs the saved Power/Min/Max into the shader's vector is not traced (docs/LACETTI_GRAPHICS_MATERIALS.md).

**The overlay.** The port draws that mix as a child entity of each reflecting part or batch:
- **Geometry:** the same mesh, in the transparent pass, with depth compare GreaterEqual and no depth write.
- **Blend:** colour = environment, alpha = weight, i.e. `lit·(1 − w) + env·w`.
- **Fog and tonemapping:** applied as for the part.
- **What it leaves alone:** the part's own material, and the systems that swap it (batching, damage, lamps).
- **What it follows:** the part's mesh (deformation recomputes the normals, so dents bend the reflection), its layers and its visibility (a smashed pane is hidden).
- **Opponents:** their livery copies carry their part's controls.

**AUTHORED:**
- Glass weighs the reflection by the pane's alpha.
- A cracked pane (damage_parts' crack-map copy): its facets turn by up to ±0.17 in a 48×48 grid of the pane's texture, and it reflects 60% less where the crack map is opaque.
- The paint program's other gates are not applied yet: the packed-map environment mask, shadow, dust, scratch and vertex masks, and the hemisphere term. So reflections are likely stronger than the original's on shadowed and masked areas.

### 4.3 Object selection and cost (31 October 2026)

**Object selection.** The probe draws a mesh only when it passes the original's reflection object test, seen from the cube's centre (docs/TRACK_LOD.md 3.3): bounding radius / distance ≥ the minimum size, and within the far distance. Meshes are rechosen when the centre has moved 2 m. The route's `route_overrides.xml` `default` gives `refmap_obj_size` and `refmap_dist` (engine defaults 0.2 and 2500, clamped to 500).

| Level | Minimum size | Far distance |
|---|---|---|
| original | route value (Donington 0.5) | route value |
| high | a quarter of the route value | route value |
| ultra | 0.004 (`main_obj_size`, what the main view draws) | 1000 m |

**Setup.** RTX 4070 and i7-12700, Vulkan. Lacetti at Donington, 3600 ticks, 30 FPS-paced offscreen runs. Private `local/reflections/runs/cost2-*` and `cost3-*`. Means per frame after the first 10 s window. The PC was shared with other sessions' builds, so CPU times vary by up to 2× between batches (the enhanced "off" render schedule measured 12.5 and 26 ms).

**Baseline 1920x1080**, one batch (cost3):

| Level | GPU, all passes | Main schedule CPU | Render schedule CPU |
|---|---|---|---|
| off | 2.94 ms | 4.0 ms | 7.4 ms |
| original | +0.0 (noise) | +0.7 ms | +9.2 ms |
| high | +1.0 ms | +2.2 ms | +15.9 ms |
| ultra | +1.9 ms | +2.8 ms | +17.7 ms |

**Enhanced 3840x2160:** GPU +0.5 to +1.3 ms. The CPU is not separable from the load.

**GPU cost.**
- About 0.4–0.6 ms is per view regardless of contents: clustering and bin unpacking for six views.
- Probe passes plus overlays add 0.2–1.2 ms.

**Per-view CPU fixes.** The CPU cost per face view was the main finding:
- Bevy builds every sun's cascades for every active camera and finds their casters on the CPU. Dropping the faces' cascades (`drop_cascades`) cut the render schedule from +28 to about +15 ms (cost → cost2).
- `ClusterConfig::None` on the faces fails Bevy's GPU clustering (an invalid dummy texture, validation errors every frame), so the faces use `ClusterConfig::Single`.
- Even with only Donington's large objects drawn, each face costs about 1.5 ms of render-schedule CPU.

**Open items:**
- That fixed per-view cost in Bevy needs a profile (`span_profile`) before any default.
- Drawing fewer faces per frame would cut it, as the original's `faces` setting allows (round robin, 0x5BCA80). That trades reflection update rate, which is the owner's decision.
- The phone has not been measured.

**Look.** Private `local/reflections/runs/s2-*`, 1080p enhanced, Lacetti against the Donington pit wall:
- The paint and windows reflect the wall, the barrier and the sky with the scene's geometry.
- The dented rear bumper shows its deformation in the reflection; with reflections off it is hardly visible.
- The paint reads strongly mirror-like, likely stronger than the original's, because the original's gates (4.2) are missing.
- The cube faces (dump) have the sky up and the ground down, and match the scene.
- Not checked yet: cracked and smashed glass in a frame, opponents' cars, the cockpit mirror, replays, the phone.
