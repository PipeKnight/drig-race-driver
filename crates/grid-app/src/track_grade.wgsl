#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Settings {
    controls: vec4<f32>,
    tint: vec4<f32>,
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
    let rgb = to_display(original.rgb);
    let luma = dot(rgb, vec3(0.2125, 0.7154, 0.0721));
    let saturated = mix(vec3(luma), rgb, settings.controls.x);
    let tinted = saturated * settings.tint.rgb;
    let balanced = clamp(tinted * settings.controls.y + settings.controls.z,
                         vec3(0.0), vec3(settings.tint.w));
    let graded = pow(balanced, vec3(settings.controls.w));
    return vec4(to_linear(graded), original.a);
}
