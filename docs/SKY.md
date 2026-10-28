# Sky and road surface on every circuit

20 October 2026 (Claude Code, owner handed sky and grey roads over from the graphics side). Static
analysis only: shaderpack disassembly with the existing `tools/disassemble_packed_grid_shader.py`
(Microsoft `d3dcompiler_47.dll` D3DDisassemble), Capstone 5.0.6 on the pinned
`local/compatibility/grid-unpacked.exe`, PSSG trees with `grid-pssg-tree`. The original game was not
run. Private evidence: worktree `sky-asphalt-fac786` `local/sky/` (shader assemblies, sky and
tracksplit trees, renders).

Labels: [D] data, [V] verified static code or test, [I] inference.

## 1. Why only Donington had a sky

- `sky.rs` read `tracks/europe/donington/sky.pssg` and `lighting.xml` whatever the route, and
  `main.rs` installed it only in the enhanced preset.
- Enhanced is gated to Lacetti on Donington route 0 (`main.rs`, "enhanced graphics currently
  requires Lacetti and Donington route_0"). Every other circuit runs the baseline preset, which
  draws no sky at all: the dark clear colour (0.055, 0.07, 0.095) is the "black sky".
- The same two gates kept the layered road surface (`track_surface.rs`) on Donington: it read
  Donington's `tracksplit.pssg`, and only in enhanced.

## 2. The original's skies [D, V]

Every track folder ships `sky.pssg` with one draw (`nickname="sky"`) and a dome of float3 positions
and float2 ST.

| Track | Shader group | Texture(s) |
|---|---|---|
| donington | sky.fx | sky_blue_11b |
| istanbul_park | sky.fx | sky_cloudy_03 |
| jarama | sky.fx | sky_blue_32 |
| le_mans | **nightsky.fx** | sky_lemans_1..4 in six slots |
| milan | sky.fx | sky_blue_12b |
| nurburgring | sky.fx | sky_blue_06b |
| spa_francorchamps | sky.fx | sky_spa_01 |
| haruna (track; routes 0, 2, 4, 5, 7 identical) | sky.fx | sky_sunset_29 |
| haruna routes 1, 3, 6, 8, 9 (night) | sky.fx | sky_sunset_03 (own `route_N/sky.pssg`) |
| okutama | sky.fx | sky_blue_07b |
| shibuya | sky.fx | sky_sunset_16 |
| yokohama_docks | sky.fx | sky_sunset_26 |
| detroit | sky.fx | sky_blue_04 |
| long_beach | sky.fx | sky_blue_15c |
| san_fran | sky.fx | sky_blue_15 |
| washington | sky.fx | sky_blue_28 |
| frontend garages | sky.fx | sky (dxt5) |

All day textures are 4096-wide DXT1; Le Mans' four are 2048 × 512 DXT1.

Shaders (shaderpack `sky` / `nightsky` groups, D3DDisassemble):

- `sky_main.psh`: `out = (tex(TDiffuseAlphaMap) · skyScaleAndInvGamma.x) ^ skyScaleAndInvGamma.y`
  (already recovered by the graphics side, SOURCE_SKY_LIGHTING.md).
- `nightsky_main.psh`: `out = (lerp(tex(TFirstMap), tex(TSecondMap), timeOfDay.z) · scale) ^ gamma`
  [V]: `r1 = s0 · c64.x; r0 = s1 · c64.x − r1; r0 = c0.z · r0 + r1; exp(log(r0) · c64.y)`.
- The sky object fills a six-entry table (+0x3EC4) by looking up the shader inputs **by name**
  `TFirstMap` .. `TSixthMap` (0x9852E8..0x985377: six string pointers, a lookup loop, texture
  type 3) [V]. Le Mans saves them as 3, 1, 2, 1, 3, 4 (parameter IDs 6..11 of its own
  SHADERGROUP). The render step 0x95FCE0 binds `tex[i]` and `tex[(i + 1) % 6]` with
  `f = fmod((skyTime + 10) · 6, 6)`, `i = clamp(⌊f⌋, 0, 5)`, `timeOfDay = f − i`
  (`grid_tools::time_of_day::sky_textures`, LE_MANS.md §3.4).
- Le Mans: at 15:00 `skyTime` ≈ 1/3 → f ≈ 2.0, slots 2/3 (sky_lemans_2 → sky_lemans_1, blend 0.001); at
  night slot 5 (sky_lemans_4) and `skyScale` falls (the daynight curve), so the dome darkens.
- Night presets darken the sky through the tone, not the texture: Shibuya `skyscale` 0.03, the
  Haruna night routes 0 (TRACK_ENVIRONMENT.md §2.1).

Which file: the route's own `sky.pssg` when it exists, else the track's [I: the same order as
`lighting.xml` in `grid_tools::environment`; the Donington load trace of 6 October shows only the
track file being read, and no route probes for either name].

## 3. The port (sky)

- `grid_formats::sky` [V unit tests]: `read(tree)` → transformed dome, UVs, indices, slot texture
  IDs and the distinct textures; `slots(bindings, material)` → one slot (`TDiffuseAlphaMap`) or six
  in name order. Unit tests: day sky, shuffled night inputs land in TFirstMap..TSixthMap order, an
  incomplete night set is rejected.
- `grid-app sky.rs`: `sky_file(root, route)`; the tone comes from the route's lighting preset
  (`grid_tools::environment`, which also reads the binary presets) instead of Donington's; on Le
  Mans route 0 (`Mode::Timeline`) a system follows `LeMansEvent::time_of_day`: slot pair, blend,
  `skyScale`/`skygamma` keys. Two-texture material; `sky.fx` binds the one map to both slots with
  blend 0.
- Installed on every track scene in both presets. `--sky procedural` still turns it off.
- **Default tone changed to source** (`--sky-tone source`, `skyscale`/`skygamma` of the preset):
  without it the night presets (Shibuya 0.03, Haruna night 0) and the Le Mans night would show a
  bright sunset dome. `--sky-tone raw` restores the earlier untoned look. The tone is applied in our
  linear pipeline as before (a linear-space adaptation, SOURCE_SKY_LIGHTING.md).
- `--ibl source` (enhanced, Donington only) bakes the start look's first slot; it is not re-baked
  as the Le Mans day turns.

## 4. Road surface

### 4.1 What the grey road was

- The 42–56 "unresolved batch" draws per track are `batchmaterial` = `batched_track.fx`. Its three
  passes are shadow (`vp_shadow` + `csm_depth_opaque`), velocity/depth and depth with
  `batched_track.cg.fp_null` [D shaderpack]: **no colour pass**. They are depth/shadow proxies of
  the road, and omitting them from the colour scene is correct. Donington has 48 of them too.
- The flat grey is the baseline renderer drawing each road material with only its first diffuse
  (`TDiffuseAlphaMap`, a grey asphalt tile at UV0). The original blends a second diffuse by the
  vertex mask's red, the macro colour map (UV1) by green, and two skid layers
  (DONINGTON_SURFACE_LAYERS.md); that reconstruction (`track_surface.rs`) ran only on Donington.

### 4.2 Shader groups on the other circuits [D]

| Group | Tracks | Albedo in the pixel program [V] |
|---|---|---|
| track_surface_simple | all but Shibuya | A/B by mask R, macro by G, skids |
| track_surface | Le Mans 10, Detroit 14, Washington 11 | same; plus a normal map (s4) |
| track_surface_reflective | Shibuya 59 | same; plus normal and reflection maps |
| track_infield | all | A/B by R, macro by G, no skids |
| track_infield_dxt5nm | Le Mans 26, Washington 7 | same; plus a normal map |
| track_infield_wsm | Haruna 36 | same, but A/B UVs from the world position |

`track_infield_wsm.cg.fp` [V]: with `s = 1/PScale` and `n` the interpolated normal, the A/B UV is
`s·(x, z)` when |n.y| is the largest component (ties to y), else `s·(y, z)` when |n.x| is, else
`s·(x, y)`. Its vertex program passes the object-space position (the track has identity
transforms). Macro UV1 and the masks are as in the other groups.

None of the vertex programs reads `Macro_U_Scale`/`Macro_V_Scale` [V grep of the disassembly].
Normal, reflection and environment-pass details are not reproduced (Bevy lighting, as on
Donington).

### 4.3 The port (road)

- `track_surface.rs` reads the route's `tracksplit.pssg` (and `land.pssg` in enhanced), accepts the
  six groups, and carries `1/PScale` for the wsm group in the material's spare control; the shader
  projects along the dominant world-normal axis.
- Installed in both presets (land only in enhanced, where `land.pssg` is appended).
- Package-local draw IDs: a same-named part from another package whose vertex count differs keeps
  its plain material and is counted (`mismatched=`); attachment no longer panics.

- Some materials leave inputs unsaved: Jarama has three road materials (its edge strips) without
  `TMacroColour`. AUTHORED [I: how the original binds an unsaved sampler is untraced]: such a
  material gets no macro blend and no macro AO (marked by a negative AO control); a missing second
  diffuse falls back to the first.

## 5. NaN texture coordinates

Every ST stream whose half words are 0xFFFF/0x7FFF decodes to NaN (TRACK_READERS.md §3.1; the
data stores them that way, nothing in the reader produces them). Which drawn triangles use them
(`grid-formats/tests/nonfinite_texcoords.rs`, every triangle grid-app draws from `objects.pssg`
closest LOD, `tracksplit.pssg` and `land.pssg` of 15 tracks) [V]:

| Package | NaN vertices | Indices drawn | Materials |
|---|---:|---:|---|
| haruna tracksplit | 3591 | 16229 | six `infield6/8`, `infield7/8`, `infield7/9` materials, all `track_infield_wsm` |
| spa objects | 15 | 21 | bridge_sponsor_a_01, building_kart_a_04, cell_tower_a_02 |
| yokohama_docks objects | 4125 | 2 | warehouse_02_c_04 |
| washington objects | 3 | 2 | building_department_c_varc_02 |
| shibuya objects | 1491 | 0 | — |
| detroit objects | 41 | 0 | — |

- Haruna: `track_infield_wsm` never reads UV0 (its A/B UVs come from the world position, §4.2), so
  the NaN UV0 is unused in the original. With the wsm projection ported, these draws no longer
  sample their NaN UV0; their UV1 (macro) is finite (the surface reader rejects non-finite UV1).
- Objects: 25 indices, i.e. at most 25 of about 690,000 drawn object triangles on Spa, Yokohama and Washington,
  have a NaN corner. They stay as they are (the original samples undefined coordinates there too);
  the test pins the count.

## 6. Results

Renders: release grid-app, baseline preset, 1280 × 800, `--scene reference`; "before" is the main
checkout's playable executable (sha256 bb725b6e…, same sky/road code as origin/main 0b51d92c).
Private PNGs and logs: worktree `sky-asphalt-fac786` `local/sky/before/`, `local/sky/after/`,
contact sheets `local/sky/sheet-horizon.png`, `sheet-lemans-day.png`, `pair-*.png`.

- Start views (`--view start`) on Le Mans, Nürburgring, Istanbul, Jarama (route 1), Yokohama,
  Shibuya, Donington; horizon views from 10 m behind `slot_00` (`local/sky/horizon.sh`) on Le Mans,
  Nürburgring, Istanbul, Jarama, Yokohama, Spa, Long Beach, Washington, Shibuya: before, a dark
  clear colour above the horizon; after, each track's own sky (checked by eye: Le Mans, Nürburgring,
  Istanbul, Jarama, Yokohama, Spa and Long Beach show clouds/sky; Washington's and Shibuya's horizon
  views look into an overpass). Haruna route 0 (looking at `160,1162,1060` → `60,1140,900`): the
  sunset sky, and the wsm cliffs show their rock texture instead of a smear.
- Le Mans with `GRID_SKY_HOUR`: 15:00 the day sky (slots 2/3, blend 0.001, scale 1, gamma 0.756);
  21:00 dusk (slots 3/4, blend 0.73); 01:00 near black (slot 5, scale 0.4, gamma 3.13). Only the
  sky follows the clock; the scene lighting is unchanged (LE_MANS.md §3.5).
- Road: `track_surface attached draws=N of=N mismatched=0` on Donington 343, Le Mans 494,
  Nürburgring 367, Istanbul 574, Jarama 360, Yokohama 310, Shibuya 303, Haruna 514, Spa 705,
  Long Beach 424, Washington 523, Detroit 437, Milan 15. Istanbul, Yokohama, Donington, Le Mans
  show the second layer and skid marks; at the Nürburgring start the change is small (mean road
  pixel difference 3.4 of 765), the data there being pale track1/track4 tiles.
- A 600-tick Le Mans 24 Hours drive (`settings/le-mans-24.ini`) runs with the sky following
  `drive_time_of_day` (15:00, slots 2/3).
- Checks: see PROJECT_LOG.

## 7. Open

- Scene lighting at night (Shibuya, Haruna night routes, the Le Mans timeline) is still the
  authored day light: only the sky darkens.
- Haruna's start view shows a smeared green foreground that is not a tracksplit surface (not
  identified); its `slot_00` start places the car on the hillside (TOUGE.md).
- The Haruna cloth flag `flag_a_01` (`cloth.fx`, no `TDiffuseAlphaMap` input) stays omitted;
  cloth is not simulated.
- `land.pssg` is drawn only in enhanced, which stays limited to Donington.
- Normal/reflection maps of the track_surface, track_surface_reflective and dxt5nm groups.


## 8. Background ordering with transparent depth writers (G272)

The independently authored source-sky material must draw before other transparent
geometry. Installed Bevy0.19.1 sorts `Transparent3d` ascending by view-space Z plus
material depth bias. The previous positive1e8 bias contradicted that intent. Frozen
crowd flash cards, which retain source depth writing even at zero alpha, exposed
black rectangles: the late sky at reverse depth0 could not pass their written depth.
The bias is now negative1e8. Sky texture/tone/fragment depth and flash blending/depth
remain unchanged. A regression uses the actual material and Bevy sort-key calculation.

The corrected bounded Detroit baseline three-arm render has exact Off/Control RGBA;
all10,952 changed Frozen pixels only brighten RGB, with no alpha changes. Native
review confirms the sky holes disappear. This demonstrates the diagnosed ordering
interaction in one view. It does not establish enhanced scene-probe compatibility,
all-track smoke behavior or source renderer parity. The separate baseline Lacetti
glass regression and private failures are recorded in G272.

G272 baseline glass follow-up: historical G271835613... and corrected84d84... both
render the same authored Donington Lacetti side view successfully,120samples each;
decoded1920x1080 RGBA matches exactly. Root native review finds no new glass/sky
edge defect. This is a qualified different-build comparison with new flash paths
defaultOff, not enhanced/DLAA/scene-probe validation. The first attempted old-exe
comparison refused a changed hardlink archive hash before GPU; that failure is
preserved and the lost archive is not represented as the original binary.

## 9. The route's own road: `routesplit.pssg` (F607)

The owner's Washington route 0 screenshot (2026-10-27) showed the main straight partly as the
background colour. Cause [V]: every route folder ships `route_N/routesplit.pssg` (62 files) beside the
track's `tracksplit.pssg`, and grid-app read only the tracksplit.

| Routes | routesplit draws (colour) | What [D] |
|---|---|---|
| Washington 0–9 | 148–216 | the layout's road, kerbs, walls and grid markings |
| Milan 0–5 | 208–412 | the same; Milan's start straight had no road at all |
| Okutama 0–5 | 239–492 | the same (Okutama still stops in the entity reader) |
| San Francisco 0–6 | 134–172 | the same |
| Detroit 8/9 (stadium) | 11 | markings |
| every other route with a split | 3–5 | grid boxes and start line (`decal.fx`) |

Plus `batched_track.fx` shadow proxies in every split (Washington route 0: 29 of 214), which stay
omitted as in section 4.1. The split has no textures: each binding is `tracksplit.pssg#NAME`.

Port: `main.rs` `append_route_split` appends the split under the `route/` namespace on every track
scene in every preset, with the diffuse maps resolved against the tracksplit's textures;
`track_surface::read_source` adds its layered materials (Washington route 0: 180 draws) with the
tracksplit's textures. Physics does not read it. The tracks without split files (Le Mans,
Nürburgring, Spa, Haruna, Yokohama) are unchanged; Donington changes only by its grid boxes.

Tests: `purchased_route_splits_resolve_on_every_route` (grid-app, all 62 routes) and the per-route
loop in `purchased_surfaces_resolve_on_every_track`.
