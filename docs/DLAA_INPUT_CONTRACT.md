# DLAA input contract follow-up

G202, 20 October2026. Source inspection and controlled captures using G201 executable; no renderer/dependency/default edits.

## Important correction to the mask proposal

The locally installed NVIDIA DLSS Programming Guide (18Dec2025, section3.15, physical PDF pages40-41) explicitly discourages BiasCurrentColor for modern models and limits its described support. Therefore the earlier presence of `bias: Option<TextureView>` in dlss_wgpu is **not** enough reason to build a reactive-mask fix. Retain this as a historical proposal, superseded by this contract check. Guide SHA256 `6e7bc3d7d7f886ea97d11fe7fe72551c95be7c13ddff85e538d974ff3f80c565`; read with already installed pypdf6.10.0. Existing local SDK reused, no installation. [Official guide repository](https://github.com/NVIDIA/DLSS/tree/main/doc); remote raw PDF fetch returned unsupported content type, so the pinned local guide is the actual evidence, not a claim about all future releases.

## Installed integration

Source read from pinned Bevy0.19.1/dlss_wgpu4.0.0/NVIDIA SDK310.5.3:

| Input | Observed forwarding | Interpretation |
| --- | --- | --- |
| Colour | Main HDR source after MainPass | Raw detail verified in G201; reconstruction precedes bloom/tone |
| Depth | Prepass depth, InvertedDepth | Bevy reverse depth convention; no identified mismatch |
| Motion | Prepass current-minus-previous UV, multiplied by negative input dimensions | Converts to previous-minus-current pixels; agrees with earlier road reprojection control |
| Jitter | Negative TemporalJitter offset | Bevy perspective projection moves raster positions by negative offset too; no sign fix justified |
| Exposure | AutoExposure context flag, Automatic evaluation | Valid implemented path; quality contribution not yet isolated |
| Scale/pre-exposure | Wrapper passes zero, SDK helper normalizes zero to1 | **Not a zero-multiplier bug** |
| Bias mask | None | Do not add solely to compensate for blur; see warning above |
| Native jitter | Eight Halton samples at ratio1 | Detailed guide DLAA table agrees; no sixteen-phase requirement inferred from generic quickstart |

Bevy applies view exposure to PBR lighting before this HDR input. That by itself does not prove an exposure error: the active path uses DLSS auto exposure. Don't apply another exposure multiplier without a controlled test.

The DLL file reports310.5.3.0 and retains pinned SHA8707e53b...1507ec. **Active neural preset/weights have not been queried.** Lower-level sdk.rs requests feature updates asynchronously and its logging callback/level are disabled. A DLL hash alone must not be presented as complete model/OTA provenance. Public Bevy camera API exposes quality mode/reset, not preset/exposure/bias. Its SDK resource and preparation/node modules are private. No vendor cache patch, unsafe private-type access, dependency fork or tool injection performed.

## Stationary control

Explicit --drive-ticks1800, zero throttle/steering, handbrake fromtick0; autopilot environment absent and no opponent flag. Scripted controls bypass live keyboard/controller. This is intentionally stationary, unlike a rejected standing-car attempt at a moving experiment. First launch incorrectly supplied --opponents0, which CLI rejects; omission fixed it, logs preserved.

Native1920x1080/virtual30 normal/reset pairs finish successfully. Actual frame793/tick1652, origin447,424, extent1024x512. Complete input HDR and motion/depth+metadata packets match between modes. Four physical exits match. End speed approximately0.000017m/s after suspension settling, no lap progress/deformation. Visually inspected road-only regions560/600/640,y780,30x30 retain substantially more contrast with ordinary history than the previous moving control; ratios are0.799/0.776/0.794 normal versus0.852/0.843/0.839 reset. Different road position/material samples mean this is not a same-texel speed sweep or proof of one internal algorithm.

Private local/graphics/dlaa-contract01 holds guide excerpts, input-contract provenance, stationary commands/logs/raw pairs, analyses and annotated ROI image. Published source contains only independently authored notes. Existing playable Signals DLAA build remains unchanged.

## Next validation

Compare the same moving scripted drive at virtual300, retaining raw HDR, motion/depth and physics controls; do not label virtual time as measured GPU300FPS. Then choose a supported targeted integration experiment (preset/exposure inputs or verified vector coverage), not blind history reset, a globally applied mask or compensating sharpening. Antenna thin-feature coverage and smoke still need their own acceptance cases.

## Moving300 and adjacent input frames

Native1920x1080 virtual300 finishes successfully; four physical exits equal G201 virtual30. Actual audit frame7921/tick1650 (virtual30 was793/1652), same jitter phase but slightly different camera/pose, so not exact same HDR input. Three road-only500/550/600,y780 ROIs retain0.251/0.261/0.236 input highpass contrast. Inspected output retains the road-detail mismatch. Thus increasing virtual render cadence alone does not cure it. Input motion in a road region is smaller but still substantial (median approximately -20.6,+12.8pixels versus -57.1,+34.5 at virtual30). Fixed simulation and presentation stepping mean this is not a continuous 300Hz camera speed sweep or a hardware performance claim.

Additional capture obtains actual adjacent extracted frame792/tick1649 before G201 frame793/tick1652, not the guessed tick1650. Unchanged source/binary, four physical exits equal. Reproject previous raw HDR using current saved motion and raster jitter delta at road-only715,680,25x25: correct correction has luminance correlation0.695/RMS0.0143 versus omitted0.610/0.0152 and opposite0.404/0.0169. Sampling/interpolation, footprint/LOD and lighting differences prevent zero residual. This supports the sign convention locally, not every colour sample or internal DLSS behaviour. Initial higher ROI715,630 maps partly into the opponent shadow in the previous frame; rejected for static-road interpretation, raw analysis retained. Final ROI inspected in both frames. Custom surface shader detail uses mesh UVs, with no screen-coordinate/time-varying noise source found; this is source inspection, not a full temporal coherence proof.

All captures use the already fully tested G201 binary/source and committed consumer. No further Rust/dependency/default changes. Failed --opponents0 launch and initial schema-key assumption corrected; no approval rejection. All game-derived data stay private. Next supported preset K/L control should use an isolated copy of the wrapper with exact patch provenance, never edit the shared registry cache or silently retune owner builds. Actual preset/weights remain unknown until a stronger query; a requested hint must not be labelled verified active model selection.
