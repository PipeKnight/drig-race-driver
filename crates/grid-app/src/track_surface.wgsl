// Independently expressed source layer arithmetic; Bevy provides lighting.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    mesh_view_bindings::view,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::{prepass_io::{VertexOutput, FragmentOutput}, pbr_deferred_functions::deferred_output}
#else
#import bevy_pbr::{forward_io::{VertexOutput, FragmentOutput}, pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}}
#import bevy_pbr::{pbr_types::PbrInput, mesh_view_bindings::lights, mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT, mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT, shadows::fetch_directional_shadow}
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
@group(#{MATERIAL_BIND_GROUP}) @binding(115) var<uniform> phong: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(116) var<uniform> phong_light: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(117) var<uniform> phong_direction: vec4<f32>;

// Independently authored dry response confined to the audited ground band.
// Source RGB/alpha, road/line texels and the existing layer blend are preserved.
fn ground_weight(code: f32, uv: vec2<f32>) -> f32 {
    if code > 1.5 {
        return 1.0 - smoothstep(0.60, 0.70, fract(uv.x));
    }
    return code;
}

#ifndef PREPASS_PIPELINE
// track_surface_simple.cg.fp (and the infield programs): the direct specular
// added to the lit albedo. I runs from the eye; `alpha` is the mixed diffuse
// alpha, `ao` the macro AO. The source's screen shadow mask becomes the sun's
// shadow map: the brightest directional light is the sun.
fn source_specular(pbr: PbrInput, alpha: f32, ao: f32) -> vec3<f32> {
    if phong.z <= 0.0 {
        return vec3(0.0);
    }
    let I = normalize(pbr.world_position.xyz - view.world_position);
    let N = pbr.N;
    let i_n = dot(I, N);
    let fresnel = mix(phong.y, phong.z, saturate(pow(max(1.0 + i_n, 0.0), phong.w)));
    let R = I - 2.0 * i_n * N;
    let lobe = pow(saturate(dot(R, phong_direction.xyz)), phong.x);
    var sun = 0u;
    var brightest = -1.0;
    for (var i = 0u; i < lights.n_directional_lights; i = i + 1u) {
        let c = lights.directional_lights[i].color.rgb;
        if max(c.r, max(c.g, c.b)) > brightest {
            brightest = max(c.r, max(c.g, c.b));
            sun = i;
        }
    }
    var shadow = 1.0;
    if brightest > 0.0 && (pbr.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
            && (lights.directional_lights[sun].flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
        let view_z = dot(vec4(
            view.view_from_world[0].z,
            view.view_from_world[1].z,
            view.view_from_world[2].z,
            view.view_from_world[3].z
        ), pbr.world_position);
        shadow = fetch_directional_shadow(sun, pbr.world_position, pbr.world_normal, view_z, pbr.frag_coord.xy);
    }
    return lobe * phong_light.rgb * alpha * fresnel * ao * 4.0 * shadow;
}
#endif

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
    // Default-off authored view-dependent F0 adaptation, NOT independent physical F90.
    // Host admits only standalone Donington road with SourceBase. Keep source alpha
    // weighting and normal-view response; no distance/speed/mip dependency.
    if rgb_filter.y > 1.0 {
        let ndotv = clamp(dot(pbr.N, pbr.V), 0.0, 1.0);
        let grazing = 1.0 - smoothstep(0.10, 0.35, ndotv);
        pbr.material.reflectance *= sqrt(1.0 + (rgb_filter.y - 1.0) * grazing);
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
    // The source lobe replaces Bevy's: F0 = 0 also zeroes its F90 and the
    // ambient specular. track_surface_reflective has no lobe, only its
    // unported ReflectionMap: Bevy's response stays in for it.
    var specular = vec3(0.0);
    if phong_light.w > 0.0 && phong.x > 0.0 {
        pbr.material.reflectance = vec3(0.0);
        specular = source_specular(pbr, clamp(layer.a, 0.0, 1.0), ao) * view.exposure;
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr);
    out.color = vec4(out.color.rgb + specular, out.color.a);
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
#endif
}
