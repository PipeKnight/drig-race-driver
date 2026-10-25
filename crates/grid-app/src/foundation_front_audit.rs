//! Read-only bounded one-main-view receipt; phase association is not GPU pixel proof.
use super::foundation_front::{self, FrontMaterial, Mode, TARGET};
use bevy::{
    camera::{MainPassResolutionOverride, NormalizedRenderTarget, RenderTarget},
    core_pipeline::core_3d::Transparent3d,
    diagnostic::FrameCount,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        camera::ExtractedCamera,
        render_phase::{ViewSortedRenderPhases, sort_phase_system},
        render_resource::{PipelineCache, TextureFormat},
        view::{ExtractedView, RenderVisibleEntities, ViewTarget},
    },
};
use std::collections::{HashMap, HashSet};
#[derive(Resource, Clone, Copy)]
struct Config(Mode);
#[derive(Clone)]
struct Meta {
    mesh: String,
    material: String,
    front: bool,
    alpha: String,
}
#[derive(Resource, Default)]
struct Snapshot {
    mode: Option<Mode>,
    frame: u32,
    camera: Option<Entity>,
    target: Option<NormalizedRenderTarget>,
    rows: HashMap<Entity, Meta>,
}
pub(crate) fn install(app: &mut App, mode: Mode) {
    if mode == Mode::Off {
        return;
    }
    app.insert_resource(Config(mode));
    app.get_sub_app_mut(RenderApp)
        .expect("foundation receipt needs renderer")
        .init_resource::<Snapshot>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(
            Render,
            record
                .in_set(RenderSystems::PhaseSort)
                .after(sort_phase_system::<Transparent3d>),
        );
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Read-only typed source/material ownership, bounded128.
fn extract(
    config: Extract<Res<Config>>,
    frame: Extract<Res<FrameCount>>,
    cameras: Extract<Query<(Entity, &RenderTarget), With<super::ViewerCamera>>>,
    parts: Extract<
        Query<(
            Entity,
            &super::track_surface::SourcePart,
            &Mesh3d,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&MeshMaterial3d<FrontMaterial>>,
        )>,
    >,
    standards: Extract<Res<Assets<StandardMaterial>>>,
    fronts: Extract<Option<Res<Assets<FrontMaterial>>>>,
    mut snapshot: ResMut<Snapshot>,
) {
    snapshot.mode = None;
    snapshot.rows.clear();
    let (camera, target) = cameras
        .single()
        .expect("foundation receipt needs one main camera");
    let target = target
        .normalize(None)
        .expect("foundation image target normalizes");
    assert!(
        matches!(target, NormalizedRenderTarget::Image(_)),
        "foundation expects image render target"
    );
    for (entity, part, mesh, standard, front) in &parts {
        if part.0 != TARGET {
            continue;
        }
        assert!(
            snapshot.rows.len() < 128,
            "foundation source metadata bound"
        );
        let (material, is_front, alpha) = match config.0 {
            Mode::Control => {
                assert!(front.is_none(), "Control source unexpectedly extended");
                let handle = &standard
                    .expect("Control missing original standard material")
                    .0;
                assert_eq!(
                    standards
                        .get(handle)
                        .expect("Control material absent")
                        .alpha_mode,
                    AlphaMode::Opaque
                );
                (
                    format!("standard:{:?}", handle.id()),
                    false,
                    format!("{:?}", standards.get(handle).unwrap().alpha_mode),
                )
            }
            Mode::SourceFront => {
                assert!(
                    standard.is_none(),
                    "SourceFront retains duplicate standard draw component"
                );
                let handle = &front.expect("SourceFront missing extended material").0;
                assert_eq!(
                    fronts
                        .as_ref()
                        .and_then(|assets| assets.get(handle))
                        .expect("SourceFront asset absent")
                        .base
                        .alpha_mode,
                    AlphaMode::Blend
                );
                (
                    format!("front:{:?}", handle.id()),
                    true,
                    format!(
                        "{:?}",
                        fronts
                            .as_ref()
                            .and_then(|assets| assets.get(handle))
                            .unwrap()
                            .base
                            .alpha_mode
                    ),
                )
            }
            Mode::Off => unreachable!(),
        };
        snapshot.rows.insert(
            entity,
            Meta {
                mesh: format!("{:?}", mesh.0.id()),
                material,
                front: is_front,
                alpha,
            },
        );
    }
    assert!(
        !snapshot.rows.is_empty(),
        "foundation target absent in Extract"
    );
    snapshot.frame = frame.0;
    snapshot.camera = Some(camera);
    snapshot.target = Some(target);
    snapshot.mode = Some(config.0);
}
/// Same actual attachment predicates for BOTH arms; format comes from ViewTarget.
fn valid_view(
    view: &ExtractedView,
    camera: &ExtractedCamera,
    format: Option<TextureFormat>,
    msaa: Option<&Msaa>,
    resolution: Option<&MainPassResolutionOverride>,
    expected: &NormalizedRenderTarget,
) -> bool {
    camera.target.as_ref() == Some(expected)
        && matches!(expected, NormalizedRenderTarget::Image(_))
        && camera.physical_target_size == Some(UVec2::new(1920, 1080))
        && camera.viewport.is_none()
        && view.viewport == UVec4::new(0, 0, 1920, 1080)
        && resolution.is_none()
        && msaa.is_some_and(|m| m.samples() == 4)
        && format == Some(TextureFormat::Rgba8UnormSrgb)
}
#[allow(clippy::type_complexity)] // Optional actual attachment components; no GPU target fabrication.
fn record(
    snapshot: Res<Snapshot>,
    phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(
        Entity,
        &ExtractedView,
        &ExtractedCamera,
        Option<&ViewTarget>,
        Option<&Msaa>,
        Option<&MainPassResolutionOverride>,
        Option<&RenderVisibleEntities>,
    )>,
    pipelines: Res<PipelineCache>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(mode) = snapshot.mode else {
        return;
    };
    let expected = snapshot.target.as_ref().expect("extracted image identity");
    let mut candidates = views.iter().filter(|(_, v, ..)| {
        v.retained_view_entity.main_entity.id() == snapshot.camera.unwrap()
            && v.retained_view_entity.subview_index == 0
    });
    let Some((render_view, view, camera, target, msaa, resolution, visible)) = candidates.next()
    else {
        return;
    };
    assert!(
        candidates.next().is_none(),
        "duplicate foundation main view"
    );
    assert!(
        valid_view(
            view,
            camera,
            target.map(ViewTarget::main_texture_format),
            msaa,
            resolution,
            expected
        ),
        "foundation actual attachment contract refused"
    );
    let Some(class) = visible.and_then(|v| v.get::<Mesh3d>()) else {
        return;
    };
    assert!(
        class.entities_cpu_culling.len() <= 50000,
        "foundation visibility scan bound"
    );
    // GPU-culling candidate membership does not prove actual visibility; refuse this domain.
    assert!(
        !class
            .entities_gpu_culling
            .keys()
            .any(|main| snapshot.rows.contains_key(&main.id())),
        "foundation target GPU-culling visibility is not admitted"
    );
    let visible_targets: HashSet<_> = class
        .entities_cpu_culling
        .iter()
        .map(|(_, main)| main.id())
        .filter(|main| snapshot.rows.contains_key(main))
        .collect();
    if visible_targets.is_empty() {
        return;
    } // No successful receipt for an off-screen fixture.
    let Some(phase) = phases.get(&view.retained_view_entity) else {
        return;
    };
    assert!(
        phase.items.len() <= 50000,
        "foundation transparent phase scan bound"
    );
    let items: Vec<_> = phase
        .items
        .iter()
        .filter(|(_, item)| snapshot.rows.contains_key(&item.entity.1.id()))
        .collect();
    if mode == Mode::Control {
        assert!(
            items.is_empty(),
            "Control exact opaque target entered Transparent3d"
        );
    }
    if mode == Mode::SourceFront {
        if items.len() != visible_targets.len() {
            return;
        }
        let mut seen = HashSet::new();
        for (_, item) in &items {
            let main = item.entity.1.id();
            assert!(
                visible_targets.contains(&main) && seen.insert(main),
                "foundation target unexpected/duplicate queued identity"
            );
            if pipelines.get_render_pipeline(item.pipeline).is_none() {
                return;
            }
            assert!(
                foundation_front::actual_pipeline(
                    pipelines.get_render_pipeline_descriptor(item.pipeline)
                ),
                "foundation ready queued pipeline contract refused"
            );
        }
    }
    // Publish only AFTER all required entries are ready and contracts agree.
    println!(
        "foundation_view mode={mode:?} frame={} main={:?} render_view={render_view:?} image_target={expected:?} actual_format={:?} samples={} native=1920x1080 full_viewport=true override=false",
        snapshot.frame,
        snapshot.camera.unwrap(),
        target.unwrap().main_texture_format(),
        msaa.unwrap().samples()
    );
    for main in &visible_targets {
        let meta = &snapshot.rows[main];
        println!(
            "foundation_visible mode={mode:?} main={main:?} mesh={} material={} typed_front={} base_alpha={} domain=cpu_main_visible",
            meta.mesh, meta.material, meta.front, meta.alpha
        );
    }
    for (_, item) in items {
        let main = item.entity.1.id();
        let meta = &snapshot.rows[&main];
        let descriptor = pipelines.get_render_pipeline_descriptor(item.pipeline);
        println!(
            "foundation_phase mode={mode:?} main={main:?} mesh={} material={} pipeline={:?} ready=true targets={} format={:?} samples={} a2c={} blend={:?} depth={:?} cull={:?} stage=sorted_before_batching",
            meta.mesh,
            meta.material,
            item.pipeline,
            descriptor.fragment.as_ref().unwrap().targets.len(),
            descriptor.fragment.as_ref().unwrap().targets[0]
                .as_ref()
                .unwrap()
                .format,
            descriptor.multisample.count,
            descriptor.multisample.alpha_to_coverage_enabled,
            descriptor.fragment.as_ref().unwrap().targets[0]
                .as_ref()
                .unwrap()
                .blend,
            descriptor.depth_stencil,
            descriptor.primitive.cull_mode
        );
    }
    println!(
        "foundation_receipt status=complete mode={mode:?} frame={} selected={} visible={} queued={} source_front_pipeline_join={} control_opaque_phase_join=false scope=one_main_CPU_visibility_sorted_phase_not_GPU_pixels",
        snapshot.frame,
        snapshot.rows.len(),
        visible_targets.len(),
        if mode == Mode::SourceFront {
            visible_targets.len()
        } else {
            0
        },
        mode == Mode::SourceFront
    );
    *done = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::ScheduleLabel;
    fn fixture() -> (ExtractedView, ExtractedCamera, NormalizedRenderTarget) {
        let target = NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget {
            handle: Handle::default(),
            scale_factor: 1.,
        });
        let defaults = Camera::default();
        let camera = ExtractedCamera {
            target: Some(target.clone()),
            physical_viewport_size: Some(UVec2::new(1920, 1080)),
            physical_target_size: Some(UVec2::new(1920, 1080)),
            viewport: None,
            schedule: bevy::app::Main.intern(),
            order: 0,
            output_mode: defaults.output_mode,
            msaa_writeback: defaults.msaa_writeback,
            clear_color: defaults.clear_color,
            sorted_camera_index_for_target: 0,
            exposure: 1.,
            hdr: false,
            compositing_space: None,
        };
        let view = ExtractedView {
            retained_view_entity: bevy::render::view::RetainedViewEntity::new(
                Entity::PLACEHOLDER.into(),
                None,
                0,
            ),
            clip_from_view: Mat4::IDENTITY,
            world_from_view: GlobalTransform::IDENTITY,
            clip_from_world: None,
            target_format: TextureFormat::Rgba8UnormSrgb,
            viewport: UVec4::new(0, 0, 1920, 1080),
            color_grading: default(),
            invert_culling: false,
        };
        (view, camera, target)
    }
    #[test]
    fn both_arms_actual_view_guard_refuses_wrong_or_missing_attachments() {
        let (mut view, mut camera, target) = fixture();
        let okay = |v: &ExtractedView, c: &ExtractedCamera| {
            valid_view(
                v,
                c,
                Some(TextureFormat::Rgba8UnormSrgb),
                Some(&Msaa::Sample4),
                None,
                &target,
            )
        };
        assert!(okay(&view, &camera));
        camera.physical_target_size = None;
        assert!(!okay(&view, &camera));
        camera.physical_target_size = Some(UVec2::new(1920, 1080));
        camera.viewport = Some(bevy::camera::Viewport::default());
        assert!(!okay(&view, &camera));
        camera.viewport = None;
        view.viewport.x = 1;
        assert!(!okay(&view, &camera));
        view.viewport.x = 0;
        assert!(!valid_view(
            &view,
            &camera,
            None,
            Some(&Msaa::Sample4),
            None,
            &target
        ));
        assert!(!valid_view(
            &view,
            &camera,
            Some(TextureFormat::Rgba16Float),
            Some(&Msaa::Sample4),
            None,
            &target
        ));
        assert!(!valid_view(
            &view,
            &camera,
            Some(TextureFormat::Rgba8UnormSrgb),
            None,
            None,
            &target
        ));
        assert!(!valid_view(
            &view,
            &camera,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(&Msaa::Off),
            None,
            &target
        ));
        let override_size = MainPassResolutionOverride(UVec2::new(960, 540));
        assert!(!valid_view(
            &view,
            &camera,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(&Msaa::Sample4),
            Some(&override_size),
            &target
        ));
        camera.target = None;
        assert!(!okay(&view, &camera));
    }
}
