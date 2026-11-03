// Independently expressed object_light_shafts arithmetic (docs/LIGHT_SHAFTS.md).
// No vendor/game shader source is embedded.
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
    pbr_bindings::{base_color_texture, base_color_sampler},
    mesh_view_bindings as view_bindings,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> colour: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> controls: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> motion: vec4<f32>;

// The card texture is uploaded as sRGB; its green and blue are data.
fn raw(uv: vec2<f32>) -> vec4<f32> {
    let t = textureSample(base_color_texture, base_color_sampler, uv);
    let c = clamp(t.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let encoded = select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
    return vec4<f32>(encoded, t.a);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, front);
    let dust_uv = in.uv_b + view_bindings::globals.time * motion.xy;
    let dust = raw(dust_uv);
    let shape = raw(in.uv);
    let rgb = (dust.g * 0.3 + 1.0) * colour.rgb;
    let strength = controls.x;
    let mixed = mix(shape.a, dust.g, strength * shape.a * 0.5);
    let smoke = raw(controls.z * vec2<f32>(in.uv.x, dust_uv.x));
    let body = mix(mixed, smoke.b, strength * mixed);
    // Vertex program: fade from start to end distance (smoothstep), and by
    // how squarely the card faces the eye, times vertex alpha.
    let offset = in.world_position.xyz - view_bindings::view.world_position;
    let distance = length(offset);
    let s = clamp((distance - motion.z) / (motion.w - motion.z), 0.0, 1.0);
    let fade = 1.0 - s * s * (3.0 - 2.0 * s);
    let facing = abs(dot(normalize(in.world_normal), offset / distance));
    // Pixel program: half the D3D depth z/w (near plane 0.5 m, AUTHORED).
    let near = 0.5 * clamp(1.0 - 0.5 / distance, 0.0, 1.0);
    let alpha = clamp(body * controls.y * in.color.a * fade * facing * near, 0.0, 1.0);
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, vec4<f32>(rgb, alpha));
    return out;
}
