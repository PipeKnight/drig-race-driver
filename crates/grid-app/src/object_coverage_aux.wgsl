// Source mesh-family auxiliary coverage, Bevy0.19.1 non-bindless domain.
// Source CSM pixel shader kills alpha<0.5 (equality survives); camera1x uses quarter.
// No temporal1x coverage, explicit4x sample_mask or deferred/meshlet support.
#import bevy_pbr::{
    prepass_io,
    pbr_bindings,
    pbr_types,
    pbr_functions::prepare_world_normal,
    pbr_prepass_functions,
    mesh_view_bindings::view,
}

fn raw_source_alpha(in: prepass_io::VertexOutput) -> f32 {
    var alpha = pbr_bindings::material.base_color.a;
#ifdef VERTEX_COLORS
    alpha *= in.color.a;
#endif
#ifdef VERTEX_UVS
#ifdef STANDARD_MATERIAL_BASE_COLOR_UV_B
    let source_uv = in.uv_b;
#else
    let source_uv = in.uv;
#endif
    let uv = (pbr_bindings::material.uv_transform * vec3<f32>(source_uv, 1.0)).xy;
    if (pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
        alpha *= textureSampleBias(pbr_bindings::base_color_texture,
            pbr_bindings::base_color_sampler, uv, view.mip_bias).a;
    }
#endif
    return alpha;
}

fn coverage_guard(in: prepass_io::VertexOutput) {
#ifdef OBJECT_AUX_CLIP_CSM_HALF
    if raw_source_alpha(in) < 0.5 { discard; }
#endif
#ifdef OBJECT_AUX_CLIP_QUARTER
    if raw_source_alpha(in) <= 0.25 { discard; }
#endif
    // OBJECT_AUX_NO_FIXED_CUTOFF is only a deliberate adaptation branch.
    // Source mask2 ALWAYS does not prove auxiliary shader-level no-discard.
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: prepass_io::VertexOutput, @builtin(front_facing) is_front: bool) -> prepass_io::FragmentOutput {
    coverage_guard(in);
    var out: prepass_io::FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
#ifdef NORMAL_PREPASS
    let double_sided = (pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
    let normal = prepare_world_normal(in.world_normal, double_sided, is_front);
    out.normal = vec4<f32>(normal * 0.5 + vec3<f32>(0.5), 1.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
#endif
    return out;
}
#else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
    coverage_guard(in);
}
#endif
