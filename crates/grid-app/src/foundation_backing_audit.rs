//! Actual main opaque-bin association, not GPU fragments or final-screenshot linkage.
use super::foundation_backing::{self as backing, Fixture, Mode, RawMaterial, TARGET};
use bevy::{
    asset::{AssetId, UntypedAssetId},
    camera::{MainPassResolutionOverride, NormalizedRenderTarget, RenderTarget},
    core_pipeline::{
        Core3d,
        core_3d::Opaque3d,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass},
    },
    diagnostic::FrameCount,
    ecs::schedule::ScheduleLabel,
    light::{ClusteredDecal, NotShadowCaster},
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        batching::NoAutomaticBatching,
        camera::ExtractedCamera,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{PipelineCache, TextureFormat},
        view::{ExtractedView, RenderVisibleEntities, ViewTarget},
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::atomic::Ordering,
};
#[derive(Clone)]
struct Meta {
    mesh: AssetId<Mesh>,
    material: String,
    cull: Option<bevy::render::render_resource::Face>,
}
#[derive(Resource, Default)]
struct Snapshot {
    mode: Option<Mode>,
    frame: u32,
    camera: Option<Entity>,
    target: Option<NormalizedRenderTarget>,
    rows: HashMap<Entity, Meta>,
}
pub(crate) fn install(app: &mut App) {
    app.get_sub_app_mut(RenderApp)
        .expect("backing audit renderer")
        .init_resource::<Snapshot>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(
            Render,
            record
                .after(RenderSystems::Queue)
                .before(RenderSystems::PrepareResourcesBatchPhases),
        );
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn extract(
    fixture: Extract<Res<Fixture>>,
    decals: Extract<Query<(), With<ClusteredDecal>>>,
    frame: Extract<Res<FrameCount>>,
    cameras: Extract<Query<(Entity, &RenderTarget), With<super::ViewerCamera>>>,
    parts: Extract<
        Query<(
            Entity,
            &super::track_surface::SourcePart,
            &Mesh3d,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&MeshMaterial3d<RawMaterial>>,
            Has<NoAutomaticBatching>,
            Has<NotShadowCaster>,
        )>,
    >,
    standards: Extract<Res<Assets<StandardMaterial>>>,
    raws: Extract<Option<Res<Assets<RawMaterial>>>>,
    mut s: ResMut<Snapshot>,
) {
    s.mode = None;
    s.rows.clear();
    if fixture.stopping.load(Ordering::Acquire) {
        return;
    }
    assert!(
        backing::clustered_scene_admitted(&decals),
        "backing no main-world ClusteredDecal contract"
    );
    let (camera, target) = cameras.single().expect("one backing main camera");
    let target = target.normalize(None).expect("backing image target");
    assert!(matches!(target, NormalizedRenderTarget::Image(_)));
    for (entity, part, mesh, standard, raw, no_auto, no_shadow) in &parts {
        if part.0 != TARGET {
            continue;
        }
        assert!(no_auto && no_shadow, "backing matched markers missing");
        assert!(s.rows.len() < 128);
        let (material, cull) = if fixture.mode == Mode::RawA2c {
            assert!(standard.is_none());
            let handle = &raw.expect("raw backing missing typed handle").0;
            let base = &raws
                .as_ref()
                .and_then(|a| a.get(handle))
                .expect("raw backing asset")
                .base;
            assert!(
                backing::zero_material_emission(base),
                "backing raw emission changed after attach"
            );
            assert_eq!(base.alpha_mode, AlphaMode::AlphaToCoverage);
            (format!("raw:{:?}", handle.id()), base.cull_mode)
        } else {
            assert!(raw.is_none());
            let handle = &standard.expect("control backing missing typed handle").0;
            let base = standards.get(handle).expect("control backing asset");
            assert!(
                backing::zero_material_emission(base),
                "backing control emission changed after attach"
            );
            assert_eq!(base.alpha_mode, AlphaMode::Opaque);
            (format!("standard:{:?}", handle.id()), base.cull_mode)
        };
        s.rows.insert(
            entity,
            Meta {
                mesh: mesh.0.id(),
                material,
                cull,
            },
        );
    }
    assert!(!s.rows.is_empty());
    s.mode = Some(fixture.mode);
    s.frame = frame.0;
    s.camera = Some(camera);
    s.target = Some(target);
}
fn view_ok(
    v: &ExtractedView,
    c: &ExtractedCamera,
    format: Option<TextureFormat>,
    msaa: Option<&Msaa>,
    override_size: bool,
    aux_prepasses: bool,
    target: &NormalizedRenderTarget,
) -> bool {
    v.retained_view_entity.auxiliary_entity.id() == Entity::PLACEHOLDER
        && v.retained_view_entity.subview_index == 0
        && c.schedule == Core3d.intern()
        && c.target.as_ref() == Some(target)
        && matches!(target, NormalizedRenderTarget::Image(_))
        && c.physical_target_size == Some(UVec2::new(1920, 1080))
        && c.viewport.is_none()
        && !v.invert_culling
        && v.viewport == UVec4::new(0, 0, 1920, 1080)
        && format == Some(TextureFormat::Rgba8UnormSrgb)
        && msaa.is_some_and(|m| m.samples() == 4)
        && !override_size
        && !aux_prepasses
}
fn admit_owner(
    seen: &mut HashSet<Entity>,
    visible: &HashSet<Entity>,
    main: Entity,
    expected: AssetId<Mesh>,
    actual: UntypedAssetId,
) -> Result<(), &'static str> {
    if !visible.contains(&main) || actual != expected.untyped() || !seen.insert(main) {
        return Err("backing duplicate/foreign/unseen opaque-bin identity");
    }
    Ok(())
}
#[allow(clippy::type_complexity)]
fn record(
    s: Res<Snapshot>,
    decals: Query<(), With<ClusteredDecal>>,
    phases: Res<ViewBinnedRenderPhases<Opaque3d>>,
    cache: Res<PipelineCache>,
    views: Query<(
        Entity,
        &ExtractedView,
        &ExtractedCamera,
        Option<&ViewTarget>,
        Option<&Msaa>,
        Option<&MainPassResolutionOverride>,
        Option<&RenderVisibleEntities>,
        Has<DepthPrepass>,
        Has<NormalPrepass>,
        Has<MotionVectorPrepass>,
        Has<DeferredPrepass>,
    )>,
    mut done: Local<bool>,
) {
    assert!(
        backing::clustered_scene_admitted(&decals),
        "backing no render-world ClusteredDecal contract"
    );
    if *done {
        return;
    }
    let Some(mode) = s.mode else {
        return;
    };
    let target = s.target.as_ref().unwrap();
    let mut matches = views.iter().filter(|(_, v, ..)| {
        v.retained_view_entity.main_entity.id() == s.camera.unwrap()
            && v.retained_view_entity.subview_index == 0
    });
    let Some((render, v, c, vt, msaa, override_size, visible, dp, np, mp, dd)) = matches.next()
    else {
        return;
    };
    assert!(matches.next().is_none());
    assert!(
        view_ok(
            v,
            c,
            vt.map(ViewTarget::main_texture_format),
            msaa,
            override_size.is_some(),
            dp || np || mp || dd,
            target
        ),
        "backing actual view contract"
    );
    let Some(class) = visible.and_then(|v| v.get::<Mesh3d>()) else {
        return;
    };
    assert!(class.entities_cpu_culling.len() <= 50000);
    assert!(
        !class
            .entities_gpu_culling
            .keys()
            .any(|m| s.rows.contains_key(&m.id())),
        "backing GPU-only visibility unsupported"
    );
    let visible: HashSet<_> = class
        .entities_cpu_culling
        .iter()
        .map(|(_, e)| e.id())
        .filter(|e| s.rows.contains_key(e))
        .collect();
    if visible.is_empty() {
        return;
    }
    let Some(phase) = phases.get(&v.retained_view_entity) else {
        return;
    };
    if phase.unbatchable_meshes.len() > 50000 {
        panic!("backing bin scan bound");
    }
    let mut found = HashSet::new();
    let mut receipt = Vec::new();
    let mut scanned = 0usize;
    for ((batch, bin), bucket) in &phase.unbatchable_meshes {
        for main in bucket.entities.keys() {
            scanned += 1;
            assert!(scanned <= 50000, "backing unbatchable entity scan bound");
            if !visible.contains(&main.id()) {
                continue;
            }
            let meta = &s.rows[&main.id()];
            admit_owner(&mut found, &visible, main.id(), meta.mesh, bin.asset_id)
                .expect("backing opaque-bin ownership");
            if cache.get_render_pipeline(batch.pipeline).is_none() {
                return;
            }
            let d = cache.get_render_pipeline_descriptor(batch.pipeline);
            assert!(
                backing::actual_pipeline(d, mode == Mode::RawA2c)
                    && d.primitive.cull_mode == meta.cull,
                "backing actual ready pipeline state"
            );
            receipt.push(format!("backing_phase mode={mode:?} main={main:?} mesh={:?} material={} pipeline={:?} ready=true hardware_a2c={} format={:?} samples={} colour_alpha_blend={:?} depth={:?} cull={:?} vertex_defs={:?} fragment_defs={:?} shader={:?} stage=opaque_unbatchable_before_prepare",meta.mesh,meta.material,batch.pipeline,d.multisample.alpha_to_coverage_enabled,d.fragment.as_ref().unwrap().targets[0].as_ref().unwrap().format,d.multisample.count,d.fragment.as_ref().unwrap().targets[0].as_ref().unwrap().blend,d.depth_stencil,d.primitive.cull_mode,d.vertex.shader_defs,d.fragment.as_ref().unwrap().shader_defs,d.fragment.as_ref().unwrap().shader));
        }
    }
    if found != visible {
        return;
    }
    println!(
        "backing_view mode={mode:?} frame={} render_view={render:?} actual_format={:?} samples={} native=1920x1080 full_viewport=true retained_auxiliary=None retained_subview=0 core3d_schedule=true aux_prepasses=false",
        s.frame,
        vt.unwrap().main_texture_format(),
        msaa.unwrap().samples()
    );
    for line in receipt {
        println!("{line}");
    }
    println!(
        "backing_receipt status=complete mode={mode:?} frame={} selected={} visible={} ready={} matched_no_auto=true matched_no_shadow=true material_emission=zero emissive_texture=false main_clustered_decals=0 render_clustered_decals=0 scene_guard=every_extract_and_render_record scope=CPU_opaque_queue_not_fragments_or_screenshot",
        s.frame,
        s.rows.len(),
        visible.len(),
        found.len()
    );
    *done = true;
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (ExtractedView, ExtractedCamera, NormalizedRenderTarget) {
        let target = NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget {
            handle: Handle::default(),
            scale_factor: 1.,
        });
        let defaults = Camera::default();
        let c = ExtractedCamera {
            target: Some(target.clone()),
            physical_viewport_size: Some(UVec2::new(1920, 1080)),
            physical_target_size: Some(UVec2::new(1920, 1080)),
            viewport: None,
            schedule: Core3d.intern(),
            order: 0,
            output_mode: defaults.output_mode,
            msaa_writeback: defaults.msaa_writeback,
            clear_color: defaults.clear_color,
            sorted_camera_index_for_target: 0,
            exposure: 1.,
            hdr: false,
            compositing_space: None,
        };
        let v = ExtractedView {
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
        (v, c, target)
    }
    #[test]
    fn actual_view_guard_rejects_wrong_target_format_msaa_auxiliary_and_inverted_view() {
        let (mut v, mut c, t) = fixture();
        assert!(view_ok(
            &v,
            &c,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(&Msaa::Sample4),
            false,
            false,
            &t
        ));
        assert!(!view_ok(
            &v,
            &c,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(&Msaa::Sample4),
            false,
            true,
            &t
        ));
        assert!(!view_ok(
            &v,
            &c,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(&Msaa::Sample4),
            true,
            false,
            &t
        ));
        assert!(!view_ok(
            &v,
            &c,
            None,
            Some(&Msaa::Sample4),
            false,
            false,
            &t
        ));
        assert!(!view_ok(
            &v,
            &c,
            Some(TextureFormat::Rgba16Float),
            Some(&Msaa::Sample4),
            false,
            false,
            &t
        ));
        assert!(!view_ok(
            &v,
            &c,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(&Msaa::Off),
            false,
            false,
            &t
        ));
        v.invert_culling = true;
        assert!(!view_ok(
            &v,
            &c,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(&Msaa::Sample4),
            false,
            false,
            &t
        ));
        v.invert_culling = false;
        c.physical_target_size = None;
        assert!(!view_ok(
            &v,
            &c,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(&Msaa::Sample4),
            false,
            false,
            &t
        ));
    }
    #[test]
    fn retained_auxiliary_subview_and_non_core3d_schedule_are_not_main_view_proof() {
        let (mut v, mut c, target) = fixture();
        let admitted = |v: &ExtractedView, c: &ExtractedCamera| {
            view_ok(
                v,
                c,
                Some(TextureFormat::Rgba8UnormSrgb),
                Some(&Msaa::Sample4),
                false,
                false,
                &target,
            )
        };
        assert!(admitted(&v, &c));
        let mut world = World::new();
        let auxiliary = world.spawn_empty().id();
        v.retained_view_entity.auxiliary_entity = auxiliary.into();
        // No prepass flags: auxiliary identity alone must still refuse.
        assert!(!admitted(&v, &c));
        v.retained_view_entity.auxiliary_entity = Entity::PLACEHOLDER.into();
        v.retained_view_entity.subview_index = 1;
        assert!(!admitted(&v, &c));
        v.retained_view_entity.subview_index = 0;
        assert!(admitted(&v, &c));
        c.schedule = bevy::app::Main.intern();
        assert!(!admitted(&v, &c));
    }
    #[test]
    fn opaque_bin_join_rejects_wrong_mesh_duplicate_and_unseen_main_entity() {
        let mut world = World::new();
        let own = world.spawn_empty().id();
        let foreign = world.spawn_empty().id();
        let a: Handle<Mesh> = bevy::asset::uuid_handle!("12345678-1111-2222-3333-444444444444");
        let b: Handle<Mesh> = bevy::asset::uuid_handle!("12345678-1111-2222-3333-555555555555");
        let visible = HashSet::from([own]);
        let mut seen = HashSet::new();
        assert!(admit_owner(&mut seen, &visible, own, a.id(), a.id().untyped()).is_ok());
        assert!(admit_owner(&mut seen, &visible, own, a.id(), a.id().untyped()).is_err());
        assert!(admit_owner(&mut HashSet::new(), &visible, own, a.id(), b.id().untyped()).is_err());
        assert!(
            admit_owner(
                &mut HashSet::new(),
                &visible,
                foreign,
                a.id(),
                a.id().untyped()
            )
            .is_err()
        );
    }
}
