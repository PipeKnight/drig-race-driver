# Donington atmosphere and distant walls

17 October2026. Graphics roadmap: lighting, depth and the owner's distant-strip defect. This is not a camera-feature task or a change to physics. Source assets/configuration and original decompilation remain private.

## Observations

The enhanced scene has no DistanceFog component. Its source sky is a separate custom material; the existing atmosphere environment therefore does not establish source distance-fog behaviour on track/object surfaces. The installed Donington lighting.xml contains saved fog controls. The original shader's complete height/distance/radiometry mapping has not been recovered.

| Saved control | Donington value | Pilot use |
|---|---|---|
| fogDistance | 500 | Ramp width in metres; statically recovered reciprocal uniform |
| fogDensityCap | 0.4 | Maximum blend through DistanceFog colour alpha |
| fogColour | 0.68235,0.74510,0.90588 | Original normalizes packed RGB bytes; pilot uses saved floats directly |
| fogColourBrightness | 0.5 | Multiplies the pilot RGB |
| fogBaseLevel / fogTopLevel | 0 / 2000 | Preserved metadata; height formula not applied |

The source route ornament attributes give parameter_wall_long LOD distances30/60/950m; track LOD overrides give track/world cull1500m and world clip1800m. The identified wall hits at733–899m are inside these saved outer ranges. This does not justify arbitrarily hiding them. Their original lower LOD and runtime visibility selection remain separate research. Their triangles are a visible source draw, not the already-excluded rigidbody duplicate.

Land/trees is track_infield, with two ground diffuse maps and don_outfield_trees_d as macro colour. The source vertex green clamp0.5 was already checked against the matching infield shader; it must not be removed merely to darken the landscape. The macro texture and saved vertex controls were inspected, without finding a new binding error. Source tree placements and closest meshes are now restored by the forest pilot; distant strips persist.

## Bounded saved-control reader

New grid-formats::track_lighting::read_fog uses the existing pinned quick-xml0.41.0, XML size<=1MiB and<=256single-attribute param elements under one lighting root. Required fog fields must be present and finite; height bounds ordered, distance positive/bounded, density cap0..1 and colour/brightness bounded. Rejects duplicates, unexpected nested structure, DTDs, malformed values and invalid ranges. Unknown other lighting keys are preserved only during parsing and do not gain invented rendering semantics. Actual purchased Donington and negative/synthetic tests pass,2tests/zeroignored. No new tools installed. Changed format/app all-target release Clippy7.11s.

Private evidence: local/graphics/vegetation01/source-lighting.txt, ornament-attributes.tsv/.txt, lod-overrides.tsv/.txt and SHA256 manifest land-wall-source-attributes.json. Earlier PowerShell -Encoding Byte was unsupported in this shell; a binary ornament file was initially read as text. Corrected by inspecting magic and using the installed bounded grid-binxml decoder; no game file was modified.

## Authored distance-only pilot

Explicit --haze source (defaultoff / --haze off) reads the selected track's source controls and requires enhanced track rendering. The renderer uses Bevy0.19.1 DistanceFog, exponential density1/fogDistance and colour alpha=fogDensityCap. Thus the authored blend is cap*(1-exp(-distance/distance_parameter)), about0.253at500m and0.318at790m. This is an adaptation, not the recovered original equation. The source height bounds, original sky gamma/scale and directional fog scattering are not applied. The source sky remains unchanged; reflection-probe/deferred combinations have not been accepted.

Pinned Bevy source applies distance fog after exposed PBR lighting and before HDR tonemapping. Existing track/car extended fragments call that common post-lighting helper; diffuse texture upload remains through texture_upload::image. Colour-space/radiometric assumptions are explicit. Native4K same-camera off/source comparisons must establish whether depth improves without washing out nearby car/road detail. No wall or object is removed. GPU acceptance is pending.


## Static fog shape recovered before promotion

Existing pinned source `local/graphics/shaders/track-infield-pixel01.txt` / metadata (`track_infield.cg.fp`, shaderpack SHA b6b29b80397d12def9c90f5afcff9ea8f335d07da3dc81e293273b468c9029a6) computes `f = saturate((length(position-eye)-30) * fogParams.z) * fogParams.w`, then mixes the lit/AO-modulated colour toward fogColour. The associated vertex stage passes world position. Only fogParams.zw are consumed in this examined variant; no per-fragment height term occurs here. That does not establish all shader variants or the CPU upload's mapping from XML to z/w/colour.

The first exponential source-inspired experiment rendered successfully but is superseded, retaining its images and logs as evidence. The revised pilot uses Bevy linear start30,end30+savedDistance,colour alpha=savedDensityCap: it matches the observed distance shape **if** z=1/savedDistance and w=savedDensityCap. Those bindings and radiometry remain explicit assumptions. The earlier exponential description above documents the initial experiment, not the current implementation. The strong far fade and remaining strips do not justify claiming a geometry repair.

Initial73f1a37 / SHAbe4c760b7c9993728c0a33f37a1870eb6164039f58a534ed5e7264f4b0ccee2f build1m21s: native4K off/source static and1200tick driving pairs inspected; pose/velocity identical, RGBA537/0BC, knownnilUUIDWARNonly. Near-road RGB changeMAE0.319byte vsfar-strip21.84bytes; first sky ROI accidentally included a crown, corrected clear-sky ROI MAE0.0053/max1byte. This is localized atmosphere evidence, not a fidelity/quality score. Original equations/radiometry and height remain open; current linear candidate still needs GPU validation.


## Uniform binding recovered and linear pilot checked

Ghidra12.1.4_PUBLIC processed the existing grid-unpacked project with `-readOnly -noanalysis`; no analysis image or Steam file was modified. Private analysis PE SHA256 aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514; Capstone5.0.6 corroborated instruction-aligned stores. Existing installed JRE reused; no tools installed.

The static chain now establishes the bindings for the examined shader:

- Field registration at741520: fogDistance at lighting+0x2c0, fogDensityCap+0x2c4, packed colour bytes+0x2c8..ca and brightness+0xa0.
- Lighting update730f90, relevant instructions731dfe..731e83: fogParams.z=1/fogDistance and .w=fogDensityCap. SSE masks atcaf420/caf430 select Z/W; numeratorcaf888 is1.0. The same update normalizes packed R/G/B bytes by cb95c4=1/255 into vector+0x80.
- Uniform dispatcher743220 case0x0f binds fogParams from+0x90; case0x1b binds fogColour from normalized colour+0x80 times brightness+0xa0. No gamma conversion occurs in this examined preparation/dispatch.

Consequently the examined infield pixel shader's factor is `cap * saturate((distance-30)/savedDistance)`. Bevy linear start30/end30+savedDistance and colour alpha=cap reproduce this distance factor. The current diagnostic label still says authored-uniforms; the stronger binding evidence was obtained after that build. This does not establish the runtime-selected variant, every original pass, height handling elsewhere, XML float-to-byte rounding, original exposure, HDR/tonemapping or 1:1 radiometry. The pilot keeps saved float RGB directly instead of reconstructing unverified quantization. Source sky remains unchanged.

Private evidence: fog-ghidra01/{74167d,743483}.c.txt; fog-ghidra03/731dfe.c.txt; fog-uniform-aligned01.txt; fog-lane-masks01.txt; fog-pe-strings01.txt. Broad offset searches also found a zero constructor742dc0 and destructor path744d20/7423a0: neither proves a fog getter, so those hypotheses were discarded. A preliminary disassembly started before a known boundary; only corrected aligned731dfe output is used. The helper length argument is hexadecimal, recorded to avoid interpreting its longer dump as additional code coverage.

Linear source8b9919fbf5c6, release build59.27s, executable SHA256 c5b9842062bfc45f962a62d45d8c826438e95e9937fb9a87ae30ae6d2a55c545: static and1200tick native3840x2160 captures inspected. haze-linear4k01 and haze-linear-drive4k01 render without unexpected WARN/ERROR (only known nil-UUID gilrs warning); forced RGBA8 path, no BC upload. Physical endpoint matches haze-off exactly. The far fade remains strong under the authored PBR lighting, strips remain visible; default haze stays off. Precise localized comparison in haze-linear-validation01.json; no whole-image fidelity score or phone verification claimed. Full test/check provenance is recorded separately before integration.

Optional local preview: `Play GRID Graphics Atmosphere.cmd` launches the source forest plus source distance haze; optional first argument is the purchased game root, second is resolution (for example3840x2160). Ordinary/Wheels/Textures launchers keep haze off. This preview exposes a comparison, not a new default or a claim that vegetation/walls are finished.


## Distant-wall shader hypothesis checked

The source parameter_wall_long_01 material binds `object_barrier.fx`, diffuse parameter_wall_01_d and a specular map. Existing disassembler/D3DDisassemble inspected the selected regular and instanced object_barrier vertex programs and regular pixel program. Both vertex programs apply ordinary affine position transforms; no terrain-conforming vertex operation occurs in those examined variants. They do not consume a vertex colour input. The examined pixel program returns diffuse alpha and has no discard; the saved wall diffuse is opaque. Thus missing shader terrain conformance or vertex-alpha invisibility are not established explanations for the strip. Runtime-selected passes/CPU placement/visibility remain unobserved.

Private shaderpack-pinned `parameter-wall-vp01.asm`, `parameter-wall-instanced-vp01.asm`, `parameter-wall-fp01.asm` plus metadata. Source object contains closest draw !pE/index source !lE, a lower LOD list with draw !uE/index source !rE, and the excluded rigidbody duplicate !oE. Lower-LOD geometry comparison remains the next concrete check; no arbitrary wall removal. This static research did not change the accepted grass candidate.
