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
            let angle = ((f32(i) + 0.5) / 64.0 - 0.5) * exposure.x;
            let c = cos(angle);
            let s = sin(angle);
            let rotation = mat2x2(vec2(c, s), vec2(-s, c));
            let u = (f32(i)+0.5)/64.0-0.5;
            let w = select(1.0,exp(-0.5*(u*6.0)*(u*6.0)),exposure.y>0.5);
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
