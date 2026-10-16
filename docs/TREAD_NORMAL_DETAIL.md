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
