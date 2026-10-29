# Static object material coverage

Static objects retain a material-instance to source shader-family association when loaded from `objects.pssg`. This is a limited PBR adaptation; it does not execute the original shaders.

## Detroit grandstand panels

The owner reported fine dark/light perforation beneath the seats in the Detroit stadium. CPU rays from the saved back-grid camera identify the affected panel as material `gg_building_grandstand_01_j_04`, family `glass_simple.fx`. Its diffuse alpha contains a fine stippled pattern. The separate fence in front is `concrete_fence_white_end_01`, family `object_simple_mesh.fx`; the backing stand also uses that mesh family.

Previously the renderer assigned `Mask(0.5)` to every `objects/` material. That hard threshold turns the panel alpha into discrete holes. Selected source geometry rays put the panel substantially behind the fence and ahead of the backing stand; they do not support a coplanar-overlap diagnosis at these samples.

The saved ordinary `glass_simple` shader pass enables SRC_ALPHA / ONE_MINUS_SRC_ALPHA blending and disables fixed-function alpha testing. The correction selects **only the exact `#glass_simple.fx` family** as `AlphaMode::Blend`. Other object materials retain their existing cutout behavior. Classification uses the source material instance, not a name containing “glass”, a texture filename, or a whole-track override. Car windows and trees use their existing independent paths.

This is source-backed colour coverage, not full pass parity. The saved source pass also writes depth and uses separate-alpha ZERO/ZERO, whereas Bevy StandardMaterial Blend has its own depth/separate-alpha behavior. The exact saved `object_simple.cg.fp` shader was also disassembled: it samples TDiffuseAlphaMap and copies its alpha directly to the output; no texkill/discard or control flow appears in that program. This closes the sampled-alpha contract for this saved variant. Live original pixels, active runtime variant/overrides and a matched original camera have not been observed. The claim “the original shows this exact appearance” therefore requires additional reference evidence.

## Verification and remaining scope

The purchased-resource test identifies the actual Detroit panel and the adjacent fence, then verifies that only the selected source glass changes from Mask to Blend. Native matched-camera checks must verify that the stippled panel improves while the fence keeps its intended openings. Preserve raw controls and input/executable hashes under ignored `local/`.

Other source families have different alpha-test thresholds and can combine testing with blending. A blanket Opaque replacement or universal threshold change is not justified by this panel fix. In particular `object_simple_mesh` has an ordinary saved threshold of zero; mapping that to a Bevy mask requires handling the source GREATER comparison and preserving zero-alpha holes. That separate adaptation remains open.

Physics, object transforms, mesh topology, diffuse textures, ground shading and smoke are unchanged by this correction. Runtime camera/LOD tests and cross-track glass controls qualify general promotion separately from the single-view diagnosis.

## Owner-approved solid backing

The first glass-only native comparison still exposed noisy holes in the backing panel `objects/building_grandstand_01_j_01`. Its source `object_simple_mesh` ordinary pass uses ONE/ZERO replacement, GREATER-zero alpha test and alpha-to-coverage, not glass alpha blending. The owner explicitly permits a solid textured fallback instead of holes. Therefore **only this exact backing material** uses Opaque, retaining source RGB, UVs and geometry. All other mesh-family objects, including the fence, retain their prior behavior. This is an authored fallback, not original MSAA coverage parity. The front glass still uses the separately justified Blend correction.

## Remaining stipple follow-up

The owner identified fine light dots remaining in the first solid-backing comparison. The front glass texture has uniform RGB but stippled alpha, and still blended in that candidate. A follow-up extends the owner-approved solid-cladding fallback to the exact front material `objects/gg_building_grandstand_01_j_04` as well. Both layers keep their original RGB, UV and geometry. Other glass-family members still use Blend. Dots from the foreground fence must be distinguished from cladding transparency; do not call the surface clean solely because the larger holes disappeared. The follow-up requires fresh native views.

## First live original reference and residual fence aliasing

A live original Detroit stadium race was captured after the owner selected the event. In the unobscured right portion, the original shows a coherent fence pattern over the dark stand; the port pose comparison shows sparse points and broken wire detail. This is a preliminary visual observation, not pixel parity: the original is paused, uses 1280x1024 letterboxing, medium quality and 2x MSAA; the static port uses 1280x720 and 4x MSAA. Read-only camera sampling found a plausible chase pose with 52-degree FOV, while the static port uses 45 degrees. The active pause camera is not established. CPU rays associate the original right region with the same panel material identities at another instance of the stand.

The follow-up both-panel Opaque candidate completed two native camera checks and one 3-metre-back check without process errors. Fine foreground points remain. At the original diagnostic camera its panel ROI differs from the previous candidate by mean absolute RGB 0.395 and maximum 10 byte values; solidifying the front panel is therefore not a complete fix for the reported points. An independent sample of ten isolated bright components places the foreground fence first at every ray, with eight having robust nonzero wire coverage above the old threshold in the base texture. GPU mip/sample coverage still needs separate treatment.

Simply changing the fence to Bevy AlphaToCoverage does not establish original parity. Pinned Bevy 0.19.1 sharpens main-pass alpha around 0.5 before coverage, falls back to a hard mask without MSAA, and uses different prepass/shadow coverage. The next fence experiment must account for colour, depth and temporal motion together, preserving genuine zero-alpha openings. Keep the exact panel fallback separate from that work.

## Bounded fence coverage diagnostic

`--object-coverage off|control|a2c|mask35` defaults to `off`. It targets only `objects/concrete_fence_white_end_01`. Active modes require a static rendered Detroit LOD0 reference, a bounded benchmark and no compositor/performance run. Baseline controls require an actual MSAA4 camera. Only Control/Mask35 may also enter an enhanced native DLAA trial, with the optional feature, AA3, default Lacetti and actual MSAA off. Other enhanced Detroit play remains rejected. The solid cladding remains unchanged. `control` changes no material fields; the same-build Off/Control decoded captures were byte-identical. Four native GPU runs completed successfully, including the plausible original chase pose.

The A2C comparison makes the near wire pattern more connected than Mask(0.5), but distant point patterns remain. This is Bevy's sharpened coverage, not a faithful implementation of the original shader, and A2C is not enabled for normal play or DLAA. The owner additionally permits modestly thicker wires; the next isolated experiment can compare a slightly lower cutout threshold while preserving zero-alpha holes, before introducing any derived texture or changing geometry. That thickness experiment is not implemented or verified here.

## Modest thickness trial

The owner permits slightly thicker wires. `mask35` adds an isolated `Mask(0.35)` trial on the same exact fence material. The initial native stage retains the bounded baseline/MSAA4 restrictions; the separate enhanced DLAA stage retains the same static/target/context bounds. It changes no RGB, texture data, mip chain, geometry or cladding. CPU atlas measurements show about 5% more accepted wire-region area at the base mip than Mask(0.5); this is coverage, not a measured world-space wire diameter. Rail/base edges in the same atlas also change slightly. A more aggressive 0.25 cutoff was not selected because the last mip's averaged alpha can admit the whole card. Zero-alpha samples remain rejected at 0.35; preservation of every minified opening is not established. Native evaluation does not establish temporal quality. The enhanced DLAA follow-up is separately tested before any playable/default promotion.

## Ordinary-default trial rejected after moving comparison

A post-main trial promoted Mask(0.35) for the exact Detroit fence to ordinary material setup. Its static Off/Control/explicit-Mask35 captures were identical, and both native MSAA and DLAA outputs exactly matched the previously tested Mask35 controls. The installed 18-package inventory found this exact identity only in Detroit. Those checks establish narrow scope and reproducibility, not moving visual quality.

Two baseline side-chase sequences then exposed the limitation: at the tested distance Mask(0.35) makes more fence coverage visible as isolated points, while the previous Mask(0.5) mostly loses those wires. The old-executable control has an identical tick/spin capture manifest and the sampled vehicle pose/velocity agrees. A connected mesh in the static DLAA comparison does not establish that this ordinary MSAA result is an improvement.

Therefore the universal default promotion was reverted before publication. Ordinary setup keeps Mask(0.5); only the existing bounded `--object-coverage mask35` experiment selects the thicker cutout. Keep the better static DLAA result as a separate candidate. A future actual DLAA moving control and pass-consistent subpixel coverage treatment are needed before selecting a wider ordinary default. No accepted launcher was replaced. The trial's executable and source hashes remain private evidence of a rejected candidate, not a binary for the restored source.
## Bounded moving native DLAA comparison (G258)

Control/Mask35 have a separate opt-in moving diagnostic for solo Lacetti on Detroit route_8/subroute0: reference LOD0, enhanced native DLAA, land off, 1920x1080, no loaded settings/event files, bounded drive and 30–60 captured frames. The static validator and ordinary Mask(.5) default remain unchanged. Capture timing reuses the existing motion contract with eight ticks of headroom. This admission is not interactive route support or proof of temporal quality; actual DLAA preparation, matching traces, visible fence and moving image review are still required. Other render arguments are pinned by the paired private runner, not universally whitelisted by admission.

The bounded G258 pilot completed two native-DLAA 60-frame sequences with identical capture manifests and sampled physics/input traces. Review of the ordered native fence crops finds substantially better near-wire connectivity with Mask35, preserving openings. Distant dots and disappearing thin wire remain. The window covers roughly 15–37 km/h and is not high-speed or real-time temporal acceptance. The ordinary default stays Mask(.5); the experiment is not promoted to an interactive launcher.


## Front-sheet pass isolation (G279; validation in progress)

The owner requests revisiting the solid fallback. The default-off experiment
`--foundation-front off|control|source-front` changes only the exact front sheet
`objects/gg_building_grandstand_01_j_04`. Control retains Opaque and only audits.
SourceFront inherits PBR/texture/geometry and uses explicit source colour
SRC_ALPHA/ONE_MINUS_SRC_ALPHA, separate alpha ZERO/ZERO, depth-write on and A2C
off. Cull remains unchanged; source auxiliary/shadow/prepass parity is not claimed.
Backing and fence remain fixed. The scope is static route8/sub0 baseline reference,
LOD0/native1080/source sky+track/sky reflections/clean settings/1..600 frames.

Both audit arms verify the actual native image/full viewport/no override/MSAA4.
SourceFront additionally joins CPU-main-visible identities to ready sorted-phase
pipelines. GPU-culling candidates are refused as visibility proof. This is one
earlier snapshot, not a final screenshot ticket or completed fragment proof;
Control has no opaque-phase pipeline join. The required crate gate passed379tests and Clippy, followed by a fresh baseline
release and six successful native GPU arms (two poses, Off/Control/SourceFront).
Off and Control decoded images match exactly at both poses. SourceFront replaces
eight material components; two CPU-visible targets join two ready phase entries
at frame3 in each view. The native front remains coherent, but fine dotted/grainy
structures remain, so this is not a complete foundation fix or default promotion.
Changes extend outside the coarse panel ROI; auxiliary/shadow behaviour is not
emulated and the ROI is not a material segmentation mask.

Stored PNGs are RGB. The historical comparison helper expands them to RGBA with
synthetic alpha255; its unchanged alpha is therefore not attachment-alpha proof.
The source-derived ZERO/ZERO alpha state is observed in the ready descriptor,
while screenshot alpha is unavailable. Native channel analysis preserves this
absence explicitly; raw captures remain unchanged. A raw-alpha4x per-sample colour/depth fixture for the separate backing pass
is planned; Bevy A2C sharpening and source D3D9 level2 are not assumed equivalent.

## G280: isolated per-sample coverage measurement

An explicit desktop-only `--coverage-sample-probe local/.../fresh-result.json`
branches before gameplay/settings/asset-root discovery. It uses no game input,
only an authored64x4 chart, private RGBA8Unorm/Depth32Float4x attachments and a
16KiB storage readback. Four policies compare Opaque, Mask.5, Bevy's sharpened
A2C and rawGT0 hardwareA2C. Each sample is recorded before and after a behind-red
surface. Colour/depth ownership, unwritten sentinel, exact threshold/equality
and alpha0/1 extremes are independently verified. Stored zero alpha is not used
as an opacity/coverage proxy; no screenshot conversion or resolve intervenes.

Actual Vulkan GPU result passes1024sample records and256masks. Covered-sample
histograms for the64pixels in each row are: Opaque4:64; Mask0:36,4:28;
sharpened0:36,2:8,4:20; raw0:23,1:13,2:9,3:8,4:11. Raw and sharpened masks differ
at22/64x positions. Intermediate counts are observations, not universal
alpha-to-sample thresholds. The fixture passes380app tests and Clippy.

This measures the selected adapter's authored4x behaviour. It does not establish
original D3D9level2 mapping, filtered source texture/mip behaviour or a visually
accepted stadium fix. Front/backing/fence ordinary materials are unchanged;
no default or playable promotion. Private evidence: backing-coverage01/sample02,
result SHA561cc284e19d2b399f3b2265ef3a8a839da125aae233d2500b02eee16292cf8e.

## G283: backing coverage experiment and shutdown ordering

The default-off `--foundation-backing control|raw-a2c` experiment selects only
`objects/building_grandstand_01_j_01`, with `--foundation-front source-front`
fixed in both arms. Control and Raw both disable batching and shadow casting
for those selected entities, retaining their previous marker presence for
restoration. Therefore backing-Off is a reference, not a pixel-neutral control.
The fence, front material, geometry, textures and culling remain fixed.
Raw preserves filtered diffuse alpha, discards zero alpha and uses hardware
4x coverage without Bevy's alpha sharpening. Emission and clustered-decals
must be absent; the ready native opaque pipeline and typed ownership are checked.
This does not establish original D3D9 level2 coverage or a complete visual fix.

The first native series stopped after its third process: Raw saved a screenshot
and exited zero but logged actual bind-group/pipeline validation errors after
material restoration. It is a failed run, retained under
`local/graphics/foundation-backing01/static-run01/`; no visual acceptance follows
from that image. Bevy's PostUpdate material collectors ran before the old Last
restore, allowing the new material binding to meet a retained pipeline.

The correction moves restoration and its deferred verification before the native
mesh/material collectors in PostUpdate. A CPU fixture installs actual
MaterialPlugins, warms change detection, delivers ScreenshotCaptured in Update,
and reads native specialization lists for both success and error AppExit.
The previous Last order is its negative control. This fixture passed in the
418-test attempt03; Clippy then rejected its complex test Query signature.
A local test-only lint annotation is the sole subsequent change. The fresh gate,
build and six native runs remain required before accepting the correction.
A shader-layout label alone does not identify the failing entity: the front
adapter never swaps back at shutdown, and no corresponding front patch is justified.

Fresh attempt04 passed the required crate gate: 418 tests and Clippy, followed by
an actual baseline release build. Executable SHA-256:
`c3f44060b5b5647c0daca04e063d94cc683027942644c1eaec67eadf240c4b4e`.
All six native processes in `foundation-backing01/static-run02` completed with
no renderer errors, including after restoration. Control and Raw each selected
eight components, joined two visible ready targets and restored all eight in
both poses. Independent offline re-parsing agrees. Two additional ordinary-Off
captures match the historical G280 controls exactly in native RGB and PNG bytes.
These are bounded static results, not temporal or enhanced/DLAA validation.

Raw restores visible structural depth behind the panel. It also leaves a dense
lower-panel point pattern, more conspicuous than the solid control in both views.
The owner likes the source-pose Raw appearance; the successfully completed fresh
capture is pixel-exact to that liked image from the failed first run. Preserve
this approved visual direction. Remaining points cannot all be assigned to the
backing: front and foreground fence are separate fixed layers. The owner also
notes weak fence visibility in this view; distance/filtering is a hypothesis
requiring near/far controls, not a confirmed cause. Default material settings
remain unchanged pending broader coverage and the fence check.

## G285: foreground fence distance control

The owner noticed the weak foreground mesh in the liked Raw view. A new pair
holds the G283 SourceFront+Raw backing, fence material, AA, FOV and other settings
fixed. The closer authored camera translates eye and look-at by 12 m along a
previously resolved fence ray: the known first hit moves from 17.31 m to 5.31 m.
This is a bounded camera test, not an original matched view or exhaustive
collision survey. Both native processes finish successfully with strict ready
pipeline/restoration checks and no renderer errors. The far repeat is native-RGB
exact to the accepted G283 direction.

The closer image clearly shows connected diamond wire on the right foreground
fence, with visibility receding along the more distant sections. Distance and
minification therefore affect its visibility in this setup. This does not
isolate texture filtering, mip selection, thresholding, sample coverage or
background contrast, nor establish parity with the original. No thickness,
mask, AA or ordinary-material setting was changed. Private evidence:
`local/graphics/foundation-backing01/distance-pair01/`.

## G286: owner-requested approximately doubled wire trial

`--fence-wire-scale 1|2` defaults to 1. Scale 2 clones the diffuse image only for
`objects/concrete_fence_white_end_01`; other materials retain the original shared
image. Its audited 512x512 atlas has wire in [0,512)x[0,384). A radius-one 3x3
alpha maximum, periodic in X and clamped within that wire region in Y, increases
measured base horizontal wire runs from median 5 to 9 texels. This is roughly
1.8x, not an exact projected or perpendicular diameter. Radius two overshoots
at about 13 texels, so it was not selected.

The existing BC decoder supplies RGBA8. All decoded RGB bytes are retained;
only the wire-region alpha and its interior mapped mip alpha change. Guard band,
lower rail and concrete alpha stay unchanged, as do the final 1x1 source mip and
all other material image handles. The original game assets are never written.
Decoded RGB preservation is not a compressed-format sampling identity claim.

Admission reuses the existing static coverage Control validator and refuses
combination with the cutout-threshold pilot. The current baseline build is the
trial target. Feature builds inherit that validator's static native-DLAA
exception; it is not an independently validated enhanced thickness mode.
No moving, general-route or playable-default promotion follows from these
static trials. Keep this small comparison separate from smoke work.
Actual G286 captures complete in four successful native processes. Original-scale near/far images exactly match G285 native RGB; the thicker candidate improves diamond-wire visibility in both views, with residual distant fragmentation. 418 tests passed before the equivalent fixed-size-chunk lint correction; strict Clippy and the release build passed afterward. No extra moving matrix was run for this owner-requested small trial. Evidence: `local/graphics/fence-wire02/gpu01/`.

## Source-family sidedness (G331)

The owner found the Detroit fence visible only from the track side. Saved `object_simple_mesh` shader-group passes (normal, velocity/depth, CSM depth and cheap variants) all specify `cullFaceType=NONE`. The port had assigned Back culling to ordinary objects. Material-instance membership in exactly `#object_simple_mesh.fx` now carries that source policy into shared object material construction, across tracks. Other families keep their previous culling; this is independent of alpha coverage and wire thickness.

The family includes22 Detroit materials, including closed buildings and stand backing. Face-aware Bevy normals preserve current front lighting and flip newly visible back normals; source pixel shader uses vFace too, but exact original winding/sign and lighting equivalence remain unverified. Default547 and combined DLAA/paint555 tests plus strict Clippy pass. Matching native opposite-face captures restore the reverse wire; a420000-pixel front-wire ROI is unchanged. Other newly visible family backfaces and shadows do change the full image. An ordinary12-car Detroit run completes600ticks with default ornament LOD and automatic smoke replay. Static comparison temporarily disables ornament LOD because a separately identified static-reference lifecycle defect leaves its levels hidden; this override is not used by the shipped culling fix. All-track/device, exact source lighting, native-DLAA fence runtime and performance parity remain unverified. Private source evidence and the proposed capture poses are in `local/graphics/fence-sidedness01/`.
