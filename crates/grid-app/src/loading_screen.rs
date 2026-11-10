//! The loading screen between the menu and the race, both ways
//! (docs/FRONT_END_FLOW.md): shown by the one-window front end
//! (front_end_host.rs) while the next screen is built.
//!
//! - The original's loading screen is a small 3D world (`loading_root` of
//!   fePersistentDatabase.pssg, seen from `load_cam` as it drifts) with a
//!   stat panel of the player's profile facts (`fact_*`), a load circle and
//!   fades [D, FRONT_END_FLOW.md section 2]. The world is drawn by
//!   [`crate::fe_world`], the stat panel by [`crate::loading_stats`] (F821).
//! - Fades up from black over 0.5 s (`load_world_fade_up`) [D]; the load
//!   circle and the `loading` text are the transition overlay's
//!   (transition.rs, F779), in the same place, so a hand-off from the
//!   overlay's "Loading" does not move it.
//! - Without the install's world (a read error, or `GRID_LOADING_WORLD=off`
//!   for A/B shots) it falls back to a plain dark screen.
//! - Its own small app: no audio, no gamepad, no size request.

use crate::pause_menu::Labels;
use crate::transition::{RingImage, draw_rings, loading_row, ring_image};
use bevy::prelude::*;

/// `load_world_fade_up` [D persistentdatacommon.xml].
const FADE_UP: f32 = 0.5;

#[derive(Component)]
struct Fade;

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
    crate::frame_pacing::install_window_shots_as(&mut app, "loading");
    // `GRID_LOADING_WORLD=off`: the plain fallback, for A/B shots and its smoke.
    let world = if std::env::var_os("GRID_LOADING_WORLD").is_some_and(|v| v == "off") {
        Err("GRID_LOADING_WORLD=off".to_owned())
    } else {
        crate::fe_world::install(&mut app, &root, &crate::fe_world::LOADING)
    };
    let camera = match world {
        Ok(camera) => Some(camera),
        Err(e) => {
            eprintln!("loading screen: no 3D world ({e})");
            None
        }
    };
    // The stat panel of the player's profile facts (loading_stats.rs), in the
    // world; `GRID_LOADING_STATS=off` leaves it out for A/B shots.
    if let Some(camera) = camera
        && !std::env::var_os("GRID_LOADING_STATS").is_some_and(|v| v == "off")
        && let Err(e) = crate::loading_stats::install(&mut app, &root, camera)
    {
        eprintln!("loading screen: no stat panel ({e})");
    }
    // The player's controls, as in the menus: the pad's right stick (Fe View
    // Tweak) turns the world's camera on its spring (camera_spring.rs).
    if camera.is_some() {
        let input = profile.input()?;
        let (actions, pad) = crate::input::load(&root, &input, false)?;
        println!("loading_input {input} pad_preset={pad}");
        crate::input::install(&mut app, actions);
    }
    app.insert_resource(WorldCamera(camera));
    Ok(app)
}

#[derive(Resource)]
struct Text2Show(String);

/// The 3D world's camera, which the UI draws over, if the world loaded.
#[derive(Resource)]
struct WorldCamera(Option<Entity>);

fn spawn(mut commands: Commands, text: Res<Text2Show>, world: Res<WorldCamera>, mut images: ResMut<Assets<Image>>) {
    let camera = world.0.unwrap_or_else(|| commands.spawn(Camera2d).id());
    let ring = ring_image(&mut images);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.),
                height: percent(100.),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .with_children(|screen| loading_row(screen, text.0.clone(), Some(ring), ()));
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

/// The fade up and the circle, by the app's own clock (each loading screen
/// is a new app, so it starts at zero).
fn animate(
    time: Res<Time>,
    mut fade: Query<&mut BackgroundColor, With<Fade>>,
    rings: Query<&RingImage>,
    mut images: ResMut<Assets<Image>>,
) {
    let t = time.elapsed_secs();
    for mut colour in &mut fade {
        let alpha = (1. - t / FADE_UP).clamp(0., 1.);
        if colour.0.alpha() != alpha {
            colour.0.set_alpha(alpha);
        }
    }
    draw_rings(t, &rings, &mut images);
}
