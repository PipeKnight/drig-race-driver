# Temporal-AA input audit (implementation design, not a completed fix)

19 October2026. The material release is frozen separately at3362fd0. This design records local Bevy0.19.1 source inspection during its compile; no audit or DLAA path is installed.

## What is established

SMAA Ultra remains the accepted mode. The earlier owner-visible TAA car trails remain unresolved. In the current application, player and opponent rigid parts really receive pose * wheel_motion * rest transforms in present_drive; body positions are not all baked into an identity transform. Bevy prepass::update_mesh_previous_global_transforms runs in PreUpdate; the default prepass shader projects current and previous models with unjittered view matrices. Thus missing deforming-position history alone is not evidence that rigid-car velocity is absent.

Bevy's documented prepass excludes alpha-blended materials. Glass therefore needs a distinct test: do not conclude that its missing velocity proves the opaque body is broken. MeshStream writes current vertex bytes; prior deforming positions for dents, CPU-facing foliage and streamed geometry still require investigation. These are separate from normal rigid transforms.

Motion-vector textures are Rg16Float UV displacement with +x right/+y down. The prepass shader computes (current_clip.xy/current_w - previous_clip.xy/previous_w) * (0.5,-0.5). TAA samples history at current_uv - motion. Depth32Float is reversed Z. TAA's current shader chooses the nearest-depth vector from five taps, applies variance clipping, and does not by itself prove motion inputs correct.

## Bounded numerical capture

Add an opt-in audit that copies a bounded region from the main ViewerCamera's ViewPrepassTextures through a compute shader to a COPY_SRC ShaderBuffer. Direct copy of the existing depth/MV textures is unsuitable because their usages do not provide COPY_SRC; do not change dependency code or every render target to obtain it. No presentation shader/color/history is replaced by this observation path.

Use the current0.19 camera-driven API, not old render-graph Node examples: schedule a root RenderGraph compute system after core_pipeline::schedule::camera_driver. This orders sampling after the camera passes and before the renderer's submit_readback_commands. Prepare bind groups only when the main-view textures, buffer and compute pipeline exist.

Each immutable capture slot needs a GPU-written header: request ID, extracted frame, physics tick, texture dimensions and ROI origin/size. Store raw float motion.xy and raw depth per pixel; keep exact column-major current/previous model matrices, unjittered current/previous view-projection, and actual jittered inverse view-projection beside the matching header. Do not pair an asynchronous callback with the callback-time car pose. Use a small fixed number of slots, one dispatch each, bounded buffer allocation and observer cleanup; unavailable prepasses or timeout must report failure, not a blank successful capture.

For depth reconstruction use the actual jittered projection used to rasterize that frame. ExtractedView.clip_from_view alone is unjittered; prepare_view_uniforms applies TemporalJitter::jitter_projection. Expected velocity itself uses the unjittered matrices. Mixing these conventions would manufacture an apparent subpixel error.

## Validation sequence

1. Static camera/static opaque road: finite depth/MV, near-zero static displacement. Known camera translation against fixed road: projected sign and scale agree.
2. Undamaged rigid body in chase and side views: reconstruct conservative interior body points from depth; map current world positions through inverse current model and previous model. Compare predicted UV offsets with sampled vectors. Exclude silhouettes, glass, wheels and disocclusions.
3. Wheels, glass, billboard foliage, dents and car resets/camera cuts independently; document expected unsupported/deforming cases instead of extrapolating body results. Verify the correct main camera rather than probe/shadow views is sampled.
4. Only after numerical inputs are checked, compare SMAA/TAA at virtual30 and300FPS with fixed controls. Inspect moving films, not one still. Virtual capture timing is not measured monitor performance.
5. Add a pinned optional native-resolution DLAA path with runtime support checks and SMAA fallback; test history resets, dimensions, jitter and custom-pass resolution. The existing bridge cargo-check is not GPU acceptance. Keep optional SDK/DLL files private.

Local sources inspected: bevy_core_pipeline0.19.1/src/{prepass/mod.rs,schedule.rs}; bevy_pbr0.19.1/src/prepass/{mod.rs,prepass.wgsl}; bevy_render0.19.1/src/{camera.rs,view/mod.rs,gpu_readback.rs}; bevy_anti_alias0.19.1/src/taa/{mod.rs,taa.wgsl}; Bevy shader_prepass and gpu_readback examples. No installed crate was edited.

## Private draft preparation

During the frozen material checks, a private draft shader and capture module were prepared under lacquer01. Direct Rust metadata checking against the actual cached Bevy0.19.1 app rlib passes with minimal Driving/ViewerCamera stand-ins; first attempt lacked Cargo's manifest environment, corrected on retry. Naga from the actual app build parses and validates the compute entry point. No Cargo dependency/project source was changed for these probes. This is not full-app typechecking, linking, GPU dispatch/readback or a temporal fix. Shader insertion, real-world metadata/error/completion gates and GPU measurements remain required. Private temporal-draft-checks01.json records exact scope and hashes.

Final3362 TAA straight-chase control rendered600ticks and30virtual30FPS motion images without GPU errors. Compared native frames0/20/29 to SMAA: TAA softens the background and thin detail; these near-constant chase-car views do not establish a reproducible whole-body trail. A side-view pair is being collected before drawing a cause/conclusion. No AA default change.

## Owner preference and fresh orbit controls

Owner observes that TAA is very soft, still playable and reasonably close to the original; prioritizes trying DLAA for clarity. Preserve TAA as a comparison/optional mode, rather than spend the next stage on endless TAA tuning. Keep SMAA fallback. Validate essential motion/depth inputs alongside the optional DLAA integration, without requiring complete original-shader reconstruction first.

Current3362 controls: straight and fixed side views each600ticks/30motion frames, plus existing --motion-orbit-rate0.4 on1200ticks with a30FPS sequence at1100..1162. All GPU runs clean; orbit SMAA/TAA physical exits exactly match4records. Native frame0/20/29 comparisons inspected: TAA softens road/trees, changes fine wheel appearance and leaves some blurred local edges during orbit. These views do not establish a numerical cause or reproduce a dramatic several-car trail conclusively. Do not relabel SMAA fallback as a repaired temporal renderer. Private orbit-controls01.json/orbit-identity01.json and figures preserve evidence.

Hardened private audit draft now also checks resource/header/finite samples, keeps the GPU-written frame ID tied to frozen metadata, restricts evidence to ignored local/, refuses overwrite, and places the bounded ROI around the projected body. Metadata-only cached-Bevy typecheck03 passes. Still requires real application wiring, capture completion/error gate, GPU execution and numerical validation before any claim of working measurement.
