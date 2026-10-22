// Independently authored diagnostic layer; no proprietary shader code embedded.
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing, alpha_discard},
    mesh_view_bindings as view_bindings,
    clustered_forward as clustering,
    environment_map::compute_radiances,
    lighting::{LayerLightingInput, perceptualRoughnessToRoughness},
}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> controls: vec4<f32>;
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, front);
    pbr.material.base_color = alpha_discard(pbr.material, pbr.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr);
#ifdef ENVIRONMENT_MAP
    if controls.x > 0.0 {
        let normal = normalize(pbr.N);
        let view_z = dot(vec4<f32>(view_bindings::view.view_from_world[0].z,
            view_bindings::view.view_from_world[1].z, view_bindings::view.view_from_world[2].z,
            view_bindings::view.view_from_world[3].z), pbr.world_position);
        let cluster = clustering::view_fragment_cluster_index(pbr.frag_coord.xy, view_z, pbr.is_orthographic);
        var ranges = clustering::unpack_clusterable_object_index_ranges(cluster);
        var layer: LayerLightingInput;
        layer.N = normal;
        layer.R = reflect(-pbr.V, normal);
        layer.NdotV = max(clamp(dot(normal, pbr.V), 0.0, 1.0), 0.0001);
        layer.perceptual_roughness = controls.y;
        layer.roughness = perceptualRoughnessToRoughness(controls.y);
        let radiances = compute_radiances(layer, &ranges, pbr.world_position.xyz, false);
        // Preserve source alpha. Blend reflected radiance in linear exposed HDR,
        // once, before the stock fog and tone mapping. No body-mask gate.
        let reflected = radiances.radiance * view_bindings::view.exposure;
        out.color = vec4<f32>(mix(out.color.rgb, reflected, controls.x), out.color.a);
    }
#endif
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
}
