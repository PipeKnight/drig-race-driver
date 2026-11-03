//! Default-off shared opaque conservative motion coverage diagnostic; quality remains unverified.
pub(crate) mod replay_audit;
use bevy::ecs::{
    query::ROQueryItem,
    system::{SystemParamItem, lifetimeless::SRes},
};
use bevy::{
    camera::{MainPassResolutionOverride, Viewport},
    core_pipeline::{
        Core3dSystems,
        prepass::{
            DeferredPrepass, MotionVectorPrepass, Opaque3dPrepass, PreviousViewUniformOffset,
            ViewPrepassTextures,
        },
        schedule::Core3d,
    },
    diagnostic::FrameCount,
    pbr::{
        DrawMesh, DrawPrepass, Material, MaterialExtension, PrepassPipeline, SetMaterialBindGroup,
        SetMeshBindGroup, SetPrepassViewBindGroup, SetPrepassViewEmptyBindGroup,
    },
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        camera::ExtractedCamera,
        render_phase::{
            CachedRenderPipelinePhaseItem, DrawFunctionId, DrawFunctions, RenderCommand,
            RenderCommandResult, RenderCommandState, SetItemPipeline, TrackedRenderPass,
            ViewBinnedRenderPhases,
        },
        render_resource::*,
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::{ExtractedView, ViewDepthTexture},
    },
    shader::{ShaderDefVal, ShaderRef},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread::ThreadId,
};
const MAX_PIPELINES: usize = 512;
#[derive(Resource)]
struct Shaders {
    standard_fragment: Handle<Shader>,
    raw_aux_fragment: Handle<Shader>,
}
#[derive(Resource)]
struct Installed {
    original: DrawFunctionId,
}
#[derive(Clone, Copy)]
struct Active {
    view: Entity,
    frame: u32,
    generation: u64,
    owner: ThreadId,
}
#[derive(Resource, Default)]
struct Scope {
    active: Mutex<Option<Active>>,
    serial: AtomicU64,
}
struct ScopeGuard<'a> {
    scope: &'a Scope,
    generation: u64,
}
impl Scope {
    fn enter(&self, view: Entity, frame: u32) -> Result<ScopeGuard<'_>, &'static str> {
        let mut active = self.active.lock().map_err(|_| "scope-poison")?;
        if active.is_some() {
            return Err("scope-already-active");
        }
        let generation = self
            .serial
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| "scope-generation-exhausted")?
            + 1;
        *active = Some(Active {
            view,
            frame,
            generation,
            owner: std::thread::current().id(),
        });
        Ok(ScopeGuard {
            scope: self,
            generation,
        })
    }
    fn selected(&self, view: Entity, frame: u32) -> Result<bool, &'static str> {
        let active = self.active.lock().map_err(|_| "scope-poison")?;
        Ok(active.as_ref().is_some_and(|a| {
            a.view == view && a.frame == frame && a.owner == std::thread::current().id()
        }))
    }
}
impl Drop for ScopeGuard<'_> {
    fn drop(&mut self) {
        // No mutex is held through phase.render. Do not replace a different generation.
        let mut active = self.scope.active.lock().unwrap_or_else(|e| e.into_inner());
        if active
            .as_ref()
            .is_some_and(|a| a.generation == self.generation)
        {
            *active = None;
        }
    }
}
#[derive(Resource, Default)]
struct FirstFailure(AtomicU64);
#[derive(Resource, Default)]
struct Derived(Mutex<HashMap<CachedRenderPipelineId, CachedRenderPipelineId>>);
#[derive(Resource, Default)]
struct Counts {
    considered: AtomicU64,
    replayed: AtomicU64,
    skipped: AtomicU64,
    selected: AtomicU64,
    selector_skip: AtomicU64,
}
fn inc(n: &AtomicU64) {
    let _ = n.try_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
        Some(n.saturating_add(1))
    });
}
fn has(defs: &[ShaderDefVal], name: &str) -> bool {
    defs.iter()
        .any(|d| matches!(d, ShaderDefVal::Bool(s, true) if s == name))
}

/// Diagnostic admission only. Root integration must also reject the old name-specific thin pilot.
pub fn requested() -> Result<bool, String> {
    match std::env::var("GRID_OPAQUE_CONSERVATIVE_MOTION") {
        Err(std::env::VarError::NotPresent) => Ok(false),
        Ok(v) if v == "1" => Ok(true),
        _ => Err("GRID_OPAQUE_CONSERVATIVE_MOTION must be absent or1".into()),
    }
}
pub fn install(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let fragment = match <StandardMaterial as Material>::prepass_fragment_shader() {
        ShaderRef::Handle(h) => h,
        ShaderRef::Path(path) => app.world().resource::<AssetServer>().load(path),
        ShaderRef::Default => return Err("unexpected StandardMaterial prepass shader ABI".into()),
    };
    let raw_aux_fragment =
        match <crate::foundation_backing::RawPass as MaterialExtension>::prepass_fragment_shader() {
            ShaderRef::Handle(h) => h,
            ShaderRef::Path(path) => app.world().resource::<AssetServer>().load(path),
            ShaderRef::Default => {
                return Err("unexpected registered RawPass auxiliary shader ABI".into());
            }
        };
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return Err("render app absent".into());
    };
    render
        .insert_resource(Shaders {
            standard_fragment: fragment,
            raw_aux_fragment,
        })
        .init_resource::<Scope>()
        .init_resource::<Derived>()
        .init_resource::<FirstFailure>()
        .init_resource::<Counts>()
        .add_systems(RenderStartup, wrap)
        .add_systems(
            Core3d,
            replay
                .after(Core3dSystems::Prepass)
                .before(Core3dSystems::MainPass),
        );
    Ok(())
}
// RenderStartup runs on every acquired RenderDevice, including in the same render world.
// Exclusive startup has no active pass; all CPU state tied to old GPU/cache IDs is discarded.
fn reset_lifetime_state(world: &mut World) {
    world.insert_resource(Scope::default());
    world.insert_resource(Derived::default());
    world.insert_resource(FirstFailure::default());
    world.insert_resource(Counts::default());
    replay_audit::reset(world);
}
fn wrap(world: &mut World) {
    reset_lifetime_state(world);
    // Rebuild SystemState against this lifetime's resources, replacing the SAME original ID.
    // Do not add_with/register another ID or wrap the old replacement recursively.
    let replacement = RenderCommandState::<Opaque3dPrepass, ScopedPrepass>::new(world);
    let original = {
        let functions = world.resource::<DrawFunctions<Opaque3dPrepass>>();
        let mut functions = functions.write();
        let id = functions
            .get_id::<DrawPrepass>()
            .expect("stock DrawPrepass ID absent");
        functions.draw_functions[id.0 as usize] = Box::new(replacement);
        id
    };
    world.insert_resource(Installed { original });
}
type ScopedPrepass = (
    SelectModePipeline,
    SetPrepassViewBindGroup<0>,
    SetPrepassViewEmptyBindGroup<1>,
    SetMeshBindGroup<2>,
    SetMaterialBindGroup<3>,
    DrawMesh,
);
struct SelectModePipeline;
impl RenderCommand<Opaque3dPrepass> for SelectModePipeline {
    type Param = (
        SRes<PipelineCache>,
        SRes<Scope>,
        SRes<Derived>,
        SRes<FrameCount>,
        SRes<Counts>,
        Option<SRes<replay_audit::GpuState>>,
    );
    type ViewQuery = Entity;
    type ItemQuery = ();
    fn render<'w>(
        item: &Opaque3dPrepass,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        (cache, scope, derived, frame, counts, audit_state): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        match scope.selected(view, frame.0) {
            Ok(false) => {
                return <SetItemPipeline as RenderCommand<Opaque3dPrepass>>::render(
                    item,
                    (),
                    None,
                    cache,
                    pass,
                );
            }
            Err(_) => return RenderCommandResult::Failure("conservative mode scope poisoned"),
            Ok(true) => {}
        }
        let id = if let Some(state) = audit_state
            .as_deref()
            .filter(|s| s.is_selected(view, frame.0))
        {
            state.selection(view, frame.0, item.cached_pipeline())
        } else {
            match derived.0.lock() {
                Ok(map) => map.get(&item.cached_pipeline()).copied(),
                Err(_) => None,
            }
        };
        let cache = cache.into_inner();
        if let Some(pipeline) = id.and_then(|id| cache.get_render_pipeline(id)) {
            pass.set_render_pipeline(pipeline);
            inc(&counts.selected);
            RenderCommandResult::Success
        } else {
            // Never fall back to a depth-writing ordinary pipeline in the MV-only pass.
            inc(&counts.selector_skip);
            RenderCommandResult::Skip
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FragmentAbi {
    Stock,
    SourceAuxQuarter,
}
fn def_name(def: &ShaderDefVal) -> &str {
    match def {
        ShaderDefVal::Bool(s, _) | ShaderDefVal::Int(s, _) | ShaderDefVal::UInt(s, _) => s,
    }
}
fn named(defs: &[ShaderDefVal], name: &str) -> usize {
    defs.iter().filter(|d| def_name(d) == name).count()
}
// Pure admission; it never adds/removes shader defs or changes quarter coverage/UV/material data.
fn fragment_abi(
    fragment: &Handle<Shader>,
    vertex: &[ShaderDefVal],
    defs: &[ShaderDefVal],
    stock: &Handle<Shader>,
    shaders: &Shaders,
) -> Result<FragmentAbi, &'static str> {
    if !has(vertex, "MOTION_VECTOR_PREPASS") || !has(defs, "MOTION_VECTOR_PREPASS") {
        return Err("missing-both-stage-MOTION_VECTOR_PREPASS");
    }
    if fragment == &shaders.raw_aux_fragment {
        const BAD: [&str; 12] = [
            "DEFERRED_PREPASS",
            "BINDLESS",
            "MESHLET_MESH_MATERIAL_PASS",
            "VISIBILITY_RANGE_DITHER",
            "STANDARD_MATERIAL_NORMAL_MAP",
            "PREMULTIPLY_ALPHA",
            "OIT_ENABLED",
            "UNCLIPPED_DEPTH_ORTHO",
            "UNCLIPPED_DEPTH_ORTHO_EMULATION",
            "VIEW_PROJECTION_ORTHOGRAPHIC",
            "READS_VIEW_TRANSMISSION_TEXTURE",
            "FORWARD_DECAL",
        ];
        if vertex
            .iter()
            .chain(defs)
            .any(|d| BAD.contains(&def_name(d)))
        {
            return Err("source-aux-unsupported-domain");
        }
        let quarter = "OBJECT_AUX_CLIP_QUARTER";
        if named(vertex, quarter) + named(defs, quarter) != 1
            || !has(defs, quarter)
            || !has(defs, "PREPASS_FRAGMENT")
            || vertex
                .iter()
                .chain(defs)
                .any(|d| def_name(d).starts_with("OBJECT_AUX_") && def_name(d) != quarter)
        {
            return Err("source-aux-exact-quarter-contract-required");
        }
        // Native A2C can remain in the opaque phase with MAY_DISCARD. Permit only the
        // reviewed Aux shader and ordinary matched stage flags, never stock/custom globally.
        let discard_ok = (named(vertex, "MAY_DISCARD") == 0 && named(defs, "MAY_DISCARD") == 0)
            || (named(vertex, "MAY_DISCARD") == 1
                && named(defs, "MAY_DISCARD") == 1
                && has(vertex, "MAY_DISCARD")
                && has(defs, "MAY_DISCARD"));
        if !discard_ok {
            return Err("source-aux-malformed-discard-routing");
        }
        return Ok(FragmentAbi::SourceAuxQuarter);
    }
    if fragment != stock && fragment != &shaders.standard_fragment {
        return Err("custom-fragment-ABI");
    }
    const BAD: [&str; 4] = [
        "DEFERRED_PREPASS",
        "MAY_DISCARD",
        "VISIBILITY_RANGE_DITHER",
        "UNCLIPPED_DEPTH_ORTHO_EMULATION",
    ];
    if vertex
        .iter()
        .chain(defs)
        .any(|d| BAD.contains(&def_name(d)) || def_name(d).starts_with("OBJECT_AUX_"))
    {
        return Err("unsupported-stock-fragment-context");
    }
    Ok(FragmentAbi::Stock)
}
fn conservative_descriptor(
    original: &RenderPipelineDescriptor,
    stock: &Handle<Shader>,
    shaders: &Shaders,
) -> Result<RenderPipelineDescriptor, &'static str> {
    // Clone only after read-only guards succeed; original colour/depth pipeline stays untouched.
    if original.vertex.shader != *stock
        || original.multisample.count != 1
        || original.primitive.topology != PrimitiveTopology::TriangleList
        || original.primitive.polygon_mode != PolygonMode::Fill
        || original.multisample.alpha_to_coverage_enabled
    {
        return Err("unsupported-vertex-or-topology");
    }
    let depth = original
        .depth_stencil
        .as_ref()
        .ok_or("missing-depth-state")?;
    if depth.format != TextureFormat::Depth32Float
        || depth.depth_compare != Some(CompareFunction::GreaterEqual)
        || depth.stencil.write_mask != 0
    {
        return Err("unsupported-depth-state");
    }
    let fragment = original.fragment.as_ref().ok_or("missing-MV-fragment")?;
    fragment_abi(
        &fragment.shader,
        &original.vertex.shader_defs,
        &fragment.shader_defs,
        stock,
        shaders,
    )?;
    let mv = fragment
        .targets
        .get(1)
        .and_then(Option::as_ref)
        .ok_or("missing-MV-slot1")?
        .clone();
    if mv.format != TextureFormat::Rg16Float || mv.blend.is_some() {
        return Err("unsupported-MV-target");
    }
    let mut derived = original.clone();
    derived.fragment.as_mut().unwrap().targets = vec![None, Some(mv), None, None];
    derived.depth_stencil.as_mut().unwrap().depth_write_enabled = Some(false);
    derived.primitive.conservative = true;
    derived.label = Some("diagnostic shared opaque conservative MV only".into());
    Ok(derived)
}
fn descriptor_failure(
    first: &FirstFailure,
    id: CachedRenderPipelineId,
    d: &RenderPipelineDescriptor,
    reason: &str,
) {
    if first
        .0
        .compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    fn bounded(value: impl std::fmt::Debug) -> String {
        format!("{value:?}").chars().take(128).collect()
    }
    fn defs(values: &[ShaderDefVal]) -> Vec<String> {
        values.iter().take(32).map(bounded).collect()
    }
    let f = d.fragment.as_ref();
    println!(
        "opaque_conservative_first_failure reason={reason} original_id={} vertex_shader={} fragment_shader={} vertex_defs_count={} vertex_defs={:?} fragment_defs_count={} fragment_defs={:?} max_defs_per_stage=32 max_chars_per_def=128 per_device_lifetime_limit=1",
        id.id(),
        bounded(&d.vertex.shader),
        bounded(f.map(|f| &f.shader)),
        d.vertex.shader_defs.len(),
        defs(&d.vertex.shader_defs),
        f.map_or(0, |f| f.shader_defs.len()),
        f.map(|f| defs(&f.shader_defs))
    );
}
fn derive(
    cache: &PipelineCache,
    originals: &HashSet<CachedRenderPipelineId>,
    derived: &Derived,
    stock: &PrepassPipeline,
    shaders: &Shaders,
    first: &FirstFailure,
) -> Result<(), &'static str> {
    let mut map = derived.0.lock().map_err(|_| "cache-poison")?;
    let mut ready = true;
    for original in originals {
        if matches!(
            cache.get_render_pipeline_state(*original),
            CachedPipelineState::Err(_)
        ) {
            return Err("original-pipeline-compile-error");
        }
        if cache.get_render_pipeline(*original).is_none() {
            ready = false;
            continue;
        }
        let original_descriptor = cache.get_render_pipeline_descriptor(*original);
        if !map.contains_key(original) {
            if map.len() == MAX_PIPELINES {
                descriptor_failure(first, *original, original_descriptor, "pipeline-cache-cap");
                return Err("pipeline-cache-cap");
            }
            let descriptor = match conservative_descriptor(
                original_descriptor,
                &stock.default_prepass_shader,
                shaders,
            ) {
                Ok(d) => d,
                Err(reason) => {
                    descriptor_failure(first, *original, original_descriptor, reason);
                    return Err(reason);
                }
            };
            map.insert(*original, cache.queue_render_pipeline(descriptor));
            ready = false;
        }
        if map.get(original).is_some_and(|id| {
            matches!(
                cache.get_render_pipeline_state(*id),
                CachedPipelineState::Err(_)
            )
        }) {
            descriptor_failure(
                first,
                *original,
                original_descriptor,
                "derived-pipeline-compile-error",
            );
            return Err("derived-pipeline-compile-error");
        }
        if map
            .get(original)
            .is_none_or(|id| cache.get_render_pipeline(*id).is_none())
        {
            ready = false;
        }
    }
    if ready {
        Ok(())
    } else {
        Err("derived-pipeline-pending")
    }
}

// Every actually dispatched stock item is constructed from these public key collections.
fn inventory(
    phase: &bevy::render::render_phase::BinnedRenderPhase<Opaque3dPrepass>,
    installed: &Installed,
) -> Result<HashSet<CachedRenderPipelineId>, &'static str> {
    if !phase.non_mesh_items.is_empty() {
        return Err("non-mesh-items");
    }
    let mut ids = HashSet::new();
    for key in phase
        .multidrawable_meshes
        .keys()
        .chain(phase.batchable_meshes.keys().map(|(k, _)| k))
        .chain(phase.unbatchable_meshes.keys().map(|(k, _)| k))
    {
        if key.draw_function != installed.original {
            return Err("unwrapped-draw-ID");
        }
        ids.insert(key.pipeline);
    }
    if ids.is_empty() {
        return Err("empty-phase");
    }
    Ok(ids)
}
type Inputs = (
    Entity,
    &'static ExtractedCamera,
    &'static ExtractedView,
    Option<&'static ViewDepthTexture>,
    Option<&'static ViewPrepassTextures>,
    Option<&'static MainPassResolutionOverride>,
    &'static Msaa,
    Option<&'static DeferredPrepass>,
    Option<&'static PreviousViewUniformOffset>,
    Has<MotionVectorPrepass>,
    Has<crate::native_dlaa::DlaaView>,
);
type AuditResources<'w> = (
    Option<Res<'w, replay_audit::Audit>>,
    Option<Res<'w, replay_audit::GpuState>>,
    Res<'w, bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    Res<'w, bevy::render::renderer::RenderAdapter>,
);
#[allow(clippy::too_many_arguments)]
fn replay(
    world: &World,
    view: ViewQuery<Inputs>,
    phases: Res<ViewBinnedRenderPhases<Opaque3dPrepass>>,
    installed: Option<Res<Installed>>,
    stock: Res<PrepassPipeline>,
    shaders: Res<Shaders>,
    first: Res<FirstFailure>,
    scope: Res<Scope>,
    derived: Res<Derived>,
    counts: Res<Counts>,
    frame: Res<FrameCount>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
    audit_resources: AuditResources,
) {
    let (audit, audit_state, images, adapter) = audit_resources;
    inc(&counts.considered);
    let (
        entity,
        camera,
        extracted,
        depth,
        prepass,
        resolution,
        msaa,
        deferred,
        previous,
        motion_prepass,
        dlaa,
    ) = view.into_inner();
    let result = (|| -> Result<(), &'static str> {
        if !dlaa
            || deferred.is_some()
            || msaa.samples() != 1
            || previous.is_none()
            || !motion_prepass
        {
            return Err("not-native-forward-MV-view");
        }
        if !device
            .features()
            .contains(WgpuFeatures::CONSERVATIVE_RASTERIZATION)
        {
            return Err("hardware-unsupported");
        }
        let (Some(depth), Some(prepass), Some(resolution), Some(installed)) =
            (depth, prepass, resolution, installed.as_deref())
        else {
            return Err("context-unavailable");
        };
        let output = UVec2::new(extracted.viewport.z, extracted.viewport.w);
        if resolution.0 != output
            || prepass.size.width != output.x
            || prepass.size.height != output.y
            || depth.texture.format() != TextureFormat::Depth32Float
            || depth.texture.sample_count() != 1
        {
            return Err("native-attachment-mismatch");
        }
        let motion = prepass
            .motion_vectors_view()
            .ok_or("motion-attachment-absent")?;
        let phase = phases
            .get(&extracted.retained_view_entity)
            .ok_or("opaque-phase-absent")?;
        let originals = inventory(phase, installed)?;
        derive(&cache, &originals, &derived, &stock, &shaders, &first)?;
        // Prewarm audit variants in advance; never render them outside the exact shared audit transaction.
        let mut audit_target = None;
        let mut audit_source = None;
        if let (Some(audit), Some(state)) = (audit.as_deref(), audit_state.as_deref()) {
            if let Err(error) = replay_audit::prewarm(audit, state, &cache, &originals, &derived) {
                replay_audit::capture_failed(audit, error);
                return Err(error);
            }
            if let Some(source) = crate::temporal_audit::replay_request(world)? {
                audit_target = Some(replay_audit::target(
                    audit, state, &cache, &originals, &adapter, &device, &images, entity, frame.0,
                    output,
                )?);
                audit_source = Some(source);
            }
        }
        // No table/phase/material/cache-key mutation after entering this scope.
        let _mode = scope.enter(entity, frame.0)?;
        let attachments = [
            audit_target.as_ref().map(|view| RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(LinearRgba::new(0., 0., 0., -1.).into()),
                    store: StoreOp::Store,
                },
            }),
            Some(RenderPassColorAttachment {
                view: motion,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            }),
            None,
            None,
        ];
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("diagnostic opaque conservative MV only"),
            color_attachments: if audit_target.is_some() {
                &attachments[..2]
            } else {
                &attachments
            },
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth.view(),
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(viewport) =
            Viewport::from_viewport_and_override(camera.viewport.as_ref(), Some(resolution))
        {
            pass.set_camera_viewport(&viewport);
        }
        phase
            .render(&mut pass, world, entity)
            .map_err(|_| "phase-render-error")?;
        drop(pass);
        if let (Some(state), Some(source)) = (audit_state.as_deref(), audit_source) {
            replay_audit::written(state, entity, frame.0, source)?;
        }
        inc(&counts.replayed);
        Ok(())
    })();
    let reason = match result {
        Err(why) => {
            inc(&counts.skipped);
            why
        }
        Ok(()) => "replayed",
    };
    // CPU selector/replay counters only. A bound pipeline is NOT a completed DrawMesh/GPU command.
    if frame.0 == 1 || frame.0.is_multiple_of(60) {
        println!(
            "opaque_conservative diagnostic=true frame={} view={entity:?} reason={reason} considered={} replay_invocations={} skipped_views={} selectors_bound={} selector_skip={} gpu_draws=unknown geometry_copy=false wholeview_preflight=true",
            frame.0,
            counts.considered.load(Ordering::Relaxed),
            counts.replayed.load(Ordering::Relaxed),
            counts.skipped.load(Ordering::Relaxed),
            counts.selected.load(Ordering::Relaxed),
            counts.selector_skip.load(Ordering::Relaxed)
        );
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    fn views() -> (Entity, Entity) {
        let mut world = World::new();
        (world.spawn_empty().id(), world.spawn_empty().id())
    }

    #[test]
    fn active_scope_selects_only_owner_view_frame_and_thread() {
        let (view, other_view) = views();
        let scope = Scope::default();
        assert!(!scope.selected(view, 17).unwrap());
        let owner = scope.enter(view, 17).unwrap();
        assert!(scope.selected(view, 17).unwrap());
        assert!(!scope.selected(other_view, 17).unwrap());
        assert!(!scope.selected(view, 18).unwrap());
        // Join while the owning thread still holds the active guard.
        std::thread::scope(|threads| {
            threads
                .spawn(|| {
                    assert!(!scope.selected(view, 17).unwrap());
                    assert!(scope.enter(view, 17).is_err());
                })
                .join()
                .unwrap();
        });
        assert!(scope.selected(view, 17).unwrap());
        drop(owner);
        assert!(!scope.selected(view, 17).unwrap());
    }

    #[test]
    fn refused_nested_enter_preserves_owner_then_next_scope_can_enter() {
        let (view, other_view) = views();
        let scope = Scope::default();
        let owner = scope.enter(view, 20).unwrap();
        assert!(scope.enter(view, 20).is_err());
        assert!(scope.enter(other_view, 21).is_err());
        assert!(scope.selected(view, 20).unwrap());
        assert!(!scope.selected(other_view, 21).unwrap());
        drop(owner);
        let next = scope.enter(other_view, 21).unwrap();
        assert!(scope.selected(other_view, 21).unwrap());
        assert!(!scope.selected(view, 20).unwrap());
        drop(next);
        assert!(!scope.selected(other_view, 21).unwrap());
    }

    #[test]
    fn propagated_error_drops_scope_and_allows_next_entry() {
        fn fail(scope: &Scope, view: Entity) -> Result<(), &'static str> {
            let _guard = scope.enter(view, 30)?;
            assert!(scope.selected(view, 30)?);
            Err("deliberate operation error")
        }
        let (view, other_view) = views();
        let scope = Scope::default();
        assert_eq!(fail(&scope, view), Err("deliberate operation error"));
        assert!(!scope.selected(view, 30).unwrap());
        let _next = scope.enter(other_view, 31).unwrap();
        assert!(scope.selected(other_view, 31).unwrap());
    }

    #[test]
    fn unwind_drops_scope_without_poison_and_allows_next_entry() {
        let (view, other_view) = views();
        let scope = Scope::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = scope.enter(view, 40).unwrap();
            assert!(scope.selected(view, 40).unwrap());
            panic!("deliberate scope unwind");
        }));
        assert!(result.is_err());
        assert!(!scope.selected(view, 40).unwrap());
        let _next = scope.enter(other_view, 41).unwrap();
        assert!(scope.selected(other_view, 41).unwrap());
        assert!(!scope.selected(view, 40).unwrap());
    }

    #[test]
    fn new_device_lifetime_clears_old_scope_cache_and_counters_without_gpu() {
        let mut world = World::new();
        let view = world.spawn_empty().id();
        world.init_resource::<Scope>();
        world.init_resource::<Derived>();
        world.init_resource::<Counts>();
        {
            // Typed cache IDs and stale CPU metadata only: no RenderDevice/PipelineCache/mock GPU.
            let scope = world.resource::<Scope>();
            scope.serial.store(9, Ordering::Relaxed);
            *scope.active.lock().unwrap() = Some(Active {
                view,
                frame: 73,
                generation: 9,
                owner: std::thread::current().id(),
            });
            assert!(scope.selected(view, 73).unwrap());
            world.resource::<Derived>().0.lock().unwrap().insert(
                CachedRenderPipelineId::new(7),
                CachedRenderPipelineId::new(8),
            );
            let counts = world.resource::<Counts>();
            for counter in [
                &counts.considered,
                &counts.replayed,
                &counts.skipped,
                &counts.selected,
                &counts.selector_skip,
            ] {
                inc(counter);
            }
        }
        reset_lifetime_state(&mut world);
        assert!(world.resource::<Derived>().0.lock().unwrap().is_empty());
        let counts = world.resource::<Counts>();
        assert!(
            [
                &counts.considered,
                &counts.replayed,
                &counts.skipped,
                &counts.selected,
                &counts.selector_skip
            ]
            .iter()
            .all(|n| n.load(Ordering::Relaxed) == 0)
        );
        let scope = world.resource::<Scope>();
        assert!(!scope.selected(view, 73).unwrap());
        assert_eq!(scope.serial.load(Ordering::Relaxed), 0);
        let next = scope.enter(view, 74).unwrap();
        assert!(scope.selected(view, 74).unwrap());
        drop(next);
        assert!(!scope.selected(view, 74).unwrap());
        // A second same-world lifetime reset is also safe.
        reset_lifetime_state(&mut world);
        assert!(world.resource::<Derived>().0.lock().unwrap().is_empty());
        let _guard = world.resource::<Scope>().enter(view, 75).unwrap();
        assert!(world.resource::<Scope>().selected(view, 75).unwrap());
    }
}

#[cfg(test)]
mod raw_aux_admission_tests {
    use super::*;
    use bevy::asset::uuid_handle;

    fn fixture() -> (Handle<Shader>, Shaders, RenderPipelineDescriptor) {
        let stock = uuid_handle!("d1491aac-0e93-490b-b687-249958c53c3c");
        let raw_aux =
            match <crate::foundation_backing::RawPass as MaterialExtension>::prepass_fragment_shader(
            ) {
                ShaderRef::Handle(h) => h,
                _ => panic!("registered source Aux contract unexpectedly changed"),
            };
        let shaders = Shaders {
            standard_fragment: uuid_handle!("b7bad2f5-0fce-443f-93ec-4b3e411c5552"),
            raw_aux_fragment: raw_aux.clone(),
        };
        let mut d = RenderPipelineDescriptor::default();
        d.vertex.shader = stock.clone();
        d.vertex.shader_defs = [
            "PREPASS_PIPELINE",
            "PREPASS_FRAGMENT",
            "NORMAL_PREPASS",
            "MOTION_VECTOR_PREPASS",
            "MAY_DISCARD",
            "VERTEX_UVS",
            "VERTEX_UVS_B",
            "STANDARD_MATERIAL_BASE_COLOR_UV_B",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let mut defs = d.vertex.shader_defs.clone();
        defs.push("OBJECT_AUX_CLIP_QUARTER".into());
        d.fragment = Some(FragmentState {
            shader: raw_aux,
            shader_defs: defs,
            targets: vec![
                Some(ColorTargetState {
                    format: TextureFormat::Rgb10a2Unorm,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                }),
                Some(ColorTargetState {
                    format: TextureFormat::Rg16Float,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                }),
                None,
                None,
            ],
            ..default()
        });
        d.depth_stencil = Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::GreaterEqual),
            stencil: StencilState {
                write_mask: 0,
                ..default()
            },
            bias: DepthBiasState {
                constant: 7,
                slope_scale: 1.25,
                clamp: 0.0,
            },
        });
        d.primitive.cull_mode = Some(Face::Back);
        d.primitive.front_face = FrontFace::Cw;
        (stock, shaders, d)
    }

    #[test]
    fn registered_aux_quarter_retains_original_shader_defs_uv_and_facing() {
        let (stock, shaders, original) = fixture();
        let f = original.fragment.as_ref().unwrap();
        assert_eq!(
            fragment_abi(
                &f.shader,
                &original.vertex.shader_defs,
                &f.shader_defs,
                &stock,
                &shaders
            ),
            Ok(FragmentAbi::SourceAuxQuarter)
        );
        let derived = conservative_descriptor(&original, &stock, &shaders).unwrap();
        assert_eq!(derived.vertex, original.vertex);
        assert_eq!(derived.layout, original.layout);
        assert_eq!(derived.fragment.as_ref().unwrap().shader, f.shader);
        assert_eq!(
            derived.fragment.as_ref().unwrap().shader_defs,
            f.shader_defs
        );
        assert_eq!(derived.primitive.front_face, original.primitive.front_face);
        assert_eq!(derived.primitive.cull_mode, original.primitive.cull_mode);
        assert_eq!(
            derived.depth_stencil.as_ref().unwrap().bias,
            original.depth_stencil.as_ref().unwrap().bias
        );
        assert!(derived.fragment.as_ref().unwrap().targets[0].is_none());
        assert!(derived.fragment.as_ref().unwrap().targets[1].is_some());
        assert_eq!(
            derived.depth_stencil.as_ref().unwrap().depth_write_enabled,
            Some(false)
        );
        assert_eq!(
            original.depth_stencil.as_ref().unwrap().depth_write_enabled,
            Some(true)
        );
        assert!(has(&f.shader_defs, "OBJECT_AUX_CLIP_QUARTER"));
        assert!(has(&f.shader_defs, "STANDARD_MATERIAL_BASE_COLOR_UV_B"));
    }

    #[test]
    fn unknown_shader_cannot_spoof_source_aux_by_copying_its_defs() {
        let (stock, shaders, mut d) = fixture();
        d.fragment.as_mut().unwrap().shader = uuid_handle!("e7b0dcbb-66b4-4e54-830e-86dde999f258");
        assert_eq!(
            conservative_descriptor(&d, &stock, &shaders).unwrap_err(),
            "custom-fragment-ABI"
        );
        d.fragment.as_mut().unwrap().shader = shaders.standard_fragment.clone();
        // Remove MAY_DISCARD so this specifically rejects a quarter marker on a stock shader.
        d.vertex
            .shader_defs
            .retain(|v| def_name(v) != "MAY_DISCARD");
        d.fragment
            .as_mut()
            .unwrap()
            .shader_defs
            .retain(|v| def_name(v) != "MAY_DISCARD");
        assert!(conservative_descriptor(&d, &stock, &shaders).is_err());
    }

    #[test]
    fn aux_rejects_wrong_duplicate_or_disabled_clip_and_forbidden_domains() {
        let (stock, shaders, original) = fixture();
        for extra in [
            "OBJECT_AUX_CLIP_CSM_HALF",
            "OBJECT_AUX_NO_FIXED_CUTOFF",
            "DEFERRED_PREPASS",
            "BINDLESS",
            "VISIBILITY_RANGE_DITHER",
            "STANDARD_MATERIAL_NORMAL_MAP",
            "UNCLIPPED_DEPTH_ORTHO_EMULATION",
        ] {
            let mut d = original.clone();
            d.fragment.as_mut().unwrap().shader_defs.push(extra.into());
            assert!(
                conservative_descriptor(&d, &stock, &shaders).is_err(),
                "{extra}"
            );
        }
        let mut duplicate = original.clone();
        duplicate
            .fragment
            .as_mut()
            .unwrap()
            .shader_defs
            .push("OBJECT_AUX_CLIP_QUARTER".into());
        assert!(conservative_descriptor(&duplicate, &stock, &shaders).is_err());
        let mut disabled = original.clone();
        let defs = &mut disabled.fragment.as_mut().unwrap().shader_defs;
        defs.retain(|v| def_name(v) != "OBJECT_AUX_CLIP_QUARTER");
        defs.push(ShaderDefVal::Bool("OBJECT_AUX_CLIP_QUARTER".into(), false));
        assert!(conservative_descriptor(&disabled, &stock, &shaders).is_err());
        let mut missing_vertex_motion = original.clone();
        missing_vertex_motion
            .vertex
            .shader_defs
            .retain(|v| def_name(v) != "MOTION_VECTOR_PREPASS");
        assert!(conservative_descriptor(&missing_vertex_motion, &stock, &shaders).is_err());
        let mut wrong_vertex = original.clone();
        wrong_vertex.vertex.shader = shaders.standard_fragment.clone();
        assert!(conservative_descriptor(&wrong_vertex, &stock, &shaders).is_err());
    }

    #[test]
    fn stock_keeps_discard_rejection_and_aux_requires_matched_discard_routing() {
        let (stock, shaders, mut d) = fixture();
        d.fragment.as_mut().unwrap().shader = shaders.standard_fragment.clone();
        d.fragment
            .as_mut()
            .unwrap()
            .shader_defs
            .retain(|v| !def_name(v).starts_with("OBJECT_AUX_"));
        assert!(conservative_descriptor(&d, &stock, &shaders).is_err());
        d.vertex
            .shader_defs
            .retain(|v| def_name(v) != "MAY_DISCARD");
        d.fragment
            .as_mut()
            .unwrap()
            .shader_defs
            .retain(|v| def_name(v) != "MAY_DISCARD");
        assert!(conservative_descriptor(&d, &stock, &shaders).is_ok());
        let (_, _, mut aux) = fixture();
        aux.vertex
            .shader_defs
            .retain(|v| def_name(v) != "MAY_DISCARD");
        assert!(conservative_descriptor(&aux, &stock, &shaders).is_err());
    }
}
