//! Shared activation state only: no emission, beams, radiometry or renderer policy.
use bevy::prelude::*;
use grid_tools::time_of_day::{Headlights, Mode};

#[derive(Resource)]
pub(crate) struct State {
    pub headlights: Headlights,
    /// Static source lamp request; absent when environment loading failed.
    /// The lamp adapter consumes it only in static Mode::None. It does not activate road projectors.
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
    /// Existing app activation adapter. Native +128C focus-owner delay override remains unjoined.
    /// This is a boolean request, never the projector weight used as emissive strength.
    pub(crate) fn lamp_request(&self, slot: usize) -> Result<bool, &'static str> {
        if slot >= self.headlights.delays().len() { return Err("lamp activation slot unavailable"); }
        match self.mode {
            Mode::None => self.marker_lights.ok_or("source static marker initialization unavailable"),
            Mode::Timeline | Mode::ForcedNight => Ok(self.headlights.car(slot).is_some()),
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

#[cfg(test)]
mod lamp_request_tests {
    use super::*;
    #[test]
    fn consumed_marker_and_night_boolean_do_not_copy_projector_strength() {
        let static_on=State::new(Mode::None,2,Some(true));
        assert_eq!(static_on.lamp_request(0),Ok(true));assert_eq!(static_on.headlights.car(0),None);
        assert!(static_on.lamp_request(2).is_err());
        assert!(State::new(Mode::None,1,None).lamp_request(0).is_err());
        let mut night=State::new(Mode::Timeline,2,Some(true));
        assert_eq!(night.lamp_request(0),Ok(false),"night does not OR static markers");
        night.advance(0,625,true,10.);
        for slot in 0..2 { assert_eq!(night.lamp_request(slot).unwrap(),night.headlights.car(slot).is_some()); }
        let before=night.headlights.delays().to_vec();let _=night.lamp_request(1);assert_eq!(night.headlights.delays(),before);
    }
}
