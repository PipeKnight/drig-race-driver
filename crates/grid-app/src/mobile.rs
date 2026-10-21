//! Mobile preset (AUTHORED, P12, docs/PERFORMANCE.md "Mobile preset"): the
//! Android default, aiming at a steady 60 FPS on phones. The owner (2026-10-22)
//! accepts a PS3-era picture on the phone and wants the desktop picture
//! unchanged, so everything here is off on the desktop unless
//! `GRID_MOBILE_PRESET=1` is set (for checks on the PC).
//!
//! What the preset changes, in presentation only (the simulation and the race
//! results are untouched):
//! - the 3D view renders at [`RENDER_ROWS`] rows and is stretched over the
//!   screen (`render_scale`; `[video] render_height` still overrides);
//! - the sun casts one shadow cascade of a [`SHADOW_MAP`] map out to
//!   [`SHADOW_METRES`];
//! - small static meshes (props, signs, billboards) cast no shadow and are not
//!   drawn beyond [`FAR_PROPS_METRES`];
//! - ground cover is drawn to [`GROUND_COVER_SCALE`] of its authored distances.
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::{RenderLayers, VisibilityRange};
use bevy::light::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster,
};
use bevy::prelude::*;
use std::sync::OnceLock;

pub const RENDER_ROWS: u32 = 720;
pub const SHADOW_MAP: usize = 1024;
pub const SHADOW_METRES: f32 = 45.;
pub const FAR_PROPS_METRES: f32 = 110.;
pub const GROUND_COVER_SCALE: f32 = 0.4;
/// Bounding radius in metres below which a static mesh counts as small.
pub const SMALL_RADIUS_METRES: f32 = 4.;

/// The preset is in use: on Android, or with `GRID_MOBILE_PRESET=1`
/// (`GRID_MOBILE_PRESET=0` turns it off on Android for comparisons).
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| match std::env::var("GRID_MOBILE_PRESET").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => cfg!(target_os = "android"),
    })
}

/// Static mesh (not a car part) with a bounding radius under
/// [`SMALL_RADIUS_METRES`].
pub fn small(aabb: &Aabb, transform: &GlobalTransform) -> bool {
    let scale = transform.compute_transform().scale.abs().max_element();
    Vec3::from(aabb.half_extents).length() * scale < SMALL_RADIUS_METRES
}

/// The draw-distance limit given to small meshes.
pub fn far_range(metres: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: metres - 10.0..metres,
        use_aabb: false,
    }
}

/// Compute threads with the preset: every core but two (the main and render
/// threads). Bevy's default leaves a quarter each to the IO and async pools,
/// which sit idle in a race: 4 of 8 cores on a phone (P12 phone profile).
pub fn compute_threads(cores: usize) -> usize {
    cores.saturating_sub(2).max(4).min(cores.max(1))
}

/// Creates Bevy's compute pool before the app does (its task pool plugin
/// keeps a pool that exists). Called first thing on Android.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn init_threads() {
    if !enabled() {
        return;
    }
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let threads = compute_threads(cores);
    bevy::tasks::ComputeTaskPool::get_or_init(|| {
        bevy::tasks::TaskPoolBuilder::new()
            .num_threads(threads)
            .thread_name("Compute Task Pool".into())
            .build()
    });
    log_line!("mobile_threads version=1 cores={cores} compute={threads}");
}

pub fn install(app: &mut App) {
    if !enabled() {
        return;
    }
    log_line!(
        "mobile_preset version=1 render_rows={RENDER_ROWS} shadow_map={SHADOW_MAP} cascades=1 shadow_metres={SHADOW_METRES} small_radius={SMALL_RADIUS_METRES} small_casters=off far_props_metres={FAR_PROPS_METRES} ground_cover_scale={GROUND_COVER_SCALE}"
    );
    app.insert_resource(DirectionalLightShadowMap { size: SHADOW_MAP })
        .add_systems(PostStartup, shadows)
        .add_systems(Last, small_meshes);
}

/// One cascade: set before the first frame (changing the cascade count of a
/// light that has rendered is not safe in Bevy 0.19.1).
fn shadows(mut lights: Query<&mut CascadeShadowConfig, With<DirectionalLight>>) {
    for mut config in &mut lights {
        *config = CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 0.1,
            maximum_distance: SHADOW_METRES,
            first_cascade_far_bound: SHADOW_METRES,
            overlap_proportion: 0.2,
        }
        .build();
    }
}

#[derive(Component)]
struct Checked;

/// New small static meshes of the main view stop casting shadows and get a
/// draw distance (meshes on other render layers, such as the OSD, are left).
#[allow(clippy::type_complexity)]
fn small_meshes(
    mut commands: Commands,
    meshes: Query<
        (Entity, &Aabb, &GlobalTransform, Has<crate::MovingPart>),
        (With<Mesh3d>, Without<Checked>, Without<RenderLayers>),
    >,
) {
    for (entity, aabb, transform, car) in &meshes {
        let mut e = commands.entity(entity);
        e.insert(Checked);
        if !car && small(aabb, transform) {
            e.insert((NotShadowCaster, far_range(FAR_PROPS_METRES)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_threads_leave_two_cores_but_keep_four() {
        assert_eq!(compute_threads(8), 6);
        assert_eq!(compute_threads(20), 18);
        assert_eq!(compute_threads(4), 4);
        assert_eq!(compute_threads(2), 2);
        assert_eq!(compute_threads(1), 1);
    }

    #[test]
    fn small_uses_the_scaled_radius() {
        let aabb = Aabb::from_min_max(Vec3::splat(-1.), Vec3::splat(1.));
        assert!(small(&aabb, &GlobalTransform::IDENTITY));
        assert!(!small(&aabb, &GlobalTransform::from_scale(Vec3::splat(3.))));
    }
}
