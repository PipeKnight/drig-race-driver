# Optional native-resolution DLAA

19 October2026. Owner prioritizes a clearer DLAA experiment; TAA is considered soft but playable/close to the original. Installed graphics3362/c1052 still uses SMAA Ultra. This document starts with source/typecheck evidence, not a claim DLAA renders yet.

## Implementation candidate

Optional grid-app feature `dlaa = ["bevy/dlss"]`, built alongside `paint-specular` for the dedicated experiment. Bevy0.19.1 selects dlss_wgpu4.0.0/wgpu29 and SDK310.5.3. Ordinary/phone builds do not enable the feature. New lock entries include Bevy's weak optional Solari metadata; cargo tree shows no active bevy_solari dependency for this feature set. No ray-tracing pipeline was enabled.

`--aa dlaa` requires enhanced graphics and a feature-enabled native build. The main ViewerCamera receives DlssPerfQualityMode::Dlaa only when Bevy reports SDK/device support. Otherwise it receives SMAA Ultra and logs that effective fallback. Menus and probe/shadow cameras do not receive DLAA. A stable authored custom-engine project UUID is inserted before DlssInitPlugin in every shared DefaultPlugins group, including menus; no account credential or registered numeric NVIDIA application ID is claimed.

Initial history resets explicitly; subsequent camera-view changes, decreasing physics ticks/restarts, large view jumps and rotations request reset. No CAS/sharpening, exposure, material, RGB texture, camera model, simulation or frame-generation changes. Bevy handles jitter, inverted depth and motion-vector conversion. A render-world readiness check requires equal input/output sizes and actual depth/motion prepasses after the completed render schedule; offscreen successful PNG exit waits for this state. Actual GPU readback/moving frames and clean native completion are still required before acceptance. Context/dispatch failures remain test failures; startup support fallback is not a promise to recover every driver failure.

## Reused native tools

Private SDK310.5.3 commit982b0d19f9e35fef8e1b3109efa6b95470563866; release DLL SHA8707e53b26c68c606b98bf31c223485ff30d310a261b1a36d48b2eaabc1507ec,54,779,504bytes, validNVIDIA signature. Existing NDK30.0.16248370/libclang21.0.0, staged Vulkan1.4.335 headers, MSVC14.44.35207 and Windows SDK10.0.26100.0. Process-local SDK/include/PATH settings only; no new LLVM/Vulkan SDK installation. SDK/DLLs/licenses/captures stay ignored under local/.

First offline Cargo resolution lacked bevy_solari index metadata. Online resolution downloaded that pinned0.19.1 package and resolved the new bridge/bindgen dependencies; existing dependency versions were not upgraded (lockfile disambiguates duplicate itertools/shlex/rustc-hash versions). Real grid-app all-target releaseClippy with paint-specular,dlaa passes in1m04s including native wrapper compilation. This is not final linking, initialization or DLAA image quality. Logs under local/graphics/dlaa01 plus the initial lacquer01 offline-resolution log.

## Next verification

Build and archive an exact source-stamped optional executable with the pinned DLL/license alongside it; keep accepted3362 separate. Run Vulkan/RTX static and fixed-clock driving controls with SMAA/TAA/DLAA using unchanged materials/light and no sharpening. Verify the log actually selects/prepares native DLAA and both sizes match; a fallback PNG is not a DLAA result. Collect bounded raw motion/depth and frozen-matrix data at30 and300virtualFPS. Check restarts/cuts and an unsupported-backend fallback. Full resource tests/default+feature Clippy, fresh merged-core canonical/quick evidence and guarded separate delivery follow actual runtime acceptance. No performance optimization or quality claim before these results.

Local authoritative sources: installed Bevy0.19.1 anti_alias/dlss/{mod,prepare,node}.rs, bevy_internal/default_plugins.rs, and dlss_wgpu4.0.0 README/build.rs. The bridge's [pinned README](https://docs.rs/crate/dlss_wgpu/4.0.0) specifies the SDK/DLL setup; [NVIDIA SDK310.5.3](https://github.com/NVIDIA/DLSS/tree/v310.5.3) includes its license and programming guide. No vendor source or binaries are committed.
