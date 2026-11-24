//! Worst fixed tick (docs/PERFORMANCE.md "Worst fixed tick"): times every
//! system of the fixed-step schedules, so a slow tick names the systems it
//! spent its time in.
//!
//! At the end of app setup (`Plugin::cleanup`) every system of `FixedMain`
//! and of each schedule in `FixedMainScheduleOrder` is wrapped in a timer:
//! two `Instant::now` and one atomic add per run, plus its deferred commands.
//! The `FixedMain` system (Bevy's `run_fixed_main`, one per tick) bounds the
//! tick; around it the process page-fault count is read.
//!
//! Output, always on:
//! - `frame_stats` (frame_split.rs): `fixed_max_ms`, the slowest single tick
//!   of the window.
//! - A `fixed_tick_slow` line for each tick of `SLOW_MS` or more (at most
//!   `SLOW_LINES` per run): tick wall time, race tick, page faults during the
//!   tick and the slowest systems (`schedule/#index=ms`; systems of one
//!   schedule run in parallel, so their times can overlap).
//! - `perf_timing` (`GRID_PERF_TIMING`): the run's worst tick the same way.
//!
//! Systems are named by their index in the schedule: release builds have no
//! system names (Bevy's `debug` feature, which only `--features profile`
//! enables). Every record prints the build revision, and the start line
//! prints the system count of each schedule. A profile build of the same
//! revision run with `GRID_FIXED_TICK_NAMES=1` prints the index to name table
//! and exits; matching counts confirm the table applies.
//!
//! Systems added to a fixed schedule after setup are not wrapped; their time
//! shows only in the tick total. Independently authored diagnostics; the
//! simulation is unchanged.
use bevy::app::{FixedMain, FixedMainScheduleOrder};
use bevy::ecs::change_detection::{CheckChangeTicks, Tick};
use bevy::ecs::query::FilteredAccessSet;
use bevy::ecs::schedule::{
    InternedScheduleLabel, InternedSystemSet, ScheduleLabel, Schedules, SystemWithAccess,
};
use bevy::ecs::system::{RunSystemError, SystemIn, SystemStateFlags};
use bevy::ecs::world::DeferredWorld;
use bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell;
use bevy::prelude::*;
use bevy::utils::prelude::DebugName;
use std::any::TypeId;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A tick this slow (ms) prints a `fixed_tick_slow` line.
const SLOW_MS: f64 = 40.;
/// At most this many `fixed_tick_slow` lines per run.
const SLOW_LINES: u32 = 100;
/// Systems named per slow or worst tick.
const TOP: usize = 8;

/// Reads a tick number to print with a slow tick (the race tick).
pub type TickLabel = fn(&World) -> Option<u64>;

/// Wrap the fixed schedules' systems when setup ends. `revision` is printed
/// with every record so an index can be matched to its name.
pub fn install(app: &mut App, label: TickLabel, revision: &'static str) {
    app.add_plugins(FixedTicksPlugin { label, revision });
}

struct FixedTicksPlugin {
    label: TickLabel,
    revision: &'static str,
}

impl Plugin for FixedTicksPlugin {
    fn build(&self, _app: &mut App) {}

    fn cleanup(&self, app: &mut App) {
        let wrapped = wrap(app.world_mut(), self.label, self.revision);
        println!("{}", wrapped.start_line(self.revision));
        if crate::dev::var("GRID_FIXED_TICK_NAMES").is_ok_and(|v| v == "1") {
            for line in wrapped.name_lines(self.revision, names_available()) {
                println!("{line}");
            }
            crate::log_out::exit(0);
        }
    }
}

/// Whether this build has Bevy's system names (its `debug` feature).
fn names_available() -> bool {
    DebugName::type_name::<Slot>().to_string().contains("Slot")
}

/// What `wrap` did: per schedule its system count, per slot its name.
struct Wrapped {
    count: usize,
    schedules: Vec<(String, usize)>,
    /// (`schedule/#index`, Bevy's system name).
    names: Vec<(String, String)>,
}

impl Wrapped {
    fn start_line(&self, revision: &str) -> String {
        let counts: Vec<String> = self
            .schedules
            .iter()
            .map(|(s, n)| format!("{s}:{n}"))
            .collect();
        format!(
            "fixed_ticks wrapped={} rev={revision} systems={}",
            self.count,
            counts.join(",")
        )
    }

    /// `fixed_tick_name` lines: index to name, when the build has names.
    fn name_lines(&self, revision: &str, available: bool) -> Vec<String> {
        if !available {
            return vec![
                "fixed_tick_name unavailable: build with --features profile (Bevy debug names)"
                    .into(),
            ];
        }
        let mut lines = vec![self.start_line(revision)];
        lines.extend(
            self.names
                .iter()
                .map(|(index, name)| format!("fixed_tick_name {index} {name}")),
        );
        lines
    }
}

/// One wrapped system's time in the current tick.
struct Slot {
    name: String,
    ns: AtomicU64,
}

struct Table {
    slots: Vec<Slot>,
    label: TickLabel,
    revision: &'static str,
}

/// Wraps every not yet built system of `FixedMain` and the schedules it runs.
fn wrap(world: &mut World, label: TickLabel, revision: &'static str) -> Wrapped {
    let mut labels: Vec<InternedScheduleLabel> = vec![FixedMain.intern()];
    if let Some(order) = world.get_resource::<FixedMainScheduleOrder>() {
        labels.extend(order.labels.iter().copied());
    }
    let mut wrapped = Wrapped {
        count: 0,
        schedules: Vec::new(),
        names: Vec::new(),
    };
    let Some(mut schedules) = world.get_resource_mut::<Schedules>() else {
        return wrapped;
    };
    // Slots first: the table is shared by all wrappers and fixed before they
    // run. A schedule's systems iterate in the order they were added.
    let mut targets = Vec::new();
    let mut slots = Vec::new();
    for &schedule_label in &labels {
        let Some(schedule) = schedules.get_mut(schedule_label) else {
            continue;
        };
        let tick = schedule_label == FixedMain.intern();
        let short = format!("{schedule_label:?}");
        let mut count = 0;
        for (index, (key, system, _)) in schedule.graph().systems.iter().enumerate() {
            let name = format!("{short}/#{index}");
            let full: String = system.name().to_string().split_whitespace().collect();
            wrapped.names.push((name.clone(), full));
            targets.push((
                schedule_label,
                key,
                tick && system.is_exclusive(),
                slots.len(),
            ));
            slots.push(Slot {
                name,
                ns: AtomicU64::new(0),
            });
            count += 1;
        }
        wrapped.schedules.push((short, count));
    }
    let table = Arc::new(Table {
        slots,
        label,
        revision,
    });
    for (schedule_label, key, tick, slot) in targets {
        let Some(schedule) = schedules.get_mut(schedule_label) else {
            continue;
        };
        let Some(node) = schedule.graph_mut().systems.get_mut(key) else {
            continue;
        };
        let placeholder = SystemWithAccess::new(Box::new(IntoSystem::into_system(|| {})));
        let inner = std::mem::replace(node, placeholder);
        *node = SystemWithAccess::new(Box::new(Timed {
            inner,
            slot,
            tick,
            table: table.clone(),
            faults: None,
        }));
        wrapped.count += 1;
    }
    wrapped
}

/// A system that adds its run and deferred-command time to its slot. The
/// tick system also starts and ends the tick record.
struct Timed {
    inner: SystemWithAccess,
    slot: usize,
    tick: bool,
    table: Arc<Table>,
    faults: Option<u64>,
}

impl Timed {
    fn add(&self, from: Instant) {
        let ns = from.elapsed().as_nanos() as u64;
        self.table.slots[self.slot]
            .ns
            .fetch_add(ns, Ordering::Relaxed);
    }
}

impl System for Timed {
    type In = ();
    type Out = ();

    fn name(&self) -> DebugName {
        self.inner.name()
    }

    fn system_type(&self) -> TypeId {
        self.inner.system_type()
    }

    fn flags(&self) -> SystemStateFlags {
        self.inner.flags()
    }

    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        if self.tick {
            for slot in &self.table.slots {
                slot.ns.store(0, Ordering::Relaxed);
            }
            self.faults = page_faults();
        }
        let from = Instant::now();
        // SAFETY: the caller upholds `run_unsafe`'s contract for this
        // system, whose access is the inner system's.
        let result = unsafe { self.inner.run_unsafe(input, world) };
        if self.tick {
            let wall = from.elapsed().as_nanos() as u64;
            let faults = self
                .faults
                .zip(page_faults())
                .map(|(a, b)| b.saturating_sub(a));
            // SAFETY: the tick system is exclusive (checked when wrapped), so
            // nothing else accesses the world while it runs.
            let label = (self.table.label)(unsafe { world.world() });
            tick_end(&self.table, wall, label, faults);
        } else {
            self.add(from);
        }
        result
    }

    fn apply_deferred(&mut self, world: &mut World) {
        let from = Instant::now();
        self.inner.apply_deferred(world);
        self.add(from);
    }

    fn queue_deferred(&mut self, world: DeferredWorld) {
        let from = Instant::now();
        self.inner.queue_deferred(world);
        self.add(from);
    }

    fn initialize(&mut self, world: &mut World) -> FilteredAccessSet {
        self.inner.initialize(world)
    }

    fn check_change_tick(&mut self, check: CheckChangeTicks) {
        self.inner.check_change_tick(check);
    }

    fn default_system_sets(&self) -> Vec<InternedSystemSet> {
        self.inner.default_system_sets()
    }

    fn get_last_run(&self) -> Tick {
        self.inner.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.inner.set_last_run(last_run);
    }
}

/// One recorded tick.
#[derive(Clone, Debug, Default)]
struct Record {
    /// Build revision (`GRID_BUILD_REVISION`), to match indices to names.
    revision: &'static str,
    ns: u64,
    /// Fixed ticks since start (this one included).
    run: u64,
    label: Option<u64>,
    faults: Option<u64>,
    top: Vec<(String, u64)>,
}

impl Record {
    fn fields(&self, prefix: &str) -> String {
        let mut s = format!(
            "{prefix}ms={:.2} {prefix}rev={} {prefix}run={} {prefix}tick={} {prefix}faults={}",
            self.ns as f64 / 1e6,
            self.revision,
            self.run,
            self.label.map_or("-".into(), |t| t.to_string()),
            self.faults.map_or("-".into(), |f| f.to_string())
        );
        // `fixed_worst.FixedUpdate/#3=` in perf_timing, `FixedUpdate/#3=` alone.
        let dot = prefix
            .strip_suffix('_')
            .map_or(String::new(), |p| format!("{p}."));
        for (name, ns) in &self.top {
            s += &format!(" {dot}{name}={:.2}", *ns as f64 / 1e6);
        }
        s
    }
}

#[derive(Default)]
struct State {
    runs: u64,
    window_max: u64,
    worst: Option<Record>,
    slow_lines: u32,
}

static STATE: Mutex<State> = Mutex::new(State {
    runs: 0,
    window_max: 0,
    worst: None,
    slow_lines: 0,
});

fn top(table: &Table) -> Vec<(String, u64)> {
    let mut all: Vec<_> = table
        .slots
        .iter()
        .map(|s| (s.name.as_str(), s.ns.load(Ordering::Relaxed)))
        .filter(|s| s.1 > 0)
        .collect();
    all.sort_by_key(|s| std::cmp::Reverse(s.1));
    all.into_iter()
        .take(TOP)
        .map(|(n, ns)| (n.to_owned(), ns))
        .collect()
}

fn tick_end(table: &Table, ns: u64, label: Option<u64>, faults: Option<u64>) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.runs += 1;
    state.window_max = state.window_max.max(ns);
    let slow = ns as f64 >= SLOW_MS * 1e6;
    let worst = state.worst.as_ref().is_none_or(|w| ns > w.ns);
    if !slow && !worst {
        return;
    }
    let record = Record {
        revision: table.revision,
        ns,
        run: state.runs,
        label,
        faults,
        top: top(table),
    };
    if slow && state.slow_lines < SLOW_LINES {
        state.slow_lines += 1;
        println!("fixed_tick_slow {}", record.fields(""));
    }
    if worst {
        state.worst = Some(record);
    }
}

/// `frame_stats` field: the slowest tick of the window (resets it).
pub fn window_suffix() -> String {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let max = std::mem::take(&mut state.window_max);
    format!(" fixed_max_ms={:.2}", max as f64 / 1e6)
}

/// `perf_timing` fields: the run's worst tick so far.
pub fn report() -> String {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state
        .worst
        .as_ref()
        .map_or(String::new(), |w| format!(" {}", w.fields("fixed_worst_")))
}

/// Page faults of this process so far (soft and hard).
#[cfg(windows)]
fn page_faults() -> Option<u64> {
    #[repr(C)]
    #[derive(Default)]
    // Same layout as front_end_host.rs (PROCESS_MEMORY_COUNTERS_EX), so
    // the two declarations of the import agree.
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut counters = Counters {
        cb: size_of::<Counters>() as u32,
        ..Default::default()
    };
    // SAFETY: this process's pseudo handle and an out-pointer to a local of
    // the size the call is given (PROCESS_MEMORY_COUNTERS_EX).
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    (ok != 0).then_some(u64::from(counters.page_fault_count))
}

/// Page faults of this process so far (minor and major).
#[cfg(unix)]
fn page_faults() -> Option<u64> {
    // SAFETY: getrusage writes the zeroed local it is given.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } == 0;
    ok.then(|| (usage.ru_minflt + usage.ru_majflt) as u64)
}

#[cfg(not(any(windows, unix)))]
fn page_faults() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct Order(Vec<&'static str>);

    fn first(mut order: ResMut<Order>) {
        order.0.push("first");
    }

    fn slow_after(mut order: ResMut<Order>, mut commands: Commands) {
        std::thread::sleep(Duration::from_millis(SLOW_MS as u64 + 5));
        order.0.push("slow");
        commands.insert_resource(Marker);
    }

    #[derive(Resource)]
    struct Marker;

    fn skipped(mut order: ResMut<Order>) {
        order.0.push("skipped");
    }

    /// The wrapped fixed loop keeps order, run conditions and commands, and
    /// its worst tick names the slow system with the race tick.
    #[test]
    fn worst_tick_names_the_slow_system() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                16,
            )))
            .init_resource::<Order>()
            .add_systems(
                FixedUpdate,
                (slow_after.after(first), first, skipped.run_if(|| false)),
            );
        install(
            &mut app,
            |world| world.get_resource::<Order>().map(|o| o.0.len() as u64),
            "r1",
        );
        app.finish();
        app.cleanup();
        for _ in 0..6 {
            app.update();
        }
        let order = &app.world().resource::<Order>().0;
        assert!(
            !order.is_empty() && order.chunks(2).all(|c| c == ["first", "slow"]),
            "{order:?}"
        );
        assert!(app.world().contains_resource::<Marker>());
        let report = report();
        assert!(report.contains(" fixed_worst_rev=r1 "), "{report}");
        // Systems are indexed in the order they were added.
        assert!(report.contains(" fixed_worst.FixedUpdate/#0="), "{report}");
        let worst = STATE.lock().unwrap().worst.clone().expect("a worst tick");
        assert!(worst.ns as f64 >= SLOW_MS * 1e6, "{worst:?}");
        assert_eq!(worst.top[0].0, "FixedUpdate/#0", "{worst:?}");
        assert!(
            worst.top.iter().all(|t| t.0 != "FixedUpdate/#2"),
            "{worst:?}"
        );
        assert!(
            worst.label.is_some_and(|t| t >= 2 && t % 2 == 0),
            "{worst:?}"
        );
        assert!(window_suffix().starts_with(" fixed_max_ms="));
    }

    /// The start line counts each schedule's systems; the name table (the
    /// `GRID_FIXED_TICK_NAMES` mode) maps indices to names in a build with
    /// Bevy's debug names and says how to get them otherwise.
    #[test]
    fn start_line_and_name_table() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Order>()
            .add_systems(FixedUpdate, (slow_after, first));
        let wrapped = wrap(app.world_mut(), |_| None, "r2");
        let line = wrapped.start_line("r2");
        let head = format!("fixed_ticks wrapped={} rev=r2 systems=", wrapped.count);
        assert!(line.starts_with(&head), "{line}");
        assert!(line.contains("FixedUpdate:2"), "{line}");
        assert!(line.contains("FixedMain:1"), "{line}");
        // With names: the start line, then one line per wrapped system.
        let lines = wrapped.name_lines("r2", true);
        assert_eq!(lines[0], line);
        assert_eq!(lines.len(), wrapped.count + 1, "{lines:?}");
        let slow = lines
            .iter()
            .find(|l| l.starts_with("fixed_tick_name FixedUpdate/#0 "));
        assert!(slow.is_some(), "{lines:?}");
        // Only a build with Bevy's debug names (profile) has the real name.
        if names_available() {
            assert!(slow.is_some_and(|l| l.ends_with("slow_after")), "{lines:?}");
        }
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("fixed_tick_name FixedUpdate/#1 "))
        );
        // Without: one line saying how to get them.
        let lines = wrapped.name_lines("r2", false);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("--features profile"), "{lines:?}");
    }
}
