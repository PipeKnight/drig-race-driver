//! In-race damage icons of the original dial (Osd/sceneconfig.xml
//! `object/dial_damage`: engine, gears, suspension, steering, plus the
//! per-wheel states), driven by FUN_008EEF60 as ported in
//! `grid_world::mechanical_damage::indicators`.
//!
//! AUTHORED presentation: the original animates icons of Osd/dial2.pssg; this
//! panel shows the same eight inputs as text labels coloured by icon stage
//! (0 hidden, 0.2 yellow, 0.7 orange, 1.0 red). The panel is hidden while no
//! icon is lit, as the original's `anyDamageIndOn` reports.
use bevy::prelude::*;
use grid_tools::source_vehicle::assists::mechanical_damage::{ICONS, indicators};

#[derive(Component)]
struct DamagePanel;

#[derive(Component)]
struct DamageIcon(usize);

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
    commands
        .spawn((
            DamagePanel,
            Node {
                position_type: PositionType::Absolute,
                // Beside the dial, as `object/dial_damage` sits in `dial_main`.
                bottom: px(200.),
                right: px(18.),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10.)),
                row_gap: px(2.),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.04, 0.065, 0.85)),
            Visibility::Hidden,
            UiTargetCamera(camera),
        ))
        .with_children(|panel| {
            for (i, name) in ICONS.iter().enumerate() {
                panel.spawn((
                    DamageIcon(i),
                    Text::new(name.to_uppercase()),
                    TextFont {
                        font_size: FontSize::Px(18.),
                        ..default()
                    },
                    TextColor(colour(0.)),
                ));
            }
        });
}

fn colour(stage: f32) -> Color {
    if stage >= 1. {
        Color::srgb(0.95, 0.18, 0.12)
    } else if stage >= 0.7 {
        Color::srgb(1., 0.55, 0.1)
    } else if stage > 0. {
        Color::srgb(1., 0.88, 0.2)
    } else {
        Color::srgba(0.6, 0.65, 0.7, 0.35)
    }
}

fn update(
    driving: Res<crate::Driving>,
    mut panel: Query<&mut Visibility, With<DamagePanel>>,
    mut icons: Query<(&DamageIcon, &mut TextColor)>,
) {
    let Some(state) = indicators(&driving.runtime.state.car.owner.vehicle) else {
        return;
    };
    for mut visibility in &mut panel {
        *visibility = if state.any {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (icon, mut colour_slot) in &mut icons {
        *colour_slot = TextColor(colour(state.grades[icon.0]));
    }
}
