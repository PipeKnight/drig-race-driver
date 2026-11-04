//! Bounded positive road-family pipeline witness for the requested projector trial.
//! Native typed GPU decal upload/draw execution is inferred, never read back.
use super::{ADAPTER_MARKER, IMPORT_PATH, SOURCE_PROJECTOR_TAG, sha};
use bevy::{
    core_pipeline::core_3d::Opaque3d,
    diagnostic::FrameCount,
    pbr::MaterialExtension,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderSystems,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{CachedPipelineState, CachedRenderPipelineId, PipelineCache},
        sync_world::{MainEntity, RenderEntity},
        view::ExtractedView,
    },
    shader::{ShaderDefVal, ShaderImport, ShaderRef, Source},
};
use std::collections::{HashMap, HashSet};
#[derive(Clone, PartialEq, Eq, Debug)]
struct Pair {
    ids: [AssetId<Shader>; 2],
    hashes: [String; 2],
}
#[derive(Clone, Copy)]
struct Candidate {
    pipeline: CachedRenderPipelineId,
    view: Entity,
    main: Option<MainEntity>,
    strength: &'static str,
}
#[derive(Resource, Default)]
pub(super) struct State {
    pair: Option<Pair>,
    modified: [bool; 2],
    frame: u32,
    roads: HashSet<MainEntity>,
    views: Vec<Entity>,
    pending: HashSet<CachedRenderPipelineId>,
    current: Vec<Candidate>,
    inserted: Vec<(Entity, usize)>,
    emitted: bool,
}
pub(super) fn install(render: &mut bevy::app::SubApp) {
    render
        .init_resource::<State>()
        .add_systems(
            ExtractSchedule,
            extract_context.before(super::extract_source_projectors),
        )
        .add_systems(
            Render,
            before_render.in_set(RenderSystems::PrepareBindGroups),
        )
        .add_systems(
            Render,
            after_render
                .in_set(RenderSystems::Cleanup)
                .after(super::refuse_pipeline_errors),
        );
}
fn invalidate_context(state: &mut State) {
    state.pair = None;
    state.modified = [false; 2];
    state.pending.clear();
    state.roads.clear();
    state.views.clear();
}
/// Called immediately after actual insert_decal and public get(entity) succeeds.
pub(super) fn inserted(s: &mut State, entity: Entity, index: usize, tag: u32, intensity: f32) {
    assert_eq!(tag, SOURCE_PROJECTOR_TAG);
    if intensity > 0. {
        s.inserted.push((entity, index));
    }
}
type RoadContextQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ViewVisibility),
    With<MeshMaterial3d<crate::track_surface::SurfaceMaterial>>,
>;
fn extract_context(
    assets: Extract<Res<Assets<Shader>>>,
    mut events: Extract<MessageReader<AssetEvent<Shader>>>,
    frame: Extract<Res<FrameCount>>,
    roads: Extract<RoadContextQuery<'_, '_>>,
    views: Extract<Query<RenderEntity, With<crate::ViewerCamera>>>,
    mut state: ResMut<State>,
) {
    state.frame = frame.0;
    state.inserted.clear();
    state.current.clear();
    if state.emitted {
        return;
    }
    let changed: HashSet<_> = events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    let mut selected = Vec::new();
    for path in [IMPORT_PATH, "bevy_pbr::pbr_functions"] {
        let matching: Vec<_> = assets
            .iter()
            .filter(|(_, s)| s.import_path == ShaderImport::Custom(path.into()))
            .collect();
        assert!(matching.len() <= 1, "duplicate witness import");
        let Some((id, shader)) = matching.first() else {
            invalidate_context(&mut state);
            return;
        };
        let Source::Wgsl(source) = &shader.source else {
            panic!("witness import is not WGSL");
        };
        if !source.contains(ADAPTER_MARKER) {
            invalidate_context(&mut state);
            return;
        }
        selected.push((*id, sha(source)));
    }
    let pair = Pair {
        ids: [selected[0].0, selected[1].0],
        hashes: [selected[0].1.clone(), selected[1].1.clone()],
    };
    if state.pair.as_ref() != Some(&pair) {
        state.pair = Some(pair.clone());
        state.modified = [false; 2];
        state.pending.clear();
    }
    for (i, id) in pair.ids.iter().enumerate() {
        state.modified[i] |= changed.contains(id);
    }
    state.roads = roads
        .iter()
        .filter_map(|(e, v)| v.get().then_some(MainEntity::from(e)))
        .collect();
    state.views = views.iter().collect();
}
fn before_render(
    phases: Res<ViewBinnedRenderPhases<Opaque3d>>,
    views: Query<&ExtractedView>,
    cache: Res<PipelineCache>,
    mut state: ResMut<State>,
) {
    if state.emitted || state.modified != [true; 2] || state.roads.is_empty() {
        return;
    }
    let mut candidates = HashMap::new();
    for entity in &state.views {
        let Ok(view) = views.get(*entity) else {
            continue;
        };
        let Some(phase) = phases.0.get(&view.retained_view_entity) else {
            continue;
        };
        // Strict public entity-bin association when these phase modes exist.
        for ((key, _), bin) in &phase.batchable_meshes {
            if let Some(main) = bin.entities().keys().find(|e| state.roads.contains(e)) {
                candidates.insert(
                    key.pipeline,
                    Candidate {
                        pipeline: key.pipeline,
                        view: *entity,
                        main: Some(*main),
                        strength: "exact-road-entity-bin",
                    },
                );
            }
        }
        for ((key, _), bin) in &phase.unbatchable_meshes {
            if let Some(main) = bin.entities.keys().find(|e| state.roads.contains(e)) {
                candidates.insert(
                    key.pipeline,
                    Candidate {
                        pipeline: key.pipeline,
                        view: *entity,
                        main: Some(*main),
                        strength: "exact-road-entity-bin",
                    },
                );
            }
        }
        // Multidraw membership is private. Require exact Surface fragment handle
        // later: this is road-FAMILY phase participation, not an entity-bin claim.
        for key in phase.multidrawable_meshes.keys() {
            candidates.entry(key.pipeline).or_insert(Candidate {
                pipeline: key.pipeline,
                view: *entity,
                main: None,
                strength: "road-family-queued-multidraw",
            });
        }
    }
    state.current = candidates.into_values().collect();
    for c in state.current.clone() {
        match cache.get_render_pipeline_state(c.pipeline) {
            CachedPipelineState::Queued | CachedPipelineState::Creating(_) => {
                state.pending.insert(c.pipeline);
            }
            CachedPipelineState::Err(error) if super::shader_is_pending(error) => {
                state.pending.insert(c.pipeline);
            }
            CachedPipelineState::Err(error) => {
                panic!("source road candidate pipeline error: {error:?}")
            }
            CachedPipelineState::Ok(_) => {}
        }
    }
    // Native process_pipeline_queue_system runs later in RenderSystems::Render.
    // No global cache-quiet heuristic and no manual set_shader/global-def changes.
}
fn after_render(cache: Res<PipelineCache>, mut state: ResMut<State>) {
    if state.emitted || state.modified != [true; 2] || state.inserted.is_empty() {
        return;
    }
    let ShaderRef::Handle(surface_shader) =
        <crate::track_surface::Surface as MaterialExtension>::fragment_shader()
    else {
        panic!("source road shader reference changed");
    };
    for candidate in state.current.clone() {
        if !state.pending.contains(&candidate.pipeline)
            || cache.get_render_pipeline(candidate.pipeline).is_none()
        {
            continue;
        }
        if !matches!(
            cache.get_render_pipeline_state(candidate.pipeline),
            CachedPipelineState::Ok(_)
        ) {
            continue;
        }
        // Descriptor lookup is safe NOW: newly queued IDs may not be in the
        // internal vector before process_queue, so no early descriptor indexing.
        let descriptor = cache.get_render_pipeline_descriptor(candidate.pipeline);
        let Some(fragment) = &descriptor.fragment else {
            continue;
        };
        if fragment.shader.id() != surface_shader.id()
            || !fragment
                .shader_defs
                .iter()
                .any(|d| matches!(d,ShaderDefVal::Bool(name,true) if name=="LIGHT_TEXTURES"))
        {
            continue;
        }
        let pair = state
            .pair
            .as_ref()
            .expect("both patched import records present");
        let (light, index) = state.inserted[0];
        println!(
            "source_projector_compiled_road version=1 frame={} view={:?} pipeline={} label={:?} fragment_shader={:?} fragment_defs={:?} road_main={:?} membership={} visible_surface_context={} lighting_asset={:?} lighting_sha={} functions_asset={:?} functions_sha={} both_modified_observed=true pending_after_mutation=true pipeline_ok_after_render=true source_entity={:?} decal_index={} source_tag={} public_insertion_map=true typed_gpu_buffer=INFERRED gpu_draw_execution=UNREADBACK",
            state.frame,
            candidate.view,
            candidate.pipeline.id(),
            descriptor.label,
            fragment.shader.id(),
            fragment.shader_defs,
            candidate.main,
            candidate.strength,
            state.roads.len(),
            pair.ids[0],
            pair.hashes[0],
            pair.ids[1],
            pair.hashes[1],
            light,
            index,
            SOURCE_PROJECTOR_TAG
        );
        state.emitted = true;
        break;
    }
}
