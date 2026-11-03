//! Shared activation state only: no emission, beams, radiometry or renderer policy.
use bevy::prelude::*;
use grid_tools::time_of_day::{Headlights, Mode};

#[derive(Resource)]
pub(crate) struct State {
    pub headlights: Headlights,
    /// Separate tail/side-marker flag; absent when environment loading failed.
    /// It does not activate road projectors.
    #[allow(
        dead_code,
        reason = "activation handoff; marker rendering is a separate task"
    )]
    pub marker_lights: Option<bool>,
    mode: Mode,
    segment: u64,
    tick: u64,
}
impl State {
    pub(crate) fn new(mode: Mode, cars: usize, marker_lights: Option<bool>) -> Self {
        // Preserve the existing app's authored deterministic per-slot delay stream.
        let mut seed = 0x1e3a_u32;
        let headlights = Headlights::new(mode, cars, || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) as u8
        });
        Self {
            headlights,
            marker_lights,
            mode,
            segment: 0,
            tick: 0,
        }
    }
    pub(crate) fn segment_changed(&self, segment: u64) -> bool {
        self.segment != segment
    }
    pub(crate) fn advance(&mut self, segment: u64, ticks: u64, wanted: bool, since_start: f64) {
        if self.segment == segment && self.tick == ticks {
            return;
        }
        // Restart keeps the initialized per-car delays and uses the existing
        // since_start<0.25 snap. Never convert reset ticks into a huge delta.
        let advanced = if self.segment == segment {
            ticks.saturating_sub(self.tick)
        } else {
            0
        };
        self.segment = segment;
        self.tick = ticks;
        self.headlights.step(
            wanted,
            since_start,
            advanced as f64 * f64::from(grid_tools::source_vehicle::DELTA),
        );
    }
}
pub(crate) fn install(app: &mut App, mode: Mode, cars: usize) {
    let markers = app
        .world()
        .get_resource::<crate::track_environment::TrackEnvironment>()
        .map(|e| e.0.vehicle_lights());
    app.insert_resource(State::new(mode, cars, markers))
        .add_systems(FixedUpdate, tick.after(crate::drive_tick));
}
/// Le Mans retains sole ownership of its clock/timeline and calls advance once
/// from its existing tick, before logging. Ordinary routes use this system.
pub(crate) fn tick(
    driving: Res<crate::Driving>,
    event: Option<Res<crate::le_mans_event::LeMansEvent>>,
    mut state: ResMut<State>,
) {
    if event.is_some() {
        return;
    }
    let wanted = state.mode == Mode::ForcedNight;
    let ticks = driving.runtime.ticks;
    state.advance(
        driving.segment,
        ticks,
        wanted,
        ticks as f64 * f64::from(grid_tools::source_vehicle::DELTA),
    );
}
