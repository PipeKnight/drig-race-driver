// tree_lowlod_shadowed.cg.fp: an impostor card lit from its baked normal
// atlas, the normal bent towards the sun by LightWrap and N.L lifted by
// InvFoliageDensity. Bevy lights the bent normal; the lift is added here.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::{view, lights},
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    shadows::fetch_directional_shadow,
}
#ifdef VISIBILITY_RANGE_DITHER
#import bevy_pbr::pbr_functions::visibility_range_dither
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var normals: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var normals_sampler: sampler;
// LightWrap, InvFoliageDensity, card frame sign (+1 or -1), unused.
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> lighting: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
#ifdef VISIBILITY_RANGE_DITHER
    visibility_range_dither(in.position, in.visibility_range_dither);
#endif
    var pbr = pbr_input_from_standard_material(in, is_front);
    pbr.material.base_color = alpha_discard(pbr.material, pbr.material.base_color);

    // The sun: the brightest directional light.
    var sun = 0u;
    var brightest = -1.0;
    for (var i = 0u; i < lights.n_directional_lights; i = i + 1u) {
        let c = lights.directional_lights[i].color.rgb;
        if max(c.r, max(c.g, c.b)) > brightest {
            brightest = max(c.r, max(c.g, c.b));
            sun = i;
        }
    }
    // The card frame: A level towards the eye, R = A x up, up.
    let toward = view.world_position - pbr.world_position.xyz;
    let A = normalize(vec3(toward.x, 0.0, toward.z) + vec3(0.0, 0.0, 1.0e-6));
    let R = cross(A, vec3(0.0, 1.0, 0.0)) * lighting.z;
    let t = textureSample(normals, normals_sampler, in.uv).xyz * 2.0 - 1.0;
    let n = normalize(R * t.x + vec3(0.0, 1.0, 0.0) * t.y + A * t.z);
    if brightest > 0.0 {
        let L = lights.directional_lights[sun].direction_to_light;
        let N = normalize(n + L * lighting.x);
        pbr.N = N;
        pbr.world_normal = N;
        pbr.material.reflectance = vec3(0.0);
        var out: FragmentOutput;
        out.color = apply_pbr_lighting(pbr);
        // sat(N.L + density) in place of Bevy's sat(N.L), for the sun.
        let ndotl = dot(N, L);
        let lift = saturate(ndotl + lighting.y) - saturate(ndotl);
        var shadow = 1.0;
        if (pbr.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
                && (lights.directional_lights[sun].flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            let view_z = dot(vec4(
                view.view_from_world[0].z,
                view.view_from_world[1].z,
                view.view_from_world[2].z,
                view.view_from_world[3].z
            ), pbr.world_position);
            shadow = fetch_directional_shadow(sun, pbr.world_position, pbr.world_normal, view_z, pbr.frag_coord.xy);
        }
        let added = pbr.material.base_color.rgb / 3.14159265 * lights.directional_lights[sun].color.rgb
            * lift * shadow * view.exposure;
        out.color = vec4(out.color.rgb + added, out.color.a);
        out.color = main_pass_post_lighting_processing(pbr, out.color);
        return out;
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr);
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
}
