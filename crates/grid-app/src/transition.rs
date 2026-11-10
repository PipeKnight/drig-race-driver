//! Screen transitions, the loading overlay and the hand-off between the
//! front-end screens and the race (docs/SCREEN_FLOW.md).
//!
//! The original front end draws one full-screen black layer, the
//! `overlay_fade` glyph, over every screen. Its alpha follows animation
//! `Anim!169` (BEZIERFLOAT, keys at frame 0 = 0 and frame 0.6667 = 1 with
//! the control points at a third and two thirds: smoothstep).
//! `fade_to_black` plays it to frame 0.8333 and `fade_to_clear` to frame 0,
//! in 1.0 s at normal speed and 0.5 s at quick and quickest speed
//! (FUN_004C6DB0, `states.xml`). Once the screen is black on the way into a
//! load, the overlay shows "Loading, please wait" with a turning circle
//! (FUN_004A4560). The circle is a procedural shader (`feov_ramp.fx`), so
//! its look here is AUTHORED.
//!
//! Each screen is its own process (winit allows one event loop per
//! process). Under the supervisor (`event_menu::supervise`) a leaving screen
//! stays black with the loading overlay until the next process shows its
//! first frame, so no desktop shows between them: see [`run_supervised`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::UiTransform;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowPosition};
use std::path::PathBuf;

/// The frame at which the fade is fully black (the animation's last key).
pub const FADE_FRAMES: f32 = 0.6667;
/// `fade_to_black`'s target frame (past the last key: black is held).
pub const TO_BLACK: f32 = 0.8333;
/// `fade_to_clear`'s target frame.
pub const TO_CLEAR: f32 = 0.;

/// Frame-time cap: a load stall does not skip the fade.
const MAX_STEP: f32 = 0.05;

/// The overlay alpha at an animation frame (the BEZIERFLOAT curve).
pub fn alpha(frame: f32) -> f32 {
    let s = (frame / FADE_FRAMES).clamp(0., 1.);
    s * s * (3. - 2. * s)
}

/// The original's animation speeds (`anim_suffix`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    /// `overlay_fade_off`, `overlay_fade_on_hh`: 1.0 s.
    Normal,
    /// `_quick` and `_quickest`: 0.5 s.
    Quick,
}

impl Speed {
    pub fn seconds(self) -> f32 {
        match self {
            Speed::Normal => 1.,
            Speed::Quick => 0.5,
        }
    }
}

/// What the app does once the screen is black after [`Fade::leave`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Close the app (exit code 0).
    Close,
    /// Exit with this code for the supervisor (Android: restart the
    /// activity, which reads its own markers).
    Code(u8),
}

/// How a screen starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// Black with the loading overlay until the pipeline warm-up is done,
    /// then `overlay_fade_off` (the race intro's `c_pre_intro_race_fade_off`).
    Race,
    /// Black, then `overlay_fade_off` (a front-end screen).
    Screen,
}

/// The overlay fade's state.
#[derive(Resource, Debug)]
pub struct Fade {
    from: f32,
    to: f32,
    seconds: f32,
    elapsed: f32,
    /// The loading text and circle show over the black.
    pub loading: bool,
    start: Option<Start>,
    leaving: Option<Next>,
    /// The go marker was written; waiting for the next process.
    handed: bool,
    waited: f32,
    frames: u32,
}

impl Fade {
    /// Black at rest (the overlay's default frame).
    pub fn black(start: Option<Start>) -> Self {
        Self {
            from: FADE_FRAMES,
            to: FADE_FRAMES,
            seconds: 0.,
            elapsed: 0.,
            loading: start == Some(Start::Race),
            start,
            leaving: None,
            handed: false,
            waited: 0.,
            frames: 0,
        }
    }

    pub fn frame(&self) -> f32 {
        if self.seconds <= 0. {
            return self.to;
        }
        let t = (self.elapsed / self.seconds).min(1.);
        self.from + (self.to - self.from) * t
    }

    pub fn alpha(&self) -> f32 {
        alpha(self.frame())
    }

    pub fn done(&self) -> bool {
        self.elapsed >= self.seconds
    }

    /// Fully black and not moving.
    pub fn is_black(&self) -> bool {
        self.done() && self.alpha() >= 1.
    }

    fn play(&mut self, to: f32, speed: Speed, event: &str, reason: &str) {
        self.from = self.frame();
        self.to = to;
        self.seconds = speed.seconds();
        self.elapsed = 0.;
        log_line!(
            "screen_transition version=1 event={event} seconds={:.2} reason={reason}",
            self.seconds
        );
    }

    pub fn fade_to_black(&mut self, speed: Speed, reason: &str) {
        self.play(TO_BLACK, speed, "fade_to_black", reason);
    }

    pub fn fade_to_clear(&mut self, speed: Speed, reason: &str) {
        self.loading = false;
        self.play(TO_CLEAR, speed, "fade_to_clear", reason);
    }

    /// Fades to black, shows the loading overlay, then does `next`.
    /// Repeats while leaving are ignored.
    pub fn leave(&mut self, next: Next, speed: Speed, reason: &str) {
        if self.leaving.is_some() {
            return;
        }
        self.leaving = Some(next);
        self.start = None;
        self.fade_to_black(speed, reason);
    }

    pub fn leaving(&self) -> bool {
        self.leaving.is_some()
    }

    /// Where a running fade out leads (`None` while not leaving).
    pub fn next(&self) -> Option<Next> {
        self.leaving
    }

    fn advance(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt.min(MAX_STEP)).min(self.seconds.max(0.));
    }
}

/// The text the loading overlay shows (language id `loading`).
#[derive(Resource)]
struct LoadingText(String);

#[derive(Component)]
struct Overlay;
#[derive(Component)]
struct Loading;
#[derive(Component)]
struct Circle;

/// Installs the overlay on a windowed app: the screen starts black and
/// `start` says when it clears. `loading` is the loading overlay's text.
pub fn install(app: &mut App, loading: &str, start: Start) {
    app.insert_resource(Fade::black(Some(start)))
        .insert_resource(LoadingText(loading.to_owned()))
        .add_systems(First, finish)
        .add_systems(Update, (advance, (spawn, present).chain()).chain())
        .add_systems(Last, handoff_ready);
}

/// Leaving an event (results Continue, Retire, finishing a test drive):
/// with the overlay installed it fades to black first (`overlay_fade_on_hh`,
/// 1.0 s), otherwise it leaves at once (offscreen and headless runs).
#[derive(SystemParam)]
pub struct Leave<'w> {
    fade: Option<ResMut<'w, Fade>>,
    exit: MessageWriter<'w, AppExit>,
    from_menu: Option<Res<'w, crate::event_menu::FromMenu>>,
}

impl Leave<'_> {
    /// Back to the menu when the race came from it, else closes the app.
    pub fn event(&mut self) {
        let Some(fade) = self.fade.as_mut() else {
            crate::event_menu::leave(&mut self.exit, self.from_menu.is_some());
            return;
        };
        if fade.leaving() {
            return;
        }
        let next = if self.from_menu.is_some() {
            log_line!("drive_event_menu version=1 event=back_to_menu");
            Next::Code(crate::event_menu::BACK_TO_MENU as u8)
        } else {
            Next::Close
        };
        fade.leave(next, Speed::Normal, "leave_event");
    }

    /// A fade out is running: menus take no more input.
    pub fn leaving(&self) -> bool {
        self.fade.as_ref().is_some_and(|f| f.leaving())
    }
}

fn exit_with(next: Next, exit: &mut MessageWriter<AppExit>) {
    match next {
        Next::Close => {
            exit.write(AppExit::Success);
        }
        Next::Code(_code) => {
            #[cfg(target_os = "android")]
            crate::android::relaunch();
            #[cfg(not(target_os = "android"))]
            exit.write(AppExit::from_code(_code));
        }
    }
}

fn advance(time: Res<Time<Real>>, mut fade: ResMut<Fade>, warmup: Option<Res<crate::pipeline_warmup::Warmup>>) {
    fade.frames += 1;
    fade.advance(time.delta_secs());
    // A supervised screen clears after its window shows (handoff_ready).
    let shown = fade.frames > SHOW_FRAME + 1;
    match fade.start {
        Some(Start::Race) if shown && warmup.is_none_or(|w| w.ready()) => {
            fade.start = None;
            fade.fade_to_clear(Speed::Normal, "race_start");
        }
        Some(Start::Screen) if shown => {
            fade.start = None;
            fade.fade_to_clear(Speed::Normal, "screen_open");
        }
        _ => {}
    }
}

/// Black after [`Fade::leave`]: shows the loading overlay, hands off to the
/// next process when supervised, then exits. In `First`, so the exit is seen
/// by systems in `Last` of the same frame (`career_screen::write_result`).
fn finish(
    time: Res<Time<Real>>,
    mut fade: ResMut<Fade>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(next) = fade.leaving else {
        return;
    };
    if !fade.is_black() {
        return;
    }
    if !fade.loading {
        fade.loading = true;
        log_line!("screen_transition version=1 event=loading_on");
    }
    let go = std::env::var_os(GO_ENV).map(PathBuf::from);
    let ready = std::env::var_os(READY_ENV).map(PathBuf::from);
    let (Some(go), Some(ready), Next::Code(code)) = (go, ready, next) else {
        log_line!("screen_transition version=1 event=exit next={next:?}");
        exit_with(next, &mut exit);
        fade.leaving = None;
        return;
    };
    if !fade.handed {
        fade.handed = true;
        let at = window.iter().next().and_then(|w| match w.position {
            WindowPosition::At(p) => Some(format!(" {} {}", p.x, p.y)),
            _ => None,
        });
        let _ = std::fs::remove_file(&ready);
        if let Err(error) = std::fs::write(&go, format!("{code}{}\n", at.unwrap_or_default())) {
            warn!("hand-off marker not written, exiting at once: {error}");
            exit_with(next, &mut exit);
            fade.leaving = None;
            return;
        }
        allow_foreground();
        log_line!("screen_transition version=1 event=handoff code={code}");
        return;
    }
    fade.waited += time.delta_secs();
    // The next process shows its first frame; a stuck one is the
    // supervisor's to stop (it ends this screen when the next one exits).
    if ready.is_file() || fade.waited > HANDOFF_LIMIT {
        log_line!(
            "screen_transition version=1 event=exit next={next:?} waited={:.2}",
            fade.waited
        );
        exit_with(next, &mut exit);
        fade.leaving = None;
    }
}

/// The longest a screen waits for the next one (its load) before exiting.
const HANDOFF_LIMIT: f32 = 120.;

/// Lets the next process bring its window to the front (the foreground
/// lock otherwise keeps it behind this one). A per-process permission, no
/// system setting.
fn allow_foreground() {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn AllowSetForegroundWindow(process: u32) -> i32;
        }
        const ASFW_ANY: u32 = u32::MAX;
        // SAFETY: plain Win32 call without pointers.
        unsafe {
            AllowSetForegroundWindow(ASFW_ANY);
        }
    }
}

fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<crate::ViewerCamera>>,
    text: Res<LoadingText>,
    mut done: Local<bool>,
) {
    let Some(camera) = camera.iter().next().filter(|_| !*done) else {
        return;
    };
    *done = true;
    commands
        .spawn((
            Overlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.),
                height: percent(100.),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            // Above everything, the warm-up cover included.
            GlobalZIndex(i32::MAX),
            UiTargetCamera(camera),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Loading,
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(56.),
                        bottom: px(44.),
                        align_items: AlignItems::Center,
                        column_gap: px(16.),
                        ..default()
                    },
                    Visibility::Hidden,
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(text.0.clone()),
                        TextFont {
                            font_size: FontSize::Px(20.),
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.87, 0.9)),
                    ));
                    row.spawn((
                        Circle,
                        Node {
                            width: px(26.),
                            height: px(26.),
                            border: UiRect::all(px(3.)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BorderColor {
                            top: Color::NONE,
                            right: Color::srgb(0.95, 0.55, 0.1),
                            bottom: Color::srgb(0.95, 0.55, 0.1),
                            left: Color::srgb(0.95, 0.55, 0.1),
                        },
                        UiTransform::default(),
                    ));
                });
        });
}

/// One turn of the circle in seconds (AUTHORED).
const TURN_SECONDS: f32 = 0.9;

#[allow(clippy::type_complexity)]
fn present(
    fade: Res<Fade>,
    time: Res<Time<Real>>,
    mut overlay: Query<(&mut BackgroundColor, &mut Visibility), With<Overlay>>,
    mut loading: Query<&mut Visibility, (With<Loading>, Without<Overlay>)>,
    mut circle: Query<&mut UiTransform, With<Circle>>,
) {
    let a = fade.alpha();
    for (mut color, mut visibility) in &mut overlay {
        color.0 = Color::BLACK.with_alpha(a);
        // Hidden when clear, so the screen under it takes the mouse.
        visibility.set_if_neq(if a > 0. || fade.loading {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for mut visibility in &mut loading {
        visibility.set_if_neq(if fade.loading {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if fade.loading {
        let turn = (time.elapsed_secs() / TURN_SECONDS).fract();
        for mut transform in &mut circle {
            transform.rotation = Rot2::turn_fraction(turn);
        }
    }
}

// ---- Hand-off between processes -------------------------------------------

/// The file a supervised screen writes when it leaves (`CODE [X Y]`: its exit
/// code and window position); the supervisor then starts the next process.
pub const GO_ENV: &str = "GRID_HANDOFF_GO";
/// The file a supervised process writes once its window shows; the leaving
/// screen then exits. A process started with it opens its window hidden.
pub const READY_ENV: &str = "GRID_HANDOFF_READY";
/// `X,Y`: where the previous screen's window was.
pub const AT_ENV: &str = "GRID_WINDOW_AT";
/// Set on a process started by the hand-off (it starts hidden).
const HANDED_ENV: &str = "GRID_HANDOFF_SHOW";

/// The frame on which a hidden window shows (after its first black frames).
const SHOW_FRAME: u32 = 3;

/// The window of a screen that installs the fade (`fades`): where the
/// previous screen's window was, and hidden until its first (black) frames
/// are drawn, so the desktop never shows the system's blank window. A
/// window without the fade opens as before.
pub fn window(mut window: Window, fades: bool) -> Window {
    if let Some(at) = std::env::var(AT_ENV).ok().and_then(|v| parse_at(&v)) {
        window.position = WindowPosition::At(at);
    } else if std::env::var_os(READY_ENV).is_some() {
        window.position = WindowPosition::Centered(MonitorSelection::Primary);
    }
    if std::env::var_os(HANDED_ENV).is_some() || (fades && cfg!(not(target_os = "android"))) {
        window.visible = false;
    }
    window
}

fn parse_at(value: &str) -> Option<IVec2> {
    let (x, y) = value.split_once(',')?;
    Some(IVec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Shows a hidden window after its first frames; in a process the hand-off
/// started, then writes the ready marker the leaving screen waits for.
fn handoff_ready(mut frames: Local<u32>, mut window: Query<&mut Window, With<PrimaryWindow>>) {
    *frames += 1;
    if *frames == SHOW_FRAME {
        for mut window in &mut window {
            if !window.visible {
                window.visible = true;
                window.focused = true;
            }
        }
    } else if std::env::var_os(HANDED_ENV).is_some()
        && *frames == SHOW_FRAME + 1
        && let Some(ready) = std::env::var_os(READY_ENV)
    {
        if let Err(error) = std::fs::write(&ready, b"ready\n") {
            warn!("hand-off ready marker not written: {error}");
        }
        log_line!("screen_transition version=1 event=handoff_ready");
    }
}

/// Runs supervised processes with the hand-off: `first` runs; when a
/// process exits with a code (or hands off with it), `next(code)` gives the
/// next process or `None` to return that code. A handing-off process stays
/// black until the next one shows its window. Returns the last exit code.
#[cfg(not(target_os = "android"))]
pub fn run_supervised(
    first: std::process::Command,
    at: Option<String>,
    mut next: impl FnMut(i32) -> Option<std::process::Command>,
) -> std::io::Result<i32> {
    use std::process::{Child, Command};
    use std::time::Duration;
    let base = std::env::temp_dir().join(format!("grid-handoff-{}", std::process::id()));
    let go = base.with_extension("go");
    let ready = base.with_extension("ready");
    let clear = || {
        let _ = std::fs::remove_file(&go);
        let _ = std::fs::remove_file(&ready);
    };
    let start = |mut command: Command, at: &Option<String>, handed: bool| -> std::io::Result<Child> {
        command.env(GO_ENV, &go).env(READY_ENV, &ready).env_remove(HANDED_ENV);
        match at {
            Some(at) => command.env(AT_ENV, at),
            None => command.env_remove(AT_ENV),
        };
        if handed {
            command.env(HANDED_ENV, "1");
        }
        command.spawn()
    };
    clear();
    let mut current = start(first, &at, false)?;
    let result = loop {
        // The current process exits, or hands off.
        let marker = loop {
            if let Some(status) = current.try_wait()? {
                break Err(status.code().unwrap_or(1));
            }
            if let Ok(text) = std::fs::read_to_string(&go)
                && let Some((code, at)) = parse_go(&text)
            {
                break Ok((code, at));
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        match marker {
            Err(code) => {
                // Exited without a hand-off (Quit, a crash, an old build).
                clear();
                match next(code) {
                    Some(command) => current = start(command, &None, false)?,
                    None => break code,
                }
            }
            Ok((code, at)) => {
                let _ = std::fs::remove_file(&go);
                let Some(command) = next(code) else {
                    // Nothing follows: the screen's own exit decides.
                    let status = current.wait()?;
                    break status.code().unwrap_or(1);
                };
                let mut following = start(command, &at, true)?;
                // The leaving screen exits after the ready marker; if the
                // next process ends first, the screen is stopped.
                loop {
                    if current.try_wait()?.is_some() {
                        break;
                    }
                    if following.try_wait()?.is_some() {
                        let _ = current.kill();
                        let _ = current.wait();
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                let _ = std::fs::remove_file(&ready);
                current = following;
            }
        }
    };
    clear();
    Ok(result)
}

/// `CODE [X Y]` from a go marker: the code and the window position.
fn parse_go(text: &str) -> Option<(i32, Option<String>)> {
    let mut words = text.split_whitespace();
    let code = words.next()?.parse().ok()?;
    let at = match (words.next(), words.next()) {
        (Some(x), Some(y)) => Some(format!("{x},{y}")),
        _ => None,
    };
    Some((code, at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_is_the_original_smoothstep() {
        assert_eq!(alpha(0.), 0.);
        assert_eq!(alpha(FADE_FRAMES), 1.);
        assert_eq!(alpha(TO_BLACK), 1.);
        assert!((alpha(FADE_FRAMES / 2.) - 0.5).abs() < 1e-6);
        // Slow at both ends (the Bezier controls at 1/3 and 2/3 of the span).
        assert!(alpha(FADE_FRAMES * 0.1) < 0.05);
        assert!(alpha(FADE_FRAMES * 0.9) > 0.95);
    }

    #[test]
    fn fades_follow_the_original_durations() {
        let mut fade = Fade::black(None);
        assert!(fade.is_black());
        fade.fade_to_clear(Speed::Normal, "test");
        for _ in 0..19 {
            fade.advance(0.05);
        }
        assert!(!fade.done() && fade.alpha() > 0.);
        fade.advance(0.05);
        assert!(fade.done() && fade.alpha() == 0.);
        // To black at quickest speed: black after 0.5 s; the visible part
        // ends at 0.6667 / 0.8333 of the play.
        // Fully dark from 0.4 s, done at 0.5 s.
        fade.fade_to_black(Speed::Quick, "test");
        for _ in 0..9 {
            fade.advance(0.05);
        }
        assert!(fade.alpha() >= 1. && !fade.done());
        fade.advance(0.06);
        assert!(fade.is_black());
    }

    #[test]
    fn a_stall_does_not_skip_the_fade() {
        let mut fade = Fade::black(None);
        fade.fade_to_clear(Speed::Normal, "test");
        fade.advance(3.);
        assert!(fade.alpha() > 0.9);
    }

    #[test]
    fn leaving_ignores_repeats() {
        let mut fade = Fade::black(None);
        fade.fade_to_clear(Speed::Quick, "test");
        fade.advance(0.5);
        fade.leave(Next::Close, Speed::Normal, "test");
        fade.advance(0.05);
        let frame = fade.frame();
        fade.leave(Next::Code(21), Speed::Quick, "test");
        assert_eq!(fade.leaving, Some(Next::Close));
        assert_eq!(fade.frame(), frame);
    }

    #[test]
    fn go_markers_parse() {
        assert_eq!(parse_go("20\n"), Some((20, None)));
        assert_eq!(parse_go("21 -8 120\n"), Some((21, Some("-8,120".into()))));
        assert_eq!(parse_go(""), None);
        assert_eq!(parse_at("-8,120"), Some(IVec2::new(-8, 120)));
    }
}
