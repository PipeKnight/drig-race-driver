//! The original's sun shadow layout (docs/SUN_SHADOWS.md "Layout"): three
//! maps of one size, set by the `[video] shadows` quality like the original's
//! `<shadows>` option: off none, medium 1024, high 2048 (the default). Low 512
//! and ultra 4096 are the owner's extensions (2026-10-30): the original's low
//! has no shadows and it has no ultra.
//!
//! - Cascade 0 is the player car's map (the original's `car_layer` map,
//!   0x5CCDC0): a light-space box around the player car, drawn with the
//!   player car only. Receivers inside the box take the darker of it and the
//!   world cascades; elsewhere it reads lit. It is off while the viewing
//!   camera is more than [`CAR_MAP_CAMERA_LIMIT`] from the car.
//! - Cascades 1 and 2 are the world's (every caster), split by view depth at
//!   the original's mode-0 ranges: 0.4 to 76 m and 60.8 to 252 m. Bevy blends
//!   the overlap; the last one fades out over its far 70 % in squared
//!   distance like the original's `shadowFade`.
//! - The filter is the original's mask filter per fragment (SUN_SHADOWS 2.6):
//!   a Poisson disc of fixed world radius, smoothed over the neighbours'
//!   kernel angles, so the softness does not follow the map size.
//!
//! The shader side is an in-memory patch of Bevy's `fetch_directional_shadow`
//! (`bevy_pbr::shadows`, pinned 0.19.1), applied like `clearcoat_prepass`.
//! `GRID_SHADOW_LAYOUT=bevy` keeps Bevy's four world cascades (comparisons).
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::CascadesVisibleEntities;
use bevy::light::cascade::Cascade;
use bevy::light::{CascadeShadowConfig, Cascades, DirectionalLightShadowMap, SimulationLightSystems};
use bevy::math::Vec3A;
use bevy::prelude::*;
use bevy::shader::{ShaderImport, Source};
use std::collections::HashSet;

/// Nearest shadowed view depth (mode table +0x20).
pub const NEAR: f32 = 0.4;
/// World cascade far bounds: range plus far extension of the mode-0
/// cascades 1 and 2 (75 + 1 m, 250 + 2 m; split setup 0x5DABC0).
pub const WORLD_FAR: [f32; 2] = [76., 252.];
/// Cascade 1's fade fraction (mode 0): the share of its range Bevy blends
/// into cascade 2.
pub const OVERLAP: f32 = 0.2;
/// The player car's map is off beyond this camera distance (0x5CD5F0, 50^2).
pub const CAR_MAP_CAMERA_LIMIT: f32 = 50.;
/// Room around the car's box in the map, metres (filter and bias) [authored].
const CAR_MARGIN: f32 = 0.5;
/// How far below the car, along the light, receivers take its map [authored].
const CAR_REACH: f32 = 50.;

/// The `[video] shadows` option.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    Off,
    /// 512: the owner's extension; the original's low is off.
    Low,
    Medium,
    #[default]
    High,
    /// 4096: the owner's extension beyond the original.
    Ultra,
}

impl Quality {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "ultra" => Ok(Self::Ultra),
            _ => Err("`shadows` needs off, low, medium, high or ultra".into()),
        }
    }

    /// The default without a saved option: high on the desktop, low with the
    /// mobile preset (the owner's choice for Android, 2026-10-31, an
    /// exception to the preset's resolution-only cut).
    pub fn default_for(mobile: bool) -> Self {
        if mobile { Self::Low } else { Self::High }
    }

    /// Map size, `None` without shadows (hardware_settings_options.xml for
    /// off, medium and high).
    pub fn map_size(self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::Low => Some(512),
            Self::Medium => Some(1024),
            Self::High => Some(2048),
            Self::Ultra => Some(4096),
        }
    }
}

pub fn enabled() -> bool {
    std::env::var("GRID_SHADOW_LAYOUT").map_or(true, |v| v != "bevy")
}

/// Cascade 0's bounds are a placeholder the car map replaces; they put
/// cascade 1's near plane at [`NEAR`].
pub fn config(world_far: f32) -> CascadeShadowConfig {
    CascadeShadowConfig {
        bounds: vec![NEAR / (1. - OVERLAP), WORLD_FAR[0], world_far],
        overlap_proportion: OVERLAP,
        minimum_distance: NEAR,
    }
}

/// The player car's caster entities this frame.
#[derive(Resource, Default)]
pub struct CarCasters(pub HashSet<Entity>);

pub fn install(app: &mut App, quality: Quality) {
    app.init_resource::<CarCasters>()
        .insert_resource(LayoutQuality(quality))
        .add_systems(Update, patch_shader)
        .add_systems(
            PostUpdate,
            (
                car_map
                    .after(SimulationLightSystems::UpdateDirectionalLightCascades)
                    .before(SimulationLightSystems::UpdateLightFrusta)
                    .after(TransformSystems::Propagate),
                car_casters.after(SimulationLightSystems::CheckLightVisibility),
            ),
        );
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn car_map(
    shadow_map: Res<DirectionalLightShadowMap>,
    camera: Query<&GlobalTransform, With<super::ViewerCamera>>,
    parts: Query<(Entity, &super::track_surface::SourcePart), Without<super::OpponentPart>>,
    hulls: Query<(Entity, &super::wheel_exposure::Hull)>,
    children: Query<&Children>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    mut lights: Query<(&GlobalTransform, &mut Cascades)>,
    mut casters: ResMut<CarCasters>,
) {
    let set = &mut casters.0;
    set.clear();
    for (entity, part) in &parts {
        if part.0.starts_with("car0/") {
            set.insert(entity);
        }
    }
    for (entity, hull) in &hulls {
        if set.contains(&hull.source) {
            set.insert(entity);
        }
    }
    let roots: Vec<Entity> = set.iter().copied().collect();
    for root in roots {
        set.extend(children.iter_descendants(root));
    }
    let corners: Vec<Vec3A> = set
        .iter()
        .filter_map(|e| bounds.get(*e).ok())
        .flat_map(|(aabb, transform)| box_corners(aabb, transform))
        .collect();
    let near_camera = camera.single().ok().is_some_and(|camera| {
        let (min, max) = world_bounds(&corners);
        !corners.is_empty()
            && camera.translation_vec3a().distance(0.5 * (min + max)) <= CAR_MAP_CAMERA_LIMIT
    });
    if !near_camera {
        set.clear();
    }
    for (transform, mut cascades) in &mut lights {
        let world_from_light = Mat4::from_quat(transform.rotation());
        let cascade = if near_camera {
            car_cascade(&corners, world_from_light, shadow_map.size as f32)
        } else {
            // Every point maps to w = 0: the shader reads it as lit.
            Cascade { clip_from_world: Mat4::ZERO, ..default() }
        };
        for list in cascades.cascades.values_mut() {
            if let Some(first) = list.first_mut() {
                *first = cascade.clone();
            }
        }
    }
}

/// Cascade 0 keeps the player car's casters only.
fn car_casters(casters: Res<CarCasters>, mut lights: Query<&mut CascadesVisibleEntities>) {
    for mut visible in &mut lights {
        for lists in visible.entities.values_mut() {
            if let Some(first) = lists.first_mut() {
                first.entities.retain(|e| casters.0.contains(e));
            }
        }
    }
}

fn box_corners(aabb: &Aabb, transform: &GlobalTransform) -> [Vec3A; 8] {
    let affine = transform.affine();
    let (c, h) = (aabb.center, aabb.half_extents);
    std::array::from_fn(|i| {
        let s = Vec3A::new(
            if i & 1 == 0 { -1. } else { 1. },
            if i & 2 == 0 { -1. } else { 1. },
            if i & 4 == 0 { -1. } else { 1. },
        );
        affine.transform_point3a(c + s * h)
    })
}

fn world_bounds(points: &[Vec3A]) -> (Vec3A, Vec3A) {
    points.iter().fold((Vec3A::splat(f32::MAX), Vec3A::splat(f32::MIN)), |(lo, hi), p| {
        (lo.min(*p), hi.max(*p))
    })
}

/// An orthographic cascade around `points` (world space), built like Bevy's
/// `calculate_cascade`: a square of whole metres in steps of one metre so the
/// texel size changes rarely, its centre snapped to texels, its near plane at
/// the topmost point and its depth reaching [`CAR_REACH`] below the lowest.
fn car_cascade(points: &[Vec3A], world_from_light: Mat4, map_size: f32) -> Cascade {
    let light_from_world = world_from_light.transpose();
    let (min, max) = world_bounds(
        &points.iter().map(|p| light_from_world.transform_point3a(*p)).collect::<Vec<_>>(),
    );
    let diameter = ((max.x - min.x).max(max.y - min.y) + 2. * CAR_MARGIN).ceil();
    let texel_size = diameter / map_size;
    let near_plane_center = Vec3A::new(
        (0.5 * (min.x + max.x) / texel_size).floor() * texel_size,
        (0.5 * (min.y + max.y) / texel_size).floor() * texel_size,
        max.z + CAR_MARGIN,
    );
    let world_from_light_transpose = world_from_light.transpose();
    let cascade_from_world = Mat4::from_cols(
        world_from_light_transpose.x_axis,
        world_from_light_transpose.y_axis,
        world_from_light_transpose.z_axis,
        (-near_plane_center).extend(1.0),
    );
    let world_from_cascade = Mat4::from_cols(
        world_from_light.x_axis,
        world_from_light.y_axis,
        world_from_light.z_axis,
        world_from_light * near_plane_center.extend(1.0),
    );
    let r = (near_plane_center.z - min.z + CAR_REACH).recip();
    let clip_from_cascade = Mat4::from_cols(
        Vec4::new(2.0 / diameter, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / diameter, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 1.0),
    );
    Cascade {
        world_from_cascade,
        clip_from_cascade,
        clip_from_world: clip_from_cascade * cascade_from_world,
        texel_size,
    }
}

const BEVY_FETCH: &str = "fn fetch_directional_shadow(
    light_id: u32,
    frag_position: vec4<f32>,
    surface_normal: vec3<f32>,
    view_z: f32,
    frag_coord_xy: vec2<f32>,
) -> f32 {
    let light = &view_bindings::lights.directional_lights[light_id];
    let cascade_index = get_cascade_index(light_id, view_z);

    if (cascade_index >= (*light).num_cascades) {
        return 1.0;
    }
";

const LAYOUT_FETCH: &str = "fn fetch_directional_shadow(
    light_id: u32,
    frag_position: vec4<f32>,
    surface_normal: vec3<f32>,
    view_z: f32,
    frag_coord_xy: vec2<f32>,
) -> f32 {
    // grid-app shadow_layout: cascade 0 is the player car's map (lit outside
    // its box), the others the world's by view depth.
    let car = grid_cascade(light_id, 0u, frag_position, surface_normal, frag_coord_xy);
    let world = grid_world_shadow(light_id, frag_position, surface_normal, view_z, frag_coord_xy);
    return min(car, world);
}

// 0: Bevy's filter; the original's with csm_smooth: 1 csm_wedge_shadow_1
// (high), 2 csm_wedge_shadow_0 (medium: one centre tap per world cascade).
const GRID_FILTER: u32 = GRID_FILTER_VALUEu;
// Scale of the original's disc radii (1: the original), and of the car's.
const GRID_SOFT: f32 = GRID_SOFT_VALUE;
const GRID_CAR_SOFT: f32 = GRID_CAR_SOFT_VALUE;
// World disc radii in metres, cascades 1 and 2: block[2] - block[1] of the
// mode-0 table (1.0 - 0.5, 2.0 - 0.6). discScales is that / range / 2, a
// share of a map 2 range wide [I].
const GRID_WORLD_RADIUS = array<f32, 2>(0.5, 1.4);
// Car map disc radius as a share of the map: (3.75 - 0.6) / 250 / 2.
const GRID_CAR_RADIUS: f32 = 0.0063;
// csm_wedge_shadow_1 taps: the car's 8 (centre, shader constants c0.zw and
// c4-c6), the world cascades' 4 (shadowSamplePos0 and 1, 0xDAFC38).
const GRID_CAR_TAPS = array<vec2<f32>, 7>(
    vec2(0.613704979, 0.441968977),
    vec2(-0.576174974, 0.495998979), vec2(-0.490496039, -0.850394011),
    vec2(0.196620971, -1.03545702), vec2(1.07985198, -0.198945999),
    vec2(-1.10642195, -0.407754004), vec2(-0.111259997, 1.26049805),
);
const GRID_WORLD_TAPS = array<vec2<f32>, 4>(
    vec2(-0.0504099987, 0.0756499991), vec2(-0.27166, -0.0241299998),
    vec2(0.356770009, -0.281650007), vec2(-0.50959003, -0.381150007),
);
// Kernel angle step per mask pixel: rotationScale (0.178 pi, 0.3 pi times
// the mask size, 0x5CF4A1) over a clip span of 2 per mask width and height.
const GRID_ANGLE_STEP = vec2<f32>(1.11840698, -1.88495559);

// One mask pixel of csm_wedge_shadow_1: the taps scaled per axis by the
// pixel's angle, averaged. The original compares point samples; here each
// tap is Bevy's 2x2 hardware comparison.
fn grid_kernel(car: bool, uv: vec2<f32>, depth: f32, array_index: i32, angle: f32, radius: f32) -> f32 {
    let s = sin(angle);
    let c = cos(angle);
    var lit = 0.0;
    if (car) {
        let scale = radius * vec2(s, c);
        lit = sample_shadow_map_hardware(uv, depth, array_index);
        for (var i = 0; i < 7; i++) {
            lit += sample_shadow_map_hardware(uv + scale * GRID_CAR_TAPS[i], depth, array_index);
        }
        return lit * 0.125;
    }
    let scale = radius * vec2(s + c, c - s);
    for (var i = 0; i < 4; i++) {
        lit += sample_shadow_map_hardware(uv + scale * GRID_WORLD_TAPS[i], depth, array_index);
    }
    return lit * 0.25;
}

// A cascade through the layout's filter. The original's csm_smooth averages
// four bilinear taps of the mask at half-pixel offsets: a 1-2-1 tent over the
// 3x3 neighbours, whose kernels differ by their angle.
fn grid_cascade(
    light_id: u32,
    cascade_index: u32,
    frag_position: vec4<f32>,
    surface_normal: vec3<f32>,
    frag_coord_xy: vec2<f32>,
) -> f32 {
    if (GRID_FILTER == 0u) {
        return sample_directional_cascade(light_id, cascade_index, frag_position, surface_normal, frag_coord_xy);
    }
    let light = &view_bindings::lights.directional_lights[light_id];
    let cascade = &(*light).cascades[cascade_index];
    let normal_offset = (*light).shadow_normal_bias * (*cascade).texel_size * surface_normal.xyz;
    let depth_offset = (*light).shadow_depth_bias * (*light).direction_to_light.xyz;
    let offset_position = vec4<f32>(frag_position.xyz + normal_offset + depth_offset, frag_position.w);
    let light_local = world_to_directional_light_local(light_id, cascade_index, offset_position);
    if (light_local.w == 0.0) {
        return 1.0;
    }
    let array_index = i32((*light).depth_texture_base_index + cascade_index);
    let car = cascade_index == 0u;
    if (!car && GRID_FILTER == 2u) {
        // The smooth of nine equal taps leaves the tap.
        return sample_shadow_map_hardware(light_local.xy, light_local.z, array_index);
    }
    var radius = GRID_CAR_RADIUS * GRID_CAR_SOFT;
    if (!car) {
        let map_size = f32(textureDimensions(view_bindings::directional_shadow_textures).x);
        radius = GRID_WORLD_RADIUS[min(cascade_index, 2u) - 1u] / ((*cascade).texel_size * map_size);
    }
    radius *= GRID_SOFT;
    let angle = dot(floor(frag_coord_xy), GRID_ANGLE_STEP);
    var lit = 0.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let weight = f32((2 - abs(i)) * (2 - abs(j)));
            let step = angle + dot(vec2(f32(i), f32(j)), GRID_ANGLE_STEP);
            lit += weight * grid_kernel(car, light_local.xy, light_local.z, array_index, step, radius);
        }
    }
    return lit / 16.0;
}

fn grid_world_shadow(
    light_id: u32,
    frag_position: vec4<f32>,
    surface_normal: vec3<f32>,
    view_z: f32,
    frag_coord_xy: vec2<f32>,
) -> f32 {
    let light = &view_bindings::lights.directional_lights[light_id];
    let cascade_index = max(get_cascade_index(light_id, view_z), 1u);
    if (cascade_index >= (*light).num_cascades) {
        return 1.0;
    }
    var shadow = grid_cascade(light_id, cascade_index, frag_position, surface_normal, frag_coord_xy);
    let next_cascade_index = cascade_index + 1u;
    if (next_cascade_index < (*light).num_cascades) {
        // Bevy's overlap blend (the original's shadowFade1).
        let this_far_bound = (*light).cascades[cascade_index].far_bound;
        let next_near_bound = (1.0 - (*light).cascades_overlap_proportion) * this_far_bound;
        if (-view_z >= next_near_bound) {
            let next_shadow = grid_cascade(light_id, next_cascade_index, frag_position, surface_normal, frag_coord_xy);
            shadow = mix(shadow, next_shadow, (-view_z - next_near_bound) / (this_far_bound - next_near_bound));
        }
        return shadow;
    }
    // The original's shadowFade: the last cascade fades out from 30 % of
    // its range, in squared distance.
    let far_bound = (*light).cascades[cascade_index].far_bound;
    let near_bound = (1.0 - (*light).cascades_overlap_proportion) * (*light).cascades[cascade_index - 1u].far_bound;
    let start = near_bound + 0.3 * (far_bound - near_bound);
    let fade = saturate((view_z * view_z - start * start) / (far_bound * far_bound - start * start));
    return mix(shadow, 1.0, fade);
}

// Bevy's body, unused.
fn grid_bevy_fetch(
    light_id: u32,
    frag_position: vec4<f32>,
    surface_normal: vec3<f32>,
    view_z: f32,
    frag_coord_xy: vec2<f32>,
) -> f32 {
    let light = &view_bindings::lights.directional_lights[light_id];
    let cascade_index = get_cascade_index(light_id, view_z);

    if (cascade_index >= (*light).num_cascades) {
        return 1.0;
    }
";

const BEVY_IMPORT: &str = "sample_shadow_map, sample_shadow_map_pcss,\n";
const LAYOUT_IMPORT: &str = "sample_shadow_map, sample_shadow_map_pcss, sample_shadow_map_hardware,\n";

/// The mask filter: the shader group `csm_wedge_shadow` runs pass
/// `maskQuality` (shadowMaskQuality0..2, 0x72ECD0): medium (1) the `_0`
/// program, one centre tap per world cascade; high (2) `_1`, the 4-tap disc.
/// Low and ultra take high's. `GRID_SHADOW_FILTER=bevy` keeps Bevy's
/// filter, `GRID_SHADOW_SOFT` scales the disc radii and
/// `GRID_SHADOW_CAR_SOFT` the car's again [research].
fn filter(quality: Quality) -> (u32, f32, f32) {
    let bevy = std::env::var("GRID_SHADOW_FILTER").is_ok_and(|v| v == "bevy");
    let scale = |name| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(1.0_f32);
    let mode = match (bevy, quality) {
        (true, _) => 0,
        (false, Quality::Medium) => 2,
        (false, _) => 1,
    };
    (mode, scale("GRID_SHADOW_SOFT"), scale("GRID_SHADOW_CAR_SOFT"))
}

/// The `[video] shadows` quality the layout runs with.
#[derive(Resource)]
struct LayoutQuality(Quality);

/// Bevy's `fetch_directional_shadow` with the layout's head, or why not.
fn patch(source: &str, (mode, soft, car_soft): (u32, f32, f32)) -> Result<String, &'static str> {
    if source.matches(BEVY_FETCH).count() != 1 {
        return Err("pinned Bevy fetch_directional_shadow absent or ambiguous");
    }
    if source.matches(BEVY_IMPORT).count() != 1 {
        return Err("pinned Bevy shadow_sampling import absent or ambiguous");
    }
    let fetch = LAYOUT_FETCH
        .replace("GRID_FILTER_VALUE", &mode.to_string())
        .replace("GRID_SOFT_VALUE", &format!("{soft:?}"))
        .replace("GRID_CAR_SOFT_VALUE", &format!("{car_soft:?}"));
    Ok(source.replacen(BEVY_IMPORT, LAYOUT_IMPORT, 1).replacen(BEVY_FETCH, &fetch, 1))
}

fn patch_shader(mut shaders: ResMut<Assets<Shader>>, quality: Res<LayoutQuality>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let targets: Vec<_> = shaders
        .iter()
        .filter_map(|(id, shader)| {
            matches!(&shader.import_path, ShaderImport::Custom(path) if path == "bevy_pbr::shadows")
                .then_some(id)
        })
        .collect();
    if targets.is_empty() {
        return;
    }
    *done = true;
    if targets.len() != 1 {
        eprintln!("shadow_layout patched=false reason=ambiguous-shader-library");
        return;
    }
    let mut shader = shaders.get_mut(targets[0]).expect("selected shader exists");
    let Source::Wgsl(source) = &shader.source else {
        eprintln!("shadow_layout patched=false reason=not-wgsl");
        return;
    };
    let (mode, soft, car_soft) = filter(quality.0);
    match patch(source, (mode, soft, car_soft)) {
        Ok(source) => {
            shader.source = Source::Wgsl(source.into());
            println!("shadow_layout patched=true bevy=0.19.1 maps=3 car_map=cascade0 filter={mode} soft={soft} car_soft={car_soft}");
        }
        Err(error) => eprintln!("shadow_layout patched=false reason={error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_reads_the_original_options() {
        assert_eq!(Quality::parse("High"), Ok(Quality::High));
        assert_eq!(Quality::default().map_size(), Some(2048));
        assert_eq!(Quality::default_for(false), Quality::High);
        assert_eq!(Quality::default_for(true).map_size(), Some(512));
        assert_eq!(Quality::parse("medium").unwrap().map_size(), Some(1024));
        assert_eq!(Quality::parse("low").unwrap().map_size(), Some(512));
        assert_eq!(Quality::parse("ultra").unwrap().map_size(), Some(4096));
        assert_eq!(Quality::parse("off").unwrap().map_size(), None);
        assert!(Quality::parse("epic").is_err());
    }

    #[test]
    fn world_cascades_start_at_the_near_distance() {
        let config = config(252.);
        assert_eq!(config.bounds.len(), 3);
        // Bevy's near bound of cascade 1 is (1 - overlap) * bound 0.
        assert!(((1. - config.overlap_proportion) * config.bounds[0] - NEAR).abs() < 1e-6);
        assert!(((1. - config.overlap_proportion) * config.bounds[1] - 60.8).abs() < 1e-4);
        assert_eq!(config.bounds[2], 252.);
    }

    #[test]
    fn the_patch_replaces_one_exact_head() {
        let source = format!("{BEVY_IMPORT}before\n{BEVY_FETCH}    var shadow = 1.0;\n    return shadow;\n}}\nafter");
        let patched = patch(&source, (1, 1., 1.)).unwrap();
        assert!(patched.contains("fn grid_world_shadow("));
        assert!(patched.contains("return min(car, world);"));
        assert!(patched.contains(LAYOUT_IMPORT));
        assert!(!patched.contains("_VALUE"));
        assert!(patch(&patched, (1, 1., 1.)).is_err());
        assert!(patch(&format!("{BEVY_IMPORT}{BEVY_FETCH}{BEVY_FETCH}"), (1, 1., 1.)).is_err());
        assert!(patch(BEVY_FETCH, (1, 1., 1.)).is_err());
    }

    #[test]
    fn car_cascade_holds_the_car_and_the_ground_below() {
        // Light straight down (light space -Z = world -Y).
        let world_from_light = Mat4::from_quat(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
        let car: Vec<Vec3A> = [-1., 1.]
            .into_iter()
            .flat_map(|x| [0., 1.4].map(move |y| (x, y)))
            .flat_map(|(x, y)| [-2.3, 2.3].map(move |z| Vec3A::new(x + 100., y, z - 40.)))
            .collect();
        let cascade = car_cascade(&car, world_from_light, 2048.);
        let clip = |p: Vec3A| cascade.clip_from_world.project_point3(p.into());
        for p in &car {
            let c = clip(*p);
            assert!(c.x.abs() <= 1. && c.y.abs() <= 1. && (0. ..=1.).contains(&c.z), "{p} -> {c}");
        }
        // The road under the car is inside, a point 60 m below is not.
        let road = clip(Vec3A::new(100., -0.1, -40.));
        assert!((0. ..=1.).contains(&road.z));
        assert!(clip(Vec3A::new(100., -60., -40.)).z < 0.);
        // Whole metres: 4.6 m + margins -> 6 m.
        assert_eq!(cascade.texel_size, 6. / 2048.);
    }
}
