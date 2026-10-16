# Original GRID image formation: Donington research

15 October 2026, graphics worktree `codex/grid-graphics`. Purchased inputs are read-only. Original shader bytecode, assembly, decoded configuration trees and texture surveys remain under ignored `local/graphics/`.

## Observed original resources

Donington has its own `sky.pssg`: 1,074,101 bytes, one static draw, 265 vertices, 480 triangles, an explicit UV stream, and a 4096 x 512 BC1 texture. The sky material saves an exposure scalar of 1.0. Its original geometry extends about 17.3 km sideways; it cannot simply be rendered as an ordinary finite scene mesh under the viewer's default far plane. The mesh and saved UVs are preserved rather than guessing a spherical projection of the panorama.

The track's text `lighting.xml` contains distinct sunlight/backlight/ambient colors, sky controls, fog, shadow/AO and specular controls. These establish that the original art direction includes colored lighting as well as postprocessing. The units/coordinate convention of its sun angles and the exact production/consumption of these parameters have not been recovered; the Bevy profile still uses authored light orientation and intensity. Reading the XML alone is not a validated lighting port.

## Original postprocessing shaders

Installed Windows `C:/Windows/System32/d3dcompiler_47.dll` is Microsoft-signed (signature verified), version 10.0.26100.9568. `tools/disassemble_grid_shader.py` calls its read-only D3DDisassemble API. No new tool installation, original process injection or original executable modification was needed. Three shader-model-3 pixel programs disassembled successfully; complete text and hashes stay private under `local/graphics/shaders/`.

Observations from these programs:

- The examined tone-scale pass scales scene RGB by a supplied key divided by sampled luminance plus an epsilon, then clamps the result to an HDR range. This pass by itself is not AgX or an ACES-style display transform.
- A color-balance merge combines scene and bloom, interpolates color against a luminance scalar for saturation, applies a supplied RGB tint, optionally blends toward a vignette color using a smooth radial weight, then applies supplied contrast/brightness, nonnegative/maximum bounds and reciprocal gamma.
- Another final merge combines up to four weighted images with an exposure-related scale on the scene input.

These are static observations, not proof that all three programs execute in the owner's current Donington race. No original runtime color coefficients, selected quality profile, render-target formats or pass ordering have been captured yet. The first enhanced profile's warm grading remains authored, and must not be called the original GRID grade.

Postprocess configuration files mix plain XML with GRID binary XML despite sharing `.xml` extensions. The first decoder attempt incorrectly treated every file as binary and stopped at an assertion; no report was written. The corrected read-only pass selects the existing binary decoder by magic and otherwise uses XML parsing. Private `postprocess-config01.json` holds all 13 decoded profiles/configuration files. Type1-4 reference tone/luminance/bloom/softening and various blur/final-merge blocks. Type5 is a simple copy path. Selection of the live original profile remains open. An earlier `Get-Content` of Type5 emitted binary text; future inspection must check framing first.

## Source sky implementation

`grid-app/src/sky.rs` reads the sky PSSG using existing format readers and resolves its saved diffuse binding. `sky.wgsl` is independently authored. It preserves projected source geometry/UVs while placing clip depth at the reversed-Z background plane. The sky is drawn as a first transparency with opaque pixel alpha, no depth prepass participation and no shadow casting/receiving. It follows camera translation; camera rotation continues to expose different parts of the original sky.

The `--sky source|procedural` selector applies to the enhanced Donington/reference/driving views. Source sky is the new enhanced default; procedural mode retains the first experiment. The original sky affects only the visible background for now: generated lighting/reflections are still the procedural atmosphere approximation. This mismatch is explicit pending source-sky environment probes. Original sky gamma/scale/shader exposure semantics are not yet reproduced; original textures pass through our current color pipeline.

First private pilot `source-sky01.png` rendered at 3840 x 2160 without GPU errors and was inspected: clouds and distant horizon appear, foreground structures/car/track remain visible. This pilot uses dirty source metadata. Final build, camera/drive/depth checks and source hashes will be recorded in the graphics journal after validation.

## Partial mip chains

Seven Donington edge textures declare three mip levels after the base and contain exactly the BC bytes for four levels: 256 x 512 BC3, 174,080 bytes. The previous reader required the chain to continue to 1 x 1, so these fell back to base-only. The reader now accepts exact count/size-validated partial chains in addition to complete chains. Texture bytes are preserved; no mip generation or speculative padding removal occurs. Counts beyond the natural dimensions, overflowed counts, truncated tails and unknown extra bytes retain the base-only fallback.

Next: validate the updated DDS chains on actual resources/GPU; inspect source sky while moving and with glass in frame; establish live postprocess/profile coefficients without interfering with the other agent's original-game session; classify material maps and connect source-sky reflections.

## Further static material/color evidence

Donington's own `effects.xml` explicitly defines an exterior effect on Type2, a separate interior effect and a transition effect. It includes numerical saturation/tint/contrast/gamma/vignette controls, so a static track-specific grade can be read from purchased resources. The exterior config uses the literal misspelled key `brighness`; a reader expecting only `brightness` would lose it. This is a saved configuration observation, not proof of the live runtime's merged coefficients. Final shader parameter preparation (including tint premultiplication and vignette shape conversion) still needs tracing.

The existing private original texture-tuple table identifies `_main_sc` as **TScratchMap**, not TSpecularMap. Consequently the filename suffix must not be treated as a conventional roughness/specular map. The same table associates `_main_nm`, `_lights_nm` and `_tread_tm_nm` with TNormalMap and explicit material names. Private decoded channel observations (`lacetti-channel-audit01.json`) show RGB BC1 normals, not DXT5nm alpha/green encoding for the examined main/lights maps. The main normal image contains conspicuous crumpled regions; its damage-related sampling/blending remains uncertain, so blindly assigning it to an undamaged painted car is not justified. Texture-channel previews and scratch data remain private.
