//! Front end before the race ([research and layout](../../../docs/DRIVER_AIDS.md)):
//! the original's Race Options screen (`event_options`) with its Driver
//! Assists and Go To Race items, and the Driver Aids screen
//! (`driveraids_optionsscreen`).
//!
//! - Driver Aids [D states.bin]: items traction, gearbox, brake, steering in
//!   that order (`itemflow wrapV="true"`), each a wrapping left/right combo
//!   (`IBComboTextSimple wrap="true"`): Off/On, Automatic/Manual; the help
//!   text of the selected item (`IBHelpInfo`); overlay Back and Button3 =
//!   Undo. Traction Control and Braking Assist also have 25 % and 50 %
//!   (AUTHORED, `grid_world::driver_aids`).
//! - The choice applies to the player's car before the first tick
//!   (`Runtime::set_assists`, as FUN_00654C20 at race set-up) and is saved in
//!   `<data dir>/settings.ini` `[assists]` when the screen is left (the
//!   original saves its profile).
//! - Not reachable in the race: the pause menu's Options have Sound, Screen
//!   Setup and Advanced only (docs/PAUSE_MENU.md).
//! - The race is held while the front end is open; keys and pad, and on a
//!   phone the touch menu zones, navigate it (P11).
use crate::input::Actions;
use crate::pause_menu::{Labels, Nav, Repeat, View};
use crate::render_scale::RenderHeight;
use bevy::prelude::*;
use bevy::window::PresentMode;
use grid_tools::source_vehicle::assists::{Assists, Level, Transmission};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    RaceOptions,
    DriverAids,
    Video,
    PerfTest,
}

/// Race Options items [D `event_options` itemflow, the ported subset].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    PerfTest,
    DriverAids,
    Video,
    Race,
}
/// Video is AUTHORED (F390); its strings are not in the game's language file,
/// so the labels fall back to the plain English ids. Performance test (AUTHORED
/// diagnostics, `perf_test`) comes first, so Go To Race stays the default and
/// last item.
const ITEMS: [(Item, &str, &str); 4] = [
    (
        Item::PerfTest,
        "Performance test",
        "Diagnostics: turn parts of the frame off live; the FPS log line names them (not saved).",
    ),
    (
        Item::DriverAids,
        "event_options_driver_aids_button_title",
        "help_info_event_options_driver_aids_button",
    ),
    (
        Item::Video,
        "Video",
        "Frame-rate counter, render resolution, vsync and refresh rate (this port's options).",
    ),
    (
        Item::Race,
        "event_options_exit_to_race_button_title",
        "help_info_event_options_race_button",
    ),
];

/// Driver Aids items in the screen's order [D].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aid {
    Traction,
    Gearbox,
    Brake,
    Steering,
}
const AIDS: [(Aid, &str, &str); 4] = [
    (
        Aid::Traction,
        "driver_aids_traction_control_toggle_title",
        "help_info_driver_aids_traction_toggle",
    ),
    (
        Aid::Gearbox,
        "driver_aids_transmission_control_toggle_title",
        "",
    ),
    (
        Aid::Brake,
        "driver_aids_braking_assist_toggle_title",
        "help_info_driver_aids_breaking_toggle",
    ),
    (
        Aid::Steering,
        "driver_aids_steering_assist_toggle_title",
        "help_info_driver_aids_steering_toggle",
    ),
];

/// Performance test rows (AUTHORED diagnostics, `perf_test`): On = that part is
/// drawn, as in the normal game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerfRow {
    Shadows,
    Foliage,
    Ground,
    Effects,
    Hud,
    Samples,
    Empty,
    Auto,
}
/// Auto cycle first: the phone shows only the first rows (owner, 2026-10-22).
const PERF_ROWS: [(PerfRow, &str, &str); 8] = [
    (
        PerfRow::Auto,
        "Auto cycle",
        "On: steps through every switch, 8 s each, and logs each step.",
    ),
    (PerfRow::Shadows, "Shadows", "Shadow casting of every mesh (the sun keeps its shadow maps, empty)."),
    (PerfRow::Foliage, "Foliage", "Camera-facing tree and bush billboards."),
    (PerfRow::Ground, "Ground cover", "Grass and ground detail sprites."),
    (PerfRow::Effects, "Smoke, debris, skid marks", "Tyre smoke, wheel debris and skid marks."),
    (PerfRow::Hud, "HUD", "The race HUD and its OSD camera (touch zones stay)."),
    (PerfRow::Samples, "Sample logging", "The periodic drive_*_sample log lines."),
    (PerfRow::Empty, "Whole scene", "Off: only your car is drawn."),
];

/// Video screen (AUTHORED, F390: GRID's PC version has its own graphics
/// options, not ported; this screen sets the port's `[osd] fps` and `[video]`
/// keys so they can be changed on a phone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoRow {
    Fps,
    RenderHeight,
    PresentMode,
    Refresh,
    Latency,
}
const VIDEO_ROWS: [(VideoRow, &str, &str); 5] = [
    (
        VideoRow::Fps,
        "FPS counter",
        "Shows frames per second, the average and the slowest frame of the last second at the top of the screen, and writes them to the log once a second.",
    ),
    (
        VideoRow::RenderHeight,
        "Render height",
        "How many pixel rows the 3D picture is drawn with before it is stretched to the screen. Fewer rows: faster, softer. Native: the full screen. Applies at once.",
    ),
    (
        VideoRow::PresentMode,
        "Present mode",
        "Fifo: classic vsync, no tearing; a late frame waits for the next refresh (60 can drop to 30). Auto vsync: a late frame is shown at once, with possible tearing. Auto no vsync: no waiting at all. Applies at once.",
    ),
    (
        VideoRow::Refresh,
        "Refresh rate",
        "Asks Android for this screen refresh rate (a hint the phone may ignore). System: let the phone decide. No effect on the desktop. Applies at once.",
    ),
    (
        VideoRow::Latency,
        "Frame latency",
        "Frames prepared ahead of the screen. 1: less input lag; 3: smoother when frames are uneven. Default is 2. Applies at the next start.",
    ),
];

const RENDER_HEIGHTS: [RenderHeight; 4] = [
    RenderHeight::Rows(720),
    RenderHeight::Rows(900),
    RenderHeight::Rows(1080),
    RenderHeight::Native,
];
const PRESENT_MODES: [PresentMode; 3] = [
    PresentMode::Fifo,
    PresentMode::AutoVsync,
    PresentMode::AutoNoVsync,
];
const REFRESH: [Option<f32>; 4] = [None, Some(60.), Some(90.), Some(120.)];
const LATENCY: [Option<u32>; 4] = [None, Some(1), Some(2), Some(3)];

/// The next value in `list` (wrapping); a value not in the list goes to the
/// first entry.
fn cycle<T: PartialEq + Copy>(list: &[T], current: T, up: bool) -> T {
    let n = list.len();
    match list.iter().position(|v| *v == current) {
        Some(i) if up => list[(i + 1) % n],
        Some(i) => list[(i + n - 1) % n],
        None => list[0],
    }
}

/// The Video screen's values; defaults are the settings' defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Video {
    pub fps: bool,
    pub render_height: RenderHeight,
    pub present_mode: PresentMode,
    pub refresh_hz: Option<f32>,
    pub frame_latency: Option<u32>,
}

impl Video {
    pub fn from_settings(s: &crate::settings::Settings) -> Self {
        Self {
            fps: s.osd_fps.unwrap_or(cfg!(target_os = "android")),
            render_height: s
                .render_height
                .unwrap_or_else(RenderHeight::platform_default),
            present_mode: s.present_mode.unwrap_or_default(),
            refresh_hz: s.refresh_hz.flatten(),
            frame_latency: s.frame_latency.map(|n| n.get()),
        }
    }

    /// `[video]` keys for the settings file; default latency removes the key.
    pub fn keys(&self) -> Vec<(&'static str, Option<String>)> {
        vec![
            (
                "render_height",
                Some(match self.render_height {
                    RenderHeight::Native => "native".into(),
                    RenderHeight::Rows(r) => r.to_string(),
                }),
            ),
            ("present_mode", Some(present_name(self.present_mode).into())),
            (
                "refresh_hz",
                Some(
                    self.refresh_hz
                        .map_or("system".into(), |hz| format!("{hz}")),
                ),
            ),
            ("frame_latency", self.frame_latency.map(|n| n.to_string())),
        ]
    }

    fn change(&mut self, row: VideoRow, up: bool) {
        match row {
            VideoRow::Fps => self.fps = !self.fps,
            VideoRow::RenderHeight => {
                self.render_height = cycle(&RENDER_HEIGHTS, self.render_height, up)
            }
            VideoRow::PresentMode => {
                self.present_mode = cycle(&PRESENT_MODES, self.present_mode, up)
            }
            VideoRow::Refresh => self.refresh_hz = cycle(&REFRESH, self.refresh_hz, up),
            VideoRow::Latency => self.frame_latency = cycle(&LATENCY, self.frame_latency, up),
        }
    }

    fn value(&self, row: VideoRow) -> String {
        match row {
            VideoRow::Fps => if self.fps { "On" } else { "Off" }.into(),
            VideoRow::RenderHeight => match self.render_height {
                RenderHeight::Native => "Native".into(),
                RenderHeight::Rows(r) => r.to_string(),
            },
            VideoRow::PresentMode => match self.present_mode {
                PresentMode::Fifo => "Fifo (vsync)".into(),
                PresentMode::AutoVsync => "Auto vsync".into(),
                PresentMode::AutoNoVsync => "Auto no vsync".into(),
                other => format!("{other:?}"),
            },
            VideoRow::Refresh => self
                .refresh_hz
                .map_or("System".into(), |hz| format!("{hz} Hz")),
            VideoRow::Latency => self
                .frame_latency
                .map_or("Default (2)".into(), |n| n.to_string()),
        }
    }
}

fn present_name(mode: PresentMode) -> &'static str {
    match mode {
        PresentMode::AutoVsync => "auto_vsync",
        PresentMode::AutoNoVsync => "auto_no_vsync",
        _ => "fifo",
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Go To Race with these aids.
    Race(Assists),
    /// The Driver Aids screen was left with a changed choice.
    Save(Assists),
    /// The Video screen was left with a changed choice.
    SaveVideo(Video),
}

#[derive(Resource)]
pub struct FrontEnd {
    pub open: bool,
    pub screen: Screen,
    item: usize,
    row: usize,
    pub values: Assists,
    /// Values on entering Driver Aids (Button3 = Undo).
    undo: Assists,
    /// The last saved (or loaded) choice.
    saved: Assists,
    repeat: Repeat,
    /// Video screen values (applied live by `apply_video`), on entry, saved.
    pub video: Video,
    video_undo: Video,
    video_saved: Video,
    /// Performance test switches (live, never saved).
    pub perf: crate::perf_test::Switches,
    pub perf_auto: bool,
    /// Where the choice is saved (`<data dir>/settings.ini`).
    pub settings: std::path::PathBuf,
}

impl FrontEnd {
    pub fn new(values: Assists, video: Video) -> Self {
        Self {
            open: true,
            screen: Screen::RaceOptions,
            // `active_default="race"`.
            item: ITEMS.len() - 1,
            video,
            video_undo: video,
            video_saved: video,
            perf: Default::default(),
            perf_auto: false,
            row: 0,
            values,
            undo: values,
            saved: values,
            repeat: Repeat::default(),
            settings: crate::settings::data_dir().join("settings.ini"),
        }
    }

    fn change(&mut self, aid: Aid, up: bool) {
        let v = &mut self.values;
        match aid {
            Aid::Traction => v.traction_control = v.traction_control.step(up),
            Aid::Brake => v.braking_assist = v.braking_assist.step(up),
            Aid::Steering => v.stability_control = !v.stability_control,
            Aid::Gearbox => {
                v.transmission = match v.transmission {
                    Transmission::Automatic => Transmission::Manual,
                    Transmission::Manual => Transmission::Automatic,
                }
            }
        }
    }

    /// One frame of menu input.
    pub fn step(&mut self, nav: Nav) -> Option<Command> {
        if !self.open {
            return None;
        }
        match self.screen {
            Screen::RaceOptions => {
                let n = ITEMS.len();
                if nav.up {
                    self.item = (self.item + n - 1) % n;
                }
                if nav.down {
                    self.item = (self.item + 1) % n;
                }
                if nav.select {
                    match ITEMS[self.item].0 {
                        Item::DriverAids => {
                            self.screen = Screen::DriverAids;
                            self.row = 0;
                            self.undo = self.values;
                        }
                        Item::Video => {
                            self.screen = Screen::Video;
                            self.row = 0;
                            self.video_undo = self.video;
                        }
                        Item::PerfTest => {
                            self.screen = Screen::PerfTest;
                            self.row = 0;
                        }
                        Item::Race => {
                            self.open = false;
                            return Some(Command::Race(self.values));
                        }
                    }
                }
                None
            }
            Screen::DriverAids => {
                let n = AIDS.len();
                if nav.up {
                    self.row = (self.row + n - 1) % n;
                }
                if nav.down {
                    self.row = (self.row + 1) % n;
                }
                if nav.left || nav.right {
                    self.change(AIDS[self.row].0, nav.right);
                }
                if nav.button3 {
                    self.values = self.undo;
                }
                if nav.back {
                    self.screen = Screen::RaceOptions;
                    if self.values != self.saved {
                        self.saved = self.values;
                        return Some(Command::Save(self.values));
                    }
                }
                None
            }
            Screen::PerfTest => {
                let n = PERF_ROWS.len();
                if nav.up {
                    self.row = (self.row + n - 1) % n;
                }
                if nav.down {
                    self.row = (self.row + 1) % n;
                }
                if nav.left || nav.right || nav.select {
                    let p = &mut self.perf;
                    match PERF_ROWS[self.row].0 {
                        PerfRow::Shadows => p.shadows = !p.shadows,
                        PerfRow::Foliage => p.foliage = !p.foliage,
                        PerfRow::Ground => p.ground = !p.ground,
                        PerfRow::Effects => p.effects = !p.effects,
                        PerfRow::Hud => p.hud = !p.hud,
                        PerfRow::Samples => p.samples = !p.samples,
                        PerfRow::Empty => p.empty = !p.empty,
                        PerfRow::Auto => self.perf_auto = !self.perf_auto,
                    }
                }
                if nav.button3 {
                    self.perf = Default::default();
                    self.perf_auto = false;
                }
                if nav.back {
                    self.screen = Screen::RaceOptions;
                }
                None
            }
            Screen::Video => {
                let n = VIDEO_ROWS.len();
                if nav.up {
                    self.row = (self.row + n - 1) % n;
                }
                if nav.down {
                    self.row = (self.row + 1) % n;
                }
                if nav.left || nav.right {
                    self.video.change(VIDEO_ROWS[self.row].0, nav.right);
                }
                if nav.button3 {
                    self.video = self.video_undo;
                }
                if nav.back {
                    self.screen = Screen::RaceOptions;
                    if self.video != self.video_saved {
                        self.video_saved = self.video;
                        return Some(Command::SaveVideo(self.video));
                    }
                }
                None
            }
        }
    }

    pub fn view(&self, labels: &Labels) -> View {
        let l = |id: &str| labels.get(id).to_owned();
        let level = |v: Level| match v.percent() {
            100 => l("on_off_on"),
            0 => l("on_off_off"),
            p => format!("{p}%"),
        };
        match self.screen {
            Screen::RaceOptions => View {
                title: l("event_options_screen_title"),
                subtitle: l(ITEMS[self.item].2),
                rows: ITEMS
                    .iter()
                    .enumerate()
                    .map(|(i, (_, id, _))| (l(id), String::new(), true, i == self.item))
                    .collect(),
                help: format!("Enter/A: {}", l("default_buttontext_select")),
            },
            Screen::DriverAids => {
                let v = &self.values;
                View {
                    title: l("driver_aids_screen_title"),
                    subtitle: match AIDS[self.row].2 {
                        "" => String::new(),
                        id => l(id),
                    },
                    rows: AIDS
                        .iter()
                        .enumerate()
                        .map(|(i, (aid, id, _))| {
                            let value = match aid {
                                Aid::Traction => level(v.traction_control),
                                Aid::Brake => level(v.braking_assist),
                                Aid::Steering => l(if v.stability_control {
                                    "on_off_on"
                                } else {
                                    "on_off_off"
                                }),
                                Aid::Gearbox => l(match v.transmission {
                                    Transmission::Automatic => "transmission_automatic",
                                    Transmission::Manual => "transmission_manual",
                                }),
                            };
                            (l(id), value, true, i == self.row)
                        })
                        .collect(),
                    help: format!(
                        "Left/Right: change    Esc/B: {}    Ctrl/X: {}",
                        l("default_buttontext_back"),
                        l("default_buttontext_undo")
                    ),
                }
            }
            Screen::PerfTest => View {
                title: "Performance test".into(),
                subtitle: PERF_ROWS[self.row].2.into(),
                rows: PERF_ROWS
                    .iter()
                    .enumerate()
                    .map(|(i, (row, name, _))| {
                        let p = &self.perf;
                        let off = match row {
                            PerfRow::Shadows => p.shadows,
                            PerfRow::Foliage => p.foliage,
                            PerfRow::Ground => p.ground,
                            PerfRow::Effects => p.effects,
                            PerfRow::Hud => p.hud,
                            PerfRow::Samples => p.samples,
                            PerfRow::Empty => p.empty,
                            PerfRow::Auto => !self.perf_auto,
                        };
                        let value = if off { "Off" } else { "On" };
                        ((*name).into(), value.into(), true, i == self.row)
                    })
                    .collect(),
                help: format!(
                    "Left/Right: change    Esc/B: {}    Ctrl/X: reset",
                    l("default_buttontext_back")
                ),
            },
            Screen::Video => View {
                title: "Video".into(),
                subtitle: VIDEO_ROWS[self.row].2.into(),
                rows: VIDEO_ROWS
                    .iter()
                    .enumerate()
                    .map(|(i, (row, name, _))| {
                        ((*name).into(), self.video.value(*row), true, i == self.row)
                    })
                    .collect(),
                help: format!(
                    "Left/Right: change    Esc/B: {} (saves)    Ctrl/X: {}",
                    l("default_buttontext_back"),
                    l("default_buttontext_undo")
                ),
            },
        }
    }
}

/// Opens the front end; the caller holds the race from the first frame (the
/// aids apply before the first tick).
pub fn install(app: &mut App, values: Assists, video: Video) {
    app.insert_resource(FrontEnd::new(values, video))
        .add_systems(
            Update,
            update
                .after(crate::replay::control)
                .before(crate::pause_menu::update)
                .before(crate::drive_actions),
        )
        .add_systems(Update, (spawn, present).chain().after(update))
        .add_systems(Update, apply_video.after(update))
        .add_systems(Update, apply_perf.after(update));
    log_line!("drive_front_end version=1 event=open assists={values}");
}

pub fn update(
    mut front: ResMut<FrontEnd>,
    mut driving: ResMut<crate::Driving>,
    actions: Res<Actions>,
    mut shifts: ResMut<crate::input::ShiftQueue>,
    time: Option<Res<Time<Real>>>,
) {
    if !front.open {
        return;
    }
    driving.paused = true;
    *shifts = Default::default();
    let dt = time.map_or(0., |t| t.delta_secs());
    let mut repeat = front.repeat;
    let mut nav = crate::pause_menu::nav(&actions, &mut repeat, dt);
    front.repeat = repeat;
    // Escape is both Pause and Menu Back.
    nav.pause = false;
    match front.step(nav) {
        Some(Command::Save(aids)) => {
            match crate::settings::save_driver_aids(&front.settings, &aids) {
                Ok(()) => log_line!(
                    "drive_front_end version=1 event=save assists={aids} path={}",
                    front.settings.display()
                ),
                Err(error) => {
                    warn!("driver aids not saved: {error}");
                    log_line!("drive_front_end version=1 event=save_failed error={error:?}");
                }
            }
        }
        Some(Command::SaveVideo(video)) => {
            match crate::settings::save_video(&front.settings, &video) {
                Ok(()) => log_line!(
                    "drive_front_end version=1 event=save_video video={video:?} path={}",
                    front.settings.display()
                ),
                Err(error) => {
                    warn!("video settings not saved: {error}");
                    log_line!("drive_front_end version=1 event=save_video_failed error={error:?}");
                }
            }
        }
        Some(Command::Race(aids)) => {
            match driving.runtime.set_assists(aids) {
                Ok(()) => log_line!("drive_assists {aids}"),
                Err(error) => warn!("driver aids not applied: {error:?}"),
            }
            driving.paused = false;
            log_line!(
                "drive_front_end version=1 event=race tick={} assists={}",
                driving.runtime.ticks,
                driving.runtime.assists
            );
        }
        None => {}
    }
}

/// Applies the Video screen's values that can change live: the FPS counter,
/// the render cap, the present mode and (Android) the refresh-rate hint.
/// Frame latency waits for the next start (wgpu reads it only when the
/// swapchain is created).
/// The Performance test screen's switches reach `perf_test` (live; never saved).
fn apply_perf(
    front: Res<FrontEnd>,
    mut last: Local<Option<(crate::perf_test::Switches, bool)>>,
    test: Option<ResMut<crate::perf_test::PerfTest>>,
) {
    let now = (front.perf, front.perf_auto);
    let Some(before) = last.replace(now) else {
        return;
    };
    if before == now {
        return;
    }
    if let Some(mut test) = test {
        test.auto = front.perf_auto;
        if !test.auto {
            test.switches = front.perf;
        }
    }
}

fn apply_video(
    front: Res<FrontEnd>,
    mut applied: Local<Option<Video>>,
    fps: Option<ResMut<crate::fps_counter::FpsCounter>>,
    scale: Option<ResMut<crate::render_scale::RenderScale>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    #[cfg(target_os = "android")] display: Option<ResMut<crate::android_display::Display>>,
) {
    let video = front.video;
    let Some(before) = applied.replace(video) else {
        return;
    };
    if before == video {
        return;
    }
    if let Some(mut fps) = fps {
        fps.shown = video.fps;
    }
    if let Some(mut scale) = scale {
        scale.set_height(video.render_height);
    }
    for mut window in &mut windows {
        if window.present_mode != video.present_mode {
            window.present_mode = video.present_mode;
        }
    }
    #[cfg(target_os = "android")]
    if let Some(mut display) = display {
        display.set_refresh(video.refresh_hz);
    }
    log_line!("drive_video version=1 event=apply video={video:?}");
}

#[derive(Component)]
struct Panel;
#[derive(Component)]
struct Title;
#[derive(Component)]
struct Subtitle;
#[derive(Component)]
struct Row(usize);

/// Row texts the panel holds: the longest screen's rows. The Performance test
/// screen's last three rows had no text before (only 5 were spawned).
const ROWS: usize = {
    let mut n = AIDS.len();
    if ITEMS.len() > n {
        n = ITEMS.len();
    }
    if VIDEO_ROWS.len() > n {
        n = VIDEO_ROWS.len();
    }
    if PERF_ROWS.len() > n {
        n = PERF_ROWS.len();
    }
    n
};
#[derive(Component)]
struct Help;

/// The pause menu's panel look (AUTHORED, docs/PAUSE_MENU.md).
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
            BackgroundColor(Color::srgba(0., 0., 0., 0.55)),
            GlobalZIndex(11),
            UiTargetCamera(camera),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(px(24.)),
                        row_gap: px(8.),
                        min_width: px(460.),
                        max_width: px(720.),
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
    front: Res<FrontEnd>,
    labels: Option<Res<Labels>>,
    mut panel: Query<&mut Visibility, With<Panel>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<Title>>,
        Query<&mut Text, With<Subtitle>>,
        Query<(&Row, &mut Text, &mut TextColor, &mut Node)>,
        Query<&mut Text, With<Help>>,
    )>,
    mut shown: Local<Option<View>>,
) {
    let want = if front.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut panel {
        visibility.set_if_neq(want);
    }
    if !front.open {
        return;
    }
    let fallback = Labels::default();
    let view = front.view(labels.as_deref().unwrap_or(&fallback));
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
        let Some((label, value, _, selected)) = view.rows.get(row.0) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let marker = if *selected { "> " } else { "   " };
        text.0 = if value.is_empty() {
            format!("{marker}{label}")
        } else {
            format!("{marker}{label}:  < {value} >")
        };
        colour.0 = if *selected {
            Color::srgb(1., 0.78, 0.2)
        } else {
            Color::WHITE
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

    fn video() -> Video {
        Video::from_settings(&crate::settings::Settings::default())
    }

    fn press(f: impl Fn(&mut Nav)) -> Nav {
        let mut nav = Nav::default();
        f(&mut nav);
        nav
    }

    #[test]
    fn race_options_open_on_go_to_race_and_lead_to_driver_aids() {
        let mut front = FrontEnd::new(Assists::default(), video());
        let labels = Labels::default();
        let view = front.view(&labels);
        assert_eq!(view.title, "event_options_screen_title");
        assert_eq!(
            view.rows
                .iter()
                .map(|r| (r.0.as_str(), r.3))
                .collect::<Vec<_>>(),
            [
                ("Performance test", false),
                ("event_options_driver_aids_button_title", false),
                ("Video", false),
                ("event_options_exit_to_race_button_title", true)
            ]
        );
        // Back does nothing here; Down wraps to Performance test, then Driver Assists.
        assert_eq!(front.step(press(|n| n.back = true)), None);
        assert!(front.open);
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.down = true));
        assert_eq!(front.step(press(|n| n.select = true)), None);
        assert_eq!(front.screen, Screen::DriverAids);
        let view = front.view(&labels);
        assert_eq!(view.title, "driver_aids_screen_title");
        assert_eq!(view.subtitle, "help_info_driver_aids_traction_toggle");
        assert_eq!(
            view.rows
                .iter()
                .map(|r| (r.0.as_str(), r.1.as_str()))
                .collect::<Vec<_>>(),
            [
                ("driver_aids_traction_control_toggle_title", "on_off_on"),
                (
                    "driver_aids_transmission_control_toggle_title",
                    "transmission_automatic"
                ),
                ("driver_aids_braking_assist_toggle_title", "on_off_on"),
                ("driver_aids_steering_assist_toggle_title", "on_off_on"),
            ]
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
    fn the_front_end_holds_the_race_saves_and_applies_the_aids() {
        use crate::pause_menu::race_tests::{
            big_stack, digest, drive_to, keys, race_app_with, tap, ticks,
        };
        use grid_tools::route::DONINGTON;
        big_stack(|| {
            let chosen = Assists {
                traction_control: Level::new(25).unwrap(),
                braking_assist: Level::new(50).unwrap(),
                transmission: Transmission::Manual,
                ..Assists::default()
            };
            // Reference: the same aids set before the first tick.
            let mut reference = race_app_with(DONINGTON, &["lac"], 2, false);
            reference
                .world_mut()
                .resource_mut::<crate::Driving>()
                .runtime
                .set_assists(chosen)
                .unwrap();
            drive_to(&mut reference, 500);
            let expected = digest(&reference);

            let dir = std::env::temp_dir().join(format!("grid-front-end-{}", std::process::id()));
            let file = dir.join("settings.ini");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&file, "[osd]\nminimap = off\n").unwrap();
            let mut app = race_app_with(DONINGTON, &["lac"], 2, false);
            app.world_mut().resource_mut::<crate::Driving>().paused = true;
            let mut front = FrontEnd::new(Assists::default(), video());
            front.settings = file.clone();
            app.insert_resource(front).add_systems(
                Update,
                update
                    .after(crate::input::sample)
                    .before(crate::pause_menu::update)
                    .before(crate::drive_actions),
            );
            let open = |app: &App| app.world().resource::<FrontEnd>().open;
            // Held: no tick; Pause (Escape) does nothing. (R is not tapped:
            // the harness has no input plugin to clear `just_pressed`.)
            for key in [KeyCode::KeyW, KeyCode::Escape] {
                tap(&mut app, key);
            }
            assert_eq!(ticks(&app), 0);
            assert!(open(&app));
            assert!(!app.world().resource::<crate::pause_menu::PauseMenu>().open);
            // Driver Assists (two up, past Video); Traction 25 %, Gearbox
            // Manual, Braking 50 %.
            tap(&mut app, KeyCode::ArrowUp);
            tap(&mut app, KeyCode::ArrowUp);
            tap(&mut app, KeyCode::Enter);
            for key in [
                KeyCode::ArrowRight,
                KeyCode::ArrowRight,
                KeyCode::ArrowDown,
                KeyCode::ArrowRight,
                KeyCode::ArrowDown,
                KeyCode::ArrowLeft,
            ] {
                tap(&mut app, key);
            }
            assert_eq!(app.world().resource::<FrontEnd>().values, chosen);
            // Back saves to the settings file, keeping its other sections.
            tap(&mut app, KeyCode::Escape);
            let saved = std::fs::read_to_string(&file).unwrap();
            assert_eq!(
                saved,
                "[osd]\nminimap = off\n[assists]\ntcs=25% stability=on braking=50% gearbox=manual\n"
            );
            assert_eq!(ticks(&app), 0);
            // Go To Race (two down, past Video): the aids are on the car
            // before the first tick.
            tap(&mut app, KeyCode::ArrowDown);
            tap(&mut app, KeyCode::ArrowDown);
            // The race frame ticks too: with drive_to's keys held.
            keys(&mut app).press(KeyCode::KeyW);
            keys(&mut app).press(KeyCode::KeyD);
            keys(&mut app).press(KeyCode::Enter);
            app.update();
            keys(&mut app).release(KeyCode::Enter);
            assert!(!open(&app));
            assert_eq!(
                app.world().resource::<crate::Driving>().runtime.assists,
                chosen
            );
            drive_to(&mut app, 500);
            assert_eq!(digest(&app), expected);
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn performance_test_screen_toggles_live_and_resets() {
        let mut front = FrontEnd::new(Assists::default(), video());
        // Race -> Down wraps to Performance test.
        front.step(press(|n| n.down = true));
        assert_eq!(front.step(press(|n| n.select = true)), None);
        assert_eq!(front.screen, Screen::PerfTest);
        let view = front.view(&Labels::default());
        assert_eq!(view.title, "Performance test");
        assert_eq!(view.rows.len(), 8);
        // Auto cycle is the first row (Off), the switches follow (On).
        assert_eq!(view.rows[0].0, "Auto cycle");
        assert_eq!(view.rows[0].1, "Off");
        assert!(view.rows[1..].iter().all(|r| r.1 == "On"));
        front.step(press(|n| n.right = true));
        assert!(front.perf_auto);
        // Shadows (row 1) off, then HUD (row 5) off; nothing is saved.
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.right = true));
        for _ in 0..4 {
            front.step(press(|n| n.down = true));
        }
        front.step(press(|n| n.left = true));
        assert_eq!(front.perf.label(), "off:shadows+hud");
        // Up wraps from the first row to Whole scene.
        for _ in 0..5 {
            front.step(press(|n| n.up = true));
        }
        front.step(press(|n| n.up = true));
        front.step(press(|n| n.right = true));
        assert!(front.perf.empty);
        // Ctrl/X resets everything; Back returns without a command.
        front.step(press(|n| n.button3 = true));
        assert_eq!(front.perf, crate::perf_test::Switches::default());
        assert!(!front.perf_auto);
        assert_eq!(front.step(press(|n| n.back = true)), None);
        assert_eq!(front.screen, Screen::RaceOptions);
    }

    #[test]
    fn video_screen_cycles_undoes_and_saves() {
        let mut front = FrontEnd::new(Assists::default(), video());
        let start = front.video;
        // Race -> up -> Video.
        front.step(press(|n| n.up = true));
        front.step(press(|n| n.select = true));
        assert_eq!(front.screen, Screen::Video);
        let view = front.view(&Labels::default());
        assert_eq!(view.title, "Video");
        assert_eq!(view.rows.len(), 5);
        assert!(view.subtitle.starts_with("Shows frames per second"));
        // FPS toggles; Render height 720 -> 900 -> 1080 -> Native wraps.
        front.step(press(|n| n.right = true));
        assert_eq!(front.video.fps, !start.fps);
        front.step(press(|n| n.down = true));
        front.video.render_height = RenderHeight::Rows(720);
        for expected in [
            RenderHeight::Rows(900),
            RenderHeight::Rows(1080),
            RenderHeight::Native,
            RenderHeight::Rows(720),
        ] {
            front.step(press(|n| n.right = true));
            assert_eq!(front.video.render_height, expected);
        }
        // An odd value from the file goes to the first entry.
        front.video.render_height = RenderHeight::Rows(600);
        front.step(press(|n| n.left = true));
        assert_eq!(front.video.render_height, RenderHeight::Rows(720));
        // Present mode, refresh rate (left wraps to 120), latency.
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.right = true));
        assert_eq!(front.video.present_mode, PresentMode::AutoVsync);
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.left = true));
        assert_eq!(front.video.refresh_hz, Some(120.));
        assert_eq!(front.view(&Labels::default()).rows[3].1, "120 Hz");
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.right = true));
        assert_eq!(front.video.frame_latency, Some(1));
        // Undo, then one change and Back: saved once.
        front.step(press(|n| n.button3 = true));
        assert_eq!(front.video, start);
        assert_eq!(
            front.step(press(|n| n.back = true)),
            None,
            "nothing changed"
        );
        front.step(press(|n| n.select = true));
        front.step(press(|n| n.right = true));
        let chosen = Video {
            fps: !start.fps,
            ..start
        };
        assert_eq!(
            front.step(press(|n| n.back = true)),
            Some(Command::SaveVideo(chosen))
        );
        assert_eq!(front.screen, Screen::RaceOptions);
    }

    #[test]
    fn driver_aids_cycle_undo_save_and_race() {
        let mut front = FrontEnd::new(Assists::default(), video());
        front.step(press(|n| n.up = true));
        front.step(press(|n| n.up = true));
        front.step(press(|n| n.select = true));
        // Traction: Right wraps from On to Off, then 25 %.
        front.step(press(|n| n.right = true));
        assert_eq!(front.values.traction_control, Level::OFF);
        front.step(press(|n| n.right = true));
        assert_eq!(front.values.traction_control.percent(), 25);
        assert_eq!(front.view(&Labels::default()).rows[0].1, "25%");
        // Gearbox, Braking (Left: On -> 50 %), Stability.
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.left = true));
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.left = true));
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.right = true));
        let chosen = Assists {
            traction_control: Level::new(25).unwrap(),
            transmission: Transmission::Manual,
            braking_assist: Level::new(50).unwrap(),
            stability_control: false,
            ..Assists::default()
        };
        assert_eq!(front.values, chosen);
        // Undo restores the values on entry; change again and leave: saved.
        front.step(press(|n| n.button3 = true));
        assert_eq!(front.values, Assists::default());
        assert_eq!(
            front.step(press(|n| n.back = true)),
            None,
            "nothing changed"
        );
        front.step(press(|n| n.select = true));
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.right = true));
        let manual = Assists {
            transmission: Transmission::Manual,
            ..Assists::default()
        };
        assert_eq!(
            front.step(press(|n| n.back = true)),
            Some(Command::Save(manual))
        );
        assert_eq!(front.screen, Screen::RaceOptions);
        // Down past Video to Go To Race.
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.down = true));
        assert_eq!(
            front.step(press(|n| n.select = true)),
            Some(Command::Race(manual))
        );
        assert!(!front.open);
        assert_eq!(front.step(press(|n| n.select = true)), None);
    }
}
