//! Mobile preset (AUTHORED, P12, docs/PERFORMANCE.md "Mobile preset"): the
//! Android default. It is off on the desktop unless `GRID_MOBILE_PRESET=1`
//! is set (for checks on the PC). It changes only:
//! - the render resolution: `[video] render_height = auto` by default (F559):
//!   the 3D view renders between [`DYNAMIC_FLOOR_ROWS`] and [`RENDER_ROWS`]
//!   rows, lower when frames run late (`render_scale::Dynamic`), and is
//!   stretched over the screen. A row count is a fixed height, `native`
//!   turns the scaling off. Heights saved before F559 are read as `auto`
//!   (`settings::RENDER_HEIGHT_VERSION`);
//! - the compute threads ([`init_threads`]), which do not change the picture,
//!   and the parallel finish of the render encoders on them (F965);
//! - the default sun shadow quality: low 512 instead of high 2048 (owner,
//!   2026-10-31; a saved `[video] shadows` wins).
//!   `shadow_layout::Quality::default_for`;
//! - the render profile `lean` (owner, 2026-11-25: the phone may show less
//!   detail and a worse picture; F928): no MSAA, a 256 px sun shadow map
//!   within 25 m with a 2x2 filter, small props cast no shadows and end at
//!   [`FAR_PROPS_METRES`], no car reflection overlays, no point or spot
//!   lights. It is the Performance test's base (`perf_test::base_for`); the
//!   setting word `full` (`[diagnostics] perf_test = full`) turns it off.
//!   Car parts, damage and detachment are untouched.
//!
//! F549 also cut the shadows, small-prop shadows and draw distance and the
//! grass distance. The owner (2026-10-22, after testing 9ac27f9a958a) found the
//! picture too poor and wants resolution as the only cut, so F555 removed
//! them; on 2026-11-25 the owner lifted that rule for the phone, and the
//! `lean` profile above brought the cuts back, measured on the phone (F927).
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
    *ENABLED.get_or_init(|| match crate::dev::var("GRID_MOBILE_PRESET").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => cfg!(target_os = "android"),
    })
}

/// Static driver figures (owner, 2026-11-26, F943): when on, the
/// drivers hold one pose and only the player's arms and hands in the head-cam
/// move. The player's driver outside the head-cam is baked into static
/// meshes like the opponents' (`exterior_driver`), and the head-cam driver
/// keeps joint entities only for its arms (`driver::Driver::anchors`), so the
/// still joints no longer cost a `GlobalTransform` each frame. Car parts,
/// damage and detachment are untouched. Off by default on every platform
/// until the phone shows a gain (coordinator, 2026-11-26: the body no longer
/// leans, which the owner can see, and the PC proxy frame change was within
/// noise); `GRID_STATIC_DRIVERS=1` turns it on.
pub fn static_drivers() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| crate::dev::var("GRID_STATIC_DRIVERS").is_ok_and(|v| v == "1"))
}

/// Static mesh (not a car part) with a bounding radius under
/// [`SMALL_RADIUS_METRES`].
pub fn small(aabb: &Aabb, transform: &GlobalTransform) -> bool {
    radius(aabb, transform) < SMALL_RADIUS_METRES
}

/// A mesh's bounding radius in metres.
pub fn radius(aabb: &Aabb, transform: &GlobalTransform) -> f32 {
    let scale = transform.compute_transform().scale.abs().max_element();
    Vec3::from(aabb.half_extents).length() * scale
}

/// The size cull (`cullNNN`) never hides a mesh nearer than this.
pub const SIZE_CULL_MIN_METRES: f32 = 40.;

/// The draw range of a static mesh: `far_props` for a small one
/// ([`far_range`]) and the size cull (`factor` times the bounding radius,
/// measured from the box centre so meshes built in world space cull by
/// their own place); the nearer of the two wins. `None` = no range.
pub fn static_range(far_prop: bool, factor: u16, radius: Option<f32>) -> Option<VisibilityRange> {
    let props = far_prop.then_some(FAR_PROPS_METRES);
    let size = (factor > 0)
        .then_some(radius)
        .flatten()
        .map(|r| (r * f32::from(factor)).max(SIZE_CULL_MIN_METRES));
    match (props, size) {
        (Some(p), Some(s)) if s < p => Some(size_range(s)),
        (Some(p), _) => Some(far_range(p)),
        (None, Some(s)) => Some(size_range(s)),
        (None, None) => None,
    }
}

/// The size cull's range: a 10 % fade, from the box centre.
pub fn size_range(metres: f32) -> VisibilityRange {
    VisibilityRange { start_margin: 0.0..0.0, end_margin: metres * 0.9..metres, use_aabb: true }
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
    let requested = crate::dev::var("GRID_COMPUTE_SPLIT").is_ok_and(|v| v == "preset");
    let split = enabled() || requested;
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let mut threads = pool_threads(cores, split);
    // `GRID_COMPUTE_THREADS=N` sets the count outright (F860 measurement).
    let forced = crate::dev::var("GRID_COMPUTE_THREADS").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n >= 1);
    if let Some(n) = forced {
        threads = n;
    }
    init_compute_pool(threads);
    if split || forced.is_some() || threads != bevy_compute_threads(cores) {
        log_line!("mobile_threads version=1 cores={cores} compute={threads}");
    }
    // The preset finishes the render systems' command encoders together on
    // the compute pool (phone pair 6, F965: render thread 25.8 -> 21.6 ms,
    // FPS median 35 -> 40, docs/ANDROID_CPU.md 5);
    // `GRID_BEVY_PARALLEL_FINISH=0` turns it off, `=1` on the desktop.
    bevy::render::renderer::set_parallel_finish_default(enabled());
    if enabled() {
        let on = bevy::render::renderer::parallel_finish_enabled();
        log_line!("parallel_finish version=1 on={on}");
    }
}

fn init_compute_pool(threads: usize) {
    bevy::tasks::ComputeTaskPool::get_or_init(|| {
        bevy::tasks::TaskPoolBuilder::new()
            .num_threads(threads)
            .thread_name("Compute Task Pool".into())
            .stack_size(COMPUTE_STACK)
            // Thread ids for the CPU placement switches (cpu_place.rs).
            .on_thread_spawn(crate::cpu_place::note_pool_thread)
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
    // The other cuts are the `lean` render profile (perf_test::base_for).
    log_line!(
        "mobile_preset version=3 render_rows={RENDER_ROWS} render_profile=lean ({})",
        crate::perf_test::Switches::LEAN.label()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cullNNN` ranges every static mesh by its size, from 40 m up; the
    /// nearer of it and `far_props` wins; nothing without a switch.
    #[test]
    fn static_range_takes_the_nearer_cull() {
        assert!(static_range(false, 0, Some(3.)).is_none());
        assert!(static_range(true, 0, Some(3.)) == Some(far_range(FAR_PROPS_METRES)));
        // 3 m x 100 = 300 m: far_props (110 m) is nearer.
        assert!(static_range(true, 100, Some(3.)) == Some(far_range(FAR_PROPS_METRES)));
        // 0.2 m x 100 = 20 m, raised to 40 m: nearer than far_props.
        assert!(static_range(true, 100, Some(0.2)) == Some(size_range(SIZE_CULL_MIN_METRES)));
        let big = static_range(false, 100, Some(12.)).unwrap();
        assert_eq!(big.end_margin, 1080.0..1200.0);
        assert!(big.use_aabb);
        assert!(static_range(false, 100, None).is_none());
    }

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
        // A thread inside the pool's `scope` (another test's App update, on
        // a libtest or an unnamed big-stack thread) may also run a spawned
        // task (F965: one full gate saw ""), so the check takes several
        // tasks: none may run on a default-pool thread, and one must reach
        // the large-stack pool's own workers.
        let pool = bevy::tasks::ComputeTaskPool::get();
        let names: Vec<String> = (0..16)
            .map(|_| bevy::tasks::block_on(pool.spawn(async { std::thread::current().name().unwrap_or_default().to_owned() })))
            .collect();
        assert!(!names.iter().any(|n| n.starts_with("TaskPool (")), "compute tasks ran on {names:?}");
        assert!(names.iter().any(|n| n.starts_with("Compute Task Pool (")), "compute tasks ran on {names:?}");
        assert_eq!(pool.thread_num(), std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get));
    }

    #[test]
    fn small_uses_the_scaled_radius() {
        let aabb = Aabb::from_min_max(Vec3::splat(-1.), Vec3::splat(1.));
        assert!(small(&aabb, &GlobalTransform::IDENTITY));
        assert!(!small(&aabb, &GlobalTransform::from_scale(Vec3::splat(3.))));
    }
}
