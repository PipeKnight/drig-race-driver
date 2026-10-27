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

/// One split pill: running position, name, gap (s) to the player, minus for
/// a car ahead.
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    pub position: usize,
    pub name: String,
    pub gap: Option<f64>,
    pub player: bool,
}

/// Position group (n / total) and the split pills around the player: the car
/// ahead, the player and the car behind [I: four pills, `player1..4`; the
/// original's choice of rows was not traced]. Gaps are race distance at the
/// player's speed (floor 10 m/s).
pub fn position(
    standings: &grid_tools::race::Standings,
    names: &[String],
    player: usize,
    speed: f64,
) -> (usize, usize, Vec<Split>) {
    let order = standings.order();
    let at = order.iter().position(|&c| c == player).unwrap_or(0);
    let (ahead, behind) = standings.neighbour_gaps(player);
    let speed = speed.max(10.);
    let mut rows = Vec::new();
    let name = |c: usize| names.get(c).cloned().unwrap_or_default();
    if at > 0 {
        rows.push(Split {
            position: at,
            name: name(order[at - 1]),
            gap: ahead.map(|d| -d / speed),
            player: false,
        });
    }
    rows.push(Split {
        position: at + 1,
        name: name(player),
        gap: None,
        player: true,
    });
    if let Some(&c) = order.get(at + 1) {
        rows.push(Split {
            position: at + 2,
            name: name(c),
            gap: behind.map(|d| d / speed),
            player: false,
        });
    }
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

/// `playerspositioninterface splitdisplay maxduration="10.0" minduration="5.0"`
/// [D]: split pills show for 5 s after each completed lap [I: the minimum
/// duration, from the line crossing].
pub const SPLIT_SECONDS: f32 = 5.;

#[derive(Resource, Default)]
struct HudState {
    wrong_way: WrongWay,
    /// (segment, completed laps) of the last split trigger, and time left.
    split: (u64, u32, f32),
    odometer: f64,
    last: Option<(u64, u64)>,
    /// New personal best lap shown with `osd_pb` for the split time.
    best: (Option<f64>, f32),
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
        app.add_systems(Update, ((frame, crate::race_osd::drive, crate::race_osd::minimap).chain(), crate::race_osd::place_help));
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
    pub chequered: bool,
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
const LABELS: [&str; 10] = ["osd_lap", "osd_race", "osd_laptime", "osd_best", "osd_pb", "osd_pos", "osd_GO", "osd_wrong_way", "kph", "mph"];

impl HudView {
    pub fn label(&self, id: &'static str) -> String {
        self.labels.get(id).cloned().unwrap_or_else(|| id.to_owned())
    }
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
    let info = lap_info(trial.as_ref().map(|t| &t.state), driving.runtime.lap.as_ref());
    // Splits show for 5 s after each completed lap; a new best is marked PB.
    let completed = driving.runtime.lap.as_ref().map_or(0, |l| l.attempts);
    if (state.split.0, state.split.1) != (driving.segment, completed) {
        let fresh = state.split.0 != driving.segment;
        state.split = (driving.segment, completed, if fresh { 0. } else { SPLIT_SECONDS });
    } else {
        state.split.2 = (state.split.2 - dt).max(0.);
    }
    if info.best != state.best.0 {
        state.best = (info.best, if info.best.is_some() { SPLIT_SECONDS } else { 0. });
    } else {
        state.best.1 = (state.best.1 - dt).max(0.);
    }
    // Wrong way: moving against the racing line.
    let wrong = driving.reset_line.as_ref().is_some_and(|line| {
        let p = driving.runtime.pose();
        let (_, d) = line.reset_target([p[12], p[13], p[14]]);
        speed > 3. && f64::from(v[0] * d[0] + v[2] * d[2]) < -1.
    });
    let wrong_way = state.wrong_way.update(wrong && !driving.paused, if driving.paused { 0. } else { dt });
    let b = &driving.runtime.state.car.owner.vehicle;
    let word = |o: usize| f32::from_bits(b[o / 4]);
    let controls = driving.last_controls.unwrap_or_default();
    let race = driving.opponents.as_ref().filter(|_| trial.is_some());
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
        new_best: state.best.1 > 0.,
        position: race.map(|o| position(&o.standings, &o.names, 0, speed)),
        splits: state.split.2 > 0.,
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
        chequered: race.is_some_and(|o| o.standings.leader_flag().is_some()),
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
                    if view.new_best { format!("  {}", l("osd_pb")) } else { String::new() }
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
                                    r.gap.map_or_else(String::new, |g| format!("{g:+.1}"))
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
            Part::MinimapFlag => set(text, if view.chequered { "CHEQUERED".into() } else { String::new() }),
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

/// World pose of car `c` (0 = the player).
pub fn car_pose(driving: &crate::Driving, c: usize) -> Option<[f32; 16]> {
    if c == 0 {
        Some(driving.runtime.pose())
    } else {
        driving.opponents.as_ref().and_then(|o| o.multi.cars.get(c)).map(|r| r.pose())
    }
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
}
