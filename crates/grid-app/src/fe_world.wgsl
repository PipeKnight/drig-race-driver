// The `fe3d_tracksurface` pixel program of a front-end world's surfaces
// (fe_world.rs, docs/FRONT_END_FLOW.md "The drawn meshes and their shader").
// [D, shaderpack disassembly]: colour = texture × instance colour × (ambient +
// Emmisive × (1 − ambient)) plus a box-projected cube reflection raised to
// r_gamma, × r_scale, × a Fresnel term between FresnelMin and FresnelMax, ×
// the texture's alpha; saturated; alpha = the instance colour's. The original
// works on the stored (sRGB) values, so the textures are re-encoded after the
// sampler's decode and the result is decoded for the linear target.
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

struct Surface {
    // xyz r_min, w r_gamma (`_refMinGamma`).
    min_gamma: vec4<f32>,
    // xyz r_max, w r_scale (`_refMaxScale`).
    max_scale: vec4<f32>,
    // rgb: instance colour × (ambient + Emmisive × (1 − ambient)); a: instance alpha.
    factor: vec4<f32>,
    // FresnelMin, FresnelMax, FresnelPower.
    fresnel: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var diffuse: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var diffuse_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var reflection: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var reflection_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> s: Surface;

fn encode(linear: vec3<f32>) -> vec3<f32> {
    let c = max(linear, vec3(0.0));
    return select(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3(0.0031308));
}

fn decode(stored: vec3<f32>) -> vec3<f32> {
    let c = max(stored, vec3(0.0));
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let pos = in.world_position.xyz;
    let v = normalize(view.world_position - pos);
    let n = normalize(in.world_normal);
    let nv = dot(v, n);
    let fresnel = clamp(pow(max(1.0 - nv, 0.0), s.fresnel.z), 0.0, 1.0);
    // The eye vector mirrored about the normal; its negation, in units of
    // the reflection box, is the ray.
    let r = n * (-2.0 * nv) + v;
    let inv = 1.0 / (s.max_scale.xyz - s.min_gamma.xyz);
    let ray = normalize(-r * inv);
    // Twice the unit ray; an axis at exactly zero gets 1e-5 so the divide
    // below stays finite.
    let d = 2.0 * ray + select(vec3(0.0), vec3(1e-5), ray == vec3(0.0));
    // The point in the box (clamped to it), and where the ray leaves it
    // (at most one ray length on).
    let p = clamp((pos - s.min_gamma.xyz) * inv, vec3(0.0), vec3(1.0));
    let exit = (clamp(p + d, vec3(0.0), vec3(1.0)) - p) / d;
    let q = d * min(min(exit.x, exit.y), exit.z) + p;
    let dir = (q * 2.0 - 1.0) * vec3(-1.0, 1.0, -1.0);
    let cube = encode(textureSample(reflection, reflection_sampler, dir).rgb);
    let texel = textureSample(diffuse, diffuse_sampler, in.uv);
    let refl = pow(cube, vec3(s.min_gamma.w)) * s.max_scale.w
        * clamp(mix(s.fresnel.x, s.fresnel.y, fresnel), 0.0, 1.0) * texel.a;
    let rgb = clamp(encode(texel.rgb) * s.factor.rgb + refl, vec3(0.0), vec3(1.0));
    return vec4(decode(rgb), clamp(s.factor.a, 0.0, 1.0));
}
