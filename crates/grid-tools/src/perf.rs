//! Opt-in wall-clock timing of the per-frame stages (docs/PERFORMANCE.md).
//!
//! Off unless the environment variable `GRID_PERF_TIMING` is set when the
//! first timer starts; then each [`Stage`] accumulates its total time and call
//! count, and [`report`] prints them. Timing never feeds back into the
//! simulation, so results are identical with it on or off.
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Timed stages. App stages are measured by grid-app; the rest by
/// [`crate::source_vehicle::multi::MultiRuntime::tick`].
#[derive(Clone, Copy, Debug)]
pub enum Stage {
    /// One whole fixed-update `drive_tick` (grid-app).
    DriveTick,
    /// The race manager and every AI driver's controls (grid-app).
    AiDrivers,
    /// One whole shared-world tick.
    WorldTick,
    /// Every car's vehicle manager (suspension rays, handling, wheel contacts).
    VehicleStage,
    /// Every car's static track, prop and boundary body contacts.
    BodyContacts,
    /// Car-to-car hull contacts.
    CarPairs,
    /// Dynamic prop piece contacts.
    PieceContacts,
    /// The world advance (solver, integration), without the contact provider.
    WorldAdvance,
    /// Prop detection, body events, lap timing and the commit.
    Commit,
    /// The main schedule, First to Last (grid-app).
    MainSchedule,
    /// The render schedule, extract commands to the end of rendering (grid-app).
    RenderSchedule,
}

const STAGES: [Stage; 11] = [
    Stage::DriveTick,
    Stage::AiDrivers,
    Stage::WorldTick,
    Stage::VehicleStage,
    Stage::BodyContacts,
    Stage::CarPairs,
    Stage::PieceContacts,
    Stage::WorldAdvance,
    Stage::Commit,
    Stage::MainSchedule,
    Stage::RenderSchedule,
];

static TOTALS: [AtomicU64; 11] = [const { AtomicU64::new(0) }; 11];
static COUNTS: [AtomicU64; 11] = [const { AtomicU64::new(0) }; 11];

/// Whether timing is on (`GRID_PERF_TIMING` set at the first query).
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("GRID_PERF_TIMING").is_some())
}

/// Add an externally measured duration to `stage`.
pub fn add(stage: Stage, nanos: u64) {
    TOTALS[stage as usize].fetch_add(nanos, Ordering::Relaxed);
    COUNTS[stage as usize].fetch_add(1, Ordering::Relaxed);
}

/// World size per shared-world tick: bodies, pieces and lost wheels
/// (bodies beyond the cars), and active contacts by kind (wheel, static
/// body, car pair, piece).
static WORLD: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

/// Count one shared-world tick's size (when timing is on).
pub fn world(bodies: usize, pieces: usize, contacts: [usize; 4]) {
    if !enabled() {
        return;
    }
    let values = [bodies, pieces, contacts[0], contacts[1], contacts[2], contacts[3]];
    for (sum, value) in WORLD.iter().zip(values) {
        sum.fetch_add(value as u64, Ordering::Relaxed);
    }
}

/// Times `stage` until dropped; does nothing when timing is off.
pub struct Timer(Option<(Stage, Instant)>);

impl Timer {
    pub fn start(stage: Stage) -> Self {
        Self(enabled().then(|| (stage, Instant::now())))
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some((stage, started)) = self.0 {
            add(stage, started.elapsed().as_nanos() as u64);
        }
    }
}

/// One line: per stage the call count and mean milliseconds per call.
/// Empty when timing is off.
pub fn report() -> String {
    if !enabled() {
        return String::new();
    }
    let mut line = format!(
        "perf_timing version=1 threads={}",
        crate::parallel::threads()
    );
    for stage in STAGES {
        let count = COUNTS[stage as usize].load(Ordering::Relaxed);
        let total = TOTALS[stage as usize].load(Ordering::Relaxed);
        if count > 0 {
            line += &format!(
                " {stage:?}={count}x{:.4}ms",
                total as f64 / count as f64 / 1e6
            );
        }
    }
    let ticks = COUNTS[Stage::WorldTick as usize].load(Ordering::Relaxed);
    if ticks > 0 {
        let mean = |k: usize| WORLD[k].load(Ordering::Relaxed) as f64 / ticks as f64;
        line += &format!(
            " world_bodies={:.1} world_pieces={:.1} contacts_wheel={:.1} contacts_body={:.1} contacts_car={:.1} contacts_piece={:.1}",
            mean(0), mean(1), mean(2), mean(3), mean(4), mean(5)
        );
    }
    line
}
