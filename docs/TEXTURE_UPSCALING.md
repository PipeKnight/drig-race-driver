# Texture enhancement workflow

16 October 2026 (Europe/Kiev), owner's question: no Topaz Gigapixel purchase/installation is required to continue. Start with available local tooling and small controlled comparisons; consider a commercial tool only if it demonstrates a useful improvement on our actual textures. Upscaling is not yet validated in the prototype.

## Tools checked and prepared

PATH, registered Windows uninstall entries, Program Files, local Programs, owner Tools/grid-research and the primary local/tools were checked for Topaz, Upscayl, Real-ESRGAN, chaiNNer, ImageMagick, texconv and Compressonator; no matching existing tool was found in those scopes. This is a scoped inventory, not proof of absence anywhere on the disk. Existing Python3.12.14/Pillow/NumPy and the PSSG/DDS readers remain reusable.

Prepared private portable **Real-ESRGAN v0.2.5.0, ncnn Vulkan Windows20220424**, official45,474,481byte release asset. Installed under this graphics checkout's ignored `local/tools/realesrgan-20220424`; no PATH/global package/game changes. Archive entries validated to remain under the private destination before extraction. Command help works. Bundled general `realesrgan-x4plus` and anime models inventoried; no default-anime assumption for realistic track textures. RTX4070/Vulkan is already verified by the renderer; upscaler inference on GRID textures remains untested.

ArchiveSHA256 `abc02804e17982a3be33675e4d471e91ea374e65b70167abc09e31acb412802d`; exeSHA256 `07e49f7cbb4ede01ae4dd4c399d3a7e5846e3d2085c3128eff881e55cb7b1a0c`. Official release API supplied no digest; these are locally recorded provenance hashes, not comparisons to a publisher checksum. Executable signature reports NotSigned. Private metadata/help/model inventory under `local/graphics/upscaler-*`. Installation/help is not successful image restoration or quality validation.

Official sources checked: [portable implementation and CLI options](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan), [project and portable release link](https://github.com/xinntao/Real-ESRGAN), [Upscayl custom-model workflow](https://github.com/upscayl/upscayl/wiki/Guide), [Gigapixel requirements](https://docs.topazlabs.com/topaz-gigapixel/system-requirements). Tool/model names and versions must remain explicit in every comparison.

## Asset-specific treatment

1. Select a small pilot of road/kerb/concrete, car livery/interior and a sign. Use actual visible bindings and UV scale; do not upscale unused catalogue placeholders. Preserve source hashes and image size/format/channel/mip metadata.
2. For color diffuse textures, compare a conventional reconstruction control with a suitable neural 2x/4x result. Choose per surface, inspecting at1:1 and in motion. Preserve color and physical texture scale; reject invented features, excessive denoising, sharpening halos and broken edge tiling.
3. Treat text/logos separately: verify letters and geometry against the original; reconstruct readable vector/graphic elements where justified, rather than accepting invented lettering. Preserve atlas layout/padding and UV correspondence.
4. Alpha/cutout/masks are data, not photographs. Preserve coverage, thresholds, channel meaning and atlas borders. Do not run every packed channel through the same photographic model. Normal maps need vector-aware filtering/renormalization and correct linear sampling/tangent convention; stronger detail is authored new information, not recovered ground truth.
5. Produce complete mip chains in the correct color space and choose appropriate color/normal/mask GPU formats. Verify texture upload, sampling, UV seams, atlas bleed and alpha coverage at several distances. Any converted/upscaled original remains private.
6. Add a local override manifest mapping original resource/texture IDs to derivative hashes and settings, with an Original/Enhanced comparison. Test the same native4K viewpoints and driving path. No global game-install replacement and no distribution of original or derived proprietary textures.

Topaz remains an optional later comparison tool. A particular model's advantage cannot be inferred from its marketing, price or output dimensions. First outcome needed: visibly better pilot textures in our renderer, with correct mipmaps and stable edges; then expand to20–30high-impact resources.
