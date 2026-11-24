//! DRIG patch (F905) check: the vendored bevy_pbr keeps the atmosphere, the
//! atmosphere environment probe and the SSAO bind groups while their inputs
//! are unchanged (third_party/README.md). A headless Bevy app renders an
//! Enhanced-like camera (HDR, atmosphere with its environment-map light,
//! SSAO). Frames without a change keep every bind group; resizing the target
//! image gives SSAO new textures, so its texture-bound groups must be new.
//! Needs a GPU adapter, so it is ignored by default and runs in prepush's full
//! tier (`--include-ignored`).

use bevy::app::PluginsState;
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{Atmosphere, AtmosphereEnvironmentMapLight};
use bevy::pbr::{
    AtmosphereBindGroups, AtmosphereProbeBindGroups, AtmosphereSettings,
    ScreenSpaceAmbientOcclusion, SsaoBindGroups,
};
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::{BindGroupId, Extent3d, TextureFormat};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

#[derive(Debug, PartialEq, Clone)]
struct Ids {
    atmosphere: Vec<BindGroupId>,
    probe: Vec<BindGroupId>,
    ssao_common: BindGroupId,
    ssao_textures: Vec<BindGroupId>,
}

/// The bind group ids of the one camera and probe, once all are prepared.
fn ids(app: &mut App) -> Option<Ids> {
    let world = app
        .get_sub_app_mut(RenderApp)
        .expect("render app")
        .world_mut();
    let atmosphere = world
        .query::<&AtmosphereBindGroups>()
        .iter(world)
        .next()
        .map(|g| {
            vec![
                g.transmittance_lut.id(),
                g.multiscattering_lut.id(),
                g.sky_view_lut.id(),
                g.aerial_view_lut.id(),
                g.render_sky.id(),
            ]
        })?;
    let probe: Vec<_> = world
        .query::<&AtmosphereProbeBindGroups>()
        .iter(world)
        .map(|g| g.environment.id())
        .collect();
    if probe.is_empty() {
        return None;
    }
    let ssao = world.query::<&SsaoBindGroups>().iter(world).next()?;
    Some(Ids {
        atmosphere,
        probe,
        ssao_common: ssao.common_bind_group.id(),
        ssao_textures: vec![
            ssao.preprocess_depth_bind_group.id(),
            ssao.ssao_bind_group.id(),
            ssao.spatial_denoise_bind_group.id(),
        ],
    })
}

#[test]
#[ignore = "needs a GPU adapter"]
fn atmosphere_and_ssao_bind_groups_kept_while_unchanged() {
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

    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            64,
            64,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let medium = app
        .world_mut()
        .resource_mut::<Assets<ScatteringMedium>>()
        .add(ScatteringMedium::earth(64, 64));
    app.world_mut().spawn(Atmosphere::earth(medium));
    app.world_mut().spawn((
        DirectionalLight::default(),
        Transform::from_xyz(1., 2., 1.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    app.world_mut().spawn((
        Camera3d::default(),
        RenderTarget::Image(target.clone().into()),
        Hdr,
        Msaa::Off,
        DepthPrepass,
        NormalPrepass,
        AtmosphereSettings::default(),
        AtmosphereEnvironmentMapLight::default(),
        ScreenSpaceAmbientOcclusion::default(),
    ));

    let mut ready = None;
    for _ in 0..240 {
        app.update();
        if let Some(found) = ids(&mut app) {
            ready = Some(found);
            break;
        }
    }
    let first = ready.expect("atmosphere, probe and SSAO bind groups within 240 frames");

    // No change: the same bind groups, frame after frame (more than the
    // cache's two-frame retention).
    for frame in 0..4 {
        app.update();
        assert_eq!(
            ids(&mut app).as_ref(),
            Some(&first),
            "unchanged inputs, frame {frame}"
        );
    }

    // A resize of the target (as of a window): SSAO gets new textures, so its
    // texture-bound groups are new. Bevy's camera_system sees an image asset
    // change, not a swapped RenderTarget, so the image is resized in place.
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .get_mut(&target)
        .expect("target image")
        .resize(Extent3d {
            width: 96,
            height: 96,
            depth_or_array_layers: 1,
        });
    // The new target image reaches the render world within a few frames.
    let mut resized = None;
    for _ in 0..30 {
        app.update();
        let now = ids(&mut app).expect("bind groups after resize");
        if now.ssao_textures != first.ssao_textures {
            resized = Some(now);
            break;
        }
    }
    let resized = resized.expect("SSAO texture bind groups change within 30 frames of a resize");
    // Let a partly updated frame settle, then every texture-bound group is new.
    app.update();
    let resized = ids(&mut app).unwrap_or(resized);
    assert!(
        resized
            .ssao_textures
            .iter()
            .all(|id| !first.ssao_textures.contains(id)),
        "resize: SSAO texture bind groups new, before {:?}, after {:?}",
        first.ssao_textures,
        resized.ssao_textures
    );
    app.update();
    assert_eq!(ids(&mut app), Some(resized), "kept after the resize");
}
