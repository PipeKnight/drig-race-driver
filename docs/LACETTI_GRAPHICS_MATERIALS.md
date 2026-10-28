# Lacetti material response

15 October 2026, independent graphics profile, source `6404c7091e44`, Bevy0.19.1. Enhanced defaults to layered car materials; `--car-materials legacy` retains the earlier authored material parameters for comparisons. Baseline is unchanged.

Previously most car surfaces used roughness0.7, with paint0.32 and a few metals metallic0.6. New authored responses distinguish dielectric paint with clearcoat, smoother glass, rough rubber, coated white alloy rims and bare metal/brake discs. Paint base roughness0.3/clearcoat1/coat roughness0.12; glass roughness0.08/IOR1.5; tyre wall0.82/tread0.9/reflectance0.3; wheel roughness0.32/metallic0.15/coat0.3; bare/shiny/disc metal0.95 with roughness0.35/0.22/0.48. Cabin/matt/aerial recognized groups remain rough dielectrics.

These are artistic starting values, not conversions of original specular strength/power. Existing diffuse/alpha bindings are retained. Original normal, scratch, damage and specular-mask consumer semantics are still incomplete; no speculative scratch-to-roughness mapping or whole-car damage normal was introduced. No new texture or proprietary data is distributed. Unknown material names retain the previous parameters.

Matched `local/graphics/car-material-legacy01.png` and `car-material-layered01.png`, native3840×2160, source-sky IBL, camera(-187,-10.7,22)/target(-183,-11.3,19.4): both inspected. Layered glass has a sharper sky response and paint/metal/rubber differ, but the car still needs better livery detail, normals, local reflections and art tuning. Paint's environment tint/brightness changes; this comparison does not establish final realistic appearance or fidelity to GRID. No claim that clearcoat alone completes the car.

Release all-target Clippy passed. GPU captures exit normally, source-sky filtering readiness logs256source/32diffuse/9specular mips, no shader errors. A 240-tick/0.25-throttle source-sky/layered-material drive has the same final pose, velocity and handling as the source-tyre control; no physics code changed. Renderer output/manifest verification recorded in private `validation-materials01.json`; the separate graphics launcher now selects source-sky IBL. Original Steam installation and primary engine checkout/executable remain untouched.

Next: original material channel/UV semantics, cockpit/lighting close-ups, detailed texture overrides, local scene reflections and a coherent full-lap visual assessment. Higher-quality rim/disc silhouettes remain open alongside those material tasks. Future DLSS/RTX requirements are in [GRAPHICS_QUALITY_CONTRACT.md](GRAPHICS_QUALITY_CONTRACT.md).

16 October follow-up: low-angle testing exposed a renderer initialization issue, not just incorrect artistic values. [CLEARCOAT_PREPASS.md](CLEARCOAT_PREPASS.md) documents the matched stock/fixed result and narrow in-memory Bevy0.19.1 repair. Current paint coat0.2/coat roughness0.18, rim coat0.1; geometric coat normals now initialized in the normal-prepass path, preserving livery contrast. Source483941155483 also includes guarded closest-LOD tread normals. Initial stronger values and all failed/control images remain recorded as history.


18 October texture-selection audit (no replacement/inference yet): purchased player livery00 has six maps. Actual main diffuse is2048x2048 BC1/full12mips, opaque; cabin512x512 BC1/full10mips is also opaque. Wheel512x512, glass1024x512 and main_m1 1024x1024 use BC3 with nonconstant alpha. Do not feed those packed/translucent maps into the current opaque-only RGB upscale packager. The main player texture dictionary and opponent livery banks are loaded separately; confirm an override's actual binding before claiming a whole-field improvement. Private local/graphics/materials-livery01/inventory01.json and base decodes preserve provenance; original installation untouched. Body texture is a future controlled detail candidate, not a completed4x upgrade.


## Highlight investigation — 18 October 2026

Owner's `body-neural-moving30-01-frames/0000.png` is a real prototype capture (source01cbcf8). A selected window-area ROI x230..750/y650..750 has1011fully255white pixels of52000; the sampled white door max is236/232/225 and sky204/201/200. This confirms local display saturation, not all-frame clipping. HUD whites must be excluded from photographic measurements. Private highlight-audit01.json records the explicit ROIs.

New source6094b4c with unchanged default EV13 reproduces the owner's full frame pixel-for-pixel. Original versus upscaled body at EV13 has the same1011white pixels in this windowROI; that ROI also crosses adjacent body/frame and has max55/mean0.395byte differences, so it is not a perfect isolated-glass mask. The clipping is reproducible without a neural body replacement. All4physical exit records match across1200tick tests. Paired2frame sequences use the same30virtualFPS/tick1102 first-frame timing; this is not a monitor-FPS/whole-lap validation.

A bounded enhanced-only `--exposure 6..20` control retains EV13 by default (+1EV halves scene light before AgX). Neural-body EV13/14/15 windowROI white counts1011/497/204; white-door means approximately(232,227,219)/(212,205,195)/(187,179,167). Darkening alone leaves a blown core while reducing body/ground brightness. It is a diagnostic, not an accepted blanket correction. Source background sky remains nearly unchanged because it has its own display mapping. Five real graphics_settings_tests passed after correcting an initial too-narrow filter that ran0tests; app Clippy5.47s, release1m13s. Private highlight-exposure-audit01.json and native-pixel comparison.

Window Glass still uses an authored roughness0.08, not a conversion of the original shader. New `--glass-roughness 0.04..1` changes only Window Glass in enhanced layered car materials, leaving light lenses/paint/alpha and defaults untouched. Six graphics settings tests/appClippy7.37s pass; GPU comparison follows. Recovered selected `car_basic.cg.fp_glass_windows` variant uses a reflected-direction Phong-like power and source environment/Fresnel/dust/shadow terms; saved power300/strength64 cannot be copied into GGX roughness or physical sun intensity. Static disassembly is not proof of runtime variant selection. Private source-window-fp01.asm/.json pins the source shaderpack/program hashes; no original code is published.


Decision after paired window-roughness captures:0.20/0.30 reduce fully255white windowROI pixels to0 (versus1011at0.08), yet broaden the pale reflection and look more frosted. Rejected as a visual correction; no default change. Both2frame sequences/1200tick4exit records match the prior control, known nilUUID warning only. A bounded window reflectance comparison knob was added separately (default0.5, matching Bevy; F0=0.16*reflectance²), not yet GPU-evaluated or chosen as an improvement. Six targeted tests/appClippy9.56s pass.

**Owner clarification:** the highlight observation was not a demand to remove clipping. Small saturated highlights are acceptable, including GRID2008's visual character. Further highlight tuning is deferred; resume the agreed graphics plan. Do not chase a zero-white-pixel score or darken the whole scene by default. The original recorded frame at225s has19fullywhite and946near-white(minRGB250)pixels in the MCN scene-sign ROI(230,395)-(540,455), which excludes the HUD. This proves output-level white saturation in the owner's original-game recording, not the original renderer's internal HDR clipping stage (lossy video/JPEG). Private original-sign-highlight01.json includes the frame hash and precise scope.


18 October owner roof comparison: a missing race-number decal is confirmed independently of the highlight investigation. Base livery00 has blank roof/two-door panels; cars/lac/decals.xml supplies matching3number placements and user_liveries/numbers/font0.pssg supplies10alpha digits. Current loader omits this composition. See [source livery decals](CAR_LIVERY_DECALS.md) for proof, remaining unknowns and implementation acceptance. This belongs to existing Lacetti material/detail work; no arbitrary number or contrast workaround has been applied.


18 October follow-up, paint consumer audit (no appearance change): the actual highLOD material name is `Car Body Paint`, shader group carpaint_metallic.fx. Saved instance constants include NormalMapScale1, EnvironmentColour≈0.537255neutral, FresnelMin0.3/Max0.6/Power1, SpecularHighlightPower300/Strength64, MetallicSpecularPower10, OcclusionMapScale1 and disabled emissive/glow. These legacy parameters are not direct PBR roughness/metalness values. This instance saves11constant inputs and no texture inputs; runtime bindings still need independent evidence. Private paint-binding-audit03.json pins the model and joins parameterIDs to group definitions. Earlier audit02 matched an incorrect short material name and yielded no materials; retained as an unsuccessful query, not a finding.

The already extracted paintDustSpecmapOcclusionReflectiveNormalScratches pixel variant samples TDustMap and uses its channels in separate dust, occlusion and reflected/specular paths. Static variant availability is not proof that this exact variant is selected for the clean Lacetti. Do not assign the whole main_m1 image as modern roughness, or introduce scratch/damage effects globally, from its filename alone. Next step after number publication is to prove the runtime texture/sampler association and isolate an opt-in material-map comparison with the same camera/light; defaults remain unchanged.


18 October static binding follow-up: pinned executable table base0xd14b10,16rows/12bytes per row, identifies `_main_m1.` → `TDustMap` for `Car Body Paint` and six other named materials; `_main.` → `TDiffuseAlphaMap` and glass/cabin bindings are separate. Read-only Ghidra consumer0x9ca780 iterates these triples, reuses lookups for equal suffixes and constructs binding callbacks; caller0x9ca990 passes this table. This establishes the saved static association, not an observed live shader selection or completed callback-resolution trace. Initial table01 grouped material-first four bytes before the actual row and crossed row associations; corrected table02 is the evidence to use. All derived code/pointer lists remain ignored/private.

Selected paint shader algebra gives a useful next hypothesis: with packed sampleG and uniform MetallicSpecularValues.w, the occlusion factor is `1 + w*(G-1)`. It multiplies lit diffuse and also participates in reflected-light weighting; it is not automatically equivalent to Bevy ambient-only occlusion. SampleR participates in dust; sampleA is reused for metallic/specular/environment masks. Exact uniform-to-saved-property binding, texture colour-space and clean-car active variant still need validation before an appearance change. Base decoded channel stats confirm this is not grayscale (meanR≈23.02/G≈135.83/A≈97.65; only4.32%equalRGB). Private paint-packed-map-stats01.json, paint-texture-binding-table02.json and paint-binding-ghidra01 preserve reproducible evidence. No packed-map upscale or roughness replacement was applied.


Uniform trace caution: read-only decompile0x9e7360 writes SpecularValues/MetallicSpecularValues, but its context includes a35material selection index, cached per-material values and conditional override flags. It must not be treated as the unconditionally active clean-car property loader. A separate0x9cb280 tree-walk candidate writes OcclusionMapScale from offset0x1c; the equal offset in an independently inferred state object is not sufficient to prove shared object layout or production defaults. The first string query assumed several labels existed in the PE and asserted; corrected query records missing names rather than inferring them. Continue tracing the actual production binding/group expressions before enabling a packed-map material change. Private paint-uniform-ghidra01 and paint-property-readrefs02.json.


## Ambient-only paint occlusion pilot — initial draft (historical)

`--body-occlusion source-ambient|off` is a new opt-in enhanced-render experiment; off remains the default and no launcher selects it. It reads each car's own BC3 main_m1 mask, copies raw green to grayscale opaque BGRA8 while retaining every stored mip, and uploads as linear data through texture_upload::image_linear after device selection. The supported bounded format deliberately rejects malformed/trailing/unsupported masks before staging any output. Source packed maps remain byte-identical; RGB/alpha dust/specular information is not repurposed or overwritten.

Only `Car Body Paint` receives the derived map. Bevy0.19.1 reads occlusion.red; replicating green and explicitly disabling sRGB avoids using source dustR/alpha by mistake. Each opponent gets its own livery/model mask on a separate material, including when diffuse falls back; it must not inherit the player mask. This is PBR ambient occlusion adaptation; original shader also uses the factor in direct/reflected lighting, and its live uniform/colour-space selection is unresolved. No source-parity/default-quality claim. CPU tests, Clippy and same-view roof/side/underbody/native4K/forcedRGBA/moving comparisons are required before promotion. The verified source-number executable stays installed during this draft.


Shader-group cross-check: the exact shaderpack group carpaint_metallic has33inputs/6passes. Its saved pass-mask1 selects the previously inspected paintDustSpecmapOcclusionReflectiveNormalScratches fragment program; mask32 selects its reflection counterpart, while velocity/depth, shadows and AO have separate passes. This ties the algebra to the corresponding named paint group rather than a similarly named arbitrary variant. It still does not observe the original runtime pass-mask or establish conversion from car-instance scalar properties into grouped float4 uniforms. Private paint-exact-group-pass01.json keeps the focused mapping; no original shader code is published.


### First occlusion GPU acceptance — candidate99902a6

Build5m12s/SHAa40aeaf7...;4CPUtests pass after the missing-root retry, app all-target release Clippy39.72s clean. Ordinary native4K view with the optionoff matches installed31e74bcontrol PNG bytes exactly (0different pixels). Enabled source-ambient and forcedRGBA native4K captures inspected: subtle darkening in bumper recesses/underbody/fasteners, number/livery retained. Derived image logs Bgra8Unorm/linear/11mips through shared helper. ForcedRGBA reports0BCassets/558images/547decoded. Full-frame BC-versusRGBA max150/mean0.088929byte is not byte parity or phone validation; the derived BGRA map itself is uncompressed in both paths.

Enabled-versus-off full mean0.220617byte,621323changedpixels. Most visible changes fall on the car; outside the broad car ROI(450,780)-(2850,2100), mean0.000669byte/9924changedpixels,11pixels exceed1byte and maximum23. These isolated background differences are measured, not attributed to an unproven startup-noise cause. Material assignment remains limited to paint; do not claim the full image changes exclusively inside its silhouette. Private body-occlusion-default-parity01.json/effect-audit01.json/effect-regions01.json/static-validation01.json. Native-pixel bumper comparison crops the same(530,1500)-(1230,1930)region without resizing. Motion/per-opponent GPU and physical-state checks pending; no new default or playable promotion.


Motion acceptance99902a6:1200ticks/oneopponent/source numbers/30unique virtual30FPSframes. Four final physical/handling/deformation/lap lines exactly match the earlier c0b control after removing walltime; this is short cross-build identity, not a newly completed full lap or measured monitorFPS. Separate car0/car1linear11mip uploads logged; first/last frames inspected. Private body-occlusion-motion-validation01.json. Default CLI-off pixel parity and source-ambient static/motion acceptance justify enabling this modest material improvement in the dedicated Lacetti graphics launcher only; third argument no-body-occlusion gives the comparison. Global app default staysoff; original full direct/reflected-light weighting is still not reproduced. Launcher queries help once and omits unsupported new flags when an older/other-agent executable is installed. Phone performance and live-original parity remain unclaimed.


Graphics build delivery: `Play GRID Graphics.cmd` prefers `local/graphics/grid-graphics-app.exe` when it exists and falls back to the checkout's `target/release/grid-app.exe`. The main checkout can therefore receive a separately verified graphics binary with `local/graphics/graphics-current-build.json`, preserving its other agent's normal executable. Graphics wrappers share this selection and the selected file's help capability checks. Installed build/source revisions and pending HUD integration stay explicit; source sync alone is not executable publication.


Combined graphics/HUD delivery80babb019213: release8m34s, SHA9ba214eeb4b8...,1200ticks/30unique virtual30FPSframes with two own-mask uploads; four physical exits equal99902a6control. Source numbers/ambient paint masks and F383 original-look HUD inspected together. Updated main-folder dedicated graphics executable and graphics verified copy, preserving peer normal2014aae6... . Full-lap evidence remains attributed to31e74b; no new full-lap/phone claim. Source tree subsequently fast-forwarded through F397/2393a9e; those newer changes are pending a separately verified graphics build.


### Specular-map transfer audit (Bevy0.19.1)

Local pinned renderer code confirms that `StandardMaterial::specular_texture` reads **alpha**, multiplies reflectance by `sampleAlpha * 0.5`, and then computes dielectric F0 as `0.16 * reflectance^2`. Simply attaching the original packed mask with the current scalar0.5 makes even a white texel produce F0=0.01 instead of the unmapped0.04. This would unintentionally dull the complete paint material. The feature is separately gated by Bevy `pbr_specular_textures` and is currently not enabled; no mobile material-binding expansion has been introduced.

For an **authored** target F0=0.04*A, one possible sampled-point encoding is alpha=sqrt(A), scalar reflectance1.0 (accounting for the half factor). Across all256 input bytes, nearest-byte encoding has maximum absolute F0 error0.00015625. This is only a transfer calculation: bilinear/mip filtering of sqrt(A) does not commute with squaring. It is not evidence of the original game's production response or of acceptable appearance. A future pilot must compare filtered mip levels and independently account for clearcoat; modulating only the base dielectric lobe leaves the coat's reflection intact.

The selected source fragment has separate alpha-derived terms (including a saturated doubled-alpha environment factor) and dust/vertex/uniform gates. A single modern specular map cannot be declared equivalent to all of them. Keep the source packed map immutable; do not copy alpha into roughness, color or opacity. Next bounded experiment should be a separate optional desktop material feature and matched native4K comparison, with per-car own-mask staging/shared linear uploads and white-mask control. Do not enable it in either launcher before GPU acceptance. Private `materials-livery01/paint-specular-transfer-audit01.json` pins the local source-file hashes and256-value calculation; no original shader text is published.


F397 graphics-source integration is now verified and installed as1d07640bfb42/SHA9f9d9937c193...:657workspace tests, clean workspace Clippy, canonical solo identity unchanged,20/20quicksoak,30moving frames and4physical exits equal earlier80control. Last frame pixel-exact; first differs on688background pixels only, cause unresolved. No new full-lap/phone claim. Source numbers/8Katlas/ambient mask forced-RGBA native4K integration also inspected on80babb0; original atlas hash retained. Matched1024probe versus sky with ambient mask works, but remains opt-in pending radiometry/parallax acceptance. See G120/privatevalidation for exact hashes and scope.


### Exact metallic-paint variant selector — static and isolated execution

Pinned-PE static metadata associates selector0x61bb80 with the exact `carpaint_metallic` family. Coherent entry follows a prior ret8/padding; callback pointer0xce4998 and literal-name reference0x61bbbd identify it independently of a guessed interior address. Read-only Ghidra12.1.4 decompilation reveals three variants: diffuse-only family entry `carpaint_metallic_noscratchnormal` (mask1), normal/no-scratch `carpaint_metallic_noscratch` (mask5), and full `carpaint_metallic` (mask7). Flags come from non-null TDiffuseAlphaMap, TScratchMap and TNormalMap handles. TDustMap presence and the paint scalar values are not tested by this selector. Selection includes an exact-match then first-superset fallback, so unsupported/missing diffuse combinations must not be interpreted as real playable content.

An isolated Unicorn2.1.4 check executes the original selector and bounded string-copy bytes with an independently authored name-to-index lookup stub and synthetic parameter collections. All8texture-presence combinations pass in both contiguous0x50-byte storage and pointer-array storage (16cases), with matching selected names. This proves this bounded instruction path, **not** the actual clean Lacetti's live texture handles, callback registration execution, scalar-to-uniform packing, or radiometry. Private `paint-exact-selector-emulation01.json`, `paint-exact-selector-emulate03.py` and read-only `paint-exact-selector-ghidra01` retain inputs/results. The shared0x6bf780 callback is a destructor, not a uniform packer.

Research corrections retained: the first analyzed-containing query failed because no analyzed function covered0x61a255; Ghidra nevertheless returned process status0, so success requires the expected artifact and no script error. Independently established entries were then created only in read-only session memory and discarded.0x96e430 (three decoded callers) selects object_* scenery replacements;0x619da0 selects env_* variants. Neither establishes paint uniform packing. TDustMap consumers0x61b2e0/0x61b730 select the separate car_di_*/car_cutout_di_* groups, not this exact family. These rejected matches prevent importing unrelated OcclusionMapScale defaults into Lacetti.

Two emulator drafts failed after an incorrect authored copy-hook calling convention. The source copy returns with ret12; the accepted retry executes its real bounded-copy instructions instead of emulating that return. No draft failure is counted as acceptance. String audit first stopped on unbacked BSS, then recorded those addresses as absent. The second texture-binding table at0xd14844 has only two wheel/tyre-wall diffuse rows and an empty-string-pointer sentinel at0xd1485c; a null-pointer sentinel assumption was corrected against the consumer. It does not supply body normal/scratch bindings. No original project/binary/game installation was modified, and no appearance default changed during this research.

Next: trace the actual body normal/scratch callback resolution and the production scalar-to-float4 parameter packing. Keep the accepted ambient-only adaptation clearly separate from claims of original shader equivalence. Text search of installed XML/FX/INI/Lua/TXT found no exposed MetallicSpecularValues/SpecularHighlightPower definitions; absence from that bounded search does not prove no binary mapping exists.


### General-bank normal/scratch linkage

Pinned PE has a separate41-row suffix/parameter/material table at0xd14c28, ending at empty-string sentinel0xd14e14. Its exact Car Body Paint rows are0xd14c34 (`_main_nm.` → TNormalMap) and0xd14d90 (`_main_sc.` → TScratchMap). A single static table reference0x9caccf leads to the coherent function0x9cac70, which calls the already identified binding consumer0x9ca780 at0x9cacd7. Assembly confirms its guarded general bank is `car_state+0xdc8+20*index`; later livery/wheel tables use the separate `car_state+0x918+20*index`. Do not infer this hidden-this field from the decompiler's repeated output-variable name alone.

Actual purchased `lac_gen_high.pssg` contains matching1024² `lac_main_nm.tga` BC1 and `lac_main_sc.tga` BC3; gen_low has the corresponding512² maps. Both inventories match the exact Lacetti-prefixed saved suffix rows, with all four stored gammaRemap flags0. Those flags are metadata, not observed D3D sampler state. This closes the static resource/table/consumer association and supports availability of the full metallic-paint selector's inputs. It still does not observe the live original callback resolving handles, or prove the clean-car strength/uniform values.

Private `paint-general-bank-inventory01.json`, `paint-general-bank-resolution01.json`, `paint-general-texture-binding-table01.json`, `paint-general-table-caller01.json`, `paint-general-bindings-code01.txt` and read-only `paint-general-bindings-ghidra01` preserve precise hashes and linkage. General high SHA8bf605b1.../low87ba56a8...; no assets or derived original code published. The renderer still does not apply these damage-controlled normals/scratches globally to an intact car. Next unresolved material step is production scalar-to-float4 packing, followed by an optional matched specular experiment; normal/scratch strength must retain the original per-vertex damage controls and the engine agent's damage-update contract.


### Base-specular alpha pilot (initial draft; CLI default off)

Build with optional grid-app feature `paint-specular`; the ordinary/phone feature set remains unchanged. `--body-specular source-base` stages each car's own packed alpha, preserves all authored mip levels, encodes sqrt(A) into an otherwise white linear BGRA image's alpha, and sets the base reflectance scalar1.0 to compensate Bevy's half factor. This authors pointwise dielectric F0=.04*A; coat, roughness, albedo and glass are unchanged. `white-control` supplies white alpha through the same GPU path to test compensation against feature-off. No launchers enable the draft and no claim of GPU/appearance acceptance is made yet.

Bilinear/mip filtering is a known approximation, and original alpha participates in multiple terms rather than this sole modern base Fresnel parameter. Do not call this an exact GRID shader port or infer physically metallic paint from the packed file name. Per-model/livery staging stays atomic and immutable, shared bounded validation rejects unsupported masks, shared P8 linear upload runs after device selection. Acceptance requires CPU tests, default/feature compilation, native4K prior/white/source comparisons, forced RGBA and motion/per-opponent validation. The accepted owner build remains available throughout.

CTAB cross-check of the exact2736-byte selected pixel program finds20constants and no stored defaults (all default offsets0), including the three Fresnel/specular float-vector registers. These absent defaults cannot be used as zero-valued production constants. The exact-family registry initializer0xc76810 writes the selector vtable/name into its static object; it is not recovered scalar packing. Private CTAB/registration/name-table artifacts retain scope.


Pilot CPU acceptance:4targeted tests pass after grouping paint-mask borrows to stay within Bevy's16system-parameter limit (first compile failure retained). Feature workspace661tests/12harnesses0failed/0ignored and all-target Clippy1m13s pass. Default-feature compilation/tests and GPU comparison still pending; no launcher/default change or prototype promotion. Parameter lookup0x4b23a0 is a case-sensitive linear name scan, not a proven compile-time hash lookup; do not search guessed CRC constants as if they were established input IDs. Private read-only paint-parameter-lookup-ghidra01.


### Pilot GPU acceptance — e19c3a5

Native4K off/white/source/forcedRGBA captures03 pass nonblank checks and synchronous offscreen compilation logs. Off and white-control equal the accepted80babb0prior PNG bytes exactly; compensation does not globally dull a white material. Source-base suppresses excess shine on paint pillars/frame; coat/roughness/albedo inputs/glass properties remain unchanged. The sample labelled glass also crosses painted frame and must not be interpreted as a glass-only measurement. Outside broad-car ROI tiny differences are measured, not causally attributed.

Source mask uploads linearBgra8Unorm11mips; forcedRGBA0BC, full-image meanabs0.088979byte/max150 versus BC, not pixel parity or phone validation.30unique virtual30FPSframes/1200ticks pass, per-car own masks logged;4exitrecords equal1dcontrol. New native4K6500tick run completes valid89.955257484second lap and4exitrecords equal31e74bfullcontrol. This is new pilot full-lap evidence; first/last moving and final full-lap images inspected. Private paint-specular-static-validation03.json/motion-validation01.json/full4k01-validation.json. The first blackoff01 is retained as a rejected startup capture; warm successful repeats do not prove an unprimed-driver-cache fix.

Dedicated graphics launcher will select source-base after this acceptance, with third argument `no-body-specular` for comparison. It omits the option entirely if the selected executable lacks the optional build feature; global CLI and phone defaults remain off. Actual extracted CMD capability block passes four old/new/comparison cases. Owner installation remains the earlier verified1dbuild until fresh peer-source integration/new promotion is complete.


### Combined F401 delivery and current scope

Installed desktop graphics source6aec3a9/SHAddf26b5b874c... after670feature-workspace tests, feature/default Clippy, canonical solo identity,20/20quicksoak and repeated native/moving/full-lap acceptance. Source-selected numbers, green ambient mask and alpha base-specular adaptation are selected by the graphics launcher when its executable supports them; global CLI and phone/default Cargo features remain off. Third argument `no-body-specular` disables the new mask for comparison. Older executables omit unsupported flags automatically. The current installed revision/hash is recorded separately in ignored graphics-current-build.json; merged source can be newer than that executable.

All four native4K modes04 equal the accepted e19mode images byte-for-byte after F401 integration. Thirty-frame motion and new6500tick full lap preserve4physical exits; lap89.955257484s. Moving1024probe+source masks+HUD passes30frames/physical identity/two HDRreadbacks;8Kbody+numbers+bothmasks+forcedRGBA0BC passes native4K, sourceDDS unchanged. First/last/full-lap/8K frames inspected; no claim of phone rendering, calibrated reflection radiometry or exact original lighting.

F402–F404input/Android/robustness sources are being retained after desktop promotion; the installed Windows binary is the verifiedF401snapshot, not relabelled as those later changes. Static normals cannot be restored by an all-on draw shortcut: the179enabled paint vertices share the spoiler draw with822disabled vertices; every other paint draw lacks an all-enabled mask. Future source normals need an interpolated dedicated control plus the damage-update contract. Keep original vertex colours separate from albedo.


### Red versus pink rear paint: bounded controls (G196, 20 October 2026)

Owner's original footage shows a more saturated red than the prototype. Four matched **3840x2160 SMAA** rear views isolate the additional authored paint environment gain (0/0.5) and exposure (EV100 13/14), using immutable source4faf708d, original sun/sky, source paint masks and lamp signals off. No launcher, default, texture or Rust code changed. Gain0 disables only the additional paint layer; StandardMaterial lighting, clearcoat and the local probe remain active.

Two fixed red-only native-pixel ROIs (boot100x80 and bumper100x70) provide display-RGB diagnostics. HSV saturation of the mean boot RGB is0.569 at gain0.5/EV13,0.595 at gain0/EV13,0.666 at gain0.5/EV14 and0.687 at gain0/EV14. The bumper gives0.538/0.565/0.637/0.663 respectively. Thus this added reflection layer accounts for a small saturation change in this pose; disabling it does not remove the pink appearance. One-stop lower exposure produces a larger saturation change but also darker paint and road. Asphalt mean RGB falls126/114/101 to95/84/71, while the separately rendered sky ROI changes by less than0.04byte per channel. Darkening the whole camera is not a demonstrated route to the original bright red.

Current enhanced authored grading explicitly sets post_saturation0.9 (track-grade mode instead sets1.0). Bevy's local0.19.1 shader confirms1 is neutral. This deliberately reduces saturation, but no isolated GPU control yet measures its contribution. Current AgX and the original selected colour-balance shader also differ; selected original paint fragment has no separate team-colour uniform. These facts do not establish a single root cause or recover the active original grade/paint constants. Historical menu/video records both name Nevada Autosport; the current matching Champion/Lunati livery is not established as a wrong-livery bug by this investigation.

Original AV1 recording metadata is8-bit yuv420p, limited-range BT.709 primaries/transfer/matrix (FFmpeg enumeration1 in each field), inspected with existing PyAV19.0.0. It does not declare HDR/PQ, but this is metadata, not display calibration or proof of a colour-correct capture. Camera, light and post processing differ between it and our controls; no absolute RGB-match claim is made.

Private body-red-diagnosis01 retains exact commands, four successful GPU-readback logs, native-roi-comparison01.png, roi-comparison01.json, surroundings01.json and video-colour-metadata01.json. Python3.12.14/Pillow12.3.0/NumPy2.3.5; no installed tools. Existing781resource tests, both workspace Clippy variants, canonicalF455 identity and quick24/24 from G195 still cover unchanged Rust. Next: isolate saturation/tone mapping in a reversible paired renderer diagnostic before choosing a material or grading change. Preserve accepted reflection detail and avoid baking a global red tint into the purchased livery.


### Original named material loader and vector packing (G197)

Read-only Ghidra12.1.4 resolves the earlier uniform setter candidate9e7360 through its actual callerA10310 and the shared35-name material tableE4FD58. Independent Capstone5.0.6 verifies bounded source instructions against the pinned PE SHAaac46a...34514. The table's index0 is Car Body Paint; it also explicitly includes lamp pods/covers, glass, wheels and tyres. This avoids borrowing similarly named properties from unrelated scenery shaders.

The confirmed99c300 reader initializes nine floats for each named material before attempting its named XML properties. Its static fallback tuple is: specularHighlightPower300, specularHighlightStrength64, metallicSpecularPower10, metallicSpecularScale4, metallicFalloff0.2, occlusionMapScale1, fresnelPower2, fresnelMin0.1, fresnelMax0.9. Exact f32 words/addresses remain in private defaults01.json. These are **loader fallbacks**, not measured live Lacetti values or direct Bevy PBR constants. Missing loose XML alone does not prove they are the final values; packed resources and other initialization/overrides remain possible.

The9e7360 path copies the selected nine-float record, matches material names, then conditionally submits SpecularValues=[highlightPower,highlightStrength,self+4,self+0], MetallicSpecularValues=[metallicPower,metallicScale,metallicFalloff,occlusionScale] and FresnelValues=[fresnelPower,fresnelMin,fresnelMax,0] through B9DD10. The meanings and initialization of self+0/+4 are still unresolved. It also writes explicit DebugScratch/Dent/Crack/MudOverride fields; this path contains reload/edit controls and must not be assumed to describe every ordinary production update. Its input fields and ordering are verified static linkage, not captured active D3D constants or complete material equivalence.

The shared9cbde0 function initially considered a loader is actually an **export writer**: it formats nine property values, serializes the named material records and emits cars/materials_%s.xml. The corresponding reader99c300 confirms the same field names. Bounded readonly enumeration finds no loose cars/materials*.xml in this installed copy. Do not invent those files or modify Steam to force a presumed source value.

Private body-uniform02 retains string-pointer candidates (including an internal suffix occurrence of SpecularValues, which has no pointer references), independently resolved Ghidra references, two owner functions, reader decompilation, exact constants/property names and bounded instructions. Original-derived listings remain private. First generic instruction tool failed because this worktree's local tool path lacks Capstone; retry reused C:/Dev/Reassembly/local/tools/python/Capstone5.0.6, no installation. No approvals rejected, game launch or renderer/default changes. Next: trace the reader's actual caller/resource and self+0/+4 initialization, or capture active original constants, before selecting an opt-in shader adaptation. This advances source material understanding; it does not close the red/pink or original BRDF gaps.


Follow-up caller/constructor boundary:99c300 has a direct global reload caller9cbce0 opening cars/materials.xml and a named-node dispatcher9ca3a0 for materials. The latter entry is independently delimited by preceding ret/padding, decompiled only in read-only session memory and discarded. This establishes a material-node parse path, not the active Lacetti resource identity. Existing owner constructorA135C0 also initializes self+0=0.5 and self+4=0.25, and the same nine current scalars; these extra values are constructor defaults, not recovered semantic names or immutable runtime values. Earlier sentence about unresolved initialization is superseded by this bounded constructor observation; their full meaning/setters remain open.

Important control: the already recovered **saved Lacetti instance** uses FresnelPower1/Min0.3/Max0.6 and neutral EnvironmentColour~0.537255, whereas the newly read XML-loader fallbacks use2/0.1/0.9. They demonstrably differ. Do not promote the XML fallbacks into a "source Lacetti" mode or assume the conditional override writer is the ordinary shader-parameter binding path. The selected shader uses a nonlinear angle function, while our environment-layer angular mix remains authored; exact saved-scalar to vector packing outside that override path is the remaining target. Initial exploratory disassembly began mid-instruction at9ca320; its leading fragment is rejected, subsequent9ca3a0 entry has separate boundary/Ghidra proof. No appearance change made.


### Single-variable post-saturation and source-sun controls (G204, 20 October2026)

New default-absent `--post-saturation 0..2` isolates the authored camera's perceptual post-tonemapping saturation. Enhanced authored grade only; finite/range checks and --grade track combination reject. Temperature/tint, exposure, paint RGB/masks, clearcoat and reflection gain stay fixed. Bevy0.19.1 tonemapping_shared.wgsl applies this parameter after the tonemapper. This is a diagnostic control, not a new original-colour shader or a retuned default.

Six matched native3840x2160/SMAA static rear-quarter cases use the established G196 camera/source-sun direction/EV13/paint environment0.5: absent(default), explicit0.9,1.0,1.1, plus a separate existing source-sun-colour0.9/1.0 pair. Default, explicit0.9 and the old G196 baseline **entire PNGs are byte-identical**. This establishes default preservation in this pose/profile, not universal GPU determinism. All six readbacks succeed; full/native paint comparison inspected.

Boot red-only ROI2400,1500..2500,1580, mean displayRGB/HSV saturation:

| Case | Mean RGB | Saturation of mean | Hue degrees |
| --- | --- | ---: | ---: |
| White sun, default0.9 | 191.86,82.67,95.27 | 0.569 | 353.08 |
| White sun,1.0 | 198.00,77.46,92.27 | 0.609 | 352.63 |
| White sun,1.1 | 203.87,71.80,89.16 | 0.648 | 352.11 |
| Saved sun colour,0.9 | 191.73,80.63,88.34 | 0.579 | 355.83 |
| Saved sun colour,1.0 | 197.94,75.29,84.46 | 0.620 | 355.51 |

Saturation increases chroma but hardly corrects hue and slightly moves this ROI further toward magenta; **it is not a complete red/pink fix**. Existing saved sun ratio[1,0.85882354,0.57254905] moves hue toward red, modestly, while also warming road/other lit objects. Its linear interpretation and radiometry remain authored adaptation. These are separated controls, not an assertion that changing a global filter alone recovers the original material.

Surroundings controls: road meanRGB126.12/114.45/101.48 becomes127.16/114.25/99.67 at saturation1.0, versus124.44/108.06/86.40 with saved sun colour at0.9. The latter changes lighting colour even at fixed scalar lux/exposure; it is not a luminance-preserving operation. Sky means shift very slightly, so no final-image sky byte-invariance claim. Both red ROIs/selected sky/road regions have zero channels at0/255 in these cases, not proof of no clipping elsewhere or in HDR. Original video pose/light/grade differ; no calibrated RGB fitting or absolute colour parity claim.

All781resource tests/12harnesses,0failed/ignored, feature+default workspace releaseClippy, canonicalF455/quick24/24 pass; five executable guards reject NaN/negative/>2/non-enhanced/track grade. No new unit tests that mirror this reversible parameter were added. Release3m12s/onejob, SHAf41d35994199ef83d9a901bd905b0b578d5a69f72e64519907205b7a1bd15eee; exact frozen source patch verified. Rust1.99.0/Bevy0.19.1/wgpu29.0.4, existing Python3.12.14/Pillow12.3.0/NumPy2.3.5, no installs/dependency version/cache changes. Private body-saturation01 records commands/source/artifact/ROIs/checks, controls and surrounding statistics. Uncited mis-sized first comparison sheet removed after guarded path check; corrected sheet retained. No renderer failure or approval rejection.

Existing Signals and separately delivered DLAA L builds remain unchanged. Owner L feedback is optional/pending. Next isolate the tone-map contribution versus source material response before choosing a permanent grade/material adjustment; preserve accepted reflection detail and keep any original-style global colour work explicit.

### Windscreen outline through rear glass (G224, 22 October)

The owner reports an overly contrasting black front-window outline through the rear pane;
rare side-view trails immediately underneath the car are explicitly deferred. Four fresh
1920x1080 SMAA captures reproduce a similar contour on undeformed static geometry. Neither
DLAA nor damage is required for this appearance. Glass reflectance0/0.5/1 controls change its
contrast; no endpoint is promoted as a correction.

The six Window Glass draws are separate, with white source vertex colours. A rear-centre CPU
ray crosses MPJRI306 (rear) then the back face of MPJRI311 (front). At a selected perimeter
point the original atlas sample is RGB0/alpha255, while the rear clear-pane sample is
RGB38/alpha165. Some side-edge rays also cross cabin structure. The perimeter is authored
source content; removing it or hiding the front pane would conceal legitimate geometry.
CPU rays/base-mip samples do not establish GPU visibility or exact filtered values.

The model stub `carglass.fx` defines saved window environment RGB0.48627454, Fresnel
power1/min0.3/max0.7, specular power300/strength64 and occlusion scale0. The shaderpack's
separate `carglass` group links the previously disassembled glass-window fragment shader;
its saved main colour pass blends source alpha, has no culling and writes depth. These are
static saved states, not a capture of the selected original runtime variant. Generic Bevy
Blend/PBR does not implement that complete response. Bevy0.19.1 sorts transparent parts by
transformed mesh AABB centre: common transform origins alone do not establish a sorting bug.

Private evidence: `local/graphics/window-through01/`, including exact source parameters,
source texture alpha, matched captures, CPU hit/UV reports and reflection-control crop.
Next compare inside/outside reflection and opacity, then isolate depth-order differences;
keep source borders and cabin, and test exterior/damaged views before a playable change.
No renderer, texture, geometry, AA default or accepted Motion launcher changed in G224.

### Isolated static response controls (G225, in validation)

`--glass-pilot off|control|environment|depth|combined` is a default-off research mode for
static, rendered, enhanced Donington Lacetti LOD0 reference scenes with source IBL and the
forward renderer. It rejects driving: the current damage presenter swaps cracked and reset
`StandardMaterial` handles, so substituting an extended material without integrating that
ownership would break the visual damage contract. This experiment does not change a launcher.

The control wraps the same base material and keeps RGB/alpha, textures, two-sided culling,
geometry and shadows. Environment blends the existing local/sky probe radiance into the
lit colour in exposed linear HDR before stock fog and tonemapping, with constant weight
0.24313727 (saved window environment0.48627454 times the0.5 midpoint of the saved Fresnel
bounds). This is a bounded independently authored experiment, not the original BRDF:
source direct specular, dust, face-dependent shadow terms and live parameter selection are
not reconstructed here. No source alpha reduction, border removal, pane hiding or texture
replacement is used. The depth variant separately enables depth writes in the otherwise
unchanged alpha-blended pass; combined enables both variables.

Acceptance starts with exact neutral-control/stock and new-stock/accepted-executable image
parity, followed by rear/front/side/inside comparisons. Depth writes and an environment lobe
must not be conflated into a single causal fix. Only after those results should a selected
response be integrated with cracked/smashed/reset states and validated in motion/native DLAA.
At this checkpoint compilation and GPU checks are pending, and the owner-approved Motion
executable remains unchanged.

Validation completed for this static pilot:281 resource-enabled application tests, default
and DLAA/paint-feature Clippy, feature build,8executable guard negatives and11native1080p
SMAA captures pass. Neutral extension matches stock exactly; new stock matches the frozen
accepted executable exactly. Rear/front/side/inside images were inspected. Rear depth-only
changes1296pixels (full-frame mean absolute difference0.002843byte); environment-only
changes33607pixels (0.22658byte) and softens the outline's contrast. This selected view does
not support a wholesale sorting failure, or prove depth is irrelevant in every view.

From the cabin the all-face draft adds a visible reflected exterior wall because the local
probe excludes the car/interior. This is a physical proxy limitation; the owner subsequently
likes the exact inside-combined frame. Preserve that variant rather than automatically
suppressing the inside response. The research camera is not a finished cockpit and driving
integration is still pending. The graphics author selects environment-only (middle in the
rear triptych) for further4K/DLAA comparison; depth-only is visually close to stock there.
Keep the depth experiment independent, and isolate its inside contribution next. No
accepted driving material or owner executable changed.

### Native4K/DLAA and independent inside depth (G226)

All13static renders succeed, with effective native DLAA verified for DLAA cases. Rear
3840x2160 controls cover off/environment/depth/combined; native4K SMAA covers off/environment.
Additional controls cover inside neutral/environment/depth and front/side DLAA off/environment.
Images were inspected. Environment remains the rear appearance candidate; inside depth
also changes dark upper-window overlaps. Preserve combined as the owner-liked reference.
Static success does not establish motion history, original parity or damage compatibility.

The matrix's final strict neutral-inside comparison failed, and its failed result/logs are
retained. An extra matched stock render also succeeds but differs from the neutral material
in25RGB channels/23pixels by exactly1byte each (mean absolute0.0000040188byte). Old60versus
new120benchmark frames is not established as the cause. This is accepted as a bounded static
neutral difference, not exact image equivalence or a fully green runner. Earlier exact rear
parity is scoped to its own G225 view. Private evidence: `local/graphics/window-response02/`.

Before driving promotion, retain the existing source material handle as the owner of crack
and reset swaps. An optional rendered child can share the source mesh, follow deformation,
transform and inherited visibility, and select a cached extended material from that source
handle. Validate intact/cracked/smashed/reset transitions, probe exclusion and static output
before replacing any owner-playable build.

### Render proxy preserves damage ownership (G227)

Optional window rendering now uses a child of the existing source draw. The source retains
its `Mesh3d`, ordinary material and damage-owned handle. The child shares that mesh, follows
its bounds, transform and inherited visibility, and selects a cached extended material after
intact/cracked/reset handle changes. The cache assumes the existing damage protocol swaps
handles rather than mutating a cached source material in place. Source render layers are
cleared before visibility extraction; only the child renders, on a layer excluded from a
local scene probe. Without a local probe it uses the ordinary main-view layer.

The281application tests pass, including the transition test; default/feature Clippy and the
feature build pass after the documented fixture and query-lint corrections. Nine intended
static captures plus a repeat render successfully, with exact source-probe readback parity.
Rear off/control/environment/combined and inside/front/side combined match G225 PNG bytes.
The neutral inside differs by the earlier23pixels at1byte. One initial stock render has an
8pixel/max19byte outlier; a repeat matches G225 exactly. Its cause remains unresolved and the
original strict failure is preserved, not recast as a fully green runner. Private evidence:
`local/graphics/window-proxy01/` and `review.json`.

The static-only CLI guard still blocks driving. These results establish static appearance
and ECS ownership transitions; actual rendered cracked/smashed/reset states and motion remain
open. The owner-approved Motion DLAA executable and launcher are unchanged.

### Bounded driving and actual crack event (G228, validation ongoing)

The pilot now permits rendered bounded `--drive-ticks` as well as static reference captures;
unbounded interactive driving remains rejected. Default crate prepush passes281tests,
Clippy and sanity in347s; feature Clippy and feature build pass. Three actual negative CLI
checks reject unbounded driving, missing render output and an unsupported car.

Off/environment/combined each finish1800ticks at native1080p DLAA, virtual30FPS, with the
accepted gentle motion shutter. All four physical exit records,30frame manifests and the
sampled motion/depth buffer match exactly. Selected first/last frames and the environment
final frame were inspected; this does not establish every-frame history or monitor acceptance.

Paired1200tick stock/combined collisions produce the same front-window Cracked event at
tick1024 and identical four physical exits. The final wreck menu obscures the glass, so this
is event/physics evidence, not visual cracked-material verification. A paired front-view
sequence around that event is next; rendered smashed/reset coverage remains open. The
accepted Motion executable is unchanged. Evidence: `local/graphics/window-drive01/`.

The first front-view follow-up finishes normally with identical events, physics and frame
manifests, but a fence occludes the window before the crack. Its intended visual coverage
is not established. Preserve that evidence and retry a raised track-side diagnostic view;
no renderer failure or cracked-glass acceptance is inferred from the successful process.

The raised track-side pair subsequently completes normally with equal events, four physical
exits and40frame manifests. Stock/combined frame17 (tick1027) and the combined final were
inspected: the damaged front pane remains visible with its crack pattern; side reflections
also respond. This covers one front-Cracked collision, not smashed windows, rendered reset
or interactive acceptance. Earlier obstructed camera evidence is retained.

### Cloudy and mostly clear glass comparison (G230)

Six native4K DLAA renders compare rear, side and inside views with the same combined glass
pilot, camera, Donington direct light/tone and material settings. Original cloudy Donington
sky is paired with a visually checked mostly clear Long Beach sky, including sparse small
horizon clouds. Both visible sky and IBL use the substituted complete sky asset; scene probe
readback succeeds for six nonzero faces, with zero upper sky misses in both assets. Source
sky hit counts differ because their domes differ. The donor panorama also has its own sun
and horizon radiance: this is an appearance comparison, not reconstructed physical weather.

Reviewed side glass shows soft grey cloud variation versus smoother blue-grey reflection;
the owner-liked inside wall reflection remains. Commands, full images, crops, source hashes
and private read-only asset overlay are under `local/graphics/sky-glass01/`. Original assets
and accepted owner executable are unchanged. The runner's Pillow import failed after the
six successful renders; composing the saved outputs with bundled Python/Pillow succeeded
without rerendering. Preserve both the failed runner result and separate composition/review
results. Never deduplicate or recursively edit through the asset overlay's links.

### Rear-outline repeat control during the reflection build (G234)

An intentional repeat of frozen stock/off driving matches physics,30frame manifests and
sampled motion/depth. In the selected rear-window ROI, frames0/29 differ by mean.0220/.0193
RGB byte from the first off run (max3/4), compared with mean3.7455/4.3092 and max99/101 for
depth-only. Thus the local window response exceeds the measured repeat noise. This is not
universal frame equivalence: small background variation and sparse larger outliers remain.
Private evidence: `local/graphics/window-outline-repeat01/repeat-noise-review.json` and
`local/graphics/window-outline01/difference-regions.json`. No depth fix is promoted yet.

The already saved original window fragment also couples output opacity to reflection,
highlights and GhostCar controls. A conditional dust-free/no-highlight check can raise a
clear-pane alpha from165/255 to.823529 with illustrative factor.5 and assumed controls1;
these are not live uniforms or a material correction. Static GhostCar initialization from
the existing lamp research is insufficient to choose runtime opacity. Keep source atlas
and border unchanged while identifying the actual missing response.

The subsequent neutral Control moving test isolates the new pipeline/proxy: selected
Off-Control window mean differences are.02231/.01954byte(max6/4), while Control-Depth is
3.74409/4.30970(max99/101). Physics and30frame manifests agree. Thus the observed local
change is associated with the depth-write state rather than the neutral wrapper. This does
not identify the precise rejected fragment or downstream consumer. A separate22446ray CPU
survey of160glass triangles at the saved static rear camera finds no pane-order reversals
or same-pane multiple intersections. Do not infer a six-pane sorting bug from these images.

### Pane-only Fresnel candidate (G233)

The new default-off `--glass-pilot fresnel` strengthens the existing dielectric response
before one stock PBR lighting evaluation. It does not mix raw probe radiance over the whole
lit window. Default clear-pane F0 changes.04 to.08, capped.16. The stored IOR is unchanged;
this is an artistic candidate, not physical IOR1.5 or an original shader conversion. The
atlas-specific gate uses the saved clear alpha165/255 and fades to zero at opaque alpha1;
it changes only the reflectance increment, not texture RGB, opacity, geometry or frame.
Existing modes remain selectable, including the owner-liked combined inside reference.

Default crate gate281tests/Clippy, feature Clippy and build pass. Ten native4K DLAA captures
cover old/new off and combined controls plus rear/side/inside under cloudy and mostly clear
sky; all corresponding source-probe readbacks match G230 exactly. Cross-build PNG controls
have small nonzero differences (off mean.004352/max35byte; inside combined.002949/max17),
so full byte identity is not claimed. The initial negative CLI harness failed before GPU
because inherited lamp flags triggered their earlier guard; minimal isolated guard inputs
correct that harness. Preserve original failure and corrected results separately.

The owner approves the new side crop. Two1800tick moving controls match four physical exits,
30frame manifests and sampled motion/depth; an actual front-Cracked shaded case matches its
stock events/physics/40frame manifest. Selected motion frames0/29 and shaded frame17 were
inspected: the previous uniform paleveil is absent there, while the source crack pane stays
visible. The rear outline remains a separate unresolved issue. Rendered Smashed/reset,
all-frame temporal behaviour and owner monitor acceptance are not established. No accepted
Motion launcher or default glass response is replaced. Private evidence:
`local/graphics/glass-fresnel01/`, including build-provenance.json and motion-review.json.

### Moving transparent queue — G237/G238

A one-frame main-view `--window-phase-audit tick:1652` revealed that all six player-window
world centres in Bevy's retained transparent phase were420..424m from their current world
AABBs. Rear306 was queued before front311. The same defect occurs with ordinary materials
and neutral extended proxies; enabling depth writes hides some symptoms without updating
those centres. Static CPU pane order alone could not expose this moving-frame problem.

Optional `--window-sort off|current` (off default) now submits current LOCAL AABB centres
every frame through the existing MeshStream adapter, which applies the current world
transform before sorting. The actual rendered entity is selected once: source with the
material pilot off, proxy otherwise. Materials, alpha, depth, shaders and geometry are unchanged.
The candidate is scoped to bounded enhanced forward Donington Lacetti LOD0 captures.

Three native1080p/virtual30 moving controls have equal physical exits, frame manifests and
sampled motion/depth. At the sampled phase, both ordinary and Fresnel current modes match
world centres exactly to logged precision, with front311 before rear306 and depth writes
still disabled. Selected native frames0/29 show softer rear-through-front contour because
the rear pane overlays the front border. The physical source border remains. An actual
front-Cracked shaded scenario preserves damage/physics and its visible crack. No claim of
rendered Smashed/reset, manual R/C, all-frame temporal correctness or other-car acceptance.
Private trace/captures: `local/graphics/window-phase01/`, `local/graphics/window-order01/`.

### Bounded real C/R transitions — G239

`--window-transitions FRAME:C,FRAME:R,...` is a default-off offscreen input diagnostic.
It submits real keyboard pulses to normal action/reset handlers, with a monotonic Extract
frame clock, bounded duration and at most one reset. No alternative camera/reset is added.
A real InputPlugin/Actions fixture exercises key edges even when synthetic ticks revisit
an old value; actual GPU runs separately verify the ordinary runtime reset.

Six native1080p/virtual30 runs exercise C90, R180, C240 with one300frame sequence. Normal
consumers change view at tick185/125; R advances segment and resets tick0. All physical
exit records agree. Chase phase audits before/after C/R and at exact reset tick0 each
contain six unique panes with logged centre error0 and unchanged depth state. The motion
sequence skips exact reset tick0 and resumes at tick2; that exact RGB frame is unverified.
DLAA logs prove a history-reset request, not an independently acknowledged NGX dispatch.

Selected images expose a separate persistent opaque own-body obstruction in bumper view.
It reproduces in accepted G222 Motion without any new window flags and in frozen G238;
G238/G239 initial-bumper PNGs are identical. Source front-bumper geometry is ahead of the
camera eye beyond near0.2. Original own-body visibility rules remain unresolved; no whole-car
hide or arbitrary camera offset is promoted. Thus machine checks pass but universal visual
C/R acceptance is not claimed. See private `window-transitions01/` and `window-bumper-baseline01/`.


### Parameter storage resolver boundary (G261, 24 October 2026)

One targeted shared-master analysis of `0x00B9F090` narrows the named-vector assignment path. The collection exposes either contiguous 0x50-byte entries or a pointer array. A pointer-array write can allocate a missing entry when explicitly requested; existing entry pointers are returned unchanged. This function does not allocate the backing collection or check the index itself; the previously traced caller checks the index first.

The conditional override writer assembles its vector before reaching this resolver, with creation enabled. Within this resolver there is no parent/default lookup, scalar-to-vector computation, value merge or copy-on-write branch. Prior population may still establish aliases or inherited values elsewhere. Constructor initialization of new entries remains outside this bounded analysis.

This closes a storage question, not the clean Lacetti production writer or active constants. It does not establish a cause of the pink appearance, so no paint, exposure or tone-mapping default changes follow. A superficially similar callback was excluded after its actual construction identified GhostCarValues. Further work needs ordinary collection population or active-value evidence rather than another traversal of this setter.

Private evidence: `local/handoff/drafts/paint-next02/binding-next.md` and `resolver-b9f090.md`; one 120-byte shared-cache decompile, independently checked against the same bounded assembly. No original-derived listing is published.

### Rendered cracked-window reset (G299, 27 October 2026)

The existing native1280 Lacetti/Donington Fresnel/current-sort fixture now has a bounded actual R lifecycle check on G298's checked executable, without source changes. The first run correctly failed qualification: requests1031/1033 preceded actual frontCracked1037 even though asynchronous readback completion appeared later. Exactly one timing correction to1040,2 produced two visibly cracked windshields before R510, followed by an intact windshield/restored body in the final new-segment frame. Logs show ordinary reset oldtick1060, newsegment1/tick0/history-reset request, then clean exit71/zero damage and no new same-window crack. Both renderer processes exited0; the first helper failure remains preserved.

This closes the selected rendered cracked→reset case left open in G233/G238. It does not establish Smashed behaviour, every proxy/part lifecycle, exact reset-tick0 pixels, per-image GPU-stage joins, SDK dispatch acknowledgment or physical Manual keyboard C/R. The test explicitly uses body-specularOff/paint-environment0 for the current dlaa-only build, so historical full-body pixels are not a control. No glass optics, haze, paint or playable defaults were changed. Exact source/artifact/recipe/log/PNG/visual-review and independent audit hashes are in G299; full private evidence stays under local/graphics/window-cracked-reset01/.

### Ordinary playable current-centre delivery (G313, 28 October2026)

The owner G312 Reinhard helper did not select G238 current sorting, and its guard refused ordinary Window driving. Four fresh same-artifact controls cross source/authored sun with stale/current sort: physical streams, capture ledger and sampled motion/depth agree exactly; stale six-pane centres are421.67..424.61m away at tick1652 and rear/front order is reversed, whereas current centres have zero error with front before rear. Current ordering visibly attenuates the overdrawn front border in both light settings; the source frame remains physically visible. Light can change contrast, but does not explain this measured stale-centre defect. Evidence: G313 and private local/graphics/window-regression01/run01/.

Ordinary direct enhanced/layered Lacetti Donington0/sub0 LOD0 forward driving now automatically selects the existing updater unless --window-sort is explicit. Explicit off remains the comparison control; existing bounded capture defaults remain off. No changes to glass textures, alpha, depth, Fresnel response or geometry. G314 closes actual new-artifact466tests/Clippy/build and ordinary Window auto-activation/Reinhard0.9/nativeDLAA/normal exit0, with the same private Reinhard launcher updated. Manual C/R and broad car/route/damage acceptance remain open.

G316 delivery clarification: the first negative follow-up was found in the old parent AgX3503 session, with no current-sort activation; exact screenshot/session join remains unproved. Both private AgX/Reinhard launchers now use the same checkedc833 executable while retaining their tone arguments, and print its build identity. Full contour/all-angle acceptance stays open. Saved original main glass is two-sided, so hiding the inward front pane is not justified by this feedback. G316 records source-contract limitations and old-reference relocation.
