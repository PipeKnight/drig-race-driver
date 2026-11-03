# Water: the original's water.fx and its port (row 108)

Status: 2026-11-03, Claude Code (session fervent-napier). Owner's report (2026-11-02, Washington,
screenshot 33): a black void where the river is. F699 drew land.pssg on every preset, so the
river showed its bed; this ports the water drawn over it.

## Where water is

- Every `water.fx` draw is in a track's `land.pssg`; no other shipped package uses the group.
- Seven tracks: Haruna (`wetness`, wet road patches), Okutama (`sea`), Yokohama docks (`sea`),
  Detroit (`placeholder_water`), Long Beach (`water_material`), San Francisco (`Material #58`)
  and Washington (`sea`).
- Each shader instance saves `bumpHeight`, `bumpSpeedX/Y`, `deepColour`, `shallowColour`,
  `reflectionColour`, `reflectionAmount`, `fresnelBias`, `fresnelPower`, `textureScaleX/Y` and
  `waterAmount`, and binds `TNormalMap` (`grid-shader-constants LAND.pssg water.fx` prints them).

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
- `WaterEnvMap` is bound at runtime and is not in the track data (the name appears only in the
  shader tables). The port reflects the route's source sky (slot 0, lighting.xml `skyscale` /
  `skygamma`) baked into a 64-texel cube by `sky_environment::bake_sized`, with the authored
  ground colour below the horizon. It reflects no buildings or cars.
- Colour constants are decoded from sRGB; the original computes in display space.
- `time` is seconds.
- The port's fog replaces the program's own.
- A missing vertex colour reads as white (no bump, opaque); missing tangents are generated.

## Checks (F705)

- Washington route 0: the river draws (1 draw, `placeholder_water_n.tga`), blue-grey with the
  sky reflected and moving ripples (private `local/r41/w0-water2.png`, `w0-water-down2.png`;
  before: `w0-water.png`, which reflected white).
- All seven water tracks load and spawn their water draw (Haruna, Okutama, Yokohama docks,
  Detroit, Long Beach, San Francisco, Washington; route 0). Yokohama's sea draws dark
  (`deepColour` 0 0 0.1, low reflection); Haruna's wet patches were not inspected close up.
