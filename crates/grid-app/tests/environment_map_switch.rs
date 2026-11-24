//! DRIG patch (F874) check: the vendored bevy_pbr keeps a camera's generated
//! environment-map bind groups only while their inputs are unchanged
//! (third_party/README.md). A headless Bevy app renders a camera with a
//! `GeneratedEnvironmentMapLight`, then switches its source cubemap the way a
//! new track's sky does. Frames without a change keep the bind groups; every
//! switch gives new ones. Needs a GPU adapter, so it is ignored by default and
//! runs in prepush's full tier (`--include-ignored`).

use bevy::app::PluginsState;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::pbr::generate::{GeneratorBindGroups, RenderEnvironmentMap};
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::{
    BindGroupId, Extent3d, TextureDimension, TextureFormat, TextureId, TextureViewDescriptor,
    TextureViewDimension,
};
use bevy::render::RenderApp;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

fn cube(color: [u8; 4]) -> Image {
    let mut image = Image::new_fill(
        Extent3d { width: 16, height: 16, depth_or_array_layers: 6 },
        TextureDimension::D2,
        &color,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image
}

/// The source texture and the bind groups of the one probe, if prepared.
fn probe(app: &mut App) -> Option<(TextureId, Vec<BindGroupId>)> {
    let world = app.get_sub_app_mut(RenderApp).expect("render app").world_mut();
    let mut query = world.query::<(&RenderEnvironmentMap, &GeneratorBindGroups)>();
    let (map, groups) = query.iter(world).next()?;
    let mut ids = vec![
        groups.downsampling_first.id(),
        groups.downsampling_second.id(),
        groups.irradiance.id(),
        groups.copy.id(),
    ];
    ids.extend(groups.radiance.iter().map(|group| group.id()));
    Some((map.environment_map.texture.id(), ids))
}

fn all_new(before: &[BindGroupId], after: &[BindGroupId]) -> bool {
    after.iter().all(|id| !before.contains(id))
}

#[test]
#[ignore = "needs a GPU adapter"]
fn environment_map_switch_gives_new_bind_groups() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    );
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();

    let (first, second, target) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (
            images.add(cube([200, 40, 40, 255])),
            images.add(cube([40, 40, 200, 255])),
            images.add(Image::new_target_texture(64, 64, TextureFormat::Rgba8UnormSrgb, None)),
        )
    };
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            RenderTarget::Image(target.into()),
            GeneratedEnvironmentMapLight {
                environment_map: first.clone(),
                intensity: 1000.,
                ..default()
            },
        ))
        .id();

    let mut ready = None;
    for _ in 0..120 {
        app.update();
        if let Some(found) = probe(&mut app) {
            ready = Some(found);
            break;
        }
    }
    let (texture_a, groups_a) = ready.expect("environment map bind groups within 120 frames");

    // No change: the same bind groups, frame after frame.
    for _ in 0..3 {
        app.update();
        assert_eq!(probe(&mut app), Some((texture_a, groups_a.clone())), "unchanged inputs");
    }

    // A new track's sky: a different source cubemap.
    let switch = |app: &mut App, map: &Handle<Image>| {
        app.world_mut()
            .get_mut::<GeneratedEnvironmentMapLight>(camera)
            .expect("camera light")
            .environment_map = map.clone();
        app.update();
        app.update();
        probe(app).expect("bind groups after switch")
    };
    let (texture_b, groups_b) = switch(&mut app, &second);
    assert_ne!(texture_b, texture_a, "the switch reached the render world");
    assert!(all_new(&groups_a, &groups_b), "switch to the second map: all bind groups new");
    app.update();
    assert_eq!(probe(&mut app), Some((texture_b, groups_b.clone())), "kept after the switch");

    // And back: the first map's texture again, but no stale bind group.
    let (texture_back, groups_back) = switch(&mut app, &first);
    assert_eq!(texture_back, texture_a);
    assert!(all_new(&groups_b, &groups_back), "switch back: all bind groups new");
}
