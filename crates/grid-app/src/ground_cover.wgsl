// Camera-facing ground cover (F580): the CPU path's corner expansion and
// distance fade, moved into the vertex stage. Every pass (main, prepass,
// deferred) faces the main camera given in `cover`, as the CPU path did.
// Vertex inputs: position = billboard centre, uv_b = corner offset (sprite
// corner x width, y height), color.a = draw distance of the instance's subset
// (0 for padding: never drawn).
#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{Vertex, VertexOutput}
#else
#import bevy_pbr::forward_io::{Vertex, VertexOutput}
#endif

struct Cover {
    right: vec4<f32>,
    up: vec4<f32>,
    eye: vec4<f32>,
    // x: fade start as a share of the draw distance.
    fade: vec4<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> cover: Cover;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let centre = vertex.position;
#ifdef VERTEX_COLORS
    let limit = vertex.color.a;
#else
    let limit = 0.0;
#endif
#ifdef VERTEX_UVS_B
    let offset = vertex.uv_b;
#else
    let offset = vec2<f32>(0.0);
#endif
    let distance = length(centre - cover.eye.xyz);
    let start = cover.fade.x * limit;
    var alpha = 1.0;
    if distance > start {
        alpha = 1.0 - (distance - start) / max(limit - start, 1e-3);
    }
    let corner = centre + cover.right.xyz * offset.x + cover.up.xyz * offset.y;
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(corner, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    if distance >= limit {
        // Outside the instance's draw distance: a degenerate quad.
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
#ifdef PREPASS_PIPELINE
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef PREPASS_PIPELINE
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#endif
#ifdef MOTION_VECTOR_PREPASS
    // The CPU path rewrote the vertices: the previous transform applied to
    // the current corner, which is the current corner (identity transform).
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(
        mesh_functions::get_previous_world_from_local(vertex.instance_index),
        vec4<f32>(corner, 1.0),
    );
#endif
#else
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#endif

#ifdef VERTEX_COLORS
    out.color = vec4<f32>(1.0, 1.0, 1.0, alpha);
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif
    return out;
}
