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

## Initial checks

Candidate3bfc2d1 resource/mask tests2/2passed. Native4K baseline ground and sun-facing views inspected; sun-facing ground reproduces the excessive pale sheen. First unstamped app compilation was cancelled before installation and restarted with explicit source stamp. GPU after comparison and complete workspace checks remain pending. F438 source checks independently passed728tests/Clippy/canonical/24quick scenarios; those do not validate the new shader.

## Native result and remaining edge limitation

Candidate3bfc2d1/executable SHA bbc1dc552cdd0d0256033f7b08681b3b60750fdd8af85908e1eb52db5a6cd053 passes both matched native3840x2160 captures and source-mask variant without GPU errors. Main ground is visibly matte toward the sun; color and detail return without changing density or exposure. Native800x500 crops are in ground-aa01/ground-native-comparison01.png. Some mixed edge atlases retain the previous bright response and now contrast more clearly with the corrected terrain: this is not a complete solution for every grass-edge texel. Do not hide it with thicker grass or imply all terrain is fixed.

ForcedRGBA8 passed (0BC assets,537images/528decoded). This is desktop fallback validation, not an Android device claim. Paired240tick/virtual30FPS smoke tests pass with all4physical exit records identical after dropping wall_seconds. No new full-lap or30–300FPS temporal-AA acceptance claim. Whole-frame bit identity is not claimed: sky/road interior comparison has at most1RGB-code difference; the first proposed road ROI crossed a terrain/curb edge and is explicitly superseded by road-control02.json. All evidence stays under ignored local/graphics/ground-aa01/.

## SDK preparation superseding the initial inventory

Official NVIDIA DLSS v310.5.3 prepared privately at local/tools/dlss-310.5.3, pinned commit982b0d19f9e35fef8e1b3109efa6b95470563866. Release nvngx_dlss.dll54,779,504bytes, SHA8707e53b26c68c606b98bf31c223485ff30d310a261b1a36d48b2eaabc1507ec, AuthenticodeValid/NVIDIA Corporation. Retain the upstream LICENSE.txt/docs; no SDK files are public source artifacts. The checkout is sparse to avoid unrelated platform binaries/submodules.

A deeper inventory found the already installed Android NDK30.0.16248370 host libclang/clang21.0.0 (LLVM commit9f872551d3c681d06fd303b36f16ed5c274735eb), plus Vulkan1.4.335 headers. No new LLVM/full Vulkan SDK installed. Staged only vulkan/ and vk_video/ headers privately, because using the entire NDK libc include directory for the Windows host would mix Android C headers into MSVC compilation. dlss_wgpu4.0.0 build.rs only requires the Vulkan include tree, not glslc or a shader compiler. A private isolated native bridge check is pending; its existence is not proof DLAA initializes or renders correctly. Per-file hashes/tool provenance are in dlaa-toolchain01.json and vulkan-header-staging01.json.

## Final candidate checks and bridge compile result

Complete3bfc2d1 source validation:728workspace tests/19harness results,0failed/ignored, feature/default all-target workspace Clippy pass. Existing F438 canonical+24scenario quick evidence carries only to unchanged core libraries, verified by empty source diff; it is not a new candidate full-lap capture. ground-aa01/acceptance01.json separates these scopes.

Private dlss_wgpu4.0.0 cargo check passes with SDK310.5.3 and reused tools. Three setup failures were kept:01libclang could not load;02adding NDKbin to processPATH fixed its libwinpthread dependency but exposed missing wchar.h;03explicit host include roots fixed lookup but the NDK's MinGW default could not parse MSVC types.04adding --target=x86_64-pc-windows-msvc -fms-extensions -fms-compatibility succeeded. Header roots: existing MSVC14.44.35207 and Windows SDK10.0.26100.0 ucrt/shared/um/winrt, supplied through BINDGEN_EXTRA_CLANG_ARGS. No systemPATH or installed crate modified. cargo check proves generated bindings/native wrapper compilation, not final app linking, supported-device initialization, jitter/motion correctness or DLAA image quality. Next step stays motion/history diagnostics before enabling a new temporal default.
