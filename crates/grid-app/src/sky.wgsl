#import bevy_pbr::{forward_io::Vertex, mesh_functions, view_transformations::position_world_to_clip}

struct SkyOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var sky_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var sky_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> sky_tone: vec4<f32>;

@vertex
fn vertex(input: Vertex) -> SkyOutput {
    let model = mesh_functions::get_world_from_local(input.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(model, vec4(input.position, 1.0));
    var clip = position_world_to_clip(world.xyz);
    // Reversed-Z background: retain projected direction, never cover opaque track depth.
    clip.z = 0.0;
    return SkyOutput(clip, input.uv);
}

@fragment
fn fragment(input: SkyOutput) -> @location(0) vec4<f32> {
    let colour = textureSample(sky_texture, sky_sampler, input.uv).rgb;
    return vec4(pow(max(colour * sky_tone.x, vec3(0.0)), vec3(sky_tone.y)), 1.0);
}
