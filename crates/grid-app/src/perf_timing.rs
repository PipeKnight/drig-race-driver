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
    if !perf::enabled() {
        return;
    }
    app.add_systems(First, || start(&MAIN_STARTED))
        .add_systems(Last, main_end);
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

fn main_end(mut frames: Local<u32>, mut exits: MessageReader<AppExit>) {
    stop(&MAIN_STARTED, Stage::MainSchedule);
    *frames += 1;
    if (*frames).is_multiple_of(REPORT_FRAMES) || exits.read().next().is_some() {
        println!("{} frames={}", perf::report(), *frames);
    }
}
