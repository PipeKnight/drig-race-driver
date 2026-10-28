# Visual quality and future renderer interfaces

Owner direction, 15 October 2026: preserve GRID 2008 atmosphere while improving the actual scene substantially. Native 3840×2160 output alone does not meet this goal. Prioritize image quality; optimize after the scene looks convincing.

## Acceptance scenes

Keep identical camera, exposure, scene state and output size for comparisons. Retain private full-resolution images, plus 1:1 crops; resizing previews conceals defects. Check stationary views and motion separately.

| Subject | Required result | Check |
| --- | --- | --- |
| Tyres and rims | Smooth circular silhouette, stable spokes and sensible surface detail | Close side and oblique views; steering and rotation; inspect geometry before replacing it |
| Paint, glass and metal | Distinct light response, coherent reflections and readable body shape | Same car in sun and shade, close and chase cameras |
| Road and kerbs | Correct-scale detail without blurred markings or repeating artificial noise | Near-ground still, chase view and motion |
| Signs and cockpit | Legible source content without invented lettering or excessive sharpening | 1:1 crops at intended viewing distance |
| Thin scenery | Stable edges without obvious crawling or disappearing wires/fences | Slow lateral camera motion, then normal driving |
| Shadows and AO | Stable contact and shape, no strong halos or detached wheels | Wheel arches, barriers and distant cascades |
| Whole image | Clear detail and GRID atmosphere, no objectionable blur, aliasing or exposure pumping | Fixed comparison views and a complete Donington lap |

Status update,22 October2026: acceptance is still incomplete. Enhanced supplies source-sky lighting, layered materials, shadows and optional moving scene reflections. Tyre silhouettes have authored refinement; wheel spin presentation and opt-in rotational filtering have been implemented and iterated after owner feedback. Strong wheel smoothing was rejected for loss of spoke detail; see [WHEEL_ROTATION_FILTERING.md](WHEEL_ROTATION_FILTERING.md) for the actual current experiment and limits. The30/60/144/300FPS acceptance requirement remains; virtual-clock probes do not establish measured monitor performance.

Status update, 27 October 2026: reproduced TAA whole-car trails led to the SMAA Ultra fallback/default. Optional native DLAA clarity is owner-approved. G222/G223's gentle Motion DLAA profile is the preferred graphics reference: the owner accepted more uniform moving-road detail and a sharp car. Occasional small trails directly under the car in side views remain explicitly deferred as low priority; antenna/thin-feature, smoke and broader temporal acceptance remain open. The earlier strongly blurred Asphalt RGB-filter comparison remains rejected, rather than describing the accepted Motion profile. Sampled motion/depth controls and virtual-clock runs do not prove every scene pixel or measured monitor performance. See [NATIVE_DLAA.md](NATIVE_DLAA.md), [ANTENNA_TEMPORAL.md](ANTENNA_TEMPORAL.md), [ASPHALT_TEMPORAL_DETAIL.md](ASPHALT_TEMPORAL_DETAIL.md), [G222](../log/G0222.md) and [G223](../log/G0223.md).

Owner-approved Fresnel appearance is retained. G237 identified stale CPU transparent-queue centres; that audit alone was not a contour fix or fragment-level causal proof. G299 later verified visibly cracked front glass followed by intact glass and restored body after ordinary R in one bounded Lacetti/Donington native-DLAA capture. This does not establish unforced Manual C/R, every damage state, car or camera. See [G237](../log/G0237.md) and [G299](../log/G0299.md).

G301's two matched static rear views show a stronger saturated red with optional Reinhard than AgX, with wider scene colour/contrast changes. AgX remains the default. These SMAA display comparisons do not establish original GRID colour parity, broader lighting or driving acceptance; one float16 component differs between existing probe readbacks, so strict probe radiometric parity is not claimed. See [G301](../log/G0301.md) and [ASPHALT_PAINT_REFLECTION_CHECK.md](ASPHALT_PAINT_REFLECTION_CHECK.md).

## DLSS paths and remaining preparation

Native-resolution DLAA is integrated as an optional feature through Bevy0.19.1/dlss_wgpu4.0.0 with pinned NVIDIA SDK/DLL310.5.3.0; actual native GPU runs and resource checks are documented in [NATIVE_DLAA.md](NATIVE_DLAA.md). It is not evidence that Super Resolution upscaling, Frame Generation or Ray Reconstruction is implemented. Keep native rendering and the optional reconstruction path. Separate internal render resolution from output/HUD resolution. Preserve camera jitter, current/previous view-projection, linear depth, exposure, scene color and object/deformation motion. Reset history on cuts, teleport/reset, resolution changes and invalid scene history. Wheels need previous rotation; dents need previous vertex positions, not only a previous rigid transform.

Motion/depth and paired HDR readbacks now support bounded diagnostics of motion vectors, disocclusion, reset and exposure. Maintain these checks when changing the integration; they do not prove every scene pixel correct. Existing native validation uses Vulkan/RTX4070; it does not establish another backend/device or a future SDK release. Revalidate resource access, synchronization and lifetimes for each additional path. Treat Super Resolution, Frame Generation and Ray Reconstruction as separate integrations with separate resource contracts and support checks. Keep UI at output resolution and retain a HUD-free scene buffer for later features.

Official research reviewed on 15 October 2026: [NVIDIA Streamline programming guide](https://github.com/NVIDIA-RTX/Streamline/blob/main/docs/ProgrammingGuide.md) currently labels itself version 2.14.1, describes Vulkan device integration and runtime feature requirements. This is documentation, not proof that our Bevy build can use a particular DLSS feature. See also [NVIDIA DLSS integration overview](https://developer.nvidia.com/blog/how-to-integrate-nvidia-dlss-4-into-your-game-with-nvidia-streamline/). SDK support must be verified for the chosen release and rendering API.

## RTX preparation

Ray tracing is also future work. Maintain stable scene-instance identities, mesh/material ownership and dynamic geometry history so acceleration structures can represent static Donington and moving/deforming cars. Materials need correct normals, roughness, layered paint, transparency and coherent sky/light radiance. Research backend access and acceleration-structure support before selecting an implementation.

First candidates are car reflections and contact shadows, evaluated against raster/probe equivalents. Full path tracing is a separate later experiment. Denoising needs its own depth/normal/motion/history contract; DLSS Ray Reconstruction must not be assumed to accept arbitrary buffers or replace every denoiser. See the official [Ray Reconstruction integration checklist](https://github.com/NVIDIA-RTX/Streamline/blob/main/plugins/streamline-agent-skills/skills/integrate-dlss-rr/references/integration-checklist.md).

These renderer choices complement detailed geometry and textures. Their integration does not remove the need to improve wheel silhouettes or poor source texture detail.
