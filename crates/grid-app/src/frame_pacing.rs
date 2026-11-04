//! Frame pacing research trace (backlog row 116, docs/FRAME_STEPPING.md
//! section 6). `[diagnostics] present_trace = on` (`GRID_PRESENT_TRACE=1`;
//! on a phone through its settings file) prints, for each drawn frame, what the
//! main world drew (`present_frame`: the frame number, its start time, the
//! frame time, the tick, `alpha`, the simulation time drawn, the view and the
//! drawn car and camera) and, from the render world after the frame was
//! handed to the swap chain, `present_done` with the same frame number. The
//! render world presents one frame after the main world drew it (pipelined
//! rendering); in a GPU-bound window the swap chain's acquire holds it back,
//! so the `present_done` times follow the GPU's pace.
//!
//! `GRID_AUDIO=off` opens no audio device in a window (silent research runs;
//! offscreen captures never open one).
use bevy::prelude::*;
use bevy::render::{
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    Render, RenderApp, RenderSystems,
};
use std::sync::OnceLock;
use std::time::Instant;

fn trace_on() -> bool {
    crate::settings::diagnostics().present_trace
}

pub(crate) fn audio_off() -> bool {
    std::env::var("GRID_AUDIO").is_ok_and(|v| v == "off")
}

/// Milliseconds since the first call (shared by both worlds).
fn now_ms() -> f64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3
}

/// The main world's frame number, carried to the render world.
#[derive(Resource, Clone, Default, ExtractResource)]
pub(crate) struct Drawn {
    frame: u64,
}

pub(crate) fn install(app: &mut App) {
    if !trace_on() {
        return;
    }
    now_ms();
    app.init_resource::<Drawn>()
        .add_plugins(ExtractResourcePlugin::<Drawn>::default());
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(Render, presented.in_set(RenderSystems::Cleanup));
    }
}

/// What one frame drew.
pub(crate) struct Frame<'a> {
    pub frame_ms: f64,
    pub ticks: u64,
    pub alpha: f32,
    pub view: &'a str,
    pub car: Option<Mat4>,
    pub eye: Vec3,
    pub rotation: Quat,
}

/// One `present_frame` line (main world, after the camera shot).
pub(crate) fn drawn(drawn: Option<&mut Drawn>, frame: Frame) {
    let Some(drawn) = drawn else {
        return;
    };
    drawn.frame += 1;
    let (car, car_rotation) = frame.car.map_or((Vec3::NAN, Quat::NAN), |p| {
        let (_, r, t) = p.to_scale_rotation_translation();
        (t, r)
    });
    // The simulation time drawn: between the last two ticks by alpha.
    let shown_ms = (frame.ticks as f64 - 1. + f64::from(frame.alpha))
        * f64::from(grid_tools::source_vehicle::DELTA)
        * 1e3;
    let (e, q, c, cq) = (frame.eye, frame.rotation, car, car_rotation);
    println!(
        "present_frame frame={} t_ms={:.3} dt_ms={:.3} tick={} alpha={:.3} shown_ms={shown_ms:.3} view={} car={:.3},{:.3},{:.3} car_q={:.5},{:.5},{:.5},{:.5} eye={:.3},{:.3},{:.3} eye_q={:.5},{:.5},{:.5},{:.5}",
        drawn.frame,
        now_ms(),
        frame.frame_ms,
        frame.ticks,
        frame.alpha,
        frame.view,
        c.x, c.y, c.z,
        cq.x, cq.y, cq.z, cq.w,
        e.x, e.y, e.z,
        q.x, q.y, q.z, q.w,
    );
}

/// After the render world presented the frame it extracted.
fn presented(drawn: Option<Res<Drawn>>) {
    if let Some(drawn) = drawn {
        println!("present_done frame={} t_ms={:.3}", drawn.frame, now_ms());
    }
}
