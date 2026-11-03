// Saved-basis direction in normalized PBR; not original lighting parity.
#import bevy_pbr::{forward_io::{VertexOutput,FragmentOutput},pbr_fragment::pbr_input_from_standard_material,pbr_functions::{alpha_discard,apply_pbr_lighting,main_pass_post_lighting_processing},mesh_view_bindings::view,decal::clustered::apply_decals}
#ifdef VISIBILITY_RANGE_DITHER
#import bevy_pbr::pbr_functions
#endif
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var normal_map:texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var normal_sampler:sampler;
struct Input {
 @builtin(position) position:vec4<f32>, @location(0) world_position:vec4<f32>,
 @location(1) world_normal:vec3<f32>, @location(2) uv:vec2<f32>,
#ifdef VERTEX_TANGENTS
 @location(4) world_tangent:vec4<f32>,
#endif
 @location(6) @interpolate(flat) instance_index:u32,
#ifdef VISIBILITY_RANGE_DITHER
 @location(7) @interpolate(flat) visibility_range_dither:i32,
#endif
 @location(8) @interpolate(perspective, centroid) raw_t:vec3<f32>, @location(9) @interpolate(perspective, centroid) raw_b:vec3<f32>, @location(10) @interpolate(perspective, centroid) raw_n:vec3<f32>, @location(11) mask:f32,
}
fn finite_normal(value:vec3<f32>,fallback:vec3<f32>)->vec3<f32>{
 let squared=dot(value,value);
 // Authored finite safety for zero/invalid filtered normal; no source parity.
 if squared>0.0 && squared<3.402823e38{return value*inverseSqrt(squared);}
 return normalize(fallback);
}
@fragment fn fragment(in:Input,@builtin(front_facing) front:bool)->FragmentOutput{
#ifdef VISIBILITY_RANGE_DITHER
 pbr_functions::visibility_range_dither(in.position,in.visibility_range_dither);
#endif
 var stock:VertexOutput;
 stock.position=in.position;stock.world_position=in.world_position;stock.world_normal=in.world_normal;stock.uv=in.uv;
#ifdef VERTEX_TANGENTS
 stock.world_tangent=in.world_tangent;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
 stock.instance_index=in.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
 stock.visibility_range_dither=in.visibility_range_dither;
#endif
 var pbr=pbr_input_from_standard_material(stock,front);
 let rgb=textureSampleBias(normal_map,normal_sampler,in.uv,view.mip_bias).rgb;
 let m=in.mask;
 let q=finite_normal(vec3<f32>(m*(2.0*rgb.r-1.0),m*(2.0*rgb.g-1.0),m*(2.0*rgb.b-2.0)+1.0),vec3<f32>(0.0,0.0,1.0));
 let full=(q.y*in.raw_b+q.x*in.raw_t)+q.z*in.raw_n;
 pbr.N=finite_normal(full,pbr.world_normal);
 // Source raw length does NOT survive this explicitly normalized PBR adapter.
 pbr.material.base_color=alpha_discard(pbr.material,pbr.material.base_color);
 apply_decals(&pbr);
 var out:FragmentOutput;out.color=apply_pbr_lighting(pbr);
 out.color=main_pass_post_lighting_processing(pbr,out.color);return out;
}
