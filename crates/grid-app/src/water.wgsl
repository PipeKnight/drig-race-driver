// Independently expressed water.fx arithmetic (docs/WATER.md). No vendor/game
// shader source is embedded.
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
    mesh_view_bindings as view_bindings,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var normal_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> deep: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> shallow: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> reflection: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> motion: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var<uniform> fresnel: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var environment: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var environment_sampler: sampler;

fn tap(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(normal_map, normal_sampler, uv).rgb * 2.0 - 1.0;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, front);
    // Vertex program: three scrolled taps of one map at 1x, 3x and 5x scale.
    let t = view_bindings::globals.time * 5.0;
    let uv = in.uv * motion.xy;
    let speed = motion.zw;
    let n = normalize(tap(uv + t * vec2<f32>(speed.x, -speed.y))
        + tap(uv * 3.0 + t * 6.0 * speed)
        + tap(uv * 5.0 + t * 2.5 * speed));
    // Basis: tangent scaled by bumpHeight * (1 - vertex green), its cross with
    // the normal, the normal; the sum is not renormalised.
    let normal = normalize(in.world_normal);
    let tangent = in.world_tangent.xyz * shallow.w * (1.0 - in.color.g);
    let bumped = tangent * n.x + cross(normal, tangent) * n.y + normal * n.z;
    let to_point = normalize(in.world_position.xyz - view_bindings::view.world_position);
    let edge = max(1.0 - max(dot(-to_point, bumped), 0.0), 0.0);
    // The cube at reflect(I, bumped), z negated (the original's lookup and
    // Bevy's cube convention agree); alpha 1, so no RGBM division.
    let r = reflect(to_point, bumped);
    let env = textureSample(environment, environment_sampler, vec3<f32>(r.x, r.y, -r.z)).rgb;
    let reflected = env * reflection.rgb * reflection.w
        * mix(fresnel.x, 1.0, pow(edge, fresnel.y));
    let colour = mix(deep.rgb, shallow.rgb, edge) * deep.w + reflected;
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, vec4<f32>(colour, in.color.a));
    return out;
}
