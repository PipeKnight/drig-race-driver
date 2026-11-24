//! Base RGBA filtering for native DLAA glazing with stock fallback.
#[path = "native_glazing_pipeline.rs"]
mod pipeline;
use bevy::{
    anti_alias::dlss::{Dlss, DlssPerfQualityMode, DlssSuperResolutionSupported},
    asset::{AssetEventSystems, AssetId, UntypedAssetId},
    camera::{MainPassResolutionOverride, visibility::RenderLayers},
    core_pipeline::{core_3d::Transparent3d, prepass::ViewPrepassTextures},
    diagnostic::FrameCount,
    pbr::{
        Lightmap, MaterialBindGroupAllocators, MaterialBindingId, MaterialExtractionSystems,
        MeshExtractionSystems, MeshesToReextractNextFrame, PreparedMaterial,
        RenderMaterialBindings, RenderMaterialInstances, RenderMeshInstanceGpuQueue,
        RenderMeshInstanceGpuQueues, RenderMeshInstances, SpecializedMaterialPipelineCache,
        collect_meshes_for_gpu_building, late_sweep_material_instances, prepare_morph_descriptors,
    },
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        camera::{DirtySpecializationSystems, DirtySpecializations},
        erased_render_asset::ErasedRenderAssets,
        render_phase::{PhaseItem, ViewSortedRenderPhases},
        render_resource::CachedRenderPipelineId,
        sync_world::{MainEntity, RenderEntity},
        view::{ExtractedView, RenderVisibleEntities, RetainedViewEntity},
    },
};
use std::collections::HashMap;

use crate::{
    native_dlaa::{DlaaView, State as DlaaState},
    window_response::{Mode, Window, WindowLayer},
};

#[derive(Component)]
pub(super) struct Pane;

#[derive(Component)]
struct FilteredPane {
    original: AssetId<StandardMaterial>,
    filtered: Handle<Window>,
}

#[derive(Resource)]
pub(super) struct Sampling {
    zero: bool,
    variants: HashMap<AssetId<StandardMaterial>, Handle<Window>>,
}

pub(super) fn is_window(label: &str) -> bool {
    crate::part_model(label).is_some()
        && label
            .split_once('/')
            .is_some_and(|(_, material)| material == "Window Glass")
}

pub(super) fn install(
    app: &mut App,
    settings: &crate::graphics::Settings,
) -> Result<(), Box<dyn std::error::Error>> {
    let native_requested = settings.anti_aliasing == 3 && settings.window_pilot == Mode::Off;
    let zero = match crate::dev::var("GRID_DLAA_GLASS_FILTER").as_deref() {
        Ok("stock") => return Ok(()),
        Err(std::env::VarError::NotPresent) if native_requested => true,
        Err(std::env::VarError::NotPresent) => return Ok(()),
        Ok("control") => false,
        Ok("zero") => true,
        _ => return Err("GRID_DLAA_GLASS_FILTER must be stock, control or zero".into()),
    };
    if !native_requested {
        return Err("glass filter requires native DLAA and the ordinary window material".into());
    }
    crate::window_response::register_material(
        app,
        if zero {
            Mode::BaseMipZero
        } else {
            Mode::BaseMipControl
        },
    )?;
    app.insert_resource(Sampling {
        zero,
        variants: HashMap::new(),
    })
    .add_systems(PostUpdate, sync.after(AssetEventSystems));
    app.get_sub_app_mut(RenderApp)
        .ok_or("glass filter requires a render app")?
        .insert_resource(Staged {
            trace: crate::dev::var("GRID_PIPELINE_TRACE").as_deref() == Ok("1"),
            ..default()
        })
        .add_systems(
            ExtractSchedule,
            stage_panes
                .in_set(DirtySpecializationSystems::CheckForChanges)
                .after(MaterialExtractionSystems)
                .before(MeshExtractionSystems)
                .before(late_sweep_material_instances),
        )
        .add_systems(
            Render,
            choose_material
                .in_set(RenderSystems::PrepareMeshes)
                .after(prepare_morph_descriptors)
                .before(collect_meshes_for_gpu_building),
        )
        .add_systems(
            Render,
            trace_queued_panes
                .after(RenderSystems::QueueMeshes)
                .before(RenderSystems::QueueSweep),
        );
    Ok(())
}

type Panes<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MeshMaterial3d<StandardMaterial>,
        Option<&'static FilteredPane>,
    ),
    (With<Pane>, With<Mesh3d>),
>;

fn sync(
    mut commands: Commands,
    supported: Option<Res<DlssSuperResolutionSupported>>,
    panes: Panes,
    source: Res<Assets<StandardMaterial>>,
    mut source_events: MessageReader<AssetEvent<StandardMaterial>>,
    mut variants: ResMut<Assets<Window>>,
    mut sampling: ResMut<Sampling>,
) {
    if supported.is_none() {
        return;
    }
    // An existing prepared handle may still describe the previous source.
    // Give each source revision its own immutable renderer asset identity.
    for event in source_events.read() {
        if let AssetEvent::Modified { id }
        | AssetEvent::Removed { id }
        | AssetEvent::Unused { id } = event
        {
            sampling.variants.remove(id);
        }
    }
    let zero = sampling.zero;
    for (entity, original, prior) in &panes {
        let id = original.id();
        let Some(base) = source.get(id).filter(|base| {
            base.alpha_mode == AlphaMode::Blend && base.base_color_texture.is_some()
        }) else {
            if prior.is_some() {
                commands.entity(entity).remove::<FilteredPane>();
            }
            continue;
        };
        let filtered = sampling.variants.entry(id).or_insert_with(|| {
            variants.add(Window {
                base: base.clone(),
                extension: WindowLayer::native_base_mip(zero),
            })
        });
        if prior.is_none_or(|prior| prior.original != id || prior.filtered.id() != filtered.id()) {
            commands.entity(entity).insert(FilteredPane {
                original: id,
                filtered: filtered.clone(),
            });
        }
    }
}

type PaneSource<'a> = (
    Entity,
    &'a ViewVisibility,
    &'a GlobalTransform,
    &'a Mesh3d,
    &'a MeshMaterial3d<StandardMaterial>,
    Option<&'a FilteredPane>,
    Option<&'a RenderLayers>,
    Has<Lightmap>,
);

type VisiblePanes<'w, 's> = Query<'w, 's, PaneSource<'static>, (With<Pane>, With<Mesh3d>)>;

type Cameras<'w, 's> = Query<
    'w,
    's,
    (
        &'static Camera,
        Option<&'static Projection>,
        Option<&'static Dlss>,
        Option<&'static RenderEntity>,
        Option<&'static RenderLayers>,
        Has<DlaaView>,
        Has<bevy::core_pipeline::prepass::DepthPrepass>,
        Has<bevy::core_pipeline::prepass::MotionVectorPrepass>,
    ),
    With<Camera3d>,
>;

type NativeViews<'w, 's> = Query<
    'w,
    's,
    (
        &'static Dlss,
        &'static MainPassResolutionOverride,
        &'static ViewPrepassTextures,
        &'static ExtractedView,
    ),
    With<DlaaView>,
>;

type SelectionInput<'w, 's> = (
    Extract<'w, 's, VisiblePanes<'static, 'static>>,
    Extract<'w, 's, Cameras<'static, 'static>>,
    Extract<'w, 's, Option<Res<'static, DlaaState>>>,
    Extract<'w, 's, Res<'static, FrameCount>>,
);

type PreparedBindings<'w> = (
    Res<'w, ErasedRenderAssets<PreparedMaterial>>,
    Res<'w, RenderMaterialBindings>,
    Res<'w, MaterialBindGroupAllocators>,
);

fn same_binding(a: MaterialBindingId, b: MaterialBindingId) -> bool {
    a.group == b.group && a.slot == b.slot
}

fn ready_binding(
    material: UntypedAssetId,
    prepared: &ErasedRenderAssets<PreparedMaterial>,
    bindings: &RenderMaterialBindings,
    allocators: &MaterialBindGroupAllocators,
) -> Option<MaterialBindingId> {
    let binding = prepared.get(material)?.binding;
    if !bindings
        .get(&material)
        .is_some_and(|current| same_binding(*current, binding))
    {
        return None;
    }
    allocators
        .get(&material.type_id())?
        .get(binding.group)?
        .bind_group()?;
    Some(binding)
}

#[derive(Resource, Default)]
struct Staged {
    proposals: HashMap<MainEntity, Proposal>,
    applied: HashMap<MainEntity, CachedRenderPipelineId>,
    trace: bool,
    witnesses: Vec<Witness>,
    reported: bool,
}

struct Proposal {
    source: AssetId<StandardMaterial>,
    alternate: Option<pipeline::PanePipeline>,
    gpu_inputs: u8,
}

struct Witness {
    owner: MainEntity,
    proposal: pipeline::PanePipeline,
    source: AssetId<StandardMaterial>,
    readiness: pipeline::Readiness,
    selected: UntypedAssetId,
    required_pipeline: Option<CachedRenderPipelineId>,
    returned_from: Option<CachedRenderPipelineId>,
    expected_binding: Option<MaterialBindingId>,
    gpu_inputs: u8,
}

fn stage_panes(
    (panes, cameras, state, frame): SelectionInput,
    native_views: NativeViews,
    prepared: PreparedBindings,
    meshes: Res<RenderMeshInstances>,
    mut reextract: Option<ResMut<MeshesToReextractNextFrame>>,
    (mut instances, mut dirty): (
        ResMut<RenderMaterialInstances>,
        ResMut<DirtySpecializations>,
    ),
    mut staged: ResMut<Staged>,
) {
    staged.proposals.clear();
    staged.witnesses.clear();
    staged.applied.retain(|owner, _| {
        if panes.contains(owner.id()) {
            true
        } else {
            // Gameplay may change the pane's material type or despawn it.
            dirty.changed_renderables.insert(*owner);
            false
        }
    });
    let mut native = Vec::new();
    let mut ordinary = Vec::new();
    let default_layers = RenderLayers::default();
    let native_ready = state
        .as_ref()
        .is_some_and(|state| state.ready() && !state.failed());
    for (camera, projection, dlss, render, layers, dlaa, depth, motion) in &cameras {
        if !camera.is_active {
            continue;
        }
        let layers = layers.unwrap_or(&default_layers);
        let request = render.and_then(|entity| {
            let (actual, input, prepass, view) = native_views.get(entity.id()).ok()?;
            (actual.perf_quality_mode == DlssPerfQualityMode::Dlaa
                && camera.physical_viewport_size() == Some(input.0)
                && prepass.depth.is_some()
                && prepass.motion_vectors.is_some())
            .then_some((entity.id(), view.retained_view_entity, input.0))
        });
        if native_ready
            && dlaa
            && depth
            && motion
            && projection.is_some_and(Projection::is_perspective)
            && dlss.is_some_and(|feature| feature.perf_quality_mode == DlssPerfQualityMode::Dlaa)
            && let Some((entity, retained, resolution)) = request
        {
            native.push((layers, entity, retained, resolution));
        } else {
            ordinary.push(layers);
        }
    }
    let (prepared, bindings, allocators) = &prepared;
    for (entity, visible, _, mesh, original, filter, layers, lightmap) in &panes {
        if !visible.get() {
            continue;
        }
        let owner = entity.into();
        let Some(instance) = instances.instances.get_mut(&owner) else {
            continue;
        };
        let source = original.id();
        let stock_id = source.untyped();
        let Some(queue) = &mut reextract else {
            // CPU mesh extraction reads the material immediately after us.
            if instance.asset_id != stock_id {
                instance.asset_id = stock_id;
                dirty.changed_renderables.insert(owner);
            }
            continue;
        };
        let layers = layers.unwrap_or(&default_layers);
        let mut relevant = native
            .iter()
            .filter(|(camera, ..)| layers.intersects(camera));
        let alternate = filter.and_then(|mirror| {
            let (_, view, retained, resolution) = *relevant.next()?;
            if lightmap
                || relevant.next().is_some()
                || ordinary.iter().any(|camera| layers.intersects(camera))
                || mirror.original != source
            {
                return None;
            }
            Some(pipeline::PanePipeline {
                frame: frame.0,
                owner,
                mesh: mesh.0.id(),
                material: mirror.filtered.id(),
                view,
                retained_view: retained,
                resolution,
                has_source_lightmap: lightmap,
            })
        });
        let stock_slot_is_stale = ready_binding(stock_id, prepared, bindings, allocators)
            .is_some_and(|expected| {
                meshes
                    .render_mesh_queue_data(owner)
                    .is_none_or(|row| !same_binding(row.shared.material_bindings_index(), expected))
            });
        if alternate.is_none()
            && instance.asset_id == stock_id
            && !stock_slot_is_stale
            && !staged.applied.contains_key(&owner)
        {
            continue;
        }
        // Before MeshExtractionSystems. The current draw may choose either
        // binding, so a live candidate always supplies a fresh GPU input.
        queue.insert(owner);
        staged.proposals.insert(
            owner,
            Proposal {
                source,
                alternate,
                gpu_inputs: 0,
            },
        );
    }
}

fn tally_gpu_inputs(world: &mut World, proposals: &mut HashMap<MainEntity, Proposal>) {
    let Some(mut queues) = world.get_resource_mut::<RenderMeshInstanceGpuQueues>() else {
        return;
    };
    let mut count = |owner: &MainEntity| {
        if let Some(proposal) = proposals.get_mut(owner) {
            proposal.gpu_inputs = proposal.gpu_inputs.saturating_add(1);
        }
    };
    for queue in queues.iter_mut() {
        match queue {
            RenderMeshInstanceGpuQueue::None => {}
            RenderMeshInstanceGpuQueue::CpuCulling { changed, .. } => {
                changed.iter().for_each(|(owner, _)| count(owner));
            }
            RenderMeshInstanceGpuQueue::GpuCulling {
                changed_cpu_culling,
                changed_gpu_culling,
                ..
            } => {
                changed_cpu_culling
                    .iter()
                    .chain(changed_gpu_culling.iter())
                    .for_each(|(owner, _, _)| count(owner));
            }
        }
    }
}

fn world_binding(world: &World, id: UntypedAssetId) -> Option<MaterialBindingId> {
    ready_binding(
        id,
        world.get_resource::<ErasedRenderAssets<PreparedMaterial>>()?,
        world.get_resource::<RenderMaterialBindings>()?,
        world.get_resource::<MaterialBindGroupAllocators>()?,
    )
}

fn choose_material(world: &mut World) {
    world.resource_scope(|world, mut staged: Mut<Staged>| {
        if staged.proposals.is_empty() {
            return;
        }
        tally_gpu_inputs(world, &mut staged.proposals);
        let trace = staged.trace;
        let mut filtered_panes = 0;
        let Staged { proposals, applied, witnesses, .. } = &mut *staged;
        for (owner, proposal) in proposals.drain() {
            let stock = proposal.source.untyped();
            let readiness = match proposal.alternate {
                Some(pane) if proposal.gpu_inputs == 1 => pipeline::certify(world, pane),
                _ => pipeline::Readiness::Stock,
            };
            let alternate = match (proposal.alternate, readiness) {
                (Some(pane), pipeline::Readiness::Ready(id)) => {
                    let asset = pane.material.untyped();
                    world_binding(world, asset).map(|binding| (asset, id, binding))
                }
                _ => None,
            };
            let selected = alternate.map_or(stock, |(asset, _, _)| asset);
            let expected = alternate.map(|(_, _, binding)| binding)
                .or_else(|| world_binding(world, stock));
            let stale_binding = expected.is_some_and(|binding| {
                world.resource::<RenderMeshInstances>().render_mesh_queue_data(owner)
                    .is_none_or(|row| !same_binding(row.shared.material_bindings_index(), binding))
            });
            let changed = {
                let mut instances = world.resource_mut::<RenderMaterialInstances>();
                let Some(instance) = instances.instances.get_mut(&owner) else {
                    continue;
                };
                let changed = instance.asset_id != selected;
                instance.asset_id = selected;
                changed
            };
            // The prepared key can change while the handle and slot stay the
            // same, for example when warmup removes a temporary crossfade.
            let stale_pipeline = alternate.is_some_and(|(_, required, _)| {
                proposal.alternate.is_some_and(|pane| {
                    world
                        .resource::<SpecializedMaterialPipelineCache>()
                        .get(&pane.retained_view)
                        .and_then(|entries| entries.get(&owner))
                        .is_some_and(|cached| *cached != required)
                })
            });
            // Material extraction can restore the stock ID before this
            // system sees it. Still invalidate the last applied alternate.
            let returned_from = match alternate {
                Some((_, pipeline, _)) => {
                    applied.insert(owner, pipeline);
                    None
                }
                None => applied.remove(&owner),
            };
            if changed || stale_binding || stale_pipeline || returned_from.is_some() {
                world.resource_mut::<DirtySpecializations>().changed_renderables.insert(owner);
            }
            filtered_panes += usize::from(alternate.is_some());
            if trace && let Some(pane) = proposal.alternate {
                witnesses.push(Witness {
                    owner, proposal: pane, source: proposal.source, readiness, selected,
                    required_pipeline: alternate.map(|(_, pipeline, _)| pipeline),
                    returned_from,
                    expected_binding: expected,
                    gpu_inputs: proposal.gpu_inputs,
                });
            }
        }
        if filtered_panes > 0 && !staged.reported {
            println!("native_glazing selected_visible_panes={filtered_panes} prepared_binding=true prepared_pipeline=true");
            staged.reported = true;
        }
    });
}

type QueueTrace<'w> = (
    Res<'w, RenderMeshInstances>,
    Res<'w, RenderMaterialInstances>,
    Res<'w, SpecializedMaterialPipelineCache>,
    Res<'w, ViewSortedRenderPhases<Transparent3d>>,
    Res<'w, bevy::render::render_resource::PipelineCache>,
);

#[derive(Debug, PartialEq, Eq)]
struct QueuedPane {
    source: AssetId<StandardMaterial>,
    alternate: AssetId<Window>,
    readiness: pipeline::Readiness,
    selected: UntypedAssetId,
    expected_pipeline: Option<CachedRenderPipelineId>,
    returned_from: Option<CachedRenderPipelineId>,
    actual_pipeline: Option<CachedRenderPipelineId>,
    queued_pipeline: Option<CachedRenderPipelineId>,
    queued_pipeline_ready: bool,
    per_view_visible: bool,
    expected_binding: Option<(u32, u32)>,
    actual_binding: Option<(u32, u32)>,
    binding_matches: bool,
    material_matches: bool,
    gpu_inputs: u8,
}

type PreviousQueueReceipts = HashMap<(RetainedViewEntity, MainEntity), QueuedPane>;

fn trace_queued_panes(
    frame: Res<FrameCount>,
    staged: Res<Staged>,
    (meshes, materials, specializations, transparent, cache): QueueTrace,
    views: Query<&RenderVisibleEntities>,
    mut prior: Local<PreviousQueueReceipts>,
) {
    if !staged.trace {
        return;
    }
    prior.retain(|key, _| {
        staged
            .witnesses
            .iter()
            .any(|witness| *key == (witness.proposal.retained_view, witness.owner))
    });
    for witness in &staged.witnesses {
        let view = witness.proposal.retained_view;
        let owner = witness.owner;
        let mesh = meshes.render_mesh_queue_data(owner);
        let actual_binding = mesh
            .as_ref()
            .map(|row| row.shared.material_bindings_index());
        let numbers = |binding: MaterialBindingId| (binding.group.0, binding.slot.0);
        let queued = transparent
            .get(&view)
            .and_then(|phase| {
                phase
                    .items
                    .values()
                    .find(|item| item.main_entity() == owner)
            })
            .map(|item| item.pipeline);
        let per_view_visible = views
            .get(witness.proposal.view)
            .ok()
            .and_then(|entities| entities.get::<Mesh3d>())
            .is_some_and(|mesh_entities| {
                mesh_entities.entities_gpu_culling.contains_key(&owner)
                    || mesh_entities
                        .entities_cpu_culling
                        .binary_search_by_key(&owner, |&(_, entity)| entity)
                        .is_ok()
            });
        let actual = QueuedPane {
            source: witness.source,
            alternate: witness.proposal.material,
            readiness: witness.readiness,
            selected: witness.selected,
            expected_pipeline: witness.required_pipeline,
            returned_from: witness.returned_from,
            actual_pipeline: specializations
                .get(&view)
                .and_then(|entries| entries.get(&owner))
                .copied(),
            queued_pipeline: queued,
            per_view_visible,
            queued_pipeline_ready: queued.is_some_and(|id| cache.get_render_pipeline(id).is_some()),
            expected_binding: witness.expected_binding.map(numbers),
            actual_binding: actual_binding.map(numbers),
            binding_matches: mesh.is_some_and(|row| row.mesh_asset_id() == witness.proposal.mesh)
                && witness
                    .expected_binding
                    .zip(actual_binding)
                    .is_some_and(|(expected, actual)| same_binding(expected, actual)),
            material_matches: materials
                .instances
                .get(&owner)
                .is_some_and(|row| row.asset_id == witness.selected),
            gpu_inputs: witness.gpu_inputs,
        };
        let key = (view, owner);
        if prior.get(&key) != Some(&actual) {
            let valid_alternate = actual.expected_pipeline.is_none_or(|expected| {
                actual.binding_matches
                    && actual.material_matches
                    && actual.gpu_inputs == 1
                    && if actual.per_view_visible {
                        actual.actual_pipeline == Some(expected)
                            && actual.queued_pipeline == Some(expected)
                            && actual.queued_pipeline_ready
                    } else {
                        actual.queued_pipeline.is_none()
                    }
            });
            let valid_fallback = actual.returned_from.is_none_or(|previous| {
                actual.material_matches
                    && actual.actual_pipeline != Some(previous)
                    && actual.queued_pipeline != Some(previous)
                    && (actual.expected_binding.is_none() || actual.binding_matches)
                    && (!actual.per_view_visible
                        || actual.queued_pipeline.is_none()
                        || actual.queued_pipeline_ready)
            });
            println!(
                "native_glazing queued frame={} owner={owner:?} view={view:?} valid_alternate={valid_alternate} valid_fallback={valid_fallback} {actual:?}",
                frame.0
            );
            prior.insert(key, actual);
        }
    }
}
