# Night lights: street lights and car headlight beams

The original's NightLights pass lights every night route (forced-night routes such as Haruna 1/3/6/8/9,
and Le Mans while its timeline says headlights are on) with the track's street lights and every car's
headlight beam. This document records the recovered algorithm and the port (`crates/grid-app/src/night_lights.rs`,
`night_lights.wgsl`). Static analysis only: the original was not run. Decompiles and shader listings stay
private under `local/`.

## 1. Data

| Source | Contents | Reader |
|---|---|---|
| `tracks/<region>/<track>/street_lights.xml` | `light name= position= direction= range= brightness= maprangemultiple=`; parser 0x732970 normalises `direction`, stores brightness and map range ×1024 as i16 (defaults: range 8, brightness 1, map range 2). Unnamed lights (up to 2048) feed the pass; named ones go to a separate table of 32 that nothing found draws. | `grid_formats::street_lights`, `grid_tools::street_lights::load` |
| `tracks/<region>/<track>/headlights.pssg` | `headlight.dds` (BC3): left half the headlight beam cookie, right half the street-light cookie | `night_lights::load_cookie` |
| `daynight.xml` (Le Mans only) | `enableHeadlights`, `lightStrength`, `headlightdirectional`, `lightColour_R/G/B` over time | `grid_tools::time_of_day::Timeline` |

Shipped data: only Le Mans (162 lights; range 5.7..54.4, brightness 0.8..4, one pointing up) and Haruna (4 lights;
range 20, brightness 5) have `street_lights.xml`; both ship
`headlights.pssg`. Without a timeline the values are the defaults of 0x72CF10: `headlightdirectional` 0.3,
`lightStrength` 1, `lightColour` (1, 1, 1).

## 2. Original algorithm [V, static]

**Lamp fade.** 0x73B890 steps a global fade F (docs/LE_MANS.md §3.3); car i has delay d_i (0x727B20).

**Street lights (0x72C460).** Each frame, every unnamed light within ±500 of the camera in x and in z
(a square, not a radius) becomes a projector record:
- weight = `brightness × sat(F / 9)`;
- forward = `direction`; up = `cross(dir, normalize(cross(Y, dir)))`, or (1, 0, 0) when `dir` is vertical;
- range = `range`; cookie = right half of the atlas.

**Headlights (0x736020).** Every car with lights on: weight `1.5 × clamp((F − d_i) · 4, 0.001, 1)`,
range 110 for the focus car (0xDB79D8) and 60 for the others (0xDB79D4), beam forward
`normalize(0, y, 0.9962)` in car space with y = −0.45 in timeline mode (0xDB79DC) and −0.8 in forced
night; cookie = left half. The beam origin is the car model's headlight anchor (`headlight_frame`).

**Shader (NightLights fragments, 0x736D80 binds the records).** Per pixel, for each light with
`v = P − light position`:
- `q = (cross(up, dir)·v, −up·v, dir·v)`; the light reaches P only when `q.z > 0`,
  `0.5 |q.x| < q.z` and `0.5 |q.y| < q.z` (a rectangle of half-angle ratio 2);
- attenuation `sat(1 − |v| / range)`;
- facing `sat((1 − d) · N·L + d)` with d = `headlightdirectional`;
- cookie at `uv = q.xy / q.z × 0.25 + 0.5`, then `u = u × 0.5 + half` (0 headlight, 0.5 street light);
- term = `cookie × attenuation × facing × 3 × min(weight, 1)`.

The terms are summed, saturated, multiplied by `lightColour × lightStrength` and written into the
shadow mask's GBA (docs/SOURCE_SKY_LIGHTING.md, "Shared screen-space lighting mask"); the surface
shader adds that RGB to the diffuse light before albedo and ambient occlusion.

**Not ported:** the pre-pass 0x736B70 / 0x7274C0 that sorts lights into screen cells (the original keeps
at most 2 headlights + 4 street lights per cell). The port sums every clustered light per pixel instead,
which is the same picture where no cell exceeds the limit.

## 3. Port

`night_lights::install` runs on every track scene whose `Mode::for_route` is not `None` (not with
collision views, headless runs or the research projector pilots).

- One shadowless Bevy `SpotLight` per unnamed street light and per car slot carries the light through
  Bevy's clustering. Its fields carry the original's values: `shadow_depth_bias` is a marker
  (−31415 headlight, −31416 street light; it also selects the cookie half), `shadow_map_near_z` the
  weight `3 × min(w, 1)`, `radius` the facing bias d, `range` the range, `color` the shared
  `lightColour × lightStrength × unit` (unit = the emissive unit, sun illuminance / π). The cone
  (`atan(2√2)`) encloses the rectangle.
- The patched `bevy_pbr::pbr_functions` spot loop skips Bevy's own lighting for a marked light and sums
  `grid_night_term` (inserted into `bevy_pbr::lighting`); after the loop
  `direct_light += sat(sum) × colour × diffuse_color × diffuse_occlusion`. The patch is pinned to the
  Bevy 0.19.1 source hashes and refuses other versions.
- The cookie atlas rides on each light as a render-world clustered decal (tag `NIGHT`), so the shader
  reads the light's own transform and texture. Without clustered decals (Bevy feature
  `pbr_clustered_decals`, binding arrays on the device) the beam is drawn untextured.
- Timing: driving uses `vehicle_lights::State` (the per-car fade) and `LeMansEvent`'s timeline; a static
  scene snaps the lamps on at its start time (`sky::static_start`) when the route wants them.
- Lights outside ±500 x/z of the camera, with zero weight, or before the shader patch are hidden.

Open:
- night routes take the NightLights branch of the shadow-mask selector, which skips the CSM sun shadow
  (docs/SOURCE_SKY_LIGHTING.md); the port still draws sun shadows there;
- cookie orientation and the brightness unit are checked visually only (see the log entry);
- the static Le Mans night scene keeps the day sun and ambient (the timeline's lighting values are
  applied in driving only).
- the forced-night Haruna scene keeps a day-bright ambient and sun in the port (static and driving), so the
  beams read weaker against it than in the original; that belongs to the sky/ambient path, not this pass.

## 4. Checks

`sh tools/night_lights_ab.sh [OUT [ROUTE...]]` renders each route with `GRID_NIGHT_LIGHTS=off` and on at
hour 0 from the same 12 audit cameras and as a scripted windowed drive (`--drive --drive-ticks 240
--render`), which runs the driving branch of `night_lights::present`, and diffs the pairs with
`tools/image_ab_compare.ps1`. A forced-night drive must report `night_lights shown ... driving=true`.
Results of the first run are in log entry F776.
