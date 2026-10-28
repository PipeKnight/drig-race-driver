//! Bounded paired main-view HDR evidence around EarlyPostProcess.
//! Read-only sampling; the existing motion/depth protocol is independent.
use bevy::{
    asset::uuid_handle,
    core_pipeline::schedule::{Core3d, Core3dSystems},
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
            binding_types::{storage_buffer, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        storage::{GpuShaderBuffer, ShaderBuffer},
        view::{ExtractedView, ViewTarget},
    },
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};
const WIDTH: u32 = 1024;
const HEIGHT: u32 = 512;
const SHADER: Handle<Shader> = uuid_handle!("26f80b16-3407-485b-af16-841ec5199893");
#[derive(Component, Clone, ExtractComponent)]
struct ColourView;
#[derive(Component)]
struct Once(usize, bool);
#[derive(Clone)]
struct Frozen {
    header: [u32; 12],
    text: String,
}
#[derive(Clone, Resource, ExtractResource)]
pub(crate) struct Audit {
    tick: u64,
    centre: Option<Vec2>,
    directory: PathBuf,
    buffers: [Handle<ShaderBuffer>; 2],
    submitted: [Arc<AtomicBool>; 2],
    queued: [bool; 2],
    complete: Arc<AtomicU8>,
    failed: Arc<AtomicBool>,
    frozen: Arc<Mutex<[Option<Frozen>; 2]>>,
}
impl Audit {
    pub(crate) fn ready(&self) -> bool {
        self.complete.load(Ordering::Acquire) == 3
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
fn fail(audit: &Audit, message: &str) {
    if !audit.failed.swap(true, Ordering::AcqRel) {
        eprintln!("temporal_colour verified=false error={message}");
    }
}
#[derive(Clone, Default, Resource, ExtractResource)]
struct Snapshot {
    frame: u32,
    tick: u64,
    body: Mat4,
}
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
type Inputs = (
    &'static ExtractedView,
    &'static ViewTarget,
    Option<&'static ColourView>,
    Option<&'static TemporalJitter>,
);

pub fn install(app: &mut App, tick: u64, directory: &Path, centre: Option<Vec2>) -> std::io::Result<()> {
    if !(1..=1_000_000).contains(&tick) || app.get_sub_app(RenderApp).is_none() {
        return Err(std::io::Error::other(
            "colour audit requires bounded positive tick and renderer",
        ));
    }
    let local = Path::new("local").canonicalize()?;
    let parent = directory
        .parent()
        .ok_or_else(|| std::io::Error::other("colour parent absent"))?
        .canonicalize()?;
    if !parent.starts_with(local) {
        return Err(std::io::Error::other(
            "colour evidence must stay under ignored local/",
        ));
    }
    std::fs::create_dir(directory)?;
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SHADER.id(),
            Shader::from_wgsl(include_str!("temporal_colour.wgsl"), "temporal_colour.wgsl"),
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let buffers = std::array::from_fn(|_| {
        let mut buffer = ShaderBuffer::from(vec![UVec4::ZERO; (3 + WIDTH * HEIGHT) as usize]);
        buffer.buffer_description.usage |= BufferUsages::COPY_SRC;
        app.world_mut()
            .resource_mut::<Assets<ShaderBuffer>>()
            .add(buffer)
    });
    app.insert_resource(Audit {
        tick,
        centre,
        directory: directory.to_owned(),
        buffers,
        submitted: std::array::from_fn(|_| Arc::default()),
        queued: [false; 2],
        complete: Arc::default(),
        failed: Arc::default(),
        frozen: Arc::new(Mutex::new([None, None])),
    })
    .init_resource::<Snapshot>()
    .add_plugins((
        ExtractComponentPlugin::<ColourView>::default(),
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
            .add_systems(
                Core3d,
                record::<0>
                    .after(Core3dSystems::MainPass)
                    .before(Core3dSystems::EarlyPostProcess),
            )
            .add_systems(
                Core3d,
                record::<1>
                    .after(Core3dSystems::EarlyPostProcess)
                    .before(Core3dSystems::PostProcess),
            );
    }
    Ok(())
}
fn mark_view(
    mut commands: Commands,
    audit: Res<Audit>,
    supported: Option<Res<bevy::anti_alias::dlss::DlssSuperResolutionSupported>>,
    views: Query<Entity, With<super::ViewerCamera>>,
) {
    if supported.is_none() {
        fail(&audit, "native DLAA unavailable for paired colour audit");
        return;
    }
    for entity in &views {
        commands.entity(entity).insert(ColourView);
    }
}
fn snapshot(
    frame: Res<FrameCount>,
    driving: Option<Res<super::Driving>>,
    mut state: ResMut<Snapshot>,
) {
    if let Some(driving) = driving {
        *state = Snapshot {
            frame: frame.0,
            tick: driving.runtime.ticks,
            body: Mat4::from_cols_array(&driving.runtime.pose()),
        };
    }
}
fn init_pipeline(mut commands: Commands, cache: Res<PipelineCache>) {
    let layout = BindGroupLayoutDescriptor::new(
        "paired temporal colour",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<Parameters>(false),
                storage_buffer::<Vec<UVec4>>(false),
            ),
        ),
    );
    let id = cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("paired read-only HDR colour".into()),
        layout: vec![layout.clone()],
        shader: SHADER,
        ..default()
    });
    commands.insert_resource(Pipeline { layout, id });
}
#[allow(clippy::too_many_arguments)]
fn record<const STAGE: usize>(
    view: ViewQuery<Inputs>,
    mut context: RenderContext,
    audit: Res<Audit>,
    state: Res<Snapshot>,
    pipeline: Res<Pipeline>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
) {
    let (view, target, marked, jitter) = view.into_inner();
    if marked.is_none()
        || state.tick < audit.tick
        || audit.failed()
        || audit.submitted[STAGE].load(Ordering::Acquire)
    {
        return;
    }
    let (Some(compiled), Some(buffer)) = (
        cache.get_compute_pipeline(pipeline.id),
        buffers.get(&audit.buffers[STAGE]),
    ) else {
        return;
    };
    if target.main_texture_format() != TextureFormat::Rgba16Float {
        fail(&audit, "expected native HDR Rgba16Float view target");
        return;
    }
    let size = UVec2::new(view.viewport.z, view.viewport.w);
    let extent = size.min(UVec2::new(WIDTH, HEIGHT));
    if extent.min_element() == 0 {
        fail(&audit, "zero colour extent");
        return;
    }
    let clip = view.clip_from_view * view.world_from_view.to_matrix().inverse();
    let center = clip * state.body * Vec4::new(0., 0.8, 0., 1.);
    if !center.is_finite() || center.w <= 0. {
        fail(&audit, "invalid projected body centre");
        return;
    }
    let pixel = (center.truncate().truncate() / center.w * Vec2::new(0.5, -0.5) + Vec2::splat(0.5))
        * size.as_vec2();
    let pixel = audit.centre.unwrap_or(pixel);
    let origin = (pixel - extent.as_vec2() / 2.)
        .clamp(Vec2::ZERO, (size - extent).as_vec2())
        .as_uvec2();
    let params = Parameters {
        identity: UVec4::new(
            1,
            state.frame,
            state.tick.min(u32::MAX as u64) as u32,
            STAGE as u32 + 1,
        ),
        dimensions_origin: UVec4::new(size.x, size.y, origin.x, origin.y),
        extent: UVec4::new(extent.x, extent.y, 0, 0),
    };
    let header: [u32; 12] = params
        .identity
        .to_array()
        .into_iter()
        .chain(params.dimensions_origin.to_array())
        .chain(params.extent.to_array())
        .collect::<Vec<_>>()
        .try_into()
        .expect("fixed colour header");
    {
        let mut frozen = audit.frozen.lock().unwrap_or_else(|e| e.into_inner());
        if STAGE == 1
            && frozen[0]
                .as_ref()
                .is_none_or(|f| f.header[..3] != header[..3] || f.header[4..] != header[4..])
        {
            fail(&audit, "before/after colour frame or ROI mismatch");
            return;
        }
        frozen[STAGE]=Some(Frozen{header,text:format!("version=1 frame={} tick={} stage={} dimensions={size:?} origin={origin:?} extent={extent:?} format={:?} jitter={:?} scope=main-HDR-around-EarlyPostProcess-before-bloom-tonemapping
",state.frame,state.tick,STAGE+1,target.main_texture_format(),jitter.map(|j|j.offset))});
    }
    let mut uniform = UniformBuffer::from(params);
    uniform.write_buffer(&device, &queue);
    let bindings = device.create_bind_group(
        None,
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((
            target.main_texture_view(),
            uniform.binding().unwrap(),
            buffer.buffer.as_entire_buffer_binding(),
        )),
    );
    {
        let mut pass = context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("bounded HDR colour evidence"),
                ..default()
            });
        pass.set_pipeline(compiled);
        pass.set_bind_group(0, &bindings, &[]);
        pass.dispatch_workgroups(extent.x.div_ceil(8), extent.y.div_ceil(8), 1);
    }
    audit.submitted[STAGE].store(true, Ordering::Release);
}
fn queue_readback(mut commands: Commands, mut audit: ResMut<Audit>) {
    for stage in 0..2 {
        if audit.submitted[stage].load(Ordering::Acquire) && !audit.queued[stage] {
            audit.queued[stage] = true;
            commands
                .spawn((
                    Readback::buffer(audit.buffers[stage].clone()),
                    Once(stage, false),
                ))
                .observe(readback);
        }
    }
}
fn readback(
    event: On<ReadbackComplete>,
    audit: Res<Audit>,
    mut commands: Commands,
    mut once: Query<&mut Once>,
) {
    let Ok(mut once) = once.get_mut(event.entity) else {
        return;
    };
    if std::mem::replace(&mut once.1, true) {
        return;
    }
    let stage = once.0;
    let result = (|| -> Result<(), String> {
        let frozen = audit.frozen.lock().unwrap_or_else(|e| e.into_inner())[stage]
            .clone()
            .ok_or("colour metadata absent")?;
        if event.data.len() != ((3 + WIDTH * HEIGHT) * 16) as usize {
            return Err("colour buffer length mismatch".into());
        }
        let words: Vec<u32> = event.data[..48]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|x| u32::from_le_bytes(*x))
            .collect();
        if words != frozen.header {
            return Err("GPU colour header mismatch".into());
        }
        let pixels = frozen.header[8] as usize * frozen.header[9] as usize;
        if pixels == 0 || pixels > (WIDTH * HEIGHT) as usize {
            return Err("invalid colour ROI".into());
        }
        let mut nonzero = false;
        for bytes in event.data[48..48 + pixels * 16].as_chunks::<16>().0 {
            let rgba: [f32; 4] = std::array::from_fn(|i| {
                f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
            });
            if rgba.iter().any(|x| !x.is_finite()) {
                return Err("nonfinite HDR colour sample".into());
            }
            nonzero |= rgba[..3].iter().any(|x| x.abs() > 1e-8);
        }
        if !nonzero {
            return Err("blank HDR colour ROI".into());
        }
        let name = if stage == 0 { "before" } else { "after" };
        std::fs::write(audit.directory.join(format!("{name}.bin")), &event.data)
            .map_err(|e| e.to_string())?;
        std::fs::write(audit.directory.join(format!("{name}.txt")), frozen.text)
            .map_err(|e| e.to_string())?;
        println!(
            "temporal_colour verified=true frame={} tick={} stage={name} pixels={pixels} bytes={}",
            frozen.header[1],
            frozen.header[2],
            event.data.len()
        );
        Ok(())
    })();
    match result {
        Ok(()) => {
            audit.complete.fetch_or(1 << stage, Ordering::AcqRel);
        }
        Err(error) => fail(&audit, &error),
    }
    commands.entity(event.entity).despawn();
}
