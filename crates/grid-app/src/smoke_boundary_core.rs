//! Independently authored grouped-direct draw transaction; bounded bridge installs it.
//! No material, shader, pipeline, mesh, source simulation or MSAA changes.
use bevy::{
    core_pipeline::core_3d::Transparent3d,
    ecs::{
        query::ROQueryItem,
        system::{SystemParamItem, lifetimeless::SRes},
    },
    pbr::DrawMaterial,
    prelude::*,
    render::{
        RenderApp,
        render_phase::{
            AddRenderCommand, DrawFunctionId, DrawFunctions, PhaseItem, RenderCommand,
            RenderCommandResult, SortedRenderPhase, TrackedRenderPass,
        },
        render_resource::{RenderPassDescriptor, StoreOp, TextureFormat},
        renderer::RenderContext,
        sync_world::MainEntity,
        view::{ExtractedView, RetainedViewEntity, ViewDepthTexture, ViewTarget},
    },
};
use std::{
    ops::Range,
    sync::{Arc, Mutex, MutexGuard},
};
type Key = (Entity, MainEntity);
const GROUPS: usize = 5;

/// Span metadata follows EXACT SortedRenderPhase::render_range skip rules.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub selected: bool,
    pub batch_len: usize,
}
pub(crate) fn plan_batches(entries: &[Entry]) -> Result<Vec<Range<usize>>, &'static str> {
    if entries.iter().filter(|e| e.selected).count() != GROUPS {
        return Err("need exactly five owned logical groups");
    }
    let mut covered = vec![false; entries.len()];
    let mut heads = Vec::new();
    let mut index = 0;
    while index < entries.len() {
        let len = entries[index].batch_len;
        if len == 0 {
            index += 1;
            continue;
        }
        let end = index
            .checked_add(len)
            .filter(|e| *e <= entries.len())
            .ok_or("batch span exceeds logical phase")?;
        let selected = entries[index].selected;
        if entries[index..end].iter().any(|e| e.selected != selected) {
            return Err("mixed selected/nonselected prepared batch");
        }
        if selected {
            covered[index..end].fill(true);
            heads.push(index..end);
        }
        index = end;
    }
    if entries
        .iter()
        .enumerate()
        .any(|(i, e)| e.selected != covered[i])
    {
        return Err("lost selected tail/head or unprepared selected group");
    }
    Ok(heads)
}

#[derive(Default)]
struct Marks {
    view: Option<RetainedViewEntity>,
    heads: Vec<(MainEntity, usize)>,
    seen: Vec<MainEntity>,
    open: bool,
    finished: bool,
    restored: bool,
    error: Option<&'static str>,
}
#[derive(Resource, Clone, Default)]
pub(crate) struct Proof(Arc<Mutex<Marks>>);
impl Proof {
    fn marks(&self) -> MutexGuard<'_, Marks> {
        self.0.lock().unwrap_or_else(|e| {
            let mut s = e.into_inner();
            s.error = Some("boundary proof poisoned");
            s
        })
    }
    pub(crate) fn failed(&self) -> bool {
        self.marks().error.is_some()
    }
    pub(crate) fn current_frame_ready(&self) -> bool {
        let s = self.marks();
        s.error.is_none() && s.finished && s.restored
    }
    /// Current replay only; caller must bind these counters to actual Extract/PNG stamp.
    pub(crate) fn successful_counts(&self) -> (usize, usize) {
        let state = self.marks();
        (
            state.seen.len(),
            state.heads.iter().take(state.seen.len()).map(|h| h.1).sum(),
        )
    }
    fn fail(&self, why: &'static str) {
        self.marks().error.get_or_insert(why);
    }
    fn begin(
        &self,
        view: RetainedViewEntity,
        heads: Vec<(MainEntity, usize)>,
    ) -> Result<(), &'static str> {
        let mut s = self.marks();
        if s.error.is_some() || s.open {
            return Err("old boundary error/open replay");
        }
        *s = Marks {
            view: Some(view),
            heads,
            ..default()
        };
        Ok(())
    }
    fn mark(
        &self,
        view: RetainedViewEntity,
        head: MainEntity,
        len: usize,
    ) -> Result<(), &'static str> {
        let mut s = self.marks();
        if !s.open || s.view != Some(view) || s.error.is_some() {
            s.error
                .get_or_insert("wrapper attempted outside owned replay");
            return Err("wrapper attempted outside owned replay");
        }
        if s.heads.get(s.seen.len()) != Some(&(head, len)) {
            s.error = Some("duplicate/reordered/wrong successful batch");
            return Err("duplicate/reordered/wrong successful batch");
        }
        s.seen.push(head);
        Ok(())
    }
    fn open_replay(&self) -> Result<(), &'static str> {
        let mut s = self.marks();
        if s.open || s.finished || s.view.is_none() || s.error.is_some() {
            s.error
                .get_or_insert("boundary duplicate/unprepared replay");
            return Err("boundary duplicate/unprepared replay");
        }
        s.open = true;
        Ok(())
    }
    fn finish(&self, draw_ok: bool) -> Result<(), &'static str> {
        let mut s = self.marks();
        s.open = false;
        if !draw_ok
            || s.seen.len() != s.heads.len()
            || s.heads.iter().map(|x| x.1).sum::<usize>() != GROUPS
            || s.error.is_some()
        {
            s.error
                .get_or_insert("group replay failed/skipped/incomplete");
            return Err("group replay failed/skipped/incomplete");
        }
        s.finished = true;
        Ok(())
    }
}
pub(crate) struct SuccessMarker;
impl RenderCommand<Transparent3d> for SuccessMarker {
    type Param = SRes<Proof>;
    type ViewQuery = &'static ExtractedView;
    type ItemQuery = ();
    fn render<'w>(
        item: &Transparent3d,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        proof: SystemParamItem<'w, '_, Self::Param>,
        _: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        match proof.into_inner().mark(
            view.retained_view_entity,
            item.main_entity(),
            item.batch_range.len(),
        ) {
            Ok(()) => RenderCommandResult::Success,
            Err(why) => RenderCommandResult::Failure(why),
        }
    }
}
// Nested tuple is intentional: upstream tuple RenderCommand short-circuits Skip.
pub(crate) type MarkedMaterial = (DrawMaterial, SuccessMarker);
#[derive(Clone, Copy)]
pub(crate) struct Ids {
    pub standard: DrawFunctionId,
    pub marked: DrawFunctionId,
}
/// No flag => return before RenderApp/registry/resources. On is NOT a complete pilot installation.
pub(crate) fn register(app: &mut App, enabled: bool) -> std::io::Result<Option<(Proof, Ids)>> {
    if !enabled {
        return Ok(None);
    }
    let render = app
        .get_sub_app_mut(RenderApp)
        .ok_or_else(|| std::io::Error::other("boundary needs renderer"))?;
    let standard = render
        .world()
        .get_resource::<DrawFunctions<Transparent3d>>()
        .ok_or_else(|| std::io::Error::other("Transparent3d draw registry absent"))?
        .read()
        .get_id::<DrawMaterial>()
        .ok_or_else(|| std::io::Error::other("standard DrawMaterial absent"))?;
    render.add_render_command::<Transparent3d, MarkedMaterial>();
    let marked = render
        .world()
        .get_resource::<DrawFunctions<Transparent3d>>()
        .ok_or_else(|| std::io::Error::other("Transparent3d draw registry absent"))?
        .read()
        .get_id::<MarkedMaterial>()
        .ok_or_else(|| std::io::Error::other("marked DrawMaterial absent"))?;
    let proof = Proof::default();
    render.insert_resource(proof.clone());
    app.insert_resource(proof.clone());
    Ok(Some((proof, Ids { standard, marked })))
}

struct Saved {
    key: Key,
    draw: DrawFunctionId,
    range: Option<Range<u32>>,
}
#[derive(Default)]
pub(crate) struct Frame {
    saved: Vec<Saved>,
    pub owned: SortedRenderPhase<Transparent3d>,
    prepared: bool,
}
impl Frame {
    pub(crate) fn clean(&self) -> bool {
        self.saved.is_empty() && self.owned.items.is_empty() && !self.prepared
    }
    // There is no retained phase left to restore. Fail permanently and release
    // owned CPU metadata; never manufacture missing retained entries.
    pub(crate) fn discard_missing_phase(&mut self, proof: &Proof) {
        proof.fail("boundary retained phase disappeared before restoration");
        self.saved.clear();
        self.owned.clear();
        self.prepared = false;
        let mut marks = proof.marks();
        marks.open = false;
        marks.restored = false;
    }
}
fn selected(main: MainEntity, groups: &[MainEntity]) -> bool {
    groups.contains(&main)
}
/// Caller: AFTER PhaseSort/all original G267 audit, BEFORE PrepareResourcesBatchPhases;
/// main-view only, five exact owned mesh IDs, source pipeline readiness prevalidated.
pub(crate) fn barrier(
    phase: &mut SortedRenderPhase<Transparent3d>,
    groups: &[MainEntity],
    ids: Ids,
    frame: &mut Frame,
) -> Result<(), &'static str> {
    if !frame.saved.is_empty() || frame.prepared {
        return Err("previous retained phase not restored");
    }
    if groups.len() != GROUPS
        || groups
            .iter()
            .enumerate()
            .any(|(i, e)| groups[..i].contains(e))
    {
        return Err("owned groups duplicate/wrong count");
    }
    let keys: Vec<_> = phase
        .items
        .iter()
        .filter(|(_, item)| selected(item.main_entity(), groups))
        .map(|(key, _)| *key)
        .collect();
    if keys.len() != GROUPS
        || keys
            .iter()
            .any(|k| phase.items[k].draw_function != ids.standard)
    {
        return Err("unsupported/missing standard owned draw");
    }
    // Validate ALL entries before first mutation. Pipeline IDs do not change.
    for key in keys {
        let item = phase
            .items
            .get_mut(&key)
            .ok_or("retained key disappeared")?;
        frame.saved.push(Saved {
            key,
            draw: item.draw_function,
            range: None,
        });
        item.draw_function = ids.marked;
    }
    Ok(())
}
fn copy_item(item: &Transparent3d) -> Transparent3d {
    Transparent3d {
        sorting_info: item.sorting_info,
        distance: item.distance,
        pipeline: item.pipeline,
        entity: item.entity,
        draw_function: item.draw_function,
        batch_range: item.batch_range.clone(),
        extra_index: item.extra_index.clone(),
        indexed: item.indexed,
    }
}
/// Caller: AFTER standard batching/buffer preparation, BEFORE RenderSystems::Render.
/// On failure restores draw IDs immediately; no wrapper may leak into ordinary draw.
pub(crate) fn prepare_and_mask(
    phase: &mut SortedRenderPhase<Transparent3d>,
    frame: &mut Frame,
    view: RetainedViewEntity,
    proof: &Proof,
) -> Result<(), &'static str> {
    prepare_transaction(
        phase,
        frame,
        view,
        proof,
        #[cfg(test)]
        None,
    )
}
// Test-only injection visits actual transaction checkpoints. No fault state,
// branch or counter exists in non-test builds.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FaultPoint {
    Saved(usize),
    Owned(usize),
    Masked(usize),
}
#[cfg(test)]
fn checkpoint(fault: Option<FaultPoint>, at: FaultPoint) -> Result<(), &'static str> {
    if fault == Some(at) {
        Err("injected preparation failure")
    } else {
        Ok(())
    }
}
fn prepare_transaction(
    phase: &mut SortedRenderPhase<Transparent3d>,
    frame: &mut Frame,
    view: RetainedViewEntity,
    proof: &Proof,
    #[cfg(test)] fault: Option<FaultPoint>,
) -> Result<(), &'static str> {
    let result = (|| {
        if frame.saved.len() != GROUPS || frame.prepared {
            return Err("missing/reused barrier");
        }
        let rows: Vec<_> = phase
            .items
            .iter()
            .map(|(key, item)| Entry {
                selected: frame.saved.iter().any(|s| s.key == *key),
                batch_len: item.batch_range.len(),
            })
            .collect();
        let heads = plan_batches(&rows)?;
        let expected = heads
            .iter()
            .map(|r| (phase.items[r.start].main_entity(), r.len()))
            .collect();
        proof.begin(view, expected)?;
        frame.owned.clear();
        #[cfg(test)]
        let mut saved_count = 0;
        for saved in &mut frame.saved {
            let item = phase
                .items
                .get(&saved.key)
                .ok_or("prepared owned key missing")?;
            saved.range = Some(item.batch_range.clone());
            #[cfg(test)]
            {
                saved_count += 1;
                checkpoint(fault, FaultPoint::Saved(saved_count))?;
            }
        }
        // Compaction preserves selected head/tail spans only because plan_batches proved isolation.
        for (key, item) in &phase.items {
            if frame.saved.iter().any(|s| s.key == *key) {
                frame.owned.add_retained(copy_item(item));
                #[cfg(test)]
                checkpoint(fault, FaultPoint::Owned(frame.owned.items.len()))?;
            }
        }
        #[cfg(test)]
        let mut masked_count = 0;
        for saved in &frame.saved {
            phase
                .items
                .get_mut(&saved.key)
                .ok_or("mask key missing")?
                .batch_range = 0..0;
            #[cfg(test)]
            {
                masked_count += 1;
                checkpoint(fault, FaultPoint::Masked(masked_count))?;
            }
        }
        frame.prepared = true;
        Ok(())
    })();
    if let Err(why) = result {
        proof.fail(why);
        let _ = restore(phase, frame, proof);
    }
    result
}
/// Called only for matching validated main view, after main_transparent_pass_3d.
/// Caller MUST validate native target/viewport/MSAA4 and original main depth.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_into_scene<'w>(
    context: &'w mut RenderContext,
    world: &'w World,
    view: Entity,
    target: &'w ViewTarget,
    depth: &'w ViewDepthTexture,
    frame: &'w Frame,
    proof: &Proof,
) -> Result<(), &'static str> {
    draw_into_scene_format(
        context,
        world,
        view,
        target,
        depth,
        frame,
        proof,
        TextureFormat::Rgba8UnormSrgb,
        "grouped-direct same scene/depth",
    )
}

/// Native HDR1 entry ONLY after actual main DlaaView/Dlss Dlaa/prepass,
/// retained identity, full physical extent, viewport and source-pipeline preflight.
/// These caller checks must also be repeated immediately before replay.
#[cfg(feature = "dlaa")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_into_scene_native_hdr1<'w>(
    context: &'w mut RenderContext,
    world: &'w World,
    view: Entity,
    target: &'w ViewTarget,
    depth: &'w ViewDepthTexture,
    frame: &'w Frame,
    proof: &Proof,
) -> Result<(), &'static str> {
    let extent = target.main_texture().size();
    if target.main_texture_format() != TextureFormat::Rgba16Float
        || target.main_texture().format() != TextureFormat::Rgba16Float
        || target.main_texture().sample_count() != 1
        || target.sampled_main_texture().is_some()
        || extent.width == 0
        || extent.height == 0
        || extent.depth_or_array_layers != 1
        || depth.texture.format() != TextureFormat::Depth32Float
        || depth.texture.sample_count() != 1
        || depth.texture.size() != extent
    {
        proof.fail("boundary native HDR1 attachment unsupported");
        return Err("boundary native HDR1 attachment unsupported");
    }
    draw_into_scene_format(
        context,
        world,
        view,
        target,
        depth,
        frame,
        proof,
        TextureFormat::Rgba16Float,
        "grouped-native HDR1 same scene/depth",
    )
}

// Private expected format, never an unrestricted public rendering profile.
// Baseline guard/label/order/body unchanged apart from this extraction.
#[allow(clippy::too_many_arguments)]
fn draw_into_scene_format<'w>(
    context: &'w mut RenderContext,
    world: &'w World,
    view: Entity,
    target: &'w ViewTarget,
    depth: &'w ViewDepthTexture,
    frame: &'w Frame,
    proof: &Proof,
    format: TextureFormat,
    label: &'static str,
) -> Result<(), &'static str> {
    if !frame.prepared || proof.failed() || target.main_texture_format() != format {
        proof.fail("boundary frame/target unready");
        return Err("boundary frame/target unready");
    }
    proof.open_replay()?;
    let ok = {
        let colours = [Some(target.get_color_attachment())];
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some(label),
            color_attachments: &colours,
            depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        frame.owned.render(&mut pass, world, view).is_ok()
    };
    proof.finish(ok)
}
/// Caller MUST run unconditionally after RenderSystems::Render/before Cleanup,
/// including absent main-node, failures/unready frames. This core cannot install that bridge by itself.
pub(crate) fn restore(
    phase: &mut SortedRenderPhase<Transparent3d>,
    frame: &mut Frame,
    proof: &Proof,
) -> Result<(), &'static str> {
    let unfinished = frame.prepared && !proof.marks().finished;
    if unfinished {
        proof.fail("prepared replay absent/incomplete");
    }
    let mut missing = false;
    for saved in frame.saved.drain(..) {
        if let Some(item) = phase.items.get_mut(&saved.key) {
            item.draw_function = saved.draw;
            if let Some(range) = saved.range {
                item.batch_range = range;
            }
        } else {
            missing = true;
        }
    }
    frame.owned.clear();
    frame.prepared = false;
    {
        let mut marks = proof.marks();
        marks.open = false;
        marks.restored = !missing;
    }
    if missing {
        proof.fail("retained restoration key missing");
        return Err("retained restoration key missing");
    }
    if unfinished {
        return Err("prepared replay absent/incomplete");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn e(selected: bool, batch_len: usize) -> Entry {
        Entry {
            selected,
            batch_len,
        }
    }
    #[test]
    fn isolated_groups_and_grouped_indirect_heads_cover_exactly_five() {
        assert_eq!(
            plan_batches(&[
                e(false, 1),
                e(true, 2),
                e(true, 0),
                e(false, 1),
                e(true, 3),
                e(true, 0),
                e(true, 0)
            ])
            .unwrap(),
            vec![1..3, 4..7]
        );
    }
    #[test]
    fn rejects_mixed_head_tail_missing_and_overflow() {
        for entries in [
            vec![
                e(false, 2),
                e(true, 0),
                e(true, 1),
                e(true, 1),
                e(true, 1),
                e(true, 1),
            ],
            vec![
                e(true, 2),
                e(false, 0),
                e(true, 1),
                e(true, 1),
                e(true, 1),
                e(true, 1),
            ],
            vec![e(true, 0), e(true, 1), e(true, 1), e(true, 1), e(true, 1)],
            vec![
                e(true, usize::MAX),
                e(true, 0),
                e(true, 0),
                e(true, 0),
                e(true, 0),
            ],
        ] {
            assert!(plan_batches(&entries).is_err());
        }
    }
    #[test]
    fn off_registration_needs_no_renderer_or_resource() {
        let mut app = App::new();
        assert!(register(&mut app, false).unwrap().is_none());
        assert!(app.get_sub_app(RenderApp).is_none());
        assert!(!app.world().contains_resource::<Proof>());
    }
    struct NoopDraw;
    impl bevy::render::render_phase::Draw<Transparent3d> for NoopDraw {
        fn draw<'w>(
            &mut self,
            _: &'w World,
            _: &mut TrackedRenderPass<'w>,
            _: Entity,
            _: &Transparent3d,
        ) -> Result<(), bevy::render::render_phase::DrawError> {
            Ok(())
        }
    }
    struct StandardFixture;
    struct MarkedFixture;
    fn fixture() -> (
        SortedRenderPhase<Transparent3d>,
        Vec<MainEntity>,
        Ids,
        RetainedViewEntity,
    ) {
        use bevy::core_pipeline::core_3d::TransparentSortingInfo3d;
        use bevy::render::{
            render_phase::PhaseItemExtraIndex, render_resource::CachedRenderPipelineId,
        };
        let mut world = World::new();
        let camera = world.spawn_empty().id();
        let other: MainEntity = world.spawn_empty().id().into();
        let groups: Vec<MainEntity> = (0..5).map(|_| world.spawn_empty().id().into()).collect();
        let registry = DrawFunctions::<Transparent3d>::default();
        let standard = registry.write().add_with::<StandardFixture, _>(NoopDraw);
        let marked = registry.write().add_with::<MarkedFixture, _>(NoopDraw);
        let ids = Ids { standard, marked };
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
                draw_function: standard,
                batch_range: 100 + index as u32..101 + index as u32,
                extra_index: PhaseItemExtraIndex::None,
                indexed: true,
            });
        }
        (
            phase,
            groups,
            ids,
            RetainedViewEntity::new(camera.into(), None, 0),
        )
    }
    fn ranges_and_draws(
        phase: &SortedRenderPhase<Transparent3d>,
    ) -> Vec<(Key, DrawFunctionId, Range<u32>)> {
        phase
            .items
            .iter()
            .map(|(key, item)| (*key, item.draw_function, item.batch_range.clone()))
            .collect()
    }
    #[test]
    fn actual_phase_mask_success_and_restore_preserve_keys_ranges_pipeline_and_nonselected() {
        let (mut phase, groups, ids, view) = fixture();
        let old = ranges_and_draws(&phase);
        let proof = Proof::default();
        let mut frame = Frame::default();
        barrier(&mut phase, &groups, ids, &mut frame).unwrap();
        assert_eq!(
            phase
                .items
                .values()
                .filter(|i| i.draw_function == ids.marked)
                .count(),
            5
        );
        prepare_and_mask(&mut phase, &mut frame, view, &proof).unwrap();
        assert_eq!(frame.owned.items.len(), 5);
        assert!(
            phase
                .items
                .values()
                .filter(|i| selected(i.main_entity(), &groups))
                .all(|i| i.batch_range.is_empty())
        );
        assert_eq!(
            phase
                .items
                .values()
                .find(|i| !selected(i.main_entity(), &groups))
                .unwrap()
                .batch_range,
            101..102
        );
        proof.marks().open = true; // CPU success-marker fixture, NOT a real GPU draw.
        for item in frame.owned.items.values() {
            proof
                .mark(view, item.main_entity(), item.batch_range.len())
                .unwrap();
        }
        proof.finish(true).unwrap();
        assert!(!proof.current_frame_ready());
        restore(&mut phase, &mut frame, &proof).unwrap();
        assert!(proof.current_frame_ready());
        assert_eq!(ranges_and_draws(&phase), old);
        assert!(frame.owned.items.is_empty());
        assert!(
            phase.items.values().all(
                |i| i.pipeline == bevy::render::render_resource::CachedRenderPipelineId::new(0)
            )
        );
    }
    #[test]
    fn reverse_pipe_viewquery_skip_leaves_no_late_terminal_and_real_core_restore_fails_closed() {
        use bevy::render::renderer::{CurrentView, ViewQuery};
        #[derive(Component)]
        struct NativeRequired;
        #[derive(Resource, Default)]
        struct Calls {
            native: usize,
            late: usize,
        }
        fn native_cpu(_view: ViewQuery<&NativeRequired>, mut calls: ResMut<Calls>) {
            calls.native += 1;
        }
        fn late_cpu(mut calls: ResMut<Calls>) {
            calls.late += 1;
        }
        // These execute real ECS ViewQuery validation and the real prepared core
        // restore, not the GPU native function or a simulated successful draw.
        let (mut phase, groups, ids, view) = fixture();
        let old = ranges_and_draws(&phase);
        let proof = Proof::default();
        let mut frame = Frame::default();
        barrier(&mut phase, &groups, ids, &mut frame).unwrap();
        prepare_and_mask(&mut phase, &mut frame, view, &proof).unwrap();
        let mut world = World::new();
        world.insert_resource(Calls::default());
        let entity = world.spawn_empty().id();
        world.insert_resource(CurrentView(entity));
        let mut schedule = Schedule::default();
        schedule.add_systems(native_cpu.pipe(late_cpu));
        schedule.run(&mut world);
        assert_eq!(world.resource::<Calls>().native, 0);
        assert_eq!(world.resource::<Calls>().late, 0);
        assert!(restore(&mut phase, &mut frame, &proof).is_err());
        assert!(proof.failed());
        assert!(!proof.current_frame_ready());
        assert_eq!(ranges_and_draws(&phase), old);
        assert!(frame.saved.is_empty());
        assert!(frame.owned.items.is_empty());
        // Missing late receipt is fatal even though native ViewQuery validation
        // skipped before either CPU body. No harmless fallback after masking.
    }
    #[test]
    fn missing_replay_or_drawmesh_skip_cannot_ack_but_still_restores_retained_state() {
        for pretend_ok in [false, true] {
            let (mut phase, groups, ids, view) = fixture();
            let old = ranges_and_draws(&phase);
            let proof = Proof::default();
            let mut frame = Frame::default();
            barrier(&mut phase, &groups, ids, &mut frame).unwrap();
            prepare_and_mask(&mut phase, &mut frame, view, &proof).unwrap();
            if pretend_ok {
                assert!(proof.finish(true).is_err());
            } // render()Ok without markers.
            assert!(restore(&mut phase, &mut frame, &proof).is_err());
            assert!(proof.failed());
            assert!(!proof.current_frame_ready());
            assert!(!proof.marks().open);
            assert_eq!(ranges_and_draws(&phase), old);
            assert!(frame.saved.is_empty());
        }
    }
    #[test]
    fn mixed_prepared_batch_transaction_rejects_and_restores_draw_barrier() {
        let (mut phase, groups, ids, view) = fixture();
        let proof = Proof::default();
        let mut frame = Frame::default();
        barrier(&mut phase, &groups, ids, &mut frame).unwrap();
        phase.items[0].batch_range = 100..102; // Mix group0 with other.
        let before = phase
            .items
            .values()
            .map(|i| i.batch_range.clone())
            .collect::<Vec<_>>();
        assert!(prepare_and_mask(&mut phase, &mut frame, view, &proof).is_err());
        assert!(
            phase
                .items
                .values()
                .all(|i| i.draw_function == ids.standard)
        );
        assert!(frame.saved.is_empty());
        assert_eq!(
            phase
                .items
                .values()
                .map(|i| i.batch_range.clone())
                .collect::<Vec<_>>(),
            before
        );
    }
    #[test]
    fn every_partial_prepare_checkpoint_restores_exactly_and_cannot_reuse_old_success() {
        for at in (1..=GROUPS).flat_map(|n| {
            [
                FaultPoint::Saved(n),
                FaultPoint::Owned(n),
                FaultPoint::Masked(n),
            ]
        }) {
            let (mut phase, groups, ids, view) = fixture();
            let old = ranges_and_draws(&phase);
            let proof = Proof::default();
            // Previous-frame success must not survive a current partial failure.
            {
                let mut marks = proof.marks();
                marks.finished = true;
                marks.restored = true;
            }
            assert!(proof.current_frame_ready());
            let mut frame = Frame::default();
            barrier(&mut phase, &groups, ids, &mut frame).unwrap();
            assert_eq!(
                prepare_transaction(&mut phase, &mut frame, view, &proof, Some(at)),
                Err("injected preparation failure"),
                "{at:?}",
            );
            assert_eq!(ranges_and_draws(&phase), old, "{at:?}");
            assert!(frame.saved.is_empty(), "{at:?}");
            assert!(frame.owned.items.is_empty(), "{at:?}");
            assert!(!frame.prepared, "{at:?}");
            assert!(proof.failed(), "{at:?}");
            assert!(!proof.current_frame_ready(), "{at:?}");
            assert!(!proof.marks().open, "{at:?}");
            // Cleanup remains idempotent, but cannot clear the failure latch.
            restore(&mut phase, &mut frame, &proof).unwrap();
            assert!(proof.failed(), "{at:?}");
            assert!(!proof.current_frame_ready(), "{at:?}");
            assert!(proof.begin(view, vec![(groups[0], GROUPS)]).is_err());
        }
    }
    #[test]
    fn duplicate_open_is_rejected_before_draw_and_missing_phase_never_acknowledges_restore() {
        let (mut phase, groups, ids, view) = fixture();
        let proof = Proof::default();
        let mut frame = Frame::default();
        barrier(&mut phase, &groups, ids, &mut frame).unwrap();
        prepare_and_mask(&mut phase, &mut frame, view, &proof).unwrap();
        proof.open_replay().unwrap();
        assert!(proof.open_replay().is_err());
        assert!(restore(&mut phase, &mut frame, &proof).is_err());
        assert!(frame.clean());
        let (mut phase, groups, ids, view) = fixture();
        let proof = Proof::default();
        let mut frame = Frame::default();
        barrier(&mut phase, &groups, ids, &mut frame).unwrap();
        prepare_and_mask(&mut phase, &mut frame, view, &proof).unwrap();
        frame.discard_missing_phase(&proof);
        assert!(frame.clean());
        assert!(proof.failed());
        assert!(!proof.current_frame_ready());
        assert!(!proof.marks().restored);
    }
    #[test]
    fn wrong_view_duplicate_marker_and_unrestored_next_frame_are_rejected() {
        let (mut phase, groups, ids, view) = fixture();
        let proof = Proof::default();
        let mut frame = Frame::default();
        barrier(&mut phase, &groups, ids, &mut frame).unwrap();
        assert!(barrier(&mut phase, &groups, ids, &mut frame).is_err());
        prepare_and_mask(&mut phase, &mut frame, view, &proof).unwrap();
        proof.marks().open = true;
        proof.mark(view, groups[0], 1).unwrap();
        assert!(proof.mark(view, groups[0], 1).is_err());
        assert!(restore(&mut phase, &mut frame, &proof).is_err());
        let (mut phase, groups, ids, view) = fixture();
        let proof = Proof::default();
        let mut frame = Frame::default();
        barrier(&mut phase, &groups, ids, &mut frame).unwrap();
        prepare_and_mask(&mut phase, &mut frame, view, &proof).unwrap();
        proof.marks().open = true;
        let wrong = RetainedViewEntity::new(groups[0], None, 0);
        assert!(proof.mark(wrong, groups[0], 1).is_err());
        assert!(restore(&mut phase, &mut frame, &proof).is_err());
    }
}
