# Grass cutout/shading diagnosis

17 October2026, owner-reported black dots/strokes in native4K far-view crops. This supplements GROUND_EFFECTS_RESEARCH.md; no physics or purchased files changed.

## Established observations

- Matched `grass-off-far4k01` using existing GRID_GROUND_EFFECTS=0 removes the marks. They are generated ground-cover sprites, not terrain albedo or tyre marks. Current scatter/source sprite selection remains unchanged.
- Current enhanced material uses Mask0.5, terrain normals, no shadow casting, cullNone/double_sidedfalse. Source gc_billboard512x512BC3 atlas inspected on a neutral background: thin dark green/brown blades. Base visible RGB mean54.51/61.77/24.07, not a black missing texture.
- Candidate233012ca0700 / executable SHAf3792dcf2d8305c3edd6cf72a67053267c25a854cc83eab4953d08a15b64b9c5 / release2m35s adds independent --ao on|off and --grass-shadows on|off diagnostic controls, defaulton preserved. At the exact owner view, disabling shadow reception leaves selected814dark pixels byte-identical. Disabling AO changes them by mean0.518byte only. Thus those mechanisms do not explain the observed black marks here; other viewpoints are not established by this test.
- Fresh full-mip export: grass texel alpha>=128 coverage falls7.106%base ->4.893%256 ->2.899%128 ->1.343%64 ->0.195%32, then0%at16and smaller. Mean alpha stays~8.2%. Visible RGB stays near55/62/24 through64: a mip-RGB-darkening hypothesis is not supported. Hard thresholding loses fine parts and keeps sparse dark opaque remnants. Texture statistics alone do not establish the whole screen-space effect.
- Existing original source ground-cover shader includes sun/back/ambient contributions plus screen shadow mask; original render state uses alpha-to-coverage. Our enhanced main view uses MSAAoff/SMAA; pinned Bevy0.19.1 alpha-to-coverage falls back to a mask without MSAA, so simply renaming the alpha mode does not restore smooth coverage. Earlier mirrored-geometry normal problem had already been corrected upstream; it is not reintroduced here.

## Current experiment and limits

Opt-in --grass-alpha blend keeps original atlas/RGB/scatter/geometry/shadows but uses soft alpha composition in enhanced mode. Ordinary mode remains mask until GPU acceptance. This is an authored alternative to original A2C, not its reconstruction. Transparent grass does not write opaque depth, is sorted as one generated mesh and may have overlap/SSAO/contact limitations; assess near/far views and movement before changing any default. No texture recolouring or guessed brightness lift.

Evidence private in graphics local/graphics/vegetation01: grass-atlas.dds/grass-atlas-grey.png, grass-alpha-colour-mips01.json, grass-lighting-isolation01.json/crop01.png, grass-{off-far,no-shadows,no-ao,no-ao-shadows}4k01 captures/logs. Nativepixel ROI2250,730..3150,1100, selected baseRGBmean<80/no-grass>120/drop>35: localized diagnostic, not quality score. Existing tools/Pillow12.3/NumPy2.3.5 reused; source assets/decompilation remain ignored.
