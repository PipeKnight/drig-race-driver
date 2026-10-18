//! Drift Battle: a lap race of the field in which every car is scored by
//! the original's drift scorer and the results are ordered by drift points
//! (docs/DISCIPLINES.md, "Drift Battle"). The race itself (grid, countdown,
//! laps, race end, AI field) is the circuit race of docs/RACE_MODE.md.
//!
//! Per car and fixed tick: the detector and scorer of
//! `grid_tools::drift`, with the race progress and position of
//! `grid_tools::race::Standings` and the car's largest solved contact
//! impact. A car that finishes or drops out is banked at once (0x65F250);
//! the race end banks the rest and scales a moving car's score to the event
//! distance (0x64D940); a zero score gets the virtual race's estimate
//! (`grid_tools::virtual_race`).
//!
//! Drift GP (docs/DISCIPLINES.md §7): the player alone on the time-trial run,
//! no position multiplier, and the free-race pool's other cars as virtual
//! rivals estimated when the run ends.
//!
//! The HUD panel shows the original OSD elements as text (the dial2 glyphs,
//! meters and animations are not ported).
use bevy::prelude::*;
use grid_tools::drift::{Entry, Sample, Settings};
use grid_tools::virtual_race::{Format, VirtualRace};

#[derive(Resource)]
pub(crate) struct DriftEvent {
    pub settings: Settings,
    pub flags: Vec<[f32; 3]>,
    pub entries: Vec<Entry>,
    pub laps: u32,
    /// drift_battle or drift_grand_prix.
    pub format: Format,
    /// Cars on the track; the entries after them are a Drift GP's virtual
    /// rivals.
    cars: usize,
    /// The result estimator of zero scores (0x64D940).
    race: VirtualRace,
    /// Its seed at the start, for Restart Race.
    race_start: VirtualRace,
    /// Driver skill per entry for the estimator (0x62B170: 1.0 without a
    /// driver row).
    performances: Vec<f32>,
    /// Route prop boxes of the close-drift flags (found at the first tick).
    flag_boxes: Option<Vec<usize>>,
    /// A Drift GP's solo run: the player's race progress along the line.
    solo: Option<Solo>,
    /// AI difficulty level for the drift bias (`DifficultyBias`).
    ai_level: usize,
    segment: u64,
    last_tick: u64,
}

/// The progress of a car alone on the track.
pub(crate) struct Solo {
    line: grid_tools::ai_driver::Line,
    start: [f32; 3],
    standings: grid_tools::race::Standings,
}

/// The AI difficulty setting picks the nearest `difficulty` row of career
/// rank 2 (`grid_tools::flashback::AI_RATIOS`) [D], which selects the
/// `DifficultyBias` row [I: 0x40B700 returns the star level].
pub(crate) fn ai_level(difficulty: f32) -> usize {
    grid_tools::flashback::star_level(difficulty)
}

impl DriftEvent {
    /// `cars` on the track and `rivals` virtual ones (Drift GP).
    pub fn new(
        settings: Settings,
        flags: Vec<[f32; 3]>,
        (format, cars, rivals): (Format, usize, usize),
        laps: u32,
        ai_level: usize,
        race: VirtualRace,
    ) -> Self {
        let mut event = Self {
            settings,
            flags,
            entries: Vec::new(),
            laps,
            format,
            cars,
            race_start: race.clone(),
            race,
            performances: vec![1.; cars + rivals],
            flag_boxes: None,
            solo: None,
            ai_level,
            segment: 0,
            last_tick: 0,
        };
        event.reset(0, 0);
        event
    }

    /// A Drift Battle of `cars` without base results (tests).
    #[cfg(test)]
    pub fn battle(
        settings: Settings,
        flags: Vec<[f32; 3]>,
        cars: usize,
        laps: u32,
        ai_level: usize,
    ) -> Self {
        let race = VirtualRace::new(None, grid_tools::virtual_race::difficulty(2, ai_level), 1);
        Self::new(
            settings,
            flags,
            (Format::DriftBattle, cars, 0),
            laps,
            ai_level,
            race,
        )
    }

    /// Restart Race: fresh entries (0x64D8E0 resets and activates them).
    /// The scorer's battle flag is format 2, Drift Battle (0x5F4E60).
    fn reset(&mut self, segment: u64, tick: u64) {
        let battle = self.format == Format::DriftBattle;
        self.entries = (0..self.performances.len())
            .map(|i| {
                if i == 0 {
                    Entry::new(battle, None, true)
                } else {
                    Entry::new(battle, Some(self.ai_level), false)
                }
            })
            .collect();
        self.race = self.race_start.clone();
        if let Some(solo) = &mut self.solo {
            solo.standings.reset(&solo.line, &[solo.start]);
        }
        self.segment = segment;
        self.last_tick = tick;
    }

    /// Scorers for a Flashback checkpoint (docs/FLASHBACK.md).
    pub fn checkpoint(&self) -> (Vec<Entry>, u64, u64) {
        (self.entries.clone(), self.segment, self.last_tick)
    }
    pub fn restore(&mut self, (entries, segment, last_tick): &(Vec<Entry>, u64, u64)) {
        self.entries = entries.clone();
        self.segment = *segment;
        self.last_tick = *last_tick;
    }

    /// Car indices in drift results order (virtual rivals after the cars).
    pub fn order(&self, standings: Option<&grid_tools::race::Standings>) -> Vec<usize> {
        let mut race_order = standings.map_or_else(|| vec![0], |s| s.order());
        race_order.extend(self.cars..self.entries.len());
        grid_tools::drift::classification(&self.entries, &race_order)
    }

    /// Banks an entry at its finish or the race end (0x65F250, 0x64D940).
    fn finish(&mut self, i: usize, gone: bool, speed: f32) {
        let performance = self.performances[i];
        self.entries[i].finish_race(
            &self.settings,
            (gone, speed, self.laps),
            &mut self.race,
            performance,
        );
    }
}

/// Loads `drift.xml`, the route's close-drift flags and the virtual race of
/// the route's free-race pool row. `cars` are on the track; a Drift GP adds
/// the pool's other cars as virtual rivals and runs the player alone.
pub(crate) fn install(
    app: &mut App,
    root: &std::path::Path,
    route: &grid_tools::route::Route,
    (format, cars, laps): (Format, usize, u32),
    ai_difficulty: f32,
    performances: &[f32],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut settings = Settings::load(root)?;
    // [V] the drift_freestyle race setup sets settings +0x1C8 (0x5F8705).
    settings.ignore_progress = format == Format::DriftFreestyle;
    let flags = grid_tools::drift::close_drift_flags(root, route)?;
    let pool = grid_tools::virtual_race::drift_pool(format, route.path());
    let level = ai_level(ai_difficulty);
    // [D] free-race pools: up to 8 cars in a battle, 2 in most Drift GPs.
    let rivals = match format {
        Format::DriftGrandPrix => pool.map_or(1, |p| p.max_cars as usize).saturating_sub(1),
        _ => 0,
    };
    // AUTHORED seed (the original seeds its generator from the clock).
    let race = VirtualRace::new(pool, grid_tools::virtual_race::difficulty(2, level), 0x5eed);
    println!(
        "drive_drift_event version=2 discipline={} route={} cars={cars} rivals={rivals} laps={laps} close_drift_flags={} ai_level={level} base_result={:?}",
        format.name(),
        route.path(),
        flags.len(),
        pool.map(|p| p.base_result)
    );
    let mut event = DriftEvent::new(settings, flags, (format, cars, rivals), laps, level, race);
    for (p, &v) in event.performances.iter_mut().zip(performances) {
        *p = v;
    }
    app.insert_resource(event)
        .add_systems(FixedUpdate, tick.after(crate::drive_tick))
        .add_systems(Update, (spawn, present).chain());
    Ok(())
}

/// After each fixed tick: scores every car, banks finished ones, and ends
/// the event with the race.
pub(crate) fn tick(
    driving: Res<crate::Driving>,
    trial: Option<Res<crate::time_trial::Trial>>,
    mut event: ResMut<DriftEvent>,
) {
    let event = &mut *event;
    match &driving.opponents {
        Some(o) => {
            let cars: Vec<&grid_tools::source_vehicle::Runtime> = std::iter::once(&driving.runtime)
                .chain(o.multi.cars.iter().skip(1))
                .collect();
            if event.segment != driving.segment {
                event.reset(driving.segment, driving.runtime.ticks);
            }
            let advanced = driving.runtime.ticks > event.last_tick;
            event.last_tick = driving.runtime.ticks;
            if advanced && !o.standings.over() {
                for (i, car) in cars.iter().enumerate() {
                    let contacts: Vec<_> = o
                        .multi
                        .contact_impacts(i)
                        .iter()
                        .map(|(x, other)| (x.impact, *other))
                        .collect();
                    let flags = event
                        .flag_boxes
                        .get_or_insert_with(|| o.multi.close_drift_flag_boxes());
                    let (impact, touched) = grid_tools::drift::split_contacts(&contacts, flags);
                    let sample = Sample::of(
                        car,
                        o.standings.laps_covered(i) as f32,
                        o.standings.position(i).saturating_sub(1),
                        cars.len(),
                        impact,
                    );
                    score(event, &driving, i, &sample, touched);
                }
            }
            // Finished or out: banked now (0x65F250); the race end: the rest.
            use grid_tools::race::Status;
            let over = o.standings.over();
            for (i, car) in cars.iter().enumerate() {
                let status = o.standings.status(i);
                let gone = matches!(status, Status::Finished | Status::Out(_));
                if !event.entries[i].done && (gone || over) {
                    bank(
                        event,
                        &driving,
                        i,
                        gone,
                        car.velocity(),
                        &format!("{status:?}"),
                    );
                }
            }
        }
        None => {
            // A Drift GP: the player alone; the race ends with the run.
            let Some(trial) = trial else {
                return;
            };
            if event.segment != driving.segment {
                event.reset(driving.segment, driving.runtime.ticks);
            }
            let advanced = driving.runtime.ticks > event.last_tick;
            event.last_tick = driving.runtime.ticks;
            let car = &driving.runtime;
            let p = car.pose();
            if event.solo.is_none()
                && let Some(line) = &driving.reset_line
            {
                let start = [p[12], p[13], p[14]];
                event.solo = Some(Solo {
                    standings: grid_tools::race::Standings::new(line, event.laps, start, &[start]),
                    line: line.clone(),
                    start,
                });
            }
            let over = trial.finished();
            if advanced && trial.driving() {
                let progress = event.solo.as_mut().map_or(0., |solo| {
                    solo.standings
                        .update(&solo.line, &[[p[12], p[13], p[14]]], 0., &[0]);
                    solo.standings.laps_covered(0) as f32
                });
                // No flag touches (AUTHORED: the solo world's prop boxes are
                // not labelled); packed road chunks are no collision.
                let impact = car
                    .impacts_off_track()
                    .iter()
                    .map(|x| x.impact)
                    .fold(0., f32::max);
                let sample = Sample::of(car, progress, 0, 1, impact);
                score(event, &driving, 0, &sample, false);
            }
            if over {
                for i in 0..event.entries.len() {
                    if !event.entries[i].done {
                        let v = if i == 0 { car.velocity() } else { [0.; 4] };
                        bank(
                            event,
                            &driving,
                            i,
                            i == 0,
                            v,
                            if i == 0 { "Finished" } else { "Virtual" },
                        );
                    }
                }
            }
        }
    }
}

/// One car's detector and scorer tick.
fn score(
    event: &mut DriftEvent,
    driving: &crate::Driving,
    i: usize,
    sample: &Sample,
    touched: bool,
) {
    let entry = &mut event.entries[i];
    if touched {
        entry.detector.touch_flag();
    }
    let before = (
        entry.scorer.combo,
        entry.scorer.rating,
        entry.scorer.combo_failed,
    );
    entry.update(
        &event.settings,
        &event.flags,
        sample,
        grid_tools::source_vehicle::DELTA,
    );
    let s = &entry.scorer;
    if i == 0 && before != (s.combo, s.rating, s.combo_failed) {
        log_line!(
            "drive_drift_event version=1 segment={} tick={} car=0 combo={} rating={} failed={} current={} total={} display={}",
            driving.segment,
            driving.runtime.ticks,
            s.combo,
            s.rating,
            s.combo_failed,
            s.current,
            s.total,
            s.display
        );
    }
}

fn bank(
    event: &mut DriftEvent,
    driving: &crate::Driving,
    i: usize,
    gone: bool,
    v: [f32; 4],
    status: &str,
) {
    let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    event.finish(i, gone, speed);
    let entry = &event.entries[i];
    log_line!(
        "drive_drift_event version=2 segment={} tick={} car={i} banked score={} estimated={} status={status}",
        driving.segment,
        driving.runtime.ticks,
        entry.score(),
        entry.estimated
    );
}

/// The drift scoreboard of the player (`post_race_drift_scoreboard`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Board {
    pub total: i64,
    pub drifts: i32,
    /// mph, seconds, points, mph, metres, degrees.
    pub fastest: f32,
    pub longest: f32,
    pub best_combo: f32,
    pub average_speed: f32,
    pub average_distance: f32,
    pub average_angle: f32,
    /// (rating label id, count, points) for each rating.
    pub ratings: Vec<(String, i32, f32)>,
}

impl DriftEvent {
    pub fn board(&self) -> Option<Board> {
        let e = self.entries.first()?;
        let s = &e.scorer;
        Some(Board {
            total: e.score(),
            // The stats screen's count comes from 0x4D1890 (not traced); the
            // banked rating counts are the drifts that scored [I].
            drifts: s.ratings.iter().map(|r| r.count).sum(),
            fastest: s.fastest_drift,
            longest: s.longest_drift,
            best_combo: s.best_combo,
            average_speed: s.average_speed,
            average_distance: s.average_distance,
            average_angle: s.average_angle,
            ratings: self
                .settings
                .types
                .iter()
                .zip(&s.ratings)
                .map(|(t, r)| (t.name.clone(), r.count, r.points))
                .collect(),
        })
    }

    /// Results rows in drift order, each with its score; a Drift GP's
    /// virtual rivals get rows of their own (named like the race's AI cars).
    pub fn results(
        &self,
        standings: Option<&grid_tools::race::Standings>,
        mut rows: Vec<crate::post_race::Row>,
    ) -> Vec<crate::post_race::Row> {
        if let Some(player) = rows.iter().find(|r| r.car == 0).cloned() {
            for car in self.cars..self.entries.len() {
                rows.push(crate::post_race::Row {
                    car,
                    name: format!("AI {car}"),
                    time: crate::post_race::Cell::Running,
                    estimated: false,
                    best: None,
                    ..player.clone()
                });
            }
        }
        self.order(standings)
            .into_iter()
            .enumerate()
            .filter_map(|(rank, car)| {
                let mut row = rows.iter().find(|r| r.car == car)?.clone();
                row.position = rank + 1;
                row.score = self.entries.get(car).map(|e| e.score());
                Some(row)
            })
            .collect()
    }
}

#[derive(Component)]
struct Panel;
#[derive(Component)]
struct PanelText;

fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<crate::ViewerCamera>>,
    mut done: Local<bool>,
) {
    let Some(camera) = camera.iter().next().filter(|_| !*done) else {
        return;
    };
    *done = true;
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                top: px(18.),
                left: percent(36.),
                width: percent(28.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(10.)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.04, 0.065, 0.78)),
            UiTargetCamera(camera),
        ))
        .with_children(|p| {
            p.spawn((
                PanelText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(22.),
                    ..default()
                },
                TextColor(Color::srgb(1., 0.85, 0.3)),
                TextLayout::justify(Justify::Center),
            ));
        });
}

/// The drift OSD as text: total score, the current drift's rating and
/// points, the combo multiplier with its timer (8 segments, like the
/// `driftcombo_timer` notes), the battle multiplier, close drifts and a
/// failed combo.
pub(crate) fn hud_text(event: &DriftEvent, labels: &crate::pause_menu::Labels) -> String {
    let Some(e) = event.entries.first() else {
        return String::new();
    };
    let s = &e.scorer;
    let mut text = format!(
        "{}  {}",
        labels.get("drift_scoreboard_total_score"),
        grouped(e.score())
    );
    if s.battle {
        text.push_str(&format!("   x{:.1}", e.detector.position_multiplier));
    }
    if s.was_drifting {
        let rating = s
            .rating
            .checked_sub(1)
            .and_then(|r| event.settings.types.get(r))
            .map_or("osd_drift", |t| t.name.as_str());
        text.push_str(&format!(
            "\n{}  {}",
            labels.get(rating),
            grouped(s.current as i64)
        ));
    }
    if s.combo > 0 {
        let lit = ((s.combo_timer / event.settings.combo_time).clamp(0., 1.) * 8.).ceil() as usize;
        text.push_str(&format!(
            "\nx{}  {}{}",
            s.combo,
            "|".repeat(lit),
            ".".repeat(8 - lit)
        ));
    }
    if s.close_awarded && s.was_drifting {
        text.push_str(&format!("\n+{}", grouped(s.last_close_bonus as i64)));
    }
    if s.combo_failed {
        text.push_str("\nX");
    }
    text
}

/// 12,345 style.
pub(crate) fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn present(
    event: Res<DriftEvent>,
    labels: Option<Res<crate::pause_menu::Labels>>,
    post: Option<Res<crate::post_race::PostRace>>,
    replay: Option<Res<crate::replay::Replay>>,
    mut panel: Query<&mut Visibility, With<Panel>>,
    mut text: Query<&mut Text, With<PanelText>>,
) {
    // Hidden behind the post-race screens and while a replay plays.
    let shown = !post.as_ref().is_some_and(|p| p.active())
        && !replay.as_ref().is_some_and(|r| r.owns_input());
    for mut visibility in &mut panel {
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
    let fallback = crate::pause_menu::Labels(None);
    let labels = labels.as_deref().unwrap_or(&fallback);
    let value = hud_text(&event, labels);
    for mut t in &mut text {
        if t.0 != value {
            t.0 = value.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_are_grouped_by_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1,000");
        assert_eq!(grouped(1234567), "1,234,567");
        assert_eq!(grouped(-12345), "-12,345");
    }

    #[test]
    fn ai_level_is_the_nearest_star_level() {
        assert_eq!(ai_level(0.606), 1);
        assert_eq!(ai_level(0.), 0);
        assert_eq!(ai_level(0.95), 4);
        assert_eq!(ai_level(2.), 5);
    }

    #[test]
    fn a_drift_gp_estimates_its_virtual_rivals_and_ranks_them() {
        use grid_tools::virtual_race::{difficulty, drift_pool};
        let pool = drift_pool(Format::DriftGrandPrix, "tracks/usa/long_beach/route_1");
        let race = VirtualRace::new(pool, difficulty(2, 1), 0x5eed);
        let mut event = DriftEvent::new(
            Settings::default(),
            Vec::new(),
            (
                Format::DriftGrandPrix,
                1,
                pool.unwrap().max_cars as usize - 1,
            ),
            2,
            1,
            race.clone(),
        );
        // No position multiplier outside a Drift Battle (0x5F4E60).
        assert_eq!(event.entries.len(), 2);
        assert!(event.entries.iter().all(|e| !e.scorer.battle));
        assert_eq!(
            hud_text(&event, &crate::pause_menu::Labels(None)),
            "drift_scoreboard_total_score  0"
        );
        // The player finishes with points; the rival never moved and gets
        // the estimate (rival skill 1.0: near base / top_time_factor × laps).
        event.entries[0].scorer.total = 5000.;
        event.entries[0].scorer.display = 5000.;
        event.finish(0, true, 30.);
        event.finish(1, false, 0.);
        assert_eq!(event.entries[0].score(), 5000);
        assert!(!event.entries[0].estimated && event.entries[1].estimated);
        let expected = race.clone().drift_score(1., 2) as i64;
        assert_eq!(event.entries[1].score(), expected);
        assert!((2 * 1_890_000 / 14 * 93 / 100..=2 * 1_890_000 / 14).contains(&expected));
        let player = crate::post_race::Row {
            position: 1,
            car: 0,
            name: "You".into(),
            time: crate::post_race::Cell::Total(200.),
            estimated: false,
            best: None,
            vehicle: "BMW 320si".into(),
            score: None,
        };
        let rows = event.results(None, vec![player]);
        assert_eq!(
            rows.iter()
                .map(|r| (r.position, r.car, r.score))
                .collect::<Vec<_>>(),
            [(1, 1, Some(expected)), (2, 0, Some(5000))]
        );
        assert_eq!(rows[0].name, "AI 1");
        // Restart Race: fresh entries and the same estimator draws.
        event.reset(1, 0);
        assert!(event.entries.iter().all(|e| !e.done));
        event.finish(1, false, 0.);
        assert_eq!(event.entries[1].score(), expected);
    }

    #[test]
    fn hud_shows_rating_combo_timer_and_battle_multiplier() {
        let mut event = DriftEvent::battle(Settings::default(), Vec::new(), 2, 2, 1);
        let labels = crate::pause_menu::Labels(None);
        assert_eq!(
            hud_text(&event, &labels),
            "drift_scoreboard_total_score  0   x1.0"
        );
        let e = &mut event.entries[0];
        e.scorer.was_drifting = true;
        e.scorer.rating = 2;
        e.scorer.current = 1234.;
        e.scorer.combo = 3;
        e.scorer.combo_timer = event.settings.combo_time / 2.;
        e.scorer.display = 3702.;
        e.detector.position_multiplier = 3.;
        assert_eq!(
            hud_text(&event, &labels),
            "drift_scoreboard_total_score  3,702   x3.0\nGreat Drift  1,234\nx3  ||||...."
        );
    }
}
