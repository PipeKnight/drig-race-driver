# Asphalt and Lacetti reflection balance

19 October 2026. Owner asks to verify whether asphalt is too mirrorlike and the car needs stronger reflections, rather than tune solely by impression. Performance and crown detail remain outside this check.

## What was compared

Installed runtime3bfc2d17c159, SHA bbc1dc552cdd0d0256033f7b08681b3b60750fdd8af85908e1eb52db5a6cd053. Three new matched native3840x2160 static reference images use eye(-191,-10.5,22), target(-183.119,-11.5,19.412), source sun/sky, fixedEV13/AgX, identical body masks, geometry and paint parameters. Controls: sky+road-maskOff, sky+road-maskSourceBase, static512scene-probe+road-maskSourceBase. All exit0/noGPU errors; probe has a verified HDR readback,1copy and ready=true. No engine/source/default-launcher changes or new tools were needed.

Also inspected the already verified matched sun-facing native4K off/source-mask pair from ground-aa01, both the same3bfc runtime. Private new evidence: local/graphics/asphalt-paint-check01/{captures01,analysis01}.json and asphalt-native-comparison01.png / paint-native-comparison01.png. Crops are native pixels, without resizing, relighting or sharpening.

Original references: owner's recording overview frame-0070.000.jpg, frame-0100.000.jpg, frame-0225.000.jpg under private primary local/recordings/donington-race-20261009/. Original asphalt retains warm, visible road grain and markings; paint has distinct highlights. These are qualitative references only: original camera/time/exposure/grade/video encoding are not matched to these prototype stills. Do not derive an original-equivalence score or a numerical target reflectance from them.

## Asphalt finding

Default source-specular mode isOff, so the original diffuse-alpha specular weight is omitted despite restored diffuse/macro/skids. The independently examined source shader weights its direct Phong specular by the mixed diffuse alpha. Saved source diffuse alpha base-mip means: track1=15.085/255, track3=15.084/255, track4=14.400/255, track2=2.500/255. These low masks support a materially weaker road lobe; they are not roughness, opacity, or a claim our rendered road is17times too bright. Original Phong/SpecScale/Fresnel and Bevy GGX are different.

Existing --surface-specular source-base applies the mixed mask to GGX baseF0 (after texture filtering/blending). In the sun-facing matched view it removes the broad pale wet-looking veil and restores asphalt detail; dry grass remains matte. The native road crop changes strongly (mean absolute RGB code difference41.68), which is an image diagnostic, not a physical reflectance measurement. Near the start-line camera the reduction is less dramatic, confirming the angular dependence.

Recommended first road correction: retain authored roughness/lighting while using this source mask, rather than flatten every road into fully matte material. The adaptation also affects base environment reflections and is not exact original direct-Phong parity. It is already selectable in primary Play GRID Graphics Surfaces.cmd. Ordinary Play GRID Graphics.cmd and its default settings are unchanged in this check. Further default promotion should retain curbs/paint distinctions and validate moving low-angle views.

## Paint finding

Current Lacetti paint has authored perceptual roughness0.30, clearcoat0.20, coat roughness0.18. The source-alpha body base mask is enabled by the launcher; glass uses separate parameters. Original runtime scalar packing is still unresolved, so these are not recovered GRID paint constants.

Sky-only versus512scene-probe changes hood/fender/glass response, but the difference is relatively modest in this one camera (mixed body ROI mean absolute RGB code difference3.80). This does not establish a universal cause or prove increasing all paint reflectance is correct. The probe experiment also changes nearby scene IBL within its volume, so the ROI is not an isolated BRDF measurement. Do not compare its difference number directly with the road crop: different regions and different experiments.

Next controlled paint test: vary only clearcoat strength and then only coat roughness, preserving source base mask, albedo, exposure, light, glass and dry road. Inspect sun-facing and shaded sides, hood/roof/rear in several camera angles and motion. A stronger, better-shaped lacquer highlight may improve body readability; a global base-reflectance increase could reintroduce bright black pillars/trim that the masks fixed. No new paint setting is accepted from these static controls, and local512reflection quality/radiometry is not promoted to the normal profile.

## Follow-through

Documented source-mask result is the first road candidate. No default runtime change was made while checking the owner's impression. The existing primary Surfaces launcher offers the exact road-mask switch for comparison. Next work remains focused: verify road-mask motion/default promotion, then a bounded clearcoat A/B; AA motion/history work remains on the backlog. No performance optimization or unrelated camera-feature work.

## G298: bounded authored roughness trial (27 October 2026)

The owner's later request is to retain matte asphalt near the car while recovering restrained sun response at grazing angles. This is distinct from motion-history/detail-boundary work. A default-off host-side trial exposes only Off,0.70,0.65 on audited standalone Donington road definitions: both diffuse layers must be don_track1..4, both dry-ground flags zero. It changes the cloned base perceptual roughness before static batching; the existing source-alpha F0 mask and all RGB/UV/texture/motion arithmetic remain unchanged. Counters describe original source materials/draws before batching, not GPU draw calls.

The ordinary nonpaint source base uses perceptual roughness0.70. Thus explicit0.70 is a material-assignment control;0.65 is a small authored lobe change. Neither value is recovered original GRID roughness. Exact Off/control image equality must be measured, not inferred from those numbers. The planned three sun-facing static captures preserve camera, source sky/sun/IBL, raw sky tone, EV13, grade and SMAA at1920x1080. Historical4K images are qualitative recipe provenance, not a pixel-parity target. These were the pre-run constraints; actual checks and image results follow below. No default, playable or native smoke profile is changed by this trial.

Actual G298 result: required tiercrate grid-app checks passed458tests/Clippy, native-feature Clippy and release; three native1920x1080 SMAA cases exited0. Both overrides select10source materials/54source draws before batching. Off and0.70 are PNG/RGB-exact.0.65 has a subtle positive road response (selected interior road ROI mean absolute RGB-code delta2.8293/max8); root sees no compelling improvement at full-frame scale. It stays default-off; no topdown/motion/multitrack expansion or playable promotion. Angular road response is still open. Full artifact/source hashes, failure history, independent audit, actual traces and unchanged-capture dedup are recorded in G298 and private local/graphics/asphalt-roughness01/. These are authored controls, not recovered original GRID constants.
