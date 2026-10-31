//! Per-pass GPU time (P12, docs/PERFORMANCE.md "Grid start: GPU passes"), on
//! in the profile build (feature `profile`) and with `GRID_GPU_PASSES=1`:
//! Bevy's render diagnostics
//! put timestamp queries (and, where the device has them, pipeline
//! statistics) around each render pass, and every
//! [`window_seconds`] the app logs the mean per frame:
//!
//! ```text
//! gpu_window version=1 seconds=10.0 frames=283 timestamps=true
//! gpu_pass rank=1 gpu_ms=6.10 primitives=812345 vertices=1201234 fragments=2101234 name=main_opaque_pass_3d
//! ```
//!
//! A pass every camera runs (the reflection faces' `main_opaque_pass_3d`
//! beside the main view's) is the sum over the cameras.
//!
//! Bevy times the main passes but not the shadow passes; `shadow_span` adds
//! one pass per sun cascade (`shadow_cam0_c0..3`). Each window also logs the
//! triangles each view submits (the camera and every shadow cascade), by
//! owner, from the visibility lists and the meshes' GPU sizes:
//!
//! ```text
//! view_triangles view=camera total=812345 opponent=401234 car_batch=... player=... batch=... track=... other=...
//! ```
//!
//! On a tiled GPU (the phones) a pass's timestamps bracket its whole tile
//! work, which is what the frame waits for. Independently authored
//! diagnostics; only GPU counters and visibility lists are read.
use bevy::camera::visibility::{CascadesVisibleEntities, VisibleEntities};
use bevy::diagnostic::DiagnosticsStore;
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::render::mesh::{RenderMesh, RenderMeshBufferInfo};
use bevy::render::render_asset::RenderAssets;
use bevy::render::{Render, RenderApp, RenderSystems};
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

/// Triangles per mesh asset, copied from the render world.
static TRIANGLES: Mutex<Option<HashMap<AssetId<Mesh>, u32>>> = Mutex::new(None);

/// Per pass: GPU ms, primitives out of the clipper, vertex and fragment
/// shader invocations; sums and frames counted.
#[derive(Default)]
struct Pass {
    sums: [f64; 4],
    frames: [u32; 4],
}

const FIELDS: [&str; 4] = [
    "elapsed_gpu",
    "clipper_primitives_out",
    "vertex_shader_invocations",
    "fragment_shader_invocations",
];

/// On in the profile build, or with `GRID_GPU_PASSES=1`.
fn profiling_enabled() -> bool {
    cfg!(feature = "profile") || std::env::var("GRID_GPU_PASSES").is_ok_and(|v| v == "1")
}

pub fn enabled() -> bool {
    profiling_enabled() || submissions::budget().is_some()
}

/// Seconds per logged table: `GRID_PROFILE_SECONDS`, default 10 (as the
/// profile build's span tables).
fn window_seconds() -> f32 {
    static SECONDS: OnceLock<f32> = OnceLock::new();
    *SECONDS.get_or_init(|| {
        std::env::var("GRID_PROFILE_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&s: &f32| s >= 1.)
            .unwrap_or(10.)
    })
}

pub fn install(app: &mut App) {
    if profiling_enabled() {
        app.add_plugins(RenderDiagnosticsPlugin)
            .add_systems(Last, (report, view_triangles));
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.add_systems(Render, mesh_triangles.after(RenderSystems::Cleanup));
            shadow_span::install(render);
        }
    }
    if let Some(budget) = submissions::budget() {
        submissions::install(app, budget);
    }
}

/// Copies the meshes' triangle counts when the main world asks (no map).
fn mesh_triangles(meshes: Res<RenderAssets<RenderMesh>>) {
    let mut map = TRIANGLES.lock().unwrap_or_else(|e| e.into_inner());
    if map.is_some() {
        return;
    }
    *map = Some(
        meshes
            .iter()
            .map(|(id, mesh)| {
                let corners = match mesh.buffer_info {
                    RenderMeshBufferInfo::Indexed { count, .. } => count,
                    RenderMeshBufferInfo::NonIndexed => mesh.vertex_count,
                };
                (id, corners / 3)
            })
            .collect(),
    );
}

type Owner = (
    &'static Mesh3d,
    Has<crate::car_batch::Batch>,
    Has<crate::OpponentPart>,
    Has<crate::MovingPart>,
    Has<crate::static_batch::Batch>,
    Has<crate::track_surface::SourcePart>,
);
const OWNERS: [&str; 6] = ["opponent", "car_batch", "player", "batch", "track", "other"];

/// Per window: triangles each view submits, by owner.
fn view_triangles(
    time: Res<Time<Real>>,
    mut elapsed: Local<f32>,
    cameras: Query<(Entity, &Camera, &VisibleEntities), With<Camera3d>>,
    lights: Query<&CascadesVisibleEntities>,
    owners: Query<Owner>,
) {
    *elapsed += time.delta_secs();
    if *elapsed < window_seconds() {
        return;
    }
    *elapsed = 0.;
    let Some(map) = TRIANGLES.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    let count = |entities: &[Entity]| {
        let mut sums = [0u64; 6];
        for entity in entities {
            let Ok((mesh, car_batch, opponent, player, batch, track)) = owners.get(*entity) else {
                continue;
            };
            let owner = if car_batch {
                1
            } else if opponent {
                0
            } else if player {
                2
            } else if batch {
                3
            } else if track {
                4
            } else {
                5
            };
            sums[owner] += u64::from(map.get(&mesh.id()).copied().unwrap_or(0));
        }
        sums
    };
    let line = |view: String, sums: [u64; 6]| {
        let fields: Vec<String> = OWNERS
            .iter()
            .zip(sums)
            .map(|(n, v)| format!("{n}={v}"))
            .collect();
        log_line!(
            "view_triangles view={view} total={} {}",
            sums.iter().sum::<u64>(),
            fields.join(" ")
        );
    };
    for (entity, camera, visible) in &cameras {
        if !camera.is_active {
            continue;
        }
        line(
            format!("camera{}", camera.order),
            count(visible.get(TypeId::of::<Mesh3d>())),
        );
        for light in &lights {
            if let Some(cascades) = light.entities.get(&entity) {
                for (i, cascade) in cascades.iter().enumerate() {
                    line(
                        format!("camera{}_cascade{i}", camera.order),
                        count(&cascade.entities),
                    );
                }
            }
        }
    }
}

fn report(
    time: Res<Time<Real>>,
    store: Res<DiagnosticsStore>,
    mut elapsed: Local<f32>,
    mut frames: Local<u32>,
    mut passes: Local<BTreeMap<String, Pass>>,
    mut seen: Local<BTreeMap<String, std::time::Instant>>,
) {
    for diagnostic in store.iter() {
        let path = diagnostic.path().as_str();
        let Some(rest) = path.strip_prefix("render/") else {
            continue;
        };
        let Some(field) = FIELDS.iter().position(|f| rest.ends_with(f)) else {
            continue;
        };
        // Every camera's pass of one name records its own measurement (the six
        // reflection faces and the main view each run main_opaque_pass_3d):
        // sum the ones added since the last frame, counting the frame once.
        let last = seen.get(path).copied();
        let new: Vec<_> = diagnostic
            .measurements()
            .filter(|m| last.is_none_or(|t| m.time > t))
            .collect();
        let Some(newest) = new.iter().map(|m| m.time).max() else {
            continue;
        };
        seen.insert(path.to_string(), newest);
        let name = rest[..rest.len() - FIELDS[field].len()].trim_end_matches('/');
        let pass = passes.entry(name.to_string()).or_default();
        pass.sums[field] += new.iter().map(|m| m.value).sum::<f64>();
        pass.frames[field] += 1;
    }
    *frames += 1;
    *elapsed += time.delta_secs();
    if *elapsed < window_seconds() {
        return;
    }
    let seconds = std::mem::take(&mut *elapsed);
    let frames = std::mem::take(&mut *frames);
    let passes = std::mem::take(&mut *passes);
    let mean = |p: &Pass, f: usize| p.sums[f] / f64::from(p.frames[f].max(1));
    let timed = passes.values().any(|p| p.frames[0] > 0);
    log_line!(
        "gpu_window version=1 seconds={seconds:.1} frames={frames} timestamps={timed}{}",
        crate::perf_test::stats_suffix()
    );
    let mut ranked: Vec<_> = passes.iter().collect();
    ranked.sort_by(|a, b| mean(b.1, 0).total_cmp(&mean(a.1, 0)));
    for (rank, (name, p)) in ranked.iter().enumerate() {
        log_line!(
            "gpu_pass rank={} gpu_ms={:.3} primitives={:.0} vertices={:.0} fragments={:.0} name={name}",
            rank + 1,
            mean(p, 0),
            mean(p, 1),
            mean(p, 2),
            mean(p, 3)
        );
    }
}

/// GPU time of each sun cascade: Bevy times its main passes but not the
/// shadow passes. Bevy's early shadow pass system is replaced by the same pass
/// with a pass span around each cascade (`shadow_camN_cI`), as Bevy times its
/// main passes. A span must open and close on one thread and before the
/// frame's query resolve: a span opened before Bevy's pass and closed after it
/// in another system first panicked the renderer (two pool threads, the F633
/// hang at exit), then, on the render thread, lost the device. The late shadow
/// pass only runs with occlusion culling, which grid-app does not use.
mod shadow_span {
    use bevy::core_pipeline::mip_generation::experimental::depth::early_downsample_depth;
    use bevy::core_pipeline::schedule::Core3d;
    use bevy::ecs::schedule::ScheduleCleanupPolicy;
    use bevy::pbr::{
        EARLY_SHADOW_PASS, LATE_SHADOW_PASS, Shadow, ShadowView, ViewLightEntities,
        early_prepass_build_indirect_parameters, per_view_shadow_pass,
    };
    use bevy::prelude::*;
    use bevy::render::camera::ExtractedCamera;
    use bevy::render::diagnostic::RecordDiagnostics;
    use bevy::render::render_phase::ViewBinnedRenderPhases;
    use bevy::render::render_resource::{RenderPassDescriptor, StoreOp};
    use bevy::render::renderer::{RenderContext, ViewQuery};
    use bevy::render::view::ExtractedView;

    pub fn install(render: &mut SubApp) {
        let removed = render.remove_systems_in_set(
            Core3d,
            per_view_shadow_pass::<EARLY_SHADOW_PASS>,
            ScheduleCleanupPolicy::RemoveSystemsOnly,
        );
        if !matches!(removed, Ok(1)) {
            log_line!("gpu_shadow_span installed=false removed={removed:?}");
            return;
        }
        render.add_systems(
            Core3d,
            timed_shadow_pass
                .after(early_prepass_build_indirect_parameters)
                .before(early_downsample_depth)
                .before(per_view_shadow_pass::<LATE_SHADOW_PASS>),
        );
    }

    /// Bevy's `per_view_shadow_pass::<EARLY_SHADOW_PASS>` with a pass span.
    fn timed_shadow_pass(
        world: &World,
        view: ViewQuery<(&ViewLightEntities, &ExtractedCamera)>,
        view_lights: Query<(&ShadowView, &ExtractedView)>,
        phases: Res<ViewBinnedRenderPhases<Shadow>>,
        mut ctx: RenderContext,
    ) {
        let (lights, camera) = view.into_inner();
        let diagnostics = ctx.diagnostic_recorder();
        let diagnostics = diagnostics.as_deref();
        for (i, &light) in lights.lights.iter().enumerate() {
            let Ok((shadow_view, extracted)) = view_lights.get(light) else {
                continue;
            };
            let Some(phase) = phases.get(&extracted.retained_view_entity) else {
                continue;
            };
            let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(&shadow_view.pass_name),
                color_attachments: &[],
                depth_stencil_attachment: Some(
                    shadow_view.depth_attachment.get_attachment(StoreOp::Store),
                ),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let span = diagnostics.pass_span(&mut pass, format!("shadow_cam{}_c{i}", camera.order));
            if let Err(err) = phase.render(&mut pass, world, light) {
                error!("Error encountered while rendering the shadow phase {err:?}");
            }
            span.end(&mut pass);
        }
    }
}

// Opt-in CPU observer. No GPU query/readback, geometry or phase-key changes.
mod submissions {
    use super::*;
    use bevy::{
        core_pipeline::core_3d::{AlphaMask3d, Opaque3d},
        ecs::{
            query::ROQueryItem,
            system::{ReadOnlySystemParam, SystemParamItem, lifetimeless::SRes},
        },
        pbr::{
            DrawDepthOnlyPrepass, DrawMaterial, DrawMesh, DrawPrepass, RenderMaterialInstances,
            SetMaterialBindGroup, SetMeshBindGroup, SetMeshViewBindGroup,
            SetMeshViewBindingArrayBindGroup, SetPrepassEmptyMaterialBindGroup,
            SetPrepassViewBindGroup, SetPrepassViewEmptyBindGroup, Shadow,
        },
        render::{
            RenderStartup,
            render_phase::{
                BinnedPhaseItem, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
                RenderCommandResult, RenderCommandState, SetItemPipeline, TrackedRenderPass,
                ViewBinnedRenderPhases,
            },
        },
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    const MAX_ROWS: usize = 256;
    const MAX_VIEWS: usize = 64;
    const MAX_MEMBERSHIPS: usize = 131_072;
    const KINDS: [&str; 4] = ["raw", "standard", "other", "unknown"];
    pub(super) fn budget() -> Option<u32> {
        std::env::var("GRID_OBJECT_SUBMISSIONS")
            .ok()?
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=360).contains(n))
    }
    fn sampled(frame: u32, limit: u32) -> bool {
        frame == 1 || frame.is_multiple_of(30) || frame == limit
    }
    #[derive(Resource)]
    struct Observer {
        active: AtomicBool,
        done: AtomicBool,
        limit: u32,
        state: Mutex<Stats>,
    }
    #[derive(Default)]
    struct Stats {
        frame: u32,
        // Render-world view ID, phase kind, material kind. Values: direct API,
        // indirect API, indirect capacity, terminal Skip, terminal Failure.
        rows: HashMap<(Entity, u8, usize), [u64; 5]>,
        overflow: u64,
    }
    #[derive(Resource)]
    struct MainBudget {
        frame: u32,
        limit: u32,
    }
    pub(super) fn install(app: &mut App, limit: u32) {
        app.insert_resource(MainBudget { frame: 0, limit })
            .add_systems(Last, visibility);
        let Some(render) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render
            .insert_resource(Observer {
                active: AtomicBool::new(false),
                done: AtomicBool::new(false),
                limit,
                state: Mutex::new(Stats::default()),
            })
            .add_systems(RenderStartup, wrap_all)
            .add_systems(
                Render,
                begin
                    .after(RenderSystems::PrepareBindGroups)
                    .before(RenderSystems::Render),
            )
            .add_systems(
                Render,
                finish
                    .after(RenderSystems::Render)
                    .before(RenderSystems::Cleanup),
            );
    }
    type CountMaterial = (
        SetItemPipeline,
        SetMeshViewBindGroup<0>,
        SetMeshViewBindingArrayBindGroup<1>,
        SetMeshBindGroup<2>,
        SetMaterialBindGroup<3>,
        CountMesh,
    );
    type CountPrepass = (
        SetItemPipeline,
        SetPrepassViewBindGroup<0>,
        SetPrepassViewEmptyBindGroup<1>,
        SetMeshBindGroup<2>,
        SetMaterialBindGroup<3>,
        CountMesh,
    );
    type CountDepthOnly = (
        SetItemPipeline,
        SetPrepassViewBindGroup<0>,
        SetPrepassViewEmptyBindGroup<1>,
        SetMeshBindGroup<2>,
        SetPrepassEmptyMaterialBindGroup<3>,
        CountMesh,
    );
    fn replace<P, Original, Counted>(world: &mut World)
    where
        P: PhaseItem,
        Original: 'static,
        Counted: RenderCommand<P> + Send + Sync + 'static,
        Counted::Param: ReadOnlySystemParam,
    {
        let replacement = RenderCommandState::<P, Counted>::new(world);
        let functions = world.resource::<DrawFunctions<P>>();
        let mut functions = functions.write();
        let id = functions
            .get_id::<Original>()
            .expect("submission observer missing stock draw function");
        // Keep the original indices map AND draw-function ID; no phase rerouting.
        functions.draw_functions[id.0 as usize] = Box::new(replacement);
    }
    fn wrap_all(world: &mut World) {
        replace::<Opaque3d, DrawMaterial, CountMaterial>(world);
        replace::<AlphaMask3d, DrawMaterial, CountMaterial>(world);
        replace::<Shadow, DrawPrepass, CountPrepass>(world);
        replace::<Shadow, DrawDepthOnlyPrepass, CountDepthOnly>(world);
    }
    struct CountMesh;
    impl<P: PhaseItem> RenderCommand<P> for CountMesh {
        type Param = (
            <DrawMesh as RenderCommand<P>>::Param,
            SRes<Observer>,
            SRes<RenderMaterialInstances>,
        );
        type ViewQuery = (<DrawMesh as RenderCommand<P>>::ViewQuery, Entity);
        type ItemQuery = <DrawMesh as RenderCommand<P>>::ItemQuery;
        fn render<'w>(
            item: &P,
            (view, view_entity): ROQueryItem<'w, '_, Self::ViewQuery>,
            entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
            (params, observer, materials): SystemParamItem<'w, '_, Self::Param>,
            pass: &mut TrackedRenderPass<'w>,
        ) -> RenderCommandResult {
            let active = observer.active.load(Ordering::Relaxed);
            let kind = if active {
                materials.instances.get(&item.main_entity()).map_or(3, |m| {
                    let id = m.asset_id.type_id();
                    if id == TypeId::of::<crate::foundation_backing::RawMaterial>() {
                        0
                    } else if id == TypeId::of::<StandardMaterial>() {
                        1
                    } else {
                        2
                    }
                })
            } else {
                3
            };
            // Delegate exactly once, preserving every GPU command and original result.
            let result = <DrawMesh as RenderCommand<P>>::render(item, view, entity, params, pass);
            if active {
                let phase = if TypeId::of::<P>() == TypeId::of::<Opaque3d>() {
                    0
                } else if TypeId::of::<P>() == TypeId::of::<AlphaMask3d>() {
                    1
                } else {
                    2
                };
                let key = (view_entity, phase, kind);
                let mut state = observer.state.lock().expect("submission observer poisoned");
                if state.rows.len() == MAX_ROWS && !state.rows.contains_key(&key) {
                    state.overflow += 1;
                } else {
                    let row = state.rows.entry(key).or_default();
                    match &result {
                        RenderCommandResult::Skip => row[3] += 1,
                        RenderCommandResult::Failure(_) => row[4] += 1,
                        RenderCommandResult::Success => match item.extra_index() {
                            PhaseItemExtraIndex::IndirectParametersIndex { range, .. } => {
                                row[1] += 1;
                                row[2] += u64::from(range.end.saturating_sub(range.start));
                            }
                            _ => row[0] += 1,
                        },
                    }
                }
            }
            result
        }
    }
    fn queue_keys<P: BinnedPhaseItem>(
        phases: Option<&ViewBinnedRenderPhases<P>>,
        frame: u32,
        phase: &str,
    ) {
        let Some(phases) = phases else {
            log_line!(
                "object_queue sample={frame} phase={phase} state=unknown reason=resource-absent"
            );
            return;
        };
        if phases.0.len() > MAX_VIEWS {
            log_line!("object_queue sample={frame} phase={phase} state=unknown reason=view-cap");
            return;
        }
        for (view, p) in &phases.0 {
            let ordinary_members: usize = p
                .batchable_meshes
                .values()
                .map(|b| b.entities().len())
                .sum();
            let unbatchable: usize = p
                .unbatchable_meshes
                .values()
                .map(|b| b.entities.len())
                .sum();
            log_line!(
                "object_queue sample={frame} view={view:?} phase={phase} multidraw_set_keys={} batchable_bins={} batchable_members={ordinary_members} unbatchable_members={unbatchable} multidraw_members=unknown prepared_batches=unknown material_split=unknown",
                p.multidrawable_meshes.len(),
                p.batchable_meshes.len()
            );
        }
    }
    fn begin(
        observer: Res<Observer>,
        opaque: Option<Res<ViewBinnedRenderPhases<Opaque3d>>>,
        mask: Option<Res<ViewBinnedRenderPhases<AlphaMask3d>>>,
        shadows: Option<Res<ViewBinnedRenderPhases<Shadow>>>,
    ) {
        if observer.done.load(Ordering::Relaxed) {
            return;
        }
        let mut state = observer.state.lock().expect("submission observer poisoned");
        state.frame += 1;
        let frame = state.frame;
        state.rows.clear();
        state.overflow = 0;
        let active = sampled(frame, observer.limit);
        observer.active.store(active, Ordering::Relaxed);
        drop(state);
        if active {
            queue_keys(opaque.as_deref(), frame, "opaque");
            queue_keys(mask.as_deref(), frame, "mask");
            queue_keys(shadows.as_deref(), frame, "shadow");
        }
    }
    fn finish(observer: Res<Observer>) {
        if !observer.active.swap(false, Ordering::Relaxed) {
            return;
        }
        let mut state = observer.state.lock().expect("submission observer poisoned");
        let frame = state.frame;
        for ((view, phase, kind), row) in state.rows.drain() {
            log_line!(
                "object_commands sample={frame} view_space=render view={view:?} phase={phase} representative_material={} direct_cpu_calls={} indirect_cpu_calls={} indirect_capacity={} terminal_skip={} terminal_failure={} gpu_executed_commands=unknown ornament_ownership=unknown",
                KINDS[kind],
                row[0],
                row[1],
                row[2],
                row[3],
                row[4]
            );
        }
        log_line!(
            "object_commands sample={} dropped_counter_updates={} join_to_main_visibility=none",
            state.frame,
            state.overflow
        );
        if state.frame == observer.limit {
            observer.done.store(true, Ordering::Relaxed);
        }
    }
    type Membership = (
        Has<MeshMaterial3d<crate::foundation_backing::RawMaterial>>,
        Has<MeshMaterial3d<StandardMaterial>>,
        Has<crate::ornament_lod::OrnamentLevel>,
    );
    fn visibility(
        mut budget: ResMut<MainBudget>,
        cameras: Query<(Entity, &Camera, &VisibleEntities), With<Camera3d>>,
        lights: Query<&CascadesVisibleEntities>,
        owners: Query<Membership, With<Mesh3d>>,
    ) {
        if budget.frame == budget.limit {
            return;
        }
        budget.frame += 1;
        if !sampled(budget.frame, budget.limit) {
            return;
        }
        let (mut views, mut visits) = (0usize, 0usize);
        let mut emit = |camera: Entity, cascade: Option<usize>, entities: &[Entity]| {
            views += 1;
            if views > MAX_VIEWS {
                return;
            }
            let mut counts = [0u64; 4];
            let mut ornaments = 0u64;
            for entity in entities {
                visits += 1;
                if visits > MAX_MEMBERSHIPS {
                    log_line!(
                        "object_visibility sample={} state=unknown reason=membership-cap",
                        budget.frame
                    );
                    return;
                }
                match owners.get(*entity) {
                    Ok((raw, standard, ornament)) => {
                        let k = match (raw, standard) {
                            (true, false) => 0,
                            (false, true) => 1,
                            (false, false) => 2,
                            _ => 3,
                        };
                        counts[k] += 1;
                        ornaments += u64::from(ornament);
                    }
                    Err(_) => counts[3] += 1,
                }
            }
            log_line!(
                "object_visibility sample={} view_space=main camera={camera:?} cascade={cascade:?} raw={} standard={} other={} unknown={} ornament_members={ornaments} metric=cpu-membership join_to_render=none",
                budget.frame,
                counts[0],
                counts[1],
                counts[2],
                counts[3]
            );
        };
        for (camera, settings, visible) in &cameras {
            if !settings.is_active {
                continue;
            }
            emit(camera, None, visible.get(TypeId::of::<Mesh3d>()));
            for light in &lights {
                if let Some(cascades) = light.entities.get(&camera) {
                    for (i, c) in cascades.iter().enumerate() {
                        emit(camera, Some(i), &c.entities);
                    }
                }
            }
        }
        if views > MAX_VIEWS {
            log_line!(
                "object_visibility sample={} state=unknown reason=view-cap",
                budget.frame
            );
        }
    }
}
