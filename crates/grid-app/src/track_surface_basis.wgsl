// Private variant for source track surfaces only. Vertex stage is selected for
// mapped color/deferred pipelines. Stock auxiliary passes keep their shader.
#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::Vertex
#else
#import bevy_pbr::forward_io::Vertex
#endif

// Existing static VertexOutput locations match Bevy 0.19.1. The specialized
// variant admits position, normal, both UV sets and float4 color. T/B are
// dedicated world-space attributes, and are perspective-interpolated unchanged.
struct SurfaceOutput {
    @builtin(position) position: vec4<f32>,
#ifdef PREPASS_PIPELINE
    @location(0) uv: vec2<f32>,
    @location(1) uv_b: vec2<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(4) world_position: vec4<f32>,
#ifdef MOTION_VECTOR_PREPASS
    @location(5) previous_world_position: vec4<f32>,
#endif
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    @location(6) unclipped_depth: f32,
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    @location(7) instance_index: u32,
#endif
    @location(8) color: vec4<f32>,
#ifdef VISIBILITY_RANGE_DITHER
    @location(9) @interpolate(flat) visibility_range_dither: i32,
#endif
#else
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv_b: vec2<f32>,
    @location(5) color: vec4<f32>,
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    @location(6) @interpolate(flat) instance_index: u32,
#endif
#ifdef VISIBILITY_RANGE_DITHER
    @location(7) @interpolate(flat) visibility_range_dither: i32,
#endif
#endif
    @location(10) source_tangent: vec3<f32>,
    @location(11) source_binormal: vec3<f32>,
}

@vertex
fn vertex(
    input: Vertex,
    @location(8) source_tangent: vec3<f32>,
    @location(9) source_binormal: vec3<f32>,
) -> SurfaceOutput {
    var out: SurfaceOutput;
    let world_from_local = mesh_functions::get_world_from_local(input.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local, vec4<f32>(input.position, 1.0),
    );
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        input.normal, input.instance_index,
    );
    out.uv = input.uv;
    out.uv_b = input.uv_b;
    out.color = input.color;
    out.source_tangent = source_tangent;
    out.source_binormal = source_binormal;
#ifdef PREPASS_PIPELINE
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    let previous = mesh_functions::get_previous_world_from_local(input.instance_index);
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(
        previous, vec4<f32>(input.position, 1.0),
    );
#endif
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = input.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        input.instance_index, world_from_local[3],
    );
#endif
    return out;
}
