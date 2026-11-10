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

Composition, traced statically in F801 [O structure, I meaning]: `env_map_face_buckets` (0x5BD1C0) reads the state of the current face (`this+0x8c`) at `this+0x200+face*0x3E0+0x3D4`.
- State 4 skips the face.
- States 2 and 3, when a far cube is linked at `+0x19E0`, call 0x5B58F0 (far cube, face, matrix at `+0x1960`).
- States 0 and 1 add the near objects' bucket (`param_2+0xC`).
- With `+0x162` set, a far cube linked, or face 5, it also adds the buckets at `+0x28`, `+0x30`, `+0x60` and `+0x5C`, and `+0x2C` when `+0x161` is set.

0x5B58F0 multiplies the face's view basis by the matrix at `+0x1960` and passes the result to 0xB9DC30 with `+0x194C`. The basis comes from 0x5B5630, a switch over the six faces: face 0 has right −Z, up +Y and forward +X, the D3D cube face order. It then makes one begin/bind/draw/end sequence through the device object at `+0x1A18` (vtable +0xC, +0x10, +0x20, +0x24, +0x28, +0x2C) on `+0x1950`, `+0x1944`, `+0x1958` and `+0x1954`, with the call (4,4,3) on `+0x1950` in between. [I] It draws the far cube's face as a background into the near face, and the near objects are drawn over it. Not traced: what 0xB9DC30 is (a shader constant set is likely), which states 0–3 the shipped settings reach, and when the far cube is linked.

The port renders one cube with the far map's ranges (or `refmap_dist`). It has no separate near pass and no background composite. The difference this makes on screen is not measured.

### 1.2 Settings

`system/hardware_settings_options.xml`, category `reflections` [O]:

| Option | faces | size | forceBilinear |
|---|---|---|---|
| low | 0 | 256 | true |
| medium | 6 | 256 | true |
| high | 6 | 256 | true |
| ultra | 6 | 512 | false |

`faces` is clamped to 0..6 and stored at +0x140; with 0 the pass is off [O, 0x602400].

`_TEnvironmentMap` (0x602FE0) binds the dynamic cube when faces > 0. Otherwise it binds the track's static environment map (track manager +0x3F20) [O]. That map is the "low" fallback; `preRegisterRaceUpdateables` (0x6030F0) and the static pass 0x5EABE0 make the same choice [O]. Section 4.4 has how the static map is made.

The shipped default, `system/hardware_settings_config.xml`, is `<reflections faces="6" size="256" forceBilinear="true" />` [O]: the dynamic cube at 256, the port's `original` level.

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
  - At run time the material tuning (1.6) replaces these saved controls: tuned paint has Fresnel 2.13 / 0.27 / 0.86.
- **Glass** (`carglass`) [O, Lacetti window]: environment ≈ 0.486, Fresnel 0.3–0.7.
- **Cracked glass** uses the `_Cracked` variant with `TDiffuseCrackMap` [O].
- **Smashed glass**: the pane is removed and shards are spawned [O, port damage_parts.rs].
- **Dents**: [I] the deformed vertices and normals feed the same shaders. Nothing separate was seen.

### 1.6 Material tuning

The saved controls of 1.5 are not the ones the original draws with [O, static: 0x99C300, 0x9E7360, 0xA135C0, 0xA10310]:

- **Source.** `raceload.jpk#materials.xml`, binary XML, root `materials`. A decoded private copy is `local/binxml/captured-dependencies/decoded-001.xml`. The vehicle manager's XML dispatch (0x9CA3A0) and a reload of `cars/materials.xml` (0x9CBCE0) both call the loader, 0x99C300.
- **The loader, 0x99C300.** It checks the root's name, then walks the 35 names of the table at 0xE4FD58 (`Car Body Paint`, `Matt`, `Head Lights Pod` … `shiny clipping shader`, with spaces written as `_`). For each name it:
  - presets the record [300, 64, 10, 4, 0.2, 1, 2, 0.1, 0.9];
  - reads the `param` children of the root's elements of that name, with the attributes `specularHighlightPower`, `specularHighlightStrength`, `metallicSpecularPower`, `metallicSpecularScale`, `metallicFalloff`, `occlusionMapScale`, `fresnelPower`, `fresnelMin` and `fresnelMax`;
  - stores the record at vehicle manager + 0x6C + i·0x24.

  At the end it sets the reload flag (+0x558).
- **The submission, 0x9E7360.** It is called every frame at the end of the vehicle manager's update (0xA10310) and runs when the debug editor byte (+0x2C) or the reload flag is set. Then, for each material of the 20 cars, it searches the table in order for the first name that matches the start of the material's name, case-insensitive (`__strnicmp` with the table name's length, 0xA66780). A tuned material gets these values through 0xB9DD10 (set a named vector):
  - `SpecularValues` = (highlight power, highlight strength, vehicle manager +4, +0);
  - `MetallicSpecularValues` = (metallic power, scale, falloff, occlusion scale);
  - `FresnelValues` = (Fresnel power, min, max, 0).

  Then it clears the flag.
- **Floors.** The vehicle manager's constructor (0xA135C0) sets +0 to 0.5 and +4 to 0.25, and also sets the reload flag. Only the debug editor changes +0/+4. So a tuned material draws with `SpecularValues.z` = 0.25 (the occlusion floor) and `.w` = 0.5 (the shadow floor).

**Values** [O, the installed file]:


| Material | Fresnel power / min / max | Occlusion scale |
|---|---|---|
| Car Body Paint | 2.13 / 0.27 / 0.86 | 1.05 |
| Window Glass | 4.35 / 0.34 / 0.81 | 0.76 |
| Head / Rear Lights Glass | 4.07 / 0.20 / 0.84, 4.07 / 0.41 / 0.80 | |
| Head / Rear Lights Pod | 6.20 / 0.24 / 0.65, 4.57 / 0.45 / 0.67 | 0.46 (head) |
| Shiny Metal, Bare Metal, Chrome | 4.16 / 0.65 / 0.81, 3.95 / 0.12 / 0.18, 4.07 / 0.81 / 0.80 | 1.09 (chrome) |
| Wheel | 3.24 / 0.29 / 0.40 | |
| Suspension, Cabin, Grills, Tyre Wall | min = max = 0 (no reflection) | |
| Grills Glass, matt clipping shader | never used: `Grills` and `Matt` come first and match the start of the name, so these materials take those records | |
| shiny clipping shader | the loader's fallback, 2 / 0.1 / 0.9 | |

The pssg paint (power 1, 0.3 / 0.6) has a flat weight of 0.45 at every angle. Tuned paint goes from 0.27 facing the camera to 0.86 at grazing angles.

[I] The cars are loaded when the flag is consumed: the update does not tick during the race load.

### 1.7 Sun terms (lighting)

The tuning record's specular and metallic fields light the paint and window glass programs [O, static: `car_basic.cg.fp_paintDustSpecmapOcclusionReflectiveNormalScratches` and `car_basic.cg.fp_glass_windows` in `shaderpack/shaderpack.pssg`, disassembled with `tools/disassemble_packed_grid_shader.py`, private `local/shaders/`]. The registers are c22 `sunDirection` (toward the sun), c23 `SpecularValues` and c26 `MetallicSpecularValues`. With s = max(`GhostCarValues.w`, the shadow mask's red), r the reflected view vector and m1 the packed map:

- **Paint**, after the reflection mix C1 of 4.2:
  - a metal mask m = sat((m1.a − 0.3)·8) darkens the diffuse colour toward grazing angles: albedo ×= 1 + m·(sat(n·v + metallic falloff) − 1);
  - the metallic tint M = m·sun·(diffuse texture·metallic scale + 0.5) replaces the colour by k = pow(sat(sun·r), metallic power)·s·m1.a·dust gate: C2 = C1 + k·(M − C1);
  - the highlight is added: C2 + sun·s·highlight strength·m1.a·dust gate·pow(sat(sun·r), highlight power);
  - then fog.
- **Window glass**:
  - the reflection has the shadow gate env ×= sat(s + `SpecularValues.w`), with s = 1 on back faces (a `vFace` compare);
  - its occlusion term is sat(1 + `SpecularValues.z`) with no occlusion map, so it is 1 at every floor ≥ 0;
  - the highlight is sun·s·highlight strength·0.5·dust gate·pow(sat(sun·r), highlight power);
  - the output alpha rises with the reflection and the highlight by `GhostCarValues.z` (1.8).

**Values** [O, the installed file] (highlight power / strength; metallic power / scale / falloff):

| Material | Highlight | Metallic |
|---|---|---|
| Car Body Paint | 35.65 / 1.16 | 14.16 / 1.20 / 0.25 |
| Window Glass | 509.16 / 1.20 | 1.00 / 0.00 / 0.00 |
| Chrome | 2.70 / 63.58 | 10.16 / 1.48 / 0.09 |
| Shiny Metal | 613.10 / 196.53 | 12.45 / 1.48 / 0.08 |
| Bare Metal | 688.17 / 167.63 | 12.45 / 1.66 / 0.09 |
| Wheel | 610.78 / 38.96 | 7.43 / 1.66 / 0.06 |

An untuned material keeps its saved `SpecularValues` (Lacetti paint 300 / 64, 1.5). [I] The original draws into an 8-bit target, so a sum above 1 clips to white.

### 1.8 Dust, scratches, vertex damage and GhostCarValues

[O, static only: the vertex programs that declare `dirtLevel` (three paint variants: basic, part state and wobble), the pixel programs of 1.7, and the decompiled functions named below; no grid.exe runs.]

- **Vertex controls.** Every paint vertex program writes o7 = (1 − vertex red, 1 − vertex green, sat(dirtLevel), 0). x is the normal-map strength and the damage gate, y the scratch amount, z the dust amount. The saved colours are in docs/CAR_SHADER_MASKS.md: on Lacetti LOD0 paint, 10074 vertices are white (x = y = 0) and 179 are red 0 / green 255 (x = 1, y = 0). No paint vertex has green below 255, so the scratch gate is 1 on a clean car.
- **Paint pixel program.** With d = m1.r·dirtLevel and the dust gate dg = sat(1 − 2d):
  - the reflection weight is sat(2·m1.a)·Fresnel·dg·(1 − scratch.a·y)·sat(1.1 − x);
  - the highlight and the metallic factor k use m1.a·dg;
  - the albedo moves toward `DustColour` by d;
  - the output alpha is the diffuse alpha·`GhostCarValues.x`, and the shadow s = max(`GhostCarValues.w`, mask red).
  - So a red-free vertex (panel gaps, grilles, trim on Lacetti) reflects at 0.1 of the weight even on a clean car.
- **Window glass pixel program.** s = 1 on back faces, else max(`GhostCarValues.w`, mask red). With dust d = dust.r·dirtLevel, dg = sat(1 − 2d) and w = dg·Fresnel:
  - α_a = lerp(a, 1, w·ghost.z);
  - α_b = sat(sat(4.5a)·(α_a + s·strength·0.5·dg·pow(sat(sun·r), power)));
  - α = lerp(α_a, α_b, ghost.z)·ghost.x;
  - the colour is the lit colour mixed toward the environment by w, plus half the highlight·dg.
- **GhostCarValues.** 0xA14540 sets (1, 1, 1, 0). The per-vehicle loop 0xA18460 rewrites it only when the car's render-record fade (float +0x60, kept at vehicle+0x3a34) changes: x = clamp(1 − fade, 0, 1), y = z = 1, w = 1 while x < 1, else 0. The writer of the fade is not traced. A normal car therefore draws with (1, 1, 1, 0): full alpha, glass alpha raised by its reflection, no shadow floor.
- **Dust and mud set-up.**
  - Each track's `mud.xml` (`MUD` element; all 18 track folders have one) gives `mudcolour` and `dirtcolour` (#3B3834 everywhere), read by 0x9ABE70.
  - The VehicleMudPlugin set-up 0xA10B30 passes them per vehicle as `MudColour` and `DustColour`.
  - Visitor 0xA10710 (vtable 0xD161E8, name table 0xE4FC98: `dirtLevel`, `mudLevels`, `backwardsMudLevels`, the side, front and rear planes) and 0xA108D0 collect the parameter indices of every paint instance per LOD.
- **Dirt level.**
  - Surfaces carry `dirtbuildup` and `mudbuildup` rates in surface_materials.xml (dry and wet elements; 60 are 0, the others 0.001–0.05).
  - The career carries `max_maintained_dirt`, `max_maintained_scratches` and `max_maintained_dents` between events (0x63D0B0).
  - [I] The level starts at 0 for a fresh car. The per-frame writer that accumulates it, and the scratch writer, are not traced.

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

Common build. Until F801 the cars reflected nothing by default (`off`). Since F801 the default is the original's low setting, the track's static map (4.4); `--car-reflections original|high|ultra` or `GRID_CAR_REFLECTIONS` picks a dynamic level, `off` none. Step 4 (the owner's comparison) decides whether the default becomes the shipped `original`.

### 4.1 The cube (`car_reflections.rs`)

- **Cameras:** six Bevy cameras, one per face. Each is HDR (Rgba16Float), unshaded by tonemapping, has no MSAA, and uses the main camera's exposure, sky lighting and fog.
  - Placement: 0.5 m above the player's car origin, as in 0x5FBC10. In a replay, the shown car.
  - Range: near 0.1 m, far 1000 m (the far map).
  - Size by level: 256 (original), 512 (high), 1024 (ultra).
- **Copy:** after each frame, the six face targets are copied into one cube texture. The main view samples it the next frame. The faces must write their targets: with `CameraOutputMode::Skip` Bevy does not draw them (4.3).
- **Layer 27:** the cameras draw only render layer 27.
  - Every mesh on the default layer is also put on layer 27, except the player car's parts and batches, the player's exterior driver (`exterior_driver::PlayerDriver`; before it was marked, the probe inside the car drew the driver's suit into every face), and the overlays.
  - A mesh that leaves the default layer leaves layer 27 too: a batched part, an opponent's hidden detail level.
  - Systems that read a mesh's layers treat "default plus 27" as the default layer (`car_reflections::default_layers`): car batching (`car_batch`) and ornament levels. Overlay children do not count as a part's children for batching. A mesh whose layers are removed is rechecked the same frame. Until F676 the probe's layers and overlays stopped every car from batching: with reflections on, opponents were drawn part by part, without their shadow body's shadow, and the measured cost included that.
- **Lighting:** a copy of the brightest sun, without shadow maps, lights layer 27. The main sun is not on that layer, so the probe has no shadow cascades: Bevy gives a view the cascades of every shadowing sun its layers see. Point and spot lights stay out of the probe.
- **Detail:** the probe draws the main camera's choice of track and car levels; finer levels for ultra are not separate yet.
- **Faces per frame:** all six, as the original. `GRID_CAR_REFLECTIONS_FACES=N` (1–5) redraws only N faces per frame in turn, the others keep their last frame; an option for the owner to weigh, not the default.
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
- **Blend:** premultiplied, `lit·(1 − w) + env·w` without sun terms (see "Sun terms" below for the full composition).
- **Order:** an opaque or masked part's overlay sorts before every transparent draw (bias −10⁶); a blended pane's overlay just after its pane (+0.01). The original mixes the reflection inside each part's own program, so a reflector, cabin or cage behind glass has its reflection before the glass is blended over it. Until 2 November 2026 every overlay sorted 0.01 in front of its part's bounds centre: glass writes no depth, so an overlay behind a pane passed the depth test and, whenever its centre sorted after the pane's, was laid over the glass. The tail lamps showed the reflector's chrome over the red lamp glass (black while the cube was black), and the order between a window and the cabin parts behind it changed with small camera moves (private `local/reflections/out/ch-lamps.png`, `orbit.png`).
- **Fog and tonemapping:** applied as for the part.
- **What it leaves alone:** the part's own material, and the systems that swap it (batching, damage, lamps).
- **What it follows:** the part's mesh (deformation recomputes the normals, so dents bend the reflection), its layers and its visibility (a smashed pane is hidden).
- **Opponents:** their livery copies carry their part's controls.

**Paint gates** [O, `carpaint_metallic` pixel program, `car-paint-normal-scratches01` private disassembly]. With m1 = the car's packed `main_m1` map (s4, `TDustMap`) and shadow = the screen shadow mask's red (s5):
- `weight ×= sat(2·m1.a)`: the map's alpha masks the reflection (trim, gaps and badges reflect less).
- `env ×= sat(1 + OcclusionMapScale·(m1.g − 1) + SpecularValues.z)`: the green is the ambient occlusion that also darkens the lighting.
- `env ×= sat(shadow + SpecularValues.w)`: where the sun is shadowed, the paint mixes toward a darker environment.
- `weight ×= sat(1 − 2·m1.r·dirt)·(1 − scratch.a·(1 − vertex.g))·sat(1.1 − (1 − vertex.r))`: dust, scratches and the vertex damage masks (1.8). A clean car has 1 for the first two; red-free vertices cut the weight to 0.1.
- No other gate. An earlier note of a fade on downward-facing surfaces was a misreading: the only `sunDirection` terms weigh the lighting and the specular, not the reflection.

The port (F-entry of this change) applies the first three to `carpaint_metallic` parts: each car's own `main_m1` (an opponent its livery's), uploaded linear; the main sun's Bevy shadow at the fragment; `OcclusionMapScale` as saved (1 when absent, the loader's fallback). Dust, scratches and the damage vertex masks are left out (the port has no dirt yet).

**Dust, scratch and damage gates and GhostCarValues** (from F818 on; shared by every car, livery and LOD):
- The car mesh loader stores (1 − red, 1 − green) of each saved vertex colour as the mesh's second UV set (`car_materials::shader_controls`; LOD0 and the low LODs). Bevy's own material ignores it, and the batching key carries it. The overlay reads it as `uv_b`.
- Paint: `weight ×= dg·(1 − y)·sat(1.1 − x)`; dg also scales k and the highlight.
  - dg comes from m1.r·`DIRT_LEVEL`, and `DIRT_LEVEL` is 0: the accumulation is not traced, and a car starts clean.
  - AUTHORED: the scratch map (`_main_sc`) is not bound and counts as alpha 1, so scratches show wherever a vertex sets y. No saved paint vertex does.
  - The `DustColour` albedo tint is not ported; it is 0 at level 0.
- Shadow: s = max(shadow, `GHOST.w`) for paint and the front of window glass.
- Window glass (tuned `carglass`, program 2, intact or cracked) follows the program's alpha with `GHOST` = (1, 1, 1, 0).
  - The part below already holds a·lit + (1 − a)·background. The overlay returns α·(w·env + 0.5·highlight·dg) with keep = clamp((1 − α)/(1 − a), 0, 1).
  - Without a highlight, keep is exactly 1 − w. The background's share then matches, and the lit share differs by (1 − a)·w·(1 − w) ≤ 0.25 (1 − a) (AUTHORED: one premultiplied blend over the drawn pane).
  - Panes with alpha below 0.22 keep at most their background (clamped).

**Material tuning** (1.6, from this change on): a material whose name starts with a tuned name takes the record's Fresnel power, min and max and, on paint, its occlusion scale, together with the floors `SpecularValues.z` = 0.25 and `.w` = 0.5 (`car_reflection_overlay::Tuning`, read once from the installation's `raceload.jpk`). Other materials keep their saved pssg controls with floors of 0. A tuned material whose min and max are 0 (cabin, grills, suspension, tyre wall) does not reflect.

**Glass floors** (1.7): tuned window glass gates its reflection by the sun's shadow with the 0.5 floor, `env ×= sat(shadow + SpecularValues.w)`, and a back face counts as lit. The 0.25 occlusion floor has no gate on glass: the program's occlusion term has no map and is 1.

**Sun terms** (1.7): the overlay of a tuned `carpaint_metallic` or `carglass` part also draws the record's sun terms (`car_reflection_overlay::Lighting`) with the main sun (the shadow-casting directional light), its shadow at the fragment and the packed map's alpha:
- paint: the metallic darkening, the metallic tint by `pow(sat(sun·r), metallic power)` and the highlight;
- glass: half the highlight.

Composition: the overlay blends premultiplied (source One, destination OneMinusSrcAlpha). It returns the weighted environment, the tint and the highlight as colour, and 1 − the share of the part that stays as alpha, so that `out = lit·keep + colour` with `keep = (1 − w)·darkening·(1 − k)`. Fog is applied to the gain as `fog(colour) − fog(0)·keep`, which equals the earlier `w·fog(env)` where there are no sun terms. Units: the original's sun colour lights the diffuse colour directly, so sun = the light's colour · exposure / π, the scale of Bevy's diffuse term. AUTHORED: the highlight's factor `strength·pow(…)` is clipped at 1 (one lit white). The original clips the whole sum in its 8-bit target; the port's HDR view would otherwise pass the metal and wheel strengths (39–197) to the bloom as light.

The part's own material then has no specular (reflectance 0, metallic 0, no clear coat, no specular texture): its highlight and environment specular would count twice. This is set once per material when its overlay is made, so it applies only while reflections are on; without the overlay (the phone, reflections off) the part keeps Codex's authored lighting (`car_materials.rs`). Untuned materials and the lamp glass and pod programs keep their own lighting.

**Strength against the original** (2 November 2026): the owner's recording of the original, `local/recordings/donington-race-20261009/overview/frame-0050.000.jpg` (clean Lacetti at the start, chase view), against the port at the same view with the cube working (private `local/reflections/out/orig-0050-zoom.png`, `fx-rear-zoom.png`). Both show a highlight along the top edges, the sky on the dark trunk area and a readable livery; the masked port is of about the original's strength, the gates off read chrome-like. By eye only; the original frame is a compressed recording with dirt-free paint at another exposure.

**Diagnostic:** `GRID_CAR_REFLECTIONS_PAINT=x,y,z,w` sets the paint gates for comparisons: x the share of the alpha gate, y the occlusion scale, z and w the two floors. `0,0,1,1` turns them all off (the F670 look).

**Cracked glass** [O, `carglass` passConfigMask 1 and 32: `car_basic.cg.fp_glass_windows`, decoded statically, private `local/reflections/shaders/fp_glass_windows.asm`]: the program blends the diffuse colour and alpha toward `TDiffuseCrackMap` by a per-vertex crack amount (the interpolated normal's w), then mixes the lit colour toward the environment by the same Fresnel weight as intact glass (`FresnelValues`, the cube flipped in Z and divided by alpha, `EnvironmentColour`, the `SpecularValues` z/w floors on the occlusion and the shadow). The normal is the interpolated one: nothing perturbs it. A dust gate `sat(1 − 2·dust)` scales the weight. The output alpha rises with the weight by `GhostCarValues.z` (1 in race, 1.8). The port follows it: a cracked pane reflects like an intact one through its crack-map copy's colour and alpha. Until 2 November 2026 the port turned authored facets on a 48×48 UV grid and cut the weight by 60% where the crack map is opaque; at every level that showed as a square mosaic on the windscreen (private `local/reflections/out/look4-player-check.png`).

**AUTHORED:**
- Lamp glass and untuned glass weigh the reflection by the pane's alpha. Tuned window glass follows its program's alpha path (above).

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

**Profile of the per-view cost** (31 October 2026; `--features profile`, baseline 1080p, original level against off, 3600 ticks, no GPU timestamps; private `local/reflections/runs/prof2-*`). Exclusive CPU per frame, summed over the six faces:

| Where | ms | What it is |
|---|---|---|
| Command buffer finish (`RenderContextState::apply`) | 3.0 | Bevy gives every render-graph system its own wgpu command encoder and finishes it per view; wgpu records and validates there. Eight systems per face: main opaque 0.73, main transparent 0.69, bin unpacking 0.41, early prepass indirect parameters 0.27, upscaling 0.26, main indirect parameters 0.25, early mesh preprocessing 0.23, clearing indirect metadata 0.18. |
| `queue_submit` | 1.7 | Submitting those command buffers (about 8 more per face). |
| Visibility (`check_visibility`) | 0.7 | Every entity tested against every view; runs in the main schedule. |
| Preprocessing setup | 0.8 | Bind groups, binned instance buffers and work items per view. |
| Pass recording | 0.5 | Opaque, transparent and upscaling draws. |
| `copy_faces` | 0.13 | |

- The cost is per view, not per object: original, which draws only Donington's large objects, pays most of it.
- GPU pass timestamps are not the cause: without them, original still adds 9.4 ms to the render schedule (two repeats, `cost4-*`).
- Three of the per-view systems do global work: clearing the indirect metadata and building the indirect parameters walk every view's batches, once per view. Bevy needs that order (each view clears, preprocesses, builds and draws in turn), so it cannot be dropped from outside Bevy.
- The transparent pass cannot be emptied either: the sky dome is a blended material.
- **Withdrawn cut (F676):** the faces skipped their output pass (`CameraOutputMode::Skip`) and `copy_faces` copied each view's own texture. Bevy's `prepare_view_targets` gives a camera with no output attachment and a clear colour no view target ("we can skip rendering entirely"), so the faces were not drawn at all: the cube stayed black, every reflection mixed toward black (the owner's playtest of 31 October: practically no reflection, black lamps), and the measured halving (+4 instead of +9 ms) was the faces not rendering. Found with a copy probe (no face ever had a view target) and a missing cube dump. Reverted in the next entry: the faces write their own targets again, as in F670.

**Cost with the faces drawn and cars batched** (2 November 2026, baseline 1080p, 3600 ticks, no GPU timestamps; private `cost6-*`). Render schedule CPU: off 4.6 ms and original 10.5 ms in quiet runs (main schedule ≤ 3.5 ms), so original adds about 6 ms (about 1 ms per face); 3 and 2 faces per frame 8.8 and 7.4 ms. High and ultra were measured only in loaded runs (main schedule 4.1–4.5 ms): 13.6–14.0 ms, against 6.6 ms for off in a loaded run. One run each; other sessions were building.

**Open items:**
- The per-view cost is Bevy's: a command buffer per render-graph system per view, and the indirect-parameter systems that redo every view's work once per view. Cutting it needs a Bevy change (one encoder per view, or building indirect parameters once).
- Fewer faces per frame is the option `GRID_CAR_REFLECTIONS_FACES` (section 4.1); whether to use it is the owner's decision.
- The phone has not been measured.

**Look.** Private `local/reflections/runs/s2-*`, 1080p enhanced, Lacetti against the Donington pit wall:
- The paint and windows reflect the wall, the barrier and the sky with the scene's geometry.
- The dented rear bumper shows its deformation in the reflection; with reflections off it is hardly visible.
- The paint reads strongly mirror-like, likely stronger than the original's, because the original's gates (4.2) are missing.
- The cube faces (dump) have the sky up and the ground down, and match the scene.
- Not checked yet: cracked and smashed glass in a frame, opponents' cars, the cockpit mirror, replays, the phone.

### 4.4 The static map, the default (F801)

Backlog row 100's complaint (matte paint that reflects nothing) was the port's default `off`: the original always reflects something, the dynamic cube or, at its low setting, the track's static map.

**The original** [O, static analysis]:
- `track_manager_load_scene_nodes` (0x985550) clears the track manager's +0x3F20, then looks for the scene node `static_env_map` in the track's scenes. It takes the node's translation (node +0x80..+0x8C; the zero vector when no node is found) and calls `static_env_probe_create` (0x9853C0), which makes a 128-texel cube named `envmap`, cleared to grey 0.2 (the same kind as the water's probes, docs/WATER.md).
- `renderer_render_static_env_maps` (0x5FBA30) draws every such probe once while the race is set up: six faces, far plane 2000 m, the water hidden (0x64C9B0). With the env pass's `faces` at 0 it points the pass's cube (+0x84) at the static map; 0x5EABE0 does the same.
- `_TEnvironmentMap` (0x602FE0) then binds +0x3F20 when `faces` is 0, so every car samples the static map with the same car programs and constants as the dynamic cube.
- Data [O]: every race track saves a `static_env_map` node, in both land.pssg and tracksplit.pssg at the same world position (all 15 track folders; the test below prints them). For example Donington (290, 0, 183) and Washington (−330, 46, −635); Spa's is at the world origin (0, 0, 0). The frontend garages save one too.

**The port** (`car_reflections::Level::Static`, `car_reflections::static_origin`):
- The default level. It reads the node from land.pssg, else tracksplit.pssg, and adds one cube at its translation to the water's probe capture (`water_probe::install` is additive): the same capture, the same 128 texels, grey clear, far 2000 m, water hidden, drawn once under the pipeline warm-up's cover. The car overlays (4.2) sample it through `car_reflection_overlay::Environment`, which the dynamic levels fill with their cube instead.
- Used where the water's probes are: track scenes with the source land drawn, not collision views. The opt-in local probe (`--reflections probe`) and the paint pilot (`--paint-environment`) replace it; a dynamic level with the probe is still an error.
- The test `every_track_saves_its_static_env_map` (ignored, needs `GRID_GAME_ROOT`) reads every track's node.
- Cost: one cube drawn once at load (six more views for the warm-up's frames) and the overlay draws of the reflecting parts each frame.

AUTHORED: the cars on the grid are in the static map (they are in the scene while the probes draw); the frontend garages keep no environment (the menu scenes are not track scenes).
