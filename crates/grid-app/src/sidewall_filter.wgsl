#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{VertexOutput, FragmentOutput}
#import bevy_pbr::pbr_deferred_functions::deferred_output
#else
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
#import bevy_pbr::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}
#endif
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var sidewall: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var sidewall_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> exposure: vec4<f32>;
// Wrapped Gaussian on a circular domain. Narrow kernels use real-space
// images; wider ones use the equivalent rapidly converging Fourier series.
fn circular_weight(theta: f32, sigma: f32) -> f32 {
    if sigma <= 1.0 {
        let a = theta / sigma;
        let b = (theta + 6.28318530718) / sigma;
        let c = (theta - 6.28318530718) / sigma;
        return exp(-0.5*a*a) + exp(-0.5*b*b) + exp(-0.5*c*c);
    }
    var weight = 1.0;
    for (var n = 1u; n <= 8u; n += 1u) {
        let k = f32(n);
        weight += 2.0 * exp(-0.5*k*k*sigma*sigma) * cos(k*theta);
    }
    return max(weight, 0.0);
}
@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(input, is_front);
    // Verified polar centre (0.5,0.5); symmetric shutter supports mirrored sides.
    if exposure.x > 0.0001 {
        let radial = input.uv - vec2(0.5);
        let dx = dpdx(input.uv);
        let dy = dpdy(input.uv);
        var colour = vec4(0.0);
        var weight = 0.0;
        for (var i = 0u; i < 64u; i += 1u) {
            let angle = ((f32(i) + 0.5) / 64.0 - 0.5) * min(exposure.x,6.28318530718);
            let c = cos(angle);
            let s = sin(angle);
            let rotation = mat2x2(vec2(c, s), vec2(-s, c));
            var w = 1.0;
            if exposure.y > 0.5 { w = circular_weight(angle,exposure.x/6.0); }
            colour += textureSampleGrad(sidewall, sidewall_sampler,
                vec2(0.5) + rotation * radial, rotation * dx, rotation * dy)*w;
            weight += w;
        }
        pbr.material.base_color = colour / weight;
    }
#ifdef PREPASS_PIPELINE
    return deferred_output(input, pbr);
#else
    var output: FragmentOutput;
    output.color = apply_pbr_lighting(pbr);
    output.color = main_pass_post_lighting_processing(pbr, output.color);
    return output;
#endif
}
