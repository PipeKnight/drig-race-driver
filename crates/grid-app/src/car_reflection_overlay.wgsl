// Car reflection overlay (docs/CAR_REFLECTIONS.md). Independently authored
// after the arithmetic of the original's car paint program: the lit colour
// moves toward the environment colour by the Fresnel weight, blended here as
// colour = env, alpha = weight over the already drawn part.
#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, lights},
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    pbr_types::pbr_input_new,
    pbr_functions::main_pass_post_lighting_processing,
    shadows::fetch_directional_shadow,
}

// EnvironmentColour (rgb), FresnelPower.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> environment: vec4<f32>;
// FresnelMin, FresnelMax, 1 when the part's own alpha scales the weight
// (glass), crack strength.
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> fresnel: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var cube: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var cube_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var base: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var base_sampler: sampler;
// The car's packed paint map (main_m1, linear); white when absent.
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var mask_sampler: sampler;
// The paint program's gates: 1 when they apply, OcclusionMapScale, and the
// SpecularValues z and w floors added to the occlusion and the shadow.
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var<uniform> paint: vec4<f32>;

// The sun's shadow at the fragment, 1 when lit or when no sun casts.
fn sun_shadow(in: VertexOutput, n: vec3<f32>) -> f32 {
    let view_z = dot(vec4(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z,
    ), in.world_position);
    for (var i = 0u; i < lights.n_directional_lights; i += 1u) {
        if (lights.directional_lights[i].flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            return fetch_directional_shadow(i, in.world_position, n, view_z, in.position.xy);
        }
    }
    return 1.0;
}

fn hash3(p: vec2<f32>) -> vec3<f32> {
    let q = vec3(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)), dot(p, vec2(419.2, 371.9)));
    return fract(sin(q) * 43758.5453);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(in.world_normal);
    if !front {
        n = -n;
    }
    let v = normalize(view.world_position.xyz - in.world_position.xyz);
    // The source program's Fresnel: t = 1 / (1 + P^(20 cos - 10)) weighs
    // FresnelMax at grazing angles against FresnelMin facing the eye.
    let c = saturate(dot(n, v));
    let t = 1.0 / (1.0 + pow(max(environment.w, 1e-4), 20.0 * c - 10.0));
    var weight = mix(fresnel.x, fresnel.y, t);
#ifdef VERTEX_UVS_A
    let texel = textureSample(base, base_sampler, in.uv);
#else
    let texel = vec4(1.0);
#endif
    weight *= mix(1.0, texel.a, fresnel.z);
    // A cracked pane: AUTHORED broken facets, each turned a little, and a
    // weaker reflection where the crack map is opaque.
    if fresnel.w > 0.0 {
#ifdef VERTEX_UVS_A
        let jitter = hash3(floor(in.uv * 48.0)) - 0.5;
#else
        let jitter = vec3(0.0);
#endif
        n = normalize(n + fresnel.w * 0.35 * jitter);
        weight *= 1.0 - 0.6 * fresnel.w * texel.a;
    }
    let r = reflect(-v, n);
    // Bevy's environment sampling flips Z (car_reflections::direction).
    var env = textureSampleLevel(cube, cube_sampler, vec3(r.xy, -r.z), 0.0).rgb * environment.rgb;
    // The paint program: the packed map's alpha gates the weight, its green
    // occludes the environment, and the sun's shadow darkens it.
    if paint.x > 0.0 {
#ifdef VERTEX_UVS_A
        let m1 = textureSample(mask, mask_sampler, in.uv);
#else
        let m1 = vec4(1.0);
#endif
        weight *= saturate(2.0 * m1.a);
        env *= saturate(1.0 + paint.y * (m1.g - 1.0) + paint.z);
        env *= saturate(sun_shadow(in, n) + paint.w);
    }
    var pbr = pbr_input_new();
    pbr.frag_coord = in.position;
    pbr.world_position = in.world_position;
    pbr.world_normal = n;
    pbr.N = n;
    pbr.V = v;
    let shaded = main_pass_post_lighting_processing(pbr, vec4(env, 1.0));
    return vec4(shaded.rgb, saturate(weight));
}
