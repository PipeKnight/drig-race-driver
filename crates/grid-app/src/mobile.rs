//! Mobile preset (AUTHORED, P12, docs/PERFORMANCE.md "Mobile preset"): the
//! Android default. It is off on the desktop unless `GRID_MOBILE_PRESET=1`
//! is set (for checks on the PC). It changes only:
//! - the render resolution: `[video] render_height = auto` by default (F559):
//!   the 3D view renders between [`DYNAMIC_FLOOR_ROWS`] and [`RENDER_ROWS`]
//!   rows, lower when frames run late (`render_scale::Dynamic`), and is
//!   stretched over the screen. A row count is a fixed height, `native`
//!   turns the scaling off. Heights saved before F559 are read as `auto`
//!   (`settings::RENDER_HEIGHT_VERSION`);
//! - the compute threads ([`init_threads`]), which do not change the picture.
//!
//! F549 also cut the shadows, small-prop shadows and draw distance and the
//! grass distance. The owner (2026-10-22, after testing 9ac27f9a958a) found the
//! picture too poor and wants resolution as the only cut, so F555 removed
//! them. The helpers [`small`] and [`far_range`] remain for the Performance
//! test's `no_small_casters` and `far_props` switches.
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use std::sync::OnceLock;

pub const RENDER_ROWS: u32 = 720;
/// Lowest dynamic render height (owner, 2026-10-22: 480 to 720 rows).
pub const DYNAMIC_FLOOR_ROWS: u32 = 480;
/// Draw distance of small meshes with the Performance test's `far_props`.
pub const FAR_PROPS_METRES: f32 = 110.;
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

pub fn install(_app: &mut App) {
    if !enabled() {
        return;
    }
    log_line!("mobile_preset version=2 render_rows={RENDER_ROWS} other_cuts=none");
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
