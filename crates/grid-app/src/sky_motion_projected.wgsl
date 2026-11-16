#import bevy_render::view::View
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_pbr::view_transformations::uv_to_ndc

// Pinned Bevy depth-zero motion prepass ABI.
struct PreviousViewUniforms {
    view_from_world: mat4x4<f32>,
    clip_from_world: mat4x4<f32>,
    clip_from_view: mat4x4<f32>,
    world_from_clip: mat4x4<f32>,
    view_from_clip: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> previous_view: PreviousViewUniforms;

fn projected_motion(current_clip: vec4<f32>, previous_clip: vec4<f32>) -> vec2<f32> {
    let w = vec2(current_clip.w, previous_clip.w);
    let exponent = bitcast<vec2<u32>>(w) & vec2<u32>(0x7f800000u);
    // A ray behind either camera has no previous screen-space correspondence.
    if !all(w > vec2(0.0)) || any(exponent == vec2<u32>(0x7f800000u)) {
        return vec2(0.0);
    }
    let current_ndc = current_clip.xy / current_clip.w;
    let previous_ndc = previous_clip.xy / previous_clip.w;
    let velocity = (current_ndc - previous_ndc) * vec2(0.5, -0.5);
    // Keep finite offscreen positions. Degenerate divisions and RG16Float
    // overflow use the same zero fallback as a ray behind the previous view.
    if !all(abs(velocity) <= vec2(65504.0)) {
        return vec2(0.0);
    }
    return velocity;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(1) vec4<f32> {
    let ray = view.world_from_clip * vec4(uv_to_ndc(in.uv), 0.0, 1.0);
    let current_clip = view.unjittered_clip_from_world * ray;
    let previous_clip = previous_view.clip_from_world * ray;
    return vec4(projected_motion(current_clip, previous_clip), 0.0, 1.0);
}
