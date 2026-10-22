// Independently expressed source layer arithmetic; Bevy provides lighting.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    mesh_view_bindings::view,
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

@group(#{MATERIAL_BIND_GROUP}) @binding(112) var<uniform> dry_ground: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(113) var<uniform> sampling: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(114) var<uniform> rgb_filter: vec4<f32>;

// Independently authored dry response confined to the audited ground band.
// Source RGB/alpha, road/line texels and the existing layer blend are preserved.
fn ground_weight(code: f32, uv: vec2<f32>) -> f32 {
    if code > 1.5 {
        return 1.0 - smoothstep(0.60, 0.70, fract(uv.x));
    }
    return code;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, is_front);
    let mip_bias = select(0.0, view.mip_bias, sampling.x > 0.5);
    // track_infield_wsm.cg.fp: diffuse UVs are the world position / PScale,
    // projected along the dominant normal axis (Y wins ties, then X).
    var uv = in.uv;
    if controls.w > 0.0 {
        let n = abs(pbr.world_normal);
        let p = pbr.world_position.xyz * controls.w;
        let m = max(max(n.x, n.y), n.z) - 1.0e-4;
        if n.y >= m {
            uv = p.xz;
        } else if n.x >= m {
            uv = p.yz;
        } else {
            uv = p.xy;
        }
    }
    let a = textureSampleBias(first, first_sampler, uv, mip_bias);
    let b = textureSampleBias(second, second_sampler, uv, mip_bias);
    let macro_sample = textureSampleBias(macro_map, macro_sampler, in.uv_b, mip_bias);
    // RGB-only speed low-pass BEFORE reconstruction. Keep original filtered alpha
    // for source F0, and leave macro colour, marks and skid weights untouched.
    var first_rgb = a.rgb;
    var second_rgb = b.rgb;
    let road_bits = u32(sampling.w);
    if sampling.z > 0.0 && rgb_filter.x > 0.0 {
        if (road_bits & 1u) != 0u {
            let filtered = textureSampleBias(first, first_sampler, uv, mip_bias + sampling.z).rgb;
            first_rgb = filtered;
            if rgb_filter.x < 1.0 { first_rgb = mix(a.rgb, filtered, rgb_filter.x); }
        }
        if (road_bits & 2u) != 0u {
            let filtered = textureSampleBias(second, second_sampler, uv, mip_bias + sampling.z).rgb;
            second_rgb = filtered;
            if rgb_filter.x < 1.0 { second_rgb = mix(b.rgb, filtered, rgb_filter.x); }
        }
    }
    let layer = vec4(mix(first_rgb, second_rgb, in.color.r), mix(a.a, b.a, in.color.r));
    // Green was limited at each vertex before interpolation, as in the source VS.
    // controls.z < 0: the material saves no macro map (no blend, no AO).
    let macro_weight = select(in.color.g, 0.0, controls.z < 0.0);
    var albedo = mix(layer.rgb, macro_sample.rgb, macro_weight);
    let skid_a_uv = vec2(in.uv_b.x * scales.z, in.uv_b.y * scales.x) + scales.w;
    let skid_b_uv = vec2(-in.uv_b.x * scales.z, in.uv_b.y * scales.y) + controls.x;
    let skid_a = textureSampleBias(skids, skid_sampler, skid_a_uv, mip_bias).r * in.color.a * 2.0 * controls.y;
    let skid_b = textureSampleBias(skids, skid_sampler, skid_b_uv, mip_bias).r * in.color.a * controls.y;
    albedo = mix(albedo, skid_colour.rgb, skid_a);
    albedo = mix(albedo, skid_colour2.rgb, skid_b);
    pbr.material.base_color = vec4(albedo, 1.0);
    // Source diffuse alpha weights its Phong lobe. This optional adaptation
    // weights Bevy's dielectric base F0 without changing roughness or opacity.
    // Square-root AFTER texture filtering/blending: F0 is quadratic in reflectance.
    // White control follows the same path and must reproduce feature-off output.
    if sampling.y > 0.5 {
        let weight = select(clamp(layer.a, 0.0, 1.0), 1.0, sampling.y > 1.5);
        pbr.material.reflectance *= sqrt(weight);
    }
    // Follow the same authored vertex blend as the diffuse layers. This keeps
    // road/curb response unchanged while dry terrain loses the broad wet sheen.
    // Do not derive roughness from source specular alpha or albedo brightness.
    let ground = clamp(mix(ground_weight(dry_ground.x, in.uv), ground_weight(dry_ground.y, in.uv), in.color.r), 0.0, 1.0);
    pbr.material.perceptual_roughness = mix(pbr.material.perceptual_roughness, dry_ground.z, ground);
    // Preserve optional source-mask weighting, applied above.
    pbr.material.reflectance *= mix(1.0, dry_ground.w / 0.5, ground);
    // Original AO-scale uniform is not recovered; scale=1 is an explicit choice.
    let ao = mix(1.0, macro_sample.a, max(controls.z, 0.0));
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
