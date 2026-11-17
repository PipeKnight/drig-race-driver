// Shared source diffuse/normal atlas sampling and Bevy PBR lighting adapter.
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::alpha_discard
#import bevy_pbr::mesh_view_bindings::view
// Forward Blend only; no deferred/prepass/meshlet entry point is supplied.
#import bevy_pbr::forward_io::{Vertex, VertexOutput, FragmentOutput}
#import bevy_pbr::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}
#ifdef OIT_ENABLED
#import bevy_core_pipeline::oit::oit_draw
#endif
#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
// Closed skid IO: source macro basis is optional on lower-limit devices.
struct SkidOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv_b: vec2<f32>,
    @location(5) color: vec4<f32>,
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    @location(6) @interpolate(flat) instance_index: u32,
#endif
#ifdef VISIBILITY_RANGE_DITHER
    @location(7) @interpolate(flat) visibility_range_dither: i32,
#endif
    @location(8) @interpolate(flat) bank: u32,
#ifdef GRID_SKID_NORMAL_ATLAS
    @location(9) binormal: vec3<f32>,
    @location(10) tangent: vec3<f32>,
#endif
}
@vertex
fn vertex(
    in: Vertex, @location(8) bank: u32,
#ifdef GRID_SKID_NORMAL_ATLAS
    @location(9) binormal: vec3<f32>, @location(10) tangent: vec3<f32>,
#endif
) -> SkidOutput {
    var out:SkidOutput;
    let world_from_local=mesh_functions::get_world_from_local(in.instance_index);
    out.world_position=mesh_functions::mesh_position_local_to_world(world_from_local,vec4(in.position,1.0));
    out.position=position_world_to_clip(out.world_position.xyz);
    out.world_normal=mesh_functions::mesh_normal_local_to_world(in.normal,in.instance_index);
    out.uv=in.uv;out.uv_b=in.uv_b;out.color=in.color;out.bank=bank;
#ifdef GRID_SKID_NORMAL_ATLAS
    out.binormal = (world_from_local * vec4<f32>(binormal, 0.0)).xyz;
    out.tangent = (world_from_local * vec4<f32>(tangent, 0.0)).xyz;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index=in.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither=mesh_functions::get_visibility_range_dither_level(in.instance_index,world_from_local[3]);
#endif
    return out;
}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var atlas: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var atlas_sampler: sampler;
#ifdef GRID_SKID_NORMAL_ATLAS
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var normal_atlas: texture_2d_array<f32>;
#endif

@fragment
fn fragment(
    in: VertexOutput, @builtin(front_facing) front: bool,
    @location(8) @interpolate(flat) skid_bank: u32,
#ifdef GRID_SKID_NORMAL_ATLAS
    @location(9) binormal: vec3<f32>, @location(10) tangent: vec3<f32>,
#endif
) -> FragmentOutput {
    // D3DCOLOR RGBA: normal, wet, source opacity times authored retirement, mix.
    // Sample both bands unconditionally so derivatives are not branch-dependent.
    let previous_sample = textureSampleBias(atlas, atlas_sampler, in.uv, i32(skid_bank), view.mip_bias);
    let current_sample = textureSampleBias(atlas, atlas_sampler, in.uv_b, i32(skid_bank), view.mip_bias);
    let blend = clamp(in.color.a, 0.0, 1.0);
    var neutral = in;
    neutral.color = vec4<f32>(1.0, 1.0, 1.0, in.color.b);
    var pbr = pbr_input_from_standard_material(neutral, front);
    // PBR already contains vertex alpha; apply the atlas alpha exactly once.
    var sampled = mix(previous_sample, current_sample, blend);
    // Authoritative skid PS uses DP2ADD on UV difference, then CMP. It is
    // an identical-UV bypass, not a screen-space derivative condition.
    let delta = in.uv - in.uv_b;
    let same_uv = dot(delta, delta) == 0.0;
    if same_uv { sampled = previous_sample; }
#ifdef GRID_SKID_NORMAL_ATLAS
    // The original TNormalMap is linear BC3 with A/G/B as X/Y/Z.
    let old_map = textureSampleBias(normal_atlas, atlas_sampler, in.uv, i32(skid_bank), view.mip_bias).agb * 2.0 - 1.0;
    let new_map = textureSampleBias(normal_atlas, atlas_sampler, in.uv_b, i32(skid_bank), view.mip_bias).agb * 2.0 - 1.0;
    let tangent_normal = select(mix(old_map, new_map, blend), old_map, same_uv);
    let geometric = in.world_normal;
    let mapped = tangent * tangent_normal.x + binormal * tangent_normal.y + geometric * tangent_normal.z;
    let mixed = mix(geometric, mapped, in.color.r);
    // Degenerate strip slots retain the PBR input normal; valid source slots normalize after the mix.
    if dot(mixed, mixed) > 1.0e-12 && dot(binormal, binormal) > 1.0e-12 {
        pbr.N = normalize(mixed);
    }
#endif
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
