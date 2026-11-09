//! The loading screen between the menu and the race, both ways
//! (docs/FRONT_END_FLOW.md): shown by the one-window front end
//! (front_end_host.rs) while the next screen is built.
//!
//! - The original's loading screen is a small 3D world with a stat panel of
//!   the player's profile facts (`fact_*`), a rotating load circle and fades
//!   [D, FRONT_END_FLOW.md section 2]. The port has no profile statistics
//!   yet, so the panel shows the `loading` string ("Loading, please wait")
//!   (AUTHORED); the 3D world and the facts are open.
//! - Fades up from black over 0.5 s (`load_world_fade_up`) [D]; the load
//!   circle is twelve dots with a sweep, one turn a second (AUTHORED).
//! - Its own small app: no audio, no gamepad, no size request.

use crate::pause_menu::Labels;
use bevy::prelude::*;

/// `load_world_fade_up` [D persistentdatacommon.xml].
const FADE_UP: f32 = 0.5;
const DOTS: usize = 12;
const RING: f32 = 22.;

#[derive(Component)]
struct Fade;
#[derive(Component)]
struct Dot(usize);

/// The loading screen app, built but not run, for the front end host.
pub fn app(args: &[String]) -> Result<App, Box<dyn std::error::Error>> {
    let profile = crate::settings::load(args)?;
    let root = crate::game_root(profile.root.clone())?;
    let text = Labels::load(&root).get("loading").to_owned();
    let mut app = App::new();
    let plugins = crate::with_profile(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "GRID Reassembly".into(),
            present_mode: profile.present_mode.unwrap_or_default(),
            ..default()
        }),
        exit_condition: bevy::window::ExitCondition::DontExit,
        ..default()
    }));
    app.add_plugins(
        crate::front_end_host::hosted(plugins, crate::front_end_host::Gpu::Ui).disable::<bevy::audio::AudioPlugin>(),
    );
    crate::ui_font::install(&mut app);
    app.insert_resource(ClearColor(Color::srgb(0.055, 0.07, 0.095)))
        .insert_resource(Text2Show(text))
        .add_systems(Startup, spawn)
        .add_systems(Update, animate);
    Ok(app)
}

#[derive(Resource)]
struct Text2Show(String);

fn spawn(mut commands: Commands, text: Res<Text2Show>) {
    let camera = commands.spawn(Camera2d).id();
    commands
        .spawn((
            Node {
                width: percent(100.),
                height: percent(100.),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(28.),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .with_children(|screen| {
            screen
                .spawn(Node {
                    width: px(2. * RING + 12.),
                    height: px(2. * RING + 12.),
                    ..default()
                })
                .with_children(|ring| {
                    for i in 0..DOTS {
                        let angle = i as f32 / DOTS as f32 * std::f32::consts::TAU;
                        ring.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(RING + RING * angle.sin()),
                                top: px(RING - RING * angle.cos()),
                                width: px(12.),
                                height: px(12.),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                            Dot(i),
                        ));
                    }
                });
            screen.spawn((
                Text::new(text.0.clone()),
                TextFont {
                    font_size: FontSize::Px(26.),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.88, 0.92)),
            ));
        });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.),
            height: percent(100.),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        GlobalZIndex(i32::MAX),
        UiTargetCamera(camera),
        Fade,
    ));
}

/// The fade up and the circle's sweep, by the app's own clock (each loading
/// screen is a new app, so it starts at zero).
fn animate(
    time: Res<Time>,
    mut fade: Query<&mut BackgroundColor, (With<Fade>, Without<Dot>)>,
    mut dots: Query<(&mut BackgroundColor, &Dot), Without<Fade>>,
) {
    let t = time.elapsed_secs();
    for mut colour in &mut fade {
        let alpha = (1. - t / FADE_UP).clamp(0., 1.);
        if colour.0.alpha() != alpha {
            colour.0.set_alpha(alpha);
        }
    }
    let head = (t * DOTS as f32).rem_euclid(DOTS as f32);
    for (mut colour, dot) in &mut dots {
        // The dot under the sweep is brightest; the trail fades over a turn.
        let behind = (head - dot.0 as f32).rem_euclid(DOTS as f32);
        let alpha = 0.15 + 0.85 * (1. - behind / DOTS as f32).powi(2);
        colour.0 = Color::srgba(0.85, 0.88, 0.92, alpha);
    }
}
