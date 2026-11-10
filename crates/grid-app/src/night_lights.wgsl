// GRID_NIGHT_LIGHTS_V1
// Night-light term of the original's NightLights fragments
// (docs/NIGHT_LIGHTS.md). Inserted into bevy_pbr::lighting by
// night_lights.rs. A night light is a shadowless spot light whose
// shadow_depth_bias is the marker; its other fields carry:
//   shadow_map_near_z  weight 3 * min(record weight, 1)
//   (the marker itself picks the cookie half: headlight left, street light right)
//   position_radius.w  headlightdirectional d
//   color (premultiplied) the shared lightColour * lightStrength * unit
fn grid_night_light(light_id: u32) -> bool {
    let marker = view_bindings::clustered_lights.data[light_id].shadow_depth_bias;
    return marker == GRID_NIGHT_HEADLIGHT || marker == GRID_NIGHT_STREET;
}
// One light's contribution before the shared saturate and colour.
fn grid_night_term(light_id: u32, position: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let light = &view_bindings::clustered_lights.data[light_id];
    let to_point = position - (*light).position_radius.xyz;
    // Spot forward, as bevy_pbr's spot_light reconstructs it.
    var dir = vec3<f32>((*light).light_custom_data.x, 0.0, (*light).light_custom_data.y);
    dir.y = sqrt(max(0.0, 1.0 - dir.x * dir.x - dir.z * dir.z));
    if ((*light).flags & POINT_LIGHT_FLAGS_SPOT_LIGHT_Y_NEGATIVE) != 0u {
        dir.y = -dir.y;
    }
    // Light frame q = (cross(up, dir), -up, dir) . v. Without the cookie
    // decal, up comes from the spot direction and world Y as the street
    // lights' does (0x72C460); with it, from the light's own transform.
    let side = cross(vec3<f32>(0.0, 1.0, 0.0), dir);
    var up = vec3<f32>(1.0, 0.0, 0.0);
    if dot(side, side) > 0.0 {
        up = cross(dir, normalize(side));
    }
    var q = vec3<f32>(dot(cross(up, dir), to_point), -dot(up, to_point), dot(dir, to_point));
    var cookie = vec3<f32>(1.0);
    var textured = false;
#ifdef CLUSTERED_DECALS_ARE_USABLE
    if (*light).decal_index != 0xFFFFFFFFu {
        let decal = &view_bindings::clustered_decals.decals[(*light).decal_index];
        if (*decal).tag == GRID_NIGHT_TAG {
            // Bevy light space: +x = dir x up, +y = up, -z = dir.
            let local = ((*decal).local_from_world * vec4(position, 1.0)).xyz;
            q = vec3<f32>(-local.x, -local.y, -local.z);
            textured = true;
        }
    }
#endif
    let range = (*light).range;
    if q.z <= 0.0 || 0.5 * abs(q.x) >= q.z || 0.5 * abs(q.y) >= q.z {
        return vec3<f32>(0.0);
    }
    let attenuation = saturate(1.0 - length(q) / range);
    let d = (*light).position_radius.w;
    let facing = saturate((1.0 - d) * dot(normal, -normalize(to_point)) + d);
#ifdef CLUSTERED_DECALS_ARE_USABLE
    if textured {
        let decal = &view_bindings::clustered_decals.decals[(*light).decal_index];
        var uv = q.xy / q.z * 0.25 + vec2<f32>(0.5);
        uv.x = uv.x * 0.5 + select(0.0, 0.5, (*light).shadow_depth_bias == GRID_NIGHT_STREET);
        cookie = textureSampleLevel(
            view_bindings::clustered_decal_textures[(*decal).base_color_texture_index],
            view_bindings::clustered_decal_sampler, uv, 0.0).rgb;
    }
#endif
    return cookie * (attenuation * facing * (*light).shadow_map_near_z);
}
