# Paired HDR temporal-colour audit

G201, 20 October 2026. Independently authored, default absent diagnostic; no playable build promotion.

## Purpose and boundary

`--temporal-colour-audit TICK` records one bounded pair of main-view HDR regions around Bevy 0.19.1 `EarlyPostProcess`, after `MainPass` and before `PostProcess`. In the tested native-DLAA profile, DLSS runs in that set; TAA, SSR and DOF are disabled. These are pre-bloom/tonemapping values, not display screenshots or calibrated original GRID radiometry. Other effects in that scheduling set would widen the interpretation. Read-only `textureLoad` does not swap/write the render target or modify history/physics.

Requires feature `dlaa`, enhanced native DLAA, live scripted driving, a render destination, virtual render clock, and at least eight simulation ticks of headroom. Collision preview, replay and flashback are rejected. Private evidence must remain beneath this checkout's ignored `local/`; existing evidence directories are rejected. No plugin/buffers are installed when the option is absent. Viewport-origin/subviewport and non-native-resolution use are not validated.

## Packet v1

Two files `before.bin` and `after.bin`, each 8,388,656 bytes: twelve little-endian u32 header words followed by capacity for 1024x512 RGBA float32 samples. Source is Rgba16Float: conversion to float32 preserves values, not additional original precision.

Header: version(1), extracted frame, actual simulation tick, stage(1/2), view width, view height, ROI origin x/y, ROI extent width/height, reserved zero/zero. Compact rows use the actual extent width; unused capacity must remain zero. Extent is clamped to the viewport, centered on the projected player body and clamped inside the view. The shader marks out-of-bounds reads nonfinite; CPU checks format, frame/ROI pairing, finite samples and nonblank colour. Independent motion/depth protocol is unchanged. Screenshot completion waits for both readbacks.

The actual capture can occur after the requested tick because extraction follows simulation steps. Compare actual header ticks, never assume the CLI tick equals the readback tick.

## Consumer

`tools/analyze_temporal_colour.py INPUT_DIRECTORY NEW_OUTPUT_DIRECTORY --roi name:x,y,width,height` validates packet length, roles, frame/tick pairing, dimensions, finiteness, padding and ROI bounds before writing results. Paths stay in ignored local/. ROI coordinates are full-view pixels; minimum 3x3. It writes source/tool SHA256, raw-linear colour/luminance statistics and a preview. The preview uses shared Reinhard plus sRGB for inspection, **not in-game AgX**. Measurements use linear luminance and residual from a 3x3 binomial kernel, not preview bytes. Contrast ratios are descriptive; jitter phase means they are not reconstruction error or an image-quality score.

## Verified controls

Frozen executable SHA256 `077bcbff19ea8eb6bc2e01a8ad583d1b817b3cd8b4693c7aa872df452817492a`, release 2m58s/one job. Rust 1.99.0, Bevy 0.19.1, wgpu 29.0.4, RTX4070 Vulkan; existing SDK310.5.3 and pinned NVIDIA DLL unchanged. Python3.12.14, NumPy2.3.5, Pillow12.3.0. No installs, dependency updates, main integration/reseed or original Steam edits.

Native 640x480/120tick smoke verified both stages at frame39/tick81 and the clamped extent/padding. Full controls use native1920x1080/1800ticks/virtual30, normal versus reset-every-frame, 30 motion frames each. Both paired readbacks have frame793/tick1652, origin442,415, extent1024x512. Full pre-stage packets are **byte-identical** (SHA256 `4cd8bafe46a0112a48fff70dbf740a5937ccf6961520a91f697d642492000cc5`); post-stage packets differ. Motion/depth and their metadata also match exactly. Four normalized physical exits agree between modes and with the previous G200 run without colour instrumentation; only wall_seconds is excluded. This is observed equality for these controls, not universal GPU determinism.

Geometrically inspected road-only ROIs at x500/550/600, y780, each30x30, exclude cars and major shadow edges. They do **not** measure the freshest disocclusion strip. After/before highpass RMS:

| Full-view ROI x,y,w,h | Normal history | Reset every frame |
| --- | ---: | ---: |
| 500,780,30,30 | 0.274 | 0.878 |
| 550,780,30,30 | 0.246 | 0.880 |
| 600,780,30,30 | 0.222 | 0.880 |

Input detail is present and equal; contrast loss occurs across the early reconstruction stage and depends strongly on history/reset state. This rules out absent source asphalt grain as a sufficient explanation in these captures. It does not identify the internal DLSS cause, exclude auto-exposure state, prove every road vector correct or justify disabling history in the owner's build. Native4K/300FPS and phone GPU acceptance remain untested for this diagnostic.

All781 resource tests/12harnesses pass,0failed/ignored; feature/default workspace releaseClippy pass. CanonicalF455 SHA `6d7184ac5d5e416d8849a2c382f1042ef3c17719cedbbe2324f666d3416f0063` and quick24/24 pass using archived tools with unchanged current core proof. Four CLI negatives reject zero/out-of-range tick, insufficient headroom and SMAA. Consumer synthetic valid case plus ten malformed controls pass. First integration script stopped on ambiguous App::new anchor; initial Clippy did not register new module, corrected and actual module compiled. CLI harness initially hit existing-render and data-dir guards before reaching the intended headroom guard; unused paths inside the configured private data directory corrected the harness. These were harness failures, no approval rejection or renderer failure.

Private `local/graphics/dlaa-colour01` retains commands, source snapshot/artifact, raw pairs, motion/depth, screenshots, road ROI overlay, public-consumer outputs, parser controls, GPU logs and checks01.json. No proprietary evidence enters Git.

## Next

Keep accepted Signals DLAA build and normal accumulation. Investigate targeted reactivity/bias-mask integration and exposure/history inputs, using this paired colour audit to reject cosmetic fixes that only hide the contrast gap. Lower-level optional bias texture exists but Bevy currently passes None; that is an investigation target, not a demonstrated solution. Antenna ghosting and smoke remain separate acceptance scenarios.


## Side/rear coverage and adjacent history (G211)

Prior512x384 motion/depth ROI around the projected body did not include rear-road candidates in the saved G201 pair. Optional `--temporal-centre X,Y` selects a bounded readback centre for both audits; their existing v1 sizes, format and default centre remain unchanged. Requires an explicit audit and a finite pixel inside the render resolution. This reads different pixels, not a render-history or motion-vector correction. New source is pending build/GPU validation.

`tools/analyze_temporal_audit.py CURRENT --previous PREVIOUS --map PRIVATE.png` validates actual adjacent frames and matching frozen camera/body transforms, then classifies four-tap previous-depth agreement with a conservative world/pixel-footprint tolerance. Maps: green stable, red previous-nearer, blue previous-farther, orange mixed, purple unavailable; yellow flags large static-motion residual inside a coarse ground volume. These volumes may contain other cars: the old pair's large residual region visibly includes the opponent. No object-ID or internal DLSS-history claim. Six synthetic reverse-Z controls live in tools/test_temporal_audit.py.


Completed rear controls (2026-10-22): feature Clippy passed57.70s; release build passed5m43s, six CLI negative guards passed, four GPU runs passed with exact normalized physical exits. All278 resource tests had already passed before the syntax-only lint fix. Raw frames792/1649 and793/1652 are adjacent; left/right rear candidates50472/33112. Evidence local/graphics/history-boundary01/result.json, capture-validation.json, left/right-history.json and left/right-colour-depth.png; both triptychs visually inspected. Static-road-consistent stable candidates have motion p99~0.16px; the large static residual regions all match the own bumper/exhaust rigid motion (body p99~0.021px). This does not prove every road vector correct in all cases. Eroded static-consistent depth classes show after/before spatial highpass ratios stable0.321/0.311 versus previous-nearer0.863/0.910 (left/right). Regions differ in brightness/material/shadow, so these descriptive ratios are not calibrated quality scores. They support history-dependent detail loss, not a demonstrated wrong-vector fix. Current left/right overlapping pre-HDR is byte-identical; post-HDR varies (max linearRGBA0.091796875, meanabs0.000022826), so do not assert exact post-stage determinism. Dedup6duplicates/0.28GiB/0errors. No quality/default/owner binary changes.

Next isolated diagnostic: pinned dlss_wgpu4.0.0 sets InFrameTimeDeltaInMsec to0. SDK310.5.3 forwards it; pinned guide calls it optional/potentially future-used. Test zero versus33.333333ms at virtual30; neither a proven integration defect nor an identified root cause. Preserve input parity and motion/depth controls, no asphalt input blur.


G215 reproducibility: add --colour CURRENT/capture-colour to the existing analyze_temporal_audit.py CURRENT/capture-temporal --previous PREVIOUS/capture-temporal command. It checks frame/view/coverage against both HDR stages, reports eroded static-motion-compatible class contrast and own-body-motion matches among static outliers. Left/right saved G211 results reproduce within1e-12; wrong-frame and insufficient-coverage controls reject. Class statistics are descriptive, not calibrated visual quality or internal DLSS history. Raw evidence remains private.

G214 exposure control completed: both explicit-unity and automatic paths request K, with identical inputHDR/motion/depth and physics, at native1080 virtual30. The road boundary remains in inspected shared HDR comparisons: stable-class highpass retention0.311585auto/0.312717unity versus previous-nearer0.910019/0.911084. Exposure changes output numerically but does not solve this bounded defect; no promotion. Active neural model is not verified by the hint roundtrip. Evidence stays private under local/graphics/exposure01.
