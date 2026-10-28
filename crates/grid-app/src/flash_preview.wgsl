// Diagnostic source FP adaptation: no PBR/light/fog/emitted scene light.
// Raw sample RGB; opacity applied ONCE to alpha before SRC_ALPHA/ONE blending.
#import bevy_pbr::forward_io::VertexOutput
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var flash_map:texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var flash_sampler:sampler;
@fragment
fn fragment(in:VertexOutput)->@location(0) vec4<f32>{
    let texel=textureSample(flash_map,flash_sampler,in.uv);
    return vec4<f32>(texel.rgb,texel.a*in.color.a);
}
