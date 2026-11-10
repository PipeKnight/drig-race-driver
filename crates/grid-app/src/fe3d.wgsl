// The original's front-end materials fe3d_generic, fe3d_genericGD,
// fe3d_genericGPD, fe3d_genericGMPD and fe3d_genericI, ported from the feig3d_* (in-game) vertex and fragment
// programs of shaderpack.pssg (docs/RACE_HUD.md §6.7). The arithmetic runs
// on raw (gamma-space) texture values as the original's does; the result is
// converted for the sRGB target at the end.
#import bevy_pbr::{mesh_functions, view_transformations::position_world_to_clip, mesh_view_bindings::view}

struct Material {
    // DiffuseColour, Alpha
    colour: vec4<f32>,
    // GlowAmount, family (0 generic, 1 GD, 2 GPD, 3 I, 4 GMPD), Emmisive, SpecularLevel
    a: vec4<f32>,
    // FresnelMin, FresnelMax, FresnelPower, SpecularPow
    b: vec4<f32>,
    // DirtScale, DirtBlend, DirtGlowBlend, PatternScale
    c: vec4<f32>,
    // PatternBlend
    d: vec4<f32>,
}

struct Constants {
    ref_min_gamma: vec4<f32>,
    ref_max_scale: vec4<f32>,
    ambient: vec4<f32>,
    d1dirds: vec4<f32>,
    d1colss: vec4<f32>,
    d2dirds: vec4<f32>,
    d2colss: vec4<f32>,
    d3dir: vec4<f32>,
    d3col: vec4<f32>,
    glowdirtpattern: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var reflection: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var reflection_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var dirt: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var dirt_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var pattern: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var pattern_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> m: Material;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> k: Constants;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var diffuse: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var diffuse_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) bounds: vec3<f32>,
    @location(3) uv: vec2<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // The instance rows times the normal (not inverse-transposed).
    @location(1) normal: vec3<f32>,
    // The object-space normal: picks the triplanar plane.
    @location(2) object_normal: vec3<f32>,
    // The local position scaled by the instance rows' lengths.
    @location(3) scaled: vec3<f32>,
    // GD/GPD: the bounds-relative position; GMPD: TEXCOORD1.
    @location(4) bounds: vec3<f32>,
    @location(5) uv: vec2<f32>,
}

@vertex
fn vertex(v: Vertex) -> Out {
    let model = mesh_functions::get_world_from_local(v.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(model, vec4(v.position, 1.0));
    let linear = mat3x3<f32>(model[0].xyz, model[1].xyz, model[2].xyz);
    // Row i of the column-vector matrix is instance stream i.
    let rows = vec3(
        length(vec3(model[0].x, model[1].x, model[2].x)),
        length(vec3(model[0].y, model[1].y, model[2].y)),
        length(vec3(model[0].z, model[1].z, model[2].z)),
    );
    return Out(position_world_to_clip(world.xyz), world.xyz, linear * v.normal, v.normal, rows * v.position, v.bounds, v.uv);
}

fn spec(dir: vec3<f32>, r: vec3<f32>) -> f32 {
    return pow(saturate(dot(dir, -r)), m.b.w);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let c1 = saturate(c);
    return select(pow((c1 + 0.055) / 1.055, vec3(2.4)), c1 / 12.92, c1 <= vec3(0.04045));
}

@fragment
fn fragment(i: Out) -> @location(0) vec4<f32> {
    let family = u32(m.a.y + 0.5);
    let n = normalize(i.normal);
    let v = normalize(view.world_position - i.world);
    let nv = dot(v, n);
    let fresnel_term = saturate(pow(max(1.0 - nv, 0.0), m.b.z));
    // The original keeps the reflected view direction negated.
    let r = v - 2.0 * nv * n;
    let env = textureSample(reflection, reflection_sampler, vec3(r.x, -r.y, r.z)).rgb;
    let refl = pow(max(env, vec3(1e-6)), vec3(k.ref_min_gamma.w)) * k.ref_max_scale.w;
    let f = saturate(m.b.x + fresnel_term * (m.b.y - m.b.x));
    var specular = refl * f
        + spec(k.d1dirds.xyz, r) * k.d1colss.rgb * m.a.w * k.d1colss.w
        + spec(k.d2dirds.xyz, r) * k.d2colss.rgb * m.a.w * k.d2colss.w
        + spec(k.d3dir.xyz, r) * k.d3col.rgb * m.a.w * k.d1colss.w;
    // No screen-space shadow mask: its sample is 1.
    var lit = k.ambient.rgb
        + saturate(dot(k.d1dirds.xyz, n)) * k.d1colss.rgb * k.d1dirds.w
        + saturate(dot(k.d2dirds.xyz, n)) * k.d2colss.rgb * k.d2dirds.w
        + saturate(dot(k.d3dir.xyz, n)) * k.d3col.rgb * 6.0;
    lit = lit + m.a.z * (1.0 - lit);
    let colour = m.colour;
    if family == 0u {
        return vec4(srgb_to_linear(colour.rgb * lit + specular), saturate(colour.a));
    }
    if family == 3u {
        let t = textureSample(diffuse, diffuse_sampler, i.uv) * colour;
        return vec4(srgb_to_linear(t.rgb * lit + specular), saturate(t.a));
    }
    // Triplanar dirt (and pattern) coordinates from the dominant object
    // normal axis: x → (z, y), z → (y, x), else (x, z).
    let a = abs(i.object_normal);
    let s = sign(vec4(a.x - a.y, a.x - a.z, a.z - a.x, a.z - a.y));
    let pick = saturate(vec2(s.y + s.x, s.w + s.z));
    let c = i.scaled;
    var uv = mix(vec2(c.x, c.z), vec2(c.z, c.y), pick.x);
    uv = mix(uv, vec2(c.y, c.x), pick.y);
    let gdp = k.glowdirtpattern;
    let d = textureSample(dirt, dirt_sampler, uv / m.c.x);
    let dirt_amount = d.a * gdp.y;
    var pattern_spec = 1.0;
    var tint = vec3(1.0);
    if family == 2u || family == 4u {
        let p = textureSample(pattern, pattern_sampler, uv / m.c.w);
        pattern_spec = gdp.z * (p.a - 1.0) + 1.0;
        tint = mix(vec3(1.0), p.rgb, gdp.z * m.d.x);
    }
    specular = specular * pattern_spec * (1.0 - dirt_amount * m.c.y);
    let base = mix(tint * colour.rgb, d.rgb, dirt_amount * m.c.y);
    let result = base * lit + specular;
    // Glow: the colour falling off from the node's bounds centre (GMPD: along
    // the per-vertex TEXCOORD1 vector).
    let g = 1.0 - saturate(dot(i.bounds, i.bounds) * 0.33);
    let glow = g * colour.rgb * (1.0 - dirt_amount * m.c.z) * gdp.x + specular * 0.25;
    let rgb = saturate(mix(result, glow, m.a.x));
    return vec4(srgb_to_linear(rgb), saturate(colour.a));
}
