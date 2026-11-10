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

/// The values the pause options change. The 900 degree mode has no
/// counterpart in the prototype; speech and music are saved but the port
/// plays neither yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// `volume_soundfx`, percent: engine and tyre sound.
    pub effects: u8,
    /// Speech and Music Volume, percent (trims of the speech and music
    /// groups, 4124A0/411DE0 [O]; the port has no speech or music yet).
    pub speech: u8,
    pub music: u8,
    /// `wheelLinearity`, percent of the option's range: the wheel response
    /// `(L - 0.5) * 2` of the 828670 command shaping [D]; 70 = 14/20, the
    /// original's default.
    pub linearity: u8,
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
            speech: 100,
            music: 100,
            linearity: 70,
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

    /// The wheel response record +14 for the linearity option.
    pub fn wheel_response(&self) -> f32 {
        (f32::from(self.linearity) / 100. - 0.5) * 2.
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
    /// A settings screen was left: the original autosaves the profile
    /// (`*_st_*_autosave` [D]); `true` when the Advanced screen's input
    /// tunes are part of it.
    Save(Options, bool),
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
    /// Where leaving a settings screen saves the options (the app's
    /// `<data dir>/settings.ini`); `None` keeps them for the session (tests).
    pub save_to: Option<std::path::PathBuf>,
    /// Menu widget sounds of the last steps (hud_sound::menu_cue).
    pub(crate) sounds: Vec<grid_tools::hud_audio::Cue>,
    /// Seconds into `restart_darken` after Restart's Yes: the menu stays
    /// open, and the race restarts when the darkening ends
    /// ([`PauseMenu::darken_step`]).
    darken: Option<f32>,
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
            save_to: None,
            sounds: Vec::new(),
            darken: None,
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

    pub(crate) fn close(&mut self) {
        self.open = false;
        self.screen = Screen::Main;
        self.darken = None;
    }

    /// Advances `restart_darken` by `dt` seconds: `None` when Restart is not
    /// darkening, `Some(true)` once it ends (the menu closes and the race
    /// restarts, `restart_overlay_hide` then `closed_shutters_restart`).
    pub fn darken_step(&mut self, dt: f32) -> Option<bool> {
        let t = self.darken? + dt;
        if t < RESTART_DARKEN {
            self.darken = Some(t);
            return Some(false);
        }
        self.close();
        Some(true)
    }

    /// The black veil's alpha over the world during `restart_darken`
    /// ([`restart_veil`]); 0 otherwise.
    pub fn veil(&self) -> f32 {
        self.darken.map_or(0., |t| restart_veil(t / RESTART_DARKEN))
    }

    /// One frame of menu input. Closed, only Pause does anything (it opens on
    /// Continue, `c_pause_initial_decision_maker` -> main menu).
    /// One frame of input; queues its widget sounds in `sounds`: the
    /// screen whoosh on opening and closing [I: the pause screen's base
    /// enter/leave], a list move, a value step, activate or back, and the
    /// popup sound when a confirmation opens or closes.
    pub fn step(&mut self, nav: Nav) -> Option<Command> {
        use grid_tools::hud_audio::Cue;
        let (open, screen, values) = (self.open, self.screen, self.values);
        let place = (self.main, self.options, self.row, self.answer);
        let darkening = self.darken.is_some();
        let command = self.navigate(nav);
        // Restart's Yes keeps the menu on screen while it darkens; it
        // sounds as the close it starts.
        let shut = !self.open || (!darkening && self.darken.is_some());
        if !open && self.open {
            self.sounds.push(Cue::ScreenWhooshIn);
        } else if open {
            let side = matches!(self.screen, Screen::Setup(_));
            let cursor = place != (self.main, self.options, self.row, self.answer);
            let left = screen != self.screen || shut;
            let cue = crate::hud_sound::menu_cue(&nav, left, cursor, values != self.values, side);
            self.sounds.extend(cue);
            // A confirmation is a `ScreenYesNo` popup over the menu.
            let popup = |open: bool, screen: Screen| open && matches!(screen, Screen::Confirm(_));
            self.sounds.extend(crate::hud_sound::overlay_cue(popup(open, screen), popup(self.open && !shut, self.screen)));
            if shut {
                self.sounds.push(Cue::ScreenWhooshOut);
            }
        }
        command
    }

    fn navigate(&mut self, nav: Nav) -> Option<Command> {
        if self.darken.is_some() {
            return None;
        }
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
                if confirm == Confirm::Restart {
                    self.darken = Some(0.);
                    return Some(Command::Restart);
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
                    return Some(Command::Save(self.values, setup == Setup::Advanced));
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
        Field::Speech => values.speech = step_percent(values.speech, VOLUME_STEP, up, 0, 100),
        Field::Music => values.music = step_percent(values.music, VOLUME_STEP, up, 0, 100),
        // 20 steps, as the option's integer slider (`wheelLinearity` /20) [D].
        Field::Linearity => values.linearity = step_percent(values.linearity, TUNE_STEP, up, 0, 100),
        Field::MultiTurn => {}
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
                            Field::Speech => Some(format!("{}%", v.speech)),
                            Field::Music => Some(format!("{}%", v.music)),
                            Field::Linearity => Some(format!("{}%", v.linearity)),
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
        speech: settings.sound_speech.unwrap_or(base.speech),
        music: settings.sound_music.unwrap_or(base.music),
        linearity: settings.assists().map_or(base.linearity, |a| {
            ((a.wheel_response / 2. + 0.5) * 100.).round().clamp(0., 100.) as u8
        }),
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
        .add_systems(Update, (spawn, present, veil).chain().after(update))
        .add_systems(Update, pause_bloom.after(update));
}

/// The pause effect keeps the race's look and overrides `FinalMix`
/// `bloomAmount` to 0.4 [D frontend/persistentdatarender.xml
/// `pause_post_process`, over the race's 1.0 in postprocess/effects.xml
/// `Effect_0000`]; `pause_increase_blur` reaches it in 0.3 s and
/// `pause_decrease_blur` returns in 0.29 s [D flow.bin, states.bin]. Ported
/// as a factor on the camera's bloom, linear in time [I]. Its ToneMapFast
/// override needs the original tone map (backlog row 77).
const PAUSE_BLOOM: f32 = 0.4;
const PAUSE_IN: f32 = 0.3;
const PAUSE_OUT: f32 = 0.29;

/// Restart's Yes runs `restart_darken`: a waypoint to `fepause_restart` in
/// 0.1 s, then `restart_overlay_hide` and `closed_shutters_restart` [D
/// flow.bin, states.bin]. `restart_post_process` sets that effect's
/// ColourBalance `contrast` to 0.15 [D frontend/persistentdatarender.xml],
/// and the final merge multiplies its colour by `contrast` before
/// `brightness` (CheapFinalMerge `mad r0.xyz, r0, c6.x, c8.x`,
/// docs/ORIGINAL_POSTPROCESS.md) [D]. The race's own contrast is taken as 1
/// (DRIG's grade is AUTHORED, backlog row 77) and the waypoint as linear in
/// real time [I].
const RESTART_DARKEN: f32 = 0.1;
const RESTART_CONTRAST: f32 = 0.15;

/// Alpha of a black veil that scales the picture's displayed values by the
/// contrast `share` of the way from 1 to [`RESTART_CONTRAST`]. The UI blends
/// in linear light, so the displayed factor k needs alpha 1 − k^2.2 [I: the
/// original multiplies display values; 2.2 approximates sRGB].
fn restart_veil(share: f32) -> f32 {
    let k = 1. + (RESTART_CONTRAST - 1.) * share.clamp(0., 1.);
    1. - k.powf(2.2)
}

/// How far the pause effect is in (0 = race, 1 = paused) after `dt` seconds.
fn pause_share(share: f32, open: bool, dt: f32) -> f32 {
    if open {
        (share + dt / PAUSE_IN).min(1.)
    } else {
        (share - dt / PAUSE_OUT).max(0.)
    }
}

/// A camera's bloom intensity without the pause effect.
#[derive(Component)]
struct RaceBloom(f32);

fn pause_bloom(
    menu: Res<PauseMenu>,
    time: Res<Time<Real>>,
    mut share: Local<f32>,
    mut commands: Commands,
    mut cameras: Query<(Entity, &mut bevy::post_process::bloom::Bloom, Option<&RaceBloom>)>,
) {
    let next = pause_share(*share, menu.open, time.delta_secs());
    if next == *share && next == 0. {
        return;
    }
    *share = next;
    let factor = 1. + (PAUSE_BLOOM - 1.) * next;
    for (entity, mut bloom, race) in &mut cameras {
        let base = match race {
            Some(race) => race.0,
            None => {
                commands.entity(entity).insert(RaceBloom(bloom.intensity));
                bloom.intensity
            }
        };
        bloom.intensity = base * factor;
    }
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update(
    mut menu: ResMut<PauseMenu>,
    mut driving: ResMut<crate::Driving>,
    mut actions: ResMut<Actions>,
    mut shifts: ResMut<ShiftQueue>,
    mut trial: Option<ResMut<crate::time_trial::Trial>>,
    (time, mut post, front, mut leave): (
        Option<Res<Time<Real>>>,
        Option<ResMut<crate::post_race::PostRace>>,
        Option<Res<crate::front_end::FrontEnd>>,
        crate::transition::Leave,
    ),
    mut replay: Option<ResMut<crate::replay::Replay>>,
    sink: Option<Res<crate::hud_sound::HudSink>>,
    mut script: Local<RestartScript>,
) {
    // A replay owns the input; after it the menu shows again. The front end
    // owns it before the race. Nothing more after leaving (the fade runs).
    if replay.as_ref().is_some_and(|r| r.owns_input())
        || front.is_some_and(|f| f.open)
        || leave.leaving()
    {
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
    let (dt, now) = time.map_or((0., 0.), |t| (t.delta_secs(), t.elapsed_secs()));
    if let Some(due) = menu.darken_step(dt) {
        if due {
            log_line!(
                "drive_menu version=1 segment={} tick={} action=restart",
                driving.segment,
                driving.runtime.ticks
            );
            crate::restart(&mut driving, trial.as_deref_mut());
        }
        *shifts = ShiftQueue::default();
        return;
    }
    let mut repeat = menu.repeat;
    let mut nav = nav(&actions, &mut repeat, dt);
    menu.repeat = repeat;
    script.drive(&mut menu, &mut nav, now);
    let command = menu.step(nav);
    crate::hud_sound::drain(&mut menu.sounds, sink.as_deref());
    let Some(command) = command else {
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
        // The race restarts when the darkening ends (top of this system).
        Command::Restart => log("restart_darken"),
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
                None => leave.event(),
            }
        }
        Command::Exit => {
            log("finish-test-drive");
            leave.event();
        }
        Command::InstantReplay => {
            log("instant-replay");
            if let Some(replay) = replay.as_mut() {
                replay.request(crate::replay::Kind::Instant, crate::replay::Origin::PauseMenu);
            }
        }
        Command::Apply(values) => {
            log_line!(
                "drive_menu version=1 action=apply effects={} speech={} music={} linearity={} lap_timer={} position={} mph={} minimap={} dials={} tunes={:?}",
                values.effects,
                values.speech,
                values.music,
                values.linearity,
                values.lap_timer,
                values.position,
                values.mph,
                values.minimap,
                values.dials,
                values.tunes
            );
            apply_tunes(&mut actions, &values);
            // Only a wheel's steering is shaped (828670); the record is read
            // every tick, and replays are snapshots (docs/REPLAY.md).
            driving.runtime.assists.wheel_response = values.wheel_response();
        }
        Command::Unavailable(id) => {
            log_line!("drive_menu version=1 action=unavailable button={id}");
        }
        Command::Save(values, tunes) => {
            if let Some(path) = &menu.save_to {
                match crate::settings::save_options(path, &values, tunes) {
                    Ok(()) => log_line!("drive_menu version=1 action=save tunes={tunes} path={}", path.display()),
                    Err(error) => log_line!("drive_menu version=1 action=save_failed error={error:?}"),
                }
            }
        }
    }
    // A press that opened, closed or acted in the menu never reaches the car
    // as a gear change (pad A is both Menu Select and Gear Up).
    *shifts = ShiftQueue::default();
}

/// `GRID_PAUSE_RESTART_AT=S` (research runs, PAUSE_MENU.md §3): at S
/// seconds of real time the menu opens, moves to Restart and opens its
/// confirmation; 0.5 s later it presses Yes. Pressed through
/// [`PauseMenu::step`], so it takes the player's path.
#[derive(Default)]
pub struct RestartScript {
    at: Option<Option<f32>>,
    stage: u8,
}

impl RestartScript {
    fn drive(&mut self, menu: &mut PauseMenu, nav: &mut Nav, now: f32) {
        let at = *self.at.get_or_insert_with(|| {
            std::env::var("GRID_PAUSE_RESTART_AT").ok().and_then(|v| v.parse().ok())
        });
        let Some(at) = at.filter(|&at| now >= at) else {
            return;
        };
        let restart = menu.buttons().iter().position(|b| b.link == Link::Restart);
        match (self.stage, restart) {
            (0, _) if !menu.open => nav.pause = true,
            (0, Some(row)) if menu.screen == Screen::Main && menu.main != row => nav.down = true,
            (0, Some(_)) if menu.screen == Screen::Main => nav.select = true,
            (0, _) if menu.screen == Screen::Confirm(Confirm::Restart) => {
                self.stage = 1;
                log_line!("drive_menu version=1 action=script_confirm t={now:.2}");
            }
            (1, _) if now >= at + 0.5 => {
                nav.select = true;
                self.stage = 2;
            }
            _ => {}
        }
    }
}

/// Reset Vehicle (pause menu Button3, `c_pause_reset_car`): a request to
/// the player's reset job, which fades, places the car on the track
/// tracker's line and keeps it out of car collisions for a while, as the
/// auto trigger does (docs/RESET_AND_RESPAWN_RESEARCH.md 6). The job starts
/// at the next tick (`drive_reset event=start trigger=menu`).
fn reset_car(driving: &mut crate::Driving) {
    let accepted = driving.reset_tracker.is_some() && driving.auto_reset.request();
    log_line!(
        "drive_menu_reset version=2 segment={} tick={} accepted={accepted}",
        driving.segment,
        driving.runtime.ticks
    );
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
/// The black veil of `restart_darken` ([`PauseMenu::veil`]).
#[derive(Component)]
struct Veil;

fn veil(menu: Res<PauseMenu>, mut veils: Query<(&mut BackgroundColor, &mut Visibility), With<Veil>>) {
    let alpha = menu.veil();
    for (mut colour, mut visibility) in &mut veils {
        let want = if alpha > 0. { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != want {
            *visibility = want;
        }
        if colour.0.alpha() != alpha {
            colour.0 = Color::srgba(0., 0., 0., alpha);
        }
    }
}

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
    // Under the panel: `restart_darken` darkens the world, not the menu [I].
    commands.spawn((
        Veil,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.),
            height: percent(100.),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Visibility::Hidden,
        GlobalZIndex(9),
        UiTargetCamera(camera),
    ));
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

    #[test]
    fn the_pause_effect_fades_in_and_out() {
        let mut s = 0.;
        for _ in 0..3 {
            s = pause_share(s, true, 0.1);
        }
        assert!((s - 1.).abs() < 1e-5);
        assert!((pause_share(1., false, 0.145) - 0.5).abs() < 1e-5);
        assert_eq!(pause_share(0.2, false, 1.), 0.);
    }

    /// The system on a race camera: the bloom drops to 0.4 of the race's
    /// while the menu is open and comes back after it closes.
    #[test]
    fn the_pause_menu_dims_the_bloom() {
        use bevy::post_process::bloom::Bloom;
        use bevy::time::TimeUpdateStrategy;
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)))
            .insert_resource(PauseMenu::new(Variant::FreeRace, false, true, Options::default()))
            .add_systems(Update, pause_bloom);
        let camera = app.world_mut().spawn(Bloom { intensity: 0.04, ..Bloom::NATURAL }).id();
        let bloom = |app: &App| app.world().get::<Bloom>(camera).unwrap().intensity;
        app.update();
        assert_eq!(bloom(&app), 0.04);
        app.world_mut().resource_mut::<PauseMenu>().open = true;
        for _ in 0..6 {
            app.update();
        }
        assert!((bloom(&app) - 0.016).abs() < 1e-6, "{}", bloom(&app));
        app.world_mut().resource_mut::<PauseMenu>().open = false;
        for _ in 0..6 {
            app.update();
        }
        assert!((bloom(&app) - 0.04).abs() < 1e-6, "{}", bloom(&app));
    }

    /// Restart's Yes: the menu stays open (no input) for the 0.1 s
    /// `restart_darken`, with the sounds of the close it starts, and the
    /// restart comes when the darkening ends.
    #[test]
    fn restart_darkens_before_the_race_restarts() {
        use grid_tools::hud_audio::Cue;
        let mut menu = PauseMenu::new(Variant::FreeRace, false, true, Options::default());
        menu.step(press(|n| n.pause = true));
        for _ in 0..2 {
            menu.step(press(|n| n.down = true));
        }
        menu.step(press(|n| n.select = true));
        assert_eq!(menu.screen, Screen::Confirm(Confirm::Restart));
        menu.sounds.clear();
        assert_eq!(menu.darken_step(0.05), None);
        assert_eq!(menu.step(press(|n| n.select = true)), Some(Command::Restart));
        assert!(menu.open);
        assert_eq!(menu.sounds.last(), Some(&Cue::ScreenWhooshOut));
        assert_eq!(menu.veil(), 0.);
        menu.sounds.clear();
        assert_eq!(menu.step(press(|n| n.pause = true)), None);
        assert!(menu.sounds.is_empty());
        assert_eq!(menu.darken_step(0.05), Some(false));
        assert!((menu.veil() - restart_veil(0.5)).abs() < 1e-6);
        assert_eq!(menu.darken_step(0.06), Some(true));
        assert!(!menu.open);
        assert_eq!((menu.veil(), menu.darken_step(0.1)), (0., None));
    }

    /// The veil scales displayed values by the contrast: 1 → 0.15.
    #[test]
    fn the_restart_veil_reaches_the_restart_contrast() {
        assert_eq!(restart_veil(0.), 0.);
        assert!((1. - restart_veil(1.)).powf(1. / 2.2) - 0.15 < 1e-6);
        assert!(((1. - restart_veil(0.5)).powf(1. / 2.2) - 0.575).abs() < 1e-6);
    }

    /// The system on the veil node: shown with the menu's alpha while
    /// Restart darkens, hidden otherwise.
    #[test]
    fn the_veil_follows_the_darkening() {
        let mut app = App::new();
        let mut menu = PauseMenu::new(Variant::TestDrive, false, true, Options::default());
        menu.open = true;
        menu.screen = Screen::Confirm(Confirm::Restart);
        menu.step(press(|n| n.select = true));
        menu.darken_step(0.025);
        app.insert_resource(menu).add_systems(Update, veil);
        let node = app.world_mut().spawn((Veil, BackgroundColor(Color::NONE), Visibility::Hidden)).id();
        app.update();
        let shown = |app: &App| {
            let world = app.world();
            (*world.get::<Visibility>(node).unwrap(), world.get::<BackgroundColor>(node).unwrap().0.alpha())
        };
        let (visibility, alpha) = shown(&app);
        assert_eq!(visibility, Visibility::Inherited);
        assert!((alpha - restart_veil(0.25)).abs() < 1e-6, "{alpha}");
        app.world_mut().resource_mut::<PauseMenu>().darken_step(1.);
        app.update();
        assert_eq!(shown(&app), (Visibility::Hidden, 0.));
    }

    fn press(f: impl Fn(&mut Nav)) -> Nav {
        let mut nav = Nav::default();
        f(&mut nav);
        nav
    }

    /// The widget sounds (AUDIO_RESEARCH.md "Menu sounds"): whoosh in on
    /// opening, moves, activate, a slider step and its dead end at the top,
    /// back, and back plus whoosh out on closing.
    #[test]
    fn steps_queue_the_menu_widget_sounds() {
        use grid_tools::hud_audio::Cue;
        let mut menu = PauseMenu::new(Variant::FreeRace, false, true, Options::default());
        menu.step(Nav::default());
        assert!(menu.sounds.is_empty());
        menu.step(press(|n| n.pause = true));
        for _ in 0..3 {
            menu.step(press(|n| n.down = true));
        }
        menu.step(press(|n| n.select = true));
        assert_eq!(menu.screen, Screen::Options);
        menu.step(press(|n| n.select = true));
        assert_eq!(menu.screen, Screen::Setup(Setup::Sound));
        assert_eq!(menu.values.effects, 100);
        menu.step(press(|n| n.right = true));
        menu.step(press(|n| n.left = true));
        assert_eq!(menu.values.effects, 90);
        menu.step(press(|n| n.back = true));
        menu.step(press(|n| n.back = true));
        assert_eq!(menu.screen, Screen::Main);
        menu.step(press(|n| n.pause = true));
        assert!(!menu.open);
        assert_eq!(
            menu.sounds,
            [
                Cue::ScreenWhooshIn,
                Cue::UdlrMove,
                Cue::UdlrMove,
                Cue::UdlrMove,
                Cue::Select,
                Cue::Select,
                Cue::DeadEnd,
                Cue::ItemDiscrete,
                Cue::Back,
                Cue::Back,
                Cue::Back,
                Cue::ScreenWhooshOut,
            ]
        );
    }

    /// A confirmation is a popup: `overlay_on` after the activate that
    /// opens it, `overlay_off` after the back or the answer that closes it,
    /// before the menu's whoosh out when Yes closes the menu.
    #[test]
    fn confirmations_queue_the_popup_sounds() {
        use grid_tools::hud_audio::Cue;
        let mut menu = PauseMenu::new(Variant::FreeRace, false, true, Options::default());
        menu.step(press(|n| n.pause = true));
        menu.step(press(|n| n.button3 = true));
        assert_eq!(menu.screen, Screen::Confirm(Confirm::ResetCar));
        menu.step(press(|n| n.down = true));
        menu.step(press(|n| n.back = true));
        assert_eq!(menu.screen, Screen::Main);
        menu.step(press(|n| n.button3 = true));
        assert_eq!(menu.step(press(|n| n.select = true)), Some(Command::ResetCar));
        assert!(!menu.open);
        assert_eq!(
            menu.sounds,
            [
                Cue::ScreenWhooshIn,
                Cue::Select,
                Cue::OverlayOn,
                Cue::UdlrMove,
                Cue::Back,
                Cue::OverlayOff,
                Cue::Select,
                Cue::OverlayOn,
                Cue::Select,
                Cue::OverlayOff,
                Cue::ScreenWhooshOut,
            ]
        );
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
        // At 100 % right changes nothing.
        assert!(matches!(
            menu.step(press(|n| n.right = true)),
            Some(Command::Apply(_))
        ));
        assert_eq!(menu.step(press(|n| n.right = true)), None);
        menu.step(press(|n| n.down = true));
        assert_eq!(menu.step(press(|n| n.right = true)), None);
        // Speech volume is a slider too (saved; the port plays no speech).
        assert!(matches!(menu.step(press(|n| n.left = true)), Some(Command::Apply(v)) if v.speech == 90));
        menu.step(press(|n| n.button3 = true));
        // Leaving a settings screen autosaves (`*_st_*_autosave` [D]).
        assert!(matches!(
            menu.step(press(|n| n.back = true)),
            Some(Command::Save(v, false)) if v.effects == 100
        ));
        assert_eq!(menu.screen, Screen::Options);
        menu.step(press(|n| n.down = true));
        menu.step(press(|n| n.select = true));
        // Speed unit, then undo.
        menu.step(press(|n| n.up = true));
        assert!(matches!(menu.step(press(|n| n.left = true)), Some(Command::Apply(v)) if v.mph));
        assert!(
            matches!(menu.step(press(|n| n.button3 = true)), Some(Command::Apply(v)) if !v.mph)
        );
        assert!(matches!(menu.step(press(|n| n.back = true)), Some(Command::Save(_, false))));
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
