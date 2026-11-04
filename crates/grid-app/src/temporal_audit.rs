//! Opt-in bounded temporal input evidence; no presentation or physics feedback.
//! One bounded raw motion/depth capture; no presentation or physics feedback.
mod cpu_geometry;
use bevy::{
    asset::uuid_handle,
    core_pipeline::{
        prepass::{DepthPrepass, PreviousViewData, ViewPrepassTextures},
        schedule::camera_driver,
    },
    diagnostic::FrameCount,
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        camera::TemporalJitter,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{storage_buffer, texture_2d, texture_depth_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderGraph, RenderQueue},
        storage::{GpuShaderBuffer, ShaderBuffer},
        view::ExtractedView,
    },
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
const SHADER: Handle<Shader> = uuid_handle!("70b8d3a5-e833-4a58-8067-72ce928e728f");
const WIDTH: u32 = 512;
const HEIGHT: u32 = 384;
#[derive(Component, Clone, ExtractComponent)]
struct AuditView;
#[derive(Component)]
struct AuditOnce(usize, bool);
#[derive(Clone, Resource, ExtractResource)]
pub(crate) struct Audit {
    #[cfg(feature = "dlaa")]
    companion: Option<super::opaque_conservative_motion::replay_audit::Signals>,
    tick: u64,
    centre: Option<Vec2>,
    directory: PathBuf,
    buffers: Vec<Handle<ShaderBuffer>>,
    submitted: Vec<Arc<AtomicBool>>,
    readback_queued: Vec<bool>,
    complete: Vec<Arc<AtomicBool>>,
    failed: Arc<AtomicBool>,
    frozen: Arc<Mutex<Vec<Option<Frozen>>>>,
    pair: Option<PairCapture>,
}
impl Audit {
    pub(crate) fn ready(&self) -> bool {
        let complete = self
            .complete
            .iter()
            .all(|done| done.load(Ordering::Acquire));
        #[cfg(feature = "dlaa")]
        let complete = complete && self.companion.as_ref().is_none_or(|c| c.ready());
        complete
    }
    pub(crate) fn failed(&self) -> bool {
        let failed = self.failed.load(Ordering::Acquire);
        #[cfg(feature = "dlaa")]
        let failed = failed || self.companion.as_ref().is_some_and(|c| c.failed());
        failed
    }
}
#[derive(Clone)]
struct Frozen {
    header: [u32; 12],
    text: String,
    geometry: Option<cpu_geometry::Frozen>,
}
fn fail(audit: &Audit, message: &str) {
    if !audit.failed.swap(true, Ordering::AcqRel) {
        eprintln!("temporal_audit verified=false error={message}");
    }
}
#[derive(Clone, Resource, Default, ExtractResource)]
struct Snapshot {
    frame: u32,
    tick: u64,
    current_body: Mat4,
    previous_body: Mat4,
}
// Sample in Update, before warm-up's PostUpdate cover-removal transition.
// Never infer readiness from a missing main-world resource in the render world.
#[derive(Clone, Debug, Resource, Default, ExtractResource)]
pub(crate) struct PairWarmup {
    source_frame: u32,
    ready: bool,
}
pub(crate) fn sample_pair_warmup(
    frame: Res<FrameCount>,
    warmup: Option<Res<super::pipeline_warmup::Warmup>>,
    mut sampled: ResMut<PairWarmup>,
) {
    *sampled = PairWarmup {
        source_frame: frame.0,
        ready: warmup.as_ref().is_none_or(|warm| warm.ready()),
    };
}
pub(crate) fn pair_transport_ready(sampled: Option<&PairWarmup>, source_frame: u32) -> bool {
    sampled.is_some_and(|sampled| sampled.ready && sampled.source_frame == source_frame)
}
type AuditInputs = (
    Entity,
    &'static ViewPrepassTextures,
    &'static ExtractedView,
    Option<&'static TemporalJitter>,
    Option<&'static PreviousViewData>,
);
#[derive(Resource)]
struct Pipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedComputePipelineId,
}
#[derive(Clone, Copy, ShaderType)]
struct Parameters {
    identity: UVec4,
    dimensions_origin: UVec4,
    extent: UVec4,
}
// Called after renderer plugins, only for an explicit private scripted capture.
pub fn install(
    app: &mut App,
    tick: u64,
    directory: &Path,
    centre: Option<Vec2>,
) -> std::io::Result<()> {
    if !(1..=1_000_000).contains(&tick) || app.get_sub_app(RenderApp).is_none() {
        return Err(std::io::Error::other(
            "temporal audit requires a bounded positive tick and renderer",
        ));
    }
    let local = Path::new("local").canonicalize()?;
    let parent = directory
        .parent()
        .ok_or_else(|| std::io::Error::other("audit parent absent"))?
        .canonicalize()?;
    if !parent.starts_with(local) {
        return Err(std::io::Error::other(
            "temporal evidence must stay under ignored local/",
        ));
    }
    std::fs::create_dir(directory)?; // Never overwrite a previous audit.
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SHADER.id(),
            Shader::from_wgsl(include_str!("temporal_audit.wgsl"), "temporal_audit.wgsl"),
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let pair = pair_requested()
        .map_err(std::io::Error::other)?
        .then(PairCapture::default);
    if let Some(points) = cpu_geometry::requested().map_err(std::io::Error::other)? {
        if pair.is_none() {
            return Err(std::io::Error::other("CPU geometry requires the exact consecutive pair"));
        }
        cpu_geometry::install(app, points).map_err(std::io::Error::other)?;
    }
    let count = if pair.is_some() { 2 } else { 1 };
    let handles = (0..count)
        .map(|_| {
            let mut buffer = ShaderBuffer::from(vec![UVec4::ZERO; (3 + WIDTH * HEIGHT) as usize]);
            buffer.buffer_description.usage |= BufferUsages::COPY_SRC;
            app.world_mut()
                .resource_mut::<Assets<ShaderBuffer>>()
                .add(buffer)
        })
        .collect();
    #[cfg(feature = "dlaa")]
    let companion = super::opaque_conservative_motion::replay_audit::install(app, tick, directory)?;
    #[cfg(feature = "dlaa")]
    if pair.is_some() && companion.is_some() {
        return Err(std::io::Error::other(
            "consecutive raw/HDR cannot combine one-shot fragment companion",
        ));
    }
    app.insert_resource(Audit {
        #[cfg(feature = "dlaa")]
        companion,
        tick,
        centre,
        directory: directory.to_owned(),
        buffers: handles,
        submitted: (0..count).map(|_| Arc::default()).collect(),
        complete: (0..count).map(|_| Arc::default()).collect(),
        failed: Arc::default(),
        frozen: Arc::new(Mutex::new(vec![None; count])),
        readback_queued: vec![false; count],
        pair,
    })
    .init_resource::<Snapshot>()
    .init_resource::<PairWarmup>()
    .add_plugins((
        ExtractComponentPlugin::<AuditView>::default(),
        ExtractResourcePlugin::<Audit>::default(),
        ExtractResourcePlugin::<Snapshot>::default(),
        ExtractResourcePlugin::<PairWarmup>::default(),
    ))
    .add_systems(PostStartup, mark_view)
    .add_systems(
        PostUpdate,
        snapshot.after(bevy::transform::TransformSystems::Propagate),
    )
    .add_systems(Update, (sample_pair_warmup, queue_readback).chain());
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(RenderGraph, compute.after(camera_driver));
    }
    Ok(())
}
fn mark_view(mut commands: Commands, views: Query<Entity, With<super::ViewerCamera>>) {
    for entity in &views {
        commands.entity(entity).insert((AuditView, DepthPrepass));
    }
}
fn snapshot(
    frame: Res<FrameCount>,
    driving: Option<Res<super::Driving>>,
    mut current: ResMut<Snapshot>,
    mut previous: Local<Option<Mat4>>,
) {
    let Some(driving) = driving else { return };
    let pose = Mat4::from_cols_array(&driving.runtime.pose());
    *current = Snapshot {
        frame: frame.0,
        tick: driving.runtime.ticks,
        current_body: pose,
        previous_body: previous.unwrap_or(pose),
    };
    *previous = Some(pose);
}
fn init_pipeline(mut commands: Commands, cache: Res<PipelineCache>) {
    let layout = BindGroupLayoutDescriptor::new(
        "temporal input audit",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_depth_2d(),
                uniform_buffer::<Parameters>(false),
                storage_buffer::<Vec<UVec4>>(false),
            ),
        ),
    );
    let id = cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("bounded temporal input audit".into()),
        layout: vec![layout.clone()],
        shader: SHADER,
        ..default()
    });
    commands.insert_resource(Pipeline { layout, id });
}
#[cfg(feature = "dlaa")]
type AaInputs = (
    Option<&'static bevy::anti_alias::dlss::Dlss>,
    Option<
        &'static bevy::anti_alias::dlss::Dlss<bevy::anti_alias::dlss::DlssRayReconstructionFeature>,
    >,
    Option<&'static bevy::anti_alias::taa::TemporalAntiAliasing>,
);
#[allow(clippy::too_many_arguments)]
fn compute(
    mut context: RenderContext,
    audit: Res<Audit>,
    render_frame: Res<FrameCount>,
    state: Res<Snapshot>,
    geometry_inputs: (Option<Res<PairWarmup>>, Option<Res<cpu_geometry::Geometry>>),
    #[cfg(feature = "dlaa")] aa: Query<AaInputs>,
    #[cfg(feature = "dlaa")] native_state: Option<Res<crate::native_dlaa::State>>,
    pipeline: Res<Pipeline>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    views: Query<AuditInputs, With<AuditView>>,
    #[cfg(feature = "dlaa")] companion: Option<
        Res<super::opaque_conservative_motion::replay_audit::Audit>,
    >,
    #[cfg(feature = "dlaa")] companion_state: Option<
        Res<super::opaque_conservative_motion::replay_audit::GpuState>,
    >,
    #[cfg(feature = "dlaa")] images: Res<RenderAssets<bevy::render::texture::GpuImage>>,
) {
    let (warmup, geometry) = geometry_inputs;
    if state.tick < audit.tick
        || audit
            .submitted
            .iter()
            .all(|done| done.load(Ordering::Acquire))
        || audit.failed()
    {
        return;
    }
    let Some(compiled) = cache.get_compute_pipeline(pipeline.id) else {
        return;
    };
    let Ok((view_entity, prepass, view, jitter, previous)) = views.single() else {
        return;
    };
    let (Some(motion), Some(depth), Some(previous)) = (
        prepass.motion_vectors_view(),
        prepass.depth_view(),
        previous,
    ) else {
        return;
    };
    let size = UVec2::new(prepass.size.width, prepass.size.height);
    let extent = size.min(UVec2::new(WIDTH, HEIGHT));
    if extent.x == 0 || extent.y == 0 {
        return;
    }
    let mut projection = view.clip_from_view;
    if let Some(jitter) = jitter {
        jitter.jitter_projection(&mut projection, size.as_vec2());
    }
    let world_from_view = view.world_from_view.to_matrix();
    let current_unjittered = view.clip_from_view * world_from_view.inverse();
    let inverse_jittered = world_from_view * projection.inverse();
    let matrices = [
        state.current_body,
        state.previous_body,
        current_unjittered,
        previous.clip_from_world,
        inverse_jittered,
    ];
    if matrices
        .iter()
        .any(|m| !m.is_finite() || m.determinant().abs() < 1e-12)
    {
        fail(&audit, "nonfinite or singular matrix");
        return;
    }
    let center = current_unjittered * state.current_body * Vec4::new(0., 0.8, 0., 1.);
    if !center.is_finite() || center.w <= 0. {
        fail(&audit, "body center behind camera");
        return;
    }
    let uv = center.truncate().truncate() / center.w * Vec2::new(0.5, -0.5) + Vec2::splat(0.5);
    let pixel = audit.centre.unwrap_or(uv * size.as_vec2());
    let origin = UVec2::new(
        (pixel.x - extent.x as f32 / 2.).clamp(0., (size.x - extent.x) as f32) as u32,
        (pixel.y - extent.y as f32 / 2.).clamp(0., (size.y - extent.y) as f32) as u32,
    );
    // A stage bit means its payload can really be submitted, not merely selected.
    if audit.pair.is_some()
        && audit
            .buffers
            .iter()
            .any(|handle| buffers.get(handle).is_none())
    {
        return;
    }
    // Freeze actual extracted CPU geometry before any pair-stage selection.
    let frozen_geometry = match geometry.as_deref()
        .map(|g| g.freeze(&state, inverse_jittered, size, origin)).transpose() {
        Ok(value) => value,
        Err(error) => { fail(&audit, &error); return; }
    };
    let slot = if let Some(pair) = &audit.pair {
        let expected_history = expected_history_clip(view.clip_from_view, &view.world_from_view);
        if !expected_history.is_finite() {
            fail(&audit, "nonfinite expected history clip");
            return;
        }
        let stamp = PairStamp::new(
            state.frame,
            state.tick,
            render_frame.0,
            view_entity,
            current_unjittered,
            expected_history,
            state.current_body,
            size,
            origin,
            extent,
            jitter.map(|j| j.offset),
        );
        match pair.select_ready(audit.tick, stamp, 4, warmup.as_deref()) {
            Ok(Some(slot)) => slot,
            Ok(None) => return,
            Err(error) => {
                fail(&audit, error);
                return;
            }
        }
    } else {
        0
    };
    let Some(buffer) = buffers.get(&audit.buffers[slot]) else {
        return;
    };
    if let Some(pair) = &audit.pair
        && let Err(error) =
            pair.previous_matches(slot, previous.clip_from_world, state.previous_body)
    {
        // Preserve the failing input metadata; never turn arithmetic drift into an exact pass.
        let _ = std::fs::write(
            audit.directory.join("previous-join-failure.txt"),
            format!(
                "{error}\n{}previous_clip={:?}\nprevious_body={:?}\n",
                pair.metadata(),
                previous.clip_from_world.to_cols_array(),
                state.previous_body.to_cols_array()
            ),
        );
        // Capture the actual failing second raw payload before the readback refuses the join.
    }
    let parameters = Parameters {
        identity: UVec4::new(
            1,
            state.frame,
            state.tick.min(u32::MAX as u64) as u32,
            slot as u32,
        ),
        dimensions_origin: UVec4::new(size.x, size.y, origin.x, origin.y),
        extent: UVec4::new(extent.x, extent.y, 0, 0),
    };
    let mut uniform = UniformBuffer::from(parameters);
    uniform.write_buffer(&device, &queue);
    let bindings = device.create_bind_group(
        None,
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((
            motion,
            depth,
            uniform.binding().unwrap(),
            buffer.buffer.as_entire_buffer_binding(),
        )),
    );
    let mut metadata = format!(
        "request=1 frame={} tick={} dimensions={:?} origin={origin:?} extent={extent:?}\ncurrent_body={:?}\nprevious_body={:?}\ncurrent_unjittered={:?}\nprevious_unjittered={:?}\ninverse_jittered={:?}\n",
        state.frame,
        state.tick,
        size,
        state.current_body.to_cols_array(),
        state.previous_body.to_cols_array(),
        current_unjittered.to_cols_array(),
        previous.clip_from_world.to_cols_array(),
        inverse_jittered.to_cols_array()
    );
    metadata.push_str(&format!("render_frame={} view={view_entity:?} jitter={:?} previous_view_from_world={:?} previous_clip_from_view={:?} previous_world_from_clip={:?}\n",
        render_frame.0,jitter.map(|j|j.offset),previous.view_from_world.to_cols_array(),
        previous.clip_from_view.to_cols_array(),previous.world_from_clip.to_cols_array()));
    if let Some(pair) = &audit.pair {
        metadata.push_str(&pair.metadata());
        metadata.push_str(&format!("warmup_update_sample={:?}\n", warmup.as_deref()));
    }
    #[cfg(feature = "dlaa")]
    metadata.push_str(&format!(
        "aa_membership={:?} prepared_metadata_ready={:?} GPU_dispatch_verified=false\n",
        aa.get(view_entity).map(|(sr, rr, taa)| (
            sr.map(|s| (s.perf_quality_mode, s.reset)),
            rr.is_some(),
            taa.map(|t| t.reset)
        )),
        native_state.as_ref().map(|s| (s.ready(), s.failed()))
    ));
    {
        let mut pass = context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("raw temporal inputs"),
                ..default()
            });
        pass.set_pipeline(compiled);
        pass.set_bind_group(0, &bindings, &[]);
        pass.dispatch_workgroups(extent.x.div_ceil(8), extent.y.div_ceil(8), 1);
    }
    let header: [u32; 12] = parameters
        .identity
        .to_array()
        .into_iter()
        .chain(parameters.dimensions_origin.to_array())
        .chain(parameters.extent.to_array())
        .collect::<Vec<_>>()
        .try_into()
        .expect("fixed parameter header");
    #[cfg(feature = "dlaa")]
    if let (Some(companion), Some(state)) = (companion.as_deref(), companion_state.as_deref())
        && let Err(error) = super::opaque_conservative_motion::replay_audit::capture(
            companion,
            state,
            &mut context,
            &queue,
            &images,
            &buffers,
            view_entity,
            header,
            &metadata,
            &cache,
        )
    {
        super::opaque_conservative_motion::replay_audit::capture_failed(companion, error);
        fail(&audit, error);
        return;
    }

    #[cfg(not(feature = "dlaa"))]
    let _ = view_entity;
    audit.frozen.lock().unwrap_or_else(|e| e.into_inner())[slot] = Some(Frozen {
        header,
        text: metadata,
        geometry: frozen_geometry,
    });
    audit.submitted[slot].store(true, Ordering::Release);
}
fn queue_readback(
    mut commands: Commands,
    mut audit: ResMut<Audit>,
    warmup: Option<Res<PairWarmup>>,
    frame: Res<FrameCount>,
) {
    if audit.pair.is_some() && !pair_transport_ready(warmup.as_deref(), frame.0) {
        return;
    }
    for slot in 0..audit.buffers.len() {
        if audit.submitted[slot].load(Ordering::Acquire) && !audit.readback_queued[slot] {
            audit.readback_queued[slot] = true;
            commands
                .spawn((
                    Readback::buffer(audit.buffers[slot].clone()),
                    AuditOnce(slot, false),
                ))
                .observe(readback);
        }
    }
}
fn readback(
    event: On<ReadbackComplete>,
    audit: Res<Audit>,
    mut commands: Commands,
    mut once: Query<&mut AuditOnce>,
) {
    let Ok(mut once) = once.get_mut(event.entity) else {
        return;
    };
    if std::mem::replace(&mut once.1, true) {
        return;
    }
    let slot = once.0;
    let result = (|| -> Result<(), String> {
        let frozen = audit.frozen.lock().unwrap_or_else(|e| e.into_inner())[slot]
            .clone()
            .ok_or("capture metadata absent")?;
        if event.data.len() != ((3 + WIDTH * HEIGHT) * 16) as usize {
            return Err("buffer size mismatch".into());
        }
        let words: Vec<u32> = event.data[..48]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| u32::from_le_bytes(*v))
            .collect();
        if words != frozen.header {
            return Err("GPU/main-frame header mismatch".into());
        }
        let pixels = frozen.header[8] as usize * frozen.header[9] as usize;
        if pixels == 0 || pixels > (WIDTH * HEIGHT) as usize {
            return Err("invalid ROI extent".into());
        }
        for bytes in event.data[48..48 + pixels * 16].as_chunks::<16>().0 {
            let values: [f32; 4] = std::array::from_fn(|i| {
                f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
            });
            if values.iter().any(|x| !x.is_finite())
                || !(0. ..=1.).contains(&values[2])
                || values[3] != 1.
            {
                return Err("nonfinite or invalid motion/depth sample".into());
            }
        }
        std::fs::write(
            audit.directory.join(if audit.pair.is_some() {
                format!("motion-depth-{slot}.bin")
            } else {
                "motion-depth.bin".into()
            }),
            &event.data,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            audit.directory.join(if audit.pair.is_some() {
                format!("metadata-{slot}.txt")
            } else {
                "metadata.txt".into()
            }),
            frozen.text,
        )
        .map_err(|e| e.to_string())?;
        if let Some(geometry) = frozen.geometry {
            geometry.write(&audit.directory, slot, frozen.header, &event.data)?;
        }
        if slot == 1
            && let Some(error) = audit.pair.as_ref().and_then(PairCapture::join_failure)
        {
            return Err(error);
        }
        println!(
            "temporal_audit verified=true request=1 frame={} tick={} pixels={pixels} bytes={}",
            frozen.header[1],
            frozen.header[2],
            event.data.len()
        );
        Ok(())
    })();
    match result {
        Ok(()) => audit.complete[slot].store(true, Ordering::Release),
        Err(error) => fail(&audit, &error),
    }
    commands.entity(event.entity).despawn();
}

// Shared exact request latch; no second camera/body snapshot or matrix computation.
#[cfg(feature = "dlaa")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceTransaction {
    pub(crate) source_frame: u32,
    pub(crate) source_tick: u64,
}
#[cfg(feature = "dlaa")]
pub(crate) fn replay_request(world: &World) -> Result<Option<SourceTransaction>, &'static str> {
    let Some(audit) = world.get_resource::<Audit>() else {
        return Err("raw-audit-absent");
    };
    let Some(state) = world.get_resource::<Snapshot>() else {
        return Err("raw-snapshot-absent");
    };
    if audit
        .submitted
        .iter()
        .all(|done| done.load(Ordering::Acquire))
        || state.tick < audit.tick
    {
        return Ok(None);
    }
    if audit.failed() || state.tick != audit.tick {
        return Err("exact-audit-source-tick-missed");
    }
    // Snapshot.frame is captured in PostUpdate; renderer FrameCount is extracted after Last.
    // Keep the source transaction authoritative instead of equating these two clock domains.
    Ok(Some(SourceTransaction {
        source_frame: state.frame,
        source_tick: state.tick,
    }))
}

#[cfg(all(test, feature = "dlaa"))]
pub(crate) fn test_audit_clock_app() -> App {
    fn snapshot_fixture(frame: Res<FrameCount>, audit: Res<Audit>, mut state: ResMut<Snapshot>) {
        *state = Snapshot {
            frame: frame.0,
            tick: audit.tick,
            current_body: Mat4::IDENTITY,
            previous_body: Mat4::IDENTITY,
        };
    }
    let mut app = App::new();
    app.add_plugins(bevy::diagnostic::FrameCountPlugin)
        .insert_resource(Audit {
            companion: None,
            tick: 1652,
            centre: Some(Vec2::new(960., 540.)),
            directory: PathBuf::new(),
            buffers: vec![Handle::default()],
            submitted: vec![Arc::default()],
            readback_queued: vec![false],
            complete: vec![Arc::default()],
            failed: Arc::default(),
            frozen: Arc::new(Mutex::new(vec![None])),
            pair: None,
        })
        .init_resource::<Snapshot>()
        .add_systems(PostUpdate, snapshot_fixture);
    app
}

// History publication uses Affine3A inverse, unlike current raster ViewUniform's Mat4 inverse.
// Keep these domains separate: equality must compare the producer's declared arithmetic.
pub(crate) fn expected_history_clip(
    clip_from_view: Mat4,
    world_from_view: &GlobalTransform,
) -> Mat4 {
    clip_from_view * Mat4::from(world_from_view.affine().inverse())
}

// Read-only two-frame transaction selector shared by raw and HDR recorders.
// Every selected stage must observe bit-identical source/view/ROI inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PairStamp {
    source_frame: u32,
    source_tick: u64,
    render_frame: u32,
    view: Entity,
    clip: [u32; 16], // current raster ViewUniform domain (Mat4 inverse)
    expected_history_clip: [u32; 16], // next native PreviousViewData producer (Affine inverse)
    body: [u32; 16],
    size: [u32; 2],
    origin: [u32; 2],
    extent: [u32; 2],
    jitter: Option<[u32; 2]>,
}
impl PairStamp {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        source_frame: u32,
        source_tick: u64,
        render_frame: u32,
        view: Entity,
        clip: Mat4,
        expected_history_clip: Mat4,
        body: Mat4,
        size: UVec2,
        origin: UVec2,
        extent: UVec2,
        jitter: Option<Vec2>,
    ) -> Self {
        Self {
            source_frame,
            source_tick,
            render_frame,
            view,
            clip: clip.to_cols_array().map(f32::to_bits),
            expected_history_clip: expected_history_clip.to_cols_array().map(f32::to_bits),
            body: body.to_cols_array().map(f32::to_bits),
            size: size.to_array(),
            origin: origin.to_array(),
            extent: extent.to_array(),
            jitter: jitter.map(|j| j.to_array().map(f32::to_bits)),
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct PairCapture {
    frames: Arc<Mutex<Vec<(PairStamp, u8)>>>,
    join_error: Arc<Mutex<Option<String>>>,
}
impl PairCapture {
    pub(crate) fn select_ready(
        &self,
        requested_tick: u64,
        stamp: PairStamp,
        stage: u8,
        warmup: Option<&PairWarmup>,
    ) -> Result<Option<usize>, &'static str> {
        if !pair_transport_ready(warmup, stamp.source_frame) {
            return Err("exact pair source frame not warmup-ready");
        }
        self.select(requested_tick, stamp, stage)
    }
    // Bits: before-Early HDR=1, after-Early HDR=2, ordinary raw inputs=4.
    pub(crate) fn select(
        &self,
        requested_tick: u64,
        stamp: PairStamp,
        stage: u8,
    ) -> Result<Option<usize>, &'static str> {
        if !matches!(stage, 1 | 2 | 4) {
            return Err("invalid pair stage");
        }
        let mut frames = self.frames.lock().map_err(|_| "pair state poisoned")?;
        let count = frames.len();
        if let Some((last, mask)) = frames.last_mut() {
            if last.source_frame == stamp.source_frame {
                if last != &stamp {
                    return Err("raw/HDR exact transaction/view/ROI mismatch");
                }
                if *mask & stage != 0 {
                    return Ok(None);
                }
                *mask |= stage;
                return Ok(Some(count - 1));
            }
            if count == 2 {
                return Ok(None);
            }
            if *mask != 7 {
                return Err("first raw/HDR frame incomplete before next render");
            }
            if last.source_frame.checked_add(1) != Some(stamp.source_frame)
                || last.render_frame.checked_add(1) != Some(stamp.render_frame)
                || stamp.source_tick < last.source_tick
                || last.view != stamp.view
            {
                return Err("capture frames are not exact consecutive source/render transactions");
            }
        } else if stamp.source_tick != requested_tick {
            return Err("exact first source tick missed");
        }
        let slot = frames.len();
        frames.push((stamp, stage));
        Ok(Some(slot))
    }
    pub(crate) fn previous_matches(
        &self,
        slot: usize,
        previous_clip: Mat4,
        previous_body: Mat4,
    ) -> Result<(), &'static str> {
        if slot == 0 {
            return Ok(());
        }
        let frames = self.frames.lock().map_err(|_| "pair state poisoned")?;
        let first = frames.first().ok_or("previous actual frame absent")?;
        if first.0.expected_history_clip != previous_clip.to_cols_array().map(f32::to_bits)
            || first.0.body != previous_body.to_cols_array().map(f32::to_bits)
        {
            *self.join_error.lock().map_err(|_| "pair join poisoned")? =
                Some("actual previous-view/body differs from preceding declared history producer".into());
            return Err("actual previous-view/body differs from preceding declared history producer");
        }
        Ok(())
    }
    pub(crate) fn join_failure(&self) -> Option<String> {
        self.join_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(crate) fn metadata(&self) -> String {
        format!(
            "pair_transaction={:?}\n",
            self.frames.lock().unwrap_or_else(|e| e.into_inner())
        )
    }
}
pub(crate) fn pair_requested() -> Result<bool, String> {
    match std::env::var("GRID_TEMPORAL_CONSECUTIVE") {
        Err(std::env::VarError::NotPresent) => Ok(false),
        Ok(v) if v == "0" => Ok(false),
        Ok(v) if v == "1" => Ok(true),
        _ => Err("GRID_TEMPORAL_CONSECUTIVE requires 0 or 1".into()),
    }
}
#[cfg(feature = "dlaa")]
pub(crate) fn pair_for_colour(app: &App) -> Option<PairCapture> {
    app.world()
        .get_resource::<Audit>()
        .and_then(|a| a.pair.clone())
}

#[cfg(test)]
mod consecutive_tests {
    use super::*;
    fn stamp(frame: u32, tick: u64) -> PairStamp {
        PairStamp::new(
            frame,
            tick,
            frame + 1,
            Entity::PLACEHOLDER,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::Z),
            UVec2::new(1920, 1080),
            UVec2::new(704, 348),
            UVec2::new(512, 384),
            Some(Vec2::ZERO),
        )
    }
    #[test]
    fn pair_joins_three_stages_and_two_exact_render_transactions() {
        let pair = PairCapture::default();
        for stage in [1, 2, 4] {
            assert_eq!(pair.select(1652, stamp(793, 1652), stage), Ok(Some(0)));
        }
        assert_eq!(pair.select(1652, stamp(793, 1652), 4), Ok(None));
        for stage in [1, 2, 4] {
            assert_eq!(pair.select(1652, stamp(794, 1655), stage), Ok(Some(1)));
        }
        assert!(
            pair.previous_matches(1, Mat4::IDENTITY, Mat4::from_translation(Vec3::Z))
                .is_ok()
        );
        assert_eq!(pair.select(1652, stamp(795, 1658), 1), Ok(None));
        assert!(pair.join_failure().is_none());
    }
    #[test]
    fn pair_refuses_missing_stage_nonconsecutive_render_and_bitwise_view_drift() {
        let pair = PairCapture::default();
        pair.select(1652, stamp(793, 1652), 1).unwrap();
        assert!(pair.select(1652, stamp(794, 1655), 1).is_err());
        let mut changed = stamp(793, 1652);
        changed.clip[12] = (-0.0_f32).to_bits();
        assert!(pair.select(1652, changed, 2).is_err());
        for stage in [2, 4] {
            pair.select(1652, stamp(793, 1652), stage).unwrap();
        }
        let mut stale = stamp(794, 1655);
        stale.render_frame += 1;
        assert!(pair.select(1652, stale, 1).is_err());
        let late = PairCapture::default();
        assert!(late.select(1652, stamp(793, 1653), 1).is_err());
    }
    #[test]
    fn pair_previous_correspondence_cannot_pass_a_one_word_difference() {
        let pair = PairCapture::default();
        for stage in [1, 2, 4] {
            pair.select(1652, stamp(793, 1652), stage).unwrap();
        }
        pair.select(1652, stamp(794, 1655), 1).unwrap();
        let mut wrong = Mat4::IDENTITY.to_cols_array();
        wrong[12] = f32::from_bits(1);
        assert!(
            pair.previous_matches(
                1,
                Mat4::from_cols_array(&wrong),
                Mat4::from_translation(Vec3::Z)
            )
            .is_err()
        );
        assert!(pair.join_failure().is_some());
    }
    #[test]
    fn pair_rejects_one_word_expected_history_drift_even_with_identical_raster_clip() {
        let pair = PairCapture::default();
        pair.select(1652, stamp(793, 1652), 1).unwrap();
        let mut changed = stamp(793, 1652);
        changed.expected_history_clip[12] = (-0.0_f32).to_bits();
        assert!(pair.select(1652, changed, 2).is_err());
    }
    #[test]
    fn actual_bevy_history_publication_uses_affine_domain_not_current_raster_inverse() {
        use bevy::core_pipeline::prepass::PreviousViewData;
        let projection = Mat4::perspective_infinite_reverse_rh(1.11, 16. / 9., 0.125);
        let world = GlobalTransform::from(Transform {
            translation: Vec3::new(137.25, -9.375, 2049.625),
            rotation: Quat::from_xyzw(0.2, -0.3, 0.4, 0.8).normalize(),
            scale: Vec3::ONE,
        });
        let mut camera = Camera::default();
        camera.computed.clip_from_view = projection;
        let mut app = App::new();
        // The real production system, in its actual PreUpdate schedule; no renderer/window.
        app.add_systems(PreUpdate, bevy::pbr::update_previous_view_data);
        let entity = app.world_mut().spawn((Camera3d::default(), camera, world)).id();
        app.update();
        let published = app.world().get::<PreviousViewData>(entity).unwrap().clip_from_world;
        let declared = expected_history_clip(projection, &world);
        let raster = projection * world.to_matrix().inverse();
        let bits = |matrix: Mat4| matrix.to_cols_array().map(f32::to_bits);
        assert_eq!(bits(published), bits(declared));
        // Rounding coincidence is valid on any architecture. Record actual word differences;
        // this diagnostic never asserts a portable difference or relaxes exact native equality.
        let differing_words = bits(published).into_iter().zip(bits(raster))
            .filter(|(history, raster)| history != raster).count();
        println!("native_history_arithmetic differing_words={differing_words} world={:?} projection={:?} raster={:?} expected_history={:?}",
            bits(world.to_matrix()), bits(projection), bits(raster), bits(declared));
        assert!(published.is_finite() && raster.is_finite());
        let make = |frame, tick| PairStamp::new(
            frame, tick, frame + 1, entity, raster, declared, Mat4::IDENTITY,
            UVec2::new(1920, 1080), UVec2::new(704, 348), UVec2::new(512, 384),
            Some(Vec2::ZERO),
        );
        let pair = PairCapture::default();
        for stage in [1, 2, 4] {
            assert_eq!(pair.select(1652, make(793, 1652), stage), Ok(Some(0)));
        }
        // A stationary nonidentity camera is a valid consecutive-frame control.
        // Run the actual native publisher again; all frame-identity inputs remain explicit.
        app.update();
        let actual_previous = app.world().get::<PreviousViewData>(entity).unwrap().clip_from_world;
        for stage in [1, 2, 4] {
            assert_eq!(pair.select(1652, make(794, 1655), stage), Ok(Some(1)));
        }
        assert!(pair.previous_matches(1, actual_previous, Mat4::IDENTITY).is_ok());
        let mut wrong = actual_previous.to_cols_array();
        wrong[12] = f32::from_bits(wrong[12].to_bits() ^ 1);
        assert!(pair.previous_matches(1, Mat4::from_cols_array(&wrong), Mat4::IDENTITY).is_err());
        assert!(pair.join_failure().is_some());
    }

}
