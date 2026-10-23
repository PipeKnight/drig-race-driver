//! Default-off one-frame CPU audit of actual main-view transparent queue order.
//! A phase snapshot is not a GPU fragment trace or final optimized draw list.
use bevy::{
    camera::primitives::Aabb,
    core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d},
    diagnostic::FrameCount,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        render_phase::{ViewSortedRenderPhases, sort_phase_system},
        render_resource::PipelineCache,
        view::ExtractedView,
    },
};
use std::{
    collections::HashMap,
    fmt::Write as FmtWrite,
    fs::{File, OpenOptions},
    io::Write as IoWrite,
    path::Path,
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target { Tick(u64), Frame(u32) }
impl Target {
    pub(crate) fn parse(text: &str) -> Result<Self, &'static str> {
        let (kind, value) = text.split_once(':').ok_or("--window-phase-audit needs tick:N or frame:N")?;
        let n: u64 = value.parse().map_err(|_| "invalid phase audit index")?;
        if !(1..=1_000_000).contains(&n) { return Err("phase audit index must be1..1000000"); }
        match kind {
            "tick" => Ok(Self::Tick(n)),
            "frame" => Ok(Self::Frame(n as u32)),
            _ => Err("--window-phase-audit needs tick:N or frame:N"),
        }
    }
    pub(crate) fn tick(self) -> Option<u64> {
        match self { Self::Tick(n) => Some(n), Self::Frame(_) => None }
    }
    fn reached(self, frame: u32, tick: Option<u64>) -> bool {
        match self { Self::Tick(n) => tick.is_some_and(|t| t >= n), Self::Frame(n) => frame >= n }
    }
}
#[derive(Resource, Clone)]
struct Audit {
    target: Target,
    output: Arc<Mutex<File>>,
    done: Arc<AtomicBool>,
}
#[derive(Clone, Debug)]
struct Meta {
    name: String,
    source_part: Option<String>,
    parent: Option<Entity>,
    mesh: String,
    material: String,
    aabb_local: Option<(Vec3, Vec3)>,
    aabb_world_centre: Option<Vec3>,
    world_from_local: Option<Mat4>,
}
impl Meta {
    fn describe(&self) -> String {
        format!("name={:?} source_part={:?} parent={:?} mesh={} material={} aabb_local={:?} aabb_world_centre={:?} world_from_local={:?}",
            self.name, self.source_part, self.parent, self.mesh, self.material,
            self.aabb_local, self.aabb_world_centre, self.world_from_local)
    }
}
#[derive(Resource, Default)]
struct Snapshot {
    audit: Option<Audit>,
    frame: u32,
    tick: Option<u64>,
    main_camera: Option<Entity>,
    metadata: HashMap<Entity, Meta>,
}

pub(crate) fn install(app: &mut App, target: Target, capture: &Path) -> std::io::Result<()> {
    if app.get_sub_app(RenderApp).is_none() {
        return Err(std::io::Error::other("window phase audit needs a renderer"));
    }
    let local = Path::new("local").canonicalize()?;
    let parent = capture.parent().ok_or_else(|| std::io::Error::other("capture parent absent"))?.canonicalize()?;
    if !parent.starts_with(local) {
        return Err(std::io::Error::other("window phase evidence must stay under ignored local/"));
    }
    let stem = capture.file_stem().ok_or_else(|| std::io::Error::other("capture stem absent"))?;
    let output = parent.join(format!("{}-window-phase.txt", stem.to_string_lossy()));
    let mut file = OpenOptions::new().write(true).create_new(true).open(&output)?;
    writeln!(file, "window_phase version=1 status=pending target={target:?} scope=sorted-cpu-phase-before-batching")?;
    file.flush()?;
    app.insert_resource(Audit { target, output: Arc::new(Mutex::new(file)), done: Arc::default() });
    let render = app.get_sub_app_mut(RenderApp).expect("renderer checked");
    render.init_resource::<Snapshot>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(Render, write_phase.in_set(RenderSystems::PhaseSort)
            .after(sort_phase_system::<Transparent3d>));
    println!("window_phase enabled=true target={target:?} output={} writes=one-main-view-phase default=false", output.display());
    Ok(())
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)] // ECS extraction; optional typed identities; all mesh entities, not only windows.
fn extract(
    audit: Extract<Option<Res<Audit>>>,
    frame: Extract<Res<FrameCount>>,
    driving: Extract<Option<Res<super::Driving>>>,
    cameras: Extract<Query<Entity, With<super::ViewerCamera>>>,
    parts: Extract<Query<(
        Entity, &Mesh3d, Option<&Name>, Option<&super::track_surface::SourcePart>,
        Option<&ChildOf>, Option<&GlobalTransform>, Option<&Aabb>,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&MeshMaterial3d<super::window_response::Window>>,
    )>>,
    standard_assets: Extract<Res<Assets<StandardMaterial>>>,
    window_assets: Extract<Option<Res<Assets<super::window_response::Window>>>>,
    mut snapshot: ResMut<Snapshot>,
) {
    snapshot.audit = None;
    let Some(audit) = audit.as_ref() else { return };
    let tick = driving.as_ref().map(|d| d.runtime.ticks);
    if audit.done.load(Ordering::Acquire) || !audit.target.reached(frame.0, tick) { return; }
    let Ok(camera) = cameras.single() else { return; };
    snapshot.metadata.clear();
    for (entity, mesh, name, source_part, parent, global, bounds, standard, window) in &parts {
        let material = if let Some(window) = window {
            let properties = window_assets.as_ref().and_then(|assets| assets.get(&window.0))
                .map(|material| format!("base_alpha={:?} base_sort_bias={} cull={:?} double_sided={}",
                    material.base.alpha_mode, material.base.depth_bias, material.base.cull_mode, material.base.double_sided));
            format!("window:{:?} properties={properties:?}", window.0.id())
        } else if let Some(standard) = standard {
            let properties = standard_assets.get(&standard.0)
                .map(|material| format!("alpha={:?} sort_bias={} cull={:?} double_sided={}",
                    material.alpha_mode, material.depth_bias, material.cull_mode, material.double_sided));
            format!("standard:{:?} properties={properties:?}", standard.0.id())
        } else {
            // Other ExtendedMaterial types are deliberately not guessed from their pipeline.
            "unmapped-typed-material".into()
        };
        snapshot.metadata.insert(entity, Meta {
            name: name.map_or_else(String::new, |v| v.as_str().to_owned()),
            source_part: source_part.map(|v| v.0.clone()),
            parent: parent.map(ChildOf::parent),
            mesh: format!("{:?}", mesh.0.id()),
            material,
            aabb_local: bounds.map(|v| (v.center.into(), v.half_extents.into())),
            aabb_world_centre: bounds.zip(global).map(|(b, g)| g.transform_point(Vec3::from(b.center))),
            world_from_local: global.map(GlobalTransform::to_matrix),
        });
    }
    snapshot.frame = frame.0;
    snapshot.tick = tick;
    snapshot.main_camera = Some(camera);
    snapshot.audit = Some((**audit).clone());
}

fn write_phase(
    snapshot: Res<Snapshot>,
    phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(Entity, &ExtractedView)>,
    pipelines: Res<PipelineCache>,
) {
    let Some(audit) = snapshot.audit.as_ref() else { return };
    if audit.done.load(Ordering::Acquire) { return; }
    let Some(main_camera) = snapshot.main_camera else { return };
    let Some((view_entity, view)) = views.iter().find(|(_, v)|
        v.retained_view_entity.main_entity.id() == main_camera
        && v.retained_view_entity.subview_index == 0)
    else { return; };
    let Some(phase) = phases.get(&view.retained_view_entity) else { return; };
    // Hard bound. A truncated list is an explicit failed audit, never a complete queue trace.
    const MAX_ITEMS: usize = 50_000;
    let mut text = String::new();
    let _ = writeln!(text, "window_phase_snapshot frame={} tick={:?} main_camera={main_camera:?} render_view={view_entity:?} retained={:?} items={} stage=after-transparent-sort-before-batching",
        snapshot.frame, snapshot.tick, view.retained_view_entity, phase.items.len());
    if phase.items.len() > MAX_ITEMS {
        let _ = writeln!(text, "window_phase status=failed reason=item-limit max={MAX_ITEMS}");
    } else {
        let rangefinder = view.rangefinder3d();
        let mut unmapped = 0;
        for (rank, (key, item)) in phase.items.iter().enumerate() {
            let main = item.entity.1.id();
            let meta = snapshot.metadata.get(&main);
            let parent = meta.and_then(|m| m.parent);
            let alias = parent.and_then(|p| snapshot.metadata.get(&p));
            if meta.is_none() { unmapped += 1; }
            let meta = meta.map(Meta::describe);
            let alias = alias.map(Meta::describe);
            let (centre, view_z, bias) = match item.sorting_info {
                TransparentSortingInfo3d::Sorted { mesh_center, depth_bias } =>
                    (Some(mesh_center), Some(rangefinder.distance(&mesh_center)), Some(depth_bias)),
                TransparentSortingInfo3d::AlwaysOnTop => (None, None, None),
            };
            // Safe for newly queued IDs: get_render_pipeline uses get-state with a queued fallback.
            // Do NOT call get_render_pipeline_descriptor unconditionally before process_queue.
            let ready = pipelines.get_render_pipeline(item.pipeline).is_some();
            let raster = if ready {
                {
                    let descriptor = pipelines.get_render_pipeline_descriptor(item.pipeline);
                    format!("label={:?} depth={:?}", descriptor.label, descriptor.depth_stencil)
                }
            } else { "not-ready-descriptor-not-read".into() };
            let _ = writeln!(text,
                "window_phase_item rank={rank} key={key:?} item_entity={:?} main={main:?} parent={parent:?} meta={meta:?} source_alias={alias:?} effective_centre={centre:?} view_z={view_z:?} sort_bias={bias:?} sorted_distance={} pipeline={:?} pipeline_ready={ready} raster={raster} draw_function={:?} indexed={} batch_range_before_prepare={:?}",
                item.entity, item.distance, item.pipeline, item.draw_function, item.indexed, item.batch_range);
        }
        let _ = writeln!(text, "window_phase status=complete frame={} tick={:?} items={} missing_main_metadata={unmapped} note=not-final-gpu-fragment-or-optimized-draw-order",
            snapshot.frame, snapshot.tick, phase.items.len());
    }
    audit.done.store(true, Ordering::Release); // Exactly one attempt, including a file failure.
    match audit.output.lock() {
        Ok(mut file) => {
            if let Err(error) = file.write_all(text.as_bytes()).and_then(|_| file.flush()) {
                error!("window_phase write failed: {error}");
            }
        }
        Err(error) => error!("window_phase file lock poisoned: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn targets_are_bounded_and_report_reached_frame_not_an_assumed_exact_tick() {
        assert_eq!(Target::parse("tick:1652"), Ok(Target::Tick(1652)));
        assert_eq!(Target::parse("frame:300"), Ok(Target::Frame(300)));
        for text in ["1652", "tick:0", "frame:1000001", "foo:1", "tick:-1"] {
            assert!(Target::parse(text).is_err());
        }
        assert!(!Target::Tick(10).reached(20, None));
        assert!(!Target::Tick(10).reached(20, Some(9)));
        assert!(Target::Tick(10).reached(20, Some(11)));
        assert!(Target::Frame(10).reached(10, None));
    }
}
