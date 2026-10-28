// Independently authored neutral C/T composer. Input RGB is associated colour; A is transmittance.
@group(0) @binding(0) var ct: texture_2d<f32>;
@fragment
fn fragment(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(ct,vec2<i32>(pixel.xy),0);
}
