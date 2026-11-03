# Light shafts: the god-ray cards (row 107)

Status: 2026-11-03, Claude Code (session fervent-napier). Owner's report (2026-11-02, Washington,
screenshots 29-32, 35): long black "drops" in the air. F699 found these are the `god_rays*.fx`
cards and stopped drawing them; the owner then asked whether they really are light shafts. This
ports their shader.

## The cards

- `objects.pssg` shader instances of `god_rays.fx` (7 tracks) and `god_rays_single_sided.fx`
  (4 tracks), on cards such as Washington's `godray_tree`, under trees.
- Each instance saves `GodColour`, `GodStrength`, `GodDensity`, `DustSpeedX/Y`,
  `distanceStartFadeOut`, `distanceEndFadeOut`, `smoke_scale` (and `LightColour`, `UV1Offset`,
  `rotation_axis_*`, which the programs below do not read). The texture is `godrays4.tga`.
- `lighting.xml` saves `lightShaft_colour`, `lightShaft_strength`, `lightShaft_density`
  (Washington: white, 1.42, 3.0).

## The original's programs [V static]

Programs `god_rays*.object_light_shafts.cg.vp` (both groups identical) and
`object_light_shafts.cg.fp_light_shafts` (private: `local/r41/` in the fervent-napier worktree).

Vertex program:
- `fade = 1 - smoothstep` of the card origin's view depth from `distanceStartFadeOut` to
  `distanceEndFadeOut`;
- alpha `fade * |dot(world normal, normalize(P - eye))| * vertex alpha`: a card seen edge-on
  vanishes;
- dust UV = UV1 + `(timer + origin.x * origin.z / 11) * DustSpeed`.

Pixel program, with `shape` the texture at UV0, `dust` at the dust UV, `S = strength * GodStrength`
and `D = density * GodDensity` (the `trackLightShaft*` uniforms, named after lighting.xml's keys [I]):
- colour `(1 + 0.3 dust.g) * backlightSunColour * GodColour * trackLightShaftColour`;
- `m = lerp(shape.a, dust.g, S shape.a / 2)`;
- `body = lerp(m, smoke.b, S m)`, with smoke the texture at `smoke_scale * (uv0.x, dust.x)`;
- alpha `body * D * vertex alpha * saturate(z/w / 2)`;
- blend SRC_ALPHA / ONE_MINUS_SRC_ALPHA.

So the texture gives only the shape and the moving dust. The colour is the sun's colour: warm
and bright on Washington (`backlight_suncolour` 2.07 1.54 0.79), and the cards are translucent.
In the original they are sunlight falling through the trees, not dark shapes.

## Port

`crates/grid-app/src/light_shafts.rs` and `light_shafts.wgsl`. `append_objects` keeps the cards
as object parts with UV1 and vertex colour; a PostStartup system swaps their material for the
light-shaft one (alpha-blended, unlit, no depth write, two-sided for `god_rays.fx`). The float
audit skips them: they hang in the air by design.

AUTHORED:
- the distance fade uses the pixel's distance, not the card origin's view depth;
- the dust has no per-card phase (`origin.x * origin.z / 11`);
- `timer` is seconds; the near-plane fade assumes a 0.5 m near plane;
- the colour is used as linear HDR before the port's tone mapping;
- the texture's green and blue are re-encoded from the sRGB upload.

## What they look like (owner's question)

The original's own arithmetic makes them faint: with the card texture's colour black where its
alpha is set (the plain-object render showed it black), `body` peaks near 0.2, and the alpha is
`body * 2.1 * vertex alpha * facing * 0.5`, at most about 0.2. So in the original they are a
thin, warm, translucent haze in the sun's colour, mostly seen against the trees they sit under.
On the baseline preset the port draws no trees (`source_trees` is enhanced only), so the cards
stand in open air and read even fainter against the sky.

No capture of the running original was made (the original is not launched); "the original's
look" here is its shader applied to its data.

Crops of the owner's Washington spot (camera 703, 8.2, 858 looking at 729, 6.6, 858.9; private
`local/r41/crops/` in the fervent-napier worktree): `shafts-before.png` (black spindles,
a8cef46b9294's plain-object draw), `shafts-after.png` (this port) and
`shafts-after-stretched.png` (the same, contrast stretched 165..200 to show the warm streaks).
