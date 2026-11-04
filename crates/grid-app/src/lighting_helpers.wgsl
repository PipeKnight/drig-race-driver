// GRID_SOURCE_PROJECTOR_AGGREGATE_V2
fn grid_source_projector(light_id: u32) -> bool {
#ifdef LIGHT_TEXTURES
    let light = &view_bindings::clustered_lights.data[light_id];
    if (*light).decal_index != 0xFFFFFFFFu {
        return view_bindings::clustered_decals.decals[(*light).decal_index].tag == 1196575044u;
    }
#endif
    return false;
}
// Pure local function: no mutable invocation-global state or PBR response.
fn grid_source_raw(light_id: u32, position: vec3<f32>) -> vec3<f32> {
#ifdef LIGHT_TEXTURES
    let light = &view_bindings::clustered_lights.data[light_id];
    if grid_source_projector(light_id) {
        let decal = &view_bindings::clustered_decals.decals[(*light).decal_index];
        let local = ((*decal).local_from_world * vec4(position, 1.0)).xyz;
        let forward = -local.z;
        if forward <= 0.0 || 0.5 * abs(local.x) >= forward || 0.5 * abs(local.y) >= forward {
            return vec3(0.0);
        }
        let radial = saturate(1.0 - length(local) / (*light).range);
        let uv = local.xy / forward * vec2(0.25, -0.25) + vec2(0.5);
        let rgb = textureSampleLevel(
            view_bindings::clustered_decal_textures[(*decal).base_color_texture_index],
            view_bindings::clustered_decal_sampler, uv, 0.0).rgb;
        // Host WHITE * (4PI * normalized source weight), Bevy extracts /4PI.
        return rgb * radial * (*light).color_inverse_square_range.r;
    }
#endif
    return vec3(0.0);
}
