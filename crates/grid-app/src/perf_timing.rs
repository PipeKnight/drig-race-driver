//! `GRID_PERF_TIMING=1`: wall-clock time of the main and render schedules and
//! the simulation stages, printed every 600 frames and at exit
//! (docs/PERFORMANCE.md). Nothing is added without the variable.
use bevy::prelude::*;
use bevy::render::{Render, RenderApp, RenderSystems};
use grid_tools::perf::{self, Stage};
use std::sync::Mutex;
use std::time::Instant;

static MAIN_STARTED: Mutex<Option<Instant>> = Mutex::new(None);
static RENDER_STARTED: Mutex<Option<Instant>> = Mutex::new(None);
const REPORT_FRAMES: u32 = 600;

pub fn install(app: &mut App) {
    // `GRID_CPU_CLUSTERING=1`: Bevy's CPU light clustering, which Android
    // always uses, instead of the desktop's GPU clustering (rendering only).
    if std::env::var_os("GRID_CPU_CLUSTERING").is_some() {
        app.add_systems(
            Startup,
            |mut settings: ResMut<bevy::light::cluster::GlobalClusterSettings>| {
                settings.gpu_clustering = None;
                println!("perf_clustering mode=cpu");
            },
        );
    }
    if !perf::enabled() {
        return;
    }
    app.add_systems(First, || start(&MAIN_STARTED))
        .add_systems(Last, main_end)
        .add_systems(PostUpdate, rebounded);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .add_systems(
                Render,
                (|| start(&RENDER_STARTED)).before(RenderSystems::ExtractCommands),
            )
            .add_systems(
                Render,
                (|| stop(&RENDER_STARTED, Stage::RenderSchedule)).after(RenderSystems::Cleanup),
            );
    }
}

fn start(started: &Mutex<Option<Instant>>) {
    *started.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

fn stop(started: &Mutex<Option<Instant>>, stage: Stage) {
    if let Some(started) = started.lock().unwrap_or_else(|e| e.into_inner()).take() {
        perf::add(stage, started.elapsed().as_nanos() as u64);
    }
}

/// The entities whose `Aabb` Bevy's `calculate_bounds` recomputes (mesh asset
/// or handle changed, frustum culled), by name: per frame the mean count and
/// vertices, printed every 600 frames (the most vertices first).
#[allow(clippy::type_complexity)]
fn rebounded(
    query: Query<
        (Option<&Name>, &Mesh3d),
        (
            Or<(bevy::asset::prelude::AssetChanged<Mesh3d>, Changed<Mesh3d>)>,
            With<bevy::camera::primitives::Aabb>,
            Without<bevy::camera::visibility::NoFrustumCulling>,
            Without<bevy::camera::visibility::NoAutoAabb>,
        ),
    >,
    meshes: Res<Assets<Mesh>>,
    mut totals: Local<std::collections::HashMap<String, (u64, u64)>>,
    mut frames: Local<u32>,
) {
    for (name, mesh) in &query {
        let key = name.map_or("unnamed", Name::as_str);
        let vertices = meshes.get(mesh).map_or(0, |m| m.count_vertices() as u64);
        let entry = totals.entry(key.to_owned()).or_default();
        entry.0 += 1;
        entry.1 += vertices;
    }
    *frames += 1;
    if (*frames).is_multiple_of(REPORT_FRAMES) {
        let mut rows: Vec<_> = totals.drain().collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.1.1));
        let n = f64::from(REPORT_FRAMES);
        let mut line = format!(
            "perf_rebounded frames={} entities={:.1} vertices={:.0}",
            *frames,
            rows.iter().map(|r| r.1.0).sum::<u64>() as f64 / n,
            rows.iter().map(|r| r.1.1).sum::<u64>() as f64 / n
        );
        for (name, (count, vertices)) in rows.iter().take(10) {
            line += &format!(
                " {:?}={:.1}x{:.0}",
                name,
                *count as f64 / n,
                *vertices as f64 / n
            );
        }
        println!("{line}");
    }
}

fn main_end(mut frames: Local<u32>, mut exits: MessageReader<AppExit>) {
    stop(&MAIN_STARTED, Stage::MainSchedule);
    *frames += 1;
    if (*frames).is_multiple_of(REPORT_FRAMES) || exits.read().next().is_some() {
        println!("{}{} frames={}", perf::report(), crate::fixed_ticks::report(), *frames);
    }
}
