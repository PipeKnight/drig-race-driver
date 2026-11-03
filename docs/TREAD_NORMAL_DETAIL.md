# Selective source tread normal mapping

16 October 2026 (Europe/Kiev), graphics source5b50cafc18fa. `--normal-detail tread|off`, enabled for enhanced closest-LOD tyres only. Body damage/scratch normals remain unbound.

The source shader/mask audit establishes that LOD0 tread's720vertex occurrences enable normal detail. Saved normal/tangent/binormal and ARGB mask data are now available. The renderer only accepts the exact fully-enabled tread mask, finite vectors and sufficient frame alignment. It orthogonalizes the source tangent against the normal and derives handedness from the saved binormal. This is an authored approximation: original basis skew is not exactly representable by Bevy tangent+sign. Left/right mirrored signs are retained.

Tyre circular refinement now carries tangent data for every new vertex, re-orthogonalizes interpolated tangents against interpolated normals and refuses triangles crossing a tangent-sign seam. It preserves separate source UV seams. Synthetic/source geometry checks verify source vertices, dimensions, transforms, finite unit tangents, normal orthogonality and sign, including all16tyre draws in both LODs. A strengthened test initially assumed LOD1 supplied the same valid frames and failed. The implementation now explicitly limits normal-detail conversion to verifiedLOD0; LOD1 keeps a tested no-normal-map fallback, while retaining smooth silhouette geometry.

`lac_tread_tm_nm.tga` is loaded separately as linear BC1 (`Bc1RgbaUnorm`, `srgb=false`), not through its unused albedo-sRGB handle. Only `Tyre Tread` materials receive this normal texture, and only when all selected tread parts have verified frame data. No source color mask is attached to Bevy's albedo-modulating vertex-color attribute. No arbitrary filenames are interpreted as roughness or body damage.

## GPU and driving checks

Source5b50cafc18fa/exeSHA367a70b1b9e4cb8a986bd9eaa782769f0dcb82526f4f92e9a1448c9aabb4f59d. `tread-normal-off01`/`tread-normal-on01` matched native3840×2160, camera(2.0,0.32,2.0)/target(0.74,0.304,1.36). Both inspected; linear normal binding logged, eight tyre refinements/no skips/errors. Tread effect is subtle at this mostly sidewall view, so it is not a major visual-quality claim. Pixel difference has nonzero full-frame bounds/mean0.06467/max63; temporal image differences are not isolated, and this is not a quality score or proof that only tread pixels changed.

`tread-normal-drive01`: enhanced/source-sky IBL,240ticks/throttle0.25, normal-map and filtering readiness logged, normal exit; complete final motion and handling match the source-tyre control excluding wallclock. `tread-normal-lod1-fallback01`: normal exit, no active-normal binding, valid image. Geometry2tests and release all-target appClippy pass. Existing65format tests pass from G011; no physics code changed.

## Material issue found while inspecting

The new low close-up reveals excessive livery washout with authored layered materials, both with normal-detail off and on. Matched Donington source/procedural IBL and legacy-material controls show that source IBL reduces the washout but the initial lacquer response remains excessive; legacy paint preserves livery much better. This is an art/material limitation, not attributed to the normal texture. Private `grazing-source-layered01`, `grazing-procedural-layered01`, `grazing-source-legacy01` retain the controls. Clearcoat strength/roughness are being tuned before the next final playable manifest update.

Normal mip completeness, finer tyre texture resolution, original mask updates, source-binormal exact representation and body/detail materials remain open. Upscaling workflow/tool provenance: [TEXTURE_UPSCALING.md](TEXTURE_UPSCALING.md).

Follow-up: [clearcoat prepass repair](CLEARCOAT_PREPASS.md) resolves the low-angle washout with a matched stock/fixed GPU comparison. Latest source483941155483 retains tread normals and passes the same240tick motion/handling comparison. The problem was missing geometric coat-normal initialization in the active prepass path, so strength-only art tuning was insufficient. Private release manifest `validation-clearcoat01.json` records the final executable and images.


## General model binding and explicit basis limits (3 November 2026, G353)

Tread preparation now keeps an independent source key, admitted mesh-frame
readiness and optional linear normal image for each Field model. Vehicle slots
resolve through their existing model namespaces; no other model supplies a
missing image or frame. The shared path preserves the existing authored basis
admission, material response, sampler, shader and profile guards.

Real load/append_car CPU controls use three models and four slots, including
missing-image/frame/parts and invalid-model cases. Combined-feature Clippy and
the exact source control pass; the required grid-app gate passes636tests and
Clippy. This does not verify actual GPU binding, shaded response or lowLOD
readiness for every model.

| Selected LOD0 source | Tread vertex occurrences | Native N/T/B | Minimum admitted-basis alignment | Occurrences below.95 |
|---|---:|---|---:|---:|
| Lacetti |720|present, finite, nonzero|.9887001|0|
| BMW |720|present, finite, nonzero|.9723601|0|
| STK |540|present, finite, nonzero|.8221446|252|

All selected masks are ff00ffff. STK therefore lacks the prepared Bevy tangent
attribute because of substantial original binormal skew, not absent native
vectors or mask rejection. The all-or-none helper refuses each of its four
draws. Keep the.95authored adaptation limit until a reviewed common basis
representation is available; lowering it is not a general source-correct fix.
The numeric audit used decoded source floats and Pythonf64, corroborated by
the actual Rust prepared-frame results; it is not an image-error measurement.

A further shader-scope check finds the saved car.fx primary pixel-program ID
differs from the earlier selected paint pixel program. The shared vertex
program independently transforms T/B/N, but this alone does not establish the
exact active tread pixel decode/pass. Do not generalize paint-PS findings into
tread-PS parity. Full saved-basis, refinement, material-cache/prepass and actual
shader-selection compatibility remain open.

Private evidence: `local/graphics/tread-per-model01/`,
`local/handoff/drafts/tread-admission50/` and `tread-full-basis51/`.

### Actual CPU upload and material consumers

The same production upload and normal-image assignment now have small shared
helpers called by setup and the purchased regression. A fresh headless
App/Assets fixture inspects actual Image and StandardMaterial assets through
MeshMaterial3d handles. It verifies separate model handles, linear BC1 data,
unchanged mips, missing-map clearing and both disabled controls. A distinct
independently authored BMW DDS in memory catches cross-model payload leakage;
the purchased Lacetti/BMW base maps happen to be byte-identical, so equality
alone would hide that defect. Source files are never modified by this control.
STK's existing frame refusal and unrelated material assignment are preserved.

The exact consumer regression and combined-feature all-target Clippy pass
(local/graphics/tread-per-model01/check02): one test, no renderer/window/GPU.
This exercises actual CPU consumers, not the complete setup/spawn/render queues
or visible shader contribution. Representative rendered acceptance remains open.

### Saved basis and sampler qualifications

The exact saved non-paint car.fx pixel program has now been disassembled for the
selected Lacetti primary pass. Its tangent-space decode normalizes the decoded
sample, sums the independently transformed T/B/N vectors, then uses that result
in lighting without an intervening normalization. This is saved-pass evidence,
not a live pass observation or a BMW/STK shader association. Normalized Bevy PBR
input remains an explicit adaptation. See private source-contract52 in
local/handoff/drafts/tread-full-basis51/.

A bounded CPU comparison53 samples only36/4/27 distinct base-mip texels for
Lacetti/BMW/STK at source vertex UVs. BMW's720UV occurrences lie outside0..1;
the declared clamp sampler reaches only four BMW edge texels. Its small numeric
delta cannot establish general filter or visual accuracy. The source helper
still refuses all four STK draws; its orthonormal comparison is counterfactual.
Original GRID sampler addressing remains unproved. Current engine setup uses
ClampToEdge with linear filters; that configuration is not an original-state
capture. Raw results and prior incorrect UV/sign prose are preserved with
transparent corrections in the private comparison53/admission50 packets.
