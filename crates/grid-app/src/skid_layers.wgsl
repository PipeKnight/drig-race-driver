// Shared diffuse-layer crossfade adaptation. Original CRT domain/error behavior, normal-map
// Fade and source precision remain open; no source shader parity claim.
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::alpha_discard
#import bevy_pbr::mesh_view_bindings::view
// Forward Blend only; no deferred/prepass/meshlet entry point is supplied.
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
#import bevy_pbr::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}
#ifdef OIT_ENABLED
#import bevy_core_pipeline::oit::oit_draw
#endif
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var atlas_sampler: sampler;
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    // COLOR.r carries longitudinal layer mix, never visible opacity or tint.
    // Sample both bands unconditionally so derivatives are not branch-dependent.
    let previous_sample = textureSampleBias(atlas, atlas_sampler, in.uv, view.mip_bias);
    let current_sample = textureSampleBias(atlas, atlas_sampler, in.uv_b, view.mip_bias);
    let blend = clamp(in.color.r, 0.0, 1.0);
    var neutral = in;
    neutral.color = vec4<f32>(1.0, 1.0, 1.0, in.color.a);
    var pbr = pbr_input_from_standard_material(neutral, front);
    // PBR already contains vertex alpha; apply the atlas alpha exactly once.
    var sampled = mix(previous_sample, current_sample, blend);
    // Authoritative skid PS uses DP2ADD on UV difference, then CMP. It is
    // an identical-UV bypass, not a screen-space derivative condition.
    let delta = in.uv - in.uv_b;
    if dot(delta, delta) == 0.0 { sampled = previous_sample; }
    pbr.material.base_color *= sampled;
    pbr.material.base_color = alpha_discard(pbr.material, pbr.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr);
    out.color = main_pass_post_lighting_processing(pbr, out.color);
#ifdef OIT_ENABLED
    // Same native OIT terminal as stock pbr.wgsl for this fixed Blend material.
    // Direct colour output would bypass resolve; route then discard instead.
    oit_draw(in.position, out.color);
    discard;
#endif
    return out;
}
