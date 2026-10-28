// Read-only bounded HDR evidence. No render target writes or history changes.
struct Parameters { identity: vec4<u32>, dimensions_origin: vec4<u32>, extent: vec4<u32> };
@group(0) @binding(0) var colour: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Parameters;
@group(0) @binding(2) var<storage, read_write> output: array<vec4<u32>>;
@compute @workgroup_size(8,8,1)
fn main(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let xy = invocation.xy;
    if any(xy >= params.extent.xy) { return; }
    if all(xy == vec2<u32>(0u)) {
        output[0] = params.identity;
        output[1] = params.dimensions_origin;
        output[2] = params.extent;
    }
    let pixel = xy + params.dimensions_origin.zw;
    if any(pixel >= textureDimensions(colour)) {
        output[3u + xy.y * params.extent.x + xy.x] = vec4<u32>(0x7fc00000u);
        return;
    }
    output[3u + xy.y * params.extent.x + xy.x] =
        bitcast<vec4<u32>>(textureLoad(colour, vec2<i32>(pixel), 0));
}
