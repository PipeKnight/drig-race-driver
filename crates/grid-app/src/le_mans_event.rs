//! Le Mans 24 Hours on Circuit de la Sarthe (docs/LE_MANS.md): the
//! time-limited race (race type 3, `grid_tools::race::Standings::with_time_limit`),
//! the class results and the time of day (`grid_tools::time_of_day`).
//!
//! - The race clock runs 24 h of day in the real time limit from 15:00 [V];
//!   on the Le Mans route outside a time-limited race it stays at 15:00.
//! - Headlights follow `enableHeadlights` with the original's fade and
//!   per-car delays [V]; this module only keeps their state, logs it and
//!   shows it on its HUD line. Drawing the lights, sky, sun and fog is the
//!   graphics side (docs/LE_MANS.md §3.5 lists the hand-off).
//! - Results [V 0x4EC820]: every car by progress (estimated cars times their
//!   class's `lemans_progress_adjustment`), the time on the race clock (24 h
//!   at the end; the original prints `1440:00.00`, the shared results table
//!   `1440:00.000`). The Class column comes from the series event
//!   (`post_race::CarClasses`, F413).
use bevy::prelude::*;
use grid_tools::time_of_day::{self, Headlights, Mode, Timeline};

#[derive(Resource)]
pub(crate) struct LeMansEvent {
    /// Real time limit (s) of the time-limited race; `None` on the Le Mans
    /// route in another race (the clock stays at 15:00).
    pub limit: Option<f64>,
    /// Clock multiplier, 86400 / limit.
    pub scale: Option<f64>,
    pub mode: Mode,
    /// Per car (0 = player): `vehicle_class` file string, sort order and
    /// progress adjustment.
    pub classes: Vec<grid_tools::series::LeMansClass>,
    pub timeline: Option<Timeline>,
    pub headlights: Headlights,
    /// Current time of day (day fraction).
    pub time_of_day: f64,
    lights_on: Option<bool>,
    last_hour: Option<u32>,
    last_tick: u64,
    reported: bool,
}

impl LeMansEvent {
    pub fn new(
        limit: Option<f64>,
        mode: Mode,
        classes: Vec<grid_tools::series::LeMansClass>,
        timeline: Option<Timeline>,
    ) -> Self {
        // AUTHORED: the per-car delays come from a fixed generator (the
        // original draws them with its C runtime rand at race init).
        let mut seed = 0x1e3a_u32;
        let headlights = Headlights::new(mode, classes.len().max(1), || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) as u8
        });
        Self {
            limit,
            scale: limit.map(time_of_day::clock_scale),
            mode,
            classes,
            timeline,
            headlights,
            time_of_day: time_of_day::day_fraction(0., None),
            lights_on: None,
            last_hour: None,
            last_tick: 0,
            reported: false,
        }
    }

    /// The class results of a time-limited race; other races keep `rows`.
    pub fn results(
        &self,
        standings: Option<&grid_tools::race::Standings>,
        rows: Vec<crate::post_race::Row>,
    ) -> Vec<crate::post_race::Row> {
        let (Some(standings), Some(scale)) = (standings, self.scale) else {
            return rows;
        };
        let adjustment: Vec<f64> = self.classes.iter().map(|c| c.progress_adjustment).collect();
        standings
            .lemans_results_order(&adjustment)
            .iter()
            .enumerate()
            .filter_map(|(place, &car)| {
                let mut row = rows.iter().find(|r| r.car == car)?.clone();
                row.position = place + 1;
                use grid_tools::race::Status;
                row.time = match (standings.status(car), standings.finished(car)) {
                    (Status::Finished | Status::Estimated, Some(t)) => {
                        crate::post_race::Cell::Total(t * scale)
                    }
                    (Status::Out(why), _) => crate::post_race::Cell::Out(why, 0),
                    _ => crate::post_race::Cell::Running,
                };
                Some(row)
            })
            .collect()
    }

    /// HUD line: clock, time remaining, headlights.
    pub fn hud(&self, race_time: Option<f64>) -> String {
        let clock = time_of_day::clock_text(self.time_of_day);
        let lights = if self.headlights.intensity() > 0. { "  lights on" } else { "" };
        match (self.limit, self.scale) {
            (Some(limit), Some(scale)) => {
                let left = (limit - race_time.unwrap_or(0.)).max(0.) * scale;
                format!(
                    "Le Mans {clock}  remaining {}{lights}",
                    time_of_day::clock_text(left / time_of_day::DAY_SECONDS)
                        .replacen("00:00:00", "24:00:00", usize::from(left >= time_of_day::DAY_SECONDS))
                )
            }
            _ => format!("Le Mans {clock}{lights}"),
        }
    }
}

#[derive(Component)]
pub(crate) struct LeMansHud;

pub(crate) fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        LeMansHud,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(18.),
            ..default()
        },
        TextColor(Color::srgb(1., 0.9, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.),
            left: Val::Percent(38.),
            ..default()
        },
    ));
}

pub(crate) fn show_hud(event: Res<LeMansEvent>, trial: Option<Res<crate::time_trial::Trial>>, driving: Res<crate::Driving>, mut text: Query<&mut Text, With<LeMansHud>>) {
    let race_time = trial.as_ref().and_then(|t| t.race_time(driving.runtime.ticks));
    let line = event.hud(race_time);
    for mut t in &mut text {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
}

/// After each fixed tick: the time of day and the headlights.
pub(crate) fn tick(
    driving: Res<crate::Driving>,
    trial: Option<Res<crate::time_trial::Trial>>,
    mut event: ResMut<LeMansEvent>,
) {
    let ticks = driving.runtime.ticks;
    let advanced = ticks.saturating_sub(event.last_tick);
    let restarted = ticks < event.last_tick;
    event.last_tick = ticks;
    if advanced == 0 && !restarted {
        return;
    }
    let race_time = trial.as_ref().and_then(|t| t.race_time(ticks));
    // The race clock stops when the race is over.
    let over = driving.opponents.as_ref().is_some_and(|o| o.standings.over());
    let clock = match (event.limit, race_time) {
        (Some(limit), Some(t)) => Some(t.min(limit)),
        _ => None,
    };
    if !(over && event.reported) {
        event.time_of_day = time_of_day::day_fraction(clock.unwrap_or(0.), event.scale.filter(|_| clock.is_some()));
    }
    let t = event.time_of_day;
    let wanted = match event.mode {
        Mode::Timeline => event
            .timeline
            .as_ref()
            .and_then(|tl| tl.flag("enableHeadlights", t))
            .unwrap_or(false),
        Mode::ForcedNight => true,
        Mode::None => false,
    };
    let dt = advanced as f64 * f64::from(grid_tools::source_vehicle::DELTA);
    let since_start = race_time.unwrap_or(0.);
    event.headlights.step(wanted, since_start, dt);
    let on = event.headlights.intensity() > 0.;
    let hour = (t * 24.) as u32;
    if event.lights_on != Some(on) || event.last_hour != Some(hour) {
        let lit = (0..event.classes.len().max(1))
            .filter(|&i| event.headlights.car(i).is_some())
            .count();
        log_line!(
            "drive_time_of_day version=1 segment={} tick={ticks} clock={} day={} headlights={} intensity={:.3} cars_lit={lit} sky={:?}",
            driving.segment,
            time_of_day::clock_text(t),
            time_of_day::is_day(t),
            if on { "on" } else { "off" },
            event.headlights.intensity(),
            event
                .timeline
                .as_ref()
                .and_then(|tl| tl.value("skyTime", t))
                .map(time_of_day::sky_textures),
        );
        event.lights_on = Some(on);
        event.last_hour = Some(hour);
    }
    if over && !event.reported {
        event.reported = true;
        if let (Some(o), Some(scale)) = (&driving.opponents, event.scale) {
            let adjustment: Vec<f64> = event.classes.iter().map(|c| c.progress_adjustment).collect();
            let sort: Vec<i32> = event.classes.iter().map(|c| c.sort_order).collect();
            let class_positions = o.standings.class_positions(&sort);
            for (place, car) in o.standings.lemans_results_order(&adjustment).into_iter().enumerate() {
                let time = o
                    .standings
                    .finished(car)
                    .map_or_else(|| "DNF".into(), |s| grid_tools::race::format_lemans_time(s * scale));
                log_line!(
                    "drive_lemans_result version=1 place={} car={car} name={} class={} class_place={} progress={:.3} time={time} status={:?}",
                    place + 1,
                    o.names.get(car).map_or("?", String::as_str),
                    event.classes.get(car).map_or("", |c| c.name.as_str()),
                    class_positions[car] + 1,
                    o.standings.progress(car),
                    o.standings.status(car),
                );
            }
        }
    } else if restarted {
        event.reported = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_tools::series::LeMansClass;

    fn class(name: &str, sort_order: i32, progress_adjustment: f64) -> LeMansClass {
        LeMansClass {
            name: name.into(),
            sort_order,
            progress_adjustment,
        }
    }

    #[test]
    fn the_hud_counts_down_the_day() {
        let event = LeMansEvent::new(
            Some(720.),
            Mode::Timeline,
            vec![class("gt1", 3, 0.92)],
            None,
        );
        assert_eq!(event.hud(Some(0.)), "Le Mans 15:00:00  remaining 24:00:00");
        assert_eq!(event.hud(Some(360.)), "Le Mans 15:00:00  remaining 12:00:00");
        let lap_race = LeMansEvent::new(None, Mode::Timeline, vec![], None);
        assert_eq!(lap_race.hud(Some(500.)), "Le Mans 15:00:00");
    }

    #[test]
    fn results_rank_by_progress_with_the_class_and_race_clock() {
        use crate::post_race::{Cell, Row};
        use grid_tools::race::{Out, Standings};
        let mut s = Standings::on_loop(8, 1000., &[1500., 2500., 3200.]).with_time_limit(100.);
        s.retire(2, Out::Wrecked);
        s.step_arcs(&[600., 600., 200.], 100., &[]);
        let event = LeMansEvent::new(
            Some(100.),
            Mode::Timeline,
            vec![class("gt2", 4, 0.84), class("lmp1", 1, 1.08), class("gt1", 3, 0.92)],
            None,
        );
        let rows: Vec<Row> = (0..3)
            .map(|car| Row {
                position: car + 1,
                car,
                name: format!("car{car}"),
                time: Cell::Running,
                estimated: false,
                best: None,
                vehicle: String::new(),
                score: None,
                touge: None,
                class: None,
                points: None,
            })
            .collect();
        let out = event.results(Some(&s), rows);
        // The wrecked car has the most progress and ranks first [V].
        assert_eq!(out.iter().map(|r| r.car).collect::<Vec<_>>(), [2, 1, 0]);
        assert_eq!(out[0].time, Cell::Out(Out::Wrecked, 0));
        assert_eq!(out[1].time, Cell::Total(86_400.));
        assert_eq!(out[2].position, 3);
    }
}
