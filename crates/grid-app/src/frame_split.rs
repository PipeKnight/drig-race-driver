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
//! - Process: CPU time over wall time (`cpu_load`, 2.0 = two cores busy),
//!   page faults (`faults`) and involuntary context switches (`ivcsw`, Unix
//!   only, `-` elsewhere) of the window. A window whose stages all slow down
//!   for the same work is told apart: more faults (memory pressure), more
//!   involuntary switches (another process took the cores), or neither
//!   (slower cores: power or thermal state).
//!
//! CPU parts are always measured (a few `Instant::now` per frame); GPU markers
//! run only while the counter is shown, and `GRID_FRAME_GPU=0` turns them
//! off. Independently authored diagnostics; the image is unchanged.
use bevy::camera::visibility::{CascadesVisibleEntities, VisibleEntities};
use bevy::core_pipeline::core_3d::{
    AlphaMask3d, Opaque3d, Transparent3d, main_opaque_pass_3d, main_transparent_pass_3d,
};
use bevy::core_pipeline::prepass::{AlphaMask3dPrepass, Opaque3dPrepass};
use bevy::core_pipeline::schedule::{camera_driver, Core2d, Core2dSystems, Core3d, Core3dSystems};
use bevy::core_pipeline::upscaling::upscaling;
use bevy::pbr::{Shadow, clear_indirect_parameters_metadata};
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_phase::{BinnedPhaseItem, ViewBinnedRenderPhases, ViewSortedRenderPhases};
use bevy::render::render_resource::{
    ComputePassDescriptor, MapMode, PollType,
};
use bevy::render::renderer::{
    PendingCommandBuffers, RenderAdapterInfo, RenderContext, RenderDevice, RenderGraph,
    RenderGraphSystems, RenderQueue,
    ViewQuery,
};
use bevy::render::view::window::prepare_windows;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::ui_render::{TransparentUi, ui_pass};
use std::any::TypeId;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
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
/// The last finished render frame's swap-chain acquire, ns (render_scale
/// steers on it, F932).
static LAST_ACQUIRE_NS: AtomicU64 = AtomicU64::new(0);
static ACC: Mutex<Option<Acc>> = Mutex::new(None);

/// The last finished render frame's swap-chain acquire, ms: the wait for a
/// free drawable, which is where a GPU-bound frame waits on Metal (F929).
/// Measured on every frame, shown or not.
pub fn last_acquire_ms() -> f32 {
    LAST_ACQUIRE_NS.load(Ordering::Relaxed) as f32 / 1e6
}

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
    uploads: [u64; 3],
    /// Main frames finished when this render frame started (the frame it draws).
    main: u64,
}
/// Uploads of the last main frame (meshes prepared, their bytes, streamed
/// vertex bytes), taken by the render frame that draws it.
static FRAME_UPLOADS: Mutex<[u64; 3]> = Mutex::new([0; 3]);

static RENDER_FRAME: Mutex<RenderFrame> = Mutex::new(RenderFrame {
    start: None,
    points: [None; POINTS],
    uploads: [0; 3],
    main: 0,
});

/// Main-world laps: wall time of named main-world work in one main frame
/// (`lap_*`, `rworst_laps` and `rslow_laps` in `frame_stats`). The work
/// charges itself ([`Lap`]); `main_end` closes the frame.
pub mod lap {
    pub const DRIVE_TICK: usize = 0;
    pub const DRIVE_PRESENT: usize = 1;
    pub const OWN_DENTS: usize = 2;
    pub const OPPONENT_DENTS: usize = 3;
    pub const GROUND: usize = 4;
    pub const GROUND_GATHER: usize = 5;
    pub const PROPAGATE: usize = 6;
    pub const BOUNDS: usize = 7;
    pub const STDOUT: usize = 8;
    pub const MULTI: usize = 9;
    pub const N: usize = 10;
    pub const NAMES: [&str; N] = ["dt", "dp", "dpo", "dpn", "gd", "gdg", "tp", "cb", "out", "mt"];
}
static LAPS: [AtomicU64; lap::N] = [const { AtomicU64::new(0) }; lap::N];
/// Finished main frames and the laps of the last four, for the render frame
/// that ran beside them.
static MAIN_LAPS: Mutex<(u64, [[u64; lap::N]; 4])> = Mutex::new((0, [[0; lap::N]; 4]));
static LAP_BEGIN: Mutex<[Option<Instant>; lap::N]> = Mutex::new([None; lap::N]);
/// Main-frame laps of `frame_stats` over 2 ms (`lap_*` third field).
const LAP_OVER_NS: u64 = 2_000_000;

/// Charges the time until it drops to one lap.
pub struct Lap(usize, Instant);
impl Lap {
    pub fn new(lap: usize) -> Self {
        Self(lap, Instant::now())
    }
}
impl Drop for Lap {
    fn drop(&mut self) {
        LAPS[self.0].fetch_add(ns(self.1, Instant::now()), Ordering::Relaxed);
    }
}

fn lap_begin<const L: usize>() {
    LAP_BEGIN.lock().unwrap_or_else(|e| e.into_inner())[L] = Some(Instant::now());
}

fn lap_end<const L: usize>() {
    if let Some(begin) = LAP_BEGIN.lock().unwrap_or_else(|e| e.into_inner())[L].take() {
        LAPS[L].fetch_add(ns(begin, Instant::now()), Ordering::Relaxed);
    }
}

/// The largest laps of the main frames after `drawn` (the frame a render
/// frame draws) up to the running one: the work beside that render frame.
fn laps_beside(drawn: u64) -> [u64; lap::N] {
    let (done, ring) = *MAIN_LAPS.lock().unwrap_or_else(|e| e.into_inner());
    let mut out: [u64; lap::N] = std::array::from_fn(|i| LAPS[i].load(Ordering::Relaxed));
    for n in (drawn + 1)..=done {
        if done - n < 4 {
            for (o, v) in out.iter_mut().zip(ring[(n % 4) as usize]) {
                *o = (*o).max(v);
            }
        }
    }
    out
}

/// With a transform propagation of 3 ms or more (`GRID_TRANSFORM_BURST_MS`
/// sets another threshold), logs what changed, by hierarchy root
/// (`transform_burst`). `same` counts the changed `Transform`s whose value
/// equals the one they had when last seen changed: writes that dirty the
/// hierarchy without moving anything (F943), with their roots in `same_roots`.
fn transform_burst(
    global: Query<Entity, Changed<GlobalTransform>>,
    local: Query<(Entity, &Transform), Changed<Transform>>,
    added: Query<(), Added<GlobalTransform>>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    mut seen: Local<std::collections::HashMap<Entity, Transform>>,
) {
    static LIMIT: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    let limit = *LIMIT.get_or_init(|| {
        crate::dev::var("GRID_TRANSFORM_BURST_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(3.)
    });
    let ms = LAPS[lap::PROPAGATE].load(Ordering::Relaxed) as f64 * 1e-6;
    let mut same = Vec::new();
    if limit < 3. {
        for (e, t) in &local {
            if seen.insert(e, *t) == Some(*t) {
                same.push(e);
            }
        }
    }
    if ms < limit {
        return;
    }
    let root = |mut e: Entity| {
        while let Ok(p) = parents.get(e) {
            e = p.parent();
        }
        e
    };
    let top = |it: &mut dyn Iterator<Item = Entity>| {
        let mut by: std::collections::HashMap<Entity, u32> = Default::default();
        let mut n = 0;
        for e in it {
            *by.entry(root(e)).or_default() += 1;
            n += 1;
        }
        let mut v: Vec<_> = by.into_iter().collect();
        v.sort_by_key(|&(e, c)| (std::cmp::Reverse(c), e));
        let s: Vec<String> = v
            .iter()
            .take(4)
            .map(|(e, c)| format!("{c}@{}", names.get(*e).map_or_else(|_| format!("{e}"), |n| n.as_str().replace(' ', "_"))))
            .collect();
        (n, s.join(","))
    };
    let (g, gs) = top(&mut global.iter());
    let (l, ls) = top(&mut local.iter().map(|(e, _)| e));
    let (n, ss) = top(&mut same.into_iter());
    println!(
        "transform_burst ms={ms:.2} global_changed={g} added={} transform_changed={l} same={n} global_roots={gs} transform_roots={ls} same_roots={ss}",
        added.iter().count()
    );
}

fn point<const I: usize>() {
    let now = Instant::now();
    let mut f = RENDER_FRAME.lock().unwrap_or_else(|e| e.into_inner());
    if I == P::Start as usize {
        f.start = Some(now);
        f.points = [None; POINTS];
        f.uploads = std::mem::take(&mut *FRAME_UPLOADS.lock().unwrap_or_else(|e| e.into_inner()));
        f.main = MAIN_LAPS.lock().unwrap_or_else(|e| e.into_inner()).0;
    }
    let Some(start) = f.start else {
        return;
    };
    f.points[I] = Some(ns(start, now));
    if I == P::Graph as usize {
        enc_begin(now);
    } else if I == P::GraphSubmit as usize {
        enc_end(now);
    }
    if I == P::End as usize {
        let done = stages(&f.points);
        let uploads = f.uploads;
        let drawn = f.main;
        f.start = None;
        drop(f);
        if let Some(done) = done {
            LAST_ACQUIRE_NS.store(done[2], Ordering::Relaxed);
            let laps = laps_beside(drawn);
            with_acc(|a| {
                a.render_frames += 1;
                a.slow_render += u32::from(done[11] >= SLOW_RENDER_NS);
                a.last_render = done[11];
                if done[11] > a.worst[11] {
                    a.worst = done;
                    a.worst_uploads = uploads;
                    a.worst_laps = laps;
                }
                if done[11] >= 20_000_000 {
                    a.rslow_n += 1;
                    for (s, v) in a.rslow_laps.iter_mut().zip(laps) {
                        *s += v;
                    }
                }
                let split = &mut a.upload_split[usize::from(uploads[0] > 0)];
                split.0 += 1;
                split.1 += done[0];
                split.2 += done[5];
                a.upload_max = a.upload_max.max(uploads[1]);
                a.stream_max = a.stream_max.max(uploads[2]);
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
pub const COUNTS: [&str; 12] = [
    "d_opaque", "d_alpha", "d_shadow", "d_transp", "d_ui", "shadow_views", "d_pre", "d_pre_alpha", "bl_slabs", "bl_res",
    "bl_std", "bl_ovl",
];

/// The prepass phases (opaque and alpha mask).
type PrepassPhases<'w> = (
    Option<Res<'w, ViewBinnedRenderPhases<Opaque3dPrepass>>>,
    Option<Res<'w, ViewBinnedRenderPhases<AlphaMask3dPrepass>>>,
);

/// Bindless resources held by the slabs of one material type.
fn resources_of<M: 'static>(allocators: &bevy::pbr::MaterialBindGroupAllocators) -> u64 {
    allocators.get(&std::any::TypeId::of::<M>()).map_or(0, |m| m.allocations())
}

fn count_phases(
    opaque: Option<Res<ViewBinnedRenderPhases<Opaque3d>>>,
    alpha: Option<Res<ViewBinnedRenderPhases<AlphaMask3d>>>,
    shadow: Option<Res<ViewBinnedRenderPhases<Shadow>>>,
    transparent: Option<Res<ViewSortedRenderPhases<Transparent3d>>>,
    ui: Option<Res<ViewSortedRenderPhases<TransparentUi>>>,
    prepass: PrepassPhases,
    allocators: Option<Res<bevy::pbr::MaterialBindGroupAllocators>>,
) {
    let counts = [
        opaque.map_or(0, |p| binned_groups(&p)),
        alpha.map_or(0, |p| binned_groups(&p)),
        shadow.as_ref().map_or(0, |p| binned_groups(p)),
        transparent.map_or(0, |p| p.values().map(|s| s.items.len() as u64).sum()),
        ui.map_or(0, |p| p.values().map(|s| s.items.len() as u64).sum()),
        shadow.map_or(0, |p| p.len() as u64),
        prepass.0.map_or(0, |p| binned_groups(&p)),
        prepass.1.map_or(0, |p| binned_groups(&p)),
        allocators.as_ref().map_or(0, |a| a.values().map(|m| m.slab_count() as u64).sum()),
        allocators.as_ref().map_or(0, |a| a.values().map(|m| m.allocations()).sum()),
        allocators.as_ref().map_or(0, |a| resources_of::<StandardMaterial>(a)),
        allocators.map_or(0, |a| resources_of::<crate::car_reflection_overlay::Overlay>(&a)),
    ];
    with_acc(|a| {
        a.count_frames += 1;
        for (sum, v) in a.counts.iter_mut().zip(counts) {
            *sum += v;
        }
    });
}

/// State changes of the sorted transparent phase after batching (`t_*` in
/// `frame_stats`): draws, switches of pipeline and material asset between
/// consecutive draws, phase items, and the distinct pipelines and materials
/// (docs/PERFORMANCE.md, F865).
fn count_transparent(
    transparent: Option<Res<ViewSortedRenderPhases<Transparent3d>>>,
    materials: Option<Res<bevy::pbr::RenderMaterialInstances>>,
) {
    let Some(transparent) = transparent else {
        return;
    };
    let mut t = [0u64; 6];
    let mut pipes = std::collections::HashSet::new();
    let mut mats = std::collections::HashSet::new();
    for phase in transparent.values() {
        let mut last = None;
        // Every item is queued with batch_range 0..1; a batch set's first item
        // gets the set's whole instance range, and the render loop
        // (SortedRenderPhase::render_range) skips that many items. So a draw is
        // an item at the walk's position, not any item with a non-empty range.
        let mut next = 0;
        for (index, item) in phase.items.iter().map(|(_, i)| i).enumerate() {
            let mat = materials
                .as_ref()
                .and_then(|m| m.instances.get(&item.entity.1))
                .map(|m| m.asset_id);
            t[2] += 1;
            if index == next && !item.batch_range.is_empty() {
                next += item.batch_range.len();
                t[0] += 1;
                if let Some((p, m)) = last {
                    t[1] += u64::from(p != item.pipeline);
                    t[3] += u64::from(m != mat);
                }
                last = Some((item.pipeline, mat));
            } else if index == next {
                next += 1;
            }
            pipes.insert(item.pipeline);
            mats.insert(mat);
        }
    }
    t[4] = pipes.len() as u64;
    t[5] = mats.len() as u64;
    with_acc(|a| {
        a.t_frames += 1;
        for (sum, v) in a.t_counts.iter_mut().zip(t) {
            *sum += v;
        }
    });
}

/// Short name of a material type for [`dump_transparent`].
fn material_name(t: TypeId) -> &'static str {
    use crate::*;
    let names: [(TypeId, &str); 16] = [
        (TypeId::of::<StandardMaterial>(), "std"),
        (TypeId::of::<car_reflection_overlay::Overlay>(), "overlay"),
        (TypeId::of::<pane_slab::Pane>(), "pane"),
        (TypeId::of::<window_response::Window>(), "window"),
        (TypeId::of::<tyre_smoke::SmokeMaterial>(), "smoke"),
        (TypeId::of::<skid_marks::SkidMaterial>(), "skid"),
        (TypeId::of::<tree_cards::CardMaterial>(), "card"),
        (TypeId::of::<crowd_preview::CrowdMaterial>(), "crowd"),
        (TypeId::of::<track_surface::SurfaceMaterial>(), "surface"),
        (TypeId::of::<light_shafts::ShaftMaterial>(), "shaft"),
        (TypeId::of::<water::WaterMaterial>(), "water"),
        (TypeId::of::<wheel_exposure::HullMaterial>(), "hull"),
        (TypeId::of::<paint_environment::Paint>(), "paint"),
        (TypeId::of::<sidewall_filter::SidewallMaterial>(), "sidewall"),
        (TypeId::of::<foundation_backing::RawMaterial>(), "raw"),
        (TypeId::of::<sky::SkyMaterial>(), "sky"),
    ];
    names.iter().find(|(id, _)| *id == t).map_or("other", |(_, n)| n)
}

#[derive(Default)]
struct TrnDump {
    frames: u64,
    draws: BTreeMap<&'static str, u64>,
    items: BTreeMap<&'static str, u64>,
    // pipeline, material bind group (type and slab), vertex buffer, index buffer, draw function
    switches: [u64; 5],
    pairs: BTreeMap<(&'static str, &'static str), u64>,
}

static TRN_DUMP: Mutex<Option<TrnDump>> = Mutex::new(None);

/// `GRID_TRN_DUMP=1` (diagnostic, F886): every 300 render frames a
/// `trn_dump` line with the sorted transparent phase's draws and items by
/// material type, the state switches between consecutive draws (pipeline,
/// material bind group = type and slab, vertex buffer, index buffer, draw
/// function) and the most common material-type transitions.
fn dump_transparent(
    transparent: Option<Res<ViewSortedRenderPhases<Transparent3d>>>,
    materials: Option<Res<bevy::pbr::RenderMaterialInstances>>,
    bindings: Option<Res<bevy::pbr::RenderMaterialBindings>>,
    meshes: Option<Res<bevy::pbr::RenderMeshInstances>>,
    allocator: Option<Res<bevy::render::mesh::allocator::MeshAllocator>>,
) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| crate::dev::var("GRID_TRN_DUMP").is_ok_and(|v| v == "1")) {
        return;
    }
    let (Some(transparent), Some(materials)) = (transparent, materials) else {
        return;
    };
    let mut guard = TRN_DUMP.lock().unwrap();
    let d = guard.get_or_insert_with(TrnDump::default);
    d.frames += 1;
    for phase in transparent.values() {
        let mut last: Option<(_, _, _, _, _, &'static str)> = None;
        let mut next = 0;
        for (index, item) in phase.items.iter().map(|(_, i)| i).enumerate() {
            let asset = materials.instances.get(&item.entity.1).map(|m| m.asset_id);
            let name = asset.map_or("none", |a| material_name(a.type_id()));
            *d.items.entry(name).or_default() += 1;
            if index != next {
                continue;
            }
            next += item.batch_range.len().max(1);
            if item.batch_range.is_empty() {
                continue;
            }
            *d.draws.entry(name).or_default() += 1;
            let group = asset
                .and_then(|a| bindings.as_ref().and_then(|b| b.get(&a)))
                .map(|b| b.group.0);
            let mesh = meshes.as_ref().and_then(|m| m.mesh_asset_id(item.entity.1));
            let vb = mesh
                .and_then(|m| allocator.as_ref().and_then(|a| a.mesh_vertex_slice(&m)))
                .map(|s| s.buffer.id());
            let ib = mesh
                .and_then(|m| allocator.as_ref().and_then(|a| a.mesh_index_slice(&m)))
                .map(|s| s.buffer.id());
            let key = (item.pipeline, (asset.map(|a| a.type_id()), group), vb, ib, item.draw_function, name);
            if let Some(l) = last {
                d.switches[0] += u64::from(l.0 != key.0);
                d.switches[1] += u64::from(l.1 != key.1);
                d.switches[2] += u64::from(l.2 != key.2);
                d.switches[3] += u64::from(l.3 != key.3);
                d.switches[4] += u64::from(l.4 != key.4);
                if l.5 != name {
                    *d.pairs.entry((l.5, name)).or_default() += 1;
                }
            }
            last = Some(key);
        }
    }
    if d.frames >= 300 {
        let f = d.frames as f64;
        let per = |m: &BTreeMap<&str, u64>| {
            m.iter().map(|(k, v)| format!("{k}:{:.1}", *v as f64 / f)).collect::<Vec<_>>().join(",")
        };
        let mut pairs: Vec<_> = d.pairs.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1));
        let pairs = pairs
            .iter()
            .take(10)
            .map(|((a, b), v)| format!("{a}>{b}:{:.1}", **v as f64 / f))
            .collect::<Vec<_>>()
            .join(",");
        let s = d.switches.map(|v| v as f64 / f);
        println!(
            "trn_dump frames={} draws={} items={} sw_pipe={:.1} sw_matgroup={:.1} sw_vb={:.1} sw_ib={:.1} sw_drawfn={:.1} pairs={pairs}",
            d.frames, per(&d.draws), per(&d.items), s[0], s[1], s[2], s[3], s[4]
        );
        *d = TrnDump::default();
    }
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
    /// Main-world laps of these frames (`slow*_laps`, `tick*_laps`).
    laps: [u64; lap::N],
}

/// Mean milliseconds of a nanosecond sum over `n` frames.
fn per(sum: u64, n: u32) -> f64 {
    sum as f64 * 1e-6 / f64::from(n.max(1))
}

impl Slow {
    /// One frame: `t` = frame, main, fixed, outside and render ns.
    fn add(&mut self, t: [u64; 5], ticks: u32, laps: &[u64; lap::N]) {
        let [frame, main, fixed, outside, render] = t;
        self.frames += 1;
        self.frame += frame;
        self.main += main;
        self.fixed += fixed;
        self.outside += outside;
        self.render += render;
        self.ticks += u64::from(ticks);
        self.multi += u32::from(ticks >= 2);
        for (s, v) in self.laps.iter_mut().zip(laps) {
            *s += v;
        }
    }

    /// `{p}_frames= {p}_ms= ... {p}_laps=` of `frame_stats`.
    fn report(&self, p: &str) -> String {
        let n = self.frames;
        let laps: Vec<String> = self.laps.iter().map(|&x| format!("{:.1}", per(x, n))).collect();
        format!(
            " {p}_frames={n} {p}_ms={:.2} {p}_main={:.2} {p}_fixed={:.2} {p}_outside={:.2} {p}_render={:.2} {p}_ticks={:.2} {p}_multi={} {p}_laps={}",
            per(self.frame, n),
            per(self.main, n),
            per(self.fixed, n),
            per(self.outside, n),
            per(self.render, n),
            self.ticks as f64 / f64::from(n.max(1)),
            self.multi,
            laps.join("/")
        )
    }
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
    // mesh_stream totals at the previous frame: (queued bytes, write_buffer calls).
    mut stream_before: Local<(u64, u64)>,
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
    let streamed = queued.saturating_sub(std::mem::replace(&mut stream_before.0, queued));
    let writes = crate::mesh_stream::upload_writes();
    let stream_writes = writes.saturating_sub(std::mem::replace(&mut stream_before.1, writes));
    let laps: [u64; lap::N] = std::array::from_fn(|i| LAPS[i].swap(0, Ordering::Relaxed));
    {
        let mut m = MAIN_LAPS.lock().unwrap_or_else(|e| e.into_inner());
        m.0 += 1;
        let at = (m.0 % 4) as usize;
        m.1[at] = laps;
    }
    let now = Instant::now();
    *FRAME_UPLOADS.lock().unwrap_or_else(|e| e.into_inner()) = [mesh.0, mesh.1, streamed];
    let mut c = MAIN_CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    let main = c.begin.take().map_or(0, |b| ns(b, now));
    let frame = c.last_end.map(|end| ns(end, now));
    let (fixed, outside, ticks) = (c.frame_fixed, c.frame_outside, c.frame_ticks);
    c.last_end = Some(now);
    drop(c);
    with_acc(|a| {
        if let Some(frame) = frame {
            let t = [frame, main, fixed, outside, a.last_render];
            for (slow, limit) in a.slow.iter_mut().zip(SLOW_MS) {
                if frame > limit * 1_000_000 {
                    slow.add(t, ticks, &laps);
                }
            }
            a.ticked[usize::from(ticks > 0)].add(t, ticks, &laps);
        }
        a.main_frames += 1;
        a.main += main;
        for (i, v) in laps.into_iter().enumerate() {
            a.laps[i] += v;
            a.lap_max[i] = a.lap_max[i].max(v);
            a.lap_over[i] += u32::from(v > LAP_OVER_NS);
        }
        a.mesh_up += mesh.0;
        a.mesh_bytes += mesh.1;
        a.stream_bytes += streamed;
        a.stream_writes += stream_writes;
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

/// Frames a submitted slot waits for its map before the ring gives up on it.
const MAP_TIMEOUT_FRAMES: u32 = 60;

struct Slot {
    marks: Vec<Mark>,
    state: SlotState,
    /// 0 waiting, 1 mapped, 2 failed. A fresh flag for every map, so the late
    /// callback of a map the ring gave up on cannot speak for the next one.
    mapped: Arc<AtomicU8>,
    /// `map_async` was called on this slot's buffer and the buffer has not
    /// been unmapped or replaced since. wgpu 29 has no map-state query; a copy
    /// into such a buffer fails the whole submit and a second `map_async`
    /// panics ("Buffer is already mapped", F929), so a slot records again
    /// only with this clear.
    map_outstanding: bool,
    /// Frames since submit, to give up on a slot whose map never completes.
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

/// One frame read back: its marks and their GPU ticks.
type ReadFrame = (Vec<Mark>, Vec<u64>);

fn readback_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame_split_readback"),
        size: u64::from(SLOT_QUERIES) * 8,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl GpuInner {
    fn new(device: &wgpu::Device, set: wgpu::QuerySet) -> Self {
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame_split_resolve"),
            size: u64::from(SLOT_QUERIES) * 8 * RING as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        Self {
            set,
            resolve,
            readback: (0..RING).map(|_| readback_buffer(device)).collect(),
            slots: (0..RING)
                .map(|_| Slot {
                    marks: Vec::new(),
                    state: SlotState::Free,
                    mapped: Arc::new(AtomicU8::new(0)),
                    map_outstanding: false,
                    age: 0,
                })
                .collect(),
            current: None,
            frame: 0,
        }
    }

    /// Collects the submitted slots whose maps finished: the marks and ticks
    /// of every frame read, and how many slots were given up on. A slot whose
    /// map failed or timed out gets a new buffer: wgpu unmaps the old one when
    /// it is dropped (a pending map is aborted), while an explicit `unmap()`
    /// of a buffer whose map failed is a validation error that Bevy turns into
    /// a render error. The caller polls the device first.
    fn collect(&mut self, device: &wgpu::Device) -> (Vec<ReadFrame>, u32) {
        let mut read = Vec::new();
        let mut lost = 0;
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.state != SlotState::Pending {
                continue;
            }
            slot.age += 1;
            let flag = slot.mapped.load(Ordering::Acquire);
            if slot.map_outstanding && flag == 1 {
                let len = slot.marks.len() as u64 * 8;
                let ticks: Vec<u64> = {
                    let data = self.readback[i].slice(..len).get_mapped_range();
                    data.as_chunks::<8>()
                        .0
                        .iter()
                        .map(|c| u64::from_le_bytes(*c))
                        .collect()
                };
                self.readback[i].unmap();
                slot.map_outstanding = false;
                read.push((std::mem::take(&mut slot.marks), ticks));
                slot.state = SlotState::Free;
            } else if slot.map_outstanding && (flag == 2 || slot.age > MAP_TIMEOUT_FRAMES) {
                self.readback[i] = readback_buffer(device);
                slot.map_outstanding = false;
                slot.state = SlotState::Free;
                lost += 1;
            } else if !slot.map_outstanding && slot.age > MAP_TIMEOUT_FRAMES {
                // Submitted but never mapped: the buffer is idle.
                slot.state = SlotState::Free;
            }
        }
        (read, lost)
    }

    /// Opens this frame's slot when the counter is shown and the slot is free;
    /// otherwise the frame records no GPU timings.
    fn open(&mut self, shown: bool) {
        self.current = None;
        self.frame = self.frame.wrapping_add(1);
        let i = self.frame % RING;
        let slot = &mut self.slots[i];
        if shown && slot.state == SlotState::Free && !slot.map_outstanding {
            slot.marks.clear();
            slot.state = SlotState::Recording;
            slot.age = 0;
            self.current = Some(i);
        }
    }

    /// Closes this frame's slot: its readback buffer, first query and query
    /// count, or `None` when it recorded nothing.
    fn close(&mut self) -> Option<(wgpu::Buffer, u32, u32)> {
        let i = self.current?;
        let slot = &mut self.slots[i];
        let used = slot.marks.len() as u32;
        if used == 0 || slot.map_outstanding {
            slot.state = SlotState::Free;
            self.current = None;
            return None;
        }
        slot.state = SlotState::Pending;
        Some((self.readback[i].clone(), i as u32 * SLOT_QUERIES, used))
    }

    /// After submit: maps this frame's readback buffer.
    fn start_map(&mut self) {
        let Some(i) = self.current else {
            return;
        };
        let slot = &mut self.slots[i];
        if slot.state != SlotState::Pending || slot.map_outstanding {
            return;
        }
        let flag = Arc::new(AtomicU8::new(0));
        slot.mapped = flag.clone();
        slot.map_outstanding = true;
        let len = slot.marks.len() as u64 * 8;
        self.readback[i].map_async(MapMode::Read, ..len, move |r| {
            flag.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
        });
    }
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
    if crate::dev::var("GRID_FRAME_GPU").is_ok_and(|v| v == "0") {
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
    commands.insert_resource(FrameGpu {
        inner: Mutex::new(GpuInner::new(wgpu_device, set)),
        period_ns: f64::from(queue.get_timestamp_period()),
    });
}

/// Start of the render graph: collect finished slots, open this frame's.
fn gpu_begin(gpu: Res<FrameGpu>, device: Res<RenderDevice>) {
    let device = device.wgpu_device();
    let _ = device.poll(PollType::Poll);
    let (read, lost) = {
        let mut g = gpu.inner.lock().unwrap_or_else(|e| e.into_inner());
        let collected = g.collect(device);
        g.open(SHOWN.load(Ordering::Relaxed));
        collected
    };
    if !read.is_empty() || lost > 0 {
        with_acc(|a| {
            a.gpu_lost += lost;
            for (marks, ticks) in read {
                let frame = split(&marks, &ticks, gpu.period_ns);
                if frame.bad == 0 && frame.span > 0. {
                    LAST_GPU_NS.store((frame.span * 1e6) as u64, Ordering::Relaxed);
                }
                a.add_gpu(frame);
            }
        });
    }
}

/// The GPU span (all markers) of the newest frame read back, in ns; 0 until
/// one is read (no markers on this adapter, or none yet). `adpf.rs` reports it.
static LAST_GPU_NS: AtomicU64 = AtomicU64::new(0);

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn last_gpu_ns() -> u64 {
    LAST_GPU_NS.load(Ordering::Relaxed)
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

/// CPU side of the view markers: when each camera schedule reached each
/// marker, between the `r_encode` points (`enc_*` in `frame_stats`).
struct EncFrame {
    graph: Option<Instant>,
    marks: Vec<(u8, isize, Instant)>,
}

static ENC: Mutex<EncFrame> = Mutex::new(EncFrame {
    graph: None,
    marks: Vec::new(),
});

/// One camera schedule run in `r_encode`: `lead` (from the previous camera's
/// last marker, or from the graph start, to this camera's first marker: it
/// holds the previous schedule's encoder finishes), then the marker
/// intervals (3D: prepass+shadows, opaque, transparent, post; 2D: ui, upscale).
const ENC_SEGS: usize = 5;

fn enc_begin(now: Instant) {
    let mut e = ENC.lock().unwrap_or_else(|e| e.into_inner());
    e.graph = Some(now);
    e.marks.clear();
}

fn enc_end(now: Instant) {
    let mut e = ENC.lock().unwrap_or_else(|e| e.into_inner());
    let Some(graph) = e.graph.take() else {
        return;
    };
    let marks = std::mem::take(&mut e.marks);
    drop(e);
    let mut cams: Vec<((u8, u8, isize), [u64; ENC_SEGS])> = Vec::new();
    let mut prev = graph;
    for &(k, order, at) in &marks {
        if k == K_C3_START || k == K_C2_START {
            let three = u8::from(k == K_C3_START);
            let mut seg = [0; ENC_SEGS];
            seg[0] = ns(prev, at);
            cams.push(((cams.len() as u8, three, order), seg));
        } else if let Some((_, seg)) = cams.last_mut() {
            let i = match k {
                K_C3_MAIN | K_C2_UPSCALE => 1,
                K_C3_TRANSPARENT | K_C2_END => 2,
                K_C3_POST => 3,
                _ => 4,
            };
            seg[i] += ns(prev, at);
        }
        prev = at;
    }
    let tail = ns(prev, now);
    with_acc(|a| {
        a.enc_frames += 1;
        a.enc_tail += tail;
        for (key, seg) in cams {
            let s = a.enc.entry(key).or_default();
            s.0 += 1;
            for (sum, v) in s.1.iter_mut().zip(seg) {
                *sum += v;
            }
        }
    });
}

fn count_buffers(pending: Res<PendingCommandBuffers>) {
    let n = pending.len() as u64;
    with_acc(|a| a.enc_buffers += n);
}

fn view_marker<const K: u8>(
    view: ViewQuery<(Entity, &ExtractedCamera)>,
    gpu: Option<Res<FrameGpu>>,
    mut ctx: RenderContext,
) {
    let now = Instant::now();
    let (entity, camera) = view.into_inner();
    {
        let mut e = ENC.lock().unwrap_or_else(|e| e.into_inner());
        if e.graph.is_some() {
            e.marks.push((K, camera.order, now));
        }
    }
    let Some(gpu) = gpu else {
        return;
    };
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
        let Some((readback, base, used)) = g.close() else {
            return;
        };
        (g.set.clone(), g.resolve.clone(), readback, base, used)
    };
    let encoder = ctx.command_encoder();
    let offset = u64::from(base) * 8;
    encoder.resolve_query_set(&set, base..base + used, &resolve, offset);
    encoder.copy_buffer_to_buffer(&resolve, offset, &readback, 0, u64::from(used) * 8);
}

/// After submit: map this frame's readback buffer.
fn gpu_finish(gpu: Res<FrameGpu>) {
    gpu.inner.lock().unwrap_or_else(|e| e.into_inner()).start_map();
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

/// A render frame this long or longer counts in `rslow20` (F860: the rate of
/// slow render frames per window, not only the slowest one).
const SLOW_RENDER_NS: u64 = 20_000_000;

#[derive(Default)]
struct Acc {
    main_frames: u32,
    main: u64,
    fixed: u64,
    outside: u64,
    extract: u64,
    render_frames: u32,
    stages: [u64; 12],
    /// Camera schedules in `r_encode` by (position in the frame, 3D, order):
    /// runs and [`ENC_SEGS`] sums; frames, the tail after the last marker
    /// and command buffers pending at submit (`enc_*`).
    enc: BTreeMap<(u8, u8, isize), (u32, [u64; ENC_SEGS])>,
    enc_frames: u32,
    enc_tail: u64,
    enc_buffers: u64,
    /// [`count_transparent`] sums and frames.
    t_counts: [u64; 6],
    t_frames: u32,
    /// Stages of the window's slowest render frame (`rworst`).
    worst: [u64; 12],
    /// Uploads drawn by that frame (`rworst_up`: meshes/KB/streamed KB).
    worst_uploads: [u64; 3],
    /// Render frames without and with prepared meshes: (frames, r_meshes,
    /// r_prepare) sums (`r_meshes_up`, `r_meshes_noup`).
    upload_split: [(u32, u64, u64); 2],
    /// Largest mesh and streamed bytes of one frame (`up_max_kb`, `stream_max_kb`).
    upload_max: u64,
    stream_max: u64,
    /// Render frames of [`SLOW_RENDER_NS`] or more (`rslow20`).
    slow_render: u32,
    /// Main-world laps: sums, largest and frames over 2 ms (`lap_*`).
    laps: [u64; lap::N],
    lap_max: [u64; lap::N],
    lap_over: [u32; lap::N],
    /// Laps beside the slowest render frame, and summed beside render frames of 20 ms or more.
    worst_laps: [u64; lap::N],
    rslow_laps: [u64; lap::N],
    rslow_n: u32,
    count_frames: u32,
    counts: [u64; COUNTS.len()],
    mesh_up: u64,
    mesh_bytes: u64,
    stream_bytes: u64,
    /// mesh_stream `write_buffer` calls (`mesh_stream_writes`, per main frame).
    stream_writes: u64,
    image_up: u64,
    image_bytes: u64,
    visible: u64,
    casters: u64,
    views3d: u64,
    gpu_frames: u32,
    gpu_bad: u32,
    /// Readback slots given up on (map failed or timed out, F930).
    gpu_lost: u32,
    gpu: [f64; 7],
    gpu_span: f64,
    /// render_ms of the last finished render frame (kept across resets).
    last_render: u64,
    slow: [Slow; 2],
    /// Main frames without and with a fixed tick (`tick0_*`, `tick1_*`).
    ticked: [Slow; 2],
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

/// Process counters so far: CPU time (user and system), page faults and
/// involuntary context switches where the system counts them.
#[derive(Clone, Copy)]
struct Usage {
    cpu_ns: u64,
    faults: u64,
    ivcsw: Option<u64>,
}

#[cfg(unix)]
fn usage() -> Option<Usage> {
    // SAFETY: getrusage writes the zeroed local it is given.
    let mut u: libc::rusage = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut u) } == 0;
    let ns = |t: libc::timeval| t.tv_sec as u64 * 1_000_000_000 + t.tv_usec as u64 * 1_000;
    ok.then(|| Usage {
        cpu_ns: ns(u.ru_utime) + ns(u.ru_stime),
        faults: (u.ru_minflt + u.ru_majflt) as u64,
        ivcsw: Some(u.ru_nivcsw as u64),
    })
}

#[cfg(windows)]
fn usage() -> Option<Usage> {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn GetProcessTimes(
            process: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }
    let [mut c, mut e, mut k, mut u] = std::array::from_fn(|_| FileTime::default());
    // SAFETY: this process's pseudo handle and four out-pointers to locals.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) } != 0;
    let ns = |t: &FileTime| ((u64::from(t.high) << 32) | u64::from(t.low)) * 100;
    if !ok {
        return None;
    }
    Some(Usage {
        cpu_ns: ns(&k) + ns(&u),
        faults: crate::fixed_ticks::page_faults()?,
        ivcsw: None,
    })
}

#[cfg(not(any(windows, unix)))]
fn usage() -> Option<Usage> {
    None
}

/// `cpu_load`, `faults` and `ivcsw` since the previous call (the previous
/// window); empty on the first call or where the counters are missing.
fn usage_fields() -> String {
    static LAST: Mutex<Option<(Instant, Usage)>> = Mutex::new(None);
    let now = (Instant::now(), usage());
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let previous = last.take();
    let Some(current) = now.1 else { return String::new() };
    *last = Some((now.0, current));
    let Some((at, before)) = previous else { return String::new() };
    let wall = now.0.duration_since(at).as_nanos().max(1) as f64;
    let ivcsw = match (current.ivcsw, before.ivcsw) {
        (Some(a), Some(b)) => a.saturating_sub(b).to_string(),
        _ => "-".into(),
    };
    format!(
        " cpu_load={:.2} faults={} ivcsw={ivcsw}",
        current.cpu_ns.saturating_sub(before.cpu_ns) as f64 / wall,
        current.faults.saturating_sub(before.faults),
    )
}

/// Fields appended to `frame_stats` for a window of `seconds`, plus the
/// per-view GPU detail lines when they are due. Resets the window.
pub fn window_report(seconds: f64) -> (String, Vec<String>) {
    let gpu_possible = SHOWN.load(Ordering::Relaxed);
    with_acc(|a| {
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
        s += &format!(
            " render_frames={} rworst={} rslow20={}",
            a.render_frames,
            worst_field(&a.worst),
            a.slow_render
        );
        if a.enc_frames > 0 {
            let cams: Vec<String> = a
                .enc
                .iter()
                .map(|(&(pos, three, order), &(runs, seg))| {
                    let ms: Vec<String> =
                        seg.iter().map(|&v| format!("{:.2}", per(v, a.enc_frames))).collect();
                    let kind = if three == 1 { "3d" } else { "2d" };
                    format!("{pos}:{kind}:{order}:{runs}:{}", ms.join("/"))
                })
                .collect();
            s += &format!(
                " enc_tail={:.2} enc_cbs={:.1} enc={}",
                per(a.enc_tail, a.enc_frames),
                a.enc_buffers as f64 / f64::from(a.enc_frames),
                cams.join(",")
            );
        }
        if a.t_frames > 0 {
            let names = ["t_draws", "t_pipe_sw", "t_items", "t_mat_sw", "t_pipes", "t_mats"];
            for (name, sum) in names.iter().zip(a.t_counts) {
                s += &format!(" {name}={:.0}", sum as f64 / f64::from(a.t_frames));
            }
        }
        let [clean, dirty] = a.upload_split;
        let w = a.worst_uploads;
        s += &format!(
            " rworst_up={}/{:.0}/{:.0} r_meshes_up={:.2} r_prepare_up={:.2} up_frames={} r_meshes_noup={:.2} r_prepare_noup={:.2} up_max_kb={:.0} stream_max_kb={:.0}",
            w[0],
            w[1] as f64 / 1024.,
            w[2] as f64 / 1024.,
            per(dirty.1, dirty.0),
            per(dirty.2, dirty.0),
            dirty.0,
            per(clean.1, clean.0),
            per(clean.2, clean.0),
            a.upload_max as f64 / 1024.,
            a.stream_max as f64 / 1024.,
        );
        for i in 0..lap::N {
            s += &format!(
                " lap_{}={:.2}/{:.1}/{}",
                lap::NAMES[i],
                per(a.laps[i], a.main_frames),
                a.lap_max[i] as f64 * 1e-6,
                a.lap_over[i]
            );
        }
        s += &usage_fields();
        let ms = |v: &[u64; lap::N], n: u32| -> String {
            v.iter().map(|&x| format!("{:.1}", per(x, n))).collect::<Vec<_>>().join("/")
        };
        s += &format!(
            " rworst_laps={} rslow_laps={}:{}",
            ms(&a.worst_laps, 1),
            a.rslow_n,
            ms(&a.rslow_laps, a.rslow_n)
        );
        if a.gpu_frames > 0 {
            let n = f64::from(a.gpu_frames);
            let total: f64 = a.gpu.iter().sum();
            s += &format!(" gpu=on gpu_ms={:.2}", total / n);
            for (name, sum) in SEGMENTS.iter().zip(a.gpu) {
                s += &format!(" {name}={:.2}", sum / n);
            }
            s += &format!(
                " g_gap={:.2} g_frames={} g_bad={} g_lost={}",
                (a.gpu_span - total) / n,
                a.gpu_frames,
                a.gpu_bad,
                a.gpu_lost
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
            " vis={:.0} casters={:.0} views3d={:.1} mesh_up_s={:.1} mesh_kb_s={:.0} mesh_stream_kb_s={:.0} mesh_stream_writes={:.1} img_up_s={:.1} img_kb_s={:.0}",
            a.visible as f64 / m,
            a.casters as f64 / m,
            a.views3d as f64 / m,
            a.mesh_up as f64 / secs,
            a.mesh_bytes as f64 / 1024. / secs,
            a.stream_bytes as f64 / 1024. / secs,
            a.stream_writes as f64 / m,
            a.image_up as f64 / secs,
            a.image_bytes as f64 / 1024. / secs,
        );
        for (slow, limit) in a.slow.iter().zip(SLOW_MS) {
            s += &slow.report(&format!("slow{limit}"));
        }
        for (t, ticked) in a.ticked.iter().enumerate() {
            s += &ticked.report(&format!("tick{t}"));
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
    use bevy::camera::visibility::VisibilitySystems;
    use bevy::transform::TransformSystems;
    app.add_systems(First, main_begin)
        .add_systems(FixedFirst, fixed_tick)
        .add_systems(Last, main_end)
        .add_systems(
            PostUpdate,
            (
                lap_begin::<{ lap::PROPAGATE }>.before(TransformSystems::Propagate),
                (lap_end::<{ lap::PROPAGATE }>, transform_burst)
                    .chain()
                    .after(TransformSystems::Propagate),
                lap_begin::<{ lap::BOUNDS }>.before(VisibilitySystems::CalculateBounds),
                lap_end::<{ lap::BOUNDS }>.after(VisibilitySystems::CalculateBounds),
            ),
        )
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
            (count_transparent, dump_transparent, point::<{ P::Graph as usize }>)
                .chain()
                .after(S::Prepare)
                .before(S::Render),
            point::<{ P::Clean as usize }>.after(S::Render).before(S::Cleanup),
            point::<{ P::End as usize }>.after(S::PostCleanup),
        ),
    );
    render.add_systems(
        RenderGraph,
        (
            (point::<{ P::GraphSubmit as usize }>, count_buffers)
                .chain()
                .after(RenderGraphSystems::Render)
                .after(gpu_resolve)
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
            a.slow_render = 1;
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
        assert!(s.contains(" rworst=7.0/0.0/0.0/0.0/0.0/0.0/5.5/0.0/0.0/0.0/0.0/0.0 rslow20=1 "), "{s}");
        assert!(s.contains(" gpu=on gpu_ms=15.00 g_pre=1.00 g_shadow=2.00"), "{s}");
        assert!(s.contains(" g_gap=1.00 g_frames=1 g_bad=0"), "{s}");
        assert!(s.contains(" mesh_up_s=85.0 "), "{s}");
        assert!(lines.is_empty());
        let (s, _) = window_report(1.);
        assert!(s.contains(" main_ms=0.00 ") && s.contains(" mesh_up_s=0.0 "), "{s}");
        assert!(s.contains(" rworst=0.0/") && s.contains(" rslow20=0 "), "{s}");
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

    /// F930, the Mac crash of F929: a slot whose map timed out, or failed, is
    /// reused when the ring comes round. Its copy must not land in a buffer
    /// that is still mapped (a validation error that drops the frame's
    /// command buffers) and its next map must not hit wgpu's "Buffer is
    /// already mapped" panic. The device lives in a child process
    /// (`gpu_test_process`, F922).
    #[test]
    fn readback_slot_is_reused_after_a_timed_out_or_failed_map() {
        crate::gpu_test_process::in_own_process(check_readback_reuse);
    }

    fn check_readback_reuse() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) =
            bevy::tasks::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        else {
            println!("no GPU adapter, skipped");
            return;
        };
        let (device, queue) =
            bevy::tasks::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device");
        let errors = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = errors.clone();
        device.on_uncaptured_error(Arc::new(move |e| sink.lock().unwrap().push(e.to_string())));
        // Occlusion needs no device feature; the ring never looks at the type.
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("frame_split_test"),
            ty: wgpu::QueryType::Occlusion,
            count: SLOT_QUERIES * RING as u32,
        });
        let mut g = GpuInner::new(&device, set);
        let source = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame_split_test_source"),
            size: 64,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ticks = [1000_u64, 1001, 1002];
        queue.write_buffer(&source, 0, &ticks.iter().flat_map(|t| t.to_le_bytes()).collect::<Vec<u8>>());
        // One render frame: open, record two marks, copy, submit, map.
        let frame = |g: &mut GpuInner| -> Option<usize> {
            g.open(true);
            let i = g.current?;
            g.slots[i].marks.extend([m(Kind::FrameBegin, 0); 2]);
            let (readback, _, used) = g.close()?;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            encoder.copy_buffer_to_buffer(&source, 0, &readback, 0, u64::from(used) * 8);
            queue.submit([encoder.finish()]);
            g.start_map();
            Some(i)
        };
        // Every slot records, then the GPU finishes and all three are read.
        let check_round = |g: &mut GpuInner, what: &str| {
            let recorded: Vec<usize> = (0..RING).filter_map(|_| frame(g)).collect();
            assert_eq!(recorded.len(), RING, "{what}: every slot records again: {recorded:?}");
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            let (read, lost) = g.collect(&device);
            assert_eq!(lost, 0, "{what}");
            assert_eq!(read.len(), RING, "{what}: every slot is read");
            for (marks, got) in read {
                assert_eq!(marks.len(), 2, "{what}");
                assert_eq!(got, ticks[..2], "{what}: the copied ticks come back");
            }
            assert!(errors.lock().unwrap().is_empty(), "{what}: {:?}", errors.lock().unwrap());
        };
        check_round(&mut g, "start");

        // Timeout: a map that never completes (nothing polls the device).
        let timed_out = frame(&mut g).expect("a free slot");
        for _ in 0..MAP_TIMEOUT_FRAMES {
            assert_eq!(g.collect(&device).1, 0, "gives up only after the timeout");
        }
        assert!(g.slots[timed_out].map_outstanding);
        assert_eq!(g.collect(&device).1, 1, "the timed-out slot is given up on");
        assert!(!g.slots[timed_out].map_outstanding && g.slots[timed_out].state == SlotState::Free);
        check_round(&mut g, "after a timed-out map");

        // Failure: the map callback reports an error.
        let failed = frame(&mut g).expect("a free slot");
        g.slots[failed].mapped.store(2, Ordering::Release);
        assert_eq!(g.collect(&device).1, 1, "the failed slot is given up on");
        check_round(&mut g, "after a failed map");

        // While a slot's map is outstanding, its frame records no timings.
        let busy = frame(&mut g).expect("a free slot");
        let (mut opened, mut skipped) = (0, 0);
        for _ in 0..RING {
            g.open(true);
            match g.current {
                Some(i) => {
                    assert_ne!(i, busy);
                    opened += 1;
                    g.close();
                }
                None => skipped += 1,
            }
        }
        assert_eq!((opened, skipped), (RING - 1, 1));
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        assert_eq!(g.collect(&device).0.len(), 1);
        assert!(errors.lock().unwrap().is_empty(), "{:?}", errors.lock().unwrap());
        println!("readback ring: timed-out and failed slots reused without errors");
    }
}
