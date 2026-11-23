//! Mobile preset (AUTHORED, P12, docs/PERFORMANCE.md "Mobile preset"): the
//! Android default. It is off on the desktop unless `GRID_MOBILE_PRESET=1`
//! is set (for checks on the PC). It changes only:
//! - the render resolution: `[video] render_height = auto` by default (F559):
//!   the 3D view renders between [`DYNAMIC_FLOOR_ROWS`] and [`RENDER_ROWS`]
//!   rows, lower when frames run late (`render_scale::Dynamic`), and is
//!   stretched over the screen. A row count is a fixed height, `native`
//!   turns the scaling off. Heights saved before F559 are read as `auto`
//!   (`settings::RENDER_HEIGHT_VERSION`);
//! - the compute threads ([`init_threads`]), which do not change the picture;
//! - the default sun shadow quality: low 512 instead of high 2048 (owner,
//!   2026-10-31, the one exception to resolution-only; a saved
//!   `[video] shadows` wins). `shadow_layout::Quality::default_for`.
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

/// Stack of the compute pool's threads. The simulation's per-car stages
/// (`grid_tools::parallel::map`) wait inside a scope on this pool, and a
/// waiting thread runs other queued tasks on its own stack, so stacks nest:
/// one `drive_tick` needs 0.5–0.75 MB, and the grid-app tests overflowed
/// Bevy's default 2 MB when parallel tests' ticks nested. 64 MiB as
/// `grid_tools::parallel::pool`; a thread stack is reserved address space,
/// committed only as it is used (Windows and Android arm64 alike).
pub const COMPUTE_STACK: usize = 64 * 1024 * 1024;

/// The compute threads Bevy's task pool plugin gives `cores` cores by
/// default (`TaskPoolOptions::default`): what is left after a quarter, at
/// least 1 and at most 4, for IO and a quarter for async compute.
pub fn bevy_compute_threads(cores: usize) -> usize {
    let quarter = |remaining: usize| {
        let proportion = cores as f32 * 0.25;
        let mut desired = proportion as usize;
        if proportion - desired as f32 >= 0.5 {
            desired += 1;
        }
        desired.min(remaining).clamp(1, 4)
    };
    let mut remaining = cores;
    remaining = remaining.saturating_sub(quarter(remaining));
    remaining = remaining.saturating_sub(quarter(remaining));
    remaining.max(1)
}

/// The most compute threads on any platform (F860). The main and render
/// executors share this pool; on a 20-thread PC 8 threads gave +1.4 to
/// +2.8 fps over Bevy's 12 in every clean paired run (lower main and
/// render time), and 18 were slower. Machines with 16 or fewer logical
/// cores (phones, most Macs) keep their count.
pub const MAX_COMPUTE_THREADS: usize = 8;

/// The compute pool's thread count for `cores` logical cores: Bevy's split,
/// or [`compute_threads`] with the preset, both at most
/// [`MAX_COMPUTE_THREADS`].
pub fn pool_threads(cores: usize, preset: bool) -> usize {
    let threads = if preset { compute_threads(cores) } else { bevy_compute_threads(cores) };
    threads.min(MAX_COMPUTE_THREADS)
}

/// Creates Bevy's compute pool before the app does (its task pool plugin
/// keeps a pool that exists), with [`COMPUTE_STACK`]. Called first thing in
/// `main`. [`pool_threads`]; the preset split applies with the preset, or on
/// the desktop with `GRID_COMPUTE_SPLIT=preset` (F573 measurement).
pub fn init_threads() {
    let requested = std::env::var("GRID_COMPUTE_SPLIT").is_ok_and(|v| v == "preset");
    let split = enabled() || requested;
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let mut threads = pool_threads(cores, split);
    // `GRID_COMPUTE_THREADS=N` sets the count outright (F860 measurement).
    let forced = std::env::var("GRID_COMPUTE_THREADS").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n >= 1);
    if let Some(n) = forced {
        threads = n;
    }
    init_compute_pool(threads);
    if split || forced.is_some() || threads != bevy_compute_threads(cores) {
        log_line!("mobile_threads version=1 cores={cores} compute={threads}");
    }
}

fn init_compute_pool(threads: usize) {
    bevy::tasks::ComputeTaskPool::get_or_init(|| {
        bevy::tasks::TaskPoolBuilder::new()
            .num_threads(threads)
            .thread_name("Compute Task Pool".into())
            .stack_size(COMPUTE_STACK)
            .build()
    });
}

/// The tests' compute pool: the thread count an App's executor would
/// create (`TaskPool::default`, every core) with [`COMPUTE_STACK`].
///
/// The first pool of a process stays, and Bevy's multi-threaded executor
/// creates `TaskPool::default` (threads "TaskPool (N)", the std default
/// 2 MiB stacks) when a schedule runs before any pool exists. Calling this
/// before each test App left the race to whichever parallel test reached
/// the pool first: about 90 test Apps did not call it, and when one of them
/// won, nested `drive_tick`s overflowed a 2 MiB "TaskPool (N)" thread in
/// about two full gates in three (F632, F886). So the test binaries create
/// the pool once at process start, before libtest runs any test, through
/// [`INIT_TEST_POOL`].
#[cfg(all(test, any(windows, target_os = "linux", target_os = "android", target_vendor = "apple")))]
extern "C" fn init_test_pool() {
    // Only `available_parallelism` and the pool's own thread spawns run
    // before `main`; nothing here touches `thread::current` on this thread,
    // which the runtime sets up in `main` (it aborts if code before main
    // already created it).
    init_compute_pool(std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get));
}

/// Runs [`init_test_pool`] at process start in test binaries: the C runtime
/// initializer table on Windows, `.init_array` on Linux and Android,
/// `__mod_init_func` on Apple targets.
#[cfg(all(test, any(windows, target_os = "linux", target_os = "android", target_vendor = "apple")))]
#[used]
#[cfg_attr(windows, unsafe(link_section = ".CRT$XCU"))]
#[cfg_attr(any(target_os = "linux", target_os = "android"), unsafe(link_section = ".init_array"))]
#[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,__mod_init_func"))]
static INIT_TEST_POOL: extern "C" fn() = init_test_pool;

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
    fn bevy_compute_threads_match_the_task_pool_plugin() {
        // 25 % each for IO and async compute (1..=4), the rest compute.
        assert_eq!(bevy_compute_threads(1), 1);
        assert_eq!(bevy_compute_threads(4), 2);
        assert_eq!(bevy_compute_threads(8), 4);
        assert_eq!(bevy_compute_threads(20), 12);
        assert_eq!(bevy_compute_threads(32), 24);
    }

    #[test]
    fn pool_threads_cap_both_splits_at_eight() {
        assert_eq!(pool_threads(8, false), 4);
        assert_eq!(pool_threads(16, false), 8);
        assert_eq!(pool_threads(20, false), 8);
        assert_eq!(pool_threads(32, false), 8);
        assert_eq!(pool_threads(8, true), 6);
        assert_eq!(pool_threads(10, true), 8);
        assert_eq!(pool_threads(20, true), 8);
    }

    #[test]
    fn compute_threads_leave_two_cores_but_keep_four() {
        assert_eq!(compute_threads(8), 6);
        assert_eq!(compute_threads(20), 18);
        assert_eq!(compute_threads(4), 4);
        assert_eq!(compute_threads(2), 2);
        assert_eq!(compute_threads(1), 1);
    }

    #[test]
    fn test_apps_get_the_large_stack_compute_pool_before_any_test() {
        // `get` panics when no pool exists yet: the initializer made it
        // before libtest ran this or any other test. A task the pool's
        // workers run (block_on does not run the pool's queue here) is on a
        // thread of `init_compute_pool`'s builder, not Bevy's
        // `TaskPool::default` "TaskPool (N)" threads with 2 MiB stacks.
        let pool = bevy::tasks::ComputeTaskPool::get();
        let name = bevy::tasks::block_on(pool.spawn(async { std::thread::current().name().unwrap_or_default().to_owned() }));
        assert!(name.starts_with("Compute Task Pool ("), "compute task ran on {name:?}");
        assert_eq!(pool.thread_num(), std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get));
    }

    #[test]
    fn small_uses_the_scaled_radius() {
        let aabb = Aabb::from_min_max(Vec3::splat(-1.), Vec3::splat(1.));
        assert!(small(&aabb, &GlobalTransform::IDENTITY));
        assert!(!small(&aabb, &GlobalTransform::from_scale(Vec3::splat(3.))));
    }
}
