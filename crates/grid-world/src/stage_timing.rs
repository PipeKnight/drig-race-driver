//! Opt-in wall-clock timing inside the world advance, read by
//! `grid_tools::perf` (`GRID_PERF_TIMING`). Off unless [`enable`] was called;
//! timing never feeds back into the simulation.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

/// Timed parts of `grouped_physical_frame::tick`.
#[derive(Clone, Copy, Debug)]
pub enum Stage {
    /// Pose prediction of the active bodies before the contact provider.
    Predict,
    /// `contact_groups::calculate_carried_owned`: islands, sleep links, pool.
    Islands,
    /// Per island: the frame input built from the world records.
    IslandPack,
    /// Per island: the frame (solver, integration, deformation phases).
    IslandFrame,
    /// Per island: the results written back to the world records.
    IslandStore,
    /// IslandFrame of an island without a registered car.
    FramePlain,
    /// IslandFrame of a registered car alone (static peers only).
    FrameCar,
    /// IslandFrame of a group of several bodies with a registered car.
    FrameGroup,
    /// Car groups: the deformation limit phase (before the solve).
    GroupPrepare,
    /// Car groups: the bulk deformation phase (after the solve).
    GroupSolved,
    /// Car groups: contacts per group, recorded with [`count`] (the report's
    /// mean "ms" is the mean count).
    GroupContacts,
}

pub const STAGES: [Stage; 11] = [
    Stage::Predict,
    Stage::Islands,
    Stage::IslandPack,
    Stage::IslandFrame,
    Stage::IslandStore,
    Stage::FramePlain,
    Stage::FrameCar,
    Stage::FrameGroup,
    Stage::GroupPrepare,
    Stage::GroupSolved,
    Stage::GroupContacts,
];

static ENABLED: AtomicBool = AtomicBool::new(false);
static TOTALS: [AtomicU64; STAGES.len()] = [const { AtomicU64::new(0) }; STAGES.len()];
static COUNTS: [AtomicU64; STAGES.len()] = [const { AtomicU64::new(0) }; STAGES.len()];

pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

/// Total nanoseconds and calls of `stage`.
pub fn totals(stage: Stage) -> (u64, u64) {
    (
        TOTALS[stage as usize].load(Ordering::Relaxed),
        COUNTS[stage as usize].load(Ordering::Relaxed),
    )
}

/// Records `n` for a counting stage (as n milliseconds, so the report's mean is
/// the mean count); does nothing unless enabled.
pub fn count(stage: Stage, n: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        TOTALS[stage as usize].fetch_add(n as u64 * 1_000_000, Ordering::Relaxed);
        COUNTS[stage as usize].fetch_add(1, Ordering::Relaxed);
    }
}

/// Times `stage` until dropped; does nothing unless enabled.
pub struct Timer(Option<(Stage, Instant)>);

impl Timer {
    pub fn start(stage: Stage) -> Self {
        Self(
            ENABLED
                .load(Ordering::Relaxed)
                .then(|| (stage, Instant::now())),
        )
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some((stage, started)) = self.0 {
            TOTALS[stage as usize]
                .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            COUNTS[stage as usize].fetch_add(1, Ordering::Relaxed);
        }
    }
}
