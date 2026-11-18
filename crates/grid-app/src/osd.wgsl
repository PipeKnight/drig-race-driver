// The original OSD shader families (Osd/dial2.pssg shader groups) as one
// independently authored shader; the effect files themselves were not
// decoded, so each mode is an interpretation (docs/RACE_HUD.md).
//
// Drawn by the OSD overlay pass (osd_scene.rs `overlay`), not through Bevy's
// mesh pipeline. The vertex stage repeats Bevy 0.19's mesh transforms
// (mesh_functions.wgsl, and the inverse transpose of mesh_preprocess.wgsl)
// so the picture matches the former OSD camera's bit for bit.
#import bevy_render::maths

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

struct View {
    clip_from_world: mat4x4<f32>,
    world_position: vec3<f32>,
}

struct Draw {
    // world_from_local, transposed (Bevy's `Affine3::to_transpose`)
    world_from_local: mat3x4<f32>,
    p: Params,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> draw: Draw;
@group(1) @binding(0) var diffuse: texture_2d<f32>;
@group(1) @binding(1) var diffuse_sampler: sampler;
@group(1) @binding(2) var glow: texture_2d<f32>;
@group(1) @binding(3) var glow_sampler: sampler;
@group(1) @binding(4) var soft: texture_cube<f32>;
@group(1) @binding(5) var soft_sampler: sampler;
@group(1) @binding(6) var sharp: texture_cube<f32>;
@group(1) @binding(7) var sharp_sampler: sampler;

struct Vertex {
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
    let model = maths::affine3_to_square(draw.world_from_local);
    let world = model * vec4(v.position, 1.0);
    var normal = v.normal;
    if any(v.normal != vec3<f32>(0.0)) {
        let l = transpose(maths::inverse_affine3(transpose(draw.world_from_local)));
        normal = normalize(mat3x3<f32>(l[0].xyz, l[1].xyz, l[2].xyz) * v.normal);
    }
    return Out(view.clip_from_world * vec4(world.xyz, 1.0), v.uv, normal, world.xyz);
}

fn linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3(0.0)), vec3(2.2));
}

fn adjust(e: vec3<f32>) -> vec3<f32> {
    let grey = dot(e, vec3(0.299, 0.587, 0.114));
    let d = mix(e, vec3(grey), draw.p.c.z);
    return max((d - 0.5) * draw.p.c.x + 0.5 + draw.p.c.y, vec3(0.0));
}

@fragment
fn fragment(i: Out) -> @location(0) vec4<f32> {
    let mode = u32(draw.p.b.x + 0.5);
    let colour = linear(draw.p.colour.rgb);
    let alpha = draw.p.colour.a;
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
            let uv = i.uv + draw.p.a.xy;
            let t = textureSample(diffuse, diffuse_sampler, uv);
            let g = textureSample(glow, glow_sampler, uv);
            rgb = t.rgb * colour + g.rgb * draw.p.a.z;
            a = clamp(t.a * alpha + g.a * draw.p.a.z, 0.0, 1.0);
        }
        // osd3d_maskramp: red is the shape, green the ramp (angle); texels
        // whose ramp is at or above Ramp show. The map is data, so its
        // sRGB-decoded values are brought back to the stored ones.
        case 3u: {
            let t = textureSample(diffuse, diffuse_sampler, i.uv);
            let ramp = pow(t.g, 1.0 / 2.2);
            let shown = clamp((ramp - draw.p.a.w) / max(draw.p.b.y, 0.001) + 0.5, 0.0, 1.0);
            a = alpha * shown * pow(t.r, 1.0 / 2.2);
        }
        // osd3d_needleblur
        case 4u: {
            let t = textureSample(diffuse, diffuse_sampler, i.uv);
            rgb = t.rgb * colour;
            a = t.a * alpha * draw.p.c.w;
        }
        // osd3d_reflect_colour: colour plus the sharp environment
        case 5u: {
            rgb = colour + adjust(textureSample(sharp, sharp_sampler, r).rgb) * draw.p.b.z * draw.p.b.w;
        }
        // osd3d_reflect_colour_text(_flat): colour lit by both environments
        case 6u: {
            let s = textureSample(soft, soft_sampler, r).rgb;
            let h = textureSample(sharp, sharp_sampler, r).rgb;
            rgb = colour * (s * draw.p.b.z + h * max(draw.p.b.w - 1.0, 0.0));
        }
        // osd3d_reflect_image
        case 7u: {
            let t = textureSample(diffuse, diffuse_sampler, i.uv);
            rgb = t.rgb * colour + adjust(textureSample(sharp, sharp_sampler, r).rgb) * draw.p.b.z * draw.p.b.w;
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
