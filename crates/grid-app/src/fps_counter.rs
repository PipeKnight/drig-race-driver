//! On-screen frame-rate counter (`[osd] fps`, docs/SETTINGS.md): frames per
//! second, the mean frame time and the worst frame of the last second, plus
//! one `frame_stats` log line per second (on Android also in the Download log
//! file), so a phone run gives numbers for P12. Independently authored
//! diagnostics; GRID has no such display. It only reads real time and the
//! simulation tick counter, so the simulation is unaffected.
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryWindow};

/// Frame times of the current one-second window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Window {
    frames: u32,
    elapsed: f64,
    worst: f64,
}

/// Summary of one finished window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub fps: f64,
    pub mean_ms: f64,
    pub worst_ms: f64,
    pub frames: u32,
    pub seconds: f64,
}

impl Window {
    /// Adds one frame of `dt` seconds; returns the summary when at least one
    /// second has been collected, and starts a new window.
    pub fn push(&mut self, dt: f64) -> Option<Stats> {
        if !dt.is_finite() || dt <= 0. {
            return None;
        }
        self.frames += 1;
        self.elapsed += dt;
        self.worst = self.worst.max(dt);
        (self.elapsed >= 1.).then(|| {
            let stats = Stats {
                fps: f64::from(self.frames) / self.elapsed,
                mean_ms: self.elapsed * 1000. / f64::from(self.frames),
                worst_ms: self.worst * 1000.,
                frames: self.frames,
                seconds: self.elapsed,
            };
            *self = Self::default();
            stats
        })
    }
}

impl Stats {
    pub fn label(&self) -> String {
        format!(
            "{:.0} FPS  {:.1} ms  max {:.1}",
            self.fps, self.mean_ms, self.worst_ms
        )
    }
}

#[derive(Resource, Debug)]
pub struct FpsCounter {
    pub shown: bool,
    window: Window,
    last_ticks: Option<u64>,
    /// Window size of the last `frame_setup` line: logged again on a change,
    /// since Android sizes the window after the first frames (a phone log
    /// showed the requested 1280x800 first).
    reported_setup: Option<UVec2>,
}

#[derive(Component)]
struct FpsText;

fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<crate::ViewerCamera>>,
    mut done: Local<bool>,
) {
    let Some(camera) = camera.iter().next().filter(|_| !*done) else {
        return;
    };
    *done = true;
    // Right edge under the original-look HUD's race position (F383) and
    // above its dial; the top centre holds the Drift Battle score panel and
    // the P11 II / CENTRE buttons (a phone render showed the overlap).
    commands.spawn((
        FpsText,
        Node {
            position_type: PositionType::Absolute,
            right: percent(1.),
            top: percent(12.),
            padding: UiRect::axes(px(6.), px(2.)),
            ..default()
        },
        BackgroundColor(Color::srgba(0., 0., 0., 0.45)),
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(24.),
            ..default()
        },
        TextColor(Color::srgb(0.75, 1., 0.75)),
        UiTargetCamera(camera),
    ));
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut counter: ResMut<FpsCounter>,
    time: Res<Time<Real>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    driving: Option<Res<crate::Driving>>,
    windows: Query<&bevy::window::Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    scale: Option<Res<crate::render_scale::RenderScale>>,
    mut text: Query<(&mut Text, &mut Visibility), With<FpsText>>,
) {
    // F9 toggles the display (desktop); the log line follows it.
    if keys.is_some_and(|k| k.just_pressed(KeyCode::F9)) {
        counter.shown = !counter.shown;
        counter.window = Window::default();
    }
    for (_, mut visibility) in &mut text {
        visibility.set_if_neq(if counter.shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if !counter.shown {
        counter.last_ticks = None;
        return;
    }
    if let Some(w) = windows.iter().next()
        && counter.reported_setup != Some(UVec2::new(w.physical_width(), w.physical_height()))
    {
        counter.reported_setup = Some(UVec2::new(w.physical_width(), w.physical_height()));
        let refresh: Vec<String> = monitors
            .iter()
            .map(|m| {
                m.refresh_rate_millihertz
                    .map_or("unknown".into(), |r| format!("{:.2}", f64::from(r) / 1000.))
            })
            .collect();
        // winit reports no refresh rate on Android; ask the Display (JNI).
        #[cfg(target_os = "android")]
        let display = format!(" display_hz={:?}", crate::android::display_refresh_hz());
        #[cfg(not(target_os = "android"))]
        let display = String::new();
        println!(
            "frame_setup present_mode={:?} frame_latency={:?} physical={}x{} logical={}x{} scale={} monitor_hz=[{}]{display} update_mode=continuous frame_cap=none",
            w.present_mode,
            w.desired_maximum_frame_latency,
            w.physical_width(),
            w.physical_height(),
            w.width(),
            w.height(),
            w.scale_factor(),
            refresh.join(",")
        );
    }
    let Some(stats) = counter.window.push(time.delta_secs_f64()) else {
        return;
    };
    let ticks = driving.map(|d| d.runtime.ticks);
    let rate = match (counter.last_ticks, ticks) {
        (Some(a), Some(b)) if b >= a => format!("{:.1}", (b - a) as f64 / stats.seconds),
        _ => "-".into(),
    };
    counter.last_ticks = ticks;
    // Render size of the 3D view against the window (render_scale.rs).
    let sizes = scale
        .as_ref()
        .filter(|s| s.render != UVec2::ZERO)
        .map(|s| (s.render, s.screen));
    let res = sizes.map_or(String::new(), |(r, s)| {
        format!(" render={}x{} screen={}x{}", r.x, r.y, s.x, s.y)
    });
    println!(
        "frame_stats fps={:.1} avg_ms={:.2} max_ms={:.2} frames={} sim_ticks_per_s={rate}{res}{}",
        stats.fps,
        stats.mean_ms,
        stats.worst_ms,
        stats.frames,
        crate::perf_test::stats_suffix()
    );
    for (mut t, _) in &mut text {
        t.0 = match sizes {
            Some((r, s)) => format!("{}\n{}x{} of {}x{}", stats.label(), r.x, r.y, s.x, s.y),
            None => stats.label(),
        };
    }
}

pub fn install(app: &mut App, shown: bool) {
    app.insert_resource(FpsCounter {
        shown,
        window: Window::default(),
        last_ticks: None,
        reported_setup: None,
    })
    .add_systems(Update, (spawn, update).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_second_windows_give_fps_mean_and_worst() {
        let mut w = Window::default();
        for _ in 0..29 {
            assert_eq!(w.push(1. / 30.), None);
        }
        // The 30th frame is a 100 ms hitch: the window closes after it.
        let s = w.push(0.1).unwrap();
        assert_eq!(s.frames, 30);
        let elapsed = 29. / 30. + 0.1;
        assert!((s.fps - 30. / elapsed).abs() < 1e-9);
        assert!((s.mean_ms - elapsed * 1000. / 30.).abs() < 1e-9);
        assert!((s.worst_ms - 100.).abs() < 1e-9);
        assert_eq!(
            s.label(),
            format!(
                "{:.0} FPS  {:.1} ms  max 100.0",
                30. / elapsed,
                elapsed * 1000. / 30.
            )
        );
        // A fresh window starts.
        assert_eq!(w, Window::default());
    }

    #[test]
    fn zero_and_invalid_frames_are_ignored() {
        let mut w = Window::default();
        assert_eq!(w.push(0.), None);
        assert_eq!(w.push(f64::NAN), None);
        assert_eq!(w.push(-1.), None);
        assert_eq!(w, Window::default());
        let s = w.push(1.5).unwrap();
        assert_eq!((s.frames, s.fps), (1, 1. / 1.5));
    }
}
