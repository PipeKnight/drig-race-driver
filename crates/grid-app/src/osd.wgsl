// The original OSD shader families (Osd/dial2.pssg shader groups) as one
// independently authored shader; the effect files themselves were not
// decoded, so each mode is an interpretation (docs/RACE_HUD.md).
#import bevy_pbr::{mesh_functions, view_transformations::position_world_to_clip, mesh_view_bindings::view}

struct Params {
    // rgb diffuse colour, alpha scaler
    colour: vec4<f32>,
    // scrollX, scrollY, glowScaler, Ramp
    a: vec4<f32>,
    // mode, Feather, ReflectionScaler, LightScaler
    b: vec4<f32>,
    // ReflectionContrast, ReflectionBrightness, ReflectionDesaturate, blurScaler
    c: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var diffuse: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var diffuse_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var glow: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var glow_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var soft: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var soft_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var sharp: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var sharp_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var<uniform> p: Params;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world: vec3<f32>,
}

@vertex
fn vertex(v: Vertex) -> Out {
    let model = mesh_functions::get_world_from_local(v.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(model, vec4(v.position, 1.0));
    let normal = mesh_functions::mesh_normal_local_to_world(v.normal, v.instance_index);
    return Out(position_world_to_clip(world.xyz), v.uv, normal, world.xyz);
}

fn linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3(0.0)), vec3(2.2));
}

fn adjust(e: vec3<f32>) -> vec3<f32> {
    let grey = dot(e, vec3(0.299, 0.587, 0.114));
    let d = mix(e, vec3(grey), p.c.z);
    return max((d - 0.5) * p.c.x + 0.5 + p.c.y, vec3(0.0));
}

@fragment
fn fragment(i: Out) -> @location(0) vec4<f32> {
    let mode = u32(p.b.x + 0.5);
    let colour = linear(p.colour.rgb);
    let alpha = p.colour.a;
    let n = normalize(i.normal);
    let eye = normalize(i.world - view.world_position);
    let r = reflect(eye, n);
    var rgb = colour;
    var a = alpha;
    switch mode {
        // osd3d_flat_image
        case 0u: {
            let t = textureSample(diffuse, diffuse_sampler, i.uv);
            rgb = t.rgb * colour;
            a = t.a * alpha;
        }
        // osd3d_flat_colour, osd3d_flat_colour_text: colour only
        case 1u: {}
        // osd3d_special_image: scrolled atlas plus its glow map
        case 2u: {
            let uv = i.uv + p.a.xy;
            let t = textureSample(diffuse, diffuse_sampler, uv);
            let g = textureSample(glow, glow_sampler, uv);
            rgb = t.rgb * colour + g.rgb * p.a.z;
            a = clamp(t.a * alpha + g.a * p.a.z, 0.0, 1.0);
        }
        // osd3d_maskramp: red is the shape, green the ramp (angle); texels
        // whose ramp is at or above Ramp show. The map is data, so its
        // sRGB-decoded values are brought back to the stored ones.
        case 3u: {
            let t = textureSample(diffuse, diffuse_sampler, i.uv);
            let ramp = pow(t.g, 1.0 / 2.2);
            let shown = clamp((ramp - p.a.w) / max(p.b.y, 0.001) + 0.5, 0.0, 1.0);
            a = alpha * shown * pow(t.r, 1.0 / 2.2);
        }
        // osd3d_needleblur
        case 4u: {
            let t = textureSample(diffuse, diffuse_sampler, i.uv);
            rgb = t.rgb * colour;
            a = t.a * alpha * p.c.w;
        }
        // osd3d_reflect_colour: colour plus the sharp environment
        case 5u: {
            rgb = colour + adjust(textureSample(sharp, sharp_sampler, r).rgb) * p.b.z * p.b.w;
        }
        // osd3d_reflect_colour_text(_flat): colour lit by both environments
        case 6u: {
            let s = textureSample(soft, soft_sampler, r).rgb;
            let h = textureSample(sharp, sharp_sampler, r).rgb;
            rgb = colour * (s * p.b.z + h * max(p.b.w - 1.0, 0.0));
        }
        // osd3d_reflect_image
        case 7u: {
            let t = textureSample(diffuse, diffuse_sampler, i.uv);
            rgb = t.rgb * colour + adjust(textureSample(sharp, sharp_sampler, r).rgb) * p.b.z * p.b.w;
            a = t.a * alpha;
        }
        // osd2d_font: glyph atlas
        default: {
            let t = textureSample(diffuse, diffuse_sampler, i.uv);
            rgb = t.rgb * colour;
            a = t.a * alpha;
        }
    }
    return vec4(rgb, clamp(a, 0.0, 1.0));
}
