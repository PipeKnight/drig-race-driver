//! The original's in-race HUD layout ([research](../../../docs/RACE_HUD.md)).
//!
//! Structure [D]: `Osd/sceneconfig.xml` (`osd_right`: dial with speedo,
//! odometer, gear, rpm needle, red zone, gear and rev lights, mph/km/h,
//! pedal gauges and damage icons, then the race position group with up to
//! four split pills; `osd_left`: the central message/countdown box, the
//! minimap with its flag, and the lap group LAP n/total, RACE, LAP, BEST;
//! `osd_top`: wrong way and penalty pills) and `Osd/config.xml` (the four
//! selectable groups dial, minimap, raceposition, lapinfo; dial revlimit
//! 0.95, gearchange 0.90, rpm display ranges; split display 5..10 s; wrong
//! way 4 s on, 2 s off). Labels come from the user's language table.
//!
//! AUTHORED presentation: plain panels with text and bars instead of the
//! animated `dial2.pssg` glyphs. The view is computed by pure functions
//! ([`lap_info`], [`position`], [`dial`], [`WrongWay`]) so it can be tested.
use crate::pause_menu::{Labels, PauseMenu};
use bevy::prelude::*;

/// Marks that the original-layout HUD is installed; the text HUD then shows
/// only the heading and status lines.
#[derive(Resource)]
pub struct Active;

/// The lap group: LAP n/total, RACE time, current LAP time, BEST lap.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LapInfo {
    pub lap: u32,
    pub total: Option<u32>,
    pub race: Option<f64>,
    pub current: Option<f64>,
    pub best: Option<f64>,
}

/// Lap group of the event. A timed event counts laps from its own state
/// (completed + 1, capped at the total); free practice shows the lap timer's.
pub fn lap_info(
    trial: Option<&grid_tools::time_trial::State>,
    lap: Option<&grid_world::lap_timing::State>,
) -> LapInfo {
    use grid_tools::time_trial::Phase;
    let best = lap.and_then(|l| l.best_lap);
    let current = lap.and_then(|l| l.elapsed());
    match trial {
        Some(t) => {
            let total = t.target();
            let (lap_number, race) = match t.phase() {
                Phase::Countdown(_) => (1, None),
                Phase::Running => ((t.completed_laps() as u32 + 1).min(total), Some(t.elapsed())),
                Phase::Finished(r) => (total, Some(r.seconds)),
            };
            LapInfo {
                lap: lap_number,
                total: Some(total),
                race,
                current: matches!(t.phase(), Phase::Running).then_some(current).flatten(),
                best,
            }
        }
        None => LapInfo {
            lap: lap.map_or(1, |l| l.lap_number()),
            total: None,
            race: None,
            current,
            best,
        },
    }
}

/// One split pill: running position, name and the time text at the last
/// split (empty while the car has not reached it).
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    pub position: usize,
    pub name: String,
    pub time: String,
    pub player: bool,
}

/// Race time of each car at every split point it has passed, in order: the
/// lap timing's time splits and the line (the original asks each participant
/// for its time at a (split, lap) pair, 0x69F010 [D structure; I that the
/// split points are the lap timing's time splits and the line]).
#[derive(Clone, Debug, Default)]
pub struct SplitTimes(Vec<Vec<f64>>);

impl SplitTimes {
    pub fn record(&mut self, car: usize, time: f64) {
        if self.0.len() <= car {
            self.0.resize(car + 1, Vec::new());
        }
        self.0[car].push(time);
    }
    /// Split points `car` has passed.
    pub fn count(&self, car: usize) -> usize {
        self.0.get(car).map_or(0, Vec::len)
    }
    /// Race time of `car` at the split with this index (0 = the first).
    pub fn time(&self, car: usize, split: usize) -> Option<f64> {
        self.0.get(car)?.get(split).copied()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// The OSD's lap and race timers use the split clock's format (0x69E050 for the lap
/// time, 0x6B3D30 for the race time), capped at 1:39:59.999, that is
/// 99:59.99 (0x69E050 [D]).
pub fn osd_clock(seconds: f64) -> String {
    split_clock(seconds.min(99. * 60. + 59.999))
}

/// `%02u:%02u.%02u` (0x69EA10): minutes, seconds, hundredths [D format].
/// Times are rounded to whole milliseconds first (the original keeps 64-bit
/// millisecond times [I]), then the hundredths are truncated.
pub fn split_clock(seconds: f64) -> String {
    let hundredths = (seconds.abs() * 1000.).round() as u64 / 10;
    format!("{:02}:{:02}.{:02}", hundredths / 6000, hundredths / 100 % 60, hundredths % 100)
}

/// The rows shown (0-based race positions), from the player's position `p`
/// of `n` cars (0x6BB9E0 [D]): the first four for a leading or second
/// player, otherwise the leader and the three around the player, the last
/// three for the last car.
pub fn split_rows(p: usize, n: usize) -> Vec<usize> {
    if n <= 4 || p <= 1 {
        return (0..n.min(4)).collect();
    }
    let first = if p + 1 >= n { p - 2 } else { p - 1 };
    std::iter::once(0).chain(first..first + 3).collect()
}

/// `playerspositioninterface splitdisplay maxduration="10.0"
/// minduration="5.0"` [D]: the pills show for the maximum after the player
/// passes a split point; a car that reaches the split later raises the time
/// left to at least the minimum (0x6BBB70, 0x6A6A00 [D structure]).
pub const SPLIT_MAX: f32 = 10.;
pub const SPLIT_MIN: f32 = 5.;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SplitDisplay {
    /// The split shown (index into each car's split times) and time left.
    split: Option<usize>,
    timer: f32,
    /// Cars whose time at the shown split was known at the last update.
    filled: Vec<bool>,
}

impl SplitDisplay {
    /// One frame: returns the split shown, if any.
    pub fn update(&mut self, times: &SplitTimes, player: usize, cars: usize, dt: f32) -> Option<usize> {
        let passed = times.count(player);
        let known = |s: usize| (0..cars).map(|c| times.time(c, s).is_some()).collect::<Vec<_>>();
        if passed == 0 {
            *self = Self::default();
            return None;
        }
        if self.split != Some(passed - 1) {
            self.split = Some(passed - 1);
            self.timer = SPLIT_MAX;
            self.filled = known(passed - 1);
            return self.split;
        }
        if self.timer <= 0. {
            return None;
        }
        self.timer -= dt;
        let now = known(self.split.unwrap_or(0));
        if now.iter().zip(&self.filled).any(|(n, f)| *n && !*f) {
            self.timer = self.timer.max(SPLIT_MIN);
        }
        self.filled = now;
        (self.timer > 0.).then_some(self.split).flatten()
    }
}

/// Position group (n / total) and the split pills: the rows of
/// [`split_rows`], each with its time at the shown split. The first row with
/// a time shows it as a race clock, the others their difference to it
/// (0x69F010, 0x69EAB0: `%s%02u:%02u.%02u` with `+` or `-` [D]).
pub fn position(
    standings: &grid_tools::race::Standings,
    names: &[String],
    player: usize,
    times: &SplitTimes,
    split: Option<usize>,
) -> (usize, usize, Vec<Split>) {
    let order = standings.order();
    let at = order.iter().position(|&c| c == player).unwrap_or(0);
    let name = |c: usize| names.get(c).cloned().unwrap_or_default();
    let mut base = None;
    let rows = split_rows(at, order.len())
        .into_iter()
        .map(|i| {
            let car = order[i];
            let time = match split.and_then(|s| times.time(car, s)) {
                None => String::new(),
                Some(t) => match base {
                    None => {
                        base = Some(t);
                        split_clock(t)
                    }
                    Some(b) => format!("{}{}", if t >= b { "+" } else { "-" }, split_clock(t - b)),
                },
            };
            Split {
                position: i + 1,
                name: name(car),
                time,
                player: car == player,
            }
        })
        .collect();
    (at + 1, standings.len(), rows)
}

/// `Osd/config.xml` dialinterface [D]: rev limit light at 0.95 of the limit,
/// gear-change light at 0.90; display ranges 12000, 10000 and 8000 rpm, each
/// with a 2000 rpm red line.
pub const REV_LIGHT: f32 = 0.95;
pub const GEAR_LIGHT: f32 = 0.90;
pub const RPM_RANGES: [f32; 3] = [8000., 10000., 12000.];
pub const RED_LINE: f32 = 2000.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dial {
    pub rpm: f32,
    /// Scale maximum (rpm).
    pub scale: f32,
    /// Red zone start (rpm).
    pub red: f32,
    pub gear_light: bool,
    pub rev_light: bool,
}

/// Engine speed words +0x1894 (current) and +0x17A4 (limit) are rad/s [I];
/// the scale is the smallest display range at or above the limit [I] and the
/// red zone its last 2000 rpm [I].
pub fn dial(engine: f32, limit: f32) -> Dial {
    let to_rpm = 60. / std::f32::consts::TAU;
    let (rpm, max) = (engine.max(0.) * to_rpm, limit.max(0.) * to_rpm);
    let scale = RPM_RANGES
        .into_iter()
        .find(|&r| r >= max)
        .unwrap_or(RPM_RANGES[2]);
    let fraction = if max > 0. { rpm / max } else { 0. };
    Dial {
        rpm,
        scale,
        red: scale - RED_LINE,
        gear_light: fraction >= GEAR_LIGHT,
        rev_light: fraction >= REV_LIGHT,
    }
}

/// `minimapinterface wrongway wrongwayinterval="4.0" rightwayinterval="2.0"`
/// [D]: the pill appears after 4 s going the wrong way and goes after 2 s the
/// right way [I for the reading of the two intervals].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WrongWay {
    pub shown: bool,
    timer: f32,
}

pub const WRONG_WAY_ON: f32 = 4.;
pub const WRONG_WAY_OFF: f32 = 2.;

impl WrongWay {
    pub fn update(&mut self, wrong: bool, dt: f32) -> bool {
        if wrong != self.shown {
            self.timer += dt;
            let limit = if wrong { WRONG_WAY_ON } else { WRONG_WAY_OFF };
            if self.timer >= limit {
                self.shown = wrong;
                self.timer = 0.;
            }
        } else {
            self.timer = 0.;
        }
        self.shown
    }
}

/// After each completed lap the lap timer shows that lap's time for this
/// long, then the running lap again; a new personal best's record pill
/// stays up until then (0x6B85C0 sets 5 s when the lap count grows and
/// clears it when the count drops; 0x6B7A90 shows the pill, 0x6B2D00 hides
/// it once the time is out [D]).
pub const LAP_HOLD: f32 = 5.;

/// The lap group's completed-lap hold and record pill.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LapHold {
    laps: Option<u32>,
    best: Option<f64>,
    timer: f32,
    record: bool,
    /// The lap that last beat the world record this session.
    beaten: Option<f64>,
    world: bool,
}

/// The record pill's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecordPill {
    #[default]
    Off,
    /// `osd_pb`: a new personal best.
    Personal,
    /// `osd_wr`: a new best that also beats the world record.
    World,
}

impl LapHold {
    /// One frame with the lap timer's completed laps, its last lap time, best
    /// lap and the world record to beat (seconds). Returns the completed lap
    /// time to show instead of the running one, and the record pill.
    pub fn update(
        &mut self,
        laps: Option<u32>,
        last: Option<f64>,
        best: Option<f64>,
        world_record: Option<f64>,
        dt: f32,
    ) -> (Option<f64>, RecordPill) {
        // A new best is the personal best message [I: the port's session
        // best; the original's PB source is not traced]. It is also a world
        // record when it beats the track's `world_record` row for the car's
        // grade (0x5C3890 [D]), or the lap that beat that row earlier this
        // session ([I]: the original keeps a beaten record as the one to
        // beat; the port does not save it).
        if best != self.best {
            self.record |= best.is_some();
            if let Some(b) = best {
                let target = match (world_record.filter(|w| *w > 0.), self.beaten) {
                    (Some(w), Some(x)) => Some(w.min(x)),
                    (w, x) => w.or(x),
                };
                if target.is_some_and(|t| b < t) {
                    self.world = true;
                    self.beaten = Some(b);
                }
            }
            self.best = best;
        }
        match (laps, self.laps) {
            (Some(n), Some(o)) if n > o => self.timer = LAP_HOLD,
            (Some(n), Some(o)) if n < o => self.timer = 0.,
            (None, _) => self.timer = 0.,
            _ => {}
        }
        self.laps = laps;
        let held = self.timer > 0.;
        self.record &= held;
        self.world &= self.record;
        self.timer = (self.timer - dt).max(0.);
        let pill = match (self.record, self.world) {
            (true, true) => RecordPill::World,
            (true, false) => RecordPill::Personal,
            _ => RecordPill::Off,
        };
        (last.filter(|_| held), pill)
    }
}

/// The world record of the race's track and the player's car (seconds):
/// the `world_record` row whose `track_model_id` is the route's track model
/// and whose `vehicle_grade_id` is the player vehicle's (0x5C3890 [D]).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldRecord(pub Option<f64>);

impl WorldRecord {
    /// `route` is `tracks/<folder>/<file>/<route>`, `car` the vehicle's
    /// folder stem (`vehicle.file_string`).
    pub fn from_database(db: &grid_formats::database::Database, route: &str, car: &str) -> Option<f64> {
        let model = crate::race_osd::track_model(db, route)?.int("id")?;
        let grade = db
            .table("vehicle")?
            .rows()
            .find(|r| r.text("file_string").is_some_and(|f| f.eq_ignore_ascii_case(car)))?
            .int("vehicle_grade_id")?;
        db.table("world_record")?
            .rows()
            .find(|r| r.int("track_model_id") == Some(model) && r.int("vehicle_grade_id") == Some(grade))
            .and_then(|r| r.float("world_record"))
            .map(f64::from)
            .filter(|w| *w > 0.)
    }

    /// [`Self::from_database`] from the installation at `root`.
    pub fn load(root: &std::path::Path, route: &str, car: &str) -> Self {
        let db = std::fs::read(root.join("database/schema.bin"))
            .and_then(|s| std::fs::read(root.join("database/database.bin")).map(|d| (s, d)))
            .ok()
            .and_then(|(s, d)| grid_formats::database::read(&s, &d).ok());
        Self(db.and_then(|db| Self::from_database(&db, route, car)))
    }
}

#[derive(Resource, Default)]
struct HudState {
    wrong_way: WrongWay,
    split: SplitDisplay,
    odometer: f64,
    last: Option<(u64, u64)>,
    lap_hold: LapHold,
}

#[derive(Component)]
pub(crate) enum Part {
    LapGroup,
    LapText,
    PositionGroup,
    PositionText,
    Splits,
    Dial,
    Speed,
    Gear,
    RpmFill,
    RpmRed,
    GearLight,
    RevLight,
    Throttle,
    Brake,
    Odometer,
    Message,
    WrongWay,
    Minimap,
    MinimapFlag,
}

#[derive(Component)]
struct CarDot(usize);

pub fn install(app: &mut App) {
    let osd = app.world().contains_resource::<crate::osd_scene::Installed>();
    app.insert_resource(Active)
        .init_resource::<HudState>()
        .init_resource::<HudView>();
    if osd {
        app.add_systems(Update, ((crate::speed_advice::update, frame, crate::race_osd::drive, crate::drift_osd::drive, crate::race_osd::minimap).chain(), crate::race_osd::place_help));
        app.add_systems(Update, crate::name_tag::team_tag.after(crate::race_osd::minimap).after(crate::drive_present));
    } else {
        app.add_systems(Update, (spawn, frame, update).chain());
    }
}

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

const PANEL: Color = Color::srgba(0.025, 0.04, 0.065, 0.78);

fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<crate::ViewerCamera>>,
    driving: Option<Res<crate::Driving>>,
    mut done: Local<bool>,
) {
    let Some(camera) = camera.iter().next().filter(|_| !*done) else {
        return;
    };
    let Some(driving) = driving else {
        return;
    };
    *done = true;
    let absolute = |left: Option<f32>, right: Option<f32>, top: Option<f32>, bottom: Option<f32>| Node {
        position_type: PositionType::Absolute,
        left: left.map_or(Val::Auto, px),
        right: right.map_or(Val::Auto, px),
        top: top.map_or(Val::Auto, px),
        bottom: bottom.map_or(Val::Auto, px),
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(px(10.)),
        row_gap: px(4.),
        ..default()
    };
    // osd_left: lap group.
    commands
        .spawn((Part::LapGroup, absolute(Some(18.), None, Some(230.), None), BackgroundColor(PANEL), UiTargetCamera(camera)))
        .with_children(|p| {
            p.spawn((Part::LapText, Text::new(""), font(20.), TextColor(Color::WHITE)));
        });
    // osd_right: race position group with its split pills.
    commands
        .spawn((Part::PositionGroup, absolute(None, Some(18.), Some(18.), None), BackgroundColor(PANEL), UiTargetCamera(camera)))
        .with_children(|p| {
            p.spawn((Part::PositionText, Text::new(""), font(34.), TextColor(Color::WHITE)));
            p.spawn((Part::Splits, Text::new(""), font(17.), TextColor(Color::srgb(0.85, 0.9, 0.95))));
        });
    // osd_right: the dial.
    let bar = |part: Part, colour: Color| {
        (
            part,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.),
                top: px(0.),
                height: percent(100.),
                width: percent(0.),
                ..default()
            },
            BackgroundColor(colour),
        )
    };
    commands
        .spawn((Part::Dial, absolute(None, Some(18.), None, Some(18.)), BackgroundColor(PANEL), UiTargetCamera(camera)))
        .with_children(|p| {
            p.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(14.),
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|row| {
                row.spawn((Part::Speed, Text::new(""), font(40.), TextColor(Color::WHITE)));
                row.spawn((Part::Gear, Text::new(""), font(40.), TextColor(Color::srgb(1., 0.78, 0.2))));
            });
            // rpm needle as a bar, with its red zone.
            p.spawn((
                Node {
                    width: px(240.),
                    height: px(14.),
                    ..default()
                },
                BackgroundColor(Color::srgba(1., 1., 1., 0.12)),
            ))
            .with_children(|b| {
                b.spawn(bar(Part::RpmRed, Color::srgba(0.9, 0.12, 0.1, 0.55)));
                b.spawn(bar(Part::RpmFill, Color::srgb(0.95, 0.95, 0.95)));
            });
            p.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(10.),
                ..default()
            })
            .with_children(|row| {
                row.spawn((Part::GearLight, Text::new("GEAR"), font(14.), TextColor(Color::srgba(1., 1., 1., 0.25))));
                row.spawn((Part::RevLight, Text::new("REV"), font(14.), TextColor(Color::srgba(1., 1., 1., 0.25))));
                row.spawn((Part::Odometer, Text::new(""), font(14.), TextColor(Color::srgb(0.75, 0.8, 0.85))));
            });
            for part in [Part::Throttle, Part::Brake] {
                let colour = if matches!(part, Part::Throttle) {
                    Color::srgb(0.3, 0.85, 0.35)
                } else {
                    Color::srgb(0.9, 0.25, 0.2)
                };
                p.spawn((
                    Node {
                        width: px(240.),
                        height: px(5.),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1., 1., 1., 0.08)),
                ))
                .with_children(|b| {
                    b.spawn(bar(part, colour));
                });
            }
        });
    // Central message box (countdown) and the wrong-way pill.
    commands.spawn((
        Part::Message,
        Node {
            position_type: PositionType::Absolute,
            top: percent(30.),
            width: percent(100.),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Text::new(""),
        font(64.),
        TextLayout::justify(Justify::Center),
        TextColor(Color::srgb(1., 0.85, 0.2)),
        UiTargetCamera(camera),
    ));
    commands.spawn((
        Part::WrongWay,
        Node {
            position_type: PositionType::Absolute,
            top: px(18.),
            width: percent(100.),
            ..default()
        },
        Text::new(""),
        font(32.),
        TextLayout::justify(Justify::Center),
        TextColor(Color::srgb(0.95, 0.2, 0.15)),
        UiTargetCamera(camera),
    ));
    // osd_left: minimap (the AI racing line and every car).
    const MAP: f32 = 190.;
    let line = driving.reset_line.as_ref().map(|l| l.points().to_vec()).unwrap_or_default();
    let bounds = map_bounds(&line);
    let cars = driving.opponents.as_ref().map_or(1, |o| o.multi.cars.len());
    commands
        .spawn((
            Part::Minimap,
            Node {
                position_type: PositionType::Absolute,
                left: px(18.),
                bottom: px(18.),
                width: px(MAP + 20.),
                height: px(MAP + 20.),
                ..default()
            },
            BackgroundColor(PANEL),
            UiTargetCamera(camera),
        ))
        .with_children(|m| {
            let step = line.len().div_ceil(160).max(1);
            for p in line.iter().step_by(step) {
                let (x, y) = map_point(*p, bounds, MAP);
                m.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(10. + x - 1.5),
                        top: px(10. + y - 1.5),
                        width: px(3.),
                        height: px(3.),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.85, 0.9, 0.95, 0.6)),
                ));
            }
            for car in (0..cars).rev() {
                m.spawn((
                    CarDot(car),
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(8.),
                        height: px(8.),
                        ..default()
                    },
                    BackgroundColor(if car == 0 {
                        Color::srgb(1., 0.78, 0.2)
                    } else {
                        Color::srgb(0.3, 0.6, 1.)
                    }),
                ));
            }
            m.spawn((
                Part::MinimapFlag,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(8.),
                    top: px(4.),
                    ..default()
                },
                Text::new(""),
                font(14.),
                TextColor(Color::WHITE),
            ));
        });
}

/// (min x, min z, size) of the line in the map plane.
pub fn map_bounds(line: &[[f32; 3]]) -> (f32, f32, f32) {
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in line {
        for (k, v) in [p[0], p[2]].into_iter().enumerate() {
            lo[k] = lo[k].min(v);
            hi[k] = hi[k].max(v);
        }
    }
    if line.is_empty() {
        return (0., 0., 1.);
    }
    let size = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.);
    (
        lo[0] - (size - (hi[0] - lo[0])) / 2.,
        lo[1] - (size - (hi[1] - lo[1])) / 2.,
        size,
    )
}

/// Map pixel of a world point: x to the right, world +Z up the map [I].
pub fn map_point(p: [f32; 3], (x0, z0, size): (f32, f32, f32), map: f32) -> (f32, f32) {
    ((p[0] - x0) / size * map, map - (p[2] - z0) / size * map)
}

pub fn lap_time(seconds: f64) -> String {
    grid_tools::race::format_time(seconds)
}

/// Everything the HUD shows this frame, for the plain panels or the OSD.
#[derive(Resource, Default)]
pub struct HudView {
    pub lap: LapInfo,
    /// A new personal best is marked (PB) for the split time.
    pub new_best: bool,
    /// The new best is also a world record (WR wins over PB).
    pub world_record: bool,
    pub position: Option<(usize, usize, Vec<Split>)>,
    pub splits: bool,
    /// Speed in the selected unit.
    pub speed: f64,
    pub mph: bool,
    pub gear: String,
    pub gauge: Option<Dial>,
    pub throttle: f32,
    pub brake: f32,
    pub odometer: f64,
    pub countdown: Option<String>,
    pub wrong_way: bool,
    pub yellow_flag: bool,
    /// The player's latest touge penalty (the penalty pill).
    pub penalty: Option<crate::touge_event::PenaltyPill>,
    pub show_lap: bool,
    pub show_position: bool,
    pub show_dials: bool,
    pub show_minimap: bool,
    /// The pause menu, a post-race screen, a replay or the front end is up:
    /// the OSD is off then [D: `post_race_entry` turns the OSD off; I: the
    /// pause menu and replays, from the original's pause frame].
    pub hidden: bool,
    labels: std::collections::HashMap<&'static str, String>,
}

/// The HUD's texts in the user's language table [D ids].
const LABELS: [&str; 13] = [
    "osd_lap", "osd_race", "osd_laptime", "osd_best", "osd_pb", "osd_wr", "osd_pos", "osd_GO", "osd_wrong_way", "kph", "mph", "osd_minor", "osd_major",
];

impl HudView {
    pub fn label(&self, id: &'static str) -> String {
        self.labels.get(id).cloned().unwrap_or_else(|| id.to_owned())
    }
}

/// Wrong way: moving against the racing line (the HUD pill's input, and the
/// speech collector's wrong-way flag [I]).
pub(crate) fn wrong_way(driving: &crate::Driving) -> bool {
    let v = driving.runtime.velocity();
    let speed = f64::from((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
    driving.reset_line.as_ref().is_some_and(|line| {
        let p = driving.runtime.pose();
        let (_, d) = line.reset_target([p[12], p[13], p[14]]);
        speed > 3. && f64::from(v[0] * d[0] + v[2] * d[2]) < -1.
    })
}

#[allow(clippy::too_many_arguments)]
fn frame(
    driving: Res<crate::Driving>,
    trial: Option<Res<crate::time_trial::Trial>>,
    menu: Option<Res<PauseMenu>>,
    labels: Option<Res<Labels>>,
    time: Option<Res<Time<Real>>>,
    post: Option<Res<crate::post_race::PostRace>>,
    replay: Option<Res<crate::replay::Replay>>,
    front: Option<Res<crate::front_end::FrontEnd>>,
    touge: Option<Res<crate::touge_event::TougeEvent>>,
    world: Option<Res<WorldRecord>>,
    mut state: ResMut<HudState>,
    mut view: ResMut<HudView>,
) {
    let dt = time.map_or(0., |t| t.delta_secs());
    let options = menu.as_ref().map(|m| m.values).unwrap_or_default();
    if view.labels.is_empty()
        && let Some(labels) = &labels
    {
        view.labels = LABELS.iter().map(|&id| (id, labels.get(id).to_owned())).collect();
    }
    let v = driving.runtime.velocity();
    let speed = f64::from((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
    // Odometer: distance driven this event (presentation; reset on restart).
    let now = (driving.segment, driving.runtime.ticks);
    match state.last {
        Some((segment, ticks)) if segment == now.0 && now.1 >= ticks => {
            state.odometer += speed * (now.1 - ticks) as f64 * f64::from(grid_tools::source_vehicle::DELTA);
        }
        _ => state.odometer = 0.,
    }
    state.last = Some(now);
    let lap = driving.runtime.lap.as_ref();
    let mut info = lap_info(trial.as_ref().map(|t| &t.state), lap);
    let (held, pill) = state.lap_hold.update(
        lap.map(|l| l.attempts),
        lap.and_then(|l| l.last_lap.as_ref()).map(|l| l.seconds),
        info.best,
        world.and_then(|w| w.0),
        if driving.paused { 0. } else { dt },
    );
    if held.is_some() {
        info.current = held;
    }
    let wrong = wrong_way(&driving);
    let wrong_way = state.wrong_way.update(wrong && !driving.paused, if driving.paused { 0. } else { dt });
    let b = &driving.runtime.state.car.owner.vehicle;
    let word = |o: usize| f32::from_bits(b[o / 4]);
    let controls = driving.last_controls.unwrap_or_default();
    let race = driving.opponents.as_ref().filter(|_| trial.is_some());
    let split = race.and_then(|o| state.split.update(&o.split_times, 0, o.standings.len(), if driving.paused { 0. } else { dt }));
    let finished = trial.as_ref().is_some_and(|t| t.finished());
    let go = view.label("osd_GO");
    // The message box counts down only the solo time trial (AUTHORED, no
    // start lights). Races start on the lights alone: the original's box
    // runs only for the online session's timer (docs/AUDIO_RESEARCH.md §28).
    let countdown = trial.as_ref().filter(|t| !t.race).and_then(|t| match t.state.phase() {
        grid_tools::time_trial::Phase::Countdown(_) => t.state.countdown_label().map(|n| n.to_string()),
        grid_tools::time_trial::Phase::Running if t.state.elapsed() < 1. => Some(go),
        _ => None,
    });
    let hidden = menu.as_ref().is_some_and(|m| m.open)
        || post.as_ref().is_some_and(|p| p.stage != crate::post_race::Stage::Hidden)
        || replay.as_ref().is_some_and(|r| r.session.is_some())
        || front.as_ref().is_some_and(|f| f.open);
    let labels = std::mem::take(&mut view.labels);
    *view = HudView {
        lap: info,
        new_best: pill != RecordPill::Off,
        world_record: pill == RecordPill::World,
        position: race.map(|o| position(&o.standings, &o.names, 0, &o.split_times, split)),
        splits: split.is_some(),
        speed: if options.mph { speed * 3.6 / 1.609_344 } else { speed * 3.6 },
        mph: options.mph,
        gear: match b[0x196c / 4] {
            0 => "N".into(),
            9 => "R".into(),
            n => n.to_string(),
        },
        gauge: Some(dial(word(0x1894), word(0x17a4))),
        throttle: controls.throttle,
        brake: controls.brake,
        odometer: state.odometer,
        countdown: if driving.paused { None } else { countdown },
        wrong_way: wrong_way && !finished,
        yellow_flag: race.is_some_and(|o| o.yellow_flag),
        penalty: touge.and_then(|t| t.pill),
        show_lap: options.lap_timer,
        show_position: options.position && race.is_some(),
        show_dials: options.dials,
        show_minimap: options.minimap && driving.reset_line.is_some(),
        hidden,
        labels,
    };
}

#[allow(clippy::type_complexity)]
fn update(
    driving: Res<crate::Driving>,
    view: Res<HudView>,
    mut parts: Query<(
        &Part,
        Option<&mut Text>,
        Option<&mut Node>,
        Option<&mut Visibility>,
        Option<&mut TextColor>,
    )>,
    mut dots: Query<(&CarDot, &mut Node, &mut Visibility), Without<Part>>,
) {
    let l = |id: &'static str| view.label(id);
    let info = &view.lap;
    let gauge = view.gauge.unwrap_or(dial(0., 0.));
    for (part, text, node, visibility, colour) in &mut parts {
        let show = |v: Option<Mut<Visibility>>, on: bool| {
            if let Some(mut v) = v {
                let want = if on { Visibility::Inherited } else { Visibility::Hidden };
                if *v != want {
                    *v = want;
                }
            }
        };
        let set = |t: Option<Mut<Text>>, s: String| {
            if let Some(mut t) = t
                && t.0 != s
            {
                t.0 = s;
            }
        };
        let width = |n: Option<Mut<Node>>, f: f32| {
            if let Some(mut n) = n {
                let w = percent((f * 100.).clamp(0., 100.));
                if n.width != w {
                    n.width = w;
                }
            }
        };
        match part {
            Part::LapGroup => show(visibility, view.show_lap),
            Part::LapText => {
                let lap = match info.total {
                    Some(total) => format!("{} {}/{}", l("osd_lap"), info.lap, total),
                    None => format!("{} {}", l("osd_lap"), info.lap),
                };
                let mut s = lap;
                if let Some(t) = info.race {
                    s.push_str(&format!("\n{}  {}", l("osd_race").to_uppercase(), lap_time(t)));
                }
                s.push_str(&format!(
                    "\n{}  {}",
                    l("osd_laptime"),
                    info.current.map_or_else(|| "-:--.---".into(), lap_time)
                ));
                s.push_str(&format!(
                    "\n{}  {}{}",
                    l("osd_best"),
                    info.best.map_or_else(|| "-:--.---".into(), lap_time),
                    if view.new_best { format!("  {}", l(if view.world_record { "osd_wr" } else { "osd_pb" })) } else { String::new() }
                ));
                set(text, s);
            }
            Part::PositionGroup => show(visibility, view.show_position),
            Part::PositionText => {
                if let Some((pos, total, _)) = &view.position {
                    set(text, format!("{} {pos}/{total}", l("osd_pos")));
                }
            }
            Part::Splits => {
                let s = view
                    .position
                    .as_ref()
                    .filter(|_| view.splits)
                    .map(|(_, _, rows)| {
                        rows.iter()
                            .map(|r| {
                                format!(
                                    "{:>2}  {:<10} {}",
                                    r.position,
                                    r.name,
                                    r.time
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                set(text, s);
            }
            Part::Dial => show(visibility, view.show_dials),
            Part::Speed => {
                let unit = if view.mph { l("mph") } else { l("kph") };
                set(text, format!("{:>3.0} {unit}", view.speed));
            }
            Part::Gear => set(text, view.gear.clone()),
            Part::RpmFill => width(node, gauge.rpm / gauge.scale),
            Part::RpmRed => {
                if let Some(mut n) = node {
                    let left = percent(gauge.red / gauge.scale * 100.);
                    if n.left != left {
                        n.left = left;
                        n.width = percent(100. - gauge.red / gauge.scale * 100.);
                    }
                }
            }
            Part::GearLight | Part::RevLight => {
                let on = if matches!(part, Part::GearLight) {
                    gauge.gear_light
                } else {
                    gauge.rev_light
                };
                if let Some(mut c) = colour {
                    c.0 = if on {
                        Color::srgb(1., 0.3, 0.2)
                    } else {
                        Color::srgba(1., 1., 1., 0.25)
                    };
                }
            }
            Part::Throttle => width(node, view.throttle),
            Part::Brake => width(node, view.brake),
            Part::Odometer => {
                let (d, unit) = if view.mph {
                    (view.odometer / 1609.344, "mi")
                } else {
                    (view.odometer / 1000., "km")
                };
                set(text, format!("{d:07.2} {unit}"));
            }
            Part::Message => set(text, view.countdown.clone().unwrap_or_default()),
            Part::WrongWay => set(text, if view.wrong_way { l("osd_wrong_way") } else { String::new() }),
            Part::Minimap => show(visibility, view.show_minimap),
            Part::MinimapFlag => set(text, if view.yellow_flag { "YELLOW".into() } else { String::new() }),
        }
    }
    // Car dots on the minimap.
    let Some(line) = &driving.reset_line else {
        return;
    };
    let bounds = map_bounds(line.points());
    for (dot, mut node, mut visibility) in &mut dots {
        if let Some(p) = car_pose(&driving, dot.0) {
            let (x, y) = map_point([p[12], p[13], p[14]], bounds, 190.);
            node.left = px(10. + x - 4.);
            node.top = px(10. + y - 4.);
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

/// World pose of car `c` (0 = the player) as drawn this frame
/// (`Driving::shown_pose`).
pub fn car_pose(driving: &crate::Driving, c: usize) -> Option<[f32; 16]> {
    driving.shown_pose(c).map(|p| p.to_cols_array())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dial_lights_and_scale_follow_the_original_ratios() {
        let rad = |rpm: f32| rpm / 60. * std::f32::consts::TAU;
        let d = dial(rad(5000.), rad(7000.));
        assert!((d.rpm - 5000.).abs() < 0.5);
        assert_eq!((d.scale, d.red), (8000., 6000.));
        assert!(!d.gear_light && !d.rev_light);
        let d = dial(rad(6400.), rad(7000.));
        assert!(d.gear_light && !d.rev_light);
        let d = dial(rad(6700.), rad(7000.));
        assert!(d.gear_light && d.rev_light);
        assert_eq!(dial(rad(1000.), rad(9500.)).scale, 10000.);
        assert_eq!(dial(rad(1000.), rad(15000.)).scale, 12000.);
        assert_eq!(dial(0., 0.).rpm, 0.);
    }

    #[test]
    fn wrong_way_needs_four_seconds_on_and_two_off() {
        let mut w = WrongWay::default();
        for _ in 0..39 {
            assert!(!w.update(true, 0.1));
        }
        assert!(w.update(true, 0.11));
        for _ in 0..19 {
            assert!(w.update(false, 0.1));
        }
        assert!(!w.update(false, 0.11));
        // A short wrong-way moment resets the timer.
        w.update(true, 3.);
        w.update(false, 0.1);
        assert!(!w.update(true, 3.));
    }

    #[test]
    fn lap_group_counts_event_laps_and_caps_at_the_total() {
        use grid_tools::time_trial::State;
        let mut t = State::new(2, 0.05, 0.1).unwrap();
        let info = lap_info(Some(&t), None);
        assert_eq!((info.lap, info.total, info.race), (1, Some(2), None));
        t.countdown_tick();
        t.countdown_tick();
        let info = lap_info(Some(&t), None);
        assert_eq!((info.lap, info.total), (1, Some(2)));
        let free = lap_info(None, None);
        assert_eq!((free.lap, free.total, free.best), (1, None, None));
    }

    #[test]
    fn map_points_keep_the_track_square_and_inside() {
        let line = [[0., 0., 0.], [100., 0., 0.], [100., 0., 50.]];
        let b = map_bounds(&line);
        assert_eq!(b, (0., -25., 100.));
        assert_eq!(map_point([0., 0., -25.], b, 200.), (0., 200.));
        assert_eq!(map_point([100., 0., 75.], b, 200.), (200., 0.));
    }

    #[test]
    fn split_rows_show_the_leader_and_three_around_the_player() {
        assert_eq!(split_rows(0, 12), [0, 1, 2, 3]);
        assert_eq!(split_rows(1, 12), [0, 1, 2, 3]);
        assert_eq!(split_rows(2, 12), [0, 1, 2, 3]);
        assert_eq!(split_rows(6, 12), [0, 5, 6, 7]);
        assert_eq!(split_rows(11, 12), [0, 9, 10, 11]);
        assert_eq!(split_rows(2, 3), [0, 1, 2]);
    }

    #[test]
    fn split_times_read_as_a_race_clock_then_differences() {
        assert_eq!(split_clock(83.456), "01:23.45");
        assert_eq!(split_clock(3600.), "60:00.00");
        // Leader (car 2) at 61.25 s, the player (car 0) 1.5 s later, car 1
        // not there yet: its pill stays empty.
        let standings = grid_tools::race::Standings::on_loop(3, 1000., &[400., 300., 450.]);
        let names: Vec<String> = ["P", "B", "L"].map(String::from).into();
        let mut times = SplitTimes::default();
        times.record(2, 61.25);
        times.record(0, 62.75);
        let (pos, total, rows) = position(&standings, &names, 0, &times, Some(0));
        assert_eq!((pos, total), (2, 3));
        let texts: Vec<_> = rows.iter().map(|r| (r.position, r.name.as_str(), r.time.as_str(), r.player)).collect();
        assert_eq!(texts, [(1, "L", "01:01.25", false), (2, "P", "+00:01.50", true), (3, "B", "", false)]);
    }

    #[test]
    fn split_pills_show_ten_seconds_and_five_after_a_late_car() {
        let mut times = SplitTimes::default();
        let mut d = SplitDisplay::default();
        assert_eq!(d.update(&times, 0, 2, 0.1), None);
        times.record(0, 10.);
        assert_eq!(d.update(&times, 0, 2, 0.1), Some(0));
        let mut shown = 0.;
        while d.update(&times, 0, 2, 0.5).is_some() {
            shown += 0.5;
            if shown == 7.5 {
                // The car behind reaches the split with 2.5 s left.
                times.record(1, 13.);
            }
        }
        assert_eq!(shown, 12.5);
        // The next split shows again; a restart clears it.
        times.record(0, 40.);
        assert_eq!(d.update(&times, 0, 2, 0.1), Some(1));
        times.clear();
        assert_eq!(d.update(&times, 0, 2, 0.1), None);
    }

    #[test]
    fn a_completed_lap_holds_its_time_and_the_record_pill_for_five_seconds() {
        let mut h = LapHold::default();
        let (pb, off) = (RecordPill::Personal, RecordPill::Off);
        assert_eq!(h.update(Some(0), None, None, None, 0.5), (None, off));
        // Lap 1 completes in 61.2 s: a new best.
        let mut frames = Vec::new();
        for _ in 0..12 {
            frames.push(h.update(Some(1), Some(61.2), Some(61.2), None, 0.5));
        }
        assert_eq!(frames.iter().filter(|f| **f == (Some(61.2), pb)).count(), 10);
        assert_eq!(frames[11], (None, off));
        // A slower lap holds its time without the pill.
        assert_eq!(h.update(Some(2), Some(63.), Some(61.2), None, 0.5), (Some(63.), off));
        // A restart (the count drops) ends the hold at once.
        assert_eq!(h.update(Some(0), None, None, None, 0.5), (None, off));
    }

    #[test]
    fn a_best_under_the_world_record_shows_wr() {
        let mut h = LapHold::default();
        let wr = Some(60.);
        h.update(Some(0), None, None, wr, 0.1);
        // 61 s: a personal best only.
        assert_eq!(h.update(Some(1), Some(61.), Some(61.), wr, 0.1).1, RecordPill::Personal);
        // 59.5 s beats the record: WR, for the hold only.
        assert_eq!(h.update(Some(2), Some(59.5), Some(59.5), wr, 0.1).1, RecordPill::World);
        for _ in 0..60 {
            h.update(Some(2), Some(59.5), Some(59.5), wr, 0.1);
        }
        assert_eq!(h.update(Some(2), Some(59.5), Some(59.5), wr, 0.1).1, RecordPill::Off);
        // After a restart 59.8 s is a PB, not a WR: 59.5 is the record now.
        h.update(Some(0), None, None, wr, 0.1);
        assert_eq!(h.update(Some(1), Some(59.8), Some(59.8), wr, 0.1).1, RecordPill::Personal);
        assert_eq!(h.update(Some(2), Some(59.4), Some(59.4), wr, 0.1).1, RecordPill::World);
        // No record row (or −1): never WR.
        let mut h = LapHold::default();
        h.update(Some(0), None, None, Some(-1.), 0.1);
        assert_eq!(h.update(Some(1), Some(1.), Some(1.), Some(-1.), 0.1).1, RecordPill::Personal);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn world_records_come_from_the_track_model_and_grade() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let db = grid_formats::database::read(&schema, &data).unwrap();
        let mut found = 0;
        for route in grid_tools::route::CIRCUITS {
            let wr = WorldRecord::from_database(&db, route, "lac");
            println!("world_record route={route} lac={wr:?}");
            found += usize::from(wr.is_some());
        }
        assert!(found > 0, "no Lacetti world record on any circuit");
        assert!(WorldRecord::from_database(&db, grid_tools::route::DONINGTON, "no_such_car").is_none());
    }

    #[test]
    fn osd_timers_read_minutes_seconds_hundredths() {
        assert_eq!(osd_clock(5.617), "00:05.61");
        assert_eq!(osd_clock(83.255), "01:23.25");
        assert_eq!(osd_clock(7200.), "99:59.99");
    }
}
