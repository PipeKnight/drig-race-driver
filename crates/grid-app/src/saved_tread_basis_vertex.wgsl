// Rigid-forward diagnostic entry. No import-path override or stock VS fork.
#import bevy_pbr::{mesh_functions,mesh_view_bindings::view,view_transformations::position_world_to_clip}
struct Input {
 @builtin(instance_index) instance_index:u32,
 @location(0) position:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32>,
#ifdef VERTEX_TANGENTS
 @location(4) tangent:vec4<f32>,
#endif
 @location(8) raw_t:vec3<f32>, @location(9) raw_b:vec3<f32>, @location(10) raw_nm:vec4<f32>,
}
struct Output {
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
@vertex fn vertex(in:Input)->Output {
 var out:Output;
 let transform=mesh_functions::get_world_from_local(in.instance_index);
 out.world_position=mesh_functions::mesh_position_local_to_world(transform,vec4<f32>(in.position,1.0));
 out.position=position_world_to_clip(out.world_position.xyz);
 out.world_normal=mesh_functions::mesh_normal_local_to_world(in.normal,in.instance_index);
 out.uv=in.uv;out.instance_index=in.instance_index;
#ifdef VERTEX_TANGENTS
 out.world_tangent=mesh_functions::mesh_tangent_local_to_world(transform,in.tangent,in.instance_index);
#endif
 let linear=mat3x3<f32>(transform[0].xyz,transform[1].xyz,transform[2].xyz);
 out.raw_t=linear*in.raw_t;out.raw_b=linear*in.raw_b;out.raw_n=linear*in.raw_nm.xyz;out.mask=in.raw_nm.w;
#ifdef VISIBILITY_RANGE_DITHER
 out.visibility_range_dither=mesh_functions::get_visibility_range_dither_level(in.instance_index,transform[3]);
#endif
 return out;
}
