// Independently authored bounded main-view diagnostic.
// Bounded main-view extraction; raw float bits, no tone mapping/filtering.
@group(0) @binding(0) var motion: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_depth_2d;
struct Parameters {
    // request ID, extracted frame, physics tick, reserved
    identity: vec4<u32>,
    // texture width/height, ROI origin x/y
    dimensions_origin: vec4<u32>,
    // ROI width/height, reserved
    extent: vec4<u32>,
};
@group(0) @binding(2) var<uniform> params: Parameters;
@group(0) @binding(3) var<storage, read_write> output: array<vec4<u32>>;
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let xy = invocation.xy;
    if any(xy >= params.extent.xy) { return; }
    if all(xy == vec2<u32>(0u)) {
        output[0] = params.identity;
        output[1] = params.dimensions_origin;
        output[2] = params.extent;
    }
    let pixel = xy + params.dimensions_origin.zw;
    if any(pixel >= textureDimensions(motion)) || any(pixel >= textureDimensions(depth)) {
        // Explicit invalid marker; never call out-of-range zeros valid motion.
        output[3u + xy.y * params.extent.x + xy.x] = vec4<u32>(0u);
        return;
    }
    let velocity = textureLoad(motion, vec2<i32>(pixel), 0).xy;
    let z = textureLoad(depth, vec2<i32>(pixel), 0);
    output[3u + xy.y * params.extent.x + xy.x] =
        bitcast<vec4<u32>>(vec4<f32>(velocity, z, 1.0));
}
