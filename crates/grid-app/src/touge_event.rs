//! Pro Touge (docs/TOUGE.md §7): two runs against one AI car on a Haruna
//! road, the second in the reverse direction from the swapped grid slots,
//! collision penalties per run (`grid_tools::touge`), and the results by
//! total time with the Round 1, Round 2 and Total Times columns
//! (`touge_round_one_results_table_title`, `touge_round_two_results_table_title`,
//! `touge_race_results_table_title` [D]).
//!
//! AUTHORED: the event moves on to run 2 by itself once both cars are done
//! (or 30 s after the player's finish, the other car's time estimated);
//! the original's round sequence (`touge_round_2_sequence`, the round
//! decision state) is not ported. Restart Race restarts the current run.
use bevy::prelude::*;
use grid_tools::touge::{Penalties, Settings, Standing, classify, penalised};

/// Seconds the event waits after the player's run-1 finish for the other
/// car before ending the run (AUTHORED).
pub const RUN_WAIT: f64 = 30.;

/// The second run, built at start-up: reverse subroute, swapped slots.
pub(crate) struct NextRun {
    pub runtime: grid_tools::source_vehicle::Runtime,
    pub geometry: Option<grid_tools::body_geometry::BodyGeometry>,
    pub opponents: Option<crate::Opponents>,
    pub reset_line: Option<grid_tools::ai_driver::Line>,
    /// Run length (m) of the reverse direction.
    pub length: f64,
}

#[derive(Resource)]
pub(crate) struct TougeEvent {
    settings: Settings,
    /// 0 or 1.
    pub run: usize,
    next: Option<NextRun>,
    pub penalties: [Penalties; 2],
    /// Finish time per run and car (s from GO; `None` unfinished).
    pub times: [[Option<f64>; 2]; 2],
    length: f64,
    waiting: f64,
    last_tick: u64,
    reported: bool,
}

impl TougeEvent {
    pub fn new(settings: Settings, length: f64, next: NextRun) -> Self {
        Self {
            settings,
            run: 0,
            next: Some(next),
            penalties: Default::default(),
            times: [[None; 2]; 2],
            length,
            waiting: 0.,
            last_tick: 0,
            reported: false,
        }
    }

    /// Both runs' standings in result order.
    pub fn standings(&self) -> Vec<Standing> {
        classify(
            (0..2)
                .map(|car| Standing {
                    car,
                    runs: [self.times[0][car], self.times[1][car]],
                    penalties: [0, 1].map(|r| self.penalties[car].seconds(&self.settings, r)),
                })
                .collect(),
        )
    }

    /// The results rows ordered and filled by both runs once run 2 is on.
    pub fn results(&self, rows: Vec<crate::post_race::Row>) -> Vec<crate::post_race::Row> {
        if self.run == 0 {
            return rows;
        }
        let standings = self.standings();
        let winner = standings.first().and_then(Standing::total);
        standings
            .iter()
            .enumerate()
            .filter_map(|(place, s)| {
                let mut row = rows.iter().find(|r| r.car == s.car)?.clone();
                row.position = place + 1;
                row.time = match (s.total(), winner) {
                    (Some(t), Some(w)) if place > 0 => crate::post_race::Cell::Gap(t - w),
                    (Some(t), _) => crate::post_race::Cell::Total(t),
                    _ => crate::post_race::Cell::Out(grid_tools::race::Out::Retired, 0),
                };
                row.estimated = false;
                row.touge = Some([s.run_total(0), s.run_total(1)]);
                Some(row)
            })
            .collect()
    }
}

fn damaged(runtime: &grid_tools::source_vehicle::Runtime) -> bool {
    (runtime.state.car.owner.vehicle[0xba5 / 4] >> 8) & 255 != 0
}

/// After each fixed tick: penalties, run times and the switch to run 2.
pub(crate) fn tick(
    mut driving: ResMut<crate::Driving>,
    trial: Option<ResMut<crate::time_trial::Trial>>,
    mut event: ResMut<TougeEvent>,
) {
    let Some(mut trial) = trial else {
        return;
    };
    let driving = &mut *driving;
    let event = &mut *event;
    let ticks = driving.runtime.ticks;
    let advanced = ticks.saturating_sub(event.last_tick);
    event.last_tick = ticks;
    let Some(o) = &mut driving.opponents else {
        return;
    };
    if advanced > 0 && trial.race_time(ticks).is_some() {
        if !o.multi.contact_pairs().is_empty() {
            let hit = damaged(&driving.runtime) || o.multi.cars.iter().skip(1).any(damaged);
            let at = |i| o.standings.laps_covered(i) * event.length;
            let (a, b) = penalised(at(0), at(1));
            for (i, on) in [a, b].into_iter().enumerate() {
                if on {
                    event.penalties[i].contact(&event.settings, hit);
                }
            }
        }
        for (i, p) in event.penalties.iter_mut().enumerate() {
            let racing = o.standings.status(i) == grid_tools::race::Status::Racing;
            for _ in 0..advanced {
                p.tick(f64::from(grid_tools::source_vehicle::DELTA), event.run, racing);
            }
        }
    }
    for car in 0..2 {
        event.times[event.run][car] = o.standings.finished(car);
    }
    if event.run == 1 && trial.finished() && o.standings.over() && !event.reported {
        event.reported = true;
        for (place, s) in event.standings().iter().enumerate() {
            let f = |t: Option<f64>| t.map_or_else(|| "DNF".into(), grid_tools::race::format_time);
            log_line!(
                "drive_touge version=1 event=result place={} car={} round1={} round2={} penalties={:?} total={}",
                place + 1,
                s.car,
                f(s.run_total(0)),
                f(s.run_total(1)),
                s.penalties,
                f(s.total())
            );
        }
    }
    if event.run != 0 || !trial.finished() {
        return;
    }
    // Run 1 is over once both cars are done, or after the wait.
    if !o.standings.over() {
        event.waiting += f64::from(advanced as f32 * grid_tools::source_vehicle::DELTA);
        if event.waiting < RUN_WAIT {
            return;
        }
        let speeds = o.speeds(&driving.runtime);
        let time = trial.race_time(ticks).unwrap_or(0.);
        o.standings.finalize(&speeds, time, &[0]);
        for car in 0..2 {
            event.times[0][car] = o.standings.finished(car);
        }
    }
    let Some(next) = event.next.take() else {
        return;
    };
    log_line!(
        "drive_touge version=1 event=run_end run=1 times={:?} penalties={:?}",
        event.times[0],
        [0, 1].map(|car| event.penalties[car].counts[0])
    );
    driving.runtime = next.runtime;
    driving.geometry = next.geometry;
    driving.opponents = next.opponents;
    driving.reset_line = next.reset_line;
    driving.auto_reset.reset();
    driving.segment += 1;
    driving.last_controls = None;
    driving.failure = None;
    driving.notices.clear();
    event.length = next.length;
    event.run = 1;
    event.last_tick = 0;
    for p in &mut event.penalties {
        p.reset_window();
    }
    trial.reset();
    log_line!("drive_touge version=1 event=run_start run=2 subroute=1 slots=swapped");
}
