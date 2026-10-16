# Local reflection experiment

16 October 2026: next agreed graphics-roadmap step after the selected Donington surface/infield layers. Existing source sky IBL remains the verified default. No change to camera backlog or engine/physics scope.

## Installed implementation evidence

Bevy0.19.1 ssr/mod.rs requires deferred rendering plus depth/deferred prepasses (components alone do not switch the opaque renderer). It reflects screen-visible geometry and loses off-screen content. Therefore this experiment does not replace the planned scene cubemap/probe solution for a car moving around a full lap. It is not ray tracing or DLSS.

Installed deferred/pbr_deferred_functions.wgsl stores clearcoat strength and roughness in quantized G-buffer fields and reconstructs the coat normal from the main normal. Unlike an early suspicion, clearcoat IS represented in this version; not every forward material property is preserved (for example colored specular tint). Must compare deferred-only against forward separately, then SSR against the same deferred control. Glass stays in the forward transparency path, so do not claim this experiment adds SSR to all windows.

Public DeferredPrepass is bevy::core_pipeline::prepass::DeferredPrepass as shown by the pinned deferred_rendering example. Initial pbr root/private-module guesses failed compilation and were corrected; app release all-target Clippy then passed.

## Opt-in source f435260

--reflections sky: existing forward source/procedural sky IBL, default unchanged.
--reflections deferred: deferred opaque rendering without SSR; comparison control.
--reflections ssr: same deferred path plus ScreenSpaceReflections,32linear steps/8bisection steps and edge fade0.02..0.12; other defaults retained. Enhanced profile required.

The road ExtendedMaterial now provides a deferred fragment output using exactly the same layer composition, alongside the existing forward fragment. Lighting remains Bevy's. No default launcher switch before visual validation. No capture is accepted from process exit alone: inspect all logs for shader/GPU errors and view the resulting images.

## Required validation

Same native4K camera: forward, deferred-only, SSR. Check body paint and rims near garage/advertising, missing geometry, preserved road layers/clearcoat/tread detail, reflection edges and movement. Then240tick control motion comparison. A visible reflection improvement is not a full-lap stability or original-fidelity claim. Proprietary reference/captures stay private. Next beyond this experiment: scene probes/cubemap capture with correct orientation/radiometry and then dynamic neighbours, preserving the established sky environment.

## First GPU observations

f435260 native4K quarter01 forward/deferred/SSR captures all completed withoutshader/GPUerrors; all343selectedsurface draws attached and skyfiltering ready. Images inspected: SSR changes body response and creates strong dark details near the mirror/door. These are suspicious screen-space self-hit artifacts, not yet identified as faithful reflected objects. Do not promote to default based on enablement alone. SSR240tick drive01 also exits cleanly and has the same final pose/velocity/handling as the previous control.

Private reflections-ssr-difference01 compares same-source SSR/deferred images: full-frame meanabsolute1.70238/max139, car-bbox mean3.08034/83261pixels over20. This is a mixed-region pixel difference, not an isolated material measure or quality score; temporal/other-pass differences may contribute.

316cb56 adds bounded --ssr-thickness0.005..1, provisional default0.05 versus Bevy's0.25. Smaller assumed thickness is a hypothesis to reduce false thin-panel intersections; same-source0.05/0.25 GPU comparison pending. Current normal launcher stays forward/sky. Local scene-probe/cubemap reflections remain the main next candidate for off-screen surroundings.

## Thickness comparison and release316cb566a45a

Same-source native4K thin/thick quarter02 (0.05/0.25) both viewed/error-free. Some dark details change, but neither proves a general quality improvement. Mixed-frame meanabsolute difference0.79070/max84, car-bbox1.09227. Repeating thickness0.25 identically produces meanabsolute0.60881/max87, car-bbox0.51107; repeat variation is substantial, so attributing the whole first difference to thickness is unjustified. Self-hit/temporal stability remains unresolved. Keep SSR experimental and the normal launcher on forward source-sky IBL.

Final normal-profile native4K graphics-default-final02 inspected; default240tickdrive02 clean, same final motion/handling as infield control. All343surfaces/94materials remain attached. App all-target release Clippy passes; resource/mask tests passed at eaa8146 and that reader/mask is unchanged. Private validation-reflections02/current-build stores per-image hashes, dimensions, experiment status and exeSHA256520753cbddd3eb882b42b3993fa11f274e2fa3eea4cdd9bc346c56fedc1facba. Verified executable backed up privately; no primary/Steam changes. No manual full-lap temporal validation or default SSR promotion.

## Next bounded reflection milestone

Capture a local HDR scene cubemap near Lacetti, retaining the source sky and reconstructed road layers; exclude the player's own car from the probe render while preserving its main-view shadows. Validate six face orientations against the existing Bevy handedness convention, linear/HDR format and exposure/radiometry before filtering with the existing GPU environment generator. Blend a bounded local probe with source-sky fallback and check an advertisement/building reflection whose object is outside the main view. Start with a static reference probe, then update the capture position/faces and include neighbours with explicit history/reset rules. Do not use a tonemapped PNG as if it were physical HDR radiance. SSR can later supplement probe reflections after its temporal artifacts are understood.

16 October update: static HDR scene capture now passes six-face raw readback, native4K mirror/control and 256/512/1024 resolution checks. Recognizable offscreen garages and advertisements are verified on a diagnostic mirror. Nonuniform probe-transform angular stretching was corrected using an isotropic transform and explicit parallax box. SSR remains opt-in; source-sky remains normal default. Moving scene capture is drafted but not yet GPU-verified. [Detailed findings](SCENE_HDR_PROBE.md).
