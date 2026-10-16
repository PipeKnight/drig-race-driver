# Private texture override pilot

16 October2026. Experimental opaque diffuse candidates, explicitly enabled with `--texture-overrides PATH` in the enhanced graphics profile. Steam files stay unchanged. Omit the option for the original texture with identical rendering settings.

## Exact source and shared uploads

Versioned UTF-8 TSV begins `GRID_TEXTURE_OVERRIDES_V1`. Each nonempty row has three tab-separated fields: existing bound texture key, original-reference DDS path, candidate DDS path. Paths resolve relative to the manifest and must canonicalize beneath the current private data directory. The original reference must equal the live purchased-resource DDS byte-for-byte. This avoids a new hash dependency while providing exact source identity; packaging metadata separately records SHA256 provenance.

Reject duplicate/unbound keys, mismatched originals, out-of-directory files, invalid/versioned envelopes and incomplete mip payloads before changing any texture. All entries are staged before replacement. Initial v1 accepts only opaque BGRA8 with full mip chains, power-of-two dimensions<=8192,64rows and a1GiB candidate batch bound. It does not support packed-alpha/cutout/normal/mask replacement. The initial packaging tool also rejects nonopaque source alpha.

Replacement only changes the CPU DDS map. The existing texture_upload::image creates images after device format selection; no DDS image creation or upload during manifest parsing. Image diagnostics record actual typed format/dimensions/mips through that shared helper. BGRA8 is already uncompressed, so it does not require BC support. Forced-RGBA GPU validation passes on the RTX4070 with zero BC1/BC3 image assets; this is an exercised fallback path, not validation on a physical phone.

V1 intentionally supports loaded object/car base diffuse bindings only. Layered track/land materials load their own extra maps and are excluded until their whole input path can honor overrides; do not report a road upgrade by changing an unused base texture. Sky, normal maps and independent opponent livery variants need their own future integration.

## Candidate preparation

`grid-formats/examples/export_texture.rs` reads one exact named texture through the validated PSSG/DDS reader and exports only to an existing private local directory, refusing overwrite. `tools/prepare_texture_candidate.py` consumes an opaque source-reference DDS and an upscaler RGB output with explicit scale/wrap padding. It crops the central tile, preserves mean linear RGB with per-channel scale, generates the complete mip chain by linear-light averaging and writes BGRA8 DDS, PNG preview, hashes/settings JSON and one-row TSV. Every output is write-once. Wrap padding is appropriate for repeating masonry; it is not suitable for every atlas.

Independent numerical checks found about0.5%residual mean-colour error. Global means now use float64 (per-pixel calculations remain float32); the enhanced02 result still has up to0.47%residual after8bit quantization, so do not describe the correction as exact radiometric equality. The model's larger colour shift is substantially reduced; colour-preservation-check02.json records the actual residual. Pillow round-trip checks exact base RGB/opaque alpha; runtime checks full byte count, channel layout, plane/cube flags and mip completeness. Original source alpha stays untouched.

## Current evidence and limits

Private local/graphics/texture-pilot01 contains source exports/provenance, inference logs, equal-scale comparisons and candidates. Actual Real-ESRGAN generalx4plus works on RTX4070. Asphalt over-smoothing is rejected. Masonry enhanced02 passes a same-camera native4K in-engine comparison and remains an explicitly enabled one-material pilot. Cabin edges improve but some weave is erased, so no whole-atlas acceptance. No inference quality ranking versus Gigapixel/Upscayl; no recovered-ground-truth claim.

First app Clippy caught the pinned Rust preference for as_chunks over constant chunks_exact; corrected. Changed-package alltargetClippy passes, unit envelope boundary test passes (wrong/partial mip chain, channel layout, cubemap flag, alpha and payload size). Candidate upload, exact-source negative checks, source geometry lookup and same-camera native4K original/candidate/forcedRGBA checks pass. CPU geometry bounds are research diagnostics only, not changes to any driving camera.

## Native 4K pilot result

Source c89eede39575 / executable SHA256 ea38ab0d9d46bb55823fd6f233178473b1c0f9dfd39e95f90dab1898af598fe4. No new dependencies; Rust1.99.0, Bevy0.19.1, NVIDIA Vulkan616.92. Actual bound key `objectstextures.pssg#building_a_03_d.tga`, material `objects/building_a_03`, part `!B1`. The first viewpoint looked through the back of the wall and was rejected as comparison evidence; the exterior camera (-590.665,-11.5,-55.2), target (-590.665,-11.9527,-51.3514) shows the actual masonry.

Original, enhanced and forced-RGBA captures at3840x2160 all exit0. Enhanced upload reports Bgra8UnormSrgb1024x1024/11mips through the shared after-device-selection helper. Forced-RGBA reports0BC1/BC3 assets and518decoded images. Both enhanced images inspected; the native wall crop is byte-identical between native-BC and forced-RGBA paths. Original/enhanced wall ROI mean absolute RGB difference4.286byte units; independent unchanged-door ROI0.0000563(max1). This supports a localized texture change, not a whole-track image-quality score. The full enhanced/fallback image differs by0.02037mean byte units because other original compressed textures use different decode paths; no exact whole-image parity claim.

Private `local/graphics/texture-pilot01/validation-native4k01.json` records source hashes, ROI bounds and differences. Captures `masonry-original-4k01.png`, `masonry-enhanced-4k01.png`, `masonry-enhanced-rgba4k01.png` and `masonry-native4k-compare01.png` retain full evidence. `bad-source.log` and `bad-mips.log` prove rejection of a mismatched original and a truncated mip payload before GPU startup. Steam remains untouched. Far-distance/moving/mip-transition acceptance and a wider selection of prominent materials remain future checks; asphalt/cabin rejection decisions are unchanged.

Full publication checks:433tests/0failed/0ignored in10harnesses, workspace alltarget release Clippy pass1.11s, exact solo statea4a35f899a490c36 and fresh quicksoak12scenarios/60000ticks/0failed/2.5min. First solo run incorrectly enabled deformation unlike the baseline; retained as `solo-texture-overrides01.log`, corrected configuration repeat02 matches exact baseline text. Headless executables reused only after confirming their grid-tools/grid-world sources equal0a2ff45. A24m native4K RGBA comparison renders cleanly, but billboards occlude the wall, so it is not mip-transition acceptance. Roof/chimney remain visible; two captures/logs are retained.
