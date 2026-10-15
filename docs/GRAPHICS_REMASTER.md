# Lacetti / Donington graphics workstream

15 October 2026. The owner assigned engine/physics work to another agent and asked this chat to concentrate on a substantially better image, including native 3840 x 2160 output. This is a graphics workstream for the independent Rust/Bevy engine. Advice pasted by the owner is reference material, not evidence about the current renderer or authorization to redistribute GRID assets.

Owner art direction confirmed during this run: preserve GRID 2008's atmosphere while substantially improving detail. The enhanced profile therefore starts with a restrained authored warm grade; it is not the recovered original LUT. Natural lighting must serve that art direction, rather than replacing it with an unrelated neutral showroom look.

## Scope and baseline

Isolated branch: `codex/grid-graphics`. Workspace: `C:/Users/debros/.codex/worktrees/grid-graphics/Reassembly`. Physics, AI, race rules and sound are owned by the other workstream; do not overwrite its running executable or uncommitted files.

Inspected source: Bevy pinned to 0.19.1; `grid-app/src/main.rs` uses StandardMaterial already, mostly diffuse-only source bindings, provisional roughness/metalness, one directional light at 18,000 lux, global ambient brightness 450, no explicit environment light or sky, and fixed 1280 x 800 window/capture targets. Source DDS mip chains are used where the reader validates their exact extent. This is not the original EGO renderer. A screenshot of the current race (`local/opponents/race-avoid-12.png` in the primary checkout) shows the dark empty sky, texture shimmer and limited material response.

Thus PBR is already present at the framework level. The missing work includes correct material inputs, coherent lighting and image formation. A larger diffuse map alone cannot supply these.

## Milestones and acceptance

1. **Controlled baseline and first lighting pass.** Add explicit output resolution and `baseline` / `enhanced` rendering profiles. Preserve the current default. Enhanced starts with 16x anisotropic filtering, four 4096-pixel shadow cascades over 250 m, procedural Earth sky and sky environment lighting, AgX tonemapping, restrained bloom, SSAO and TAA. All values are authored starting points, not recovered original settings. Capture the same stationary car/camera at 1280 x 800 and 3840 x 2160. Check logs for shader/device errors; inspect images at full resolution, including fences and car silhouette. Record executable/source hashes. Do not claim 4K performance from a still render.
2. **Restore and classify materials.** Inventory original sampler names, shader inputs, UV sets, texture dimensions/formats/mips and all visible material classes. Prioritize asphalt/markings/kerbs/grass, paint/glass/tyres/wheels and signs. Decode normal/specular/mask maps in linear space, validate channel meaning and tangent orientation; preserve diffuse as sRGB. Original specular/gloss controls need an explicit mapping to Bevy roughness/metalness. Recover existing detail textures and layered inputs before fabricating replacements.
3. **Reflections and lighting consistency.** Start with sky IBL; it does not reflect buildings or nearby cars. Add local/static environment probes, then investigate bounded dynamic probes or SSR for car paint/glass. Measure GPU cost and visible errors before increasing resolution/update rate. Validate shadow acne, detached shadows, cascade transitions, transparency, contact AO and lighting direction while driving.
4. **Selective higher-resolution assets.** Select 20-30 high-impact Lacetti/Donington diffuse textures by screen coverage and actual visible deficiency. Use local override packages keyed by resource identity; keep originals and enhancements under ignored `local/`. Compare 2x and 4x per axis with complete regenerated mip chains and suitable compression. A 4x width/height increase means 16x texels and roughly 16x compressed storage for the same format/mip policy. Handle text/logos separately; learned detail is an interpretation and must not be presented as recovered source detail. Do not upscale normals/masks using a photographic RGB model.
5. **Polish and measured 4K target.** Choose natural color and/or a separately authored GRID-inspired grade after owner comparison. Test a complete lap with 12 cars, barriers, foliage, glass, distant structures and fast camera motion. Record CPU/GPU, driver/API, frame-time distribution and VRAM; aim initially for 4K/60 on the owner's RTX 4070, subject to measurement. Adjust quality tiers, LOD distances and reflections using those results. RTX is optional later work, not a milestone prerequisite.

The small remaster milestone is one car/one circuit with coherent sky/lighting, correct materials, stable shadows and reflections, selected improved textures and a measured 4K mode. It is not a promise that this first profile alone exceeds the original in every scene.

## Tools and sources

Existing tools suffice for the first milestone: Rust/Cargo 1.99.0, Bevy 0.19.1, existing PSSG/DDS readers, GPU offscreen capture, Python 3.12.14 and installed image tools. No duplicate tool or upscaler installed yet. Inventory installed upscalers/texture compressors before choosing a later enhancement pipeline; pin licenses, model/tool versions and hashes then.

Primary API references: [Bevy cascade configuration](https://docs.rs/bevy/0.19.1/bevy/light/cascade/struct.CascadeShadowConfigBuilder.html), [Bevy SSAO](https://docs.rs/bevy/0.19.1/bevy/pbr/struct.ScreenSpaceAmbientOcclusion.html), and the installed 0.19.1 examples `atmosphere.rs`, `ssao.rs`, `bloom_3d.rs`, `tonemapping.rs`. SSAO uses single-sample depth, so the enhanced camera selects MSAA Off and TAA instead. Procedural atmosphere and its generated environment map require no downloaded sky assets.

## Initial implementation status

Rendering profile and resolution changes are being validated in the isolated workspace. No visual quality, performance or playable release claim until build and image checks are recorded below. The primary playable release remains owned by the engine workstream.
