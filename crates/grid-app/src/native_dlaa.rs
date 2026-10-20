//! Optional native-resolution DLAA through Bevy's pinned DLSS integration.
//! Main camera only. Unsupported Vulkan devices/SDKs retain SMAA Ultra.
use bevy::{
    anti_alias::{
        dlss::{
            Dlss, DlssInitPlugin, DlssPerfQualityMode, DlssProjectId, DlssSuperResolutionSupported,
        },
        smaa::{Smaa, SmaaPreset},
    },
    app::PluginGroupBuilder,
    asset::uuid::Uuid,
    camera::MainPassResolutionOverride,
    core_pipeline::prepass::ViewPrepassTextures,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        view::ExtractedView,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// Stable authored project UUID for NGX's custom-engine API; not a registered
// NVIDIA numeric application ID. No account credential or SDK asset is embedded.
const PROJECT_ID: u128 = 0x759b77c4_fa84_41d7_912d_542fe95449cf;
struct ProjectIdentity;
impl Plugin for ProjectIdentity {
    fn build(&self, app: &mut App) {
        app.insert_resource(DlssProjectId(Uuid::from_u128(PROJECT_ID)));
    }
}
/// Every DefaultPlugins group, including menus, needs the ID before DLSS init.
pub fn plugins(plugins: PluginGroupBuilder) -> PluginGroupBuilder {
    plugins.add_before::<DlssInitPlugin>(ProjectIdentity)
}
#[derive(Component, Clone, ExtractComponent)]
pub(super) struct DlaaView;
#[derive(Resource, Clone, ExtractResource)]
pub struct State {
    ready: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    forced_history_reset: Arc<AtomicBool>,
}
impl State {
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
pub fn install(app: &mut App) {
    app.insert_resource(State {
        ready: Arc::new(AtomicBool::new(true)),
        failed: Arc::default(),
        forced_history_reset: Arc::default(),
    })
    .add_plugins((
        ExtractComponentPlugin::<DlaaView>::default(),
        ExtractResourcePlugin::<State>::default(),
    ))
    .add_systems(PostStartup, activate)
    .add_systems(
        PostUpdate,
        reset_on_cut.after(bevy::transform::TransformSystems::Propagate),
    );
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(Render, report_prepared.in_set(RenderSystems::Cleanup));
    }
}
fn activate(
    mut commands: Commands,
    settings: Res<super::graphics::Settings>,
    supported: Option<Res<DlssSuperResolutionSupported>>,
    state: Res<State>,
    views: Query<Entity, With<super::ViewerCamera>>,
) {
    if settings.anti_aliasing != 3 {
        return;
    }
    state.forced_history_reset.store(settings.dlaa_reset_every_frame, Ordering::Release);
    for entity in &views {
        if supported.is_some() {
            state.ready.store(false, Ordering::Release);
            commands.entity(entity).insert((
                DlaaView,
                Dlss {
                    perf_quality_mode: DlssPerfQualityMode::Dlaa,
                    reset: true,
                    ..default()
                },
            ));
            println!(
                "graphics_dlaa requested=true supported=true effective=dlaa native_resolution=true sharpening=false dispatch_verified=false"
            );
        } else {
            commands.entity(entity).insert(Smaa {
                preset: SmaaPreset::Ultra,
            });
            println!(
                "graphics_dlaa requested=true supported=false effective=smaa reason=runtime-sdk-or-device-unavailable"
            );
        }
    }
}
fn report_prepared(
    state: Res<State>,
    views: Query<
        (
            &ExtractedView,
            &MainPassResolutionOverride,
            &ViewPrepassTextures,
            &Dlss,
            &bevy::render::camera::MipBias,
        ),
        With<DlaaView>,
    >,
    mut previous: Local<Option<UVec2>>,
) {
    for (view, resolution, prepass, dlss, mip_bias) in &views {
        let output = UVec2::new(view.viewport.z, view.viewport.w);
        let input = resolution.0;
        if (state.forced_history_reset.load(Ordering::Acquire) && !dlss.reset)
            || input != output
            || dlss.perf_quality_mode != DlssPerfQualityMode::Dlaa
            || prepass.depth.is_none()
            || prepass.motion_vectors.is_none()
        {
            if !state.failed.swap(true, Ordering::AcqRel) {
                eprintln!(
                    "graphics_dlaa prepared=false input={input:?} output={output:?} reason=invalid-native-resolution-or-prepass"
                );
            }
            return;
        }
        if *previous != Some(input) {
            println!(
                "graphics_dlaa prepared=true input={}x{} output={}x{} depth=true motion=true mip_bias={} history_reset={} forced_history_reset={} scope=context-and-completed-render-schedule-not-gpu-readback",
                input.x, input.y, output.x, output.y, mip_bias.0, dlss.reset, state.forced_history_reset.load(Ordering::Acquire)
            );
            *previous = Some(input);
        }
        state.ready.store(true, Ordering::Release);
    }
}
#[derive(Default)]
struct History {
    tick: u64,
    view: String,
    translation: Vec3,
    rotation: Quat,
}
fn reset_on_cut(
    settings: Res<super::graphics::Settings>,
    driving: Option<Res<super::Driving>>,
    cameras: Option<Res<super::camera::Cameras>>,
    mut views: Query<(&GlobalTransform, &mut Dlss), With<super::ViewerCamera>>,
    mut previous: Local<Option<History>>,
) {
    let Ok((transform, mut dlss)) = views.single_mut() else {
        return;
    };
    if settings.dlaa_reset_every_frame {
        if previous.is_none() {
            println!("graphics_dlaa diagnostic_history_reset_every_frame=true scope=bounded-scripted-capture-not-playable-fix");
        }
        dlss.reset = true;
    }
    let tick = driving.map_or(0, |d| d.runtime.ticks);
    let view = cameras.map_or_else(String::new, |c| c.ident().to_owned());
    let camera = transform.compute_transform();
    if let Some(old) = previous.as_ref() {
        let cut = tick < old.tick
            || view != old.view
            || camera.translation.distance(old.translation) > 25.
            || camera.rotation.angle_between(old.rotation).abs() > 1.2;
        if cut {
            dlss.reset = true;
            println!("graphics_dlaa history_reset=true tick={tick} view={view}");
        }
    }
    *previous = Some(History {
        tick,
        view,
        translation: camera.translation,
        rotation: camera.rotation,
    });
}
