//! Tree billboards faced on the GPU (F838). `present` used to face every visible
//! placement on the main thread and send its whole vertex buffer through mesh_stream:
//! 90-230 MB/s at Donington, about 1.5 ms of main thread and 3 ms of render thread per
//! frame (F837). Here each placement's constant data (matrices, kind, attribute offsets)
//! and each distinct source mesh's centres, offsets and atlas UVs go to the GPU once; per
//! frame the main world sends only the list of visible placements and the camera. A
//! compute pass at the start of the frame's render graph writes the faced POSITION,
//! NORMAL and (cards) UV0/UV1 straight into each mesh's vertex slab, with the arithmetic
//! of the CPU path (`tree_facing.wgsl`). Materials, shadow twins (faced to the sun on the
//! CPU, rarely) and ornament proxies are unchanged, and so is the image.
//!
//! A compute pass rather than a vertex shader: the drawn parts use StandardMaterial and the
//! card ExtendedMaterial, which other modules swap, copy and reuse (shadow twins, ornament
//! proxies), so facing in their vertex stage would need a material variant for each.
//!
//! The CPU path stays: until the pipeline is compiled, on adapters without compute shaders
//! or enough storage buffers, without a render world, for a mesh whose slab lacks
//! `STORAGE` (Bevy's large-object slabs), and with `GRID_TREE_FACING=cpu` (review).
use super::{Facing, blend_views};
use bevy::{
    asset::{AssetId, uuid_handle},
    prelude::*,
    render::{
        Extract, ExtractSchedule, RenderApp, RenderStartup,
        mesh::allocator::{MeshAllocator, MeshAllocatorSettings},
        render_resource::{
            binding_types::{storage_buffer_read_only_sized, storage_buffer_sized, uniform_buffer_sized},
            *,
        },
        renderer::{RenderAdapter, RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue},
    },
};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const SHADER: Handle<Shader> = uuid_handle!("3c9e4a71-58d2-4b0f-9a63-2e7f1d0c8b54");
const NONE: u32 = u32::MAX;
/// u32 words per placement (`Placement` in tree_facing.wgsl).
const PLACEMENT_WORDS: usize = 48;
/// f32 per source vertex (three vec4).
const SOURCE_FLOATS: usize = 12;

/// Whether the GPU path may be used at all (`GRID_TREE_FACING=cpu` turns it off).
fn wanted() -> bool {
    crate::dev::var("GRID_TREE_FACING").map_or(true, |v| v != "cpu")
}

/// Main-world state: placements and sources as the GPU holds them, and this frame's jobs.
#[derive(Resource)]
pub(super) struct GpuFacing {
    /// Set by the render world once the pipeline can run.
    active: Arc<AtomicBool>,
    /// Meshes whose slab the pass cannot write (no `STORAGE`): faced on the CPU.
    cpu_meshes: Arc<Mutex<HashSet<AssetId<Mesh>>>>,
    slots: HashMap<Entity, (u32, usize)>,
    free: Vec<u32>,
    placements: Vec<u32>,
    placements_generation: u64,
    /// Source key (its packed buffer's address) -> (buffer kept alive, first vertex, users).
    sources: HashMap<usize, (Arc<Vec<u8>>, u32, u32)>,
    source_data: Vec<f32>,
    sources_generation: u64,
    jobs: Vec<(u32, AssetId<Mesh>)>,
    view: [f32; 12],
}

impl GpuFacing {
    fn new(active: Arc<AtomicBool>, cpu_meshes: Arc<Mutex<HashSet<AssetId<Mesh>>>>) -> Self {
        Self {
            active,
            cpu_meshes,
            slots: HashMap::new(),
            free: Vec::new(),
            placements: Vec::new(),
            placements_generation: 0,
            sources: HashMap::new(),
            source_data: Vec::new(),
            sources_generation: 0,
            jobs: Vec::new(),
            view: [0.; 12],
        }
    }

    /// The faced vertices of these placements come from the GPU this frame.
    pub(super) fn active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Starts a frame: the camera's right, up and position; clears the jobs. Returns the
    /// meshes to face on the CPU anyway (usually none).
    pub(super) fn begin(&mut self, right: Vec3, up: Vec3, eye: Vec3) -> HashSet<AssetId<Mesh>> {
        self.jobs.clear();
        let blend = if blend_views() { 1. } else { 0. };
        self.view = [right.x, right.y, right.z, blend, up.x, up.y, up.z, 0., eye.x, eye.y, eye.z, 0.];
        let cpu = self.cpu_meshes.lock().unwrap_or_else(|e| e.into_inner());
        if cpu.is_empty() { HashSet::new() } else { cpu.clone() }
    }

    /// Queues `entity`'s placement for this frame; false if it has none (CPU path).
    pub(super) fn push(&mut self, entity: Entity, mesh: AssetId<Mesh>) -> bool {
        match self.slots.get(&entity) {
            Some(&(slot, _)) => {
                self.jobs.push((slot, mesh));
                true
            }
            None => false,
        }
    }

    fn register(&mut self, entity: Entity, facing: &Facing) {
        self.unregister(entity);
        let source = &facing.source;
        let layout = &source.layout;
        let Some(position) = layout.offset(Mesh::ATTRIBUTE_POSITION) else {
            return;
        };
        let key = Arc::as_ptr(&source.packed) as usize;
        let count = source.centers.len() as u32;
        let first = match self.sources.get_mut(&key) {
            Some(entry) => {
                entry.2 += 1;
                entry.1
            }
            None => {
                let first = (self.source_data.len() / SOURCE_FLOATS) as u32;
                self.source_data.extend(source_floats(source));
                self.sources.insert(key, (source.packed.clone(), first, 1));
                self.sources_generation += 1;
                first
            }
        };
        let word = |at: Option<usize>| at.map_or(NONE, |at| (at / 4) as u32);
        let kind = match (source.cylindrical, source.atlas.is_some()) {
            (false, _) => 0,
            (true, false) => 1,
            (true, true) => 2,
        };
        let mut record = [0u32; PLACEMENT_WORDS];
        for (k, v) in facing.world.to_cols_array().into_iter().enumerate() {
            record[k] = v.to_bits();
        }
        for (k, v) in facing.inverse.to_cols_array().into_iter().enumerate() {
            record[16 + k] = v.to_bits();
        }
        record[32..36].copy_from_slice(&[kind, first, count, facing.scale.to_bits()]);
        record[36..40].copy_from_slice(&[
            (layout.stride / 4) as u32,
            (position / 4) as u32,
            word(layout.offset(Mesh::ATTRIBUTE_NORMAL)),
            word(layout.offset(Mesh::ATTRIBUTE_UV_0)),
        ]);
        record[40] = word(layout.offset(Mesh::ATTRIBUTE_UV_1));
        let slot = self.free.pop().unwrap_or((self.placements.len() / PLACEMENT_WORDS) as u32);
        let at = slot as usize * PLACEMENT_WORDS;
        if self.placements.len() < at + PLACEMENT_WORDS {
            self.placements.resize(at + PLACEMENT_WORDS, 0);
        }
        self.placements[at..at + PLACEMENT_WORDS].copy_from_slice(&record);
        self.placements_generation += 1;
        self.slots.insert(entity, (slot, key));
    }

    fn unregister(&mut self, entity: Entity) {
        let Some((slot, key)) = self.slots.remove(&entity) else {
            return;
        };
        self.free.push(slot);
        if let Some(entry) = self.sources.get_mut(&key) {
            entry.2 -= 1;
        }
        // A track's placements leave together: start the source table afresh.
        if self.slots.is_empty() {
            self.sources.clear();
            self.source_data.clear();
            self.placements.clear();
            self.free.clear();
            self.sources_generation += 1;
            self.placements_generation += 1;
        }
    }
}

/// A source mesh's vertices as the shader reads them: centre, offset, atlas UV and the
/// offset normalised to the source's offset bounds (the card's radial normal).
fn source_floats(source: &super::FacingSource) -> Vec<f32> {
    let (low, high) = source.offsets.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), o| {
        (lo.min(Vec2::from_array(*o)), hi.max(Vec2::from_array(*o)))
    });
    let (middle, half) = ((low + high) / 2., ((high - low) / 2.).max(Vec2::splat(1e-6)));
    let mut out = Vec::with_capacity(source.centers.len() * SOURCE_FLOATS);
    for (i, (c, o)) in source.centers.iter().zip(&source.offsets).enumerate() {
        let atlas = source.atlas.as_ref().map_or([0.; 2], |a| a[i]);
        let n = (Vec2::from_array(*o) - middle) / half;
        out.extend_from_slice(&[c[0], c[1], c[2], o[0], o[1], atlas[0], atlas[1], n.x, n.y, 0., 0., 0.]);
    }
    out
}

/// Registers new or changed placements and drops removed ones.
fn register(
    gpu: Option<ResMut<GpuFacing>>,
    changed: Query<(Entity, &Facing), Changed<Facing>>,
    mut removed: RemovedComponents<Facing>,
) {
    let Some(mut gpu) = gpu else {
        return;
    };
    for entity in removed.read() {
        gpu.unregister(entity);
    }
    for (entity, facing) in &changed {
        gpu.register(entity, facing);
    }
}

pub(super) fn install(app: &mut App) {
    if !wanted() {
        return;
    }
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let active = Arc::new(AtomicBool::new(false));
    let cpu_meshes = Arc::new(Mutex::new(HashSet::new()));
    render
        .insert_resource(RenderFacing::new(active.clone(), cpu_meshes.clone()))
        .add_systems(
            RenderStartup,
            (allow_storage.before(bevy::render::init_gpu_resource::<MeshAllocator>), init_pipeline).chain(),
        )
        .add_systems(ExtractSchedule, extract)
        .add_systems(RenderGraph, dispatch.in_set(RenderGraphSystems::Begin));
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(SHADER.id(), Shader::from_wgsl(include_str!("tree_facing.wgsl"), "tree_facing.wgsl"))
        .expect("tree facing shader");
    app.insert_resource(GpuFacing::new(active, cpu_meshes))
        .add_systems(Update, register.before(super::present));
}

/// Render-world copy of the placements, sources and this frame's jobs.
#[derive(Resource)]
struct RenderFacing {
    active: Arc<AtomicBool>,
    cpu_meshes: Arc<Mutex<HashSet<AssetId<Mesh>>>>,
    supported: bool,
    placements: Vec<u32>,
    placements_generation: u64,
    sources: Vec<f32>,
    sources_generation: u64,
    jobs: Vec<(u32, AssetId<Mesh>)>,
    view: [f32; 12],
    buffers: Option<Buffers>,
}

struct Buffers {
    placements: Buffer,
    placements_generation: u64,
    sources: Buffer,
    sources_generation: u64,
    jobs: Buffer,
    view: Buffer,
}

impl RenderFacing {
    fn new(active: Arc<AtomicBool>, cpu_meshes: Arc<Mutex<HashSet<AssetId<Mesh>>>>) -> Self {
        Self {
            active,
            cpu_meshes,
            supported: false,
            placements: Vec::new(),
            placements_generation: u64::MAX,
            sources: Vec::new(),
            sources_generation: u64::MAX,
            jobs: Vec::new(),
            view: [0.; 12],
            buffers: None,
        }
    }
}

#[derive(Resource)]
struct FacingPipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedComputePipelineId,
}

/// Before Bevy creates its mesh allocator: vertex slabs may also be bound as storage,
/// where the adapter runs compute shaders with enough storage buffers.
fn allow_storage(
    adapter: Res<RenderAdapter>,
    device: Res<RenderDevice>,
    mut settings: ResMut<MeshAllocatorSettings>,
    mut facing: ResMut<RenderFacing>,
) {
    facing.supported = gpu_path_supported(adapter.get_downlevel_capabilities().flags, &device.limits());
    if facing.supported {
        settings.extra_buffer_usages |= BufferUsages::STORAGE;
    }
}

/// Whether this device can face on the GPU: compute shaders, the four storage buffers of the
/// pass in one compute stage, and workgroups. Decided from the adapter alone, so WebGL2, GLES
/// devices without compute or with fewer storage blocks, and Vulkan devices with a low
/// `max_storage_buffers_per_shader_stage` keep the CPU path without any setting.
fn gpu_path_supported(flags: DownlevelFlags, limits: &WgpuLimits) -> bool {
    flags.contains(DownlevelFlags::COMPUTE_SHADERS)
        && limits.max_storage_buffers_per_shader_stage >= 4
        && limits.max_compute_workgroups_per_dimension >= 1
        && limits.max_compute_invocations_per_workgroup >= 64
        && limits.max_compute_workgroup_size_x >= 64
}

fn init_pipeline(mut commands: Commands, cache: Res<PipelineCache>, facing: Res<RenderFacing>) {
    if !facing.supported {
        return;
    }
    let layout = BindGroupLayoutDescriptor::new(
        "tree facing",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                uniform_buffer_sized(false, NonZeroU64::new(48)),
            ),
        ),
    );
    let id = cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("tree facing".into()),
        layout: vec![layout.clone()],
        shader: SHADER,
        entry_point: Some("face".into()),
        ..default()
    });
    commands.insert_resource(FacingPipeline { layout, id });
}

fn extract(main: Extract<Option<Res<GpuFacing>>>, mut render: ResMut<RenderFacing>) {
    let Some(main) = main.as_ref() else {
        return;
    };
    let render = &mut *render;
    if render.placements_generation != main.placements_generation {
        render.placements.clone_from(&main.placements);
        render.placements_generation = main.placements_generation;
    }
    if render.sources_generation != main.sources_generation {
        render.sources.clone_from(&main.source_data);
        render.sources_generation = main.sources_generation;
    }
    render.jobs.clone_from(&main.jobs);
    render.view = main.view;
}

/// A slab buffer and its jobs (byte start, byte end, placement).
type SlabJobs = (Buffer, Vec<(u64, u64, u32)>);

/// One storage binding of a slab: the bound byte range and its jobs (placement, first word).
#[derive(Debug, PartialEq)]
struct Window {
    offset: u64,
    size: u64,
    jobs: Vec<[u32; 2]>,
}

/// Groups one slab's jobs (byte start, byte end, placement) into bindings that start on
/// `align` and are at most `max` bytes long. Jobs longer than `max` are dropped.
fn windows(mut jobs: Vec<(u64, u64, u32)>, align: u64, max: u64) -> Vec<Window> {
    jobs.sort_unstable_by_key(|j| j.0);
    let mut out: Vec<Window> = Vec::new();
    for (start, end, slot) in jobs {
        if let Some(window) = out.last_mut()
            && end - window.offset <= max
        {
            window.size = window.size.max(end - window.offset);
            window.jobs.push([slot, ((start - window.offset) / 4) as u32]);
            continue;
        }
        let offset = start / align * align;
        if end - offset > max {
            continue;
        }
        out.push(Window { offset, size: end - offset, jobs: vec![[slot, ((start - offset) / 4) as u32]] });
    }
    out
}

fn words_bytes(words: impl IntoIterator<Item = u32>) -> Vec<u8> {
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

fn storage(device: &RenderDevice, label: &str, contents: &[u8]) -> Buffer {
    device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some(label),
        contents,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    })
}

#[allow(clippy::too_many_arguments)]
fn dispatch(
    mut context: RenderContext,
    mut facing: ResMut<RenderFacing>,
    pipeline: Option<Res<FacingPipeline>>,
    cache: Res<PipelineCache>,
    allocator: Res<MeshAllocator>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let Some(pipeline) = pipeline else {
        return;
    };
    let Some(compiled) = cache.get_compute_pipeline(pipeline.id) else {
        return;
    };
    // Placements or sources beyond one storage binding: the CPU path from the next frame on
    // (`active` is read by the main world each frame).
    let max_binding = device.limits().max_storage_buffer_binding_size;
    let fits = (facing.placements.len() * 4) as u64 <= max_binding && (facing.sources.len() * 4) as u64 <= max_binding;
    facing.active.store(fits, Ordering::Release);
    if !fits {
        return;
    }
    let facing = &mut *facing;
    if facing.jobs.is_empty() || facing.placements.is_empty() || facing.sources.is_empty() {
        return;
    }
    // Buffers: placements and sources when they changed, jobs and view each frame.
    let rebuild_placements = facing.buffers.as_ref().is_none_or(|b| b.placements_generation != facing.placements_generation);
    let rebuild_sources = facing.buffers.as_ref().is_none_or(|b| b.sources_generation != facing.sources_generation);
    if facing.buffers.is_none() {
        facing.buffers = Some(Buffers {
            placements: storage(&device, "tree facing placements", &words_bytes(facing.placements.iter().copied())),
            placements_generation: facing.placements_generation,
            sources: storage(&device, "tree facing sources", &words_bytes(facing.sources.iter().map(|f| f.to_bits()))),
            sources_generation: facing.sources_generation,
            jobs: storage(&device, "tree facing jobs", &[0; 1024]),
            view: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("tree facing view"),
                contents: &[0; 48],
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            }),
        });
    }
    let buffers = facing.buffers.as_mut().expect("created above");
    if rebuild_placements && buffers.placements_generation != facing.placements_generation {
        buffers.placements = storage(&device, "tree facing placements", &words_bytes(facing.placements.iter().copied()));
        buffers.placements_generation = facing.placements_generation;
    }
    if rebuild_sources && buffers.sources_generation != facing.sources_generation {
        buffers.sources = storage(&device, "tree facing sources", &words_bytes(facing.sources.iter().map(|f| f.to_bits())));
        buffers.sources_generation = facing.sources_generation;
    }
    queue.write_buffer(&buffers.view, 0, &words_bytes(facing.view.iter().map(|f| f.to_bits())));

    // Jobs grouped by slab buffer, then into bindable windows.
    let limits = device.limits();
    let align = u64::from(limits.min_storage_buffer_offset_alignment.max(4));
    let placements = facing.placements.len() / PLACEMENT_WORDS;
    let mut per_buffer: HashMap<BufferId, SlabJobs> = HashMap::new();
    let mut lacking = Vec::new();
    for &(slot, mesh) in &facing.jobs {
        if slot as usize >= placements {
            continue;
        }
        let record = &facing.placements[slot as usize * PLACEMENT_WORDS..][..PLACEMENT_WORDS];
        let (count, stride) = (u64::from(record[34]), u64::from(record[36]) * 4);
        // Not allocated yet (first frame of a new asset): its own data is drawn, as on the CPU path.
        let Some(slice) = allocator.mesh_vertex_slice(&mesh) else {
            continue;
        };
        if u64::from(slice.range.end - slice.range.start) != count || stride == 0 {
            continue;
        }
        let start = u64::from(slice.range.start) * stride;
        // No STORAGE (large-object slab) or a mesh larger than one binding: faced on the CPU.
        if !slice.buffer.usage().contains(BufferUsages::STORAGE) || start % align + count * stride > max_binding {
            lacking.push(mesh);
            continue;
        }
        let entry = per_buffer.entry(slice.buffer.id()).or_insert_with(|| (slice.buffer.clone(), Vec::new()));
        entry.1.push((start, start + count * stride, slot));
    }
    if !lacking.is_empty() {
        facing.cpu_meshes.lock().unwrap_or_else(|e| e.into_inner()).extend(lacking);
    }
    let mut groups: Vec<(Buffer, Window)> = Vec::new();
    for (_, (buffer, jobs)) in per_buffer {
        for window in windows(jobs, align, max_binding) {
            groups.push((buffer.clone(), window));
        }
    }
    if groups.is_empty() {
        return;
    }
    // Each window's jobs at an aligned offset of one jobs buffer.
    let mut job_words: Vec<u32> = Vec::new();
    let mut ranges = Vec::with_capacity(groups.len());
    let align_words = (align / 4) as usize;
    for (_, window) in &groups {
        let start = job_words.len().div_ceil(align_words) * align_words;
        job_words.resize(start, 0);
        job_words.extend(window.jobs.iter().flatten());
        ranges.push((start as u64 * 4, window.jobs.len() as u64 * 8));
    }
    let job_bytes = words_bytes(job_words);
    if buffers.jobs.size() < job_bytes.len() as u64 {
        buffers.jobs = storage(&device, "tree facing jobs", &vec![0; job_bytes.len().next_power_of_two()]);
    }
    queue.write_buffer(&buffers.jobs, 0, &job_bytes);

    let layout = cache.get_bind_group_layout(&pipeline.layout);
    let groups_per_row = limits.max_compute_workgroups_per_dimension.max(1);
    let mut pass = context.command_encoder().begin_compute_pass(&ComputePassDescriptor {
        label: Some("tree facing"),
        ..default()
    });
    pass.set_pipeline(compiled);
    for ((slab, window), (job_offset, job_size)) in groups.iter().zip(ranges) {
        let bind_group = device.create_bind_group(
            "tree facing",
            &layout,
            &BindGroupEntries::sequential((
                BufferBinding { buffer: slab, offset: window.offset, size: NonZeroU64::new(window.size) },
                buffers.placements.as_entire_buffer_binding(),
                buffers.sources.as_entire_buffer_binding(),
                BufferBinding { buffer: &buffers.jobs, offset: job_offset, size: NonZeroU64::new(job_size) },
                buffers.view.as_entire_buffer_binding(),
            )),
        );
        pass.set_bind_group(0, &bind_group, &[]);
        let n = window.jobs.len() as u32;
        pass.dispatch_workgroups(n.min(groups_per_row), n.div_ceil(groups_per_row), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gpu_path_needs_compute_and_four_storage_buffers() {
        // Desktop Vulkan, DX12 and Metal defaults.
        assert!(gpu_path_supported(DownlevelFlags::all(), &WgpuLimits::default()));
        // GLES 3.1-class devices with compute and four storage blocks.
        assert!(gpu_path_supported(DownlevelFlags::all(), &WgpuLimits::downlevel_defaults()));
        // WebGL2 / GLES 3.0: no compute shaders.
        let webgl = WgpuLimits::downlevel_webgl2_defaults();
        assert!(!gpu_path_supported(DownlevelFlags::all() - DownlevelFlags::COMPUTE_SHADERS, &webgl));
        assert!(!gpu_path_supported(DownlevelFlags::all(), &webgl));
        // A device with too few storage buffers per stage.
        for n in 0..4 {
            let limits = WgpuLimits { max_storage_buffers_per_shader_stage: n, ..WgpuLimits::default() };
            assert!(!gpu_path_supported(DownlevelFlags::all(), &limits));
        }
    }

    #[test]
    fn windows_start_aligned_and_stay_within_the_binding_limit() {
        let jobs = vec![(1000, 1400, 7), (300, 700, 3), (5000, 5400, 9)];
        let w = windows(jobs, 256, 2048);
        assert_eq!(
            w,
            vec![
                Window { offset: 256, size: 1144, jobs: vec![[3, 11], [7, 186]] },
                Window { offset: 4864, size: 536, jobs: vec![[9, 34]] },
            ]
        );
        // A job longer than the limit is left to nobody rather than bound short.
        assert!(windows(vec![(0, 5000, 1)], 256, 2048).is_empty());
    }

    #[test]
    fn placements_reuse_sources_and_slots_and_reset_with_the_track() {
        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0., 1., 0.]; 4]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 0.]; 4]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.1, 0.2], [0.2, 0.2], [0.2, 0.3], [0.1, 0.3]]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, vec![[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]]);
        let source = super::super::FacingSource::from_mesh("trees/card/x", &mesh).expect("card source");
        let mut gpu = GpuFacing::new(Arc::default(), Arc::default());
        let (a, b) = (Entity::from_raw_u32(1).unwrap(), Entity::from_raw_u32(2).unwrap());
        gpu.register(a, &source.placed(Mat4::from_translation(Vec3::X)));
        gpu.register(b, &source.placed(Mat4::from_scale(Vec3::splat(2.))));
        assert_eq!(gpu.sources.len(), 1, "one copy of a shared source");
        assert_eq!(gpu.source_data.len(), 4 * SOURCE_FLOATS);
        assert_eq!(gpu.placements.len(), 2 * PLACEMENT_WORDS);
        let record = &gpu.placements[PLACEMENT_WORDS..];
        // Card, first vertex 0, 4 vertices, scale 2; words: stride, POSITION, NORMAL, UV0, UV1.
        assert_eq!(&record[32..35], &[2, 0, 4]);
        assert_eq!(f32::from_bits(record[35]), 2.);
        let layout = &source.layout;
        assert_eq!(record[36] as usize, layout.stride / 4);
        assert_eq!(record[37] as usize, layout.offset(Mesh::ATTRIBUTE_POSITION).unwrap() / 4);
        assert_eq!(record[39] as usize, layout.offset(Mesh::ATTRIBUTE_UV_0).unwrap() / 4);
        assert_eq!(record[40] as usize, layout.offset(Mesh::ATTRIBUTE_UV_1).unwrap() / 4);
        // Card normal inputs: offsets normalised to their bounds.
        assert_eq!(&gpu.source_data[7..9], &[-1., -1.]);
        assert!(gpu.push(a, AssetId::default()));
        assert!(!gpu.push(Entity::from_raw_u32(3).unwrap(), AssetId::default()));
        gpu.unregister(a);
        gpu.register(a, &source.placed(Mat4::IDENTITY));
        assert_eq!(gpu.placements.len(), 2 * PLACEMENT_WORDS, "slot reused");
        gpu.unregister(a);
        gpu.unregister(b);
        assert!(gpu.sources.is_empty() && gpu.source_data.is_empty() && gpu.placements.is_empty());
    }
}
