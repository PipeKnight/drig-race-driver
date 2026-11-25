#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#ifdef SRGB_TO_LINEAR
#import bevy_render::color_operations::srgb_to_linear
#endif
#ifdef OKLAB_TO_LINEAR
#import bevy_render::color_operations::oklab_to_linear_rgb
#endif

@group(0) @binding(0) var in_texture: texture_2d<f32>;
@group(0) @binding(1) var in_sampler: sampler;

@fragment
fn fs_main(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // DRIG patch (F927): under Vulkan pre-rotation the output is in the panel's orientation
    // and holds the picture rotated clockwise by the display transform; sample the picture
    // at the source position of this output pixel.
    var uv = in.uv;
#ifdef DRIG_PRE_ROTATE_90
    uv = vec2(in.uv.y, 1.0 - in.uv.x);
#endif
#ifdef DRIG_PRE_ROTATE_180
    uv = vec2(1.0 - in.uv.x, 1.0 - in.uv.y);
#endif
#ifdef DRIG_PRE_ROTATE_270
    uv = vec2(1.0 - in.uv.y, in.uv.x);
#endif
    var color = textureSample(in_texture, in_sampler, uv);
#ifdef SRGB_TO_LINEAR
    color = vec4(srgb_to_linear(color.rgb), color.a);
#endif
#ifdef OKLAB_TO_LINEAR
    color = vec4(oklab_to_linear_rgb(color.rgb), color.a);
#endif
    return color;
}
