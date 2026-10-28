struct Parameters {
    jittered_clip: mat4x4<f32>, current_clip: mat4x4<f32>, previous_clip: mat4x4<f32>,
    current_model: mat4x4<f32>, previous_model: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> p: Parameters;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) current: vec4<f32>,
    @location(1) previous: vec4<f32>,
}
@vertex fn vertex(@location(0) local: vec3<f32>) -> Output {
    var out: Output;
    let world = p.current_model * vec4(local, 1.0);
    out.position = p.jittered_clip * world;
    out.current = p.current_clip * world;
    out.previous = p.previous_clip * p.previous_model * vec4(local, 1.0);
    return out;
}
@fragment fn fragment(input: Output) -> @location(0) vec2<f32> {
    if input.current.w <= 0.0 || input.previous.w <= 0.0 { discard; }
    return (input.current.xy / input.current.w - input.previous.xy / input.previous.w) * vec2(0.5, -0.5);
}
