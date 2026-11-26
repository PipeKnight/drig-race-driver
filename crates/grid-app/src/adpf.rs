//! ADPF performance hints (AUTHORED, docs/ANDROID_GPU.md section 10): with
//! the `perf_test` word `adpf` the game opens an Android performance hint
//! session for its threads and reports every frame's work against the 60 Hz
//! budget, so the governors see a missed frame instead of guessing from load.
//! From API 35 (`APerformanceHint_reportActualWorkDuration2`) the report also
//! carries the GPU time of the newest frame `frame_split` read back, which the
//! vendor's power HAL may use for the GPU clock.
//!
//! The hypothesis (F927 [I]): about 22 GPU ms stay with almost nothing drawn,
//! so the Adreno clock may fall under the vsync-paced load. The bench's GPU
//! clock samples (tools/android_bench.ps1) show whether this switch moves it.
//!
//! Only hints change: nothing is drawn differently. Every symbol is looked up
//! at run time (`dlsym`), so a phone below API 33 logs `unsupported` and runs
//! as before. Other platforms add no systems.
use bevy::prelude::*;

/// Threads that ran the game's schedules (main, render, task pools),
/// collected over the first [`COLLECT_FRAMES`] frames.
#[cfg(target_os = "android")]
static TIDS: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());
#[cfg(target_os = "android")]
static SEEN_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
#[cfg(target_os = "android")]
const COLLECT_FRAMES: u32 = 120;
/// The session's thread list (ADPF limits it; a few busy threads matter).
#[cfg(target_os = "android")]
const MAX_TIDS: usize = 16;
/// The frame budget: the game's 60 Hz cap (F928).
pub const TARGET_NS: i64 = 16_666_667;
/// A `report` log line every this many frames while the session is open.
#[cfg(target_os = "android")]
const REPORT_EVERY: u32 = 600;

/// One frame's work for the hint: the frame interval (main thread, First to
/// First), the main thread's own work (First to Last) and the newest GPU span
/// (0 = none). Durations below 1 ns are invalid for ADPF and are raised to 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Work {
    pub total_ns: i64,
    pub cpu_ns: i64,
    pub gpu_ns: i64,
}

impl Work {
    pub fn new(interval_ns: u64, main_ns: u64, gpu_ns: u64) -> Self {
        let clamp = |v: u64| i64::try_from(v).unwrap_or(i64::MAX).max(1);
        let total = clamp(interval_ns);
        Self { total_ns: total, cpu_ns: clamp(main_ns).min(total), gpu_ns: i64::try_from(gpu_ns).unwrap_or(0) }
    }
}

pub fn install(app: &mut App) {
    #[cfg(target_os = "android")]
    {
        app.init_resource::<imp::State>()
            .add_systems(First, imp::frame_begin)
            .add_systems(Update, imp::note_pool_thread)
            .add_systems(Last, imp::report);
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.add_systems(
                bevy::render::Render,
                (
                    imp::note_render_thread.in_set(bevy::render::RenderSystems::Cleanup),
                    imp::note_pool_thread.in_set(bevy::render::RenderSystems::Queue),
                    imp::note_pool_thread.in_set(bevy::render::RenderSystems::Prepare),
                ),
            );
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
}

#[cfg(target_os = "android")]
mod imp {
    use super::*;
    use std::ffi::{CStr, c_void};
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    type GetManager = unsafe extern "C" fn() -> *mut c_void;
    type CreateSession = unsafe extern "C" fn(*mut c_void, *const i32, usize, i64) -> *mut c_void;
    type Report = unsafe extern "C" fn(*mut c_void, i64) -> i32;
    type Close = unsafe extern "C" fn(*mut c_void);
    type DurationCreate = unsafe extern "C" fn() -> *mut c_void;
    type DurationSet = unsafe extern "C" fn(*mut c_void, i64);
    type Report2 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;

    /// The API 35 calls: a work duration object and its setters.
    struct Api35 {
        report2: Report2,
        duration: usize,
        start: DurationSet,
        total: DurationSet,
        cpu: DurationSet,
        gpu: DurationSet,
    }

    struct Api {
        manager: usize,
        create: CreateSession,
        report: Report,
        close: Close,
        api35: Option<Api35>,
    }

    fn symbol(lib: *mut c_void, name: &CStr) -> Option<*mut c_void> {
        // SAFETY: `lib` is a live dlopen handle and `name` is NUL-terminated.
        let p = unsafe { libc::dlsym(lib, name.as_ptr()) };
        (!p.is_null()).then_some(p)
    }

    fn load() -> Option<Api> {
        // SAFETY: dlopen of a system library by name; the handle is never closed.
        let lib = unsafe { libc::dlopen(c"libandroid.so".as_ptr(), libc::RTLD_NOW) };
        if lib.is_null() {
            return None;
        }
        // SAFETY (all transmutes): the symbols have the NDK's
        // `android/performance_hint.h` signatures written in the types above.
        unsafe {
            let get: GetManager = std::mem::transmute(symbol(lib, c"APerformanceHint_getManager")?);
            let create: CreateSession = std::mem::transmute(symbol(lib, c"APerformanceHint_createSession")?);
            let report: Report = std::mem::transmute(symbol(lib, c"APerformanceHint_reportActualWorkDuration")?);
            let close: Close = std::mem::transmute(symbol(lib, c"APerformanceHint_closeSession")?);
            let manager = get();
            if manager.is_null() {
                return None;
            }
            let api35 = (|| {
                let make: DurationCreate = std::mem::transmute(symbol(lib, c"AWorkDuration_create")?);
                let duration = make();
                if duration.is_null() {
                    return None;
                }
                Some(Api35 {
                    report2: std::mem::transmute::<*mut c_void, Report2>(symbol(
                        lib,
                        c"APerformanceHint_reportActualWorkDuration2",
                    )?),
                    duration: duration as usize,
                    start: std::mem::transmute::<*mut c_void, DurationSet>(symbol(
                        lib,
                        c"AWorkDuration_setWorkPeriodStartTimestampNanos",
                    )?),
                    total: std::mem::transmute::<*mut c_void, DurationSet>(symbol(
                        lib,
                        c"AWorkDuration_setActualTotalDurationNanos",
                    )?),
                    cpu: std::mem::transmute::<*mut c_void, DurationSet>(symbol(
                        lib,
                        c"AWorkDuration_setActualCpuDurationNanos",
                    )?),
                    gpu: std::mem::transmute::<*mut c_void, DurationSet>(symbol(
                        lib,
                        c"AWorkDuration_setActualGpuDurationNanos",
                    )?),
                })
            })();
            Some(Api { manager: manager as usize, create, report, close, api35 })
        }
    }

    #[derive(Resource, Default)]
    pub struct State {
        api: Option<Api>,
        loaded: bool,
        /// The open session's pointer.
        session: Option<usize>,
        begin: Option<Instant>,
        begin_mono_ns: i64,
        previous: Option<Instant>,
        frames: u32,
        failed: u32,
        first_error: Option<i32>,
        logged_waiting: bool,
    }

    fn note(tid: i32) {
        let mut tids = TIDS.lock().unwrap_or_else(|e| e.into_inner());
        if tids.len() < MAX_TIDS && !tids.contains(&tid) {
            tids.push(tid);
        }
    }

    fn collecting() -> bool {
        SEEN_FRAMES.load(Ordering::Relaxed) < COLLECT_FRAMES
    }

    fn monotonic_ns() -> i64 {
        let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: a valid timespec out-pointer.
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) };
        t.tv_sec * 1_000_000_000 + t.tv_nsec
    }

    /// Exclusive, so it runs on the main thread itself.
    pub fn frame_begin(world: &mut World) {
        if collecting() {
            // SAFETY: gettid has no preconditions.
            note(unsafe { libc::gettid() });
            SEEN_FRAMES.fetch_add(1, Ordering::Relaxed);
        }
        let mut state = world.resource_mut::<State>();
        state.previous = state.begin;
        state.begin = Some(Instant::now());
        state.begin_mono_ns = monotonic_ns();
    }

    /// Exclusive, so it runs on the render thread itself.
    pub fn note_render_thread(_world: &mut World) {
        if collecting() {
            // SAFETY: gettid has no preconditions.
            note(unsafe { libc::gettid() });
        }
    }

    /// A task-pool thread that runs systems.
    pub fn note_pool_thread() {
        if collecting() {
            // SAFETY: gettid has no preconditions.
            note(unsafe { libc::gettid() });
        }
    }

    pub fn report(mut state: ResMut<State>, test: Option<Res<crate::perf_test::PerfTest>>) {
        let on = test.is_some_and(|t| t.active().adpf);
        let s = &mut *state;
        if !on {
            if let (Some(session), Some(api)) = (s.session.take(), s.api.as_ref()) {
                // SAFETY: a session this module opened and has not closed.
                unsafe { (api.close)(session as *mut c_void) };
                log_line!("adpf version=1 event=close frames={} failed={}", s.frames, s.failed);
            }
            return;
        }
        if collecting() {
            if !s.logged_waiting {
                s.logged_waiting = true;
                log_line!("adpf version=1 event=collecting frames={COLLECT_FRAMES}");
            }
            return;
        }
        if !s.loaded {
            s.loaded = true;
            s.api = load();
            if s.api.is_none() {
                log_line!("adpf version=1 event=open result=unsupported");
            }
        }
        let Some(api) = s.api.as_ref() else {
            return;
        };
        if s.session.is_none() {
            let tids = TIDS.lock().unwrap_or_else(|e| e.into_inner()).clone();
            // SAFETY: the manager from getManager, a valid tid slice.
            let session = unsafe { (api.create)(api.manager as *mut c_void, tids.as_ptr(), tids.len(), TARGET_NS) };
            if session.is_null() {
                log_line!("adpf version=1 event=open result=error tids={}", tids.len());
                s.api = None;
                return;
            }
            s.session = Some(session as usize);
            s.frames = 0;
            s.failed = 0;
            s.first_error = None;
            log_line!(
                "adpf version=1 event=open result=ok tids={} gpu_report={} target_ms={:.2}",
                tids.len(),
                api.api35.is_some(),
                TARGET_NS as f64 * 1e-6
            );
        }
        let (Some(begin), Some(previous), Some(session)) = (s.begin, s.previous, s.session) else {
            return;
        };
        let now = Instant::now();
        let work = Work::new(
            begin.duration_since(previous).as_nanos() as u64,
            now.duration_since(begin).as_nanos() as u64,
            crate::frame_split::last_gpu_ns(),
        );
        let session = session as *mut c_void;
        // SAFETY: an open session and, for API 35, the duration object load() made.
        let result = unsafe {
            match &api.api35 {
                Some(a) => {
                    let d = a.duration as *mut c_void;
                    (a.start)(d, s.begin_mono_ns.max(1));
                    (a.total)(d, work.total_ns);
                    (a.cpu)(d, work.cpu_ns);
                    (a.gpu)(d, work.gpu_ns);
                    (a.report2)(session, d)
                }
                None => (api.report)(session, work.total_ns),
            }
        };
        s.frames += 1;
        if result != 0 {
            s.failed += 1;
            s.first_error.get_or_insert(result);
        }
        if s.frames.is_multiple_of(REPORT_EVERY) {
            log_line!(
                "adpf version=1 event=report frames={} failed={} first_error={:?} total_ms={:.2} cpu_ms={:.2} gpu_ms={:.2}",
                s.frames,
                s.failed,
                s.first_error,
                work.total_ns as f64 * 1e-6,
                work.cpu_ns as f64 * 1e-6,
                work.gpu_ns as f64 * 1e-6
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_is_valid_for_adpf() {
        assert_eq!(Work::new(26_000_000, 9_000_000, 21_000_000), Work { total_ns: 26_000_000, cpu_ns: 9_000_000, gpu_ns: 21_000_000 });
        // Zero is invalid for the totals and the CPU part; the CPU part never exceeds the total.
        assert_eq!(Work::new(0, 0, 0), Work { total_ns: 1, cpu_ns: 1, gpu_ns: 0 });
        assert_eq!(Work::new(5, 9, 0).cpu_ns, 5);
    }
}
