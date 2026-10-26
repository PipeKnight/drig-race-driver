//! Opt-in BMW baseline live bridge; shares the existing five-group transaction.
//! No screenshot ledger, source shader/material/physics changes or native-AA admission.
use super::{Audit, core};
use bevy::{
    camera::{MainPassResolutionOverride, NormalizedRenderTarget, RenderTarget},
    core_pipeline::{
        core_3d::{AlphaMask3d, Opaque3d, Transparent3d, main_transparent_pass_3d},
        schedule::{Core3d, Core3dSystems},
    },
    diagnostic::FrameCount,
    ecs::{schedule::ScheduleLabel, system::SystemParam},
    pbr::{
        MaterialBindGroupAllocators, MeshViewBindGroup, PreparedMaterial, PreprocessBindGroups,
        PreprocessPipelines, RenderMaterialInstances, RenderMeshInstances,
    },
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport},
        camera::ExtractedCamera,
        erased_render_asset::ErasedRenderAssets,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_phase::{
            PhaseItem, SortedRenderPhase, ViewBinnedRenderPhases, ViewSortedRenderPhases,
        },
        render_resource::{CachedComputePipelineId, Extent3d, PipelineCache, TextureFormat},
        renderer::{RenderContext, ViewQuery},
        sync_world::MainEntity,
        view::{
            ExtractedView, RetainedViewEntity, ViewDepthTexture, ViewTarget, ViewUniformOffset,
        },
    },
    window::PrimaryWindow,
};
use std::sync::{Arc, Mutex, MutexGuard};
#[derive(Default)]
struct Counters {
    calls: u64,
    fallback: [u64; 7],
    replayed: u64,
    first_reported: bool,
    native_deferral_reported: bool,
    error: Option<&'static str>,
}
#[derive(Resource, Clone, Default, ExtractResource)]
pub(crate) struct State(Arc<Mutex<Counters>>);
impl State {
    fn lock(&self) -> MutexGuard<'_, Counters> {
        self.0.lock().unwrap_or_else(|e| {
            let mut s = e.into_inner();
            s.error = Some("live boundary state poisoned");
            s
        })
    }
    pub(crate) fn failed(&self) -> bool {
        self.lock().error.is_some()
    }
    fn fail(&self, why: &'static str) {
        self.lock().error.get_or_insert(why);
    }
    fn fallback(&self, index: usize) {
        self.lock().fallback[index] += 1;
    }
    fn native_deferral(&self, reason: &'static str, snapshot: &Snapshot) {
        let mut stats = self.lock();
        if !stats.native_deferral_reported {
            println!(
                "smoke_boundary_live event=first_native_deferral frame={} segment={} tick={} reason={reason} masked=false ordinary_fallback=true",
                snapshot.frame, snapshot.segment, snapshot.tick
            );
            stats.native_deferral_reported = true;
        }
    }
}
#[derive(Resource)]
struct Ids(core::Ids);
#[derive(Resource, Default)]
struct Snapshot {
    frame: u32,
    segment: u64,
    tick: u64,
    camera: Option<Entity>,
    target: Option<NormalizedRenderTarget>,
    groups: Vec<MainEntity>,
}
#[derive(Resource, Default)]
struct Routing {
    frame: core::Frame,
    view: Option<RetainedViewEntity>,
    clock: Option<(u32, u64, u64)>,
    prepared: bool,
    size: Option<UVec2>,
}
impl Routing {
    fn clean(&self) -> bool {
        self.view.is_none() && self.clock.is_none() && !self.prepared && self.frame.clean()
    }
}
fn stamp(snapshot: &Snapshot) -> (u32, u64, u64) {
    (snapshot.frame, snapshot.segment, snapshot.tick)
}
pub(super) fn install(app: &mut App) -> std::io::Result<()> {
    if app.world().contains_resource::<State>() || app.world().contains_resource::<Audit>() {
        return Err(std::io::Error::other("boundary already installed"));
    }
    let Some((_, ids)) = core::register(app, true)? else {
        return Err(std::io::Error::other("live boundary renderer absent"));
    };
    // The first Extract already reads render-world Res<State>. Resource extraction
    // through Commands would insert it only later in Render::ExtractCommands.
    // Share one Arc eagerly, exactly as the existing transaction Proof does.
    let state = State::default();
    app.insert_resource(state.clone())
        .add_plugins(ExtractResourcePlugin::<State>::default())
        .add_systems(Update, publish);
    let render = app
        .get_sub_app_mut(RenderApp)
        .ok_or_else(|| std::io::Error::other("live boundary render app absent"))?;
    render
        .insert_resource(state)
        .insert_resource(Ids(ids))
        .init_resource::<Snapshot>()
        .init_resource::<Routing>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(
            Render,
            barrier
                .after(RenderSystems::PhaseSort)
                .before(RenderSystems::PrepareResourcesBatchPhases),
        )
        .add_systems(
            Render,
            prepare
                .after(RenderSystems::PrepareBindGroups)
                .before(RenderSystems::Render),
        )
        .add_systems(
            Core3d,
            replay
                .after(main_transparent_pass_3d)
                .in_set(Core3dSystems::MainPass),
        )
        .add_systems(
            Render,
            finalize
                .after(RenderSystems::Render)
                .before(RenderSystems::Cleanup),
        );
    println!(
        "smoke_boundary mode=grouped-live scope=bmw-istanbul0-baseline groups=5 screenshot_ledger=false native_aa=false positive_fragments=unknown"
    );
    Ok(())
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn extract(
    frame: Extract<Res<FrameCount>>,
    driving: Extract<Res<crate::Driving>>,
    cameras: Extract<Query<(Entity, &Camera, &RenderTarget), With<crate::ViewerCamera>>>,
    windows: Extract<Query<Entity, With<PrimaryWindow>>>,
    smoke: Extract<Option<Res<crate::tyre_smoke::TyreSmoke>>>,
    meshes: Extract<Query<(Entity, &Mesh3d)>>,
    mut snapshot: ResMut<Snapshot>,
    mut route: ResMut<Routing>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    proof: Res<core::Proof>,
    state: Res<State>,
) {
    if !route.clean() {
        // Missing a mandatory previous finalizer is an invariant violation, not cold startup.
        state.fail("live previous frame missed finalizer");
        if let Some(view) = route.view {
            if let Some(phase) = phases.get_mut(&view) {
                let _ = core::restore(phase, &mut route.frame, &proof);
            } else if route.prepared {
                route.frame.discard_missing_phase(&proof);
            }
        }
        *route = Routing::default();
    }
    *snapshot = Snapshot {
        frame: frame.0,
        segment: driving.segment,
        tick: driving.runtime.ticks,
        ..default()
    };
    if state.failed() {
        return;
    }
    let mut active = cameras.iter().filter(|(_, camera, _)| camera.is_active);
    let Some((entity, _, target)) = active.next() else {
        return;
    };
    if active.next().is_some() {
        return;
    }
    let normalized = normalize_target(target, windows.single().ok());
    if !matches!(
        normalized,
        Some(NormalizedRenderTarget::Window(_) | NormalizedRenderTarget::Image(_))
    ) {
        return;
    }
    snapshot.camera = Some(entity);
    snapshot.target = normalized;
    let Some(smoke) = smoke.as_ref() else {
        return;
    };
    let handles = smoke.mesh_handles();
    if handles.len() != 5
        || handles
            .iter()
            .enumerate()
            .any(|(i, h)| handles[..i].iter().any(|old| old.id() == h.id()))
    {
        return;
    }
    for handle in handles {
        let mut owners = meshes.iter().filter(|(_, mesh)| mesh.id() == handle.id());
        let Some((entity, _)) = owners.next() else {
            snapshot.groups.clear();
            return;
        };
        if owners.next().is_some() {
            snapshot.groups.clear();
            return;
        }
        snapshot.groups.push(entity.into());
    }
}
fn normalize_target(
    target: &RenderTarget,
    primary: Option<Entity>,
) -> Option<NormalizedRenderTarget> {
    target.normalize(primary)
}
type Inputs = (
    &'static ExtractedView,
    &'static ExtractedCamera,
    &'static ViewTarget,
    &'static ViewDepthTexture,
    &'static Msaa,
    Option<&'static MainPassResolutionOverride>,
    &'static ViewUniformOffset,
);
fn valid_view(
    snapshot: &Snapshot,
    view: &ExtractedView,
    camera: &ExtractedCamera,
    target: &ViewTarget,
    depth: &ViewDepthTexture,
    msaa: &Msaa,
    resolution: Option<&MainPassResolutionOverride>,
) -> bool {
    let Some(entity) = snapshot.camera else {
        return false;
    };
    let Some(size) = camera.physical_target_size else {
        return false;
    };
    let extent = Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: 1,
    };
    size.x > 0
        && size.y > 0
        && view.retained_view_entity == RetainedViewEntity::new(entity.into(), None, 0)
        && camera.target == snapshot.target
        && camera.schedule == Core3d.intern()
        && !camera.hdr
        && matches!(
            camera.target,
            Some(NormalizedRenderTarget::Window(_) | NormalizedRenderTarget::Image(_))
        )
        && camera.viewport.is_none()
        && camera.physical_viewport_size == Some(size)
        && resolution.is_none()
        && view.viewport == UVec4::new(0, 0, size.x, size.y)
        && msaa.samples() == 4
        && target.main_texture_format() == TextureFormat::Rgba8UnormSrgb
        && target.main_texture().sample_count() == 1
        && target.main_texture().size() == extent
        && target.sampled_main_texture().is_some_and(|t| {
            t.sample_count() == 4
                && t.format() == TextureFormat::Rgba8UnormSrgb
                && t.size() == extent
        })
        && depth.texture.format() == TextureFormat::Depth32Float
        && depth.texture.sample_count() == 4
        && depth.texture.size() == extent
}
fn source_pipeline(cache: &PipelineCache, item: &Transparent3d) -> bool {
    if cache.get_render_pipeline(item.pipeline).is_none() {
        return false;
    }
    let d = cache.get_render_pipeline_descriptor(item.pipeline);
    d.multisample.count == 4
        && !d.multisample.alpha_to_coverage_enabled
        && d.depth_stencil.as_ref().is_some_and(|d| {
            d.format == TextureFormat::Depth32Float && d.depth_write_enabled == Some(false)
        })
        && d.fragment.as_ref().is_some_and(|f| {
            f.targets.len() == 1
                && f.targets[0]
                    .as_ref()
                    .is_some_and(|t| t.format == TextureFormat::Rgba8UnormSrgb)
        })
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn barrier(
    snapshot: Res<Snapshot>,
    state: Res<State>,
    ids: Res<Ids>,
    cache: Res<PipelineCache>,
    views: Query<Inputs>,
    opaque: Res<ViewBinnedRenderPhases<Opaque3d>>,
    alpha: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut route: ResMut<Routing>,
) {
    state.lock().calls += 1;
    if state.failed() {
        return;
    }
    if !route.clean() {
        state.fail("live barrier found active transaction");
        return;
    }
    if snapshot.groups.len() != 5 {
        state.fallback(0);
        return;
    }
    let mut matched = views.iter().filter(|(v, ..)| {
        Some(v.retained_view_entity.main_entity.id()) == snapshot.camera
            && v.retained_view_entity.subview_index == 0
    });
    let Some((view, camera, target, depth, msaa, resolution, _)) = matched.next() else {
        state.fallback(1);
        return;
    };
    if matched.next().is_some()
        || !valid_view(&snapshot, view, camera, target, depth, msaa, resolution)
        || !opaque.contains_key(&view.retained_view_entity)
        || !alpha.contains_key(&view.retained_view_entity)
    {
        state.fallback(1);
        return;
    }
    let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
        state.fallback(2);
        return;
    };
    if phase.items.len() > 50_000 {
        state.fallback(2);
        return;
    }
    let selected: Vec<_> = phase
        .items
        .values()
        .filter(|i| snapshot.groups.contains(&i.main_entity()))
        .collect();
    if selected.len() != 5
        || snapshot.groups.iter().any(|group| {
            selected
                .iter()
                .filter(|item| item.main_entity() == *group)
                .count()
                != 1
        })
    {
        state.fallback(2);
        return;
    }
    if selected.iter().any(|i| {
        i.draw_function != ids.0.standard || !i.distance.is_finite() || !source_pipeline(&cache, i)
    }) {
        state.fallback(3);
        return;
    }
    route.view = Some(view.retained_view_entity);
    route.clock = Some(stamp(&snapshot));
    route.size = camera.physical_target_size;
    if core::barrier(phase, &snapshot.groups, ids.0, &mut route.frame).is_err() {
        state.fail("live validated barrier unexpectedly failed");
    }
}
fn cancel_before_mask(
    phases: &mut ViewSortedRenderPhases<Transparent3d>,
    route: &mut Routing,
    proof: &core::Proof,
    state: &State,
) {
    if route.prepared {
        state.fail("live attempted ordinary fallback after masking");
        return;
    }
    if let Some(view) = route.view
        && let Some(phase) = phases.get_mut(&view)
        && core::restore(phase, &mut route.frame, proof).is_err()
    {
        state.fail("live pre-mask draw ID restoration failed");
    }
    // A missing phase has no retained ordinary items left to restore and is not armed.
    *route = Routing::default();
}
// Bevy0.19.1 DrawMesh's exact public-field preprocessing readiness predicate.
fn native_gpu_preprocess_ready(
    pipelines: Option<&PreprocessPipelines>,
    cache: &PipelineCache,
    support: &GpuPreprocessingSupport,
    has_preprocess_bind_groups: bool,
) -> bool {
    let Some(p) = pipelines else {
        return true;
    };
    if !has_preprocess_bind_groups {
        return false;
    }
    let loaded = |id: Option<CachedComputePipelineId>| {
        id.is_some_and(|id| cache.get_compute_pipeline(id).is_some())
    };
    let basic = [
        p.direct_preprocess.pipeline_id,
        p.gpu_frustum_culling_preprocess.pipeline_id,
    ];
    match support.max_supported_mode {
        GpuPreprocessingMode::None => false,
        GpuPreprocessingMode::PreprocessingOnly => basic.into_iter().all(loaded),
        GpuPreprocessingMode::Culling => {
            basic.into_iter().all(loaded)
                && [
                    p.early_gpu_occlusion_culling_preprocess.pipeline_id,
                    p.late_gpu_occlusion_culling_preprocess.pipeline_id,
                    p.gpu_frustum_culling_build_indexed_indirect_params
                        .pipeline_id,
                    p.gpu_frustum_culling_build_non_indexed_indirect_params
                        .pipeline_id,
                    p.early_phase.reset_indirect_batch_sets.pipeline_id,
                    p.early_phase
                        .gpu_occlusion_culling_build_indexed_indirect_params
                        .pipeline_id,
                    p.early_phase
                        .gpu_occlusion_culling_build_non_indexed_indirect_params
                        .pipeline_id,
                    p.late_phase.reset_indirect_batch_sets.pipeline_id,
                    p.late_phase
                        .gpu_occlusion_culling_build_indexed_indirect_params
                        .pipeline_id,
                    p.late_phase
                        .gpu_occlusion_culling_build_non_indexed_indirect_params
                        .pipeline_id,
                    p.main_phase.reset_indirect_batch_sets.pipeline_id,
                    p.main_phase
                        .gpu_occlusion_culling_build_indexed_indirect_params
                        .pipeline_id,
                    p.main_phase
                        .gpu_occlusion_culling_build_non_indexed_indirect_params
                        .pipeline_id,
                ]
                .into_iter()
                .all(loaded)
        }
    }
}

#[derive(SystemParam)]
struct NativeReadiness<'w, 's> {
    pipelines: Option<Res<'w, PreprocessPipelines>>,
    support: Option<Res<'w, GpuPreprocessingSupport>>,
    views: Query<
        'w,
        's,
        (
            &'static ExtractedView,
            Has<MeshViewBindGroup>,
            Has<PreprocessBindGroups>,
        ),
    >,
    meshes: Option<Res<'w, RenderAssets<RenderMesh>>>,
    instances: Option<Res<'w, RenderMeshInstances>>,
    allocator: Option<Res<'w, MeshAllocator>>,
    materials: Option<Res<'w, ErasedRenderAssets<PreparedMaterial>>>,
    material_instances: Option<Res<'w, RenderMaterialInstances>>,
    material_allocators: Option<Res<'w, MaterialBindGroupAllocators>>,
}
impl NativeReadiness<'_, '_> {
    fn check(
        &self,
        cache: &PipelineCache,
        retained: RetainedViewEntity,
        phase: &SortedRenderPhase<Transparent3d>,
        groups: &[MainEntity],
    ) -> Result<(), &'static str> {
        let mut matched = self
            .views
            .iter()
            .filter(|(v, ..)| v.retained_view_entity == retained);
        let Some((_, has_view_bind_group, has_preprocess_bind_groups)) = matched.next() else {
            return Err("native_view_absent");
        };
        if matched.next().is_some() {
            return Err("native_view_ambiguous");
        }
        if !has_view_bind_group {
            return Err("mesh_view_bind_group_absent");
        }
        let Some(support) = self.support.as_deref() else {
            return Err("gpu_preprocess_support_absent");
        };
        if self.pipelines.is_some() && !has_preprocess_bind_groups {
            return Err("preprocess_view_bind_groups_absent");
        }
        if !native_gpu_preprocess_ready(
            self.pipelines.as_deref(),
            cache,
            support,
            has_preprocess_bind_groups,
        ) {
            return Err("gpu_preprocess_pipelines_not_loaded");
        }
        let (
            Some(meshes),
            Some(instances),
            Some(allocator),
            Some(materials),
            Some(material_instances),
            Some(material_allocators),
        ) = (
            self.meshes.as_deref(),
            self.instances.as_deref(),
            self.allocator.as_deref(),
            self.materials.as_deref(),
            self.material_instances.as_deref(),
            self.material_allocators.as_deref(),
        )
        else {
            return Err("native_mesh_material_resources_absent");
        };
        // Only straightforward public gates for the five owned source meshes.
        // Skin/morph/lightmap/indirect internals are not rewritten or guessed here.
        for item in phase
            .items
            .values()
            .filter(|i| groups.contains(&i.main_entity()))
        {
            let Some(mesh_id) = instances.mesh_asset_id(item.main_entity()) else {
                return Err("native_mesh_instance_absent");
            };
            let Some(mesh) = meshes.get(mesh_id) else {
                return Err("native_gpu_mesh_absent");
            };
            if allocator.mesh_vertex_slice(&mesh_id).is_none() {
                return Err("native_vertex_allocation_absent");
            }
            if matches!(&mesh.buffer_info, RenderMeshBufferInfo::Indexed { .. })
                && allocator.mesh_index_slice(&mesh_id).is_none()
            {
                return Err("native_index_allocation_absent");
            }
            let Some(instance) = material_instances.instances.get(&item.main_entity()) else {
                return Err("native_material_instance_absent");
            };
            let Some(allocator) = material_allocators.get(&instance.asset_id.type_id()) else {
                return Err("native_material_allocator_absent");
            };
            let Some(material) = materials.get(instance.asset_id) else {
                return Err("native_prepared_material_absent");
            };
            let Some(slab) = allocator.get(material.binding.group) else {
                return Err("native_material_slab_absent");
            };
            if slab.bind_group().is_none() {
                return Err("native_material_bind_group_absent");
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn prepare(
    snapshot: Res<Snapshot>,
    state: Res<State>,
    ids: Res<Ids>,
    cache: Res<PipelineCache>,
    native: NativeReadiness,
    views: Query<Inputs>,
    opaque: Res<ViewBinnedRenderPhases<Opaque3d>>,
    alpha: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
    proof: Res<core::Proof>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut route: ResMut<Routing>,
) {
    if state.failed() {
        cancel_before_mask(&mut phases, &mut route, &proof, &state);
        return;
    }
    let Some(retained) = route.view else {
        return;
    };
    if route.clock != Some(stamp(&snapshot)) {
        state.fail("live prepare frame/camera clock mismatch");
        cancel_before_mask(&mut phases, &mut route, &proof, &state);
        return;
    }
    let found = views
        .iter()
        .find(|(v, ..)| v.retained_view_entity == retained);
    let supported = found.is_some_and(|(v, c, t, d, m, r, _)| {
        valid_view(&snapshot, v, c, t, d, m, r)
            && opaque.contains_key(&retained)
            && alpha.contains_key(&retained)
    });
    if !supported {
        state.fallback(4);
        cancel_before_mask(&mut phases, &mut route, &proof, &state);
        return;
    }
    let Some(phase) = phases.get_mut(&retained) else {
        state.fallback(4);
        cancel_before_mask(&mut phases, &mut route, &proof, &state);
        return;
    };
    let mut ready = true;
    let rows: Vec<_> = phase
        .items
        .values()
        .map(|i| {
            let selected = snapshot.groups.contains(&i.main_entity());
            if selected && (i.draw_function != ids.0.marked || !source_pipeline(&cache, i)) {
                ready = false;
            }
            core::Entry {
                selected,
                batch_len: i.batch_range.len(),
            }
        })
        .collect();
    // No emission/unprepared or unsupported batch remains ordinary, never arms Proof.
    if !ready || core::plan_batches(&rows).is_err() {
        state.fallback(5);
        cancel_before_mask(&mut phases, &mut route, &proof, &state);
        return;
    }
    if let Err(reason) = native.check(&cache, retained, phase, &snapshot.groups) {
        state.native_deferral(reason, &snapshot);
        state.fallback(5);
        cancel_before_mask(&mut phases, &mut route, &proof, &state);
        return;
    }
    match core::prepare_and_mask(phase, &mut route.frame, retained, &proof) {
        Ok(()) => route.prepared = true,
        Err(_) => {
            state.fail("live prevalidated prepare unexpectedly failed");
            *route = Routing::default();
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn replay(
    world: &World,
    view: ViewQuery<Inputs>,
    mut context: RenderContext,
    snapshot: Res<Snapshot>,
    state: Res<State>,
    route: Res<Routing>,
    proof: Res<core::Proof>,
    opaque: Res<ViewBinnedRenderPhases<Opaque3d>>,
    alpha: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
) {
    if !route.prepared {
        return;
    }
    let entity = view.entity();
    let (v, c, t, d, m, r, _) = view.into_inner();
    if Some(v.retained_view_entity) != route.view {
        return;
    }
    if state.failed()
        || route.clock != Some(stamp(&snapshot))
        || !valid_view(&snapshot, v, c, t, d, m, r)
        || !opaque.contains_key(&v.retained_view_entity)
        || !alpha.contains_key(&v.retained_view_entity)
    {
        state.fail("live post-mask replay scope/view unsupported");
        return;
    }
    if core::draw_into_scene(&mut context, world, entity, t, d, &route.frame, &proof).is_err() {
        let (heads, groups) = proof.successful_counts();
        println!(
            "smoke_boundary_live event=replay_failure frame={} segment={} tick={} terminal_heads={heads} logical_groups={groups} masked=true fatal=true",
            snapshot.frame, snapshot.segment, snapshot.tick
        );
        state.fail("live smoke terminal replay failed/skipped/incomplete");
    }
}
fn finalize(
    snapshot: Res<Snapshot>,
    state: Res<State>,
    ids: Res<Ids>,
    proof: Res<core::Proof>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut route: ResMut<Routing>,
) {
    let Some(view) = route.view else {
        return;
    };
    if !route.prepared {
        state.fallback(6);
        cancel_before_mask(&mut phases, &mut route, &proof, &state);
        return;
    }
    let clock_ok = route.clock == Some(stamp(&snapshot));
    let mut success = false;
    if let Some(phase) = phases.get_mut(&view) {
        let ranges: Vec<_> = route
            .frame
            .owned
            .items
            .iter()
            .map(|(k, i)| (*k, i.batch_range.clone()))
            .collect();
        if core::restore(phase, &mut route.frame, &proof).is_err() {
            state.fail("live masked frame missing replay/restoration");
        }
        let restored = ranges.len() == 5
            && ranges.iter().all(|(key, range)| {
                phase
                    .items
                    .get(key)
                    .is_some_and(|i| i.batch_range == *range && i.draw_function == ids.0.standard)
            });
        if !restored || !proof.current_frame_ready() || !clock_ok {
            state.fail("live terminal/ranges/drawIDs/clock restoration proof failed");
        }
        success = restored && proof.current_frame_ready() && clock_ok && !state.failed();
    } else {
        route.frame.discard_missing_phase(&proof);
        state.fail("live masked retained phase disappeared");
    }
    if !route.frame.clean() {
        state.fail("live finalizer left owned/saved transaction");
    }
    let size = route.size;
    *route = Routing::default();
    if success {
        let mut stats = state.lock();
        stats.replayed += 1;
        if !stats.first_reported {
            println!(
                "smoke_boundary_live event=first_replayed frame={} segment={} tick={} groups=5 restored=true terminal=true positive_fragments=unknown screenshot_ledger=false size={size:?} target={:?} samples=4 format=Rgba8UnormSrgb depth=Depth32Float",
                snapshot.frame, snapshot.segment, snapshot.tick, snapshot.target
            );
            stats.first_reported = true;
        }
    }
}
fn publish(
    state: Res<State>,
    mut exit: MessageWriter<AppExit>,
    mut reported: Local<bool>,
    mut summary: Local<u8>,
) {
    let stats = state.lock();
    if !*reported && let Some(reason) = stats.error {
        error!("smoke_boundary_live failed={reason} ordinary_state_restoration_attempted=true");
        *reported = true;
        exit.write(AppExit::error());
    }
    let threshold = match *summary {
        0 => 240,
        1 => 360,
        _ => u64::MAX,
    };
    if stats.calls >= threshold {
        println!(
            "smoke_boundary_live event=summary calls={} replayed={} fallback_counts={:?} screenshot_ledger=false expected_shots=none error={:?}",
            stats.calls, stats.replayed, stats.fallback, stats.error
        );
        *summary += 1;
    }
    // One first-replay and two summary lines; no per-frame sidecar/fake90 completion.
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::core_pipeline::core_3d::TransparentSortingInfo3d;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::render::{
        render_phase::{
            Draw, DrawError, DrawFunctions, PhaseItemExtraIndex, SortedRenderPhase,
            TrackedRenderPass,
        },
        render_resource::CachedRenderPipelineId,
    };
    struct Noop;
    impl Draw<Transparent3d> for Noop {
        fn draw<'w>(
            &mut self,
            _: &'w World,
            _: &mut TrackedRenderPass<'w>,
            _: Entity,
            _: &Transparent3d,
        ) -> Result<(), DrawError> {
            Ok(())
        }
    }
    #[test]
    fn actual_installer_shares_state_before_first_extraction_without_gpu() {
        let mut app = App::new();
        let mut render = bevy::app::SubApp::new();
        render.init_resource::<DrawFunctions<Transparent3d>>();
        // Registry prerequisite only: no DrawMaterial or GPU command executes.
        render
            .world()
            .resource::<DrawFunctions<Transparent3d>>()
            .write()
            .add_with::<bevy::pbr::DrawMaterial, _>(Noop);
        app.insert_sub_app(RenderApp, render);
        install(&mut app).unwrap();
        let main_state = app.world().resource::<State>().clone();
        let render = app.sub_app_mut(RenderApp);
        // Actual installer + required Res fetch BEFORE any ExtractSchedule runs.
        // The previous deferred-only resource installer fails at this fetch.
        let mut required_state =
            bevy::ecs::system::SystemState::<Res<State>>::new(render.world_mut());
        let render_state = required_state.get(render.world()).unwrap();
        assert!(Arc::ptr_eq(&main_state.0, &render_state.0));
        render_state.fail("fixture-only shared error propagation");
        assert!(main_state.failed());
        assert!(app.world().resource::<State>().failed());
        assert!(
            app.sub_app(RenderApp)
                .world()
                .contains_resource::<Snapshot>()
        );
        assert!(
            app.sub_app(RenderApp)
                .world()
                .contains_resource::<Routing>()
        );
        assert!(
            app.sub_app(RenderApp)
                .world()
                .contains_resource::<core::Proof>()
        );
        assert!(install(&mut app).is_err());
        // No full extract/DrawMaterial/renderer/GPU acceptance follows.
    }
    struct Original;
    struct Marked;
    fn fixture() -> (
        World,
        ViewSortedRenderPhases<Transparent3d>,
        Routing,
        core::Ids,
        Snapshot,
    ) {
        let mut world = World::new();
        let camera = world.spawn_empty().id();
        let other: MainEntity = world.spawn_empty().id().into();
        let groups: Vec<MainEntity> = (0..5).map(|_| world.spawn_empty().id().into()).collect();
        let registry = DrawFunctions::<Transparent3d>::default();
        let ids = {
            let mut draws = registry.write();
            let standard = draws.add_with::<Original, _>(Noop);
            let marked = draws.add_with::<Marked, _>(Noop);
            core::Ids { standard, marked }
        };
        let view = RetainedViewEntity::new(camera.into(), None, 0);
        let mut phase = SortedRenderPhase::default();
        for (index, main) in [groups[0], other, groups[1], groups[2], groups[3], groups[4]]
            .into_iter()
            .enumerate()
        {
            phase.add_retained(Transparent3d {
                sorting_info: TransparentSortingInfo3d::Sorted {
                    mesh_center: Vec3::ZERO,
                    depth_bias: 0.,
                },
                distance: index as f32,
                pipeline: CachedRenderPipelineId::new(0),
                entity: (Entity::PLACEHOLDER, main),
                draw_function: ids.standard,
                batch_range: 100 + index as u32..101 + index as u32,
                extra_index: PhaseItemExtraIndex::None,
                indexed: true,
            });
        }
        let snapshot = Snapshot {
            frame: 17,
            segment: 2,
            tick: 0,
            camera: Some(camera),
            groups: groups.clone(),
            ..default()
        };
        let mut route = Routing {
            view: Some(view),
            clock: Some(stamp(&snapshot)),
            ..default()
        };
        core::barrier(&mut phase, &groups, ids, &mut route.frame).unwrap();
        let mut phases = ViewSortedRenderPhases::<Transparent3d>::default();
        phases.insert(view, phase);
        (world, phases, route, ids, snapshot)
    }
    #[test]
    fn primary_window_actual_component_query_normalizes_and_missing_primary_refuses() {
        let mut world = World::new();
        let primary = world.spawn(PrimaryWindow).id();
        let mut q = world.query_filtered::<Entity, With<PrimaryWindow>>();
        let target = RenderTarget::Window(bevy::window::WindowRef::Primary);
        assert!(normalize_target(&target, None).is_none());
        let normalized = normalize_target(&target, q.single(&world).ok()).unwrap();
        assert!(matches!(normalized, NormalizedRenderTarget::Window(_)));
        assert_eq!(
            normalize_target(
                &RenderTarget::Window(bevy::window::WindowRef::Entity(primary)),
                None
            ),
            Some(normalized)
        );
        // Component/query/API proof only, NOT actual OS minimize/render acceptance.
    }
    #[test]
    fn empty_prepared_groups_cancel_before_mask_restores_real_retained_draw_ids() {
        let (_world, mut phases, mut route, ids, snapshot) = fixture();
        let view = route.view.unwrap();
        let proof = core::Proof::default();
        let state = State::default();
        for item in phases.get_mut(&view).unwrap().items.values_mut() {
            if snapshot.groups.contains(&item.main_entity()) {
                item.batch_range = 0..0;
            }
        }
        let rows: Vec<_> = phases[&view]
            .items
            .values()
            .map(|i| core::Entry {
                selected: snapshot.groups.contains(&i.main_entity()),
                batch_len: i.batch_range.len(),
            })
            .collect();
        assert!(core::plan_batches(&rows).is_err());
        cancel_before_mask(&mut phases, &mut route, &proof, &state);
        assert!(route.clean());
        assert!(!state.failed());
        assert!(!proof.failed());
        assert_eq!(state.lock().replayed, 0);
        assert!(
            phases[&view]
                .items
                .values()
                .all(|i| i.draw_function == ids.standard)
        );
        assert!(
            phases[&view]
                .items
                .values()
                .filter(|i| snapshot.groups.contains(&i.main_entity()))
                .all(|i| i.batch_range.is_empty())
        );
        assert_eq!(
            phases[&view]
                .items
                .values()
                .find(|i| !snapshot.groups.contains(&i.main_entity()))
                .unwrap()
                .batch_range,
            101..102
        );
    }
    #[test]
    fn late_replay_absence_actual_finalizer_restores_all_ranges_and_reports_failure() {
        let (mut world, mut phases, mut route, ids, snapshot) = fixture();
        let view = route.view.unwrap();
        let proof = core::Proof::default();
        let original: Vec<_> = phases[&view]
            .items
            .iter()
            .map(|(k, i)| (*k, i.batch_range.clone()))
            .collect();
        core::prepare_and_mask(
            phases.get_mut(&view).unwrap(),
            &mut route.frame,
            view,
            &proof,
        )
        .unwrap();
        route.prepared = true;
        world.insert_resource(phases);
        world.insert_resource(route);
        world.insert_resource(Ids(ids));
        world.insert_resource(snapshot);
        world.insert_resource(proof);
        world.init_resource::<State>();
        world.run_system_once(finalize).unwrap();
        assert!(world.resource::<State>().failed());
        assert!(world.resource::<core::Proof>().failed());
        assert_eq!(world.resource::<State>().lock().replayed, 0);
        assert!(world.resource::<Routing>().clean());
        let phases = world.resource::<ViewSortedRenderPhases<Transparent3d>>();
        assert!(original.iter().all(
            |(key, range)| phases[&view].items[key].batch_range == *range
                && phases[&view].items[key].draw_function == ids.standard
        ));
        // Missing native draw/terminal cannot become successful live acknowledgement.
    }
    #[test]
    fn no_emission_publisher_has_no_screenshot_completion_dependency() {
        let mut app = App::new();
        app.add_message::<AppExit>()
            .init_resource::<State>()
            .add_systems(Update, publish);
        app.update();
        assert!(!app.world().resource::<State>().failed());
        assert_eq!(app.world().resource::<State>().lock().replayed, 0);
        assert_eq!(app.world().resource::<Messages<AppExit>>().len(), 0);
        // This executes actual publisher, not a fake90-ledger state machine.
    }
}
