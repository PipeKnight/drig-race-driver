//! Frame split (docs/PERFORMANCE.md "Frame split"): appends where one frame's
//! time goes to the per-second `frame_stats` line of the frame-rate counter
//! (`[osd] fps`, F9), so a single log from another machine (the owner's Mac)
//! shows the cost of each part.
//!
//! - Main world: the main schedule (`main_ms`), the fixed simulation loop
//!   inside it (`fixed_ms`), the time between two main frames (`outside_ms`:
//!   render-world extract, waiting for the render thread, the window system)
//!   and the extract itself (`extract_ms`).
//! - Render world, per render frame: the time of each `RenderSystems` stage
//!   (`r_*`), the swap-chain acquire (`acquire`), the render-graph encode
//!   (`r_encode`), the queue submit (`r_submit`) and the present
//!   (`r_present`).
//! - GPU, with timestamp queries: empty compute passes as markers between the
//!   passes of every camera, and the sun cascades timed by their own render
//!   passes (gpu_passes::shadow_span). Read back two frames later; nothing
//!   waits for the GPU.
//! - Counts: draw groups per phase, visible meshes, shadow casters and shadow
//!   views, and mesh and image uploads per second.
//!
//! CPU parts are always measured (a few `Instant::now` per frame); GPU markers
//! run only while the counter is shown, and `GRID_FRAME_GPU=0` turns them
//! off. Independently authored diagnostics; the image is unchanged.
use bevy::camera::visibility::{CascadesVisibleEntities, VisibleEntities};
use bevy::core_pipeline::core_3d::{
    AlphaMask3d, Opaque3d, Transparent3d, main_opaque_pass_3d, main_transparent_pass_3d,
};
use bevy::core_pipeline::schedule::{camera_driver, Core2d, Core2dSystems, Core3d, Core3dSystems};
use bevy::core_pipeline::upscaling::upscaling;
use bevy::pbr::{Shadow, clear_indirect_parameters_metadata};
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_phase::{BinnedPhaseItem, ViewBinnedRenderPhases, ViewSortedRenderPhases};
use bevy::render::render_resource::{
    BufferDescriptor, BufferUsages, ComputePassDescriptor, MapMode, PollType,
};
use bevy::render::renderer::{
    RenderAdapterInfo, RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue,
    ViewQuery,
};
use bevy::render::view::window::prepare_windows;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::ui_render::{TransparentUi, ui_pass};
use std::any::TypeId;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Frames in flight for GPU readback: a frame's timestamps are read when its
/// slot comes round again, three frames later.
const RING: usize = 3;
/// Timestamp queries per frame.
const SLOT_QUERIES: u32 = 256;
/// GPU per-view detail lines: once per this many `frame_stats` windows.
const DETAIL_WINDOWS: u32 = 10;

static SHOWN: AtomicBool = AtomicBool::new(false);
static ACC: Mutex<Option<Acc>> = Mutex::new(None);

/// The frame-rate counter calls this: GPU markers run only while it is shown.
pub fn set_shown(shown: bool) {
    SHOWN.store(shown, Ordering::Relaxed);
}

fn with_acc<R>(f: impl FnOnce(&mut Acc) -> R) -> R {
    let mut guard = ACC.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Acc::default))
}

fn ns(from: Instant, to: Instant) -> u64 {
    to.saturating_duration_since(from).as_nanos() as u64
}

// ---------------------------------------------------------------------------
// Render-world CPU stages.

/// Marker points in one render frame, in schedule order.
#[derive(Clone, Copy)]
#[repr(usize)]
enum P {
    Start,
    Views,
    AcquireBegin,
    AcquireEnd,
    Queue,
    Sort,
    Prepare,
    Bind,
    Graph,
    GraphSubmit,
    GraphFinish,
    Clean,
    End,
}
const POINTS: usize = P::End as usize + 1;

/// Stage durations of a render frame, the names printed in `frame_stats`.
pub const STAGES: [&str; 12] = [
    "r_meshes", "r_views", "acquire", "r_queue", "r_sort", "r_prepare", "r_bind", "r_encode",
    "r_submit", "r_present", "r_cleanup", "render_ms",
];

/// Turns one frame's marker times (ns since the frame start) into stage
/// durations; `None` when a marker is missing.
pub fn stages(points: &[Option<u64>; POINTS]) -> Option<[u64; 12]> {
    let p = |i: P| points[i as usize];
    let d = |a: P, b: P| Some(p(b)?.saturating_sub(p(a)?));
    let acquire = d(P::AcquireBegin, P::AcquireEnd).unwrap_or(0);
    Some([
        d(P::Start, P::Views)?,
        d(P::Views, P::Queue)?.saturating_sub(acquire),
        acquire,
        d(P::Queue, P::Sort)?,
        d(P::Sort, P::Prepare)?,
        d(P::Prepare, P::Bind)?,
        d(P::Bind, P::Graph)?,
        d(P::Graph, P::GraphSubmit)?,
        d(P::GraphSubmit, P::GraphFinish)?,
        d(P::GraphFinish, P::Clean)?,
        d(P::Clean, P::End)?,
        d(P::Start, P::End)?,
    ])
}

struct RenderFrame {
    start: Option<Instant>,
    points: [Option<u64>; POINTS],
}
static RENDER_FRAME: Mutex<RenderFrame> = Mutex::new(RenderFrame {
    start: None,
    points: [None; POINTS],
});

fn point<const I: usize>() {
    let now = Instant::now();
    let mut f = RENDER_FRAME.lock().unwrap_or_else(|e| e.into_inner());
    if I == P::Start as usize {
        f.start = Some(now);
        f.points = [None; POINTS];
    }
    let Some(start) = f.start else {
        return;
    };
    f.points[I] = Some(ns(start, now));
    if I == P::End as usize {
        let done = stages(&f.points);
        f.start = None;
        drop(f);
        if let Some(done) = done {
            with_acc(|a| {
                a.render_frames += 1;
                a.last_render = done[11];
                if done[11] > a.worst[11] {
                    a.worst = done;
                }
                for (sum, v) in a.stages.iter_mut().zip(done) {
                    *sum += v;
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Phase counts (after sorting, before the graph).

/// Draw groups of the binned phases: multidraw sets, batchable bins,
/// unbatchable and non-mesh entities (each at least one draw command).
fn binned_groups<B: BinnedPhaseItem>(phases: &ViewBinnedRenderPhases<B>) -> u64 {
    phases
        .values()
        .map(|p| {
            p.multidrawable_meshes.len()
                + p.batchable_meshes.len()
                + p.unbatchable_meshes.values().map(|u| u.entities.len()).sum::<usize>()
                + p.non_mesh_items.values().map(|n| n.entities.len()).sum::<usize>()
        })
        .sum::<usize>() as u64
}

/// Names of the per-frame phase counts.
pub const COUNTS: [&str; 6] = [
    "d_opaque", "d_alpha", "d_shadow", "d_transp", "d_ui", "shadow_views",
];

fn count_phases(
    opaque: Option<Res<ViewBinnedRenderPhases<Opaque3d>>>,
    alpha: Option<Res<ViewBinnedRenderPhases<AlphaMask3d>>>,
    shadow: Option<Res<ViewBinnedRenderPhases<Shadow>>>,
    transparent: Option<Res<ViewSortedRenderPhases<Transparent3d>>>,
    ui: Option<Res<ViewSortedRenderPhases<TransparentUi>>>,
) {
    let counts = [
        opaque.map_or(0, |p| binned_groups(&p)),
        alpha.map_or(0, |p| binned_groups(&p)),
        shadow.as_ref().map_or(0, |p| binned_groups(p)),
        transparent.map_or(0, |p| p.values().map(|s| s.items.len() as u64).sum()),
        ui.map_or(0, |p| p.values().map(|s| s.items.len() as u64).sum()),
        shadow.map_or(0, |p| p.len() as u64),
    ];
    with_acc(|a| {
        a.count_frames += 1;
        for (sum, v) in a.counts.iter_mut().zip(counts) {
            *sum += v;
        }
    });
}

// ---------------------------------------------------------------------------
// Main world.

#[derive(Default)]
struct MainClock {
    begin: Option<Instant>,
    last_end: Option<Instant>,
    fixed_begin: Option<Instant>,
    /// This frame's fixed loop, outside time and fixed ticks.
    frame_fixed: u64,
    frame_outside: u64,
    frame_ticks: u32,
}
static MAIN_CLOCK: Mutex<MainClock> = Mutex::new(MainClock {
    begin: None,
    last_end: None,
    fixed_begin: None,
    frame_fixed: 0,
    frame_outside: 0,
    frame_ticks: 0,
});

/// Frame-time limits (ms) of the slow-frame attribution: `slow10_*` and
/// `slow14_*` in `frame_stats` (the 100 fps budget and the 1 % lows).
const SLOW_MS: [u64; 2] = [10, 14];

/// Sums over the frames slower than one of `SLOW_MS`: where the worst frames'
/// time goes (main schedule, its fixed ticks, waiting for the render thread).
#[derive(Clone, Copy, Default)]
struct Slow {
    frames: u32,
    frame: u64,
    main: u64,
    fixed: u64,
    outside: u64,
    render: u64,
    ticks: u64,
    /// Frames that ran two or more fixed ticks (catch-up).
    multi: u32,
}

fn main_begin() {
    let now = Instant::now();
    let mut c = MAIN_CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    c.begin = Some(now);
    c.frame_fixed = 0;
    c.frame_ticks = 0;
    c.frame_outside = 0;
    if let Some(end) = c.last_end {
        let outside = ns(end, now);
        c.frame_outside = outside;
        with_acc(|a| a.outside += outside);
    }
}

/// One fixed tick (`FixedFirst`).
fn fixed_tick() {
    MAIN_CLOCK.lock().unwrap_or_else(|e| e.into_inner()).frame_ticks += 1;
}

fn fixed_begin() {
    MAIN_CLOCK.lock().unwrap_or_else(|e| e.into_inner()).fixed_begin = Some(Instant::now());
}

fn fixed_end() {
    let now = Instant::now();
    let mut c = MAIN_CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(begin) = c.fixed_begin.take() {
        let fixed = ns(begin, now);
        c.frame_fixed += fixed;
        drop(c);
        with_acc(|a| a.fixed += fixed);
    }
}

/// Last system of the main frame: main time, uploads, visible meshes.
fn main_end(
    mut meshes: MessageReader<AssetEvent<Mesh>>,
    mut images: MessageReader<AssetEvent<Image>>,
    mesh_assets: Res<Assets<Mesh>>,
    image_assets: Res<Assets<Image>>,
    cameras: Query<(Entity, &Camera, &VisibleEntities), With<Camera3d>>,
    lights: Query<&CascadesVisibleEntities>,
    mut streamed_before: Local<u64>,
) {
    // The render world prepares each added or modified mesh once per frame, however many
    // events it got: count distinct ids. A mesh kept for the main world only is not uploaded.
    let mut mesh = (0u64, 0u64);
    let mut seen = std::collections::HashSet::new();
    for event in meshes.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = event
            && seen.insert(*id)
        {
            let asset = mesh_assets.get(*id);
            if asset.is_some_and(|m| !uploaded(m)) {
                continue;
            }
            mesh.0 += 1;
            mesh.1 += asset.map_or(0, mesh_bytes);
        }
    }
    let mut image = (0u64, 0u64);
    for event in images.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = event {
            image.0 += 1;
            image.1 += image_assets
                .get(*id)
                .and_then(|i| i.data.as_ref())
                .map_or(0, |d| d.len() as u64);
        }
    }
    let mut visible = 0u64;
    let mut casters = 0u64;
    let mut views = 0u64;
    for (entity, camera, entities) in &cameras {
        if !camera.is_active {
            continue;
        }
        views += 1;
        visible += entities.get(TypeId::of::<Mesh3d>()).len() as u64;
        for light in &lights {
            if let Some(cascades) = light.entities.get(&entity) {
                casters += cascades.iter().map(|c| c.entities.len() as u64).sum::<u64>();
            }
        }
    }
    // Vertex bytes written in place (mesh_stream), not counted as uploads.
    let queued = crate::mesh_stream::queued_bytes();
    let streamed = queued.saturating_sub(std::mem::replace(&mut *streamed_before, queued));
    let now = Instant::now();
    let mut c = MAIN_CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    let main = c.begin.take().map_or(0, |b| ns(b, now));
    let frame = c.last_end.map(|end| ns(end, now));
    let (fixed, outside, ticks) = (c.frame_fixed, c.frame_outside, c.frame_ticks);
    c.last_end = Some(now);
    drop(c);
    with_acc(|a| {
        if let Some(frame) = frame {
            for (slow, limit) in a.slow.iter_mut().zip(SLOW_MS) {
                if frame > limit * 1_000_000 {
                    slow.frames += 1;
                    slow.frame += frame;
                    slow.main += main;
                    slow.fixed += fixed;
                    slow.outside += outside;
                    slow.render += a.last_render;
                    slow.ticks += u64::from(ticks);
                    slow.multi += u32::from(ticks >= 2);
                }
            }
        }
        a.main_frames += 1;
        a.main += main;
        a.mesh_up += mesh.0;
        a.mesh_bytes += mesh.1;
        a.stream_bytes += streamed;
        a.image_up += image.0;
        a.image_bytes += image.1;
        a.visible += visible;
        a.casters += casters;
        a.views3d += views;
    });
}

/// Whether a changed mesh goes to the render world (is prepared and uploaded).
pub(crate) fn uploaded(mesh: &Mesh) -> bool {
    mesh.asset_usage.contains(bevy::asset::RenderAssetUsages::RENDER_WORLD)
}

/// CPU bytes of a mesh's vertex attributes and indices (what an upload
/// copies); never panics on an asset whose data left the CPU.
pub fn mesh_bytes(mesh: &Mesh) -> u64 {
    let vertices: u64 = mesh.try_attributes().map_or(0, |attributes| {
        attributes
            .map(|(attribute, values)| attribute.format.size() * values.len() as u64)
            .sum()
    });
    let indices = mesh.try_indices().ok().map_or(0, |i| match i {
        bevy::mesh::Indices::U16(v) => 2 * v.len() as u64,
        bevy::mesh::Indices::U32(v) => 4 * v.len() as u64,
    });
    vertices + indices
}

// ---------------------------------------------------------------------------
// GPU timestamps.

/// What a timestamp marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    FrameBegin,
    FrameEnd,
    /// 3D camera: before culling and shadows, before the opaque pass, before
    /// the transparent pass, after the main pass, after upscaling.
    C3Start,
    C3Main,
    C3Transparent,
    C3Post,
    C3End,
    /// 2D (display) camera: start, before upscaling, after upscaling.
    C2Start,
    C2Upscale,
    C2End,
    ShadowBegin(u8),
    ShadowEnd(u8),
}

const K_C3_START: u8 = 0;
const K_C3_MAIN: u8 = 1;
const K_C3_TRANSPARENT: u8 = 2;
const K_C3_POST: u8 = 3;
const K_C3_END: u8 = 4;
const K_C2_START: u8 = 5;
const K_C2_UPSCALE: u8 = 6;
const K_C2_END: u8 = 7;
const K_FRAME_BEGIN: u8 = 8;
const K_FRAME_END: u8 = 9;

const fn kind(k: u8) -> Kind {
    match k {
        K_C3_START => Kind::C3Start,
        K_C3_MAIN => Kind::C3Main,
        K_C3_TRANSPARENT => Kind::C3Transparent,
        K_C3_POST => Kind::C3Post,
        K_C3_END => Kind::C3End,
        K_C2_START => Kind::C2Start,
        K_C2_UPSCALE => Kind::C2Upscale,
        K_C2_END => Kind::C2End,
        K_FRAME_BEGIN => Kind::FrameBegin,
        _ => Kind::FrameEnd,
    }
}

/// One timestamp: what it marks and the view (camera order, render entity
/// index) it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    pub kind: Kind,
    pub order: isize,
    pub view: u32,
}

/// GPU segment names, in `frame_stats` order (ms per frame).
pub const SEGMENTS: [&str; 7] = [
    "g_pre", "g_shadow", "g_opaque", "g_transp", "g_post", "g_ui", "g_upscale",
];

/// Per-view times of one frame (ms): pre, shadow, opaque, transparent, post,
/// ui, upscale, and the cascades.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewTimes {
    pub order: isize,
    pub view: u32,
    pub three_d: bool,
    pub segments: [f64; 7],
    pub cascades: Vec<f64>,
}

/// One frame's GPU split: per-view times, the span of all markers and the
/// number of invalid (negative or missing-partner) intervals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameTimes {
    pub views: Vec<ViewTimes>,
    pub span: f64,
    pub bad: u32,
}

/// Splits one frame's timestamps (ticks) into segments; `period_ns` is the
/// queue's timestamp period.
pub fn split(marks: &[Mark], ticks: &[u64], period_ns: f64) -> FrameTimes {
    let ms = |t: u64| t as f64 * period_ns * 1e-6;
    let mut out = FrameTimes::default();
    let valid: Vec<u64> = ticks.iter().copied().filter(|&t| t != 0).collect();
    if let (Some(min), Some(max)) = (valid.iter().min(), valid.iter().max()) {
        out.span = ms(max - min);
    }
    let mut views: BTreeMap<(isize, u32), Vec<(Kind, u64)>> = BTreeMap::new();
    for (m, &t) in marks.iter().zip(ticks) {
        if !matches!(m.kind, Kind::FrameBegin | Kind::FrameEnd) {
            views.entry((m.order, m.view)).or_default().push((m.kind, t));
        }
    }
    for ((order, view), list) in views {
        let at = |k: Kind| list.iter().find(|(x, _)| *x == k).map(|(_, t)| *t);
        let mut bad = 0;
        let mut d = |a: Option<u64>, b: Option<u64>| match (a, b) {
            (Some(a), Some(b)) if b >= a && a != 0 => ms(b - a),
            (Some(_), Some(_)) => {
                bad += 1;
                0.
            }
            _ => 0.,
        };
        let mut times = ViewTimes {
            order,
            view,
            ..default()
        };
        let three_d = at(Kind::C3Start).is_some() || at(Kind::C3Main).is_some();
        times.three_d = three_d;
        if three_d {
            for i in 0..=u8::MAX {
                let begin = at(Kind::ShadowBegin(i));
                if begin.is_none() {
                    break;
                }
                times.cascades.push(d(begin, at(Kind::ShadowEnd(i))));
            }
            let shadow: f64 = times.cascades.iter().sum();
            let pre = d(at(Kind::C3Start), at(Kind::C3Main));
            times.segments = [
                (pre - shadow).max(0.),
                shadow,
                d(at(Kind::C3Main), at(Kind::C3Transparent)),
                d(at(Kind::C3Transparent), at(Kind::C3Post)),
                d(at(Kind::C3Post), at(Kind::C3End)),
                0.,
                0.,
            ];
        } else {
            times.segments[5] = d(at(Kind::C2Start), at(Kind::C2Upscale));
            times.segments[6] = d(at(Kind::C2Upscale), at(Kind::C2End));
        }
        out.bad += bad;
        out.views.push(times);
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SlotState {
    Free,
    Recording,
    Pending,
}

struct Slot {
    marks: Vec<Mark>,
    state: SlotState,
    /// 0 waiting, 1 mapped, 2 failed.
    mapped: Arc<AtomicU8>,
    /// Frames since submit, to drop a slot whose map never completes.
    age: u32,
}

struct GpuInner {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: Vec<wgpu::Buffer>,
    slots: Vec<Slot>,
    current: Option<usize>,
    frame: usize,
}

/// Render-world resource for the GPU markers (present only when the adapter
/// has timestamp queries and `GRID_FRAME_GPU` is not `0`).
#[derive(Resource)]
pub struct FrameGpu {
    inner: Mutex<GpuInner>,
    period_ns: f64,
}

impl FrameGpu {
    /// Reserves `n` consecutive queries of this frame for `marks`; `None`
    /// when no frame is recording or the slot is full.
    pub fn alloc(&self, marks: &[Mark]) -> Option<(wgpu::QuerySet, u32)> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let slot = g.current?;
        let used = g.slots[slot].marks.len() as u32;
        if used + marks.len() as u32 > SLOT_QUERIES {
            return None;
        }
        g.slots[slot].marks.extend_from_slice(marks);
        Some((g.set.clone(), slot as u32 * SLOT_QUERIES + used))
    }
}

fn gpu_reason(features: wgpu::Features) -> Result<(), &'static str> {
    if std::env::var("GRID_FRAME_GPU").is_ok_and(|v| v == "0") {
        return Err("GRID_FRAME_GPU=0");
    }
    if !features.contains(wgpu::Features::TIMESTAMP_QUERY) {
        return Err("no_timestamp_query");
    }
    Ok(())
}

fn gpu_startup(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    info: Res<RenderAdapterInfo>,
) {
    let features = device.features();
    let reason = gpu_reason(features);
    println!(
        "frame_split version=1 backend={:?} adapter=\"{}\" timestamp_query={} inside_encoders={} inside_passes={} gpu_markers={} reason={} near_present_marker={}",
        info.backend,
        info.name,
        features.contains(wgpu::Features::TIMESTAMP_QUERY),
        features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS),
        features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES),
        if reason.is_ok() { "on" } else { "off" },
        reason.err().unwrap_or("ok"),
        !cfg!(target_os = "macos"),
    );
    if reason.is_err() {
        return;
    }
    let wgpu_device = device.wgpu_device();
    let set = wgpu_device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("frame_split"),
        ty: wgpu::QueryType::Timestamp,
        count: SLOT_QUERIES * RING as u32,
    });
    let bytes = u64::from(SLOT_QUERIES) * 8;
    let resolve = device.create_buffer(&BufferDescriptor {
        label: Some("frame_split_resolve"),
        size: bytes * RING as u64,
        usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = (0..RING)
        .map(|_| {
            device.create_buffer(&BufferDescriptor {
                label: Some("frame_split_readback"),
                size: bytes,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        })
        .map(|b| (*b).clone())
        .collect();
    let slots = (0..RING)
        .map(|_| Slot {
            marks: Vec::new(),
            state: SlotState::Free,
            mapped: Arc::new(AtomicU8::new(0)),
            age: 0,
        })
        .collect();
    commands.insert_resource(FrameGpu {
        inner: Mutex::new(GpuInner {
            set,
            resolve: (*resolve).clone(),
            readback,
            slots,
            current: None,
            frame: 0,
        }),
        period_ns: f64::from(queue.get_timestamp_period()),
    });
}

/// Start of the render graph: collect finished slots, open this frame's.
fn gpu_begin(gpu: Res<FrameGpu>, device: Res<RenderDevice>) {
    let _ = device.wgpu_device().poll(PollType::Poll);
    let mut finished = Vec::new();
    {
        let mut g = gpu.inner.lock().unwrap_or_else(|e| e.into_inner());
        let g = &mut *g;
        for (i, slot) in g.slots.iter_mut().enumerate() {
            if slot.state != SlotState::Pending {
                continue;
            }
            slot.age += 1;
            match slot.mapped.load(Ordering::Acquire) {
                1 => {
                    let len = slot.marks.len() as u64 * 8;
                    let ticks: Vec<u64> = {
                        let data = g.readback[i].slice(..len).get_mapped_range();
                        data.as_chunks::<8>()
                            .0
                            .iter()
                            .map(|c| u64::from_le_bytes(*c))
                            .collect()
                    };
                    g.readback[i].unmap();
                    finished.push(split(&slot.marks, &ticks, gpu.period_ns));
                    slot.state = SlotState::Free;
                }
                2 => slot.state = SlotState::Free,
                // A map that never completes would block the ring.
                _ if slot.age > 60 => slot.state = SlotState::Free,
                _ => {}
            }
        }
        g.current = None;
        g.frame = g.frame.wrapping_add(1);
        let i = g.frame % RING;
        if SHOWN.load(Ordering::Relaxed) && g.slots[i].state == SlotState::Free {
            let slot = &mut g.slots[i];
            slot.marks.clear();
            slot.state = SlotState::Recording;
            slot.age = 0;
            slot.mapped.store(0, Ordering::Release);
            g.current = Some(i);
        }
    }
    if !finished.is_empty() {
        with_acc(|a| {
            for frame in finished {
                a.add_gpu(frame);
            }
        });
    }
}

fn write_marker(gpu: &FrameGpu, ctx: &mut RenderContext, mark: Mark) {
    let Some((set, index)) = gpu.alloc(&[mark]) else {
        return;
    };
    // An empty compute pass whose beginning writes the timestamp: needs only
    // TIMESTAMP_QUERY (Metal has no timestamps inside passes, and Bevy avoids
    // encoder timestamps there).
    let _pass = ctx
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("frame_split_marker"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: &set,
                beginning_of_pass_write_index: Some(index),
                end_of_pass_write_index: None,
            }),
        });
}

fn view_marker<const K: u8>(
    view: ViewQuery<(Entity, &ExtractedCamera)>,
    gpu: Option<Res<FrameGpu>>,
    mut ctx: RenderContext,
) {
    let Some(gpu) = gpu else {
        return;
    };
    let (entity, camera) = view.into_inner();
    let mark = Mark {
        kind: kind(K),
        order: camera.order,
        view: entity.index_u32(),
    };
    write_marker(&gpu, &mut ctx, mark);
}

fn frame_marker<const K: u8>(gpu: Option<Res<FrameGpu>>, mut ctx: RenderContext) {
    let Some(gpu) = gpu else {
        return;
    };
    let mark = Mark {
        kind: kind(K),
        order: isize::MIN,
        view: 0,
    };
    write_marker(&gpu, &mut ctx, mark);
}

/// After the camera passes: resolve this frame's queries into its readback
/// buffer (one command buffer, submitted with the frame).
fn gpu_resolve(gpu: Res<FrameGpu>, mut ctx: RenderContext) {
    let (set, resolve, readback, base, used) = {
        let mut g = gpu.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(i) = g.current else {
            return;
        };
        let used = g.slots[i].marks.len() as u32;
        if used == 0 {
            g.slots[i].state = SlotState::Free;
            g.current = None;
            return;
        }
        g.slots[i].state = SlotState::Pending;
        (
            g.set.clone(),
            g.resolve.clone(),
            g.readback[i].clone(),
            i as u32 * SLOT_QUERIES,
            used,
        )
    };
    let encoder = ctx.command_encoder();
    let offset = u64::from(base) * 8;
    encoder.resolve_query_set(&set, base..base + used, &resolve, offset);
    encoder.copy_buffer_to_buffer(&resolve, offset, &readback, 0, u64::from(used) * 8);
}

/// After submit: map this frame's readback buffer.
fn gpu_finish(gpu: Res<FrameGpu>) {
    let g = gpu.inner.lock().unwrap_or_else(|e| e.into_inner());
    let Some(i) = g.current else {
        return;
    };
    if g.slots[i].state != SlotState::Pending {
        return;
    }
    let flag = g.slots[i].mapped.clone();
    let len = g.slots[i].marks.len() as u64 * 8;
    g.readback[i].map_async(MapMode::Read, ..len, move |r| {
        flag.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
    });
}

// ---------------------------------------------------------------------------
// Window accumulator and report.

#[derive(Default)]
struct ViewAcc {
    frames: u32,
    three_d: bool,
    segments: [f64; 7],
    cascades: Vec<f64>,
}

#[derive(Default)]
struct Acc {
    main_frames: u32,
    main: u64,
    fixed: u64,
    outside: u64,
    extract: u64,
    render_frames: u32,
    stages: [u64; 12],
    /// Stages of the window's slowest render frame (`rworst`).
    worst: [u64; 12],
    count_frames: u32,
    counts: [u64; 6],
    mesh_up: u64,
    mesh_bytes: u64,
    stream_bytes: u64,
    image_up: u64,
    image_bytes: u64,
    visible: u64,
    casters: u64,
    views3d: u64,
    gpu_frames: u32,
    gpu_bad: u32,
    gpu: [f64; 7],
    gpu_span: f64,
    /// render_ms of the last finished render frame (kept across resets).
    last_render: u64,
    slow: [Slow; 2],
    /// Per-view detail over `DETAIL_WINDOWS` windows (kept across resets).
    detail: BTreeMap<(isize, u32), ViewAcc>,
    detail_windows: u32,
}

impl Acc {
    fn add_gpu(&mut self, frame: FrameTimes) {
        self.gpu_frames += 1;
        self.gpu_bad += frame.bad;
        self.gpu_span += frame.span;
        for v in frame.views {
            for (sum, x) in self.gpu.iter_mut().zip(v.segments) {
                *sum += x;
            }
            let d = self.detail.entry((v.order, v.view)).or_default();
            d.frames += 1;
            d.three_d = v.three_d;
            for (sum, x) in d.segments.iter_mut().zip(v.segments) {
                *sum += x;
            }
            if d.cascades.len() < v.cascades.len() {
                d.cascades.resize(v.cascades.len(), 0.);
            }
            for (sum, x) in d.cascades.iter_mut().zip(&v.cascades) {
                *sum += x;
            }
        }
    }
}

/// Fields appended to `frame_stats` for a window of `seconds`, plus the
/// per-view GPU detail lines when they are due. Resets the window.
pub fn window_report(seconds: f64) -> (String, Vec<String>) {
    let gpu_possible = SHOWN.load(Ordering::Relaxed);
    with_acc(|a| {
        let per = |sum: u64, n: u32| sum as f64 * 1e-6 / f64::from(n.max(1));
        let mut s = format!(
            " main_ms={:.2} fixed_ms={:.2} outside_ms={:.2} extract_ms={:.2}",
            per(a.main, a.main_frames),
            per(a.fixed, a.main_frames),
            per(a.outside, a.main_frames),
            per(a.extract, a.main_frames),
        );
        for (name, sum) in STAGES.iter().zip(a.stages) {
            s += &format!(" {name}={:.2}", per(sum, a.render_frames));
        }
        s += &format!(" render_frames={} rworst={}", a.render_frames, worst_field(&a.worst));
        if a.gpu_frames > 0 {
            let n = f64::from(a.gpu_frames);
            let total: f64 = a.gpu.iter().sum();
            s += &format!(" gpu=on gpu_ms={:.2}", total / n);
            for (name, sum) in SEGMENTS.iter().zip(a.gpu) {
                s += &format!(" {name}={:.2}", sum / n);
            }
            s += &format!(
                " g_gap={:.2} g_frames={} g_bad={}",
                (a.gpu_span - total) / n,
                a.gpu_frames,
                a.gpu_bad
            );
        } else {
            s += if gpu_possible { " gpu=off" } else { " gpu=hidden" };
        }
        let c = f64::from(a.count_frames.max(1));
        for (name, sum) in COUNTS.iter().zip(a.counts) {
            s += &format!(" {name}={:.0}", sum as f64 / c);
        }
        let m = f64::from(a.main_frames.max(1));
        let secs = seconds.max(1e-3);
        s += &format!(
            " vis={:.0} casters={:.0} views3d={:.1} mesh_up_s={:.1} mesh_kb_s={:.0} mesh_stream_kb_s={:.0} img_up_s={:.1} img_kb_s={:.0}",
            a.visible as f64 / m,
            a.casters as f64 / m,
            a.views3d as f64 / m,
            a.mesh_up as f64 / secs,
            a.mesh_bytes as f64 / 1024. / secs,
            a.stream_bytes as f64 / 1024. / secs,
            a.image_up as f64 / secs,
            a.image_bytes as f64 / 1024. / secs,
        );
        for (slow, limit) in a.slow.iter().zip(SLOW_MS) {
            let n = slow.frames;
            s += &format!(
                " slow{limit}_frames={n} slow{limit}_ms={:.2} slow{limit}_main={:.2} slow{limit}_fixed={:.2} slow{limit}_outside={:.2} slow{limit}_render={:.2} slow{limit}_ticks={:.2} slow{limit}_multi={}",
                per(slow.frame, n),
                per(slow.main, n),
                per(slow.fixed, n),
                per(slow.outside, n),
                per(slow.render, n),
                slow.ticks as f64 / f64::from(n.max(1)),
                slow.multi
            );
        }
        s += &crate::fixed_ticks::window_suffix();
        let mut lines = Vec::new();
        a.detail_windows += 1;
        if a.detail_windows >= DETAIL_WINDOWS && !a.detail.is_empty() {
            for ((order, view), d) in std::mem::take(&mut a.detail) {
                let n = f64::from(d.frames.max(1));
                let mut line = format!(
                    "frame_gpu_views cam={order} view={view} kind={} frames={}",
                    if d.three_d { "3d" } else { "2d" },
                    d.frames
                );
                for (name, sum) in SEGMENTS.iter().zip(d.segments) {
                    line += &format!(" {name}={:.3}", sum / n);
                }
                for (i, sum) in d.cascades.iter().enumerate() {
                    line += &format!(" shadow_c{i}={:.3}", sum / n);
                }
                lines.push(line);
            }
            a.detail_windows = 0;
        }
        let detail = std::mem::take(&mut a.detail);
        let detail_windows = a.detail_windows;
        let last_render = a.last_render;
        *a = Acc {
            detail,
            detail_windows,
            last_render,
            ..Acc::default()
        };
        (s, lines)
    })
}

/// The slowest render frame of a window: `render_ms` first, then the
/// other stages in `STAGES` order, in ms, joined by `/`.
fn worst_field(worst: &[u64; 12]) -> String {
    std::iter::once(worst[11])
        .chain(worst[..11].iter().copied())
        .map(|v| format!("{:.1}", v as f64 * 1e-6))
        .collect::<Vec<_>>()
        .join("/")
}

// ---------------------------------------------------------------------------

pub fn install(app: &mut App) {
    app.add_systems(First, main_begin)
        .add_systems(FixedFirst, fixed_tick)
        .add_systems(Last, main_end)
        .add_systems(
            RunFixedMainLoop,
            (
                fixed_begin
                    .after(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                    .before(RunFixedMainLoopSystems::FixedMainLoop),
                fixed_end
                    .after(RunFixedMainLoopSystems::FixedMainLoop)
                    .before(RunFixedMainLoopSystems::AfterFixedMainLoop),
            ),
        );
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    // The extract runs on the main thread between two main frames.
    if let Some(mut extract) = render.take_extract() {
        render.set_extract(move |main, render| {
            let begin = Instant::now();
            extract(main, render);
            let took = ns(begin, Instant::now());
            with_acc(|a| a.extract += took);
        });
    }
    use RenderSystems as S;
    render.add_systems(
        Render,
        (
            point::<{ P::Start as usize }>.before(S::ExtractCommands),
            point::<{ P::Views as usize }>.after(S::Specialize).before(S::PrepareViews),
            point::<{ P::AcquireBegin as usize }>
                .in_set(S::PrepareViews)
                .before(prepare_windows),
            point::<{ P::AcquireEnd as usize }>
                .in_set(S::PrepareViews)
                .after(prepare_windows),
            point::<{ P::Queue as usize }>.after(S::PrepareViews).before(S::Queue),
            point::<{ P::Sort as usize }>.after(S::Queue).before(S::PhaseSort),
            (point::<{ P::Prepare as usize }>, count_phases)
                .after(S::PhaseSort)
                .before(S::Prepare),
            point::<{ P::Bind as usize }>
                .after(S::PrepareResourcesFlush)
                .before(S::PrepareBindGroups),
            point::<{ P::Graph as usize }>.after(S::Prepare).before(S::Render),
            point::<{ P::Clean as usize }>.after(S::Render).before(S::Cleanup),
            point::<{ P::End as usize }>.after(S::PostCleanup),
        ),
    );
    render.add_systems(
        RenderGraph,
        (
            point::<{ P::GraphSubmit as usize }>
                .after(RenderGraphSystems::Render)
                .before(RenderGraphSystems::Submit),
            point::<{ P::GraphFinish as usize }>.after(RenderGraphSystems::Finish),
        ),
    );
    render.add_systems(RenderStartup, gpu_startup);
    let has_gpu = |gpu: Option<Res<FrameGpu>>| gpu.is_some();
    render.add_systems(
        RenderGraph,
        (
            (gpu_begin, frame_marker::<K_FRAME_BEGIN>)
                .chain()
                .in_set(RenderGraphSystems::Begin)
                .run_if(has_gpu),
            gpu_resolve
                .after(RenderGraphSystems::Render)
                .before(RenderGraphSystems::Submit)
                .run_if(has_gpu),
            gpu_finish
                .in_set(RenderGraphSystems::Finish)
                .run_if(has_gpu),
        ),
    );
    // Timestamps after the last pass flicker the present on macOS (Bevy skips
    // its encoder timestamps there for bevy#22257): no end markers there.
    if !cfg!(target_os = "macos") {
        render.add_systems(
            RenderGraph,
            frame_marker::<K_FRAME_END>
                .after(camera_driver)
                .before(gpu_resolve)
                .run_if(has_gpu),
        );
    }
    render.add_systems(
        Core3d,
        (
            view_marker::<K_C3_START>
                .before(Core3dSystems::Prepass)
                .before(clear_indirect_parameters_metadata),
            view_marker::<K_C3_MAIN>
                .in_set(Core3dSystems::MainPass)
                .before(main_opaque_pass_3d),
            view_marker::<K_C3_TRANSPARENT>
                .in_set(Core3dSystems::MainPass)
                .after(main_opaque_pass_3d)
                .before(main_transparent_pass_3d),
            view_marker::<K_C3_POST>
                .after(Core3dSystems::MainPass)
                .before(Core3dSystems::EarlyPostProcess),
            view_marker::<K_C3_END>.after(upscaling),
        ),
    );
    render.add_systems(
        Core2d,
        (
            view_marker::<K_C2_START>.before(Core2dSystems::Prepass),
            view_marker::<K_C2_UPSCALE>.after(ui_pass).before(upscaling),
        ),
    );
    if !cfg!(target_os = "macos") {
        render.add_systems(Core2d, view_marker::<K_C2_END>.after(upscaling));
    }
    crate::gpu_passes::install_shadow_span(render);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(kind: Kind, order: isize) -> Mark {
        Mark {
            kind,
            order,
            view: order as u32,
        }
    }

    #[test]
    fn render_stages_split_the_frame_and_sum_to_it() {
        let mut p = [None; POINTS];
        for (i, t) in [0, 10, 12, 15, 20, 26, 30, 40, 45, 70, 80, 90, 95]
            .into_iter()
            .enumerate()
        {
            p[i] = Some(t);
        }
        let s = stages(&p).unwrap();
        // meshes, views minus acquire, acquire, queue, sort, prepare, bind,
        // encode (graph..submit), submit, present, cleanup, total.
        assert_eq!(s, [10, 7, 3, 6, 4, 10, 5, 25, 10, 10, 5, 95]);
        assert_eq!(s[..11].iter().sum::<u64>(), s[11]);
        p[P::Bind as usize] = None;
        assert_eq!(stages(&p), None);
    }

    #[test]
    fn gpu_split_separates_shadows_passes_and_ui() {
        let marks = [
            m(Kind::FrameBegin, isize::MIN),
            m(Kind::C3Start, 0),
            m(Kind::ShadowBegin(0), 0),
            m(Kind::ShadowEnd(0), 0),
            m(Kind::ShadowBegin(1), 0),
            m(Kind::ShadowEnd(1), 0),
            m(Kind::C3Main, 0),
            m(Kind::C3Transparent, 0),
            m(Kind::C3Post, 0),
            m(Kind::C3End, 0),
            m(Kind::C2Start, 1),
            m(Kind::C2Upscale, 1),
            m(Kind::C2End, 1),
            m(Kind::FrameEnd, isize::MIN),
        ];
        // Ticks in microseconds with a 1000 ns period: values in ms / 1000.
        let ticks = [
            1000, 1000, 1500, 2500, 2500, 3000, 4000, 7000, 8000, 9000, 9000, 9500, 9700, 9800,
        ];
        let f = split(&marks, &ticks, 1000.);
        assert_eq!(f.bad, 0);
        assert!((f.span - 8.8).abs() < 1e-9);
        assert_eq!(f.views.len(), 2);
        let v3 = &f.views[0];
        assert!(v3.three_d);
        assert_eq!(v3.cascades, vec![1.0, 0.5]);
        let close = |a: [f64; 7], b: [f64; 7]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(close(v3.segments, [1.5, 1.5, 3., 1., 1., 0., 0.]), "{v3:?}");
        let v2 = &f.views[1];
        assert!(!v2.three_d);
        assert!(close(v2.segments, [0., 0., 0., 0., 0., 0.5, 0.2]), "{v2:?}");
        let total: f64 = f.views.iter().flat_map(|v| v.segments).sum();
        assert!((f.span - total - 0.1).abs() < 1e-9, "gap is the 0.1 ms tail");
    }

    #[test]
    fn negative_or_unwritten_intervals_count_as_bad() {
        let marks = [m(Kind::C3Start, 0), m(Kind::C3Main, 0), m(Kind::C3Transparent, 0)];
        let f = split(&marks, &[5000, 3000, 0], 1000.);
        assert_eq!(f.bad, 2);
        assert!(f.views[0].segments.iter().all(|&x| x == 0.));
    }

    #[test]
    fn report_averages_and_resets() {
        with_acc(|a| {
            *a = Acc::default();
            a.main_frames = 2;
            a.main = 4_000_000;
            a.render_frames = 2;
            a.stages[11] = 10_000_000;
            a.worst[11] = 7_000_000;
            a.worst[5] = 5_500_000;
            a.mesh_up = 85;
            a.add_gpu(FrameTimes {
                views: vec![ViewTimes {
                    order: 0,
                    view: 3,
                    three_d: true,
                    segments: [1., 2., 3., 4., 5., 0., 0.],
                    cascades: vec![1., 1.],
                }],
                span: 16.,
                bad: 0,
            });
        });
        let (s, lines) = window_report(1.);
        assert!(s.contains(" main_ms=2.00 "), "{s}");
        assert!(s.contains(" render_ms=5.00 "), "{s}");
        assert!(s.contains(" rworst=7.0/0.0/0.0/0.0/0.0/0.0/5.5/0.0/0.0/0.0/0.0/0.0 "), "{s}");
        assert!(s.contains(" gpu=on gpu_ms=15.00 g_pre=1.00 g_shadow=2.00"), "{s}");
        assert!(s.contains(" g_gap=1.00 g_frames=1 g_bad=0"), "{s}");
        assert!(s.contains(" mesh_up_s=85.0 "), "{s}");
        assert!(lines.is_empty());
        let (s, _) = window_report(1.);
        assert!(s.contains(" main_ms=0.00 ") && s.contains(" mesh_up_s=0.0 "), "{s}");
        assert!(s.contains(" rworst=0.0/"), "{s}");
        // The per-view detail survives resets until it is due.
        for _ in 0..DETAIL_WINDOWS - 3 {
            assert!(window_report(1.).1.is_empty());
        }
        let (_, lines) = window_report(1.);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].starts_with("frame_gpu_views cam=0 view=3 kind=3d frames=1 g_pre=1.000")
                && lines[0].ends_with(" shadow_c0=1.000 shadow_c1=1.000"),
            "{}",
            lines[0]
        );
    }
}
