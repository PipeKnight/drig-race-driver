# Texture enhancement workflow

16 October 2026 (Europe/Kiev), owner's question: no Topaz Gigapixel purchase/installation is required to continue. Start with available local tooling and small controlled comparisons; consider a commercial tool only if it demonstrates a useful improvement on our actual textures. Upscaling is not yet validated in the prototype.

Latest status: first three actual GRID RGB inferences completed on RTX4070. Asphalt candidate rejected; masonry candidate selected for a guarded in-engine experiment; cabin mixed/pattern loss. No renderer texture override applied yet. Detailed pilot results below.

## Initial tools checked and prepared (historical)

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

## Quality comparison qualification (16 October 2026)

No GRID texture inference or A/B comparison has been completed. The installed general realesrgan-x4plus model from the 20220424 portable package is an initial baseline, not a demonstrated match for current Gigapixel or Upscayl models. Choosing a free, scriptable tool does not establish equal quality.

Upscayl supports custom NCNN models; compare exact model, version and settings rather than application names. Gigapixel documents specialized Text & Shapes and general/high-fidelity models, but those descriptions are not a comparative benchmark: https://github.com/upscayl/upscayl/wiki/Guide and https://docs.topazlabs.com/topaz-gigapixel/enhancements/ai-models/core-models .

Pilot acceptance: matched original asphalt, concrete, livery/sign text and cockpit samples, identical output dimensions, inspection at 1:1 and in-engine motion. Check faithful lettering, surface structure, repeating edges, mip stability and sharpening artifacts. Preserve all inputs/settings/hashes privately. Select per texture; add a commercial candidate if its demonstrated improvement warrants it. No purchase is needed before this evidence exists, and no commitment to free tooling overrides the owner's quality-first goal.

## First actual pilot,16October2026

Existing portable tool and x4plus.bin/.param hashes rechecked against private provenance, no installation. Real inference succeeds on Vulkan device0 NVIDIA GeForceRTX4070 (IntelUHD770 isdevice1), -s4 -t256 -g0 -j1:1:1, OMP_NUM_THREADS2. First asphalt runtime9.45s. Independently authored grid-formats/examples/export_texture.rs reuses validated_dds, writes only beneath this checkout's canonical ignored local/, refuses overwrites. Actual BC3asphalt/BC1masonry/cabin exports decode with Pillow; formatalltargetClippy passes, overwrite negative check preserves exact original hash.

Pilot source/derived pixels and exact hashes remain local/graphics/texture-pilot01. Asphalt source512x512/10mips has alpha0..105: preserve separately, infer RGB only. x4plus output2048x2048 visibly over-smooths grain and creates smooth blotches; rejected before renderer integration. RGBstandard deviations fall~20to~8; downsampleRMSE18.36 vsLanczos3.48. This is an observed configuration failure, not proof that every neural model/tool fails asphalt, nor isolated model-vs-Vulkan numeric causality.

Masonry source256x256/opaquealpha ->1024x1024: inspected equal-scale crops, sharper mortar/cracks/stone structure; candidate for in-engine evaluation, not yet accepted final quality. Mean RGB shifts~(-5.57,-3.05,+0.72)bytes; preserve source radiometric/color character before judging materials. Wrap64pixel source padding inferred separately and central crop planned to respect repeating UV context. Cabin512x512/opaquealpha ->2048x2048: cleaner panel edges but some carbon/weave pattern lost; do not replace whole atlas without material-specific treatment/text checks. Preserving original patterns and letters takes precedence over output size. No universal upscaler ranking or recovered-ground-truth detail claim.

Next guarded override contract: private versioned TSV maps existing bound diffuse texture key to original-reference DDS and derivative DDS. Compare original reference byte-for-byte against the live purchased-resource DDS (strong exact identity without a new hash dependency); reject mismatches/unused keys/duplicate entries/outside-private paths before mutation. Apply all validated replacements atomically to the CPU texture byte map, then retain the existing texture_upload::image path; never instantiate/upload DDS during pre-device configuration. First derivative envelope is opaque BGRA8 with complete linear-light mip chain. Original/enhanced CLI comparison remains explicit; no Steam writes or published assets.
