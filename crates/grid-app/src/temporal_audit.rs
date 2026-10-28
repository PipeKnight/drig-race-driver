//! Opt-in bounded temporal input evidence; no presentation or physics feedback.
//! One bounded raw motion/depth capture; no presentation or physics feedback.
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
struct AuditOnce(bool);
#[derive(Clone, Resource, ExtractResource)]
pub(crate) struct Audit {
    tick: u64,
    centre: Option<Vec2>,
    directory: PathBuf,
    buffer: Handle<ShaderBuffer>,
    submitted: Arc<AtomicBool>,
    readback_queued: bool,
    complete: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    frozen: Arc<Mutex<Option<Frozen>>>,
}
impl Audit {
    pub(crate) fn ready(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
#[derive(Clone)]
struct Frozen {
    header: [u32; 12],
    text: String,
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
type AuditInputs = (
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
pub fn install(app: &mut App, tick: u64, directory: &Path, centre: Option<Vec2>) -> std::io::Result<()> {
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
    let mut buffer = ShaderBuffer::from(vec![UVec4::ZERO; (3 + WIDTH * HEIGHT) as usize]);
    buffer.buffer_description.usage |= BufferUsages::COPY_SRC;
    let handle = app
        .world_mut()
        .resource_mut::<Assets<ShaderBuffer>>()
        .add(buffer);
    app.insert_resource(Audit {
        tick,
        centre,
        directory: directory.to_owned(),
        buffer: handle,
        submitted: Arc::default(),
        complete: Arc::default(),
        failed: Arc::default(),
        frozen: Arc::default(),
        readback_queued: false,
    })
    .init_resource::<Snapshot>()
    .add_plugins((
        ExtractComponentPlugin::<AuditView>::default(),
        ExtractResourcePlugin::<Audit>::default(),
        ExtractResourcePlugin::<Snapshot>::default(),
    ))
    .add_systems(PostStartup, mark_view)
    .add_systems(
        PostUpdate,
        snapshot.after(bevy::transform::TransformSystems::Propagate),
    )
    .add_systems(Update, queue_readback);
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
#[allow(clippy::too_many_arguments)]
fn compute(
    mut context: RenderContext,
    audit: Res<Audit>,
    state: Res<Snapshot>,
    pipeline: Res<Pipeline>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    views: Query<AuditInputs, With<AuditView>>,
) {
    if state.tick < audit.tick || audit.submitted.load(Ordering::Acquire) || audit.failed() {
        return;
    }
    let Some(compiled) = cache.get_compute_pipeline(pipeline.id) else {
        return;
    };
    let Ok((prepass, view, jitter, previous)) = views.single() else {
        return;
    };
    let (Some(motion), Some(depth), Some(previous), Some(buffer)) = (
        prepass.motion_vectors_view(),
        prepass.depth_view(),
        previous,
        buffers.get(&audit.buffer),
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
    let parameters = Parameters {
        identity: UVec4::new(1, state.frame, state.tick.min(u32::MAX as u64) as u32, 0),
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
    let metadata = format!(
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
    *audit.frozen.lock().unwrap_or_else(|e| e.into_inner()) = Some(Frozen {
        header,
        text: metadata,
    });
    audit.submitted.store(true, Ordering::Release);
}
fn queue_readback(mut commands: Commands, mut audit: ResMut<Audit>) {
    if !audit.submitted.load(Ordering::Acquire) || audit.readback_queued {
        return;
    }
    audit.readback_queued = true;
    commands
        .spawn((Readback::buffer(audit.buffer.clone()), AuditOnce(false)))
        .observe(readback);
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
    if std::mem::replace(&mut once.0, true) {
        return;
    }
    let result = (|| -> Result<(), String> {
        let frozen = audit
            .frozen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
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
        std::fs::write(audit.directory.join("motion-depth.bin"), &event.data)
            .map_err(|e| e.to_string())?;
        std::fs::write(audit.directory.join("metadata.txt"), frozen.text)
            .map_err(|e| e.to_string())?;
        println!(
            "temporal_audit verified=true request=1 frame={} tick={} pixels={pixels} bytes={}",
            frozen.header[1],
            frozen.header[2],
            event.data.len()
        );
        Ok(())
    })();
    match result {
        Ok(()) => audit.complete.store(true, Ordering::Release),
        Err(error) => fail(&audit, &error),
    }
    commands.entity(event.entity).despawn();
}
