// The original CheapFinalMerge ColourBalance and its vignette
// (docs/FLASHBACK.md section 2.5), applied in display space after tonemapping.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Settings {
    // contrast, brightness, maximum, colour
    balance: vec4<f32>,
    // pre-tint rgb, vignette amount
    tint: vec4<f32>,
    // vignette rgb, vignette shape
    vignette: vec4<f32>,
    // vignette scale x, y, offset x, y
    frame: vec4<f32>,
    // cos and sin of the rotation, transition, unused
    rotation: vec4<f32>,
}
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: Settings;

fn to_display(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055,
                  c * 12.92, c <= vec3(0.0031308));
}
fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

@fragment
fn fragment(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let original = textureSample(scene, scene_sampler, input.uv);
    // vignette.vsh: screen position, rotated, shaped, offset and scaled.
    let p = vec2(2.0 * input.uv.x - 1.0, 1.0 - 2.0 * input.uv.y);
    let c = settings.rotation.x;
    let s = settings.rotation.y;
    let r = vec2(c * p.x - s * p.y, c * p.y + s * p.x);
    let q = 0.89 * settings.frame.xy
        * (settings.vignette.w * r + vec2(-settings.frame.z, settings.frame.w));
    let tau = settings.rotation.z;
    let z = saturate(saturate(dot(q, q)) / tau + 1.0 - 1.0 / tau);
    let weight = settings.tint.w * z * z * (3.0 - 2.0 * z);
    // CheapFinalMerge: saturation, tint, vignette, contrast and brightness.
    let rgb = to_display(original.rgb);
    let luma = dot(rgb, vec3(0.2125, 0.7154, 0.0721));
    let tinted = mix(vec3(luma), rgb, settings.balance.w) * settings.tint.rgb;
    let shaded = mix(tinted, settings.vignette.rgb, weight);
    // The original writes a unorm target, which holds no negative value.
    let graded = clamp(shaded * settings.balance.x + settings.balance.y,
                       vec3(0.0), vec3(settings.balance.z));
    return vec4(to_linear(graded), original.a);
}
