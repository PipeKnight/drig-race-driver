# Original car normal and scratch masks

15 October 2026, static purchased-resource research. Original program bytecode/disassembly and metadata remain in ignored `local/graphics/`; source code in `tools/` is independently authored diagnostics.

## Selected programs and linkage

The installed game uses `shaderpack/shaderpack.pssg`, not loose car shader files. The first search incorrectly used plural `shaderpacks`, then the correct packed resource was identified. Full pack is4,554,507bytes:366shader groups and1121programs;119program IDs include car/paint/glass. The exploratory survey initially tried to traverse `SHADERPROGRAMBINARYCODEBLOCK` as nodes and failed on a shader-version word. Recognizing that explicitly named raw-data node makes the complete bounded tree parse; no generic error swallowing or schema guessing was added.

New `tools/disassemble_packed_grid_shader.py` selects one exact program, verifies source stability/code type/binary size, and calls the already installed Microsoft `d3dcompiler_47.dll`10.0.26100.9568 through the existing disassembler. Outputs and hash/parameter sidecars must stay private and refuse overwrite. No installation or runtime injection.

The saved Lacetti `Car Body Paint`, rims and several metal materials reference `carpaint_metallic.fx`. Shaderpack group `carpaint_metallic` passConfigMask1 links the paint/normal/scratch fragment variant and the palette vertex variant. This establishes a saved matching pass, not the live runtime's selected quality/config mask. Other passes include depth/velocity, shadows, AO, cheap and reflection variants.

Private evidence: `shaderpack-survey01.json`, `car-program-inventory01.json`, `shaders/car-paint-normal-scratches01.txt` and sidecar (2736byte pixel program), `shaders/car-paint-vertex01.txt` and sidecar (988byte vertex program).

## Static arithmetic observations

In the selected vertex variant, input vertex color red and green are inverted into output color controls; the blue control comes from clamped dirt level. In the matching pixel variant:

- Normal RGB is decoded around the neutral tangent-space normal, blended using the first interpolated control, then normalized and transformed by the supplied tangent/binormal/normal basis.
- Scratch RGB blends with diffuse color using scratch alpha multiplied by the second interpolated control. It also reduces a reflective contribution. Scratch alpha is not a conventional standalone roughness map.
- Dirt has its own texture/color/control path. A shadow mask and the environment cubemap also contribute. Reflection lookup flips Z and divides sampled cubemap RGB by alpha, so the original environment texture is not necessarily an ordinary LDR RGB cube.

With original normalized ARGB interpretation, the selected path's normal strength is `1 - vertex_red`, scratch control `1 - vertex_green`. White input suppresses both; red0/green255 permits normals while suppressing scratches. This explains why simply binding a crumpled normal map everywhere is wrong. Runtime damage mutation, exact backend channel conversion and all shader variants are not yet verified. Do not label a static texture as permanently active damage or assign it to all undamaged paint.

## Actual saved Lacetti colors

`tools/audit_car_shader_masks.py` resolves LOD0 draw->render source->Color stream->draw vertex range. All98selected draws have one `uint_color_argb` stream. Evidence `lacetti-draw-color-masks01.json`, plus exploratory `lacetti-color-streams01.json`/`lacetti-color-values01.json`. First audit failed because an exploratory four-byte zero also has an empty-string preview; layout fields now prefer the integer interpretation. No file was written by the failed run.

Observed draw-range vertex occurrences (not unique global vertices): paint10074white,179red0/green255 and32alpha254/RGBwhite; rims4784white; tyre walls880white; tread720red0/green255. Many metal/cabin/light groups use red0/green255; aerials have grayscale variation. This is not enough to assume every group uses the paint shader or to ignore saved `NormalMapScale` and pass selection.

Current Rust mesh reader discards Color/Tangent/Binormal streams. Bevy vertex color normally modulates base color, so copying the original damage controls into its default color attribute would visibly tint the car. A faithful material path needs separate mask attributes plus the correct original tangent basis (or a validated equivalent), and a damage update contract with the engine workstream. Start with known fully enabled tread/lighting regions only after verifying their shader and tangent convention; retain original colors for body damage/scratch work.

Next: validate tangent/binormal handedness and original mask defaults per actual material pass; carry mask data separately from albedo; then implement selective normal/detail mapping. Preserve the current playable graphics release while this research remains offline.
