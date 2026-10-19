# Moving-asphalt detail discontinuity with DLAA

19 October2026. Open defect; no visual fix installed at this checkpoint.

## Reproduction and controls

Owner screenshot codex-clipboard-e9fc9913-6574-47c2-af87-e667a1e644d6.png shows a conspicuously sharper patch next to the player car and smoother road ahead near the Carlube bridge. Reproduced the same type of car-adjacent patch on delivered9abb921, SHA827049d70a69cbb78fd116d1b5cddf36b89fe3a51ad986d1876b7e88d48f9a1e.

Paired automated1800tick native1920x1080 captures use the same moving1024probe, paintgain0.5, source road/materials, chase camera, virtual30FPS and one opponent. Both runs exit0 with no shader/GPU errors and four identical physical exit records. Captured30motion frames beginning1650; final image is later at1800. Compared DLAA with SMAA: the sharp car-adjacent patch is visible in DLAA and absent in the SMAA control, whose asphalt grain remains more spatially uniform. Both full frames and native-pixel crops inspected. Private local/graphics/asphalt-distance01/{dlaa,smaa}/command.json, run.log, capture.png, capture-frames and controls.json preserve reproduction. asphalt-aa-native01.png provides an unscaled same-screen crop comparison. This is a nearby deterministic reproduction, not an exact replay of the owner's session or measured display30FPS.

This isolates the visible discontinuity to a temporal-AA-related difference in this setup. It does not identify the precise DLSS history/rejection/filtering cause. A plausible explanation is different treatment of newly uncovered road near the moving car versus established history; this remains an inference, not a measured internal DLSS state. Do not label it a repaired mip/LOD bug or a reflection-probe defect.

## Checks against simpler explanations

Enhanced graphics already uses anisotropy16 and Bevy ImageSamplerDescriptor::linear, whose installed0.19.1 implementation selects linear minification/magnification AND mip interpolation. Track surface texture creation uses this sampler through texture_upload::image/image_linear. The authored surface shader has no distance threshold that abruptly disables detail. Mip levels still legitimately reduce distant texture detail, but increasing anisotropy from1 to16 is not a fix:16is already active.

The surface extension explicitly samples repeating layers, macro colour and skid weights. Unlike stock PBR, these custom samples currently do not use view.mip_bias; installed Bevy DLSS sets that bias from its context. This is a separate confirmed integration difference and a candidate controlled experiment, not proof that it causes the near-car patch. Its numeric value in this run was not captured. A negative bias may introduce shimmering, so do not apply a global sharpening/bias change without motion controls.

## Motion/depth evidence and a false-positive mask

At frozen frame793/tick1652, near-ground/car-volume audit initially reports ground p99error37.7498pixels. Investigating rather than declaring all road vectors broken shows1074high-error pixels confined to x698..851,y558..588, overlapping the visible opponent in motion frame0000. The conservative car-local ground volume is not an object-ID mask and admitted part of that moving car. Preserve the failed broad result in dlaa/capture-temporal/analysis.json and ground-regions.json.

A geometrically selected road-only band y>=610, inspected against that frame and chosen to exclude the opponent's screen band rather than by error threshold, has15797samples. Residual pixels p50/p90/p99/max:0.02348/0.04556/0.06755/0.08852. Upper player-body audit p99is0.02018pixels. These support consistent reprojection on these selected regions; they do not prove every road pixel, every frame, subpixel texture stability or correct DLSS internal history. Private road-only-analysis01.json records the restriction explicitly.

## Next controlled change

Keep native DLAA and accepted reflection build available; existing third launcher argument smaa provides comparison without changing defaults. Next isolate custom-surface view mip bias and inspect road detail inside/outside the disocclusion region across a turn, then confirm stability at virtual30/300and native4K. If bias does not address the patch, investigate history/disocclusion handling instead of increasing asphalt resolution or adding compensating blur. Do not tune the accepted paint reflection gain for this defect.

No Rust/source shader/runtime/default or primary peer files changed in this diagnostic stage. Python3.12.14/NumPy2.3.5/Pillow12.3.0 and existing Bevy0.19.1/wgpu29.0.4 executable reused. No tool installation or desktop input.
