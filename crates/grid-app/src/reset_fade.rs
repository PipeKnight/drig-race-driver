//! The player's reset fade (row 43, docs/RESET_AND_RESPAWN_RESEARCH.md 6):
//! a black screen overlay whose opacity is the running reset job's
//! [`PlayerReset::darkness`](grid_tools::source_vehicle::player_reset::PlayerReset::darkness),
//! out over the first half of the 1 s fade and back in over the second
//! after the placement.
//!
//! [I] The original's job sends 0x5F9AE0 / 0x5F18C0 messages at the fade
//! start and end (event 2, event 5); whether they drive a screen fade or a
//! camera effect is still open, so the presentation (black, linear) is
//! AUTHORED.
use bevy::prelude::*;

#[derive(Component)]
struct ResetFade;

pub(crate) fn install(app: &mut App) {
    app.add_systems(Update, (spawn, update).chain());
}

fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<crate::ViewerCamera>>,
    mut done: Local<bool>,
) {
    let Some(camera) = camera.iter().next().filter(|_| !*done) else {
        return;
    };
    *done = true;
    commands.spawn((
        ResetFade,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.),
            height: percent(100.),
            ..default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.)),
        // Below the flashback whiteout (10) and the menus.
        GlobalZIndex(5),
        Visibility::Hidden,
        UiTargetCamera(camera),
    ));
}

fn update(
    driving: Option<Res<crate::Driving>>,
    mut fade: Query<(&mut BackgroundColor, &mut Visibility), With<ResetFade>>,
) {
    let darkness = driving.map_or(0., |d| d.auto_reset.darkness());
    for (mut colour, mut visibility) in &mut fade {
        let shown = if darkness > 0. {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(shown);
        let wanted = Color::BLACK.with_alpha(darkness);
        if colour.0 != wanted {
            colour.0 = wanted;
        }
    }
}
