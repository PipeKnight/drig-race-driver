// Shows the scaled 3D image opaque (render_scale.rs): colour only, alpha 1.
#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var scene: texture_2d<f32>;
@group(1) @binding(1) var scene_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    return vec4(textureSample(scene, scene_sampler, in.uv).rgb, 1.0);
}
