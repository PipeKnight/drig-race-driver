//! Light clustering only when there is something to cluster
//! (docs/PERFORMANCE.md "Light clustering and the OSD view (F661)").
//!
//! Bevy clusters point and spot lights, light probes and clustered decals per
//! 3D camera, every frame. grid-app has none of them unless the light-probe
//! reflections are on (`--reflections probe`, `scene_probe`); the sun is not
//! clustered. On the desktop Bevy clusters on the GPU, and that path rebuilds
//! its buffers and runs its passes for every camera even with nothing to
//! cluster (about 1 ms of the render thread for the main and the OSD camera).
//! So without probes the app clusters on the CPU, as Android always does, and
//! while nothing clusterable exists every camera's `ClusterConfig` is `None`,
//! which skips the CPU assignment and uploads an empty cluster grid. A light,
//! probe or decal appearing restores Bevy's default config the same frame.
//! The picture is unchanged: an empty cluster contributes nothing to shading.
//!
//! `GRID_LIGHT_CLUSTERS=bevy` keeps Bevy's clustering (comparisons).
use bevy::light::cluster::{ClusterConfig, GlobalClusterSettings};
use bevy::light::{ClusteredDecal, LightProbe, SimulationLightSystems};
use bevy::prelude::*;

pub fn install(app: &mut App, light_probes: bool) {
    if std::env::var("GRID_LIGHT_CLUSTERS").is_ok_and(|v| v == "bevy") {
        return;
    }
    if !light_probes {
        app.add_systems(Startup, |mut settings: ResMut<GlobalClusterSettings>| {
            settings.gpu_clustering = None;
        });
    }
    app.add_systems(
        PostUpdate,
        configure.before(SimulationLightSystems::AssignLightsToClusters),
    );
}

/// `None` for every 3D camera while the CPU clusters and nothing clusterable
/// exists, Bevy's default otherwise.
#[allow(clippy::type_complexity)]
fn configure(
    mut commands: Commands,
    settings: Option<Res<GlobalClusterSettings>>,
    clusterable: Query<
        (),
        Or<(
            With<PointLight>,
            With<SpotLight>,
            With<LightProbe>,
            With<ClusteredDecal>,
        )>,
    >,
    cameras: Query<(Entity, Option<&ClusterConfig>), With<Camera3d>>,
) {
    let cpu = settings.is_some_and(|s| s.gpu_clustering.is_none());
    let off = cpu && clusterable.is_empty();
    for (camera, config) in &cameras {
        let is_off = matches!(config, Some(ClusterConfig::None));
        if off != is_off {
            commands.entity(camera).insert(if off {
                ClusterConfig::None
            } else {
                ClusterConfig::default()
            });
        }
    }
}
