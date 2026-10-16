# Donington source surface layers

16 October 2026. Current graphics-roadmap step 3, after the source-sky lighting and first car-material pass. Camera ideas are future backlog, not a change of priority.

## Static evidence

Private local/graphics/track-material-bindings01.json inventories 97 saved materials: 90 track_surface_simple, four track_infield, one batched_track and two decal. Packed vertex/pixel programs selected by the matching group/mask link were disassembled using the existing Microsoft D3DDisassemble tool; local/graphics/shaders/track-surface-{vertex,pixel}01.txt and metadata sidecars preserve source/program hashes. These are statically linked variants, not proof of live selection in every original-game pass.

The examined surface variant carries UV0/UV1 and linear vertex controls. Its color construction is:

- Both repeating diffuse maps sample UV0. Vertex red blends first to second, including the diffuse alpha used by the original specular path.
- Macro color samples UV1. Vertex green is limited to 0.5 before interpolating between the diffuse mixture and macro RGB.
- First skid UV is (C*UV1.x, A*UV1.y) + A_Offset; its red sample times vertex alpha times TWO is the first skid-color blend. The factor two follows scalar-swizzled dp2add, not a guess from texture appearance.
- Second skid UV is (-C*UV1.x, B*UV1.y) + B_Offset; its red sample times vertex alpha blends toward the second skid color. Original lerps have no saturation; do not silently clamp their weights.
- Macro alpha modulates lighting via a separate original AO scale uniform. That runtime scale has not been recovered.

All 90 saved skid colors are float3, not float4. Original source colors are preserved as raw ARGB bytes; the presentation converts them to RGBA linear controls, never albedo tint. Exact original sampler sRGB states remain unverified.

## Independently authored implementation

track_surface.rs/.wgsl uses ExtendedMaterial<StandardMaterial, Surface> for only the examined track_surface_simple group. It reads the purchased track package at runtime, validates saved bindings/constants, draw UV1/color counts and texture resolution, then attaches the second UV and control colors only to selected source draw meshes. Other track shaders, car materials and baseline remain unchanged. The custom fragment overrides the default vertex-color tint with explicitly composed albedo.

Color maps sample sRGB; skid red is a separately uploaded linear weight. Macro alpha stays linear. Bevy supplies lighting, shadows, SSAO, sky IBL and post-lighting operations. Macro AO strength=1 is an authored provisional choice; original Phong/Fresnel/specular-alpha response and lighting have not been ported. This is source-layer restoration, not a claim of original renderer equivalence or newly upscaled textures.

Enhanced profile selects source layers by default; --track-materials single provides the previous first-diffuse-only control, --track-materials source enables layers. No surface displacement, tyre contact, physical material, geometry positions or simulation inputs change. All extracted textures/captures remain ignored/private.

## Validation in progress

App all-target release Clippy passed after correcting mutable AssetMut and the newer as_chunks lint. An initial four-component skid-color assumption was corrected using the saved float3 payload audit before the first resource test. Private real-resource validation and matched native4K single/source GPU captures are required before calling this a verified playable update. Rust1.99.0, Bevy0.19.1, CPU jobs2.

Actual purchased-resource test passed:90definitions/306draws/35textures, UV/control counts valid, intermediate blend weights present. Both selected tests passed. Code revision63f24df; source-stamped GPU build pending.

First native4K source-layer pilot at63f24df exited101 before capture: name-only selection matched an unrelated package draw (264mesh vertices vs418expected). Package-local IDs are not globally unique. SourcePart now carries material identity; attachment requires both draw ID and material match before any mesh mutation. Revision6e64b97; all-target Clippy passes again, corrected source-stamped build/GPU retry pending. Failed log preserved as local/graphics/track-layers-source01.log; no successful image was produced.

Remaining infield variant statically inspected while build runs: local/graphics/shaders/track-infield-pixel01.txt (2064byte program)/track-infield-vertex01.txt (484bytes), metadata .txt.json sidecars. Vertex assembly matches the examined surface variant and keeps the same green limit; pixel variant uses the same two diffuse/macro blends and macro alpha AO but no skid sampler. Four infield materials remain excluded from this first implementation. Two incorrect metadata filename attempts were corrected by listing actual .txt.json files; evidence remains private.

Corrected6e64b97 GPU pilot attached all306draws/90materials, but produced two WGSL compilation errors because macro is reserved. Process exit0 and a saved PNG were insufficient validation; local/graphics/track-layers-source02.* is retained as FAILED shader evidence. Fixed identifier and corrected vertex-green clamp ordering: limit at vertices BEFORE interpolation, not per fragment. Added midpoint regression (0..255 green must interpolate0.25 at halfway). f538ae1 alltargetClippy passes; next GPU capture still required. Build6e64b97 took5m07s under background CPU load; no unrelated processes altered.

## Verified graphics release f538ae1f035c

Final two tests (including purchased resources and midpoint mask-order regression) passed; release all-target Clippy passed. Native4K track-layers-source03/single03 inspected: broad road variation and skid markings restored, source road grain remains original resolution. Both plus1280x800/240tick drive03 exit cleanly with no GPU/shader errors, all306selecteddraws/90materials attached, source IBL and clearcoat fix active. Final motion/handling equals G014 control exactly except wall clock. No full-lap/native-window/fidelity/performance claim. Private validation-track-layers01/current-build contain hashes and dimensions. ExeSHA256 b9c540320b61bf8e424ad7c7519585486897f17bad596f1fe73d5c5722d18a2e. Separate launcher uses this release; verified executable backup remains private. Next four infield materials, then reflection work in the agreed order.
