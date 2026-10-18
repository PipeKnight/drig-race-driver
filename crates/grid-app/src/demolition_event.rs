//! Demolition Derby (docs/DEMOLITION_DERBY.md): a lap race on a Detroit
//! stadium route where a flipped player and stopped AI cars are
//! disqualified (`grid_tools::demolition`). The player's flip disqualifies
//! through `PlayerReset::demolition` (0x60D590); here the AI cars' not-moving
//! rule (0x630CF0) and the classification of disqualified cars.
use bevy::prelude::*;
use grid_tools::demolition::{NotMoving, Settings};

#[derive(Resource)]
pub(crate) struct DemolitionEvent {
    settings: Settings,
    not_moving: Vec<NotMoving>,
    segment: u64,
    last_tick: u64,
}

impl DemolitionEvent {
    pub fn new(settings: Settings, cars: usize) -> Self {
        Self {
            settings,
            not_moving: vec![NotMoving::default(); cars],
            segment: 0,
            last_tick: 0,
        }
    }
}

/// After each fixed tick: the AI cars' not-moving rule and the classification
/// of every newly disqualified car.
pub(crate) fn tick(mut driving: ResMut<crate::Driving>, mut event: ResMut<DemolitionEvent>) {
    let driving = &mut *driving;
    let event = &mut *event;
    if event.segment != driving.segment {
        // Restart Race: fresh records (0x64E030).
        event.segment = driving.segment;
        event.not_moving.iter_mut().for_each(|n| *n = NotMoving::default());
    }
    let ticks = driving.runtime.ticks;
    let advanced = ticks.saturating_sub(event.last_tick);
    event.last_tick = ticks;
    let player_out = driving.runtime.disqualified();
    let segment = driving.segment;
    let Some(o) = &mut driving.opponents else {
        return;
    };
    use grid_tools::race::{Out, Status};
    if player_out && o.standings.status(0) == Status::Racing {
        o.standings.retire(0, Out::Disqualified);
        log_line!("drive_demolition version=1 segment={segment} tick={ticks} car=0 event=disqualified reason=flipped");
    }
    if advanced == 0 {
        return;
    }
    for i in 1..o.multi.cars.len().min(event.not_moving.len()) {
        if o.standings.status(i) != Status::Racing {
            continue;
        }
        let v = o.multi.cars[i].velocity();
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let mut out = false;
        for _ in 0..advanced {
            out |= event.not_moving[i].tick(
                &event.settings,
                speed,
                f64::from(grid_tools::source_vehicle::DELTA),
            );
        }
        if out && o.multi.cars[i].disqualify() {
            o.standings.retire(i, Out::Disqualified);
            log_line!(
                "drive_demolition version=1 segment={segment} tick={ticks} car={i} event=disqualified reason=not-moving"
            );
        }
    }
}
