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

Status: acceptance not yet achieved. Current enhanced prototype supplies sky, lighting, shadows, filtering and temporal AA, but materials, texture layers and reflections remain incomplete. Wheel roundness has not yet been quantitatively audited. Temporal AA is not evidence of correct motion vectors for all deformed meshes.

## DLSS preparation

DLSS is a future explicit feature, not installed or integrated. Keep native rendering and an optional reconstruction path. Separate internal render resolution from output/HUD resolution. Preserve camera jitter, current/previous view-projection, linear depth, exposure, scene color and object/deformation motion. Reset history on cuts, teleport/reset, resolution changes and invalid scene history. Wheels need previous rotation; dents need previous vertex positions, not only a previous rigid transform.

Before SDK integration, add debug views and deterministic tests for motion vectors, disocclusion, reset and exposure. Choose and pin an SDK release only after proving Bevy/wgpu native Vulkan or DX12 resource access, synchronization and lifetimes. Treat Super Resolution, Frame Generation and Ray Reconstruction as separate integrations with separate resource contracts and support checks. Keep UI at output resolution and retain a HUD-free scene buffer for later features.

Official research reviewed on 15 October 2026: [NVIDIA Streamline programming guide](https://github.com/NVIDIA-RTX/Streamline/blob/main/docs/ProgrammingGuide.md) currently labels itself version 2.14.1, describes Vulkan device integration and runtime feature requirements. This is documentation, not proof that our Bevy build can use a particular DLSS feature. See also [NVIDIA DLSS integration overview](https://developer.nvidia.com/blog/how-to-integrate-nvidia-dlss-4-into-your-game-with-nvidia-streamline/). SDK support must be verified for the chosen release and rendering API.

## RTX preparation

Ray tracing is also future work. Maintain stable scene-instance identities, mesh/material ownership and dynamic geometry history so acceleration structures can represent static Donington and moving/deforming cars. Materials need correct normals, roughness, layered paint, transparency and coherent sky/light radiance. Research backend access and acceleration-structure support before selecting an implementation.

First candidates are car reflections and contact shadows, evaluated against raster/probe equivalents. Full path tracing is a separate later experiment. Denoising needs its own depth/normal/motion/history contract; DLSS Ray Reconstruction must not be assumed to accept arbitrary buffers or replace every denoiser. See the official [Ray Reconstruction integration checklist](https://github.com/NVIDIA-RTX/Streamline/blob/main/plugins/streamline-agent-skills/skills/integrate-dlss-rr/references/integration-checklist.md).

These renderer choices complement detailed geometry and textures. Their integration does not remove the need to improve wheel silhouettes or poor source texture detail.
