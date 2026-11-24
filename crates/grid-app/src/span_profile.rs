//! On-device per-system profile (grid-app feature `profile`, P12,
//! docs/PERFORMANCE.md "Phone bisection"): with Bevy's `trace` spans on, a
//! tracing layer adds up each span's inclusive and exclusive time per name,
//! and every [`window_seconds`] the app logs a compact table, so a phone
//! without adb still reports which systems take its frame:
//!
//! ```text
//! profile_window version=1 seconds=10.0 frames=301 perf_test=normal
//! profile_thread rank=1 in_spans_ms_per_frame=27.1 work_ms_per_frame=9.3 name=main
//! profile_span rank=1 excl_ms_per_frame=4.2 incl_ms_per_frame=4.2 calls_per_frame=1.0 name=system: name="bevy_render::..."
//! ```
//!
//! Slow render frames: every render-app update (`sub app: name=RenderApp`)
//! of at least [`slow_ms`] logs its spans by exclusive time, from all
//! threads while that update ran, beside the window's mean per render frame:
//!
//! ```text
//! profile_slow render_ms=24.1 rank=1 excl_ms=9.3 mean_ms=1.2 name=...
//! profile_render_mean frames=612 rank=1 excl_ms=1.2 name=...
//! ```
//!
//! Main-world spans that overlap the render update (pipelined rendering)
//! are included; their names tell them apart.
//!
//! Independently authored diagnostics; only time is read, so the simulation
//! is unaffected. Not in the normal build: span instrumentation costs time.
use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Subscriber, span};
use bevy::log::tracing_subscriber::layer::{Context, Layer};
use bevy::log::tracing_subscriber::registry::LookupSpan;
use bevy::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// Spans that only wait for their children (schedules, executors, sub-app
/// wrappers): ranked separately from the work.
const WRAPPERS: [&str; 9] = [
    "schedule",
    "multithreaded executor",
    "update",
    "sub app",
    "main app",
    "main_render_schedule",
    "camera_schedule",
    "system: name=\"bevy_render::run_render_schedule\"",
    "system: name=\"bevy_time::fixed::run_fixed_main_schedule\"",
];
const TOP_WORK: usize = 60;
const TOP_WRAPPERS: usize = 30;

/// Seconds per logged table: `GRID_PROFILE_SECONDS`, default 10.
pub fn window_seconds() -> f32 {
    static SECONDS: OnceLock<f32> = OnceLock::new();
    *SECONDS.get_or_init(|| {
        crate::dev::var("GRID_PROFILE_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&s: &f32| s >= 1.)
            .unwrap_or(10.)
    })
}

/// Spans per table: `GRID_PROFILE_TOP`, default [`TOP_WORK`] (a local
/// run can list them all).
fn top_work() -> usize {
    static TOP: OnceLock<usize> = OnceLock::new();
    *TOP.get_or_init(|| {
        crate::dev::var("GRID_PROFILE_TOP")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(TOP_WORK)
    })
}

/// Render frames at or above this many ms list their spans:
/// `GRID_PROFILE_SLOW_MS`, default 20.
fn slow_ms() -> f64 {
    static MS: OnceLock<f64> = OnceLock::new();
    *MS.get_or_init(|| crate::dev::var("GRID_PROFILE_SLOW_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(20.))
}

/// Spans listed per slow frame.
const TOP_SLOW: usize = 25;
/// Slow frames listed per window.
const MAX_SLOW: usize = 12;

/// The render app's update span (pipelined rendering's render thread).
fn render_key() -> u32 {
    static KEY: OnceLock<u32> = OnceLock::new();
    *KEY.get_or_init(|| intern("sub app: name=RenderApp".to_owned()))
}

/// A render-app update is running.
static RENDER_OPEN: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct RenderFrames {
    /// Per key: exclusive ns over the window's render frames.
    sum: HashMap<u32, u64>,
    frames: u64,
    /// Render ms and exclusive ns per key of the slow frames.
    slow: Vec<(f64, Vec<(u32, u64)>)>,
}

static RENDER: Mutex<Option<RenderFrames>> = Mutex::new(None);

/// Interned span name ("name: fields").
struct Key(u32);

#[derive(Default)]
struct ThreadStats {
    name: String,
    /// Per key: inclusive ns, exclusive ns, calls.
    spans: HashMap<u32, [u64; 3]>,
    /// Exclusive ns per key during the current render-app update.
    frame: HashMap<u32, u64>,
}

static NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static THREADS: Mutex<Vec<Arc<Mutex<ThreadStats>>>> = Mutex::new(Vec::new());

thread_local! {
    /// Open spans of this thread: span id, key, start, child time.
    static STACK: RefCell<Vec<(u64, u32, Instant, u64)>> = const { RefCell::new(Vec::new()) };
    static LOCAL: Arc<Mutex<ThreadStats>> = {
        let stats = Arc::new(Mutex::new(ThreadStats {
            name: std::thread::current().name().unwrap_or("unnamed").to_owned(),
            ..Default::default()
        }));
        THREADS.lock().unwrap_or_else(|e| e.into_inner()).push(stats.clone());
        stats
    };
}

fn intern(name: String) -> u32 {
    static INDEX: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();
    let mut index = INDEX.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&key) = index.get(&name) {
        return key;
    }
    let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    let key = names.len() as u32;
    names.push(name.clone());
    index.insert(name, key);
    key
}

struct Fields(String);
impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.0, "{}{}={value:?}", if self.0.is_empty() { "" } else { " " }, field.name());
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        let _ = write!(self.0, "{}{}=\"{value}\"", if self.0.is_empty() { "" } else { " " }, field.name());
    }
}

/// The layer added to Bevy's log plugin (`LogPlugin::custom_layer`).
pub struct SpanProfile;

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for SpanProfile {
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let mut fields = Fields(String::new());
        attrs.record(&mut fields);
        let name = format!("{}: {}", attrs.metadata().name(), fields.0);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Key(intern(name)));
        }
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let Some(key) = ctx.span(id).and_then(|s| s.extensions().get::<Key>().map(|k| k.0)) else {
            return;
        };
        if key == render_key() {
            RENDER_OPEN.store(true, Ordering::Relaxed);
        }
        STACK.with(|s| s.borrow_mut().push((id.into_u64(), key, Instant::now(), 0)));
    }

    fn on_exit(&self, id: &span::Id, _: Context<'_, S>) {
        let now = Instant::now();
        let id = id.into_u64();
        STACK.with(|s| {
            let mut stack = s.borrow_mut();
            // Spans normally close in order; skip any left open by a moved task.
            let Some(at) = stack.iter().rposition(|e| e.0 == id) else {
                return;
            };
            let (_, key, start, children) = stack.remove(at);
            stack.truncate(at);
            let duration = now.duration_since(start).as_nanos() as u64;
            if let Some(parent) = stack.last_mut() {
                parent.3 += duration;
            }
            LOCAL.with(|local| {
                let mut stats = local.lock().unwrap_or_else(|e| e.into_inner());
                let entry = stats.spans.entry(key).or_default();
                entry[0] += duration;
                entry[1] += duration.saturating_sub(children);
                entry[2] += 1;
                if RENDER_OPEN.load(Ordering::Relaxed) && key != render_key() {
                    *stats.frame.entry(key).or_default() += duration.saturating_sub(children);
                }
            });
            if key == render_key() {
                RENDER_OPEN.store(false, Ordering::Relaxed);
                end_render_frame(duration);
            }
        });
    }
}

/// Collects every thread's spans of the render update that just ended.
fn end_render_frame(duration: u64) {
    let mut frame: HashMap<u32, u64> = HashMap::new();
    for thread in THREADS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        let mut stats = thread.lock().unwrap_or_else(|e| e.into_inner());
        for (key, ns) in stats.frame.drain() {
            *frame.entry(key).or_default() += ns;
        }
    }
    let mut render = RENDER.lock().unwrap_or_else(|e| e.into_inner());
    let render = render.get_or_insert_with(Default::default);
    render.frames += 1;
    for (&key, &ns) in &frame {
        *render.sum.entry(key).or_default() += ns;
    }
    let ms = duration as f64 / 1e6;
    if ms >= slow_ms() && render.slow.len() < MAX_SLOW {
        let mut top: Vec<_> = frame.into_iter().collect();
        top.sort_by_key(|e| std::cmp::Reverse(e.1));
        top.truncate(TOP_SLOW);
        render.slow.push((ms, top));
    }
}

/// A span name cut for one log line.
fn short(name: &str) -> String {
    let name = name.strip_prefix("system: name=").unwrap_or(name).trim_matches('"');
    name.chars().take(140).collect()
}

pub fn install(app: &mut App) {
    app.add_systems(Last, report);
    log_line!(
        "profile_start version=1 window_seconds={} scope=bevy-trace-spans",
        window_seconds()
    );
}

fn report(time: Res<Time<Real>>, mut elapsed: Local<f32>) {
    *elapsed += time.delta_secs();
    if *elapsed < window_seconds() {
        return;
    }
    let seconds = std::mem::take(&mut *elapsed);
    let names = NAMES.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut spans: HashMap<u32, [u64; 3]> = HashMap::new();
    // Per thread: time inside any span, and inside work spans (not wrappers).
    let mut threads: Vec<(String, u64, u64)> = Vec::new();
    let is_wrapper = |key: u32| {
        names
            .get(key as usize)
            .is_some_and(|n| WRAPPERS.iter().any(|w| n.starts_with(w)))
    };
    for thread in THREADS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        let mut stats = thread.lock().unwrap_or_else(|e| e.into_inner());
        let (mut inside, mut work) = (0, 0);
        for (key, v) in stats.spans.drain() {
            inside += v[1];
            if !is_wrapper(key) {
                work += v[1];
            }
            let entry = spans.entry(key).or_default();
            for k in 0..3 {
                entry[k] += v[k];
            }
        }
        threads.push((stats.name.clone(), inside, work));
    }
    let name = |key: u32| names.get(key as usize).map_or("?", String::as_str);
    let frames = spans
        .iter()
        .filter(|(k, _)| name(**k) == "schedule: name=Main")
        .map(|(_, v)| v[2])
        .sum::<u64>()
        .max(1) as f64;
    let ms = |ns: u64| ns as f64 / 1e6 / frames;
    log_line!(
        "profile_window version=1 seconds={seconds:.1} frames={frames} fps={:.1}{}",
        frames / f64::from(seconds),
        crate::perf_test::stats_suffix()
    );
    threads.sort_by_key(|t| std::cmp::Reverse(t.1));
    for (rank, (thread, inside, work)) in threads.iter().filter(|t| t.1 > 0).take(12).enumerate() {
        log_line!(
            "profile_thread rank={} in_spans_ms_per_frame={:.3} work_ms_per_frame={:.3} name={thread}",
            rank + 1,
            ms(*inside),
            ms(*work)
        );
    }
    let (wrappers, mut work): (Vec<_>, Vec<_>) = spans
        .into_iter()
        .partition(|(k, _)| WRAPPERS.iter().any(|w| name(*k).starts_with(w)));
    work.sort_by_key(|(_, v)| std::cmp::Reverse(v[1]));
    for (rank, (key, v)) in work.iter().take(top_work()).enumerate() {
        log_line!(
            "profile_span rank={} excl_ms_per_frame={:.3} incl_ms_per_frame={:.3} calls_per_frame={:.2} name={}",
            rank + 1,
            ms(v[1]),
            ms(v[0]),
            v[2] as f64 / frames,
            name(*key)
        );
    }
    let render = RENDER.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
    let mean = |key: u32| render.sum.get(&key).copied().unwrap_or(0) as f64 / 1e6 / render.frames.max(1) as f64;
    for (ms, top) in &render.slow {
        for (rank, (key, ns)) in top.iter().enumerate() {
            log_line!(
                "profile_slow render_ms={ms:.2} rank={} excl_ms={:.3} mean_ms={:.3} name={}",
                rank + 1,
                *ns as f64 / 1e6,
                mean(*key),
                short(name(*key))
            );
        }
    }
    let mut means: Vec<_> = render.sum.keys().copied().collect();
    means.sort_by(|a, b| mean(*b).total_cmp(&mean(*a)));
    for (rank, key) in means.iter().take(TOP_SLOW).enumerate() {
        log_line!(
            "profile_render_mean frames={} rank={} excl_ms={:.3} name={}",
            render.frames,
            rank + 1,
            mean(*key),
            short(name(*key))
        );
    }
    let mut wrappers = wrappers;
    wrappers.sort_by_key(|(_, v)| std::cmp::Reverse(v[0]));
    for (rank, (key, v)) in wrappers.iter().take(TOP_WRAPPERS).enumerate() {
        log_line!(
            "profile_schedule rank={} incl_ms_per_frame={:.3} calls_per_frame={:.2} name={}",
            rank + 1,
            ms(v[0]),
            v[2] as f64 / frames,
            name(*key)
        );
    }
}
