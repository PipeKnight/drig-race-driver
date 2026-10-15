# Lacetti / Donington graphics workstream

Current owner priority (15 October 2026): **image quality first; optimize later**. The ordered Russian roadmap is [GRAPHICS_ROADMAP_RU.md](GRAPHICS_ROADMAP_RU.md). Earlier performance targets below are deferred and do not block visual development. The owner reports heavy background CPU load, so current frame-interval observations cannot isolate rendering cost.

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

The first experimental profile is built and smoke-tested in the isolated workspace. The primary playable release remains owned by the engine workstream.

## G001 validation and first experimental build

Source `b476fe36ef4754c80f0d8fc4fc5d8965a51b0215`; executable `target/release/grid-app.exe`, SHA-256 `7204f4de99a5586c87ed0c7a69e846b5f50c4ff6f12a3977af3290a4cd2c4777`. Every app Rust source file was compared with that revision. Rust 1.99.0 / Bevy 0.19.1; observed adapter RTX 4070 / Vulkan / NVIDIA driver-info 616.92. Clippy `-p grid-app --release --offline --no-deps -- -D warnings` passed. No new tools installed.

Private evidence is under this checkout's `local/graphics/`:

- `baseline-4k-final.png` and `enhanced-4k-final.png`: identical stationary Lacetti/start-line camera, native 3840 x 2160, successful GPU saves and PNG verification; both images inspected. Enhanced has a lit sky, sky ambient/specular response, softer filtered shadows and warmer image formation. It remains visibly provisional: sky is plain, the image somewhat pale, and diffuse-only materials do not reproduce the original material detail. Quality is not rated against the original game yet.
- `baseline-drive-4k` and `enhanced-drive-4k` PNG/log pairs: 240 ticks, 0.25 throttle, identical complete final pose/velocity line at 3.840 simulation seconds. This is bounded smoke equivalence, not a full-lap or physics validation. Enhanced `wall_seconds` at tick 240 was 7.138276; baseline exit was 5.296601, both include startup/pipeline work. Neither number establishes steady-state FPS.
- `enhanced-body-4k` PNG/log pair: same bounded 4K drive with the existing private CD01/BP01/DF01 parameters, no render/runtime failure. This is not a crash-damage scenario.
- `validation01.json`: exact hashes, dimensions, build/source identity, guard results, equivalence statement and tool versions. Invalid zero/oversize/overflow resolution and unknown preset fail before asset loading. Inventory rejects public output paths and existing output files.

First commit attempt failed because the new worktree had no Git author configured. Restored the existing `Codex <codex@local>` identity **only for this worktree**. Initial static captures had a pre-commit build header and are retained as pilots; final captures above were rerun with the correct `b476fe3` header.

Run `Play GRID Graphics.cmd` from this checkout for the separate experimental driving build (default 1920 x 1080; maximize the window as desired). Optional first argument is the purchased game folder; optional second argument is `3840x2160`. It uses private collision/event/deformation parameters when present, logs under `local/graphics/`, and does not replace the primary launchers. Manual driving, audible output and native-window DPI behaviour for this build have not been observed in this run.

Reproduce a 4K comparison:

```powershell
target/release/grid-app.exe --scene reference --view start --graphics baseline --resolution 3840x2160 --render local/graphics/new-baseline.png
target/release/grid-app.exe --scene reference --view start --graphics enhanced --resolution 3840x2160 --render local/graphics/new-enhanced.png
```

## G002 resource inventory and next graphics tasks

`tools/inventory_graphics_resources.py` reuses the installed read-only PSSG survey and saves metadata only under ignored `local/`. `resource-inventory02.json` covers nine actual/catalog/sponsor packages and 860 texture records; 411 meet the current full-chain size/count contract. These counts include duplicate IDs across files and must not be described as 860 distinct loaded textures.

| Package role | Records | Current full mip-chain contract matches |
| --- | ---: | ---: |
| Track surfaces | 45 | 30 |
| Objects catalogue | 391 | 0 (all 4 x 4 single-level placeholders) |
| Actual object texture patch package | 391 | 359 |
| Three sponsor packages together | 14 | 14 |
| Lacetti livery | 6 | 4 |
| Lacetti generic | 13 | 4 |

The app already replaces catalogue object textures using `patchup_ot.pssg`. The first five-package inventory (`resource-inventory01.json`) omitted that replacement and sponsors; retained as an incomplete pilot, not evidence that the renderer uses 4 x 4 object textures. Single-level images and failed full-chain contracts are different cases. Some rectangular maps have packed sizes differing from a conventional complete BC chain; their mip layout remains unresolved.

Shader definitions include normal/specular/occlusion samplers, and track macro/second diffuse inputs. These names establish available inputs, not verified channel meanings. Prioritize (1) recover original Donington lighting/sky/grade for the owner's GRID atmosphere, (2) resolve useful rectangular/partial mips and classify normal/specular/mask channels, (3) implement the highest-impact material bindings with a same-camera comparison, (4) add local environment reflections, (5) introduce selected higher-resolution local texture overrides. Measure warmed-up frame times/VRAM before calling any profile 4K/60 ready.
