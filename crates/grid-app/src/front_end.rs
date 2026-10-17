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
use bevy::prelude::*;
use grid_tools::source_vehicle::assists::{Assists, Level, Transmission};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    RaceOptions,
    DriverAids,
}

/// Race Options items [D `event_options` itemflow, the ported subset].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    DriverAids,
    Race,
}
const ITEMS: [(Item, &str, &str); 2] = [
    (
        Item::DriverAids,
        "event_options_driver_aids_button_title",
        "help_info_event_options_driver_aids_button",
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
    (Aid::Gearbox, "driver_aids_transmission_control_toggle_title", ""),
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

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Go To Race with these aids.
    Race(Assists),
    /// The Driver Aids screen was left with a changed choice.
    Save(Assists),
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
    /// Where the choice is saved (`<data dir>/settings.ini`).
    pub settings: std::path::PathBuf,
}

impl FrontEnd {
    pub fn new(values: Assists) -> Self {
        Self {
            open: true,
            screen: Screen::RaceOptions,
            // `active_default="race"`.
            item: 1,
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
        }
    }
}

/// Opens the front end; the caller holds the race from the first frame (the
/// aids apply before the first tick).
pub fn install(app: &mut App, values: Assists) {
    app.insert_resource(FrontEnd::new(values))
        .add_systems(
            Update,
            update
                .after(crate::replay::control)
                .before(crate::pause_menu::update)
                .before(crate::drive_actions),
        )
        .add_systems(Update, (spawn, present).chain().after(update));
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
        Some(Command::Save(aids)) => match crate::settings::save_driver_aids(&front.settings, &aids) {
            Ok(()) => log_line!(
                "drive_front_end version=1 event=save assists={aids} path={}",
                front.settings.display()
            ),
            Err(error) => {
                warn!("driver aids not saved: {error}");
                log_line!("drive_front_end version=1 event=save_failed error={error:?}");
            }
        },
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
                    for i in 0..AIDS.len().max(ITEMS.len()) {
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

    fn press(f: impl Fn(&mut Nav)) -> Nav {
        let mut nav = Nav::default();
        f(&mut nav);
        nav
    }

    #[test]
    fn race_options_open_on_go_to_race_and_lead_to_driver_aids() {
        let mut front = FrontEnd::new(Assists::default());
        let labels = Labels::default();
        let view = front.view(&labels);
        assert_eq!(view.title, "event_options_screen_title");
        assert_eq!(
            view.rows.iter().map(|r| (r.0.as_str(), r.3)).collect::<Vec<_>>(),
            [
                ("event_options_driver_aids_button_title", false),
                ("event_options_exit_to_race_button_title", true)
            ]
        );
        // Back does nothing here; Up wraps to Driver Assists.
        assert_eq!(front.step(press(|n| n.back = true)), None);
        assert!(front.open);
        front.step(press(|n| n.down = true));
        assert_eq!(front.step(press(|n| n.select = true)), None);
        assert_eq!(front.screen, Screen::DriverAids);
        let view = front.view(&labels);
        assert_eq!(view.title, "driver_aids_screen_title");
        assert_eq!(view.subtitle, "help_info_driver_aids_traction_toggle");
        assert_eq!(
            view.rows.iter().map(|r| (r.0.as_str(), r.1.as_str())).collect::<Vec<_>>(),
            [
                ("driver_aids_traction_control_toggle_title", "on_off_on"),
                ("driver_aids_transmission_control_toggle_title", "transmission_automatic"),
                ("driver_aids_braking_assist_toggle_title", "on_off_on"),
                ("driver_aids_steering_assist_toggle_title", "on_off_on"),
            ]
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
    fn the_front_end_holds_the_race_saves_and_applies_the_aids() {
        use crate::pause_menu::race_tests::{big_stack, digest, drive_to, keys, race_app_with, tap, ticks};
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
            let mut front = FrontEnd::new(Assists::default());
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
            // Driver Assists; Traction 25 %, Gearbox Manual, Braking 50 %.
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
            // Go To Race: the aids are on the car before the first tick.
            tap(&mut app, KeyCode::ArrowDown);
            // The race frame ticks too: with drive_to's keys held.
            keys(&mut app).press(KeyCode::KeyW);
            keys(&mut app).press(KeyCode::KeyD);
            keys(&mut app).press(KeyCode::Enter);
            app.update();
            keys(&mut app).release(KeyCode::Enter);
            assert!(!open(&app));
            assert_eq!(app.world().resource::<crate::Driving>().runtime.assists, chosen);
            drive_to(&mut app, 500);
            assert_eq!(digest(&app), expected);
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn driver_aids_cycle_undo_save_and_race() {
        let mut front = FrontEnd::new(Assists::default());
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
        assert_eq!(front.step(press(|n| n.back = true)), None, "nothing changed");
        front.step(press(|n| n.select = true));
        front.step(press(|n| n.down = true));
        front.step(press(|n| n.right = true));
        let manual = Assists {
            transmission: Transmission::Manual,
            ..Assists::default()
        };
        assert_eq!(front.step(press(|n| n.back = true)), Some(Command::Save(manual)));
        assert_eq!(front.screen, Screen::RaceOptions);
        // Down to Go To Race.
        front.step(press(|n| n.down = true));
        assert_eq!(front.step(press(|n| n.select = true)), Some(Command::Race(manual)));
        assert!(!front.open);
        assert_eq!(front.step(press(|n| n.select = true)), None);
    }
}
