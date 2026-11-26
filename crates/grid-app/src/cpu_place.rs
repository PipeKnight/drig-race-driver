//! CPU placement switches for the phone (docs/ANDROID_CPU.md).
//!
//! The phone's CPU is big.LITTLE (Snapdragon 8 Gen 2: 3 little, 4 big,
//! 1 prime core). Where the scheduler runs the main and render threads
//! decides how long a frame takes once the GPU is no longer the limit.
//! These words in the diagnostics `perf_test` list (tools/android_bench.ps1
//! `-PerfTest`) are runtime experiments, all off by default:
//!
//! - `pin_main_big`, `pin_main_prime`: the main thread's affinity
//!   (`sched_setaffinity` on our own thread);
//! - `pin_render_big`, `pin_render_prime`: the render thread's;
//! - `pin_pool_big`, `pin_pool_noprime`: the compute task pool's threads;
//! - `uclamp`: a minimum utilisation clamp of half the scale on the main and
//!   render threads (`sched_setattr`), which steers the energy-aware
//!   scheduler to faster cores and clocks without pinning.
//!
//! ADPF performance hints are the `adpf` word of `adpf.rs` (docs/ANDROID_GPU.md
//! section 10).
//!
//! The core classes come from `/sys/devices/system/cpu/cpuN/cpu_capacity`
//! (fallback `cpufreq/cpuinfo_max_freq`), never from a fixed core list:
//! little = the lowest capacity, big = every faster core, prime = the
//! highest. Every frame_stats window (Linux and Android) also reports, per
//! frame, each thread group's run time and runqueue wait from
//! `/proc/self/task/<tid>/schedstat`, and the share of frames the main and
//! render threads began on a little or a prime core. Other systems compile
//! this module to no-ops.

/// The perf_test words this module owns.
pub const WORDS: &[&str] = &[
    "pin_main_big",
    "pin_main_prime",
    "pin_render_big",
    "pin_render_prime",
    "pin_pool_big",
    "pin_pool_noprime",
    "uclamp",
];

/// Whether `word` is one of this module's perf_test words.
pub fn is_word(word: &str) -> bool {
    WORDS.contains(&word)
}

/// Which cores a thread group may use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(any(target_os = "android", target_os = "linux")), allow(dead_code))]
enum Place {
    /// Unchanged: the scheduler decides.
    #[default]
    Any,
    Big,
    Prime,
    NoPrime,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(any(target_os = "android", target_os = "linux")), allow(dead_code))]
struct Words {
    main: Place,
    render: Place,
    pool: Place,
    uclamp: bool,
}

#[cfg_attr(not(any(target_os = "android", target_os = "linux")), allow(dead_code))]
impl Words {
    fn parse(list: &str) -> Self {
        let mut w = Words::default();
        for part in list.split(',').map(str::trim) {
            match part {
                "pin_main_big" => w.main = Place::Big,
                "pin_main_prime" => w.main = Place::Prime,
                "pin_render_big" => w.render = Place::Big,
                "pin_render_prime" => w.render = Place::Prime,
                "pin_pool_big" => w.pool = Place::Big,
                "pin_pool_noprime" => w.pool = Place::NoPrime,
                "uclamp" => w.uclamp = true,
                _ => {}
            }
        }
        w
    }

    fn any(&self) -> bool {
        *self != Words::default()
    }

    /// The words in a fixed order, for the log.
    fn label(&self) -> String {
        let mut out = Vec::new();
        let name = |p: Place| match p {
            Place::Any => "",
            Place::Big => "big",
            Place::Prime => "prime",
            Place::NoPrime => "noprime",
        };
        for (group, place) in [("main", self.main), ("render", self.render), ("pool", self.pool)] {
            if place != Place::Any {
                out.push(format!("pin_{group}_{}", name(place)));
            }
        }
        if self.uclamp {
            out.push("uclamp".into());
        }
        out.join("+")
    }
}

/// The core classes of this CPU, from each core's capacity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(any(target_os = "android", target_os = "linux")), allow(dead_code))]
struct Topology {
    capacity: Vec<u32>,
    little: Vec<usize>,
    big: Vec<usize>,
    prime: Vec<usize>,
}

#[cfg_attr(not(any(target_os = "android", target_os = "linux")), allow(dead_code))]
impl Topology {
    /// Classes from per-core capacities (index = core number). An unknown
    /// capacity (0) anywhere, or equal capacities, make every core big and
    /// prime and none little.
    fn from_capacity(capacity: &[u32]) -> Self {
        let all: Vec<usize> = (0..capacity.len()).collect();
        let min = capacity.iter().copied().min().unwrap_or(0);
        let max = capacity.iter().copied().max().unwrap_or(0);
        if min == 0 || min == max {
            return Topology { capacity: capacity.to_vec(), little: Vec::new(), big: all.clone(), prime: all };
        }
        let pick = |f: &dyn Fn(u32) -> bool| all.iter().copied().filter(|&i| f(capacity[i])).collect::<Vec<_>>();
        Topology {
            capacity: capacity.to_vec(),
            little: pick(&|c| c == min),
            big: pick(&|c| c > min),
            prime: pick(&|c| c == max),
        }
    }

    /// The cores for `place`; empty = leave the affinity as it is.
    fn cores(&self, place: Place) -> Vec<usize> {
        match place {
            Place::Any => Vec::new(),
            Place::Big => self.big.clone(),
            Place::Prime => self.prime.clone(),
            Place::NoPrime => {
                let rest: Vec<usize> = (0..self.capacity.len()).filter(|c| !self.prime.contains(c)).collect();
                if rest.is_empty() { (0..self.capacity.len()).collect() } else { rest }
            }
        }
    }

    fn class(&self, cpu: i32) -> (bool, bool) {
        let Ok(cpu) = usize::try_from(cpu) else { return (false, false) };
        (self.little.contains(&cpu), !self.little.is_empty() && self.prime.contains(&cpu))
    }
}

/// Run time and runqueue wait in ms per frame over a window.
#[cfg_attr(not(any(target_os = "android", target_os = "linux")), allow(dead_code))]
fn per_frame_ms(delta_ns: u64, frames: u32) -> f64 {
    if frames == 0 { 0. } else { delta_ns as f64 / 1e6 / f64::from(frames) }
}

/// Parses `/proc/<..>/schedstat`: run ns, wait ns, timeslices.
#[cfg_attr(not(any(target_os = "android", target_os = "linux")), allow(dead_code))]
fn parse_schedstat(text: &str) -> Option<(u64, u64)> {
    let mut it = text.split_whitespace().map(str::parse::<u64>);
    Some((it.next()?.ok()?, it.next()?.ok()?))
}

#[cfg(any(target_os = "android", target_os = "linux"))]
mod imp {
    use super::{Place, Topology, Words, per_frame_ms};
    use bevy::prelude::*;
    use bevy::render::{Render, RenderApp, RenderSystems};
    use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
    use std::sync::{Mutex, OnceLock};

    struct Config {
        words: Words,
        topo: Topology,
    }

    static CONFIG: OnceLock<Config> = OnceLock::new();
    /// Compute pool threads, recorded as they start (mobile.rs).
    static POOL_TIDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());
    static RENDER_TID: AtomicI32 = AtomicI32::new(0);
    static RENDER_FRAMES: AtomicU64 = AtomicU64::new(0);
    static RENDER_LITTLE: AtomicU64 = AtomicU64::new(0);
    static RENDER_PRIME: AtomicU64 = AtomicU64::new(0);

    #[derive(Default)]
    struct Main {
        tid: i32,
        placed: bool,
        frames: u64,
        little: u64,
        prime: u64,
        /// Previous window: schedstat sums (main, render, pool) and counters.
        last: [(u64, u64); 3],
        last_counts: [u64; 6],
    }

    static MAIN: Mutex<Option<Main>> = Mutex::new(None);

    fn tid() -> i32 {
        // SAFETY: gettid has no preconditions.
        unsafe { libc::gettid() }
    }

    fn cpu() -> i32 {
        // SAFETY: sched_getcpu has no preconditions.
        unsafe { libc::sched_getcpu() }
    }

    fn errno() -> i32 {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(-1)
    }

    /// `sched_setaffinity` on one of our threads; 0 or the errno.
    fn set_affinity(tid: i32, cores: &[usize]) -> i32 {
        // SAFETY: a zeroed cpu_set_t is valid; the pointer and size describe it.
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut set);
            for &c in cores {
                libc::CPU_SET(c, &mut set);
            }
            if libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0 { 0 } else { errno() }
        }
    }

    /// The kernel's `struct sched_attr` (include/uapi/linux/sched/types.h).
    #[repr(C)]
    #[derive(Default)]
    struct SchedAttr {
        size: u32,
        policy: u32,
        flags: u64,
        nice: i32,
        priority: u32,
        runtime: u64,
        deadline: u64,
        period: u64,
        util_min: u32,
        util_max: u32,
    }

    /// Sets only the minimum utilisation clamp; 0 or the errno.
    fn uclamp_min(tid: i32, util_min: u32) -> i32 {
        const KEEP_POLICY: u64 = 0x08;
        const KEEP_PARAMS: u64 = 0x10;
        const UTIL_CLAMP_MIN: u64 = 0x20;
        let attr = SchedAttr {
            size: std::mem::size_of::<SchedAttr>() as u32,
            flags: KEEP_POLICY | KEEP_PARAMS | UTIL_CLAMP_MIN,
            util_min,
            util_max: 1024,
            ..Default::default()
        };
        // SAFETY: the pointer is a valid sched_attr of the size it states.
        let r = unsafe { libc::syscall(libc::SYS_sched_setattr, tid, &attr as *const SchedAttr, 0u32) };
        if r == 0 { 0 } else { errno() }
    }

    fn schedstat(tid: i32) -> Option<(u64, u64)> {
        let text = std::fs::read_to_string(format!("/proc/self/task/{tid}/schedstat")).ok()?;
        super::parse_schedstat(&text)
    }

    fn read_u32(path: &str) -> Option<u32> {
        std::fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    fn read_capacities() -> Vec<u32> {
        let mut out = Vec::new();
        for n in 0..256 {
            let base = format!("/sys/devices/system/cpu/cpu{n}");
            if !std::path::Path::new(&base).exists() {
                break;
            }
            let cap = read_u32(&format!("{base}/cpu_capacity"))
                .or_else(|| read_u32(&format!("{base}/cpufreq/cpuinfo_max_freq")));
            out.push(cap.unwrap_or(0));
        }
        out
    }

    fn place(thread: &str, tid: i32, place: Place, topo: &Topology) {
        let cores = topo.cores(place);
        if cores.is_empty() {
            return;
        }
        let result = set_affinity(tid, &cores);
        log_line!("cpu_place version=1 event=pin thread={thread} tid={tid} cpus={cores:?} result={result}");
    }

    fn clamp(thread: &str, tid: i32) {
        let result = uclamp_min(tid, 512);
        log_line!("cpu_place version=1 event=uclamp thread={thread} tid={tid} util_min=512 result={result}");
    }

    pub fn note_pool_thread() {
        if let Ok(mut tids) = POOL_TIDS.lock() {
            tids.push(tid());
        }
    }

    pub fn install(app: &mut App) {
        let list = crate::settings::diagnostics().perf_test.clone().unwrap_or_default();
        let words = Words::parse(&list);
        let topo = Topology::from_capacity(&read_capacities());
        log_line!(
            "cpu_topology version=1 capacity={:?} little={:?} big={:?} prime={:?} pool_threads={}",
            topo.capacity,
            topo.little,
            topo.big,
            topo.prime,
            POOL_TIDS.lock().map_or(0, |t| t.len())
        );
        let _ = CONFIG.set(Config { words, topo });
        if let Ok(mut main) = MAIN.lock() {
            *main = Some(Main::default());
        }
        app.add_systems(First, main_begin);
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render
                .add_systems(Render, render_begin.before(RenderSystems::ExtractCommands));
        }
    }

    fn render_begin(_: &mut World) {
        let Some(config) = CONFIG.get() else { return };
        if RENDER_TID.load(Ordering::Relaxed) == 0 {
            let id = tid();
            place("render", id, config.words.render, &config.topo);
            if config.words.uclamp {
                clamp("render", id);
            }
            RENDER_TID.store(id, Ordering::Relaxed);
        }
        let (little, prime) = config.topo.class(cpu());
        RENDER_FRAMES.fetch_add(1, Ordering::Relaxed);
        RENDER_LITTLE.fetch_add(u64::from(little), Ordering::Relaxed);
        RENDER_PRIME.fetch_add(u64::from(prime), Ordering::Relaxed);
    }

    fn main_begin(_: &mut World) {
        let Some(config) = CONFIG.get() else { return };
        let Ok(mut guard) = MAIN.lock() else { return };
        let Some(main) = guard.as_mut() else { return };
        if !main.placed {
            main.placed = true;
            main.tid = tid();
            place("main", main.tid, config.words.main, &config.topo);
            if config.words.uclamp {
                clamp("main", main.tid);
            }
            if config.words.pool != Place::Any {
                for &t in POOL_TIDS.lock().map(|t| t.clone()).unwrap_or_default().iter() {
                    place("pool", t, config.words.pool, &config.topo);
                }
            }
        }
        let (little, prime) = config.topo.class(cpu());
        main.frames += 1;
        main.little += u64::from(little);
        main.prime += u64::from(prime);
    }

    /// The frame_stats suffix for one window of `frames` frames.
    pub fn window_report(frames: u32) -> String {
        let Some(config) = CONFIG.get() else { return String::new() };
        let Ok(mut guard) = MAIN.lock() else { return String::new() };
        let Some(main) = guard.as_mut() else { return String::new() };
        let sum = |tids: &[i32]| {
            tids.iter().filter_map(|&t| schedstat(t)).fold((0u64, 0u64), |a, b| (a.0 + b.0, a.1 + b.1))
        };
        let render_tid = RENDER_TID.load(Ordering::Relaxed);
        let pool = POOL_TIDS.lock().map(|t| t.clone()).unwrap_or_default();
        let now = [
            if main.tid != 0 { sum(&[main.tid]) } else { (0, 0) },
            if render_tid != 0 { sum(&[render_tid]) } else { (0, 0) },
            sum(&pool),
        ];
        let counts = [
            main.frames,
            main.little,
            main.prime,
            RENDER_FRAMES.load(Ordering::Relaxed),
            RENDER_LITTLE.load(Ordering::Relaxed),
            RENDER_PRIME.load(Ordering::Relaxed),
        ];
        let d = |i: usize| (now[i].0.saturating_sub(main.last[i].0), now[i].1.saturating_sub(main.last[i].1));
        let c = |i: usize| counts[i].saturating_sub(main.last_counts[i]);
        let share = |part: u64, all: u64| if all == 0 { 0. } else { 100. * part as f64 / all as f64 };
        let mut out = String::new();
        for (i, name) in ["main", "render", "pool"].into_iter().enumerate() {
            let (run, wait) = d(i);
            out += &format!(" cpu_{name}_run={:.2} cpu_{name}_wait={:.2}", per_frame_ms(run, frames), per_frame_ms(wait, frames));
        }
        out += &format!(
            " main_little={:.0}% main_prime={:.0}% render_little={:.0}% render_prime={:.0}%",
            share(c(1), c(0)),
            share(c(2), c(0)),
            share(c(4), c(3)),
            share(c(5), c(3))
        );
        if config.words.any() {
            out += &format!(" cpu_place={}", config.words.label());
        }
        main.last = now;
        main.last_counts = counts;
        out
    }
}

#[cfg(not(any(target_os = "android", target_os = "linux")))]
mod imp {
    use bevy::prelude::App;

    pub fn note_pool_thread() {}

    pub fn install(_: &mut App) {}

    pub fn window_report(_: u32) -> String {
        String::new()
    }
}

pub use imp::{install, note_pool_thread, window_report};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_of_a_one_four_three_cpu() {
        // Snapdragon 8 Gen 2 capacities: three little, four big, one prime.
        let t = Topology::from_capacity(&[325, 325, 325, 829, 829, 829, 829, 1024]);
        assert_eq!(t.little, vec![0, 1, 2]);
        assert_eq!(t.big, vec![3, 4, 5, 6, 7]);
        assert_eq!(t.prime, vec![7]);
        assert_eq!(t.cores(Place::NoPrime), vec![0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(t.cores(Place::Any), Vec::<usize>::new());
        assert_eq!(t.class(1), (true, false));
        assert_eq!(t.class(7), (false, true));
        assert_eq!(t.class(4), (false, false));
    }

    #[test]
    fn equal_or_unknown_capacities_have_no_little_cores() {
        for caps in [vec![1024; 4], vec![1024, 0, 1024, 1024]] {
            let t = Topology::from_capacity(&caps);
            assert!(t.little.is_empty());
            assert_eq!(t.big, vec![0, 1, 2, 3]);
            assert_eq!(t.cores(Place::NoPrime), vec![0, 1, 2, 3]);
            assert_eq!(t.class(0), (false, false));
        }
    }

    #[test]
    fn words_parse_and_label() {
        let w = Words::parse("lean, pin_main_prime,pin_render_big,pin_pool_noprime,uclamp");
        assert_eq!(w.main, Place::Prime);
        assert_eq!(w.render, Place::Big);
        assert_eq!(w.pool, Place::NoPrime);
        assert!(w.uclamp && w.any());
        assert_eq!(w.label(), "pin_main_prime+pin_render_big+pin_pool_noprime+uclamp");
        assert!(!Words::parse("lean,render_single").any());
        assert!(WORDS.iter().all(|w| is_word(w)) && !is_word("lean"));
    }

    #[test]
    fn schedstat_line() {
        assert_eq!(parse_schedstat("123456 789 42\n"), Some((123456, 789)));
        assert_eq!(parse_schedstat(""), None);
        assert!((per_frame_ms(30_000_000, 30) - 1.).abs() < 1e-9);
        assert_eq!(per_frame_ms(5, 0), 0.);
    }
}
