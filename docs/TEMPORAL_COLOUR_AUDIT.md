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
