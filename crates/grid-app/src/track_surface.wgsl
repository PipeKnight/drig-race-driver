// Independently expressed source layer arithmetic; Bevy provides lighting.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::{prepass_io::{VertexOutput, FragmentOutput}, pbr_deferred_functions::deferred_output}
#else
#import bevy_pbr::{forward_io::{VertexOutput, FragmentOutput}, pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}}
#endif
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var first: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var first_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var second: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var second_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var macro_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var macro_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var skids: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var skid_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var<uniform> scales: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var<uniform> controls: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var<uniform> skid_colour: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var<uniform> skid_colour2: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, is_front);
    let a = textureSample(first, first_sampler, in.uv);
    let b = textureSample(second, second_sampler, in.uv);
    let macro_sample = textureSample(macro_map, macro_sampler, in.uv_b);
    let layer = mix(a, b, in.color.r);
    // Green was limited at each vertex before interpolation, as in the source VS.
    var albedo = mix(layer.rgb, macro_sample.rgb, in.color.g);
    let skid_a_uv = vec2(in.uv_b.x * scales.z, in.uv_b.y * scales.x) + scales.w;
    let skid_b_uv = vec2(-in.uv_b.x * scales.z, in.uv_b.y * scales.y) + controls.x;
    let skid_a = textureSample(skids, skid_sampler, skid_a_uv).r * in.color.a * 2.0 * controls.y;
    let skid_b = textureSample(skids, skid_sampler, skid_b_uv).r * in.color.a * controls.y;
    albedo = mix(albedo, skid_colour.rgb, skid_a);
    albedo = mix(albedo, skid_colour2.rgb, skid_b);
    pbr.material.base_color = vec4(albedo, 1.0);
    // Source diffuse alpha weights its Phong lobe. This optional adaptation
    // weights Bevy's dielectric base F0 without changing roughness or opacity.
    // Square-root AFTER texture filtering/blending: F0 is quadratic in reflectance.
    // White control follows the same path and must reproduce feature-off output.
    if controls.w > 0.5 {
        let weight = select(clamp(layer.a, 0.0, 1.0), 1.0, controls.w > 1.5);
        pbr.material.reflectance *= sqrt(weight);
    }
    // Original AO-scale uniform is not recovered; scale=1 is an explicit choice.
    let ao = mix(1.0, macro_sample.a, controls.z);
    pbr.diffuse_occlusion *= ao;
    pbr.specular_occlusion *= ao;
#ifdef PREPASS_PIPELINE
    return deferred_output(in, pbr);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr);
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
#endif
}
