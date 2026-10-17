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
//! distance (0x64D940).
//!
//! The HUD panel shows the original OSD elements as text (the dial2 glyphs,
//! meters and animations are not ported).
use bevy::prelude::*;
use grid_tools::drift::{Entry, Sample, Settings};

#[derive(Resource)]
pub(crate) struct DriftEvent {
    pub settings: Settings,
    pub flags: Vec<[f32; 3]>,
    pub entries: Vec<Entry>,
    pub laps: u32,
    /// AI difficulty level for the drift bias (`DifficultyBias`).
    ai_level: usize,
    segment: u64,
    last_tick: u64,
}

/// The AI difficulty setting picks the nearest `difficulty` row of career
/// rank 2 (`grid_tools::flashback::AI_RATIOS`) [D], which selects the
/// `DifficultyBias` row [I: 0x40B700 returns the star level].
pub(crate) fn ai_level(difficulty: f32) -> usize {
    grid_tools::flashback::star_level(difficulty)
}

impl DriftEvent {
    pub fn new(
        settings: Settings,
        flags: Vec<[f32; 3]>,
        cars: usize,
        laps: u32,
        ai_level: usize,
    ) -> Self {
        let mut event = Self {
            settings,
            flags,
            entries: Vec::new(),
            laps,
            ai_level,
            segment: 0,
            last_tick: 0,
        };
        event.reset(cars, 0, 0);
        event
    }

    /// Restart Race: fresh entries (0x64D8E0 resets and activates them).
    fn reset(&mut self, cars: usize, segment: u64, tick: u64) {
        self.entries = (0..cars)
            .map(|i| {
                if i == 0 {
                    Entry::new(true, None, true)
                } else {
                    Entry::new(true, Some(self.ai_level), false)
                }
            })
            .collect();
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

    /// Car indices in drift results order.
    pub fn order(&self, standings: &grid_tools::race::Standings) -> Vec<usize> {
        grid_tools::drift::classification(&self.entries, &standings.order())
    }
}

/// Loads `drift.xml` and the route's close-drift flags.
pub(crate) fn install(
    app: &mut App,
    root: &std::path::Path,
    route: &grid_tools::route::Route,
    cars: usize,
    laps: u32,
    ai_difficulty: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::load(root)?;
    let flags = grid_tools::drift::close_drift_flags(root, route)?;
    println!(
        "drive_drift_event version=1 discipline=drift_battle route={} cars={cars} laps={laps} close_drift_flags={} ai_level={}",
        route.path(),
        flags.len(),
        ai_level(ai_difficulty)
    );
    app.insert_resource(DriftEvent::new(
        settings,
        flags,
        cars,
        laps,
        ai_level(ai_difficulty),
    ))
    .add_systems(FixedUpdate, tick.after(crate::drive_tick))
    .add_systems(Update, (spawn, present).chain());
    Ok(())
}

/// After each fixed tick: scores every car, banks finished ones, and ends
/// the event with the race.
pub(crate) fn tick(driving: Res<crate::Driving>, mut event: ResMut<DriftEvent>) {
    let Some(o) = &driving.opponents else {
        return;
    };
    let cars: Vec<&grid_tools::source_vehicle::Runtime> = std::iter::once(&driving.runtime)
        .chain(o.multi.cars.iter().skip(1))
        .collect();
    if event.segment != driving.segment {
        event.reset(cars.len(), driving.segment, driving.runtime.ticks);
    }
    let event = &mut *event;
    let dt = grid_tools::source_vehicle::DELTA;
    let advanced = driving.runtime.ticks > event.last_tick;
    event.last_tick = driving.runtime.ticks;
    if advanced && !o.standings.over() {
        for (i, car) in cars.iter().enumerate() {
            let pose = car.pose();
            let v = car.velocity();
            let wheels = car.wheels().map(|w| {
                std::array::from_fn(|k| {
                    pose[12 + k]
                        + w.anchor[0] * pose[k]
                        + w.anchor[1] * pose[4 + k]
                        + w.anchor[2] * pose[8 + k]
                })
            });
            let sample = Sample {
                velocity: [v[0], v[1], v[2]],
                forward: [pose[8], pose[9], pose[10]],
                left: [pose[0], pose[1], pose[2]],
                wheels,
                progress: o.standings.laps_covered(i) as f32,
                position: o.standings.position(i).saturating_sub(1),
                cars: cars.len(),
                impact: o
                    .multi
                    .impacts(i)
                    .iter()
                    .map(|x| x.impact)
                    .fold(0., f32::max),
            };
            let entry = &mut event.entries[i];
            let before = (
                entry.scorer.combo,
                entry.scorer.rating,
                entry.scorer.combo_failed,
            );
            entry.update(&event.settings, &event.flags, &sample, dt);
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
    }
    // Finished or out: banked now (0x65F250); the race end: the rest.
    use grid_tools::race::Status;
    let over = o.standings.over();
    for (i, car) in cars.iter().enumerate() {
        let entry = &mut event.entries[i];
        if entry.done {
            continue;
        }
        let status = o.standings.status(i);
        let gone = matches!(status, Status::Finished | Status::Out(_));
        if gone || over {
            let v = car.velocity();
            let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            entry.finish_race(&event.settings, gone, speed, event.laps);
            log_line!(
                "drive_drift_event version=1 segment={} tick={} car={i} banked score={} estimate_needed={} status={status:?}",
                driving.segment,
                driving.runtime.ticks,
                entry.score(),
                entry.needs_estimate
            );
        }
    }
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

    /// Results rows in drift order, each with its score.
    pub fn results(
        &self,
        standings: &grid_tools::race::Standings,
        rows: Vec<crate::post_race::Row>,
    ) -> Vec<crate::post_race::Row> {
        self.order(standings)
            .into_iter()
            .enumerate()
            .filter_map(|(rank, car)| {
                let mut row = rows.iter().find(|r| r.car == car)?.clone();
                row.position = rank + 1;
                row.score = self.entries.get(car).map(|e| (e.score(), e.needs_estimate));
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
    fn hud_shows_rating_combo_timer_and_battle_multiplier() {
        let mut event = DriftEvent::new(Settings::default(), Vec::new(), 2, 2, 1);
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
