//! Default-off authored ground pass before ALL ordinary transparent draws.
//! U/S order, phase keys/ranges, materials and raster state remain native.
//! Singleton CPU command success is not positive-fragment or source parity proof.
use super::ground_skid_pass_core::{self as core, Gate};
use bevy::{
    asset::AssetId,
    camera::{MainPassResolutionOverride, NormalizedRenderTarget, RenderTarget},
    core_pipeline::{
        core_3d::{
            AlphaMask3d, Opaque3d, Transparent3d, main_opaque_pass_3d, main_transparent_pass_3d,
        },
        oit::OrderIndependentTransparencySettings,
        schedule::{Core3d, Core3dSystems},
    },
    diagnostic::FrameCount,
    ecs::{
        query::ROQueryItem,
        schedule::ScheduleLabel,
        system::{StaticSystemParam, SystemParamItem, lifetimeless::SRes},
    },
    pbr::{DrawMaterial, MeshPipeline, RenderMaterialInstances, RenderMeshInstances},
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        batching::{GetBatchData, GetFullBatchData, NoAutomaticBatching},
        camera::ExtractedCamera,
        mesh::{RenderMesh, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_phase::{
            AddRenderCommand, DrawFunctionId, DrawFunctions, RenderCommand, RenderCommandResult,
            TrackedRenderPass, ViewBinnedRenderPhases, ViewSortedRenderPhases,
        },
        render_resource::{
            LoadOp, Operations, PipelineCache, RenderPassColorAttachment,
            RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp, TextureFormat,
        },
        renderer::{RenderContext, ViewQuery},
        sync_world::MainEntity,
        view::{
            ExtractedView, RetainedViewEntity, ViewDepthTexture, ViewTarget, ViewUniformOffset,
        },
    },
    window::PrimaryWindow,
};
use std::sync::{Arc, Mutex, MutexGuard};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Off,
    Control,
    BeforeTransparent,
}
impl Mode {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        match text {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "before-transparent" => Ok(Self::BeforeTransparent),
            _ => Err("--ground-skid-pass needs off, control or before-transparent"),
        }
    }
    pub fn validate(self, c: Context) -> Result<(), &'static str> {
        if self == Self::Off {
            return Ok(());
        }
        if !c.baseline
            || !c.drive
            || c.headless
            || c.collision
            || !c.aa0
            || !c.groups
            || !c.ground
            || !c.skid_off
            || !c.composition_off
            || !c.other_policy_off
            || !c.sky
        {
            return Err(
                "ground-skid-pass supports ordinary baseline AA0 driving with Groups/ground effects, sky reflection and all other phase policies off",
            );
        }
        Ok(())
    }
}
pub(crate) struct Context {
    pub baseline: bool,
    pub drive: bool,
    pub headless: bool,
    pub collision: bool,
    pub aa0: bool,
    pub groups: bool,
    pub ground: bool,
    pub skid_off: bool,
    pub composition_off: bool,
    pub other_policy_off: bool,
    pub sky: bool,
}
#[derive(Resource, Clone, Copy)]
pub(crate) struct ProducerPolicy;
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OwnedGroundSkid {
    pub entity: Entity,
    pub mesh: AssetId<Mesh>,
    pub material: AssetId<StandardMaterial>,
}
impl OwnedGroundSkid {
    pub fn new(entity: Entity, mesh: AssetId<Mesh>, material: AssetId<StandardMaterial>) -> Self {
        Self {
            entity,
            mesh,
            material,
        }
    }
}
type Key = (Entity, MainEntity);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ticket {
    frame: u32,
    view: Entity,
    key: Key,
}
#[derive(Resource, Clone, Default)]
struct Proof(Arc<Mutex<Gate<Ticket>>>);
impl Proof {
    fn lock(&self) -> MutexGuard<'_, Gate<Ticket>> {
        self.0.lock().unwrap_or_else(|e| {
            let mut g = e.into_inner();
            g.fail("ground skid gate mutex poisoned");
            g
        })
    }
    fn scope(&self, view: Entity, item: &Transparent3d) -> Option<Ticket> {
        self.lock()
            .ticket
            .filter(|t| t.view == view && t.key == item.entity)
    }
}
#[derive(Resource, Clone, Copy)]
struct Config(Mode);
#[derive(Resource, Default)]
struct Snapshot {
    frame: u32,
    camera: Option<Entity>,
    target: Option<NormalizedRenderTarget>,
    owner: Option<OwnedGroundSkid>,
    clock: Option<(u64, u64)>,
    view_name: String,
    unavailable: Reason,
}
#[derive(Resource, Clone, Copy)]
struct Ids {
    standard: DrawFunctionId,
    ordinary: DrawFunctionId,
    replay: DrawFunctionId,
}
struct Saved {
    ticket: Ticket,
    view: RetainedViewEntity,
    original: DrawFunctionId,
    item: Transparent3d,
}
#[derive(Resource, Default)]
struct Routing {
    saved: Option<Saved>,
    calls: u64,
    ready: u64,
    replayed: u64,
    fallback: u64,
    reasons: [u64; 11],
    previous_reason: Option<Reason>,
    previous_stamp: Option<(Option<u64>, String)>,
    notes: u8,
}

// Fixed small receipt-only counters. No admission/geometry/gate decisions depend on them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Reason {
    Ready,
    Camera,
    View,
    #[default]
    OwnerAbsent,
    OwnerUnverified,
    PhaseAbsent,
    NotQueued,
    Head,
    Pipeline,
    GpuInput,
    Invariant,
}
fn outcome(route: &mut Routing, snapshot: &Snapshot, reason: Reason) {
    route.reasons[reason as usize] += 1;
    if route.previous_reason != Some(reason) && reason != Reason::Ready && route.notes < 32 {
        println!(
            "ground_skid_pass event=fallback_transition frame={} calls={} clock={:?} camera_view={} reason={reason:?} positive_fragments=unknown",
            snapshot.frame, route.calls, snapshot.clock, snapshot.view_name
        );
        route.notes += 1;
    }
    route.previous_reason = Some(reason);
}
fn state_stamp(route: &mut Routing, snapshot: &Snapshot) {
    let stamp = (snapshot.clock.map(|c| c.0), snapshot.view_name.clone());
    if route.previous_stamp.as_ref() != Some(&stamp) {
        if route.notes < 32 {
            println!(
                "ground_skid_pass event=state frame={} calls={} clock={:?} camera_view={}",
                snapshot.frame, route.calls, snapshot.clock, snapshot.view_name
            );
            route.notes += 1;
        }
        route.previous_stamp = Some(stamp);
    }
}

struct ViewConditionalGate;
impl RenderCommand<Transparent3d> for ViewConditionalGate {
    type Param = SRes<Proof>;
    type ViewQuery = Entity;
    type ItemQuery = ();
    fn render<'w>(
        item: &Transparent3d,
        view: ROQueryItem<Self::ViewQuery>,
        _: Option<()>,
        proof: SystemParamItem<'w, '_, Self::Param>,
        _: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let proof = proof.into_inner();
        if proof
            .scope(view, item)
            .is_some_and(|t| proof.lock().skip_ordinary(t))
        {
            RenderCommandResult::Skip
        } else {
            RenderCommandResult::Success
        }
    }
}
struct TerminalSuccess;
impl RenderCommand<Transparent3d> for TerminalSuccess {
    type Param = SRes<Proof>;
    type ViewQuery = Entity;
    type ItemQuery = ();
    fn render<'w>(
        item: &Transparent3d,
        view: ROQueryItem<Self::ViewQuery>,
        _: Option<()>,
        proof: SystemParamItem<'w, '_, Self::Param>,
        _: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let proof = proof.into_inner();
        if let Some(ticket) = proof.scope(view, item) {
            proof.lock().terminal(ticket)
        } else {
            proof.lock().fail("terminal scope mismatch")
        }
        // Infallible after nested DrawMesh. Never lose a successful mask due to a
        // fallible marker after draw emission; fatal state preserves success.
        RenderCommandResult::Success
    }
}
type Ordinary = (ViewConditionalGate, DrawMaterial);
type Replay = (DrawMaterial, TerminalSuccess);

pub(crate) fn install(app: &mut App, mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    if mode == Mode::Off {
        return Ok(());
    }
    let proof = Proof::default();
    app.insert_resource(ProducerPolicy)
        .insert_resource(Config(mode))
        .insert_resource(proof.clone())
        .add_systems(Update, fail_process);
    let render = app
        .get_sub_app_mut(RenderApp)
        .ok_or_else(|| std::io::Error::other("ground skid renderer absent"))?;
    if render.world().contains_resource::<Ids>() {
        return Err(std::io::Error::other("ground skid already registered").into());
    }
    let standard = render
        .world()
        .resource::<DrawFunctions<Transparent3d>>()
        .read()
        .get_id::<DrawMaterial>()
        .ok_or_else(|| std::io::Error::other("ground skid standard draw absent"))?;
    // Register ONCE per app, never per frame/slot/reentry.
    render
        .add_render_command::<Transparent3d, Ordinary>()
        .add_render_command::<Transparent3d, Replay>();
    let (ordinary, replay) = {
        let d = render
            .world()
            .resource::<DrawFunctions<Transparent3d>>()
            .read();
        (
            d.get_id::<Ordinary>().unwrap(),
            d.get_id::<Replay>().unwrap(),
        )
    };
    render
        .insert_resource(Config(mode))
        .insert_resource(Ids {
            standard,
            ordinary,
            replay,
        })
        .insert_resource(proof)
        .init_resource::<Snapshot>()
        .init_resource::<Routing>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(
            Render,
            prepare
                .after(RenderSystems::PrepareBindGroups)
                .before(RenderSystems::Render),
        )
        .add_systems(
            Core3d,
            draw_early
                .after(main_opaque_pass_3d)
                .before(main_transparent_pass_3d)
                .in_set(Core3dSystems::MainPass),
        )
        .add_systems(
            Render,
            finalize
                .after(RenderSystems::Render)
                .before(RenderSystems::Cleanup),
        );
    println!(
        "ground_skid_pass mode={mode:?} scope=ordinary_main_groups source_parity=false positive_fragments=unknown noautomaticbatching=owned_skid_only"
    );
    Ok(())
}
fn copy_item(i: &Transparent3d) -> Transparent3d {
    Transparent3d {
        sorting_info: i.sorting_info,
        distance: i.distance,
        pipeline: i.pipeline,
        entity: i.entity,
        draw_function: i.draw_function,
        batch_range: i.batch_range.clone(),
        extra_index: i.extra_index.clone(),
        indexed: i.indexed,
    }
}
fn restore(phases: &mut ViewSortedRenderPhases<Transparent3d>, route: &mut Routing, proof: &Proof) {
    let Some(saved) = route.saved.take() else {
        return;
    };
    if let Some(item) = phases
        .get_mut(&saved.view)
        .and_then(|p| p.items.get_mut(&saved.ticket.key))
    {
        item.draw_function = saved.original;
    } else {
        proof
            .lock()
            .fail("ground skid retained key/view missing at restore")
    }
    let succeeded = proof.lock().success;
    if succeeded {
        route.replayed += 1
    } else {
        route.fallback += 1;
        if route.fallback == 1 && route.notes < 32 {
            println!(
                "ground_skid_pass event=fallback_transition frame={} calls={} reason=NodeOrCommandNoTerminal positive_fragments=unknown",
                saved.ticket.frame, route.calls
            );
            route.notes += 1;
        }
    }
    if route.calls <= 360 {
        println!(
            "ground_skid_pass event=transaction frame={} calls={} view={:?} key={:?} terminal_success={} restored_key_present={} phase_keys_changed=false batch_fields_changed=false positive_fragments=unknown",
            saved.ticket.frame,
            route.calls,
            saved.ticket.view,
            saved.ticket.key,
            succeeded,
            phases
                .get(&saved.view)
                .is_some_and(|p| p.items.contains_key(&saved.ticket.key))
        );
    }
    let restored = { proof.lock().restored(saved.ticket) };
    if restored.is_err() {
        proof.lock().fail("ground skid restore scope differs")
    }
    // Always remove stale success, even after missing-key failure. Any stranded
    // wrapper must fall through to ordinary DrawMaterial, never mask next frame.
    let mut g = proof.lock();
    g.ticket = None;
    g.running = false;
    g.success = false;
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn extract(
    frame: Extract<Res<FrameCount>>,
    marks: Extract<Option<Res<super::skid_marks::SkidMarks>>>,
    driving: Extract<Option<Res<super::Driving>>>,
    camera_state: Extract<Option<Res<super::camera::Cameras>>>,
    cameras: Extract<Query<(Entity, &Camera, &RenderTarget), With<super::ViewerCamera>>>,
    windows: Extract<Query<Entity, With<PrimaryWindow>>>,
    owners: Extract<
        Query<(
            &OwnedGroundSkid,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            Has<NoAutomaticBatching>,
            Option<&Visibility>,
        )>,
    >,
    materials: Extract<Res<Assets<StandardMaterial>>>,
    mut snapshot: ResMut<Snapshot>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut route: ResMut<Routing>,
    proof: Res<Proof>,
) {
    if route.saved.is_some() {
        restore(&mut phases, &mut route, &proof);
        proof.lock().fail("ground skid missed finalizer")
    }
    snapshot.frame = frame.0;
    snapshot.clock = driving.as_ref().map(|d| (d.segment, d.runtime.ticks));
    snapshot.view_name = camera_state
        .as_ref()
        .map_or("unavailable", |c| c.ident())
        .to_owned();
    snapshot.unavailable = Reason::OwnerAbsent;
    snapshot.camera = None;
    snapshot.target = None;
    snapshot.owner = None;
    if proof.lock().fatal.is_some() {
        return;
    }
    let active: Vec<_> = cameras.iter().filter(|(_, c, _)| c.is_active).collect();
    if active.len() != 1 {
        snapshot.unavailable = Reason::Camera;
        return;
    }
    let (entity, _, target) = active[0];
    let Some(target) = target.normalize(windows.single().ok()) else {
        snapshot.unavailable = Reason::View;
        return;
    };
    if !matches!(
        target,
        NormalizedRenderTarget::Window(_) | NormalizedRenderTarget::Image(_)
    ) {
        snapshot.unavailable = Reason::View;
        return;
    }
    snapshot.camera = Some(entity);
    snapshot.target = Some(target);
    let Some(expected) = marks.as_ref().and_then(|m| m.owned_ground_draw()) else {
        return;
    };
    let Ok((tag, mesh, material, no_batch, visibility)) = owners.get(expected.entity) else {
        snapshot.unavailable = Reason::OwnerUnverified;
        return;
    };
    if *tag != expected
        || mesh.0.id() != expected.mesh
        || material.0.id() != expected.material
        || !no_batch
        || visibility == Some(&Visibility::Hidden)
        || !materials
            .get(expected.material)
            .is_some_and(|m| m.alpha_mode == AlphaMode::Blend)
    {
        snapshot.unavailable = Reason::OwnerUnverified;
        return;
    }
    snapshot.owner = Some(expected);
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn valid_view(
    snapshot: &Snapshot,
    view: &ExtractedView,
    camera: &ExtractedCamera,
    target: &ViewTarget,
    depth: &ViewDepthTexture,
    msaa: &Msaa,
    resolution: Option<&MainPassResolutionOverride>,
    oit: bool,
) -> bool {
    let Some(entity) = snapshot.camera else {
        return false;
    };
    let Some(size) = camera.physical_target_size else {
        return false;
    };
    let Some(sampled) = target.sampled_main_texture() else {
        return false;
    };
    view.retained_view_entity == RetainedViewEntity::new(entity.into(), None, 0)
        && camera.target == snapshot.target
        && camera.schedule == Core3d.intern()
        && !camera.hdr
        && !oit
        && resolution.is_none()
        && camera.viewport.is_none()
        && camera.physical_viewport_size == Some(size)
        && view.viewport == UVec4::new(0, 0, size.x, size.y)
        && msaa.samples() == 4
        && target.main_texture_format() == TextureFormat::Rgba8UnormSrgb
        && target.main_texture().sample_count() == 1
        && sampled.sample_count() == 4
        && sampled.size() == target.main_texture().size()
        && depth.texture.sample_count() == 4
        && depth.texture.size() == sampled.size()
        && depth.texture.format() == TextureFormat::Depth32Float
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn prepare(
    snapshot: Res<Snapshot>,
    config: Res<Config>,
    ids: Res<Ids>,
    proof: Res<Proof>,
    mut route: ResMut<Routing>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    opaque: Res<ViewBinnedRenderPhases<Opaque3d>>,
    alpha: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
    views: Query<(
        Entity,
        &ExtractedView,
        &ExtractedCamera,
        &ViewTarget,
        &ViewDepthTexture,
        &Msaa,
        Option<&MainPassResolutionOverride>,
        Has<OrderIndependentTransparencySettings>,
        &ViewUniformOffset,
    )>,
    pipelines: Res<PipelineCache>,
    instances: Res<RenderMeshInstances>,
    materials: Res<RenderMaterialInstances>,
    meshes: Res<RenderAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    batch: StaticSystemParam<<MeshPipeline as GetBatchData>::Param>,
) {
    route.calls += 1;
    state_stamp(&mut route, &snapshot);
    if route.saved.is_some() || proof.lock().fatal.is_some() {
        outcome(&mut route, &snapshot, Reason::Invariant);
        return;
    }
    let Some(owner) = snapshot.owner else {
        outcome(&mut route, &snapshot, snapshot.unavailable);
        return;
    };
    let mut selected = views.iter().filter(|(_, v, ..)| {
        Some(v.retained_view_entity.main_entity.id()) == snapshot.camera
            && v.retained_view_entity.subview_index == 0
            && v.retained_view_entity.auxiliary_entity.id() == Entity::PLACEHOLDER
    });
    let Some((render_view, view, camera, target, depth, msaa, resolution, oit, _)) =
        selected.next()
    else {
        outcome(&mut route, &snapshot, Reason::View);
        return;
    };
    if selected.next().is_some()
        || !valid_view(
            &snapshot, view, camera, target, depth, msaa, resolution, oit,
        )
        || !opaque.contains_key(&view.retained_view_entity)
        || !alpha.contains_key(&view.retained_view_entity)
    {
        outcome(&mut route, &snapshot, Reason::View);
        return;
    }
    let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
        outcome(&mut route, &snapshot, Reason::PhaseAbsent);
        return;
    };
    let keys: Vec<_> = phase
        .items
        .keys()
        .filter(|k| k.1.id() == owner.entity)
        .copied()
        .collect();
    if keys.len() != 1 {
        outcome(&mut route, &snapshot, Reason::NotQueued);
        return;
    } // Absent/ambiguous owner is ordinary fallback, not forcevisible.
    let key = keys[0];
    let rows: Vec<_> = phase
        .items
        .iter()
        .map(|(k, i)| core::Entry {
            owned: *k == key,
            len: i.batch_range.len(),
        })
        .collect();
    let Ok(Some(index)) = core::singleton_head(&rows) else {
        outcome(&mut route, &snapshot, Reason::Head);
        return;
    };
    let item = &phase.items[index];
    if item.draw_function != ids.standard
        || !item.indexed
        || pipelines.get_render_pipeline(item.pipeline).is_none()
    {
        outcome(&mut route, &snapshot, Reason::Pipeline);
        return;
    }
    let descriptor = pipelines.get_render_pipeline_descriptor(item.pipeline);
    if descriptor.multisample.count != 4
        || descriptor.multisample.alpha_to_coverage_enabled
        || descriptor.fragment.as_ref().is_none_or(|f| {
            f.targets.len() != 1
                || f.targets[0]
                    .as_ref()
                    .is_none_or(|t| t.format != TextureFormat::Rgba8UnormSrgb)
        })
        || descriptor.depth_stencil.as_ref().is_none_or(|d| {
            d.format != TextureFormat::Depth32Float
                || d.depth_write_enabled != Some(false)
                || d.bias.constant != 50
        })
    {
        outcome(&mut route, &snapshot, Reason::Pipeline);
        return;
    }
    // Explicit first implementation GPU-building domain. CPU-building remains
    // ordinary fallback; no fake GPU-ready claim from a historical len1 range.
    if !matches!(*instances, RenderMeshInstances::GpuBuilding(_)) {
        outcome(&mut route, &snapshot, Reason::GpuInput);
        return;
    }
    let Some(queue) = instances.render_mesh_queue_data(key.1) else {
        outcome(&mut route, &snapshot, Reason::GpuInput);
        return;
    };
    if queue.mesh_asset_id() != owner.mesh
        || queue.should_batch()
        || queue.current_uniform_index.0 == u32::MAX
        || materials
            .instances
            .get(&key.1)
            .is_none_or(|m| m.asset_id != owner.material.untyped())
        || meshes.get(owner.mesh).is_none()
        || allocator.mesh_vertex_slice(&owner.mesh).is_none()
        || allocator.mesh_index_slice(&owner.mesh).is_none()
    {
        outcome(&mut route, &snapshot, Reason::GpuInput);
        return;
    }
    let params = batch.into_inner();
    let Some((_, comparison)) =
        <MeshPipeline as GetFullBatchData>::get_index_and_compare_data(&params, key.1)
    else {
        outcome(&mut route, &snapshot, Reason::GpuInput);
        return;
    };
    if comparison.is_some() {
        outcome(&mut route, &snapshot, Reason::GpuInput);
        return;
    } // Actual extracted NoAutomaticBatching must reach batching input.
    route.ready += 1;
    outcome(&mut route, &snapshot, Reason::Ready);
    if route.ready == 1 {
        println!(
            "ground_skid_pass event=first_ready mode={:?} frame={} calls={} clock={:?} camera_view={} retained={:?} render_view={render_view:?} key={key:?} owner={owner:?} range={:?} extra={:?} pipeline={:?} building=gpu input_index={} noautomaticbatching={} pipeline_samples={} main_samples={} msaa_samples={} depth_samples={} colour_format={:?} depth_format={:?} positive_fragments=unknown screenshot_join=false",
            config.0,
            snapshot.frame,
            route.calls,
            snapshot.clock,
            snapshot.view_name,
            view.retained_view_entity,
            item.batch_range,
            item.extra_index,
            item.pipeline,
            queue.current_uniform_index.0,
            !queue.should_batch(),
            descriptor.multisample.count,
            target.main_texture().sample_count(),
            target.sampled_main_texture().unwrap().sample_count(),
            depth.texture.sample_count(),
            target.main_texture_format(),
            depth.texture.format()
        );
    }
    if config.0 == Mode::Control {
        return;
    }
    let ticket = Ticket {
        frame: snapshot.frame,
        view: render_view,
        key,
    };
    if proof.lock().arm(ticket).is_err() {
        return;
    }
    route.saved = Some(Saved {
        ticket,
        view: view.retained_view_entity,
        original: item.draw_function,
        item: copy_item(item),
    });
    phase.items[index].draw_function = ids.ordinary; // Only retained mutation; AFTER all batching.
}
type Inputs = (
    &'static ExtractedView,
    &'static ExtractedCamera,
    &'static ViewTarget,
    &'static ViewDepthTexture,
    &'static Msaa,
    Option<&'static MainPassResolutionOverride>,
    Has<OrderIndependentTransparencySettings>,
    &'static ViewUniformOffset,
);
#[allow(clippy::too_many_arguments)]
fn draw_early(
    world: &World,
    view: ViewQuery<Inputs>,
    mut context: RenderContext,
    snapshot: Res<Snapshot>,
    route: Res<Routing>,
    proof: Res<Proof>,
    ids: Res<Ids>,
    opaque: Res<ViewBinnedRenderPhases<Opaque3d>>,
    alpha: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
) {
    let render_view = view.entity();
    let (extracted, camera, target, depth, msaa, resolution, oit, _) = view.into_inner();
    let Some(saved) = route.saved.as_ref() else {
        return;
    };
    if saved.ticket.frame != snapshot.frame
        || saved.ticket.view != render_view
        || saved.view != extracted.retained_view_entity
        || !valid_view(
            &snapshot, extracted, camera, target, depth, msaa, resolution, oit,
        )
        || !opaque.contains_key(&saved.view)
        || !alpha.contains_key(&saved.view)
    {
        return;
    }
    // Exact opaque query (including ViewUniformOffset) and phase existence above
    // match native opaque node's unconditional pass admission. This system is
    // scheduled AFTER that node and BEFORE main transparency for the SAME view.
    let Some(sampled) = target.sampled_main_texture_view() else {
        return;
    };
    if !proof.lock().begin(saved.ticket) {
        return;
    }
    let colors = [Some(RenderPassColorAttachment {
        view: sampled,
        depth_slice: None,
        resolve_target: Some(target.main_texture_view()),
        ops: Operations {
            load: LoadOp::Load,
            store: StoreOp::Store,
        },
    })];
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ground skid before ordinary transparency"),
        color_attachments: &colors,
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: depth.view(),
            depth_ops: Some(Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    // Never call attachment getters for preflight: they mutate first-clear state.
    // Full native viewport is required; no override/custom viewport to emulate.
    let functions = world.resource::<DrawFunctions<Transparent3d>>();
    let mut functions = functions.write();
    if let Some(draw) = functions.get_mut(ids.replay) {
        draw.prepare(world); // Only this known stock-material wrapper, not every registry entry.
        let _result = draw.draw(world, &mut pass, render_view, &saved.item);
        // Ok can mean Skip. ONLY TerminalSuccess permits ordinary suppression.
    } else {
        proof
            .lock()
            .fail("registered ground skid replay draw disappeared")
    }
    drop(pass);
    proof.lock().end(saved.ticket);
}
fn finalize(
    snapshot: Res<Snapshot>,
    config: Res<Config>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut route: ResMut<Routing>,
    proof: Res<Proof>,
    mut reported: Local<bool>,
) {
    restore(&mut phases, &mut route, &proof);
    if route.calls == 240 || route.calls == 360 {
        println!(
            "ground_skid_pass event=summary mode={:?} frame={} calls={} clock={:?} camera_view={} reason_order=Ready,Camera,View,OwnerAbsent,OwnerUnverified,PhaseAbsent,NotQueued,Head,Pipeline,GpuInput,Invariant counts={:?} ready={} replayed={} node_or_command_no_terminal={} notes={} screenshot_join=false positive_fragments=unknown",
            config.0,
            snapshot.frame,
            route.calls,
            snapshot.clock,
            snapshot.view_name,
            route.reasons,
            route.ready,
            route.replayed,
            route.fallback,
            route.notes
        );
    }
    if !*reported && let Some(why) = proof.lock().fatal {
        error!("ground_skid_pass failed={why} disabled=true ordinary_fallback=true");
        *reported = true;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> Context {
        Context {
            baseline: true,
            drive: true,
            headless: false,
            collision: false,
            aa0: true,
            groups: true,
            ground: true,
            skid_off: true,
            composition_off: true,
            other_policy_off: true,
            sky: true,
        }
    }
    #[test]
    fn off_needs_no_renderer() {
        let mut app = App::new();
        install(&mut app, Mode::Off).unwrap();
        assert!(app.get_sub_app(RenderApp).is_none());
        assert!(!app.world().contains_resource::<ProducerPolicy>());
    }
    #[test]
    fn explicit_mode_parser() {
        assert_eq!(Mode::parse("off"), Ok(Mode::Off));
        assert_eq!(
            Mode::parse("before-transparent"),
            Ok(Mode::BeforeTransparent)
        );
        assert!(Mode::parse("guess").is_err());
    }
    #[test]
    fn live_and_offscreen_share_guard_without_route_or_car() {
        assert!(Mode::BeforeTransparent.validate(context()).is_ok());
    }
    #[test]
    fn hdr_dlaa_enhanced_configuration_refused() {
        let mut c = context();
        c.baseline = false;
        assert!(Mode::BeforeTransparent.validate(c).is_err());
        let mut c = context();
        c.aa0 = false;
        assert!(Mode::Control.validate(c).is_err());
    }
    #[test]
    fn incompatible_phase_policies_refused() {
        let mut c = context();
        c.composition_off = false;
        assert!(Mode::Control.validate(c).is_err());
        let mut c = context();
        c.skid_off = false;
        assert!(Mode::BeforeTransparent.validate(c).is_err());
    }
}

#[cfg(test)]
mod retained_tests {
    use super::*;
    use bevy::{
        core_pipeline::core_3d::TransparentSortingInfo3d,
        render::{
            render_phase::{Draw, DrawError, PhaseItemExtraIndex, SortedRenderPhase},
            render_resource::CachedRenderPipelineId,
        },
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
    struct Standard;
    struct Wrapper;
    fn fixture() -> (
        ViewSortedRenderPhases<Transparent3d>,
        Routing,
        Proof,
        Key,
        DrawFunctionId,
    ) {
        let mut w = World::new();
        let camera = w.spawn_empty().id();
        let owner = w.spawn_empty().id();
        let foreign = w.spawn_empty().id();
        let ids = DrawFunctions::<Transparent3d>::default();
        let standard = ids.write().add_with::<Standard, _>(Noop);
        let wrapper = ids.write().add_with::<Wrapper, _>(Noop);
        let view = RetainedViewEntity::new(camera.into(), None, 0);
        let key = (Entity::PLACEHOLDER, owner.into());
        let mut phase = SortedRenderPhase::default();
        for (i, main) in [foreign, owner].into_iter().enumerate() {
            phase.add_retained(Transparent3d {
                sorting_info: TransparentSortingInfo3d::Sorted {
                    mesh_center: Vec3::new(1., 2., 3.),
                    depth_bias: 50.,
                },
                distance: f32::from_bits(0x80000000),
                pipeline: CachedRenderPipelineId::new(0),
                entity: (Entity::PLACEHOLDER, main.into()),
                draw_function: standard,
                batch_range: (30 + i as u32)..(31 + i as u32),
                extra_index: PhaseItemExtraIndex::None,
                indexed: true,
            });
        }
        let ticket = Ticket {
            frame: 17,
            view: camera,
            key,
        };
        let item = copy_item(&phase.items[&key]);
        phase.items.get_mut(&key).unwrap().draw_function = wrapper;
        let mut phases = ViewSortedRenderPhases::default();
        phases.insert(view, phase);
        let proof = Proof::default();
        proof.lock().arm(ticket).unwrap();
        let routing = Routing {
            saved: Some(Saved {
                ticket,
                view,
                original: standard,
                item,
            }),
            ..default()
        };
        (phases, routing, proof, key, standard)
    }
    #[test]
    fn real_phase_restore_preserves_keys_ranges_pipeline_and_foreign() {
        let (mut phases, mut route, proof, key, standard) = fixture();
        let view = route.saved.as_ref().unwrap().view;
        let before: Vec<_> = phases[&view]
            .items
            .iter()
            .map(|(k, i)| {
                (
                    *k,
                    i.batch_range.clone(),
                    i.distance.to_bits(),
                    i.pipeline,
                    i.indexed,
                )
            })
            .collect();
        let foreign = phases[&view].items[0].draw_function;
        restore(&mut phases, &mut route, &proof);
        let after: Vec<_> = phases[&view]
            .items
            .iter()
            .map(|(k, i)| {
                (
                    *k,
                    i.batch_range.clone(),
                    i.distance.to_bits(),
                    i.pipeline,
                    i.indexed,
                )
            })
            .collect();
        assert_eq!(before, after);
        assert_eq!(phases[&view].items[0].draw_function, foreign);
        assert_eq!(phases[&view].items[&key].draw_function, standard);
        assert!(route.saved.is_none());
        assert_eq!(route.fallback, 1);
        assert!(proof.lock().fatal.is_none());
    }
    #[test]
    fn real_phase_success_marker_restore_does_not_mutate_ranges() {
        let (mut phases, mut route, proof, key, standard) = fixture();
        let saved = route.saved.as_ref().unwrap();
        let view = saved.view;
        let ticket = saved.ticket;
        assert!(proof.lock().begin(ticket));
        proof.lock().terminal(ticket);
        proof.lock().end(ticket);
        assert!(proof.lock().skip_ordinary(ticket));
        let range = phases[&view].items[&key].batch_range.clone();
        restore(&mut phases, &mut route, &proof);
        assert_eq!(route.replayed, 1);
        assert_eq!(phases[&view].items[&key].batch_range, range);
        assert_eq!(phases[&view].items[&key].draw_function, standard);
        assert!(!proof.lock().skip_ordinary(ticket));
    }
    #[test]
    fn missing_retained_view_is_failure_not_success_or_stale_mask() {
        let (mut phases, mut route, proof, _, _) = fixture();
        let ticket = route.saved.as_ref().unwrap().ticket;
        phases.clear();
        restore(&mut phases, &mut route, &proof);
        assert!(proof.lock().fatal.is_some());
        assert!(proof.lock().ticket.is_none());
        assert!(!proof.lock().skip_ordinary(ticket));
    }
    #[test]
    fn node_skip_fallback_restores_exact_id_without_replay() {
        let (mut phases, mut route, proof, key, standard) = fixture();
        let view = route.saved.as_ref().unwrap().view;
        let ticket = route.saved.as_ref().unwrap().ticket;
        assert!(proof.lock().begin(ticket));
        proof.lock().end(ticket);
        assert!(!proof.lock().skip_ordinary(ticket));
        restore(&mut phases, &mut route, &proof);
        assert_eq!(phases[&view].items[&key].draw_function, standard);
        assert_eq!(route.fallback, 1);
        assert_eq!(route.replayed, 0);
    }
}

fn fail_process(proof: Res<Proof>, mut exit: MessageWriter<AppExit>, mut reported: Local<bool>) {
    if !*reported && let Some(why) = proof.lock().fatal {
        error!("ground_skid_pass invariant_failure={why}");
        *reported = true;
        exit.write(AppExit::error());
    }
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    #[test]
    fn fixed_reason_counts_and_transition_cap_do_not_change_gate() {
        let mut r = Routing::default();
        let snap = Snapshot::default();
        for _ in 0..100 {
            outcome(&mut r, &snap, Reason::GpuInput);
            outcome(&mut r, &snap, Reason::OwnerAbsent);
        }
        assert_eq!(r.notes, 32);
        assert_eq!(r.reasons[Reason::GpuInput as usize], 100);
        assert!(r.saved.is_none());
    }
    #[test]
    fn clock_ticks_do_not_spam_state_but_reset_and_view_change_are_stamped() {
        let mut r = Routing::default();
        let mut snap = Snapshot {
            clock: Some((0, 1)),
            view_name: "chase_close".into(),
            ..default()
        };
        state_stamp(&mut r, &snap);
        snap.clock = Some((0, 2));
        state_stamp(&mut r, &snap);
        assert_eq!(r.notes, 1);
        snap.clock = Some((1, 0));
        state_stamp(&mut r, &snap);
        snap.view_name = "bumper".into();
        state_stamp(&mut r, &snap);
        assert_eq!(r.notes, 3);
    }
}

#[cfg(test)]
mod native_pipe_cpu_feasibility {
//! CPU feasibility controls for preserving the native transparency dependency.
//! No RenderContext/GPU execution; actual native function is initialized, NEVER run.
use bevy::{
    core_pipeline::{core_3d::main_transparent_pass_3d, schedule::Core3dSystems},
    ecs::{schedule::{IntoSystemSet, InternedSystemSet, ScheduleCleanupPolicy, SystemSet, NodeId, ScheduleError}, system::{IntoSystem, System}},
    prelude::*,
    render::renderer::{CurrentView, ViewQuery},
};
#[derive(Resource, Default)]
struct Trace { events:Vec<&'static str>, a_calls:usize, native_calls:usize }
#[derive(Resource)] struct SkidRequest;
#[derive(Component)] struct SupportedSkidView;
fn opaque(mut t:ResMut<Trace>){t.events.push("opaque");}
// All A-only resources are optional; missing view is an ordinary Query error.
// Trace is mandatory for BOTH A and B in CPU forwarding tests, never A-only.
fn try_a(current:Option<Res<CurrentView>>,request:Option<Res<SkidRequest>>,views:Query<&SupportedSkidView>,mut t:ResMut<Trace>){
    t.a_calls+=1;
    let (Some(current),Some(_request))=(current,request) else{return};
    if views.get(current.0).is_err(){return;}
    t.events.push("skid");
}
fn native_cpu(mut t:ResMut<Trace>){t.native_calls+=1;t.events.push("native");}
// Deliberately broken control, demonstrating why the production ViewQuery A cannot be piped unchanged.
fn skipped_a(_view:ViewQuery<&SupportedSkidView>,mut t:ResMut<Trace>){t.a_calls+=1;t.events.push("bad-a");}
mod private_dependency {
    use super::*;
    // Caller cannot name this private function. Its dependency is registered by plugin code.
    fn scattering(mut t:ResMut<Trace>){t.events.push("atmosphere");}
    pub(super) fn register_actual(schedule:&mut Schedule)->InternedSystemSet {
        schedule.add_systems(scattering.after(opaque).before(main_transparent_pass_3d));
        scattering.into_system_set().intern()
    }
    pub(super) fn register_unordered_actual(schedule:&mut Schedule)->InternedSystemSet {
        schedule.add_systems(scattering.after(opaque));
        scattering.into_system_set().intern()
    }
    pub(super) fn register_cpu(schedule:&mut Schedule)->InternedSystemSet {
        schedule.add_systems(scattering.after(opaque).before(native_cpu));
        scattering.into_system_set().intern()
    }
}
fn count(schedule:&Schedule,set:InternedSystemSet)->usize {
    schedule.graph().systems_in_set(set).expect("initialized typed set").len()
}
fn sole(schedule:&Schedule,set:InternedSystemSet)->usize {
    let ids=schedule.graph().systems_in_set(set).unwrap();assert_eq!(ids.len(),1);
    let key=*ids.iter().next().unwrap();
    schedule.systems().unwrap().position(|(id,_)|id==key).unwrap()
}
#[derive(Debug, PartialEq, Eq)]
enum Refusal { Count(usize), MissingSet, InvalidGraph }
fn refuse_if_not_one(schedule:&Schedule,set:InternedSystemSet)->Result<(),Refusal>{
    match schedule.graph().systems_in_set(set) {
        Ok(ids) if ids.len()==1=>Ok(()),
        Ok(ids)=>Err(Refusal::Count(ids.len())),
        Err(ScheduleError::SetNotFound)=>Err(Refusal::MissingSet),
        Err(_)=>Err(Refusal::InvalidGraph),
    }
}
// Direct graph constraint is the proof; one observed topological index order is only diagnostic.
fn private_before_native_edge(schedule:&Schedule,private_set:InternedSystemSet,native_set:InternedSystemSet)->bool {
    let graph=schedule.graph();
    let Ok(ids)=graph.systems_in_set(private_set) else{return false};
    if ids.len()!=1{return false;}
    let Some(native)=graph.system_sets.get_key(native_set) else{return false};
    graph.dependency().graph().contains_edge(NodeId::System(*ids.iter().next().unwrap()),NodeId::Set(native))
}
#[test]
fn actual_native_public_type_set_exact_count_survives_remove_readd_graph_only(){
    let mut world=World::new();world.insert_resource(Trace::default());
    let mut schedule=Schedule::default();
    let native_set=main_transparent_pass_3d.into_system_set().intern();
    schedule.add_systems((opaque,main_transparent_pass_3d).chain().in_set(Core3dSystems::MainPass));
    let private_set=private_dependency::register_actual(&mut schedule);
    let _=schedule.initialize(&mut world).unwrap();
    assert_eq!(refuse_if_not_one(&schedule,native_set),Ok(()));
    assert!(private_before_native_edge(&schedule,private_set,native_set));
    assert!(sole(&schedule,private_set)<sole(&schedule,native_set));
    let total_before=schedule.systems_len();
    // Refuse BEFORE mutation; returned removed count is a second independent check.
    let removed=schedule.remove_systems_in_set(main_transparent_pass_3d,&mut world,ScheduleCleanupPolicy::RemoveSystemsOnly).unwrap();
    assert_eq!(removed,1);
    let combined=IntoSystem::into_system(try_a.pipe(main_transparent_pass_3d));
    assert!(combined.default_system_sets().contains(&native_set));
    schedule.add_systems(combined.after(opaque).in_set(Core3dSystems::MainPass));
    let _=schedule.initialize(&mut world).unwrap();
    assert_eq!(count(&schedule,native_set),1);
    assert_eq!(schedule.systems_len(),total_before);
    assert!(private_before_native_edge(&schedule,private_set,native_set));
    assert!(sole(&schedule,private_set)<sole(&schedule,native_set));
    // Adversarial structure: same nodes, no before(native) constraint. A coincidental
    // topological index order must NOT pass the dependency proof.
    let mut unordered=Schedule::default();
    unordered.add_systems((opaque,main_transparent_pass_3d).chain());
    let unordered_private=private_dependency::register_unordered_actual(&mut unordered);
    let _=unordered.initialize(&mut world).unwrap();
    assert_eq!(count(&unordered,native_set),1);
    assert!(!private_before_native_edge(&unordered,unordered_private,native_set));
    // DO NOT schedule.run(): actual native ViewTarget/RenderContext needs renderer/GPU.
}
#[test]
fn exact_count_refuses_zero_and_duplicate_before_any_mutation(){
    let mut world=World::new();let mut schedule=Schedule::default();
    let native_set=main_transparent_pass_3d.into_system_set().intern();
    // Absent is a refusal, not a successful zero-count admission or a panic.
    let _=schedule.initialize(&mut world).unwrap();
    assert!(matches!(schedule.graph().systems_in_set(native_set),Err(ScheduleError::SetNotFound)));
    assert_eq!(refuse_if_not_one(&schedule,native_set),Err(Refusal::MissingSet));
    assert_eq!(schedule.systems_len(),0);
    // Changed/uninitialized graphs are also refused without mutating native systems.
    schedule.add_systems(opaque);
    assert_eq!(refuse_if_not_one(&schedule,native_set),Err(Refusal::InvalidGraph));
    let _=schedule.initialize(&mut world).unwrap();
    // Existing empty native set: initialize native, remove it, retain the typed set.
    schedule.add_systems(main_transparent_pass_3d);let _=schedule.initialize(&mut world).unwrap();
    assert_eq!(schedule.remove_systems_in_set(main_transparent_pass_3d,&mut world,ScheduleCleanupPolicy::RemoveSystemsOnly).unwrap(),1);
    let _=schedule.initialize(&mut world).unwrap();assert_eq!(refuse_if_not_one(&schedule,native_set),Err(Refusal::Count(0)));
    schedule.add_systems((main_transparent_pass_3d,main_transparent_pass_3d));let _=schedule.initialize(&mut world).unwrap();
    assert_eq!(refuse_if_not_one(&schedule,native_set),Err(Refusal::Count(2)));assert_eq!(count(&schedule,native_set),2);
}
#[derive(Clone,Copy)] enum Case {Supported,NoRequest,NoCurrentView,UnsupportedView,StaleEntity}
fn run_cpu(case:Case)->Trace {
    let mut world=World::new();world.insert_resource(Trace::default());world.insert_resource(SkidRequest);
    let entity=match case {Case::UnsupportedView=>world.spawn_empty().id(),_=>world.spawn(SupportedSkidView).id()};
    world.insert_resource(CurrentView(entity));
    match case {Case::NoRequest=>{let _=world.remove_resource::<SkidRequest>();},Case::NoCurrentView=>{let _=world.remove_resource::<CurrentView>();},Case::StaleEntity=>{let _=world.despawn(entity);},_=>{}}
    let mut schedule=Schedule::default();let native_set=native_cpu.into_system_set().intern();
    schedule.add_systems((opaque,native_cpu).chain());let private_set=private_dependency::register_cpu(&mut schedule);
    let _=schedule.initialize(&mut world).unwrap();assert_eq!(count(&schedule,native_set),1);
    assert_eq!(schedule.remove_systems_in_set(native_cpu,&mut world,ScheduleCleanupPolicy::RemoveSystemsOnly).unwrap(),1);
    schedule.add_systems(try_a.pipe(native_cpu).after(opaque));let _=schedule.initialize(&mut world).unwrap();
    assert_eq!(count(&schedule,native_set),1);assert!(private_before_native_edge(&schedule,private_set,native_set));
    assert!(sole(&schedule,private_set)<sole(&schedule,native_set));
    schedule.run(&mut world);world.remove_resource::<Trace>().unwrap()
}
#[test]
fn cpu_native_continuation_once_for_supported_and_every_normal_refusal(){
    for case in [Case::Supported,Case::NoRequest,Case::NoCurrentView,Case::UnsupportedView,Case::StaleEntity]{
        let t=run_cpu(case);assert_eq!(t.a_calls,1);assert_eq!(t.native_calls,1);
        let expected=if matches!(case,Case::Supported){vec!["opaque","atmosphere","skid","native"]}else{vec!["opaque","atmosphere","native"]};
        assert_eq!(t.events,expected);
    }
}
#[test]
fn real_viewquery_skip_negative_control_suppresses_b(){
    let mut world=World::new();world.insert_resource(Trace::default());
    let entity=world.spawn_empty().id();world.insert_resource(CurrentView(entity));
    let mut schedule=Schedule::default();schedule.add_systems(skipped_a.pipe(native_cpu));schedule.run(&mut world);
    let t=world.resource::<Trace>();assert_eq!(t.a_calls,0);assert_eq!(t.native_calls,0);
}

}
