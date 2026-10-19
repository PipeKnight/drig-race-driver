# Dry ground and anti-aliasing follow-up

19 October 2026, owner feedback from dedicated graphics manual-499-28815.log.

## Reproduction and scope

The run identifies runtime2c5f9cadd020/F427, Enhanced1920x1080, source surface layers, source sky/sun and SMAA Ultra (AA mode1). Surface alpha/F0 pilot isOff. Owner reports weak AA and sparse grass exposing a glossy, almost mirrorlike ground. Performance is another agent's work; crown detail is deferred. Do not change grass density or global exposure to hide the ground.

The layer extension clones the generic StandardMaterial (perceptual roughness0.7, nonmetallic, default reflectance0.5). It reconstructs diffuse/macro/skids but had no distinction between dry grass/earth and asphalt. Source diffuse alpha is a Phong-specular control, not opacity or a ready-made GGX roughness map. This is a plausible source of excess sheen, pending matched GPU images; it is not proof of the original game's exact material values.

## Candidate

Classify only standalone known Donington infield/outfield diffuse layers and the explicitly selected track_infield shader. Keep mixed edge atlases unclassified: they contain road/paint too. Carry the two terrain weights with each layer definition, and blend them with the same vertex red used for diffuse. Dry terrain tends toward authored perceptual roughness1.0 and reflectance0.25 (normal-incidence dielectric F0=1% from the former4%). Optional source-alpha/F0 weighting remains multiplicative. Asphalt, curb, car, grass cards, source textures, geometry, macro AO and lighting are unchanged. No extra sampler or upload path: the existing P8-compatible texture_upload helpers remain in use.

This is a dry terrain response adaptation, not a recovered source shader or wet-weather model. Infield textures embedded inside road-edge atlases remain a known limit. Native4K matched views, shader validation and a driving smoke check are required before delivery. Private evidence: local/graphics/ground-aa01/.

## TAA and DLAA

SMAA Ultra is currently default because TAA produced verified trailing copies of the moving car. Adding DLAA is not itself a cure for invalid temporal data. The existing MotionVectorPrepass gives rigid transforms, but mesh_stream rewrites current vertex buffers without separate previous deforming positions; foliage facing and dents therefore require explicit history consideration. This does not by itself explain all observed whole-car trailing.

Installed Bevy0.19.1 has anti_alias::dlss and DlssPerfQualityMode::Dlaa, gated by the optional dlss feature. Its Cargo metadata selects dlss_wgpu4.0.0. The compatible bridge targets wgpu29 and NVIDIA DLSS310.5.3, rather than the latest bridge6/wgpu30. See the [published4.0.0 README](https://docs.rs/crate/dlss_wgpu/4.0.0) and [official bridge](https://github.com/bevyengine/dlss_wgpu). Local Bevy module requires Windows/Linux Vulkan and RTX, project ID inserted before DefaultPlugins, runtime support resource check, and correct MainPassResolutionOverride handling in custom passes. This is existing integration code, not a tested GRID build.

Inventory: Rust/Cargo1.99.0, Bevy0.19.1, RTX4070/Vulkan driver616.92. vulkaninfo is installed onPATH; DLSS_SDK/VULKAN_SDK unset and clang not onPATH. That limited check does not prove no compiler/SDK exists elsewhere. No SDK downloaded or new dependencies enabled yet. Initial attempted Git tag README URL returned404; docs.rs package README supplies the pinned requirements instead. SDK license and DLL distribution requirements must be retained if this optional path is built.

Next: validate dry ground, then inspect actual velocity/depth/history in fixed-clock moving captures, including car against road, transparent glass, wheels, foliage, reset/cuts and30/300virtualFPS. Use Bevy's installed prepass/fullscreen infrastructure for a debug view. Only then compare TAA with a pinned optional DLAA path; keep SMAA fallback for unsupported hardware. NVIDIA describes native-resolution DLAA as part of the DLSS family: [official overview](https://developer.nvidia.com/rtx/dlss). No claim DLAA is currently enabled, ghosting solved, or frame generation/ray reconstruction integrated.
