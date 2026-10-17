//! The original's in-race pause menu ([research](../../../docs/PAUSE_MENU.md)).
//!
//! Structure [D]: `system/states.bin` screens `pause_mainmenu_free_race` and
//! `pause_mainmenu_test_drive` (`ScreenPauseMainMenu`), the options screens
//! `pause_settings_standard` / `pause_settings_wheel_no_ffb`, the sound, screen
//! and advanced-controls screens, and the `ScreenYesNo` confirmations; flow
//! `system/flow.bin` branch `f_pause` (Race Day free race) for where each link
//! goes. Labels are read at run time from the user's `language_eng.lng`.
//!
//! The menu is a pure state machine ([`PauseMenu::step`]) driven by the input
//! layer's menu actions; [`update`] applies its commands to the race. While it
//! is open `Driving::paused` holds every fixed tick, so the simulation is
//! frozen bit for bit and resumes exactly where it stopped.
//!
//! AUTHORED presentation: a plain panel instead of the original's animated
//! `in_game_N_button` glyphs, post-process blur and paused-world camera.
use crate::input::{Action, Actions, ShiftQueue};
use bevy::prelude::*;
use grid_formats::language::Language;

/// Original menu screen for the app's event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Race Day races (flow `f_`), timed events with or without opponents.
    FreeRace,
    /// Free practice: the nearest original event is a test drive.
    TestDrive,
}

/// `IBSelectableSimple link` of a main-menu button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// `BACK`: back to the race.
    Back,
    InstantReplay,
    Restart,
    Settings,
    RetireRace,
    /// `exit`: finish the test drive.
    Exit,
    GhostCar,
}

/// One main-menu button: its link and language string id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    pub link: Link,
    pub string: &'static str,
}

const fn button(link: Link, string: &'static str) -> Button {
    Button { link, string }
}

/// `pause_mainmenu_free_race`, itemflow order [D].
pub const FREE_RACE: [Button; 5] = [
    button(Link::Back, "pause_mainmenu_button_continue"),
    button(Link::InstantReplay, "pause_mainmenu_button_instant_replay"),
    button(Link::Restart, "pause_mainmenu_button_restart"),
    button(Link::Settings, "pause_mainmenu_button_options"),
    button(Link::RetireRace, "pause_mainmenu_button_retire_race"),
];

/// `pause_mainmenu_test_drive`, itemflow order [D].
pub const TEST_DRIVE: [Button; 6] = [
    button(Link::Back, "pause_mainmenu_button_continue"),
    button(Link::InstantReplay, "pause_mainmenu_button_instant_replay"),
    button(Link::Restart, "pause_mainmenu_button_restart_test_drive"),
    button(Link::Settings, "pause_mainmenu_button_options"),
    button(Link::Exit, "pause_mainmenu_button_finish_test_drive"),
    button(Link::GhostCar, "test_track_select_ghost_car_title"),
];

/// Not in the prototype yet: shown, not selectable.
fn available(link: Link) -> bool {
    !matches!(link, Link::GhostCar)
}

/// Options screen links (`pause_settings_standard`, `_wheel_no_ffb`) [D].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setup {
    Sound,
    Screen,
    /// Wheels only.
    Advanced,
}

/// `ScreenYesNo` confirmations [D].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Restart,
    RetireRace,
    FinishTestDrive,
    ResetCar,
    /// Results screen Exit (`exit_race_confirm`).
    Exit,
}

impl Confirm {
    /// (title, subtitle) string ids.
    pub fn strings(self, variant: Variant) -> (&'static str, &'static str) {
        match (self, variant) {
            (Self::Restart, Variant::TestDrive) => (
                "restart_test_drive_confirm_title",
                "restart_test_drive_confirm_subtitle",
            ),
            (Self::Restart, _) => (
                "restart_race_confirm_title",
                "restart_race_confirm_subtitle",
            ),
            (Self::RetireRace, _) => ("retire_race_confirm_title", "retire_race_confirm_subtitle"),
            (Self::FinishTestDrive, _) => (
                "end_test_drive_confirm_title",
                "end_test_drive_confirm_subtitle",
            ),
            (Self::ResetCar, _) => (
                "reset_vehicle_confirm_title",
                "reset_vehicle_confirm_subtitle",
            ),
            (Self::Exit, _) => ("quit_race_confirm_title", "quit_race_confirm_subtitle"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Main,
    Confirm(Confirm),
    Options,
    Setup(Setup),
}

/// The values the pause options change. Speech and music volume, steering
/// linearity and 900 degree mode have no counterpart in the prototype.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// `volume_soundfx`, percent: engine and tyre sound.
    pub effects: u8,
    /// `osd_laptimer`: lap and times block of the HUD.
    pub lap_timer: bool,
    /// `osd_position`: race position block of the HUD.
    pub position: bool,
    /// `imperialunits`: mph instead of km/h.
    pub mph: bool,
    /// `osd_minimap`: the HUD's minimap.
    pub minimap: bool,
    /// `osd_dials`: the HUD's dial (speed, gear, revs).
    pub dials: bool,
    /// Advanced controls, percent: steering, accelerate and brake dead zone
    /// and saturation.
    pub tunes: [[u8; 2]; 3],
}

impl Default for Options {
    /// The prototype's values before this menu existed [I for the original's
    /// profile defaults, which were not traced].
    fn default() -> Self {
        Self {
            effects: 100,
            lap_timer: true,
            position: true,
            mph: false,
            minimap: true,
            dials: true,
            tunes: [[20, 100], [0, 100], [0, 100]],
        }
    }
}

impl Options {
    /// Effects volume as a linear gain.
    pub fn gain(&self) -> f32 {
        f32::from(self.effects) / 100.
    }
}

/// One frame's menu input: pressed edges, directions with the original's
/// repeat (`ActionMap/repeatSettings.xml`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nav {
    pub pause: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub select: bool,
    pub back: bool,
    pub button3: bool,
}

/// What the race has to do after a menu step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Open,
    Resume,
    Restart,
    RetireRace,
    Exit,
    ResetCar,
    /// `instant_replay`: the last 10 s (docs/REPLAY.md); the menu stays
    /// open underneath and shows again after Replay Exit.
    InstantReplay,
    /// Options changed (applied at once).
    Apply(Options),
    /// A button whose feature is not in the prototype.
    Unavailable(&'static str),
}

/// Sub-screen rows: (language id, what the row edits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Effects,
    Speech,
    Music,
    LapTimer,
    Position,
    MiniMap,
    Dials,
    SpeedUnit,
    Tune(usize, usize),
    Linearity,
    MultiTurn,
}

const SOUND: [(&str, Field); 3] = [
    ("sound_setup_effects_volume_slider_title", Field::Effects),
    ("sound_setup_speech_volume_slider_title", Field::Speech),
    ("sound_setup_music_volume_slider_title", Field::Music),
];
const SCREEN: [(&str, Field); 5] = [
    ("osd_options_lap_timer_toggle_title", Field::LapTimer),
    ("osd_options_position_toggle_title", Field::Position),
    ("osd_options_mini_map_toggle_title", Field::MiniMap),
    ("osd_options_dials_toggle_title", Field::Dials),
    ("osd_options_speed_unit_select_item_title", Field::SpeedUnit),
];
const ADVANCED: [(&str, Field); 8] = [
    ("controllerSetup_steerDeadzone", Field::Tune(0, 0)),
    ("controllerSetup_steerSaturation", Field::Tune(0, 1)),
    ("controllerSetup_wheelLinearity", Field::Linearity),
    ("controllerSetup_accDeadzone", Field::Tune(1, 0)),
    ("controllerSetup_accSaturation", Field::Tune(1, 1)),
    ("controllerSetup_brkDeadzone", Field::Tune(2, 0)),
    ("controllerSetup_brkSaturation", Field::Tune(2, 1)),
    ("900_degree_mode_toggle_header", Field::MultiTurn),
];

fn fields(setup: Setup) -> &'static [(&'static str, Field)] {
    match setup {
        Setup::Sound => &SOUND,
        Setup::Screen => &SCREEN,
        Setup::Advanced => &ADVANCED,
    }
}

/// Slider steps, percent (AUTHORED: the original's discrete slider step
/// counts were not traced).
const VOLUME_STEP: u8 = 10;
const TUNE_STEP: u8 = 5;

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PauseMenu {
    pub variant: Variant,
    /// Wheel: the options screen has Advanced (`pause_settings_wheel_no_ffb`).
    pub wheel: bool,
    /// The race has a reset line (Button3 = Reset Vehicle).
    pub can_reset: bool,
    pub open: bool,
    pub screen: Screen,
    main: usize,
    options: usize,
    row: usize,
    /// 0 = No, 1 = Yes; `active_default="0.library.button_1"` [D].
    answer: usize,
    pub values: Options,
    /// Values on entering a setup screen (Button3 = Undo).
    undo: Options,
    repeat: Repeat,
}

impl PauseMenu {
    pub fn new(variant: Variant, wheel: bool, can_reset: bool, values: Options) -> Self {
        Self {
            variant,
            wheel,
            can_reset,
            open: false,
            screen: Screen::Main,
            main: 0,
            options: 0,
            row: 0,
            answer: 1,
            values,
            undo: values,
            repeat: Repeat::default(),
        }
    }

    pub fn buttons(&self) -> &'static [Button] {
        match self.variant {
            Variant::FreeRace => &FREE_RACE,
            Variant::TestDrive => &TEST_DRIVE,
        }
    }

    pub fn setups(&self) -> &'static [Setup] {
        if self.wheel {
            &[Setup::Sound, Setup::Screen, Setup::Advanced]
        } else {
            &[Setup::Sound, Setup::Screen]
        }
    }

    fn close(&mut self) {
        self.open = false;
        self.screen = Screen::Main;
    }

    /// One frame of menu input. Closed, only Pause does anything (it opens on
    /// Continue, `c_pause_initial_decision_maker` -> main menu).
    pub fn step(&mut self, nav: Nav) -> Option<Command> {
        if !self.open {
            if nav.pause {
                self.open = true;
                self.screen = Screen::Main;
                self.main = 0;
                return Some(Command::Open);
            }
            return None;
        }
        // Keyboard Escape is both Pause and Menu Back: one step back.
        let back = nav.back || nav.pause;
        let wrap = |i: usize, n: usize, up: bool| if up { (i + n - 1) % n } else { (i + 1) % n };
        match self.screen {
            Screen::Main => {
                let buttons = self.buttons();
                if back {
                    self.close();
                    return Some(Command::Resume);
                }
                if nav.up || nav.down {
                    self.main = wrap(self.main, buttons.len(), nav.up);
                }
                if nav.button3 && self.can_reset {
                    self.confirm(Confirm::ResetCar);
                    return None;
                }
                if !nav.select {
                    return None;
                }
                let b = buttons[self.main];
                if !available(b.link) {
                    return Some(Command::Unavailable(b.string));
                }
                match b.link {
                    Link::Back => {
                        self.close();
                        return Some(Command::Resume);
                    }
                    Link::Restart => self.confirm(Confirm::Restart),
                    Link::Settings => {
                        self.screen = Screen::Options;
                        self.options = 0;
                    }
                    Link::RetireRace => self.confirm(Confirm::RetireRace),
                    Link::Exit => self.confirm(Confirm::FinishTestDrive),
                    Link::InstantReplay => return Some(Command::InstantReplay),
                    Link::GhostCar => {}
                }
                None
            }
            Screen::Confirm(confirm) => {
                if nav.left || nav.right || nav.up || nav.down {
                    self.answer = 1 - self.answer;
                }
                if back || (nav.select && self.answer == 0) {
                    self.screen = Screen::Main;
                    return None;
                }
                if !nav.select {
                    return None;
                }
                self.close();
                Some(match confirm {
                    Confirm::Restart => Command::Restart,
                    Confirm::RetireRace => Command::RetireRace,
                    Confirm::FinishTestDrive | Confirm::Exit => Command::Exit,
                    Confirm::ResetCar => Command::ResetCar,
                })
            }
            Screen::Options => {
                let setups = self.setups();
                if back {
                    self.screen = Screen::Main;
                    return None;
                }
                if nav.up || nav.down {
                    self.options = wrap(self.options, setups.len(), nav.up);
                }
                if nav.select {
                    self.screen = Screen::Setup(setups[self.options]);
                    self.row = 0;
                    self.undo = self.values;
                }
                None
            }
            Screen::Setup(setup) => {
                let rows = fields(setup);
                if back {
                    self.screen = Screen::Options;
                    return None;
                }
                if nav.button3 && self.values != self.undo {
                    self.values = self.undo;
                    return Some(Command::Apply(self.values));
                }
                if nav.up || nav.down {
                    self.row = wrap(self.row, rows.len(), nav.up);
                }
                if !(nav.left || nav.right) {
                    return None;
                }
                let before = self.values;
                change(&mut self.values, rows[self.row].1, nav.right);
                (self.values != before).then_some(Command::Apply(self.values))
            }
        }
    }

    fn confirm(&mut self, confirm: Confirm) {
        self.screen = Screen::Confirm(confirm);
        self.answer = 1;
    }
}

fn step_percent(v: u8, step: u8, up: bool, lo: u8, hi: u8) -> u8 {
    if up {
        v.saturating_add(step).min(hi)
    } else {
        v.saturating_sub(step).max(lo)
    }
}

fn change(values: &mut Options, field: Field, up: bool) {
    match field {
        Field::Effects => values.effects = step_percent(values.effects, VOLUME_STEP, up, 0, 100),
        // Combo boxes wrap (`wrap="true"`): left and right both toggle.
        Field::LapTimer => values.lap_timer = !values.lap_timer,
        Field::Position => values.position = !values.position,
        Field::MiniMap => values.minimap = !values.minimap,
        Field::Dials => values.dials = !values.dials,
        Field::SpeedUnit => values.mph = !values.mph,
        Field::Tune(group, which) => {
            let [dz, sat] = values.tunes[group];
            // A dead zone must stay below its saturation (Advanced Controls).
            values.tunes[group] = if which == 0 {
                [step_percent(dz, TUNE_STEP, up, 0, sat - TUNE_STEP), sat]
            } else {
                [dz, step_percent(sat, TUNE_STEP, up, dz + TUNE_STEP, 100)]
            };
        }
        Field::Speech
        | Field::Music
        | Field::Linearity
        | Field::MultiTurn => {}
    }
}

/// Held direction repeat: first repeat after 0.35 s, then every 0.15 s
/// (`<RepeatRateSetting delay="0.35" rate="0.15">`) [D].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Repeat {
    held: Option<usize>,
    next: f32,
}

const REPEAT_DELAY: f32 = 0.35;
const REPEAT_RATE: f32 = 0.15;

impl Repeat {
    /// `down[i]` is whether direction i is held; returns the edges to act on.
    fn update(&mut self, down: [bool; 4], dt: f32) -> [bool; 4] {
        let mut out = [false; 4];
        let held = down.iter().position(|&d| d);
        if held != self.held {
            self.held = held;
            self.next = REPEAT_DELAY;
            if let Some(i) = held {
                out[i] = true;
            }
        } else if let Some(i) = held {
            self.next -= dt;
            if self.next <= 0. {
                self.next += REPEAT_RATE;
                out[i] = true;
            }
        }
        out
    }
}

/// Nav from the input layer's actions (keyboard and pad alike).
pub(crate) fn nav(actions: &Actions, repeat: &mut Repeat, dt: f32) -> Nav {
    use Action::*;
    let [up, down, left, right] = repeat.update(
        [MenuUp, MenuDown, MenuLeft, MenuRight].map(|a| actions.down(a)),
        dt,
    );
    Nav {
        pause: actions.pressed(Pause),
        up,
        down,
        left,
        right,
        select: actions.pressed(MenuSelect),
        back: actions.pressed(MenuBack),
        button3: actions.pressed(MenuButton3),
    }
}

/// UI text for the menu (the user's own language table; ids as fallback).
#[derive(Resource, Default)]
pub struct Labels(pub Option<Language>);

impl Labels {
    pub fn load(root: &std::path::Path) -> Self {
        let path = root.join("language").join("language_eng.lng");
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| grid_formats::language::read(&b).map_err(|e| e.to_string()))
        {
            Ok(language) => Self(Some(language)),
            Err(error) => {
                warn!("Menu labels unavailable, showing string ids: {error}");
                Self(None)
            }
        }
    }

    pub fn get<'a>(&'a self, id: &'a str) -> &'a str {
        self.0.as_ref().and_then(|l| l.get(id)).unwrap_or(id).trim()
    }
}

/// What the panel shows: title, subtitle, rows (text, value, enabled,
/// selected) and the button help line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub title: String,
    pub subtitle: String,
    pub rows: Vec<(String, String, bool, bool)>,
    pub help: String,
}

impl PauseMenu {
    pub fn view(&self, labels: &Labels) -> View {
        let l = |id: &str| labels.get(id).to_owned();
        let on = |b: bool| l(if b { "on_off_on" } else { "on_off_off" });
        let mut view = View::default();
        // Overlay info items [D]: select, back = "Back", button3 = "Reset Vehicle".
        let help = |select: bool, back: &str, button3: Option<&str>| {
            let mut parts = Vec::new();
            if select {
                parts.push(format!("Enter/A: {}", l("default_buttontext_select")));
            }
            parts.push(format!("Esc/B: {}", l(back)));
            if let Some(id) = button3 {
                parts.push(format!("Ctrl/X: {}", l(id)));
            }
            parts.join("    ")
        };
        match self.screen {
            Screen::Main => {
                view.title = l("pause_mainmenu_screen_title");
                view.rows = self
                    .buttons()
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        (
                            l(b.string),
                            String::new(),
                            available(b.link),
                            i == self.main,
                        )
                    })
                    .collect();
                view.help = help(
                    true,
                    "pause_return_to_race",
                    self.can_reset.then_some("pause_reset_to_track"),
                );
            }
            Screen::Confirm(confirm) => {
                let (title, subtitle) = confirm.strings(self.variant);
                view.title = l(title);
                view.subtitle = l(subtitle);
                view.rows = [
                    "overlay_popup_generic_decline",
                    "overlay_popup_generic_accept",
                ]
                .iter()
                .enumerate()
                .map(|(i, id)| (l(id), String::new(), true, i == self.answer))
                .collect();
                view.help = help(true, "default_buttontext_back", None);
            }
            Screen::Options => {
                view.title = l("pause_options_screen_title");
                view.rows = self
                    .setups()
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let id = match s {
                            Setup::Sound => "pause_options_sound_button_title",
                            Setup::Screen => "pause_options_osd_button_title",
                            Setup::Advanced => "controllerSetup_advancedSetup",
                        };
                        (l(id), String::new(), true, i == self.options)
                    })
                    .collect();
                view.help = help(true, "default_buttontext_back", None);
            }
            Screen::Setup(setup) => {
                view.title = l(match setup {
                    Setup::Sound => "sound_setup_screen_title",
                    Setup::Screen => "osd_setup_screen_title",
                    Setup::Advanced => "controllerSetup_advancedSetup",
                });
                let v = &self.values;
                view.rows = fields(setup)
                    .iter()
                    .enumerate()
                    .map(|(i, (id, field))| {
                        let value = match field {
                            Field::Effects => Some(format!("{}%", v.effects)),
                            Field::LapTimer => Some(on(v.lap_timer)),
                            Field::Position => Some(on(v.position)),
                            Field::MiniMap => Some(on(v.minimap)),
                            Field::Dials => Some(on(v.dials)),
                            Field::SpeedUnit => Some(l(if v.mph { "mph" } else { "kph" })),
                            Field::Tune(g, w) => Some(format!("{}%", v.tunes[*g][*w])),
                            _ => None,
                        };
                        let enabled = value.is_some();
                        (
                            l(id),
                            value.unwrap_or_else(|| "--".into()),
                            enabled,
                            i == self.row,
                        )
                    })
                    .collect();
                view.help = format!(
                    "Left/Right: change    {}",
                    help(
                        false,
                        "default_buttontext_back",
                        Some("default_buttontext_undo")
                    )
                );
            }
        }
        view
    }
}

/// Initial option values from the settings and the input map.
pub fn initial_options(settings: &crate::settings::Settings, actions: Option<&Actions>) -> Options {
    let base = Options::default();
    let mut values = Options {
        effects: settings.sound_effects.unwrap_or(base.effects),
        lap_timer: settings.osd_lap_timer.unwrap_or(base.lap_timer),
        position: settings.osd_position.unwrap_or(base.position),
        mph: settings.osd_mph.unwrap_or(base.mph),
        minimap: settings.osd_minimap.unwrap_or(base.minimap),
        dials: settings.osd_dials.unwrap_or(base.dials),
        ..base
    };
    if let Some(actions) = actions {
        for (group, action) in [Action::SteerLeft, Action::Accelerate, Action::Brake]
            .into_iter()
            .enumerate()
        {
            if let Some(c) = actions.calibration(action) {
                let percent = |x: f32| (x * 100.).round().clamp(0., 100.) as u8;
                values.tunes[group] = [percent(c.dead_zone), percent(c.saturation)];
            }
        }
    }
    values
}

pub fn install(app: &mut App, menu: PauseMenu, labels: Labels) {
    app.insert_resource(menu)
        .insert_resource(labels)
        .add_systems(Update, update.before(crate::drive_actions))
        .add_systems(Update, (spawn, present).chain().after(update));
}

/// Applies the advanced controls to every steering, accelerate and brake binding.
fn apply_tunes(actions: &mut Actions, values: &Options) {
    for (group, list) in [
        &[Action::SteerLeft, Action::SteerRight][..],
        &[Action::Accelerate],
        &[Action::Brake],
    ]
    .into_iter()
    .enumerate()
    {
        let [dz, sat] = values.tunes[group].map(|p| f32::from(p) / 100.);
        for &action in list {
            if let Err(error) = actions.calibrate(
                action,
                crate::input::Tune {
                    dead_zone: Some(dz),
                    saturation: Some(sat),
                },
            ) {
                warn!("advanced controls not applied: {error}");
            }
        }
    }
}

pub fn update(
    mut menu: ResMut<PauseMenu>,
    mut driving: ResMut<crate::Driving>,
    mut actions: ResMut<Actions>,
    mut shifts: ResMut<ShiftQueue>,
    mut trial: Option<ResMut<crate::time_trial::Trial>>,
    (time, mut post): (
        Option<Res<Time<Real>>>,
        Option<ResMut<crate::post_race::PostRace>>,
    ),
    mut exit: MessageWriter<AppExit>,
    mut replay: Option<ResMut<crate::replay::Replay>>,
) {
    // A replay owns the input; after it the menu shows again.
    if replay.as_ref().is_some_and(|r| r.owns_input()) {
        return;
    }
    // A restart from elsewhere (R) closes the menu.
    if menu.open && !driving.paused {
        menu.close();
    }
    // The post-race screens own the input; no pause over them.
    if post.as_ref().is_some_and(|p| p.active()) {
        return;
    }
    let dt = time.map_or(0., |t| t.delta_secs());
    let mut repeat = menu.repeat;
    let nav = nav(&actions, &mut repeat, dt);
    menu.repeat = repeat;
    let Some(command) = menu.step(nav) else {
        return;
    };
    let log = |action: &str| {
        log_line!(
            "drive_menu version=1 segment={} tick={} action={action} screen={:?}",
            driving.segment,
            driving.runtime.ticks,
            menu.screen
        )
    };
    match command {
        Command::Open => {
            log("open");
            driving.paused = true;
            driving.report("pause", driving.last_controls.unwrap_or_default());
        }
        Command::Resume => {
            log("resume");
            driving.paused = false;
            driving.report("resume", driving.last_controls.unwrap_or_default());
        }
        Command::Restart => {
            log("restart");
            crate::restart(&mut driving, trial.as_deref_mut());
        }
        Command::ResetCar => {
            log("reset-car");
            driving.paused = false;
            reset_car(&mut driving);
        }
        Command::RetireRace => {
            log("retire-race");
            retire_player(&mut driving, trial.as_deref());
            driving.paused = false;
            match post.as_mut() {
                Some(post) => post.on_retire(),
                None => {
                    exit.write(AppExit::Success);
                }
            }
        }
        Command::Exit => {
            log("finish-test-drive");
            exit.write(AppExit::Success);
        }
        Command::InstantReplay => {
            log("instant-replay");
            if let Some(replay) = replay.as_mut() {
                replay.request(crate::replay::Kind::Instant, crate::replay::Origin::PauseMenu);
            }
        }
        Command::Apply(values) => {
            log_line!(
                "drive_menu version=1 action=apply effects={} lap_timer={} position={} mph={} minimap={} dials={} tunes={:?}",
                values.effects,
                values.lap_timer,
                values.position,
                values.mph,
                values.minimap,
                values.dials,
                values.tunes
            );
            apply_tunes(&mut actions, &values);
        }
        Command::Unavailable(id) => {
            log_line!("drive_menu version=1 action=unavailable button={id}");
        }
    }
    // A press that opened, closed or acted in the menu never reaches the car
    // as a gear change (pad A is both Menu Select and Gear Up).
    *shifts = ShiftQueue::default();
}

/// Reset Vehicle (pause menu Button3, `c_pause_reset_car`): the player's car
/// back onto the racing line at rest, as the auto-reset places it (F341).
fn reset_car(driving: &mut crate::Driving) {
    let Some(line) = &driving.reset_line else {
        return;
    };
    let shared = driving.opponents.as_mut().map(|o| &mut o.multi);
    let result =
        grid_tools::source_vehicle::player_reset::place(&mut driving.runtime, shared, line);
    if result.is_ok() {
        driving.auto_reset.reset();
    }
    match result {
        Ok(reset) => log_line!(
            "drive_menu_reset version=1 segment={} tick={} point={:?} heading={:?}",
            driving.segment,
            driving.runtime.ticks,
            reset.point,
            reset.heading
        ),
        Err(error) => log_line!(
            "drive_menu_reset version=1 segment={} tick={} error={:?}",
            driving.segment,
            driving.runtime.ticks,
            error.0
        ),
    }
}
/// Retire from Race: the player is out (`retire_race`), the race is
/// classified and logged; the results screen follows (post_race).
pub(crate) fn retire_player(driving: &mut crate::Driving, trial: Option<&crate::time_trial::Trial>) {
    driving.report("retire", driving.last_controls.unwrap_or_default());
    if let Some(opponents) = &mut driving.opponents
        && !opponents.standings.over()
    {
        use grid_tools::race::{Out, Status};
        if opponents.standings.status(0) == Status::Racing {
            opponents.standings.retire(0, Out::Retired);
        }
        let speeds = opponents.speeds(&driving.runtime);
        let time = trial
            .and_then(|t| t.race_time(driving.runtime.ticks))
            .unwrap_or(0.);
        opponents.standings.finalize(&speeds, time, &[0]);
        for line in opponents.standings.classification(&opponents.names) {
            log_line!("drive_race_result {line}");
        }
    }
}
#[derive(Component)]
struct Panel;
#[derive(Component)]
struct Title;
#[derive(Component)]
struct Subtitle;
#[derive(Component)]
struct Row(usize);
#[derive(Component)]
struct Help;

const ROWS: usize = 8;

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
            BackgroundColor(Color::srgba(0., 0., 0., 0.45)),
            Visibility::Hidden,
            GlobalZIndex(10),
            UiTargetCamera(camera),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(px(24.)),
                        row_gap: px(8.),
                        min_width: px(420.),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.025, 0.04, 0.065, 0.92)),
                ))
                .with_children(|panel| {
                    panel.spawn((Title, Text::new(""), font(30.), TextColor(Color::WHITE)));
                    panel.spawn((
                        Subtitle,
                        Text::new(""),
                        font(18.),
                        TextColor(Color::srgb(0.75, 0.8, 0.85)),
                    ));
                    for i in 0..ROWS {
                        panel.spawn((Row(i), Text::new(""), font(22.), TextColor(Color::WHITE)));
                    }
                    panel.spawn((
                        Help,
                        Text::new(""),
                        font(15.),
                        TextColor(Color::srgb(0.6, 0.65, 0.7)),
                    ));
                });
        });
}

#[allow(clippy::type_complexity)]
fn present(
    menu: Res<PauseMenu>,
    labels: Res<Labels>,
    mut panel: Query<&mut Visibility, With<Panel>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<Title>>,
        Query<&mut Text, With<Subtitle>>,
        Query<(&Row, &mut Text, &mut TextColor, &mut Node)>,
        Query<&mut Text, With<Help>>,
    )>,
    mut shown: Local<Option<View>>,
) {
    for mut visibility in &mut panel {
        let want = if menu.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
    if !menu.open {
        *shown = None;
        return;
    }
    let view = menu.view(&labels);
    if shown.as_ref() == Some(&view) {
        return;
    }
    for mut text in &mut texts.p0() {
        text.0.clone_from(&view.title);
    }
    for mut text in &mut texts.p1() {
        text.0.clone_from(&view.subtitle);
    }
    for (row, mut text, mut colour, mut node) in &mut texts.p2() {
        let Some((label, value, enabled, selected)) = view.rows.get(row.0) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let marker = if *selected { "> " } else { "   " };
        text.0 = if value.is_empty() {
            format!("{marker}{label}")
        } else {
            format!("{marker}{label}:  {value}")
        };
        colour.0 = match (enabled, selected) {
            (false, _) => Color::srgba(0.6, 0.65, 0.7, 0.45),
            (true, true) => Color::srgb(1., 0.78, 0.2),
            (true, false) => Color::WHITE,
        };
    }
    for mut text in &mut texts.p3() {
        text.0.clone_from(&view.help);
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
    fn free_race_main_menu_order_and_links() {
        let mut menu = PauseMenu::new(Variant::FreeRace, false, true, Options::default());
        assert_eq!(menu.step(Nav::default()), None);
        assert_eq!(menu.step(press(|n| n.pause = true)), Some(Command::Open));
        assert_eq!(
            menu.buttons().iter().map(|b| b.link).collect::<Vec<_>>(),
            [
                Link::Back,
                Link::InstantReplay,
                Link::Restart,
                Link::Settings,
                Link::RetireRace
            ]
        );
        // Up wraps (wrapV="true") to Retire from Race.
        menu.step(press(|n| n.up = true));
        assert_eq!(menu.step(press(|n| n.select = true)), None);
        assert_eq!(menu.screen, Screen::Confirm(Confirm::RetireRace));
        // Yes is the default answer.
        assert_eq!(
            menu.step(press(|n| n.select = true)),
            Some(Command::RetireRace)
        );
        assert!(!menu.open);
    }

    #[test]
    fn back_resumes_and_no_returns_to_the_main_menu() {
        let mut menu = PauseMenu::new(Variant::FreeRace, false, true, Options::default());
        menu.step(press(|n| n.pause = true));
        for _ in 0..2 {
            menu.step(press(|n| n.down = true));
        }
        menu.step(press(|n| n.select = true));
        assert_eq!(menu.screen, Screen::Confirm(Confirm::Restart));
        menu.step(press(|n| n.left = true));
        assert_eq!(menu.step(press(|n| n.select = true)), None);
        assert_eq!(menu.screen, Screen::Main);
        // Escape is Pause and Menu Back at once: one step only.
        assert_eq!(
            menu.step(press(|n| {
                n.pause = true;
                n.back = true
            })),
            Some(Command::Resume)
        );
        assert!(!menu.open);
    }

    #[test]
    fn unavailable_buttons_and_reset_vehicle() {
        let mut menu = PauseMenu::new(Variant::TestDrive, false, true, Options::default());
        menu.step(press(|n| n.pause = true));
        menu.step(press(|n| n.down = true));
        // Instant Replay starts the replay; the menu stays open under it.
        assert_eq!(
            menu.step(press(|n| n.select = true)),
            Some(Command::InstantReplay)
        );
        assert!(menu.open);
        // Ghost Car is not in the prototype.
        for _ in 0..4 {
            menu.step(press(|n| n.down = true));
        }
        assert_eq!(
            menu.step(press(|n| n.select = true)),
            Some(Command::Unavailable("test_track_select_ghost_car_title"))
        );
        menu.step(press(|n| n.up = true));
        menu.step(press(|n| n.up = true));
        menu.step(press(|n| n.up = true));
        menu.step(press(|n| n.up = true));
        menu.step(press(|n| n.button3 = true));
        assert_eq!(menu.screen, Screen::Confirm(Confirm::ResetCar));
        assert_eq!(
            menu.step(press(|n| n.select = true)),
            Some(Command::ResetCar)
        );
        // Without a reset line Button3 does nothing.
        let mut menu = PauseMenu::new(Variant::TestDrive, false, false, Options::default());
        menu.step(press(|n| n.pause = true));
        menu.step(press(|n| n.button3 = true));
        assert_eq!(menu.screen, Screen::Main);
        // Finish Test Drive exits after its confirmation.
        for _ in 0..4 {
            menu.step(press(|n| n.down = true));
        }
        menu.step(press(|n| n.select = true));
        assert_eq!(menu.screen, Screen::Confirm(Confirm::FinishTestDrive));
        assert_eq!(menu.step(press(|n| n.select = true)), Some(Command::Exit));
    }

    #[test]
    fn options_change_apply_undo_and_limits() {
        let mut menu = PauseMenu::new(Variant::FreeRace, true, true, Options::default());
        menu.step(press(|n| n.pause = true));
        for _ in 0..3 {
            menu.step(press(|n| n.down = true));
        }
        menu.step(press(|n| n.select = true));
        assert_eq!(menu.screen, Screen::Options);
        assert_eq!(menu.setups().len(), 3);
        menu.step(press(|n| n.select = true));
        assert_eq!(menu.screen, Screen::Setup(Setup::Sound));
        let Some(Command::Apply(v)) = menu.step(press(|n| n.left = true)) else {
            panic!()
        };
        assert_eq!(v.effects, 90);
        // At 100 % right changes nothing; speech is not in the prototype.
        assert!(matches!(
            menu.step(press(|n| n.right = true)),
            Some(Command::Apply(_))
        ));
        assert_eq!(menu.step(press(|n| n.right = true)), None);
        menu.step(press(|n| n.down = true));
        assert_eq!(menu.step(press(|n| n.right = true)), None);
        menu.step(press(|n| n.back = true));
        assert_eq!(menu.screen, Screen::Options);
        menu.step(press(|n| n.down = true));
        menu.step(press(|n| n.select = true));
        // Speed unit, then undo.
        menu.step(press(|n| n.up = true));
        assert!(matches!(menu.step(press(|n| n.left = true)), Some(Command::Apply(v)) if v.mph));
        assert!(
            matches!(menu.step(press(|n| n.button3 = true)), Some(Command::Apply(v)) if !v.mph)
        );
        menu.step(press(|n| n.back = true));
        menu.step(press(|n| n.down = true));
        menu.step(press(|n| n.select = true));
        assert_eq!(menu.screen, Screen::Setup(Setup::Advanced));
        // Steering dead zone 20 % cannot pass the saturation.
        for _ in 0..30 {
            menu.step(press(|n| n.right = true));
        }
        assert_eq!(menu.values.tunes[0], [95, 100]);
        menu.step(press(|n| n.down = true));
        for _ in 0..30 {
            menu.step(press(|n| n.left = true));
        }
        assert_eq!(menu.values.tunes[0], [95, 100]);
        // Back twice returns to the main menu.
        menu.step(press(|n| n.back = true));
        menu.step(press(|n| n.back = true));
        assert_eq!(menu.screen, Screen::Main);
        assert!(menu.open);
    }

    #[test]
    fn standard_options_have_no_advanced_screen() {
        let menu = PauseMenu::new(Variant::FreeRace, false, true, Options::default());
        assert_eq!(menu.setups(), [Setup::Sound, Setup::Screen]);
    }

    #[test]
    fn held_direction_repeats_after_the_original_delay() {
        let mut r = Repeat::default();
        let held = [true, false, false, false];
        assert_eq!(r.update(held, 0.), [true, false, false, false]);
        let mut fired = 0;
        for _ in 0..100 {
            fired += usize::from(r.update(held, 0.01)[0]);
        }
        // 1.0 s held: repeats at 0.35, 0.50, 0.65, 0.80, 0.95.
        assert_eq!(fired, 5);
        assert_eq!(r.update([false; 4], 0.01), [false; 4]);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn menus_match_the_original_states_file() {
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
        // (screen id, [(IBSelectableSimple link, IBTextStatic string)] in itemflow order)
        let screen = |id: &str| -> Vec<(String, String)> {
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
                    (
                        link.unwrap_or("").to_owned(),
                        string.unwrap_or("").to_owned(),
                    )
                })
                .collect()
        };
        let link = |l: Link| match l {
            Link::Back => "BACK",
            Link::InstantReplay => "instant_replay",
            Link::Restart => "restart",
            Link::Settings => "settings",
            Link::RetireRace => "retire_race",
            Link::Exit => "exit",
            Link::GhostCar => "ghost_car",
        };
        for (id, buttons) in [
            ("pause_mainmenu_free_race", &FREE_RACE[..]),
            ("pause_mainmenu_test_drive", &TEST_DRIVE[..]),
        ] {
            let ours: Vec<(String, String)> = buttons
                .iter()
                .map(|b| (link(b.link).to_owned(), b.string.to_owned()))
                .collect();
            assert_eq!(screen(id), ours, "{id}");
        }
        let options = |id: &str| {
            screen(id)
                .into_iter()
                .map(|(link, _)| link)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            options("pause_settings_standard"),
            ["sound_setup", "osd_setup"]
        );
        assert_eq!(
            options("pause_settings_wheel_no_ffb"),
            ["sound_setup", "osd_setup", "advanced"]
        );
        // Sub-screen rows in itemflow order, by their header strings.
        let headers = |id: &str| -> Vec<String> {
            let s = (0..doc.elements.len())
                .find(|&i| attr(i, "id") == Some(id))
                .unwrap();
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
                    attr(child(e, "IBTextStatic").unwrap(), "string")
                        .unwrap()
                        .to_owned()
                })
                .collect()
        };
        for (id, setup) in [
            ("f_pause_st_sound_optionsscreen", Setup::Sound),
            ("f_pause_st_osd_optionsscreen", Setup::Screen),
            ("f_pause_st_adv_optionsscreen", Setup::Advanced),
            ("t_pause_st_sound_optionsscreen", Setup::Sound),
            ("t_pause_st_osd_optionsscreen", Setup::Screen),
            ("t_pause_st_adv_optionsscreen", Setup::Advanced),
        ] {
            let ours: Vec<String> = fields(setup).iter().map(|(s, _)| (*s).to_owned()).collect();
            assert_eq!(headers(id), ours, "{id}");
        }
        // Confirmations: No then Yes, Yes active by default.
        for confirm in [
            Confirm::Restart,
            Confirm::RetireRace,
            Confirm::FinishTestDrive,
            Confirm::ResetCar,
        ] {
            let (title, _) = confirm.strings(Variant::TestDrive);
            let id = title.trim_end_matches("_title");
            let id = match id {
                "end_test_drive_confirm" | "restart_test_drive_confirm" => id.to_owned(),
                "reset_vehicle_confirm" => "reset_car_confirm".to_owned(),
                other => other.to_owned(),
            };
            let s = (0..doc.elements.len())
                .find(|&i| attr(i, "id") == Some(id.as_str()))
                .unwrap_or_else(|| panic!("{id}"));
            let flow = child(s, "itemflow").unwrap();
            assert_eq!(attr(flow, "active_default"), Some("0.library.button_1"));
            assert_eq!(
                screen(&id),
                [
                    ("no".into(), "overlay_popup_generic_decline".into()),
                    ("yes".into(), "overlay_popup_generic_accept".into())
                ]
            );
        }
        // Every label exists in the English table.
        let labels = Labels::load(root);
        let language = labels.0.as_ref().unwrap();
        for id in FREE_RACE.iter().chain(&TEST_DRIVE).map(|b| b.string).chain(
            SOUND
                .iter()
                .chain(&SCREEN)
                .chain(&ADVANCED)
                .map(|(s, _)| *s),
        ) {
            assert!(language.get(id).is_some(), "{id}");
        }
    }
}

#[cfg(test)]
pub(crate) mod race_tests;
