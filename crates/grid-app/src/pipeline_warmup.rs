//! Pipeline warm-up at scene start (owner 2026-11-03: the frame "blinks" at
//! the start of the Washington race; docs/PIPELINE_WARMUP.md).
//!
//! A window compiles render pipelines in the background (Bevy's default), and
//! a draw whose pipeline is still compiling is skipped (`SetItemPipeline`
//! returns `Skip`). Pipelines are made on first use, so the first fade of a
//! material (the `VISIBILITY_RANGE_DITHER` variant, F703/F708), its masked
//! prepass, a shadow proxy (F713) or a material first seen mid-race hid the
//! objects using it until the compile finished, both levels of a fading
//! object included. [`Warmup`] makes those pipelines at the start, behind a
//! black cover: every mesh is drawn without frustum culling, every hidden
//! mesh (and hidden parent) is shown and every ornament level is drawn
//! (`ornament_lod::select`) until the pipeline cache is quiet; then every mesh also gets a step-0 fade range (drawn whole, but
//! with the dither variant) and every shadow-level part a step-0 shadow proxy
//! until it is quiet again; then all of that is undone and the cover lifts
//! once the cache is quiet a third time. `GRID_PIPELINE_WARMUP=0` skips it.
//!
//! Research: `GRID_PIPELINE_TRACE=1` prints each new pipeline (label and key
//! defines), how many frames it waited and the frames that had any waiting;
//! `GRID_CAPTURE_ASYNC_PIPELINES=1` makes an offscreen capture compile in the
//! background as a window does. With the trace, each new pipeline is also
//! attributed to up to three entities that use it in the main, prepass or
//! shadow pass (`pipeline_user` lines: ornament level, car level part, shadow
//! proxy or body, tree, name; row 115).
use bevy::camera::visibility::{NoFrustumCulling, VisibilityRange, VisibilitySystems};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::{render_resource::PipelineCache, Render, RenderApp, RenderSystems};
use bevy::shader::ShaderDefVal;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Frames without a new or waiting pipeline that end a phase.
const QUIET_FRAMES: u32 = 8;
/// The cover lifts after this many frames even if pipelines keep coming.
const MAX_FRAMES: u32 = 900;

fn env_on(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1")
}

/// `GRID_CAPTURE_ASYNC_PIPELINES=1`: offscreen captures keep background compilation.
pub fn capture_async() -> bool {
    env_on("GRID_CAPTURE_ASYNC_PIPELINES")
}

/// The render world's pipeline cache as the main world sees it: waiting and
/// total pipelines after the last rendered frame.
#[derive(Resource, Clone, Default)]
struct Counters(Arc<[AtomicUsize; 2]>);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    /// No viewer camera or mesh yet.
    #[default]
    Scene,
    /// Everything drawn, unculled.
    Plain,
    /// Also with the fade (dither) and shadow-proxy variants.
    Dither,
    /// Undone; the cover stays until the cache is quiet.
    Restore,
    Done,
}

#[derive(Resource, Default)]
pub struct Warmup {
    phase: Phase,
    frames: u32,
    quiet: u32,
    total: usize,
    cover: Option<Entity>,
    unculled: Vec<Entity>,
    ranged: Vec<Entity>,
    /// Entities shown for the warm-up, with the visibility their own systems
    /// last gave them (restored at the end).
    shown: HashMap<Entity, Visibility>,
}

impl Warmup {
    /// The cover was removed and temporary visibility/range changes restored.
    /// Update consumers observe this on the frame after PostUpdate completion.
    pub fn ready(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Every ornament level is drawn (`ornament_lod::select`).
    pub fn showing_all(&self) -> bool {
        matches!(self.phase, Phase::Plain | Phase::Dither)
    }

    /// Every shadow-level part casts through a step-0 proxy (`ornament_lod::shadow_proxies`).
    pub fn proxies(&self) -> bool {
        self.phase == Phase::Dither
    }

    /// The scene is back to its own visibility and the cover still up.
    pub fn restoring(&self) -> bool {
        self.phase == Phase::Restore
    }

    /// Read in Update: this frame's `step` lifts the cover if the frame
    /// turns out quiet (or the frame limit is reached).
    pub fn finishing(&self) -> bool {
        self.phase == Phase::Restore
            && (self.quiet + 1 >= QUIET_FRAMES || self.frames >= MAX_FRAMES)
    }
}

pub fn install(app: &mut App) {
    let counters = Counters::default();
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render.insert_resource(counters.clone()).add_systems(Render, count.in_set(RenderSystems::Cleanup));
    if env_on("GRID_PIPELINE_TRACE") {
        let users = Users::default();
        render.insert_resource(users.clone()).add_systems(Render, trace.in_set(RenderSystems::Cleanup));
        app.insert_resource(users).add_systems(Last, (describe_users, missed_casters, cascade_census));
    }
    if std::env::var("GRID_PIPELINE_WARMUP").is_ok_and(|v| v == "0") {
        return;
    }
    app.insert_resource(counters).init_resource::<Warmup>().add_systems(
        PostUpdate,
        step.after(crate::ornament_lod::select).before(VisibilitySystems::VisibilityPropagate),
    );
}

fn count(cache: Res<PipelineCache>, counters: Res<Counters>) {
    counters.0[0].store(cache.waiting_pipelines().count(), Ordering::Relaxed);
    counters.0[1].store(cache.pipelines().count(), Ordering::Relaxed);
}

#[allow(clippy::type_complexity)]
fn step(
    mut warm: ResMut<Warmup>,
    counters: Res<Counters>,
    cameras: Query<Entity, With<crate::ViewerCamera>>,
    meshes: Query<(Entity, Has<NoFrustumCulling>, Option<&VisibilityRange>), With<Mesh3d>>,
    mut visibility: Query<(Entity, &mut Visibility), (Without<Node>, Or<(With<Mesh3d>, With<Children>)>)>,
    mut commands: Commands,
) {
    let warm = &mut *warm;
    if warm.phase == Phase::Done {
        return;
    }
    let (waiting, total) = (counters.0[0].load(Ordering::Relaxed), counters.0[1].load(Ordering::Relaxed));
    if waiting == 0 && total == warm.total {
        warm.quiet += 1;
    } else {
        warm.quiet = 0;
        warm.total = total;
    }
    let next = warm.quiet >= QUIET_FRAMES || warm.frames >= MAX_FRAMES;
    let before = warm.phase;
    match warm.phase {
        Phase::Scene => {
            let Some(camera) = cameras.iter().next() else {
                return;
            };
            if meshes.is_empty() {
                return;
            }
            warm.cover = Some(
                commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.),
                            height: Val::Percent(100.),
                            ..default()
                        },
                        BackgroundColor(Color::BLACK),
                        GlobalZIndex(i32::MAX),
                        UiTargetCamera(camera),
                    ))
                    .id(),
            );
            warm.phase = Phase::Plain;
        }
        Phase::Plain | Phase::Dither => {
            let dither = warm.phase == Phase::Dither;
            // Another system's change since the last frame is what it wants
            // after the warm-up; this system's own writes are not changes here.
            for (entity, mut shown) in &mut visibility {
                let wanted = *shown;
                match warm.shown.get_mut(&entity) {
                    Some(record) if shown.is_changed() => *record = wanted,
                    Some(_) => {}
                    None if wanted == Visibility::Hidden => {
                        warm.shown.insert(entity, wanted);
                    }
                    None => continue,
                }
                shown.set_if_neq(Visibility::Inherited);
            }
            for (entity, unculled, range) in &meshes {
                if !unculled {
                    commands.entity(entity).try_insert(NoFrustumCulling);
                    warm.unculled.push(entity);
                }
                if dither && range.is_none() {
                    commands.entity(entity).try_insert(crate::ornament_lod::fade_range(false, 0));
                    warm.ranged.push(entity);
                }
            }
            if next {
                if dither {
                    // A fade that began meanwhile keeps its own range.
                    let mine = crate::ornament_lod::fade_range(false, 0);
                    for entity in warm.ranged.drain(..) {
                        if meshes.get(entity).is_ok_and(|(_, _, range)| range == Some(&mine)) {
                            commands.entity(entity).try_remove::<VisibilityRange>();
                        }
                    }
                    for entity in warm.unculled.drain(..) {
                        commands.entity(entity).try_remove::<NoFrustumCulling>();
                    }
                    for (entity, wanted) in warm.shown.drain() {
                        if let Ok((_, mut shown)) = visibility.get_mut(entity) {
                            shown.set_if_neq(wanted);
                        }
                    }
                    warm.phase = Phase::Restore;
                } else {
                    warm.phase = Phase::Dither;
                }
            }
        }
        Phase::Restore => {
            if next {
                if let Some(cover) = warm.cover.take() {
                    commands.entity(cover).try_despawn();
                }
                warm.phase = Phase::Done;
            }
        }
        Phase::Done => {}
    }
    warm.frames += 1;
    if warm.phase != before {
        warm.quiet = 0;
        println!("pipeline_warmup phase={:?} frame={} pipelines={total}", warm.phase, warm.frames);
    }
}

/// New pipelines' users found in the render world, described by the main world.
#[derive(Resource, Clone, Default)]
struct Users(Arc<std::sync::Mutex<Vec<PipelineUser>>>, Arc<std::sync::Mutex<bevy::platform::collections::HashSet<Entity>>>);

/// (frame, pipeline id, pass, main-world entity).
type PipelineUser = (u64, usize, &'static str, Entity);

/// Up to three users per pass of each pipeline in `ids`.
fn find_users(
    ids: &[usize],
    main: Option<&bevy::pbr::SpecializedMaterialPipelineCache>,
    prepass: Option<&bevy::pbr::SpecializedPrepassMaterialPipelineCache>,
    shadow: Option<&bevy::pbr::SpecializedShadowMaterialPipelineCache>,
) -> Vec<(usize, &'static str, Entity)> {
    let mut found: Vec<(usize, &'static str, Entity)> = Vec::new();
    let mut add = |id: usize, pass: &'static str, entity: Entity| {
        if ids.contains(&id)
            && found.iter().filter(|f| f.0 == id && f.1 == pass).count() < 3
            && !found.contains(&(id, pass, entity))
        {
            found.push((id, pass, entity));
        }
    };
    for view in main.iter().flat_map(|c| c.values()) {
        for (entity, pipeline) in view.iter() {
            add(pipeline.id(), "main", entity.id());
        }
    }
    for view in prepass.iter().flat_map(|c| c.values()) {
        for (entity, (_, pipeline, _)) in view.iter() {
            add(pipeline.id(), "prepass", entity.id());
        }
    }
    for view in shadow.iter().flat_map(|c| c.values()) {
        for (entity, (pipeline, _)) in view.iter() {
            add(pipeline.id(), "shadow", entity.id());
        }
    }
    found
}

/// At the end of the warm-up: shadow-level ornament parts that never got a
/// shadow pipeline while it ran (row 115).
#[allow(clippy::type_complexity)]
fn missed_casters(
    users: Res<Users>,
    warm: Option<Res<Warmup>>,
    parts: Query<(
        Entity,
        Option<&Name>,
        &crate::ornament_lod::OrnamentLevel,
        &Visibility,
        &InheritedVisibility,
        Has<bevy::camera::primitives::Aabb>,
        Has<bevy::light::NotShadowCaster>,
        &bevy::camera::visibility::RenderLayers,
        &Mesh3d,
    )>,
    meshes: Res<Assets<Mesh>>,
    mut done: Local<u32>,
) {
    if !warm.is_some_and(|w| w.ready()) || *done > 2 {
        return;
    }
    *done += 1;
    if *done < 2 {
        return;
    }
    let cast = users.1.lock().unwrap();
    let missed: Vec<_> = parts.iter().filter(|p| !cast.contains(&p.0)).collect();
    println!("pipeline_missed_casters parts={} missed={}", parts.iter().count(), missed.len());
    for (e, name, level, vis, inherited, aabb, shadowless, layers, mesh) in missed.iter().take(12) {
        println!(
            "pipeline_missed entity={e} name={:?} ornament=({}, {}) visibility={vis:?} inherited={} aabb={aabb} not_shadow_caster={shadowless} layers={:?} mesh_loaded={}",
            name.map(|n| n.as_str()), level.instance, level.level, inherited.get(), layers.iter().collect::<Vec<_>>(), meshes.contains(&mesh.0)
        );
    }
}

/// Each warm-up frame: shadow-level ornament parts that are casters (visible,
/// no `NotShadowCaster`), how many the sun's cascades list, and how many have
/// had a shadow pipeline so far (row 115).
#[allow(clippy::type_complexity)]
fn cascade_census(
    users: Res<Users>,
    warm: Option<Res<Warmup>>,
    parts: Query<(Entity, &InheritedVisibility, Has<bevy::light::NotShadowCaster>, Has<NoFrustumCulling>, Option<&VisibilityRange>), (With<crate::ornament_lod::OrnamentLevel>, With<bevy::camera::visibility::RenderLayers>)>,
    lights: Query<&bevy::camera::visibility::CascadesVisibleEntities>,
) {
    let Some(warm) = warm else { return };
    if warm.ready() {
        return;
    }
    let listed: bevy::platform::collections::HashSet<Entity> = lights
        .iter()
        .flat_map(|c| c.entities.values())
        .flat_map(|cascades| cascades.iter())
        .flat_map(|v| v.entities.iter().copied())
        .collect();
    let cast = users.1.lock().unwrap();
    let (mut casters, mut in_cascades, mut piped, mut unculled, mut ranged) = (0, 0, 0, 0, 0);
    for (e, inherited, shadowless, nfc, range) in &parts {
        if !inherited.get() || shadowless {
            continue;
        }
        casters += 1;
        in_cascades += usize::from(listed.contains(&e));
        piped += usize::from(cast.contains(&e));
        unculled += usize::from(nfc);
        ranged += usize::from(range.is_some());
    }
    println!(
        "pipeline_census frame={} phase={:?} parts={} casters={casters} unculled={unculled} ranged={ranged} in_cascades={in_cascades} with_shadow_pipeline={piped} cascade_entities={}",
        warm.frames, warm.phase, parts.iter().count(), listed.len()
    );
}

#[allow(clippy::type_complexity)]
fn describe_users(
    users: Res<Users>,
    parts: Query<(
        Option<&Name>,
        Option<&crate::ornament_lod::OrnamentLevel>,
        Option<&crate::car_batch::LodPart>,
        Has<crate::car_batch::ShadowBody>,
        Has<bevy::mesh::MeshTag>,
        Has<crate::source_trees::Facing>,
        Has<bevy::light::NotShadowCaster>,
        Option<&bevy::camera::visibility::RenderLayers>,
        Option<&ChildOf>,
    )>,
    names: Query<&Name>,
) {
    let found = std::mem::take(&mut *users.0.lock().unwrap());
    for (frame, id, pass, entity) in found {
        let Ok((name, ornament, lod, body, tag, tree, shadowless, layers, parent)) = parts.get(entity) else {
            println!("pipeline_user frame={frame} id={id} pass={pass} entity={entity} gone");
            continue;
        };
        let parent = parent.and_then(|p| names.get(p.parent()).ok()).map_or(String::new(), |n| n.to_string());
        println!(
            "pipeline_user frame={frame} id={id} pass={pass} entity={entity} name={:?} parent={parent:?} ornament={:?} car_level={:?} shadow_body={body} mesh_tag={tag} tree={tree} not_shadow_caster={shadowless} layers={:?}",
            name.map(|n| n.as_str()),
            ornament.map(|o| (o.instance, o.level)),
            lod.map(|l| l.level),
            layers.map(|l| l.iter().collect::<Vec<_>>()),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn trace(
    cache: Res<PipelineCache>,
    mut frame: Local<u64>,
    mut created: Local<usize>,
    mut since: Local<HashMap<usize, u64>>,
    users: Res<Users>,
    main: Option<Res<bevy::pbr::SpecializedMaterialPipelineCache>>,
    prepass: Option<Res<bevy::pbr::SpecializedPrepassMaterialPipelineCache>>,
    shadow: Option<Res<bevy::pbr::SpecializedShadowMaterialPipelineCache>>,
) {
    use bevy::render::render_resource::PipelineDescriptor;
    *frame += 1;
    let frame = *frame;
    if let Some(shadow) = shadow.as_deref() {
        let mut cast = users.1.lock().unwrap();
        for view in shadow.values() {
            cast.extend(view.keys().map(|e| e.id()));
        }
    }
    let fresh: Vec<usize> = (*created..cache.pipelines().count()).collect();
    if !fresh.is_empty() {
        let found = find_users(&fresh, main.as_deref(), prepass.as_deref(), shadow.as_deref());
        users.0.lock().unwrap().extend(found.into_iter().map(|(id, pass, e)| (frame, id, pass, e)));
    }
    let total = cache.pipelines().count();
    let waiting: Vec<usize> = cache.waiting_pipelines().collect();
    for (id, pipeline) in cache.pipelines().enumerate().skip(*created) {
        let (label, defs) = match &pipeline.descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(d) => (
                d.label.clone().unwrap_or_default(),
                d.fragment.as_ref().map_or(&d.vertex.shader_defs, |f| &f.shader_defs).iter().map(|d| match d {
                    ShaderDefVal::Bool(n, _) => n.clone(),
                    ShaderDefVal::Int(n, v) => format!("{n}={v}"),
                    ShaderDefVal::UInt(n, v) => format!("{n}={v}"),
                }).filter(|n| env_on("GRID_PIPELINE_TRACE_ALL") || KEYS.iter().any(|k| n.contains(k))).collect::<Vec<_>>().join(","),
            ),
            PipelineDescriptor::ComputePipelineDescriptor(d) => (d.label.clone().unwrap_or_default(), String::new()),
        };
        println!("pipeline_new frame={frame} id={id} label={label:?} defs={defs}");
        since.insert(id, frame);
    }
    *created = total;
    since.retain(|id, start| {
        let pending = waiting.contains(id);
        if !pending {
            println!("pipeline_ready frame={frame} id={id} waited_frames={}", frame - *start);
        }
        pending
    });
    if !waiting.is_empty() {
        println!("pipeline_pending frame={frame} waiting={} total={total}", waiting.len());
    }
}

const KEYS: [&str; 12] = ["DITHER", "PREPASS", "SHADOW", "MAY_DISCARD", "BLEND", "OIT", "SKINNED", "MORPH", "MULTISAMPLED", "ALPHA", "DEFERRED", "MOTION"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warm_up_draws_unculled_then_dithered_then_restores_and_lifts_the_cover() {
        let mut app = App::new();
        let counters = Counters::default();
        app.insert_resource(counters.clone()).init_resource::<Warmup>().add_systems(PostUpdate, step);
        let frame = |app: &mut App| app.world_mut().run_schedule(PostUpdate);
        let phase = |app: &App| app.world().resource::<Warmup>().phase;
        let covers = |app: &mut App| app.world_mut().query::<&Node>().iter(app.world()).count();

        // Nothing to warm before the scene exists.
        frame(&mut app);
        assert_eq!(phase(&app), Phase::Scene);
        assert!(!app.world().resource::<Warmup>().ready());
        app.world_mut().spawn(crate::ViewerCamera);
        let mesh = app.world_mut().spawn(Mesh3d(Handle::default())).id();
        frame(&mut app);
        assert_eq!(phase(&app), Phase::Plain);
        assert!(!app.world().resource::<Warmup>().ready());
        assert_eq!(covers(&mut app), 1);
        frame(&mut app);
        assert!(app.world().get::<NoFrustumCulling>(mesh).is_some());
        assert!(app.world().get::<VisibilityRange>(mesh).is_none());
        // Hidden meshes are shown meanwhile; what their systems set is kept.
        let hidden = app.world_mut().spawn((Mesh3d(Handle::default()), Visibility::Hidden)).id();
        let toggled = app.world_mut().spawn((Mesh3d(Handle::default()), Visibility::Hidden)).id();
        frame(&mut app);
        assert_eq!(app.world().get::<Visibility>(hidden), Some(&Visibility::Inherited));
        *app.world_mut().get_mut::<Visibility>(toggled).unwrap() = Visibility::Visible;
        frame(&mut app);
        assert_eq!(app.world().get::<Visibility>(toggled), Some(&Visibility::Inherited));

        // Pipelines still coming keep the phase; a quiet cache ends it.
        counters.0[1].store(5, Ordering::Relaxed);
        for _ in 0..QUIET_FRAMES {
            frame(&mut app);
        }
        assert_eq!(phase(&app), Phase::Plain);
        assert!(!app.world().resource::<Warmup>().ready());
        frame(&mut app);
        assert_eq!(phase(&app), Phase::Dither);
        assert!(!app.world().resource::<Warmup>().ready());
        frame(&mut app);
        assert!(app.world().get::<VisibilityRange>(mesh) == Some(&crate::ornament_lod::fade_range(false, 0)));

        // A waiting pipeline also holds it.
        counters.0[0].store(1, Ordering::Relaxed);
        for _ in 0..2 * QUIET_FRAMES {
            frame(&mut app);
        }
        assert_eq!(phase(&app), Phase::Dither);
        assert!(!app.world().resource::<Warmup>().ready());
        counters.0[0].store(0, Ordering::Relaxed);
        for _ in 0..=QUIET_FRAMES {
            frame(&mut app);
        }
        assert_eq!(phase(&app), Phase::Restore);
        assert!(!app.world().resource::<Warmup>().ready());
        frame(&mut app);
        assert!(app.world().get::<VisibilityRange>(mesh).is_none());
        assert!(app.world().get::<NoFrustumCulling>(mesh).is_none());
        assert_eq!(app.world().get::<Visibility>(hidden), Some(&Visibility::Hidden));
        assert_eq!(app.world().get::<Visibility>(toggled), Some(&Visibility::Visible));
        assert_eq!(covers(&mut app), 1, "the cover stays until the restored scene is compiled");
        for _ in 0..=QUIET_FRAMES {
            frame(&mut app);
        }
        assert_eq!(phase(&app), Phase::Done);
        assert!(app.world().resource::<Warmup>().ready());
        assert_eq!(covers(&mut app), 0);
    }
    #[test]
    fn actual_offscreen_capture_waits_for_postupdate_cover_removal_but_missing_warmup_is_ready() {
        use bevy::render::view::screenshot::Screenshot;
        fn capture_app() -> App {
            let mut app = App::new();
            app.add_message::<bevy::app::AppExit>()
                .insert_resource(crate::graphics::Settings::default())
                .insert_resource(crate::Capture {
                    path: "local/UNWRITTEN-capture-warmup-regression.png".into(),
                    target: Some(Handle::default()),
                    frames: 179,
                    requested: false,
                    started: std::time::Instant::now(),
                })
                .add_systems(Update, crate::capture);
            app
        }
        // A missing resource preserves the existing no-warmup capture contract.
        let mut absent = capture_app();
        absent.update();
        assert!(absent.world().resource::<crate::Capture>().requested);
        assert_eq!(absent.world_mut().query::<&Screenshot>().iter(absent.world()).count(), 1);

        let mut app = capture_app();
        let camera = app.world_mut().spawn(crate::ViewerCamera).id();
        app.world_mut().spawn(Mesh3d(Handle::default()));
        let cover = app.world_mut().spawn((Node::default(), UiTargetCamera(camera))).id();
        app.insert_resource(Counters::default())
            .insert_resource(Warmup {
                phase: Phase::Restore,
                quiet: QUIET_FRAMES - 1,
                cover: Some(cover),
                ..default()
            })
            .add_systems(PostUpdate, step);
        // Actual capture(Update) runs before actual step(PostUpdate). This frame
        // reaches Done and flushes the cover despawn, but cannot request an image.
        app.update();
        assert!(!app.world().resource::<crate::Capture>().requested);
        assert!(app.world().resource::<Warmup>().ready());
        assert!(app.world().get_entity(cover).is_err());
        assert_eq!(app.world_mut().query::<&Screenshot>().iter(app.world()).count(), 0);
        // Only the following Update can queue Screenshot, with no cover entity.
        app.update();
        assert!(app.world().resource::<crate::Capture>().requested);
        assert!(app.world().get_entity(cover).is_err());
        assert_eq!(app.world_mut().query::<&Screenshot>().iter(app.world()).count(), 1);
        // No RenderApp/GPU/ScreenshotCaptured event: no image or file is written.
    }

    #[test]
    fn consecutive_temporal_latch_and_transport_wait_for_actual_warmup_completion() {
        use crate::temporal_audit::{
            PairCapture, PairStamp, PairWarmup, pair_transport_ready, sample_pair_warmup,
        };
        use bevy::diagnostic::{FrameCount, FrameCountPlugin};
        let mut app = App::new();
        app.add_plugins(FrameCountPlugin)
            .init_resource::<PairWarmup>()
            .add_systems(Update, sample_pair_warmup);
        let camera = app.world_mut().spawn(crate::ViewerCamera).id();
        app.world_mut().spawn(Mesh3d(Handle::default()));
        let cover = app
            .world_mut()
            .spawn((Node::default(), UiTargetCamera(camera)))
            .id();
        app.insert_resource(Counters::default())
            .insert_resource(Warmup {
                phase: Phase::Restore,
                quiet: QUIET_FRAMES - 1,
                cover: Some(cover),
                ..default()
            })
            .add_systems(PostUpdate, step);
        let pair = PairCapture::default();
        let try_stage = |app: &App, stage| {
            // FrameCount increments in Last; use the sampled Update's count.
            let source_frame = app.world().resource::<FrameCount>().0 - 1;
            let stamp = PairStamp::new(
                source_frame,
                1652,
                source_frame + 1,
                Entity::PLACEHOLDER,
                Mat4::IDENTITY,
                Mat4::IDENTITY,
                Mat4::IDENTITY,
                UVec2::new(1920, 1080),
                UVec2::new(704, 348),
                UVec2::new(512, 384),
                Some(Vec2::ZERO),
            );
            pair.select_ready(1652, stamp, stage, app.world().get_resource::<PairWarmup>())
        };
        // Done and cover despawn happen in PostUpdate; earlier admission stays false.
        app.update();
        assert!(app.world().resource::<Warmup>().ready());
        assert!(app.world().get_entity(cover).is_err());
        for stage in [1, 2, 4] {
            assert!(try_stage(&app, stage).is_err());
        }
        let previous_frame = app.world().resource::<FrameCount>().0 - 1;
        assert!(!pair_transport_ready(
            app.world().get_resource::<PairWarmup>(),
            previous_frame
        ));
        assert!(!pair_transport_ready(None, previous_frame));
        // Failed admission consumed no stage bits: all first-frame slots remain.
        app.update();
        for stage in [1, 2, 4] {
            assert_eq!(try_stage(&app, stage), Ok(Some(0)));
        }
        let ready_frame = app.world().resource::<FrameCount>().0 - 1;
        assert!(pair_transport_ready(
            app.world().get_resource::<PairWarmup>(),
            ready_frame
        ));
        assert!(!pair_transport_ready(
            app.world().get_resource::<PairWarmup>(),
            previous_frame
        ));
        // Explicit no-warmup differs from a missing extracted readiness sample.
        let mut without_warmup = App::new();
        without_warmup
            .add_plugins(FrameCountPlugin)
            .init_resource::<PairWarmup>()
            .add_systems(Update, sample_pair_warmup);
        without_warmup.update();
        let no_warmup_frame = without_warmup.world().resource::<FrameCount>().0 - 1;
        assert!(pair_transport_ready(
            without_warmup.world().get_resource::<PairWarmup>(),
            no_warmup_frame
        ));
    }

}
