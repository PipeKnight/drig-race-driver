# Water: the original's water.fx and its port (row 108)

Status: 2026-11-03, Claude Code (session fervent-napier). Owner's report (2026-11-02, Washington,
screenshot 33): a black void where the river is. F699 drew land.pssg on every preset, so the
river showed its bed; this ports the water drawn over it.

## Where water is

- Every `water.fx` draw is in a track's `land.pssg`; no other shipped package uses the group.
- Seven tracks: Haruna (`wetness`, a half-transparent layer in the gullies and streams beside
  the road, F757), Okutama (`sea`), Yokohama docks (`sea`),
  Detroit (`placeholder_water`), Long Beach (`water_material`), San Francisco (`Material #58`)
  and Washington (`sea`).
- Each shader instance saves `bumpHeight`, `bumpSpeedX/Y`, `deepColour`, `shallowColour`,
  `reflectionColour`, `reflectionAmount`, `fresnelBias`, `fresnelPower`, `textureScaleX/Y` and
  `waterAmount`, and binds `TNormalMap` (`grid-shader-constants LAND.pssg water.fx` prints them;
  `grid-water-mesh LAND.pssg [CELL]` prints each water draw's vertex colours and the world cells
  holding its visible vertices).

## The original's programs [V static]

`tools/disassemble_packed_grid_shader.py` on `shaderpack/shaderpack.pssg`, programs
`water.water.cg.vp` and `water.cg.fp` (private: `local/r41/water.cg.{vp,fp}.asm` in the
fervent-napier worktree).

Vertex program:
- three UV sets from UV0 scaled by `textureScale`: `s + 5t(sx, -sy)`, `3s + 30t(sx, sy)` and
  `5s + 12.5t(sx, sy)`, where `t` is the runtime `time` and `(sx, sy)` the bump speeds;
- the world tangent scaled by `bumpHeight * (1 - vertex green)`; the world normal and position;
- the vertex colour passed through (its alpha is the output alpha).

Pixel program:
- `n = normalize(sum of three (2 tap - 1))` of `TNormalMap` at the three UV sets;
- bumped normal `T n.x + cross(N, T) n.y + N n.z` (not renormalised);
- `edge = 1 - max(dot(-I, bumped), 0)` with `I` the eye-to-point direction;
- reflection: a cube `WaterEnvMap` at `reflect(I, bumped)` (z negated), RGB divided by its alpha,
  times `reflectionColour * reflectionAmount * lerp(fresnelBias, 1, edge^fresnelPower)`;
- colour `lerp(deepColour, shallowColour, edge) * waterAmount + reflection`;
- linear fog to `fogColour` beyond 30 m (`fogParams`); alpha = vertex alpha;
- pass blend SRC_ALPHA / ONE_MINUS_SRC_ALPHA.

No lighting term: water is not lit by the sun.

## Port

`crates/grid-app/src/water.rs` and `water.wgsl`, on every preset wherever land.pssg is drawn.
`load_package` still counts the draws (`omitted_water_draws`) and leaves them out of the part
list; `water.rs` reads them from land.pssg itself, with the same transforms.

The arithmetic above is ported as written. AUTHORED:
- Colour constants are decoded from sRGB; the original computes in display space.
- `time` is seconds.
- The port's fog replaces the program's own.
- A missing vertex colour reads as white (no bump, opaque); missing tangents are generated.

## WaterEnvMap: the original's static probe [V static]

`WaterEnvMap` is not in the track data; the game renders it at race setup. Static analysis of
grid.exe (master Ghidra project, `grid_re.py decompile`; no runtime trace):

- `track_manager_load_scene_nodes` (0x985550) looks up the `water` node of the land scene and,
  for each child node, calls `static_env_probe_create` (0x9853C0): one probe per child with a
  128-texel cube (`WaterEnvMap`) bound to the water drawn under that child. The probe stands at
  the world position of the child's first child node when it has one (the translation of that
  node composed with the child's and the `water` node's matrices), else at the child's own.
- `renderer_render_static_env_maps` (0x5FBA30) renders them once, through the base scene
  renderer: six faces per probe (`local_18f0 = 6`), near/far planes of 2000.0
  (`0x44fa0000`), and `static_env_probes_render` (0x64C9B0) hides the water itself and clears
  each face to grey 0.2 before drawing the scene and sky. Without a probe the slot falls back to
  the track manager's `_TEnvironmentMap` (+0x3F20).
- In the shipped land.pssg each `water` node has one child, the water mesh, and six of them
  carry an authored marker node named `*_cube_01` as its first child (world positions):

  | track | marker | probe position (x, y, z) | water draw height |
  |---|---|---|---|
  | Haruna | `sea_mesh_cube_01` | 662.8, 961.6, 25.4 | wet patches 855 to 1107 |
  | Okutama | none (`sea_mesh` itself) | 0, 0, 0 | 1.7 to 110 |
  | Yokohama docks | `sea_mesh_cube_01` | -483.8, 0, -206.6 | -2.2 to 0 |
  | Detroit | `sea_mesh_01_cube_01` | 330.5, 264.6, 948.0 | 158.8 |
  | Long Beach | `landscape_water_cube_01` | -98.4, 40.1, 356.0 | -6.4 to 9.4 |
  | San Francisco | `water_mesh_cube_01 ` (trailing space) | 1036.8, -22.0, -272.5 | 0 |
  | Washington | `sea_mesh_cube_01` | 395.6, 16.9, 1002.4 | -1.8 to 0.6 |

The cube is therefore a static picture of the track around the probe, made once, with the sky
and the buildings in it; the water's Fresnel term reflects that picture, not a mirror image.

## Port of the probe

`crates/grid-app/src/water_probe.rs` renders the same probes on every water track:

- `water.rs` reads the `water` node's children (`probe_nodes`) and assigns each water draw to
  the child it lies under; draws outside every child keep the baked sky cube (below).
- One 128-texel cube per used probe, cleared to grey 0.2, six 90-degree face cameras at the
  probe's origin, near 0.1 m, far 2000 m, the viewer's exposure, fog and environment light,
  tone mapping off.
- Rendered once, behind the start cover: the faces render through the pipeline warm-up's
  restore phase (docs/PIPELINE_WARMUP.md), with the water hidden and the sun's shadow maps off,
  and each frame's faces are copied into the cube layers in the render world. The restore phase
  ends only after eight frames without a new pipeline, so the faces' own pipelines are ready
  before the last copy, and the faces are removed in the frame the cover lifts. Without a
  warm-up (scenes that have none) the capture runs for the first 120 frames.
- The sky dome is centred on the probe while the faces render (`sky::follow_camera`).
- `GRID_WATER_PROBE=0` reflects the baked sky cube only (F705's look, for comparisons);
  `GRID_WATER_PROBE_DUMP=1` writes each cube as a tone-mapped strip to
  `<data dir>/graphics/water-probe-N.png`.

AUTHORED differences from the original probe:
- The cube is Rgba16Float (linear HDR), as the port's lighting; the original's format is not
  decoded.
- Near plane 0.1 m (the original's near value is not decoded; 2000 is its far plane).
- What the probe sees is the port's scene at the start: the cars on the grid are in it, and
  ornaments use the main camera's levels.
- Several probes would share one sky position (the first); every shipped track has one probe.
- Without the warm-up, the capture frames are visible to the player.

Before F754 the port reflected only the route's source sky (slot 0, lighting.xml `skyscale` /
`skygamma`) baked into a 64-texel cube by `sky_environment::bake_sized`, with the authored ground
colour below the horizon. That cube remains the fallback.

## Checks (F757): Haruna's wet layer

`grid-water-mesh` on Haruna's land.pssg: one `wetness` draw, 2659 vertices and 4624 triangles at
y 855..1107. Every vertex has a colour: alpha 101..165 (mean 132, so 40 to 65 % opaque), RGB 20, 30,
30. The constants are a dark grey water (`deepColour` 0.051, `shallowColour` 0.102, `waterAmount`
1) with a strong grey-blue reflection (`reflectionColour` 0.278, 0.278, 0.349 x 1.2, Fresnel bias
0.1, power 6), a 1.5 bump, a texture scale of 5 by 10 and a scroll of -0.1 along V, which is a
flowing stream. The other six tracks' water is nearly opaque (Okutama alpha 203..255, Washington
154..229).

Views from 6 m above and 17 m back from the four densest 50 m cells (route 0, offscreen
1280 x 800, probe vs `GRID_WATER_PROBE=0`; private `local/water/hw-after`, `hw-before`,
`hw-pair.png`, `hw-zoom2.png`): all 8 runs exited 0 with no panic, `probed_materials=1` with the
probe and `sky_cube_materials=1` without it.

- The layer lies in rocky gullies and stream beds beside the road and under a bridge, not on the
  road surface.
- It reads as dark green-grey moving water with streaks along the flow. It meets the rocks without
  a gap, and shows no z-fighting, holes or floating edges.
- Probe and sky cube look the same here: the probe at the marker (662.8, 961.6, 25.4) sees mostly
  green slopes and haze, about the colour of the sky cube's horizon.

A night-route drive (route 1, autopilot, chase camera, 8 points over 12 000 ticks) shows no
visible change either. The night routes light the terrain as day, which is SKY.md's open night
lighting item, not the water.

Not checked: the original's picture of these streams (no live capture). Seen in two views, not
inspected: tall beige vertical streaks under the bridge and beside a cliff. They are the same with
and without the probe, so they are not the water draw.

## Checks (F754)

Release build of the F754 tree. Offscreen 2560 × 1369 views from a camera near each probe
marker, probe vs `GRID_WATER_PROBE=0`, shadow bias held at `GRID_SHADOW_BIAS=bevy` in both.
Private evidence: `local/water/w-after`, `w-before`, `water-montage.png`. All 14 runs exited 0 with
no panic. Every water track logged `probed_materials=1` with the probe, and `sky_cube_materials=1`
without it.

| track (route 0) | probe origin | what the view shows |
|---|---|---|
| Haruna | 662.8, 961.6, 25.4 (marker) | no open water in view; Haruna's water material is the wet layer at y 855..1107 (close views: Checks (F757)) |
| Okutama | 0, 0, 0 (no marker child; the child's own origin, as 0x9853C0) | the dam pool reflects its banks, a little darker and bluer than the sky cube |
| Yokohama docks | -483.8, 0, -206.6 (marker) | nearly the same as the sky cube (dark `deepColour`, low reflection) |
| Detroit | 330.5, 264.6, 948.0 (marker) | the far shore and city appear as dark streaks in the river |
| Long Beach | -98.4, 40.1, 356.0 (marker) | nearly the same as the sky cube |
| San Francisco | 1036.8, -22.0, -272.5 (marker) | the buildings and the bridge pier appear in the water near the shore |
| Washington | 395.6, 16.9, 1002.4 (marker) | faint shoreline and building reflections |

No seams, black faces or flicker in any view. Not checked: the original's probe image (no live
capture), and Okutama's reflection from the original, whose probe also sits at the node origin.

## Checks (F705)

- Washington route 0: the river draws (1 draw, `placeholder_water_n.tga`), blue-grey with the
  sky reflected and moving ripples (private `local/r41/w0-water2.png`, `w0-water-down2.png`;
  before: `w0-water.png`, which reflected white).
- All seven water tracks load and spawn their water draw (Haruna, Okutama, Yokohama docks,
  Detroit, Long Beach, San Francisco, Washington; route 0). Yokohama's sea draws dark
  (`deepColour` 0 0 0.1, low reflection); Haruna's wet patches were not inspected close up.

## Addresses

| address | role |
|---|---|
| 0x985550 | `track_manager_load_scene_nodes`: finds the land scene's `water` node, one probe per child |
| 0x9853C0 | `static_env_probe_create`: a 128-texel `WaterEnvMap` cube at the child's world position |
| 0x64C9B0 | `static_env_probes_render`: hides the water, clears each face to grey 0.2, draws the scene |
| 0x5FBA30 | `renderer_render_static_env_maps`: base scene renderer, 6 faces, near/far 2000, `_TEnvironmentMap` fallback (+0x3F20) |
