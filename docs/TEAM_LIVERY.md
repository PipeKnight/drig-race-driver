# Team livery on the race cars

The livery a player designs for their own team (the career's livery page,
CAREER_SCREEN.md "Season end, Le Mans entry, team name and livery") and how
the original paints it onto the car. Markers: [V] verified in the binary,
shader disassembly or data; [I] inferred; [O] observed in the port.

## Which cars wear it

- [V] Three callers (0x63FF70, 0x649C60, 0x643460) read
  `vehicle_livery.file_string` for the car's livery id, take the text after
  the last `_` (separator string at 0xCE67EC) and compare it with `user`
  (0xCC321C). Only on a match do they call the team-livery builder 0x5F6250
  and hand the result to 0x5ABC80.
- [V] `vehicle_livery` has 47 `*_user` rows, one per car that can carry a
  team livery (for example `lam_user`, `rx7_user`; `tra_user` appears three
  times).
- [I] The career assigns the `*_user` livery to the player and the team-mate
  in the player's own team. Driver offers and tests race in the team's
  shipped livery.

## The livery object (0x5F6250)

- [V] The number is `livery_number.number_string` (0x5E9160 → 0x4F23E0);
  the pattern is `livery_pattern.livery_image_name` (0x5E9060 → 0x4F2440,
  which writes +0x14).
- [V] The primary, secondary and tertiary colours sit at +0x174, +0x178 and
  +0x17C, each packed `0xFFBBGGRR` by 0x5F1D50 from `livery_colour` red,
  green and blue.
- [V] Sponsors: 0x649AB0 and 0x642630 read `sponsor.file_string` with
  `"%s_%d"`, one main slot and seven more.
- [V] 0x6C53C0 binds `TDiffuseAlphaMap` (the car's `livery_user` main
  texture) and `TLiveryDiffuseMap` (the pattern) and sets the three tint
  constants; 0x6C7D40 runs the passes below; 0x6CC080 releases them.
- [I] Primary → red, secondary → green, tertiary → blue follows the copy
  order into +0x5C, +0x60, +0x64.

## Files

- `cars/<car>/livery_user/textures_high/<car>_tex_high_user.pssg` [V]: the
  car's body textures for the team livery. `<car>_main.tga` is the body base
  (DXT5); its alpha marks where the base covers the pattern (alpha 1 is the
  base, alpha 0 the pattern). The other textures (wheels, glass, cabin) are
  used as they are.
- `cars/<car>/<car>_livery.pssg` [V]: the livery map, a whole car model
  (`LOD0_` matrix-palette bundle, one joint per part, render instances with
  a `shader` and index ranges). Its render sources have a `SkinIndices`
  stream and two `ST float2` streams and no positions; the first ST is the
  place on the material's own texture, the second the place on the pattern.
  Its shader instances bind no textures. Each render instance selects
  `indicesCountFromOffset` indices from `indexOffset`, all inside
  `streamOffset` + `elementCountFromOffset` vertices.
- `cars/user_liveries/liveries/<livery_image_name>` [V]: the pattern, named
  by `livery_pattern.livery_image_name` (for example `simple_01.pssg`); 53
  files, each one 2048² DXT1 texture (`blank.pssg` is 128²). Its
  red, green and blue channels say how much of the primary, secondary and
  tertiary colour lies at each texel.
- `livery_colour` [V]: 66 rows of `red`, `green`, `blue` (0–255).

## The compositor (0x6C7D40)

- [V] Which triangles it draws: 0x6C7D40 walks the livery scene with the
  visitor whose vtable is at 0xCEE170. Its node callback 0x6C4130 accepts a
  joint whose id contains (`strstr`, 0xA66880) `x0_` (0xCEE17C, level of
  detail 0) and one of 27 body-part suffixes (table 0xDB5308: `_bonnet`,
  `_boot`, the six bumpers, six canopy parts, four doors, `_main_body`,
  `_under_f`, `_under_b`, four wings, two wing mirrors). Of that joint's
  render instances it keeps those whose shader instance name contains the
  material 0xE4FD58[byte 0xCEDDF4 = 0], `Car Body Paint`, and rebinds them
  to the override shader instance (`shader_instance`, looked up by
  0xBA5950, given `livery_map_livery_and_blend`).
- [V] The other materials sample other textures: the material table at
  0xD14B10 maps `Car Body Paint`, `Matt`, `Shiny Metal`, `Bare Metal` and
  `Aerials` to `_main` (TDiffuseAlphaMap), `Cabin` to `_cabin`,
  `Suspension` to `_susp`, lights and discs to `_lights`; wheels and tyres
  are not in it. [O] Drawing every map triangle painted 99 % of the lac
  body texture: wheels, tyres, suspension and discs have every second ST at
  (0, 1) and their first ST spans their own texture.

- [V] The colour words at the parameter's +0x5C, +0x60 and +0x64 become the
  shader constants `tintForRed`, `tintForGreen`, `tintForBlue`, unpacked as
  `0xFFBBGGRR`.
- [V] Vertex program `map_livery_and_alpha_blend`: the position is
  `(2u0 − 1, 1 − 2v0)`, so each triangle is drawn in body-texture space; it
  passes the first ST to the base sample and the second to the pattern
  sample. The blocks it loops over only tile the render target.
- [V] Fragment program: `P = pattern(uv1)`,
  `tinted = min(P.r·c0 + P.g·c1 + P.b·c2, 1)`,
  `out.rgb = lerp(tinted, base.rgb, base.a)`, `out.a = 1`.
- [V] `fill_gaps`: a texel whose alpha is below 1 takes the first of its eight
  neighbours with alpha 1, so the edges of the drawn triangles bleed one
  texel outward and mip filtering does not pull in undrawn texels. [I] The
  port runs it once; the pass count was not recovered.
- [V] `replace_texture` and `clone_texture` (with `tintColour`) belong to the
  later sponsor and number passes, which are not ported.

## Port

- `grid_formats::livery_map` reads the livery map and keeps the instance
  ranges the visitor accepts; `grid-app` `team_livery` rasterises those
  triangles on the CPU at texel centres with the shader's
  arithmetic, runs one `fill_gaps` pass, makes the result opaque and builds
  the mip chain with the race-number helper.
- [I] The shader math is applied to raw 8-bit values (no sRGB conversion);
  the original render-target format was not recovered.
- The race profile key `[game] team_livery = <pattern file>,RRGGBB,RRGGBB,RRGGBB`
  turns it on for the player and the `teammate` car when their model has a
  `livery_user` package; other cars and models without one keep their shipped
  livery. Opponents of the player's model keep their own textures: missing
  keys in their livery bank are filled from the player's shipped livery
  before the team livery replaces it.
- The career writes the key for own-team events once the team has a livery.
- Not ported: sponsor and number passes, the livery page's 3D preview.
