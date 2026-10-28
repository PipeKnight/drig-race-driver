//! Per-pass GPU time (P12, docs/PERFORMANCE.md "Grid start: GPU passes"), on
//! in the profile build (feature `profile`) and with `GRID_GPU_PASSES=1`:
//! Bevy's render diagnostics
//! put timestamp queries (and, where the device has them, pipeline
//! statistics) around each render pass, and every
//! [`window_seconds`] the app logs the mean per frame:
//!
//! ```text
//! gpu_window version=1 seconds=10.0 frames=283 timestamps=true
//! gpu_pass rank=1 gpu_ms=6.10 primitives=812345 vertices=1201234 fragments=2101234 name=main_opaque_pass_3d
//! ```
//!
//! Bevy times the main passes but not the shadow passes, so each window also
//! logs the triangles each view submits (the camera and every shadow
//! cascade), by owner, from the visibility lists and the meshes' GPU sizes:
//!
//! ```text
//! view_triangles view=camera total=812345 opponent=401234 car_batch=... player=... batch=... track=... other=...
//! ```
//!
//! On a tiled GPU (the phones) a pass's timestamps bracket its whole tile
//! work, which is what the frame waits for. Independently authored
//! diagnostics; only GPU counters and visibility lists are read.
use bevy::camera::visibility::{CascadesVisibleEntities, VisibleEntities};
use bevy::diagnostic::DiagnosticsStore;
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::render::mesh::{RenderMesh, RenderMeshBufferInfo};
use bevy::render::render_asset::RenderAssets;
use bevy::render::{Render, RenderApp, RenderSystems};
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

/// Triangles per mesh asset, copied from the render world.
static TRIANGLES: Mutex<Option<HashMap<AssetId<Mesh>, u32>>> = Mutex::new(None);

/// Per pass: GPU ms, primitives out of the clipper, vertex and fragment
/// shader invocations; sums and frames counted.
#[derive(Default)]
struct Pass {
    sums: [f64; 4],
    frames: [u32; 4],
}

const FIELDS: [&str; 4] = [
    "elapsed_gpu",
    "clipper_primitives_out",
    "vertex_shader_invocations",
    "fragment_shader_invocations",
];

/// On in the profile build, or with `GRID_GPU_PASSES=1`.
pub fn enabled() -> bool {
    cfg!(feature = "profile") || std::env::var("GRID_GPU_PASSES").is_ok_and(|v| v == "1")
}

/// Seconds per logged table: `GRID_PROFILE_SECONDS`, default 10 (as the
/// profile build's span tables).
fn window_seconds() -> f32 {
    static SECONDS: OnceLock<f32> = OnceLock::new();
    *SECONDS.get_or_init(|| {
        std::env::var("GRID_PROFILE_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&s: &f32| s >= 1.)
            .unwrap_or(10.)
    })
}

pub fn install(app: &mut App) {
    app.add_plugins(RenderDiagnosticsPlugin)
        .add_systems(Last, (report, view_triangles));
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(Render, mesh_triangles.after(RenderSystems::Cleanup));
    }
}

/// Copies the meshes' triangle counts when the main world asks (no map).
fn mesh_triangles(meshes: Res<RenderAssets<RenderMesh>>) {
    let mut map = TRIANGLES.lock().unwrap_or_else(|e| e.into_inner());
    if map.is_some() {
        return;
    }
    *map = Some(
        meshes
            .iter()
            .map(|(id, mesh)| {
                let corners = match mesh.buffer_info {
                    RenderMeshBufferInfo::Indexed { count, .. } => count,
                    RenderMeshBufferInfo::NonIndexed => mesh.vertex_count,
                };
                (id, corners / 3)
            })
            .collect(),
    );
}

type Owner = (
    &'static Mesh3d,
    Has<crate::car_batch::Batch>,
    Has<crate::OpponentPart>,
    Has<crate::MovingPart>,
    Has<crate::static_batch::Batch>,
    Has<crate::track_surface::SourcePart>,
);
const OWNERS: [&str; 6] = ["opponent", "car_batch", "player", "batch", "track", "other"];

/// Per window: triangles each view submits, by owner.
fn view_triangles(
    time: Res<Time<Real>>,
    mut elapsed: Local<f32>,
    cameras: Query<(Entity, &Camera, &VisibleEntities), With<Camera3d>>,
    lights: Query<&CascadesVisibleEntities>,
    owners: Query<Owner>,
) {
    *elapsed += time.delta_secs();
    if *elapsed < window_seconds() {
        return;
    }
    *elapsed = 0.;
    let Some(map) = TRIANGLES.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    let count = |entities: &[Entity]| {
        let mut sums = [0u64; 6];
        for entity in entities {
            let Ok((mesh, car_batch, opponent, player, batch, track)) = owners.get(*entity) else {
                continue;
            };
            let owner = if car_batch {
                1
            } else if opponent {
                0
            } else if player {
                2
            } else if batch {
                3
            } else if track {
                4
            } else {
                5
            };
            sums[owner] += u64::from(map.get(&mesh.id()).copied().unwrap_or(0));
        }
        sums
    };
    let line = |view: String, sums: [u64; 6]| {
        let fields: Vec<String> = OWNERS.iter().zip(sums).map(|(n, v)| format!("{n}={v}")).collect();
        log_line!(
            "view_triangles view={view} total={} {}",
            sums.iter().sum::<u64>(),
            fields.join(" ")
        );
    };
    for (entity, camera, visible) in &cameras {
        if !camera.is_active {
            continue;
        }
        line(format!("camera{}", camera.order), count(visible.get(TypeId::of::<Mesh3d>())));
        for light in &lights {
            if let Some(cascades) = light.entities.get(&entity) {
                for (i, cascade) in cascades.iter().enumerate() {
                    line(format!("camera{}_cascade{i}", camera.order), count(&cascade.entities));
                }
            }
        }
    }
}

fn report(
    time: Res<Time<Real>>,
    store: Res<DiagnosticsStore>,
    mut elapsed: Local<f32>,
    mut frames: Local<u32>,
    mut passes: Local<BTreeMap<String, Pass>>,
    mut seen: Local<BTreeMap<String, std::time::Instant>>,
) {
    for diagnostic in store.iter() {
        let path = diagnostic.path().as_str();
        let Some(rest) = path.strip_prefix("render/") else {
            continue;
        };
        let Some(field) = FIELDS.iter().position(|f| rest.ends_with(f)) else {
            continue;
        };
        let Some(m) = diagnostic.measurement() else {
            continue;
        };
        // One measurement per rendered frame; count each once.
        if seen.get(path) == Some(&m.time) {
            continue;
        }
        seen.insert(path.to_string(), m.time);
        let name = rest[..rest.len() - FIELDS[field].len()].trim_end_matches('/');
        let pass = passes.entry(name.to_string()).or_default();
        pass.sums[field] += m.value;
        pass.frames[field] += 1;
    }
    *frames += 1;
    *elapsed += time.delta_secs();
    if *elapsed < window_seconds() {
        return;
    }
    let seconds = std::mem::take(&mut *elapsed);
    let frames = std::mem::take(&mut *frames);
    let passes = std::mem::take(&mut *passes);
    let mean = |p: &Pass, f: usize| p.sums[f] / f64::from(p.frames[f].max(1));
    let timed = passes.values().any(|p| p.frames[0] > 0);
    log_line!(
        "gpu_window version=1 seconds={seconds:.1} frames={frames} timestamps={timed}{}",
        crate::perf_test::stats_suffix()
    );
    let mut ranked: Vec<_> = passes.iter().collect();
    ranked.sort_by(|a, b| mean(b.1, 0).total_cmp(&mean(a.1, 0)));
    for (rank, (name, p)) in ranked.iter().enumerate() {
        log_line!(
            "gpu_pass rank={} gpu_ms={:.3} primitives={:.0} vertices={:.0} fragments={:.0} name={name}",
            rank + 1,
            mean(p, 0),
            mean(p, 1),
            mean(p, 2),
            mean(p, 3)
        );
    }
}
