# Static object material coverage

Static objects retain a material-instance to source shader-family association when loaded from `objects.pssg`. This is a limited PBR adaptation; it does not execute the original shaders.

## Detroit grandstand panels

The owner reported fine dark/light perforation beneath the seats in the Detroit stadium. CPU rays from the saved back-grid camera identify the affected panel as material `gg_building_grandstand_01_j_04`, family `glass_simple.fx`. Its diffuse alpha contains a fine stippled pattern. The separate fence in front is `concrete_fence_white_end_01`, family `object_simple_mesh.fx`; the backing stand also uses that mesh family.

Previously the renderer assigned `Mask(0.5)` to every `objects/` material. That hard threshold turns the panel alpha into discrete holes. Selected source geometry rays put the panel substantially behind the fence and ahead of the backing stand; they do not support a coplanar-overlap diagnosis at these samples.

The saved ordinary `glass_simple` shader pass enables SRC_ALPHA / ONE_MINUS_SRC_ALPHA blending and disables fixed-function alpha testing. The correction selects **only the exact `#glass_simple.fx` family** as `AlphaMode::Blend`. Other object materials retain their existing cutout behavior. Classification uses the source material instance, not a name containing “glass”, a texture filename, or a whole-track override. Car windows and trees use their existing independent paths.

This is source-backed colour coverage, not full pass parity. The saved source pass also writes depth and uses separate-alpha ZERO/ZERO, whereas Bevy StandardMaterial Blend has its own depth/separate-alpha behavior. The exact saved `object_simple.cg.fp` shader was also disassembled: it samples TDiffuseAlphaMap and copies its alpha directly to the output; no texkill/discard or control flow appears in that program. This closes the sampled-alpha contract for this saved variant. Live original pixels, active runtime variant/overrides and a matched original camera have not been observed. The claim “the original shows this exact appearance” therefore requires additional reference evidence.

## Verification and remaining scope

The purchased-resource test identifies the actual Detroit panel and the adjacent fence, then verifies that only the selected source glass changes from Mask to Blend. Native matched-camera checks must verify that the stippled panel improves while the fence keeps its intended openings. Preserve raw controls and input/executable hashes under ignored `local/`.

Other source families have different alpha-test thresholds and can combine testing with blending. A blanket Opaque replacement or universal threshold change is not justified by this panel fix. In particular `object_simple_mesh` has an ordinary saved threshold of zero; mapping that to a Bevy mask requires handling the source GREATER comparison and preserving zero-alpha holes. That separate adaptation remains open.

Physics, object transforms, mesh topology, diffuse textures, ground shading and smoke are unchanged by this correction. Runtime camera/LOD tests and cross-track glass controls qualify general promotion separately from the single-view diagnosis.

## Owner-approved solid backing

The first glass-only native comparison still exposed noisy holes in the backing panel `objects/building_grandstand_01_j_01`. Its source `object_simple_mesh` ordinary pass uses ONE/ZERO replacement, GREATER-zero alpha test and alpha-to-coverage, not glass alpha blending. The owner explicitly permits a solid textured fallback instead of holes. Therefore **only this exact backing material** uses Opaque, retaining source RGB, UVs and geometry. All other mesh-family objects, including the fence, retain their prior behavior. This is an authored fallback, not original MSAA coverage parity. The front glass still uses the separately justified Blend correction.
