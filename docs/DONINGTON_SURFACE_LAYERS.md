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

Infield follow-up eaa8146: four definitions now use the same source diffuse/macro composition with an explicit disabled skid path;94materials/343draws/39textures. Two selected tests/alltargetClippy passed. Native4K elevated source01 inspected and240tickdrive01 completed withoutGPUerrors; motion/handling unchanged. Private validation-track-infield01/current-build/verifiedexe updated. Batched/decal remain on their existing renderer. Next reflection experiment, with forward surface profile as verified control.


## Packed alpha and next surface-lighting pilot (18 October 2026)

A fresh read-only audit ties all94 selected material bindings to the purchased tracksplit package SHA7e83a37e... and decodes the base mip of all39 bound textures. All27 repeating diffuse maps have nonwhite alpha. For example don_track1_d is512x512 BC3, alpha0..105/mean15.085; don_track2_d is512x512 BC3, alpha0..42/mean2.500. Rumble strip03 is256x512 BC3, alpha0..255/mean43.665. Alpha is a lighting control, not road transparency: the inspected source surface fragment interpolates the two complete RGBA samples by vertex red and uses the mixed alpha to weight its direct specular term. Macro/skid RGB blends do not replace that alpha. The existing Bevy extension composes RGB but currently omits this specular mask.

There is no TNormalMap binding among these94 materials and no normal sampler in the examined surface/infield programs. Do not claim a forgotten source road normal map from this evidence. All94 saved Macro_U_Scale/Macro_V_Scale values are1; the examined vertex program copies UV0/UV1 directly, so current unscaled UVs agree for this content. Surface PhongPower=5/SpecScale=3 for90 definitions; infield10/1 for4. Those saved scalars are not a recovered original runtime packing or a direct conversion to GGX roughness.

Private materials-livery01/track-surface-alpha-audit01.py/.json and track-surface-alpha-atlas01.png retain hashes, exact per-texture alpha statistics, bindings and inspected RGB/alpha contact sheet. The script uses the existing independent PSSG survey and Pillow12.3.0 decoder; no assets copied into public source. Base-only statistics do not validate all mip contents or original sampler state. Existing packed masks must survive any future RGB upscale; the opaque body packager correctly rejects these images and must not be used to erase alpha.

Next bounded implementation: an explicit opt-in source-alpha surface reflectance adaptation, using the existing samplers and mixed alpha, with white-control/off image parity, native4K road/curb comparisons, RGBA fallback and moving/full-lap validation before changing launchers. Keep the original Phong/Fresnel/direct-light algebra separate from the authored Bevy response; no invented height map, wet-road sheen or roughness-from-colour shortcut.


### Off-by-default surface F0 control

`--surface-specular off|source-base|white-control` is a worktree pilot; defaults stay off and no launcher selects it. It requires an enhanced track scene and source layers. Existing material controls.w carries the mode without new textures/samplers or mobile binding pressure. Source-base multiplies the material reflectance by sqrt(clamp(mixed diffuse alpha,0,1)) AFTER sampling and blending, so authored GGX dielectric F0 is weighted by that alpha. White-control uses1 through the same shader branch, requiring rendered parity with off. Unlike the paint texture pilot this path needs no KHR texture half-factor compensation because it edits the already-decoded material reflectance directly. Colours, roughness, opacity, macro/skid composition and geometry stay unchanged; all uploads still use texture_upload::image/image_linear.

Both examined source programs weight direct Phong specular by mixed alpha. Our GGX adaptation also changes the base environment response and does not reproduce that original light separation, Fresnel or saved SpecScale. This distinction must be judged with matched native4K images; CPU compilation alone does not establish shader validity or visual quality. The198 application tests pass (surface-specular-tests01.log); the final validation guard was edited while that compilation was active, so subsequent Clippy/build and negative CLI checks must cover the exact committed source. GPU validation and source-stamped candidate build pending; current installed graphics binary remains6aec3a9.


### Native/moving pilot acceptance at5b5d408

Release4m20s/SHA1f858a1bfdb91445da1097ebcd37bcae25c1cf1e73c9d0543afb334f2de2ae24. All4 native3840x2160 captures are nonblank/error-free. Off and white-control equal each other AND accepted6aec3a9 source-base paint image bytes exactly. Source-alpha changes mean4.396164byte/max84 versus white, chiefly visible as less uniform pale road sheen; do not attribute every changed pixel without a material-ID mask. ForcedRGBA uploads0BC and559images, full-image mean0.104973byte/max150 versus BC, not pixel parity/phone validation. Comparison sheet inspected; no source texture replacement or artificial normal map.

Motion1200ticks/one opponent has30unique1080p virtual30FPSframes, request clock error2.05ns, all4physical exit records equal6aec control. First/last frames inspected. Native4K6500tick run completes1valid89.955257484second lap; final4physical/handling/deformation/lap records equal6aec control exactly except wallclock. Finalimage inspected. Exact committed-source negative CLI checks reject unknown mode, baseline and single-layer usage. Full675workspace tests/12harnesses0failed0ignored and all-target feature Clippy3.01s/default25.35s pass. Private surface-specular-* captures/logs/validation JSONs; no original shader/asset distribution.

A separate `Play GRID Graphics Surfaces.cmd` comparison wrapper is being prepared; normal graphics remains off. Actual extracted CMD checks pass3cases: old executable omits unsupported flag, new opt-in selects it, new default omits it. Delivery/source publication still pending completed pre-push stress validation.

Next texture work must extend the contract deliberately: existing override V1 accepts opaque RGB candidates and scopes used keys to objects/cars; it rejects road keys rather than silently replacing layered surfaces. The surface extension owns a separate CPU texture map re-read from the purchased package. Road RGB overrides will require an explicit alpha-preserving version and routing through that extension, retaining shared post-device uploads and atomic exact-source checks. Do not relax V1's opaque-alpha validation or mistake a general scene texture replacement for layered-surface delivery. The existing asphalt x4plus pilot was already rejected for smoothing; this audit does not reverse that decision.


### Preserve source alpha while replacing RGB

Offline curb03 proof produces a1024x2048 BGRA carrier/full12mips from256x512 BC3/full10mips. Candidate levels2..11 copy the corresponding authored alpha mip exactly; levels0/1 use deterministic periodic integer bilinear of the original base alpha. Candidate RGB bytes do not change during this attachment. The original DDS is immutable; constant/wrapping-edge invariants and all10mip byte comparisons pass. This is private preparation, not an engine-ready override or a recovered normal/height map. Neural RGB crack/grain changes are still an aesthetic candidate only.

Next implementation should expose an explicit surface-RGB-only override contract with purchased-source exact-byte guards, per-bound-diffuse key checks, bounded atomic staging and source alpha restored by the runtime; existing opaque V1 stays strict. The layered extension owns its own CPU bytes and must consume that validated replacement before shared post-device upload. Do not add extra GPU mask samplers merely to preserve data already present in the source packed diffuse.
