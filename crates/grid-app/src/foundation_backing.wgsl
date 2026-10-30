// Forward raw4x / strict-quarter1x coverage adaptation. PBR RGB follows pinned Bevy0.19.1.
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
    decal::clustered::apply_decals,
}
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // Specialize removes ONLY ALPHA_TO_COVERAGE remap define; hardware A2C stays on.
    var pbr = pbr_input_from_standard_material(in, is_front);
    let raw_alpha = pbr.material.base_color.a;
    // Explicit source-colour GREATER0; do not call Opaque/Mask alpha_discard.
#ifdef OBJECT_COVERAGE_QUARTER
    // Saved cheap mask16 strictGREATER.25 coverage; ordinary PBR RGB retained.
    if raw_alpha <= 0.25 { discard; }
#else
    if raw_alpha <= 0.0 { discard; }
#endif
    apply_decals(&pbr);
    var out: FragmentOutput;
    if (pbr.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr);
    } else {
        out.color = pbr.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    // Guard excludes PREMULTIPLY_ALPHA. Keep coverage independent of fog/post alpha.
    out.color.a = raw_alpha;
    return out;
}
