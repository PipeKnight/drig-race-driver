//! Tick-cost budget of `grid-soak perf` (the build gate's performance check,
//! docs/TOOLCHAIN.md "Build gate"): one full-field quick scenario per track,
//! the cost of every `Session::step`, and a per-track budget recorded from
//! main. Diagnostics only; timing never feeds back into the simulation.
//!
//! The budget is on CPU time, not wall time: the gate shares the PC with other
//! sessions' builds, and the tick fans out over every core
//! ([`crate::parallel`]), so its wall time follows what else is running (two
//! runs of the same Milan case on one afternoon: 14.4 and 9.3 ms). `perf` runs
//! its cases with `GRID_THREADS=1` and sums the process CPU time spent inside
//! the steps; only the p99 stays a wall time, with a wide margin.
use std::collections::BTreeMap;

/// One perf case: a quick-profile seed on its track, with its field size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Case {
    pub track: &'static str,
    pub seed: u64,
    pub cars: usize,
}

/// One full-field quick seed per track (docs/SOAK_TESTING.md, quick seeds).
/// Haruna's touge grid holds two cars. Le Mans runs 12 cars here: grid-soak's
/// shared world stops at 12, while the app's 24 Hours field has 16 or 17.
pub const CASES: [Case; 13] = [
    Case { track: "donington", seed: 1, cars: 12 },
    Case { track: "nurburgring", seed: 3, cars: 12 },
    Case { track: "istanbul", seed: 14, cars: 12 },
    Case { track: "long_beach", seed: 17, cars: 12 },
    Case { track: "jarama", seed: 22, cars: 12 },
    Case { track: "yokohama", seed: 25, cars: 12 },
    Case { track: "haruna", seed: 30, cars: 2 },
    Case { track: "detroit", seed: 33, cars: 12 },
    Case { track: "spa", seed: 37, cars: 12 },
    Case { track: "milan", seed: 41, cars: 12 },
    Case { track: "shibuya", seed: 45, cars: 12 },
    Case { track: "san_francisco", seed: 49, cars: 12 },
    Case { track: "le_mans", seed: 54, cars: 12 },
];

/// Default ticks per case: 48 s of race, long enough for the first corners'
/// prop breaks (the heavy part of Milan seed 41).
pub const DEFAULT_TICKS: u64 = 3000;

/// Process CPU time (user + kernel) in ms, where the platform reports it.
#[cfg(windows)]
pub fn process_cpu_ms() -> Option<f64> {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn GetProcessTimes(
            process: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }
    let (mut c, mut e, mut k, mut u) =
        (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
    // SAFETY: the pseudo-handle of this process and four valid out-pointers.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
    let ticks = |t: &FileTime| (u64::from(t.high) << 32 | u64::from(t.low)) as f64;
    (ok != 0).then(|| (ticks(&k) + ticks(&u)) / 10_000.)
}

/// Process CPU time: not measured off Windows.
#[cfg(not(windows))]
pub fn process_cpu_ms() -> Option<f64> {
    None
}

/// Restricts this process (and the processes it starts later, which inherit
/// it) to the logical processors in `mask`. On the owner's i7-12700 an E-core
/// (logical 16..19) spends twice the CPU time of a P-core on the same tick
/// (Haruna seed 30: 0.30 against 0.62-0.66 ms), so `perf` pins to the P-core
/// threads, mask `FFFF`.
#[cfg(windows)]
pub fn set_affinity(mask: u64) -> Result<(), String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn SetProcessAffinityMask(process: isize, mask: usize) -> i32;
    }
    // SAFETY: the pseudo-handle of this process; a mask is a plain value.
    let ok = unsafe { SetProcessAffinityMask(GetCurrentProcess(), mask as usize) };
    if ok == 0 {
        return Err(format!("SetProcessAffinityMask({mask:#x}) failed: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

/// Affinity: Windows only.
#[cfg(not(windows))]
pub fn set_affinity(_mask: u64) -> Result<(), String> {
    Err("--affinity is implemented on Windows only".into())
}

/// Summary of per-tick costs in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickTimes {
    pub ticks: usize,
    /// Process CPU time per tick (NaN where not measured).
    pub cpu: f64,
    /// Wall time per tick.
    pub mean: f64,
    pub p50: f64,
    pub p99: f64,
    pub max: f64,
}

impl TickTimes {
    /// From each step's wall time and the CPU time summed over the steps
    /// (`None` without samples).
    pub fn of(ms: &[f32], cpu_ms: Option<f64>) -> Option<Self> {
        if ms.is_empty() {
            return None;
        }
        let mut sorted: Vec<f64> = ms.iter().map(|&v| f64::from(v)).collect();
        sorted.sort_by(f64::total_cmp);
        let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
        let n = sorted.len() as f64;
        Some(Self {
            ticks: sorted.len(),
            cpu: cpu_ms.map_or(f64::NAN, |c| c / n),
            mean: sorted.iter().sum::<f64>() / n,
            p50: at(0.5),
            p99: at(0.99),
            max: sorted[sorted.len() - 1],
        })
    }

    /// The `soak_tick_time` line of `grid-soak run`.
    pub fn line(&self, seed: u64) -> String {
        format!(
            "soak_tick_time seed={seed} ticks={} cpu_ms={:.3} mean_ms={:.3} p50_ms={:.3} p99_ms={:.3} max_ms={:.3}",
            self.ticks, self.cpu, self.mean, self.p50, self.p99, self.max
        )
    }

    /// Parses a `soak_tick_time` line.
    pub fn parse(line: &str) -> Option<(u64, Self)> {
        let rest = line.strip_prefix("soak_tick_time ")?;
        let mut fields = BTreeMap::new();
        for pair in rest.split_whitespace() {
            let (k, v) = pair.split_once('=')?;
            fields.insert(k, v);
        }
        let f = |k: &str| fields.get(k)?.parse::<f64>().ok();
        Some((
            fields.get("seed")?.parse().ok()?,
            Self {
                ticks: fields.get("ticks")?.parse().ok()?,
                cpu: f("cpu_ms")?,
                mean: f("mean_ms")?,
                p50: f("p50_ms")?,
                p99: f("p99_ms")?,
                max: f("max_ms")?,
            },
        ))
    }
}

/// A track's budget: CPU time per tick and 99th-percentile wall time, in ms,
/// and the scenario they were measured on (its end-state digest and peak
/// world body count, from `soak_result`).
#[derive(Clone, Debug, PartialEq)]
pub struct Budget {
    pub baseline_cpu: f64,
    pub baseline_p99: f64,
    pub cpu: f64,
    pub p99: f64,
    pub digest: String,
    pub bodies: usize,
}

/// Headroom over the recorded baseline: 1.3x CPU per tick catches a
/// regression such as F486's Milan seed 41 (14 → 19 ms per tick); the wall
/// p99 follows the PC's load and only catches gross stalls.
pub const CPU_FACTOR: f64 = 1.3;
pub const P99_FACTOR: f64 = 3.0;
/// A change of driving or physics behaviour changes the scenario itself (other
/// crashes, other broken props: F517's AI change moved Istanbul seed 14 from
/// 1.5 to 2.9 ms per tick), so its cost is not comparable. Such a case only
/// gets a gross check at 2x the old baseline CPU and asks for a new budget.
pub const CHANGED_FACTOR: f64 = 2.0;

/// The outcome of a budget check.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    /// The scenario differs from the one the budget was recorded on.
    pub changed: bool,
    /// Why the measurement is over the budget (empty when within).
    pub over: Vec<String>,
}

impl Budget {
    pub fn from_baseline(t: &TickTimes, digest: &str, bodies: usize) -> Self {
        Self {
            baseline_cpu: t.cpu,
            baseline_p99: t.p99,
            cpu: t.cpu * CPU_FACTOR,
            p99: t.p99 * P99_FACTOR,
            digest: digest.to_string(),
            bodies,
        }
    }

    /// Checks a measurement of the scenario with this `digest`. A NaN on
    /// either side is not a failure.
    pub fn check(&self, t: &TickTimes, digest: &str) -> Verdict {
        let changed = digest != self.digest;
        let (cpu, p99) = if changed {
            (
                self.baseline_cpu * CHANGED_FACTOR,
                self.baseline_p99 * P99_FACTOR * CHANGED_FACTOR,
            )
        } else {
            (self.cpu, self.p99)
        };
        let mut over = Vec::new();
        if t.cpu > cpu {
            over.push(format!("cpu {:.2} ms/tick > budget {cpu:.2} ms", t.cpu));
        }
        if t.p99 > p99 {
            over.push(format!("wall p99 {:.2} ms > budget {p99:.2} ms", t.p99));
        }
        Verdict { changed, over }
    }
}

/// Budget file: `#` comments, then the [`BUDGET_HEADER`] columns separated by
/// tabs. Keyed by track.
pub fn parse_budgets(text: &str) -> Result<BTreeMap<String, Budget>, String> {
    let mut out = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("track\t") {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 10 {
            return Err(format!("budget line {}: expected 10 columns", n + 1));
        }
        let num = |i: usize| {
            cols[i]
                .parse::<f64>()
                .map_err(|_| format!("budget line {}: bad number `{}`", n + 1, cols[i]))
        };
        out.insert(
            cols[0].to_string(),
            Budget {
                baseline_cpu: num(4)?,
                baseline_p99: num(5)?,
                cpu: num(6)?,
                p99: num(7)?,
                digest: cols[8].to_string(),
                bodies: num(9)? as usize,
            },
        );
    }
    Ok(out)
}

/// One budget row for a measured case.
pub fn budget_row(case: &Case, t: &TickTimes, digest: &str, bodies: usize) -> String {
    let b = Budget::from_baseline(t, digest, bodies);
    format!(
        "{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{digest}\t{bodies}",
        case.track, case.seed, case.cars, t.ticks, t.cpu, t.p99, b.cpu, b.p99
    )
}

pub const BUDGET_HEADER: &str = "track\tseed\tcars\tticks\tbaseline_cpu_ms\tbaseline_p99_ms\tbudget_cpu_ms\tbudget_p99_ms\tdigest\tmax_bodies";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_times_give_cpu_mean_percentiles_and_max() {
        let ms: Vec<f32> = (1..=100).map(|v| v as f32).collect();
        let t = TickTimes::of(&ms, Some(250.)).unwrap();
        assert_eq!(t.ticks, 100);
        assert!((t.mean - 50.5).abs() < 1e-9);
        assert_eq!((t.cpu, t.p50, t.p99, t.max), (2.5, 51., 99., 100.));
        assert!(TickTimes::of(&ms, None).unwrap().cpu.is_nan());
        assert!(TickTimes::of(&[], Some(1.)).is_none());
    }

    #[test]
    fn tick_time_lines_round_trip() {
        let t = TickTimes { ticks: 3000, cpu: 5.5, mean: 6.25, p50: 6., p99: 11.5, max: 40.125 };
        assert_eq!(TickTimes::parse(&t.line(41)), Some((41, t)));
        assert_eq!(TickTimes::parse("soak_result seed=41"), None);
    }

    #[test]
    fn budgets_round_trip_and_flag_regressions() {
        let case = CASES[9];
        assert_eq!(case.track, "milan");
        let t = TickTimes { ticks: 3000, cpu: 10., mean: 10., p50: 9., p99: 20., max: 50. };
        let text = format!("# recorded\n{BUDGET_HEADER}\n{}\n", budget_row(&case, &t, "00ab", 132));
        let budgets = parse_budgets(&text).unwrap();
        let b = &budgets["milan"];
        assert_eq!((b.cpu, b.p99, b.digest.as_str(), b.bodies), (13., 60., "00ab", 132));
        assert_eq!(b.check(&t, "00ab"), Verdict { changed: false, over: vec![] });
        // F486's Milan regression (14 → 19 ms) against a 14 ms baseline fails.
        let base = Budget::from_baseline(&TickTimes { cpu: 14., ..t }, "00ab", 132);
        assert!(base.check(&TickTimes { cpu: 18., ..t }, "00ab").over.is_empty());
        assert_eq!(base.check(&TickTimes { cpu: 19., ..t }, "00ab").over.len(), 1);
        assert!(base.check(&TickTimes { cpu: f64::NAN, ..t }, "00ab").over.is_empty());
        // Another scenario: only a gross check (2x the old baseline).
        let other = base.check(&TickTimes { cpu: 19., ..t }, "ffff");
        assert_eq!(other, Verdict { changed: true, over: vec![] });
        assert_eq!(base.check(&TickTimes { cpu: 29., ..t }, "ffff").over.len(), 1);
        assert!(parse_budgets("milan\t41\n").is_err());
    }

    #[test]
    fn every_track_has_one_case() {
        let mut tracks: Vec<_> = CASES.iter().map(|c| c.track).collect();
        tracks.sort_unstable();
        tracks.dedup();
        assert_eq!(tracks.len(), CASES.len());
    }

    #[cfg(windows)]
    #[test]
    fn process_cpu_time_advances_with_work() {
        let a = process_cpu_ms().unwrap();
        let mut x = 0u64;
        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < 100 {
            x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
        assert!(process_cpu_ms().unwrap() > a);
    }
}
