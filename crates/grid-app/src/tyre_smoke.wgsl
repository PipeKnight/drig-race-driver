// Tyre smoke light after the original particle fragment programs
// (TYRE_SMOKE_RESEARCH.md §8). Lit smoke (`pfx_smoke` / `pfx_half_smoke`):
//   light = saturate((1 + N·sun) * 0.25 + 0.5) * pfxSunColour * shadow
//           + pfxPreMultipliedAmbientColour,
// N = the billboard's spherical corner normal, interpolated across the quad,
// shadow = the particle's birth light sample. Burnout rings
// (`pfx_unlit_half_depth` "fakelight"): light = pfxUnlitAmbientColour.
// The colours are the track's lighting.xml values; one scalar per pixel maps
// them to this renderer's radiometry so that a sprite facing the sun is as
// bright as the scene-lit material drew it (AUTHORED calibration).
// The material is unlit, so only this light, fog and tone mapping run per pixel.
// Drawn into the reduced smoke buffer (TYRE_SMOKE_REDUCED, smoke_reduced.rs),
// alpha also fades in front of the reduced scene depth as pfx_smoke's half
// fragment programs fade it: × saturate(2.5 × distance in front, metres).
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
    mesh_view_bindings as view_bindings,
    lighting::F_AB,
    view_transformations::depth_ndc_to_view_z,
}
#import bevy_render::maths::PI
#ifdef ATMOSPHERE
#import bevy_pbr::{
    atmosphere::functions::{calculate_visible_sun_ratio, clamp_to_surface},
    lighting::sample_transmittance_lut,
}
#endif

// xyz = pfxSunColour; w = 1 for lit smoke, 0 for the fakelight rings.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> pfx_sun: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> pfx_ambient: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> pfx_unlit: vec4<f32>;
#ifdef TYRE_SMOKE_REDUCED
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var reduced_depth: texture_depth_2d;
#endif

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

// Diffuse irradiance of the view's environment map towards `n`, as Bevy's
// `compute_radiances` samples it for the view cubemap.
fn view_irradiance(n: vec3<f32>) -> vec3<f32> {
#ifdef ENVIRONMENT_MAP
    let index = view_bindings::light_probes.view_cubemap_index;
    if index < 0 {
        return vec3(0.0);
    }
    let q = view_bindings::light_probes.view_rotation;
    let t = 2.0 * cross(q.xyz, n);
    var dir = n + q.w * t + cross(q.xyz, t);
    dir.z = -dir.z;
#ifdef MULTIPLE_LIGHT_PROBES_IN_ARRAY
    let irradiance = textureSampleLevel(view_bindings::diffuse_environment_maps[index],
        view_bindings::environment_map_sampler, dir, 0.0).rgb;
#else
    let irradiance = textureSampleLevel(view_bindings::diffuse_environment_map,
        view_bindings::environment_map_sampler, dir, 0.0).rgb;
#endif
    return irradiance * view_bindings::light_probes.intensity_for_view;
#else
    return vec3(0.0);
#endif
}

// The first directional light's radiance on a white Lambertian facing it.
fn sun_radiance(p: vec3<f32>) -> vec3<f32> {
    if view_bindings::lights.n_directional_lights == 0u {
        return vec3(0.0);
    }
    var colour = view_bindings::lights.directional_lights[0].color.rgb / PI;
#ifdef ATMOSPHERE
    let atmosphere = view_bindings::atmosphere;
    let l = view_bindings::lights.directional_lights[0].direction_to_light;
    let at = clamp_to_surface(atmosphere, (atmosphere.world_to_atmosphere * vec4(p, 1.0)).xyz);
    let r = length(at);
    let mu = dot(l, normalize(at));
    colour *= sample_transmittance_lut(r, mu) * calculate_visible_sun_ratio(atmosphere, r, mu,
        view_bindings::lights.directional_lights[0].sun_disk_angular_size);
#endif
    return colour;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // Unlit: base colour × vertex colour × texture.
    let pbr = pbr_input_from_standard_material(in, is_front);
    let albedo = pbr.material.base_color;
    let p = in.world_position.xyz;
    let v = normalize(view_bindings::view.world_position.xyz - p);
    // What the scene-lit material gave a white sprite facing the sun: diffuse
    // sun, ambient (EnvBRDFApprox at roughness 1, N·V 1) and environment map.
    let f_ab = F_AB(1.0, 1.0);
    let port = sun_radiance(p) + view_bindings::lights.ambient_color.rgb * (f_ab.x + f_ab.y)
        + view_irradiance(v);
    let sunlit = pfx_sun.rgb + pfx_ambient.rgb;
    var light = pfx_unlit.rgb;
    if pfx_sun.w > 0.5 {
        var wrap = 1.0;
        if view_bindings::lights.n_directional_lights > 0u {
            let l = view_bindings::lights.directional_lights[0].direction_to_light;
            wrap = saturate((1.0 + dot(in.world_normal, l)) * 0.25 + 0.5);
        }
        // The birth light sample is 1: the static shadow grid is not loaded.
        light = wrap * pfx_sun.rgb + pfx_ambient.rgb;
    }
    let scale = luminance(port) / max(luminance(sunlit), 1e-4);
    var alpha = albedo.a;
#ifdef TYRE_SMOKE_REDUCED
    let scene = textureLoad(reduced_depth, vec2<i32>(in.position.xy), 0);
    alpha *= saturate(2.5 * (depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(scene)));
#endif
    let colour = vec4(albedo.rgb * light * (scale * view_bindings::view.exposure), alpha);
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, colour);
    return out;
}
