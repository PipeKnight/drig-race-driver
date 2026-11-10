// Car reflection overlay (docs/CAR_REFLECTIONS.md). Independently authored
// after the arithmetic of the original's car paint program: the lit colour
// moves toward the environment colour by the Fresnel weight, then (paint) toward
// the sun's metallic tint, and the sun's highlight is added. Blended
// premultiplied over the already drawn part: colour = what the part gains,
// alpha = the share of the part that is replaced.
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
// (glass), unused.
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
// A tuned material's sun terms: SpecularValues x (highlight power) and y
// (strength), MetallicSpecularValues x (tint power) and y (tint scale).
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var<uniform> sun_terms: vec4<f32>;
// MetallicSpecularValues z (falloff), the program (0 none, 1 paint, 2 window
// glass), unused.
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var<uniform> sun_program: vec4<f32>;

const PI: f32 = 3.141592653589793;

// The sun: the first directional light that casts shadows, else the first.
fn sun_index() -> u32 {
    for (var i = 0u; i < lights.n_directional_lights; i += 1u) {
        if (lights.directional_lights[i].flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            return i;
        }
    }
    return 0u;
}

// The sun's shadow at the fragment, 1 when lit or when no sun casts.
fn sun_shadow(in: VertexOutput, n: vec3<f32>) -> f32 {
    let view_z = dot(vec4(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z,
    ), in.world_position);
    let i = sun_index();
    if i < lights.n_directional_lights
        && (lights.directional_lights[i].flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
        return fetch_directional_shadow(i, in.world_position, n, view_z, in.position.xy);
    }
    return 1.0;
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
    // A cracked pane reflects as an intact one: the original's glass program
    // (car_basic fp_glass_windows) blends only the colour and alpha toward the
    // crack map and keeps the interpolated normal and the Fresnel weight.
    let r = reflect(-v, n);
    // Bevy's environment sampling flips Z (car_reflections::direction).
    var env = textureSampleLevel(cube, cube_sampler, vec3(r.xy, -r.z), 0.0).rgb * environment.rgb;
    let program = sun_program.y;
    var m1 = vec4(1.0);
    var shadow = 1.0;
    if paint.x > 0.0 || program > 0.0 {
#ifdef VERTEX_UVS_A
        if paint.x > 0.0 {
            m1 = textureSample(mask, mask_sampler, in.uv);
        }
#endif
        // The glass program lights a back face as unshadowed.
        if front || program < 1.5 {
            shadow = sun_shadow(in, n);
        }
    }
    // The paint program: the packed map's alpha gates the weight, its green
    // occludes the environment, and the sun's shadow darkens it.
    if paint.x > 0.0 {
        // paint.x: the share of the alpha gate applied (1 = the program's).
        weight *= mix(1.0, saturate(2.0 * m1.a), saturate(paint.x));
        env *= saturate(1.0 + paint.y * (m1.g - 1.0) + paint.z);
        env *= saturate(shadow + paint.w);
    } else if program > 1.5 {
        // The window glass program: the same shadow floor (its occlusion
        // term is 1 + SpecularValues.z, saturated: no occlusion map).
        env *= saturate(shadow + paint.w);
    }
    weight = saturate(weight);
    // Premultiplied: what the part gains, and the share of it that stays.
    var colour = env * weight;
    var keep = 1.0 - weight;
    if program > 0.0 && lights.n_directional_lights > 0u {
        let sun_light = lights.directional_lights[sun_index()];
        // The original's sun colour lights the diffuse colour directly;
        // Bevy's Lambert term divides by pi.
        let sun = sun_light.color.rgb * view.exposure / PI;
        let lr = max(dot(sun_light.direction_to_light, r), 1e-6);
        // AUTHORED: the original's 8-bit target clips the sum at 1; the
        // highlight's factor is clipped at 1 (a lit white) so strengths of
        // 39-197 (metal, wheel) do not reach the bloom as HDR energy.
        let highlight = sun * shadow * min(sun_terms.y * pow(saturate(lr), sun_terms.x), 1.0);
        if program < 1.5 {
            // The paint program: a metal mask from the packed map's alpha,
            // the diffuse colour darkened toward grazing angles, then mixed
            // toward the sun's metallic tint by its own power, then the
            // highlight; the alpha gates both.
            let metal = saturate((m1.a - 0.3) * 8.0);
            let darken = 1.0 + metal * (saturate(dot(n, v) + sun_program.x) - 1.0);
            let k = saturate(pow(saturate(lr), sun_terms.z) * shadow * m1.a);
            let tint = sun * (texel.rgb * sun_terms.w + 0.5) * metal;
            colour = colour * (1.0 - k) + tint * k + highlight * m1.a;
            keep *= darken * (1.0 - k);
        } else {
            // The window glass program: half the highlight. AUTHORED: scaled
            // by the pane's alpha like its reflection (GhostCarValues.z, which
            // sets how the highlight raises the pane's alpha, is not traced).
            colour += highlight * 0.5 * texel.a;
        }
    }
    let alpha = 1.0 - keep;
    var pbr = pbr_input_new();
    pbr.frag_coord = in.position;
    pbr.world_position = in.world_position;
    pbr.world_normal = n;
    pbr.N = n;
    pbr.V = v;
    // Fog is affine: fog(x) = (1 - f) x + f fog colour. The part below is
    // already fogged, so the gain is fogged as (1 - f) colour + f fog alpha.
    let fogged = main_pass_post_lighting_processing(pbr, vec4(colour, 1.0)).rgb;
    let fog_only = main_pass_post_lighting_processing(pbr, vec4(vec3(0.0), 1.0)).rgb;
    return vec4(fogged - fog_only * keep, alpha);
}
