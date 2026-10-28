//! Private bounded pre-resolve OIT occupancy evidence; diagnostic only.
//! Attempts/actual capacity read on GPU; no original OIT buffer usage or data changed.
use bevy::{
    asset::uuid_handle,
    core_pipeline::{
        core_3d::main_transparent_pass_3d,
        oit::{OitBuffers, OrderIndependentTransparencySettings, resolve::{OitResolvePipelineId, node::oit_resolve}},
        schedule::{Core3d, Core3dSystems},
    },
    diagnostic::FrameCount,
    prelude::*,
    render::{
        Extract, ExtractSchedule, RenderApp, RenderStartup,
        camera::ExtractedCamera,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::{binding_types::{storage_buffer, storage_buffer_read_only, uniform_buffer}, *},
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        storage::{GpuShaderBuffer, ShaderBuffer},
    },
};
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}};
use super::window_phase_audit::Target;
const SHADER: Handle<Shader> = uuid_handle!("63d0b41d-6fd5-4783-bb06-4855679d4025");
#[derive(Component, Clone, ExtractComponent)]
struct AuditView;
#[derive(Component, Default)]
struct AuditOnce(bool);
#[derive(Clone, Resource, ExtractResource)]
pub(crate) struct Audit {
    target: Target,
    directory: PathBuf,
    output: Handle<ShaderBuffer>,
    submitted: Arc<AtomicBool>,
    complete: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    frozen: Arc<Mutex<Option<Frozen>>>,
    readback_queued: bool,
}
impl Audit {
    pub(crate) fn ready(&self) -> bool { self.complete.load(Ordering::Acquire) }
    pub(crate) fn failed(&self) -> bool { self.failed.load(Ordering::Acquire) }
}
#[derive(Clone)]
struct Frozen { words: [u32; 12], metadata: String }
#[derive(Default, Resource)]
struct Snapshot { frame: u32, tick: u64 }
#[derive(Clone, Copy, ShaderType)]
struct Parameters { identity: UVec4, allocation: UVec4, settings: UVec4 }
#[derive(Resource)]
struct Pipeline { layout: BindGroupLayoutDescriptor, id: CachedComputePipelineId }
type Inputs = (&'static AuditView, &'static ExtractedCamera,
    &'static OrderIndependentTransparencySettings, &'static OitResolvePipelineId);

/// Caller must require bounded baseline640x360 Oit16/Oit32 and preserve capture
/// readiness/failure gating. This function is never installed by default.
pub(crate) fn install(app: &mut App, target: Target, directory: &Path) -> std::io::Result<()> {
    let index = match target { Target::Tick(n) => n, Target::Frame(n) => u64::from(n) };
    if !(1..=1_000_000).contains(&index) || app.get_sub_app(RenderApp).is_none() {
        return Err(std::io::Error::other("OIT audit requires bounded positive target and renderer"));
    }
    let local = Path::new("local").canonicalize()?;
    let parent = directory.parent().ok_or_else(|| std::io::Error::other("audit parent absent"))?.canonicalize()?;
    if !parent.starts_with(local) { return Err(std::io::Error::other("OIT evidence must stay inside ignored local")); }
    std::fs::create_dir(directory)?;
    app.world_mut().resource_mut::<Assets<Shader>>().insert(SHADER.id(),
        Shader::from_wgsl(include_str!("oit_capacity_audit.wgsl"), "oit_capacity_audit.wgsl"))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut output = ShaderBuffer::from(vec![UVec4::ZERO; 3]);
    output.buffer_description.usage |= BufferUsages::COPY_SRC;
    let output = app.world_mut().resource_mut::<Assets<ShaderBuffer>>().add(output);
    std::fs::write(directory.join("pending.txt"), format!("status=pending requested={target:?} scope=selected-main-view-pre-resolve-counter\n"))?;
    app.insert_resource(Audit { target, directory: directory.to_owned(), output,
        submitted: Arc::default(), complete: Arc::default(), failed: Arc::default(),
        frozen: Arc::default(), readback_queued: false })
        .add_plugins((ExtractResourcePlugin::<Audit>::default(),
            ExtractComponentPlugin::<AuditView>::default()))
        .add_systems(PostStartup, mark_view)
        .add_systems(Update, queue_readback);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.init_resource::<Snapshot>().add_systems(ExtractSchedule, snapshot)
            .add_systems(RenderStartup, init_pipeline).add_systems(Core3d,
            compute.after(main_transparent_pass_3d).before(oit_resolve).in_set(Core3dSystems::MainPass));
    }
    Ok(())
}
fn mark_view(mut commands: Commands, views: Query<Entity, With<super::ViewerCamera>>) {
    for entity in &views { commands.entity(entity).insert(AuditView); }
}
fn snapshot(frame: Extract<Res<FrameCount>>, driving: Extract<Option<Res<super::Driving>>>, mut state: ResMut<Snapshot>) {
    state.frame = frame.0;
    state.tick = driving.as_ref().map_or(0, |d| d.runtime.ticks);
}
fn init_pipeline(mut commands: Commands, cache: Res<PipelineCache>) {
    let layout = BindGroupLayoutDescriptor::new("OIT bounded occupancy audit",
        &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, (
            storage_buffer_read_only::<u32>(false), uniform_buffer::<u32>(false),
            uniform_buffer::<Parameters>(false), storage_buffer::<Vec<UVec4>>(false))));
    let id = cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("OIT bounded occupancy audit".into()), layout: vec![layout.clone()], shader: SHADER, ..default() });
    commands.insert_resource(Pipeline { layout, id });
}
#[allow(clippy::too_many_arguments)] // Explicit render diagnostic resources, same pattern as temporal_audit.
fn compute(view: ViewQuery<Inputs>, mut context: RenderContext, audit: Res<Audit>, state: Res<Snapshot>,
    pipeline: Option<Res<Pipeline>>, cache: Res<PipelineCache>, buffers: Res<OitBuffers>,
    outputs: Res<RenderAssets<GpuShaderBuffer>>, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
    if audit.submitted.load(Ordering::Acquire) || audit.failed() { return; }
    let reached = match audit.target { Target::Tick(n) => state.tick >= n, Target::Frame(n) => state.frame >= n };
    if !reached || state.tick > u64::from(u32::MAX) { return; }
    let Some(pipeline) = pipeline else { return };
    let Some(compiled) = cache.get_compute_pipeline(pipeline.id) else { return };
    let (_, camera, settings, resolve_id) = view.into_inner();
    // Pinned transparent pass also skips if this resolve pipeline is unready.
    if cache.get_render_pipeline(resolve_id.0).is_none() { return; }
    let Some(size) = camera.physical_target_size else { return };
    if size != UVec2::new(640, 360) { return; }
    let (Some(counter), Some(capacity), Some(output), Some(nodes), Some(heads)) = (
        buffers.atomic_counter.binding(), buffers.nodes_capacity.binding(), outputs.get(&audit.output),
        buffers.nodes.buffer(), buffers.heads.buffer()) else { return };
    let parameters = Parameters {
        identity: UVec4::new(0x4f495441, 1, state.frame, state.tick as u32),
        allocation: UVec4::new(*buffers.nodes_capacity.get(), size.x, size.y, 0),
        settings: UVec4::new(settings.sorted_fragment_max_count, settings.fragments_per_pixel_average.to_bits(),
            settings.alpha_threshold.to_bits(), 0xc0dec0de),
    };
    let mut uniform = UniformBuffer::from(parameters); uniform.write_buffer(&device, &queue);
    let Some(parameter_binding) = uniform.binding() else { return };
    let bindings = device.create_bind_group(None, &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((counter, capacity, parameter_binding, output.buffer.as_entire_buffer_binding())));
    {
        let mut pass = context.command_encoder().begin_compute_pass(&ComputePassDescriptor {
            label: Some("OIT occupancy before resolve reset"), ..default() });
        pass.set_pipeline(compiled); pass.set_bind_group(0, &bindings, &[]); pass.dispatch_workgroups(1, 1, 1);
    }
    let mut words = [0; 12]; words[..4].copy_from_slice(&parameters.identity.to_array());
    words[5..8].copy_from_slice(&parameters.allocation.to_array()[..3]);
    words[8..].copy_from_slice(&parameters.settings.to_array());
    let metadata = format!("requested={:?} actual_frame={} actual_tick={} target={}x{} nodes_capacity_host={} node_elements={} node_buffer_bytes={} head_buffer_bytes={} sorted_max={} average={} alpha_threshold={} stage=after-main-transparent-before-oit-resolve\n",
        audit.target, state.frame, state.tick, size.x, size.y, buffers.nodes_capacity.get(), buffers.nodes.capacity(),
        nodes.size(), heads.size(), settings.sorted_fragment_max_count, settings.fragments_per_pixel_average, settings.alpha_threshold);
    *audit.frozen.lock().unwrap_or_else(|e| e.into_inner()) = Some(Frozen { words, metadata });
    audit.submitted.store(true, Ordering::Release);
}
fn queue_readback(mut commands: Commands, mut audit: ResMut<Audit>) {
    if !audit.submitted.load(Ordering::Acquire) || audit.readback_queued { return; }
    audit.readback_queued = true;
    commands.spawn((Readback::buffer(audit.output.clone()), AuditOnce::default())).observe(readback);
}
fn readback(event: On<ReadbackComplete>, audit: Res<Audit>, mut commands: Commands,
    mut once: Query<&mut AuditOnce>, mut exit: MessageWriter<AppExit>) {
    let Ok(mut once) = once.get_mut(event.entity) else { return };
    if std::mem::replace(&mut once.0, true) { return; }
    let result = (|| -> Result<(), String> {
        let frozen = audit.frozen.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or("missing frame identity")?;
        let words = verified_words(&event.data, &frozen.words)?;
        let attempts = words[4]; let capacity = words[5];
        let dropped = attempts.saturating_sub(capacity);
        let status = if attempts > capacity { "overflow-detected" } else { "no-global-overflow-observed-for-this-snapshot" };
        let text = format!("{}gpu_attempts={attempts} gpu_capacity={capacity} dropped_if_no_u32_wrap={dropped} status={status} u32_wrap_unchecked=true scope=global-capacity-only-not-per-pixel-sort-exactness\n", frozen.metadata);
        std::fs::write(audit.directory.join("capacity.txt"), text).map_err(|e| e.to_string())?;
        std::fs::write(audit.directory.join("capacity.bin"), &event.data).map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => { audit.complete.store(true, Ordering::Release); println!("oit_capacity_audit readback_verified=true evidence={}", audit.directory.display()); }
        Err(error) => { audit.failed.store(true, Ordering::Release); eprintln!("oit_capacity_audit readback_verified=false error={error}"); exit.write(AppExit::error()); }
    }
    commands.entity(event.entity).despawn();
}

fn verified_words(data: &[u8], expected: &[u32; 12]) -> Result<[u32; 12], String> {
    if data.len() != 48 { return Err("OIT audit requires48bytes".into()); }
    let words: [u32; 12] = std::array::from_fn(|i| u32::from_le_bytes(data[i*4..i*4+4].try_into().unwrap()));
    if words.iter().enumerate().any(|(i,v)| i != 4 && *v != expected[i]) {
        return Err("GPU identity/capacity/settings differ from selected CPU frame".into());
    }
    Ok(words)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readback_requires_exact_selected_frame_capacity_and_settings() {
        let expected = [0x4f495441,1,146,302,0,7372800,640,360,32,32f32.to_bits(),0,0xc0dec0de];
        let mut actual=expected; actual[4]=8_000_000;
        let bytes: Vec<u8> = actual.iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(verified_words(&bytes,&expected).unwrap(),actual);
        for index in [0,1,2,3,5,6,7,8,9,10,11] {
            let mut wrong=bytes.clone();wrong[index*4]^=1;
            assert!(verified_words(&wrong,&expected).is_err());
        }
        assert!(verified_words(&bytes[..47],&expected).is_err());
        let mut extra=bytes.clone();extra.push(0);assert!(verified_words(&extra,&expected).is_err());
    }
}
