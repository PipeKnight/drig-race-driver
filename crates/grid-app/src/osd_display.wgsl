// Draws the OSD render (premultiplied colour) under the rest of the UI.
#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var osd: texture_2d<f32>;
@group(1) @binding(1) var osd_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(osd, osd_sampler, in.uv);
}
