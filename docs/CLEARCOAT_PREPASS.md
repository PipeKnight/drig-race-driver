# Clearcoat normal-prepass investigation

16 October 2026 (Europe/Kiev), Bevy0.19.1 pinned. Low-angle views revealed washed-out paint with authored clearcoat, both with tread normal mapping off and on. Source-sky IBL reduces the effect compared with procedural IBL, but legacy no-clearcoat paint retains much more livery contrast. Private matched controls: `grazing-source-layered01`, `grazing-procedural-layered01`, `grazing-source-legacy01`.

## First art adjustment and its limits

Source4ee1b03c676a reduces paint coat1.0/roughness0.12 to0.2/0.18 and rim coat0.3 to0.1. `grazing-source-coat02` and `car-coat02` native4K inspected: low-angle washout is reduced but remains excessive, while quarter view is usable. This is not a completed correction. An unsupported `--camera=...` invocation failed before rendering, preserved as `grazing-source-coat02-failed-cli.log`; corrected separate-token arguments then captured successfully.

## Pinned shader source finding

Installed `bevy_pbr-0.19.1/src/render/mesh.rs` enables `LOAD_PREPASS_NORMALS` for normal-prepass, one-sample opaque pipelines. Our enhanced SSAO/TAA view has those conditions. In `pbr_fragment.wgsl`, the normal-prepass branch loads base `PbrInput.N` but does not initialize `clearcoat_N`; the assignment of the coat normal is inside the opposite branch. `pbr_types.wgsl` initializes that member to zero. Clearcoat lighting then clamps its N·V to0.0001, making the Fresnel layer much stronger than a valid front-facing normal would produce. A normal-based reflection direction also cannot be meaningful with that zero vector.

This static path explains the observations, but the repair requires a matched GPU comparison before claiming causality in our actual view. Reducing coat strength alone cannot fix an invalid normal.

## Narrow application repair

`clearcoat_prepass.rs` waits for the loaded `bevy_pbr::pbr_fragment` WGSL library, requires one exact pinned source signature, then adds the missing geometric-normal initialization to its prepass branch. Existing shader import/dependency/definition/validation metadata is retained. No installed crate files, global dependencies or other-agent code are modified. Geometric coat normal is appropriate for our current materials, which have no separate coat normal texture. Supporting a distinct coat normal map later requires additional work.

`--clearcoat-prepass fixed|stock` selects the repair/control; enhanced defaults fixed, baseline unaffected. The unit check validates the exact single change and rejects absent/ambiguous/already-repaired signatures. Initial missing mutable asset binding compilation error corrected. Unit and release all-target Clippy pass. Runtime log records application and normal-prepass view count; GPU counterfactual capture follows. Signature mismatch reports explicitly rather than applying a broad replacement to another renderer version.

## Matched GPU result

Source483941155483: `clearcoat-prepass-stock01` and `clearcoat-prepass-fixed01`, native3840×2160, identical low camera, source sky lighting and0.2coat/0.18coat roughness. Both inspected. Stock path washes black/red livery toward grey/pink; the repaired path retains black/red contrast and readable lettering. Runtime logs report the one targeted repair applied, one normal-prepass view, unchanged shader validation, and no shader errors. This counterfactual supports the missing coat normal as the cause in this pinned build; the earlier attribution to coat strength alone was incomplete.

`clearcoat-quarter-final01` inspected at native4K; `clearcoat-drive-final01`240ticks/throttle0.25 exits cleanly and retains the source control's exact final motion/handling. Source tread normals and sky-IBL dispatch readiness remain active. New exeSHA8b37b7b458a72a0adfab9128bd81cc163b450f13d11b197fca0d07bf56195445, separate graphics launcher/current-build manifest updated. No original-game execution, dependency installation/update, other-agent source or installed Bevy edits. Final coat strength stays0.2 pending broader art tuning; restoring a larger strength is a separate controlled decision, not part of proving the repair.
