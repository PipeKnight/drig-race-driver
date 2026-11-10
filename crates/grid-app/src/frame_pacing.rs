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
//! offscreen captures never open one). `[diagnostics] window_shots = S,S,...`
//! (`GRID_WINDOW_SHOTS`) saves what the window shows, the HUD included, at
//! those seconds of real time to `<data dir>/window-shot-S.png`.
//! `[diagnostics] window_frames = START,SECONDS` (`GRID_WINDOW_FRAMES`)
//! captures every frame the window shows over that stretch of real time and
//! appends it, box-averaged by [`FRAME_SCALE`], to
//! `<data dir>/window-frames.rgb` for tools/blink_detect.py (row 115): per
//! frame a 16-byte header (u32 frame, f32 real seconds, u16 width, u16
//! height, u32 full width) and the RGB bytes. Each capture also prints
//! `window_frame frame=N t_s=S` with Bevy's `FrameCount`, the number the
//! other per-frame traces (`shadow_dropout`, shadow_cull.rs) use.
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
    install_window_shots(app);
    if !crate::settings::diagnostics().window_frames.is_empty() {
        app.add_systems(Last, window_frames.before(bevy::diagnostic::update_frame_count));
    }
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

/// `window_shots` alone, for the front-end screens (event menu, career).
pub(crate) fn install_window_shots(app: &mut App) {
    install_window_shots_as(app, "");
}

/// `window_shots` of a screen whose shots must not replace another
/// screen's of the same time: `window-shot-<screen>-<t>.png`.
pub(crate) fn install_window_shots_as(app: &mut App, screen: &'static str) {
    if !crate::settings::diagnostics().window_shots.is_empty() {
        app.add_systems(Last, move |commands: Commands, time: Res<Time<Real>>, next: Local<usize>| {
            window_shots(screen, commands, time, next)
        });
    }
}

fn window_shots(screen: &str, mut commands: Commands, time: Res<Time<Real>>, mut next: Local<usize>) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    while let Some(&at) = crate::settings::diagnostics().window_shots.get(*next)
        && time.elapsed_secs() >= at
    {
        let name = if screen.is_empty() { String::new() } else { format!("{screen}-") };
        let path = crate::settings::data_dir().join(format!("window-shot-{name}{at:05.1}.png"));
        println!("window_shot at_s={at} path={}", path.display());
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *next += 1;
    }
}

/// The box size `window_frames` averages each captured frame over.
pub(crate) const FRAME_SCALE: u32 = 8;

fn window_frames(mut commands: Commands, time: Res<Time<Real>>, frame: Res<bevy::diagnostic::FrameCount>) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
    let &[start, seconds] = &crate::settings::diagnostics().window_frames[..] else {
        return;
    };
    let (n, t) = (frame.0, time.elapsed_secs());
    if t < start || t > start + seconds {
        return;
    }
    println!("window_frame frame={n} t_s={t:.4}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(move |shot: On<ScreenshotCaptured>| append_frame(n, t, &shot.image));
}

/// One frame of `window-frames.rgb` (the file is replaced by the first).
fn append_frame(frame: u32, t: f32, image: &Image) {
    use std::io::Write;
    use std::sync::Mutex;
    static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
    let Ok(rgb) = image.clone().try_into_dynamic().map(|i| i.to_rgb8()) else {
        return;
    };
    let block = reduce_frame(frame, t, rgb.width(), rgb.height(), rgb.as_raw());
    let mut file = FILE.lock().unwrap_or_else(|e| e.into_inner());
    if file.is_none() {
        *file = std::fs::File::create(crate::settings::data_dir().join("window-frames.rgb")).ok();
    }
    if let Some(f) = file.as_mut() {
        let _ = f.write_all(&block);
    }
}

/// The header and the frame box-averaged by [`FRAME_SCALE`].
fn reduce_frame(frame: u32, t: f32, width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let (w, h) = (width / FRAME_SCALE, height / FRAME_SCALE);
    let mut out = Vec::with_capacity(16 + (w * h * 3) as usize);
    out.extend_from_slice(&frame.to_le_bytes());
    out.extend_from_slice(&t.to_le_bytes());
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 3];
            for dy in 0..FRAME_SCALE {
                let row = ((y * FRAME_SCALE + dy) * width + x * FRAME_SCALE) as usize * 3;
                for px in rgb[row..row + FRAME_SCALE as usize * 3].as_chunks::<3>().0 {
                    for (s, &v) in sum.iter_mut().zip(px) {
                        *s += u32::from(v);
                    }
                }
            }
            out.extend(sum.map(|s| (s / (FRAME_SCALE * FRAME_SCALE)) as u8));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reduced_frame_has_the_header_and_box_averages() {
        let (width, height) = (2 * FRAME_SCALE + 3, FRAME_SCALE + 1);
        let rgb: Vec<u8> = (0..width * height)
            .flat_map(|i| {
                let x = i % width;
                if x < FRAME_SCALE { [10, 20, 30] } else { [200, 100, 0] }
            })
            .collect();
        let block = reduce_frame(7, 1.5, width, height, &rgb);
        assert_eq!(&block[..4], &7u32.to_le_bytes());
        assert_eq!(&block[4..8], &1.5f32.to_le_bytes());
        assert_eq!(&block[8..12], &[2, 0, 1, 0]);
        assert_eq!(&block[12..16], &width.to_le_bytes());
        assert_eq!(&block[16..], &[10, 20, 30, 200, 100, 0]);
    }
}
