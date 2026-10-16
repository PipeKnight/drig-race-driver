#import bevy_pbr::{forward_io::{VertexOutput, FragmentOutput}, mesh_view_bindings}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> wheel_world: mat4x4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> wheel_inverse: mat4x4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> controls: vec4<f32>;
@fragment
fn fragment(input: VertexOutput) -> FragmentOutput {
    if controls.x < 0.01 { discard; }
    let axis = normalize(wheel_world[1].xyz);
    let facing = abs(dot(axis, normalize(mesh_view_bindings::view.world_position-input.world_position.xyz)));
    // An edge-on hull is not a reliable radial reconstruction domain.
    if facing < 0.2 { discard; }
    let local = wheel_inverse * input.world_position;
    let extent = mesh_view_bindings::view.viewport.zw;
    let original_uv = input.position.xy / extent;
    let original = textureSampleLevel(mesh_view_bindings::view_transmission_texture,
        mesh_view_bindings::view_transmission_sampler, original_uv, 0.0);
    var sum = vec4(0.0);
    var weight = 0.0;
    for (var i = 0u; i < 64u; i += 1u) {
        let a = ((f32(i)+0.5)/64.0-0.5)*controls.x;
        let c = cos(a);
        let s = sin(a);
        let p = vec4(c*local.x-s*local.z,local.y,s*local.x+c*local.z,1.0);
        let clip = mesh_view_bindings::view.clip_from_world * wheel_world * p;
        if clip.w <= 0.0 { continue; }
        let ndc = clip.xyz / clip.w;
        let uv = ndc.xy * vec2(0.5,-0.5) + vec2(0.5);
        if any(uv < vec2(0.0)) || any(uv >= vec2(1.0)) { continue; }
#ifdef DEPTH_PREPASS
        let depth = textureLoad(mesh_view_bindings::depth_prepass_texture, vec2<i32>(uv*extent), 0);
        // Reversed Z: reject foreground geometry in front of the swept hull.
        if depth > ndc.z + 0.00001 { continue; }
#endif
        let u = (f32(i)+0.5)/64.0-0.5;
        let w = select(1.0,exp(-0.5*(u*6.0)*(u*6.0)),controls.y>0.5);
        sum += textureSampleLevel(mesh_view_bindings::view_transmission_texture,
            mesh_view_bindings::view_transmission_sampler, uv, 0.0)*w;
        weight += w;
    }
    var output: FragmentOutput;
    if weight <= 0.0 { discard; }
    let filtered = sum / max(weight,1.0);
    let blend = smoothstep(0.01,0.15,controls.x) * smoothstep(0.2,0.4,facing);
    output.color = vec4(mix(original.rgb, filtered.rgb, blend), 1.0);
    return output;
}
