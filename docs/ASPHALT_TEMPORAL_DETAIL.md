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


## Controlled view mip-bias experiment (20 October2026, G199)

Pinned dlss_wgpu4.0.0 recommends log2(renderWidth/outputWidth)-1; actual prepared native1920x1080 DLAA log confirms **-1**. Added default-off --surface-mip-bias off|view: all custom first/second/macro/skid samples opt in to the same camera bias, with unchanged surface RGB/alpha/blends/material response. Bevy stock PBR already uses that view value. No defaults, accepted launchers, physics or original assets changed. This is a diagnostic for future investigation/upscaling integration, not a promoted visual fix.

Four1800tick virtual30 captures compare DLAA and SMAA, each with bias off/view. All four normalized physical exit records agree exactly; only wall_seconds is excluded as elapsed real time. Actual images are1920x1080,30moving frames each; not measured monitor30FPS, not300FPS/4K/phone acceptance. Both SMAA final images are **byte-identical**, and new off image equals the old accepted9abb921 SMAA full image at this pose. DLAA changes texture sampling slightly but retains the conspicuously sharper band next to the car. Full frames/native crops/selected sequence frames inspected. Therefore missing surface bias is **not sufficient to explain or fix this defect**; default stays off.

Descriptive grayscale high-pass RMS (1pixel Gaussian residual, byte units) in fixed50x50 road ROIs: smooth area0.344off ->0.351view versus car-adjacent4.239 ->4.200. SMAA gives5.865/7.116 in the same regions, identical between modes. Different positions have different underlying surface patterns, so these are descriptive paired checks, not a calibrated disocclusion-quality score or proof of a specific internal DLSS algorithm.

Release5m43s/oneCargo job, Rust1.99.0/Bevy0.19.1/wgpu29.0.4; immutable private executable SHA28c68376aed41cfd5ba9b01effdc4138ace8cb5ccb01f84010ae216d047fbff0. All781resource tests/12harnesses, feature+default workspaceClippy, canonicalF455 identity and quick24/24 pass. Canonical/soak reuse archived bcb2e2c tools with current core byte-unchanged proof. First compiler attempt used wrong MipBias namespace, repaired before build. No new tools, dependency update or upstream merge/reseed.

Harness failures retained: first script cleared the original GRID_AUTOPILOT environment and produced standing-car controls, rejected and its uncited raw captures removed. Moving retry explicitly enables autopilot. Initial physical comparator included wall_seconds and falsely failed; removing only that timing field confirms identical physical records without repeating successful GPU runs. Native GPU output/return-code/readback guards passed before this comparator. Not renderer failures or approval rejections.

Private local/graphics/surface-bias01 contains source patch/hash, artifact01.json, checks01.json, moving-{dlaa,smaa}-{off,view}/commands/logs/captures, validation01.json, old-off-compat01.json and native comparisons. Existing raw temporal capture has motion/depth only, **no pre-DLAA colour**, so it cannot yet prove where the grain is lost. Next collect bounded pre-DLAA colour alongside reconstructed output at the same displayed frame, or use a strictly diagnostic history-reset control; separate sample-input loss from history behaviour before any compensating blur/sharpening. Antenna trails remain a separate open issue. Do not present this negative experiment as a new owner build.


Planned bounded colour audit boundary (not implemented/verified yet): installed Bevy0.19.1 Core3d schedule publicly chains MainPass -> EarlyPostProcess -> PostProcess; DLSS dispatch is in EarlyPostProcess. Read-only compute sampling of ViewTarget before that set and after it/before PostProcess could capture the same main-view HDR texture before/after reconstruction without calling post_process_write or swapping render targets. Require paired frame/tick/dimensions/origin headers, exact ROI and finite raw floats; inspect pinned scheduling for other active effects before calling either texture the exact DLSS boundary. Preserve the existing motion-depth binary protocol independently. This is a design note, not evidence that raw colour was captured. The simpler reset counterfactual currently under test cannot isolate each internal DLSS buffer or auto-exposure state.


## History-reset counterfactual (G200, 20 October2026)

New --dlaa-history normal|reset-every-frame is diagnostic only: forced reset requires enhanced native DLAA, scripted --drive-ticks and --render. Ordinary/unbounded/non-DLAA uses reject; four executable CLI guard cases pass. Normal remains default. PostUpdate asserts reset for every frame in forced mode; the render-world prepared check also verifies that the reset flag survives extraction. Pinned Bevy extraction copies Dlss to the render world then clears the main-world reset flag, which is why a one-time assignment cannot provide this control. No textures/materials/physics/DLL or owner launcher changed.

Two successful1800tick native1920x1080 virtual30 cases retain four identical physical exits. Their saved audit headers and raw motion/depth buffers are **byte-identical**, maximum float difference0. Full scenes, native road crops and frames0/14/28 inspected: the distinct sharper car-adjacent band disappears with continuous reset, while the road recovers much more uniform fine grain. Descriptive smooth/car-adjacent highpass RMS0.345/4.237 becomes5.458/6.120. This strongly supports a history/state-dependent reconstruction effect. It does not identify the individual internal DLSS mechanism, prove pre-DLAA colour equivalence, or establish a playable fix. Continuous reset forfeits temporal accumulation and may introduce aliasing/shimmer; it is not promoted.

New normal control differs slightly from the earlier G199 normal image: mean absolute channel difference0.01247byte, p991/max16 across69683pixels. Its raw motion/depth/header matches the earlier run exactly. Preserve this variation rather than claiming pixel-perfect normal parity or inventing a cause. Existing source default behaviour is unchanged; GPU output is not universally byte deterministic in these observations.

Private dlaa-history01 retains source.patch/artifact01.json, CLI guards, both exact commands/logs,30frames each, motion/depth evidence, validation01.json, normal-prior-variation01.json and native comparisons. Immutable executable SHAdd8166d51e80d8905a5e919f79b004ce00c3176707b796217b5c8a71c32e8ff4; release6m55s/onejob, same Rust1.99.0/Bevy0.19.1/wgpu29.0.4/SDK310.5.3. No new tools/dependencies/upstream merge or original-install writes. All781resource tests/12harnesses, feature+default workspaceClippy, canonicalF455 and quick24/24 pass. Four CLI guards pass; source patch matches the frozen executable.

Next: bounded same-frame pre/post-DLAA colour evidence, then determine a targeted history/reactivity integration rather than globally disabling reconstruction. Local pinned dlss_wgpu exposes an optional per-pixel bias texture described as making DLSS more reactive. Bevy0.19.1's current dispatch passes bias=None; its public Dlss camera component exposes quality mode/reset only. Presence of the lower-level field is not a tested mask implementation or proof it cures this issue; no SDK/cache source patched and no dependency fork introduced. Preserve the separate antenna-ghost and future smoke acceptance scenarios.


## Same-frame raw HDR isolation (G201, 20 October2026)

Implemented and GPU-validated the previously proposed bounded colour audit; see [protocol, controls and limitations](TEMPORAL_COLOUR_AUDIT.md). Actual frame793/tick1652 has byte-identical entire pre-stage HDR packets between normal/reset modes, and exact motion/depth+metadata equality. Post-stage colour differs. Three road-only30x30 ROIs retain22-27% of input linear high-frequency contrast under normal history, versus88% under reset. They exclude the freshest disocclusion strip: these are established-road measurements, not a mask or quality score. Grain exists in the input; loss is across early reconstruction and history-dependent in these controls. Exact internal cause/exposure contribution unresolved, continuous reset not promoted. Native1920x1080/virtual30; no4K/300FPS acceptance claim.781tests, both workspaceClippy, canonicalF455/quick24/24, fourCLI guards, parser malformed controls and640x480 GPU smoke pass. Private dlaa-colour01, no owner build/default changes. Next targeted reactivity/input-state integration rather than compensating asphalt blur or sharpen.


G202 follow-up supersedes the mask-first proposal: installed NVIDIA guide discourages BiasCurrentColor for modern models. Stationary history control retains78-80% highpass contrast; moving virtual300 still24-26%. Correct jitter sign is locally supported by adjacent raw-HDR reprojection, not a complete integration proof. See [input contract, exact scope and next supported preset experiment](DLAA_INPUT_CONTRACT.md). No owner build change.


## Uniform-softness pilot (G207, 21 October2026; not yet validated)

The owner rejected the optional L road shimmer and prefers a uniformly soft surface. The new default-off `--asphalt-soften 0..4` experiment applies a positive RGB mip bias only to the four audited standalone Donington track textures, with smooth strength from18to108km/h. It changes neither alpha/F0 nor macro colour, skid maps, mixed edge/paint/kerb textures or vehicle materials. Physical speed is read after each fixed simulation step, so the law does not depend on rendering FPS. This is authored source-detail filtering before DLAA, not original motion-blur reconstruction and not a claim to repair the underlying temporal mechanism. Runtime/default-parity, motion30/300and4K checks remain pending. Private evidence: local/graphics/asphalt-soft01; integrated unchanged controls: local/graphics/integrated-main01.


## Native4K follow-up and optional comparison (G208)

Native3840x2160 controls0/2/3 and a1080p full-road1800tick virtual300fps control2 all exit0 and preserve four physical exit records. Native crops and selected moving frames reviewed:2 is a moderate1080p candidate;3 better reduces the residual4K grain patch, while prior4 was overly flat. This is bounded camera/sequence evidence, not measured monitor FPS, a full-lap ghosting guarantee or owner acceptance. A repeat0 image differs from the first default by meanRGB[0.009899,0.012064,0.010279] byteunits, similar to the explicit0 difference; retain this temporal variation rather than claiming exact PNG parity.

`Play GRID Graphics Asphalt DLAA.cmd` is a separate comparison launcher in the graphics worktree. It uses a frozen ignored `local/graphics/asphalt-soft01/grid-app.exe`,1080p/strength2 by default, strength3 when resolution argument2 is3840x2160. Argument1 is the game folder; argument3 may select SMAA/TAA. Existing launchers are untouched. Frozen main baseline is F538, so subsequent F544/F546 updates are not in that executable. Hashes and limitations: private delivery.json. Current source is separately integrating newer main before publication.


## Owner rejection supersedes pilot assessment (G210, 22 October2026)

The owner tested the Asphalt DLAA comparison and rejected the excessive blur/loss of texture. Strength2/3 is not accepted and must not be promoted. The target is retained road detail without sharp side/rear trails or sudden history/detail transitions near the moving car. Return to temporal-boundary reprojection/depth-history diagnosis; input low-pass was compensation, not a demonstrated root fix. Earlier static-frame preference is superseded by this live feedback. Base motion/jitter conventions have bounded source/capture support (DLAA_INPUT_CONTRACT.md); correctness across all silhouette, thin-feature and disocclusion pixels remains unverified. Newly revealed road/history differences are a working explanation to test, not a confirmed internal DLSS cause.
