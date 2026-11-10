//! The original's post-race flow for a Race Day free race
//! ([research](../../../docs/RACE_HUD.md), section "Post-race").
//!
//! Flow `system/flow.bin` branch `f_post` [D]:
//! - finish: `post_race_entry` → OSD off → `post_race_cutscene` →
//!   `post_race_stats_free_race` (stats `position` and `fastest_lap`, select =
//!   Continue) → `stats_flow_looper` → `post_race_results_generic`;
//! - Retire from Race: `retire_race` → replay → `results_show_screen` →
//!   `post_race_results_generic` (no stats screen);
//! - results pills (itemflow order): Continue (`end_race_finish_race`, back
//!   to the front end), View Replay, Exit (`exit_race_confirm`), Restart Race
//!   (`restart_race_confirm`);
//! - terminal damage: `terminal_flash` 1 s and `terminal_fade` 1 s, then
//!   `fr_terminal_damage` (Restart Race, Retire → `retire_race_confirm` →
//!   `end_race_exit_event`, Instant Replay) or `td_terminal_damage` for a test
//!   drive (Restart Test Drive, Finish Test Drive, Instant Replay).
//!
//! Results columns [D 0x52E4F0..0x52E6A8, `race_results`]: position, driver,
//! time, car and team (points only in career events).
//!
//! Drift events (docs/DISCIPLINES.md) [D `f_post` drift links]: finish →
//! `post_race_stats_drift_scores` (`drift_total`) → `post_race_drift_scoreboard`
//! (the same four pills; Continue opens the results) → `post_race_results_drift`;
//! Retire → the scoreboard.
//!
//! Leaving the event (Continue, Exit, Retire from the wreck screen, Finish
//! Test Drive) returns to the event menu when the race was started from it
//! (event_menu.rs), as the original returns to its front end; a race started
//! by a launcher closes the app, back to the launcher.
//! The cutscene, replays and the original's look are not ported.
use crate::input::{Actions, ShiftQueue};
use crate::pause_menu::{Confirm, Labels, Nav, Repeat, Variant};
use bevy::prelude::*;
use grid_tools::race::{Out, Standings, Status};

/// The time cell of a results row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cell {
    /// The winner's race time.
    Total(f64),
    /// Behind the winner (finished or estimated).
    Gap(f64),
    /// Out of the race, with the laps completed.
    Out(Out, u32),
    /// Still racing (results shown before the race was finalised).
    Running,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub position: usize,
    pub car: usize,
    pub name: String,
    pub time: Cell,
    /// Finish time was estimated when the race was ended [V 0x90FCD0].
    pub estimated: bool,
    /// (seconds, one-based lap).
    pub best: Option<(f64, usize)>,
    pub vehicle: String,
    /// Drift events: the drift score (a zero score replaced by the virtual
    /// race's estimate, docs/DISCIPLINES.md).
    pub score: Option<i64>,
    /// Pro Touge: each run's time with its penalty (`None` unfinished);
    /// the time column then holds the total (docs/TOUGE.md §7).
    pub touge: Option<[Option<f64>; 2]>,
    /// Le Mans Series: the car's class name key (`db_lmp1`, ...), shown in
    /// the Class column (`post_race_results_lemans`, F413).
    pub class: Option<String>,
    /// A career race: the points this position scores (the event's points
    /// table, 0 past it), shown in the Points column.
    pub points: Option<i32>,
}

/// Each car's class name key by car index (the player first) in a Le Mans
/// Series race (docs/SERIES.md).
#[derive(Resource, Clone, Debug, Default)]
pub struct CarClasses(pub Vec<String>);

/// A career race's points table by finishing position (`[game] points`,
/// docs/CAREER_SCREEN.md).
#[derive(Resource, Clone, Debug, Default)]
pub struct RacePoints(pub Vec<i32>);

/// Results rows in classification order. `player_best` replaces car 0's
/// line-timed best lap with its gate-timed one.
pub fn rows(
    standings: &Standings,
    names: &[String],
    vehicles: &[String],
    player_best: Option<(f64, usize)>,
) -> Vec<Row> {
    let order = standings.order();
    let leader = order.first().and_then(|&c| standings.finished(c));
    order
        .iter()
        .enumerate()
        .map(|(rank, &car)| {
            let status = standings.status(car);
            let time = match (status, standings.finished(car), leader) {
                (Status::Out(why), _, _) => {
                    Cell::Out(why, standings.laps_covered(car).floor().max(0.) as u32)
                }
                (_, Some(t), Some(l)) if rank > 0 => Cell::Gap(t - l),
                (_, Some(t), _) => Cell::Total(t),
                _ => Cell::Running,
            };
            Row {
                position: rank + 1,
                car,
                name: names.get(car).cloned().unwrap_or_default(),
                time,
                estimated: status == Status::Estimated,
                best: if car == 0 {
                    player_best.or_else(|| standings.best_lap(car))
                } else {
                    standings.best_lap(car)
                },
                vehicle: vehicles.get(car).cloned().unwrap_or_default(),
                score: None,
                touge: None,
                class: None,
                points: None,
            }
        })
        .collect()
}

/// A solo timed event: the player alone.
pub fn solo_row(name: &str, vehicle: &str, result: Option<&grid_tools::time_trial::Results>) -> Row {
    Row {
        position: 1,
        car: 0,
        name: name.into(),
        time: result.map_or(Cell::Out(Out::Retired, 0), |r| Cell::Total(r.seconds)),
        estimated: false,
        best: result.and_then(|r| {
            r.laps
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.seconds.total_cmp(&b.1.seconds))
                .map(|(i, l)| (l.seconds, i + 1))
        }),
        vehicle: vehicle.into(),
        score: None,
        touge: None,
        class: None,
        points: None,
    }
}

/// Results pill links, itemflow order [D].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pill {
    Continue,
    Replay,
    Exit,
    Restart,
    /// Wreck screen: retire (free race) or finish the test drive.
    Retire,
}

pub const RESULTS_PILLS: [(Pill, &str); 4] = [
    (Pill::Continue, "post_race_menu_continue_button_header"),
    (Pill::Replay, "post_race_menu_replay_button_header"),
    (Pill::Exit, "post_race_menu_quit"),
    (Pill::Restart, "terminal_damage_restart_button_header"),
];
pub const WRECK_PILLS: [(Pill, &str); 3] = [
    (Pill::Restart, "terminal_damage_restart_button_header"),
    (Pill::Retire, "terminal_damage_retire_button_header"),
    (Pill::Replay, "terminal_damage_replay_button_header"),
];
pub const TEST_DRIVE_WRECK_PILLS: [(Pill, &str); 3] = [
    (Pill::Restart, "pause_mainmenu_button_restart_test_drive"),
    (Pill::Retire, "pause_mainmenu_button_finish_test_drive"),
    (Pill::Replay, "terminal_damage_replay_button_header"),
];

/// `terminal_flash` + `terminal_fade` time delays (1.0 s each) [D].
pub const WRECK_TRANSITIONS: f32 = 2.;
/// `terminal_flash` slowMo: the time scale 0.1 (0xCAFCD4) [D].
pub const WRECK_SLOW: f32 = 0.1;

/// The game-time scale `elapsed` real seconds into the wreck transitions:
/// `terminal_flash` holds [`WRECK_SLOW`] for its 1 s, then `terminal_fade`
/// rampSlowMo (0x57A140) gives min(0.1 + (2 × its elapsed)², 1) [D]. The
/// states' clocks are taken as real time [I].
pub fn wreck_time_scale(elapsed: f32) -> f32 {
    let fade = elapsed - 1.;
    if fade < 0. {
        WRECK_SLOW
    } else {
        (WRECK_SLOW + (2. * fade).powi(2)).min(1.)
    }
}
/// `StateTerminalDamageDelay` then waits for the player's car to come to
/// rest, giving up this long after entering the state (0x57F1A0) [D] ...
pub const WRECK_REST_LIMIT: f32 = 2.;
/// ... and returns "done" this long after the rest (0x58E2E0) [D].
pub const WRECK_AFTER_REST: f32 = 1.;
/// At rest: angular speed at most 0.1 and linear speed at most 0.5
/// (0xCAFCD4, 0xCAF88C) [D values; I: rad/s and m/s, as magnitudes].
pub const REST_ANGULAR: f32 = 0.1;
pub const REST_LINEAR: f32 = 0.5;

/// The car rests by the terminal-damage delay's thresholds.
pub fn at_rest(linear: [f32; 4], angular: [f32; 4]) -> bool {
    let len = |v: [f32; 4]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    len(angular) <= REST_ANGULAR && len(linear) <= REST_LINEAR
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Hidden,
    Stats,
    Results,
    /// Drift events: the player's drift scoreboard before the results
    /// (`post_race_drift_scoreboard` [D]).
    Scoreboard,
    Wreck,
    /// A confirmation and the stage it returns to.
    Confirm(Confirm, Return),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Return {
    Results,
    Scoreboard,
    Wreck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Stats → results: end the race (finalisation from the front end, 0x583B00).
    Finalize,
    Restart,
    /// Leave the event: close the app.
    Exit,
    /// Retire from the wreck screen, then leave the event.
    RetireAndExit,
    /// View Replay (`c_post_replay_interface`, the whole race); back to
    /// this screen on Replay Exit.
    Replay(crate::replay::Kind),
    /// The wreck screen's Replay: the terminal-damage replay
    /// (`crate::replay::Replay::request_terminal`).
    TerminalReplay,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PostRace {
    pub variant: Variant,
    /// A drift event: stats, scoreboard, drift results (`f_post` drift links).
    pub drift: bool,
    pub stage: Stage,
    cursor: usize,
    answer: usize,
    wreck_timer: f32,
    /// The simulation tick of the terminal damage (the original stores the
    /// crash time in P+0xA18 from 0x59D870), the terminal replay's anchor.
    pub wreck_tick: Option<u64>,
    /// Seconds since the wrecked car came to rest (or the rest wait gave up).
    wreck_rest: Option<f32>,
    /// The wreck screen just opened: [`update`] plays the crash replay first.
    crash_replay: bool,
    segment: u64,
    repeat: Repeat,
    /// Menu widget and popup sounds of the last steps (drained to the
    /// race's HUD sink by `update`).
    pub(crate) sounds: Vec<grid_tools::hud_audio::Cue>,
}

impl PostRace {
    pub fn new(variant: Variant) -> Self {
        Self {
            variant,
            drift: false,
            stage: Stage::Hidden,
            cursor: 0,
            answer: 1,
            wreck_timer: 0.,
            wreck_tick: None,
            wreck_rest: None,
            crash_replay: false,
            segment: 0,
            repeat: Repeat::default(),
            sounds: Vec::new(),
        }
    }

    /// The post-race screens own the input (no pause menu).
    pub fn active(&self) -> bool {
        self.stage != Stage::Hidden
    }

    pub fn wreck_pills(&self) -> &'static [(Pill, &'static str)] {
        match self.variant {
            Variant::FreeRace => &WRECK_PILLS,
            Variant::TestDrive => &TEST_DRIVE_WRECK_PILLS,
        }
    }

    fn pills(&self) -> &'static [(Pill, &'static str)] {
        match self.stage {
            Stage::Wreck | Stage::Confirm(_, Return::Wreck) => self.wreck_pills(),
            _ => &RESULTS_PILLS,
        }
    }

    /// The player crossed the line for the last time.
    pub fn on_finish(&mut self) {
        if self.stage == Stage::Hidden {
            self.stage = Stage::Stats;
        }
    }

    /// Retire from Race (pause menu): straight to the results (a drift
    /// event's scoreboard first, `f_post_retire_discipline_decision` [D]).
    pub fn on_retire(&mut self) {
        self.stage = self.first_results();
        self.cursor = 0;
    }

    fn first_results(&self) -> Stage {
        if self.drift {
            Stage::Scoreboard
        } else {
            Stage::Results
        }
    }

    /// Terminal damage this frame (`dt` real seconds, `rest` the car's
    /// [`at_rest`]): the two transitions, the rest wait (at most
    /// [`WRECK_REST_LIMIT`]), then [`WRECK_AFTER_REST`] and the wreck screen.
    /// [`update`] then plays the crash replay; the menu shows on its exit.
    pub fn on_wreck(&mut self, wrecked: bool, rest: bool, dt: f32) {
        if !wrecked || self.stage != Stage::Hidden {
            self.wreck_timer = 0.;
            self.wreck_rest = None;
            return;
        }
        self.wreck_timer += dt;
        let in_state = self.wreck_timer - WRECK_TRANSITIONS;
        if in_state < 0. {
            return;
        }
        match &mut self.wreck_rest {
            Some(t) => *t += dt,
            None if rest || in_state > WRECK_REST_LIMIT => self.wreck_rest = Some(0.),
            None => {}
        }
        if self.wreck_rest.is_some_and(|t| t >= WRECK_AFTER_REST) {
            self.stage = Stage::Wreck;
            self.cursor = 0;
            self.crash_replay = true;
        }
    }

    /// Real seconds into the wreck's `terminal_flash` and `terminal_fade`
    /// (the first [`WRECK_TRANSITIONS`] seconds of the terminal damage).
    pub fn wreck_transition(&self) -> Option<f32> {
        (self.stage == Stage::Hidden
            && self.wreck_timer > 0.
            && self.wreck_timer < WRECK_TRANSITIONS)
            .then_some(self.wreck_timer)
    }

    /// A Flashback from the wreck screen's replay: the race goes on.
    pub fn resume_race(&mut self) {
        if matches!(self.stage, Stage::Wreck | Stage::Confirm(_, Return::Wreck)) {
            self.stage = Stage::Hidden;
        }
        self.wreck_timer = 0.;
        self.wreck_rest = None;
    }

    pub fn reset(&mut self, segment: u64) {
        let (repeat, drift) = (self.repeat, self.drift);
        *self = Self::new(self.variant);
        self.segment = segment;
        self.repeat = repeat;
        self.drift = drift;
    }

    /// One input frame, queueing its sounds: a pill move or a Yes/No
    /// change, activate or back (hud_sound::menu_cue), and the popup sound
    /// when a confirmation (a `ScreenYesNo` such as `exit_race_confirm`)
    /// opens or closes.
    pub fn step(&mut self, nav: Nav) -> Option<Command> {
        let (stage, place) = (self.stage, (self.cursor, self.answer));
        let command = self.navigate(nav);
        if stage == Stage::Hidden {
            return command;
        }
        let left = stage != self.stage || command.is_some();
        let cursor = place != (self.cursor, self.answer);
        self.sounds.extend(crate::hud_sound::menu_cue(&nav, left, cursor, false, false));
        let popup = |s: Stage| matches!(s, Stage::Confirm(..));
        self.sounds.extend(crate::hud_sound::overlay_cue(popup(stage), popup(self.stage) && command.is_none()));
        command
    }

    fn navigate(&mut self, nav: Nav) -> Option<Command> {
        let n = self.pills().len();
        match self.stage {
            Stage::Hidden => None,
            Stage::Stats => {
                if nav.select {
                    self.stage = self.first_results();
                    self.cursor = 0;
                    return Some(Command::Finalize);
                }
                None
            }
            Stage::Results | Stage::Scoreboard | Stage::Wreck => {
                if nav.left || nav.up {
                    self.cursor = (self.cursor + n - 1) % n;
                }
                if nav.right || nav.down {
                    self.cursor = (self.cursor + 1) % n;
                }
                if !nav.select {
                    return None;
                }
                let back = match self.stage {
                    Stage::Wreck => Return::Wreck,
                    Stage::Scoreboard => Return::Scoreboard,
                    _ => Return::Results,
                };
                let (pill, _) = self.pills()[self.cursor];
                let confirm = |c| Stage::Confirm(c, back);
                match pill {
                    // The scoreboard's Continue opens the drift results.
                    Pill::Continue if self.stage == Stage::Scoreboard => {
                        self.stage = Stage::Results;
                        self.cursor = 0;
                        return None;
                    }
                    Pill::Continue => return Some(Command::Exit),
                    Pill::Replay => {
                        return Some(if self.stage == Stage::Wreck {
                            Command::TerminalReplay
                        } else {
                            Command::Replay(crate::replay::Kind::Full)
                        });
                    }
                    Pill::Exit => self.stage = confirm(Confirm::Exit),
                    Pill::Restart => self.stage = confirm(Confirm::Restart),
                    Pill::Retire => {
                        self.stage = confirm(match self.variant {
                            Variant::FreeRace => Confirm::RetireRace,
                            Variant::TestDrive => Confirm::FinishTestDrive,
                        })
                    }
                }
                self.answer = 1;
                None
            }
            Stage::Confirm(confirm, back) => {
                if nav.left || nav.right || nav.up || nav.down {
                    self.answer = 1 - self.answer;
                }
                let previous = match back {
                    Return::Results => Stage::Results,
                    Return::Scoreboard => Stage::Scoreboard,
                    Return::Wreck => Stage::Wreck,
                };
                if nav.back || (nav.select && self.answer == 0) {
                    self.stage = previous;
                    return None;
                }
                if !nav.select {
                    return None;
                }
                Some(match confirm {
                    Confirm::Restart => {
                        self.stage = Stage::Hidden;
                        Command::Restart
                    }
                    Confirm::RetireRace => Command::RetireAndExit,
                    _ => Command::Exit,
                })
            }
        }
    }
}

pub fn install(app: &mut App, variant: Variant) {
    app.insert_resource(PostRace::new(variant))
        .add_systems(
            Update,
            update
                .before(crate::pause_menu::update)
                .before(crate::drive_actions),
        )
        .add_systems(Update, (spawn, present).chain().after(update))
        .add_systems(Update, wreck_slow_motion.after(update));
}

/// The wreck transitions' slow motion on the game clock; back to 1 after.
fn wreck_slow_motion(
    post: Res<PostRace>,
    time: Option<ResMut<Time<Virtual>>>,
    mut scaled: Local<bool>,
) {
    let Some(mut time) = time else {
        return;
    };
    match post.wreck_transition().map(wreck_time_scale) {
        Some(scale) => {
            time.set_relative_speed(scale);
            *scaled = true;
        }
        None if *scaled => {
            time.set_relative_speed(1.);
            *scaled = false;
        }
        None => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut post: ResMut<PostRace>,
    mut driving: ResMut<crate::Driving>,
    actions: Res<Actions>,
    mut shifts: ResMut<ShiftQueue>,
    mut trial: Option<ResMut<crate::time_trial::Trial>>,
    (time, mut leave): (Option<Res<Time<Real>>>, crate::transition::Leave),
    sink: Option<Res<crate::hud_sound::HudSink>>,
    mut replay: Option<ResMut<crate::replay::Replay>>,
    touge: Option<Res<crate::touge_event::TougeEvent>>,
    mut waited: Local<f32>,
) {
    // A replay owns the input; its exit returns here. Nothing more after
    // leaving (the fade to black runs).
    if replay.as_ref().is_some_and(|r| r.owns_input()) || leave.leaving() {
        return;
    }
    let dt = time.map_or(0., |t| t.delta_secs());
    if post.segment != driving.segment {
        let segment = driving.segment;
        post.reset(segment);
    }
    // A Pro Touge's first run ends without the results (touge_event).
    let finished = trial.as_ref().is_some_and(|t| t.finished())
        && touge.as_ref().is_none_or(|t| t.run == 1);
    if finished && post.stage == Stage::Hidden {
        log_line!(
            "drive_post_race version=1 segment={} tick={} stage=stats",
            driving.segment,
            driving.runtime.ticks
        );
        post.on_finish();
    }
    // A disqualified player (Demolition Derby) goes to the results.
    if driving.runtime.disqualified() && post.stage == Stage::Hidden {
        log_line!(
            "drive_post_race version=1 segment={} tick={} stage=results reason=disqualified",
            driving.segment,
            driving.runtime.ticks
        );
        post.on_retire();
    }
    let wrecked = driving.runtime.terminal_wheel().is_some() && !finished;
    if !wrecked {
        post.wreck_tick = None;
    } else if post.wreck_tick.is_none() {
        post.wreck_tick = Some(driving.runtime.ticks);
    }
    let before = post.stage;
    let rest = at_rest(driving.runtime.velocity(), driving.runtime.angular_velocity());
    post.on_wreck(wrecked, rest, if driving.paused { 0. } else { dt });
    if before != post.stage {
        log_line!(
            "drive_post_race version=1 segment={} tick={} stage=wreck",
            driving.segment,
            driving.runtime.ticks
        );
    }
    // The shipped flow plays the crash replay by itself after
    // `terminal_damage_delay` (`instant_race_replay_silent`, terminal) and
    // shows the wreck menu on its exit [O: flow.bin]; the menu's Instant
    // Replay plays the same window again. An unattended scripted run
    // (diagnostics results_continue) goes straight to the menu.
    if std::mem::take(&mut post.crash_replay) {
        let unattended =
            crate::settings::diagnostics().results_continue.is_some() && driving.scripted_ticks.is_some();
        if !unattended && let Some(replay) = replay.as_mut() {
            replay.request_terminal(post.wreck_tick.map(crate::replay::seconds));
        }
    }
    let mut repeat = post.repeat;
    let mut nav = crate::pause_menu::nav(&actions, &mut repeat, dt);
    post.repeat = repeat;
    if !post.active() {
        return;
    }
    // Diagnostics results_continue: a scripted run selects Continue on the
    // stats and results screens by itself (unattended checks); on the wreck
    // screen it retires and confirms, so an unattended career race still
    // reports a result.
    if let Some(after) = crate::settings::diagnostics().results_continue
        && driving.scripted_ticks.is_some()
        && matches!(
            post.stage,
            Stage::Stats | Stage::Results | Stage::Wreck | Stage::Confirm(Confirm::RetireRace, Return::Wreck)
        )
    {
        *waited += dt;
        if f64::from(*waited) >= after {
            *waited = 0.;
            post.cursor = match post.stage {
                Stage::Wreck => post.wreck_pills().iter().position(|(p, _)| *p == Pill::Retire).unwrap_or(0),
                _ => 0,
            };
            nav.select = true;
        }
    } else {
        *waited = 0.;
    }
    // The car is held while the screens are up (no gear edges leak).
    *shifts = ShiftQueue::default();
    let command = post.step(nav);
    crate::hud_sound::drain(&mut post.sounds, sink.as_deref());
    let Some(command) = command else {
        return;
    };
    log_line!(
        "drive_post_race version=1 segment={} tick={} command={command:?} stage={:?}",
        driving.segment,
        driving.runtime.ticks,
        post.stage
    );
    match command {
        Command::Finalize => {
            let driving = &mut *driving;
            if let Some(opponents) = &mut driving.opponents
                && !opponents.standings.over()
            {
                let speeds = opponents.speeds(&driving.runtime);
                let time = trial
                    .as_ref()
                    .and_then(|t| t.race_time(driving.runtime.ticks))
                    .unwrap_or(0.);
                opponents.standings.finalize(&speeds, time, &[0]);
                for line in opponents.standings.classification(&opponents.names) {
                    log_line!("drive_race_result {line}");
                }
            }
        }
        Command::Restart => crate::restart(&mut driving, trial.as_deref_mut()),
        // Back to the event menu when the race came from it (the original's
        // `end_race_finish_race` returns to its front end).
        Command::Exit => leave.event(),
        Command::RetireAndExit => {
            crate::pause_menu::retire_player(&mut driving, trial.as_deref());
            leave.event();
        }
        Command::Replay(kind) => {
            if let Some(replay) = replay.as_mut() {
                replay.request(kind, crate::replay::Origin::Results);
            }
        }
        Command::TerminalReplay => {
            if let Some(replay) = replay.as_mut() {
                replay.request_terminal(post.wreck_tick.map(crate::replay::seconds));
            }
        }

    }
}

/// The results table of the event (empty in free practice).
pub(crate) fn table(driving: &crate::Driving, trial: Option<&crate::time_trial::Trial>) -> Vec<Row> {
    let vehicle = driving.runtime.car().label().to_string();
    let player_best = driving
        .runtime
        .lap
        .as_ref()
        .and_then(|l| l.best_lap)
        .map(|t| (t, 0));
    match (&driving.opponents, trial) {
        (Some(o), _) => {
            let vehicles: Vec<String> = std::iter::once(vehicle)
                .chain(o.multi.cars.iter().skip(1).map(|c| c.car().label().to_string()))
                .collect();
            // The gate-timed best lap of the player, numbered from the event's laps.
            let best = trial
                .and_then(|t| match t.state.phase() {
                    grid_tools::time_trial::Phase::Finished(r) => {
                        solo_row("", "", Some(r)).best
                    }
                    _ => None,
                })
                .or(player_best);
            rows(&o.standings, &o.names, &vehicles, best)
        }
        (None, Some(t)) => {
            let result = match t.state.phase() {
                grid_tools::time_trial::Phase::Finished(r) => Some(r),
                _ => None,
            };
            vec![solo_row("You", &vehicle, result)]
        }
        (None, None) => Vec::new(),
    }
}

/// The results list's difference text (0x4C8280, written into
/// `list.%d.anim.content.difference`): `+%02u.%02u` under a minute, else
/// `+%02u:%02u.%02u` with whole minutes, truncated hundredths; a zero
/// difference is a blank [D]. The `*` for an estimated finish is AUTHORED.
fn results_gap(seconds: f64) -> String {
    let hundredths = (seconds.max(0.) * 1000.).round() as u64 / 10;
    let (minutes, seconds, hundredths) = (hundredths / 6000, hundredths / 100 % 60, hundredths % 100);
    match (minutes, seconds, hundredths) {
        (0, 0, 0) => " ".into(),
        (0, _, _) => format!("+{seconds:02}.{hundredths:02}"),
        _ => format!("+{minutes:02}:{seconds:02}.{hundredths:02}"),
    }
}

fn time_text(row: &Row, l: &dyn Fn(&'static str) -> String) -> String {
    let est = if row.estimated { " *" } else { "" };
    match row.time {
        // The results list's time text, `list.%d.anim.content.time`
        // (0x4C8200 -> 0x908860 [D], the Le Mans results' format).
        Cell::Total(t) => format!("{}{est}", grid_tools::race::format_lemans_time(t)),
        Cell::Gap(g) => format!("{}{est}", results_gap(g)),
        Cell::Out(why, laps) => {
            let word = match why {
                Out::Retired => l("RETIRED"),
                Out::Wrecked => l("net_race_results_terminal_damage"),
                Out::Stopped => "DNF".into(),
                Out::Disqualified => l("net_race_results_disqualified"),
            };
            format!("{word} ({laps})")
        }
        Cell::Running => "--".into(),
    }
}

/// Ordinal for the stats screen: 1st, 2nd, 3rd, 4th… (`ordinalsuffix_*l`).
fn ordinal(n: usize, l: &dyn Fn(&'static str) -> String) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "ordinalsuffix_1l",
        (2, x) if x != 12 => "ordinalsuffix_2l",
        (3, x) if x != 13 => "ordinalsuffix_3l",
        _ => "ordinalsuffix_4l",
    };
    format!("{n}{}", l(suffix))
}

#[derive(Component)]
struct Panel;
#[derive(Component)]
enum Part {
    Title,
    Body,
    Table(usize),
    Pills,
}

const COLUMNS: [(&str, f32); 5] = [
    ("post_race_positions_table_positions_column", 50.),
    ("post_race_positions_table_name_column", 170.),
    ("post_race_positions_table_time_header", 170.),
    ("post_race_positions_table_vehicle_column", 230.),
    ("post_race_positions_table_team_column", 90.),
];
const BEST_COLUMN: f32 = 130.;
/// The header and up to 20 cars (the routes' grid slots; Le Mans Series
/// fields have 15-17, F409).
const TABLE_ROWS: usize = 21;

fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<crate::ViewerCamera>>,
    mut done: Local<bool>,
) {
    let Some(camera) = camera.iter().next().filter(|_| !*done) else {
        return;
    };
    *done = true;
    let font = |size: f32| TextFont {
        font_size: FontSize::Px(size),
        ..default()
    };
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.),
                height: percent(100.),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0., 0., 0., 0.5)),
            Visibility::Hidden,
            GlobalZIndex(9),
            UiTargetCamera(camera),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(px(24.)),
                        row_gap: px(6.),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.025, 0.04, 0.065, 0.94)),
                ))
                .with_children(|panel| {
                    panel.spawn((Part::Title, Text::new(""), font(30.), TextColor(Color::WHITE)));
                    panel.spawn((
                        Part::Body,
                        Text::new(""),
                        font(20.),
                        TextColor(Color::srgb(0.85, 0.9, 0.95)),
                    ));
                    for row in 0..TABLE_ROWS {
                        panel
                            .spawn(Node {
                                flex_direction: FlexDirection::Row,
                                ..default()
                            })
                            .with_children(|r| {
                                for (column, (_, width)) in COLUMNS
                                    .iter()
                                    .chain(std::iter::once(&("", BEST_COLUMN)))
                                    .enumerate()
                                {
                                    r.spawn((
                                        Part::Table(row * 8 + column),
                                        Node {
                                            width: px(*width),
                                            ..default()
                                        },
                                        Text::new(""),
                                        font(if row == 0 { 15. } else { 18. }),
                                        TextColor(Color::WHITE),
                                    ));
                                }
                            });
                    }
                    panel.spawn((
                        Part::Pills,
                        Text::new(""),
                        font(20.),
                        TextColor(Color::srgb(1., 0.78, 0.2)),
                    ));
                });
        });
}

/// What the panel shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub title: String,
    pub body: String,
    /// Header row then one row per car: six cells.
    pub table: Vec<[String; 6]>,
    pub player_row: Option<usize>,
    pub pills: String,
}

impl PostRace {
    #[cfg(test)]
    pub fn view(&self, rows: &[Row], labels: &Labels) -> View {
        self.view_with(rows, labels, None)
    }

    /// `board`: the player's drift scoreboard in a drift event.
    pub fn view_with(
        &self,
        rows: &[Row],
        labels: &Labels,
        board: Option<&crate::drift_event::Board>,
    ) -> View {
        let l = |id: &'static str| labels.get(id).to_owned();
        let pill_line = |pills: &[(Pill, &'static str)], cursor: usize| {
            pills
                .iter()
                .enumerate()
                .map(|(i, (_, id))| {
                    if i == cursor {
                        format!("[ {} ]", l(id))
                    } else {
                        format!("  {}  ", l(id))
                    }
                })
                .collect::<Vec<_>>()
                .join("   ")
        };
        let mut view = View::default();
        let player = rows.iter().find(|r| r.car == 0);
        let points = |n: i64| {
            if n == 1 {
                l("post_race_stats_drift_score_single")
            } else {
                l("post_race_stats_drift_score_multiple")
                    .replace("[SCORE]", &crate::drift_event::grouped(n))
            }
        };
        match self.stage {
            Stage::Hidden => {}
            // `post_race_stats_drift_scores`: `stat type="drift_total"` [D].
            Stage::Stats if self.drift => {
                let board = board.cloned();
                view.title = l("post_race_stats_drift_total_title");
                view.body = board.map_or_else(String::new, |b| {
                    let drifts = if b.drifts == 1 {
                        l("post_race_stats_drift_amount_single")
                    } else {
                        l("post_race_stats_drift_amount_multiple")
                            .replace("[AMOUNT]", &b.drifts.to_string())
                    };
                    format!("{}\n{drifts}", points(b.total))
                });
                view.pills = format!("Enter/A: {}", l("post_race_stats_buttontext_continue"));
            }
            Stage::Scoreboard | Stage::Confirm(_, Return::Scoreboard) => {
                view.title = l("drift_scoreboard_screen_title");
                if let Some(b) = board {
                    view.body = [
                        (l("drift_scoreboard_fastest_speed"), format!("{:.0} mph", b.fastest)),
                        (l("drift_scoreboard_longest_drift_time"), format!("{:.2} s", b.longest)),
                        (
                            l("drift_scoreboard_largest_combo_score"),
                            crate::drift_event::grouped(b.best_combo as i64),
                        ),
                        (l("drift_scoreboard_average_speed"), format!("{:.0} mph", b.average_speed)),
                        (
                            l("drift_scoreboard_average_distance"),
                            format!("{:.0} m", b.average_distance),
                        ),
                        (l("drift_scoreboard_average_angle"), format!("{:.0}°", b.average_angle)),
                        (l("drift_scoreboard_total_score"), crate::drift_event::grouped(b.total)),
                    ]
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                    view.table.push([
                        l("drift_leaderboard_drift_type_column"),
                        l("drift_leaderboard_drift_count_column"),
                        l("drift_leaderboard_drift_score_column"),
                        String::new(),
                        String::new(),
                        String::new(),
                    ]);
                    for (name, count, score) in &b.ratings {
                        view.table.push([
                            labels.get(name).to_owned(),
                            count.to_string(),
                            crate::drift_event::grouped(*score as i64),
                            String::new(),
                            String::new(),
                            String::new(),
                        ]);
                    }
                }
                view.pills = pill_line(&RESULTS_PILLS, self.cursor);
            }
            Stage::Stats => {
                // `stat type="position"` and `stat type="fastest_lap"` [D].
                let position = player.map_or(0, |r| r.position);
                view.title = l(match position {
                    1 => "post_race_stats_position_winner_title",
                    2 | 3 => "post_race_stats_position_podium_title",
                    _ => "post_race_stats_position_title",
                });
                let mut body = format!("{} / {}", ordinal(position.max(1), &l), rows.len().max(1));
                if let Some((t, lap)) = player.and_then(|r| r.best) {
                    body.push_str(&format!(
                        "\n\n{}  {}\n{}",
                        l("post_race_stats_fastest_lap_title"),
                        crate::race_hud::lap_time(t),
                        l("post_race_stats_fastest_lap_details").replace("[LAPNO]", &lap.to_string())
                    ));
                }
                view.body = body;
                view.pills = format!("Enter/A: {}", l("post_race_stats_buttontext_continue"));
            }
            Stage::Results | Stage::Confirm(_, Return::Results) => {
                view.title = l("post_race_results_screen_title");
                view.body = l(if self.drift {
                    "post_race_drift_results_table_title"
                } else {
                    "post_race_results_table_title"
                });
                let mut header: [String; 6] = std::array::from_fn(|i| {
                    COLUMNS.get(i).map_or_else(|| l("osd_best"), |(id, _)| l(id))
                });
                header[5] = l("post_race_stats_fastest_lap_title");
                if self.drift {
                    // Score in place of the time [I: the drift results
                    // columns were not traced].
                    header[2] = l("drift_leaderboard_drift_score_column");
                    header[5] = String::new();
                }
                let touge = rows.iter().any(|r| r.touge.is_some());
                if touge {
                    // Total Times, Round 1, Round 2 [D strings; layout I].
                    header[2] = l("touge_race_results_table_title");
                    header[4] = l("touge_round_one_results_table_title");
                    header[5] = l("touge_round_two_results_table_title");
                }
                let classes = rows.iter().any(|r| r.class.is_some());
                if classes {
                    // The Le Mans results' Class column [D string
                    // post_race_positions_table_car_class_header; its place
                    // in the layout is I: the team column's].
                    header[4] = l("post_race_positions_table_car_class_header");
                } else if rows.iter().any(|r| r.points.is_some()) {
                    // A career race's Points column [D string
                    // post_race_positions_table_points_column, the first of
                    // its two entries; its place is I: the team column's].
                    header[4] = l("post_race_positions_table_points_column");
                }
                view.table.push(header);
                for (i, row) in rows.iter().enumerate() {
                    if row.car == 0 {
                        view.player_row = Some(i + 1);
                    }
                    view.table.push([
                        row.position.to_string(),
                        row.name.clone(),
                        match row.score {
                            Some(score) => crate::drift_event::grouped(score),
                            None => time_text(row, &l),
                        },
                        row.vehicle.clone(),
                        match (row.touge, &row.class, row.points) {
                            (Some([run, _]), _, _) => run.map_or_else(|| "DNF".into(), crate::race_hud::lap_time),
                            (None, Some(class), _) => labels.get(class).to_owned(),
                            (None, None, Some(points)) => points.to_string(),
                            (None, None, None) => "--".into(),
                        },
                        match (row.touge, row.score) {
                            (Some([_, run]), _) => run.map_or_else(|| "DNF".into(), crate::race_hud::lap_time),
                            (None, Some(_)) => String::new(),
                            (None, None) => row
                                .best
                                .map_or_else(|| "--".into(), |(t, _)| crate::race_hud::lap_time(t)),
                        },
                    ]);
                }
                view.pills = pill_line(&RESULTS_PILLS, self.cursor);
            }
            Stage::Wreck | Stage::Confirm(_, Return::Wreck) => {
                view.title = l(match self.variant {
                    Variant::FreeRace => "terminal_damage_screen_title",
                    Variant::TestDrive => "terminal_damage_test_drive_screen_title",
                });
                view.body = format!(
                    "{}\n{}",
                    l("terminal_damage_screen_info_box_header"),
                    l(match self.variant {
                        Variant::FreeRace => "terminal_damage_screen_info_box_subheader",
                        Variant::TestDrive => "terminal_damage_screen_subheader_test_drive",
                    })
                );
                view.pills = pill_line(self.wreck_pills(), self.cursor);
            }
        }
        if let Stage::Confirm(confirm, _) = self.stage {
            let (title, subtitle) = confirm.strings(self.variant);
            let answers = [
                l("overlay_popup_generic_decline"),
                l("overlay_popup_generic_accept"),
            ];
            view.pills = format!(
                "{} - {}\n{}",
                l(title),
                l(subtitle),
                answers
                    .iter()
                    .enumerate()
                    .map(|(i, a)| if i == self.answer {
                        format!("[ {a} ]")
                    } else {
                        format!("  {a}  ")
                    })
                    .collect::<Vec<_>>()
                    .join("   ")
            );
        }
        view
    }
}

/// The event resources the results depend on.
type Events<'w> = (
    Option<Res<'w, crate::time_trial::Trial>>,
    Option<Res<'w, crate::drift_event::DriftEvent>>,
    Option<Res<'w, crate::touge_event::TougeEvent>>,
    Option<Res<'w, CarClasses>>,
    Option<Res<'w, crate::le_mans_event::LeMansEvent>>,
    Option<Res<'w, RacePoints>>,
);

fn present(
    post: Res<PostRace>,
    labels: Res<Labels>,
    driving: Res<crate::Driving>,
    (trial, drift, touge, classes, le_mans, points): Events,
    mut panel: Query<&mut Visibility, With<Panel>>,
    mut parts: Query<(&Part, &mut Text, &mut TextColor, &mut Node)>,
    mut shown: Local<Option<View>>,
) {
    for mut visibility in &mut panel {
        let want = if post.active() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
    if !post.active() {
        *shown = None;
        return;
    }
    let mut rows = table(&driving, trial.as_deref());
    let board = drift.as_ref().and_then(|d| d.board());
    if let Some(d) = &drift {
        rows = d.results(driving.opponents.as_ref().map(|o| &o.standings), rows);
    }
    if let Some(t) = &touge {
        rows = t.results(rows);
    }
    if let Some(c) = &classes {
        for row in &mut rows {
            row.class = c.0.get(row.car).cloned();
        }
    }
    if let Some(e) = &le_mans {
        rows = e.results(driving.opponents.as_ref().map(|o| &o.standings), rows);
    }
    if let Some(p) = &points {
        for row in &mut rows {
            row.points = Some(p.0.get(row.position.saturating_sub(1)).copied().unwrap_or(0));
        }
    }
    let view = post.view_with(&rows, &labels, board.as_ref());
    if shown.as_ref() == Some(&view) {
        return;
    }
    // The shown table, one line per row (unattended runs check it).
    if matches!(post.stage, Stage::Results) {
        for (i, row) in view.table.iter().enumerate() {
            log_line!("drive_post_race_table version=1 row={i} cells={}", row.join("|"));
        }
    }
    for (part, mut text, mut colour, mut node) in &mut parts {
        let value = match part {
            Part::Title => view.title.clone(),
            Part::Body => view.body.clone(),
            Part::Pills => view.pills.clone(),
            Part::Table(cell) => {
                let (row, column) = (cell / 8, cell % 8);
                let value = view.table.get(row).map(|r| r[column].clone());
                node.display = if value.is_some() {
                    Display::Flex
                } else {
                    Display::None
                };
                colour.0 = if Some(row) == view.player_row {
                    Color::srgb(1., 0.78, 0.2)
                } else if row == 0 {
                    Color::srgb(0.6, 0.65, 0.7)
                } else {
                    Color::WHITE
                };
                value.unwrap_or_default()
            }
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    *shown = Some(view);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(f: impl Fn(&mut Nav)) -> Nav {
        let mut nav = Nav::default();
        f(&mut nav);
        nav
    }

    #[test]
    fn finish_shows_stats_then_results_then_leaves() {
        let mut post = PostRace::new(Variant::FreeRace);
        assert_eq!(post.step(press(|n| n.select = true)), None);
        post.on_finish();
        assert_eq!(post.stage, Stage::Stats);
        assert_eq!(post.step(press(|n| n.select = true)), Some(Command::Finalize));
        assert_eq!(post.stage, Stage::Results);
        // Continue leaves the event.
        assert_eq!(post.step(press(|n| n.select = true)), Some(Command::Exit));
    }

    #[test]
    fn results_pills_confirm_exit_and_restart() {
        let mut post = PostRace::new(Variant::FreeRace);
        post.on_retire();
        assert_eq!(post.stage, Stage::Results);
        // View Replay: the whole race.
        post.step(press(|n| n.right = true));
        assert_eq!(
            post.step(press(|n| n.select = true)),
            Some(Command::Replay(crate::replay::Kind::Full))
        );
        assert_eq!(post.stage, Stage::Results);
        post.step(press(|n| n.right = true));
        post.step(press(|n| n.select = true));
        assert_eq!(post.stage, Stage::Confirm(Confirm::Exit, Return::Results));
        // Back returns to the results.
        post.step(press(|n| n.back = true));
        assert_eq!(post.stage, Stage::Results);
        post.step(press(|n| n.right = true));
        post.step(press(|n| n.select = true));
        assert_eq!(post.stage, Stage::Confirm(Confirm::Restart, Return::Results));
        assert_eq!(post.step(press(|n| n.select = true)), Some(Command::Restart));
        assert!(!post.active());
    }

    /// Pill moves, activates and the confirmation popup's sounds; a hidden
    /// screen stays silent.
    #[test]
    fn steps_queue_the_widget_and_popup_sounds() {
        use grid_tools::hud_audio::Cue;
        let mut post = PostRace::new(Variant::FreeRace);
        post.step(press(|n| n.select = true));
        assert!(post.sounds.is_empty());
        post.on_retire();
        post.step(press(|n| n.right = true));
        post.step(press(|n| n.right = true));
        post.step(press(|n| n.select = true));
        assert_eq!(post.stage, Stage::Confirm(Confirm::Exit, Return::Results));
        post.step(press(|n| n.left = true));
        post.step(press(|n| n.back = true));
        post.step(press(|n| n.right = true));
        post.step(press(|n| n.select = true));
        assert_eq!(post.stage, Stage::Confirm(Confirm::Restart, Return::Results));
        assert_eq!(post.step(press(|n| n.select = true)), Some(Command::Restart));
        assert_eq!(
            post.sounds,
            [
                Cue::UdlrMove,
                Cue::UdlrMove,
                Cue::Select,
                Cue::OverlayOn,
                Cue::UdlrMove,
                Cue::Back,
                Cue::OverlayOff,
                Cue::UdlrMove,
                Cue::Select,
                Cue::OverlayOn,
                Cue::Select,
                Cue::OverlayOff,
            ]
        );
    }

    #[test]
    fn drift_events_show_the_scoreboard_before_the_results() {
        let mut post = PostRace::new(Variant::FreeRace);
        post.drift = true;
        post.on_finish();
        assert_eq!(post.step(press(|n| n.select = true)), Some(Command::Finalize));
        assert_eq!(post.stage, Stage::Scoreboard);
        // Restart's confirmation returns to the scoreboard.
        for _ in 0..3 {
            post.step(press(|n| n.right = true));
        }
        post.step(press(|n| n.select = true));
        assert_eq!(post.stage, Stage::Confirm(Confirm::Restart, Return::Scoreboard));
        post.step(press(|n| n.back = true));
        assert_eq!(post.stage, Stage::Scoreboard);
        // Continue (first pill): the drift results, then out of the event.
        post.step(press(|n| n.right = true));
        assert_eq!(post.step(press(|n| n.select = true)), None);
        assert_eq!(post.stage, Stage::Results);
        assert_eq!(post.step(press(|n| n.select = true)), Some(Command::Exit));
        // Retire goes to the scoreboard too; a restart keeps the discipline.
        post.reset(1);
        assert!(post.drift && !post.active());
        post.on_retire();
        assert_eq!(post.stage, Stage::Scoreboard);
        let board = crate::drift_event::Board {
            total: 12345,
            drifts: 3,
            fastest: 71.,
            longest: 4.5,
            best_combo: 9000.,
            average_speed: 60.,
            average_distance: 80.,
            average_angle: 33.,
            ratings: vec![("osd_drift".into(), 2, 300.), ("osd_great_drift".into(), 1, 2000.)],
        };
        let view = post.view_with(&[], &Labels::default(), Some(&board));
        assert_eq!(view.title, "drift_scoreboard_screen_title");
        assert!(view.body.ends_with("drift_scoreboard_total_score: 12,345"), "{}", view.body);
        assert_eq!(view.table.len(), 3);
        assert_eq!(view.table[2][..3], ["osd_great_drift", "1", "2,000"]);
        post.stage = Stage::Stats;
        let view = post.view_with(&[], &Labels::default(), Some(&board));
        assert_eq!(view.title, "post_race_stats_drift_total_title");
        assert_eq!(
            view.body,
            "post_race_stats_drift_score_multiple\npost_race_stats_drift_amount_multiple"
        );
    }

    #[test]
    fn wreck_transitions_slow_the_game_then_ramp_back() {
        let mut post = PostRace::new(Variant::FreeRace);
        assert_eq!(post.wreck_transition(), None);
        post.on_wreck(true, false, 0.5);
        assert_eq!(post.wreck_transition(), Some(0.5));
        assert_eq!(wreck_time_scale(0.5), WRECK_SLOW);
        assert!((wreck_time_scale(1.25) - 0.35).abs() < 1e-6);
        assert_eq!(wreck_time_scale(1.5), 1.);
        post.on_wreck(true, false, 1.5);
        assert_eq!(post.wreck_transition(), None);
    }

    #[test]
    fn wreck_screen_replay_is_the_terminal_damage_replay() {
        let mut post = PostRace::new(Variant::FreeRace);
        post.on_wreck(true, true, 2.0);
        post.on_wreck(true, true, 1.0);
        assert_eq!(post.stage, Stage::Wreck);
        // The crash replay plays first, once [O: flow.bin
        // `terminal_damage_delay` -> `instant_race_replay_silent`].
        assert!(std::mem::take(&mut post.crash_replay));
        post.on_wreck(true, true, 1.0);
        assert!(!post.crash_replay);
        post.step(press(|n| n.down = true));
        post.step(press(|n| n.down = true));
        assert_eq!(post.step(press(|n| n.select = true)), Some(Command::TerminalReplay));
        assert_eq!(post.stage, Stage::Wreck);
    }

    #[test]
    fn wreck_screen_after_the_rest_wait_and_retire_leaves() {
        // A car at rest: the 2 s transitions, then 1 s.
        let mut post = PostRace::new(Variant::FreeRace);
        post.on_wreck(true, true, 1.5);
        post.on_wreck(true, true, 0.5);
        assert!(!post.active());
        post.on_wreck(true, true, 0.9);
        assert!(!post.active());
        post.on_wreck(true, true, 0.1);
        assert_eq!(post.stage, Stage::Wreck);
        post.step(press(|n| n.down = true));
        post.step(press(|n| n.select = true));
        assert_eq!(post.stage, Stage::Confirm(Confirm::RetireRace, Return::Wreck));
        assert_eq!(
            post.step(press(|n| n.select = true)),
            Some(Command::RetireAndExit)
        );
        let mut td = PostRace::new(Variant::TestDrive);
        // Still sliding: the rest wait gives up after 2 s, then 1 s.
        for _ in 0..49 {
            td.on_wreck(true, false, 0.1);
        }
        assert!(!td.active());
        td.on_wreck(true, false, 0.3);
        assert!(td.active());
        td.step(press(|n| n.down = true));
        td.step(press(|n| n.select = true));
        assert_eq!(td.stage, Stage::Confirm(Confirm::FinishTestDrive, Return::Wreck));
        assert_eq!(td.step(press(|n| n.select = true)), Some(Command::Exit));
        // A restart clears everything.
        td.reset(3);
        assert!(!td.active());
    }

    /// Moves `car` once round a 1000 m loop, crossing the line at `time`.
    fn lap(s: &mut Standings, arcs: &mut [f64; 4], car: usize, time: f64) {
        for k in 1..=4 {
            arcs[car] = (arcs[car] + 250.) % 1000.;
            s.step_arcs(&arcs[..], time - 4. + f64::from(k), &[0]);
        }
    }

    #[test]
    fn results_rows_order_gaps_out_and_best_laps() {
        let mut s = Standings::on_loop(2, 1000., &[0.; 4]);
        let mut arcs = [0.; 4];
        // Car 2 wins in 100 s (laps 50, 50); the player, car 0, is second in
        // 101.5 s; car 1 is estimated; car 3 retires after one lap of 60 s.
        lap(&mut s, &mut arcs, 2, 50.);
        lap(&mut s, &mut arcs, 2, 100.);
        lap(&mut s, &mut arcs, 0, 49.);
        lap(&mut s, &mut arcs, 0, 101.5);
        s.finish(0, 101.5);
        lap(&mut s, &mut arcs, 3, 60.);
        s.retire(3, Out::Retired);
        arcs[1] = 900.;
        s.step_arcs(&arcs, 101.9, &[0]);
        s.finalize(&[0., 30., 0., 0.], 102., &[0]);
        assert_eq!(s.status(2), Status::Finished);
        let names: Vec<String> = (0..4).map(|i| format!("car {i}")).collect();
        let vehicles = vec!["Lacetti".to_string(); 4];
        let rows = rows(&s, &names, &vehicles, Some((48.5, 1)));
        let order: Vec<usize> = rows.iter().map(|r| r.car).collect();
        assert_eq!(order, [2, 0, 1, 3]);
        assert_eq!(rows[0].time, Cell::Total(100.));
        assert_eq!(rows[1].time, Cell::Gap(1.5));
        assert!(matches!(rows[2].time, Cell::Gap(g) if g > 2.) && rows[2].estimated);
        assert_eq!(rows[3].time, Cell::Out(Out::Retired, 1));
        assert_eq!(rows[0].best, Some((50., 1)));
        // The player's gate-timed best lap replaces the line-timed one.
        assert_eq!(rows[1].best, Some((48.5, 1)));
        assert_eq!(rows[2].best, None);
        assert_eq!(rows[3].best, Some((60., 1)));
        assert_eq!(
            rows.iter().map(|r| r.position).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(rows[1].name, "car 0");
    }

    #[test]
    fn pro_touge_results_show_both_runs_and_the_total() {
        let mut post = PostRace::new(Variant::FreeRace);
        post.stage = Stage::Results;
        let row = |position, car, time, touge| Row {
            position,
            car,
            name: format!("car {car}"),
            time,
            estimated: false,
            best: None,
            vehicle: "Lacetti".into(),
            score: None,
            touge: Some(touge),
            class: None,
            points: None,
        };
        let rows = [
            row(1, 1, Cell::Total(299.136), [Some(152.272), Some(146.864)]),
            row(2, 0, Cell::Gap(3.072), [Some(153.968), None]),
        ];
        let view = post.view(&rows, &Labels::default());
        assert_eq!(view.table[0][2], "touge_race_results_table_title");
        assert_eq!(view.table[0][4], "touge_round_one_results_table_title");
        assert_eq!(view.table[0][5], "touge_round_two_results_table_title");
        assert_eq!(view.table[1][4], crate::race_hud::lap_time(152.272));
        assert_eq!(view.table[1][5], crate::race_hud::lap_time(146.864));
        assert_eq!(view.table[2][5], "DNF");
        assert_eq!(view.player_row, Some(2));
    }

    #[test]
    fn le_mans_results_show_the_class_column() {
        let mut post = PostRace::new(Variant::FreeRace);
        post.stage = Stage::Results;
        let row = |position, car, class: &str| Row {
            position,
            car,
            name: format!("car {car}"),
            time: Cell::Running,
            estimated: false,
            best: None,
            vehicle: "Aston Martin DBR9".into(),
            score: None,
            touge: None,
            class: Some(class.into()),
            points: None,
        };
        let rows: Vec<Row> = (0..16)
            .map(|i| row(i + 1, i, if i < 3 { "db_lmp1" } else { "db_gt1" }))
            .collect();
        let view = post.view(&rows, &Labels::default());
        assert_eq!(view.table[0][4], "post_race_positions_table_car_class_header");
        assert_eq!(view.table[1][4], "db_lmp1");
        assert_eq!(view.table[16][4], "db_gt1");
        // Header plus 16 cars fit the table.
        assert!(view.table.len() <= TABLE_ROWS);
    }

    #[test]
    fn career_results_show_the_points_column() {
        let mut post = PostRace::new(Variant::FreeRace);
        post.stage = Stage::Results;
        let rows: Vec<Row> = (0..12)
            .map(|i| Row {
                position: i + 1,
                car: i,
                name: format!("car {i}"),
                time: Cell::Running,
                estimated: false,
                best: None,
                vehicle: "BMW 320si".into(),
                score: None,
                touge: None,
                class: None,
                points: Some([20, 18, 16][..].get(i).copied().unwrap_or(0)),
            })
            .collect();
        let view = post.view(&rows, &Labels::default());
        assert_eq!(view.table[0][4], "post_race_positions_table_points_column");
        assert_eq!((view.table[1][4].as_str(), view.table[3][4].as_str()), ("20", "16"));
        assert_eq!(view.table[12][4], "0");
    }

    #[test]
    fn ordinals_follow_the_english_suffixes() {
        let l = |id: &'static str| {
            match id {
                "ordinalsuffix_1l" => "st",
                "ordinalsuffix_2l" => "nd",
                "ordinalsuffix_3l" => "rd",
                _ => "th",
            }
            .to_string()
        };
        let got: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22].iter().map(|&n| ordinal(n, &l)).collect();
        assert_eq!(got, ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd"]);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn screens_match_the_original_states_file() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        let bytes = std::fs::read(root.join("system/states.bin")).unwrap();
        let doc = grid_formats::binxml::read(&bytes).unwrap();
        let attr = |i: usize, key: &str| {
            doc.elements[i]
                .attributes
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| *v)
        };
        let child = |i: usize, name: &str| {
            doc.elements[i]
                .children
                .clone()
                .find(|&c| doc.elements[c].name == name)
        };
        let pills = |id: &str| -> Vec<(String, String)> {
            let s = (0..doc.elements.len())
                .find(|&i| attr(i, "id") == Some(id))
                .unwrap_or_else(|| panic!("{id}"));
            let flow = child(s, "itemflow").unwrap();
            doc.elements[flow]
                .value
                .unwrap()
                .split_whitespace()
                .map(|item| {
                    let e = doc.elements[s]
                        .children
                        .clone()
                        .find(|&c| attr(c, "id") == Some(item))
                        .unwrap();
                    let link = child(e, "IBSelectableSimple").and_then(|c| attr(c, "link"));
                    let string = child(e, "IBTextStatic").and_then(|c| attr(c, "string"));
                    (link.unwrap().to_owned(), string.unwrap().to_owned())
                })
                .collect()
        };
        let link = |p: Pill, variant: Variant| match p {
            Pill::Continue => "continue",
            Pill::Replay => "replay",
            Pill::Exit => "save_and_exit",
            Pill::Restart => "restart",
            Pill::Retire if variant == Variant::FreeRace => "exit",
            Pill::Retire => "exit",
        };
        for (id, list, variant) in [
            ("post_race_results_generic", &RESULTS_PILLS[..], Variant::FreeRace),
            ("fr_terminal_damage", &WRECK_PILLS[..], Variant::FreeRace),
            ("td_terminal_damage", &TEST_DRIVE_WRECK_PILLS[..], Variant::TestDrive),
        ] {
            let ours: Vec<(String, String)> = list
                .iter()
                .map(|(p, s)| (link(*p, variant).to_owned(), (*s).to_owned()))
                .collect();
            assert_eq!(pills(id), ours, "{id}");
        }
        // The free-race stats screen shows the position and the fastest lap.
        let s = (0..doc.elements.len())
            .find(|&i| attr(i, "id") == Some("post_race_stats_free_race"))
            .unwrap();
        let stats: Vec<&str> = doc.elements[s]
            .children
            .clone()
            .filter(|&c| doc.elements[c].name == "stat")
            .filter_map(|c| attr(c, "type"))
            .collect();
        assert_eq!(stats, ["position", "fastest_lap"]);
        let labels = Labels::load(root);
        let language = labels.0.as_ref().unwrap();
        for id in RESULTS_PILLS
            .iter()
            .chain(&WRECK_PILLS)
            .chain(&TEST_DRIVE_WRECK_PILLS)
            .map(|(_, s)| *s)
            .chain(COLUMNS.iter().map(|(s, _)| *s))
        {
            assert!(language.get(id).is_some(), "{id}");
        }
    }

    #[test]
    fn results_times_use_the_original_formats() {
        assert_eq!(results_gap(3.072), "+03.07");
        assert_eq!(results_gap(83.5), "+01:23.50");
        assert_eq!(results_gap(0.), " ");
        let row = |time| Row { time, ..solo_row("You", "car", None) };
        let l = |k: &'static str| k.to_owned();
        assert_eq!(time_text(&row(Cell::Total(299.136)), &l), "04:59.14");
        assert_eq!(time_text(&row(Cell::Gap(3.072)), &l), "+03.07");
    }
}
