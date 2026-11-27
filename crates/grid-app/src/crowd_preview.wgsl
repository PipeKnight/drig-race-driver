// Crowd tint and the route's encoded lighting for distant spectator cards.
#import bevy_pbr::{forward_io::{VertexOutput,FragmentOutput},pbr_fragment::pbr_input_from_standard_material,pbr_functions::{apply_pbr_lighting,main_pass_post_lighting_processing,alpha_discard}}
#import bevy_pbr::mesh_view_bindings::view
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var tint_map:texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var tint_sampler:sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> colour0:vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> colour1:vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> colour2:vec4<f32>;

struct BillboardLight {
    sun: vec4<f32>,
    back: vec4<f32>,
    ambient: vec4<f32>,
    direction: vec4<f32>,
    fog: vec4<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> billboard_light: BillboardLight;

fn display_to_linear(display: vec3<f32>) -> vec3<f32> {
    let c = max(display, vec3(0.0));
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

#ifdef GRID_SOURCE_CARD_COVERAGE
struct CardOutput {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) samples: u32,
}

fn card_output(colour: vec4<f32>) -> CardOutput {
    let alpha = colour.a;
#ifdef GRID_CARD_COVERAGE_2
    let covered = select(select(0u, 1u, alpha >= 0.25), 3u, alpha >= 0.75);
#else
    // Original sample order at standard Vulkan 4x locations: 3, 0, 1, 2.
    let low = select(0u, 8u, alpha >= 0.125);
    let mid = select(low, 9u, alpha >= 0.375);
    let high = select(mid, 11u, alpha >= 0.625);
    let covered = select(high, 15u, alpha >= 0.875);
#endif
    return CardOutput(vec4(colour.rgb, 1.0), covered);
}
#else
alias CardOutput = FragmentOutput;

fn card_output(colour: vec4<f32>) -> CardOutput {
    var out: FragmentOutput;
    out.color = colour;
    return out;
}
#endif

@fragment
fn fragment(in:VertexOutput,@builtin(front_facing) front:bool)->CardOutput{
    var pbr=pbr_input_from_standard_material(in,front);
    let mask=textureSample(tint_map,tint_sampler,in.uv).rgb;
    let rest=clamp(1.0-mask.r-mask.g-mask.b,0.0,1.0);
    let tint=clamp(colour0.rgb*mask.r+colour1.rgb*mask.g+colour2.rgb*mask.b+vec3<f32>(rest),vec3<f32>(0.0),vec3<f32>(1.0));
#ifdef VERTEX_UVS_B
    if billboard_light.back.w > 0.5 {
        let base = alpha_discard(pbr.material, pbr.material.base_color);
        let horizontal = pbr.N.xz / max(length(pbr.N.xz), 0.0001);
        let sun_dot = dot(horizontal, billboard_light.direction.xz);
        let sun_weight = saturate(sun_dot) * mix(1.0, in.uv_b.x, billboard_light.sun.w);
        let illumination = billboard_light.sun.rgb * sun_weight
            + billboard_light.back.rgb * (0.5 * (1.0 - sun_dot))
            + billboard_light.ambient.rgb;
        let range = distance(pbr.world_position.xyz, view.world_position);
        let haze = saturate((range - 30.0) / max(billboard_light.ambient.w, 0.001))
            * billboard_light.direction.w;
        let display = mix(base.rgb * tint * illumination, billboard_light.fog.rgb, haze);
        return card_output(vec4(display_to_linear(display), base.a));
    }
#endif
    pbr.material.base_color=vec4<f32>(pbr.material.base_color.rgb*tint,pbr.material.base_color.a);
    pbr.material.base_color=alpha_discard(pbr.material,pbr.material.base_color);
    var out:FragmentOutput;out.color=apply_pbr_lighting(pbr);
    return card_output(main_pass_post_lighting_processing(pbr,out.color));
}
