//! Bounded five-group direct-boundary control; independently authored.
//! Moves existing prepared draws only. No material, pipeline, simulation or sample changes.
use super::smoke_boundary_core as core;
mod live;
pub(crate) mod attachments;
pub(crate) mod native_diagnostic;
use super::smoke_pilot::Profile;
use bevy::{
    camera::{MainPassResolutionOverride, NormalizedRenderTarget, RenderTarget},
    core_pipeline::{
        core_3d::{Transparent3d, main_transparent_pass_3d},
        schedule::{Core3d, Core3dSystems},
    },
    diagnostic::FrameCount,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        camera::ExtractedCamera,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_phase::{PhaseItem, ViewSortedRenderPhases},
        render_resource::{PipelineCache, TextureFormat},
        renderer::{RenderContext, ViewQuery},
        sync_world::MainEntity,
        view::{
            ExtractedView, RetainedViewEntity, ViewDepthTexture, ViewTarget, screenshot::Screenshot,
        },
    },
};
pub(crate) use live::{State as LiveState, NativeLatePolicy};
use std::{
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::Write as _,
    ops::Range,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Off,
    GroupedDirect,
    GroupedLive,
    GroupedLiveDerby,
}
impl Mode {
    pub(crate) fn parse(text: &str) -> Result<Self, &'static str> {
        match text {
            "off" => Ok(Self::Off),
            "grouped-direct" => Ok(Self::GroupedDirect),
            "grouped-live" => Ok(Self::GroupedLive),
            "grouped-live-derby" => Ok(Self::GroupedLiveDerby),
            _ => Err("--smoke-boundary needs off, grouped-direct, grouped-live or grouped-live-derby"),
        }
    }
}
/// Explicit Derby identity and capture scope; it reuses the Baseline4 renderer.
#[derive(Clone, Copy)]
pub(crate) struct DerbyFixture<'a> {
    pub route: &'a str,
    pub subroute: u8,
    pub car: &'a str,
    pub opponents: Option<usize>,
    pub opponent_models_stk: bool,
    pub discipline: Option<&'a str>,
    pub laps: Option<u32>,
    pub back_grid: bool,
    pub damage_enabled: bool,
    pub output: bool,
    pub ticks: Option<u64>,
    pub fps: Option<u32>,
    pub motion: Option<crate::motion_capture::Config>,
    pub handbrake: Option<u64>,
    pub throttle: f32,
    pub steering: f32,
}
impl DerbyFixture<'_> {
    pub(crate) fn admitted(self) -> bool {
        let identity = self.route == "tracks/usa/detroit/route_8" && self.subroute == 0
            && self.car == "stk" && self.opponents == Some(11) && self.opponent_models_stk
            && self.discipline == Some("demolition_derby") && self.laps == Some(6)
            && self.back_grid && self.damage_enabled;
        let capture = if self.output {
            self.ticks == Some(900) && self.fps == Some(30)
                && self.motion.is_some_and(|m| m.exact_phase_window(350, 120, 30, 900))
                && self.handbrake == Some(500) && self.throttle == 1. && self.steering == 1.
        } else {
            self.ticks.is_none() && self.fps.is_none() && self.motion.is_none()
                && self.handbrake.is_none() && self.throttle == 0. && self.steering == 0.
        };
        identity && capture
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ticket {
    entity: u64,
    frame: u32,
    tick: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Extracted,
    Barrier,
    Prepared,
    Replayed,
    Restored,
}
#[derive(Debug)]
struct Row {
    ticket: Ticket,
    stage: Stage,
    text: String,
}
#[derive(Default, Debug)]
struct Ledger {
    profile: Profile,
    rows: Vec<Row>,
    error: Option<String>,
    published: bool,
}
impl Ledger {
    fn observe(&mut self, ticket: Ticket) -> Result<Option<usize>, &'static str> {
        // Old pending screenshots are expected during asynchronous readback.
        if self.rows.iter().any(|r| r.ticket.entity == ticket.entity) {
            return Ok(None);
        }
        if self.rows.len() >= self.profile.shots()
            || self
                .rows
                .last()
                .is_some_and(|r| ticket.frame <= r.ticket.frame || ticket.tick < r.ticket.tick)
        {
            return Err("boundary unexpected/reordered screenshot extraction");
        }
        self.rows.push(Row {
            ticket,
            stage: Stage::Extracted,
            text: String::new(),
        });
        Ok(Some(self.rows.len() - 1))
    }
    fn advance(
        &mut self,
        index: usize,
        ticket: Ticket,
        from: Stage,
        to: Stage,
        text: &str,
    ) -> Result<(), &'static str> {
        if self.error.is_some() {
            return Err("boundary failure already latched");
        }
        if !matches!(
            (from, to),
            (Stage::Extracted, Stage::Barrier)
                | (Stage::Barrier, Stage::Prepared)
                | (Stage::Prepared, Stage::Replayed)
                | (Stage::Replayed, Stage::Restored)
        ) {
            return Err("boundary invalid stage transition");
        }
        if self
            .rows
            .iter()
            .take(index)
            .any(|r| r.stage != Stage::Restored)
        {
            return Err("boundary earlier captured frame incomplete");
        }
        let row = self
            .rows
            .get_mut(index)
            .ok_or("boundary stage without expected screenshot")?;
        if row.ticket != ticket || row.stage != from {
            return Err("boundary wrong/duplicate/missing stage association");
        }
        row.stage = to;
        row.text.push_str(text);
        Ok(())
    }
    fn fail(&mut self, text: impl Into<String>) {
        self.error.get_or_insert_with(|| text.into());
    }
    fn complete(&self) -> bool {
        self.error.is_none()
            && self.rows.len() == self.profile.shots()
            && self.rows.iter().all(|r| r.stage == Stage::Restored)
    }
}
#[derive(Resource, Clone, ExtractResource)]
pub(crate) struct Audit {
    state: Arc<Mutex<Ledger>>,
    output: Arc<Mutex<File>>,
}
impl Audit {
    fn state(&self) -> MutexGuard<'_, Ledger> {
        self.state.lock().unwrap_or_else(|e| {
            let mut s = e.into_inner();
            s.fail("boundary ledger poisoned");
            s
        })
    }
    fn fail(&self, text: impl Into<String>) {
        self.state().fail(text);
    }
    pub(crate) fn failed(&self) -> bool {
        self.state().error.is_some()
    }
    pub(crate) fn ready(&self) -> bool {
        let s = self.state();
        s.published && s.complete()
    }
    fn advance(&self, shot: Option<(usize, Ticket)>, from: Stage, to: Stage, text: &str) {
        let Some((i, t)) = shot else {
            return;
        };
        let mut s = self.state();
        if let Err(e) = s.advance(i, t, from, to, text) {
            s.fail(e);
        }
    }
}
#[derive(Resource, Default)]
struct Snapshot {
    frame: u32,
    tick: u64,
    camera: Option<Entity>,
    target: Option<NormalizedRenderTarget>,
    groups: Vec<MainEntity>,
    shot: Option<(usize, Ticket)>,
}
#[derive(Resource)]
struct Registry(core::Ids);
#[derive(Resource, Default)]
struct Routing {
    frame: core::Frame,
    view: Option<RetainedViewEntity>,
    clock: Option<(u32, u64)>,
    shot: Option<(usize, Ticket)>,
    stage: Option<Stage>,
    ranges: Vec<((Entity, MainEntity), Range<u32>)>,
}
impl Routing {
    fn idle(&self) -> bool {
        self.view.is_none() && self.clock.is_none() && self.frame.clean() && self.stage.is_none()
    }
    fn reset(&mut self) {
        self.view = None;
        self.clock = None;
        self.shot = None;
        self.stage = None;
        self.ranges.clear();
    }
}
/// Raw selector presence, including explicit Off, isolates historical diagnostic controls.
pub(crate) fn explicit_policy_arg(arg:&str)->bool {
    matches!(arg,"--smoke-boundary"|"--native-smoke-boundary"|"--native-smoke-profile"|"--smoke-ct"
        |"--smoke-compositor"|"--ground-skid-pass"|"--skid-order"|"--skid-reflectance"
        |"--smoke-order"|"--motion-phase-map"|"--motion-phase-profile"
        |"--window-phase-audit"|"--oit-capacity-audit"|"--temporal-audit"|"--temporal-colour-audit")
}
/// Absence of an explicit render-policy selector is the ordinary shipping path.
pub(crate) fn automatic_selected(driving:bool,rendered:bool,ground:bool,groups:bool,explicit:bool)->bool {
    driving&&rendered&&ground&&groups&&!explicit
}
pub(crate) fn install_automatic(app:&mut App)->std::io::Result<()> {live::install_automatic(app)}

/// Off does not install resources, hooks, files or render commands.
pub(crate) fn install(
    app: &mut App,
    mode: Mode,
    capture: Option<&Path>,
    profile: Profile,
) -> std::io::Result<()> {
    if mode == Mode::Off {
        return Ok(());
    }
    if mode == Mode::GroupedLive {
        return live::install(app);
    }
    if mode == Mode::GroupedLiveDerby {
        return live::install_derby(app);
    }
    if app.world().contains_resource::<Audit>() {
        return Err(std::io::Error::other("duplicate boundary install"));
    }
    let capture = capture.ok_or_else(|| std::io::Error::other("boundary needs bounded capture"))?;
    let local = Path::new("local").canonicalize()?;
    let parent = capture
        .parent()
        .ok_or_else(|| std::io::Error::other("boundary capture parent absent"))?
        .canonicalize()?;
    if !parent.starts_with(local) {
        return Err(std::io::Error::other(
            "boundary evidence must remain under ignored local/",
        ));
    }
    let path = parent.join(
        capture
            .with_extension("smoke-boundary.txt")
            .file_name()
            .ok_or_else(|| std::io::Error::other("boundary output name absent"))?,
    );
    let count = profile.shots();
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    writeln!(
        output,
        "smoke_boundary version=1 status=pending expected={count} mode=grouped-direct"
    )?;
    output.flush()?;
    let Some((_, ids)) = core::register(app, true)? else {
        return Err(std::io::Error::other(
            "boundary renderer registration absent",
        ));
    };
    app.insert_resource(Audit {
        state: Arc::new(Mutex::new(Ledger {
            profile,
            ..default()
        })),
        output: Arc::new(Mutex::new(output)),
    })
    .add_plugins(ExtractResourcePlugin::<Audit>::default())
    .add_systems(Update, publish);
    let render = app
        .get_sub_app_mut(RenderApp)
        .ok_or_else(|| std::io::Error::other("boundary renderer absent"))?;
    render
        .insert_resource(Registry(ids))
        .init_resource::<Snapshot>()
        .init_resource::<Routing>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(
            Render,
            apply_barrier
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
        "smoke_boundary enabled=true expected={count} output={} default=false",
        path.display()
    );
    Ok(())
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Explicit bounded extraction.
fn extract(
    frame: Extract<Res<FrameCount>>,
    driving: Extract<Res<super::Driving>>,
    audit: Extract<Res<Audit>>,
    shots: Extract<Query<(Entity, &super::skid_phase_ledger::Stamp, &Screenshot)>>,
    cameras: Extract<Query<(Entity, &RenderTarget), With<super::ViewerCamera>>>,
    smoke: Extract<Option<Res<super::tyre_smoke::TyreSmoke>>>,
    meshes: Extract<Query<(Entity, &Mesh3d)>>,
    mut route: ResMut<Routing>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    proof: Res<core::Proof>,
    mut snapshot: ResMut<Snapshot>,
) {
    *snapshot = Snapshot {
        frame: frame.0,
        tick: driving.runtime.ticks,
        ..default()
    };
    // Register expected fresh screenshots BEFORE any view/group/pipeline readiness check.
    let mut fresh = Vec::new();
    {
        let mut s = audit.state();
        for (entity, _, shot) in &shots {
            let ticket = Ticket {
                entity: entity.to_bits(),
                frame: frame.0,
                tick: driving.runtime.ticks,
            };
            match s.observe(ticket) {
                Ok(Some(i)) => fresh.push((i, ticket, shot.0.normalize(None))),
                Ok(None) => {}
                Err(e) => s.fail(e),
            }
        }
        if fresh.len() > 1 {
            s.fail("boundary multiple new stamped screenshots per Extract");
        }
    }
    snapshot.shot = fresh.first().map(|(i, t, _)| (*i, *t));
    if !route.idle() {
        audit.fail("boundary prior render frame was not finalized/restored");
        // The previous Render schedule has returned; restore retained CPU state
        // before the next queue/batch pass, even if the finalizer was omitted.
        if let Some(view) = route.view {
            if let Some(phase) = phases.get_mut(&view) {
                if let Err(e) = core::restore(phase, &mut route.frame, &proof) {
                    audit.fail(e);
                }
            } else {
                route.frame.discard_missing_phase(&proof);
            }
        } else if !route.frame.clean() {
            route.frame.discard_missing_phase(&proof);
        }
        route.reset();
    }
    if audit.failed() {
        return;
    }
    let Ok((camera, target)) = cameras.single() else {
        if snapshot.shot.is_some() {
            audit.fail("boundary selected camera missing/nonunique");
        }
        return;
    };
    let normalized = target.normalize(None);
    if let Some((_, _, expected)) = fresh.first()
        && (expected.is_none() || *expected != normalized)
    {
        audit.fail("boundary screenshot/main target mismatch");
        return;
    }
    snapshot.camera = Some(camera);
    snapshot.target = normalized;
    let Some(smoke) = smoke.as_ref() else {
        if snapshot.shot.is_some() {
            audit.fail("boundary selected smoke owner missing");
        }
        return;
    };
    let handles = smoke.mesh_handles();
    if handles.len() != 5
        || handles
            .iter()
            .enumerate()
            .any(|(i, h)| handles[..i].iter().any(|old| old.id() == h.id()))
    {
        if snapshot.shot.is_some() {
            audit.fail("boundary ownership not five unique Groups mesh handles");
        }
        return;
    }
    for (entity, mesh) in &meshes {
        if handles.iter().any(|h| h.id() == mesh.id()) {
            snapshot.groups.push(entity.into());
        }
    }
    if snapshot.groups.len() != 5 {
        snapshot.groups.clear();
        if snapshot.shot.is_some() {
            audit.fail("boundary selected source group entities missing/aliased");
        }
    }
}
fn unavailable(audit: &Audit, snapshot: &Snapshot, text: &str) {
    if snapshot.shot.is_some() {
        audit.fail(text);
    }
}
fn valid_main_view(
    extracted: &ExtractedView,
    camera: &ExtractedCamera,
    target: &ViewTarget,
    msaa: &Msaa,
    resolution_override: Option<&MainPassResolutionOverride>,
    expected: Option<&NormalizedRenderTarget>,
) -> bool {
    camera.target.as_ref() == expected
        && matches!(
            camera.target.as_ref(),
            Some(NormalizedRenderTarget::Image(_))
        )
        && camera.physical_target_size == Some(UVec2::new(1920, 1080))
        && camera.viewport.is_none()
        && resolution_override.is_none()
        && extracted.viewport == UVec4::new(0, 0, 1920, 1080)
        && msaa.samples() == 4
        && target.main_texture_format() == TextureFormat::Rgba8UnormSrgb
}
#[allow(clippy::type_complexity)] // Explicit main-view attachment contract before mutation.
fn apply_barrier(
    snapshot: Res<Snapshot>,
    audit: Res<Audit>,
    ids: Res<Registry>,
    cache: Res<PipelineCache>,
    views: Query<(
        &ExtractedView,
        &ExtractedCamera,
        &ViewTarget,
        &ViewDepthTexture,
        &Msaa,
        Option<&MainPassResolutionOverride>,
    )>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut route: ResMut<Routing>,
) {
    if audit.failed() {
        return;
    }
    if !route.idle() {
        audit.fail("boundary previous route still active");
        return;
    }
    if snapshot.groups.len() != 5 {
        unavailable(&audit, &snapshot, "boundary selected ownership not ready");
        return;
    }
    let matches: Vec<_> = views
        .iter()
        .filter(|(v, _, _, _, _, _)| {
            Some(v.retained_view_entity.main_entity.id()) == snapshot.camera
                && v.retained_view_entity.subview_index == 0
        })
        .collect();
    if matches.len() != 1 {
        unavailable(
            &audit,
            &snapshot,
            "boundary selected extracted view not unique",
        );
        return;
    }
    let (view, camera, target, _, msaa, resolution_override) = matches[0];
    if !valid_main_view(
        view,
        camera,
        target,
        msaa,
        resolution_override,
        snapshot.target.as_ref(),
    ) {
        unavailable(
            &audit,
            &snapshot,
            "boundary extracted view/attachment contract differs",
        );
        return;
    }
    let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
        unavailable(&audit, &snapshot, "boundary selected phase missing");
        return;
    };
    if phase.items.len() > 50_000 {
        unavailable(
            &audit,
            &snapshot,
            "boundary phase exceeds bounded item limit",
        );
        return;
    }
    let selected: Vec<_> = phase
        .items
        .values()
        .filter(|i| snapshot.groups.contains(&i.main_entity()))
        .collect();
    if selected.len() != 5 {
        unavailable(
            &audit,
            &snapshot,
            "boundary selected phase lacks five Groups",
        );
        return;
    }
    let mut details = format!(
        "barrier frame={} tick={} camera={:?} target={:?} retained_view={:?} groups=5\n",
        snapshot.frame, snapshot.tick, snapshot.camera, snapshot.target, view.retained_view_entity,
    );
    for item in selected {
        if !item.distance.is_finite() {
            unavailable(&audit, &snapshot, "boundary source sort distance nonfinite");
            return;
        }
        if cache.get_render_pipeline(item.pipeline).is_none() {
            unavailable(
                &audit,
                &snapshot,
                "boundary selected source pipeline unready",
            );
            return;
        }
        // Descriptor accessor is safe only AFTER the ready check.
        let desc = cache.get_render_pipeline_descriptor(item.pipeline);
        if desc.multisample.count != 4
            || desc.depth_stencil.as_ref().is_none_or(|d| {
                d.depth_write_enabled != Some(false) || d.format != TextureFormat::Depth32Float
            })
            || desc.fragment.as_ref().is_none_or(|f| {
                f.targets.len() != 1
                    || f.targets[0]
                        .as_ref()
                        .is_none_or(|t| t.format != TextureFormat::Rgba8UnormSrgb)
            })
        {
            unavailable(
                &audit,
                &snapshot,
                "boundary selected source pipeline contract differs",
            );
            return;
        }
        let _ = writeln!(
            details,
            "source main={:?} pipeline={:?} sort_distance={} draw={:?} raster={:?}",
            item.main_entity(),
            item.pipeline,
            item.distance,
            item.draw_function,
            desc.depth_stencil.as_ref().map(|d| d.bias)
        );
    }
    // Record scope BEFORE mutation, so even an unexpected partial barrier failure is cleaned.
    route.view = Some(view.retained_view_entity);
    route.clock = Some((snapshot.frame, snapshot.tick));
    route.shot = snapshot.shot;
    match core::barrier(phase, &snapshot.groups, ids.0, &mut route.frame) {
        Ok(()) => {
            route.stage = Some(Stage::Barrier);
            audit.advance(route.shot, Stage::Extracted, Stage::Barrier, &details);
        }
        Err(e) => audit.fail(e),
    }
}
fn prepare(
    snapshot: Res<Snapshot>,
    audit: Res<Audit>,
    proof: Res<core::Proof>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut route: ResMut<Routing>,
) {
    if audit.failed() {
        return;
    }
    let Some(view) = route.view else {
        return;
    };
    if route.clock != Some((snapshot.frame, snapshot.tick)) || route.stage != Some(Stage::Barrier) {
        audit.fail("boundary prepare scope/stage differs");
        return;
    }
    let Some(phase) = phases.get_mut(&view) else {
        audit.fail("boundary prepared phase missing");
        return;
    };
    route.ranges = phase
        .items
        .iter()
        .filter(|(_, i)| snapshot.groups.contains(&i.main_entity()))
        .map(|(k, i)| (*k, i.batch_range.clone()))
        .collect();
    match core::prepare_and_mask(phase, &mut route.frame, view, &proof) {
        Ok(()) => {
            route.stage = Some(Stage::Prepared);
            let text = format!(
                "prepared groups=5 ranges={:?} owned={} masked=true\n",
                route.ranges,
                route.frame.owned.items.len()
            );
            audit.advance(route.shot, Stage::Barrier, Stage::Prepared, &text);
        }
        Err(e) => audit.fail(e),
    }
}
type Inputs = (
    &'static ExtractedView,
    &'static ExtractedCamera,
    &'static ViewTarget,
    &'static ViewDepthTexture,
    &'static Msaa,
    Option<&'static MainPassResolutionOverride>,
);
#[allow(clippy::too_many_arguments)] // Same readonly-world render pattern as Bevy main transparency.
fn replay(
    world: &World,
    view: ViewQuery<Inputs>,
    mut context: RenderContext,
    audit: Res<Audit>,
    snapshot: Res<Snapshot>,
    route: Res<Routing>,
    proof: Res<core::Proof>,
) {
    let entity = view.entity();
    let (extracted, camera, target, depth, msaa, resolution_override) = view.into_inner();
    if Some(extracted.retained_view_entity) != route.view {
        return;
    }
    if audit.failed() {
        return;
    }
    if route.clock != Some((snapshot.frame, snapshot.tick))
        || route.stage != Some(Stage::Prepared)
        || !valid_main_view(
            extracted,
            camera,
            target,
            msaa,
            resolution_override,
            snapshot.target.as_ref(),
        )
    {
        audit.fail("boundary replay selected scope/view/target/MSAA differs");
        return;
    }
    // Route is readonly during Core3d; selected-row stage lives in shared Audit.
    // Noncapture frames use Proof.finished/restored and the route Prepared state.
    if let Err(e) = core::draw_into_scene(
        &mut context,
        world,
        entity,
        target,
        depth,
        &route.frame,
        &proof,
    ) {
        audit.fail(e);
        return;
    }
    let (heads, groups) = proof.successful_counts();
    audit.advance(
        route.shot,
        Stage::Prepared,
        Stage::Replayed,
        &format!(
            "replayed heads={heads} groups={groups} render_frame={} tick={}\n",
            snapshot.frame, snapshot.tick
        ),
    );
}
fn finalize(
    snapshot: Res<Snapshot>,
    audit: Res<Audit>,
    proof: Res<core::Proof>,
    ids: Res<Registry>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut route: ResMut<Routing>,
) {
    // UNCONDITIONAL even after failure, skipped node, missing view or unready pipeline.
    if let Some(view) = route.view {
        let scope_ok = route.clock == Some((snapshot.frame, snapshot.tick));
        if !scope_ok {
            audit.fail("boundary finalizer scope differs");
        }
        if let Some(phase) = phases.get_mut(&view) {
            if let Err(e) = core::restore(phase, &mut route.frame, &proof) {
                audit.fail(e);
            }
            let exact = route.ranges.len() == 5
                && route.ranges.iter().all(|(key, range)| {
                    phase.items.get(key).is_some_and(|i| {
                        i.batch_range == *range && i.draw_function == ids.0.standard
                    })
                });
            if !exact {
                audit.fail("boundary retained ranges/draw IDs not exactly restored");
            }
            if !proof.current_frame_ready() {
                audit.fail("boundary frame missing successful replay/restoration");
            }
            if !audit.failed() && scope_ok && exact {
                audit.advance(
                    route.shot,
                    Stage::Replayed,
                    Stage::Restored,
                    "restored=true exact_ranges=true draw_ids=true\n",
                );
            }
        } else {
            audit.fail("boundary finalizer retained phase missing");
            route.frame.discard_missing_phase(&proof);
        }
        if !route.frame.clean() {
            audit.fail("boundary cleanup left saved/owned/prepared state");
        }
        route.reset();
    } else if snapshot.shot.is_some() {
        audit.fail("boundary selected frame had no barrier/prepare/replay");
    }
}
fn publish(audit: Res<Audit>, driving: Res<super::Driving>, mut exit: MessageWriter<AppExit>) {
    let mut s = audit.state();
    let count = s.profile.shots();
    if !s.complete()
        && (driving.paused
            || driving.failure.is_some()
            || driving
                .scripted_ticks
                .is_some_and(|end| driving.runtime.ticks >= end))
    {
        s.fail("boundary incomplete expected ledger at stopped/failed/script-limit checkpoint");
    }
    if !s.published && (s.complete() || s.error.is_some()) {
        let mut text = String::new();
        for (index, row) in s.rows.iter().enumerate() {
            let _ = writeln!(
                text,
                "shot index={index} png={index:04}.png entity_bits={} extract_frame={} tick={} stage={:?}",
                row.ticket.entity, row.ticket.frame, row.ticket.tick, row.stage
            );
            text.push_str(&row.text);
        }
        let _ = writeln!(
            text,
            "smoke_boundary status={} expected={count} rows={} error={:?}",
            if s.complete() { "complete" } else { "failed" },
            s.rows.len(),
            s.error
        );
        let result = audit
            .output
            .lock()
            .map_err(|_| std::io::Error::other("boundary output lock poisoned"))
            .and_then(|mut file| file.write_all(text.as_bytes()).and_then(|_| file.flush()));
        if let Err(e) = result {
            s.fail(format!("boundary sidecar write: {e}"));
        }
        s.published = true;
    }
    if let Some(e) = &s.error {
        error!("smoke_boundary failed: {e}");
        exit.write(AppExit::error());
    }
}
/// Context token construction is private to native_diagnostic::Arm::validate.
/// Off and Control deliberately register no draw/phase policy. Control is an
/// ordinary-transparency admission control, NOT an image-neutral late reorder.
pub(crate) fn install_native_diagnostic(app: &mut App, context: native_diagnostic::Validated) -> std::io::Result<()> {
    #[cfg(not(feature = "dlaa"))]
    let _ = app;
    println!("native_smoke_diagnostic arm={:?} source_context_validated=true native_runtime=unproven boundary_registration={} control_semantics=ordinary_transparency route={:?} capture_profile={:?}", context.arm(), context.installs_boundary(), context.route(), context.capture_profile());
    if context.installs_boundary() {
        #[cfg(feature="dlaa")]
        { return live::install_profile(app, crate::ground_skid_pass::Profile::NativeDlaaHdr1); }
        #[cfg(not(feature="dlaa"))]
        { return Err(std::io::Error::other("native smoke requires dlaa feature")); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_derby_live_scope_rejects_crossed_identity_and_capture() {
        let valid = DerbyFixture {
            route: "tracks/usa/detroit/route_8", subroute: 0, car: "stk", opponents: Some(11),
            opponent_models_stk: true, discipline: Some("demolition_derby"), laps: Some(6),
            back_grid: true, damage_enabled: true, output: true, ticks: Some(900), fps: Some(30),
            motion: Some(crate::motion_capture::Config::parse("350,120").unwrap()),
            handbrake: Some(500), throttle: 1., steering: 1.,
        };
        assert!(valid.admitted());
        for wrong in [
            DerbyFixture { route: "tracks/europe/istanbul_park/route_0", ..valid },
            DerbyFixture { subroute: 1, ..valid }, DerbyFixture { car: "bmw", ..valid },
            DerbyFixture { opponents: Some(10), ..valid }, DerbyFixture { opponent_models_stk: false, ..valid },
            DerbyFixture { discipline: None, ..valid }, DerbyFixture { laps: Some(3), ..valid },
            DerbyFixture { back_grid: false, ..valid }, DerbyFixture { damage_enabled: false, ..valid },
            DerbyFixture { ticks: Some(450), ..valid }, DerbyFixture { fps: Some(60), ..valid },
            DerbyFixture { motion: Some(crate::motion_capture::Config::parse("190,90").unwrap()), ..valid },
            DerbyFixture { handbrake: Some(190), ..valid }, DerbyFixture { throttle: f32::NAN, ..valid },
            DerbyFixture { steering: f32::INFINITY, ..valid }, DerbyFixture { output: false, ..valid },
        ] { assert!(!wrong.admitted()); }
        let manual = DerbyFixture { output: false, ticks: None, fps: None, motion: None,
            handbrake: None, throttle: 0., steering: 0., ..valid };
        assert!(manual.admitted());
        assert!(!DerbyFixture { ticks: Some(900), ..manual }.admitted());
        assert!(!DerbyFixture { fps: Some(30), ..manual }.admitted());
        assert_eq!(Mode::parse("grouped-live-derby").unwrap(), Mode::GroupedLiveDerby);
        assert_eq!(Mode::parse("grouped-live").unwrap(), Mode::GroupedLive);
        assert_eq!(Mode::default(), Mode::Off);
        assert!(Mode::parse("derby").is_err());
    }

    fn t(i: u32) -> Ticket {
        Ticket {
            entity: 100 + u64::from(i),
            frame: 200 + i,
            tick: 350 + u64::from(i),
        }
    }
    fn finish(s: &mut Ledger, i: usize, ticket: Ticket) {
        for (from, to) in [
            (Stage::Extracted, Stage::Barrier),
            (Stage::Barrier, Stage::Prepared),
            (Stage::Prepared, Stage::Replayed),
            (Stage::Replayed, Stage::Restored),
        ] {
            s.advance(i, ticket, from, to, "proof\n").unwrap();
        }
    }
    #[test]
    fn absent_installer_is_resource_and_renderer_free() {
        let mut app = App::new();
        install(&mut app, Mode::Off, None, Profile::default()).unwrap();
        assert!(!app.world().contains_resource::<Audit>());
        assert!(!app.world().contains_resource::<core::Proof>());
        assert!(app.get_sub_app(RenderApp).is_none());
    }
    #[test]
    fn exactly120_ordered_expected_tickets_with_all_stages_required() {
        let mut s = Ledger::default();
        for i in 0..120 {
            let ticket = t(i);
            assert_eq!(s.observe(ticket), Ok(Some(i as usize)));
            finish(&mut s, i as usize, ticket);
        }
        assert!(s.complete());
        assert!(s.observe(t(120)).is_err());
        s.rows[74].stage = Stage::Prepared;
        assert!(!s.complete());
    }
    #[test]
    fn bmw_expected90_does_not_silently_change_derby120_or_admit_incomplete_rows() {
        let mut bmw = Ledger {
            profile: Profile::BmwIstanbul90,
            ..default()
        };
        let mut derby = Ledger::default();
        for i in 0..90 {
            let ticket = Ticket {
                tick: 190 + u64::from(i),
                ..t(i)
            };
            bmw.observe(ticket).unwrap();
            finish(&mut bmw, i as usize, ticket);
            derby.observe(t(i)).unwrap();
            finish(&mut derby, i as usize, t(i));
        }
        assert!(bmw.complete());
        assert!(!derby.complete());
        assert!(bmw.observe(Ticket { tick: 280, ..t(90) }).is_err());
        bmw.rows[53].stage = Stage::Replayed;
        assert!(!bmw.complete());
        for i in 90..120 {
            derby.observe(t(i)).unwrap();
            finish(&mut derby, i as usize, t(i));
        }
        assert!(derby.complete());
    }
    #[test]
    fn lost_entire_stage_duplicate_replay_or_wrong_frame_cannot_use_previous_success() {
        let mut s = Ledger::default();
        s.observe(t(0)).unwrap();
        finish(&mut s, 0, t(0));
        s.observe(t(1)).unwrap();
        assert!(
            s.advance(1, t(1), Stage::Prepared, Stage::Replayed, "")
                .is_err()
        );
        assert!(
            s.advance(0, t(0), Stage::Replayed, Stage::Restored, "")
                .is_err()
        );
        let wrong = Ticket { frame: 999, ..t(1) };
        assert!(
            s.advance(1, wrong, Stage::Extracted, Stage::Barrier, "")
                .is_err()
        );
        assert!(!s.complete());
    }
    #[test]
    fn pending_dedup_out_of_order_rollback_and_previous_incomplete_are_distinct() {
        let mut s = Ledger::default();
        s.observe(t(0)).unwrap();
        assert_eq!(s.observe(Ticket { frame: 999, ..t(0) }), Ok(None));
        assert!(
            s.observe(Ticket {
                entity: 999,
                frame: 199,
                ..t(1)
            })
            .is_err()
        );
        assert!(s.observe(Ticket { tick: 349, ..t(1) }).is_err());
        s.observe(t(1)).unwrap();
        assert!(
            s.advance(1, t(1), Stage::Extracted, Stage::Barrier, "")
                .is_err()
        );
        s.fail("selected unready");
        assert!(
            s.advance(0, t(0), Stage::Extracted, Stage::Barrier, "")
                .is_err()
        );
        assert!(!s.complete());
    }
}

#[cfg(test)]
mod automatic_arbitration_tests {
    use super::*;
    #[test]
    fn actual_parser_presence_keeps_explicit_off_controls_out_of_automatic_installation() {
        let ordinary=["--drive","--settings","event.ini","--aa","dlaa","--glass-pilot","base-mip-zero","--window-sort","current","--car-transparent-sort","current"];
        let explicit=ordinary.iter().any(|a|explicit_policy_arg(a));
        assert!(automatic_selected(true,true,true,true,explicit));
        for args in [["--smoke-boundary","off"],["--native-smoke-boundary","off"],
            ["--smoke-ct","off"],["--smoke-compositor","off"],["--ground-skid-pass","off"],
            ["--skid-order","off"],["--smoke-order","groups"]] {
            let explicit=args.iter().any(|a|explicit_policy_arg(a));
            assert!(!automatic_selected(true,true,true,true,explicit));
        }
        assert!(!automatic_selected(false,true,true,true,false));
        assert!(!automatic_selected(true,false,true,true,false));
        assert!(!automatic_selected(true,true,false,true,false));
        assert!(!automatic_selected(true,true,true,false,false));
    }
}
