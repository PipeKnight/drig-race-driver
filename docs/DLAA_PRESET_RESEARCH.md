# Supported DLAA preset hint experiment

G203, 20 October2026. Follow-up to [paired HDR audit](TEMPORAL_COLOUR_AUDIT.md) and [input contract](DLAA_INPUT_CONTRACT.md).

## Result

Requesting supported preset L considerably reduces the asphalt-detail discontinuity in the tested moving scene while retaining ordinary temporal history. Default and requested K still show the discontinuity. No albedo, paint, reflection gain, mip bias, camera/physics or sharpening change is used.

| Road ROI | Default after/input HF contrast | K hint | L hint |
| --- | ---: | ---: | ---: |
| 500,780,30,30 | 0.274 | 0.274 | 0.750 |
| 550,780,30,30 | 0.246 | 0.246 | 0.763 |
| 600,780,30,30 | 0.222 | 0.222 | 0.714 |

Native1920x1080/1800ticks/virtual30, actual frame793/tick1652. Entire pre-stage HDR packets match exactly across all three modes and G201 (SHA4cd8bafe...000cc5); raw motion/depth+metadata and four physical exits also agree. Ratios are raw-linear descriptive spatial contrast, not an image-quality score. Inspected full scenes/native HDR previews and sequence frames0/14/28: L retains much more grain away from the car and substantially weakens the abrupt fresh-detail band. Some temporal filtering remains; no perfect parity or universal ghost-free claim. Default/K differ slightly at GPU output, not labelled pixel-identical.

Requested/stored hint roundtrip succeeds (K11/L12). **The active neural weights/preset have not been queried.** These results demonstrate the effect of the requested hint in this runtime, not verification of every internal DLSS model/OTA detail. The existing model-update uncertainty remains.

## Isolated reproducible build

`tools/prepare_dlaa_preset_probe.py` accepts an installed dlss_wgpu4.0.0 directory and a new ignored-local output directory. It verifies upstream super_resolution.rs SHA339dead011e19775da9bd73d3228ca20a4cb3a55308ec2cedc254d802c0ef558, preserves original MIT/Apache licenses, copies the crate, inserts K/L-only native-DLAA hint control, and writes patch/provenance/per-command Cargo configuration. `GRID_DLSS_PRESET_HINT=K` or `L` selects a request; absent environment retains the upstream path. The patch checks parameter SetUI/GetUI roundtrip before context creation and logs active_model_verified=false.

Build with `cargo build --locked --offline --release -p grid-app --features dlaa,paint-specular --config PATH_TO_GENERATED_CONFIG`. This configuration is **per invocation**. No public Cargo manifest/lock update, global Cargo configuration, shared registry-cache modification or Bevy fork is needed. Normal/default/phone builds remain on unmodified dependencies. This tool is a research preparation route, not a new public settings UI or final upstream integration.

Private copied-wrapper release3m23s/onejob, executable SHAc9d2fd27bc5653baeaabf547a155a28ce8d0240061d158ea9d197cc31ca97945. App source is4117fff3 (G201 Rust content), plus exact wrapper patch recorded privately and reproduced by the tool. Existing DLL310.5.3.0/SHA8707e53b...1507ec unchanged. Rust1.99.0/Bevy0.19.1/wgpu29.0.4/SDK310.5.3, Python3.12.14/NumPy2.3.5/Pillow12.3.0 reused. No installs/rebase/reseed or Steam edits. Build compiled wrapper/anti_alias/internal/Bevy facade/app only, rather than rebuilding the rendering dependencies.

## Additional acceptance and limits

Combined moving probes with existing lamp reflectors/signals succeed at native3840x2160/virtual30 with forced RGBA CPU decode and native1920x1080/virtual300 with30-frame sequence. Both600tick physical exits agree, native DLAA input/output dimensions confirmed, both bounded HDR readbacks/probe readback complete;12live lamp sections attach. Full/native4K and selected high-rate frames inspected. This checks desktop RGBA upload path, not a phone GPU or hardware300FPS. Short600tick high-rate scene is not the same1800tick detail measurement; native4K is a final PNG, not a4K motion sequence.

All781resource tests/12harnesses,0failed/ignored, patched-feature workspace releaseClippy, canonicalF455/quick24/24 pass. Default workspaceClippy from G201 remains applicable to byte-unchanged app/core/unpatched cache source. Generator reproduces the compiled patch byte-for-byte; existing/output-outside-private guards reject. First wrapper compile found enum i32/u32 mismatch, second edit lost a closing brace; corrected before accepted build. First combined4K command hit the existing1080 motion-sequence limit; retried4K final PNG plus HDR, retained1080 high-rate sequence. Logs retained; no approval rejection.

Private local/graphics/dlaa-presets01 retains copied licensed crate/patch/config, provenance, exact commands, default/K/L and combined captures, raw HDR/motion/depth, analyses, checks and failures. Original game evidence and binaries stay ignored. Existing accepted Signals launcher/build remains available. This graphics branch has not yet integrated the newer main physics/audio changes; do not present an optional graphics lab build as the latest general playable build.

## Next

Prepare a separate opt-in L graphics launcher after validation, preserving existing builds. Owner live judgement can complement captures without blocking continued research. Longer4K motion, antenna trails, reset/camera-cut and smoke cases remain open. Upstreamable preset exposure with explicit settings and verified model provenance remains future integration work; do not silently patch dependency caches or switch all defaults.

## Separate optional delivery

Installed `Play GRID Graphics DLAA L.cmd` in the graphics worktree and C:/Dev/Reassembly, with immutable local/graphics/dlaa-l-2a4ea21a/grid-app.exe, pinned DLL, original license/notices/guide, wrapper MIT/Apache licenses, README and build manifest. This launcher requestsL in a setlocal environment and otherwise retains Signals options (moving1024probe, paint gain0.5, combined reflectors/live lamps). Normal temporal history; no sharpening/upscale/frame generation. All existing graphics launcher hashes preserved. It is an additional graphics-lab comparison, not a main rebuild.

Actual installed primary-path600tick nativeDLAA smoke succeeds without DLSS_SDK environment: requested/storedL12,12live lamp sections, probe readback, motion/depth and both HDR readbacks pass; no shader/error/panic. Private C:/Dev/Reassembly/local/graphics/dlaa-l-delivery-smoke01/validation.json. Owner live appearance acceptance is pending, not required to continue independent work. Existing Signals build remains available.
