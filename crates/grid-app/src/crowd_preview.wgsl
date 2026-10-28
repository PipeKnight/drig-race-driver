// Independently authored source-tint adaptation for a bounded static diagnostic.
// Saved crowd shader's RGB tint contract is retained, but source lighting,
// source A2C and runtime animation/colour selection are not reproduced.
#import bevy_pbr::{forward_io::{VertexOutput,FragmentOutput},pbr_fragment::pbr_input_from_standard_material,pbr_functions::{apply_pbr_lighting,main_pass_post_lighting_processing,alpha_discard}}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var tint_map:texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var tint_sampler:sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> colour0:vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> colour1:vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> colour2:vec4<f32>;
@fragment
fn fragment(in:VertexOutput,@builtin(front_facing) front:bool)->FragmentOutput{
    var pbr=pbr_input_from_standard_material(in,front);
    let mask=textureSample(tint_map,tint_sampler,in.uv).rgb;
    let rest=clamp(1.0-mask.r-mask.g-mask.b,0.0,1.0);
    let tint=clamp(colour0.rgb*mask.r+colour1.rgb*mask.g+colour2.rgb*mask.b+vec3<f32>(rest),vec3<f32>(0.0),vec3<f32>(1.0));
    pbr.material.base_color=vec4<f32>(pbr.material.base_color.rgb*tint,pbr.material.base_color.a);
    pbr.material.base_color=alpha_discard(pbr.material,pbr.material.base_color);
    var out:FragmentOutput;out.color=apply_pbr_lighting(pbr);
    out.color=main_pass_post_lighting_processing(pbr,out.color);return out;
}
